use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x45 (69): RequestChunkRadiusPacket
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestChunkRadiusPacket {
    pub radius: i32,
    pub max_radius: u8,
}

impl RequestChunkRadiusPacket {
    pub fn new(radius: i32, max_radius: u8) -> Self {
        Self { radius, max_radius }
    }
}

impl Packet for RequestChunkRadiusPacket {
    fn packet_id(&self) -> u32 {
        0x45
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_i32(self.radius);
        writer.write_byte(self.max_radius);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        // Reference `RequestChunkRadiusPacket::decodePayload` always reads both
        // fields: `radius` (VarInt signed) then `maxRadius` (unsigned byte).
        Ok(Self {
            radius: reader.read_var_i32()?,
            max_radius: reader.read_byte()?,
        })
    }
}

/// Packet ID 0x46 (70): ChunkRadiusUpdatedPacket
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkRadiusUpdatedPacket {
    pub radius: i32,
}

impl ChunkRadiusUpdatedPacket {
    pub fn new(radius: i32) -> Self {
        Self { radius }
    }
}

impl Packet for ChunkRadiusUpdatedPacket {
    fn packet_id(&self) -> u32 {
        0x46
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_i32(self.radius);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            radius: reader.read_var_i32()?,
        })
    }
}
