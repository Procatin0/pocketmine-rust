//! Block and player physics, translated from PocketMine-MP (`target/axolotl-pm`).
//!
//! Ported from:
//! - `src/block/Block.php` (`getSupportType` / `getAdjacentSupportType`),
//!   `src/block/utils/SupportType.php`, `src/block/utils/StaticSupportTrait.php`
//! - `src/block/utils/FallableTrait.php`
//! - `src/block/{Torch,Lantern,Sand,Gravel,ConcretePowder,Anvil,SnowLayer,
//!   Carpet,Flower,Sapling,Crops,BaseRail,Ladder,Vine,SugarCane}.php`
//! - `src/entity/object/FallingBlock.php`
//! - `src/entity/Entity.php` (`tryChangeMovement`, `updateFallState`),
//!   `src/entity/Living.php` (`calculateFallDamage`, `onHitGround`)
//! - `src/player/Player.php` (`checkGroundState`, `calculateFallDamage`)
//!
//! Everything here is name/state driven: the canonical block-state table
//! (`bedrock_world::block_states`) already knows every Bedrock state, so a
//! rule is expressed as "name + property" instead of a per-class trait.
//!
//! # What is intentionally not modelled
//!
//! PocketMine's falling blocks are entities (`FallingBlock`) that collide with
//! other entities and can hurt them (an anvil deals 2 HP per block). This
//! server has no entity simulation outside players, so a falling block is
//! simulated as a block that moves down one cell per tick (`WorldState`), and
//! its landing only applies the block physics (`onHitGround` damage to
//! entities is not applied). See [`FallingBlock`](crate::world_state::FallingBlock).

use bedrock_world::block_states::{self, Value};

/// PocketMine `Facing` constants (Bedrock order, identical to
/// `UseItemTransactionData`'s face field and `PlayerBlockAction::face`).
pub const DOWN: i32 = 0;
pub const UP: i32 = 1;
pub const NORTH: i32 = 2;
pub const SOUTH: i32 = 3;
pub const WEST: i32 = 4;
pub const EAST: i32 = 5;

/// `Facing::ALL`
pub const ALL_FACES: [i32; 6] = [DOWN, UP, NORTH, SOUTH, WEST, EAST];

/// `Facing::opposite()`.
pub fn opposite(face: i32) -> i32 {
    match face {
        DOWN => UP,
        UP => DOWN,
        NORTH => SOUTH,
        SOUTH => NORTH,
        WEST => EAST,
        EAST => WEST,
        _ => DOWN,
    }
}

/// Unit offset of a face (`Vector3::getSide`).
pub fn offset(face: i32) -> (i32, i32, i32) {
    match face {
        UP => (0, 1, 0),
        DOWN => (0, -1, 0),
        NORTH => (0, 0, -1),
        SOUTH => (0, 0, 1),
        WEST => (-1, 0, 0),
        EAST => (1, 0, 0),
        _ => (0, 0, 0),
    }
}

/// `pocketmine\block\utils\SupportType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportType {
    Full,
    Center,
    Edge,
    None,
}

impl SupportType {
    pub fn has_edge_support(self) -> bool {
        matches!(self, SupportType::Edge | SupportType::Full)
    }

    pub fn has_center_support(self) -> bool {
        matches!(self, SupportType::Center | SupportType::Full)
    }
}

fn string_property(id: u32, key: &str) -> Option<&'static str> {
    block_states::state_properties(id)?
        .iter()
        .find(|(name, _)| name == key)
        .and_then(|(_, value)| match value {
            Value::String(text) => Some(text.as_str()),
            _ => None,
        })
}

/// `Block::getSupportType($facing)`.
///
/// PocketMine overrides it for the partial shapes it models
/// (`Slab`, `SnowLayer`, `Stair`, ...); the canonical state table only tells
/// full cubes apart, so this translates the two overrides whose behaviour
/// depends on a state property (slabs) and falls back to
/// `Block::isFullCube()`.
pub fn support_type(id: u32, facing: i32) -> SupportType {
    let Some(name) = block_states::state_name(id) else {
        // Custom/unknown block: the reference default is FULL.
        return SupportType::Full;
    };
    if name.ends_with("_double_slab") {
        // `Slab::getSupportType()`: a double slab is a full cube.
        return SupportType::Full;
    }
    if name.ends_with("_slab") {
        // `Slab::getSupportType()`: a single slab only supports upwards from
        // its solid half (`FULL` towards that face, `NONE` elsewhere).
        return match string_property(id, "minecraft:vertical_half") {
            Some("top") if facing == UP => SupportType::Full,
            Some("top") => SupportType::None,
            _ if facing == DOWN => SupportType::Full,
            _ => SupportType::None,
        };
    }
    if block_states::is_full_cube(name) {
        SupportType::Full
    } else {
        SupportType::None
    }
}

