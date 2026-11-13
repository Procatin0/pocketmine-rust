//! Java Edition (1.18+) terrain-noise port from `niduy/minecraft-world-generator`.
//!
//! Compared against `NormalGenerator` (`normal.rs`), the default Bedrock /
//! PocketMine-MP world generator of this crate:
//!
//! | Aspect            | `normal.rs` (PocketMine `Normal`)      | This module (Java `BiomeNoise`) |
//! |-------------------|----------------------------------------|---------------------------------|
//! | Noise             | 3D Simplex noise, 4 octaves, `p=0.25`, `exp=1/32` (`simplex.rs`) | 6 x `DoublePerlinNoise` (classic Perlin + 2 octave sets, 2nd at `337/331`) |
//! | PRNG (seed)       | XorShift128 (`random.rs`)              | Xoroshiro128+ `setSeed` |
//! | Relief            | `min/max` heights per biome, Gaussian-smoothed | `Spline` tree over `(continentalness, erosion, peaks, weirdness)` |
//! | Solidity          | `noise - slope > 0`                    | `density = 1 - 4y/128 - 83/160 + offset > 0` |
//! | Sea level         | 62                                     | 63 (`JAVA_SEA_LEVEL`) |
//! | Biomes            | `BiomeSelector` (temp/rainfall Simplex) | `BIOME_TREE` nearest node — ported in `bds_biome_tree.rs`, wired up by `bds_terrain.rs` |
//! | Blocks            | Stone/water + GroundCover/ores/trees   | Not generated here (niduy only renders a biome preview at y=255) |
//!
//! `NormalGenerator` stays the default world generator: it is the only one
//! with Bedrock parity (e.g. seed 0, chunk (0,0) -> water at Y=62, section 7
//! fluidCount 812). `JavaTerrain` is an opt-in, side-by-side height model,
//! useful to preview or cross-check Java-Edition-shaped relief. It does not
//! implement `WorldGenerator` and does not touch streaming/chunks.

use std::fmt;

pub const JAVA_SEA_LEVEL: i32 = 63;
pub const BEDROCK_NORMAL_SEA_LEVEL: i32 = 62;

fn lerp(part: f64, from: f64, to: f64) -> f64 {
    from + part * (to - from)
}

// ---------------------------------------------------------------------------
// Xoroshiro128+ (niduy `xSetSeed` / `xNextLong` / `xNextInt` / `xNextDouble`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Xoroshiro128Plus {
    low: u64,
    high: u64,
}

impl Xoroshiro128Plus {
    pub fn from_seed(seed: u64) -> Self {
        const XL: u64 = 0x9e3779b97f4a7c15;
        const XH: u64 = 0x6a09e667f3bcc909;
        const A: u64 = 0xbf58476d1ce4e5b9;
        const B: u64 = 0x94d049bb133111eb;
        let mut l = seed ^ XH;
        let mut h = l.wrapping_add(XL);
        l = (l ^ (l >> 30)).wrapping_mul(A);
        h = (h ^ (h >> 30)).wrapping_mul(A);
        l = (l ^ (l >> 27)).wrapping_mul(B);
        h = (h ^ (h >> 27)).wrapping_mul(B);
        l ^= l >> 31;
        h ^= h >> 31;
        Self { low: l, high: h }
    }

    pub fn next_long(&mut self) -> u64 {
        let n = self
            .low
            .wrapping_add(self.high)
            .rotate_left(17)
            .wrapping_add(self.low);
        let x = self.high ^ self.low;
        self.low = self.low.rotate_left(49) ^ x ^ (x << 21);
        self.high = x.rotate_left(28);
        n
    }

    /// Mirrors niduy `xNextInt`: `r = (nextLong & 0xffffffff) * n`,
    /// `return r >> 32`. The TS rejection loop compares against
    /// `(~n + 1) % n`, which is always `-0`, so it never triggers;
    /// intentionally omitted (fidelity note, not a simplification).
    pub fn next_int(&mut self, n: i32) -> i32 {
        let r = u128::from((self.next_long() & 0xffff_ffff) as u32)
            .wrapping_mul(n as u32 as u128);
        ((r >> 32) as u32) as i32
    }

    pub fn next_double(&mut self) -> f64 {
        ((self.next_long() >> 11) as f64) * 1.1102230246251565e-16
    }
}

