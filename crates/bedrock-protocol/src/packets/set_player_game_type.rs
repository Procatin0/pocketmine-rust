use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x3E (62): SetPlayerGameTypePacket
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPlayerGameTypePacket {
    pub gamemode: i32,
}

impl SetPlayerGameTypePacket {
    pub fn new(gamemode: i32) -> Self {
        Self { gamemode }
    }
}

impl Packet for SetPlayerGameTypePacket {
    fn packet_id(&self) -> u32 {
        0x3E
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_i32(self.gamemode);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let gamemode = reader.read_var_i32()?;
        Ok(Self { gamemode })
    }
}
