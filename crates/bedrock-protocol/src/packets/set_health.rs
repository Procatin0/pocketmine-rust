use crate::binary::{CodecError, Reader, Writer};

/// Packet ID 0x2A (42): SetHealthPacket, carrying the player's health as a
/// signed varint (1 health point = 2 units in the client HUD).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SetHealthPacket {
    pub health: i32,
}

impl SetHealthPacket {
    pub const PACKET_ID: u32 = 0x2A;

    pub fn new(health: i32) -> Self {
        Self { health }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_i32(self.health);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            health: reader.read_var_i32()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        for health in [0, 20, 7] {
            let mut writer = Writer::new();
            SetHealthPacket::new(health).encode_payload(&mut writer);
            let bytes = writer.into_bytes();
            let mut reader = Reader::new(&bytes);
            assert_eq!(
                SetHealthPacket::decode_payload(&mut reader).unwrap().health,
                health
            );
            assert_eq!(reader.remaining(), 0);
        }
    }

    #[test]
    fn death_sends_zero_in_one_byte() {
        let mut writer = Writer::new();
        SetHealthPacket::new(0).encode_payload(&mut writer);
        assert_eq!(&writer.into_bytes()[..], &[0x00]);
    }
}
