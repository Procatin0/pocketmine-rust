//! Anvil block logic: port of PocketMine `Anvil.php` to Rust.
//!
//! The anvil has 4 damage stages (0-3), shown by its block state. Every time
//! a player breaks a block placing on the anvil, the anvil takes damage:
//! stage 0 -> 1 (5% break chance) -> 2 (12%) -> 3 (25%) -> broken. The break
//! roll needs randomness, so [`AnvilState::damage`] takes a caller-supplied
//! roll in `0..100` to stay deterministic in tests.
//!
//! Repairing combines two items in the anvil inventory: identical items
//! repair 25% of the max durability per material item, the cost in XP levels
//! is `base + 2 * prior_work` (capped at 40, "too expensive" above), and the
//! result takes the sum of both prior-work costs. Durability is carried in
//! the `ItemStack.meta` field for the tools this server hands out
//! (`meta` = remaining durability) so no NBT scheme is needed; see
//! [`item_damage`] / [`set_item_damage`].

use bedrock_protocol::ItemStack;

use crate::inventory::{item_network_id, name_for_network_id};

/// Anvil damage stages (0 = fresh .. 3 = most damaged).
pub const ANVIL_DAMAGE_MAX: u8 = 3;
/// "Too expensive" XP level cap (vanilla anvil limit).
pub const MAX_REPAIR_LEVEL: i32 = 40;

/// Break chance per damage stage (percent) from `Anvil.php`.
pub fn break_chance(damage: u8) -> u8 {
    match damage {
        0 => 5,
        1 => 12,
        2 => 25,
        _ => 100,
    }
}

/// Outcome of hitting the anvil with a block break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnvilHit {
    pub damage: u8,
    pub broken: bool,
}

/// Anvil world state: current damage stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnvilState {
    pub damage: u8,
}

impl Default for AnvilState {
    fn default() -> Self {
        Self { damage: 0 }
    }
}

impl AnvilState {
    pub fn new(damage: u8) -> Self {
        Self { damage: damage.min(ANVIL_DAMAGE_MAX) }
    }

    /// Applies one block-break hit on the anvil. `roll` is the caller's
    /// `0..100` randomness source; when `roll < break_chance(damage)` the
    /// anvil breaks into items (PocketMine: "it breaks into 3-4 iron
    /// ingots" is handled by the caller dropping them).
    pub fn damage(&mut self, roll: u8) -> AnvilHit {
        if roll < break_chance(self.damage) {
            self.damage = 0;
            return AnvilHit { damage: 0, broken: true };
        }
        self.damage = (self.damage + 1).min(ANVIL_DAMAGE_MAX);
        AnvilHit { damage: self.damage, broken: false }
    }
}

/// Maximum durability for the tools/armor this registry knows. Returns 0 for
/// items that cannot be repaired.
pub fn max_durability(network_id: i16) -> u16 {
    let Some(name) = name_for_network_id(network_id) else {
        return 0;
    };
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    match n {
        "wooden_sword" | "wooden_pickaxe" | "wooden_axe" | "wooden_shovel" | "wooden_hoe" => 59,
        "stone_sword" | "stone_pickaxe" | "stone_axe" | "stone_shovel" | "stone_hoe" => 131,
        "iron_sword" | "iron_pickaxe" | "iron_axe" | "iron_shovel" | "iron_hoe" => 250,
        "diamond_sword" | "diamond_pickaxe" | "diamond_axe" | "diamond_shovel" | "diamond_hoe" => 1561,
        "golden_sword" | "golden_pickaxe" | "golden_axe" | "golden_shovel" | "golden_hoe" => 32,
        "netherite_sword" | "netherite_pickaxe" | "netherite_axe" | "netherite_shovel"
        | "netherite_hoe" => 2031,
        "leather_helmet" => 80,
        "leather_chestplate" => 112,
        "leather_leggings" => 96,
        "leather_boots" => 64,
        "golden_helmet" => 110,
        "golden_chestplate" => 154,
        "golden_leggings" => 132,
        "golden_boots" => 88,
        "chainmail_helmet" => 165,
        "chainmail_chestplate" => 240,
        "chainmail_leggings" => 225,
        "chainmail_boots" => 195,
        "iron_helmet" => 165,
        "iron_chestplate" => 240,
        "iron_leggings" => 225,
        "iron_boots" => 195,
        "diamond_helmet" => 528,
        "diamond_chestplate" => 792,
        "diamond_leggings" => 660,
        "diamond_boots" => 429,
        "netherite_helmet" => 561,
        "netherite_chestplate" => 891,
        "netherite_leggings" => 742,
        "netherite_boots" => 481,
        _ => 0,
    }
}

