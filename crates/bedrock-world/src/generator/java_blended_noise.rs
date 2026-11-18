//! Vanilla's `old_blended_noise` density function (`base_3d_noise`).
//!
//! The Overworld terrain adds a classic 1.17-style noise to the spline shape.
//! It blends a *lower* and an *upper* limit noise with a third "main" noise, so
//! the field is smooth in the vertical direction (the historical "smear" of
//! pre-1.18 terrain) while still varying horizontally.
//!
//! The vanilla function is created with `Xoroshiro128PlusPlusRandom(0L)`, i.e.
//! it does **not** depend on the world seed: every world has the same
//! `base_3d_noise` field. The octave samplers are seeded sequentially from that
//! random source (the "legacy" path), which is what `new_blended` does.

use super::java_biomes::{Perlin, Xoroshiro128PlusPlus};

/// Vanilla multiplies both scales by this before sampling.
const COORDINATE_SCALE: f64 = 684.412;

/// Blended classic terrain noise.
#[derive(Debug, Clone)]
pub struct BlendedNoise {
    lower: Vec<Perlin>,
    upper: Vec<Perlin>,
    main: Vec<Perlin>,
    xz_multiplier: f64,
    y_multiplier: f64,
    xz_factor: f64,
    y_factor: f64,
    smear_scale_multiplier: f64,
}

impl BlendedNoise {
    /// Builds the sampler the way `base_3d_noise` does: a zero-seeded random
    /// source and the vanilla parameters.
    pub fn base_3d_noise() -> Self {
        Self::new(0.25, 0.125, 80.0, 160.0, 8.0)
    }

    /// Builds a sampler with explicit parameters (vanilla's public
    /// `createUnseeded` parameters).
    pub fn new(
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> Self {
        let mut random = Xoroshiro128PlusPlus::from_seed(0);
        // The three stacks are drawn one after the other from the same source:
        // 16 octaves for the limits (-15..=0) and 8 for the main noise (-7..=0).
        let mut build = |octaves: usize| -> Vec<Perlin> {
            (0..octaves)
                .map(|_| Perlin::from_xoroshiro(&mut random))
                .collect()
        };
        let lower = build(16);
        let upper = build(16);
        let main = build(8);
        Self {
            lower,
            upper,
            main,
            xz_multiplier: COORDINATE_SCALE * xz_scale,
            y_multiplier: COORDINATE_SCALE * y_scale,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
        }
    }

    /// Samples the blended noise at a block position.
    pub fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        let limit_x = x * self.xz_multiplier;
        let limit_y = y * self.y_multiplier;
        let limit_z = z * self.xz_multiplier;
        let main_x = limit_x / self.xz_factor;
        let main_y = limit_y / self.y_factor;
        let main_z = limit_z / self.xz_factor;
        let limit_smear = self.y_multiplier * self.smear_scale_multiplier;
        let main_smear = limit_smear / self.y_factor;

        // Main noise decides where between the two limit noises we land.
        let mut main_value = 0.0;
        let mut pow = 1.0;
        for octave in &self.main {
            main_value += octave.sample_smeared(
                main_x * pow,
                main_y * pow,
                main_z * pow,
                main_smear * pow,
                main_y * pow,
            ) / pow;
            pow /= 2.0;
        }

        let factor = (main_value / 10.0 + 1.0) / 2.0;
        let use_lower = factor < 1.0;
        let use_upper = factor > 0.0;

        let mut blend_lower = 0.0;
        let mut blend_upper = 0.0;
        let mut pow = 1.0;
        for index in 0..16 {
            let x = limit_x * pow;
            let y = limit_y * pow;
            let z = limit_z * pow;
            let smear = limit_smear * pow;
            if use_lower {
                if let Some(octave) = self.lower.get(index) {
                    blend_lower += octave.sample_smeared(x, y, z, smear, y) / pow;
                }
            }
            if use_upper {
                if let Some(octave) = self.upper.get(index) {
                    blend_upper += octave.sample_smeared(x, y, z, smear, y) / pow;
                }
            }
            pow /= 2.0;
        }

        let lower = blend_lower / 512.0;
        let upper = blend_upper / 512.0;
        let blended = if factor <= 0.0 {
            lower
        } else if factor >= 1.0 {
            upper
        } else {
            lower + factor * (upper - lower)
        };
        blended / 128.0
    }
}
