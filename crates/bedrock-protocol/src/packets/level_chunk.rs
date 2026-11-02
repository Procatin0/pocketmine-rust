use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x3A (58): LevelChunkPacket
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelChunkPacket {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub sub_chunk_count: u32,
    pub payload: Vec<u8>,
}

impl LevelChunkPacket {
    pub fn new(chunk_x: i32, chunk_z: i32, sub_chunk_count: u32, payload: Vec<u8>) -> Self {
        Self {
            chunk_x,
            chunk_z,
            sub_chunk_count,
            payload,
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
        writer.write_var_u32(self.sub_chunk_count);
        writer.write_byte_array(&self.payload);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let chunk_x = reader.read_var_i32()?;
        let chunk_z = reader.read_var_i32()?;
        let sub_chunk_count = reader.read_var_u32()?;
        let payload = reader.read_byte_array()?;
        Ok(Self {
            chunk_x,
            chunk_z,
            sub_chunk_count,
            payload,
        })
    }
}