impl fmt::Display for Xoroshiro128Plus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Xoroshiro({:#x}, {:#x})", self.low, self.high)
    }
}

// ---------------------------------------------------------------------------
// PerlinNoise / DoublePerlinNoise
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PerlinNoise {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub perm: [usize; 512],
    pub amplitude: f64,
    pub lacunarity: f64,
}

impl Default for PerlinNoise {
    fn default() -> Self {
        Self {
            a: 0.0,
            b: 0.0,
            c: 0.0,
            perm: [0; 512],
            amplitude: 0.0,
            lacunarity: 0.0,
        }
    }
}

impl PerlinNoise {
    pub fn init(&mut self, xr: &mut Xoroshiro128Plus) {
        self.a = xr.next_double() * 256.0;
        self.b = xr.next_double() * 256.0;
        self.c = xr.next_double() * 256.0;
        self.amplitude = 1.0;
        self.lacunarity = 1.0;
        for i in 0..256 {
            self.perm[i] = i;
        }
        for i in 0..256 {
            let j = xr.next_int(256 - i as i32) as usize + i;
            self.perm.swap(i, j);
            self.perm[i + 256] = self.perm[i];
        }
    }

    pub fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut d1 = x + self.a;
        let mut d2 = y + self.b;
        let mut d3 = z + self.c;
        let i1 = d1.floor() as i32;
        let i2 = d2.floor() as i32;
        let i3 = d3.floor() as i32;
        d1 -= i1 as f64;
        d2 -= i2 as f64;
        d3 -= i3 as f64;
        let t1 = d1 * d1 * d1 * (d1 * (d1 * 6.0 - 15.0) + 10.0);
        let t2 = d2 * d2 * d2 * (d2 * (d2 * 6.0 - 15.0) + 10.0);
        let t3 = d3 * d3 * d3 * (d3 * (d3 * 6.0 - 15.0) + 10.0);
        let i = (i1 & 0xff) as usize;
        let j = (i2 & 0xff) as usize;
        let k = (i3 & 0xff) as usize;
        let a1 = self.perm[i] + j;
        let b1 = self.perm[i + 1] + j;
        let a2 = self.perm[a1] + k;
        let a3 = self.perm[a1 + 1] + k;
        let b2 = self.perm[b1] + k;
        let b3 = self.perm[b1 + 1] + k;
        let mut l1 = get_indexed_lerp(self.perm[a2], d1, d2, d3);
        let l2 = get_indexed_lerp(self.perm[b2], d1 - 1.0, d2, d3);
        let mut l3 = get_indexed_lerp(self.perm[a3], d1, d2 - 1.0, d3);
        let l4 = get_indexed_lerp(self.perm[b3], d1 - 1.0, d2 - 1.0, d3);
        let mut l5 = get_indexed_lerp(self.perm[a2 + 1], d1, d2, d3 - 1.0);
        let l6 = get_indexed_lerp(self.perm[b2 + 1], d1 - 1.0, d2, d3 - 1.0);
        let mut l7 = get_indexed_lerp(self.perm[a3 + 1], d1, d2 - 1.0, d3 - 1.0);
        let l8 = get_indexed_lerp(self.perm[b3 + 1], d1 - 1.0, d2 - 1.0, d3 - 1.0);
        l1 = lerp(t1, l1, l2);
        l3 = lerp(t1, l3, l4);
        l5 = lerp(t1, l5, l6);
        l7 = lerp(t1, l7, l8);
        l1 = lerp(t2, l1, l3);
        l5 = lerp(t2, l5, l7);
        lerp(t3, l1, l5)
    }
}

fn get_indexed_lerp(idx: usize, a: f64, b: f64, c: f64) -> f64 {
    match idx & 0xf {
        0 => a + b,
        1 => -a + b,
        2 => a - b,
        3 => -a - b,
        4 => a + c,
        5 => -a + c,
        6 => a - c,
        7 => -a - c,
        8 => b + c,
        9 => -b + c,
        10 => b - c,
        11 => -b - c,
        12 => a + b,
        13 => -b + c,
        14 => -a + b,
        15 => -b - c,
        _ => 0.0,
    }
}

#[derive(Debug, Clone, Default)]
pub struct DoublePerlinNoise {
    pub amplitude: f64,
    pub octaves1: Vec<PerlinNoise>,
    pub octaves2: Vec<PerlinNoise>,
}

