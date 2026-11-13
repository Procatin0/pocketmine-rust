//! Bedrock Dedicated Server (BDS) Terrain Generator
//!
//! This generator replicates the terrain generation used by Bedrock Edition 1.18+
//! (which is used by BDS). It is based on the same noise-based system as Java Edition
//! but with Bedrock-specific parameters and biome mappings.
//!
//! Key differences from Java Edition:
//! - Sea level: 62 (vs 63 in Java)
//! - Different noise seeds and parameters for continentalness, erosion, etc.
//! - Bedrock biome IDs (0-255 legacy IDs)
//! - Slightly different spline parameters for terrain shape
//!
//! Biome mapping uses Java 1.18+ KD-tree (BIOME_TREE + BIOME_PARAMS) from
//! niduy/minecraft-world-generator / Cubitect/cubiomes for correct biome
//! distribution with realistic sizes.

use crate::block_id;
use crate::chunk::Chunk;
use crate::generator::bds_biome_tree;
use crate::generator::biome::{self, Gaussian};
use crate::generator::caves::CavePopulator;
use crate::generator::manager::{ChunkManager, ChunkManager3x3};
use crate::generator::populators::{
    GroundCoverPopulator, OrePopulator, TallGrassPopulator, TreePopulator, TreeType,
};
use crate::generator::random::Random;
use crate::generator::WorldGenerator;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Columns kept for single-block queries.
const COLUMN_CACHE_CAP: usize = 32;

#[derive(Debug, Default)]
struct ColumnCache {
    entries: HashMap<(i32, i32), Arc<Chunk>>,
    order: VecDeque<(i32, i32)>,
}

/// Bedrock-specific sea level
pub const BDS_SEA_LEVEL: i32 = 62;

/// Noise parameters for Bedrock Edition terrain generation
/// These are derived from the Java Edition parameters but adjusted for Bedrock
mod bds_noise {
    /// Seeds for the 6 climate noises (temperature, humidity, continentalness, erosion, peaks, weirdness)
    /// These differ from Java Edition to produce Bedrock's distinct world shape
    pub const CLIMATE_SEEDS: [(u64, u64); 6] = [
        (0x5c7e6b29735f0d7f, 0xf7d86f1bbc734988), // temperature
        (0x81bb4d22e8dc168e, 0xf1c8b4bea16303cd), // humidity
        (0x83886c9d0ae3a662, 0xafa638a61b42e8ad), // continentalness
        (0xd02491e6058f6fd8, 0x4792512c94c17a80), // erosion
        (0x080518cf6af25384, 0x3f3dfb40a54febd5), // peaks_and_valleys
        (0xefc8ef4d36102b34, 0x1beeeb324a0f24ea), // weirdness
    ];

    /// Amplitude parameters for each climate noise
    pub const CLIMATE_PARAMS: [&[f64]; 6] = [
        &[1.5, 0.0, 1.0, 0.0, 0.0, 0.0],         // temperature
        &[1.0, 1.0, 0.0, 0.0, 0.0, 0.0],         // humidity
        &[1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0], // continentalness
        &[1.0, 1.0, 0.0, 1.0, 1.0],              // erosion
        &[1.0, 1.0, 1.0, 0.0],                   // peaks_and_valleys
        &[1.0, 2.0, 1.0, 0.0, 0.0, 0.0],         // weirdness
    ];

    /// Minimum octave for each climate noise
    pub const CLIMATE_OMIN: [i32; 6] = [-10, -8, -9, -9, -3, -7];
}

/// Bedrock biome IDs (legacy format used by Bedrock Edition)
pub mod bds_biome_ids {
    pub const OCEAN: u8 = 0;
    pub const PLAINS: u8 = 1;
    pub const DESERT: u8 = 2;
    pub const EXTREME_HILLS: u8 = 3;
    pub const FOREST: u8 = 4;
    pub const TAIGA: u8 = 5;
    pub const SWAMPLAND: u8 = 6;
    pub const RIVER: u8 = 7;
    pub const ICE_PLAINS: u8 = 12;
    pub const EXTREME_HILLS_EDGE: u8 = 20;
    pub const BIRCH_FOREST: u8 = 27;
    // Badlands family (not emitted by selector but defined for completeness)
    pub const BADLANDS: u8 = 37;
    pub const WOODED_BADLANDS_PLATEAU: u8 = 38;
    pub const BADLANDS_PLATEAU: u8 = 39;
    // 1.13+ variants
    pub const ERODED_BADLANDS: u8 = 165;
    pub const MODIFIED_WOODED_BADLANDS_PLATEAU: u8 = 166;
    pub const MODIFIED_BADLANDS_PLATEAU: u8 = 167;
}

/// Xoroshiro128+ PRNG (same as JavaTerrain for seed compatibility)
#[derive(Debug, Clone)]
struct Xoroshiro128Plus {
    low: u64,
    high: u64,
}

