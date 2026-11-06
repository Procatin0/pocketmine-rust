use crate::binary::{CodecError, Reader, Writer};
use crate::packets::item_stack::ItemStack;

/// Packet ID 0x32 (50): InventorySlotPacket.
///
/// Layout per Axolotl's `InventorySlotPacket` for protocol 2225: window id,
/// inventory slot, optional `FullContainerName`, optional storage stack and the
/// slot contents.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InventorySlotPacket {
    pub window_id: u32,
    pub inventory_slot: u32,
    pub container_id: Option<u8>,
    pub dynamic_container_id: Option<u32>,
    pub storage: Option<ItemStack>,
    pub item: ItemStack,
}

impl InventorySlotPacket {
    pub const PACKET_ID: u32 = 0x32;

    pub fn new(window_id: u32, inventory_slot: u32, item: ItemStack) -> Self {
        Self {
            window_id,
            inventory_slot,
            item,
            ..Self::default()
        }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_u32(self.window_id);
        writer.write_var_u32(self.inventory_slot);

        writer.write_bool(self.container_id.is_some());
        if let Some(container_id) = self.container_id {
            writer.write_byte(container_id);
            writer.write_bool(self.dynamic_container_id.is_some());
            if let Some(dynamic_id) = self.dynamic_container_id {
                writer.write_u32_le(dynamic_id);
            }
        }

        writer.write_bool(self.storage.is_some());
        if let Some(storage) = &self.storage {
            storage.encode(writer);
        }

        self.item.encode(writer);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let window_id = reader.read_var_u32()?;
        let inventory_slot = reader.read_var_u32()?;

        let (container_id, dynamic_container_id) = if reader.read_bool()? {
            let container_id = reader.read_byte()?;
            let dynamic = if reader.read_bool()? {
                Some(reader.read_u32_le()?)
            } else {
                None
            };
            (Some(container_id), dynamic)
        } else {
            (None, None)
        };

        let storage = if reader.read_bool()? {
            Some(ItemStack::decode(reader)?)
        } else {
            None
        };

        let item = ItemStack::decode(reader)?;

        Ok(Self {
            window_id,
            inventory_slot,
            container_id,
            dynamic_container_id,
            storage,
            item,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_a_slot_update() {
        let packet = InventorySlotPacket::new(0, 5, ItemStack::new(1, 64));

        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();

        let mut reader = Reader::new(&bytes);
        let decoded = InventorySlotPacket::decode_payload(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn roundtrips_with_container_name_and_storage() {
        let packet = InventorySlotPacket {
            window_id: 120,
            inventory_slot: 2,
            container_id: Some(28),
            dynamic_container_id: Some(7),
            storage: Some(ItemStack::new(1, 1)),
            item: ItemStack::new(347, 1),
        };

        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();

        let mut reader = Reader::new(&bytes);
        assert_eq!(
            InventorySlotPacket::decode_payload(&mut reader).expect("decode"),
            packet
        );
        assert_eq!(reader.remaining(), 0);
    }
}
