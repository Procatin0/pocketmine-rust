//! Java 1.18+ terrain density functions for the Overworld.
//!
//! The vanilla generator describes the terrain as a graph of *density
//! functions* (`worldgen/density_function/**`) evaluated at arbitrary
//! coordinates: the result is positive inside solid rock and negative in air.
//! The graph, the splines and the noise parameters are game data and live in
//! `crates/bedrock-world/data/` (see the README there); the evaluation below is
//! our own implementation.
//!
//! Everything is plain `f64` arithmetic: the vanilla implementation computes in
//! `f32`/`f64` mixed precision, so results are close but not bit-identical. The
//! surface height this module predicts is therefore an approximation of what
//! the game stores, which is exactly what the BDS heightmap lets us measure.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

use super::java_biomes::{OctaveNoise, Xoroshiro128PlusPlus};
use super::java_blended_noise::BlendedNoise;

/// Seed salts for the named terrain noises: `md5("minecraft:<name>")` split
/// into two big-endian `u64`s, the same scheme the climate noises use.
mod salts {
    /// `md5("minecraft:aquifer_barrier")`.
    pub const AQUIFER_BARRIER: (u64, u64) = (0xe170fda6_c51d093f, 0x134f3e4b_c3f40b0c);
    /// `md5("minecraft:aquifer_fluid_level_floodedness")`.
    pub const AQUIFER_FLUID_LEVEL_FLOODEDNESS: (u64, u64) = (0x1b5d4f85_fdc2d89e, 0x22c7e3e6_11bf67c6);
    /// `md5("minecraft:aquifer_fluid_level_spread")`.
    pub const AQUIFER_FLUID_LEVEL_SPREAD: (u64, u64) = (0x08f136f4_7cd9aaa2, 0x3666182f_5fa0ac6f);
    /// `md5("minecraft:aquifer_lava")`.
    pub const AQUIFER_LAVA: (u64, u64) = (0x0e512127_7ab51b72, 0x2724d8c5_6aa005ba);
    /// `md5("minecraft:badlands_pillar")`.
    pub const BADLANDS_PILLAR: (u64, u64) = (0xbb4375a4_97aa6bbc, 0x2f58a6a3_242fc1dd);
    /// `md5("minecraft:badlands_pillar_roof")`.
    pub const BADLANDS_PILLAR_ROOF: (u64, u64) = (0x6c2ff8dd_7d6714af, 0x3f5f14fd_315e519d);
    /// `md5("minecraft:badlands_surface")`.
    pub const BADLANDS_SURFACE: (u64, u64) = (0xfcf6fc69_70f25434, 0x734c6bec_840906c7);
    /// `md5("minecraft:calcite")`.
    pub const CALCITE: (u64, u64) = (0x44109d85_5bf3fc10, 0x10ceeba6_3afff149);
    /// `md5("minecraft:cave_cheese")`.
    pub const CAVE_CHEESE: (u64, u64) = (0xb159093b_c7baaa50, 0x53abc454_24417c20);
    /// `md5("minecraft:cave_entrance")`.
    pub const CAVE_ENTRANCE: (u64, u64) = (0xf1008a17_493d9a65, 0xbd1ce09e_8bc70ea0);
    /// `md5("minecraft:cave_layer")`.
    pub const CAVE_LAYER: (u64, u64) = (0x4dfe67be_2ef51a83, 0x5a4ae8c4_423e7206);
    /// `md5("minecraft:clay_bands_offset")`.
    pub const CLAY_BANDS_OFFSET: (u64, u64) = (0x64b4c74e_13c7f75e, 0x9adbd232_10c35425);
    /// `md5("minecraft:continentalness")`.
    pub const CONTINENTALNESS: (u64, u64) = (0x83886c9d_0ae3a662, 0xafa638a6_1b42e8ad);
    /// `md5("minecraft:continentalness_large")`.
    pub const CONTINENTALNESS_LARGE: (u64, u64) = (0x9a3f51a1_13fce8dc, 0xee2dbd15_7e5dcdad);
    /// `md5("minecraft:erosion")`.
    pub const EROSION: (u64, u64) = (0xd02491e6_058f6fd8, 0x4792512c_94c17a80);
    /// `md5("minecraft:erosion_large")`.
    pub const EROSION_LARGE: (u64, u64) = (0x8c984b1f_8702a951, 0xead7b1f9_2bae535f);
    /// `md5("minecraft:gravel")`.
    pub const GRAVEL: (u64, u64) = (0x1001fbe1_934018b7, 0x6ba60255_f94d20e3);
    /// `md5("minecraft:gravel_layer")`.
    pub const GRAVEL_LAYER: (u64, u64) = (0x335bb816_137a2e58, 0x50d660a7_8465ba79);
    /// `md5("minecraft:ice")`.
    pub const ICE: (u64, u64) = (0x6ef0b935_71ee34a9, 0xe23c5409_32f9b7b6);
    /// `md5("minecraft:iceberg_pillar")`.
    pub const ICEBERG_PILLAR: (u64, u64) = (0xd27f48cf_79b78ad1, 0x7fe27bff_85dadd65);
    /// `md5("minecraft:iceberg_pillar_roof")`.
    pub const ICEBERG_PILLAR_ROOF: (u64, u64) = (0x5d2cecc3_b9766e21, 0xf3329a2d_badd7627);
    /// `md5("minecraft:iceberg_surface")`.
    pub const ICEBERG_SURFACE: (u64, u64) = (0xbbdbd507_2e18adc7, 0x79083208_4121615d);
    /// `md5("minecraft:jagged")`.
    pub const JAGGED: (u64, u64) = (0xf902c0a7_c9daa994, 0x71ecd96a_8da5e503);
    /// `md5("minecraft:nether_state_selector")`.
    pub const NETHER_STATE_SELECTOR: (u64, u64) = (0x5270bd81_bbbfe325, 0x9c3ea3bf_b55b4520);
    /// `md5("minecraft:nether_wart")`.
    pub const NETHER_WART: (u64, u64) = (0xf5ae6724_51a3ddaf, 0xeee51162_10e766c7);
    /// `md5("minecraft:netherrack")`.
    pub const NETHERRACK: (u64, u64) = (0x8fb5d0ee_61017108, 0x9b2fea87_a320debb);
    /// `md5("minecraft:noodle")`.
    pub const NOODLE: (u64, u64) = (0xd23ce12b_0c37e44b, 0x66b1fdfb_f6a474f3);
    /// `md5("minecraft:noodle_ridge_a")`.
    pub const NOODLE_RIDGE_A: (u64, u64) = (0x86172ac1_315f6026, 0x4a664470_c7d7205f);
    /// `md5("minecraft:noodle_ridge_b")`.
    pub const NOODLE_RIDGE_B: (u64, u64) = (0xeb232b4b_89fdde91, 0x031ee565_ead84e5c);
    /// `md5("minecraft:noodle_thickness")`.
    pub const NOODLE_THICKNESS: (u64, u64) = (0x2fa49bbe_949d4212, 0x5f82c952_51d19891);
    /// `md5("minecraft:offset")`.
    pub const OFFSET: (u64, u64) = (0x080518cf_6af25384, 0x3f3dfb40_a54febd5);
    /// `md5("minecraft:ore_gap")`.
    pub const ORE_GAP: (u64, u64) = (0x9c4cc6b2_fb0be4bb, 0xbd596470_5573bb5e);
    /// `md5("minecraft:ore_vein_a")`.
    pub const ORE_VEIN_A: (u64, u64) = (0x4cd8d69b_9a841649, 0xcdd63f17_bfe8f5ed);
    /// `md5("minecraft:ore_vein_b")`.
    pub const ORE_VEIN_B: (u64, u64) = (0x6b26220b_31f7c6c9, 0xae077ede_bf6aaec1);
    /// `md5("minecraft:ore_veininess")`.
    pub const ORE_VEININESS: (u64, u64) = (0x6b86c782_0a307171, 0xd87fb0fe_fd9c1624);
    /// `md5("minecraft:packed_ice")`.
    pub const PACKED_ICE: (u64, u64) = (0x5cb05807_08af7023, 0x694d1fd0_d72fd7c2);
    /// `md5("minecraft:patch")`.
    pub const PATCH: (u64, u64) = (0xed592d1b_1bc6ed27, 0x21c3acf4_55c0ebbb);
    /// `md5("minecraft:pillar")`.
    pub const PILLAR: (u64, u64) = (0xd4defd14_00fa5347, 0xccb67854_51feaa1c);
    /// `md5("minecraft:pillar_rareness")`.
    pub const PILLAR_RARENESS: (u64, u64) = (0x8ddd6be7_cd4b24d0, 0x0485e866_5333197a);
    /// `md5("minecraft:pillar_thickness")`.
    pub const PILLAR_THICKNESS: (u64, u64) = (0x1a28cb25_42f8308d, 0xc4bba10b_7fc7168c);
    /// `md5("minecraft:powder_snow")`.
    pub const POWDER_SNOW: (u64, u64) = (0x7520c0b8_9fe4562f, 0x62ed8c50_c818845d);
    /// `md5("minecraft:ridge")`.
    pub const RIDGE: (u64, u64) = (0xefc8ef4d_36102b34, 0x1beeeb32_4a0f24ea);
    /// `md5("minecraft:soul_sand_layer")`.
    pub const SOUL_SAND_LAYER: (u64, u64) = (0xb3e0a5ab_d45988c3, 0x2e452e74_72e0c1eb);
    /// `md5("minecraft:spaghetti_2d")`.
    pub const SPAGHETTI_2D: (u64, u64) = (0x34748c98_fa19c477, 0x2a02051e_b9e9b9cd);
    /// `md5("minecraft:spaghetti_2d_elevation")`.
    pub const SPAGHETTI_2D_ELEVATION: (u64, u64) = (0x29dd5fcf_38d9edcd, 0x5bb1d7a8_39f6d4ab);
    /// `md5("minecraft:spaghetti_2d_modulator")`.
    pub const SPAGHETTI_2D_MODULATOR: (u64, u64) = (0x95e07097_e4685522, 0x0232deaf_95eb5fed);
    /// `md5("minecraft:spaghetti_2d_thickness")`.
    pub const SPAGHETTI_2D_THICKNESS: (u64, u64) = (0x37e5cb43_2b85f4c8, 0xe5e16b35_b407aebc);
    /// `md5("minecraft:spaghetti_3d_1")`.
    pub const SPAGHETTI_3D_1: (u64, u64) = (0xa5053f53_ce3f5840, 0x834fa38a_3ded87b7);
    /// `md5("minecraft:spaghetti_3d_2")`.
    pub const SPAGHETTI_3D_2: (u64, u64) = (0xb9a63673_35a0ceef, 0xd61ccfd0_fac10ac3);
    /// `md5("minecraft:spaghetti_3d_rarity")`.
    pub const SPAGHETTI_3D_RARITY: (u64, u64) = (0xee4780c9_8d93a439, 0xbe45d334_fdb76d2c);
    /// `md5("minecraft:spaghetti_3d_thickness")`.
    pub const SPAGHETTI_3D_THICKNESS: (u64, u64) = (0x897e1f20_ad2d2b27, 0xf4142f5a_58d1d5ab);
    /// `md5("minecraft:spaghetti_roughness")`.
    pub const SPAGHETTI_ROUGHNESS: (u64, u64) = (0x7aed57fd_327d6591, 0xd495a359_3e4d67bd);
    /// `md5("minecraft:spaghetti_roughness_modulator")`.
    pub const SPAGHETTI_ROUGHNESS_MODULATOR: (u64, u64) = (0xe19387d2_ceaf4eaa, 0xcacdcd34_e1bc2003);
    /// `md5("minecraft:surface")`.
    pub const SURFACE: (u64, u64) = (0x4b3097bb_e1a7e1ac, 0xf4956157_1ec10de8);
    /// `md5("minecraft:surface_secondary")`.
    pub const SURFACE_SECONDARY: (u64, u64) = (0x1c789a65_1f209f99, 0x2028c41c_495aba50);
    /// `md5("minecraft:surface_swamp")`.
    pub const SURFACE_SWAMP: (u64, u64) = (0xc7afe50d_db0bafd7, 0x13a4f289_e298db6a);
    /// `md5("minecraft:temperature")`.
    pub const TEMPERATURE: (u64, u64) = (0x5c7e6b29_735f0d7f, 0xf7d86f1b_bc734988);
    /// `md5("minecraft:temperature_large")`.
    pub const TEMPERATURE_LARGE: (u64, u64) = (0x944b0073_edf549db, 0x4ff44347_e9d22b96);
    /// `md5("minecraft:vegetation")`.
    pub const VEGETATION: (u64, u64) = (0x81bb4d22_e8dc168e, 0xf1c8b4be_a16303cd);
    /// `md5("minecraft:vegetation_large")`.
    pub const VEGETATION_LARGE: (u64, u64) = (0x71b8ab94_3dbd5301, 0xbb63ddcf_39ff7a2b);
}

