use bedrock_protocol::packets::inventory_transaction::{
    NetworkInventoryAction, SOURCE_CONTAINER, SOURCE_CREATIVE,
};
use bedrock_protocol::ItemStack;

/// Slots in the player's main inventory window (9 hotbar + 27 storage).
pub const INVENTORY_SIZE: usize = 36;
/// Slots reachable from the hotbar.
pub const HOTBAR_SIZE: usize = 9;
/// Default maximum stack size used by `/give`.
pub const MAX_STACK_SIZE: u16 = 64;

/// Window ids BDS uses for the player's containers.
pub const WINDOW_ID_INVENTORY: u32 = 0;
pub const WINDOW_ID_OFFHAND: u32 = 119;
pub const WINDOW_ID_ARMOR: u32 = 120;
pub const WINDOW_ID_CURSOR: u32 = 124;
/// Dynamic crafting windows (ItemStackRequest container ids).
pub const WINDOW_ID_CRAFT_SMALL: u32 = 125;
pub const WINDOW_ID_CRAFT_BIG: u32 = 126;
/// Result slot indices inside the crafting windows (grid + result).
pub const CRAFT_SMALL_RESULT: usize = 4;
pub const CRAFT_BIG_RESULT: usize = 9;
/// Armor slots: head, chest, legs, feet, body (Bedrock 1.21+ / protocol 2225).
pub const ARMOR_SIZE: usize = 5;
/// UI container slots BDS syncs in window 124 (54 slots, matching capture 3.bin).
pub const UI_SIZE: usize = 54;

/// Server-side player inventory.
///
/// Only the 36 main slots are modelled: they are the ones BDS syncs in the
/// `InventoryContentPacket` for window 0, and the only ones this server can
/// change (`/give`, `/clear`), since armor and the cursor need
/// `InventoryTransaction` handling that is not implemented.
#[derive(Debug, Clone)]
pub struct PlayerInventory {
    slots: Vec<ItemStack>,
    armor: Vec<ItemStack>,
    offhand: Vec<ItemStack>,
    cursor: Vec<ItemStack>,
    /// 2x2 player crafting grid (4 inputs + result at index 4).
    craft_small: Vec<ItemStack>,
    /// 3x3 crafting-table grid (9 inputs + result at index 9).
    craft_big: Vec<ItemStack>,
    selected_hotbar_slot: u8,
}

/// Result of applying an `InventoryTransactionPacket`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppliedTransaction {
    /// `(window id, slot)` pairs that changed and must be resent.
    pub changed: Vec<(u32, u32)>,
    /// The client's idea of a slot did not match the server: resync everything.
    pub rejected: bool,
}

impl Default for PlayerInventory {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerInventory {
    pub fn new() -> Self {
        Self {
            slots: vec![ItemStack::empty(); INVENTORY_SIZE],
            armor: vec![ItemStack::empty(); ARMOR_SIZE],
            offhand: vec![ItemStack::empty(); 1],
            cursor: vec![ItemStack::empty(); UI_SIZE],
            craft_small: vec![ItemStack::empty(); CRAFT_SMALL_RESULT + 1],
            craft_big: vec![ItemStack::empty(); CRAFT_BIG_RESULT + 1],
            selected_hotbar_slot: 0,
        }
    }

    pub fn slots(&self) -> &[ItemStack] {
        &self.slots
    }

    pub fn selected_hotbar_slot(&self) -> u8 {
        self.selected_hotbar_slot
    }

    pub fn set_selected_hotbar_slot(&mut self, slot: u8) {
        self.selected_hotbar_slot = slot.min(HOTBAR_SIZE as u8 - 1);
    }

    /// Adds an item, stacking on equal items first, and returns the leftover
    /// count that did not fit.
    pub fn add_item(&mut self, item: &ItemStack) -> u16 {
        if item.is_empty() || item.count == 0 {
            return 0;
        }

        let mut remaining = item.count;
        let max_stack = max_stack_size(item.network_id);

        for slot in self.slots.iter_mut() {
            if remaining == 0 {
                break;
            }
            if slot.is_empty()
                || slot.network_id != item.network_id
                || slot.meta != item.meta
                || slot.nbt != item.nbt
                || slot.count >= max_stack
            {
                continue;
            }
            let space = max_stack - slot.count;
            let moved = remaining.min(space);
            slot.count += moved;
            remaining -= moved;
        }

        for slot in self.slots.iter_mut() {
            if remaining == 0 {
                break;
            }
            if !slot.is_empty() {
                continue;
            }
            let moved = remaining.min(max_stack);
            *slot = ItemStack {
                count: moved,
                ..item.clone()
            };
            remaining -= moved;
        }

        remaining
    }

