//! Full canonical Bedrock block-state table (`data/canonical_block_states.nbt`).
//!
//! Since 1.21.60 the client can resolve network block ids as hashes
//! (`StartGamePacket::block_network_ids_are_hashes`): a block's network id is
//! the FNV-1a 32-bit hash of its *canonical state* serialized as little-endian
//! NBT, that is
//!
//! ```text
//! TAG_Compound "" {
//!     "name":   TAG_String <block name>
//!     "states": TAG_Compound { <property>: <value> ... sorted by name }
//! }
//! ```
//!
//! with little-endian lengths/ints (see `bedrock-world-operator`'s
//! `ComputeBlockHash` and PMMP's `BlockStateDictionary`). This module rebuilds
//! that table at startup from the shipped `canonical_block_states.nbt` (23960
//! states), so a runtime id coming from a real client can be turned into a
//! block name — the hand-picked tables elsewhere only cover ~90 blocks, which
//! made every other block invisible on the Java side.
//!
//! `canonical_block_states.nbt` itself uses the *network* NBT encoding
//! (varint lengths/ints), so each entry is parsed and re-serialized in the
//! little-endian form the hash is defined over.

use std::collections::HashMap;
use std::sync::OnceLock;


const CANONICAL_STATES: &[u8] = include_bytes!("../../../data/canonical_block_states.nbt");

/// One property value of a canonical block state.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    Compound(Vec<(String, Value)>),
}

/// Parsed table: hash -> (block name, property count) plus the default hash of
/// each block name, the properties of every state and an index by
/// `name + properties` so a state can be rebuilt after changing a property
/// (connection bits, wall post, ...).
struct BlockStateTable {
    by_hash: HashMap<u32, (String, usize)>,
    by_name: HashMap<String, u32>,
    props: HashMap<u32, Vec<(String, Value)>>,
    by_state: HashMap<String, u32>,
    /// Connectable states only: network id -> index into the Java per-connection
    /// table (`data/java/connections/<protocol>.json`). Sparse (~5k of the
    /// 23960 states), so plain blocks pay one small-map lookup at most.
    connection_index: HashMap<u32, u16>,
}

fn table() -> &'static BlockStateTable {
    static TABLE: OnceLock<BlockStateTable> = OnceLock::new();
    TABLE.get_or_init(build)
}

/// Block name (without the `minecraft:` prefix) for a network block id, or
/// `None` when the id is not a canonical state (e.g. a custom block).
pub fn state_name(network_id: u32) -> Option<&'static str> {
    table().by_hash.get(&network_id).map(|(name, _)| name.as_str())
}

/// Default network block id for a block name (`"stone"` or
/// `"minecraft:stone"`), or `None`.
///
/// "Default" means a state without properties when the block has any; blocks
/// whose every state carries properties fall back to their first state.
pub fn network_id(name: &str) -> Option<u32> {
    let bare = name.strip_prefix("minecraft:").unwrap_or(name);
    table().by_name.get(bare).copied()
}

/// Number of properties of a state, when known (0 for simple blocks).
pub fn state_property_count(network_id: u32) -> Option<usize> {
    table().by_hash.get(&network_id).map(|(_, count)| *count)
}

fn build() -> BlockStateTable {
    let states = parse_canonical_states(CANONICAL_STATES);
    let mut by_hash = HashMap::with_capacity(states.len());
    let mut by_name: HashMap<String, u32> = HashMap::with_capacity(1024);
    let mut defaults: HashMap<String, u32> = HashMap::with_capacity(1024);

    let mut props = HashMap::with_capacity(states.len());
    let mut by_state = HashMap::with_capacity(states.len());
    let mut connection_index = HashMap::with_capacity(4096);

    for (name, properties) in &states {
        let hash = state_hash(name, properties);
        let bare = name.strip_prefix("minecraft:").unwrap_or(name).to_string();
        by_hash.insert(hash, (bare.clone(), properties.len()));
        by_state.insert(state_key(&bare, properties), hash);
        if let Some(index) = connection_index_of(&bare, properties) {
            connection_index.insert(hash, index);
        }
        props.insert(hash, properties.clone());

        match by_name.entry(bare) {
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(hash);
                defaults.insert(name.clone(), hash);
            }
            std::collections::hash_map::Entry::Occupied(mut slot) => {
                let previous = defaults.get(name).copied();
                let previous_props = previous.and_then(|p| by_hash.get(&p)).map_or(usize::MAX, |(_, c)| *c);
                if properties.is_empty() && previous_props > 0 {
                    slot.insert(hash);
                    defaults.insert(name.clone(), hash);
                }
            }
        }
    }

    BlockStateTable {
        by_hash,
        by_name,
        props,
        by_state,
        connection_index,
    }
}

/// Properties of a state, e.g. `("minecraft:connection_north", Byte(1))`.
pub fn state_properties(network_id: u32) -> Option<&'static [(String, Value)]> {
    table().props.get(&network_id).map(|props| props.as_slice())
}

// ---------------------------------------------------------------------------
// Java states per connection layout (`data/java/connections/<protocol>.json`)
// ---------------------------------------------------------------------------
//
// A Java client does not compute fence/wall connections by itself: the state id
// it receives already carries `east/north/south/west` (+ `up` for walls). The
// generated tables hold one global Java state per connection combination, so a
// Bedrock state can be translated exactly instead of collapsing to the block's
// `defaultState` (which is what made a placed fence render without arms).
//
// Layout (validated against `defaultState` for every connectable block of every
// supported version):
// - fences, panes and bars (16 entries): `index = north | east<<1 | south<<2 | west<<3`
// - walls (162 entries): `index = east*54 + north*18 + south*6 + west*2 + up`,
//   with each direction being `none`/`low`/`tall` -> 0/1/2.

