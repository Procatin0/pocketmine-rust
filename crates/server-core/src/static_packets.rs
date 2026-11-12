//! Static pre-spawn packets captured from vanilla Bedrock Dedicated Server 1.26 (protocol 2225).
//!
//! Faithful to Axolotl's `StaticPacketCache.php` and `PreSpawnPacketHandler.php`.
//! These contain world/registry declarations required by modern Bedrock (1.21+ / 1.26)
//! before and immediately after StartGamePacket.

pub struct StaticPackets;

impl StaticPackets {
    pub const JIGSAW_STRUCTURE_DATA: &'static [u8] =
        include_bytes!("../../../data/bds-capture/jigsaw_structure_data/1.bin");

    pub const VOXEL_SHAPES: &'static [u8] =
        include_bytes!("../../../data/bds-capture/voxel_shapes/1.bin");

    pub const ITEM_REGISTRY: &'static [u8] =
        include_bytes!("../../../data/bds-capture/item_registry/1.bin");

    pub const AVAILABLE_ENTITY_IDENTIFIERS: &'static [u8] =
        include_bytes!("../../../data/bds-capture/available_entity_identifiers/1.bin");

    pub const BIOME_DEFINITION_LIST: &'static [u8] =
        include_bytes!("../../../data/bds-capture/biome_definition_list/1.bin");

    pub const CAMERA_PRESETS: &'static [u8] =
        include_bytes!("../../../data/bds-capture/camera_presets/1.bin");

    /// BDS `CreativeContentPacket` (0x91) capture (54149 B), sent by default
    /// (unless `PM_CORE_CREATIVE=off` skips the push; `PM_CORE_CREATIVE_MODE=
    /// generated` in `PlayerSession::creative_content_wire` selects the
    /// generated wire from `data/creative/` instead). WARNING — like
    /// `CRAFTING_DATA` below, this file is a re-encoding made from the
    /// normalized `1.json`, not verbatim BDS wire. Its wire layout IS the
    /// BDS 1.26 layout, however, verified byte-for-byte against `1.json`
    /// (123 groups / 1875 items, fully consumed, sequential entry ids):
    /// groups are LE-u32 category id (1=construction, 2=nature, 3=equipment,
    /// 4=items) + name + `ItemStackWithoutStackId` icon (`var_i32` id,
    /// `u16` count, `var_u32` meta, `var_i32` FNV block runtime hash,
    /// string `ItemStackExtraData`, 10-byte minimum when there is no NBT;
    /// a `false` icon (`id == 0`) is a single `var_i32` 0 with nothing
    /// after it), and items are `var_u32` entry id + stack + `var_u32`
    /// group index. The 2225 PHP snapshot in `target/BedrockProtocol`
    /// is stale for `0x91`: `CreativeGroupEntry::read` uses a single
    /// category byte and no null-icon short form, so it will misparse
    /// this BDS packet (36 of the 123 groups have null icons). `0x91`
    /// parse failure with `Invalid enum value` on real clients is
    /// reported for both this capture and the `generated` wire; the
    /// framing above is verified, but which field the client rejects
    /// cannot be determined from bytes alone. Do NOT "fix" this by
    /// re-encoding from `data/creative/`: the generated wire drops 47%
    /// of the entries (no negative data-driven ids), ignores
    /// `block_states` (runtime hash 0 for block items), emits the
    /// `ItemStackWrapper`/`ItemStackWithoutStackId` mix-up described in
    /// `creative_content.rs`, and emits 134 groups instead of 123.
    pub const CREATIVE_CONTENT: &'static [u8] =
        include_bytes!("../../../data/bds-capture/creative_content/1.bin");

    pub const TRIM_DATA: &'static [u8] =
        include_bytes!("../../../data/bds-capture/trim_data/1.bin");

    /// BDS `StartGamePacket` (`0x0B`) capture (1025 B), kept for bisect:
    /// our generated StartGame is 218 B and may miss LevelSettings fields
    /// (`gameRules`, `generator 1`, `biome_name "minecraft:plains"`).
    pub const START_GAME: &'static [u8] =
        include_bytes!("../../../data/bds-capture/start_game/1.bin");

    /// BDS `CraftingDataPacket` (0x34) capture (424984 B), sent by default
    /// (`PlayerSession::crafting_data_wire`, unless
    /// `PM_CORE_CRAFTING_MODE=generated` or `=off`; see `session.rs`). This
    /// file is NOT verbatim BDS wire: it was re-encoded from the normalized
    /// `1.json` (one unified `recipes` list of 3660 entries spanning
    /// shaped/shapeless/shulker_box/multi/smithing_*) using this server's
    /// own crafting codec. Proof: after the `0x34` header the first varu is
    /// `3660` — the total unified count, not a shaped-only count — and the
    /// first recipe is `minecraft:smithing_armor_trim`. The 2225 reference
    /// wire (`target/BedrockProtocol/src/CraftingDataPacket.php`, protocol
    /// 2225 like ours) is 11 independent lists (shaped, shapeless, multi,
    /// user-data, chemistry, smithing-transform, smithing-trim, potion
    /// types, potion containers, material reducers) with no per-recipe type
    /// byte, shaped ingredient counts, `ItemStackWithoutStackId` outputs
    /// and string-id ("mess") descriptors; the current codec emits a single
    /// unified list with per-recipe type bytes. A 2225 client therefore
    /// misparses BOTH this capture and the generated wire (~92kB) at the
    /// first string read (`invalid string readNoHeader packetId:52`):
    /// size (425kB raw -> ~108kB compressed, one packet-atomic batch) and
    /// compression are not the blocker — the NetherNet WebRTC
    /// `SctpMaxMessageSize::Bounded(262144)` is never exceeded after
    /// compression and the reference likewise sends one batch per flush
    /// with no packet-level fragmentation. Porting
    /// `crates/bedrock-protocol/src/packets/crafting_data.rs` to the
    /// 11-list layout is required before any mode can be client-valid;
    /// `PM_CORE_CRAFTING_MODE=off` is the only working workaround today.
    pub const CRAFTING_DATA: &'static [u8] =
        include_bytes!("../../../data/bds-capture/crafting_data/1.bin");

    pub const SET_COMMANDS_ENABLED: &'static [u8] =
        include_bytes!("../../../data/bds-capture/set_commands_enabled/1.bin");

    pub const SET_DIFFICULTY: &'static [u8] =
        include_bytes!("../../../data/bds-capture/set_difficulty/1.bin");

    pub const GAME_RULES_CHANGED: &'static [u8] =
        include_bytes!("../../../data/bds-capture/game_rules_changed/1.bin");

    pub const SET_SPAWN_POSITION: &'static [u8] =
        include_bytes!("../../../data/bds-capture/set_spawn_position/1.bin");

    pub const NETWORK_CHUNK_PUBLISHER_UPDATE: &'static [u8] =
        include_bytes!("../../../data/bds-capture/network_chunk_publisher_update/1.bin");

    pub const UPDATE_ADVENTURE_SETTINGS: &'static [u8] =
        include_bytes!("../../../data/bds-capture/update_adventure_settings/1.bin");

    pub const SET_ENTITY_DATA: &'static [u8] =
        include_bytes!("../../../data/bds-capture/set_entity_data/1.bin");

    pub const UPDATE_ATTRIBUTES: &'static [u8] =
        include_bytes!("../../../data/bds-capture/update_attributes/1.bin");

    pub const UPDATE_ABILITIES: &'static [u8] =
        include_bytes!("../../../data/bds-capture/update_abilities/1.bin");
}
