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
//! entities is not applied). See [`EntityKind::FallingBlock`](crate::entity::EntityKind).

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
        // `Slab::getSupportType()`: a bottom slab supports upwards
        // (`FULL` towards UP), a top slab supports downwards.
        return match string_property(id, "minecraft:vertical_half") {
            Some("top") if facing == DOWN => SupportType::Full,
            Some("top") => SupportType::None,
            _ if facing == UP => SupportType::Full,
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
    /// Redstone dust, repeaters, comparators, pressure plates: centred support
    /// below (`onNearbyBlockChange()` breaks them when it disappears).
    CenterBelow,
    /// `Lever::onNearbyBlockChange()`: centred support on the side opposite
    /// `lever_direction` (the lever points away from the block it hangs on).
    Lever,
    /// `Button::onNearbyBlockChange()`: centred support on the side opposite
    /// `facing_direction`. `facing_direction` is the button's outward normal
    /// (the face it points at), so the block it hangs on is behind it.
    Button,
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
        "torch" | "soul_torch" | "copper_torch" | "underwater_torch" | "redstone_torch"
            | "unlit_redstone_torch"
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
    if name == "redstone_wire"
        || matches!(
            name,
            "repeater" | "unpowered_repeater" | "powered_repeater" | "comparator"
                | "unpowered_comparator" | "powered_comparator"
        )
        || name.ends_with("_pressure_plate")
    {
        // `RedstoneWire`/`RedstoneDiode`/`PressurePlate`: the block they sit on
        // is their support, so removing it drops them.
        return Some(Rule::CenterBelow);
    }
    if name == "lever" {
        return Some(Rule::Lever);
    }
    if name.ends_with("_button") {
        return Some(Rule::Button);
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

/// `Torch::place()` / `Lantern::place()`: the concrete state a block takes when
/// placed against `face`.
///
/// Bedrock only sends the held item's own block state
/// (`ItemStack::blockRuntimeId`), and for a torch that state is
/// `torch_facing_direction: "unknown"` - a fixed variant, not the state the
/// placement should produce. PocketMine resolves it server-side in
/// `Block::getStateForPlacement()`; without this every placed torch rendered
/// as a wall torch (the `unknown` state) and stopped noticing that the block
/// under it was gone, which is also why it no longer broke with its support.
///
/// `at(dx, dy, dz)` answers block ids relative to the target cell. Returns
/// `None` when the reference would refuse the placement.
/// `Yaw::getHorizontalFacing()`: the horizontal face a player looks at.
///
/// Bedrock yaw: 0 = south, 90 = west, 180 = north, 270 = east.
pub fn horizontal_facing(yaw: f32) -> i32 {
    let yaw = (yaw.round() as i32).rem_euclid(360);
    match ((yaw + 45) / 90).rem_euclid(4) {
        0 => SOUTH,
        1 => WEST,
        2 => NORTH,
        _ => EAST,
    }
}

/// `Axis::X` of the player's facing: `true` when they look east or west. A
/// floor/ceiling lever uses it to pick its east-west vs north-south base.
pub fn horizontal_axis_is_x(yaw: f32) -> bool {
    matches!(horizontal_facing(yaw), WEST | EAST)
}

/// Face a `lever_direction` state points at (the lever points away from the
/// block it is mounted on).
fn lever_facing(id: u32) -> i32 {
    match string_property(id, "lever_direction").unwrap_or("up_north_south") {
        "down_east_west" | "down_north_south" => DOWN,
        "north" => NORTH,
        "south" => SOUTH,
        "west" => WEST,
        "east" => EAST,
        _ => UP,
    }
}

/// Face a button points at, from its `facing_direction` (`0..=5`, the same
/// order as `Facing`).
///
/// `Button::place()` stores `$this->facing = $face`, the clicked face, which is
/// the outward normal: a floor button is `UP` (1), a ceiling button `DOWN` (0),
/// and the block it is mounted on is at `opposite(facing_direction)`.
fn button_facing(id: u32) -> i32 {
    match block_states::int_property(id, "facing_direction").unwrap_or(1) {
        0 => DOWN,
        2 => NORTH,
        3 => SOUTH,
        4 => WEST,
        5 => EAST,
        _ => UP,
    }
}

fn facing_name(face: i32) -> Option<&'static str> {
    match face {
        NORTH => Some("north"),
        SOUTH => Some("south"),
        WEST => Some("west"),
        EAST => Some("east"),
        _ => None,
    }
}

/// The 2-bit `direction` property used by trapdoors: the client reads it as
/// `0 = south, 1 = west, 2 = north, 3 = east` (the order of the state's value
/// list, same as `minecraft:cardinal_direction`).
///
/// PocketMine's `RuntimeDataDescriber::horizontalFacing` writes the mirrored
/// order (`0 = north, ...`), which turned every placed trapdoor half a turn
/// around.
fn horizontal_facing_value(face: i32) -> Option<&'static str> {
    match face {
        SOUTH => Some("0"),
        WEST => Some("1"),
        NORTH => Some("2"),
        EAST => Some("3"),
        _ => None,
    }
}