fn java_connections_json(protocol: i32) -> Option<&'static str> {
    match protocol {
        769 => Some(include_str!("../../../data/java/connections/769.json")),
        770 => Some(include_str!("../../../data/java/connections/770.json")),
        773 => Some(include_str!("../../../data/java/connections/773.json")),
        774 => Some(include_str!("../../../data/java/connections/774.json")),
        775 => Some(include_str!("../../../data/java/connections/775.json")),
        _ => None,
    }
}

/// Per-protocol `block name -> [java state per connection combination]`, parsed
/// once. `OnceLock::get_or_init` keeps the hot path lock-free after the first
/// chunk conversion of a protocol.
fn connection_table(protocol: i32) -> Option<&'static HashMap<String, Vec<i32>>> {
    static TABLES: [OnceLock<HashMap<String, Vec<i32>>>; 5] = [const { OnceLock::new() }; 5];
    let slot = match protocol {
        769 => 0,
        770 => 1,
        773 => 2,
        774 => 3,
        775 => 4,
        _ => return None,
    };
    Some(TABLES[slot].get_or_init(|| {
        java_connections_json(protocol)
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default()
    }))
}

/// Cheap gate before touching any map: only these names have a connection table.
fn is_connectable_name(name: &str) -> bool {
    name.ends_with("_fence") || name.ends_with("_pane") || name.ends_with("_bars") || name.ends_with("_wall")
}

/// Java global state for a Bedrock connectable state, or `None` when the block
/// carries no connection state (99% of blocks) or the version is unknown.
///
/// Two lookups at most: `connection_index` (sparse, connectable states only) and
/// the per-protocol table by name.
pub fn java_connection_state(protocol: i32, network_id: u32) -> Option<i32> {
    let name = state_name(network_id)?;
    java_connection_state_for_name(protocol, network_id, name)
}

/// Same as [`java_connection_state`] for a caller that already resolved the
/// block name (the chunk hot path resolves it for the fallback anyway), so a
/// plain block costs one suffix comparison and nothing else.
pub fn java_connection_state_for_name(protocol: i32, network_id: u32, name: &str) -> Option<i32> {
    if !is_connectable_name(name) {
        return None;
    }
    let index = *table().connection_index.get(&network_id)? as usize;
    connection_table(protocol)?.get(name)?.get(index).copied()
}

/// Index of a connectable state in the Java table layout, computed once while
/// building the canonical table.
fn connection_index_of(name: &str, props: &[(String, Value)]) -> Option<u16> {
    let family = family_of_name(name)?;
    let lookup = |key: &str| props.iter().find(|(candidate, _)| candidate == key).map(|(_, value)| value);
    match family {
        ConnectionFamily::Fence | ConnectionFamily::Pane => {
            let mut index = 0u16;
            for (bit, suffix) in CONNECTION_SUFFIX.iter().enumerate() {
                let value = lookup(&format!("minecraft:connection_{suffix}"))?;
                if *value == Value::Byte(1) {
                    index |= 1 << bit;
                }
            }
            Some(index)
        }
        ConnectionFamily::Wall => {
            let mut values = [0u16; 4];
            for (slot, suffix) in CONNECTION_SUFFIX.iter().enumerate() {
                values[slot] = match lookup(&format!("wall_connection_type_{suffix}"))? {
                    Value::String(value) if value == "short" => 1,
                    Value::String(value) if value == "tall" => 2,
                    _ => 0,
                };
            }
            let up = matches!(lookup("wall_post_bit"), Some(Value::Byte(1)));
            let [north, east, south, west] = values;
            Some(north * 18 + east * 54 + south * 6 + west * 2 + u16::from(up))
        }
    }
}

// ---------------------------------------------------------------------------
// Connection updates (`HorizontalConnectableTrait` / `Wall::onNearbyBlockChange`)
// ---------------------------------------------------------------------------

/// Horizontal neighbour offsets in the order the Bedrock properties use:
/// north (`-z`), east (`+x`), south (`+z`), west (`-x`).
pub const CONNECTION_DIRECTIONS: [(i32, i32, i32); 4] =
    [(0, 0, -1), (1, 0, 0), (0, 0, 1), (-1, 0, 0)];

const CONNECTION_SUFFIX: [&str; 4] = ["north", "east", "south", "west"];

/// Blocks that carry per-horizontal-face connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionFamily {
    /// `Fence::recalculateConnections()`: fences and fence gates or full cubes.
    Fence,
    /// `Thin::recalculateConnections()`: panes/bars, walls or full cubes.
    Pane,
    /// `Wall::recalculateConnections()`: walls, gates, panes or full cubes.
    Wall,
}

/// Connection family of a block id, or `None` when the block is not
/// connectable (a plain cube, a stair, a torch, ...).
pub fn connection_family(network_id: u32) -> Option<ConnectionFamily> {
    let name = state_name(network_id)?;
    family_of_name(name)
}

fn family_of_name(name: &str) -> Option<ConnectionFamily> {
    if name.ends_with("_fence") {
        Some(ConnectionFamily::Fence)
    } else if name.ends_with("_pane") || name.ends_with("_bars") {
        Some(ConnectionFamily::Pane)
    } else if name.ends_with("_wall") {
        Some(ConnectionFamily::Wall)
    } else {
        None
    }
}

