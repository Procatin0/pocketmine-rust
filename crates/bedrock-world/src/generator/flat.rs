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

use bedrock_protocol::Writer;
use crate::block_id;
use crate::generator::WorldGenerator;
use crate::subchunk::SubChunk;

pub struct FlatGenerator {
    cached_payload: Vec<u8>,
}

impl Default for FlatGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl FlatGenerator {
    pub fn new() -> Self {
        let mut writer = Writer::new();

        // 1. Subchunks -4, -3, -2, -1 (Y = -64..-1): Empty subchunks [0x08, 0x00]
        let empty_subchunk = SubChunk::empty();
        for _ in 0..4 {
            empty_subchunk.serialize_network_to(&mut writer);
        }

        // 2. Subchunk 0 (Y = 0..15): Bedrock (y=0), Dirt (y=1..2), Grass (y=3), Air (y=4..15)
        let mut s0 = SubChunk::new(block_id::AIR);
        for x in 0..16 {
            for z in 0..16 {
                s0.set_block(x, 0, z, 0, block_id::BEDROCK);
                s0.set_block(x, 1, z, 0, block_id::DIRT);
                s0.set_block(x, 2, z, 0, block_id::DIRT);
                s0.set_block(x, 3, z, 0, block_id::GRASS_BLOCK);
            }
        }
        s0.serialize_network_to(&mut writer);

        // 3. 24 Overworld biome palettes (Plains = 1)
        for _ in 0..24 {
            writer.write_byte(1); // (0 << 1) | 1 (bpb=0, runtime=1)
            writer.write_var_i32(1); // Plains
        }

        // 4. Border blocks count: 0
        writer.write_byte(0);

        // 5. Block entities (tiles): 0
        let cached_payload = writer.into_bytes().to_vec();
        Self { cached_payload }
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
        (5, self.cached_payload.clone())
    }
}