/// Support requirements a block carries (`StaticSupportTrait::canBeSupportedAt`,
/// plus the per-state `onNearbyBlockChange()` of torches and lanterns).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    /// `Torch`: centred support below (`top` variant) or a full wall block.
    Torch,
    /// `Lantern`: centred support below, or above while hanging.
    Lantern,
    /// `Sapling`/`Flower`/grass: dirt-like block below.
    DirtBelow,
    /// `DeadBush`: sand/mud/dirt below.
    SandBelow,
    /// `Crops`: farmland below.
    FarmlandBelow,
    /// `SnowLayer`, rails, signs, banners: full block below.
    FullBelow,
    /// Hanging signs: full block above.
    FullAbove,
    /// `Carpet`: any non-air block below.
    NonAirBelow,
    /// `Ladder`: full block on any horizontal side.
    SideFull,
    /// `Vine`/`GlowLichen`: full block on any side or above.
    AnyFull,
    /// `SugarCane`: sand/dirt below or water beside it.
    SugarCane,
}

/// `BlockTypeTags::DIRT` (plus mud, which the reference also accepts).
const DIRT_LIKE: &[&str] = &[
    "dirt",
    "coarse_dirt",
    "grass_block",
    "podzol",
    "mycelium",
    "rooted_dirt",
    "moss_block",
    "mud",
    "muddy_mangrove_roots",
    "dirt_with_roots",
    "crimson_nylium",
    "warped_nylium",
    "soul_soil",
];

/// `BlockTypeTags::SAND` (+ mud, which `DeadBush` also accepts).
const SAND_LIKE: &[&str] = &[
    "sand",
    "red_sand",
    "suspicious_sand",
    "mud",
    "muddy_mangrove_roots",
    "dirt",
    "coarse_dirt",
    "grass_block",
    "podzol",
    "mycelium",
    "rooted_dirt",
    "moss_block",
];

/// `Flower` (and the single-block flowers the game shares its support rule
/// with).
const FLOWERS: &[&str] = &[
    "dandelion",
    "poppy",
    "blue_orchid",
    "allium",
    "azure_bluet",
    "oxeye_daisy",
    "cornflower",
    "lily_of_the_valley",
    "torchflower",
    "wither_rose",
    "closed_eyeblossom",
    "open_eyeblossom",
    "sunflower",
    "lilac",
    "rose_bush",
    "peony",
    "pink_petals",
];

/// `Crops::canBeSupportedAt()` -> `Farmland`.
const CROPS: &[&str] = &[
    "wheat",
    "carrots",
    "potatoes",
    "beetroots",
    "torchflower_crop",
    "pitcher_crop",
];

fn rule_for(name: &str) -> Option<Rule> {
    if matches!(
        name,
        "torch" | "soul_torch" | "copper_torch" | "underwater_torch"
    ) {
        return Some(Rule::Torch);
    }
    if matches!(name, "lantern" | "soul_lantern" | "copper_lantern") {
        return Some(Rule::Lantern);
    }
    if name == "ladder" {
        return Some(Rule::SideFull);
    }
    if matches!(name, "vine" | "glow_lichen" | "cave_vines") {
        return Some(Rule::AnyFull);
    }
    if name == "sugar_cane" {
        return Some(Rule::SugarCane);
    }
    if name.ends_with("_hanging_sign") {
        return Some(Rule::FullAbove);
    }
    if name == "snow_layer" || name == "lightning_rod" {
        return Some(Rule::FullBelow);
    }
    if name.ends_with("_sign") || name.ends_with("_banner") {
        return Some(Rule::FullBelow);
    }
    if name == "rail" || name.ends_with("_rail") {
        // `BaseRail::onNearbyBlockChange()`: EDGE support below.
        return Some(Rule::FullBelow);
    }
    if name.ends_with("_carpet") {
        return Some(Rule::NonAirBelow);
    }
    if name == "dead_bush" {
        return Some(Rule::SandBelow);
    }
    if name.ends_with("_sapling")
        || matches!(name, "short_grass" | "tall_grass" | "fern" | "large_fern")
    {
        return Some(Rule::DirtBelow);
    }
    if FLOWERS.contains(&name) {
        return Some(Rule::DirtBelow);
    }
    if CROPS.contains(&name) || name.ends_with("_stem") {
        return Some(Rule::FarmlandBelow);
    }
    None
}

