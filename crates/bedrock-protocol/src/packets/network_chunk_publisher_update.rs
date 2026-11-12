use crate::binary::{CodecError, Reader, Writer};
use crate::types::BlockPos;
use crate::Packet;

/// Packet ID 0x79 (121): NetworkChunkPublisherUpdatePacket
///
/// Announces to the client the center point and radius of chunk publishing.
/// Radius is expressed in blocks (view_distance * 16).
///
/// Reference: `NetworkChunkPublisherUpdatePacket.php`:
/// `BlockPosition $blockPosition`, `int $radius` (`VarInt::writeUnsignedInt`),
/// `ChunkPosition[] $savedChunks` (`LE::writeUnsignedInt` count, each
/// `ChunkPosition::write` = `var_i32` x + `var_i32` z — NOT a BlockPosition).
/// Reference (`NetworkChunkPublisherUpdatePacket.php`):
/// `public const MAX_SAVED_CHUNKS = 9216;` — `decodePayload` throws
/// `PacketDecodeException` when the LE count exceeds it.
pub const MAX_SAVED_CHUNKS: usize = 9216;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkChunkPublisherUpdatePacket {
    pub position: BlockPos,
    pub radius: u32,
    /// Saved chunks as `(chunk_x, chunk_z)` pairs, matching the reference
    /// `ChunkPosition[]` encoding (two `var_i32` per entry).
    pub saved_chunks: Vec<(i32, i32)>,
}

impl NetworkChunkPublisherUpdatePacket {
    pub fn new(position: BlockPos, radius: u32) -> Self {
        Self {
            position,
            radius,
            saved_chunks: Vec::new(),
        }
    }
}

impl Packet for NetworkChunkPublisherUpdatePacket {
    fn packet_id(&self) -> u32 {
        0x79
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        self.position.write(writer);
        writer.write_var_u32(self.radius);
        writer.write_u32_le(self.saved_chunks.len() as u32);
        for &(chunk_x, chunk_z) in &self.saved_chunks {
            writer.write_var_i32(chunk_x);
            writer.write_var_i32(chunk_z);
        }
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let position = BlockPos::read(reader)?;
        let radius = reader.read_var_u32()?;
        let count = reader.read_u32_le()? as usize;
        if count > MAX_SAVED_CHUNKS {
            return Err(CodecError::Custom(format!(
                "Expected at most {MAX_SAVED_CHUNKS} saved chunks, got {count}"
            )));
        }
        let mut saved_chunks = Vec::with_capacity(count);
        for _ in 0..count {
            saved_chunks.push((reader.read_var_i32()?, reader.read_var_i32()?));
        }
        Ok(Self {
            position,
            radius,
            saved_chunks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_with_saved_chunks_uses_chunk_position() {
        let mut packet = NetworkChunkPublisherUpdatePacket::new(BlockPos::new(1, 64, 2), 128);
        packet.saved_chunks.push((3, -4));
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded =
            NetworkChunkPublisherUpdatePacket::decode(&mut reader).expect("decode");
        assert_eq!(reader.remaining(), 0);
        assert_eq!(packet, decoded);
        assert_eq!(decoded.saved_chunks, vec![(3, -4)]);
    }
}