/// `NormalNoise`: two octave stacks sampled at slightly different frequencies.
///
/// Unlike the climate `DoublePerlinNoise` (whose amplitudes are normalised by
/// `(10/6)·len/(len+1)`), the terrain noises scale their sum so the result
/// stays in `[-1, 1]`, which is `1 / (2^n - 1)` over the octave count.
#[derive(Debug, Clone)]
pub struct NormalNoise {
    first: OctaveNoise,
    second: OctaveNoise,
    value_factor: f64,
}

impl NormalNoise {
    fn new(seed: u64, salt: (u64, u64), amplitudes: &[f64], first_octave: i32) -> Self {
        let mut positional = Xoroshiro128PlusPlus::from_seed(seed);
        let (low, high) = (positional.next_long(), positional.next_long());
        let mut random = Xoroshiro128PlusPlus {
            lo: low ^ salt.0,
            hi: high ^ salt.1,
        };
        let first = OctaveNoise::decaying(&mut random, amplitudes, first_octave);
        let second = OctaveNoise::decaying(&mut random, amplitudes, first_octave);
        // Vanilla: `valueFactor = (1/6) / expectedDeviation(maxOctave - minOctave)`
        // with `expectedDeviation(span) = 0.1 * (1 + 1/(span + 1))`, so the sum
        // lands around unit standard deviation whatever the octave count.
        let (mut lowest, mut highest) = (i32::MAX, i32::MIN);
        for (index, amplitude) in amplitudes.iter().enumerate() {
            if *amplitude != 0.0 {
                lowest = lowest.min(index as i32);
                highest = highest.max(index as i32);
            }
        }
        let span = (highest - lowest) as f64;
        let expected_deviation = 0.1 * (1.0 + 1.0 / (span + 1.0));
        let value_factor = (1.0 / 6.0) / expected_deviation;
        Self {
            first,
            second,
            value_factor,
        }
    }