    /// Removes every item and returns how many individual items were removed.
    pub fn clear(&mut self) -> u32 {
        let mut removed = 0;
        for slot in self.slots.iter_mut() {
            if !slot.is_empty() {
                removed += slot.count as u32;
                *slot = ItemStack::empty();
            }
        }
        removed
    }

    /// Removes items matching `network_id` (and `meta` when given), stopping at
    /// `limit` items when one is provided. Returns how many were removed.
    pub fn remove_item(&mut self, network_id: i16, meta: Option<u32>, limit: Option<u32>) -> u32 {
        let mut removed = 0;
        for slot in self.slots.iter_mut() {
            if slot.is_empty() || slot.network_id != network_id {
                continue;
            }
            if let Some(meta) = meta {
                if slot.meta != meta {
                    continue;
                }
            }

            let to_remove = match limit {
                Some(limit) => {
                    let left = limit.saturating_sub(removed);
                    if left == 0 {
                        break;
                    }
                    (slot.count as u32).min(left)
                }
                None => slot.count as u32,
            };

            removed += to_remove;
            slot.count -= to_remove as u16;
            if slot.count == 0 {
                *slot = ItemStack::empty();
            }
        }
        removed
    }

    /// Counts items matching `network_id` (and `meta` when given).
    pub fn count_item(&self, network_id: i16, meta: Option<u32>) -> u32 {
        self.slots
            .iter()
            .filter(|slot| !slot.is_empty() && slot.network_id == network_id)
            .filter(|slot| meta.map(|meta| slot.meta == meta).unwrap_or(true))
            .map(|slot| slot.count as u32)
            .sum()
    }

    /// Windows covered by `/clear`, in the order vanilla iterates them:
    /// inventory, cursor, armor and offhand.
    const CLEAR_ORDER: [u32; 4] = [
        WINDOW_ID_INVENTORY,
        WINDOW_ID_CURSOR,
        WINDOW_ID_ARMOR,
        WINDOW_ID_OFFHAND,
    ];

    /// Mirrors PocketMine's `ClearCommand`: `max_count == 0` only counts the
    /// matching items, `-1` removes every one of them and a positive value
    /// removes at most that many across all player windows. Returns
    /// `(amount, counted_only)`.
    ///
    /// The filter is an item because `Item::equals()` compares the type,
    /// damage **and** named tag: a stack carrying NBT does not match the tagless
    /// item `/clear` builds from its argument.
    pub fn clear_matching(&mut self, item: Option<&ItemStack>, max_count: i32) -> (u32, bool) {
        let Some(filter) = item else {
            let mut removed = 0;
            for window_id in Self::CLEAR_ORDER {
                if let Some(slots) = self.window_mut(window_id) {
                    for slot in slots.iter_mut() {
                        if !slot.is_empty() {
                            removed += slot.count as u32;
                            *slot = ItemStack::empty();
                        }
                    }
                }
            }
            return (removed, false);
        };

        if max_count == 0 {
            let mut count = 0;
            for window_id in Self::CLEAR_ORDER {
                if let Some(slots) = self.window(window_id) {
                    count += slots
                        .iter()
                        .filter(|slot| Self::matches_filter(slot, filter))
                        .map(|slot| slot.count as u32)
                        .sum::<u32>();
                }
            }
            return (count, true);
        }

        let mut remaining = if max_count < 0 {
            u32::MAX
        } else {
            max_count as u32
        };
        let mut removed = 0;
        for window_id in Self::CLEAR_ORDER {
            let Some(slots) = self.window_mut(window_id) else {
                continue;
            };
            for slot in slots.iter_mut() {
                if remaining == 0 {
                    break;
                }
                if !Self::matches_filter(slot, filter) {
                    continue;
                }
                let take = (slot.count as u32).min(remaining);
                removed += take;
                remaining -= take;
                slot.count -= take as u16;
                if slot.count == 0 {
                    *slot = ItemStack::empty();
                }
            }
            if remaining == 0 {
                break;
            }
        }
        (removed, false)
    }

