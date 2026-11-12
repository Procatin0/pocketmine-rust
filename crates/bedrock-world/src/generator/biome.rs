//! Port of PocketMine-MP's Biome and BiomeSelector system, extended with
//! the badlands family from the legacy biome-id table that
//! `niduy/minecraft-world-generator` (`src/lib/minecraft/const.ts`,
//! `BIOME`: `Ocean: 0, Plains: 1, ..., Badlands: 37, WoodedBadlands: 38,
//! BadlandsPlateau: 39, ..., ErodedBadlands: 165,
//! ModifiedWoodedBadlandsPlateau: 166, ModifiedBadlandsPlateau: 167`)
//! funnels into its cubiomes biome noise. `Plains` (1) and `Ocean` (0)
//! already matched; the six badlands ids were the gap.
//!
//! What is *not* ported from niduy: its 1.18+ climate engine (temperature,
//! humidity, continentalness, erosion and weirdness double-Perlin noises
//! plus spline/Voronoi biome noise and a ~1.6k-entry parameter tree).
//! PocketMine's `BiomeSelector` is a temperature/rainfall lookup, and the
//! PocketMine seed-parity suite expects those elevations/blocks exactly,
//! so the selector keeps its 11 emitted biomes and, like PocketMine,
//! never emits a badlands id (see
//! [`tests::selector_lookup_never_emits_badlands`]). niduy locates biomes
//! and structures only — it has no terrain or water pass — so sea level
//! stays PocketMine's (`NormalGenerator::SEA_LEVEL = 62`); the Java
//! client side (plains/badlands registry indexes, 26.1 `fluidCount`)
//! lives in `crate::java_chunk`.

use crate::block_id;
use crate::generator::noise::SimplexNoise;
use crate::generator::random::Random;

pub mod biome_ids {
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
    /// Badlands family (`niduy .../const.ts` `BIOME`): legacy biome ids
    /// 37-39 and the 1.13+ eroded/modified variants 165-167. Named like
    /// PocketMine (`WOODED_BADLANDS_PLATEAU` for 38). Defined for
    /// completeness (info/cover) but, like PocketMine, not emitted by
    /// `BiomeSelector::lookup`.
    pub const BADLANDS: u8 = 37;
    pub const WOODED_BADLANDS_PLATEAU: u8 = 38;
    pub const BADLANDS_PLATEAU: u8 = 39;
    pub const ERODED_BADLANDS: u8 = 165;
    pub const MODIFIED_WOODED_BADLANDS_PLATEAU: u8 = 166;
    pub const MODIFIED_BADLANDS_PLATEAU: u8 = 167;
}

pub use biome_ids::*;

#[derive(Debug, Clone, Copy)]
pub struct BiomeInfo {
    pub id: u8,
    pub name: &'static str,
    pub min_elevation: f64,
    pub max_elevation: f64,
    pub ground_cover: &'static [u32],
    pub temperature: f64,
    pub rainfall: f64,
}

static GRASSY_COVER: &[u32] = &[
    block_id::GRASS_BLOCK,
    block_id::DIRT,
    block_id::DIRT,
    block_id::DIRT,
    block_id::DIRT,
];

static SANDY_COVER: &[u32] = &[
    block_id::SAND,
    block_id::SAND,
    block_id::SANDSTONE,
    block_id::SANDSTONE,
    block_id::SANDSTONE,
];

/// Badlands surface: PocketMine has no red sand/terracotta block in
/// `block_id`, so the desert's sand/sandstone stack stands in.
static BADLANDS_COVER: &[u32] = &[
    block_id::SAND,
    block_id::SAND,
    block_id::SAND,
    block_id::SANDSTONE,
    block_id::SANDSTONE,
];

static SNOWY_COVER: &[u32] = &[
    block_id::SNOW_LAYER,
    block_id::GRASS_BLOCK,
    block_id::DIRT,
    block_id::DIRT,
    block_id::DIRT,
];

static RIVER_COVER: &[u32] = &[
    block_id::DIRT,
    block_id::DIRT,
    block_id::DIRT,
    block_id::DIRT,
    block_id::DIRT,
];

static GRAVEL_COVER: &[u32] = &[
    block_id::GRAVEL,
    block_id::GRAVEL,
    block_id::GRAVEL,
    block_id::GRAVEL,
    block_id::GRAVEL,
];

static _EMPTY_COVER: &[u32] = &[];

