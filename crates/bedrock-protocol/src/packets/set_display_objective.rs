use crate::binary::{CodecError, Reader, Writer};
use crate::Packet;

/// Scoreboard display-slot strings
/// (`SetDisplayObjectivePacket::DISPLAY_SLOT_*`). The slot travels as a
/// string, not a number.
pub mod display_slot {
    pub const LIST: &'static str = "list";
    pub const SIDEBAR: &'static str = "sidebar";
    pub const BELOW_NAME: &'static str = "belowname";
}

/// Sort order (`SORT_ORDER_*`): scores sorted ascending or descending.
/// On the wire this is a signed varint.
pub mod sort_order {
    pub const ASCENDING: i32 = 0;
    pub const DESCENDING: i32 = 1;
}

/// Packet ID 0x6B (107): SetDisplayObjectivePacket
///
/// Creates (or updates) a scoreboard objective and chooses where the
/// client displays it.
///
/// Wire order (pmmp/BedrockProtocol `SetDisplayObjectivePacket`):
/// `displaySlot` string (`"list"`/`"sidebar"`/`"belowname"`),
/// `objectiveName` string, `displayName` string, `criteriaName`
/// string, `sortOrder` signed varint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetDisplayObjectivePacket {
    pub display_slot: String,
    pub objective_name: String,
    pub display_name: String,
    pub criteria_name: String,
    pub sort_order: i32,
}

impl SetDisplayObjectivePacket {
    /// The only criteria PocketMine ever sends for plugin scoreboards.
    pub const CRITERIA_DUMMY: &'static str = "dummy";

    pub fn new(
        display_slot: impl Into<String>,
        objective_name: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            display_slot: display_slot.into(),
            objective_name: objective_name.into(),
            display_name: display_name.into(),
            criteria_name: Self::CRITERIA_DUMMY.to_string(),
            sort_order: sort_order::DESCENDING,
        }
    }

    pub fn sidebar(objective_name: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self::new(display_slot::SIDEBAR, objective_name, display_name)
    }
}

impl Packet for SetDisplayObjectivePacket {
    fn packet_id(&self) -> u32 {
        0x6B
    }

    fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        writer.write_string(&self.display_slot);
        writer.write_string(&self.objective_name);
        writer.write_string(&self.display_name);
        writer.write_string(&self.criteria_name);
        writer.write_var_i32(self.sort_order);
        Ok(())
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            display_slot: reader.read_string()?,
            objective_name: reader.read_string()?,
            display_name: reader.read_string()?,
            criteria_name: reader.read_string()?,
            sort_order: reader.read_var_i32()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_id_is_0x6b_per_protocol_info() {
        assert_eq!(SetDisplayObjectivePacket::sidebar("o", "t").packet_id(), 107);
    }

    #[test]
    fn defaults_are_dummy_criteria_and_descending() {
        let packet = SetDisplayObjectivePacket::sidebar("main", "Números");
        assert_eq!(packet.display_slot, "sidebar");
        assert_eq!(packet.criteria_name, "dummy");
        assert_eq!(packet.sort_order, sort_order::DESCENDING);
    }

    #[test]
    fn encodes_display_slot_first_then_objective_then_display_name() {
        // slot first: "belowname" len 9, "main", "t", "dummy", sortOrder 0 (zigzag 0).
        let packet = SetDisplayObjectivePacket {
            display_slot: "belowname".to_string(),
            objective_name: "main".to_string(),
            display_name: "t".to_string(),
            criteria_name: "dummy".to_string(),
            sort_order: 0,
        };
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        assert_eq!(&bytes[..2], &[9, b'b']);
        let mut reader = Reader::new(&bytes);
        let decoded = SetDisplayObjectivePacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }
}
