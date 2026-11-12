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
use crate::generator::caves::CavePopulator;
use crate::generator::manager::{ChunkManager, ChunkManager3x3};
use crate::generator::noise::SimplexNoise;
use crate::generator::populators::{
    GroundCoverPopulator, OrePopulator, TallGrassPopulator, TreePopulator, TreeType,
};
use crate::generator::random::Random;
use crate::generator::structures::{self, StructurePosition};
use crate::generator::WorldGenerator;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Columns kept for single-block queries.
///
/// [`NormalGenerator::generate_chunk_internal`] builds a 3x3 neighbourhood and
/// runs every populator (~18 ms), so asking it for one block is unaffordable:
/// `WorldState::propagate_connections` alone reads dozens of cells per edit,
/// which turned a single block break into 100-600 ms of terrain generation
/// (`[lag-slow] actions=4 total=665.4ms slowest=665.2ms: SetBlock`). A column is
/// a pure function of the seed (edits live in `WorldState`), so caching one is
/// safe; the bound keeps it to a few MB.
const COLUMN_CACHE_CAP: usize = 32;

#[derive(Default)]
struct ColumnCache {
    entries: HashMap<(i32, i32), Arc<Chunk>>,
    order: VecDeque<(i32, i32)>,
}

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
    cave_populator: CavePopulator,
    spawn_pos: (i32, i32, i32),
    /// See [`COLUMN_CACHE_CAP`]; only `block_at`/`highest_block_at` use it.
    column_cache: Mutex<ColumnCache>,
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

        let cave_populator = CavePopulator::new();

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
            cave_populator,
            spawn_pos: (0, Self::SEA_LEVEL + 2, 0),
            column_cache: Mutex::new(ColumnCache::default()),
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
        let mut random =
            Random::new(0xdeadbeef ^ ((chunk_x as i64) << 8) ^ (chunk_z as i64) ^ self.seed);
        let base_x = chunk_x * 16;
        let base_z = chunk_z * 16;

        let (chunk_biomes, min_noise_heights, max_noise_heights) =
            generate_chunk_biomes_and_elevations(
                self.seed,
                &self.selector,
                &self.gaussian,
                base_x,
                base_z,
            );

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

        let noise = self
            .noise_base
            .get_fast_noise_3d(16, y_size, 16, 4, 8, 4, base_x, noise_min, base_z);

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
                            noise[x][z][ny]
                                - (1.0 / smooth_height) * (y as f64 - smooth_height - min_sum)
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
        self.ground_cover
            .populate(&mut chunk, chunk_x, chunk_z, &mut random);

        chunk
    }

    /// Populates a chunk using the world manager, matching `Normal::populateChunk`.
    pub fn populate_chunk<M: ChunkManager>(
        &self,
        world: &mut M,
        chunk_x: i32,
        chunk_z: i32,
        dominant_biome: u8,
    ) {
        let mut random =
            Random::new(0xdeadbeef ^ ((chunk_x as i64) << 8) ^ (chunk_z as i64) ^ self.seed);

        // 1. Overworld standard ores
        self.ore_populator
            .populate(world, chunk_x, chunk_z, &mut random);

        // 2. Biome-specific population
        match dominant_biome {
            biome::PLAINS => {
                self.tall_grass_plains
                    .populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::FOREST => {
                self.tree_forest
                    .populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_forest
                    .populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::BIRCH_FOREST => {
                self.tree_birch
                    .populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_forest
                    .populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::TAIGA => {
                self.tree_taiga
                    .populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_taiga
                    .populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::EXTREME_HILLS | biome::EXTREME_HILLS_EDGE => {
                self.tree_mountains
                    .populate(world, chunk_x, chunk_z, &mut random);
                self.tall_grass_mountains
                    .populate(world, chunk_x, chunk_z, &mut random);
                self.emerald_ore
                    .populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::RIVER => {
                self.tall_grass_river
                    .populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::OCEAN => {
                self.tall_grass_ocean
                    .populate(world, chunk_x, chunk_z, &mut random);
            }
            biome::ICE_PLAINS => {
                self.tall_grass_ice_plains
                    .populate(world, chunk_x, chunk_z, &mut random);
            }
            _ => {}
        }

        // Cave tunnels on a dedicated RNG — they run after the shared
        // `random` stream is done, so ores/trees/tall-grass above are
        // untouched. Skipped for water biomes: the certified seed-0 chunks
        // (0,0)=RIVER keep their section-7 stone/water byte-for-byte, and
        // the Y <= 44 cap keeps section 7 (Y 48..=63, the 812-fluid zone)
        // untouched in every other chunk too.
        match dominant_biome {
            biome::OCEAN | biome::RIVER | biome::ICE_PLAINS | biome::SWAMPLAND => {}
            _ => {
                let mut cave_random = Random::new(
                    self.seed ^ ((chunk_x as i64) << 32) ^ (chunk_z as i64) ^ 0xC4E5_F00D,
                );
                self.cave_populator
                    .populate(world, chunk_x, chunk_z, &mut cave_random);
            }
        }
    }

    /// Ported niduy structure placements for one chunk (position only —
    /// mutates no voxel; see `generator::structures`).
    pub fn structures_at(&self, chunk_x: i32, chunk_z: i32) -> Vec<StructurePosition> {
        structures::structure_positions_for_chunk(self.seed, chunk_x, chunk_z)
    }

    /// Column for a single-block query, generating (and caching) on a miss.
    fn cached_column(&self, chunk_x: i32, chunk_z: i32) -> Arc<Chunk> {
        let key = (chunk_x, chunk_z);
        if let Some(chunk) = self.column_cache.lock().entries.get(&key) {
            return Arc::clone(chunk);
        }
        // Generated outside the lock: another thread may win the insert, which
        // is harmless (same seed, same column).
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
    fn seed(&self) -> i64 {
        self.seed
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

    /// Returns the block id at absolute world coordinates.
    ///
    /// **Expensive:** this regenerates the target chunk (and its 3x3
    /// neighbourhood of base chunks) on every call. Use it sparingly, e.g.
    /// for occasional plugin `getBlock` queries; prefer reading from a cached
    /// `Chunk` when the server already holds one.
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
