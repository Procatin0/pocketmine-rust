//! Bedrock chunk records as stored by BDS.
//!
//! Only the parts needed for world-generation parity are decoded here:
//! the `Data3D` record (0x2B), which holds the 16x16 heightmap plus the 24
//! 16x16x16 biome sections of the Overworld.
//!
//! `Data3D` layout (verified against BDS 1.26.63.1 output):
//!
//! ```text
//! 512 bytes      heightmap, 256 * i16 little endian
//! 24 sections    one per 16-block slice from y=-64 upwards:
//!                header byte = (bits_per_index << 1) | persistent_palette
//!                  bits == 0x7f      -> "same as previous section"
//!                  bits == 0         -> one u32 value, no palette header
//!                  otherwise         -> 4096 packed indices (LSB first),
//!                                       then u32 entry count,
//!                                       then count * u32 biome ids
//! ```

/// Number of 16x16x16 biome sections in the Overworld.
pub const SECTION_COUNT: usize = 24;
/// Lowest block covered by the first section.
pub const MIN_SECTION_Y: i32 = -64;
/// Cells per section.
pub const SECTION_CELLS: usize = 16 * 16 * 16;

const HEIGHTMAP_BYTES: usize = 256 * 2;

/// LevelDB record tags that live in a chunk key. The ones this tool does not
/// decode yet are documented so later phases (blocks, block entities) can use
/// them without re-deriving the format.
#[allow(dead_code)]
pub mod tag {
    pub const VERSION: u8 = 0x2c;
    pub const DATA_3D: u8 = 0x2b;
    pub const SUB_CHUNK: u8 = 0x2f;
    pub const FINALIZED_STATE: u8 = 0x36;
}

/// Decoded `Data3D` record.
#[derive(Debug, Clone)]
pub struct Data3d {
    /// 256 heights, indexed `x | z << 4`.
    pub heightmap: [i16; 256],
    /// One entry per section with the "inherit previous section" marker
    /// already resolved, so every section holds 4096 biome ids.
    pub sections: Vec<Vec<u32>>,
    /// `stored[i]` is false when section `i` was marked as inherited.
    pub stored: Vec<bool>,
}

impl Data3d {
    /// Biome id at an absolute block position.
    ///
    /// Section contents are stored in XZY order, matching the block storage
    /// order used by Bedrock: `x | z << 4 | y << 8`.
    pub fn biome_at(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        if !(0..16).contains(&x) || !(0..16).contains(&z) {
            return None;
        }
        let section_y = y.div_euclid(16);
        let index = (section_y - MIN_SECTION_Y / 16) as usize;
        if index >= self.sections.len() {
            return None;
        }
        let section = self.sections.get(index)?;
        let local_y = y.rem_euclid(16) as usize;
        let cell = (x as usize) | ((z as usize) << 4) | (local_y << 8);
        section.get(cell).copied()
    }

    /// Highest block of each column, indexed `x | z << 4`.
    pub fn height_at(&self, x: usize, z: usize) -> i16 {
        self.heightmap[x | (z << 4)]
    }
}

