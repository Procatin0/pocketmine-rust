//! Packet ID 0x94 (148): ItemStackResponsePacket.
//!
//! Server answer to ItemStackRequestPacket (0x93): one response entry per
//! request id with a status and the mutated container slots so the client can
//! correct its prediction. Status 0 = OK, 1 = Error (requires resync).

use crate::binary::{CodecError, Reader, Writer};
use crate::packets::item_stack::ItemStack;
use crate::packets::item_stack_request::ItemStackContainerSlot;

pub const STATUS_OK: u8 = 0;
pub const STATUS_ERROR: u8 = 1;

/// One mutated slot confirmed by the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStackResponseSlot {
    pub slot: ItemStackContainerSlot,
    pub item: ItemStack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStackResponseEntry {
    pub request_id: i32,
    pub status: u8,
    pub slots: Vec<ItemStackResponseSlot>,
}

impl ItemStackResponseEntry {
    pub fn ok(request_id: i32) -> Self {
        Self { request_id, status: STATUS_OK, slots: Vec::new() }
    }

    pub fn error(request_id: i32) -> Self {
        Self { request_id, status: STATUS_ERROR, slots: Vec::new() }
    }

    pub fn encode(&self, writer: &mut Writer) {
        writer.write_var_i32(self.request_id);
        writer.write_byte(self.status);
        writer.write_var_u32(self.slots.len() as u32);
        for entry in &self.slots {
            entry.slot.encode(writer);
            entry.item.encode(writer);
        }
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let request_id = reader.read_var_i32()?;
        let status = reader.read_byte()?;
        let count = reader.read_var_u32()?;
        let mut slots = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let slot = ItemStackContainerSlot::decode(reader)?;
            let item = ItemStack::decode(reader)?;
            slots.push(ItemStackResponseSlot { slot, item });
        }
        Ok(Self { request_id, status, slots })
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
        ok.slots.push(ItemStackResponseSlot {
            slot: ItemStackContainerSlot::new(0, 3),
            item: ItemStack::new(1, 64),
        });
        let packet = ItemStackResponsePacket::new(vec![ok, ItemStackResponseEntry::error(8)]);
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(ItemStackResponsePacket::decode_payload(&mut reader).unwrap(), packet);
        assert_eq!(reader.remaining(), 0);
    }
}