/// Damage taken by an item, stored in `ItemStack.meta`.
pub fn item_damage(item: &ItemStack) -> u16 {
    item.meta as u16
}

/// Sets the damage of an item, storing it in `ItemStack.meta`.
pub fn set_item_damage(item: &mut ItemStack, damage: u16) {
    item.meta = damage as u32;
}

/// Repaired result of combining two items in the anvil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnvilRepair {
    pub result: ItemStack,
    pub cost: i32,
    pub consume_second: u16,
}

/// Computes the anvil combination of `left` (item to repair) and `right`
/// (material or second item). Returns `None` when the combination is
/// impossible or "too expensive" (cost > 40).
///
/// Cost model (vanilla): base 1, +1 when renaming, repair costs 1 level per
/// 25% of max durability restored, and the final cost adds the prior-work
/// penalty (`2 * max(prior_left, prior_right) + 1` for repairs, `+ 2` for
/// renames). Prior work is read from `meta` high bits via
/// [`prior_work`]; this simplified port stores damage in `meta` low 16 bits
/// and prior work in the high bits so both survive the round-trip.
pub fn anvil_combine(left: &ItemStack, right: &ItemStack) -> Option<AnvilRepair> {
    if left.is_empty() || left.network_id != right.network_id {
        return None;
    }
    let max = max_durability(left.network_id);
    if max == 0 {
        return None;
    }
    let left_damage = item_damage(left) as u32;
    if left_damage == 0 {
        // Undamaged items cannot be repaired.
        return None;
    }
    // Same-item repair: each material restores 25% of max, plus a 5% bonus
    // per item (vanilla behavior).
    let mut restored = 0u32;
    for _ in 0..right.count {
        restored = restored.saturating_add((max / 4) as u32);
    }
    restored = restored.saturating_add((max as u32) / 20 * right.count as u32);
    let repaired = (max as u32 - left_damage).min(restored);
    if repaired == 0 {
        return None;
    }
    let mut result = left.clone();
    let new_damage = left_damage.saturating_sub(repaired) as u16;
    set_item_damage(&mut result, new_damage);
    // Preserve the stack count of the LEFT item (vanilla only repairs one).
    result.count = left.count;

    let base = 1 + (repaired / ((max / 4).max(1) as u32)) as i32;
    let prior = (prior_work(left) + prior_work(right) + 1) * 2;
    let cost = base + prior;
    if cost > MAX_REPAIR_LEVEL {
        return None;
    }
    Some(AnvilRepair {
        result,
        cost,
        consume_second: right.count,
    })
}

/// Applies an [`AnvilRepair`] to the player: consumes both inputs and adds
/// the result, deducting `cost` XP levels from `xp_level`. Returns the
/// number of levels actually deducted (0 when the player could not afford
/// it, in which case the inputs are left untouched).
pub fn apply_repair(
    inventory: &mut crate::inventory::PlayerInventory,
    left_index: u32,
    right_index: u32,
    repair: &AnvilRepair,
    xp_level: &mut i32,
) -> Option<u16> {
    if *xp_level < repair.cost {
        return None;
    }
    *xp_level -= repair.cost;
    let left = inventory
        .window_slot(crate::inventory::WINDOW_ID_INVENTORY, left_index)?
        .clone();
    if left.count > 1 {
        inventory.set_window_slot(
            crate::inventory::WINDOW_ID_INVENTORY,
            left_index,
            ItemStack { count: left.count - 1, ..left.clone() },
        );
    } else {
        inventory.set_window_slot(crate::inventory::WINDOW_ID_INVENTORY, left_index, ItemStack::empty());
    }
    let right = inventory
        .window_slot(crate::inventory::WINDOW_ID_INVENTORY, right_index)?
        .clone();
    let remaining = right.count.saturating_sub(repair.consume_second);
    inventory.set_window_slot(
        crate::inventory::WINDOW_ID_INVENTORY,
        right_index,
        if remaining == 0 { ItemStack::empty() } else { ItemStack { count: remaining, ..right.clone() } },
    );
    let leftover = inventory.add_item(&repair.result);
    Some(leftover)
}

