//! Entity type registry, translated from PocketMine-MP / axolotl-pm.
//!
//! Each entry mirrors one `pocketmine\entity\*` class: its
//! `getNetworkTypeId()` (`EntityIds`), `getInitialSizeInfo()`
//! (`EntitySizeInfo`) and, for `Living`, its maximum health
//! (`getInitialMaxHealth()` / `AttributeFactory::HEALTH`).  Movement
//! constants come from `Entity::getInitialGravity()` and
//! `Entity::getInitialDragMultiplier()`.
//!
//! In PocketMine there is no vanilla mob AI: a `Mob` only falls, is pushed by
//! the world and takes damage.  This port keeps that behaviour and adds the
//! light wander pass in [`crate::entity`] so mobs are visible in motion, but
//! the table itself is a direct translation of the PHP metadata.

/// One entity type known to the server.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityType {
    /// `EntityIds` value, e.g. `minecraft:zombie`.
    pub id: &'static str,
    /// `EntitySizeInfo::getHeight()`.
    pub height: f32,
    /// `EntitySizeInfo::getWidth()`.
    pub width: f32,
    /// `Living::getMaxHealth()` (0 for non-living entities).
    pub max_health: f32,
    /// `Entity::getInitialGravity()`.
    pub gravity: f32,
    /// `Entity::getInitialDragMultiplier()`.
    pub drag: f32,
    /// `Entity::applyDragBeforeGravity()`.
    pub drag_before_gravity: bool,
    /// Whether the type is a `Living` (has health, can die, drops loot).
    pub is_mob: bool,
    /// `Living::getXpDropAmount()`.
    pub xp: i32,
}

impl EntityType {
    const fn mob(id: &'static str, height: f32, width: f32, max_health: f32, xp: i32) -> Self {
        Self {
            id,
            height,
            width,
            max_health,
            gravity: 0.04,
            drag: 0.02,
            drag_before_gravity: false,
            is_mob: true,
            xp,
        }
    }

    const fn object(id: &'static str, height: f32, width: f32) -> Self {
        Self {
            id,
            height,
            width,
            max_health: 0.0,
            gravity: 0.04,
            drag: 0.02,
            drag_before_gravity: false,
            is_mob: false,
            xp: 0,
        }
    }

    const fn projectile(id: &'static str, gravity: f32) -> Self {
        Self {
            id,
            height: 0.25,
            width: 0.25,
            max_health: 0.0,
            gravity,
            drag: 0.01,
            drag_before_gravity: true,
            is_mob: false,
            xp: 0,
        }
    }
}

