use crate::binary::{CodecError, Reader};

/// Packet ID 0x4D (77): CommandRequestPacket.
///
/// Modern clients send this instead of a `TextPacket` when the player runs a
/// slash command from the chat bar or the command UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRequestPacket {
    pub command: String,
    pub is_internal: bool,
}

impl CommandRequestPacket {
    pub const PACKET_ID: u32 = 0x4D;

    /// Decodes the command line. The trailing origin data is skipped on
    /// purpose: the server already knows which session sent the request, and
    /// tolerating a shorter trailer keeps old clients working.
    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let command = reader.read_string()?;

        // CommandOriginData: string type, 16-byte UUID, string request id and
        // the actor unique id as a little-endian i64.
        if reader.remaining() > 16 + 8 + 1 {
            let _origin_type = reader.read_string().unwrap_or_default();
            let _uuid = reader.read_bytes(16).unwrap_or_default();
            let _request_id = reader.read_string().unwrap_or_default();
            let _actor_unique_id = reader.read_i64_le().unwrap_or(0);
        }

        let is_internal = reader.read_bool().unwrap_or(false);

        Ok(Self {
            command,
            is_internal,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::Writer;

    fn build(command: &str) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_string(command);
        writer.write_string("player"); // origin type
        writer.write_bytes(&[0u8; 16]); // uuid
        writer.write_string(""); // request id
        writer.write_i64_le(0); // actor unique id
        writer.write_bool(false); // internal
        writer.write_string(""); // version
        writer.into_bytes().to_vec()
    }

    #[test]
    fn decodes_the_command_line() {
        let payload = build("/tp Steve 10 70 -5");
        let mut reader = Reader::new(&payload);
        let packet = CommandRequestPacket::decode(&mut reader).expect("decode");
        assert_eq!(packet.command, "/tp Steve 10 70 -5");
        assert!(!packet.is_internal);
    }

    #[test]
    fn tolerates_a_missing_origin_trailer() {
        let mut writer = Writer::new();
        writer.write_string("/time set day");
        let payload = writer.into_bytes();
        let mut reader = Reader::new(&payload);
        let packet = CommandRequestPacket::decode(&mut reader).expect("decode");
        assert_eq!(packet.command, "/time set day");
    }
}