pub fn get_biome_info(id: u8) -> BiomeInfo {
    match id {
        OCEAN => BiomeInfo {
            id: OCEAN,
            name: "Ocean",
            min_elevation: 46.0,
            max_elevation: 58.0,
            ground_cover: GRAVEL_COVER,
            temperature: 0.5,
            rainfall: 0.5,
        },
        PLAINS => BiomeInfo {
            id: PLAINS,
            name: "Plains",
            min_elevation: 63.0,
            max_elevation: 68.0,
            ground_cover: GRASSY_COVER,
            temperature: 0.8,
            rainfall: 0.4,
        },
        DESERT => BiomeInfo {
            id: DESERT,
            name: "Desert",
            min_elevation: 63.0,
            max_elevation: 74.0,
            ground_cover: SANDY_COVER,
            temperature: 2.0,
            rainfall: 0.0,
        },
        EXTREME_HILLS => BiomeInfo {
            id: EXTREME_HILLS,
            name: "Mountains",
            min_elevation: 63.0,
            max_elevation: 127.0,
            ground_cover: GRASSY_COVER,
            temperature: 0.4,
            rainfall: 0.5,
        },
        FOREST => BiomeInfo {
            id: FOREST,
            name: "Forest",
            min_elevation: 63.0,
            max_elevation: 81.0,
            ground_cover: GRASSY_COVER,
            temperature: 0.7,
            rainfall: 0.8,
        },
        TAIGA => BiomeInfo {
            id: TAIGA,
            name: "Taiga",
            min_elevation: 63.0,
            max_elevation: 81.0,
            ground_cover: SNOWY_COVER,
            temperature: 0.05,
            rainfall: 0.8,
        },
        SWAMPLAND => BiomeInfo {
            id: SWAMPLAND,
            name: "Swamp",
            min_elevation: 62.0,
            max_elevation: 63.0,
            ground_cover: GRASSY_COVER,
            temperature: 0.8,
            rainfall: 0.9,
        },
        RIVER => BiomeInfo {
            id: RIVER,
            name: "River",
            min_elevation: 58.0,
            max_elevation: 62.0,
            ground_cover: RIVER_COVER,
            temperature: 0.5,
            rainfall: 0.7,
        },
        ICE_PLAINS => BiomeInfo {
            id: ICE_PLAINS,
            name: "Ice Plains",
            min_elevation: 63.0,
            max_elevation: 74.0,
            ground_cover: SNOWY_COVER,
            temperature: 0.05,
            rainfall: 0.8,
        },
        EXTREME_HILLS_EDGE => BiomeInfo {
            id: EXTREME_HILLS_EDGE,
            name: "Small Mountains",
            min_elevation: 63.0,
            max_elevation: 97.0,
            ground_cover: GRASSY_COVER,
            temperature: 0.4,
            rainfall: 0.5,
        },
        BIRCH_FOREST => BiomeInfo {
            id: BIRCH_FOREST,
            name: "Birch Forest",
            min_elevation: 63.0,
            max_elevation: 81.0,
            ground_cover: GRASSY_COVER,
            temperature: 0.6,
            rainfall: 0.5,
        },
        BADLANDS => BiomeInfo {
            id: BADLANDS,
            name: "Badlands",
            min_elevation: 63.0,
            max_elevation: 120.0,
            ground_cover: BADLANDS_COVER,
            temperature: 2.0,
            rainfall: 0.0,
        },
        WOODED_BADLANDS_PLATEAU => BiomeInfo {
            id: WOODED_BADLANDS_PLATEAU,
            name: "Wooded Badlands Plateau",
            min_elevation: 63.0,
            max_elevation: 110.0,
            ground_cover: BADLANDS_COVER,
            temperature: 2.0,
            rainfall: 0.2,
        },
        BADLANDS_PLATEAU => BiomeInfo {
            id: BADLANDS_PLATEAU,
            name: "Badlands Plateau",
            min_elevation: 63.0,
            max_elevation: 100.0,
            ground_cover: BADLANDS_COVER,
            temperature: 2.0,
            rainfall: 0.0,
        },
        ERODED_BADLANDS => BiomeInfo {
            id: ERODED_BADLANDS,
            name: "Eroded Badlands",
            min_elevation: 63.0,
            max_elevation: 127.0,
            ground_cover: BADLANDS_COVER,
            temperature: 2.0,
            rainfall: 0.0,
        },
        MODIFIED_WOODED_BADLANDS_PLATEAU => BiomeInfo {
            id: MODIFIED_WOODED_BADLANDS_PLATEAU,
            name: "Modified Wooded Badlands Plateau",
            min_elevation: 63.0,
            max_elevation: 110.0,
            ground_cover: BADLANDS_COVER,
            temperature: 2.0,
            rainfall: 0.2,
        },
        MODIFIED_BADLANDS_PLATEAU => BiomeInfo {
            id: MODIFIED_BADLANDS_PLATEAU,
            name: "Modified Badlands Plateau",
            min_elevation: 63.0,
            max_elevation: 100.0,
            ground_cover: BADLANDS_COVER,
            temperature: 2.0,
            rainfall: 0.0,
        },
        _ => BiomeInfo {
            id,
            name: "Ocean",
            min_elevation: 46.0,
            max_elevation: 58.0,
            ground_cover: GRAVEL_COVER,
            temperature: 0.5,
            rainfall: 0.5,
        },
    }
}

