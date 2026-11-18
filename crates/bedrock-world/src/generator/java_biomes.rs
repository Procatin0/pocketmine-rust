//! Exact Java Edition 1.18+ climate sampling, as used by Bedrock 1.18+.
//!
//! Bedrock and Java produce the same biomes for the same 64-bit seed (Mojang
//! documents the parity since 1.18.30 and it has been verified against real
//! BDS captures, see `docs/bds-biome-parity.md`), so the generator is
//! reimplemented here from the Java specification instead of being tuned by
//! hand.
//!
//! The pipeline per column is:
//!
//! 1. Six `DoublePerlinNoise` fields (offset, temperature, vegetation,
//!    continents, erosion, ridge), each with its own octave stack whose seeds
//!    are derived from the world seed with `Xoroshiro128++` and the MD5 of the
//!    noise name.
//! 2. A coordinate shift driven by the "offset" noise.
//! 3. The `depth` parameter, which is the terrain offset spline (continents,
//!    erosion, ridges, weirdness) evaluated at the column and combined with
//!    the sampled Y.
//! 4. A nearest-neighbour search over the vanilla biome parameter points
//!    (`bds_biome_tree`), which yields the biome id.
//!
//! Every constant below is part of the public worldgen format (MD5-derived
//! seed salts, per-noise octave amplitudes and minimum octave); none of it is
//! copied from Mojang's code.

use super::bds_biome_tree;
use super::java_biome_tree;
use super::java_terrain::{eval_spline, terrain_spline, Spline};

/// Climate noise slots. The first four and the ridge feed the biome search;
/// the offset noise is only used to distort the sample position.
pub const NP_TEMPERATURE: usize = 0;
pub const NP_VEGETATION: usize = 1;
pub const NP_CONTINENTS: usize = 2;
pub const NP_EROSION: usize = 3;
pub const NP_OFFSET: usize = 4;
pub const NP_RIDGE: usize = 5;

/// `Xoroshiro128++`, the PRNG Java uses for worldgen since 1.18.
#[derive(Debug, Clone, Copy)]
pub struct Xoroshiro128PlusPlus {
    pub lo: u64,
    pub hi: u64,
}

const XL: u64 = 0x9e37_79b9_7f4a_7c15;
const XH: u64 = 0x6a09_e667_f3bc_c909;

