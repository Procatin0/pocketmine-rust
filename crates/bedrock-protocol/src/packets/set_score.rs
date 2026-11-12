use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// What a [`ScorePacketEntry`] does on the client
/// (`ScorePacketEntryAction`).
///
/// The ordinal is the declaration order in the PHP enum
/// (`REMOVE`=0, `CHANGE_PLAYER`=1, `CHANGE_ENTITY`=2,
/// `CHANGE_FAKE_PLAYER`=3) and the value is the inner string the packet
/// carries next to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScorePacketEntryAction {
    Remove,
    ChangePlayer,
    ChangeEntity,
    ChangeFakePlayer,
}

impl ScorePacketEntryAction {
    /// `ScorePacketEntryAction::toOrdinal()`: declaration index.
    pub fn ordinal(self) -> u32 {
        match self {
            Self::Remove => 0,
            Self::ChangePlayer => 1,
            Self::ChangeEntity => 2,
            Self::ChangeFakePlayer => 3,
        }
    }

    /// `ScorePacketEntryAction::fromOrdinal()`.
    pub fn from_ordinal(ordinal: u32) -> Option<Self> {
        match ordinal {
            0 => Some(Self::Remove),
            1 => Some(Self::ChangePlayer),
            2 => Some(Self::ChangeEntity),
            3 => Some(Self::ChangeFakePlayer),
            _ => None,
        }
    }

    /// The backed enum value sent as the entry's inner string.
    pub fn value(self) -> &'static str {
        match self {
            Self::Remove => "remove",
            Self::ChangePlayer => "changeplayer",
            Self::ChangeEntity => "changeentity",
            Self::ChangeFakePlayer => "changefakeplayer",
        }
    }

    /// `ScorePacketEntryAction::fromPacket()`.
    pub fn from_packet(value: &str) -> Option<Self> {
        match value {
            "remove" => Some(Self::Remove),
            "changeplayer" => Some(Self::ChangePlayer),
            "changeentity" => Some(Self::ChangeEntity),
            "changefakeplayer" => Some(Self::ChangeFakePlayer),
            _ => None,
        }
    }
}

/// One scoreboard line as sent on the wire (`ScorePacketEntry`).
///
/// `scoreboard_id` is the `i64` row key shared with
/// `SetScoreboardIdentityPacket`; `score` sorts the rows on the client.
/// Which of `actor_unique_id` / `custom_name` is meaningful depends on
/// `action`: `actor_unique_id` for player/entity rows, `custom_name`
/// (the fake-player name shown on the client) for fake-player rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorePacketEntry {
    pub scoreboard_id: i64,
    /// Optional for [`ScorePacketEntryAction::Remove`].
    pub objective_name: Option<String>,
    pub score: i32,
    pub action: ScorePacketEntryAction,
    /// Set for player/entity actions.
    pub actor_unique_id: Option<i64>,
    /// Set for the fake-player action.
    pub custom_name: Option<String>,
}

impl ScorePacketEntry {
    pub fn remove(
        scoreboard_id: i64,
        objective_name: Option<impl Into<String>>,
    ) -> Self {
        Self {
            scoreboard_id,
            objective_name: objective_name.map(Into::into),
            score: 0,
            action: ScorePacketEntryAction::Remove,
            actor_unique_id: None,
            custom_name: None,
        }
    }

    pub fn change_player(
        scoreboard_id: i64,
        objective_name: impl Into<String>,
        score: i32,
        actor_unique_id: i64,
    ) -> Self {
        Self {
            scoreboard_id,
            objective_name: Some(objective_name.into()),
            score,
            action: ScorePacketEntryAction::ChangePlayer,
            actor_unique_id: Some(actor_unique_id),
            custom_name: None,
        }
    }

