//! Packet ID 0x2F (47): ContainerClosePacket.
//! Closes a container screen. Layout: window_id u8, container_type u8,
//! server_initiated bool (Mojang `ContainerClosePacket`).
//!
//! The container type byte sits between the id and the flag; without it the
//! client mis-parses the ack (and the server mis-reads the client's close
//! request, taking the type byte for the boolean).

use crate::binary::{CodecError, Reader, Writer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContainerClosePacket {
    pub window_id: u8,
    pub container_type: u8,
    pub server_initiated: bool,
}

impl ContainerClosePacket {
    pub const PACKET_ID: u32 = 0x2F;

    pub fn new(window_id: u8, container_type: u8, server_initiated: bool) -> Self {
        Self { window_id, container_type, server_initiated }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_byte(self.window_id);
        writer.write_byte(self.container_type);
        writer.write_bool(self.server_initiated);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            window_id: reader.read_byte()?,
            container_type: reader.read_byte()?,
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
            let packet = ContainerClosePacket::new(10, 1, initiated);
            let mut writer = Writer::new();
            packet.encode_payload(&mut writer);
            let bytes = writer.into_bytes();
            let mut reader = Reader::new(&bytes);
            assert_eq!(ContainerClosePacket::decode_payload(&mut reader).unwrap(), packet);
            assert_eq!(reader.remaining(), 0);
        }
    }
}