impl DoublePerlinNoise {
    pub fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        const F: f64 = 337.0 / 331.0;
        let mut v = 0.0;
        for p in &self.octaves1 {
            v += p.amplitude * p.sample(x * p.lacunarity, y * p.lacunarity, z * p.lacunarity);
        }
        for p in &self.octaves2 {
            v += p.amplitude
                * p.sample(x * F * p.lacunarity, y * F * p.lacunarity, z * F * p.lacunarity);
        }
        v * self.amplitude
    }
}

const MD5_OCTAVE_N: [[u64; 2]; 13] = [
    [0xb198de63a8012672, 0x7b84cad43ef7b5a8],
    [0x0fd787bfbc403ec3, 0x74a4a31ca21b48b8],
    [0x36d326eed40efeb2, 0x5be9ce18223c636a],
    [0x082fe255f8be6631, 0x4e96119e22dedc81],
    [0x0ef68ec68504005e, 0x48b6bf93a2789640],
    [0xf11268128982754f, 0x257a1d670430b0aa],
    [0xe51c98ce7d1de664, 0x5f9478a733040c45],
    [0x6d7b49e7e429850a, 0x2e3063c622a24777],
    [0xbd90d5377ba1b762, 0xc07317d419a7548d],
    [0x53d39c6752dac858, 0xbcd1c5a80ab65b3e],
    [0xb4a24d7a84e7677b, 0x023ff9668e89b5c4],
    [0xdffa22b534c5f608, 0xb9b67517d3665ca9],
    [0xd50708086cef4d7c, 0x6e1651ecc7f43309],
];

const LACUNA_INI: [f64; 13] = [
    1.0,
    0.5,
    0.25,
    1.0 / 8.0,
    1.0 / 16.0,
    1.0 / 32.0,
    1.0 / 64.0,
    1.0 / 128.0,
    1.0 / 256.0,
    1.0 / 512.0,
    1.0 / 1024.0,
    1.0 / 2048.0,
    1.0 / 4096.0,
];

const PERSIST_INI: [f64; 10] = [
    0.0,
    1.0,
    2.0 / 3.0,
    4.0 / 7.0,
    8.0 / 15.0,
    16.0 / 31.0,
    32.0 / 63.0,
    64.0 / 127.0,
    128.0 / 255.0,
    256.0 / 511.0,
];

const AMP_INI: [f64; 10] = [
    0.0,
    5.0 / 6.0,
    10.0 / 9.0,
    15.0 / 12.0,
    20.0 / 15.0,
    25.0 / 18.0,
    30.0 / 21.0,
    35.0 / 24.0,
    40.0 / 27.0,
    45.0 / 30.0,
];

fn octave_init(
    xr: &mut Xoroshiro128Plus,
    amplitudes: &[f64],
    omin: i32,
    length: usize,
    nmax: i32,
) -> Vec<PerlinNoise> {
    let mut lacuna = LACUNA_INI[(-omin) as usize];
    let mut persist = PERSIST_INI[length];
    let x_low = xr.next_long();
    let x_high = xr.next_long();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < length && (nmax < 0 || out.len() as i32 != nmax) {
        if amplitudes[i] != 0.0 {
            let mut octave_xr = Xoroshiro128Plus {
                low: x_low ^ MD5_OCTAVE_N[(12 + omin) as usize + i][0],
                high: x_high ^ MD5_OCTAVE_N[(12 + omin) as usize + i][1],
            };
            let mut p = PerlinNoise::default();
            p.init(&mut octave_xr);
            p.amplitude = amplitudes[i] * persist;
            p.lacunarity = lacuna;
            out.push(p);
        }
        lacuna *= 2.0;
        persist *= 0.5;
        i += 1;
    }
    out
}

