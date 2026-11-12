use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// One scoreboard row binding (`ScoreboardIdentityPacketEntry`).
///
/// `scoreboard_id` is the same `i64` row key handed to `SetScorePacket`
/// as `scoreboardId`; `actor_unique_id` names a real entity when the row
/// shows one, or `None` for a fake-player row (there is no actor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreboardIdentityPacketEntry {
    pub scoreboard_id: i64,
    pub actor_unique_id: Option<i64>,
}

impl ScoreboardIdentityPacketEntry {
    pub fn new(scoreboard_id: i64, actor_unique_id: Option<i64>) -> Self {
        Self {
            scoreboard_id,
            actor_unique_id,
        }
    }

    /// A fake-player row: no owning actor.
    pub fn fake_player(scoreboard_id: i64) -> Self {
        Self::new(scoreboard_id, None)
    }

    fn encode(&self, writer: &mut Writer) {
        writer.write_var_i64(self.scoreboard_id);
        // CommonTypes::writeOptional(.., putActorUniqueId): a bool
        // presence flag, then the signed varlong when present.
        match self.actor_unique_id {
            Some(actor_unique_id) => {
                writer.write_bool(true);
                writer.write_var_i64(actor_unique_id);
            }
            None => writer.write_bool(false),
        }
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let scoreboard_id = reader.read_var_i64()?;
        let actor_unique_id = if reader.read_bool()? {
            Some(reader.read_var_i64()?)
        } else {
            None
        };
        Ok(Self {
            scoreboard_id,
            actor_unique_id,
        })
    }
}

/// Packet ID 0x70 (112): SetScoreboardIdentityPacket
///
/// Registers (or clears) the binding between scoreboard rows and the
/// actors/fake players behind them.
///
/// Wire order (pmmp/BedrockProtocol `SetScoreboardIdentityPacket`):
/// `type` unsigned byte (0 register, 1 clear), entry count unsigned
/// varint, then per entry `scoreboardId` signed varlong and
/// `actorUniqueId` optional signed varlong (bool flag + value).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SetScoreboardIdentityPacket {
    pub packet_type: u8,
    pub entries: Vec<ScoreboardIdentityPacketEntry>,
}

impl SetScoreboardIdentityPacket {
    /// `TYPE_REGISTER_IDENTITY`: add these rows to the client.
    pub const TYPE_REGISTER_IDENTITY: u8 = 0;
    /// `TYPE_CLEAR_IDENTITY`: drop these rows from the client.
    pub const TYPE_CLEAR_IDENTITY: u8 = 1;

    pub fn new(packet_type: u8, entries: Vec<ScoreboardIdentityPacketEntry>) -> Self {
        Self {
            packet_type,
            entries,
        }
    }

    pub fn register(entries: Vec<ScoreboardIdentityPacketEntry>) -> Self {
        Self::new(Self::TYPE_REGISTER_IDENTITY, entries)
    }

    pub fn clear(entries: Vec<ScoreboardIdentityPacketEntry>) -> Self {
        Self::new(Self::TYPE_CLEAR_IDENTITY, entries)
    }
}

impl Packet for SetScoreboardIdentityPacket {
    fn packet_id(&self) -> u32 {
        0x70
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_byte(self.packet_type);
        writer.write_var_u32(self.entries.len() as u32);
        for entry in &self.entries {
            entry.encode(writer);
        }
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let packet_type = reader.read_byte()?;
        let count = reader.read_var_u32()? as usize;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(ScoreboardIdentityPacketEntry::decode(reader)?);
        }
        Ok(Self {
            packet_type,
            entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_id_is_0x70_per_protocol_info() {
        assert_eq!(SetScoreboardIdentityPacket::default().packet_id(), 112);
    }

    #[test]
    fn fake_player_entry_encodes_to_hand_computed_bytes() {
        // type 0, count 1, scoreboardId 1 (zigzag varlong 2), no actor (bool 0).
        let packet = SetScoreboardIdentityPacket::register(vec![
            ScoreboardIdentityPacketEntry::fake_player(1),
        ]);
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        assert_eq!(&writer.into_bytes()[..], &[0, 1, 2, 0]);
    }

    #[test]
    fn actor_entry_with_negative_ids_roundtrips() {
        // type 0, count 1, scoreboardId 1 (2), actor present (1),
        // actorUniqueId -2 (zigzag 3).
        let packet = SetScoreboardIdentityPacket::register(vec![
            ScoreboardIdentityPacketEntry::new(1, Some(-2)),
        ]);
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        assert_eq!(&bytes[..], &[0, 1, 2, 1, 3]);
        let mut reader = Reader::new(&bytes);
        let decoded = SetScoreboardIdentityPacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn clear_with_multiple_entries_roundtrips() {
        let packet = SetScoreboardIdentityPacket::clear(vec![
            ScoreboardIdentityPacketEntry::fake_player(-1),
            ScoreboardIdentityPacketEntry::new(1_234_567_890, Some(42)),
        ]);
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = SetScoreboardIdentityPacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(decoded.packet_type, 1);
        assert_eq!(reader.remaining(), 0);
    }
}