/// Whether the block at `id` has any support requirement at all.
pub fn needs_support(id: u32) -> bool {
    block_states::state_name(id)
        .and_then(rule_for)
        .is_some()
}

/// `Block::getAdjacentSupportType($face)`: the neighbour at `$face`'s support
/// towards us (`getSupportType(opposite($face))`).
fn neighbour_supports<F>(face: i32, required: SupportType, at: &F) -> bool
where
    F: Fn(i32, i32, i32) -> u32,
{
    let (dx, dy, dz) = offset(face);
    let support = support_type(at(dx, dy, dz), opposite(face));
    match required {
        SupportType::Full => support == SupportType::Full,
        SupportType::Center => support.has_center_support(),
        SupportType::Edge => support.has_edge_support(),
        SupportType::None => true,
    }
}

fn neighbour_name<F>(face: i32, at: &F) -> Option<&'static str>
where
    F: Fn(i32, i32, i32) -> u32,
{
    let (dx, dy, dz) = offset(face);
    block_states::state_name(at(dx, dy, dz))
}

/// `canBeSupportedAt($block)` for one block: `at(dx, dy, dz)` answers block ids
/// relative to it.
///
/// Unknown blocks are always supported (the reference default), so a custom
/// block is never deleted by the cascade.
pub fn is_supported<F>(id: u32, at: F) -> bool
where
    F: Fn(i32, i32, i32) -> u32,
{
    let Some(name) = block_states::state_name(id) else {
        return true;
    };
    let Some(rule) = rule_for(name) else {
        return true;
    };

    match rule {
        Rule::Torch => {
            // `Torch::onNearbyBlockChange()`: a wall torch is supported by a
            // full block on the face it hangs from, a floor torch by a block
            // with centre support below.
            let direction = string_property(id, "torch_facing_direction").unwrap_or("top");
            let face = match direction {
                "west" => EAST,
                "east" => WEST,
                "north" => SOUTH,
                "south" => NORTH,
                _ => DOWN,
            };
            let required = if face == DOWN {
                SupportType::Center
            } else {
                SupportType::Full
            };
            neighbour_supports(face, required, &at)
        }
        Rule::Lantern => {
            let face = if block_states::bool_property(id, "hanging").unwrap_or(false) {
                UP
            } else {
                DOWN
            };
            neighbour_supports(face, SupportType::Center, &at)
        }
        Rule::DirtBelow => neighbour_name(DOWN, &at).is_some_and(|n| DIRT_LIKE.contains(&n)),
        Rule::SandBelow => neighbour_name(DOWN, &at).is_some_and(|n| SAND_LIKE.contains(&n)),
        Rule::FarmlandBelow => neighbour_name(DOWN, &at) == Some("farmland"),
        Rule::FullBelow => neighbour_supports(DOWN, SupportType::Full, &at),
        Rule::FullAbove => neighbour_supports(UP, SupportType::Full, &at),
        Rule::NonAirBelow => neighbour_name(DOWN, &at).is_some_and(|n| n != "air"),
        Rule::SideFull => [NORTH, SOUTH, WEST, EAST]
            .iter()
            .any(|face| neighbour_supports(*face, SupportType::Full, &at)),
        Rule::AnyFull => [UP, NORTH, SOUTH, WEST, EAST]
            .iter()
            .any(|face| neighbour_supports(*face, SupportType::Full, &at)),
        Rule::SugarCane => {
            // `Sugarcane::onNearbyBlockChange()`: sand/dirt/mud/sugarcane
            // below, *and* water on one of the four horizontal sides.
            let supported = neighbour_name(DOWN, &at).is_some_and(|n| {
                SAND_LIKE.contains(&n) || matches!(n, "sugar_cane" | "farmland")
            });
            supported
                && [NORTH, SOUTH, WEST, EAST]
                    .iter()
                    .any(|face| matches!(neighbour_name(*face, &at), Some("water" | "flowing_water")))
        }
    }
}