fn double_perlin_init(
    xr: &mut Xoroshiro128Plus,
    amplitudes: &[f64],
    omin: i32,
    length: usize,
    nmax: i32,
) -> DoublePerlinNoise {
    let (na, nb) = if nmax > 0 {
        let na = ((nmax + 1) as u32 >> 1) as i32;
        (na, nmax - na)
    } else {
        (-1, -1)
    };
    let oct1 = octave_init(xr, amplitudes, omin, length, na);
    let oct2 = octave_init(xr, amplitudes, omin, length, nb);
    // niduy trims trailing zeros then leading zeros, shrinking `length`
    // before indexing `amp_ini` (e.g. Temperature [1.5,0,1,0,0,0] -> 3 -> 1.25).
    let mut effective_len = length;
    while effective_len > 0 && amplitudes[effective_len - 1] == 0.0 {
        effective_len -= 1;
    }
    let mut lead = 0usize;
    while lead < effective_len && amplitudes[lead] == 0.0 {
        effective_len -= 1;
        lead += 1;
    }
    DoublePerlinNoise {
        amplitude: AMP_INI[effective_len],
        octaves1: oct1,
        octaves2: oct2,
    }
}

// ---------------------------------------------------------------------------
// Spline tree (terrain density offset)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Spline {
    Fixed(f64),
    Curve { ty: u8, points: Vec<SplinePoint> },
}

#[derive(Debug, Clone)]
pub struct SplinePoint {
    /// F32-quantized like the TS `Float32Array` fields: every `loc` and
    /// `der` written by `addSplineValue` is rounded to f32.
    pub loc: f32,
    pub value: Spline,
    pub der: f32,
}

fn p(loc: f64, value: Spline, der: f64) -> SplinePoint {
    SplinePoint {
        loc: loc as f32,
        value,
        der: der as f32,
    }
}

pub fn eval_spline(vals: &[f64], sp: &Spline) -> f64 {
    match sp {
        Spline::Fixed(v) => *v,
        Spline::Curve { ty, points } => {
            assert!(!points.is_empty() && points.len() < 12, "invalid spline");
            let f = vals[*ty as usize];
            let mut i = points.len();
            for (idx, pt) in points.iter().enumerate() {
                if (pt.loc as f64) >= f {
                    i = idx;
                    break;
                }
            }
            if i == 0 || i == points.len() {
                let idx = if i == 0 { 0 } else { i - 1 };
                let pt = &points[idx];
                eval_spline(vals, &pt.value) + (pt.der as f64) * (f - (pt.loc as f64))
            } else {
                let g = points[i - 1].loc as f64;
                let h = points[i].loc as f64;
                let k = (f - g) / (h - g);
                let l = points[i - 1].der as f64;
                let m = points[i].der as f64;
                let n = eval_spline(vals, &points[i - 1].value);
                let o = eval_spline(vals, &points[i].value);
                let pp = l * (h - g) - (o - n);
                let q = -m * (h - g) + (o - n);
                lerp(k, n, o) + k * (1.0 - k) * lerp(k, pp, q)
            }
        }
    }
}

fn offset_value(weirdness: f64, continentalness: f64) -> f64 {
    let f0 = 1.0 - (1.0 - continentalness) * 0.5;
    let f1 = 0.5 * (1.0 - continentalness);
    let f2 = (weirdness + 1.17) * 0.46082947;
    let offset = f2 * f0 - f1;
    if weirdness < -0.7 {
        if offset > -0.2222 { offset } else { -0.2222 }
    } else if offset > 0.0 {
        offset
    } else {
        0.0
    }
}

fn create_flat_offset_spline(o: [f64; 6]) -> Spline {
    let mut l = 0.5 * (o[1] - o[0]);
    if l < o[5] {
        l = o[5];
    }
    let m = 5.0 * (o[2] - o[1]);
    let lmid = if l < m { l } else { m };
    Spline::Curve {
        ty: 2,
        points: vec![
            p(-1.0, Spline::Fixed(o[0]), l),
            p(-0.4, Spline::Fixed(o[1]), lmid),
            p(0.0, Spline::Fixed(o[2]), m),
            p(0.4, Spline::Fixed(o[3]), 2.0 * (o[3] - o[2])),
            p(1.0, Spline::Fixed(o[4]), 0.7 * (o[4] - o[3])),
        ],
    }
}