    /// Samples the noise at world coordinates already scaled by the caller.
    pub fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        const FREQUENCY: f64 = 337.0 / 331.0;
        let value = self.first.sample_at(x, y, z)
            + self
                .second
                .sample_at(x * FREQUENCY, y * FREQUENCY, z * FREQUENCY);
        value * self.value_factor
    }
}

/// One node of the density function graph.
#[derive(Debug, Clone)]
pub enum Density {
    Constant(f64),
    Noise {
        noise: usize,
        xz_scale: f64,
        y_scale: f64,
    },
    ShiftedNoise {
        noise: usize,
        shift_x: Box<Density>,
        shift_y: Box<Density>,
        shift_z: Box<Density>,
        xz_scale: f64,
        y_scale: f64,
    },
    Add(Box<Density>, Box<Density>),
    Mul(Box<Density>, Box<Density>),
    Sub(Box<Density>, Box<Density>),
    Min(Box<Density>, Box<Density>),
    Max(Box<Density>, Box<Density>),
    Abs(Box<Density>),
    Cube(Box<Density>),
    Square(Box<Density>),
    Squeeze(Box<Density>),
    /// `weird_scaled_sampler`: scales the sampling position by a factor derived
    /// from the input (rarity mapper) and returns the absolute noise value.
    WeirdScaledSampler {
        noise: usize,
        input: Box<Density>,
        min_scale: f64,
        max_scale: f64,
    },
    /// Vanilla evaluates this subtree on the corners of a 4x8x4 cell and
    /// interpolates between them; everything outside it is sampled exactly.
    Interpolated(Box<Density>),
    Clamp {
        input: Box<Density>,
        min: f64,
        max: f64,
    },
    QuarterNegative(Box<Density>),
    HalfNegative(Box<Density>),
    Spline(Spline),
    YClampedGradient {
        from_y: f64,
        to_y: f64,
        from_value: f64,
        to_value: f64,
    },
    RangeChoice {
        input: Box<Density>,
        min: f64,
        max: f64,
        when_in_range: Box<Density>,
        when_out_of_range: Box<Density>,
    },
    Shift {
        noise: usize,
        swapped: bool,
    },
    Y,
    BlendAlpha,
    BlendOffset,
    /// `old_blended_noise`: the classic 1.17 terrain noise that `base_3d_noise`
    /// adds on top of the spline shape. Its parameters are constant, so the
    /// field is identical in every world.
    OldBlendedNoise(BlendedNoise),
}

/// A cubic spline over one coordinate, as used by `offset`, `factor` and
/// `jaggedness`. All vanilla points have zero derivatives, so evaluation is
/// plain linear interpolation.
#[derive(Debug, Clone)]
pub struct Spline {
    coordinate: Box<Density>,
    points: Vec<(f64, SplineValue)>,
}

#[derive(Debug, Clone)]
pub enum SplineValue {
    Constant(f64),
    Spline(Box<Spline>),
}

/// Everything needed to evaluate the terrain graph for one seed.
pub struct TerrainDensity {
    seed: u64,
    root_kind: DensityRoot,
    samplers: Vec<NormalNoise>,
    root: Density,
    /// Named sub-graphs, kept for diagnostics.
    extra: HashMap<&'static str, Density>,
}

type Cache = HashMap<String, Density>;