impl Xoroshiro128Plus {
    fn from_seed(seed: u64) -> Self {
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

    fn next_long(&mut self) -> u64 {
        let n = self.low.wrapping_add(self.high).rotate_left(17).wrapping_add(self.low);
        let x = self.high ^ self.low;
        self.low = self.low.rotate_left(49) ^ x ^ (x << 21);
        self.high = x.rotate_left(28);
        n
    }

    fn next_double(&mut self) -> f64 {
        ((self.next_long() >> 11) as f64) * 1.1102230246251565e-16
    }
}

/// Perlin noise implementation matching Java/Bedrock
#[derive(Debug, Clone)]
struct PerlinNoise {
    a: f64,
    b: f64,
    c: f64,
    perm: [usize; 512],
    amplitude: f64,
    lacunarity: f64,
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
    fn init(&mut self, xr: &mut Xoroshiro128Plus) {
        self.a = xr.next_double() * 256.0;
        self.b = xr.next_double() * 256.0;
        self.c = xr.next_double() * 256.0;
        self.amplitude = 1.0;
        self.lacunarity = 1.0;
        for i in 0..256 {
            self.perm[i] = i;
        }
        for i in 0..256 {
            // niduy `xPerlinInit` / `xNextInt`: `j = (nextLong & 0xffffffff) *
            // n >> 32`. The rejection loop in the TS never fires (see the
            // note on `java_terrain::Xoroshiro128Plus::next_int`); a plain
            // `next_long() % n` here draws from the wrong stream and shifts
            // every climate noise field away from niduy parity.
            let r = ((xr.next_long() & 0xffff_ffff) as u64).wrapping_mul((256 - i) as u64);
            let j = (r >> 32) as usize + i;
            self.perm.swap(i, j);
            self.perm[i + 256] = self.perm[i];
        }
    }

    #[inline]
    fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
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

#[inline]
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

#[inline]
fn lerp(part: f64, from: f64, to: f64) -> f64 {
    from + part * (to - from)
}

/// Double Perlin noise (two sets of octaves with slightly different frequency)
#[derive(Debug, Clone, Default)]
pub struct DoublePerlinNoise {
    amplitude: f64,
    octaves1: Vec<PerlinNoise>,
    octaves2: Vec<PerlinNoise>,
}

impl DoublePerlinNoise {
    #[inline]
    fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
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
    1.0, 0.5, 0.25, 1.0 / 8.0, 1.0 / 16.0, 1.0 / 32.0, 1.0 / 64.0,
    1.0 / 128.0, 1.0 / 256.0, 1.0 / 512.0, 1.0 / 1024.0, 1.0 / 2048.0, 1.0 / 4096.0,
];

const PERSIST_INI: [f64; 10] = [
    0.0, 1.0, 2.0 / 3.0, 4.0 / 7.0, 8.0 / 15.0, 16.0 / 31.0, 32.0 / 63.0,
    64.0 / 127.0, 128.0 / 255.0, 256.0 / 511.0,
];

const AMP_INI: [f64; 10] = [
    0.0, 5.0 / 6.0, 10.0 / 9.0, 15.0 / 12.0, 20.0 / 15.0, 25.0 / 18.0,
    30.0 / 21.0, 35.0 / 24.0, 40.0 / 27.0, 45.0 / 30.0,
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

/// Spline system for terrain density offset
#[derive(Debug, Clone)]
pub enum Spline {
    Fixed(f64),
    Curve { ty: u8, points: Vec<SplinePoint> },
}

#[derive(Debug, Clone)]
pub struct SplinePoint {
    loc: f32,
    value: Spline,
    der: f32,
}

fn p(loc: f64, value: Spline, der: f64) -> SplinePoint {
    SplinePoint {
        loc: loc as f32,
        value,
        der: der as f32,
    }
}

fn eval_spline(vals: &[f64], sp: &Spline) -> f64 {
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
    if l < o[5] { l = o[5]; }
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
            points.push(p(-1.0, Spline::Fixed(if i > 0.2 { i } else { 0.2 }), 0.0));
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

/// Climate indices
const CLIMATE_TEMPERATURE: usize = 0;
const CLIMATE_HUMIDITY: usize = 1;
const CLIMATE_CONTINENTALNESS: usize = 2;
const CLIMATE_EROSION: usize = 3;
const CLIMATE_PEAKS_AND_VALLEYS: usize = 4;
const CLIMATE_WEIRDNESS: usize = 5;

/// Precomputed climate values for a single column
///
/// `temperature`/`humidity` are kept for structural completeness but the raw
/// sampler always leaves them at 0: the terrain offset only consumes
/// continentalness/erosion/peaks/weirdness, and the biome map resamples
/// temperature/humidity at its shifted point.
#[derive(Clone, Copy)]
struct ClimateValues {
    // Temperature/humidity don't affect the 1.18 terrain offset, and the
    // biome sampler resamples every noise at the shifted point, so these two
    // stay on the grid only for completeness of the sampled climate set.
    #[allow(dead_code)]
    temperature: f64,
    #[allow(dead_code)]
    humidity: f64,
    continentalness: f64,
    erosion: f64,
    peaks: f64,
    weirdness: f64,
}

/// BDS Terrain Generator using noise-based generation (1.18+ style)
#[derive(Debug)]
pub struct BDSTerrain {
    pub seed: u64,
    pub climate: [DoublePerlinNoise; 6],
    pub spline: Spline,
    gaussian: Gaussian,
    water_height: i32,
    ground_cover: GroundCoverPopulator,
    ore_populator: OrePopulator,
    emerald_ore: OrePopulator,
    tree_forest: TreePopulator,
    tree_birch: TreePopulator,
    tree_taiga: TreePopulator,
    tree_mountains: TreePopulator,
    tall_grass_plains: TallGrassPopulator,
    tall_grass_forest: TallGrassPopulator,
    tall_grass_taiga: TallGrassPopulator,
    tall_grass_mountains: TallGrassPopulator,
    tall_grass_river: TallGrassPopulator,
    tall_grass_ocean: TallGrassPopulator,
    tall_grass_ice_plains: TallGrassPopulator,
    cave_populator: CavePopulator,
    spawn_pos: (i32, i32, i32),
    column_cache: Mutex<ColumnCache>,
}

impl BDSTerrain {
    pub fn new(seed: u64) -> Self {
        // Initialize climate noises
        let mut pxr = Xoroshiro128Plus::from_seed(seed);
        let x_low = pxr.next_long();
        let x_high = pxr.next_long();
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
                low: x_low ^ bds_noise::CLIMATE_SEEDS[i].0,
                high: x_high ^ bds_noise::CLIMATE_SEEDS[i].1,
            };
            let amps = bds_noise::CLIMATE_PARAMS[i];
            climate[i] = double_perlin_init(&mut xr, amps, bds_noise::CLIMATE_OMIN[i], amps.len(), -1);
        }

        let _random = Random::new(seed as i64);
        let gaussian = Gaussian::new(2);

        let ground_cover = GroundCoverPopulator::new();
        let ore_populator = OrePopulator::new_overworld();
        let emerald_ore = OrePopulator::new_emerald();

        let tree_forest = TreePopulator::new(TreeType::Oak, 5, 1);
        let tree_birch = TreePopulator::new(TreeType::Birch, 5, 1);
        let tree_taiga = TreePopulator::new(TreeType::Spruce, 10, 1);
        let tree_mountains = TreePopulator::new(TreeType::Oak, 1, 1);

        let tall_grass_plains = TallGrassPopulator::new(12);
        let tall_grass_forest = TallGrassPopulator::new(3);
        let tall_grass_taiga = TallGrassPopulator::new(1);
        let tall_grass_mountains = TallGrassPopulator::new(1);
        let tall_grass_river = TallGrassPopulator::new(5);
        let tall_grass_ocean = TallGrassPopulator::new(5);
        let tall_grass_ice_plains = TallGrassPopulator::new(5);

        let cave_populator = CavePopulator::new();

        let mut gen = Self {
            seed,
            climate,
            spline: terrain_spline(),
            gaussian,
            water_height: BDS_SEA_LEVEL,
            ground_cover,
            ore_populator,
            emerald_ore,
            tree_forest,
            tree_birch,
            tree_taiga,
            tree_mountains,
            tall_grass_plains,
            tall_grass_forest,
            tall_grass_taiga,
            tall_grass_mountains,
            tall_grass_river,
            tall_grass_ocean,
            tall_grass_ice_plains,
            cave_populator,
            spawn_pos: (0, BDS_SEA_LEVEL + 2, 0),
            column_cache: Mutex::new(ColumnCache::default()),
        };

        // Find safe spawn point
        let mut spawn = (0, BDS_SEA_LEVEL + 2, 0);
        'search: for radius in 0i32..16 {
            for cx in -radius..=radius {
                for cz in -radius..=radius {
                    if cx.abs() != radius && cz.abs() != radius {
                        continue;
                    }
                    let chunk = gen.generate_base_chunk(cx, cz);
                    for x in 0..16 {
                        for z in 0..16 {
                            if let Some(h) = chunk.get_highest_block_at(x, z) {
                                let b = chunk.get_block(x, h, z);
                                if h > BDS_SEA_LEVEL && block_id::is_solid(b) {
                                    let world_x = cx * 16 + x as i32;
                                    let world_z = cz * 16 + z as i32;
                                    spawn = (world_x, h + 1, world_z);
                                    break 'search;
                                }
                            }
                        }
                    }
                }
            }
        }
        gen.spawn_pos = spawn;

        gen
    }