/// Lenient placement guard (`Block::canBePlacedAt()` for the blocks using
/// `StaticSupportTrait`).
///
/// The client-chosen state may not carry the final facing yet (`Torch::place`
/// and `Lantern::place` pick one server-side), so a placement is only refused
/// when no face around the target cell could hold the block.
pub fn can_be_placed_at<F>(id: u32, at: F) -> bool
where
    F: Fn(i32, i32, i32) -> u32,
{
    if !needs_support(id) {
        return true;
    }
    if neighbour_supports(DOWN, SupportType::Center, &at)
        || neighbour_supports(UP, SupportType::Center, &at)
    {
        return true;
    }
    [NORTH, SOUTH, WEST, EAST]
        .iter()
        .any(|face| neighbour_supports(*face, SupportType::Full, &at))
}

/// `Fallable` blocks (`FallableTrait`): they leave the world and fall when the
/// cell below can be replaced.
pub fn is_fallable(name: &str) -> bool {
    matches!(
        name,
        "sand"
            | "red_sand"
            | "gravel"
            | "dragon_egg"
            | "suspicious_sand"
            | "suspicious_gravel"
            | "snow_layer"
    ) || name.ends_with("_concrete_powder")
        || name.ends_with("anvil")
}

/// `Block::getFallDamagePerBlock()`: only the anvil hurts (2 HP per block).
pub fn fall_damage_per_block(name: &str) -> f32 {
    if name.ends_with("anvil") {
        2.0
    } else {
        0.0
    }
}

/// `Block::getMaxFallDamage()`.
pub fn max_fall_damage(name: &str) -> f32 {
    if name.ends_with("anvil") {
        40.0
    } else {
        0.0
    }
}

/// `Block::canBeReplaced()` for the blocks `FallableTrait` can fall into.
///
/// Conservative on purpose: a non-replaceable block stops the fall (and the
/// falling block turns back into a block beside it), so a wrong `true` would
/// delete blocks from the world.
pub fn is_replaceable(name: &str) -> bool {
    matches!(
        name,
        "air"
            | "water"
            | "flowing_water"
            | "lava"
            | "flowing_lava"
            | "fire"
            | "soul_fire"
            | "short_grass"
            | "tall_grass"
            | "fern"
            | "large_fern"
            | "dead_bush"
            | "seagrass"
            | "kelp"
            | "vine"
            | "glow_lichen"
            | "snow_layer"
            | "structure_void"
            | "light_block"
            | "bubble_column"
    ) || name.ends_with("_sapling")
        || FLOWERS.contains(&name)
        || CROPS.contains(&name)
}

/// `onEntityInside()` of the blocks that cancel a fall on contact
/// (`Water`, `Lava`, `Cobweb`, `Ladder`, `Vine`, `SweetBerryBush`, ...).
pub fn resets_fall_distance(name: &str) -> bool {
    matches!(
        name,
        "water"
            | "flowing_water"
            | "lava"
            | "flowing_lava"
            | "web"
            | "ladder"
            | "vine"
            | "cave_vines"
            | "cave_vines_body_with_berries"
            | "cave_vines_head_with_berries"
            | "weeping_vines"
            | "twisting_vines"
            | "sweet_berry_bush"
            | "scaffolding"
            | "bubble_column"
            | "powder_snow"
            | "slime_block"
            | "honey_block"
    )
}

/// `Block::onEntityLand()`: what the landing block does to the fall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LandingEffect {
    /// No change (`Block::onEntityLand()` returns null).
    None,
    /// `HayBale`/`Bed`: `$entity->fallDistance *= factor`.
    Multiply(f32),
    /// `Slime`: `$entity->resetFallDistance()`.
    Reset,
}

pub fn on_entity_land(name: &str) -> LandingEffect {
    if name == "hay_block" {
        LandingEffect::Multiply(0.2)
    } else if name.ends_with("_bed") {
        LandingEffect::Multiply(0.5)
    } else if name == "slime_block" {
        LandingEffect::Reset
    } else {
        LandingEffect::None
    }
}