/// `weirdo_direction` (`Stairs`): the direction the stairs' full-block side
/// faces, stored as `0 = east, 1 = west, 2 = south, 3 = north`.
fn stairs_direction_value(face: i32) -> Option<&'static str> {
    match face {
        EAST => Some("0"),
        WEST => Some("1"),
        SOUTH => Some("2"),
        NORTH => Some("3"),
        _ => None,
    }
}

/// `Door::place()`: adds the hinge side to a door state already oriented by
/// [`state_for_placement_oriented`].
///
/// `$next->hasSameTypeId($this) || (!$next2->isTransparent() &&
/// $next->isTransparent())`, where `$next` is the block on the left and
/// `$next2` the one on the right (`isTransparent` mapped to !`is_full_cube`).
/// `at(dx, dy, dz)` answers block ids relative to the door's lower half.
pub fn orient_door<F>(state: u32, yaw: f32, at: F) -> Option<u32>
where
    F: Fn(i32, i32, i32) -> u32,
{
    let door_name = block_states::state_name(state)?;
    let facing = horizontal_facing(yaw);
    let side = |face: i32| {
        let (dx, dy, dz) = offset(face);
        let name = block_states::state_name(at(dx, dy, dz)).unwrap_or("air");
        (name == door_name, !block_states::is_full_cube(name))
    };
    let (left_same, left_transparent) = side(rotate_y(facing, false));
    let (_, right_transparent) = side(rotate_y(facing, true));
    let hinge_right = left_same || (left_transparent && !right_transparent);
    block_states::with_property(
        state,
        "door_hinge_bit",
        if hinge_right { "true" } else { "false" },
    )
}

/// `Facing::rotateY($face, $clockwise)`, used by `Door::place()` to pick the
/// hinge side from the block on either hand of the player.
pub fn rotate_y(face: i32, clockwise: bool) -> i32 {
    match (face, clockwise) {
        (NORTH, true) => EAST,
        (NORTH, false) => WEST,
        (EAST, true) => SOUTH,
        (EAST, false) => NORTH,
        (SOUTH, true) => WEST,
        (SOUTH, false) => EAST,
        (WEST, true) => NORTH,
        (WEST, false) => SOUTH,
        (other, _) => other,
    }
}

/// [`state_for_placement`] with the player's yaw, needed by blocks whose
/// placement depends on where the player looks (`Lever::place`).
/// `Block::place()` refuses to place a block whose support would be missing
/// (`Lever::place()`, `Button::place()`, `RedstoneDiode::place()` all return
/// `false`), so the oriented state is only kept when it can stand.
fn supported<F>(state: Option<u32>, at: &F) -> Option<u32>
where
    F: Fn(i32, i32, i32) -> u32,
{
    state.filter(|id| is_supported(*id, at))
}