    /// Sample the four climate noises the terrain offset needs
    /// (continentalness, erosion, peaks, weirdness).
    ///
    /// Temperature/humidity are not read from the raw sample anywhere: the
    /// terrain offset consumes only c/e/p/w, and the biome map resamples
    /// every noise at its peaks-shifted point, so raw t/h samples would be
    /// dead work.
    #[inline]
    fn sample_terrain_climate(&self, x: f64, z: f64) -> ClimateValues {
        ClimateValues {
            temperature: 0.0,
            humidity: 0.0,
            continentalness: self.climate[CLIMATE_CONTINENTALNESS].sample(x, 0.0, z),
            erosion: self.climate[CLIMATE_EROSION].sample(x, 0.0, z),
            peaks: self.climate[CLIMATE_PEAKS_AND_VALLEYS].sample(x, 0.0, z),
            weirdness: self.climate[CLIMATE_WEIRDNESS].sample(x, 0.0, z),
        }
    }

    /// Surface biome id at a single block column — the exact per-point niduy
    /// sampler (`sampleBiomeNoise(x, 0, z, true, false)` mapped to Bedrock
    /// legacy ids). The chunk fill (`generate_base_chunk`) runs this same
    /// sampler at quart-cell corners and expands per column, so the value
    /// stored for a column is exactly `biome_at` at the containing 4×4
    /// cell's corner.
    pub fn biome_at(&self, x: i32, z: i32) -> u8 {
        let c = self.sample_terrain_climate(x as f64, z as f64);
        self.climate_to_biome_from_values(x, z, &c)
    }

