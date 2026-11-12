//! Packet ID 0x2E (46): ContainerOpenPacket.
//! Opens a dynamic container (anvil, crafting table, furnace, ...).
//! Layout: window_id u8, window_name string, position BlockPos (3x var_i32),
//! unique_id var_u32 (dynamic container id).

use crate::binary::{CodecError, Reader, Writer};
use crate::types::BlockPos;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContainerOpenPacket {
    pub window_id: u8,
    pub window_name: String,
    pub position: BlockPos,
    pub unique_id: u32,
}

impl ContainerOpenPacket {
    pub const PACKET_ID: u32 = 0x2E;

    pub fn new(window_id: u8, window_name: impl Into<String>, position: BlockPos, unique_id: u32) -> Self {
        Self { window_id, window_name: window_name.into(), position, unique_id }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_byte(self.window_id);
        writer.write_string(&self.window_name);
        self.position.write(writer);
        writer.write_var_u32(self.unique_id);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            window_id: reader.read_byte()?,
            window_name: reader.read_string()?,
            position: BlockPos::read(reader)?,
            unique_id: reader.read_var_u32()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips() {
        let packet = ContainerOpenPacket::new(10, "minecraft:crafting_table", BlockPos::new(1, 64, -3), 12345);
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(ContainerOpenPacket::decode_payload(&mut reader).unwrap(), packet);
        assert_eq!(reader.remaining(), 0);
    }
}
