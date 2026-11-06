use crate::binary::{CodecError, Reader, Writer};
use crate::packets::item_stack::ItemStack;

/// Container id for the player's main inventory (window 0).
pub const CONTAINER_ID_INVENTORY: u8 = 0;
/// Window id of the player's main inventory, as sent by BDS.
pub const WINDOW_ID_INVENTORY: u32 = 0;

/// Packet ID 0x31 (49): InventoryContentPacket.
///
/// Layout per Axolotl's `InventoryContentPacket` for protocol 2225: window id,
/// item count, item stacks, the full container name and the storage stack.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InventoryContentPacket {
    pub window_id: u32,
    pub items: Vec<ItemStack>,
    pub container_id: u8,
    pub dynamic_container_id: Option<u32>,
    pub storage: ItemStack,
}

impl InventoryContentPacket {
    pub const PACKET_ID: u32 = 0x31;

    /// Full contents of the player's 36-slot inventory window.
    pub fn inventory(items: Vec<ItemStack>) -> Self {
        Self {
            window_id: WINDOW_ID_INVENTORY,
            items,
            container_id: CONTAINER_ID_INVENTORY,
            dynamic_container_id: None,
            storage: ItemStack::empty(),
        }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_u32(self.window_id);
        writer.write_var_u32(self.items.len() as u32);
        for item in &self.items {
            item.encode(writer);
        }

        writer.write_byte(self.container_id);
        writer.write_bool(self.dynamic_container_id.is_some());
        if let Some(dynamic_id) = self.dynamic_container_id {
            writer.write_u32_le(dynamic_id);
        }
        self.storage.encode(writer);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let window_id = reader.read_var_u32()?;
        let count = reader.read_var_u32()?;
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            items.push(ItemStack::decode(reader)?);
        }

        let container_id = reader.read_byte()?;
        let dynamic_container_id = if reader.read_bool()? {
            Some(reader.read_u32_le()?)
        } else {
            None
        };
        let storage = ItemStack::decode(reader)?;

        Ok(Self {
            window_id,
            items,
            container_id,
            dynamic_container_id,
            storage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real BDS 1.26.60 packets: player inventory (36), armor (5),
    /// cursor/UI (54) and offhand (1) windows.
    const CAPTURES: [&[u8]; 4] = [
        include_bytes!("../../../../data/bds-capture/inventory_content/1.bin"),
        include_bytes!("../../../../data/bds-capture/inventory_content/2.bin"),
        include_bytes!("../../../../data/bds-capture/inventory_content/3.bin"),
        include_bytes!("../../../../data/bds-capture/inventory_content/4.bin"),
    ];

    #[test]
    fn decodes_bds_inventory_captures() {
        let expected = [(0u32, 36usize), (120, 5), (124, 54), (119, 1)];

        for (bytes, (window_id, slots)) in CAPTURES.iter().zip(expected) {
            let mut reader = Reader::new(bytes);
            assert_eq!(
                reader.read_var_u32().unwrap(),
                InventoryContentPacket::PACKET_ID
            );
            let packet = InventoryContentPacket::decode_payload(&mut reader).expect("decode");

            assert_eq!(packet.window_id, window_id);
            assert_eq!(packet.items.len(), slots);
            assert!(packet.items.iter().all(ItemStack::is_empty));
            assert_eq!(packet.container_id, 0);
            assert_eq!(packet.dynamic_container_id, None);
            assert!(packet.storage.is_empty());
            assert_eq!(reader.remaining(), 0, "capture must be consumed exactly");
        }
    }

    #[test]
    fn encodes_a_full_inventory_window() {
        let mut items = vec![ItemStack::empty(); 36];
        items[0] = ItemStack::new(1, 64);
        let packet = InventoryContentPacket::inventory(items);

        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();

        let mut reader = Reader::new(&bytes);
        let decoded = InventoryContentPacket::decode_payload(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }
}
