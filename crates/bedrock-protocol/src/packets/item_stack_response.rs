//! Packet ID 0x94 (148): ItemStackResponsePacket.
//!
//! Server answer to `ItemStackRequestPacket` (0x93): one entry per request id
//! with a result code (0 = OK, 1 = error) and, for OK, the container slots
//! the server corrected.
//!
//! Layout (ported from `axolotl-pm/BedrockProtocol`
//! `types/inventory/stackresponse/*`, protocol 2225 / 1.26.60):
//!   responses: var_u32 count, each:
//!     result: u8
//!     request_id: var_i32 (zigzag)
//!     containers: **optional** - a presence bool is always written, and the
//!       list follows only when present:
//!       containers: var_u32 count, each:
//!         container: u8, optional dynamic id (bool + u32 LE)
//!         slots: var_u32 count, each:
//!           slot u8, hotbar_slot u8, count u8,
//!           optional item_stack_id (bool + var_i32),
//!           custom_name string,
//!           optional filtered_custom_name (bool + string),
//!           durability_correction var_i32
//!
//! The presence byte was missing before, so every response ended one byte
//! short and the client kicked the session with a truncated 0x94
//! ("BinaryStream read() incomplete").
//!
//! The server model keeps no per-slot stack ids, so OK responses carry no
//! corrections (the client's prediction already matches) and the mutated
//! slots travel separately as `InventorySlotPacket`. An error result makes
//! the client roll back its prediction, which is what an unsupported action
//! needs.

use crate::binary::{CodecError, Reader, Writer};

pub const STATUS_OK: u8 = 0;
pub const STATUS_ERROR: u8 = 1;

/// One corrected slot inside a container.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemStackResponseSlot {
    /// Slot index inside the container (UI id numbering).
    pub slot: u8,
    pub hotbar_slot: u8,
    pub count: u8,
    pub item_stack_id: Option<i32>,
    pub custom_name: String,
    pub filtered_custom_name: Option<String>,
    pub durability_correction: i32,
}

impl ItemStackResponseSlot {
    pub fn encode(&self, writer: &mut Writer) {
        writer.write_byte(self.slot);
        writer.write_byte(self.hotbar_slot);
        writer.write_byte(self.count);
        writer.write_bool(self.item_stack_id.is_some());
        if let Some(id) = self.item_stack_id {
            writer.write_var_i32(id);
        }
        writer.write_string(&self.custom_name);
        writer.write_bool(self.filtered_custom_name.is_some());
        if let Some(name) = &self.filtered_custom_name {
            writer.write_string(name);
        }
        writer.write_var_i32(self.durability_correction);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let slot = reader.read_byte()?;
        let hotbar_slot = reader.read_byte()?;
        let count = reader.read_byte()?;
        let item_stack_id = if reader.read_bool()? {
            Some(reader.read_var_i32()?)
        } else {
            None
        };
        let custom_name = reader.read_string()?;
        let filtered_custom_name = if reader.read_bool()? {
            Some(reader.read_string()?)
        } else {
            None
        };
        let durability_correction = reader.read_var_i32()?;
        Ok(Self {
            slot,
            hotbar_slot,
            count,
            item_stack_id,
            custom_name,
            filtered_custom_name,
            durability_correction,
        })
    }
}

/// One container's corrected slots.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemStackResponseContainer {
    /// Container UI id (`UI_INVENTORY`, `UI_HOTBAR`, ...).
    pub container: u8,
    pub dynamic_id: Option<u32>,
    pub slots: Vec<ItemStackResponseSlot>,
}