fn data(name: &str) -> &'static str {
    match name {
        "noise_settings" => include_str!("../../data/noise_settings_overworld.json"),
        "base_3d_noise" => include_str!("../../data/density_function/overworld/base_3d_noise.json"),
        "continents" => include_str!("../../data/density_function/overworld/continents.json"),
        "depth" => include_str!("../../data/density_function/overworld/depth.json"),
        "erosion" => include_str!("../../data/density_function/overworld/erosion.json"),
        "factor" => include_str!("../../data/density_function/overworld/factor.json"),
        "jaggedness" => include_str!("../../data/density_function/overworld/jaggedness.json"),
        "offset" => include_str!("../../data/density_function/overworld/offset.json"),
        "ridges" => include_str!("../../data/density_function/overworld/ridges.json"),
        "ridges_folded" => include_str!("../../data/density_function/overworld/ridges_folded.json"),
        "sloped_cheese" => include_str!("../../data/density_function/overworld/sloped_cheese.json"),
        "shift_x" => include_str!("../../data/density_function/shift_x.json"),
        "shift_z" => include_str!("../../data/density_function/shift_z.json"),
        "noise/cave_entrance" => include_str!("../../data/noise/cave_entrance.json"),
        "noise/noodle" => include_str!("../../data/noise/noodle.json"),
        "noise/noodle_thickness" => include_str!("../../data/noise/noodle_thickness.json"),
        "noise/pillar" => include_str!("../../data/noise/pillar.json"),
        "noise/spaghetti_2d" => include_str!("../../data/noise/spaghetti_2d.json"),
        "noise/spaghetti_roughness" => include_str!("../../data/noise/spaghetti_roughness.json"),
        "noise/temperature" => include_str!("../../data/noise/temperature.json"),
        "noise/vegetation" => include_str!("../../data/noise/vegetation.json"),
        "noise/aquifer_barrier" => include_str!("../../data/noise/aquifer_barrier.json"),
        "noise/aquifer_fluid_level_floodedness" => include_str!("../../data/noise/aquifer_fluid_level_floodedness.json"),
        "noise/ore_gap" => include_str!("../../data/noise/ore_gap.json"),
        "noise/ore_vein_a" => include_str!("../../data/noise/ore_vein_a.json"),
        "noise/ore_vein_b" => include_str!("../../data/noise/ore_vein_b.json"),
        "noise/spaghetti_2d_modulator" => include_str!("../../data/noise/spaghetti_2d_modulator.json"),
        "noise/spaghetti_2d_thickness" => include_str!("../../data/noise/spaghetti_2d_thickness.json"),
        "noise/spaghetti_roughness_modulator" => include_str!("../../data/noise/spaghetti_roughness_modulator.json"),
        "noise/surface_swamp" => include_str!("../../data/noise/surface_swamp.json"),
        "noise/aquifer_fluid_level_spread" => include_str!("../../data/noise/aquifer_fluid_level_spread.json"),
        "noise/aquifer_lava" => include_str!("../../data/noise/aquifer_lava.json"),
        "noise/badlands_pillar" => include_str!("../../data/noise/badlands_pillar.json"),
        "noise/badlands_pillar_roof" => include_str!("../../data/noise/badlands_pillar_roof.json"),
        "noise/badlands_surface" => include_str!("../../data/noise/badlands_surface.json"),
        "noise/calcite" => include_str!("../../data/noise/calcite.json"),
        "noise/clay_bands_offset" => include_str!("../../data/noise/clay_bands_offset.json"),
        "noise/continentalness_large" => include_str!("../../data/noise/continentalness_large.json"),
        "noise/erosion_large" => include_str!("../../data/noise/erosion_large.json"),
        "noise/gravel" => include_str!("../../data/noise/gravel.json"),
        "noise/gravel_layer" => include_str!("../../data/noise/gravel_layer.json"),
        "noise/ice" => include_str!("../../data/noise/ice.json"),
        "noise/iceberg_pillar" => include_str!("../../data/noise/iceberg_pillar.json"),
        "noise/iceberg_pillar_roof" => include_str!("../../data/noise/iceberg_pillar_roof.json"),
        "noise/iceberg_surface" => include_str!("../../data/noise/iceberg_surface.json"),
        "noise/nether_state_selector" => include_str!("../../data/noise/nether_state_selector.json"),
        "noise/nether_wart" => include_str!("../../data/noise/nether_wart.json"),
        "noise/netherrack" => include_str!("../../data/noise/netherrack.json"),
        "noise/noodle_ridge_a" => include_str!("../../data/noise/noodle_ridge_a.json"),
        "noise/noodle_ridge_b" => include_str!("../../data/noise/noodle_ridge_b.json"),
        "noise/ore_veininess" => include_str!("../../data/noise/ore_veininess.json"),
        "noise/packed_ice" => include_str!("../../data/noise/packed_ice.json"),
        "noise/patch" => include_str!("../../data/noise/patch.json"),
        "noise/pillar_rareness" => include_str!("../../data/noise/pillar_rareness.json"),
        "noise/pillar_thickness" => include_str!("../../data/noise/pillar_thickness.json"),
        "noise/powder_snow" => include_str!("../../data/noise/powder_snow.json"),
        "noise/soul_sand_layer" => include_str!("../../data/noise/soul_sand_layer.json"),
        "noise/spaghetti_2d_elevation" => include_str!("../../data/noise/spaghetti_2d_elevation.json"),
        "noise/spaghetti_3d_1" => include_str!("../../data/noise/spaghetti_3d_1.json"),
        "noise/spaghetti_3d_2" => include_str!("../../data/noise/spaghetti_3d_2.json"),
        "noise/spaghetti_3d_rarity" => include_str!("../../data/noise/spaghetti_3d_rarity.json"),
        "noise/spaghetti_3d_thickness" => include_str!("../../data/noise/spaghetti_3d_thickness.json"),
        "noise/temperature_large" => include_str!("../../data/noise/temperature_large.json"),
        "noise/vegetation_large" => include_str!("../../data/noise/vegetation_large.json"),
        "noise/offset" => include_str!("../../data/noise/offset.json"),
        "noise/cave_layer" => include_str!("../../data/noise/cave_layer.json"),
        "caves/entrances" => include_str!("../../data/density_function/overworld/caves/entrances.json"),
        "caves/noodle" => include_str!("../../data/density_function/overworld/caves/noodle.json"),
        "caves/pillars" => include_str!("../../data/density_function/overworld/caves/pillars.json"),
        "caves/spaghetti_2d" => {
            include_str!("../../data/density_function/overworld/caves/spaghetti_2d.json")
        }
        "caves/spaghetti_2d_thickness_modulator" => {
            include_str!(
                "../../data/density_function/overworld/caves/spaghetti_2d_thickness_modulator.json"
            )
        }
        "caves/spaghetti_roughness_function" => {
            include_str!(
                "../../data/density_function/overworld/caves/spaghetti_roughness_function.json"
            )
        }
        "final_density" => {
            include_str!("../../data/density_function/overworld/final_density.json")
        }
        "noise/continentalness" => include_str!("../../data/noise/continentalness.json"),
        "noise/erosion" => include_str!("../../data/noise/erosion.json"),
        "noise/ridge" => include_str!("../../data/noise/ridge.json"),
        "noise/jagged" => include_str!("../../data/noise/jagged.json"),
        "noise/surface" => include_str!("../../data/noise/surface.json"),
        "noise/surface_secondary" => include_str!("../../data/noise/surface_secondary.json"),
        "noise/cave_cheese" => include_str!("../../data/noise/cave_cheese.json"),
        other => panic!("unknown worldgen data file: {other}"),
    }
}