    /// `Item::equals($filter)`: same type, damage and named tag; the count is
    /// irrelevant.
    fn matches_filter(slot: &ItemStack, filter: &ItemStack) -> bool {
        !slot.is_empty()
            && slot.network_id == filter.network_id
            && slot.meta == filter.meta
            && slot.nbt == filter.nbt
    }

    /// Slots of a window, when the server models it.
    pub fn window(&self, window_id: u32) -> Option<&[ItemStack]> {
        match window_id {
            WINDOW_ID_INVENTORY => Some(&self.slots),
            WINDOW_ID_ARMOR => Some(&self.armor),
            WINDOW_ID_OFFHAND => Some(&self.offhand),
            WINDOW_ID_CURSOR => Some(&self.cursor),
            _ => None,
        }
    }

    fn window_mut(&mut self, window_id: u32) -> Option<&mut Vec<ItemStack>> {
        match window_id {
            WINDOW_ID_INVENTORY => Some(&mut self.slots),
            WINDOW_ID_ARMOR => Some(&mut self.armor),
            WINDOW_ID_OFFHAND => Some(&mut self.offhand),
            WINDOW_ID_CURSOR => Some(&mut self.cursor),
            _ => None,
        }
    }

    pub fn window_slot(&self, window_id: u32, slot: u32) -> Option<&ItemStack> {
        self.window(window_id)?.get(slot as usize)
    }

    pub fn set_window_slot(&mut self, window_id: u32, slot: u32, item: ItemStack) -> bool {
        let Some(slots) = self.window_mut(window_id) else {
            return false;
        };
        let Some(target) = slots.get_mut(slot as usize) else {
            return false;
        };
        *target = item;
        true
    }

    /// Applies the container actions of an `InventoryTransactionPacket`, the
    /// way PocketMine's `InventoryTransaction::execute` does: each action must
    /// describe the slot as the server has it, otherwise the whole transaction
    /// is rejected and the client is resynced.
    pub fn apply_actions(&mut self, actions: &[NetworkInventoryAction]) -> AppliedTransaction {
        let mut result = AppliedTransaction::default();
        let mut pending: Vec<(u32, u32, ItemStack)> = Vec::new();

        // PocketMine rejects the whole transaction when any action describes a
        // slot differently than the server holds it, so validate before
        // touching anything.
        for action in actions {
            match action.source_type {
                SOURCE_CONTAINER => {
                    let Some(window_id) = action.window_id.map(|id| id as i32) else {
                        continue;
                    };
                    if window_id < 0 {
                        result.rejected = true;
                        continue;
                    }
                    let window_id = window_id as u32;

                    let Some(current) = self.window_slot(window_id, action.inventory_slot) else {
                        result.rejected = true;
                        continue;
                    };
                    if !same_item(current, &action.old_item) {
                        result.rejected = true;
                        continue;
                    }
                    pending.push((window_id, action.inventory_slot, action.new_item.clone()));
                }
                // Creative edits create and delete items client-side; without
                // validating them the server would trust the client, so they
                // force a resync.
                SOURCE_CREATIVE => result.rejected = true,
                // Drops, pickups and client-side windows are not modelled yet.
                _ => {}
            }
        }

        if result.rejected {
            return result;
        }

        for (window_id, slot, item) in pending {
            if self.set_window_slot(window_id, slot, item) {
                result.changed.push((window_id, slot));
            }
        }

        result
    }

    pub fn total_items(&self) -> u32 {
        self.slots.iter().map(|slot| slot.count as u32).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(ItemStack::is_empty)
    }

    /// Contents as a fixed 36-slot list, ready for `InventoryContentPacket`.
    pub fn contents(&self) -> Vec<ItemStack> {
        self.slots.clone()
    }

    /// Item currently held in hand (hotbar slot), as sent in AddPlayerPacket.
    pub fn held_item(&self) -> ItemStack {
        self.slots
            .get(self.selected_hotbar_slot as usize)
            .cloned()
            .unwrap_or_else(ItemStack::empty)
    }

    /// Every modelled player window as (window id, slots), in the order
    /// vanilla syncs them: main inventory, armor, offhand and cursor.
    pub fn windows(&self) -> Vec<(u32, Vec<ItemStack>)> {
        vec![
            (WINDOW_ID_INVENTORY, self.slots.clone()),
            (WINDOW_ID_ARMOR, self.armor.clone()),
            (WINDOW_ID_OFFHAND, self.offhand.clone()),
            (WINDOW_ID_CURSOR, self.cursor.clone()),
        ]
    }

