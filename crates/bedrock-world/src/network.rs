//! Bedrock SubChunk network serialization and deserialization.
//!
//! Minecraft Bedrock SubChunk network format (version 8 / 9):
//! - version: u8 (version 8 is standard for network chunk serialization)
//! - storage_layers_count: u8 (number of block layers, typically 1 or 2 for waterlogging)
//! - For each layer:
//!   - Palette header: u8 = (bits_per_block << 1) | is_runtime (where is_runtime = 1)
//!   - words: &[u32] in little-endian (0 words if bits_per_block == 0)
//!   - palette_size: i32 (VarInt ZigZag)
//!   - palette entries: [i32; palette_size] block runtime IDs (VarInt ZigZag)

use bedrock_protocol::{Reader, Writer};
use thiserror::Error;

use crate::paletted_block_array::PalettedBlockArray;
use crate::subchunk::SubChunk;

/// Current Bedrock subchunk network version (8 is standard for network chunk serialization)
pub const CURRENT_VERSION: u8 = 8;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SubChunkNetworkError {
    #[error("Buffer/codec error: {0}")]
    Codec(String),
    #[error("Unsupported subchunk version: {0}")]
    UnsupportedVersion(u8),
    #[error("Expected runtime palette header flag (is_runtime = 1)")]
    NonRuntimePalette,
    #[error("Unsupported bits per block: {0}")]
    UnsupportedBitsPerBlock(u8),
    #[error("Invalid palette size: {0}")]
    InvalidPaletteSize(i32),
}

impl From<bedrock_protocol::binary::CodecError> for SubChunkNetworkError {
    fn from(err: bedrock_protocol::binary::CodecError) -> Self {
        Self::Codec(err.to_string())
    }
}

/// Serializes a SubChunk to network format into the provided `Writer`.
pub fn serialize_subchunk(subchunk: &SubChunk, writer: &mut Writer) {
    serialize_subchunk_version(subchunk, writer, CURRENT_VERSION);
}

/// Serializes a SubChunk to network format with an explicit version byte.
pub fn serialize_subchunk_version(subchunk: &SubChunk, writer: &mut Writer, version: u8) {
    writer.write_byte(version);
    writer.write_byte(subchunk.layers.len() as u8);

    for layer in &subchunk.layers {
        let bpb = layer.bits_per_block();
        let header = (bpb << 1) | 1; // is_runtime = 1 for network IDs
        writer.write_byte(header);

        for &word in layer.words() {
            writer.write_u32_le(word);
        }

        let palette = layer.palette();
        writer.write_var_i32(palette.len() as i32);
        for &runtime_id in palette {
            writer.write_var_i32(runtime_id as i32);
        }
    }
}

/// Deserializes a SubChunk from network format using the provided `Reader`.
pub fn deserialize_subchunk(reader: &mut Reader) -> Result<SubChunk, SubChunkNetworkError> {
    let version = reader.read_byte()?;
    if version != 8 && version != 9 {
        return Err(SubChunkNetworkError::UnsupportedVersion(version));
    }

    let layers_count = reader.read_byte()? as usize;
    let mut layers = Vec::with_capacity(layers_count);

    for _ in 0..layers_count {
        let header = reader.read_byte()?;
        let is_runtime = (header & 1) != 0;
        if !is_runtime {
            return Err(SubChunkNetworkError::NonRuntimePalette);
        }

        let bits_per_block = header >> 1;
        let num_words = PalettedBlockArray::words_count_for_bpb(bits_per_block)
            .ok_or(SubChunkNetworkError::UnsupportedBitsPerBlock(bits_per_block))?;

        let mut words = Vec::with_capacity(num_words);
        for _ in 0..num_words {
            words.push(reader.read_u32_le()?);
        }

        let palette_size = reader.read_var_i32()?;
        if palette_size <= 0 || palette_size > (PalettedBlockArray::SIZE as i32) {
            return Err(SubChunkNetworkError::InvalidPaletteSize(palette_size));
        }

        let mut palette = Vec::with_capacity(palette_size as usize);
        for _ in 0..palette_size {
            let runtime_id = reader.read_var_i32()?;
            palette.push(runtime_id as u32);
        }

        layers.push(PalettedBlockArray::from_raw(bits_per_block, words, palette));
    }

    Ok(SubChunk { layers })
}