fn salt_for(name: &str) -> (u64, u64) {
    match name {
        "minecraft:aquifer_barrier" => salts::AQUIFER_BARRIER,
        "minecraft:aquifer_fluid_level_floodedness" => salts::AQUIFER_FLUID_LEVEL_FLOODEDNESS,
        "minecraft:aquifer_fluid_level_spread" => salts::AQUIFER_FLUID_LEVEL_SPREAD,
        "minecraft:aquifer_lava" => salts::AQUIFER_LAVA,
        "minecraft:badlands_pillar" => salts::BADLANDS_PILLAR,
        "minecraft:badlands_pillar_roof" => salts::BADLANDS_PILLAR_ROOF,
        "minecraft:badlands_surface" => salts::BADLANDS_SURFACE,
        "minecraft:calcite" => salts::CALCITE,
        "minecraft:cave_cheese" => salts::CAVE_CHEESE,
        "minecraft:cave_entrance" => salts::CAVE_ENTRANCE,
        "minecraft:cave_layer" => salts::CAVE_LAYER,
        "minecraft:clay_bands_offset" => salts::CLAY_BANDS_OFFSET,
        "minecraft:continentalness" => salts::CONTINENTALNESS,
        "minecraft:continentalness_large" => salts::CONTINENTALNESS_LARGE,
        "minecraft:erosion" => salts::EROSION,
        "minecraft:erosion_large" => salts::EROSION_LARGE,
        "minecraft:gravel" => salts::GRAVEL,
        "minecraft:gravel_layer" => salts::GRAVEL_LAYER,
        "minecraft:ice" => salts::ICE,
        "minecraft:iceberg_pillar" => salts::ICEBERG_PILLAR,
        "minecraft:iceberg_pillar_roof" => salts::ICEBERG_PILLAR_ROOF,
        "minecraft:iceberg_surface" => salts::ICEBERG_SURFACE,
        "minecraft:jagged" => salts::JAGGED,
        "minecraft:nether_state_selector" => salts::NETHER_STATE_SELECTOR,
        "minecraft:nether_wart" => salts::NETHER_WART,
        "minecraft:netherrack" => salts::NETHERRACK,
        "minecraft:noodle" => salts::NOODLE,
        "minecraft:noodle_ridge_a" => salts::NOODLE_RIDGE_A,
        "minecraft:noodle_ridge_b" => salts::NOODLE_RIDGE_B,
        "minecraft:noodle_thickness" => salts::NOODLE_THICKNESS,
        "minecraft:offset" => salts::OFFSET,
        "minecraft:ore_gap" => salts::ORE_GAP,
        "minecraft:ore_vein_a" => salts::ORE_VEIN_A,
        "minecraft:ore_vein_b" => salts::ORE_VEIN_B,
        "minecraft:ore_veininess" => salts::ORE_VEININESS,
        "minecraft:packed_ice" => salts::PACKED_ICE,
        "minecraft:patch" => salts::PATCH,
        "minecraft:pillar" => salts::PILLAR,
        "minecraft:pillar_rareness" => salts::PILLAR_RARENESS,
        "minecraft:pillar_thickness" => salts::PILLAR_THICKNESS,
        "minecraft:powder_snow" => salts::POWDER_SNOW,
        "minecraft:ridge" => salts::RIDGE,
        "minecraft:soul_sand_layer" => salts::SOUL_SAND_LAYER,
        "minecraft:spaghetti_2d" => salts::SPAGHETTI_2D,
        "minecraft:spaghetti_2d_elevation" => salts::SPAGHETTI_2D_ELEVATION,
        "minecraft:spaghetti_2d_modulator" => salts::SPAGHETTI_2D_MODULATOR,
        "minecraft:spaghetti_2d_thickness" => salts::SPAGHETTI_2D_THICKNESS,
        "minecraft:spaghetti_3d_1" => salts::SPAGHETTI_3D_1,
        "minecraft:spaghetti_3d_2" => salts::SPAGHETTI_3D_2,
        "minecraft:spaghetti_3d_rarity" => salts::SPAGHETTI_3D_RARITY,
        "minecraft:spaghetti_3d_thickness" => salts::SPAGHETTI_3D_THICKNESS,
        "minecraft:spaghetti_roughness" => salts::SPAGHETTI_ROUGHNESS,
        "minecraft:spaghetti_roughness_modulator" => salts::SPAGHETTI_ROUGHNESS_MODULATOR,
        "minecraft:surface" => salts::SURFACE,
        "minecraft:surface_secondary" => salts::SURFACE_SECONDARY,
        "minecraft:surface_swamp" => salts::SURFACE_SWAMP,
        "minecraft:temperature" => salts::TEMPERATURE,
        "minecraft:temperature_large" => salts::TEMPERATURE_LARGE,
        "minecraft:vegetation" => salts::VEGETATION,
        "minecraft:vegetation_large" => salts::VEGETATION_LARGE,
        other => panic!("no salt for noise {other}"),
    }
}

fn noise_parameters(name: &str) -> &'static str {
    match name {
        "minecraft:aquifer_barrier" => data("noise/aquifer_barrier"),
        "minecraft:aquifer_fluid_level_floodedness" => data("noise/aquifer_fluid_level_floodedness"),
        "minecraft:aquifer_fluid_level_spread" => data("noise/aquifer_fluid_level_spread"),
        "minecraft:aquifer_lava" => data("noise/aquifer_lava"),
        "minecraft:badlands_pillar" => data("noise/badlands_pillar"),
        "minecraft:badlands_pillar_roof" => data("noise/badlands_pillar_roof"),
        "minecraft:badlands_surface" => data("noise/badlands_surface"),
        "minecraft:calcite" => data("noise/calcite"),
        "minecraft:cave_cheese" => data("noise/cave_cheese"),
        "minecraft:cave_entrance" => data("noise/cave_entrance"),
        "minecraft:cave_layer" => data("noise/cave_layer"),
        "minecraft:clay_bands_offset" => data("noise/clay_bands_offset"),
        "minecraft:continentalness" => data("noise/continentalness"),
        "minecraft:continentalness_large" => data("noise/continentalness_large"),
        "minecraft:erosion" => data("noise/erosion"),
        "minecraft:erosion_large" => data("noise/erosion_large"),
        "minecraft:gravel" => data("noise/gravel"),
        "minecraft:gravel_layer" => data("noise/gravel_layer"),
        "minecraft:ice" => data("noise/ice"),
        "minecraft:iceberg_pillar" => data("noise/iceberg_pillar"),
        "minecraft:iceberg_pillar_roof" => data("noise/iceberg_pillar_roof"),
        "minecraft:iceberg_surface" => data("noise/iceberg_surface"),
        "minecraft:jagged" => data("noise/jagged"),
        "minecraft:nether_state_selector" => data("noise/nether_state_selector"),
        "minecraft:nether_wart" => data("noise/nether_wart"),
        "minecraft:netherrack" => data("noise/netherrack"),
        "minecraft:noodle" => data("noise/noodle"),
        "minecraft:noodle_ridge_a" => data("noise/noodle_ridge_a"),
        "minecraft:noodle_ridge_b" => data("noise/noodle_ridge_b"),
        "minecraft:noodle_thickness" => data("noise/noodle_thickness"),
        "minecraft:offset" => data("noise/offset"),
        "minecraft:ore_gap" => data("noise/ore_gap"),
        "minecraft:ore_vein_a" => data("noise/ore_vein_a"),
        "minecraft:ore_vein_b" => data("noise/ore_vein_b"),
        "minecraft:ore_veininess" => data("noise/ore_veininess"),
        "minecraft:packed_ice" => data("noise/packed_ice"),
        "minecraft:patch" => data("noise/patch"),
        "minecraft:pillar" => data("noise/pillar"),
        "minecraft:pillar_rareness" => data("noise/pillar_rareness"),
        "minecraft:pillar_thickness" => data("noise/pillar_thickness"),
        "minecraft:powder_snow" => data("noise/powder_snow"),
        "minecraft:ridge" => data("noise/ridge"),
        "minecraft:soul_sand_layer" => data("noise/soul_sand_layer"),
        "minecraft:spaghetti_2d" => data("noise/spaghetti_2d"),
        "minecraft:spaghetti_2d_elevation" => data("noise/spaghetti_2d_elevation"),
        "minecraft:spaghetti_2d_modulator" => data("noise/spaghetti_2d_modulator"),
        "minecraft:spaghetti_2d_thickness" => data("noise/spaghetti_2d_thickness"),
        "minecraft:spaghetti_3d_1" => data("noise/spaghetti_3d_1"),
        "minecraft:spaghetti_3d_2" => data("noise/spaghetti_3d_2"),
        "minecraft:spaghetti_3d_rarity" => data("noise/spaghetti_3d_rarity"),
        "minecraft:spaghetti_3d_thickness" => data("noise/spaghetti_3d_thickness"),
        "minecraft:spaghetti_roughness" => data("noise/spaghetti_roughness"),
        "minecraft:spaghetti_roughness_modulator" => data("noise/spaghetti_roughness_modulator"),
        "minecraft:surface" => data("noise/surface"),
        "minecraft:surface_secondary" => data("noise/surface_secondary"),
        "minecraft:surface_swamp" => data("noise/surface_swamp"),
        "minecraft:temperature" => data("noise/temperature"),
        "minecraft:temperature_large" => data("noise/temperature_large"),
        "minecraft:vegetation" => data("noise/vegetation"),
        "minecraft:vegetation_large" => data("noise/vegetation_large"),
        other => panic!("no noise parameters for {other}"),
    }
}