    /// Raw 1.18 depth-spline offset (spline + 0.015), NOT scaled by the
    /// legacy ×2 factor. This is the value the Java generator feeds into its
    /// climate `depth` parameter, so the biome map uses it as-is.
    #[inline]
    fn terrain_offset_raw(&self, c: &ClimateValues) -> f64 {
        let peaks = -3.0 * ((c.weirdness.abs() - 0.6666667).abs() - 0.33333334);
        eval_spline(&[c.continentalness, c.erosion, peaks, c.weirdness], &self.spline) + 0.015
    }

    /// Get terrain offset from precomputed climate values
    ///
    /// Still scaled ×2 to keep the current terrain shape; the biome map uses
    /// [`BDSTerrain::terrain_offset_raw`] instead.
    #[inline]
    fn terrain_offset_from_climate(&self, c: &ClimateValues) -> f64 {
        self.terrain_offset_raw(c) * 2.0
    }

    /// Density at position using precomputed offset
    #[inline]
    fn density_at_with_offset(&self, y: i32, offset: f64) -> f64 {
        2.5 - ((y as f64) * 4.0) / 128.0 - 83.0 / 160.0 + offset
    }

    /// Get height at column using precomputed climate
    #[inline]
    fn height_at_with_climate(&self, c: &ClimateValues) -> i32 {
        let off = self.terrain_offset_from_climate(c);
        let limit = (1.0 - 83.0 / 160.0 + off) * 32.0;
        let mut h = limit.floor() as i32;
        while self.density_at_with_offset(h + 1, off) > 0.0 {
            h += 1;
        }
        while self.density_at_with_offset(h, off) <= 0.0 {
            h -= 1;
        }
        h.clamp(-64, 255)
    }

    /// Public API: Sample climate noise at (x, z)
    #[inline]
    pub fn sample_climate(&self, index: usize, x: f64, z: f64) -> f64 {
        self.climate[index].sample(x, 0.0, z)
    }

    /// Public API: Get terrain offset for density function
    #[inline]
    pub fn terrain_offset(&self, x: i32, z: i32) -> f64 {
        let c = self.sample_terrain_climate(x as f64, z as f64);
        self.terrain_offset_from_climate(&c)
    }

    /// Public API: Density at position (x, y, z)
    #[inline]
    pub fn density_at(&self, x: i32, y: i32, z: i32) -> f64 {
        let c = self.sample_terrain_climate(x as f64, z as f64);
        let off = self.terrain_offset_from_climate(&c);
        self.density_at_with_offset(y, off)
    }

    /// Public API: Is position solid
    #[inline]
    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.density_at(x, y, z) > 0.0
    }

    /// Public API: Get height at column (x, z)
    #[inline]
    pub fn height_at(&self, x: i32, z: i32) -> i32 {
        let c = self.sample_terrain_climate(x as f64, z as f64);
        self.height_at_with_climate(&c)
    }

    /// Map climate values to a Bedrock biome id through the Java 1.18+
    /// climate KD-tree (`bds_biome_tree`), mirroring cubiomes'
    /// `sampleBiomeNoise` for the surface biome map: the sample point is
    /// shifted by the peaks/valleys noise (`x` by `peaks(x,0,z)`, `z` by
    /// `peaks(z,x,0)`, both ×4), every climate noise is resampled at the
    /// shifted point, and the depth parameter is evaluated at y=0 — the
    /// convention cubiomes `generator.c` (line 634) uses for the surface map.
    ///
    /// Only `c.peaks` is read from `c` (the first shift term); the raw
    /// c/e/w/t/h samples it may carry are ignored, so five climate noises
    /// plus the swapped-peaks one are sampled here.
    fn climate_to_biome_from_values(&self, x: i32, z: i32, c: &ClimateValues) -> u8 {
        let xf = x as f64;
        let zf = z as f64;
        let px = xf + c.peaks * 4.0;
        let pz = zf + self.climate[CLIMATE_PEAKS_AND_VALLEYS].sample(zf, xf, 0.0) * 4.0;

        let cont = self.climate[CLIMATE_CONTINENTALNESS].sample(px, 0.0, pz);
        let erosion = self.climate[CLIMATE_EROSION].sample(px, 0.0, pz);
        let weirdness = self.climate[CLIMATE_WEIRDNESS].sample(px, 0.0, pz);
        let temperature = self.climate[CLIMATE_TEMPERATURE].sample(px, 0.0, pz);
        let humidity = self.climate[CLIMATE_HUMIDITY].sample(px, 0.0, pz);

        // Depth at y=0: `1 - y*4/128 - 83/160 + spline(c, e, peaks, w) + 0.015`.
        let peaks_val = -3.0 * ((weirdness.abs() - 0.6666667).abs() - 0.33333334);
        let raw_off = eval_spline(&[cont, erosion, peaks_val, weirdness], &self.spline) + 0.015;
        let depth = 1.0 - 83.0 / 160.0 + raw_off;

        // `Math.trunc(10000 * value)` from sampleBiomeNoise.
        let np: [i32; 6] = [
            (temperature * 10000.0) as i32,
            (humidity * 10000.0) as i32,
            (cont * 10000.0) as i32,
            (erosion * 10000.0) as i32,
            (depth * 10000.0) as i32,
            (weirdness * 10000.0) as i32,
        ];
        bds_biome_tree::java_to_bedrock(bds_biome_tree::gen_overworld(&np))
    }

