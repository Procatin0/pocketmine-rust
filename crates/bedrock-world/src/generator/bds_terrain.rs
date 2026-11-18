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
use crate::generator::biome;
use crate::generator::cubiomes::Cubiomes;
use crate::generator::caves::CavePopulator;
use crate::generator::manager::{ChunkManager, ChunkManager3x3};
use crate::generator::populators::{
    GroundCoverPopulator, OrePopulator, TallGrassPopulator, TreePopulator, TreeType,
    UnderwaterPopulator,
};
use crate::generator::random::Random;
use crate::generator::WorldGenerator;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Full chunks kept for single-block queries.
const COLUMN_CACHE_CAP: usize = 32;

/// Base chunks kept for the 3x3 windows [`BDSTerrain::generate_chunk_internal`]
/// assembles; without it every chunk regenerates its eight neighbours from
/// scratch.
const BASE_CACHE_CAP: usize = 128;

/// Small FIFO cache of generated chunks. Generation happens outside the lock, so
/// two threads asking for the same chunk may both build it; the result is pure,
/// so whichever lands last is the same chunk.
#[derive(Debug, Default)]
struct ChunkCache {
    entries: HashMap<(i32, i32), Arc<Chunk>>,
    order: VecDeque<(i32, i32)>,
}

impl ChunkCache {
    fn get(&self, key: (i32, i32)) -> Option<Arc<Chunk>> {
        self.entries.get(&key).cloned()
    }

    fn insert(&mut self, key: (i32, i32), chunk: Arc<Chunk>, cap: usize) {
        if self.entries.insert(key, chunk).is_none() {
            self.order.push_back(key);
            while self.order.len() > cap {
                if let Some(oldest) = self.order.pop_front() {
                    self.entries.remove(&oldest);
                }
            }
        }
    }
}

/// Biome cells kept between chunks. The biome pass reads a window of cells around
/// each chunk so it can smooth across borders, which means the same cell is
/// computed by several neighbouring chunks; without this the wide window would
/// cost a full climate sample per neighbour per chunk.
const BIOME_CELL_CACHE_CAP: usize = 8192;

/// Biome ids by quart cell, for the smoothing window. Plain values: a cell is one
/// byte and there are no references to keep alive.
#[derive(Debug, Default)]
struct CellCache {
    entries: HashMap<(i32, i32), u8>,
    order: VecDeque<(i32, i32)>,
}

impl CellCache {
    fn get(&self, key: (i32, i32)) -> Option<u8> {
        self.entries.get(&key).copied()
    }

    fn insert(&mut self, key: (i32, i32), value: u8, cap: usize) {
        if self.entries.insert(key, value).is_none() {
            self.order.push_back(key);
            while self.order.len() > cap {
                if let Some(oldest) = self.order.pop_front() {
                    self.entries.remove(&oldest);
                }
            }
        }
    }
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
    eval_spline_impl(vals, sp, false)
}

/// The same spline with straight segments, ignoring `der`.
///
/// Every point in this file is built with `der = 0.0`, and a cubic Hermite with
/// zero slopes is a smoothstep: flat at both knots and up to 1.5x steeper in
/// between. Across the world that reads as a plain, then a two block wall, then
/// another plain. With no derivative to honour, a straight line spreads the same
/// rise evenly.
///
/// This is for the *shape* of the terrain only. The biome classifier keeps
/// [`eval_spline`] because the `depth` it hands to the KD-tree has to stay
/// comparable with the BDS captures (see `niduy_parity_...`).
fn eval_spline_straight(vals: &[f64], sp: &Spline) -> f64 {
    eval_spline_impl(vals, sp, true)
}