    pub fn change_entity(
        scoreboard_id: i64,
        objective_name: impl Into<String>,
        score: i32,
        actor_unique_id: i64,
    ) -> Self {
        Self {
            scoreboard_id,
            objective_name: Some(objective_name.into()),
            score,
            action: ScorePacketEntryAction::ChangeEntity,
            actor_unique_id: Some(actor_unique_id),
            custom_name: None,
        }
    }

    pub fn change_fake_player(
        scoreboard_id: i64,
        objective_name: impl Into<String>,
        score: i32,
        custom_name: impl Into<String>,
    ) -> Self {
        Self {
            scoreboard_id,
            objective_name: Some(objective_name.into()),
            score,
            action: ScorePacketEntryAction::ChangeFakePlayer,
            actor_unique_id: None,
            custom_name: Some(custom_name.into()),
        }
    }

    pub fn is_remove(&self) -> bool {
        self.action == ScorePacketEntryAction::Remove
    }

    fn encode(&self, writer: &mut Writer) {
        writer.write_var_u32(self.action.ordinal());
        writer.write_string(self.action.value());
        writer.write_var_i64(self.scoreboard_id);
        match self.action {
            ScorePacketEntryAction::Remove => {
                // CommonTypes::writeOptional(.., putString).
                match &self.objective_name {
                    Some(objective_name) => {
                        writer.write_bool(true);
                        writer.write_string(objective_name);
                    }
                    None => writer.write_bool(false),
                }
            }
            ScorePacketEntryAction::ChangePlayer | ScorePacketEntryAction::ChangeEntity => {
                writer.write_string(self.objective_name.as_deref().unwrap_or_default());
                writer.write_i32_le(self.score);
                writer.write_var_i64(self.actor_unique_id.unwrap_or(0));
            }
            ScorePacketEntryAction::ChangeFakePlayer => {
                writer.write_string(self.objective_name.as_deref().unwrap_or_default());
                writer.write_i32_le(self.score);
                writer.write_string(self.custom_name.as_deref().unwrap_or_default());
            }
        }
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let action = ScorePacketEntryAction::from_ordinal(reader.read_var_u32()?)
            .ok_or_else(|| {
                CodecError::Custom(format!(
                    "Invalid ordinal value for ScorePacketEntryAction"
                ))
            })?;
        let inner_type = reader.read_string()?;
        if ScorePacketEntryAction::from_packet(&inner_type) != Some(action) {
            return Err(CodecError::Custom(format!(
                "Expected inner type {} for score packet entry ordinal {}, got {}",
                action.value(),
                action.ordinal(),
                inner_type
            )));
        }
        let scoreboard_id = reader.read_var_i64()?;
        let mut entry = Self {
            scoreboard_id,
            objective_name: None,
            score: 0,
            action,
            actor_unique_id: None,
            custom_name: None,
        };
        match action {
            ScorePacketEntryAction::Remove => {
                if reader.read_bool()? {
                    entry.objective_name = Some(reader.read_string()?);
                }
            }
            ScorePacketEntryAction::ChangePlayer | ScorePacketEntryAction::ChangeEntity => {
                entry.objective_name = Some(reader.read_string()?);
                entry.score = reader.read_i32_le()?;
                entry.actor_unique_id = Some(reader.read_var_i64()?);
            }
            ScorePacketEntryAction::ChangeFakePlayer => {
                entry.objective_name = Some(reader.read_string()?);
                entry.score = reader.read_i32_le()?;
                entry.custom_name = Some(reader.read_string()?);
            }
        }
        Ok(entry)
    }
}

/// Packet ID 0x6C (108): SetScorePacket
///
/// Sets or removes scoreboard rows.
///
/// Wire order (pmmp/BedrockProtocol `SetScorePacket`): entry count
/// unsigned varint, then per entry the action ordinal unsigned varint,
/// the action's inner string, `scoreboardId` signed varlong, and per
/// action: `remove` optional objective string; player/entity objective
/// string + score LE i32 + actor unique id; fake-player objective
/// string + score LE i32 + custom name string.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SetScorePacket {
    pub entries: Vec<ScorePacketEntry>,
}

