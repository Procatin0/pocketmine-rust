use crate::binary::{CodecError, Reader, Writer};

/// `ActorEvent::DEATH_ANIMATION`, the event that plays the death animation.
pub const EVENT_DEATH_ANIMATION: u8 = 3;

/// Packet ID 0x1B (27): ActorEventPacket.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ActorEventPacket {
    pub runtime_id: u64,
    pub event_id: u8,
    pub event_data: i32,
    /// Optional fire position, only used by some events.
    pub position: Option<(f32, f32, f32)>,
}

impl ActorEventPacket {
    pub const PACKET_ID: u32 = 0x1B;

    pub fn new(runtime_id: u64, event_id: u8, event_data: i32) -> Self {
        Self {
            runtime_id,
            event_id,
            event_data,
            position: None,
        }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_u64(self.runtime_id);
        writer.write_byte(self.event_id);
        writer.write_var_i32(self.event_data);
        writer.write_bool(self.position.is_some());
        if let Some((x, y, z)) = self.position {
            writer.write_f32_le(x);
            writer.write_f32_le(y);
            writer.write_f32_le(z);
        }
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let runtime_id = reader.read_var_u64()?;
        let event_id = reader.read_byte()?;
        let event_data = reader.read_var_i32()?;
        let position = if reader.read_bool()? {
            Some((
                reader.read_f32_le()?,
                reader.read_f32_le()?,
                reader.read_f32_le()?,
            ))
        } else {
            None
        };
        Ok(Self {
            runtime_id,
            event_id,
            event_data,
            position,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn death_event_roundtrip() {
        let packet = ActorEventPacket::new(42, EVENT_DEATH_ANIMATION, 0);
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();

        let mut reader = Reader::new(&bytes);
        let decoded = ActorEventPacket::decode_payload(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(decoded.event_id, 3);
        assert_eq!(reader.remaining(), 0);
    }
}
