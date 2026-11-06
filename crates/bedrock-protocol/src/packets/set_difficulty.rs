use crate::binary::{CodecError, Reader, Writer};

/// Packet ID 0x3C (60): SetDifficultyPacket.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SetDifficultyPacket {
    pub difficulty: u32,
}

impl SetDifficultyPacket {
    pub const PACKET_ID: u32 = 0x3C;

    pub const PEACEFUL: u32 = 0;
    pub const EASY: u32 = 1;
    pub const NORMAL: u32 = 2;
    pub const HARD: u32 = 3;

    pub fn new(difficulty: u32) -> Self {
        Self { difficulty }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_u32(self.difficulty);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            difficulty: reader.read_var_u32()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_bds_capture() {
        // data/bds-capture/set_difficulty/1.bin: id 0x3C, difficulty 1.
        let bytes: &[u8] = include_bytes!("../../../../data/bds-capture/set_difficulty/1.bin");
        let mut reader = Reader::new(bytes);
        assert_eq!(
            reader.read_var_u32().expect("packet id"),
            SetDifficultyPacket::PACKET_ID
        );
        let packet = SetDifficultyPacket::decode_payload(&mut reader).expect("decode");
        assert_eq!(packet.difficulty, SetDifficultyPacket::EASY);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn roundtrips_every_difficulty() {
        for difficulty in [
            SetDifficultyPacket::PEACEFUL,
            SetDifficultyPacket::EASY,
            SetDifficultyPacket::NORMAL,
            SetDifficultyPacket::HARD,
        ] {
            let packet = SetDifficultyPacket::new(difficulty);
            let mut writer = Writer::new();
            packet.encode_payload(&mut writer);
            let bytes = writer.into_bytes();
            let mut reader = Reader::new(&bytes);
            assert_eq!(
                SetDifficultyPacket::decode_payload(&mut reader).expect("decode"),
                packet
            );
            assert_eq!(reader.remaining(), 0);
        }
    }
}