impl SetScorePacket {
    pub fn new(entries: Vec<ScorePacketEntry>) -> Self {
        Self { entries }
    }

    pub fn change_fake_player_one(
        scoreboard_id: i64,
        objective_name: impl Into<String>,
        score: i32,
        custom_name: impl Into<String>,
    ) -> Self {
        Self::new(vec![ScorePacketEntry::change_fake_player(
            scoreboard_id,
            objective_name,
            score,
            custom_name,
        )])
    }

    pub fn remove_one(scoreboard_id: i64, objective_name: Option<impl Into<String>>) -> Self {
        Self::new(vec![ScorePacketEntry::remove(
            scoreboard_id,
            objective_name,
        )])
    }
}

impl Packet for SetScorePacket {
    fn packet_id(&self) -> u32 {
        0x6C
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_var_u32(self.entries.len() as u32);
        for entry in &self.entries {
            entry.encode(writer);
        }
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let count = reader.read_var_u32()? as usize;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(ScorePacketEntry::decode(reader)?);
        }
        Ok(Self { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_id_is_0x6c_per_protocol_info() {
        assert_eq!(SetScorePacket::default().packet_id(), 108);
    }

    #[test]
    fn action_ordinals_and_inner_strings_match_php_enum() {
        assert_eq!(ScorePacketEntryAction::Remove.ordinal(), 0);
        assert_eq!(ScorePacketEntryAction::ChangePlayer.ordinal(), 1);
        assert_eq!(ScorePacketEntryAction::ChangeEntity.ordinal(), 2);
        assert_eq!(ScorePacketEntryAction::ChangeFakePlayer.ordinal(), 3);
        assert_eq!(
            ScorePacketEntryAction::from_packet("changefakeplayer"),
            Some(ScorePacketEntryAction::ChangeFakePlayer)
        );
        assert_eq!(ScorePacketEntryAction::from_ordinal(4), None);
    }

    #[test]
    fn fake_player_entry_encodes_to_hand_computed_bytes() {
        // count 1, ordinal 3, "changefakeplayer" (16), scoreboardId 1 (2),
        // "main", score 100 LE, custom "Oro".
        let packet = SetScorePacket::change_fake_player_one(1, "main", 100, "Oro");
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        assert_eq!(bytes[0], 1, "unsigned varint entry count");
        assert_eq!(bytes[1], 3, "action ordinal before inner string");
        assert_eq!(bytes[2], 16, "inner string length");
        assert_eq!(&bytes[3..19], b"changefakeplayer");
        let mut reader = Reader::new(&bytes);
        let decoded = SetScorePacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
        assert_eq!(decoded.entries[0].score, 100);
    }

    #[test]
    fn roundtrips_remove_and_change_player_entries() {
        let packet = SetScorePacket::new(vec![
            ScorePacketEntry::remove(7, None::<String>),
            ScorePacketEntry::change_player(8, "main", -3, 42),
        ]);
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = SetScorePacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
        assert!(decoded.entries[0].is_remove());
        assert_eq!(decoded.entries[1].score, -3);
        assert_eq!(decoded.entries[1].actor_unique_id, Some(42));
    }

    #[test]
    fn decode_rejects_inner_string_mismatching_the_ordinal() {
        // count 1, ordinal 0 ("remove"), but inner string "changefakeplayer".
        let mut bytes = vec![1u8, 0, 16];
        bytes.extend_from_slice(b"changefakeplayer");
        bytes.extend_from_slice(&[2, 0]); // scoreboardId 1, no objective
        let mut reader = Reader::new(&bytes);
        assert!(SetScorePacket::decode(&mut reader).is_err());
    }

    #[test]
    fn empty_packet_is_a_single_zero_count() {
        let packet = SetScorePacket::default();
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        assert_eq!(&writer.into_bytes()[..], &[0]);
    }
}