    /// Offhand item, as broadcast when another player spawns the holder.
    pub fn offhand_item(&self) -> ItemStack {
        self.offhand.first().cloned().unwrap_or_else(ItemStack::empty)
    }
}

/// Armor slots (head, chest, legs, feet), port of PocketMine
/// `ArmorInventory`. Legends are the network slot order BDS syncs in
/// window 120.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmorInventory {
    pub slots: Vec<ItemStack>,
}

impl Default for ArmorInventory {
    fn default() -> Self {
        Self::new()
    }
}

impl ArmorInventory {
    pub fn new() -> Self {
        Self { slots: vec![ItemStack::empty(); ARMOR_SIZE] }
    }

    pub fn slots(&self) -> &[ItemStack] {
        &self.slots
    }

    /// Equips `item` in armor `slot` (0-3) when the item name ends in the
    /// expected suffix (`_helmet`, `_chestplate`, `_leggings`, `_boots`) and
    /// returns the previously worn item. Returns `None` without touching
    /// anything when the slot is out of range or the item does not fit.
    pub fn equip(&mut self, slot: u8, item: ItemStack) -> Option<ItemStack> {
        let index = slot as usize;
        if index >= ARMOR_SIZE {
            return None;
        }
        if !item.is_empty() && !fits_armor_slot(&item, index) {
            return None;
        }
        let previous = self.slots[index].clone();
        self.slots[index] = item;
        Some(previous)
    }
}

/// Whether a canonical item name fits the armor slot (head/chest/legs/feet).
fn fits_armor_slot(item: &ItemStack, slot: usize) -> bool {
    let Some(name) = name_for_network_id(item.network_id) else {
        return false;
    };
    let expected = match slot {
        0 => "_helmet",
        1 => "_chestplate",
        2 => "_leggings",
        3 => "_boots",
        _ => return false,
    };
    name.ends_with(expected)
}

impl PlayerInventory {
    /// Armor slots (window 120), head first.
    pub fn armor_slots(&self) -> &[ItemStack] {
        &self.armor
    }

    /// Offhand slot (window 119).
    pub fn offhand(&self) -> ItemStack {
        self.offhand_item()
    }

    /// Cursor stack (window 124): the stack held between click and drop.
    pub fn cursor_item(&self) -> ItemStack {
        self.cursor.first().cloned().unwrap_or_else(ItemStack::empty)
    }

    pub fn set_cursor(&mut self, item: ItemStack) -> bool {
        let Some(target) = self.cursor.first_mut() else {
            return false;
        };
        *target = item;
        true
    }

    /// Crafting slot: `small=true` addresses the player 2x2 window (4 inputs +
    /// result at index 4), otherwise the 3x3 window (9 inputs + result at 9).
    pub fn craft_slot(&self, small: bool, slot: usize) -> Option<ItemStack> {
        let window = if small { &self.craft_small } else { &self.craft_big };
        window.get(slot).cloned()
    }

    pub fn set_craft_slot(&mut self, small: bool, slot: usize, item: ItemStack) -> bool {
        let window = if small {
            &mut self.craft_small
        } else {
            &mut self.craft_big
        };
        let Some(target) = window.get_mut(slot) else {
            return false;
        };
        *target = item;
        true
    }

    /// Q-key drop: removes up to `count` items from `(window, slot)` and
    /// returns the dropped stack (`None` when the slot is empty or unknown).
    pub fn drop_from_slot(
        &mut self,
        window_id: u32,
        slot: u32,
        count: u32,
    ) -> Option<ItemStack> {
        let current = self.window_slot(window_id, slot)?.clone();
        if current.is_empty() {
            return None;
        }
        let taken = count.min(current.count as u32) as u16;
        let dropped = ItemStack { count: taken, ..current.clone() };
        let mut updated = current;
        updated.count -= taken;
        self.set_window_slot(
            window_id,
            slot,
            if updated.count == 0 { ItemStack::empty() } else { updated },
        );
        Some(dropped)
    }

    /// Swaps two main-inventory slots (hotbar scroll / number-key swap).
    pub fn swap_inventory_slots(&mut self, a: usize, b: usize) {
        if a < INVENTORY_SIZE && b < INVENTORY_SIZE {
            self.slots.swap(a, b);
        }
    }

