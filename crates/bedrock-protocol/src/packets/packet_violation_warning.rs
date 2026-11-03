use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x9C (156): PacketViolationWarningPacket
///
/// Sent by client or server when an invalid, unexpected, or malformed packet is received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketViolationWarningPacket {
    pub violation_type: i32,
    pub severity: i32,
    pub packet_id: i32,
    pub message: String,
}

impl PacketViolationWarningPacket {
    pub const TYPE_MALFORMED: i32 = 0;

    pub const SEVERITY_WARNING: i32 = 0;
    pub const SEVERITY_FINAL_WARNING: i32 = 1;
    pub const SEVERITY_TERMINATING_CONNECTION: i32 = 2;
}

impl Packet for PacketViolationWarningPacket {
    fn packet_id(&self) -> u32 {
        0x9C
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_i32(self.violation_type);
        writer.write_var_i32(self.severity);
        writer.write_var_i32(self.packet_id);
        writer.write_string(&self.message);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let violation_type = reader.read_var_i32()?;
        let severity = reader.read_var_i32()?;
        let packet_id = reader.read_var_i32()?;
        let message = reader.read_string()?;
        Ok(Self {
            violation_type,
            severity,
            packet_id,
            message,
        })
    }
}