/// Every type the server can spawn, indexed by network type id.
pub const ENTITY_TYPES: &[EntityType] = &[
    // ---- objects ----------------------------------------------------
    EntityType::object("minecraft:item", 0.25, 0.25),
    EntityType::object("minecraft:falling_block", 0.98, 0.98),
    EntityType::object("minecraft:xp_orb", 0.25, 0.25),
    EntityType::object("minecraft:tnt", 0.98, 0.98),
    // ---- projectiles (`src/entity/projectile/*.php`) ----------------
    EntityType::projectile("minecraft:arrow", 0.05),
    EntityType::projectile("minecraft:snowball", 0.03),
    EntityType::projectile("minecraft:egg", 0.03),
    EntityType::projectile("minecraft:ender_pearl", 0.03),
    EntityType::projectile("minecraft:xp_bottle", 0.03),
    EntityType::projectile("minecraft:splash_potion", 0.03),
    EntityType::projectile("minecraft:thrown_trident", 0.05),
    EntityType::projectile("minecraft:ice_bomb", 0.03),
    // ---- mobs (`src/entity/*.php`, `getInitialSizeInfo`) ------------
    EntityType::mob("minecraft:chicken", 0.7, 0.4, 4.0, 1),
    EntityType::mob("minecraft:cow", 1.4, 0.9, 10.0, 1),
    EntityType::mob("minecraft:pig", 0.9, 0.9, 10.0, 1),
    EntityType::mob("minecraft:sheep", 1.3, 0.9, 8.0, 1),
    EntityType::mob("minecraft:wolf", 0.85, 0.6, 8.0, 1),
    EntityType::mob("minecraft:villager", 1.95, 0.6, 20.0, 0),
    EntityType::mob("minecraft:mooshroom", 1.4, 0.9, 10.0, 1),
    EntityType::mob("minecraft:squid", 0.8, 0.6, 10.0, 1),
    EntityType::mob("minecraft:rabbit", 0.5, 0.4, 3.0, 1),
    EntityType::mob("minecraft:bat", 0.5, 0.5, 6.0, 0),
    EntityType::mob("minecraft:iron_golem", 2.7, 1.4, 100.0, 0),
    EntityType::mob("minecraft:snow_golem", 1.9, 0.7, 4.0, 0),
    EntityType::mob("minecraft:ocelot", 0.7, 0.6, 10.0, 1),
    EntityType::mob("minecraft:horse", 1.6, 1.4, 15.0, 1),
    EntityType::mob("minecraft:donkey", 1.6, 1.4, 15.0, 1),
    EntityType::mob("minecraft:mule", 1.6, 1.4, 15.0, 1),
    EntityType::mob("minecraft:skeleton_horse", 1.6, 1.4, 15.0, 1),
    EntityType::mob("minecraft:zombie_horse", 1.6, 1.4, 15.0, 1),
    EntityType::mob("minecraft:polar_bear", 1.4, 1.3, 30.0, 1),
    EntityType::mob("minecraft:llama", 1.87, 0.9, 15.0, 1),
    EntityType::mob("minecraft:parrot", 0.9, 0.5, 6.0, 0),
    EntityType::mob("minecraft:dolphin", 0.6, 0.9, 10.0, 0),
    EntityType::mob("minecraft:zombie", 1.9, 0.6, 20.0, 5),
    EntityType::mob("minecraft:creeper", 1.7, 0.6, 20.0, 5),
    EntityType::mob("minecraft:skeleton", 1.9, 0.6, 20.0, 5),
    EntityType::mob("minecraft:spider", 0.9, 1.4, 16.0, 5),
    EntityType::mob("minecraft:zombie_pigman", 1.9, 0.6, 20.0, 5),
    EntityType::mob("minecraft:slime", 0.5, 0.5, 1.0, 1),
    EntityType::mob("minecraft:enderman", 2.9, 0.6, 40.0, 5),
    EntityType::mob("minecraft:silverfish", 0.3, 0.4, 8.0, 5),
    EntityType::mob("minecraft:cave_spider", 0.5, 0.7, 12.0, 5),
    EntityType::mob("minecraft:ghast", 4.0, 4.0, 10.0, 5),
    EntityType::mob("minecraft:magma_cube", 0.5, 0.5, 1.0, 1),
    EntityType::mob("minecraft:blaze", 1.8, 0.6, 20.0, 10),
    EntityType::mob("minecraft:zombie_villager", 1.95, 0.6, 20.0, 5),
    EntityType::mob("minecraft:witch", 1.95, 0.6, 26.0, 5),
    EntityType::mob("minecraft:stray", 1.9, 0.6, 20.0, 5),
    EntityType::mob("minecraft:husk", 1.9, 0.6, 20.0, 5),
    EntityType::mob("minecraft:wither_skeleton", 2.4, 0.7, 20.0, 5),
    EntityType::mob("minecraft:guardian", 0.85, 0.85, 30.0, 10),
    EntityType::mob("minecraft:elder_guardian", 2.0, 1.8, 80.0, 10),
    EntityType::mob("minecraft:shulker", 1.0, 1.0, 30.0, 5),
    EntityType::mob("minecraft:endermite", 0.3, 0.4, 8.0, 3),
    EntityType::mob("minecraft:vindicator", 1.95, 0.6, 24.0, 5),
    EntityType::mob("minecraft:phantom", 0.5, 0.9, 20.0, 5),
    EntityType::mob("minecraft:ravager", 2.2, 1.95, 100.0, 20),
    EntityType::mob("minecraft:armor_stand", 1.975, 0.5, 20.0, 0),
    EntityType::mob("minecraft:wither", 3.5, 0.9, 300.0, 50),
    EntityType::mob("minecraft:ender_dragon", 8.0, 16.0, 200.0, 0),
    EntityType::mob("minecraft:turtle", 0.4, 1.2, 30.0, 1),
    EntityType::mob("minecraft:panda", 1.25, 1.3, 20.0, 1),
    EntityType::mob("minecraft:fox", 0.7, 0.6, 10.0, 1),
    EntityType::mob("minecraft:bee", 0.6, 0.6, 10.0, 1),
    EntityType::mob("minecraft:hoglin", 1.4, 1.4, 40.0, 1),
    EntityType::mob("minecraft:piglin", 1.9, 0.6, 16.0, 1),
    EntityType::mob("minecraft:strider", 1.7, 0.9, 20.0, 1),
    EntityType::mob("minecraft:zoglin", 1.4, 1.4, 40.0, 1),
    EntityType::mob("minecraft:warden", 2.9, 0.9, 500.0, 5),
];

