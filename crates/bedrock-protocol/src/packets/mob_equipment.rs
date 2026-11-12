//! Packet ID 0x1F (31): MobEquipmentPacket.
//! Tells viewers which item an actor holds (and in which hand/slot).
//! Layout: actor_runtime_id var_u64, item ItemStack, inventory_slot var_i32,
//! hand u8, window_id u8.

use crate::binary::{CodecError, Reader, Writer};
use crate::packets::item_stack::ItemStack;

pub const HAND_MAIN: u8 = 0;
pub const HAND_OFF: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobEquipmentPacket {
    pub actor_runtime_id: u64,
    pub item: ItemStack,
    pub inventory_slot: i32,
    pub hand: u8,
    pub window_id: u8,
}

impl MobEquipmentPacket {
    pub const PACKET_ID: u32 = 0x1F;

    pub fn new(actor_runtime_id: u64, item: ItemStack, inventory_slot: i32, hand: u8, window_id: u8) -> Self {
        Self { actor_runtime_id, item, inventory_slot, hand, window_id }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_u64(self.actor_runtime_id);
        self.item.encode(writer);
        writer.write_var_i32(self.inventory_slot);
        writer.write_byte(self.hand);
        writer.write_byte(self.window_id);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            actor_runtime_id: reader.read_var_u64()?,
            item: ItemStack::decode(reader)?,
            inventory_slot: reader.read_var_i32()?,
            hand: reader.read_byte()?,
            window_id: reader.read_byte()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips() {
        let packet = MobEquipmentPacket::new(9, ItemStack::new(347, 1), 2, HAND_MAIN, 0);
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(MobEquipmentPacket::decode_payload(&mut reader).unwrap(), packet);
        assert_eq!(reader.remaining(), 0);
    }
}
