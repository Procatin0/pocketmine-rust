use crate::binary::{CodecError, Reader, Writer};
use crate::types::BlockPos;
use crate::Packet;

/// Packet ID 0x79 (121): NetworkChunkPublisherUpdatePacket
///
/// Announces to the client the center point and radius of chunk publishing.
/// Radius is expressed in blocks (view_distance * 16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkChunkPublisherUpdatePacket {
    pub position: BlockPos,
    pub radius: u32,
    pub saved_chunks: Vec<BlockPos>,
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
        for chunk in &self.saved_chunks {
            chunk.write(writer);
        }
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let position = BlockPos::read(reader)?;
        let radius = reader.read_var_u32()?;
        let count = reader.read_u32_le()? as usize;
        let mut saved_chunks = Vec::with_capacity(count);
        for _ in 0..count {
            saved_chunks.push(BlockPos::read(reader)?);
        }
        Ok(Self {
            position,
            radius,
            saved_chunks,
        })
    }
}