/// Gaussian elevation blur kernel matching `pocketmine\world\generator\Gaussian(2)`.
#[derive(Debug, Clone)]
pub struct Gaussian {
    pub smooth_size: i32,
    pub kernel_1d: Vec<f64>,
    pub weight_sum_1d: f64,
}

impl Gaussian {
    pub fn new(smooth_size: i32) -> Self {
        let bell_size = 1.0 / smooth_size as f64;
        let bell_height = 2.0 * smooth_size as f64;
        let count = (smooth_size * 2 + 1) as usize;
        let mut kernel_1d = vec![0.0; count];

        for sx in -smooth_size..=smooth_size {
            let bx = bell_size * sx as f64;
            kernel_1d[(sx + smooth_size) as usize] = bell_height.sqrt() * (-bx * bx / 2.0).exp();
        }

        let weight_sum_1d: f64 = kernel_1d.iter().sum();
        Self {
            smooth_size,
            kernel_1d,
            weight_sum_1d,
        }
    }
}

/// BiomeSelector implementing the temperature and rainfall Voronoi-like map.
#[derive(Debug, Clone)]
pub struct BiomeSelector {
    temperature: SimplexNoise,
    rainfall: SimplexNoise,
    map: [u8; 64 * 64],
}

impl BiomeSelector {
    pub fn new(random: &mut Random) -> Self {
        let temperature = SimplexNoise::new(random, 2, 1.0 / 16.0, 1.0 / 512.0);
        let rainfall = SimplexNoise::new(random, 2, 1.0 / 16.0, 1.0 / 512.0);

        let mut map = [0u8; 64 * 64];
        for i in 0..64 {
            for j in 0..64 {
                map[i + (j << 6)] = Self::lookup(i as f64 / 63.0, j as f64 / 63.0);
            }
        }

        Self {
            temperature,
            rainfall,
            map,
        }
    }

    pub fn lookup(temperature: f64, rainfall: f64) -> u8 {
        if rainfall < 0.25 {
            if temperature < 0.7 {
                OCEAN
            } else if temperature < 0.85 {
                RIVER
            } else {
                SWAMPLAND
            }
        } else if rainfall < 0.60 {
            if temperature < 0.25 {
                ICE_PLAINS
            } else if temperature < 0.75 {
                PLAINS
            } else {
                DESERT
            }
        } else if rainfall < 0.80 {
            if temperature < 0.25 {
                TAIGA
            } else if temperature < 0.75 {
                FOREST
            } else {
                BIRCH_FOREST
            }
        } else if temperature < 0.20 {
            EXTREME_HILLS
        } else if temperature < 0.40 {
            EXTREME_HILLS_EDGE
        } else {
            RIVER
        }
    }

    pub fn get_temperature(&self, x: f64, z: f64) -> f64 {
        (self.temperature.noise_2d(x, z, true) + 1.0) / 2.0
    }

    pub fn get_rainfall(&self, x: f64, z: f64) -> f64 {
        (self.rainfall.noise_2d(x, z, true) + 1.0) / 2.0
    }

    pub fn pick_biome(&self, x: i32, z: i32) -> u8 {
        let temp = (self.get_temperature(x as f64, z as f64) * 63.0) as i32;
        let rain = (self.get_rainfall(x as f64, z as f64) * 63.0) as i32;

        let temp_clamped = temp.clamp(0, 63) as usize;
        let rain_clamped = rain.clamp(0, 63) as usize;

        self.map[temp_clamped + (rain_clamped << 6)]
    }
}