    /// Generate base terrain for a chunk (bedrock, stone, water) - OPTIMIZED
    pub fn generate_base_chunk(&self, chunk_x: i32, chunk_z: i32) -> Chunk {
        let mut random = Random::new(0xdeadbeef ^ ((chunk_x as i64) << 8) ^ (chunk_z as i64) ^ self.seed as i64);
        let base_x = chunk_x * 16;
        let base_z = chunk_z * 16;

        // STEP 1: Biome map on the quart (4x4) grid, expanded per column.
        //
        // Real BDS samples biome noise per quart cell and fills the legacy
        // per-column array with the containing cell's id, and the climate
        // noise field is designed for quart sampling — per-block sampling
        // oversamples it ~16x for no benefit. Every cell value comes from
        // the exact per-point niduy sampler (`climate_to_biome_from_values`)
        // evaluated at the cell corner, so a column whose coordinates are
        // multiples of 4 always stores `biome_at(x, z)` exactly.
        let padding = self.gaussian.smooth_size;
        let grid_size = (16 + padding * 2) as usize;
        let start = -padding;
        let end = 16 + padding;

        // Quart cells covering the padded column range [start, end).
        let cell_min = start.div_euclid(4);
        let cell_max = (end - 1).div_euclid(4);

        let mut biome_grid = vec![0u8; grid_size * grid_size];
        let mut chunk_biomes = [0u8; 256];
        let mut uniform_biome: Option<u8> = None;
        let mut is_uniform = true;

        for qcx in cell_min..=cell_max {
            for qcz in cell_min..=cell_max {
                let cell_x = qcx * 4;
                let cell_z = qcz * 4;

                // One sample set per cell, at the cell corner (world
                // coordinates: `cell_x`/`cell_z` are chunk-local columns).
                let c = self.sample_terrain_climate(
                    (base_x + cell_x) as f64,
                    (base_z + cell_z) as f64,
                );
                let b = self.climate_to_biome_from_values(base_x + cell_x, base_z + cell_z, &c);

                for x in cell_x..cell_x + 4 {
                    if x < start || x >= end {
                        continue;
                    }
                    let gx = (x + padding) as usize;
                    for z in cell_z..cell_z + 4 {
                        if z < start || z >= end {
                            continue;
                        }
                        let gz = (z + padding) as usize;
                        biome_grid[gx * grid_size + gz] = b;

                        if x >= 0 && x < 16 && z >= 0 && z < 16 {
                            chunk_biomes[(x as usize) * 16 + z as usize] = b;
                        }

                        if is_uniform {
                            match uniform_biome {
                                None => uniform_biome = Some(b),
                                Some(prev) if prev == b => {}
                                _ => is_uniform = false,
                            }
                        }
                    }
                }
            }
        }

        let dominant_biome = chunk_biomes[7 * 16 + 7];
        let mut chunk = Chunk::new(dominant_biome);
        chunk.biomes = chunk_biomes;

        // STEP 2: If uniform biome, skip Gaussian blur for elevations
        let mut min_heights = [0.0f64; 256];
        let mut max_heights = [0.0f64; 256];

        if is_uniform {
            let b_info = biome::get_biome_info(uniform_biome.unwrap_or(biome::PLAINS));
            let min_el = b_info.min_elevation - 1.0;
            let max_el = b_info.max_elevation;
            min_heights.fill(min_el);
            max_heights.fill(max_el);
        } else {
            // Blur along X axis - read from biome_grid (with padding)
            let mut min_heights_x = vec![0.0f64; (16 * grid_size) as usize];
            let mut max_heights_x = vec![0.0f64; (16 * grid_size) as usize];

            for x in 0..16usize {
                for z in start..end {
                    let gz = (z + padding) as usize;
                    let mut min_sum = 0.0;
                    let mut max_sum = 0.0;

                    for sx in -self.gaussian.smooth_size..=self.gaussian.smooth_size {
                        let weight = self.gaussian.kernel_1d[(sx + self.gaussian.smooth_size) as usize];
                        let adj_gx = (x as i32 + sx + padding) as usize;
                        let adj_biome = biome_grid[adj_gx * grid_size as usize + gz];
                        let info = biome::get_biome_info(adj_biome);
                        min_sum += (info.min_elevation - 1.0) * weight;
                        max_sum += info.max_elevation * weight;
                    }

                    let idx = x * grid_size as usize + gz;
                    min_heights_x[idx] = min_sum / self.gaussian.weight_sum_1d;
                    max_heights_x[idx] = max_sum / self.gaussian.weight_sum_1d;
                }
            }

            // Blur along Z axis
            for x in 0..16usize {
                for z in 0..16usize {
                    let mut min_sum = 0.0;
                    let mut max_sum = 0.0;

                    for sz in -self.gaussian.smooth_size..=self.gaussian.smooth_size {
                        let weight = self.gaussian.kernel_1d[(sz + self.gaussian.smooth_size) as usize];
                        let adj_gz = (z as i32 + sz + padding) as usize;
                        let idx = x * grid_size as usize + adj_gz;
                        min_sum += min_heights_x[idx] * weight;
                        max_sum += max_heights_x[idx] * weight;
                    }

                    let out_idx = x * 16 + z;
                    min_heights[out_idx] = min_sum / self.gaussian.weight_sum_1d;
                    max_heights[out_idx] = max_sum / self.gaussian.weight_sum_1d;
                }
            }
        }

        // STEP 3: Generate terrain using precomputed climate for the 16x16 chunk columns
        // This avoids resampling climate noises for each column
        for x in 0..16usize {
            for z in 0..16usize {
                let world_x = base_x + x as i32;
                let world_z = base_z + z as i32;

                chunk.set_block(x, 0, z, block_id::BEDROCK);

                // Terrain climate for this column (t/h are not read by the
                // terrain offset; the biome map was sampled per quart cell
                // in STEP 1).
                let c = self.sample_terrain_climate(world_x as f64, world_z as f64);
                
                // Get height from precomputed climate
                let highest_solid = self.height_at_with_climate(&c);

                // Fill from bedrock up to highest solid
                for y in 1..=highest_solid {
                    chunk.set_block(x, y, z, block_id::STONE);
                }

                // Fill water above highest solid up to sea level
                for y in (highest_solid + 1)..=self.water_height {
                    chunk.set_block(x, y, z, block_id::WATER);
                }
            }
        }

        // Apply ground cover
        self.ground_cover.populate(&mut chunk, chunk_x, chunk_z, &mut random);

        chunk
    }

