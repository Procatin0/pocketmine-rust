//! Server-side inventories of block containers, port of PocketMine's
//! `BlockInventory` implementations (`ChestInventory`, `DispenserInventory`,
//! `HopperInventory`, `BrewingStandInventory`, `FurnaceInventory`).
//!
//! Bedrock has no container ids of its own: the client addresses the slots of
//! the window it has open with the id it received in `ContainerOpenPacket`
//! (or with the `ContainerUIIds` id that window type uses, see
//! [`crate::crafting::ui_container_window`]). The contents are stored in the
//! world (see [`crate::world_state::WorldState::container_slots`]) so they
//! survive closing the window and remain the same inventory a dispenser fires
//! from.

use bedrock_protocol::ItemStack;
use bedrock_world::block_states::ContainerKind;

use crate::packets_wire::{
    CONTAINER_TYPE_BLAST_FURNACE, CONTAINER_TYPE_BREWING_STAND, CONTAINER_TYPE_CONTAINER,
    CONTAINER_TYPE_DISPENSER, CONTAINER_TYPE_DROPPER, CONTAINER_TYPE_FURNACE,
    CONTAINER_TYPE_HOPPER, CONTAINER_TYPE_SMOKER,
};

/// One open block container: the window id/type the client was told about and
/// the slots it owns.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockContainer {
    pub kind: ContainerKind,
    pub position: (i32, i32, i32),
    /// Window id sent in `ContainerOpenPacket`.
    pub window_id: u8,
    /// Bedrock `WindowTypes` byte sent in `ContainerOpenPacket`.
    pub container_type: u8,
    /// Slot contents, in the order the client addresses them.
    pub slots: Vec<ItemStack>,
}

impl BlockContainer {
    /// Number of slots the block owns, or `None` when this server does not
    /// model its inventory yet (enchanting table, grindstone, loom, ...).
    ///
    /// * chest/barrel/shulker/end chest: 27
    /// * dispenser/dropper: 9
    /// * hopper: 5
    /// * brewing stand: 3 potions (0..=2), ingredient (3), fuel (4)
    /// * furnace family: ingredient (0), fuel (1), result (2)
    pub fn slot_count(kind: ContainerKind) -> Option<usize> {
        Some(match kind {
            ContainerKind::Chest
            | ContainerKind::Barrel
            | ContainerKind::ShulkerBox
            | ContainerKind::EnderChest => 27,
            ContainerKind::Dispenser | ContainerKind::Dropper => 9,
            ContainerKind::Hopper => 5,
            ContainerKind::BrewingStand => 5,
            ContainerKind::Furnace | ContainerKind::BlastFurnace | ContainerKind::Smoker => 3,
            _ => return None,
        })
    }

    /// Bedrock `WindowTypes` byte: PocketMine's `InventoryManager` sends
    /// `CONTAINER` for chests, barrels, shulkers, dispensers and droppers, and
    /// the dedicated type for hoppers, brewing stands and the furnace family.
    pub fn container_type(kind: ContainerKind) -> Option<u8> {
        Some(match kind {
            ContainerKind::Chest
            | ContainerKind::Barrel
            | ContainerKind::ShulkerBox
            | ContainerKind::EnderChest => CONTAINER_TYPE_CONTAINER,
            ContainerKind::Dispenser => CONTAINER_TYPE_DISPENSER,
            ContainerKind::Dropper => CONTAINER_TYPE_DROPPER,
            ContainerKind::Hopper => CONTAINER_TYPE_HOPPER,
            ContainerKind::BrewingStand => CONTAINER_TYPE_BREWING_STAND,
            ContainerKind::Furnace => CONTAINER_TYPE_FURNACE,
            ContainerKind::BlastFurnace => CONTAINER_TYPE_BLAST_FURNACE,
            ContainerKind::Smoker => CONTAINER_TYPE_SMOKER,
            _ => return None,
        })
    }

    /// Window id sent in `ContainerOpenPacket`. These are the ids BDS uses for
    /// the block windows; only one container can be open at a time, so the
    /// furnace family shares one.
    pub fn window_id(kind: ContainerKind) -> Option<u8> {
        Some(match kind {
            ContainerKind::Chest
            | ContainerKind::Barrel
            | ContainerKind::ShulkerBox
            | ContainerKind::EnderChest => 2,
            ContainerKind::Dispenser => 4,
            ContainerKind::Dropper => 5,
            ContainerKind::Hopper => 8,
            ContainerKind::BrewingStand => 14,
            ContainerKind::Furnace | ContainerKind::BlastFurnace | ContainerKind::Smoker => 15,
            _ => return None,
        })
    }

    /// Builds the container for `kind` from the slots stored in the world.
    /// `None` when the kind has no modelled inventory.
    pub fn open(
        kind: ContainerKind,
        position: (i32, i32, i32),
        slots: Vec<ItemStack>,
    ) -> Option<Self> {
        let slot_count = Self::slot_count(kind)?;
        let mut slots = slots;
        slots.resize(slot_count, ItemStack::empty());
        Some(Self {
            kind,
            position,
            window_id: Self::window_id(kind)?,
            container_type: Self::container_type(kind)?,
            slots,
        })
    }

    /// Whether the kind is backed by a real inventory (and so can be opened).
    pub fn is_modelled(kind: ContainerKind) -> bool {
        Self::slot_count(kind).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_counts_and_types_match_the_reference() {
        assert_eq!(BlockContainer::slot_count(ContainerKind::Chest), Some(27));
        assert_eq!(BlockContainer::slot_count(ContainerKind::Dispenser), Some(9));
        assert_eq!(BlockContainer::slot_count(ContainerKind::Hopper), Some(5));
        assert_eq!(BlockContainer::slot_count(ContainerKind::BrewingStand), Some(5));
        assert_eq!(BlockContainer::slot_count(ContainerKind::Furnace), Some(3));
        assert_eq!(BlockContainer::slot_count(ContainerKind::Loom), None);

        assert_eq!(
            BlockContainer::container_type(ContainerKind::BrewingStand),
            Some(CONTAINER_TYPE_BREWING_STAND)
        );
        assert_eq!(
            BlockContainer::container_type(ContainerKind::Dispenser),
            Some(CONTAINER_TYPE_DISPENSER)
        );
        assert_eq!(
            BlockContainer::container_type(ContainerKind::Smoker),
            Some(CONTAINER_TYPE_SMOKER)
        );
    }

    #[test]
    fn opening_pads_the_stored_slots() {
        let container = BlockContainer::open(ContainerKind::BrewingStand, (1, 64, 2), Vec::new())
            .expect("brewing stand is modelled");
        assert_eq!(container.slots.len(), 5);
        assert_eq!(container.window_id, 14);
        assert!(container.slots.iter().all(ItemStack::is_empty));
    }
}
