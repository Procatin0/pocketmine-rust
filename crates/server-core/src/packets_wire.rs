//! Wire helpers for inventory/crafting packets (0x2E/0x2F/0x91/0x93/0x94).
//!
//! These build full packet bytes (id + payload) so sessions can send them
//! through `OutgoingPacket::RawWire`, matching the existing static-packet
//! style.

use bedrock_protocol::binary::Writer;
use bedrock_protocol::packets::container_close::ContainerClosePacket;
use bedrock_protocol::packets::container_open::ContainerOpenPacket;

pub use bedrock_protocol::packets::container_open::{
    BLOCK_CONTAINER_ACTOR_ID, CONTAINER_TYPE_ANVIL, CONTAINER_TYPE_CONTAINER,
    CONTAINER_TYPE_INVENTORY, CONTAINER_TYPE_WORKBENCH,
};
use bedrock_protocol::packets::item_stack_response::ItemStackResponsePacket;
use bedrock_protocol::{BlockPos, ItemStack};

/// `ContainerOpenPacket` (0x2E): window id, container type, position,
/// actor unique id (`-1` for block containers).
pub fn container_open_wire(
    window_id: u8,
    container_type: u8,
    position: (i32, i32, i32),
    actor_unique_id: i64,
) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(ContainerOpenPacket::PACKET_ID);
    ContainerOpenPacket::new(
        window_id,
        container_type,
        BlockPos::new(position.0, position.1, position.2),
        actor_unique_id,
    )
    .encode_payload(&mut writer);
    writer.into_bytes().to_vec()
}

/// `ContainerClosePacket` (0x2F).
pub fn container_close_wire(window_id: u8, container_type: u8, server_initiated: bool) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(ContainerClosePacket::PACKET_ID);
    ContainerClosePacket::new(window_id, container_type, server_initiated)
        .encode_payload(&mut writer);
    writer.into_bytes().to_vec()
}

/// `ItemStackResponsePacket` (0x94): one entry per request id.
pub fn item_stack_response_wire(
    entry: &bedrock_protocol::packets::item_stack_response::ItemStackResponseEntry,
) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(ItemStackResponsePacket::PACKET_ID);
    ItemStackResponsePacket::new(vec![entry.clone()]).encode_payload(&mut writer);
    writer.into_bytes().to_vec()
}

/// Compatibility shim: a negative sentinel hash for a tag. PocketMine's
/// `CraftingDataPacket` actually serializes tags as `ItemTag` descriptors
/// ("type 3 + tag string"), which is what `RecipeDescriptor::ItemTag` and
/// `generated_crafting_data_wire` already emit — so this hash is NOT used
/// for the wire. It is kept only for callers that need a stable negative
/// key for a tag.
pub fn tag_network_id(tag: &str) -> i16 {
    let mut hash: u32 = 0x811c9dc5;
    for byte in tag.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    ((hash % 2000) as i16) * -1 - 1
}

/// Shape pattern of a recipe as `(network_id, meta)` input pairs.
pub fn recipe_shape_inputs(
    shape: &[Vec<Option<crate::crafting::RecipeIngredient>>],
) -> Vec<(i16, u32)> {
    shape
        .iter()
        .flatten()
        .filter_map(|cell| cell.as_ref())
        .map(|ingredient| match ingredient {
            crate::crafting::RecipeIngredient::Exact { network_id, meta } => (*network_id, *meta),
            crate::crafting::RecipeIngredient::MetaWildcard { network_id } => (*network_id, 32767),
            crate::crafting::RecipeIngredient::TagWildcard { tag } => (tag_network_id(tag), 0),
        })
        .collect()
}

/// Empty-stack slot update for `InventorySlotPacket` (0x93 endpoint usage:
/// the full slot-list response is built by sessions; this helper covers the
/// single-slot case).
pub fn empty_stack() -> ItemStack {
    ItemStack::empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_wires_roundtrip() {
        let open = container_open_wire(10, 1, (1, 64, -3), -1);
        let packet_id_read = bedrock_protocol::binary::Reader::new(&open).read_var_u32().unwrap();
        assert_eq!(packet_id_read, ContainerOpenPacket::PACKET_ID);
        let close = container_close_wire(16, 5, false);
        let close_id = bedrock_protocol::binary::Reader::new(&close).read_var_u32().unwrap();
        assert_eq!(close_id, ContainerClosePacket::PACKET_ID);
    }

    #[test]
    fn tag_sentinels_are_stable_and_negative() {
        assert_eq!(tag_network_id("minecraft:stone_crafting_materials"), tag_network_id("minecraft:stone_crafting_materials"));
        assert!(tag_network_id("minecraft:stone_crafting_materials") < 0);
    }
}