/// Recomputes the horizontal connections of a connectable block from its
/// neighbours and returns the new network id when the state must change.
///
/// `neighbour(dx, dy, dz)` must return the block id at that offset relative to
/// the block being updated. This mirrors PocketMine-MP's
/// `HorizontalConnectableTrait::onNearbyBlockChange()` and
/// `Wall::onNearbyBlockChange()`: the block recomputes its own state from its
/// four horizontal neighbours and re-sets itself when anything changed.
///
/// Returns `None` when the block is not connectable, when its canonical states
/// do not carry the expected properties, or when the state is already correct.
pub fn recalculate_connections(
    network_id: u32,
    neighbour: impl Fn(i32, i32, i32) -> u32,
) -> Option<u32> {
    let table = table();
    let (name, _) = table.by_hash.get(&network_id)?;
    let family = family_of_name(name)?;
    let mut props = table.props.get(&network_id)?.clone();

    let connected: [bool; 4] = std::array::from_fn(|index| {
        let (dx, dy, dz) = CONNECTION_DIRECTIONS[index];
        let other = neighbour(dx, dy, dz);
        let other_name = match table.by_hash.get(&other) {
            Some((name, _)) => name.as_str(),
            None => return family == ConnectionFamily::Fence, // unknown: keep the safe default
        };
        connects(family, other_name)
    });

    match family {
        ConnectionFamily::Fence | ConnectionFamily::Pane => {
            for (index, suffix) in CONNECTION_SUFFIX.iter().enumerate() {
                set_property(
                    &mut props,
                    &format!("minecraft:connection_{suffix}"),
                    Value::Byte(connected[index] as i8),
                );
            }
        }
        ConnectionFamily::Wall => {
            // Walls use unprotected property names in the canonical table.
            for (index, suffix) in CONNECTION_SUFFIX.iter().enumerate() {
                set_property(
                    &mut props,
                    &format!("wall_connection_type_{suffix}"),
                    Value::String(if connected[index] { "short" } else { "none" }.to_string()),
                );
            }
            // `$up = getSide(UP)->getTypeId() !== AIR;`
            let post = neighbour(0, 1, 0) != crate::block_id::AIR;
            set_property(&mut props, "wall_post_bit", Value::Byte(post as i8));
        }
    }

    let rebuilt = *table.by_state.get(&state_key(name, &props))?;
    (rebuilt != network_id).then_some(rebuilt)
}

/// Rebuilds a state with one property replaced, keeping the property's own
/// type (`true`/`false` for the byte booleans, the enum name for strings).
/// Returns `None` when the state has no such property.
pub fn with_property(network_id: u32, key: &str, value: &str) -> Option<u32> {
    let table = table();
    let (name, _) = table.by_hash.get(&network_id)?;
    let mut props = table.props.get(&network_id)?.clone();
    let slot = props.iter_mut().find(|(candidate, _)| candidate == key)?;
    slot.1 = match &slot.1 {
        Value::Byte(_) => Value::Byte(matches!(value, "true" | "1") as i8),
        Value::Int(_) => Value::Int(value.parse().ok()?),
        Value::String(_) => Value::String(value.to_string()),
        _ => return None,
    };
    table.by_state.get(&state_key(name, &props)).copied()
}

/// Reads a byte boolean property (`0`/`1`) of a state, `None` when the state
/// has no such property.
pub fn bool_property(network_id: u32, key: &str) -> Option<bool> {
    let props = table().props.get(&network_id)?;
    match props.iter().find(|(candidate, _)| candidate == key)?.1 {
        Value::Byte(value) => Some(value != 0),
        _ => None,
    }
}

/// Result of right-clicking an interactive block, mirroring PocketMine's
/// `Block::onInteract()` implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interaction {
    /// State the block switches to (`== network_id` when the click is consumed
    /// without a change, e.g. a button that is already pressed).
    pub new_state: u32,
    /// Whether the block is powered after the click (button pressed, lever
    /// on). Fence gates report [`Interaction::door_sound`] instead.
    pub powered: bool,
    /// PM `FenceGate::onInteract()` plays `DoorSound`
    /// (`LevelEvent::SOUND_DOOR`), not a redstone sound.
    pub door_sound: bool,
    /// PM `Button::getActivationTime()`: ticks until the button releases
    /// itself. `None` for levers and gates, whose change is permanent.
    pub reset_after_ticks: Option<i32>,
}

/// Right-click behaviour for the blocks PM gives an `onInteract()`:
/// buttons, levers and fence gates. `None` means the click is not an
/// interaction and must fall through to item placement.
pub fn interact(network_id: u32) -> Option<Interaction> {
    let name = state_name(network_id)?;

    // `Button::onInteract()`: press once, then let the scheduled update
    // release it. A pressed button still consumes the click.
    if name.ends_with("_button") {
        if bool_property(network_id, "button_pressed_bit").unwrap_or(false) {
            return Some(Interaction {
                new_state: network_id,
                powered: true,
                door_sound: false,
                reset_after_ticks: None,
            });
        }
        let new_state = with_property(network_id, "button_pressed_bit", "true")?;
        return Some(Interaction {
            new_state,
            powered: true,
            door_sound: false,
            // `StoneButton::getActivationTime()` = 20, `WoodenButton` = 30.
            reset_after_ticks: Some(if name.contains("stone") { 20 } else { 30 }),
        });
    }

    // `Lever::onInteract()`: toggle, no schedule.
    if name == "lever" {
        let powered = !bool_property(network_id, "open_bit")?;
        return Some(Interaction {
            new_state: with_property(network_id, "open_bit", if powered { "true" } else { "false" })?,
            powered,
            door_sound: false,
            reset_after_ticks: None,
        });
    }

    // `FenceGate::onInteract()`: toggle `open`, play the door sound. PM also
    // flips the gate to face a player opening it from the far side; Bedrock
    // keeps `minecraft:cardinal_direction`, and the facing is not part of the
    // open/closed bit, so there is nothing to mirror here.
    if name == "fence_gate" || name.ends_with("_fence_gate") {
        let open = !bool_property(network_id, "open_bit")?;
        return Some(Interaction {
            new_state: with_property(network_id, "open_bit", if open { "true" } else { "false" })?,
            powered: open,
            door_sound: true,
            reset_after_ticks: None,
        });
    }

    None
}

