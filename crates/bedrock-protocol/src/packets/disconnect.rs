use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x05: DisconnectPacket
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisconnectPacket {
    pub reason: i32,
    pub skip_message: bool,
    pub message: Option<String>,
}

impl Packet for DisconnectPacket {
    fn packet_id(&self) -> u32 {
        0x05
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_i32(self.reason);
        writer.write_bool(self.skip_message);
        if !self.skip_message {
            writer.write_string(self.message.as_deref().unwrap_or(""));
        }
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let reason = reader.read_var_i32()?;
        let skip_message = reader.read_bool()?;
        let message = if !skip_message {
            Some(reader.read_string()?)
        } else {
            None
        };
        Ok(Self {
            reason,
            skip_message,
            message,
        })
    }
}
