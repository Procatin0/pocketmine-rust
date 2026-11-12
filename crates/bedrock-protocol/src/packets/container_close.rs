//! Packet ID 0x2F (47): ContainerClosePacket.
//! Closes a dynamic container. Layout: window_id u8, server_initiated bool.

use crate::binary::{CodecError, Reader, Writer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContainerClosePacket {
    pub window_id: u8,
    pub server_initiated: bool,
}

impl ContainerClosePacket {
    pub const PACKET_ID: u32 = 0x2F;

    pub fn new(window_id: u8, server_initiated: bool) -> Self {
        Self { window_id, server_initiated }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_byte(self.window_id);
        writer.write_bool(self.server_initiated);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            window_id: reader.read_byte()?,
            server_initiated: reader.read_bool()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips() {
        for initiated in [false, true] {
            let packet = ContainerClosePacket::new(10, initiated);
            let mut writer = Writer::new();
            packet.encode_payload(&mut writer);
            let bytes = writer.into_bytes();
            let mut reader = Reader::new(&bytes);
            assert_eq!(ContainerClosePacket::decode_payload(&mut reader).unwrap(), packet);
            assert_eq!(reader.remaining(), 0);
        }
    }
}