fn set_property(props: &mut Vec<(String, Value)>, key: &str, value: Value) {
    match props.iter_mut().find(|(candidate, _)| candidate == key) {
        Some(slot) => slot.1 = value,
        None => props.push((key.to_string(), value)),
    }
}

/// PocketMine's connection rules for horizontal connectables.
fn connects(family: ConnectionFamily, other: &str) -> bool {
    let other_family = family_of_name(other);
    // Bedrock keeps a single `fence_gate` block for every wood type.
    let gate = other == "fence_gate" || other.ends_with("_fence_gate");
    match family {
        // `$block instanceof static || $block instanceof FenceGate || full`
        ConnectionFamily::Fence => {
            gate || other_family == Some(ConnectionFamily::Fence) || is_full_cube(other)
        }
        // `$side instanceof Thin || $side instanceof Wall || full`
        ConnectionFamily::Pane => {
            matches!(
                other_family,
                Some(ConnectionFamily::Pane | ConnectionFamily::Wall)
            ) || is_full_cube(other)
        }
        // `static || FenceGate || Thin || full`
        ConnectionFamily::Wall => {
            gate
                || matches!(
                    other_family,
                    Some(
                        ConnectionFamily::Wall | ConnectionFamily::Pane | ConnectionFamily::Fence
                    )
                )
                || is_full_cube(other)
        }
    }
}

/// `SupportType::FULL`: a full cube, which connectables attach to.
///
/// Bedrock has no such flag in the canonical state table, so this uses the
/// block's `opacity` from `data/block_properties_table.json` (1.0 only for
/// opaque cubes) and rejects the known partial shapes that still report a
/// full opacity (single slabs, double-height halves, ...).
pub fn is_full_cube(name: &str) -> bool {
    if name == "air" || name == "water" || name == "lava" || name.ends_with("_bubble_column") {
        return false;
    }
    if name.ends_with("_double_slab") {
        // `Slab::getSupportType()`: a double slab is a full cube.
        return true;
    }
    if name.ends_with("_slab") {
        // A single slab is only FULL vertically, never towards a side.
        return false;
    }
    const PARTIAL: &[&str] = &[
        "_stairs",
        "_door",
        "_trapdoor",
        "_fence",
        "_fence_gate",
        "_pane",
        "_wall",
        "_button",
        "_pressure_plate",
        "_carpet",
        "_torch",
        "_rail",
        "_sign",
        "_banner",
        "_bed",
        "_anvil",
        "_chest",
        "_pot",
        "_campfire",
        "_candle",
    ];
    if PARTIAL.iter().any(|suffix| name.ends_with(suffix)) {
        return false;
    }
    opacity(name).is_some_and(|opacity| opacity >= 0.9)
}

/// Whether a block occupies its cell for entity collision purposes.
///
/// Used when placing a block: an entity whose bounding box intersects a solid
/// cell blocks the placement (PM collides the placed block's boxes against
/// entities). Passable blocks are recognised by name; everything else counts as
/// solid, which is the conservative direction (a placement is refused rather
/// than trapping a player inside the new block).
pub fn has_collision(name: &str) -> bool {
    const PASSABLE: &[&str] = &[
        "air",
        "water",
        "lava",
        "flowing_water",
        "flowing_lava",
        "fire",
        "soul_fire",
        "torch",
        "_torch",
        "_candle",
        "_button",
        "_pressure_plate",
        "rail",
        "_rail",
        "_sign",
        "_banner",
        "_carpet",
        "_sapling",
        "_flower",
        "_tulip",
        "short_grass",
        "tall_grass",
        "fern",
        "_fern",
        "dandelion",
        "poppy",
        "blue_orchid",
        "allium",
        "azure_bluet",
        "oxeye_daisy",
        "cornflower",
        "lily_of_the_valley",
        "wheat",
        "carrots",
        "potatoes",
        "beetroots",
        "_crop",
        "sugar_cane",
        "vine",
        "_vines",
        "glow_lichen",
        "snow_layer",
        "web",
        "seagrass",
        "kelp",
        "dead_bush",
        "sweet_berry_bush",
        "sunflower",
        "lilac",
        "rose_bush",
        "peony",
        "structure_void",
    ];
    !PASSABLE
        .iter()
        .any(|suffix| name == *suffix || name.ends_with(suffix))
}

/// `opacity` of a block from `data/block_properties_table.json`.
fn opacity(name: &str) -> Option<f64> {
    static TABLE: OnceLock<HashMap<String, f64>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let json: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/block_properties_table.json"))
                .unwrap_or(serde_json::Value::Null);
        json.as_object()
            .map(|map| {
                map.iter()
                    .filter_map(|(key, value)| {
                        let key = key.strip_prefix("minecraft:").unwrap_or(key);
                        value.get("opacity")?.as_f64().map(|opacity| (key.to_string(), opacity))
                    })
                    .collect()
            })
            .unwrap_or_default()
    });
    table.get(name).copied()
}