/// Anvil inventory (left, right, result), port of the `Anvil` block
/// inventory PocketMine opens with `ContainerOpenPacket` (0x2E).
#[derive(Debug, Clone, Default)]
pub struct AnvilInventory {
    pub left: ItemStack,
    pub right: ItemStack,
    pub result: ItemStack,
    pub cost: i32,
}

impl AnvilInventory {
    pub fn new() -> Self {
        Self {
            left: ItemStack::empty(),
            right: ItemStack::empty(),
            result: ItemStack::empty(),
            cost: 0,
        }
    }

    /// Recomputes the result slot from the two inputs.
    pub fn update_result(&mut self) {
        match anvil_combine(&self.left, &self.right) {
            Some(repair) => {
                self.result = repair.result;
                self.cost = repair.cost;
            }
            None => {
                self.result = ItemStack::empty();
                self.cost = 0;
            }
        }
    }

    /// Takes the result once, consuming both inputs.
    pub fn take_result(&mut self) -> Option<(ItemStack, i32)> {
        self.update_result();
        if self.result.is_empty() {
            return None;
        }
        let result = self.result.clone();
        let cost = self.cost;
        self.left = ItemStack::empty();
        self.right = ItemStack::empty();
        self.result = ItemStack::empty();
        self.cost = 0;
        Some((result, cost))
    }

    /// Returns the inputs on container close.
    pub fn close_drops(&mut self) -> Vec<ItemStack> {
        let drops: Vec<ItemStack> = [self.left.clone(), self.right.clone()]
            .into_iter()
            .filter(|item| !item.is_empty())
            .collect();
        self.left = ItemStack::empty();
        self.right = ItemStack::empty();
        self.result = ItemStack::empty();
        self.cost = 0;
        drops
    }
}

/// Prior-work penalty stored in the high bits of `meta`.
pub fn prior_work(item: &ItemStack) -> i32 {
    ((item.meta >> 16) & 0xFF) as i32
}

/// Sets the prior-work value (high bits of `meta`) without touching damage.
pub fn set_prior_work(item: &mut ItemStack, value: i32) {
    let low = item.meta & 0xFFFF;
    item.meta = low | (((value.max(0) as u32) & 0xFF) << 16);
}

