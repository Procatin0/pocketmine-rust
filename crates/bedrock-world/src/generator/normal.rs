//! PocketMine-MP Normal World Generator port.
//!
//! Matches `pocketmine\world\generator\normal\Normal`:
//! - 3D Simplex noise terrain generation with 4/8/4 sampling rate
//! - Overworld Voronoi-like biomes with Gaussian elevation smoothing
//! - Bedrock foundation at Y=0, solid stone beneath noise threshold
//! - Sea level water at Y <= 62
//! - GroundCover populator (Grass, Dirt, Sand, Sandstone, Snow Layer)
//! - Ore populator (Coal, Iron, Redstone, Lapis, Gold, Diamond, Dirt, Gravel)
//! - Biome populators (Trees: Oak, Spruce, Birch; TallGrass; Emerald Ore)

use crate::block_id;
use crate::chunk::Chunk;
use crate::generator::biome::{
    self, generate_chunk_biomes_and_elevations, BiomeSelector, Gaussian,
};
use crate::generator::manager::{ChunkManager, ChunkManager3x3};
use crate::generator::noise::SimplexNoise;
use crate::generator::populators::{
    GroundCoverPopulator, OrePopulator, TallGrassPopulator, TreePopulator, TreeType,
};
use crate::generator::random::Random;
use crate::generator::WorldGenerator;

pub struct NormalGenerator {
    seed: i64,
    noise_base: SimplexNoise,
    selector: BiomeSelector,
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
    spawn_pos: (i32, i32, i32),
}

impl NormalGenerator {
    pub const SEA_LEVEL: i32 = 62;

    pub fn new(seed: i64) -> Self {
        let mut random = Random::new(seed);
        let gaussian = Gaussian::new(2);
        let noise_base = SimplexNoise::new(&mut random, 4, 0.25, 1.0 / 32.0);

        random.set_seed(seed);
        let selector = BiomeSelector::new(&mut random);

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

        let mut gen = Self {
            seed,
            noise_base,
            selector,
            gaussian,
            water_height: Self::SEA_LEVEL,
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
            spawn_pos: (0, Self::SEA_LEVEL + 2, 0),
        };

        // Search for nearest solid land above sea level (safe spawn point matching PocketMine getSafeSpawn)
        let mut spawn = (0, Self::SEA_LEVEL + 2, 0);
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
                                if h > Self::SEA_LEVEL && block_id::is_solid(b) {
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

    /// Generates base terrain for a chunk (bedrock, stone, water, and GroundCover).
    pub fn generate_base_chunk(&self, chunk_x: i32, chunk_z: i32) -> Chunk {
        let mut random = Random::new(0xdeadbeef ^ ((chunk_x as i64) << 8) ^ (chunk_z as i64) ^ self.seed);
        let base_x = chunk_x * 16;
        let base_z = chunk_z * 16;

        let (chunk_biomes, min_noise_heights, max_noise_heights) =
            generate_chunk_biomes_and_elevations(self.seed, &self.selector, &self.gaussian, base_x, base_z);

        let dominant_biome = chunk_biomes[7 * 16 + 7];
        let mut chunk = Chunk::new(dominant_biome);
        chunk.biomes = chunk_biomes;

        let mut lowest_f = f64::INFINITY;
        let mut highest_f = f64::NEG_INFINITY;
        for &h in &min_noise_heights {
            if h < lowest_f {
                lowest_f = h;
            }
        }
        for &h in &max_noise_heights {
            if h > highest_f {
                highest_f = h;
            }
        }

        let lowest_noise_block = lowest_f.floor() as i32;
        let highest_noise_block = highest_f.ceil() as i32;

        let noise_min = (lowest_noise_block as f64 / 8.0).floor() as i32 * 8;
        let noise_max = (highest_noise_block as f64 / 8.0).ceil() as i32 * 8;
        let y_size = (noise_max - noise_min) as usize;

        let noise = self.noise_base.get_fast_noise_3d(
            16,
            y_size,
            16,
            4,
            8,
            4,
            base_x,
            noise_min,
            base_z,
        );

        let min_noise_subchunk = (noise_min as f64 / 16.0).floor() as i32;

        // Pre-fill solid stone subchunks below min_noise_subchunk
        for sc in 0..min_noise_subchunk {
            let sc_array_idx = (sc + 4) as usize;
            if sc_array_idx < chunk.subchunks.len() {
                chunk.subchunks[sc_array_idx] = crate::subchunk::SubChunk::new(block_id::STONE);
            }
        }

        for x in 0..16 {
            for z in 0..16 {
                chunk.set_block(x, 0, z, block_id::BEDROCK);

                let col_idx = x * 16 + z;
                let min_sum = min_noise_heights[col_idx];
                let max_sum = max_noise_heights[col_idx];
                let max_block_y = (max_sum as i32).max(self.water_height);
                let smooth_height = (max_sum - min_sum) / 2.0;

                let stone_start_y = min_noise_subchunk * 16;
                let mut y = stone_start_y;
                while (y as f64) < min_sum {
                    chunk.set_block(x, y, z, block_id::STONE);
                    y += 1;
                }

                for y in (min_sum.floor() as i32)..=max_block_y {
                    let noise_val = if y > noise_max {
                        -1.0
                    } else {
                        let ny = (y - noise_min) as usize;
                        if ny < noise[x][z].len() && smooth_height > 0.0 {
                            noise[x][z][ny] - (1.0 / smooth_height) * (y as f64 - smooth_height - min_sum)
                        } else {
                            -1.0
                        }
                    };

                    if noise_val > 0.0 {
                        chunk.set_block(x, y, z, block_id::STONE);
                    } else if y <= self.water_height {
                        chunk.set_block(x, y, z, block_id::WATER);
                    }
                }
            }
        }

        // Generation Populator: GroundCover
        self.ground_cover.populate(&mut chunk, chunk_x, chunk_z, &mut random);

        chunk
    }

    /// Populates a chunk using the world manager, matching `Normal::populateChunk`.
    pub fn populate_chunk<M: ChunkManager>(&self, world: &mut M, chunk_x: i32, chunk_z: i32, dominant_biome: u8) {
        let mut random = Random::new(0xdeadbeef ^ ((chunk_x as i64) << 8) ^ (chunk_z as i64) ^ self.seed);

        // 1. Overworld standard ores
        self.ore_populator.populate(world, chunk_x, chunk_z, &mut random);

        // 2. Biome-specific population
        match dominant_biome {
            biome::PLAINS => {
                self.tall_grass_plains.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::FOREST => {
                self.tree_forest.populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_forest.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::BIRCH_FOREST => {
                self.tree_birch.populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_forest.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::TAIGA => {
                self.tree_taiga.populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_taiga.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::EXTREME_HILLS | biome::EXTREME_HILLS_EDGE => {
                self.tree_mountains.populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_mountains.populate(world, chunk_x, chunk_z, &mut random);
                self.emerald_ore.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::RIVER => {
                self.tall_grass_river.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::OCEAN => {
                self.tall_grass_ocean.populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::ICE_PLAINS => {
                self.tall_grass_ice_plains.populate(world, chunk_x, chunk_z, &mut random);
            }
            _ => {}
        }
    }

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
}

impl WorldGenerator for NormalGenerator {
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
}
