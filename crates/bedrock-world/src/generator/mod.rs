//! World Generation module for Bedrock Edition
//!
//! Provides flat and procedural terrain generators ported directly from PocketMine-MP.

use std::sync::Arc;

pub mod biome;
pub mod flat;
pub mod noise;
pub mod normal;
pub mod populators;
pub mod random;

pub use flat::FlatGenerator;
pub use normal::NormalGenerator;

/// Common trait for Bedrock world generators.
pub trait WorldGenerator: Send + Sync {
    /// Returns the Bedrock generator type (1 = Flat, 2 = Default/Infinite).
    fn generator_type(&self) -> i32;

    /// Recommended world spawn position (x, y, z).
    fn spawn_position(&self) -> (i32, i32, i32);

    /// Recommended player eye spawn position (x, y, z).
    fn player_position(&self) -> (f32, f32, f32);

    /// Generates LevelChunk payload for (chunk_x, chunk_z).
    /// Returns `(sub_chunk_count, payload_bytes)`.
    fn generate_chunk(&self, chunk_x: i32, chunk_z: i32) -> (u32, Vec<u8>);
}

/// Creates a `WorldGenerator` from a level-type string ("flat" or "normal"/"default") and a seed.
pub fn create_generator(level_type: &str, seed: u64) -> Arc<dyn WorldGenerator> {
    match level_type.to_lowercase().as_str() {
        "flat" => Arc::new(FlatGenerator::new()),
        _ => Arc::new(NormalGenerator::new(seed as i64)),
    }
}