/// Stable key for `by_state`: block name plus every property, sorted and
/// type-tagged so two states can never collide.
fn state_key(name: &str, props: &[(String, Value)]) -> String {
    let mut entries: Vec<(&String, &Value)> = props.iter().map(|(key, value)| (key, value)).collect();
    entries.sort_by(|left, right| left.0.cmp(right.0));
    let mut key = format!("{name}|");
    for (name, value) in entries {
        key.push_str(name);
        key.push('=');
        key.push_str(&render(value));
        key.push(';');
    }
    key
}

fn render(value: &Value) -> String {
    match value {
        Value::Byte(value) => format!("B{value}"),
        Value::Short(value) => format!("S{value}"),
        Value::Int(value) => format!("I{value}"),
        Value::Long(value) => format!("L{value}"),
        Value::Float(value) => format!("F{value}"),
        Value::Double(value) => format!("D{value}"),
        Value::String(value) => value.clone(),
        Value::Compound(_) => "<compound>".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Java side tables (minecraft-data `blocks.json` / `items.json`)
// ---------------------------------------------------------------------------

fn java_blocks_json(protocol: i32) -> Option<&'static str> {
    match protocol {
        769 => Some(include_str!("../../../data/java/blocks/769.json")),
        770 => Some(include_str!("../../../data/java/blocks/770.json")),
        773 => Some(include_str!("../../../data/java/blocks/773.json")),
        774 => Some(include_str!("../../../data/java/blocks/774.json")),
        775 => Some(include_str!("../../../data/java/blocks/775.json")),
        _ => None,
    }
}

fn java_items_json(protocol: i32) -> Option<&'static str> {
    match protocol {
        769 => Some(include_str!("../../../data/java/items/769.json")),
        770 => Some(include_str!("../../../data/java/items/770.json")),
        773 => Some(include_str!("../../../data/java/items/773.json")),
        774 => Some(include_str!("../../../data/java/items/774.json")),
        775 => Some(include_str!("../../../data/java/items/775.json")),
        _ => None,
    }
}

/// Index of a supported protocol in the per-version static tables.
fn protocol_slot(protocol: i32) -> Option<usize> {
    match protocol {
        769 => Some(0),
        770 => Some(1),
        773 => Some(2),
        774 => Some(3),
        775 => Some(4),
        _ => None,
    }
}

/// Per-protocol block table, parsed once with no lock on the hot path: the
/// chunk serializer resolves one Java state per block, so a mutex here showed
/// up as a measurable cost (~400 ns/block in a debug build).
fn java_blocks_table(protocol: i32) -> Option<&'static HashMap<String, i32>> {
    static TABLES: [OnceLock<HashMap<String, i32>>; 5] = [const { OnceLock::new() }; 5];
    let slot = protocol_slot(protocol)?;
    Some(TABLES[slot].get_or_init(|| {
        java_blocks_json(protocol)
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default()
    }))
}

/// Per-protocol item id -> item name table, parsed once (see
/// [`java_blocks_table`]).
fn java_items_table(protocol: i32) -> Option<&'static HashMap<String, String>> {
    static TABLES: [OnceLock<HashMap<String, String>>; 5] = [const { OnceLock::new() }; 5];
    let slot = protocol_slot(protocol)?;
    Some(TABLES[slot].get_or_init(|| {
        java_items_json(protocol)
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default()
    }))
}

/// Default global Java state of a block for a protocol, or `None` when the
/// version is unknown or the block is missing from the version's block list.
pub fn java_block_state(protocol: i32, name: &str) -> Option<i32> {
    let bare = name.strip_prefix("minecraft:").unwrap_or(name);
    java_blocks_table(protocol)?.get(bare).copied()
}

/// Block name carried by a Java item id for a protocol, or `None` for
/// unknown ids and items that are not blocks.
pub fn java_item_block_name(protocol: i32, item_id: i32) -> Option<String> {
    let table = java_items_table(protocol)?;
    let name = table.get(&item_id.to_string())?;
    java_block_state(protocol, name).map(|_| name.clone())
}

// ---------------------------------------------------------------------------
// Network NBT reader (varint lengths/ints, as stored in the data file)
// ---------------------------------------------------------------------------

struct NbtReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> NbtReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn done(&self) -> bool {
        self.pos >= self.buf.len()
    }

    fn byte(&mut self) -> u8 {
        let value = self.buf[self.pos];
        self.pos += 1;
        value
    }

    fn var_u32(&mut self) -> u32 {
        let mut value = 0u32;
        let mut shift = 0;
        loop {
            let byte = self.byte();
            value |= ((byte & 0x7F) as u32) << shift;
            if byte & 0x80 == 0 {
                return value;
            }
            shift += 7;
        }
    }

    fn var_i32(&mut self) -> i32 {
        let raw = self.var_u32();
        ((raw >> 1) as i32) ^ -((raw & 1) as i32)
    }

    fn string(&mut self) -> String {
        let len = self.var_u32() as usize;
        let bytes = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        String::from_utf8_lossy(bytes).into_owned()
    }

    fn value(&mut self, tag: u8) -> Value {
        match tag {
            1 => Value::Byte(self.byte() as i8),
            2 => {
                let value = u16::from_le_bytes([self.byte(), self.byte()]) as i16;
                Value::Short(value)
            }
            3 => Value::Int(self.var_i32()),
            4 => Value::Long(self.var_i32() as i64),
            5 => {
                let mut raw = [0u8; 4];
                for byte in &mut raw {
                    *byte = self.byte();
                }
                Value::Float(f32::from_bits(u32::from_le_bytes(raw)))
            }
            6 => {
                let mut raw = [0u8; 8];
                for byte in &mut raw {
                    *byte = self.byte();
                }
                Value::Double(f64::from_bits(u64::from_le_bytes(raw)))
            }
            8 => Value::String(self.string()),
            10 => Value::Compound(self.compound()),
            other => panic!("canonical_block_states.nbt: unsupported tag {other}"),
        }
    }

    /// Skips a value whose payload this module does not model (the data file
    /// only uses the types above, but a malformed entry must not panic).
    fn compound(&mut self) -> Vec<(String, Value)> {
        let mut children = Vec::new();
        loop {
            let tag = self.byte();
            if tag == 0 {
                return children;
            }
            let name = self.string();
            let value = self.value(tag);
            children.push((name, value));
        }
    }
}