fn read_u32(buf: &[u8], pos: usize) -> Option<u32> {
    buf.get(pos..pos + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Decodes a `Data3D` value. Returns `None` when the record is malformed.
pub fn decode_data3d(value: &[u8]) -> Option<Data3d> {
    if value.len() < HEIGHTMAP_BYTES {
        return None;
    }
    let mut heightmap = [0i16; 256];
    for (i, slot) in heightmap.iter_mut().enumerate() {
        let lo = value[i * 2] as u16;
        let hi = value[i * 2 + 1] as u16;
        *slot = (lo | (hi << 8)) as i16;
    }

    let mut pos = HEIGHTMAP_BYTES;
    let mut sections: Vec<Vec<u32>> = Vec::with_capacity(SECTION_COUNT);
    let mut stored: Vec<bool> = Vec::with_capacity(SECTION_COUNT);
    while sections.len() < SECTION_COUNT {
        let header = *value.get(pos)?;
        pos += 1;
        let bits = header >> 1;
        if bits == 0x7f {
            // "Same as the previous section": BDS omits sections that repeat
            // the last stored one, which is how the biome above the terrain
            // surface is encoded.
            let previous = sections.last().cloned().unwrap_or_else(|| vec![0; SECTION_CELLS]);
            sections.push(previous);
            stored.push(false);
            continue;
        }
        if bits == 0 {
            let value = read_u32(value, pos)?;
            pos += 4;
            sections.push(vec![value; SECTION_CELLS]);
            stored.push(true);
            continue;
        }
        let per_word = 32 / bits as usize;
        if per_word == 0 {
            return None;
        }
        let words = SECTION_CELLS.div_ceil(per_word);
        let mask = (1u32 << bits) - 1;
        let mut indices = Vec::with_capacity(SECTION_CELLS);
        for cell in 0..SECTION_CELLS {
            let word = read_u32(value, pos + (cell / per_word) * 4)?;
            let shift = (cell % per_word) * bits as usize;
            indices.push(((word >> shift) & mask) as usize);
        }
        pos += words * 4;
        let count = read_u32(value, pos)? as usize;
        pos += 4;
        if count == 0 || count > SECTION_CELLS {
            return None;
        }
        let mut palette = Vec::with_capacity(count);
        for i in 0..count {
            palette.push(read_u32(value, pos + i * 4)?);
        }
        pos += count * 4;
        let cells = indices
            .into_iter()
            .map(|i| palette.get(i).copied().unwrap_or(0))
            .collect();
        sections.push(cells);
        stored.push(true);
    }

    Some(Data3d {
        heightmap,
        sections,
        stored,
    })
}

/// A chunk key, decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkKey {
    pub x: i32,
    pub z: i32,
    pub dimension: i32,
    pub tag: u8,
    pub sub_chunk: Option<i8>,
}

/// Parses the LevelDB key of a chunk record.
///
/// Overworld chunk keys are `x i32 | z i32 | tag u8` (9 bytes), sub-chunk
/// records append the sub-chunk Y index (10 bytes). Records of other
/// dimensions insert the dimension id before the tag.
pub fn parse_chunk_key(key: &[u8]) -> Option<ChunkKey> {
    let (x, z, dimension, tag_at, rest) = match key.len() {
        9 | 10 => (read_u32(key, 0)?, read_u32(key, 4)?, 0, 8, key.len() - 8),
        13 | 14 => (
            read_u32(key, 0)?,
            read_u32(key, 4)?,
            read_u32(key, 8)? as i32,
            12,
            key.len() - 12,
        ),
        _ => return None,
    };
    let tag = *key.get(tag_at)?;
    let sub_chunk = if rest == 2 {
        Some(*key.get(tag_at + 1)? as i8)
    } else {
        None
    };
    Some(ChunkKey {
        x: x as i32,
        z: z as i32,
        dimension,
        tag,
        sub_chunk,
    })
}

/// Builds the LevelDB key for a `Data3D` record of the Overworld.
pub fn data3d_key(x: i32, z: i32) -> Vec<u8> {
    let mut key = Vec::with_capacity(9);
    key.extend_from_slice(&x.to_le_bytes());
    key.extend_from_slice(&z.to_le_bytes());
    key.push(tag::DATA_3D);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encodes a section exactly as BDS does, to exercise the decoder.
    fn single_value_section(biome: u32, out: &mut Vec<u8>) {
        out.push(0x01);
        out.extend_from_slice(&biome.to_le_bytes());
    }

    #[test]
    fn decodes_single_value_and_inherited_sections() {
        let mut value = vec![0u8; HEIGHTMAP_BYTES];
        for section in 0..SECTION_COUNT {
            if section < 3 {
                single_value_section(1, &mut value);
            } else {
                value.push(0xff);
            }
        }
        let data = decode_data3d(&value).expect("decodes");
        assert_eq!(data.sections.len(), SECTION_COUNT);
        assert_eq!(data.sections[0][0], 1);
        assert!(!data.stored[3]);
        assert_eq!(data.biome_at(0, -64, 0), Some(1));
        // Section 2 (y=16..31) is the last stored one: everything above
        // inherits it.
        assert_eq!(data.biome_at(0, 100, 0), Some(1));
    }

    #[test]
    fn decodes_paletted_section_lsb_first() {
        // bits = 2 (header 0x05), 4096 cells -> 256 words, 3-entry palette.
        let mut value = vec![0u8; HEIGHTMAP_BYTES];
        value.push(0x05);
        let mut words = vec![0u32; 256];
        // cell 0 -> index 1, cell 1 -> index 2, cell 2 -> index 0
        words[0] = 0b00_10_01;
        for word in &words {
            value.extend_from_slice(&word.to_le_bytes());
        }
        value.extend_from_slice(&3u32.to_le_bytes());
        for biome in [7u32, 8, 9] {
            value.extend_from_slice(&biome.to_le_bytes());
        }
        for _ in 1..SECTION_COUNT {
            value.push(0xff);
        }
        let data = decode_data3d(&value).expect("decodes");
        let cells = &data.sections[0];
        assert_eq!(cells[0], 8);
        assert_eq!(cells[1], 9);
        assert_eq!(cells[2], 7);
    }

    #[test]
    fn chunk_keys_round_trip() {
        let key = data3d_key(-2, 5);
        let parsed = parse_chunk_key(&key).expect("parses");
        assert_eq!((parsed.x, parsed.z, parsed.tag), (-2, 5, tag::DATA_3D));
        assert_eq!(parsed.sub_chunk, None);
    }
}
