//! PocketMine-MP Flat World Generator port.
//!
//! Matches `pocketmine\world\generator\Flat` with default preset `"2;bedrock,2xdirt,grass;1;"`:
//! - Y = 0: Bedrock
//! - Y = 1..2: Dirt (2 layers)
//! - Y = 3: Grass Block
//! - Y = 4..15: Air
//!
//! Subchunks -4..-1 (Y = -64..-1) are empty, serialized as `[0x08, 0x00]`.
//! Subchunk 0 (Y = 0..15) contains the 4 layers.
//! Subchunk count sent to client: 5 (covering Y = -64..15).

use crate::block_id;
use crate::chunk::Chunk;
use crate::generator::WorldGenerator;

pub struct FlatGenerator {
    /// Unmodified terrain, cloned when a chunk is generated with block edits.
    base_chunk: Chunk,
    /// Pre-serialized payload for the common, edit-free path.
    cached_payload: Vec<u8>,
    cached_subchunk_count: u32,
}

impl Default for FlatGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl FlatGenerator {
    pub fn new() -> Self {
        // Base chunk: Bedrock (y=0), Dirt (y=1..2), Grass (y=3), Air (y=4..15),
        // all columns with the Plains biome.
        let mut base_chunk = Chunk::new(1);
        for x in 0..16 {
            for z in 0..16 {
                base_chunk.set_block(x, 0, z, block_id::BEDROCK);
                base_chunk.set_block(x, 1, z, block_id::DIRT);
                base_chunk.set_block(x, 2, z, block_id::DIRT);
                base_chunk.set_block(x, 3, z, block_id::GRASS_BLOCK);
            }
        }

        // Invalidates nothing: the payload contains 4 empty subchunks plus subchunk 0.
        let (cached_subchunk_count, cached_payload) = base_chunk.serialize_network_payload(1);

        Self {
            base_chunk,
            cached_payload,
            cached_subchunk_count,
        }
    }
}

impl WorldGenerator for FlatGenerator {
    fn generator_type(&self) -> i32 {
        1 // FLAT
    }

    fn spawn_position(&self) -> (i32, i32, i32) {
        (0, 4, 0)
    }

    fn player_position(&self) -> (f32, f32, f32) {
        // Player standing on block at Y=3: feet at Y=4.0, eye height 1.62
        (0.5, 5.62, 0.5)
    }

    fn generate_chunk(&self, _chunk_x: i32, _chunk_z: i32) -> (u32, Vec<u8>) {
        (self.cached_subchunk_count, self.cached_payload.clone())
    }

    /// Analytic flat terrain query. `x` and `z` do not affect the result.
    fn block_at(&self, _x: i32, y: i32, _z: i32) -> u32 {
        match y {
            0 => block_id::BEDROCK,
            1..=2 => block_id::DIRT,
            3 => block_id::GRASS_BLOCK,
            _ => block_id::AIR,
        }
    }

    fn highest_block_at(&self, _x: i32, _z: i32) -> i32 {
        3
    }

    fn generate_chunk_with_edits(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        edits: &[(i32, i32, i32, u32)],
    ) -> (u32, Vec<u8>) {
        // Keep the cached fast path when no edit belongs to this chunk.
        let has_local_edits = edits
            .iter()
            .any(|&(x, _, z, _)| (x >> 4) == chunk_x && (z >> 4) == chunk_z);
        if !has_local_edits {
            return (self.cached_subchunk_count, self.cached_payload.clone());
        }

        let mut chunk = self.base_chunk.clone();
        for &(x, y, z, id) in edits {
            if (x >> 4) == chunk_x && (z >> 4) == chunk_z {
                chunk.set_block((x & 15) as usize, y, (z & 15) as usize, id);
            }
        }

        chunk.serialize_network_payload(1)
    }
}
