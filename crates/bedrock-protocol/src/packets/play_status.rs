use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x02: PlayStatusPacket
pub struct PlayStatusPacket {
    pub status: i32,
}

impl PlayStatusPacket {
    pub const LOGIN_SUCCESS: i32 = 0;
    pub const LOGIN_FAILED_CLIENT: i32 = 1;
    pub const LOGIN_FAILED_SERVER: i32 = 2;
    pub const PLAYER_SPAWN: i32 = 3;
    pub const LOGIN_FAILED_INVALID_TENANT: i32 = 4;
    pub const LOGIN_FAILED_VANILLA_EDU: i32 = 5;
    pub const LOGIN_FAILED_EDU_VANILLA: i32 = 6;
    pub const LOGIN_FAILED_SERVER_FULL: i32 = 7;
}

impl Packet for PlayStatusPacket {
    fn packet_id(&self) -> u32 {
        0x02
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_i32_le(self.status);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            status: reader.read_i32_le()?,
        })
    }
}