fn eval_spline_impl(vals: &[f64], sp: &Spline, straight: bool) -> f64 {
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
                eval_spline_impl(vals, &pt.value, straight)
                    + (pt.der as f64) * (f - (pt.loc as f64))
            } else {
                let g = points[i - 1].loc as f64;
                let h = points[i].loc as f64;
                let k = (f - g) / (h - g);
                let l = points[i - 1].der as f64;
                let m = points[i].der as f64;
                let n = eval_spline_impl(vals, &points[i - 1].value, straight);
                let o = eval_spline_impl(vals, &points[i].value, straight);
                if straight && l == 0.0 && m == 0.0 {
                    return lerp(k, n, o);
                }
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

/// Per-noise coordinate scale for the world's climate sampling, in the order of
/// the `CLIMATE_*` indices: temperature, humidity, continentalness, erosion,
/// peaks and valleys, weirdness.
const CLIMATE_SCALES: [f64; 6] = [1.3, 1.3, 0.6, 0.6, 0.6, 0.6];

/// Domain warp amplitude in blocks applied to the climate sample point. Without
/// it the climate field is a plain smooth function, so lakes and biome borders
/// come out as near-perfect ellipses.
const WARP_BLOCKS: f64 = 60.0;

/// Amplitude in blocks of the high-frequency detail added to the surface. The
/// density changes by 4/128 per block, so one block is 1/32 of an offset unit.
const DETAIL_BLOCKS: f64 = 7.0;

/// Half width, in noise units, of the river band: columns whose river noise is
/// within this of zero are carved down to just below sea level, which turns the
/// zero crossing of the noise into a winding channel.
///
/// One noise unit is worth roughly 30 blocks across the zero crossing, so this
/// value is about four blocks to each side: a river eight blocks wide, which is
/// what vanilla rivers measure. The previous 0.035 carved the same channel but
/// only flooded its innermost block or two, which read as a dry gully with a
/// string of water in it.
const RIVER_HALF_WIDTH: f64 = 0.34;

/// How far below sea level the river bed sits at the channel centre, in blocks.
/// The carve fades to zero at the band edge, so the depth tapers from this to
/// the surrounding terrain.
const RIVER_DEPTH: i32 = 2;

/// How far above sea level a column can be and still be carved into a river.
/// Rivers belong in valleys; capping the carve keeps them off hills and is what
/// stops the noise from drawing a channel through every part of the map.
const RIVER_MAX_HEIGHT: i32 = 6;

/// River noise strength below which a column is not a river: only the centre of
/// the band carves, so the channels stay narrow.
const RIVER_CENTRE: f64 = 0.35;

/// Blocks below `sea + RIVER_MAX_HEIGHT` over which the river carve fades out, so
/// the hillside end of a channel is a bank instead of a wall.
const RIVER_CAP_FADE: f64 = 8.0;

/// Elevation bands for biomes the climate tree picks by climate alone (world
/// coordinates, sea level is 62). Above the peaks floor only the tops of our
/// terrain qualify; a stone shore has to sit at the waterline.
const PEAKS_MIN_HEIGHT: f64 = 92.0;
const GROVE_MIN_HEIGHT: f64 = 88.0;
const SHORE_MAX_HEIGHT: f64 = 64.0;



/// Water this deep gets the ocean's deep variant.
const DEEP_WATER: f64 = 10.0;

/// Constant the density field has at y=0 besides the offset, so the analytic
/// height of a column is `(DENSITY_BASE + offset) * 32`.
const DENSITY_BASE: f64 = 2.5 - 83.0 / 160.0;

/// Offset added to every column before it becomes a height. The spline puts
/// `offset == 0` at y≈63, which is barely above sea level, so the shoreline fell
/// where the spline crosses zero and a large slice of the map ended up under
/// water. Raising the whole field pushes the shoreline a little further out to
/// sea: land grows, water shrinks, and nothing else about the shape changes.
/// One unit of offset is 32 blocks, so this is about three blocks.
const LAND_BIAS: f64 = 0.16;

/// Scale applied to the raw spline offset (one unit is 32 blocks of height).
/// 2.0 made the slopes steeper than vanilla's; 1.6 keeps the shape with a
/// gentler gradient.
const TERRAIN_GAIN: f64 = 1.6;

/// Amplitude in blocks of the 3D density noise: how far the zero crossing can
/// move from where the 2D offset puts it.
const DEPTH_NOISE_BLOCKS: f64 = 5.0;

/// Vertical stretch of the 3D density noise. Its octaves run at ~32 blocks in
/// the horizontal plane, so sampling y at twice the rate keeps the features from
/// being tall tubes.
const DEPTH_NOISE_VERTICAL: f64 = 3.0;

/// Share of the submerged relief that is flattened away: 0 would keep the
/// stairs, 1 would make flat plates.
const SHORE_BED_GAIN: f64 = 0.35;

/// Blocks of water the spline has to put a column under before the open-sea
/// deepening reaches full strength. The deepening is proportional to this, so it
/// is zero at the shoreline: the coast keeps the spline's own height instead of
/// jumping down into a trench.
const SEA_FLOOR_RAMP: f64 = 14.0;

/// Extra depth (on top of what the spline already gives) added to the sea floor
/// of open ocean, in blocks, before the relief-noise modulation.
const SEA_FLOOR_MAX_DEPTH: f64 = 22.0;


/// Amplitude in blocks of the broad relief. Without it the spline offset is the
/// only shape the terrain has, and because that changes over hundreds of blocks
/// every plain comes out as a plateau with contour-like steps. This is what tilts
/// plains and blends one region into the next.
const RELIEF_BLOCKS: f64 = 16.0;

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

/// Quart-cell window kept for the biome majority vote: the chunk itself plus a
/// one-cell ring on every side.
/// Cells each side of the chunk that the biome pass reads. It has to cover the
/// chunk (0..=3) plus the smoothing radius, so a window at the chunk border sees
/// the same neighbourhood the neighbouring chunk would.
const CELL_LO: i32 = -10;
const CELL_HI: i32 = 12;
const CELL_GRID: usize = (CELL_HI - CELL_LO + 1) as usize;

/// Minimum size, in cells, that a biome patch has to beat to appear: the window is
/// `2 * BIOME_SMOOTH_RADIUS + 1` cells, so a patch narrower than about twice the
/// radius loses the majority everywhere and is replaced by its surroundings.
///
/// Six cells is 24 blocks each side, which is what it takes to stop the slivers
/// the climate tree produces where two thresholds cross: sand strips, badlands
/// fingers and gravel specks that the player reads as "mini biomes" even though
/// they are not single-cell specks. Smaller radii only cleaned up single cells.
const BIOME_SMOOTH_RADIUS: i32 = 8;

/// Pick the biome shared by most of the `2 * BIOME_SMOOTH_RADIUS + 1` cells around
/// `(qcx, qcz)`, falling back to the cell's own value when nothing has a majority.
/// A 3x3 window only dissolves isolated specks; a wide window also breaks the
/// A-B-A-B striping the climate tree produces near a tie and removes any patch
/// that cannot hold its ground across a few dozen blocks.
/// Mode of the `(2 * radius + 1)` square around the grid entry `(i, j)`, ties
/// going to the centre.
fn majority_at(ids: &[[u8; CELL_GRID]; CELL_GRID], i: usize, j: usize, radius: usize) -> u8 {
    let own = ids[i][j];
    let mut seen: [(u8, u16); 64] = [(0, 0); 64];
    let mut len = 0usize;
    for di in 0..=2 * radius {
        for dj in 0..=2 * radius {
            let (ni, nj) = (i + di, j + dj);
            if ni < radius || nj < radius {
                continue;
            }
            let (ni, nj) = (ni - radius, nj - radius);
            if ni >= CELL_GRID || nj >= CELL_GRID {
                continue;
            }
            let id = ids[ni][nj];
            if let Some(entry) = seen[..len].iter_mut().find(|(other, _)| *other == id) {
                entry.1 += 1;
            } else if len < seen.len() {
                seen[len] = (id, 1);
                len += 1;
            }
        }
    }
    let mut best = own;
    let mut best_count = 0u16;
    for &(id, count) in &seen[..len] {
        if count > best_count || (count == best_count && id == own) {
            best = id;
            best_count = count;
        }
    }
    best
}

fn majority_biome(ids: &[[u8; CELL_GRID]; CELL_GRID], qcx: i32, qcz: i32) -> u8 {
    const RADIUS: i32 = BIOME_SMOOTH_RADIUS;
    // The window holds (2 * RADIUS + 1)^2 = 289 cells, so the per-biome count
    // cannot be a u8: one biome filling the whole window overflowed it and
    // panicked every chunk worker. Real Minecraft biomes are big, so that is the
    // common case, not an edge case.
    let mut seen: [(u8, u16); 289] = [(0, 0); 289];
    let mut len = 0usize;
    for dx in -RADIUS..=RADIUS {
        for dz in -RADIUS..=RADIUS {
            let (i, j) = (qcx + dx - CELL_LO, qcz + dz - CELL_LO);
            if !(0..CELL_GRID as i32).contains(&i) || !(0..CELL_GRID as i32).contains(&j) {
                continue;
            }
            let id = ids[i as usize][j as usize];
            if let Some(entry) = seen[..len].iter_mut().find(|(other, _)| *other == id) {
                entry.1 += 1;
            } else {
                seen[len] = (id, 1);
                len += 1;
            }
        }
    }
    // The most common id wins outright; on a tie the cell keeps its own. Falling
    // back to "own" whenever nothing has a strict majority was the bug: in mixed
    // terrain almost no window has one, so every cell answered with its own noisy
    // value and the filter made the map more fragmented, not less.
    let own = ids[(qcx - CELL_LO) as usize][(qcz - CELL_LO) as usize];
    let mut best = own;
    let mut best_count = 0u16;
    for &(id, count) in &seen[..len] {
        if count > best_count || (count == best_count && id == own) {
            best = id;
            best_count = count;
        }
    }
    best
}

/// BDS Terrain Generator using noise-based generation (1.18+ style)
#[derive(Debug)]
pub struct BDSTerrain {
    pub seed: u64,
    pub climate: [DoublePerlinNoise; 6],
    /// Warp the climate sample point so contours bend instead of closing in
    /// ellipses (one noise per horizontal axis).
    warp_x: DoublePerlinNoise,
    warp_z: DoublePerlinNoise,
    /// River noise: its zero crossings become river channels.
    river: DoublePerlinNoise,
    /// Broad relief, added to the density offset, in blocks (see
    /// [`RELIEF_BLOCKS`]).
    relief: DoublePerlinNoise,
    /// High-frequency height detail, added to the density offset.
    detail: DoublePerlinNoise,
    /// 3D noise inside the density function (see `density_at_with_offset`). This
    /// is what makes the surface irregular: it moves the zero crossing up and
    /// down by a few blocks with a period of a handful of blocks.
    depth_noise: DoublePerlinNoise,
    pub spline: Spline,
    /// Minecraft's own biome generator (vendored cubiomes), which decides the
    /// biome of every surface column.
    cubiomes: Option<Cubiomes>,
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
    underwater: UnderwaterPopulator,
    tall_grass_ice_plains: TallGrassPopulator,
    cave_populator: CavePopulator,
    spawn_pos: (i32, i32, i32),
    column_cache: Mutex<ChunkCache>,
    base_cache: Mutex<ChunkCache>,
    /// Biome id per quart cell, so the smoothing window does not re-sample the
    /// climate for cells several chunks already asked for.
    biome_cells: Mutex<CellCache>,
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

        let warp_noise = |tag: (u64, u64), amplitudes: &[f64], omin: i32| {
            let mut xr = Xoroshiro128Plus {
                low: x_low ^ tag.0,
                high: x_high ^ tag.1,
            };
            double_perlin_init(&mut xr, amplitudes, omin, amplitudes.len(), -1)
        };
        let warp_x = warp_noise(
            (0x9e3779b97f4a7c15, 0xbf58476d1ce4e5b9),
            &[1.0, 1.0, 1.0],
            -8,
        );
        let warp_z = warp_noise(
            (0x94d049bb133111eb, 0x2545f4914f6cdd1d),
            &[1.0, 1.0, 1.0],
            -8,
        );
        let river = warp_noise(
            (0x510e527fade682d1, 0x9b05688c2b3e6c1f),
            &[1.0, 1.0],
            -7,
        );
        let relief = warp_noise(
            (0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1),
            &[1.0, 1.0, 1.0],
            -8,
        );
        // Two extra octaves of 8 and 4 blocks: on a uniform slope the 2D field
        // puts a one-block contour every couple of blocks, and those contours
        // are what read as a staircase. Roughness at the scale of the steps
        // breaks the runs so no two neighbouring columns step together.
        let detail = warp_noise(
            (0x6a09e667f3bcc909, 0xbb67ae8584caa73b),
            &[1.0, 0.6, 0.45, 0.3],
            -5,
        );
        // Octaves -5 and -4 are 32 and 16 blocks wide, so a 20 block bowl was a
        // single smooth slope: quantised to whole blocks that is a set of
        // concentric one-block stairs, which is what the lake shores looked
        // like. The two extra octaves put 8 and 4 block bumps on the surface and
        // break those rings into an ordinary rocky bed.
        let depth_noise = warp_noise(
            (0x243f6a8885a308d3, 0x13198a2e03707344),
            &[1.0, 0.6, 0.32, 0.17],
            -5,
        );

        let _random = Random::new(seed as i64);

        let ground_cover = GroundCoverPopulator::new(seed);
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
            warp_x,
            warp_z,
            river,
            relief,
            detail,
            depth_noise,
            spline: terrain_spline(),
            cubiomes: Cubiomes::new(seed),
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
            underwater: UnderwaterPopulator,
            tall_grass_ice_plains,
            cave_populator,
            spawn_pos: (0, BDS_SEA_LEVEL + 2, 0),
            column_cache: Mutex::new(ChunkCache::default()),
            base_cache: Mutex::new(ChunkCache::default()),
            biome_cells: Mutex::new(CellCache::default()),
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
    /// Climate sample point for a world column: the coordinates are warped by
    /// two low-frequency noises so the climate contours bend instead of closing
    /// in ellipses, then pulled in by [`CLIMATE_SCALE`] so biomes and landmasses
    /// cover more blocks.
    #[inline]
    fn climate_point(&self, x: f64, z: f64) -> (f64, f64) {
        let wx = self.warp_x.sample(x, 0.0, z);
        let wz = self.warp_z.sample(x, 0.0, z);
        (x + wx * WARP_BLOCKS, z + wz * WARP_BLOCKS)
    }

    #[inline]
    fn climate_sample(&self, index: usize, x: f64, z: f64, world: bool) -> f64 {
        let scale = if world { CLIMATE_SCALES[index] } else { 1.0 };
        self.climate[index].sample(x * scale, 0.0, z * scale)
    }

    /// High-frequency surface detail in density-offset units (1 block = 1/32):
    /// it removes the "contour rings" a pure low-frequency field leaves behind
    /// and keeps shorelines from being smooth arcs.
    /// How strongly the column belongs to a river: 1 on the channel centre, 0
    /// outside the band.
    #[inline]
    fn river_strength(&self, x: f64, z: f64) -> f64 {
        let distance = self.river.sample(x, 0.0, z).abs();
        if distance >= RIVER_HALF_WIDTH {
            0.0
        } else {
            1.0 - distance / RIVER_HALF_WIDTH
        }
    }

    #[inline]
    fn detail_offset(&self, x: f64, z: f64) -> f64 {
        let relief = self.relief.sample(x, 0.0, z) * (RELIEF_BLOCKS / 32.0);
        let detail = self.detail.sample(x, 0.0, z) * (DETAIL_BLOCKS / 32.0);
        relief + detail
    }

    /// Climate values sampled at an already chosen point of the climate domain.
    #[inline]
    fn climate_at_point(&self, cx: f64, cz: f64) -> ClimateValues {
        ClimateValues {
            temperature: 0.0,
            humidity: 0.0,
            continentalness: self.climate[CLIMATE_CONTINENTALNESS].sample(cx, 0.0, cz),
            erosion: self.climate[CLIMATE_EROSION].sample(cx, 0.0, cz),
            peaks: self.climate[CLIMATE_PEAKS_AND_VALLEYS].sample(cx, 0.0, cz),
            weirdness: self.climate[CLIMATE_WEIRDNESS].sample(cx, 0.0, cz),
        }
    }

    #[inline]
    fn sample_terrain_climate(&self, x: f64, z: f64) -> ClimateValues {
        // The shape of the land is still ours: the spline is fed by our own noise
        // fields, which is the terrain the world was tuned and reviewed against.
        // The biome that goes on top of it comes from cubiomes (see
        // `biome_from_climate`), and the height is what keeps the two honest with
        // each other.
        let (cx, cz) = self.climate_point(x, z);
        ClimateValues {
            temperature: self.climate_sample(CLIMATE_TEMPERATURE, cx, cz, true),
            humidity: self.climate_sample(CLIMATE_HUMIDITY, cx, cz, true),
            continentalness: self.climate_sample(CLIMATE_CONTINENTALNESS, cx, cz, true),
            erosion: self.climate_sample(CLIMATE_EROSION, cx, cz, true),
            peaks: self.climate_sample(CLIMATE_PEAKS_AND_VALLEYS, cx, cz, true),
            weirdness: self.climate_sample(CLIMATE_WEIRDNESS, cx, cz, true),
        }
    }

    /// Climate sampled at the raw world coordinates, with neither warp nor the
    /// scale change. This is the sampler that matches the BDS captures, and it
    /// is what [`BDSTerrain::biome_at_bds`] and the parity tests use.
    #[inline]
    fn sample_bds_climate(&self, x: f64, z: f64) -> ClimateValues {
        self.climate_at_point(x, z)
    }

    /// Surface biome id at a single block column — the exact per-point niduy
    /// sampler (`sampleBiomeNoise(x, 0, z, true, false)` mapped to Bedrock
    /// legacy ids). The chunk fill (`generate_base_chunk`) runs this same
    /// sampler at quart-cell corners and expands per column, so the value
    /// stored for a column is exactly `biome_at` at the containing 4×4
    /// cell's corner.
    pub fn biome_at_bds(&self, x: i32, z: i32) -> u8 {
        let c = self.sample_bds_climate(x as f64, z as f64);
        self.climate_to_biome_from_values(x, z, &c)
    }

    pub fn biome_at(&self, x: i32, z: i32) -> u8 {
        let c = self.sample_terrain_climate(x as f64, z as f64);
        self.surface_biome_from_values(x, z, &c)
    }

    /// Get terrain offset from precomputed climate values
    ///
    /// The spline is scaled by [`TERRAIN_GAIN`] to keep the terrain shape.
    ///
    /// [`LAND_BIAS`] lifts the whole field, which moves the shoreline out to sea.
    /// The sea floor itself is not touched here: see
    /// [`BDSTerrain::sea_floor_height`], which lowers only deep water and does it
    /// smoothly. Amplifying this offset instead (the previous approach) made the
    /// spline's own step at continentalness ≈ −0.47 into a 20 block cliff.
    #[inline]
    fn terrain_offset_from_climate(&self, c: &ClimateValues) -> f64 {
        let peaks = -3.0 * ((c.weirdness.abs() - 0.6666667).abs() - 0.33333334);
        let raw = eval_spline_straight(
            &[c.continentalness, c.erosion, peaks, c.weirdness],
            &self.spline,
        ) + 0.015;
        raw * TERRAIN_GAIN + LAND_BIAS
    }

/// Extra depth, in blocks, that the open ocean adds under a column whose
    /// spline height is `height`. Zero at the shoreline (the spline is still at
    /// sea level there) and full strength a few blocks out, following a
    /// smoothstep of how deep the column already is, modulated by the smooth
    /// relief noise so the seabed undulates instead of being a plate.
    #[inline]
    fn sea_floor_extra(&self, height: i32, x: f64, z: f64) -> f64 {
        let below = (self.water_height - 1 - height) as f64;
        if below <= 0.0 {
            return 0.0;
        }
        let t = (below / SEA_FLOOR_RAMP).min(1.0);
        let ramp = t * t * (3.0 - 2.0 * t);
        let roll = (self.relief.sample(x, 0.0, z) * 0.5 + 0.5).clamp(0.0, 1.0);
        ramp * (6.0 + SEA_FLOOR_MAX_DEPTH * roll)
    }

    /// Density at one block position, given the column's offset.
    ///
    /// The last term is a real 3D noise, not a height decoration: the surface is
    /// the height where this expression crosses zero, so the terrain is the
    /// inverse of a smooth 3D function. That is how vanilla gets an irregular
    /// surface out of a smooth density, and it is what a 2D height field cannot
    /// do — quantising a 2D field to whole blocks is what produced the terraces.
    #[inline]
    fn density_at_with_offset(&self, x: f64, y: i32, z: f64, offset: f64) -> f64 {
        // Full strength everywhere, including the waterline. Fading the noise
        // out near sea level kept the coast a clean curve, but a noise-free
        // monotone height field quantised to blocks is a set of long parallel
        // bands one block high: that is what the shallow shores looked like,
        // first as stairs and then, once the bed was lifted, as flat plates.
        let damp = 1.0;
        let noise = self.depth_noise.sample(x, y as f64 * DEPTH_NOISE_VERTICAL, z)
            * (DEPTH_NOISE_BLOCKS / 32.0)
            * damp;
        2.5 - ((y as f64) * 4.0) / 128.0 - 83.0 / 160.0 + offset + noise
    }

    /// Get height at column using precomputed climate
    #[inline]
    fn height_at_with_climate(&self, c: &ClimateValues, detail: f64, x: f64, z: f64) -> i32 {
        let off = self.terrain_offset_from_climate(c) + detail;
        let limit = (DENSITY_BASE + off) * 32.0;
        let mut h = limit.floor() as i32;
        // The 3D term moves the crossing by at most `DEPTH_NOISE_BLOCKS`, so the
        // walk is short; the guards only keep a pathological noise from looping.
        let mut steps = 0;
        while self.density_at_with_offset(x, h + 1, z, off) > 0.0 && steps < 96 {
            h += 1;
            steps += 1;
        }
        while self.density_at_with_offset(x, h, z, off) <= 0.0 && steps < 192 {
            h -= 1;
            steps += 1;
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
        self.terrain_offset_from_climate(&c) + self.detail_offset(x as f64, z as f64)
    }

    /// Public API: Density at position (x, y, z)
    #[inline]
    pub fn density_at(&self, x: i32, y: i32, z: i32) -> f64 {
        let c = self.sample_terrain_climate(x as f64, z as f64);
        let off = self.terrain_offset_from_climate(&c) + self.detail_offset(x as f64, z as f64);
        self.density_at_with_offset(x as f64, y, z as f64, off)
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
        self.height_at_with_climate(
            &c,
            self.detail_offset(x as f64, z as f64),
            x as f64,
            z as f64,
        )
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
        self.biome_from_climate(x, z, c, false)
    }

    /// Surface biome for a column: same tree as the capture sampler, but the
    /// climate `depth` is taken at the column's own surface height.
    ///
    /// Sampling it at y=0 puts every column deep underground as far as the tree
    /// is concerned, so it answers with cave biomes (lush caves was the second
    /// most common "surface" biome) and those speckles chopped the map into
    /// confetti.
    fn surface_biome_from_values(&self, x: i32, z: i32, c: &ClimateValues) -> u8 {
        self.biome_from_climate(x, z, c, true)
    }

    fn biome_from_climate(&self, x: i32, z: i32, c: &ClimateValues, surface: bool) -> u8 {
        if surface {
            // World path: the climate of this column came from cubiomes, and so
            // does the biome. The height is only needed because 1.18's generator
            // puts rivers and caves in their own depth bands, and because the
            // elevation gates below need it.
            let xf = x as f64;
            let zf = z as f64;
            let detail = self.detail_offset(xf, zf);
            let y = self.height_at_with_climate(c, detail, xf, zf) as f64;
            let java = self
                .cubiomes
                .as_ref()
                .and_then(|generator| generator.biome_at(x, y as i32, z));
            // The ocean variant (and the fallback) must use vanilla's temperature,
            // not ours: our field runs cold and turned lakes into frozen oceans,
            // which then grew a sheet of ice in the middle of a desert.
            let vanilla = self
                .cubiomes
                .as_ref()
                .and_then(|generator| generator.climate_at(x, z));
            let (temperature, humidity) = match vanilla {
                Some(np) => (np[0], np[1]),
                None => (c.temperature, c.humidity),
            };
            let id = match java {
                Some(java) => bds_biome_tree::java_to_bedrock(java),
                // cubiomes could not be set up: answer from the climate instead of
                // failing the chunk.
                None => self.climate_fallback_biome(temperature, humidity),
            };
            return self.fix_biome_elevation(id, y, temperature, humidity);
        }

        // Capture path (1:1 with the BDS screenshots): raw coordinates, our own
        // noise samples and the BDS biome tree.
        let xf = x as f64;
        let zf = z as f64;
        let bx = xf;
        let bz = zf;
        let px = bx + c.peaks * 4.0;
        let peaks_shift = self.climate[CLIMATE_PEAKS_AND_VALLEYS].sample(bz, bx, 0.0);
        let pz = bz + peaks_shift * 4.0;

        let cont = self.climate_sample(CLIMATE_CONTINENTALNESS, px, pz, false);
        let erosion = self.climate_sample(CLIMATE_EROSION, px, pz, false);
        let weirdness = self.climate_sample(CLIMATE_WEIRDNESS, px, pz, false);
        let temperature = self.climate_sample(CLIMATE_TEMPERATURE, px, pz, false);
        let humidity = self.climate_sample(CLIMATE_HUMIDITY, px, pz, false);
        // `1 - y*4/128 - 83/160 + spline(c, e, peaks, w) + 0.015`, at y=0.
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

    /// Keeps elevation-specific biomes on the terrain that justifies them.
    ///
    /// The climate tree answers "which biome does this climate get", and its
    /// mountain branches are keyed on climate, not on how tall the ground came
    /// out. Our spline builds far gentler slopes than 1.18's, so the tree happily
    /// hands out mountain biomes for a `weirdness` ridge the terrain never built:
    /// stone shores spread thirty blocks above the waterline and groves sat at
    /// sea level, which the player sees as grey and snowy "minibiomas" inside a
    /// forest. This is the world sampler only; the BDS capture path
    /// ([`BDSTerrain::biome_at_bds`]) still answers exactly what the tree says.
    fn fix_biome_elevation(
        &self,
        id: u8,
        height: f64,
        temperature: f64,
        humidity: f64,
    ) -> u8 {
        // cubiomes answers with a cave biome when the column's height puts it
        // below the surface climate: the world surface must never be one, or the
        // map fills with specks of lush caves and deep dark.
        if matches!(
            id,
            biome::LUSH_CAVES | biome::DRIPSTONE_CAVES | biome::DEEP_DARK
        ) {
            return self.climate_fallback_biome(temperature, humidity);
        }

        // Ocean biomes are chosen by continentalness, but here the sea is decided
        // by the ground height, so the two disagree along every shelf: the tree
        // handed out ocean for ground up to y=86 and land biomes for lake beds.
        // A dry plain of ocean biome is exactly what reads as a "minibioma" of
        // gravel in the middle of a forest, so whichever way the column went, the
        // biome follows the water.
        let wet = height < self.water_height as f64;
        let ocean = matches!(
            id,
            biome::OCEAN
                | biome::DEEP_OCEAN
                | biome::WARM_OCEAN
                | biome::LUKEWARM_OCEAN
                | biome::DEEP_LUKEWARM_OCEAN
                | biome::COLD_OCEAN
                | biome::DEEP_COLD_OCEAN
                | biome::FROZEN_OCEAN
                | biome::DEEP_FROZEN_OCEAN
        );
        if wet && !ocean {
            let depth = self.water_height as f64 - height;
            return self.ocean_biome(temperature, depth);
        }
        if !wet && ocean {
            return self.climate_fallback_biome(temperature, humidity);
        }

        // cubiomes' rivers follow vanilla's river noise. Our terrain has its own
        // river noise, so the two almost never agree and every vanilla river shows
        // up as a one-block line of dirt across dry ground. Only a column our own
        // water actually covers keeps the river biome.
        if !wet
            && matches!(id, biome::RIVER | biome::FROZEN_RIVER)
        {
            return self.climate_fallback_biome(temperature, humidity);
        }


        // Measured over 48x48 chunks (3 km, seed 12345) before this rule: stone
        // shore y=61..96 (p50 69), grove y=62..125 (p50 87), stony peaks from
        // y=75. Frozen peaks and snowy slopes were already above their floors.
        let (floor, ceiling) = match id {
            // A shore is two blocks above the water at most. The old ceiling of
            // 66 left vanilla's stone shores as grey one-block contours running
            // hundreds of blocks inland, because our terrain is much flatter than
            // the 1.18 one they were drawn for.
            biome::STONE_SHORE | biome::SNOWY_BEACH => (f64::NEG_INFINITY, SHORE_MAX_HEIGHT),
            biome::GROVE => (GROVE_MIN_HEIGHT, f64::INFINITY),
            biome::STONY_PEAKS
            | biome::JAGGED_PEAKS
            | biome::FROZEN_PEAKS
            | biome::GRAVELLY_MOUNTAINS
            | biome::WOODED_MOUNTAINS => (PEAKS_MIN_HEIGHT, f64::INFINITY),
            _ => return id,
        };
        if height >= floor && height <= ceiling {
            return id;
        }
        self.climate_fallback_biome(temperature, humidity)
    }

    /// The ocean biome a body of water of this temperature and depth gets, in
    /// vanilla's terms: frozen, cold, temperate, lukewarm or warm, and deep once
    /// the bed is far enough down.
    fn ocean_biome(&self, temperature: f64, depth: f64) -> u8 {
        let deep = depth >= DEEP_WATER;
        if temperature < -0.45 {
            if deep {
                biome::DEEP_FROZEN_OCEAN
            } else {
                biome::FROZEN_OCEAN
            }
        } else if temperature < -0.1 {
            if deep {
                biome::DEEP_COLD_OCEAN
            } else {
                biome::COLD_OCEAN
            }
        } else if temperature > 0.55 {
            biome::WARM_OCEAN
        } else if temperature > 0.2 {
            if deep {
                biome::DEEP_LUKEWARM_OCEAN
            } else {
                biome::LUKEWARM_OCEAN
            }
        } else if deep {
            biome::DEEP_OCEAN
        } else {
            biome::OCEAN
        }
    }

    /// The plain biome the climate asks for, used where an elevation-specific
    /// biome has no elevation to stand on.
    fn climate_fallback_biome(&self, temperature: f64, humidity: f64) -> u8 {
        if temperature < -0.45 {
            biome::ICE_PLAINS
        } else if temperature < -0.1 {
            biome::SNOWY_TAIGA
        } else if humidity > 0.3 {
            if temperature < 0.15 {
                biome::TAIGA
            } else {
                biome::FOREST
            }
        } else if temperature > 0.55 && humidity < -0.2 {
            biome::DESERT
        } else {
            biome::PLAINS
        }
    }

    /// Biome id of the quart cell whose corner is `(x, z)`, cached because the
    /// smoothing window reads the same cells from every chunk that touches them.
    fn biome_cell(&self, x: i32, z: i32) -> u8 {
        let key = (x >> 2, z >> 2);
        if let Some(id) = self.biome_cells.lock().get(key) {
            return id;
        }
        let c = self.sample_terrain_climate(x as f64, z as f64);
        let id = self.surface_biome_from_values(x, z, &c);
        self.biome_cells
            .lock()
            .insert(key, id, BIOME_CELL_CACHE_CAP);
        id
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
        // Quart cells covering the chunk; each cell value comes from the exact
        // per-point sampler evaluated at the cell corner, so a column on a
        // multiple of 4 stores `biome_at(x, z)` exactly.
        let mut chunk_biomes = [0u8; 256];

        let mut cell_ids = [[0u8; CELL_GRID]; CELL_GRID];
        for (i, qcx) in (CELL_LO..=CELL_HI).enumerate() {
            for (j, qcz) in (CELL_LO..=CELL_HI).enumerate() {
                cell_ids[i][j] = self.biome_cell(base_x + qcx * 4, base_z + qcz * 4);
            }
        }

        // Second pass: the wide vote leaves one-cell notches wherever two biomes
        // meet, because each cell's window resolves on its own. A 3x3 mode over
        // the voted grid dissolves them without touching the patch sizes the wide
        // window just established.
        let mut voted = cell_ids;
        for _ in 0..2 {
            let before = voted;
            for qcx in CELL_LO..=CELL_HI {
                for qcz in CELL_LO..=CELL_HI {
                    let (i, j) = ((qcx - CELL_LO) as usize, (qcz - CELL_LO) as usize);
                    voted[i][j] = majority_at(&before, i, j, 1);
                }
            }
        }
        let cell_ids = voted;

        for qcx in -1..=3 {
            for qcz in -1..=3 {
                // Majority vote over the 3x3 cell neighbourhood: single-cell
                // specks (cave biomes and near-ties in the climate tree) break
                // the biome shapes into confetti, so they are dissolved into
                // whichever biome surrounds them.
                let b = majority_biome(&cell_ids, qcx, qcz);
                let cell_x = qcx * 4;
                let cell_z = qcz * 4;
                for x in cell_x..cell_x + 4 {
                    if !(0..16).contains(&x) {
                        continue;
                    }
                    for z in cell_z..cell_z + 4 {
                        if (0..16).contains(&z) {
                            chunk_biomes[(x as usize) * 16 + z as usize] = b;
                        }
                    }
                }
            }
        }

        let dominant_biome = chunk_biomes[7 * 16 + 7];
        let mut chunk = Chunk::new(dominant_biome);
        chunk.biomes = chunk_biomes;

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
                let mut highest_solid = self.height_at_with_climate(
                    &c,
                    self.detail_offset(world_x as f64, world_z as f64),
                    world_x as f64,
                    world_z as f64,
                );

                // Rivers: carve the column down towards just below sea level as
                // the river noise approaches its zero crossing.
                let river = self.river_strength(world_x as f64, world_z as f64);
                // Rivers follow the low ground: only the middle of the noise band
                // carves, and never through a hill.
                //
                // Both limits are ramps, not cuts. Cutting either one gave the map
                // one block wide walls: where the river got its top block at 0.49
                // and 0.50, the carve jumped from nothing to eight blocks deep, and
                // the same happened at the height cap. Ramping them turns those
                // walls into ordinary banks.
                let centre = ((river - RIVER_CENTRE) / (1.0 - RIVER_CENTRE)).clamp(0.0, 1.0);
                let centre = centre * centre * (3.0 - 2.0 * centre);
                let above = (highest_solid - self.water_height) as f64;
                let height_fade = ((RIVER_MAX_HEIGHT as f64 - above) / RIVER_CAP_FADE).clamp(0.0, 1.0);
                let river = centre * height_fade;

                if river > 0.0 {
                    let bed = self.water_height - RIVER_DEPTH;
                    if highest_solid > bed {
                        let carved =
                            highest_solid as f64 + (bed - highest_solid) as f64 * river;
                        highest_solid = carved.round() as i32;
                    }
                }

                // Open sea: lower the bed smoothly towards a real depth. The
                // spline alone never goes below y≈50 here, so without this every
                // ocean was a sheet of water a few blocks deep. It is a
                // subtraction, not a clamp, so the coast keeps its height and
                // there is no wall where the sea starts.
                let extra = self.sea_floor_extra(highest_solid, world_x as f64, world_z as f64);
                if extra > 0.0 {
                    highest_solid -= extra.round() as i32;
                }

                // Shallow bed, gently: compress the submerged relief by a third.
                // A slope of 0.8 blocks per block puts a one-block contour every
                // block or so, which is the stair look; at 0.5 the contours are
                // twice as far apart. It is a plain scaling, not the saturating
                // lift tried before: the depth order is preserved, so it can
                // never turn a lake into flat plates.
                let depth = self.water_height - highest_solid;
                if depth > 2 {
                    let lift = ((depth - 2) as f64 * SHORE_BED_GAIN).round() as i32;
                    highest_solid += lift;
                }


                // A dry column exactly at sea level is the same thing from the
                // other side: a flat plate level with the water, which reads as a
                // hole with no water in the middle of a lake. Three percent of
                // the map sat exactly there. The depth noise decides which way
                // each column leaves the waterline, so the plate becomes a ragged
                // shore instead of a lid.
                if highest_solid == self.water_height {
                    let n = self.depth_noise.sample(world_x as f64, 0.0, world_z as f64);
                    highest_solid += if n > 0.0 { -1 } else { 1 };
                }


                // Sea is decided on the final bed height, after the shelf,
                // the one-block rule and the sea-level step below: those move
                // the bed, and a column dug under the waterline has to be
                // filled. Computing it before them left dry holes one block
                // below the surface, which is exactly the missing water the
                // player found at 632, 61, -84.
                let wet = highest_solid < self.water_height;

                // Fill from bedrock up to highest solid (`fill_column` resolves
                // the palette entry once per subchunk instead of per block).
                chunk.fill_column(x, z, 1, highest_solid, block_id::STONE);

                // Fill water above the solid ground, up to sea level.
                //
                if wet {
                    // `.max(1)` keeps the bedrock layer at y=0.
                    chunk.fill_column(
                        x,
                        z,
                        (highest_solid + 1).max(1),
                        self.water_height,
                        block_id::WATER,
                    );
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
                let base_chunk = self.cached_base_chunk(cx, cz);
                manager.set_chunk(cx, cz, (*base_chunk).clone());
            }
        }

        let dominant_biome = manager.get_chunk(chunk_x, chunk_z).unwrap().biomes[7 * 16 + 7];
        self.populate_chunk(&mut manager, chunk_x, chunk_z, dominant_biome);

        manager.take_chunk(chunk_x, chunk_z).unwrap()
    }

    /// Generates a `width x depth` block of columns, building every base
    /// chunk in the expanded region only once.
    ///
    /// The per-column path ([`Self::generate_chunk_internal`]) rebuilds the
    /// 3x3 base neighbourhood for every column, so a row-major sweep pays for
    /// ~3 base chunks per column (up to 9 in random order) even when the base
    /// cache is warm: moving one chunk east pulls in a fresh `x + 1` column.
    /// A region needs `(width + 2) x (depth + 2)` base chunks for
    /// `width x depth` columns, i.e. ~1.1 per column for 16x16 tiles, which is
    /// where the pregenerator's ~2x comes from.
    pub fn generate_region(
        &self,
        start_x: i32,
        start_z: i32,
        width: i32,
        depth: i32,
    ) -> Vec<((i32, i32), Chunk)> {
        let mut bases: HashMap<(i32, i32), Arc<Chunk>> = HashMap::new();
        for cx in (start_x - 1)..=(start_x + width) {
            for cz in (start_z - 1)..=(start_z + depth) {
                bases.insert((cx, cz), self.cached_base_chunk(cx, cz));
            }
        }

        let mut out = Vec::with_capacity((width.max(0) * depth.max(0)) as usize);
        for cx in start_x..(start_x + width) {
            for cz in start_z..(start_z + depth) {
                let mut manager = ChunkManager3x3::new(cx, cz);
                for dx in -1..=1 {
                    for dz in -1..=1 {
                        let key = (cx + dx, cz + dz);
                        let base = bases
                            .get(&key)
                            .cloned()
                            .unwrap_or_else(|| self.cached_base_chunk(key.0, key.1));
                        manager.set_chunk(key.0, key.1, (*base).clone());
                    }
                }
                let dominant_biome = manager.get_chunk(cx, cz).unwrap().biomes[7 * 16 + 7];
                self.populate_chunk(&mut manager, cx, cz, dominant_biome);
                out.push(((cx, cz), manager.take_chunk(cx, cz).unwrap()));
            }
        }
        out
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
                self.underwater
                    .populate(world, chunk_x, chunk_z, &mut random, dominant_biome);
            }
            biome::SWAMPLAND | biome::MANGROVE_SWAMP => {
                self.underwater
                    .populate(world, chunk_x, chunk_z, &mut random, dominant_biome);
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
                self.underwater
                    .populate(world, chunk_x, chunk_z, &mut random, dominant_biome);
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
        if let Some(chunk) = self.column_cache.lock().get(key) {
            return chunk;
        }
        let chunk = Arc::new(self.generate_chunk_internal(chunk_x, chunk_z));
        self.column_cache
            .lock()
            .insert(key, Arc::clone(&chunk), COLUMN_CACHE_CAP);
        chunk
    }

    fn cached_base_chunk(&self, chunk_x: i32, chunk_z: i32) -> Arc<Chunk> {
        let key = (chunk_x, chunk_z);
        if let Some(chunk) = self.base_cache.lock().get(key) {
            return chunk;
        }
        let chunk = Arc::new(self.generate_base_chunk(chunk_x, chunk_z));
        self.base_cache
            .lock()
            .insert(key, Arc::clone(&chunk), BASE_CACHE_CAP);
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

    /// Batched path: one build per base chunk for the whole region (see
    /// [`BDSTerrain::generate_region`]).
    fn generate_region(
        &self,
        start_x: i32,
        start_z: i32,
        width: i32,
        depth: i32,
    ) -> Vec<((i32, i32), Chunk)> {
        BDSTerrain::generate_region(self, start_x, start_z, width, depth)
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

    /// A region must produce exactly the columns the per-column path builds
    /// (same base terrain and population), only generating each base chunk
    /// once instead of once per neighbour.
    #[test]
    fn region_generation_matches_per_column() {
        let terrain = BDSTerrain::new(12345);
        let region = terrain.generate_region(7, 9, 2, 2);
        assert_eq!(region.len(), 4);
        for ((cx, cz), chunk) in region {
            let expected = terrain.generate_chunk_internal(cx, cz);
            assert_eq!(chunk.biomes, expected.biomes, "biomes in ({cx},{cz})");
            for x in 0..16usize {
                for z in 0..16usize {
                    for y in 0..128 {
                        assert_eq!(
                            chunk.get_block(x, y, z),
                            expected.get_block(x, y, z),
                            "block at ({x},{y},{z}) in ({cx},{cz})"
                        );
                    }
                }
            }
        }
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
            let got = terrain.biome_at_bds(x, z);
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
            assert_eq!(terrain.biome_at_bds(x, 0), *want, "strip biome at x={x}");
        }

        // The same points through the capture sampler: it samples the climate at
        // y=0, which is what the BDS captures hold.
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
            assert_eq!(terrain.biome_at_bds(x, z), want, "capture biome at ({x},{z})");
        }

        // The world sampler classifies at the column's own surface height, so it
        // never answers with a cave biome; those were the specks that turned the
        // surface map into confetti.
        for x in (-2048..2048).step_by(37) {
            for z in (-2048..2048).step_by(53) {
                let id = terrain.biome_at(x, z);
                assert!(
                    !matches!(
                        id,
                        biome::LUSH_CAVES | biome::DRIPSTONE_CAVES | biome::DEEP_DARK
                    ),
                    "surface biome at ({x},{z}) was a cave biome: {id}"
                );
            }
        }

        // The chunk fill expands one value per quart cell, so the sixteen columns
        // of a cell always agree.
        for (cx, cz) in [(0, 0), (-31, 7), (250, -250)] {
            let chunk = terrain.generate_base_chunk(cx, cz);
            let mut cells = std::collections::HashMap::new();
            for x in 0..16usize {
                for z in 0..16usize {
                    let id = chunk.biomes[x * 16 + z];
                    let cell = cells.entry((x / 4, z / 4)).or_insert(id);
                    assert_eq!(*cell, id, "chunk ({cx},{cz}) cell ({},{}) differs", x / 4, z / 4);
                }
            }
        }
    }
}