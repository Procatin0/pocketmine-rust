use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x3A (58): LevelChunkPacket
///
/// Full Bedrock 1.26.60 / Axolotl-PM LevelChunkPacket implementation.
/// Field order mirrors `LevelChunkPacket::encodePayload`: `ChunkPosition`
/// (chunk x/z, var_i32), dimension id (var_i32 signed), subchunk count
/// (var_u32), optional subchunk request limit, cache-enabled flag,
/// `used_blob_hashes` (var_u32 count + LE u64 each), the chunk payload as
/// `CommonTypes::putString` (var_u32 length + raw bytes) and the
/// client-biome-update flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelChunkPacket {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub dimension_id: i32,
    pub sub_chunk_count: u32,
    pub sub_chunk_request_limit: Option<i32>,
    pub cache_enabled: bool,
    pub used_blob_hashes: Vec<u64>,
    pub payload: Vec<u8>,
    pub client_biome_update: bool,
}

impl LevelChunkPacket {
    pub fn new(
        chunk_x: i32,
        chunk_z: i32,
        dimension_id: i32,
        sub_chunk_count: u32,
        sub_chunk_request_limit: Option<i32>,
        cache_enabled: bool,
        used_blob_hashes: Vec<u64>,
        payload: Vec<u8>,
        client_biome_update: bool,
    ) -> Self {
        Self {
            chunk_x,
            chunk_z,
            dimension_id,
            sub_chunk_count,
            sub_chunk_request_limit,
            cache_enabled,
            used_blob_hashes,
            payload,
            client_biome_update,
        }
    }
}

impl Packet for LevelChunkPacket {
    fn packet_id(&self) -> u32 {
        0x3A
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_i32(self.chunk_x);
        writer.write_var_i32(self.chunk_z);
        writer.write_var_i32(self.dimension_id);
        writer.write_var_u32(self.sub_chunk_count);

        match self.sub_chunk_request_limit {
            Some(limit) => {
                writer.write_bool(true);
                writer.write_var_i32(limit);
            }
            None => writer.write_bool(false),
        }

        writer.write_bool(self.cache_enabled);
        writer.write_var_u32(self.used_blob_hashes.len() as u32);
        for &hash in &self.used_blob_hashes {
            writer.write_u64_le(hash);
        }

        writer.write_byte_array(&self.payload);
        writer.write_bool(self.client_biome_update);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let chunk_x = reader.read_var_i32()?;
        let chunk_z = reader.read_var_i32()?;
        let dimension_id = reader.read_var_i32()?;
        let sub_chunk_count = reader.read_var_u32()?;

        let has_limit = reader.read_bool()?;
        let sub_chunk_request_limit = if has_limit {
            Some(reader.read_var_i32()?)
        } else {
            None
        };

        let cache_enabled = reader.read_bool()?;
        // Reference rejects more than MAX_BLOB_HASHES (64) hashes with a
        // PacketDecodeException; mirror that bound instead of allocating
        // an unbounded Vec from client-controlled input.
        let hash_count = reader.read_var_u32()? as usize;
        if hash_count > 64 {
            return Err(CodecError::Custom(format!(
                "Expected at most 64 blob hashes, got {hash_count}"
            )));
        }
        let mut used_blob_hashes = Vec::with_capacity(hash_count);
        for _ in 0..hash_count {
            used_blob_hashes.push(reader.read_u64_le()?);
        }

        let payload = reader.read_byte_array()?.to_vec();
        let client_biome_update = reader.read_bool()?;

        Ok(Self {
            chunk_x,
            chunk_z,
            dimension_id,
            sub_chunk_count,
            sub_chunk_request_limit,
            cache_enabled,
            used_blob_hashes,
            payload,
            client_biome_update,
        })
    }
}
