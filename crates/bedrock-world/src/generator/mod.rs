//! World Generation module for Bedrock Edition
//!
//! Provides flat and procedural terrain generators ported directly from PocketMine-MP.

use std::sync::Arc;

pub mod biome;
pub mod bds_biome_tree;
pub mod bds_noise_params;
pub mod bds_seed_hash;
pub mod bds_terrain;
pub mod caves;
pub mod cubiomes;
pub mod flat;
pub mod java_biome_tree;
pub mod java_blended_noise;
pub mod java_density;
pub mod java_biomes;
pub mod java_terrain;
pub mod manager;
pub mod noise;
pub mod normal;
pub mod populators;
pub mod random;
pub mod structures;

pub use bds_noise_params::{BEDROCK_CLIMATE_SEEDS, BedrockNoiseConfig, CLIMATE_TEMPERATURE, CLIMATE_HUMIDITY, CLIMATE_CONTINENTALNESS, CLIMATE_EROSION, CLIMATE_PEAKS_AND_VALLEYS, CLIMATE_WEIRDNESS, bedrock_string_seed, get_climate_seed_by_name};
pub use bds_seed_hash::{bedrock_string_to_seed, bedrock_string_seed_to_u64, murmur3_32};
pub use bds_terrain::{BDSTerrain, BDS_SEA_LEVEL};
pub use flat::FlatGenerator;
pub use java_biomes::{JavaBiomes, DoublePerlinNoise as JavaDoublePerlinNoise};
pub use java_density::{DensityRoot, TerrainDensity};
pub use java_terrain::{
    DoublePerlinNoise, JavaTerrain, PerlinNoise, Spline, Xoroshiro128Plus,
    BEDROCK_NORMAL_SEA_LEVEL, JAVA_SEA_LEVEL,
};
pub use manager::{ChunkManager, ChunkManager3x3};
pub use normal::NormalGenerator;

/// Common trait for Bedrock world generators.
pub trait WorldGenerator: Send + Sync {
    /// Returns the Bedrock generator type (1 = Flat, 2 = Default/Infinite).
    fn generator_type(&self) -> i32;

    /// Seed the world was generated with, reported by `/seed`.
    fn seed(&self) -> i64 {
        0
    }

    /// Recommended world spawn position (x, y, z).
    fn spawn_position(&self) -> (i32, i32, i32);

    /// Recommended player eye spawn position (x, y, z).
    fn player_position(&self) -> (f32, f32, f32);

    /// Generates LevelChunk payload for (chunk_x, chunk_z).
    /// Returns `(sub_chunk_count, payload_bytes)`.
    fn generate_chunk(&self, chunk_x: i32, chunk_z: i32) -> (u32, Vec<u8>);

    /// Builds the in-memory [`Chunk`](crate::chunk::Chunk) for a column,
    /// without edits. Used by the Java bridge to frame `ModernChunk` /
    /// `ChunkBuilder` payloads; the default is an empty plains column and
    /// concrete generators override it (the per-block `block_at` for
    /// `NormalGenerator` regenerates this column internally, so the
    /// Java path must not go through `block_at` per voxel).
    fn generate_chunk_object(&self, _chunk_x: i32, _chunk_z: i32) -> crate::chunk::Chunk {
        crate::chunk::Chunk::new(1)
    }

    /// Returns the network block id at absolute world coordinates.
    ///
    /// The default returns air because the trait has no access to a chunk store.
    fn block_at(&self, _x: i32, _y: i32, _z: i32) -> u32 {
        crate::block_id::AIR
    }

    /// Generates a `width x depth` block of columns, returning them in row
    /// order.
    ///
    /// The default walks [`WorldGenerator::generate_chunk_object`] per
    /// column; generators whose population pass needs the 3x3 base
    /// neighbourhood should override it so a bulk pass (pregenerator) builds
    /// each base chunk once instead of once per neighbour. `BDSTerrain` does
    /// that with [`BDSTerrain::generate_region`], which measured ~2.3x the
    /// per-column path on one core and ~3x with 16 threads.
    fn generate_region(
        &self,
        start_x: i32,
        start_z: i32,
        width: i32,
        depth: i32,
    ) -> Vec<((i32, i32), crate::chunk::Chunk)> {
        let mut out = Vec::with_capacity((width.max(0) * depth.max(0)) as usize);
        for cx in start_x..(start_x + width) {
            for cz in start_z..(start_z + depth) {
                out.push(((cx, cz), self.generate_chunk_object(cx, cz)));
            }
        }
        out
    }

    /// Generates a chunk applying block edits in ABSOLUTE world coordinates.
    /// Edits outside the chunk are ignored.
    fn generate_chunk_with_edits(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        _edits: &[(i32, i32, i32, u32)],
    ) -> (u32, Vec<u8>) {
        self.generate_chunk(chunk_x, chunk_z)
    }

    /// Highest non-air block in a world column (base terrain only).
    ///
    /// Generators should override this with a cheap implementation; the default
    /// assumes a typical overworld surface so plugins still get a sane answer.
    fn highest_block_at(&self, _x: i32, _z: i32) -> i32 {
        63
    }
}

/// Creates a `WorldGenerator` from a level-type string ("flat", "normal"/"default", or "bds") and a seed.
pub fn create_generator(level_type: &str, seed: u64) -> Arc<dyn WorldGenerator> {
    match level_type.to_lowercase().as_str() {
        "flat" => Arc::new(FlatGenerator::new()),
        "bds" => Arc::new(BDSTerrain::new(seed)),
        _ => Arc::new(NormalGenerator::new(seed as i64)),
    }
}