/// Which overworld density graph the terrain field is evaluated from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DensityRoot {
    /// `minecraft:overworld/sloped_cheese`: the spline shape alone, without the
    /// slide term or cave carving. Default; 7.53 blocks MAE on land.
    SlopedCheese,
    /// `minecraft:overworld/final_density`: the complete graph, with slide,
    /// squeeze and the cave noises. Experimental: it evaluates, but its MAE on
    /// land is 16.60 blocks (14.43 with the caves disabled), so it is not the
    /// default until the `slide`/`range_choice`/`weird_scaled_sampler`
    /// semantics are pinned down.
    FinalDensity,
}

impl DensityRoot {
    fn data_path(self) -> &'static str {
        match self {
            DensityRoot::SlopedCheese => "sloped_cheese",
            DensityRoot::FinalDensity => "final_density",
        }
    }
}

impl TerrainDensity {
    /// Builds the terrain graph for `seed` with the default [`DensityRoot`].
    pub fn new(seed: u64) -> Self {
        Self::with_root(seed, DensityRoot::SlopedCheese)
    }

    /// Builds the terrain graph for `seed` from an explicitly chosen root.
    pub fn with_root(seed: u64, root: DensityRoot) -> Self {
        let mut builder = Builder {
            seed,
            noises: HashMap::new(),
            samplers: Vec::new(),
            cache: Cache::new(),
        };
        let root_value: Value =
            serde_json::from_str(data(root.data_path())).expect("valid json");
        let root_node = builder.build(&root_value);
        let mut extra = HashMap::new();
        for name in [
            "continents",
            "erosion",
            "ridges",
            "ridges_folded",
            "offset",
            "factor",
            "jaggedness",
            "depth",
        ] {
            let node = builder.reference(&format!("minecraft:overworld/{name}"));
            extra.insert(name, node);
        }
        Self {
            seed,
            root_kind: root,
            samplers: builder.samplers,
            root: root_node,
            extra,
        }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Terrain density at a block position: positive is solid rock.
    /// Trilinear interpolation of one subtree over the 4x8x4 cell that contains
    /// the position, which is what vanilla's `interpolated` node does.
    fn interpolate(&self, node: &Density, x: f64, y: f64, z: f64) -> f64 {
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let zi = z.floor() as i32;
        let x0 = xi.div_euclid(4) * 4;
        let z0 = zi.div_euclid(4) * 4;
        let y0 = WORLD_BOTTOM + (yi - WORLD_BOTTOM).div_euclid(8) * 8;
        let (x1, y1, z1) = (x0 + 4, y0 + 8, z0 + 4);
        let corner = |cx: i32, cy: i32, cz: i32| {
            self.eval(node, cx as f64, cy as f64, cz as f64)
        };
        let c000 = corner(x0, y0, z0);
        let c100 = corner(x1, y0, z0);
        let c010 = corner(x0, y1, z0);
        let c110 = corner(x1, y1, z0);
        let c001 = corner(x0, y0, z1);
        let c101 = corner(x1, y0, z1);
        let c011 = corner(x0, y1, z1);
        let c111 = corner(x1, y1, z1);
        let tx = (x - x0 as f64) / 4.0;
        let ty = (y - y0 as f64) / 8.0;
        let tz = (z - z0 as f64) / 4.0;
        let lerp = |t: f64, a: f64, b: f64| a + t * (b - a);
        lerp(
            tz,
            lerp(ty, lerp(tx, c000, c100), lerp(tx, c010, c110)),
            lerp(ty, lerp(tx, c001, c101), lerp(tx, c011, c111)),
        )
    }

    /// Density at a block position.
    ///
    /// Vanilla evaluates the density graph per block and only samples coarsely
    /// inside an `interpolated` node. [`DensityRoot::FinalDensity`] contains
    /// that node, so it is evaluated exactly; [`DensityRoot::SlopedCheese`]
    /// does not, and there the 4x8x4 cell interpolation is supplied here.
    pub fn density_at_block(&self, x: i32, y: i32, z: i32) -> f64 {
        if self.root_kind == DensityRoot::FinalDensity {
            return self.density(x as f64, y as f64, z as f64);
        }
        let x0 = x.div_euclid(4) * 4;
        let z0 = z.div_euclid(4) * 4;
        let y0 = WORLD_BOTTOM + (y - WORLD_BOTTOM).div_euclid(8) * 8;
        let (x1, y1, z1) = (x0 + 4, y0 + 8, z0 + 4);
        let corner = |cx: i32, cy: i32, cz: i32| self.density(cx as f64, cy as f64, cz as f64);
        let (c000, c100) = (corner(x0, y0, z0), corner(x1, y0, z0));
        let (c010, c110) = (corner(x0, y1, z0), corner(x1, y1, z0));
        let (c001, c101) = (corner(x0, y0, z1), corner(x1, y0, z1));
        let (c011, c111) = (corner(x0, y1, z1), corner(x1, y1, z1));
        let lerp = |t: f64, a: f64, b: f64| a + t * (b - a);
        let tx = (x - x0) as f64 / 4.0;
        let ty = (y - y0) as f64 / 8.0;
        let tz = (z - z0) as f64 / 4.0;
        lerp(
            tz,
            lerp(ty, lerp(tx, c000, c100), lerp(tx, c010, c110)),
            lerp(ty, lerp(tx, c001, c101), lerp(tx, c011, c111)),
        )
    }

    pub fn density(&self, x: f64, y: f64, z: f64) -> f64 {
        self.eval(&self.root, x, y, z)
    }

    fn eval(&self, node: &Density, x: f64, y: f64, z: f64) -> f64 {
        match node {
            Density::Constant(value) => *value,
            Density::Noise {
                noise,
                xz_scale,
                y_scale,
            } => self.samplers[*noise].sample(x * xz_scale, y * y_scale, z * xz_scale),
            Density::ShiftedNoise {
                noise,
                shift_x,
                shift_y,
                shift_z,
                xz_scale,
                y_scale,
            } => {
                let sx = self.eval(shift_x, x, y, z) * 4.0;
                let sy = self.eval(shift_y, x, y, z) * 4.0;
                let sz = self.eval(shift_z, x, y, z) * 4.0;
                self.samplers[*noise].sample(
                    (x + sx) * xz_scale,
                    (y + sy) * y_scale,
                    (z + sz) * xz_scale,
                )
            }
            Density::Add(a, b) => self.eval(a, x, y, z) + self.eval(b, x, y, z),
            Density::Mul(a, b) => self.eval(a, x, y, z) * self.eval(b, x, y, z),
            Density::Sub(a, b) => self.eval(a, x, y, z) - self.eval(b, x, y, z),
            Density::Min(a, b) => self.eval(a, x, y, z).min(self.eval(b, x, y, z)),
            Density::Max(a, b) => self.eval(a, x, y, z).max(self.eval(b, x, y, z)),
            Density::Abs(a) => self.eval(a, x, y, z).abs(),
            Density::Cube(a) => self.eval(a, x, y, z).powi(3),
            Density::Square(a) => {
                let value = self.eval(a, x, y, z);
                value * value
            }
            Density::Squeeze(a) => {
                let value = self.eval(a, x, y, z).clamp(-1.0, 1.0);
                value / 2.0 - value * value * value / 24.0
            }
            Density::Interpolated(a) => self.interpolate(a, x, y, z),
            Density::WeirdScaledSampler {
                noise,
                input,
                min_scale,
                max_scale,
            } => {
                let rarity = self.eval(input, x, y, z).clamp(-1.0, 1.0);
                let scale = *min_scale + (rarity + 1.0) / 2.0 * (*max_scale - *min_scale);
                self.samplers[*noise]
                    .sample(x * scale, y * scale, z * scale)
                    .abs()
            }
            Density::Clamp { input, min, max } => self.eval(input, x, y, z).clamp(*min, *max),
            Density::QuarterNegative(a) => {
                let value = self.eval(a, x, y, z);
                if value < 0.0 {
                    value * 0.25
                } else {
                    value
                }
            }
            Density::HalfNegative(a) => {
                let value = self.eval(a, x, y, z);
                if value < 0.0 {
                    value * 0.5
                } else {
                    value
                }
            }
            Density::Spline(spline) => {
                let coordinate = self.eval(&spline.coordinate, x, y, z);
                self.apply_spline(spline, coordinate, x, y, z)
            }
            Density::YClampedGradient {
                from_y,
                to_y,
                from_value,
                to_value,
            } => {
                if y <= *from_y {
                    *from_value
                } else if y >= *to_y {
                    *to_value
                } else {
                    let t = (y - from_y) / (to_y - from_y);
                    from_value + t * (to_value - from_value)
                }
            }
            Density::RangeChoice {
                input,
                min,
                max,
                when_in_range,
                when_out_of_range,
            } => {
                let value = self.eval(input, x, y, z);
                if value >= *min && value < *max {
                    self.eval(when_in_range, x, y, z)
                } else {
                    self.eval(when_out_of_range, x, y, z)
                }
            }
            Density::Shift { noise, swapped } => {
                let value = if *swapped {
                    self.samplers[*noise].sample(z * 0.25, x * 0.25, 0.0)
                } else {
                    self.samplers[*noise].sample(x * 0.25, 0.0, z * 0.25)
                };
                value * 4.0
            }
            Density::Y => y,
            Density::BlendAlpha => 1.0,
            Density::BlendOffset => 0.0,
            // TODO: the classic 1.17 `BlendedNoise`. Until it is ported the
            // terrain loses its 3D detail and only keeps the spline shape.
            Density::OldBlendedNoise(noise) => noise.sample(x, y, z),
        }
    }

    fn apply_spline(&self, spline: &Spline, value: f64, x: f64, y: f64, z: f64) -> f64 {
        let (Some(first), Some(last)) = (spline.points.first(), spline.points.last()) else {
            return 0.0;
        };
        if value <= first.0 {
            return self.apply_spline_value(&first.1, x, y, z);
        }
        if value >= last.0 {
            return self.apply_spline_value(&last.1, x, y, z);
        }
        for window in spline.points.windows(2) {
            let (from, from_value) = &window[0];
            let (to, to_value) = &window[1];
            if value <= *to {
                let span = to - from;
                let t = if span == 0.0 {
                    0.0
                } else {
                    (value - from) / span
                };
                let low = self.apply_spline_value(from_value, x, y, z);
                let high = self.apply_spline_value(to_value, x, y, z);
                return low + t * (high - low);
            }
        }
        self.apply_spline_value(&last.1, x, y, z)
    }

    fn apply_spline_value(&self, value: &SplineValue, x: f64, y: f64, z: f64) -> f64 {
        match value {
            SplineValue::Constant(value) => *value,
            SplineValue::Spline(spline) => {
                let coordinate = self.eval(&spline.coordinate, x, y, z);
                self.apply_spline(spline, coordinate, x, y, z)
            }
        }
    }

    /// Value of one named sub-graph, for diagnostics.
    pub fn named(&self, name: &str, x: f64, y: f64, z: f64) -> f64 {
        match self.extra.get(name) {
            Some(node) => self.eval(node, x, y, z),
            None => panic!("unknown density function {name}"),
        }
    }

    /// Highest solid block in a column, or `None` when the column is all air.
    /// Density is (in practice) monotonically decreasing with height, so the
    /// solid/air boundary can be found with a binary search instead of walking
    /// all 384 blocks of the column.
    /// Highest block of the column whose density is positive, which is what the
    /// world-surface heightmap records.
    ///
    /// Scans down from the world top instead of bisecting: with cave carving the
    /// density is not monotone in y, so a bisection can settle on a cave floor
    /// far below the real surface.
    pub fn surface_height(&self, x: i32, z: i32) -> Option<i32> {
        // Coarse pass every 8 blocks, then a one-block walk around the first hit
        // so a thin solid layer just above it is still found.
        let mut coarse = None;
        let mut y = WORLD_TOP - 1;
        while y >= WORLD_BOTTOM {
            if self.density_at_block(x, y, z) > 0.0 {
                coarse = Some(y);
                break;
            }
            y -= 8;
        }
        let coarse = coarse?;
        let top = (coarse + 7).min(WORLD_TOP - 1);
        let bottom = (coarse - 7).max(WORLD_BOTTOM);
        (bottom..=top)
            .rev()
            .find(|&y| self.density_at_block(x, y, z) > 0.0)
    }
}

/// World vertical bounds of the Overworld in 1.18+.
pub const WORLD_BOTTOM: i32 = -64;
pub const WORLD_TOP: i32 = 320;

struct Builder {
    seed: u64,
    noises: HashMap<String, (usize, Vec<f64>, i32)>,
    samplers: Vec<NormalNoise>,
    cache: Cache,
}

impl Builder {
    fn noise(&mut self, name: &str) -> usize {
        if let Some((index, _, _)) = self.noises.get(name) {
            return *index;
        }
        let parameters: Value =
            serde_json::from_str(noise_parameters(name)).expect("valid noise json");
        let amplitudes: Vec<f64> = parameters["amplitudes"]
            .as_array()
            .expect("amplitudes")
            .iter()
            .map(|v| v.as_f64().expect("amplitude"))
            .collect();
        let first_octave = parameters["firstOctave"].as_i64().expect("firstOctave") as i32;
        let index = self.samplers.len();
        let sampler = NormalNoise::new(self.seed, salt_for(name), &amplitudes, first_octave);
        self.samplers.push(sampler);
        self.noises
            .insert(name.to_string(), (index, amplitudes, first_octave));
        index
    }