    /// Equips one armor piece, returning the previously worn item (if any).
    /// Rejects non-armor items (returns the item unchanged and equips
    /// nothing) the same way `ArmorInventory::equip` does.
    pub fn equip_armor(&mut self, slot: u8, item: ItemStack) -> Option<ItemStack> {
        let index = slot as usize;
        if index >= ARMOR_SIZE {
            return None;
        }
        if !item.is_empty() && !fits_armor_slot(&item, index) {
            return None;
        }
        let previous = self.armor[index].clone();
        self.armor[index] = item;
        Some(previous)
    }
}

/// Compares the fields the server validates on a transaction action (PocketMine
/// ignores the stack network id and NBT for this comparison).
fn same_item(a: &ItemStack, b: &ItemStack) -> bool {
    a.network_id == b.network_id
        && a.meta == b.meta
        && a.count == b.count
        && a.block_runtime_id == b.block_runtime_id
}

/// Stack size limit for an item; block-like items stack to 64 and the few
/// single-stack items this server can hand out are listed explicitly.
pub fn max_stack_size(network_id: i16) -> u16 {
    match network_id {
        // Buckets, tools and weapons are single-stack in vanilla.
        325..=350 => 1,
        _ => MAX_STACK_SIZE,
    }
}

/// Item dictionary captured from vanilla BDS 1.26.60 (protocol 2225), one
/// `identifier<TAB>numeric id` pair per line.
///
/// Data-driven items do not have a usable numeric id in this protocol, so they
/// are absent: `/give` can only hand out the classic, id-based items.
const ITEM_REGISTRY: &str = include_str!("../../../data/item_ids.txt");

fn registry() -> &'static std::collections::HashMap<&'static str, i16> {
    static REGISTRY: std::sync::OnceLock<std::collections::HashMap<&'static str, i16>> =
        std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        for line in ITEM_REGISTRY.lines() {
            let mut fields = line.split('\t');
            let (Some(name), Some(id)) = (fields.next(), fields.next()) else {
                continue;
            };
            if let Ok(id) = id.trim().parse::<i16>() {
                if id > 0 {
                    map.insert(name, id);
                }
            }
        }
        map
    })
}

/// Resolves a user supplied item name (`stone`, `Stone`, `minecraft:stone`) to
/// its canonical identifier, when the item exists in the registry.
pub fn resolve_item_name(input: &str) -> Option<&'static str> {
    let trimmed = input.trim().replace(' ', "_");
    if trimmed.is_empty() {
        return None;
    }
    let lowered = trimmed.to_lowercase();
    let bare = lowered
        .strip_prefix("minecraft:")
        .unwrap_or(&lowered)
        .to_string();

    let aliases: [(&str, &str); 2] = [
        ("grass", "minecraft:grass_block"),
        ("slab", "minecraft:oak_slab"),
    ];
    if let Some((_, canonical)) = aliases.iter().find(|(alias, _)| *alias == bare) {
        return Some(canonical);
    }

    let canonical = format!("minecraft:{bare}");
    registry()
        .get_key_value(canonical.as_str())
        .map(|(name, _)| *name)
}

/// Numeric item id used by `ItemStack` for a canonical identifier.
pub fn item_network_id(name: &str) -> Option<i16> {
    let lowered = name.to_lowercase();
    registry().get(lowered.as_str()).copied()
}

fn reverse_registry() -> &'static std::collections::HashMap<i16, &'static str> {
    static REVERSE: std::sync::OnceLock<std::collections::HashMap<i16, &'static str>> =
        std::sync::OnceLock::new();
    REVERSE.get_or_init(|| {
        registry()
            .iter()
            .map(|(name, id)| (*id, *name))
            .collect()
    })
}

/// Resolves the `<item[:damage]>` argument the way PocketMine does: the name
/// parser first, then the legacy parser (`1`, `1:2`).
///
/// Returns `(network id, meta, canonical name)`.
pub fn resolve_item(input: &str) -> Option<(i16, u32, &'static str)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (base, meta) = match trimmed.rsplit_once(':') {
        Some((base, suffix))
            if !base.is_empty()
                && !suffix.is_empty()
                && suffix.chars().all(|c| c.is_ascii_digit()) =>
        {
            (base, suffix.parse::<u32>().ok()?)
        }
        _ => (trimmed, 0),
    };

    if let Ok(id) = base.parse::<i16>() {
        // Legacy numeric ids only resolve when the registry knows them.
        let name = reverse_registry().get(&id)?;
        return Some((id, meta, *name));
    }

    let canonical = resolve_item_name(base)?;
    let id = item_network_id(canonical)?;
    Some((id, meta, canonical))
}