pub fn state_for_placement_oriented<F>(
    block_id: u32,
    face: i32,
    yaw: f32,
    at: F,
) -> Option<u32>
where
    F: Fn(i32, i32, i32) -> u32,
{
    if let Some(name) = block_states::state_name(block_id) {
        if name == "lever" {
            // `Lever::place()`: a wall lever takes the clicked face; a lever on
            // the floor/ceiling picks its base axis from the player's facing.
            let axis_x = horizontal_axis_is_x(yaw);
            let value = match face {
                UP if axis_x => "up_east_west",
                UP => "up_north_south",
                DOWN if axis_x => "down_east_west",
                DOWN => "down_north_south",
                NORTH => "north",
                SOUTH => "south",
                WEST => "west",
                EAST => "east",
                _ => return None,
            };
            return supported(
                block_states::with_property(block_id, "lever_direction", value)
                    .or(Some(block_id)),
                &at,
            );
        }
        if matches!(
            name,
            "repeater" | "unpowered_repeater" | "powered_repeater" | "comparator"
                | "unpowered_comparator" | "powered_comparator"
        ) {
            // `RedstoneDiode::place()`: `facing = opposite(player facing)`, so
            // the arrow points away from the player instead of always south.
            let output = facing_name(opposite(horizontal_facing(yaw)))?;
            return supported(
                block_states::with_property(block_id, "cardinal_direction", output),
                &at,
            );
        }
        if matches!(name, "piston" | "sticky_piston" | "dispenser" | "dropper") {
            // `FacingTrait::place()` of `Piston`/`Dispenser`: the block faces
            // the face that was clicked.
            return supported(
                block_states::with_property(block_id, "facing_direction", &face.to_string()),
                &at,
            );
        }
        if name.ends_with("_button") {
            // `Button::place()`: `facing = $face` (the clicked face, i.e. the
            // outward normal), and the block it hangs on must be behind it.
            return supported(
                block_states::with_property(block_id, "facing_direction", &face.to_string()),
                &at,
            );
        }
        if name.ends_with("_door") {
            // `Door::place()`: only on the top face, and the door faces where
            // the player looks (`$player->getHorizontalFacing()`). Without
            // this every door kept the fixed default direction, so all of
            // them ended up facing south.
            //
            // Bedrock draws the door a quarter turn clockwise from the
            // `minecraft:cardinal_direction` it carries, so the state has to
            // be the facing turned a quarter turn counter-clockwise: with the
            // raw facing every door came out as if the player had been
            // looking from their own right side.
            if face != UP {
                return None;
            }
            let facing = facing_name(rotate_y(horizontal_facing(yaw), true))?;
            return Some(
                block_states::with_property(block_id, "minecraft:cardinal_direction", facing)?,
            );
        }
        if name == "fence_gate" || name.ends_with("_fence_gate") {
            // `FenceGate::place()`: `$this->facing = $player->getHorizontalFacing()`.
            let facing = facing_name(horizontal_facing(yaw))?;
            return Some(
                block_states::with_property(block_id, "minecraft:cardinal_direction", facing)?,
            );
        }
        if name.ends_with("_stairs") {
            // `Stairs::place()`: `$this->facing = $player->getHorizontalFacing()`,
            // stored in `weirdo_direction` (the full-block side faces the
            // player). Without this every stair kept the default east side.
            // The upper/lower half is resolved by the session, which knows the
            // click position.
            let value = stairs_direction_value(horizontal_facing(yaw))?;
            let state = block_states::with_property(block_id, "weirdo_direction", value)?;
            return Some(match face {
                DOWN => block_states::with_property(state, "upside_down_bit", "true")
                    .unwrap_or(state),
                _ => state,
            });
        }
        if name == "trapdoor" || name.ends_with("_trapdoor") {
            // `Trapdoor::place()`: `facing = opposite($player->getHorizontalFacing())`,
            // stored in the 2-bit `direction` property. The upper/lower half
            // is resolved by the session, which knows the click position.
            //
            // The four `direction` values cover the four orientations, and in
            // game the neighbours of the raw facing came out as a quarter turn
            // to the right (raw), a quarter turn to the left (one turn
            // clockwise) and half a turn (the mirrored table), so the half turn
            // counter-clockwise is the one that lands on the facing.
            let facing = rotate_y(opposite(horizontal_facing(yaw)), false);
            let value = horizontal_facing_value(facing)?;
            let state = block_states::with_property(block_id, "direction", value)?;
            return Some(match face {
                DOWN => block_states::with_property(state, "upside_down_bit", "true")
                    .unwrap_or(state),
                _ => state,
            });
        }
    }
    state_for_placement(block_id, face, at)
}