/// `EntityIds` lookup. Accepts both spellings PocketMine uses in commands and
/// plugins (`zombie` / `minecraft:zombie`) and is case-insensitive.
pub fn lookup(id: &str) -> Option<&'static EntityType> {
    let wants_namespace = !id.contains(':');
    ENTITY_TYPES.iter().find(|entity_type| {
        if wants_namespace {
            entity_type
                .id
                .strip_prefix("minecraft:")
                .is_some_and(|name| name.eq_ignore_ascii_case(id))
        } else {
            entity_type.id.eq_ignore_ascii_case(id)
        }
    })
}

/// Every spawnable mob, alphabetical, for `/summon` suggestions and tests.
pub fn mob_types() -> impl Iterator<Item = &'static EntityType> {
    ENTITY_TYPES.iter().filter(|entity_type| entity_type.is_mob)
}

/// Mob network id for a `*_spawn_egg` item id (`SpawnEgg` use path).
///
/// Bedrock's spawn eggs are ordinary items named
/// `minecraft:<mob>_spawn_egg`; the map is built from the item registry so a
/// mob without an egg simply has no entry.
pub fn spawn_egg_type(network_id: i16) -> Option<&'static str> {
    spawn_egg_map().get(&network_id).copied()
}

fn spawn_egg_map() -> &'static std::collections::HashMap<i16, &'static str> {
    static MAP: std::sync::OnceLock<std::collections::HashMap<i16, &'static str>> =
        std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        for entity_type in mob_types() {
            let short = entity_type
                .id
                .strip_prefix("minecraft:")
                .unwrap_or(entity_type.id);
            let egg = format!("minecraft:{short}_spawn_egg");
            if let Some(network_id) = crate::inventory::item_network_id(&egg) {
                map.insert(network_id, entity_type.id);
            }
        }
        map
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_with_or_without_namespace_and_ignores_case() {
        assert_eq!(lookup("zombie").unwrap().id, "minecraft:zombie");
        assert_eq!(lookup("minecraft:zombie").unwrap().max_health, 20.0);
        assert_eq!(lookup("ZOMBIE").unwrap().id, "minecraft:zombie");
        assert!(lookup("minecraft:not_a_real_mob").is_none());
    }

    #[test]
    fn entity_ids_are_unique() {
        let mut ids: Vec<&str> = ENTITY_TYPES.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        let unique = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), unique);
    }

    #[test]
    fn projectiles_match_pocketmine_movement_constants() {
        let arrow = lookup("minecraft:arrow").unwrap();
        assert_eq!(arrow.gravity, 0.05);
        assert_eq!(arrow.drag, 0.01);
        assert!(arrow.drag_before_gravity);
        let snowball = lookup("minecraft:snowball").unwrap();
        assert_eq!(snowball.gravity, 0.03);
    }

    #[test]
    fn mob_table_is_not_empty() {
        assert!(mob_types().count() > 40);
    }

    #[test]
    fn spawn_eggs_resolve_to_their_mob() {
        for (egg, mob) in [
            ("minecraft:zombie_spawn_egg", "minecraft:zombie"),
            ("minecraft:creeper_spawn_egg", "minecraft:creeper"),
            ("minecraft:cow_spawn_egg", "minecraft:cow"),
        ] {
            let egg = crate::inventory::item_network_id(egg).expect("the egg is an item");
            assert_eq!(spawn_egg_type(egg), Some(mob));
        }
        // A non-egg item resolves to no mob.
        assert!(spawn_egg_type(1).is_none(), "stone is not a spawn egg");
        // Mobs without an egg item (the player) have no entry.
        assert!(
            crate::inventory::item_network_id("minecraft:player_spawn_egg").is_none()
        );
    }
}