/// Human readable item name, e.g. `347` -> `"Diamond Sword"`, which is what the
/// vanilla `/give` success line prints.
pub fn item_display_name(network_id: i16) -> Option<String> {
    let canonical = reverse_registry().get(&network_id)?;
    let bare = canonical.strip_prefix("minecraft:").unwrap_or(canonical);
    Some(
        bare.split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Number of items known to the dictionary (used by diagnostics and tests).
pub fn known_item_count() -> usize {
    registry().len()
}

/// Canonical identifier for a numeric id, if the registry knows it.
pub fn name_for_network_id(network_id: i16) -> Option<&'static str> {
    reverse_registry().get(&network_id).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_and_stacks_items() {
        let mut inventory = PlayerInventory::new();

        assert_eq!(inventory.add_item(&ItemStack::new(1, 64)), 0);
        assert_eq!(inventory.add_item(&ItemStack::new(1, 10)), 0);
        assert_eq!(inventory.slots()[0].count, 64);
        assert_eq!(inventory.slots()[1].count, 10);
        assert_eq!(inventory.total_items(), 74);

        // Tools do not stack: two swords take two slots.
        assert_eq!(inventory.add_item(&ItemStack::new(347, 2)), 0);
        assert_eq!(inventory.slots()[2].count, 1);
        assert_eq!(inventory.slots()[3].count, 1);
    }

    #[test]
    fn reports_leftovers_when_the_inventory_is_full() {
        let mut inventory = PlayerInventory::new();
        for _ in 0..INVENTORY_SIZE {
            inventory.add_item(&ItemStack::new(1, 64));
        }
        assert_eq!(inventory.total_items(), 64 * INVENTORY_SIZE as u32);
        assert_eq!(inventory.add_item(&ItemStack::new(3, 5)), 5);
    }

    #[test]
    fn clears_and_removes_matching_items() {
        let mut inventory = PlayerInventory::new();
        inventory.add_item(&ItemStack::new(1, 32));
        inventory.add_item(&ItemStack::new(3, 5));

        assert_eq!(inventory.remove_item(1, None, Some(10)), 10);
        assert_eq!(inventory.count_item(1, None), 22);
        assert_eq!(inventory.remove_item(1, None, None), 22);
        assert_eq!(inventory.count_item(1, None), 0);

        assert_eq!(inventory.clear(), 5);
        assert!(inventory.is_empty());
        assert_eq!(inventory.contents().len(), INVENTORY_SIZE);
    }

    #[test]
    fn registry_resolves_names_and_ids() {
        assert!(known_item_count() > 800, "registry looks truncated");

        assert_eq!(resolve_item_name("stone"), Some("minecraft:stone"));
        assert_eq!(resolve_item_name("Stone"), Some("minecraft:stone"));
        assert_eq!(resolve_item_name("minecraft:stone"), Some("minecraft:stone"));
        assert_eq!(
            resolve_item_name("DIAMOND_SWORD"),
            Some("minecraft:diamond_sword")
        );
        assert_eq!(resolve_item_name("grass"), Some("minecraft:grass_block"));
        assert_eq!(resolve_item_name("not_an_item"), None);
        assert_eq!(resolve_item_name("   "), None);

        // Numeric ids come straight from the BDS item registry.
        assert_eq!(item_network_id("minecraft:stone"), Some(1));
        assert_eq!(item_network_id("minecraft:dirt"), Some(3));
        assert_eq!(item_network_id("minecraft:apple"), Some(285));
        assert_eq!(item_network_id("minecraft:diamond_sword"), Some(347));
    }

    #[test]
    fn resolve_item_accepts_names_ids_and_damage() {
        assert_eq!(resolve_item("stone"), Some((1, 0, "minecraft:stone")));
        assert_eq!(resolve_item("minecraft:stone"), Some((1, 0, "minecraft:stone")));
        assert_eq!(resolve_item("1"), Some((1, 0, "minecraft:stone")));
        assert_eq!(resolve_item("1:2"), Some((1, 2, "minecraft:stone")));
        assert_eq!(
            resolve_item("diamond_sword:5"),
            Some((347, 5, "minecraft:diamond_sword"))
        );
        assert_eq!(resolve_item("not_an_item"), None);
        assert_eq!(resolve_item("9999"), None);

        assert_eq!(item_display_name(347).as_deref(), Some("Diamond Sword"));
        assert_eq!(item_display_name(1).as_deref(), Some("Stone"));
    }

    /// The tagless item `/clear` builds from its argument.
    fn filter(network_id: i16, meta: u32) -> ItemStack {
        let mut stack = ItemStack::new(network_id, 1);
        stack.meta = meta;
        stack
    }

    #[test]
    fn clear_skips_stacks_whose_named_tag_differs() {
        let mut inventory = PlayerInventory::new();
        let mut tagged = ItemStack::new(1, 4);
        tagged.nbt = vec![10, 0, 0];
        inventory.add_item(&tagged);
        inventory.add_item(&ItemStack::new(1, 3));

        assert_eq!(inventory.clear_matching(Some(&filter(1, 0)), 0), (3, true));
        assert_eq!(inventory.clear_matching(Some(&filter(1, 0)), -1), (3, false));
        // Only the tagged stack is left, and clearing everything removes it.
        assert_eq!(inventory.clear_matching(None, -1), (4, false));
    }

    #[test]
    fn clear_matching_follows_the_vanilla_rules() {
        let mut inventory = PlayerInventory::new();
        inventory.add_item(&ItemStack::new(1, 10));
        inventory.add_item(&ItemStack::new(1, 6));

        assert_eq!(inventory.clear_matching(Some(&filter(1, 0)), 0), (16, true));
        assert_eq!(inventory.total_items(), 16);
        assert_eq!(inventory.clear_matching(Some(&filter(1, 0)), 5), (5, false));
        assert_eq!(inventory.total_items(), 11);
        assert_eq!(inventory.clear_matching(Some(&filter(1, 0)), -1), (11, false));
        assert!(inventory.is_empty());
    }

    #[test]
    fn clear_reaches_the_other_player_windows() {
        let mut inventory = PlayerInventory::new();
        inventory.add_item(&ItemStack::new(1, 4));
        inventory.set_window_slot(WINDOW_ID_ARMOR, 0, ItemStack::new(1, 2));
        inventory.set_window_slot(WINDOW_ID_OFFHAND, 0, ItemStack::new(1, 1));
        inventory.set_window_slot(WINDOW_ID_CURSOR, 0, ItemStack::new(1, 1));

        // Counting mode sees every window.
        assert_eq!(inventory.clear_matching(Some(&filter(1, 0)), 0), (8, true));

        // The limit is shared, in vanilla order: main inventory first, then
        // cursor, then armor and offhand.
        assert_eq!(inventory.clear_matching(Some(&filter(1, 0)), 5), (5, false));
        assert_eq!(inventory.total_items(), 0);
        assert_eq!(inventory.window_slot(WINDOW_ID_ARMOR, 0).unwrap().count, 2);
        assert_eq!(inventory.window_slot(WINDOW_ID_OFFHAND, 0).unwrap().count, 1);

        assert_eq!(inventory.clear_matching(Some(&filter(1, 0)), -1), (3, false));
        assert!(inventory
            .window(WINDOW_ID_ARMOR)
            .unwrap()
            .iter()
            .all(ItemStack::is_empty));
    }

    fn container_action(
        window_id: i8,
        slot: u32,
        old: ItemStack,
        new: ItemStack,
    ) -> NetworkInventoryAction {
        NetworkInventoryAction {
            source_type: SOURCE_CONTAINER,
            window_id: Some(window_id),
            source_flags: None,
            inventory_slot: slot,
            old_item: old,
            new_item: new,
        }
    }

    #[test]
    fn models_every_player_window() {
        let mut inventory = PlayerInventory::new();
        assert_eq!(inventory.window(WINDOW_ID_INVENTORY).map(<[ItemStack]>::len), Some(36));
        assert_eq!(inventory.window(WINDOW_ID_ARMOR).map(<[ItemStack]>::len), Some(ARMOR_SIZE));
        assert_eq!(inventory.window(WINDOW_ID_OFFHAND).map(<[ItemStack]>::len), Some(1));
        assert_eq!(inventory.window(WINDOW_ID_CURSOR).map(<[ItemStack]>::len), Some(UI_SIZE));
        assert!(inventory.window(999).is_none());

        assert!(inventory.set_window_slot(WINDOW_ID_ARMOR, 2, ItemStack::new(347, 1)));
        assert_eq!(inventory.window_slot(WINDOW_ID_ARMOR, 2), Some(&ItemStack::new(347, 1)));
        assert!(!inventory.set_window_slot(WINDOW_ID_ARMOR, 9, ItemStack::new(1, 1)));
    }

    #[test]
    fn applies_matching_transactions_and_rejects_stale_ones() {
        let mut inventory = PlayerInventory::new();
        inventory.set_window_slot(WINDOW_ID_INVENTORY, 0, ItemStack::new(1, 32));

        let applied = inventory.apply_actions(&[
            container_action(0, 0, ItemStack::new(1, 32), ItemStack::empty()),
            container_action(0, 5, ItemStack::empty(), ItemStack::new(1, 32)),
        ]);
        assert!(!applied.rejected);
        assert_eq!(applied.changed, vec![(0, 0), (0, 5)]);
        assert!(inventory.window_slot(WINDOW_ID_INVENTORY, 0).unwrap().is_empty());
        assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 5).unwrap().count, 32);

        // The client now claims a different stack than the server holds.
        let applied = inventory.apply_actions(&[
            container_action(0, 5, ItemStack::new(1, 64), ItemStack::empty()),
            container_action(0, 6, ItemStack::empty(), ItemStack::new(1, 64)),
        ]);
        assert!(applied.rejected);
        assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 5).unwrap().count, 32);
        assert!(inventory.window_slot(WINDOW_ID_INVENTORY, 6).unwrap().is_empty());
    }

    #[test]
    fn creative_actions_force_a_resync() {
        let mut inventory = PlayerInventory::new();
        let applied = inventory.apply_actions(&[NetworkInventoryAction {
            source_type: SOURCE_CREATIVE,
            window_id: None,
            source_flags: None,
            inventory_slot: 0,
            old_item: ItemStack::empty(),
            new_item: ItemStack::new(1, 64),
        }]);
        assert!(applied.rejected);
    }

    #[test]
    fn armor_equip_validates_piece_type() {
        let helmet = ItemStack::new(item_network_id("minecraft:iron_helmet").unwrap(), 1);
        let boots = ItemStack::new(item_network_id("minecraft:iron_boots").unwrap(), 1);
        let mut inventory = PlayerInventory::new();
        assert_eq!(inventory.equip_armor(0, helmet.clone()), Some(ItemStack::empty()));
        assert_eq!(inventory.armor_slots()[0].network_id, helmet.network_id);
        // Boots do not fit the helmet slot, and the slot stays untouched.
        assert_eq!(inventory.equip_armor(0, boots.clone()), None);
        assert_eq!(inventory.armor_slots()[0].network_id, helmet.network_id);
        assert_eq!(inventory.equip_armor(3, boots.clone()), Some(ItemStack::empty()));
        assert_eq!(inventory.armor_slots()[3].network_id, boots.network_id);
    }

    #[test]
    fn armor_inventory_struct_equip_and_reject() {
        let mut armor = ArmorInventory::new();
        let chest = ItemStack::new(item_network_id("minecraft:diamond_chestplate").unwrap(), 1);
        assert_eq!(armor.equip(1, chest.clone()), Some(ItemStack::empty()));
        assert_eq!(armor.slots()[1].network_id, chest.network_id);
        let sword = ItemStack::new(347, 1);
        assert_eq!(armor.equip(2, sword), None);
        assert!(armor.slots()[2].is_empty());
    }

    #[test]
    fn cursor_and_crafting_slots_roundtrip() {
        let mut inventory = PlayerInventory::new();
        inventory.set_cursor(ItemStack::new(1, 32));
        assert_eq!(inventory.cursor_item(), ItemStack::new(1, 32));
        assert!(inventory.set_craft_slot(true, 4, ItemStack::new(5, 4)));
        assert_eq!(inventory.craft_slot(true, 4), Some(ItemStack::new(5, 4)));
        assert!(inventory.set_craft_slot(false, 9, ItemStack::new(58, 1)));
        assert_eq!(inventory.craft_slot(false, 9), Some(ItemStack::new(58, 1)));
        assert!(!inventory.set_craft_slot(true, 9, ItemStack::new(1, 1)));
    }

    #[test]
    fn drop_out_of_range_returns_none() {
        let mut inventory = PlayerInventory::new();
        assert_eq!(inventory.drop_from_slot(WINDOW_ID_INVENTORY, 0, 1), None);
        assert_eq!(inventory.drop_from_slot(999, 0, 1), None);
    }
}
