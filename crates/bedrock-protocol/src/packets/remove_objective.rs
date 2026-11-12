use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Packet ID 0x6A (106): RemoveObjectivePacket
///
/// Removes a scoreboard objective from the client. Carries only the
/// objective name; every score under it disappears with it.
///
/// Wire order (pmmp/BedrockProtocol `RemoveObjectivePacket`):
/// `objectiveName` string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveObjectivePacket {
    pub objective_name: String,
}

impl RemoveObjectivePacket {
    pub fn new(objective_name: impl Into<String>) -> Self {
        Self {
            objective_name: objective_name.into(),
        }
    }
}

impl Packet for RemoveObjectivePacket {
    fn packet_id(&self) -> u32 {
        0x6A
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_string(&self.objective_name);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            objective_name: reader.read_string()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_id_is_0x6a_per_protocol_info() {
        assert_eq!(RemoveObjectivePacket::new("x").packet_id(), 106);
    }

    #[test]
    fn roundtrips_objective_name() {
        let packet = RemoveObjectivePacket::new("main");
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        // var_u32 len (4) + "main"
        assert_eq!(&bytes[..], &[4, b'm', b'a', b'i', b'n']);
        let mut reader = Reader::new(&bytes);
        let decoded = RemoveObjectivePacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }
}