/// Applies noise jitter to coordinates and picks the biome, matching `Normal::pickBiome`.
pub fn pick_biome_with_hash(seed: i64, selector: &BiomeSelector, x: i32, z: i32) -> u8 {
    let h1 = (x as i64).wrapping_mul(2345803) ^ (z as i64).wrapping_mul(9236449) ^ seed;
    let hash_f = (h1 as f64) * ((h1 as f64) + 223.0);
    let hash = (hash_f % (2.0f64.powi(63))) as i64;
    let mut x_noise = (hash >> 20) & 3;
    let mut z_noise = (hash >> 22) & 3;
    if x_noise == 3 {
        x_noise = 1;
    }
    if z_noise == 3 {
        z_noise = 1;
    }
    selector.pick_biome(x + x_noise as i32 - 1, z + z_noise as i32 - 1)
}

/// Computes smoothed min and max elevation heightmaps for a chunk (16x16)
/// using Gaussian blur across a (16 + 2*padding)^2 grid.
pub fn generate_chunk_biomes_and_elevations(
    seed: i64,
    selector: &BiomeSelector,
    gaussian: &Gaussian,
    base_x: i32,
    base_z: i32,
) -> ([u8; 256], [f64; 256], [f64; 256]) {
    let padding = gaussian.smooth_size;
    let grid_size = 16 + padding * 2; // 20
    let start = -padding;
    let end = 16 + padding;

    // 1. Sample biomes across grid [-2..18]
    let mut grid_biomes = vec![0u8; (grid_size * grid_size) as usize];
    let mut chunk_biomes = [0u8; 256];
    let mut uniform_biome: Option<u8> = None;
    let mut is_uniform = true;

    for x in start..end {
        let gx = (x + padding) as usize;
        let abs_x = base_x + x;
        for z in start..end {
            let gz = (z + padding) as usize;
            let abs_z = base_z + z;
            let b = pick_biome_with_hash(seed, selector, abs_x, abs_z);
            grid_biomes[gx * grid_size as usize + gz] = b;

            if is_uniform {
                match uniform_biome {
                    None => uniform_biome = Some(b),
                    Some(prev) if prev == b => {}
                    _ => is_uniform = false,
                }
            }

            if x >= 0 && x < 16 && z >= 0 && z < 16 {
                chunk_biomes[x as usize * 16 + z as usize] = b;
            }
        }
    }

    let mut min_heights = [0.0f64; 256];
    let mut max_heights = [0.0f64; 256];

    if is_uniform {
        let b_info = get_biome_info(uniform_biome.unwrap_or(PLAINS));
        let min_el = b_info.min_elevation - 1.0;
        let max_el = b_info.max_elevation;
        min_heights.fill(min_el);
        max_heights.fill(max_el);
        return (chunk_biomes, min_heights, max_heights);
    }

    // 2. Blur along X axis first: min_heights_x and max_heights_x for (x: 0..16, z: -2..18)
    let mut min_heights_x = vec![0.0f64; (16 * grid_size) as usize];
    let mut max_heights_x = vec![0.0f64; (16 * grid_size) as usize];

    for x in 0..16usize {
        for z in start..end {
            let gz = (z + padding) as usize;
            let mut min_sum = 0.0;
            let mut max_sum = 0.0;

            for sx in -gaussian.smooth_size..=gaussian.smooth_size {
                let weight = gaussian.kernel_1d[(sx + gaussian.smooth_size) as usize];
                let adj_gx = (x as i32 + sx + padding) as usize;
                let adj_biome = grid_biomes[adj_gx * grid_size as usize + gz];
                let info = get_biome_info(adj_biome);
                min_sum += (info.min_elevation - 1.0) * weight;
                max_sum += info.max_elevation * weight;
            }

            let idx = x * grid_size as usize + gz;
            min_heights_x[idx] = min_sum / gaussian.weight_sum_1d;
            max_heights_x[idx] = max_sum / gaussian.weight_sum_1d;
        }
    }

    // 3. Blur along Z axis using min_heights_x
    for x in 0..16usize {
        for z in 0..16usize {
            let mut min_sum = 0.0;
            let mut max_sum = 0.0;

            for sz in -gaussian.smooth_size..=gaussian.smooth_size {
                let weight = gaussian.kernel_1d[(sz + gaussian.smooth_size) as usize];
                let adj_gz = (z as i32 + sz + padding) as usize;
                let idx = x * grid_size as usize + adj_gz;
                min_sum += min_heights_x[idx] * weight;
                max_sum += max_heights_x[idx] * weight;
            }

            let out_idx = x * 16 + z;
            min_heights[out_idx] = min_sum / gaussian.weight_sum_1d;
            max_heights[out_idx] = max_sum / gaussian.weight_sum_1d;
        }
    }

    (chunk_biomes, min_heights, max_heights)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Legacy biome ids are the contract shared with
    /// `niduy/minecraft-world-generator` (`src/lib/minecraft/const.ts`
    /// `BIOME}) and PocketMine's `Biome` constants. The pre-existing 11
    /// keep their ids; the badlands family closes the gap.
    #[test]
    fn legacy_ids_match_niduy_and_pocketmine() {
        // Focus of the port: plains / ocean / badlands.
        assert_eq!(OCEAN, 0);
        assert_eq!(PLAINS, 1);
        assert_eq!(BADLANDS, 37);
        assert_eq!(WOODED_BADLANDS_PLATEAU, 38);
        assert_eq!(BADLANDS_PLATEAU, 39);
        assert_eq!(ERODED_BADLANDS, 165);
        assert_eq!(MODIFIED_WOODED_BADLANDS_PLATEAU, 166);
        assert_eq!(MODIFIED_BADLANDS_PLATEAU, 167);
        // The other PocketMine 11, pinned so the set cannot drift.
        assert_eq!(DESERT, 2);
        assert_eq!(EXTREME_HILLS, 3);
        assert_eq!(FOREST, 4);
        assert_eq!(TAIGA, 5);
        assert_eq!(SWAMPLAND, 6);
        assert_eq!(RIVER, 7);
        assert_eq!(ICE_PLAINS, 12);
        assert_eq!(EXTREME_HILLS_EDGE, 20);
        assert_eq!(BIRCH_FOREST, 27);
    }

    #[test]
    fn badlands_family_uses_sandy_cover_and_hot_dry_climate() {
        for id in [
            BADLANDS,
            WOODED_BADLANDS_PLATEAU,
            BADLANDS_PLATEAU,
            ERODED_BADLANDS,
            MODIFIED_WOODED_BADLANDS_PLATEAU,
            MODIFIED_BADLANDS_PLATEAU,
        ] {
            let info = get_biome_info(id);
            assert_eq!(info.id, id, "bioma {id}");
            assert_eq!(info.temperature, 2.0, "bioma {id}: calor como el desierto");
            assert!(info.rainfall < info.temperature, "bioma {id}: seco");
            assert!(
                info.ground_cover.iter().any(|&b| b == block_id::SAND)
                    && info.ground_cover.iter().any(|&b| b == block_id::SANDSTONE),
                "bioma {id}: cubierta arenosa (sin red sand en block_id)"
            );
        }
        // Plains/ocean references for the comparison: grass vs gravel.
        assert_eq!(
            get_biome_info(PLAINS).ground_cover[0],
            block_id::GRASS_BLOCK
        );
        assert_eq!(get_biome_info(OCEAN).ground_cover[0], block_id::GRAVEL);
    }

    /// Parity lock: like PocketMine, the temperature/rainfall selector
    /// never emits a badlands id anywhere in its 64x64 lookup table.
    /// niduy's climate engine *would* (erosion + temperature), but
    /// porting it would change PocketMine seed terrain, so the badlands
    /// family stays info-only.
    #[test]
    fn selector_lookup_never_emits_badlands() {
        for i in 0..=63 {
            for j in 0..=63 {
                let b = BiomeSelector::lookup(i as f64 / 63.0, j as f64 / 63.0);
                assert!(
                    !matches!(
                        b,
                        BADLANDS
                            | WOODED_BADLANDS_PLATEAU
                            | BADLANDS_PLATEAU
                            | ERODED_BADLANDS
                            | MODIFIED_WOODED_BADLANDS_PLATEAU
                            | MODIFIED_BADLANDS_PLATEAU
                    ),
                    "lookup({i},{j}) emitió badlands {b}"
                );
            }
        }
        // Spot checks of the two focus biomes that *are* emitted.
        assert_eq!(BiomeSelector::lookup(0.0, 0.0), OCEAN);
        assert_eq!(BiomeSelector::lookup(0.5, 0.5), PLAINS);
    }
}