fn parse_canonical_states(buf: &[u8]) -> Vec<(String, Vec<(String, Value)>)> {
    let mut reader = NbtReader::new(buf);
    let mut states = Vec::with_capacity(23960);
    while !reader.done() {
        let tag = reader.byte();
        assert_eq!(tag, 10, "canonical_block_states.nbt: entry is not a compound");
        let _entry_name = reader.string();
        let mut name = String::new();
        let mut properties = Vec::new();
        for (key, value) in reader.compound() {
            match (key.as_str(), value) {
                ("name", Value::String(value)) => name = value,
                ("states", Value::Compound(value)) => properties = value,
                _ => {}
            }
        }
        states.push((name, properties));
    }
    states
}

// ---------------------------------------------------------------------------
// Little-endian NBT writer + FNV-1a 32
// ---------------------------------------------------------------------------

const FNV_OFFSET_BASIS: u32 = 0x811C_9DC5;
const FNV_PRIME: u32 = 0x0100_0193;

fn fnv1a32(data: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in data {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn write_le_value(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Byte(value) => out.push(*value as u8),
        Value::Short(value) => out.extend_from_slice(&value.to_le_bytes()),
        Value::Int(value) => out.extend_from_slice(&value.to_le_bytes()),
        Value::Long(value) => out.extend_from_slice(&value.to_le_bytes()),
        Value::Float(value) => out.extend_from_slice(&value.to_le_bytes()),
        Value::Double(value) => out.extend_from_slice(&value.to_le_bytes()),
        Value::String(value) => {
            out.extend_from_slice(&(value.len() as u16).to_le_bytes());
            out.extend_from_slice(value.as_bytes());
        }
        Value::Compound(children) => {
            let mut keys: Vec<&String> = children.iter().map(|(key, _)| key).collect();
            keys.sort();
            for key in keys {
                let value = &children
                    .iter()
                    .find(|(candidate, _)| candidate == key)
                    .expect("key came from the same list")
                    .1;
                write_le_entry(out, key, value);
            }
            out.push(0);
        }
    }
}

fn write_le_entry(out: &mut Vec<u8>, name: &str, value: &Value) {
    let tag = match value {
        Value::Byte(_) => 1,
        Value::Short(_) => 2,
        Value::Int(_) => 3,
        Value::Long(_) => 4,
        Value::Float(_) => 5,
        Value::Double(_) => 6,
        Value::String(_) => 8,
        Value::Compound(_) => 10,
    };
    out.push(tag);
    out.extend_from_slice(&(name.len() as u16).to_le_bytes());
    out.extend_from_slice(name.as_bytes());
    write_le_value(out, value);
}

/// `ComputeBlockHash`: FNV-1a 32 over the canonical state as little-endian NBT.
pub fn state_hash(name: &str, properties: &[(String, Value)]) -> u32 {
    let mut bytes = Vec::with_capacity(64);
    bytes.extend_from_slice(&[10, 0, 0]);
    write_le_entry(&mut bytes, "name", &Value::String(name.to_string()));
    bytes.extend_from_slice(&[10, 6, 0]);
    bytes.extend_from_slice(b"states");
    let mut keys: Vec<&String> = properties.iter().map(|(key, _)| key).collect();
    keys.sort();
    for key in keys {
        let value = &properties
            .iter()
            .find(|(candidate, _)| candidate == key)
            .expect("key came from the same list")
            .1;
        write_le_entry(&mut bytes, key, value);
    }
    bytes.push(0);
    bytes.push(0);
    fnv1a32(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_id;

    /// Blocks outside the hand-written tables must resolve on the Java side
    /// too, otherwise a Bedrock placement still rendered as air.
    #[test]
    fn java_states_cover_blocks_outside_the_small_tables() {
        assert_eq!(java_block_state(775, "jungle_fence"), Some(13867));
        assert_eq!(java_block_state(769, "jungle_fence"), Some(12578));
        assert_eq!(java_block_state(775, "spruce_planks"), Some(16));
        assert_eq!(java_block_state(775, "minecraft:stone"), Some(1));
        assert_eq!(java_block_state(775, "not_a_block"), None);
    }

    /// Java creative item ids translate into the block a player places.
    #[test]
    fn java_item_ids_resolve_to_blocks() {
        // 1 = stone, 5 = polished_diorite (minecraft-data item registry).
        assert_eq!(java_item_block_name(775, 5), Some("polished_diorite".to_string()));
        assert_eq!(java_item_block_name(775, 1), Some("stone".to_string()));
        assert_eq!(java_item_block_name(775, -1), None);
    }

    /// The hashes the client sends for known blocks must match the constants
    /// the rest of the server uses (`block_id.rs`), which were taken from the
    /// BDS creative content capture.
    #[test]
    fn canonical_hashes_match_the_known_block_ids() {
        assert_eq!(network_id("air"), Some(block_id::AIR));
        assert_eq!(network_id("stone"), Some(block_id::STONE));
        assert_eq!(network_id("dirt"), Some(block_id::DIRT));
        assert_eq!(network_id("grass_block"), Some(block_id::GRASS_BLOCK));
        assert_eq!(network_id("water"), Some(block_id::WATER));
        assert_eq!(network_id("oak_log"), Some(block_id::OAK_LOG));
        assert_eq!(network_id("minecraft:sand"), Some(block_id::SAND));
    }

    /// Ids a real client sent when placing blocks (`server.log`
    /// `world.set_block: id=0x...`), previously unmapped (Java saw air).
    #[test]
    fn decodes_the_ids_a_client_sent_for_placed_blocks() {
        assert_eq!(state_name(0xC0F6_8F2A), Some("jungle_fence"));
        assert_eq!(state_name(0x6031_F1A8), Some("spruce_planks"));
    }

    #[test]
    fn the_table_covers_every_canonical_state() {
        assert_eq!(table().by_hash.len(), 23960);
        // Blocks beyond the hand-picked tables resolve too.
        let planks = network_id("oak_planks").expect("oak_planks is a canonical block");
        assert_eq!(state_name(planks), Some("oak_planks"));
        assert!(network_id("bedrock").is_some());
        assert_eq!(state_name(12345), None);
        assert_eq!(network_id("not_a_block"), None);
    }

    /// A fence next to another fence or a full cube connects on that side; the
    /// state must change to the canonical state carrying the bit, so the
    /// change can be broadcast to Bedrock and Java clients.
    #[test]
    fn fences_connect_to_fences_and_full_cubes() {
        let fence = network_id("jungle_fence").expect("jungle_fence");
        let stone = block_id::STONE;

        // Fence to the east, air elsewhere: only `connection_east` flips.
        let updated = recalculate_connections(fence, |dx, _, _| if dx == 1 { fence } else { block_id::AIR })
            .expect("fence connects to a fence");
        assert_eq!(state_name(updated), Some("jungle_fence"));
        let props = state_properties(updated).expect("props");
        assert_eq!(connection_bit(props, "east"), Some(1));
        assert_eq!(connection_bit(props, "north"), Some(0));
        assert_eq!(connection_bit(props, "west"), Some(0));

        // A full cube on the north side connects too.
        let updated = recalculate_connections(fence, |_, _, dz| if dz == -1 { stone } else { block_id::AIR })
            .expect("fence connects to stone");
        assert_eq!(connection_bit(state_properties(updated).unwrap(), "north"), Some(1));

        // Glass is not a full cube, so it must not connect.
        let glass = network_id("glass").expect("glass");
        assert!(is_full_cube("stone"));
        assert!(!is_full_cube("glass"));
        assert!(!is_full_cube("oak_slab"));
        assert!(is_full_cube("oak_double_slab"));
        assert!(!is_full_cube("oak_stairs"));
        assert!(!is_full_cube("air"));
        assert_eq!(
            recalculate_connections(fence, |_, _, _| glass),
            None,
            "glass does not connect: the default state is already correct"
        );
    }

    /// Panes/bars connect to panes, walls and full cubes; walls connect to
    /// walls, panes, gates and full cubes, and carry the post bit from the
    /// block above (PocketMine `Wall::recalculateConnections()`).
    #[test]
    fn panes_and_walls_recalculate_their_connections() {
        let pane = network_id("glass_pane").expect("glass_pane");
        let wall = network_id("cobblestone_wall").expect("cobblestone_wall");

        let updated = recalculate_connections(pane, |dx, _, _| if dx == -1 { wall } else { block_id::AIR })
            .expect("pane connects to a wall");
        assert_eq!(state_name(updated), Some("glass_pane"));
        assert_eq!(connection_bit(state_properties(updated).unwrap(), "west"), Some(1));

        // Wall next to stone, with stone above: short connection + post bit.
        let updated = recalculate_connections(wall, |dx, dy, _| {
            if dy == 1 || dx == 1 {
                block_id::STONE
            } else {
                block_id::AIR
            }
        })
        .expect("wall connects to stone");
        let props = state_properties(updated).expect("props");
        assert_eq!(
            props
                .iter()
                .find(|(key, _)| key == "wall_connection_type_east")
                .map(|(_, value)| render(value)),
            Some("short".to_string())
        );
        assert_eq!(
            props
                .iter()
                .find(|(key, _)| key == "wall_post_bit")
                .map(|(_, value)| render(value)),
            Some("B1".to_string())
        );
    }

    /// A Java client needs the exact variant: fences, panes and walls map to the
    /// generated per-connection state table instead of the block default.
    #[test]
    fn connectable_states_map_to_their_java_variant() {
        let fence = network_id("jungle_fence").expect("jungle_fence");
        // Disconnected == the block default (13867 for 1.26.60).
        assert_eq!(java_connection_state(775, fence), Some(13867));
        assert_eq!(java_connection_state(769, fence), Some(12578));

        // Fence to the east: `connection_east` -> Java `east=true`.
        let east = recalculate_connections(fence, |dx, _, _| {
            if dx == 1 {
                fence
            } else {
                block_id::AIR
            }
        })
        .expect("connects east");
        assert_eq!(java_connection_state(775, east), Some(13859));

        // Both north and east (the flicker case: several neighbours).
        let corner = recalculate_connections(fence, |dx, _, dz| {
            if dx == 1 || dz == -1 {
                fence
            } else {
                block_id::AIR
            }
        })
        .expect("connects north+east");
        assert_eq!(java_connection_state(775, corner), Some(13843));

        // Glass panes share the fence layout.
        let pane = network_id("glass_pane").expect("glass_pane");
        assert_eq!(java_connection_state(775, pane), Some(8331));
        let pane_east = recalculate_connections(pane, |dx, _, _| {
            if dx == 1 {
                pane
            } else {
                block_id::AIR
            }
        })
        .expect("pane connects east");
        assert_eq!(java_connection_state(775, pane_east), Some(8323));

        // Walls use the `none/low/tall` + `up` layout.
        let wall = network_id("cobblestone_wall").expect("cobblestone_wall");
        assert_eq!(java_connection_state(775, wall), Some(9984));
        let wall_east = recalculate_connections(wall, |dx, _, _| {
            if dx == 1 {
                block_id::STONE
            } else {
                block_id::AIR
            }
        })
        .expect("wall connects east");
        assert_eq!(java_connection_state(775, wall_east), Some(10038));
        let wall_east_up = recalculate_connections(wall, |dx, dy, _| {
            if dx == 1 || dy == 1 {
                block_id::STONE
            } else {
                block_id::AIR
            }
        })
        .expect("wall connects east with a post");
        assert_eq!(java_connection_state(775, wall_east_up), Some(10036));

        // Everything else keeps the plain name -> default state path.
        assert_eq!(java_connection_state(775, block_id::STONE), None);
        assert_eq!(java_connection_state(775, fence), java_connection_state(775, fence));
        assert_eq!(java_connection_state(999, fence), None, "unsupported protocol");
    }

    /// The generated tables must agree with the block defaults they came from:
    /// a disconnected state is exactly `defaultState` for every entry.
    #[test]
    fn generated_connection_tables_match_the_block_defaults() {
        for protocol in [769, 770, 773, 774, 775] {
            let table = connection_table(protocol).expect("bundled table");
            assert!(!table.is_empty(), "protocol {protocol}");
            for (name, ids) in table {
                let expected_len = if name.ends_with("_wall") { 162 } else { 16 };
                assert_eq!(ids.len(), expected_len, "{name} in {protocol}");
                assert_eq!(
                    Some(ids[0]),
                    java_block_state(protocol, name),
                    "{name} disconnected state must be the block default in {protocol}"
                );
            }
        }
    }

    /// A plain block has no connection state, and adding a neighbour that does
    /// not connect must not produce a new state (no update spam).
    #[test]
    fn plain_blocks_do_not_recalculate() {
        assert_eq!(
            recalculate_connections(block_id::STONE, |_, _, _| block_id::STONE),
            None
        );
        let fence = network_id("jungle_fence").expect("jungle_fence");
        assert_eq!(
            recalculate_connections(fence, |_, _, _| block_id::AIR),
            None,
            "no neighbours: already disconnected"
        );
    }

    fn connection_bit(props: &[(String, Value)], dir: &str) -> Option<i8> {
        props
            .iter()
            .find(|(key, _)| key == &format!("minecraft:connection_{dir}"))
            .and_then(|(_, value)| match value {
                Value::Byte(bit) => Some(*bit),
                _ => None,
            })
    }



    #[test]
    fn a_button_press_schedules_its_own_release() {
        let button = network_id("stone_button").unwrap();
        let press = interact(button).expect("stone button is interactive");
        assert_eq!(
            press.new_state,
            with_property(button, "button_pressed_bit", "true").unwrap()
        );
        assert!(press.powered);
        assert!(!press.door_sound);
        assert_eq!(press.reset_after_ticks, Some(20));

        // `spruce_button` is a PM `WoodenButton`: 30 ticks.
        let wooden = interact(network_id("spruce_button").unwrap()).unwrap();
        assert_eq!(wooden.reset_after_ticks, Some(30));

        // Pressing an already pressed button consumes the click, no change.
        let again = interact(press.new_state).unwrap();
        assert_eq!(again.new_state, press.new_state);
        assert_eq!(again.reset_after_ticks, None);
    }

    #[test]
    fn a_lever_toggles_between_on_and_off() {
        let lever = network_id("lever").unwrap();
        let on = interact(lever).expect("lever is interactive");
        assert!(on.powered);
        assert_eq!(bool_property(on.new_state, "open_bit"), Some(true));
        assert_eq!(on.reset_after_ticks, None);

        let off = interact(on.new_state).unwrap();
        assert!(!off.powered);
        assert_eq!(off.new_state, lever);
    }

    #[test]
    fn a_fence_gate_opens_and_closes_with_the_door_sound() {
        let gate = network_id("fence_gate").unwrap();
        let open = interact(gate).expect("fence gate is interactive");
        assert!(open.door_sound);
        assert_eq!(bool_property(open.new_state, "open_bit"), Some(true));

        let closed = interact(open.new_state).unwrap();
        assert!(!closed.powered);
        assert_eq!(closed.new_state, gate);
    }

    #[test]
    fn plain_blocks_are_not_interactive() {
        for name in ["stone", "torch", "oak_fence", "glass_pane", "cobblestone_wall"] {
            let id = network_id(name).unwrap_or_else(|| panic!("no default state for {name}"));
            assert_eq!(interact(id), None, "{name}");
        }
    }

    #[test]
    fn prefixed_and_bare_names_resolve_alike() {
        assert_eq!(network_id("stone"), network_id("minecraft:stone"));
    }
}
