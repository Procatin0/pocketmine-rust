use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0xC1 (193): RequestNetworkSettingsPacket
pub struct RequestNetworkSettingsPacket {
    pub client_protocol_version: i32,
}

impl Packet for RequestNetworkSettingsPacket {
    fn packet_id(&self) -> u32 {
        0xC1
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_i32_le(self.client_protocol_version);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            client_protocol_version: reader.read_i32_le()?,
        })
    }
}

/// Packet ID 0x8F (143): NetworkSettingsPacket
pub struct NetworkSettingsPacket {
    pub compression_threshold: u16,
    pub compression_algorithm: u16,
    pub client_throttle_threshold: u8,
    pub client_throttle_scalar: f32,
}

impl Packet for NetworkSettingsPacket {
    fn packet_id(&self) -> u32 {
        0x8F
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_u16_le(self.compression_threshold);
        writer.write_u16_le(self.compression_algorithm);
        writer.write_byte(self.client_throttle_threshold);
        writer.write_f32_le(self.client_throttle_scalar);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            compression_threshold: reader.read_u16_le()?,
            compression_algorithm: reader.read_u16_le()?,
            client_throttle_threshold: reader.read_byte()?,
            client_throttle_scalar: reader.read_f32_le()?,
        })
    }
}