fn create_spline_38219(continentalness: f64, depth: bool) -> Spline {
    let i = offset_value(-1.0, continentalness);
    let k = offset_value(1.0, continentalness);
    let mut l = 1.0 - (1.0 - continentalness) * 0.5;
    let u = 0.5 * (1.0 - continentalness);
    l = u / (0.46082947 * l) - 1.17;
    if l > -0.65 && l < 1.0 {
        let u2 = offset_value(-0.65, continentalness);
        let pp = offset_value(-0.75, continentalness);
        let q = (pp - i) * 4.0;
        let r = offset_value(l, continentalness);
        let s = (k - r) / (1.0 - l);
        Spline::Curve {
            ty: 2,
            points: vec![
                p(-1.0, Spline::Fixed(i), q),
                p(-0.75, Spline::Fixed(pp), 0.0),
                p(-0.65, Spline::Fixed(u2), 0.0),
                p(l - 0.01, Spline::Fixed(r), 0.0),
                p(l, Spline::Fixed(r), s),
                p(1.0, Spline::Fixed(k), s),
            ],
        }
    } else {
        let uu = (k - i) * 0.5;
        let mut points = Vec::new();
        if depth {
            points.push(p(
                -1.0,
                Spline::Fixed(if i > 0.2 { i } else { 0.2 }),
                0.0,
            ));
            points.push(p(0.0, Spline::Fixed(lerp(0.5, i, k)), uu));
        } else {
            points.push(p(-1.0, Spline::Fixed(i), uu));
        }
        points.push(p(1.0, Spline::Fixed(k), uu));
        Spline::Curve { ty: 2, points }
    }
}

fn create_land_spline(o: [f64; 6], depth: bool) -> Spline {
    let sp1 = create_spline_38219(lerp(o[3], 0.6, 1.5), depth);
    let sp2 = create_spline_38219(lerp(o[3], 0.6, 1.0), depth);
    let sp3 = create_spline_38219(o[3], depth);
    let ih = 0.5 * o[3];
    let sp4 = create_flat_offset_spline([o[0] - 0.15, ih, ih, ih, o[3] * 0.6, 0.5]);
    let sp5 = create_flat_offset_spline([o[0], o[4] * o[3], o[1] * o[3], ih, o[3] * 0.6, 0.5]);
    let sp6 = create_flat_offset_spline([o[0], o[4], o[4], o[1], o[2], 0.5]);
    let sp8 = Spline::Curve {
        ty: 2,
        points: vec![
            p(-1.0, Spline::Fixed(o[0]), 0.0),
            p(-0.4, sp6.clone(), 0.0),
            p(0.0, Spline::Fixed(o[2] + 0.07), 0.0),
        ],
    };
    let sp9 = create_flat_offset_spline([-0.02, o[5], o[5], o[1], o[2], 0.0]);
    let mut points = vec![
        p(-0.85, sp1, 0.0),
        p(-0.7, sp2, 0.0),
        p(-0.4, sp3, 0.0),
        p(-0.35, sp4, 0.0),
        p(-0.1, sp5, 0.0),
        p(0.2, sp6, 0.0),
    ];
    if depth {
        points.push(p(
            0.4,
            create_flat_offset_spline([o[0], o[4], o[4], o[1], o[2], 0.5]),
            0.0,
        ));
        points.push(p(0.45, sp8.clone(), 0.0));
        points.push(p(0.55, sp8, 0.0));
        points.push(p(
            0.58,
            create_flat_offset_spline([o[0], o[4], o[4], o[1], o[2], 0.5]),
            0.0,
        ));
    }
    points.push(p(0.7, sp9, 0.0));
    Spline::Curve { ty: 1, points }
}

fn terrain_spline() -> Spline {
    let sp1 = create_land_spline([-0.15, 0.0, 0.0, 0.1, 0.0, -0.03], false);
    let sp2 = create_land_spline([-0.1, 0.03, 0.1, 0.1, 0.01, -0.03], false);
    let sp3 = create_land_spline([-0.1, 0.03, 0.1, 0.7, 0.01, -0.03], true);
    let sp4 = create_land_spline([-0.05, 0.03, 0.1, 1.0, 0.01, 0.01], true);
    Spline::Curve {
        ty: 0,
        points: vec![
            p(-1.1, Spline::Fixed(0.044), 0.0),
            p(-1.02, Spline::Fixed(-0.2222), 0.0),
            p(-0.51, Spline::Fixed(-0.2222), 0.0),
            p(-0.44, Spline::Fixed(-0.12), 0.0),
            p(-0.18, Spline::Fixed(-0.12), 0.0),
            p(-0.16, sp1.clone(), 0.0),
            p(-0.15, sp1, 0.0),
            p(-0.1, sp2, 0.0),
            p(0.25, sp3, 0.0),
            p(1.0, sp4, 0.0),
        ],
    }
}

