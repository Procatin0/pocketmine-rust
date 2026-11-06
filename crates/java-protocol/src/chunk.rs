//! Java 1.12.2 chunk column encoding.
//!
//! Layout of `map_chunk` (0x20) data for protocol 340, ground-up continuous:
//! for every section present in the bit mask, in ascending Y order:
//! `bits_per_block (u8)`, palette (`varint` length + `varint` states),
//! data array (`varint` long count + big-endian longs), block light
//! (2048 bytes, nibbles), sky light (2048 bytes, nibbles); after all sections,
//! 256 biome bytes.
//!
//! 1.12.2 uses the *non-compact* packing: a value never spans two longs, so a
//! long holds `64 / bits_per_block` states and the remainder is padding.

use crate::binary::Writer;

/// Number of block states in a full Java column (16 x 16 x 256).
pub const STATES_PER_CHUNK: usize = 16 * 16 * 256;

/// Length of the per-section light arrays, in bytes (4096 nibbles).
const LIGHT_ARRAY_BYTES: usize = 2048;

/// Builds the chunk data payload plus bit mask for a column.
///
/// `states` is indexed as `(y << 8) | (z << 4) | x`, which is the Java section
/// index order, and holds block states for the whole 0..256 height range.
pub fn build_chunk_data(states: &[u16]) -> (i32, Vec<u8>) {
    assert_eq!(
        states.len(),
        STATES_PER_CHUNK,
        "chunk builder needs a full 16x16x256 column"
    );

    let mut writer = Writer::new();
    let mut bit_map: i32 = 0;

    for section in 0..16usize {
        let base = section * 4096;
        let slice = &states[base..base + 4096];
        if slice.iter().all(|&state| state == 0) {
            continue;
        }
        bit_map |= 1 << section;

        let mut palette: Vec<u16> = Vec::new();
        for &state in slice {
            if !palette.contains(&state) {
                palette.push(state);
            }
        }

        let mut bits_per_block = 4u8;
        while (1usize << bits_per_block) < palette.len() {
            bits_per_block += 1;
        }
        // 1.12.2 supports up to 8 bits per block in the section format; the
        // generators this bridge serves never produce more than a handful of
        // distinct states per section, so the clamp is a safety net only.
        if bits_per_block > 8 {
            bits_per_block = 8;
            palette.truncate(256);
        }

        writer.write_u8(bits_per_block);
        writer.write_var_i32(palette.len() as i32);
        for &state in &palette {
            writer.write_var_i32(state as i32);
        }

        let states_per_long = 64 / bits_per_block as usize;
        let long_count = 4096 / states_per_long;
        writer.write_var_i32(long_count as i32);

        let mut words = vec![0u64; long_count];
        for (index, &state) in slice.iter().enumerate() {
            let palette_index = palette
                .iter()
                .position(|&entry| entry == state)
                .unwrap_or(0) as u64;
            let word = index / states_per_long;
            let shift = (index % states_per_long) * bits_per_block as usize;
            words[word] |= palette_index << shift;
        }
        for word in words {
            writer.write_i64(word as i64);
        }

        // Block light: no light sources are simulated yet.
        writer.write_bytes(&[0u8; LIGHT_ARRAY_BYTES]);
        // Sky light: full daylight so the column is not pitch black.
        writer.write_bytes(&[0xFFu8; LIGHT_ARRAY_BYTES]);
    }

    // Biomes: plains (1) for every column.
    writer.write_bytes(&[1u8; 256]);

    (bit_map, writer.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::java_state_for_bedrock;
    use bedrock_world::block_id;

    fn flat_column() -> Vec<u16> {
        let mut states = vec![0u16; STATES_PER_CHUNK];
        let grass = java_state_for_bedrock(block_id::GRASS_BLOCK) as usize;
        let dirt = java_state_for_bedrock(block_id::DIRT) as usize;
        for x in 0..16 {
            for z in 0..16 {
                states[(3 << 8) | (z << 4) | x] = grass as u16;
                states[(2 << 8) | (z << 4) | x] = dirt as u16;
                states[(1 << 8) | (z << 4) | x] = dirt as u16;
            }
        }
        states
    }

    #[test]
    fn flat_column_only_sets_section_zero() {
        let (bit_map, data) = build_chunk_data(&flat_column());
        assert_eq!(bit_map, 1);

        // bits per block, three-entry palette.
        assert_eq!(data[0], 4);
        assert_eq!(data[1], 3);
        assert_eq!(data.len(), 1 + 1 + 3 + 2 + 256 * 8 + 2048 + 2048 + 256);
        assert!(data[data.len() - 256..].iter().all(|&biome| biome == 1));
        assert!(data[data.len() - 256 - 2048..data.len() - 256]
            .iter()
            .all(|&light| light == 0xFF));
    }

    #[test]
    fn empty_column_has_no_sections() {
        let (bit_map, data) = build_chunk_data(&vec![0u16; STATES_PER_CHUNK]);
        assert_eq!(bit_map, 0);
        assert_eq!(data.len(), 256);
    }

    #[test]
    fn tall_column_marks_every_used_section() {
        let mut states = vec![0u16; STATES_PER_CHUNK];
        states[(255 << 8) | 0] = 1 << 4;
        let (bit_map, data) = build_chunk_data(&states);
        assert_eq!(bit_map, 0b1000_0000_0000_0000u16 as i32);
        assert!(!data.is_empty());
    }
}
