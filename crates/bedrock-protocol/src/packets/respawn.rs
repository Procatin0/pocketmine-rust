use crate::binary::{CodecError, Reader, Writer};

/// Packet ID 0x2D (45): RespawnPacket, sent both ways.
///
/// The server sends `SEARCHING_FOR_SPAWN`/`READY_TO_SPAWN`, and the client
/// answers with `CLIENT_READY_TO_SPAWN` once the player accepts the respawn.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RespawnPacket {
    pub position: (f32, f32, f32),
    pub state: u8,
    pub runtime_id: u64,
}

impl RespawnPacket {
    pub const PACKET_ID: u32 = 0x2D;

    pub const SEARCHING_FOR_SPAWN: u8 = 0;
    pub const READY_TO_SPAWN: u8 = 1;
    pub const CLIENT_READY_TO_SPAWN: u8 = 2;

    pub fn new(position: (f32, f32, f32), state: u8, runtime_id: u64) -> Self {
        Self {
            position,
            state,
            runtime_id,
        }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_f32_le(self.position.0);
        writer.write_f32_le(self.position.1);
        writer.write_f32_le(self.position.2);
        writer.write_byte(self.state);
        writer.write_var_u64(self.runtime_id);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            position: (
                reader.read_f32_le()?,
                reader.read_f32_le()?,
                reader.read_f32_le()?,
            ),
            state: reader.read_byte()?,
            runtime_id: reader.read_var_u64()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPTURES: [&[u8]; 3] = [
        include_bytes!("../../../../data/bds-capture/respawn/1.bin"),
        include_bytes!("../../../../data/bds-capture/respawn/2.bin"),
        include_bytes!("../../../../data/bds-capture/respawn/3.bin"),
    ];

    #[test]
    fn decodes_bds_respawn_captures() {
        let states = [0u8, 0, 1];
        for (bytes, expected_state) in CAPTURES.iter().zip(states) {
            let mut reader = Reader::new(bytes);
            assert_eq!(reader.read_var_u32().unwrap(), RespawnPacket::PACKET_ID);
            let packet = RespawnPacket::decode_payload(&mut reader).expect("decode");

            assert_eq!(packet.state, expected_state);
            assert_eq!(packet.runtime_id, 0);
            assert!((packet.position.0 - 37.5).abs() < f32::EPSILON);
            assert!((packet.position.2 - 143.5).abs() < f32::EPSILON);
            assert_eq!(reader.remaining(), 0);
        }
    }

    #[test]
    fn ready_to_spawn_carries_the_real_spawn_height() {
        let mut reader = Reader::new(CAPTURES[2]);
        let _ = reader.read_var_u32().unwrap();
        let packet = RespawnPacket::decode_payload(&mut reader).expect("decode");
        assert!(
            (packet.position.1 - 72.62).abs() < 0.01,
            "unexpected y: {}",
            packet.position.1
        );
    }

    #[test]
    fn roundtrip() {
        let packet = RespawnPacket::new((1.5, 70.0, -3.25), 1, 9);
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();

        let mut reader = Reader::new(&bytes);
        assert_eq!(
            RespawnPacket::decode_payload(&mut reader).expect("decode"),
            packet
        );
        assert_eq!(reader.remaining(), 0);
    }
}