// ---------------------------------------------------------------------------
// JavaTerrain (height map + density)
// ---------------------------------------------------------------------------

pub const CLIMATE_TEMPERATURE: usize = 0;
pub const CLIMATE_HUMIDITY: usize = 1;
pub const CLIMATE_CONTINENTALNESS: usize = 2;
pub const CLIMATE_EROSION: usize = 3;
pub const CLIMATE_PEAKS_AND_VALLEYS: usize = 4;
pub const CLIMATE_WEIRDNESS: usize = 5;

#[derive(Debug, Clone)]
pub struct JavaTerrain {
    pub seed: u64,
    pub climate: [DoublePerlinNoise; 6],
    pub spline: Spline,
}

impl JavaTerrain {
    pub fn new(seed: u64) -> Self {
        let mut pxr = Xoroshiro128Plus::from_seed(seed);
        let x_low = pxr.next_long();
        let x_high = pxr.next_long();
        let seeds: [(u64, u64); 6] = [
            (0x5c7e6b29735f0d7f, 0xf7d86f1bbc734988),
            (0x81bb4d22e8dc168e, 0xf1c8b4bea16303cd),
            (0x83886c9d0ae3a662, 0xafa638a61b42e8ad),
            (0xd02491e6058f6fd8, 0x4792512c94c17a80),
            (0x080518cf6af25384, 0x3f3dfb40a54febd5),
            (0xefc8ef4d36102b34, 0x1beeeb324a0f24ea),
        ];
        let params: [&[f64]; 6] = [
            &[1.5, 0.0, 1.0, 0.0, 0.0, 0.0],
            &[1.0, 1.0, 0.0, 0.0, 0.0, 0.0],
            &[1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0],
            &[1.0, 1.0, 0.0, 1.0, 1.0],
            &[1.0, 1.0, 1.0, 0.0],
            &[1.0, 2.0, 1.0, 0.0, 0.0, 0.0],
        ];
        let omin: [i32; 6] = [-10, -8, -9, -9, -3, -7];
        let mut climate = [
            DoublePerlinNoise::default(),
            DoublePerlinNoise::default(),
            DoublePerlinNoise::default(),
            DoublePerlinNoise::default(),
            DoublePerlinNoise::default(),
            DoublePerlinNoise::default(),
        ];
        for i in 0..6 {
            let mut xr = Xoroshiro128Plus {
                low: x_low ^ seeds[i].0,
                high: x_high ^ seeds[i].1,
            };
            let amps = params[i];
            climate[i] = double_perlin_init(&mut xr, amps, omin[i], amps.len(), -1);
        }
        Self {
            seed,
            climate,
            spline: terrain_spline(),
        }
    }

    pub fn sample_climate(&self, index: usize, x: f64, z: f64) -> f64 {
        self.climate[index].sample(x, 0.0, z)
    }

    /// Spline offset used by the density function (without the y term).
    pub fn terrain_offset(&self, x: i32, z: i32) -> f64 {
        let c = self.sample_climate(CLIMATE_CONTINENTALNESS, x as f64, z as f64);
        let e = self.sample_climate(CLIMATE_EROSION, x as f64, z as f64);
        let w = self.sample_climate(CLIMATE_WEIRDNESS, x as f64, z as f64);
        let peaks = -3.0 * ((w.abs() - 0.6666667).abs() - 0.33333334);
        eval_spline(&[c, e, peaks, w], &self.spline) + 0.015
    }