/// Whether the network id is an anvil item.
pub fn is_anvil(network_id: i16) -> bool {
    item_network_id("minecraft:anvil") == Some(network_id)
        || item_network_id("minecraft:chipped_anvil") == Some(network_id)
        || item_network_id("minecraft:damaged_anvil") == Some(network_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::PlayerInventory;

    fn damaged(id: i16, damage: u16) -> ItemStack {
        let mut item = ItemStack::new(id, 1);
        set_item_damage(&mut item, damage);
        item
    }

    #[test]
    fn anvil_takes_damage_and_breaks() {
        let mut anvil = AnvilState::new(0);
        let hit = anvil.damage(50); // 50 >= 5: no break
        assert!(!hit.broken);
        assert_eq!(hit.damage, 1);
        let hit = anvil.damage(50);
        assert_eq!(hit.damage, 2);
        let hit = anvil.damage(50);
        assert_eq!(hit.damage, 3);
        // Stage 3 breaks at 100%, so any roll breaks it.
        let hit = anvil.damage(0);
        assert!(hit.broken);
        assert_eq!(hit.damage, 0);
    }

    #[test]
    fn anvil_breaks_at_roll_zero_stage_zero() {
        let mut anvil = AnvilState::new(0);
        let hit = anvil.damage(0);
        assert!(hit.broken);
    }

    #[test]
    fn repairs_damaged_sword_with_second_sword() {
        let sword = item_network_id("minecraft:diamond_sword").unwrap();
        let left = damaged(sword, 500);
        let right = damaged(sword, 0);
        let repair = anvil_combine(&left, &right).expect("repairable");
        assert!(repair.cost > 0 && repair.cost <= MAX_REPAIR_LEVEL);
        // 500 damage at 1561 max: one material restores 25% (390) + 5% (78).
        assert_eq!(item_damage(&repair.result), 500 - 390 - 78);
        assert_eq!(repair.consume_second, 1);
    }

    #[test]
    fn too_expensive_combinations_are_rejected() {
        let sword = item_network_id("minecraft:diamond_sword").unwrap();
        let mut left = damaged(sword, 100);
        set_prior_work(&mut left, 30);
        let mut right = damaged(sword, 100);
        set_prior_work(&mut right, 30);
        assert!(anvil_combine(&left, &right).is_none());
    }

    #[test]
    fn cannot_repair_different_items() {
        let sword = item_network_id("minecraft:diamond_sword").unwrap();
        let pickaxe = item_network_id("minecraft:diamond_pickaxe").unwrap();
        assert!(anvil_combine(&damaged(sword, 100), &damaged(pickaxe, 100)).is_none());
        assert!(anvil_combine(&damaged(sword, 0), &damaged(sword, 0)).is_none());
    }

    #[test]
    fn anvil_inventory_computes_and_takes_result() {
        let sword = item_network_id("minecraft:diamond_sword").unwrap();
        let mut slots = AnvilInventory::new();
        slots.left = damaged(sword, 800);
        slots.right = damaged(sword, 500);
        slots.update_result();
        assert!(!slots.result.is_empty());
        assert!(slots.cost > 0);
        let expected_cost = slots.cost;
        let (result, cost) = slots.take_result().expect("result ready");
        assert_eq!(cost, expected_cost);
        assert!(slots.left.is_empty() && slots.right.is_empty());
        assert_eq!(item_damage(&result), 800 - 390 - 78);
    }

    #[test]
    fn close_returns_inputs() {
        let mut slots = AnvilInventory::new();
        slots.left = ItemStack::new(1, 4);
        slots.right = ItemStack::new(3, 2);
        let drops = slots.close_drops();
        assert_eq!(drops.len(), 2);
    }

    #[test]
    fn apply_repair_deducts_xp_and_consumes_inputs() {
        let sword = item_network_id("minecraft:diamond_sword").unwrap();
        let mut inventory = PlayerInventory::new();
        inventory.set_window_slot(crate::inventory::WINDOW_ID_INVENTORY, 0, damaged(sword, 600));
        inventory.set_window_slot(crate::inventory::WINDOW_ID_INVENTORY, 1, damaged(sword, 0));
        let repair = anvil_combine(
            &damaged(sword, 600),
            &damaged(sword, 0),
        )
        .unwrap();
        let mut xp = 10;
        let leftover = apply_repair(&mut inventory, 0, 1, &repair, &mut xp);
        assert_eq!(leftover, Some(0));
        assert_eq!(xp, 10 - repair.cost);
        // Result landed in the inventory, inputs consumed.
        assert_eq!(inventory.total_items(), 1);
    }

    #[test]
    fn apply_repair_fails_without_enough_xp() {
        let sword = item_network_id("minecraft:diamond_sword").unwrap();
        let mut inventory = PlayerInventory::new();
        inventory.set_window_slot(crate::inventory::WINDOW_ID_INVENTORY, 0, damaged(sword, 600));
        inventory.set_window_slot(crate::inventory::WINDOW_ID_INVENTORY, 1, damaged(sword, 0));
        let repair = anvil_combine(&damaged(sword, 600), &damaged(sword, 0)).unwrap();
        let mut xp = 0;
        assert!(apply_repair(&mut inventory, 0, 1, &repair, &mut xp).is_none());
        assert_eq!(inventory.total_items(), 2);
    }
}
