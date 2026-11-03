use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x01: LoginPacket
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginPacket {
    pub protocol_version: u32,
    pub connection_request: String,
}

impl LoginPacket {
    pub fn new(protocol_version: u32, connection_request: impl Into<String>) -> Self {
        Self {
            protocol_version,
            connection_request: connection_request.into(),
        }
    }
}

impl Packet for LoginPacket {
    fn packet_id(&self) -> u32 {
        0x01
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_u32_be(self.protocol_version);
        writer.write_string(&self.connection_request);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let protocol_version = reader.read_u32_be()?;
        let len = match reader.read_var_u32() {
            Ok(l) => l as usize,
            Err(_) => reader.remaining(),
        };
        let bytes = reader.read_bytes(len.min(reader.remaining()))?;
        let connection_request = String::from_utf8_lossy(bytes).into_owned();
        Ok(Self {
            protocol_version,
            connection_request,
        })
    }
}