    /// Generate full chunk with population
    pub fn generate_chunk_internal(&self, chunk_x: i32, chunk_z: i32) -> Chunk {
        let mut manager = ChunkManager3x3::new(chunk_x, chunk_z);

        for cx in (chunk_x - 1)..=(chunk_x + 1) {
            for cz in (chunk_z - 1)..=(chunk_z + 1) {
                let base_chunk = self.generate_base_chunk(cx, cz);
                manager.set_chunk(cx, cz, base_chunk);
            }
        }

        let dominant_biome = manager.get_chunk(chunk_x, chunk_z).unwrap().biomes[7 * 16 + 7];
        self.populate_chunk(&mut manager, chunk_x, chunk_z, dominant_biome);

        manager.take_chunk(chunk_x, chunk_z).unwrap()
    }

    /// Populate a chunk with ores, trees, tall grass, caves
    pub fn populate_chunk<M: ChunkManager>(
        &self,
        world: &mut M,
        chunk_x: i32,
        chunk_z: i32,
        dominant_biome: u8,
    ) {
        let mut random = Random::new(0xdeadbeef ^ ((chunk_x as i64) << 8) ^ (chunk_z as i64) ^ self.seed as i64);

        // 1. Overworld standard ores
        self.ore_populator.populate(world, chunk_x, chunk_z, &mut random);

        // 2. Biome-specific population (grouped by family; the KD-tree can
        // now emit many more ids than the old threshold mapper)
        match dominant_biome {
            biome::PLAINS | biome::SUNFLOWER_PLAINS => {
                self.tall_grass_plains.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::FOREST | biome::FLOWER_FOREST | biome::DARK_FOREST => {
                self.tree_forest.populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_forest.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::BIRCH_FOREST | biome::TALL_BIRCH_FOREST => {
                self.tree_birch.populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_forest.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::TAIGA
            | biome::SNOWY_TAIGA
            | biome::GIANT_TREE_TAIGA
            | biome::GIANT_SPRUCE_TAIGA => {
                self.tree_taiga.populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_taiga.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::JUNGLE | biome::JUNGLE_EDGE | biome::BAMBOO_JUNGLE => {
                self.tree_forest.populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_forest.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::SAVANNA | biome::SAVANNA_PLATEAU | biome::SHATTERED_SAVANNA => {
                self.tall_grass_plains.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::EXTREME_HILLS
            | biome::EXTREME_HILLS_EDGE
            | biome::WOODED_MOUNTAINS
            | biome::GRAVELLY_MOUNTAINS => {
                self.tree_mountains.populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_mountains.populate(world, chunk_x, chunk_z, &mut random);
                self.emerald_ore.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::MEADOW | biome::GROVE | biome::SNOWY_SLOPES => {
                self.tree_mountains.populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_mountains.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::JAGGED_PEAKS | biome::FROZEN_PEAKS | biome::STONY_PEAKS => {
                self.tall_grass_mountains.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::RIVER | biome::FROZEN_RIVER => {
                self.tall_grass_river.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::OCEAN
            | biome::DEEP_OCEAN
            | biome::WARM_OCEAN
            | biome::LUKEWARM_OCEAN
            | biome::DEEP_LUKEWARM_OCEAN
            | biome::COLD_OCEAN
            | biome::DEEP_COLD_OCEAN
            | biome::FROZEN_OCEAN
            | biome::DEEP_FROZEN_OCEAN => {
                self.tall_grass_ocean.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::ICE_PLAINS | biome::ICE_SPIKES => {
                self.tall_grass_ice_plains.populate(world, chunk_x, chunk_z, &mut random);
            }
            _ => {}
        }

        // Cave tunnels (skip open water, ice and the deep dark)
        match dominant_biome {
            biome::OCEAN
            | biome::DEEP_OCEAN
            | biome::WARM_OCEAN
            | biome::LUKEWARM_OCEAN
            | biome::DEEP_LUKEWARM_OCEAN
            | biome::COLD_OCEAN
            | biome::DEEP_COLD_OCEAN
            | biome::FROZEN_OCEAN
            | biome::DEEP_FROZEN_OCEAN
            | biome::RIVER
            | biome::FROZEN_RIVER
            | biome::ICE_PLAINS
            | biome::DEEP_DARK
            | biome::SWAMPLAND
            | biome::MANGROVE_SWAMP => {}
            _ => {
                let mut cave_random = Random::new(
                    self.seed as i64 ^ ((chunk_x as i64) << 32) ^ (chunk_z as i64) ^ 0xC4E5_F00D,
                );
                self.cave_populator.populate(world, chunk_x, chunk_z, &mut cave_random);
            }
        }
    }

    fn cached_column(&self, chunk_x: i32, chunk_z: i32) -> Arc<Chunk> {
        let key = (chunk_x, chunk_z);
        if let Some(chunk) = self.column_cache.lock().entries.get(&key) {
            return Arc::clone(chunk);
        }
        let chunk = Arc::new(self.generate_chunk_internal(chunk_x, chunk_z));
        let mut cache = self.column_cache.lock();
        if cache.entries.insert(key, Arc::clone(&chunk)).is_none() {
            cache.order.push_back(key);
            while cache.order.len() > COLUMN_CACHE_CAP {
                if let Some(oldest) = cache.order.pop_front() {
                    cache.entries.remove(&oldest);
                }
            }
        }
        chunk
    }
}

impl WorldGenerator for BDSTerrain {
    fn seed(&self) -> i64 {
        self.seed as i64
    }

    fn generator_type(&self) -> i32 {
        2 // DEFAULT / INFINITE
    }

    fn spawn_position(&self) -> (i32, i32, i32) {
        self.spawn_pos
    }

    fn player_position(&self) -> (f32, f32, f32) {
        let (x, y, z) = self.spawn_pos;
        (x as f32 + 0.5, y as f32 + 1.62, z as f32 + 0.5)
    }

    fn generate_chunk(&self, chunk_x: i32, chunk_z: i32) -> (u32, Vec<u8>) {
        let chunk = self.generate_chunk_internal(chunk_x, chunk_z);
        let dominant_biome = chunk.biomes[7 * 16 + 7];
        chunk.serialize_network_payload(dominant_biome)
    }

    fn generate_chunk_object(&self, chunk_x: i32, chunk_z: i32) -> Chunk {
        self.generate_chunk_internal(chunk_x, chunk_z)
    }

    fn block_at(&self, x: i32, y: i32, z: i32) -> u32 {
        self.cached_column(x >> 4, z >> 4)
            .get_block((x & 15) as usize, y, (z & 15) as usize)
    }

    fn highest_block_at(&self, x: i32, z: i32) -> i32 {
        self.cached_column(x >> 4, z >> 4)
            .get_highest_block_at((x & 15) as usize, (z & 15) as usize)
            .unwrap_or(-64)
    }

    fn generate_chunk_with_edits(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        edits: &[(i32, i32, i32, u32)],
    ) -> (u32, Vec<u8>) {
        let mut chunk = self.generate_chunk_internal(chunk_x, chunk_z);
        for &(x, y, z, id) in edits {
            if (x >> 4) == chunk_x && (z >> 4) == chunk_z {
                chunk.set_block((x & 15) as usize, y, (z & 15) as usize, id);
            }
        }
        let dominant_biome = chunk.biomes[7 * 16 + 7];
        chunk.serialize_network_payload(dominant_biome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn bds_terrain_basic() {
        let terrain = BDSTerrain::new(0);

        // Test that density decreases with Y
        let d0 = terrain.density_at(0, 0, 0);
        let d1 = terrain.density_at(0, 64, 0);
        assert!(d0 > d1, "density must decrease with y");

        // Test height_at returns reasonable values
        let h = terrain.height_at(0, 0);
        assert!(h >= 0 && h <= 255, "height must be in valid range");

        // Test is_solid consistency
        assert!(terrain.is_solid(0, h, 0), "height_at must be solid");
        assert!(!terrain.is_solid(0, h + 1, 0), "above height must be air");
    }

    #[test]
    fn bds_terrain_variation() {
        let terrain = BDSTerrain::new(12345);

        // Test that terrain height varies across different positions
        let mut heights = Vec::new();
        for x in -50..=50 {
            let h = terrain.height_at(x, 0);
            heights.push(h);
        }

        // Should have some variation (not all the same height)
        let min_h = *heights.iter().min().unwrap();
        let max_h = *heights.iter().max().unwrap();
        assert!(max_h > min_h, "terrain should have height variation, got min={}, max={}", min_h, max_h);

        // Heights should be in reasonable range
        for h in heights {
            assert!(h >= -64 && h <= 255, "height out of range: {}", h);
        }
    }

    #[test]
    fn bds_sea_level() {
        assert_eq!(BDS_SEA_LEVEL, 62);
    }

    #[test]
    fn bds_generator_creation() {
        let gen = BDSTerrain::new(12345);
        assert_eq!(gen.seed, 12345);
        assert_eq!(gen.generator_type(), 2);
    }

    /// Permanent regression for the niduy-parity work: the climate-noise
    /// pipeline (Xoroshiro128+ → double-Perlin octaves with the `xNextInt`
    /// perm shuffle) and the 1.18+ KD-tree biome selection were verified
    /// against a bundled copy of `niduy/minecraft-world-generator` for seed
    /// 12345 — `w.sampleDoublePerlinNoise(biomeNoise.climate[i], x, 0, z)` for
    /// the noise values and `w.sampleBiomeNoise(x, 0, z, true, false)` (the
    /// surface map at y=0) for the biomes. The noise constants are recorded at
    /// the reference's printed precision (6 decimals) and the biome ids are
    /// exact Bedrock legacy ids.
    #[test]
    fn niduy_parity_climate_noise_and_biomes_seed_12345() {
        let terrain = BDSTerrain::new(12345);

        // Climate noise values: [temperature, humidity, continentalness,
        // erosion, peaks, weirdness].
        let points: [(&str, f64, f64, [f64; 6]); 4] = [
            (
                "(0,0)",
                0.0,
                0.0,
                [-0.024734, 0.680247, -0.402616, -0.734568, -0.537154, 0.175442],
            ),
            (
                "(500,500)",
                500.0,
                500.0,
                [-1.030762, -0.276024, 0.413789, -0.022153, 0.191433, -0.182315],
            ),
            (
                "(12345,-6789)",
                12345.0,
                -6789.0,
                [0.074453, -0.008910, -0.710447, -0.074597, -0.217447, -0.312699],
            ),
            (
                "(-2048,0)",
                -2048.0,
                0.0,
                [0.170523, -0.290719, -0.428767, 0.019340, 0.023419, 0.328943],
            ),
        ];
        for (label, x, z, want) in points {
            for (i, w) in want.iter().enumerate() {
                let got = terrain.sample_climate(i, x, z);
                // The constants are the reference's printed values (6
                // decimals), so the tolerance absorbs half a unit in the
                // last printed place plus float error.
                assert!(
                    (got - w).abs() < 2e-6,
                    "climate[{i}] at {label}: got {got}, want {w}"
                );
            }
        }

        // Surface biomes through the exact per-point sampler (`biome_at` →
        // peaks shift → KD-tree → `java_to_bedrock`), matching the reference
        // comparison point for point.
        let biome_points: [(i32, i32, u8); 9] = [
            (0, 0, biome::LUSH_CAVES),
            (100, 0, biome::DEEP_DARK),
            (500, 500, biome::DRIPSTONE_CAVES),
            (-1000, -1000, biome::SAVANNA),
            (3000, -2000, biome::DEEP_LUKEWARM_OCEAN),
            (-4520, 8350, biome::COLD_OCEAN),
            (12345, -6789, biome::DEEP_OCEAN),
            (100000, 100000, biome::SAVANNA),
            (-100000, 50000, biome::DEEP_OCEAN),
        ];
        for (x, z, want) in biome_points {
            let got = terrain.biome_at(x, z);
            assert_eq!(got, want, "per-point biome at ({x},{z})");
        }

        // The verified strip along z=0 (niduy TS output, 65/65 exact): x =
        // -2048 + 64*i, Bedrock legacy ids (0 ocean, 1 plains, 4 forest,
        // 5 taiga, 6 swampland, 7 river, 11 frozen river, 12 ice plains,
        // 16 beach, 23 jungle edge, 25 stone shore, 27 birch forest,
        // 30 snowy taiga, 35 savanna, 46 frozen ocean, 129 sunflower
        // plains, 155 tall birch, 185 grove, 186 meadow, 187 lush caves,
        // 188 dripstone caves, 190 deep dark).
        const STRIP: [u8; 65] = [
            0, 0, 187, 187, 155, 187, 187, 4, 7, 129, 7, 7, 25, 0, 0, 0, 0, 4, 4, 4, 35, 7,
            188, 188, 188, 188, 187, 23, 1, 190, 0, 0, 187, 190, 185, 187, 187, 11, 30, 11,
            46, 187, 187, 30, 188, 188, 188, 12, 188, 186, 188, 188, 188, 188, 4, 4, 155, 0,
            0, 16, 4, 27, 188, 5, 6,
        ];
        for (i, want) in STRIP.iter().enumerate() {
            let x = -2048 + 64 * i as i32;
            assert_eq!(terrain.biome_at(x, 0), *want, "strip biome at x={x}");
        }

        // The chunk fill runs the same sampler at quart-cell corners and
        // expands per column, so a column on the quart grid must equal
        // `biome_at` exactly (all coordinates below are multiples of 4).
        let corner_points = [
            (0, 0, biome::LUSH_CAVES),
            (100, 0, biome::DEEP_DARK),
            (500, 500, biome::DRIPSTONE_CAVES),
            (-1000, -1000, biome::SAVANNA),
            (3000, -2000, biome::DEEP_LUKEWARM_OCEAN),
            (100000, 100000, biome::SAVANNA),
            (-100000, 50000, biome::DEEP_OCEAN),
        ];
        for (x, z, want) in corner_points {
            let chunk = terrain.generate_base_chunk(x >> 4, z >> 4);
            let got = chunk.biomes[((x & 15) as usize) * 16 + ((z & 15) as usize)];
            assert_eq!(got, want, "quart-corner chunk biome at ({x},{z})");
        }

        // And every column of a chunk carries its quart cell's corner value
        // (checked on the two off-grid points of the list above).
        for (x, z) in [(-4520, 8350), (12345, -6789)] {
            let chunk = terrain.generate_base_chunk(x >> 4, z >> 4);
            let got = chunk.biomes[((x & 15) as usize) * 16 + ((z & 15) as usize)];
            let cell = terrain.biome_at(x.div_euclid(4) * 4, z.div_euclid(4) * 4);
            assert_eq!(got, cell, "chunk column ({x},{z}) carries its quart cell's value");
        }
    }
}