/// `Entity::updateFallState()` distance bookkeeping (without the ground part).
///
/// `dy` is the vertical distance moved this tick (negative when falling). When
/// the entity descends, or ascends back towards the apex, the fall distance
/// grows by the distance; ascending past the apex restarts the measurement.
pub fn update_fall_distance(fall_distance: f32, dy: f32) -> f32 {
    if dy < fall_distance {
        fall_distance - dy
    } else {
        0.0
    }
}

/// `Living::calculateFallDamage()` / `Player::calculateFallDamage()`.
///
/// `ceil($fallDistance - 3 - jumpBoostLevel)`, and nothing while flying.
pub fn calculate_fall_damage(fall_distance: f32, flying: bool, jump_boost_level: i32) -> f32 {
    if flying {
        return 0.0;
    }
    let damage = (fall_distance - 3.0 - jump_boost_level as f32).ceil();
    if damage > 0.0 {
        damage
    } else {
        0.0
    }
}

/// `Player::checkGroundState()`: the player is on the ground when a block with
/// collision intersects the 0.4-high slice of the player's bounding box
/// (`minY = y - 0.2`, `maxY = y + 0.2`, 0.6 wide).
///
/// `at(x, y, z)` answers absolute block ids.
pub fn is_on_ground<F>(x: f32, y: f32, z: f32, at: F) -> bool
where
    F: Fn(i32, i32, i32) -> u32,
{
    const HALF_WIDTH: f32 = 0.3;
    let min_y = (y - 0.2).floor() as i32;
    let max_y = (y + 0.2).floor() as i32;
    let xs = [(x - HALF_WIDTH).floor() as i32, (x + HALF_WIDTH).floor() as i32];
    let zs = [(z - HALF_WIDTH).floor() as i32, (z + HALF_WIDTH).floor() as i32];
    for bx in xs {
        for bz in zs {
            for by in min_y..=max_y {
                let id = at(bx, by, bz);
                if block_states::state_name(id).is_some_and(block_states::has_collision) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> u32 {
        block_states::network_id(name).unwrap_or_else(|| panic!("missing canonical state {name}"))
    }

    fn air() -> u32 {
        id("air")
    }

    #[test]
    fn fall_damage_matches_living_formula() {
        // `ceil($fallDistance - 3 - jumpBoost)`, nothing below/at 3 blocks.
        assert_eq!(calculate_fall_damage(0.0, false, 0), 0.0);
        assert_eq!(calculate_fall_damage(3.0, false, 0), 0.0);
        assert_eq!(calculate_fall_damage(3.5, false, 0), 1.0);
        assert_eq!(calculate_fall_damage(4.2, false, 0), 2.0);
        assert_eq!(calculate_fall_damage(10.0, false, 0), 7.0);
        // Jump Boost absorbs one block per level.
        assert_eq!(calculate_fall_damage(10.0, false, 2), 5.0);
        // `Player::calculateFallDamage()`: flying players never take damage.
        assert_eq!(calculate_fall_damage(30.0, true, 0), 0.0);
    }

    #[test]
    fn fall_distance_grows_while_descending_and_resets_above_the_apex() {
        // Descending 2 blocks grows the distance; ascending 0.5 keeps it.
        let d = update_fall_distance(0.0, -2.0);
        assert_eq!(d, 2.0);
        assert_eq!(update_fall_distance(d, 0.5), 1.5);
        // Ascending past the apex restarts the measurement.
        assert_eq!(update_fall_distance(d, 3.0), 0.0);
    }

    #[test]
    fn only_fallable_blocks_fall() {
        assert!(is_fallable("gravel"));
        assert!(is_fallable("sand"));
        assert!(is_fallable("red_sand"));
        assert!(is_fallable("white_concrete_powder"));
        assert!(is_fallable("anvil"));
        assert!(!is_fallable("stone"));
        assert!(!is_fallable("torch"));
        // Anvil is the only block in the reference that hurts on landing.
        assert_eq!(fall_damage_per_block("anvil"), 2.0);
        assert_eq!(max_fall_damage("anvil"), 40.0);
        assert_eq!(fall_damage_per_block("gravel"), 0.0);
    }

    #[test]
    fn a_torch_needs_centre_support_below() {
        let torch = id("torch");
        let stone = id("stone");
        assert!(is_supported(torch, |dx, dy, dz| if (dx, dy, dz) == (0, -1, 0) {
            stone
        } else {
            air()
        }));
        assert!(!is_supported(torch, |_, _, _| air()));
        // A wall torch is supported by a full block on the wall side.
        let wall = block_states::with_property(torch, "torch_facing_direction", "west")
            .expect("torch has a facing property");
        assert!(is_supported(wall, |dx, dy, dz| if (dx, dy, dz) == (1, 0, 0) {
            stone
        } else {
            air()
        }));
        assert!(!is_supported(wall, |dx, dy, dz| if (dx, dy, dz) == (0, -1, 0) {
            stone
        } else {
            air()
        }));
    }

    #[test]
    fn a_lantern_hangs_from_the_block_above() {
        let lantern = id("lantern");
        let stone = id("stone");
        let hanging = block_states::with_property(lantern, "hanging", "true").unwrap();
        assert!(is_supported(hanging, |dx, dy, dz| if (dx, dy, dz) == (0, 1, 0) {
            stone
        } else {
            air()
        }));
        assert!(!is_supported(hanging, |dx, dy, dz| if (dx, dy, dz) == (0, -1, 0) {
            stone
        } else {
            air()
        }));
        // A standing lantern is the opposite way round.
        let standing = block_states::with_property(lantern, "hanging", "false").unwrap();
        assert!(is_supported(standing, |dx, dy, dz| if (dx, dy, dz) == (0, -1, 0) {
            stone
        } else {
            air()
        }));
    }

    #[test]
    fn crops_need_farmland_and_saplings_need_dirt() {
        let wheat = id("wheat");
        let sapling = id("oak_sapling");
        assert!(is_supported(wheat, |dx, dy, dz| if (dx, dy, dz) == (0, -1, 0) {
            id("farmland")
        } else {
            air()
        }));
        assert!(!is_supported(wheat, |dx, dy, dz| if (dx, dy, dz) == (0, -1, 0) {
            id("stone")
        } else {
            air()
        }));
        assert!(is_supported(sapling, |dx, dy, dz| if (dx, dy, dz) == (0, -1, 0) {
            id("grass_block")
        } else {
            air()
        }));
        assert!(!is_supported(sapling, |dx, dy, dz| if (dx, dy, dz) == (0, -1, 0) {
            id("stone")
        } else {
            air()
        }));
    }

    #[test]
    fn only_replaceable_blocks_let_a_falling_block_through() {
        assert!(is_replaceable("air"));
        assert!(is_replaceable("water"));
        assert!(is_replaceable("short_grass"));
        assert!(!is_replaceable("stone"));
        assert!(!is_replaceable("stone_button"));
        assert!(!is_replaceable("chest"));
    }

    #[test]
    fn support_type_reports_slab_halves_and_full_cubes() {
        let stone = id("stone");
        assert_eq!(support_type(stone, UP), SupportType::Full);
        let bottom = id("oak_slab");
        assert_eq!(support_type(bottom, DOWN), SupportType::Full);
        assert_eq!(support_type(bottom, UP), SupportType::None);
        let top = block_states::with_property(bottom, "minecraft:vertical_half", "top").unwrap();
        assert_eq!(support_type(top, UP), SupportType::Full);
        assert_eq!(support_type(top, DOWN), SupportType::None);
    }

    #[test]
    fn ground_state_matches_a_block_under_the_feet_slice() {
        let stone = id("stone");
        // Feet at y=64.0 standing on stone at y=63.
        assert!(is_on_ground(0.5, 64.0, 0.5, |_, y, _| if y == 63 {
            stone
        } else {
            air()
        }));
        assert!(!is_on_ground(0.5, 64.0, 0.5, |_, _, _| air()));
        // A passable block under the feet is not ground.
        let grass = id("short_grass");
        assert!(!is_on_ground(0.5, 64.0, 0.5, |_, y, _| if y == 63 {
            grass
        } else {
            air()
        }));
    }

    #[test]
    fn landing_effects_come_from_the_reference_blocks() {
        assert_eq!(on_entity_land("hay_block"), LandingEffect::Multiply(0.2));
        assert_eq!(on_entity_land("red_bed"), LandingEffect::Multiply(0.5));
        assert_eq!(on_entity_land("slime_block"), LandingEffect::Reset);
        assert_eq!(on_entity_land("stone"), LandingEffect::None);
        assert!(resets_fall_distance("water"));
        assert!(resets_fall_distance("web"));
        assert!(resets_fall_distance("ladder"));
        assert!(!resets_fall_distance("stone"));
    }
}