impl ItemStackResponseContainer {
    pub fn encode(&self, writer: &mut Writer) {
        writer.write_byte(self.container);
        writer.write_bool(self.dynamic_id.is_some());
        if let Some(id) = self.dynamic_id {
            writer.write_u32_le(id);
        }
        writer.write_var_u32(self.slots.len() as u32);
        for slot in &self.slots {
            slot.encode(writer);
        }
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let container = reader.read_byte()?;
        let dynamic_id = if reader.read_bool()? {
            Some(reader.read_u32_le()?)
        } else {
            None
        };
        let count = reader.read_var_u32()?;
        if count > 128 {
            return Err(CodecError::Custom(format!(
                "Too many ItemStackResponse slots: {count}"
            )));
        }
        let mut slots = Vec::with_capacity(count as usize);
        for _ in 0..count {
            slots.push(ItemStackResponseSlot::decode(reader)?);
        }
        Ok(Self { container, dynamic_id, slots })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStackResponseEntry {
    pub request_id: i32,
    pub status: u8,
    /// `Some` for an OK response (possibly empty), `None` for an error, whose
    /// optional containers carries no value.
    pub containers: Option<Vec<ItemStackResponseContainer>>,
}

impl ItemStackResponseEntry {
    pub fn ok(request_id: i32) -> Self {
        Self { request_id, status: STATUS_OK, containers: Some(Vec::new()) }
    }

    pub fn error(request_id: i32) -> Self {
        Self { request_id, status: STATUS_ERROR, containers: None }
    }

    pub fn encode(&self, writer: &mut Writer) {
        writer.write_byte(self.status);
        writer.write_var_i32(self.request_id);
        match &self.containers {
            Some(containers) => {
                writer.write_bool(true);
                writer.write_var_u32(containers.len() as u32);
                for container in containers {
                    container.encode(writer);
                }
            }
            None => writer.write_bool(false),
        }
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let status = reader.read_byte()?;
        let request_id = reader.read_var_i32()?;
        let containers = if reader.read_bool()? {
            let count = reader.read_var_u32()?;
            if count > 64 {
                return Err(CodecError::Custom(format!(
                    "Too many ItemStackResponse containers: {count}"
                )));
            }
            let mut containers = Vec::with_capacity(count as usize);
            for _ in 0..count {
                containers.push(ItemStackResponseContainer::decode(reader)?);
            }
            Some(containers)
        } else {
            None
        };
        Ok(Self { status, request_id, containers })
    }
}

/// ItemStackResponsePacket (0x94).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemStackResponsePacket {
    pub responses: Vec<ItemStackResponseEntry>,
}

impl ItemStackResponsePacket {
    pub const PACKET_ID: u32 = 0x94;

    pub fn new(responses: Vec<ItemStackResponseEntry>) -> Self {
        Self { responses }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_u32(self.responses.len() as u32);
        for entry in &self.responses {
            entry.encode(writer);
        }
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let count = reader.read_var_u32()?;
        if count > 64 {
            return Err(CodecError::Custom(format!(
                "Too many ItemStackResponses: {count}"
            )));
        }
        let mut responses = Vec::with_capacity(count as usize);
        for _ in 0..count {
            responses.push(ItemStackResponseEntry::decode(reader)?);
        }
        Ok(Self { responses })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_ok_and_error_entries() {
        let mut ok = ItemStackResponseEntry::ok(7);
        ok.containers.as_mut().unwrap().push(ItemStackResponseContainer {
            container: 12,
            dynamic_id: None,
            slots: vec![ItemStackResponseSlot {
                slot: 3,
                hotbar_slot: 3,
                count: 64,
                item_stack_id: Some(5),
                custom_name: String::new(),
                filtered_custom_name: None,
                durability_correction: 0,
            }],
        });
        let packet = ItemStackResponsePacket::new(vec![ok, ItemStackResponseEntry::error(8)]);
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(ItemStackResponsePacket::decode_payload(&mut reader).unwrap(), packet);
        assert_eq!(reader.remaining(), 0);
    }

    /// An error response is `count, result=1, request id, presence=false`:
    /// the optional containers byte is always written.
    #[test]
    fn error_entry_has_an_empty_optional() {
        let packet = ItemStackResponsePacket::new(vec![ItemStackResponseEntry::error(3)]);
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        assert_eq!(writer.into_bytes(), vec![0x01, STATUS_ERROR, 0x06, 0x00]);
    }

    /// An OK response without corrections is `count, 0, id, presence=true, 0`.
    #[test]
    fn ok_entry_without_corrections_writes_an_empty_list() {
        let packet = ItemStackResponsePacket::new(vec![ItemStackResponseEntry::ok(1)]);
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        assert_eq!(writer.into_bytes(), vec![0x01, STATUS_OK, 0x02, 0x01, 0x00]);
    }
}