    pub fn density_at(&self, x: i32, y: i32, z: i32) -> f64 {
        1.0 - ((y as f64) * 4.0) / 128.0 - 83.0 / 160.0 + self.terrain_offset(x, z)
    }

    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.density_at(x, y, z) > 0.0
    }

    /// Highest solid y for column (x, z), matching niduy's
    /// `1 - (y<<2)/128 - 83/160 + off > 0`. Solved analytically, then
    /// corrected against `is_solid` so the result is always the true
    /// `density > 0` frontier.
    pub fn height_at(&self, x: i32, z: i32) -> i32 {
        let off = self.terrain_offset(x, z);
        let limit = (1.0 - 83.0 / 160.0 + off) * 32.0;
        let mut h = limit.floor() as i32;
        while self.density_at(x, h + 1, z) > 0.0 {
            h += 1;
        }
        while self.density_at(x, h, z) <= 0.0 {
            h -= 1;
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn xoroshiro_vectors() {
        let mut xr = Xoroshiro128Plus::from_seed(0);
        assert_eq!(xr.next_long(), 0x2a2ca488f66f517e);
        assert_eq!(xr.next_long(), 0xccbc22d72e97c372);
        assert_eq!(xr.next_long(), 0x404e64b826f4b9f4);
        let mut xr = Xoroshiro128Plus::from_seed(12345);
        assert_eq!(xr.next_long(), 0x8f5558a8036890fb);
        assert_eq!(xr.next_long(), 0x725fe51ad193097e);
        assert_eq!(xr.next_long(), 0x3981933fe0893743);
    }

    #[test]
    fn temperature_octave_vectors_seed0() {
        let t = JavaTerrain::new(0);
        let p0 = &t.climate[CLIMATE_TEMPERATURE].octaves1[0];
        assert!(approx(p0.a, 2.165741276509493));
        assert!(approx(p0.b, 229.08346368562928));
        assert!(approx(p0.c, 128.97439321321335));
        assert!(approx(p0.amplitude, 0.7619047619047619));
        assert!((p0.lacunarity - 0.0009765625).abs() < 1e-15);
        assert!(approx(t.climate[CLIMATE_TEMPERATURE].amplitude, 1.25));
        assert_eq!(t.climate[CLIMATE_TEMPERATURE].octaves1.len(), 2);
        assert_eq!(t.climate[CLIMATE_TEMPERATURE].octaves2.len(), 2);
        assert!(approx(t.climate[CLIMATE_CONTINENTALNESS].amplitude, 1.5));
        assert_eq!(t.climate[CLIMATE_CONTINENTALNESS].octaves1.len(), 9);
        assert_eq!(t.climate[CLIMATE_CONTINENTALNESS].octaves2.len(), 9);
        assert!(approx(
            t.climate[CLIMATE_EROSION].amplitude,
            1.3888888888888888
        ));
        assert_eq!(t.climate[CLIMATE_EROSION].octaves1.len(), 4);
        assert!(approx(t.climate[CLIMATE_WEIRDNESS].amplitude, 1.25));
        assert_eq!(t.climate[CLIMATE_WEIRDNESS].octaves1.len(), 3);
    }

    fn climate_vectors(seed: u64, rows: &[(i32, i32, f64, f64, f64, f64, f64, f64, i32)]) {
        let t = JavaTerrain::new(seed);
        for &(x, z, exp_t, exp_h, exp_c, exp_e, exp_w, exp_off, exp_height) in rows {
            let got_t = t.sample_climate(CLIMATE_TEMPERATURE, x as f64, z as f64);
            let got_h = t.sample_climate(CLIMATE_HUMIDITY, x as f64, z as f64);
            let got_c = t.sample_climate(CLIMATE_CONTINENTALNESS, x as f64, z as f64);
            let got_e = t.sample_climate(CLIMATE_EROSION, x as f64, z as f64);
            let got_w = t.sample_climate(CLIMATE_WEIRDNESS, x as f64, z as f64);
            let got_off = t.terrain_offset(x, z);
            let got_hy = t.height_at(x, z);
            assert!(approx(got_t, exp_t), "seed {seed} ({x},{z}) t {got_t} != {exp_t}");
            assert!(approx(got_h, exp_h), "seed {seed} ({x},{z}) h {got_h} != {exp_h}");
            assert!(approx(got_c, exp_c), "seed {seed} ({x},{z}) c {got_c} != {exp_c}");
            assert!(approx(got_e, exp_e), "seed {seed} ({x},{z}) e {got_e} != {exp_e}");
            assert!(approx(got_w, exp_w), "seed {seed} ({x},{z}) w {got_w} != {exp_w}");
            assert!(
                approx(got_off, exp_off),
                "seed {seed} ({x},{z}) off {got_off} != {exp_off}"
            );
            assert_eq!(got_hy, exp_height, "seed {seed} ({x},{z}) height");
        }
    }

    #[test]
    fn climate_vectors_seed0() {
        let rows = [
            (0, 0, 0.12233288281141312, -0.008981807710965708, -0.027065791584939207, -0.10539698186379509, 0.02319205871994339, -0.054341742733010956, 13),
            (1, 0, 0.12356759228847132, -0.01028374800459657, -0.03326221841891166, -0.10545788387320254, 0.006841211145866277, -0.07530074177250153, 12),
            (0, 1, 0.12300308982558478, -0.011206463940579697, -0.033049338272311116, -0.10590712711958672, 0.04446061973418427, -0.03239974835026275, 14),
            (16, 32, 0.1643749677798201, -0.11970473296613685, 0.024435968522547348, -0.034780803434129696, 0.3837994274778671, 0.05369929521880299, 17),
            (-64, 128, 0.1854037786767913, 0.03764810246422903, 0.12720613401282224, 0.21395639014737214, 0.38223235653886223, 0.02971434375824094, 16),
            (100, -200, 0.23827028150181118, 0.039883570891026586, 0.01572542525323763, 0.3699508475181435, 0.3731762829859474, 0.02161996752743733, 16),
        ];
        climate_vectors(0, &rows);
    }

    #[test]
    fn climate_vectors_seed12345() {
        let rows = [
            (0, 0, -0.02473449901678134, 0.6802474455197169, -0.4026159714905887, -0.7345684011563401, 0.1754419660718595, -0.105, 12),
            (1, 0, -0.026021932011689487, 0.6826739990978105, -0.40053392515288255, -0.7348395470379135, 0.1982827745450632, -0.105, 12),
            (0, 1, -0.02492274786266055, 0.6810474221540017, -0.40780845809528365, -0.7374469965906679, 0.15874034455948705, -0.105, 12),
            (16, 32, -0.054097754877763526, 0.6632591769523424, -0.3141916734595667, -0.6987582987190062, 0.27189800188548546, -0.105, 12),
            (-64, 128, -0.004768414246600176, -0.20801594950531815, -0.1805783657709515, -0.6994144988277738, 0.07385507449141948, -0.105, 12),
            (100, -200, -0.2823926455441201, -0.10890292591505085, 0.2194432417651468, -0.09429775295063414, -0.31754553752069037, 0.032882042170266516, 16),
        ];
        climate_vectors(12345, &rows);
    }

    #[test]
    fn climate_vectors_seed_demo() {
        let rows = [
            (0, 0, -0.11628254752257286, 0.3145409267266602, -0.17908349656987588, 0.14139659302694058, -0.1671588502549099, -0.10429180598853852, 12),
            (1, 0, -0.11624222058629205, 0.3156129479463752, -0.18012338599820416, 0.1431378865274046, -0.18821252422403043, -0.105, 12),
            (0, 1, -0.11795245854249632, 0.3143165965005309, -0.17909191923138776, 0.14340594950642632, -0.18929408515224644, -0.10428288217423604, 12),
            (16, 32, -0.16621933713323253, 0.24573457136403307, -0.08838421888751782, 0.19942693768403388, -0.6392127264866785, 0.10879107686624824, 18),
            (-64, 128, -0.37343190541059473, 0.35582798830827256, -0.28385886516021636, 0.16296228767820464, 0.5945011602263622, -0.105, 12),
            (100, -200, 0.27710720996063987, 0.23218499094318712, -0.4724942647863929, -0.09703176189644723, 0.06426239381689582, -0.15062182776471844, 10),
        ];
        climate_vectors(12370816993565, &rows);
    }

    #[test]
    fn height_is_density_frontier() {
        let t = JavaTerrain::new(0);
        for &(x, z) in &[(0, 0), (5, -7), (-33, 41), (128, 256)] {
            let h = t.height_at(x, z);
            assert!(t.is_solid(x, h, z), "({x},{z}) h={h} must be solid");
            assert!(!t.is_solid(x, h + 1, z), "({x},{z}) h+1 must be air");
            let d0 = t.density_at(x, 0, z);
            let d1 = t.density_at(x, 64, z);
            assert!(d0 > d1, "density must decrease with y");
        }
    }

    #[test]
    fn sea_levels_differ_from_bedrock() {
        assert_eq!(JAVA_SEA_LEVEL, 63);
        assert_eq!(BEDROCK_NORMAL_SEA_LEVEL, 62);
        assert_eq!(
            crate::generator::normal::NormalGenerator::SEA_LEVEL,
            BEDROCK_NORMAL_SEA_LEVEL
        );
    }
}