impl Xoroshiro128PlusPlus {
    pub fn from_seed(seed: u64) -> Self {
        let l = seed ^ XH;
        let h = l.wrapping_add(XL);
        let l = (l ^ (l >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        let h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        let l = (l ^ (l >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        let h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Self {
            lo: l ^ (l >> 31),
            hi: h ^ (h >> 31),
        }
    }

    #[inline]
    pub fn next_long(&mut self) -> u64 {
        let l = self.lo;
        let h = self.hi;
        let result = l.wrapping_add(h).rotate_left(17).wrapping_add(l);
        let h = h ^ l;
        self.lo = l.rotate_left(49) ^ h ^ (h << 21);
        self.hi = h.rotate_left(28);
        result
    }

    #[inline]
    fn next_double(&mut self) -> f64 {
        (self.next_long() >> 11) as f64 * 1.110_223_024_625_156_5E-16
    }

    /// `nextInt` as implemented by Java's `XoroshiroRandomSource`.
    fn next_int(&mut self, n: u32) -> u32 {
        let mut r = (self.next_long() & 0xffff_ffff).wrapping_mul(n as u64);
        if (r as u32) < n {
            let threshold = (n as u64).wrapping_neg() % n as u64;
            while (r as u32) < threshold as u32 {
                r = (self.next_long() & 0xffff_ffff).wrapping_mul(n as u64);
            }
        }
        (r >> 32) as u32
    }
}

/// Per-octave seed salts: `md5("octave_<n>")` for n = -16..=0, which covers
/// every octave the climate and terrain noises use.
const OCTAVE_SALTS: [[u64; 2]; 17] = [
    [0xc613_bf76_6619_f992, 0x9547_53f8_6691_b86a], // octave_-16
    [0x7eee_475a_921c_6cf5, 0xf2bd_3942_6f8d_a413], // octave_-15
    [0xfc00_27ce_f968_3114, 0xb758_d395_4dcb_fdd3], // octave_-14
    [0xd1fc_8a05_be56_5eca, 0xdc2a_3915_cbdd_a25b], // octave_-13
    [0xb198_de63_a801_2672, 0x7b84_cad4_3ef7_b5a8], // octave_-12
    [0x0fd7_87bf_bc40_3ec3, 0x74a4_a31c_a21b_48b8], // octave_-11
    [0x36d3_26ee_d40e_feb2, 0x5be9_ce18_223c_636a], // octave_-10
    [0x082f_e255_f8be_6631, 0x4e96_119e_22de_dc81], // octave_-9
    [0x0ef6_8ec6_8504_005e, 0x48b6_bf93_a278_9640], // octave_-8
    [0xf112_6812_8982_754f, 0x257a_1d67_0430_b0aa], // octave_-7
    [0xe51c_98ce_7d1d_e664, 0x5f94_78a7_3304_0c45], // octave_-6
    [0x6d7b_49e7_e429_850a, 0x2e30_63c6_22a2_4777], // octave_-5
    [0xbd90_d537_7ba1_b762, 0xc073_17d4_19a7_548d], // octave_-4
    [0x53d3_9c67_52da_c858, 0xbcd1_c5a8_0ab6_5b3e], // octave_-3
    [0xb4a2_4d7a_84e7_677b, 0x023f_f966_8e89_b5c4], // octave_-2
    [0xdffa_22b5_34c5_f608, 0xb9b6_7517_d366_5ca9], // octave_-1
    [0xd507_0808_6cef_4d7c, 0x6e16_51ec_c7f4_3309], // octave_0
];

/// A single octave of improved Perlin noise, seeded the Java way.
#[derive(Debug, Clone)]
pub struct Perlin {
    a: f64,
    b: f64,
    c: f64,
    permutation: [u8; 257],
    amplitude: f64,
    lacunarity: f64,
    floor_b: i32,
    frac_b: f64,
    fade_b: f64,
}

impl Perlin {
    pub(crate) fn from_xoroshiro(random: &mut Xoroshiro128PlusPlus) -> Self {
        let a = random.next_double() * 256.0;
        let b = random.next_double() * 256.0;
        let c = random.next_double() * 256.0;
        let mut permutation = [0u8; 257];
        for (i, slot) in permutation.iter_mut().take(256).enumerate() {
            *slot = i as u8;
        }
        for i in 0..256usize {
            let j = random.next_int((256 - i) as u32) as usize + i;
            permutation.swap(i, j);
        }
        permutation[256] = permutation[0];
        let floor_b = b.floor();
        let frac_b = b - floor_b;
        let fade_b = frac_b * frac_b * frac_b * (frac_b * (frac_b * 6.0 - 15.0) + 10.0);
        Self {
            a,
            b,
            c,
            permutation,
            amplitude: 1.0,
            lacunarity: 1.0,
            floor_b: floor_b as i32,
            frac_b,
            fade_b,
        }
    }

    /// Java's gradient dot product for one corner, encoded as a switch over the
    /// low four bits of the hash.
    #[inline]
    fn corner(hash: u8, x: f64, y: f64, z: f64) -> f64 {
        match hash & 0xf {
            0 => x + y,
            1 => -x + y,
            2 => x - y,
            3 => -x - y,
            4 => x + z,
            5 => -x + z,
            6 => x - z,
            7 => -x - z,
            8 => y + z,
            9 => -y + z,
            10 => y - z,
            11 => -y - z,
            12 => x + y,
            13 => -y + z,
            14 => -x + y,
            _ => -y - z,
        }
    }

    #[inline]
    fn fade(t: f64) -> f64 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    #[inline]
    fn lerp(t: f64, from: f64, to: f64) -> f64 {
        from + t * (to - from)
    }

    /// `PerlinNoiseSampler.sample(x, y, z, yScale, yMax)`, the variant that
    /// smears the vertical coordinate by snapping it to `yScale` steps.
    pub(crate) fn sample_smeared(
        &self,
        x: f64,
        y: f64,
        z: f64,
        y_scale: f64,
        y_max: f64,
    ) -> f64 {
        let d = x + self.a;
        let e = y + self.b;
        let f = z + self.c;
        let (i, g) = (d.floor(), d - d.floor());
        let (j, h) = (e.floor(), e - e.floor());
        let (k, l) = (f.floor(), f - f.floor());
        let n = if y_scale != 0.0 {
            let m = if y_max >= 0.0 && y_max < h { y_max } else { h };
            (m / y_scale + 1.0e-7).floor() * y_scale
        } else {
            0.0
        };
        let map = |value: i32| self.permutation[(value & 0xff) as usize] as i32;
        let (sx, sy, sz) = (i as i32, j as i32, k as i32);
        let (lx, ly, lz) = (g, h - n, l);
        let (h0, h1) = (map(sx), map(sx + 1));
        let (a0, a1) = (map(h0 + sy), map(h0 + sy + 1));
        let (b0, b1) = (map(h1 + sy), map(h1 + sy + 1));
        let d000 = Self::corner(map(a0 + sz) as u8, lx, ly, lz);
        let d100 = Self::corner(map(b0 + sz) as u8, lx - 1.0, ly, lz);
        let d010 = Self::corner(map(a1 + sz) as u8, lx, ly - 1.0, lz);
        let d110 = Self::corner(map(b1 + sz) as u8, lx - 1.0, ly - 1.0, lz);
        let d001 = Self::corner(map(a0 + sz + 1) as u8, lx, ly, lz - 1.0);
        let d101 = Self::corner(map(b0 + sz + 1) as u8, lx - 1.0, ly, lz - 1.0);
        let d011 = Self::corner(map(a1 + sz + 1) as u8, lx, ly - 1.0, lz - 1.0);
        let d111 = Self::corner(map(b1 + sz + 1) as u8, lx - 1.0, ly - 1.0, lz - 1.0);
        let fade_x = Self::fade(lx);
        let fade_y = Self::fade(h);
        let fade_z = Self::fade(lz);
        Self::lerp(
            fade_z,
            Self::lerp(
                fade_y,
                Self::lerp(fade_x, d000, d100),
                Self::lerp(fade_x, d010, d110),
            ),
            Self::lerp(
                fade_y,
                Self::lerp(fade_x, d001, d101),
                Self::lerp(fade_x, d011, d111),
            ),
        )
    }

    fn sample(&self, mut x: f64, mut y: f64, mut z: f64) -> f64 {
        // The caller passes y = 0 for the 2D climate noises; reuse the
        // precomputed offset in that case, exactly like the reference.
        let (hash_y, fade_y) = if y == 0.0 {
            // A zero Y means "use the precomputed B offset": the coordinate
            // itself becomes that offset's fraction, not zero.
            y = self.frac_b;
            (self.floor_b, self.fade_b)
        } else {
            y += self.b;
            let floor_y = y.floor();
            y -= floor_y;
            (floor_y as i32, Self::fade(y))
        };

        x += self.a;
        z += self.c;
        let floor_x = x.floor();
        let floor_z = z.floor();
        x -= floor_x;
        z -= floor_z;

        let idx = &self.permutation;
        let hx = floor_x as i32 as u8;
        let hy = hash_y as u8;
        let hz = floor_z as i32 as u8;

        let fade_x = Self::fade(x);
        let fade_z = Self::fade(z);

        // Corner hashes. The X step is the `+1` on the horizontal lookup and
        // the Y step is the `+1` on the next one, both wrapping modulo 256.
        let x_hashes = [
            idx[hx as usize].wrapping_add(hy),
            idx[hx as usize + 1].wrapping_add(hy),
        ];
        let mut corners = [0u8; 8];
        for (x_side, base) in x_hashes.iter().enumerate() {
            for y_side in 0..2 {
                let z_base = idx[*base as usize + y_side];
                for z_side in 0..2 {
                    corners[x_side * 4 + y_side * 2 + z_side] =
                        idx[z_base.wrapping_add(hz) as usize + z_side];
                }
            }
        }
        let at = |x_side: usize, y_side: usize, z_side: usize| {
            corners[x_side * 4 + y_side * 2 + z_side]
        };

        let (dx, dy, dz) = (x - 1.0, y - 1.0, z - 1.0);
        let near_x = |y_side: usize, z_side: usize, yy: f64, zz: f64| {
            Self::lerp(
                fade_x,
                Self::corner(at(0, y_side, z_side), x, yy, zz),
                Self::corner(at(1, y_side, z_side), dx, yy, zz),
            )
        };
        let z_near = Self::lerp(fade_y, near_x(0, 0, y, z), near_x(1, 0, dy, z));
        let z_far = Self::lerp(fade_y, near_x(0, 1, y, dz), near_x(1, 1, dy, dz));
        Self::lerp(fade_z, z_near, z_far)
    }
}

/// A stack of octaves of [`Perlin`] noise.
#[derive(Debug, Clone)]
pub struct OctaveNoise {
    octaves: Vec<Perlin>,
}

impl OctaveNoise {
    /// Same, but with the octave amplitudes halved at each step like the
    /// climate `DoublePerlinNoise` does, so the low octaves dominate.
    pub(crate) fn decaying(
        random: &mut Xoroshiro128PlusPlus,
        amplitudes: &[f64],
        minimum_octave: i32,
    ) -> Self {
        let low = random.next_long();
        let high = random.next_long();
        let mut lacunarity = 2f64.powi(minimum_octave);
        let count = amplitudes.len().max(1);
        let mut persistence = (1u64 << (count - 1)) as f64 / ((1u64 << count) as f64 - 1.0);
        let mut octaves = Vec::with_capacity(amplitudes.len());
        for (i, amplitude) in amplitudes.iter().enumerate() {
            if *amplitude != 0.0 {
                let salt = OCTAVE_SALTS[(16 + minimum_octave) as usize + i];
                let mut octave_random = Xoroshiro128PlusPlus {
                    lo: low ^ salt[0],
                    hi: high ^ salt[1],
                };
                let mut perlin = Perlin::from_xoroshiro(&mut octave_random);
                perlin.amplitude = amplitude * persistence;
                perlin.lacunarity = lacunarity;
                octaves.push(perlin);
            }
            lacunarity *= 2.0;
            persistence *= 0.5;
        }
        OctaveNoise { octaves }
    }

    /// Public access to the octave sum.
    pub(crate) fn sample_at(&self, x: f64, y: f64, z: f64) -> f64 {
        self.sample(x, y, z)
    }

    fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut value = 0.0;
        for perlin in &self.octaves {
            let scale = perlin.lacunarity;
            value += perlin.amplitude * perlin.sample(x * scale, y * scale, z * scale);
        }
        value
    }
}

/// Two octave stacks sampled at slightly different frequencies.
#[derive(Debug, Clone)]
pub struct DoublePerlinNoise {
    first: OctaveNoise,
    second: OctaveNoise,
    amplitude: f64,
}

impl DoublePerlinNoise {
    /// Builds a noise from its amplitude table, minimum octave and octave count.
    fn new(
        random: &mut Xoroshiro128PlusPlus,
        amplitudes: &[f64],
        minimum_octave: i32,
    ) -> Self {
        let first = Self::octaves(random, amplitudes, minimum_octave);
        let second = Self::octaves(random, amplitudes, minimum_octave);
        // Zero amplitudes at either end do not produce octaves, but they are
        // excluded from the normalisation too: the amplitude is
        // `(10/6) * len / (len + 1)` over the trimmed length.
        let mut start = 0;
        while start < amplitudes.len() && amplitudes[start] == 0.0 {
            start += 1;
        }
        let mut end = amplitudes.len();
        while end > start && amplitudes[end - 1] == 0.0 {
            end -= 1;
        }
        let len = end - start;
        let amplitude = (10.0 / 6.0) * len as f64 / (len as f64 + 1.0);
        Self {
            first,
            second,
            amplitude,
        }
    }

    fn octaves(
        random: &mut Xoroshiro128PlusPlus,
        amplitudes: &[f64],
        minimum_octave: i32,
    ) -> OctaveNoise {
        let low = random.next_long();
        let high = random.next_long();
        let mut lacunarity = 2f64.powi(minimum_octave);
        // Persistence is normalised so the octave amplitudes sum to one: the
        // first octave gets `2^(n-1) / (2^n - 1)` and each following one
        // doubles.
        let count = amplitudes.len().max(1);
        let mut persistence = (1u64 << (count - 1)) as f64 / ((1u64 << count) as f64 - 1.0);
        let mut octaves = Vec::with_capacity(amplitudes.len());
        for (i, amplitude) in amplitudes.iter().enumerate() {
            if *amplitude != 0.0 {
                let salt = OCTAVE_SALTS[(16 + minimum_octave) as usize + i];
                let mut octave_random = Xoroshiro128PlusPlus {
                    lo: low ^ salt[0],
                    hi: high ^ salt[1],
                };
                let mut perlin = Perlin::from_xoroshiro(&mut octave_random);
                perlin.amplitude = amplitude * persistence;
                perlin.lacunarity = lacunarity;
                octaves.push(perlin);
            }
            lacunarity *= 2.0;
            persistence *= 0.5;
        }
        OctaveNoise { octaves }
    }

    /// Internal state of the first octave stack, for bit-exact validation
    /// against a Java generator implementation.
    pub fn debug_octaves(&self) -> Vec<(f64, f64, f64, f64, f64, Vec<u8>)> {
        self.first
            .octaves
            .iter()
            .map(|p| {
                (
                    p.amplitude,
                    p.lacunarity,
                    p.a,
                    p.b,
                    p.c,
                    p.permutation[..8].to_vec(),
                )
            })
            .collect()
    }

    pub fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        const FREQUENCY: f64 = 337.0 / 331.0;
        let value = self.first.sample(x, y, z)
            + self
                .second
                .sample(x * FREQUENCY, y * FREQUENCY, z * FREQUENCY);
        value * self.amplitude
    }
}

/// Seed salts: `md5("minecraft:<noise name>")`.
mod salts {
    pub const OFFSET: (u64, u64) = (0x0805_18cf_6af2_5384, 0x3f3d_fb40_a54f_ebd5);
    pub const TEMPERATURE: (u64, u64) = (0x5c7e_6b29_735f_0d7f, 0xf7d8_6f1b_bc73_4988);
    pub const VEGETATION: (u64, u64) = (0x81bb_4d22_e8dc_168e, 0xf1c8_b4be_a163_03cd);
    pub const CONTINENTS: (u64, u64) = (0x8388_6c9d_0ae3_a662, 0xafa6_38a6_1b42_e8ad);
    pub const EROSION: (u64, u64) = (0xd024_91e6_058f_6fd8, 0x4792_512c_94c1_7a80);
    pub const RIDGE: (u64, u64) = (0xefc8_ef4d_3610_2b34, 0x1bee_eb32_4a0f_24ea);
}

/// Per-noise amplitude tables and minimum octaves (vanilla noise settings).
const NOISE_CONFIGS: [(&[f64], i32); 6] = [
    // minecraft:temperature
    (&[1.5, 0.0, 1.0, 0.0, 0.0, 0.0], -10),
    // minecraft:vegetation
    (&[1.0, 1.0, 0.0, 0.0, 0.0, 0.0], -8),
    // minecraft:continentalness
    (&[1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0], -9),
    // minecraft:erosion
    (&[1.0, 1.0, 0.0, 1.0, 1.0], -9),
    // minecraft:offset (noise used to shift the sample position)
    (&[1.0, 1.0, 1.0, 0.0], -3),
    // minecraft:ridge
    (&[1.0, 2.0, 1.0, 0.0, 0.0, 0.0], -7),
];

const NOISE_SALTS: [(u64, u64); 6] = [
    salts::TEMPERATURE,
    salts::VEGETATION,
    salts::CONTINENTS,
    salts::EROSION,
    salts::OFFSET,
    salts::RIDGE,
];

/// Java 1.18+ biome sampler for one seed.
#[derive(Debug, Clone)]
pub struct JavaBiomes {
    seed: u64,
    climate: [DoublePerlinNoise; 6],
    spline: Spline,
}

impl JavaBiomes {
    pub fn new(seed: u64) -> Self {
        let mut positional = Xoroshiro128PlusPlus::from_seed(seed);
        let low = positional.next_long();
        let high = positional.next_long();
        let climate = std::array::from_fn(|index| {
            let (salt_low, salt_high) = NOISE_SALTS[index];
            let mut random = Xoroshiro128PlusPlus {
                lo: low ^ salt_low,
                hi: high ^ salt_high,
            };
            let (amplitudes, minimum_octave) = NOISE_CONFIGS[index];
            DoublePerlinNoise::new(&mut random, amplitudes, minimum_octave)
        });
        Self {
            seed,
            climate,
            spline: terrain_spline(),
        }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Debug access to one climate noise's first octave stack.
    pub fn debug_climate(&self, index: usize) -> Vec<(f64, f64, f64, f64, f64, Vec<u8>)> {
        self.climate[index].debug_octaves()
    }

    /// Raw (unshifted, unquantised) climate noise sample, for validation.
    pub fn noise_at(&self, index: usize, x: f64, z: f64) -> f64 {
        self.climate[index].sample(x, 0.0, z)
    }

    /// One raw perlin octave sample, for validation.
    pub fn debug_perlin(&self, index: usize, octave: usize, x: f64, y: f64, z: f64) -> f64 {
        self.climate[index].first.octaves[octave].sample(x, y, z)
    }

    /// The six quantised climate parameters Java feeds to the biome search:
    /// `[temperature, vegetation, continents, erosion, depth, ridge]`.
    pub fn climate_at(&self, x: i32, y: i32, z: i32) -> [i32; 6] {
        self.climate_at_f64(x as f64, y as f64, z as f64)
    }

    /// Climate sample at fractional quart coordinates, which is what a
    /// sampler working at block resolution needs (`block / 4`).
    pub fn climate_at_f64(&self, x: f64, y: f64, z: f64) -> [i32; 6] {
        // Coordinate shift driven by the "offset" noise. The second component
        // deliberately samples with the axes swapped.
        let shifted_x = x + self.climate[NP_OFFSET].sample(x, 0.0, z) * 4.0;
        let shifted_z = z + self.climate[NP_OFFSET].sample(z, x, 0.0) * 4.0;

        let continents = self.climate[NP_CONTINENTS].sample(shifted_x, 0.0, shifted_z) as f32;
        let erosion = self.climate[NP_EROSION].sample(shifted_x, 0.0, shifted_z) as f32;
        let ridge = self.climate[NP_RIDGE].sample(shifted_x, 0.0, shifted_z) as f32;

        // Depth: the terrain offset spline (in f32, like the reference)
        // combined with the sampled Y.
        let peaks = -3.0f32 * ((ridge.abs() - 0.666_666_7f32).abs() - 0.333_333_34f32);
        let offset = eval_spline(
            &[
                continents as f64,
                erosion as f64,
                peaks as f64,
                ridge as f64,
            ],
            &self.spline,
        ) + 0.015;
        let depth = (1.0 - (y as f64 * 4.0) / 128.0 - 83.0 / 160.0 + offset) as f32;

        let temperature = self.climate[NP_TEMPERATURE].sample(shifted_x, 0.0, shifted_z) as f32;
        let vegetation = self.climate[NP_VEGETATION].sample(shifted_x, 0.0, shifted_z) as f32;

        [
            (10000.0f32 * temperature) as i32,
            (10000.0f32 * vegetation) as i32,
            (10000.0f32 * continents) as i32,
            (10000.0f32 * erosion) as i32,
            (10000.0f32 * depth) as i32,
            (10000.0f32 * ridge) as i32,
        ]
    }

    /// Java biome id for a position in *quart* coordinates, i.e. block
    /// coordinate divided by four, which is the resolution the vanilla climate
    /// sampler works at.
    pub fn java_biome_at_quart(&self, x: i32, y: i32, z: i32) -> u8 {
        java_biome_tree::climate_to_biome(&self.climate_at(x, y, z))
    }

    /// Java biome id at a block position.
    pub fn java_biome_at(&self, x: i32, y: i32, z: i32) -> u8 {
        self.java_biome_at_quart(x >> 2, y >> 2, z >> 2)
    }

    /// Bedrock (legacy) biome id at a block position.
    pub fn biome_at(&self, x: i32, y: i32, z: i32) -> u8 {
        bds_biome_tree::java_to_bedrock(self.java_biome_at(x, y, z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xoroshiro_matches_java_vectors() {
        // Xoroshiro128++ first outputs for seed 0, shared with the Java
        // implementation this port is verified against.
        let mut random = Xoroshiro128PlusPlus::from_seed(0);
        assert_eq!(random.next_long(), 0x2a2c_a488_f66f_517e);
        assert_eq!(random.next_long(), 0xccbc_22d7_2e97_c372);
    }

    #[test]
    fn climate_values_are_bounded() {
        let biomes = JavaBiomes::new(0);
        for x in (-512..512).step_by(37) {
            for z in (-512..512).step_by(41) {
                let climate = biomes.climate_at(x, 64, z);
                for value in climate {
                    assert!(value.abs() < 60_000, "climate {value} out of range");
                }
            }
        }
    }

    #[test]
    fn ocean_seed_produces_ocean_biomes() {
        // Seed 1 spawns in an ocean; the generator must not report land there.
        let biomes = JavaBiomes::new(1);
        let mut ocean = 0;
        for x in 0..16 {
            for z in 0..16 {
                let id = biomes.biome_at(x, 64, z);
                if matches!(id, 0 | 10 | 24 | 25 | 26 | 40..=47) {
                    ocean += 1;
                }
            }
        }
        assert!(ocean > 200, "expected an ocean, got {ocean}/256 water columns");
    }
}