    fn build(&mut self, value: &Value) -> Density {
        if let Some(number) = value.as_f64() {
            return Density::Constant(number);
        }
        if let Some(reference) = value.as_str() {
            return self.reference(reference);
        }
        let kind = value["type"].as_str().expect("density function type");
        let child = |builder: &mut Builder, key: &str| Box::new(builder.build(&value[key]));
        match kind {
            "minecraft:constant" => Density::Constant(value["argument"].as_f64().unwrap_or(0.0)),
            "minecraft:y" => Density::Y,
            "minecraft:blend_alpha" => Density::BlendAlpha,
            "minecraft:blend_offset" => Density::BlendOffset,
            "minecraft:noise" => Density::Noise {
                noise: self.noise(value["noise"].as_str().expect("noise")),
                xz_scale: value["xz_scale"].as_f64().unwrap_or(1.0),
                y_scale: value["y_scale"].as_f64().unwrap_or(1.0),
            },
            "minecraft:shifted_noise" => {
                let noise = self.noise(value["noise"].as_str().expect("noise"));
                let shift_x = child(self, "shift_x");
                let shift_y = child(self, "shift_y");
                let shift_z = child(self, "shift_z");
                Density::ShiftedNoise {
                    noise,
                    shift_x,
                    shift_y,
                    shift_z,
                    xz_scale: value["xz_scale"].as_f64().unwrap_or(1.0),
                    y_scale: value["y_scale"].as_f64().unwrap_or(1.0),
                }
            }
            "minecraft:add" => {
                let a = child(self, "argument1");
                let b = child(self, "argument2");
                Density::Add(a, b)
            }
            "minecraft:mul" => {
                let a = child(self, "argument1");
                let b = child(self, "argument2");
                Density::Mul(a, b)
            }
            "minecraft:sub" => {
                let a = child(self, "argument1");
                let b = child(self, "argument2");
                Density::Sub(a, b)
            }
            "minecraft:min" => {
                let a = child(self, "argument1");
                let b = child(self, "argument2");
                Density::Min(a, b)
            }
            "minecraft:max" => {
                let a = child(self, "argument1");
                let b = child(self, "argument2");
                Density::Max(a, b)
            }
            "minecraft:abs" => Density::Abs(child(self, "argument")),
            "minecraft:cube" => Density::Cube(child(self, "argument")),
            "minecraft:square" => Density::Square(child(self, "argument")),
            "minecraft:squeeze" => Density::Squeeze(child(self, "argument")),
            "minecraft:weird_scaled_sampler" => {
                let (min_scale, max_scale) = match value["rarity_value_mapper"].as_str() {
                    Some("type_2") => (0.5, 3.0),
                    _ => (0.75, 2.0),
                };
                let name = value["noise"].as_str().expect("noise").to_string();
                Density::WeirdScaledSampler {
                    noise: self.noise(&name),
                    input: child(self, "input"),
                    min_scale,
                    max_scale,
                }
            }
            "minecraft:interpolated" => Density::Interpolated(child(self, "argument")),
            // The world has no neighbours to blend with, so the blend factor is
            // 1 and the offset 0: the node is a plain pass-through.
            "minecraft:blend_density" => *child(self, "argument"),
            "minecraft:quarter_negative" => Density::QuarterNegative(child(self, "argument")),
            "minecraft:half_negative" => Density::HalfNegative(child(self, "argument")),
            "minecraft:clamp" => {
                let input = child(self, "input");
                Density::Clamp {
                    input,
                    min: value["min"].as_f64().unwrap_or(f64::MIN),
                    max: value["max"].as_f64().unwrap_or(f64::MAX),
                }
            }
            "minecraft:spline" => Density::Spline(self.spline(&value["spline"], value)),
            "minecraft:y_clamped_gradient" => Density::YClampedGradient {
                from_y: value["from_y"].as_f64().expect("from_y"),
                to_y: value["to_y"].as_f64().expect("to_y"),
                from_value: value["from_value"].as_f64().expect("from_value"),
                to_value: value["to_value"].as_f64().expect("to_value"),
            },
            "minecraft:range_choice" => {
                let input = child(self, "input");
                let when_in_range = child(self, "when_in_range");
                let when_out_of_range = child(self, "when_out_of_range");
                Density::RangeChoice {
                    input,
                    min: value["min_inclusive"].as_f64().expect("min_inclusive"),
                    max: value["max_exclusive"].as_f64().expect("max_exclusive"),
                    when_in_range,
                    when_out_of_range,
                }
            }
            "minecraft:old_blended_noise" => {
                Density::OldBlendedNoise(BlendedNoise::base_3d_noise())
            }
            "minecraft:shift_a" | "minecraft:shift_b" => {
                // Both wrap the `offset` noise at a quarter scale; `shift_b`
                // deliberately swaps the horizontal axes.
                let swapped = kind == "minecraft:shift_b";
                Density::Shift {
                    noise: self.noise("minecraft:offset"),
                    swapped,
                }
            }
            // Caches and interpolation do not change the value at a point.
            "minecraft:flat_cache"
            | "minecraft:cache_2d"
            | "minecraft:cache_once"
            | "minecraft:cache_all_in_cell" => self.build(&value["argument"]),
            other => panic!("unsupported density function: {other}"),
        }
    }

