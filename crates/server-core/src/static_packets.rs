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

    pub const CREATIVE_CONTENT: &'static [u8] =
        include_bytes!("../../../data/bds-capture/creative_content/1.bin");

    pub const TRIM_DATA: &'static [u8] =
        include_bytes!("../../../data/bds-capture/trim_data/1.bin");

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
}
