//! In-memory Bedrock Chunk (16x16 with 24 subchunks from index -4 to 19, Y = -64..319).

use crate::block_id;
use crate::subchunk::SubChunk;
use bedrock_protocol::Writer;

pub const MIN_SUBCHUNK_INDEX: i32 = -4;
pub const MAX_SUBCHUNK_INDEX: i32 = 19;
pub const TOTAL_SUBCHUNKS: usize = (MAX_SUBCHUNK_INDEX - MIN_SUBCHUNK_INDEX + 1) as usize; // 24

#[derive(Debug, Clone)]
pub struct Chunk {
    pub subchunks: [SubChunk; TOTAL_SUBCHUNKS],
    pub biomes: [u8; 256],
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new(1) // PLAINS
    }
}

impl Chunk {
    pub fn new(biome_id: u8) -> Self {
        Self {
            subchunks: std::array::from_fn(|_| SubChunk::empty()),
            biomes: [biome_id; 256],
        }
    }

    #[inline(always)]
    pub fn subchunk_index(y: i32) -> Option<usize> {
        let sc = (y >> 4) - MIN_SUBCHUNK_INDEX;
        if sc >= 0 && (sc as usize) < TOTAL_SUBCHUNKS {
            Some(sc as usize)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn get_block(&self, x: usize, y: i32, z: usize) -> u32 {
        if let Some(sc_idx) = Self::subchunk_index(y) {
            let by = (y & 15) as usize;
            let id = self.subchunks[sc_idx].get_block(x, by, z, 0);
            if id == 0 {
                block_id::AIR
            } else {
                id
            }
        } else {
            block_id::AIR
        }
    }

    #[inline(always)]
    pub fn set_block(&mut self, x: usize, y: i32, z: usize, id: u32) {
        if let Some(sc_idx) = Self::subchunk_index(y) {
            let by = (y & 15) as usize;
            self.subchunks[sc_idx].set_block(x, by, z, 0, id);
        }
    }

    pub fn get_highest_block_at(&self, x: usize, z: usize) -> Option<i32> {
        for sc_idx in (0..TOTAL_SUBCHUNKS).rev() {
            if self.subchunks[sc_idx].is_empty() {
                continue;
            }
            let base_y = (sc_idx as i32 + MIN_SUBCHUNK_INDEX) * 16;
            for by in (0..16).rev() {
                let id = self.subchunks[sc_idx].get_block(x, by, z, 0);
                if id != 0 && id != block_id::AIR {
                    return Some(base_y + by as i32);
                }
            }
        }
        None
    }

    /// Serializes chunk into Bedrock network format payload:
    /// - `subchunk_count` SubChunks (from -4 up to highest non-empty subchunk)
    /// - 24 Overworld Biome Palettes
    /// - 0 border blocks
    /// - 0 block entities
    ///
    /// Mirrors `ChunkSerializer::serializeFullChunk` /
    /// `ChunkSerializer::getSubChunkCount`: the count covers `-4` up to the
    /// top non-empty subchunk, and a fully empty column reports `0` (no
    /// subchunks are written, but the 24 biome palettes, 0 border blocks
    /// and the empty tiles section still follow, exactly like the
    /// reference). There is no minimum count; the old `.max(5)` floor is
    /// removed because a flat-world column already reports 5 on its own
    /// (subchunk 0 holds bedrock/dirt/grass).
    pub fn serialize_network_payload(&self, dominant_biome_id: u8) -> (u32, Vec<u8>) {
        let mut top_sc_idx = 0;
        let mut found_non_empty = false;
        for sc_idx in (0..TOTAL_SUBCHUNKS).rev() {
            if !self.subchunks[sc_idx].is_empty() {
                top_sc_idx = sc_idx;
                found_non_empty = true;
                break;
            }
        }

        let subchunk_count = if found_non_empty {
            (top_sc_idx + 1) as u32
        } else {
            0
        };

        let mut writer = Writer::new();
        for sc_idx in 0..(subchunk_count as usize) {
            let sc = &self.subchunks[sc_idx];
            if sc.is_empty() {
                writer.write_byte(8); // version 8
                writer.write_byte(0); // 0 layers
            } else {
                sc.serialize_network_to(&mut writer);
            }
        }

        // 24 Overworld Biome Palettes
        for _ in 0..24 {
            writer.write_byte(1); // (0 << 1) | 1 (bpb=0, runtime=1)
            writer.write_var_i32(dominant_biome_id as i32);
        }

        // Border blocks: 0
        writer.write_byte(0);

        // Tiles as byte array (empty = varu length 0, like `writeByteArray`)
        writer.write_var_u32(0);

        (subchunk_count, writer.into_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_chunk_reports_zero_subchunks() {
        // `ChunkSerializer::getSubChunkCount` returns 0 for a fully empty
        // column; the payload still carries 24 biome palettes, the 0
        // border-block byte and no tiles.
        let chunk = Chunk::new(1);
        let (count, payload) = chunk.serialize_network_payload(1);
        assert_eq!(count, 0);
        // 24 * (header byte + var_i32 biome) + 1 border byte.
        assert_eq!(payload.len(), 24 * 2 + 1 + 1);
    }

    #[test]
    fn flat_column_reports_five_subchunks() {
        // Bedrock at Y=0 lives in subchunk 0 (index 4 from -4), so the
        // overworld count is 4 + 1 = 5, like the PM flat generator.
        let mut chunk = Chunk::new(1);
        chunk.set_block(0, 0, 0, block_id::BEDROCK);
        let (count, _) = chunk.serialize_network_payload(1);
        assert_eq!(count, 5);
    }
}