    fn spline(&mut self, value: &Value, parent: &Value) -> Spline {
        let coordinate = match value.get("coordinate") {
            Some(coordinate) => Box::new(self.build(coordinate)),
            None => Box::new(self.build(&parent["coordinate"])),
        };
        let mut points = Vec::new();
        for point in value["points"].as_array().expect("points") {
            let location = point["location"].as_f64().expect("location");
            let value_node = &point["value"];
            let spline_value = if value_node.get("coordinate").is_some() {
                SplineValue::Spline(Box::new(self.spline(value_node, value)))
            } else {
                SplineValue::Constant(value_node.as_f64().expect("spline value"))
            };
            points.push((location, spline_value));
        }
        Spline {
            coordinate,
            points,
        }
    }

    fn reference(&mut self, name: &str) -> Density {
        match name {
            "minecraft:y" => return Density::Y,
            "minecraft:zero" => return Density::Constant(0.0),
            _ => {}
        }
        if let Some(node) = self.cache.get(name) {
            return node.clone();
        }
        let file = match name.strip_prefix("minecraft:") {
            Some("shift_x") => "shift_x",
            Some("shift_z") => "shift_z",
            other => other
                .and_then(|rest| rest.strip_prefix("overworld/"))
                .unwrap_or_else(|| panic!("unsupported density function reference: {name}")),
        };
        let value: Value =
            serde_json::from_str(data(file)).unwrap_or_else(|_| panic!("missing data for {name}"));
        let node = self.build(&value);
        self.cache.insert(name.to_string(), node.clone());
        node
    }
}

/// Cached terrain graph for one seed, built on first use.
pub fn terrain(seed: u64) -> &'static TerrainDensity {
    static CACHE: OnceLock<std::sync::Mutex<HashMap<u64, &'static TerrainDensity>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut guard = cache.lock().expect("terrain cache");
    if let Some(terrain) = guard.get(&seed) {
        return terrain;
    }
    let terrain: &'static TerrainDensity = Box::leak(Box::new(TerrainDensity::new(seed)));
    guard.insert(seed, terrain);
    terrain
}