pub fn state_for_placement<F>(block_id: u32, face: i32, at: F) -> Option<u32>
where
    F: Fn(i32, i32, i32) -> u32,
{
    let Some(name) = block_states::state_name(block_id) else {
        // Custom/unknown block: keep the id the caller brought.
        return Some(block_id);
    };
    if name.ends_with("_leaves") {
        // `Leaves::place()`: leaves a player puts down are marked persistent,
        // so a random tick does not decay them when no log is nearby. Without
        // this, every leaf a builder places falls apart on its own.
        let persistent =
            block_states::with_property(block_id, "persistent_bit", "true").unwrap_or(block_id);
        return Some(
            block_states::with_property(persistent, "update_bit", "true").unwrap_or(persistent),
        );
    }
    if matches!(
        name,
        "torch" | "soul_torch" | "copper_torch" | "underwater_torch"
    ) {
        // `Torch::place()`: `canBeSupportedAt($blockReplace, $face)` for
        // `opposite($facing)`; a centre-support block below, a full block on
        // any horizontal side.
        let supported = |side: i32| {
            let (dx, dy, dz) = offset(side);
            let neighbour = at(dx, dy, dz);
            if side == DOWN {
                support_type(neighbour, UP).has_center_support()
            } else {
                support_type(neighbour, opposite(side)) == SupportType::Full
            }
        };
        // The clicked face first, then the reference's fallback order.
        let facing = if face != DOWN && supported(opposite(face)) {
            Some(face)
        } else {
            [SOUTH, WEST, NORTH, EAST, DOWN]
                .into_iter()
                .find(|side| supported(*side))
                .map(opposite)
        }?;
        let value = match facing {
            UP => "top",
            // `ValueMappings::$torchFacing`: the horizontal directions are
            // serialized flipped (MCPE-152036), so a torch pointing west is
            // stored as `east`.
            WEST => "east",
            EAST => "west",
            NORTH => "south",
            SOUTH => "north",
            _ => return None,
        };
        return Some(
            block_states::with_property(block_id, "torch_facing_direction", value)
                .unwrap_or(block_id),
        );
    }
    if matches!(name, "lantern" | "soul_lantern" | "copper_lantern") {
        // `Lantern::place()`: needs centre support below or above, and hangs
        // whenever it is placed on a ceiling or the floor gives none.
        let down_support = support_type(at(0, -1, 0), UP).has_center_support();
        let up_support = support_type(at(0, 1, 0), DOWN).has_center_support();
        if !down_support && !up_support {
            return None;
        }
        let hanging = face == DOWN || !down_support;
        return Some(
            block_states::with_property(
                block_id,
                "hanging",
                if hanging { "true" } else { "false" },
            )
            .unwrap_or(block_id),
        );
    }
    Some(block_id)
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
            // `ValueMappings::$torchFacing` serializes the *horizontal* facing
            // flipped (MCPE-152036), so the string names the side the torch is
            // attached to, not the way it points.
            let face = match direction {
                "west" => WEST,
                "east" => EAST,
                "north" => NORTH,
                "south" => SOUTH,
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
        Rule::CenterBelow => neighbour_supports(DOWN, SupportType::Center, &at),
        Rule::Lever => {
            let face = lever_facing(id);
            neighbour_supports(opposite(face), SupportType::Center, &at)
        }
        Rule::Button => neighbour_supports(opposite(button_facing(id)), SupportType::Center, &at),
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
        // A wall torch is supported by a full block on the wall side; the
        // state string names that side.
        let wall = block_states::with_property(torch, "torch_facing_direction", "east")
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
        // `Slab::getSupportType()`: a bottom slab supports upwards.
        assert_eq!(support_type(bottom, UP), SupportType::Full);
        assert_eq!(support_type(bottom, DOWN), SupportType::None);
        let top = block_states::with_property(bottom, "minecraft:vertical_half", "top").unwrap();
        assert_eq!(support_type(top, DOWN), SupportType::Full);
        assert_eq!(support_type(top, UP), SupportType::None);
    }

    #[test]
    fn a_placed_torch_takes_its_facing_from_the_clicked_face() {
        let stone = id("stone");
        let torch = id("torch");
        // Clicking the top face of a block: a floor torch supported below.
        let top =
            state_for_placement(torch, UP, |_, dy, _| if dy == -1 { stone } else { air() })
                .expect("placeable on top");
        assert_eq!(
            string_property(top, "torch_facing_direction"),
            Some("top")
        );
        // Clicking a side face: a wall torch hanging from the clicked block.
        // The stored value is the flipped (`ValueMappings::$torchFacing`)
        // horizontal facing, i.e. the side the torch is attached to.
        let wall =
            state_for_placement(torch, WEST, |dx, _, _| if dx == 1 { stone } else { air() })
                .expect("placeable on a wall");
        assert_eq!(
            string_property(wall, "torch_facing_direction"),
            Some("east")
        );
        // That state agrees with `is_supported()`: the block it was clicked
        // against keeps it there, and nothing else does.
        assert!(is_supported(wall, |dx, _, _| if dx == 1 {
            stone
        } else {
            air()
        }));
        assert!(!is_supported(wall, |dx, _, _| if dx == -1 {
            stone
        } else {
            air()
        }));
        // No support anywhere: the reference refuses the placement.
        assert!(state_for_placement(torch, UP, |_, _, _| air()).is_none());
        // Torches may not face DOWN either.
        assert!(state_for_placement(torch, DOWN, |_, _, _| air()).is_none());
    }

    #[test]
    fn a_lantern_hangs_from_a_ceiling_and_stands_on_a_floor() {
        let stone = id("stone");
        let lantern = id("lantern");
        let hanging = state_for_placement(lantern, DOWN, |_, dy, _| if dy == 1 { stone } else { air() })
            .expect("placeable under a block");
        assert_eq!(
            block_states::bool_property(hanging, "hanging"),
            Some(true)
        );
        let floor = state_for_placement(lantern, UP, |_, dy, _| if dy == -1 { stone } else { air() })
            .expect("placeable on the floor");
        assert_eq!(
            block_states::bool_property(floor, "hanging"),
            Some(false)
        );
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

#[cfg(test)]
mod lever_placement_tests {
    use super::*;
    use bedrock_world::block_states;

    fn lever() -> u32 {
        block_states::network_id("lever").unwrap()
    }

    fn direction(id: u32) -> &'static str {
        block_states::string_property(id, "lever_direction").unwrap()
    }

    /// `Lever::place()`: wall levers take the clicked face, floor/ceiling
    /// levers take their base axis from the player's facing (yaw 90 = west).
    #[test]
    fn levers_take_their_direction_from_the_clicked_face() {
        let at = |_, _, _| block_states::network_id("stone").unwrap();
        assert_eq!(direction(state_for_placement_oriented(lever(), NORTH, 0.0, at).unwrap()), "north");
        assert_eq!(direction(state_for_placement_oriented(lever(), EAST, 0.0, at).unwrap()), "east");
        assert_eq!(
            direction(state_for_placement_oriented(lever(), UP, 90.0, at).unwrap()),
            "up_east_west"
        );
        assert_eq!(
            direction(state_for_placement_oriented(lever(), UP, 0.0, at).unwrap()),
            "up_north_south"
        );
        assert_eq!(
            direction(state_for_placement_oriented(lever(), DOWN, 270.0, at).unwrap()),
            "down_east_west"
        );
    }
}

#[cfg(test)]
mod redstone_support_tests {
    use super::*;
    use std::sync::Arc;

    use bedrock_world::block_id::AIR;
    use bedrock_world::generator::flat::FlatGenerator;

    use crate::world_state::WorldState;

    fn world() -> WorldState {
        WorldState::new("world", Arc::new(FlatGenerator::new()))
    }

    fn id(name: &str) -> u32 {
        block_states::network_id(name).unwrap_or_else(|| panic!("no default state for {name}"))
    }

    fn cardinal(id: u32) -> &'static str {
        block_states::string_property(id, "cardinal_direction").unwrap()
    }

    /// Every redstone component sits on its support block; removing the
    /// support must drop the component (PM `onNearbyBlockChange()` ->
    /// `useBreakOn()`), instead of leaving it floating.
    #[test]
    fn redstone_components_break_with_their_support() {
        let world = world();
        let cases: [(&str, Option<(&str, &str)>); 6] = [
            ("redstone_wire", None),
            ("unpowered_repeater", None),
            ("unpowered_comparator", None),
            ("wooden_pressure_plate", None),
            ("lever", Some(("lever_direction", "up_north_south"))),
            ("stone_button", Some(("facing_direction", "1"))),
        ];
        for (index, (name, property)) in cases.into_iter().enumerate() {
            let x = index as i32 * 2;
            let support = (x, 4, 0);
            let above = (x, 5, 0);
            world.set_block(support.0, support.1, support.2, id("stone"));
            let mut component = id(name);
            if let Some((key, value)) = property {
                component = block_states::with_property(component, key, value).unwrap();
            }
            world.set_block(above.0, above.1, above.2, component);
            assert_eq!(
                world.block_at(above.0, above.1, above.2),
                component,
                "{name} was placed"
            );

            world.set_block(support.0, support.1, support.2, AIR);
            assert_eq!(
                world.block_at(above.0, above.1, above.2),
                AIR,
                "{name} must break with its support"
            );
        }
    }

    /// `facing_direction` is the button's outward normal, so the only accepted
    /// support is the block behind it: a ceiling button cannot hang off the
    /// floor block below it.
    #[test]
    fn a_button_hangs_only_off_the_block_behind_it() {
        let world = world();
        let ceiling =
            block_states::with_property(id("stone_button"), "facing_direction", "0").unwrap();
        world.set_block(0, 4, 0, id("stone"));
        world.set_block(0, 5, 0, ceiling);
        assert_eq!(
            world.block_at(0, 5, 0),
            AIR,
            "a downward button needs a block above it"
        );

        world.set_block(0, 6, 0, id("stone"));
        world.set_block(0, 5, 0, ceiling);
        assert_eq!(world.block_at(0, 5, 0), ceiling);

        // PM `Button::place()` returns false instead of placing it unsupported.
        let at = |_, _, _| AIR;
        assert_eq!(state_for_placement_oriented(id("stone_button"), UP, 0.0, at), None);
    }

    /// Repeaters/comparators face away from the player (`RedstoneDiode::place`)
    /// and buttons take the clicked face, instead of always facing south/down.
    #[test]
    fn diodes_and_buttons_take_the_player_facing() {
        let at = |_, _, _| id("stone");
        let repeater = id("unpowered_repeater");
        // yaw 0 = facing south -> the diode points north.
        assert_eq!(cardinal(state_for_placement_oriented(repeater, UP, 0.0, at).unwrap()), "north");
        assert_eq!(cardinal(state_for_placement_oriented(repeater, UP, 180.0, at).unwrap()), "south");
        assert_eq!(cardinal(state_for_placement_oriented(repeater, UP, 90.0, at).unwrap()), "east");
        assert_eq!(cardinal(state_for_placement_oriented(repeater, UP, 270.0, at).unwrap()), "west");

        // Pistons and dispensers face the clicked face as well.
        let piston = id("piston");
        let placed = state_for_placement_oriented(piston, EAST, 0.0, at).unwrap();
        assert_eq!(block_states::int_property(placed, "facing_direction"), Some(5));
        let placed = state_for_placement_oriented(piston, UP, 0.0, at).unwrap();
        assert_eq!(block_states::int_property(placed, "facing_direction"), Some(1));

        let button = id("stone_button");
        let placed = state_for_placement_oriented(button, NORTH, 0.0, at).unwrap();
        assert_eq!(block_states::int_property(placed, "facing_direction"), Some(2));
        let placed = state_for_placement_oriented(button, UP, 0.0, at).unwrap();
        assert_eq!(block_states::int_property(placed, "facing_direction"), Some(1));
    }
}
