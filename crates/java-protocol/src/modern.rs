//! Byte-for-byte port of `plugins/JavaBridge/src/javabridge/ModernProfile.php`:
//! packet-id tables for the modern profiles (769/770/773/774/775),
//! global block states, registry/tags/brand payloads, and the 26.1
//! `update_time` clock format.
//!
//! Packet ids come from PrismarineJS/minecraft-data
//! (`data/pc/<version>/protocol.json`) and differ per protocol; each
//! protocol therefore has its own table below. The modern flow is
//! handshake -> login -> configuration -> play (1.20.2+).
//!
//! Registry/tag data is embedded with `include_str!` from verbatim copies
//! of `plugins/JavaBridge/resources/{registry,tags,tag_members}/<proto>.json`
//! (the same files `ModernProfile` reads with `dirname(__DIR__, 2)` in PHP),
//! so the Rust payloads are byte-identical without a filesystem lookup.
//! Compound key order is preserved (`serde_json` `preserve_order`),
//! exactly like PHP arrays.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;
use thiserror::Error;

use crate::{nbt, protocol};

/// Protocol -> version name.
pub const VERSIONS: &[(i32, &str)] = &[
    (769, "1.21.4"),
    (770, "1.21.5"),
    (773, "1.21.9"),
    (774, "1.21.11"),
    (775, "26.1"),
];

/// Version names in `VERSIONS` order.
pub const VERSION_NAMES: [&str; 5] = ["1.21.4", "1.21.5", "1.21.9", "1.21.11", "26.1"];

/// Server brand shown in F3 (`Server:`).
pub const BRAND: &str = "PocketMine";
/// Brand channel in configuration `custom_payload` (`config_brand`, id 1).
pub const BRAND_CHANNEL: &str = "minecraft:brand";

/// Errors mirroring PHP `InvalidArgumentException` / `RuntimeException`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModernError {
    #[error("Protocolo moderno no soportado: {0}")]
    UnsupportedProtocol(i32),
    #[error("Paquete desconocido para {version}: {key}")]
    UnknownPacket { version: String, key: String },
    #[error("Faltan los registros de Java para {0}: {1}")]
    MissingRegistries(String, String),
    #[error("Faltan los tags de Java para {0}: {1}")]
    MissingTags(String, String),
    #[error("NBT desconocido: {0}")]
    Nbt(String),
}

fn version_name(protocol: i32) -> Option<&'static str> {
    VERSIONS
        .iter()
        .find(|(p, _)| *p == protocol)
        .map(|(_, n)| *n)
}

fn registry_json(protocol: i32) -> Option<&'static str> {
    match protocol {
        769 => Some(include_str!("../resources/registry/769.json")),
        770 => Some(include_str!("../resources/registry/770.json")),
        773 => Some(include_str!("../resources/registry/773.json")),
        774 => Some(include_str!("../resources/registry/774.json")),
        775 => Some(include_str!("../resources/registry/775.json")),
        _ => None,
    }
}

fn tags_json(protocol: i32) -> Option<&'static str> {
    match protocol {
        769 => Some(include_str!("../resources/tags/769.json")),
        770 => Some(include_str!("../resources/tags/770.json")),
        773 => Some(include_str!("../resources/tags/773.json")),
        774 => Some(include_str!("../resources/tags/774.json")),
        775 => Some(include_str!("../resources/tags/775.json")),
        _ => None,
    }
}

fn tag_members_json(protocol: i32) -> Option<&'static str> {
    match protocol {
        769 => Some(include_str!("../resources/tag_members/769.json")),
        770 => Some(include_str!("../resources/tag_members/770.json")),
        773 => Some(include_str!("../resources/tag_members/773.json")),
        774 => Some(include_str!("../resources/tag_members/774.json")),
        775 => Some(include_str!("../resources/tag_members/775.json")),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct RegistryData {
    payloads: Vec<Vec<u8>>,
    ids: Vec<String>,
}

fn registry_cache() -> &'static Mutex<HashMap<i32, RegistryData>> {
    static CACHE: OnceLock<Mutex<HashMap<i32, RegistryData>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn tags_cache() -> &'static Mutex<HashMap<i32, Vec<u8>>> {
    static CACHE: OnceLock<Mutex<HashMap<i32, Vec<u8>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn tag_members_cache() -> &'static Mutex<HashMap<i32, Value>> {
    static CACHE: OnceLock<Mutex<HashMap<i32, Value>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn plains_cache() -> &'static Mutex<HashMap<i32, i32>> {
    static CACHE: OnceLock<Mutex<HashMap<i32, i32>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn packet_id(protocol: i32, key: &str) -> Option<i32> {
    // `config_*` keys are configuration-phase packets; the table below
    // intentionally keeps `config_brand` (clientbound custom_payload, 1)
    // distinct from `config_custom_payload` (serverbound, 2). `update_tags`
    // is an alias of `tags` (configuration clientbound 13 in 769-775).
    match (protocol, key) {
        (769, "login_success") => Some(2),
        (769, "join_game") => Some(44),
        (769, "position") => Some(66),
        (769, "map_chunk") => Some(40),
        (769, "game_state_change") => Some(35),
        (769, "abilities") => Some(58),
        (769, "system_chat") => Some(115),
        (769, "block_change") => Some(9),
        (769, "unload_chunk") => Some(34),
        (769, "kick") => Some(29),
        (769, "keep_alive") => Some(39),
        (769, "chunk_batch_start") => Some(13),
        (769, "chunk_batch_finished") => Some(12),
        (769, "registry_data") => Some(7),
        (769, "finish_configuration") => Some(3),
        (769, "feature_flags") => Some(12),
        (769, "tags") => Some(13),
        (769, "update_tags") => Some(13),
        (769, "config_brand") => Some(1),
        (769, "login_acknowledged") => Some(3),
        (769, "config_settings") => Some(0),
        (769, "config_custom_payload") => Some(2),
        (769, "config_select_known_packs") => Some(7),
        (769, "config_finish") => Some(3),
        (769, "chat_message") => Some(7),
        (769, "chat_command") => Some(5),
        (769, "sb_position") => Some(28),
        (769, "sb_position_look") => Some(29),
        (769, "sb_look") => Some(30),
        (769, "client_command") => Some(10),
        (769, "block_dig") => Some(39),
        (769, "block_place") => Some(60),
        (769, "entity_action") => Some(40),
        (769, "player_input") => Some(41),
        (769, "teleport_confirm") => Some(0),
        (769, "chunk_batch_received") => Some(9),
        (769, "player_loaded") => Some(42),
        (769, "spawn_entity") => Some(1),
        (769, "rel_entity_move") => Some(47),
        (769, "entity_move_look") => Some(48),
        (769, "entity_look") => Some(50),
        (769, "entity_head_rotation") => Some(77),
        (769, "entity_destroy") => Some(71),
        (769, "player_info") => Some(64),
        (769, "player_remove") => Some(63),
        (769, "entity_metadata") => Some(93),
        (769, "entity_teleport") => Some(119),
        (769, "sync_entity_position") => Some(32),
        (769, "update_view_position") => Some(88),
        (769, "update_view_distance") => Some(89),
        (769, "update_time") => Some(107),
        (769, "boss_bar") => Some(0x0A),
        (769, "scoreboard_objective") => Some(100),
        (769, "scoreboard_score") => Some(104),
        (769, "scoreboard_display_objective") => Some(92),
        (769, "reset_score") => Some(73),

        (770, "login_success") => Some(2),
        (770, "join_game") => Some(43),
        (770, "position") => Some(65),
        (770, "map_chunk") => Some(39),
        (770, "game_state_change") => Some(34),
        (770, "abilities") => Some(57),
        (770, "system_chat") => Some(114),
        (770, "block_change") => Some(8),
        (770, "unload_chunk") => Some(33),
        (770, "kick") => Some(28),
        (770, "keep_alive") => Some(38),
        (770, "chunk_batch_start") => Some(12),
        (770, "chunk_batch_finished") => Some(11),
        (770, "registry_data") => Some(7),
        (770, "finish_configuration") => Some(3),
        (770, "feature_flags") => Some(12),
        (770, "tags") => Some(13),
        (770, "update_tags") => Some(13),
        (770, "config_brand") => Some(1),
        (770, "login_acknowledged") => Some(3),
        (770, "config_settings") => Some(0),
        (770, "config_custom_payload") => Some(2),
        (770, "config_select_known_packs") => Some(7),
        (770, "config_finish") => Some(3),
        (770, "chat_message") => Some(7),
        (770, "chat_command") => Some(5),
        (770, "sb_position") => Some(28),
        (770, "sb_position_look") => Some(29),
        (770, "sb_look") => Some(30),
        (770, "client_command") => Some(10),
        (770, "block_dig") => Some(39),
        (770, "block_place") => Some(62),
        (770, "entity_action") => Some(40),
        (770, "player_input") => Some(41),
        (770, "teleport_confirm") => Some(0),
        (770, "chunk_batch_received") => Some(9),
        (770, "player_loaded") => Some(42),
        (770, "spawn_entity") => Some(1),
        (770, "rel_entity_move") => Some(46),
        (770, "entity_move_look") => Some(47),
        (770, "entity_look") => Some(49),
        (770, "entity_head_rotation") => Some(76),
        (770, "entity_destroy") => Some(70),
        (770, "player_info") => Some(63),
        (770, "player_remove") => Some(62),
        (770, "entity_metadata") => Some(92),
        (770, "entity_teleport") => Some(118),
        (770, "sync_entity_position") => Some(31),
        (770, "update_view_position") => Some(87),
        (770, "update_view_distance") => Some(88),
        (770, "update_time") => Some(106),
        (770, "boss_bar") => Some(0x09),
        (770, "scoreboard_objective") => Some(99),
        (770, "scoreboard_score") => Some(103),
        (770, "scoreboard_display_objective") => Some(91),
        (770, "reset_score") => Some(72),

        (773, "login_success") => Some(2),
        (773, "join_game") => Some(48),
        (773, "position") => Some(70),
        (773, "map_chunk") => Some(44),
        (773, "game_state_change") => Some(38),
        (773, "abilities") => Some(62),
        (773, "system_chat") => Some(119),
        (773, "block_change") => Some(8),
        (773, "unload_chunk") => Some(37),
        (773, "kick") => Some(32),
        (773, "keep_alive") => Some(43),
        (773, "chunk_batch_start") => Some(12),
        (773, "chunk_batch_finished") => Some(11),
        (773, "registry_data") => Some(7),
        (773, "finish_configuration") => Some(3),
        (773, "feature_flags") => Some(12),
        (773, "tags") => Some(13),
        (773, "update_tags") => Some(13),
        (773, "config_brand") => Some(1),
        (773, "login_acknowledged") => Some(3),
        (773, "config_settings") => Some(0),
        (773, "config_custom_payload") => Some(2),
        (773, "config_select_known_packs") => Some(7),
        (773, "config_finish") => Some(3),
        (773, "chat_message") => Some(8),
        (773, "chat_command") => Some(6),
        (773, "sb_position") => Some(29),
        (773, "sb_position_look") => Some(30),
        (773, "sb_look") => Some(31),
        (773, "client_command") => Some(11),
        (773, "block_dig") => Some(40),
        (773, "block_place") => Some(63),
        (773, "entity_action") => Some(41),
        (773, "player_input") => Some(42),
        (773, "teleport_confirm") => Some(0),
        (773, "chunk_batch_received") => Some(10),
        (773, "player_loaded") => Some(43),
        (773, "spawn_entity") => Some(1),
        (773, "rel_entity_move") => Some(51),
        (773, "entity_move_look") => Some(52),
        (773, "entity_look") => Some(54),
        (773, "entity_head_rotation") => Some(81),
        (773, "entity_destroy") => Some(75),
        (773, "player_info") => Some(68),
        (773, "player_remove") => Some(67),
        (773, "entity_metadata") => Some(97),
        (773, "entity_teleport") => Some(123),
        (773, "sync_entity_position") => Some(35),
        (773, "update_view_position") => Some(92),
        (773, "update_view_distance") => Some(93),
        (773, "update_time") => Some(111),
        (773, "boss_bar") => Some(0x09),
        (773, "scoreboard_objective") => Some(104),
        (773, "scoreboard_score") => Some(108),
        (773, "scoreboard_display_objective") => Some(96),
        (773, "reset_score") => Some(77),

        (774, "login_success") => Some(2),
        (774, "join_game") => Some(48),
        (774, "position") => Some(70),
        (774, "map_chunk") => Some(44),
        (774, "game_state_change") => Some(38),
        (774, "abilities") => Some(62),
        (774, "system_chat") => Some(119),
        (774, "block_change") => Some(8),
        (774, "unload_chunk") => Some(37),
        (774, "kick") => Some(32),
        (774, "keep_alive") => Some(43),
        (774, "chunk_batch_start") => Some(12),
        (774, "chunk_batch_finished") => Some(11),
        (774, "registry_data") => Some(7),
        (774, "finish_configuration") => Some(3),
        (774, "feature_flags") => Some(12),
        (774, "tags") => Some(13),
        (774, "update_tags") => Some(13),
        (774, "config_brand") => Some(1),
        (774, "login_acknowledged") => Some(3),
        (774, "config_settings") => Some(0),
        (774, "config_custom_payload") => Some(2),
        (774, "config_select_known_packs") => Some(7),
        (774, "config_finish") => Some(3),
        (774, "chat_message") => Some(8),
        (774, "chat_command") => Some(6),
        (774, "sb_position") => Some(29),
        (774, "sb_position_look") => Some(30),
        (774, "sb_look") => Some(31),
        (774, "client_command") => Some(11),
        (774, "block_dig") => Some(40),
        (774, "block_place") => Some(63),
        (774, "entity_action") => Some(41),
        (774, "player_input") => Some(42),
        (774, "teleport_confirm") => Some(0),
        (774, "chunk_batch_received") => Some(10),
        (774, "player_loaded") => Some(43),
        (774, "spawn_entity") => Some(1),
        (774, "rel_entity_move") => Some(51),
        (774, "entity_move_look") => Some(52),
        (774, "entity_look") => Some(54),
        (774, "entity_head_rotation") => Some(81),
        (774, "entity_destroy") => Some(75),
        (774, "player_info") => Some(68),
        (774, "player_remove") => Some(67),
        (774, "entity_metadata") => Some(97),
        (774, "entity_teleport") => Some(123),
        (774, "sync_entity_position") => Some(35),
        (774, "update_view_position") => Some(92),
        (774, "update_view_distance") => Some(93),
        (774, "update_time") => Some(111),
        (774, "boss_bar") => Some(0x09),
        (774, "scoreboard_objective") => Some(104),
        (774, "scoreboard_score") => Some(108),
        (774, "scoreboard_display_objective") => Some(96),
        (774, "reset_score") => Some(77),

        (775, "login_success") => Some(2),
        (775, "join_game") => Some(49),
        (775, "position") => Some(72),
        (775, "map_chunk") => Some(45),
        (775, "game_state_change") => Some(38),
        (775, "abilities") => Some(64),
        (775, "system_chat") => Some(121),
        (775, "block_change") => Some(8),
        (775, "unload_chunk") => Some(37),
        (775, "kick") => Some(32),
        (775, "keep_alive") => Some(44),
        (775, "chunk_batch_start") => Some(12),
        (775, "chunk_batch_finished") => Some(11),
        (775, "registry_data") => Some(7),
        (775, "finish_configuration") => Some(3),
        (775, "feature_flags") => Some(12),
        (775, "tags") => Some(13),
        (775, "update_tags") => Some(13),
        (775, "config_brand") => Some(1),
        (775, "login_acknowledged") => Some(3),
        (775, "config_settings") => Some(0),
        (775, "config_custom_payload") => Some(2),
        (775, "config_select_known_packs") => Some(7),
        (775, "config_finish") => Some(3),
        (775, "chat_message") => Some(9),
        (775, "chat_command") => Some(7),
        (775, "sb_position") => Some(30),
        (775, "sb_position_look") => Some(31),
        (775, "sb_look") => Some(32),
        (775, "client_command") => Some(12),
        (775, "block_dig") => Some(41),
        (775, "block_place") => Some(66),
        (775, "entity_action") => Some(42),
        (775, "player_input") => Some(43),
        (775, "teleport_confirm") => Some(0),
        (775, "chunk_batch_received") => Some(11),
        (775, "player_loaded") => Some(44),
        (775, "spawn_entity") => Some(1),
        (775, "rel_entity_move") => Some(53),
        (775, "entity_move_look") => Some(54),
        (775, "entity_look") => Some(56),
        (775, "entity_head_rotation") => Some(83),
        (775, "entity_destroy") => Some(77),
        (775, "player_info") => Some(70),
        (775, "player_remove") => Some(69),
        (775, "entity_metadata") => Some(99),
        (775, "entity_teleport") => Some(125),
        (775, "sync_entity_position") => Some(35),
        (775, "update_view_position") => Some(94),
        (775, "update_view_distance") => Some(95),
        (775, "update_time") => Some(113),
        (775, "boss_bar") => Some(0x09),
        (775, "scoreboard_objective") => Some(106),
        (775, "scoreboard_score") => Some(110),
        (775, "scoreboard_display_objective") => Some(98),
        (775, "reset_score") => Some(79),

        _ => None,
    }
}

fn id_alias(key: &str) -> Option<&'static str> {
    match key {
        "position_look" => Some("sb_position_look"),
        "look" => Some("sb_look"),
        "player_info_update" => Some("player_info"),
        "rel_move" => Some("rel_entity_move"),
        "brand" => Some("config_brand"),
        // Identity aliases kept for PHP parity (lookup falls back to key).
        "position" => Some("position"),
        "entity_teleport" => Some("entity_teleport"),
        "entity_destroy" => Some("entity_destroy"),
        "entity_metadata" => Some("entity_metadata"),
        _ => None,
    }
}

fn block_state_stable(name: &str) -> Option<i32> {
    match name {
        "air" => Some(0),
        "stone" => Some(1),
        "granite" => Some(2),
        "diorite" => Some(4),
        "andesite" => Some(6),
        "grass_block" => Some(9),
        "dirt" => Some(10),
        "cobblestone" => Some(14),
        "bedrock" => Some(85),
        "water" => Some(86),
        "sand" => Some(118),
        "gravel" => Some(124),
        "gold_ore" => Some(129),
        "iron_ore" => Some(131),
        "coal_ore" => Some(133),
        "oak_log" => Some(137),
        "spruce_log" => Some(140),
        "birch_log" => Some(143),
        "oak_leaves" => Some(279),
        "spruce_leaves" => Some(307),
        "birch_leaves" => Some(335),
        "lapis_ore" => Some(563),
        "sandstone" => Some(578),
        _ => None,
    }
}

fn block_state_by_protocol(protocol: i32, name: &str) -> Option<i32> {
    match (protocol, name) {
        (769, "redstone_ore") => Some(5904),
        (769, "diamond_ore") => Some(4329),
        (769, "emerald_ore") => Some(8285),
        (769, "short_grass") => Some(2048),
        (769, "tall_grass") => Some(11635),
        (769, "snow") => Some(5941),
        (769, "dandelion") => Some(2118),
        (769, "poppy") => Some(2120),

        (770, "redstone_ore") => Some(5913),
        (770, "diamond_ore") => Some(4338),
        (770, "emerald_ore") => Some(8295),
        (770, "short_grass") => Some(2048),
        (770, "tall_grass") => Some(11645),
        (770, "snow") => Some(5950),
        (770, "dandelion") => Some(2121),
        (770, "poppy") => Some(2123),

        (773, "redstone_ore") => Some(6681),
        (773, "diamond_ore") => Some(5106),
        (773, "emerald_ore") => Some(9372),
        (773, "short_grass") => Some(2048),
        (773, "tall_grass") => Some(12722),
        (773, "snow") => Some(6718),
        (773, "dandelion") => Some(2121),
        (773, "poppy") => Some(2123),

        (774, "redstone_ore") => Some(6681),
        (774, "diamond_ore") => Some(5106),
        (774, "emerald_ore") => Some(9372),
        (774, "short_grass") => Some(2048),
        (774, "tall_grass") => Some(12722),
        (774, "snow") => Some(6718),
        (774, "dandelion") => Some(2121),
        (774, "poppy") => Some(2123),

        (775, "redstone_ore") => Some(6882),
        (775, "diamond_ore") => Some(5307),
        (775, "emerald_ore") => Some(9573),
        (775, "short_grass") => Some(2248),
        (775, "tall_grass") => Some(12924),
        (775, "snow") => Some(6919),
        (775, "dandelion") => Some(2321),
        (775, "poppy") => Some(2324),
        _ => None,
    }
}

/// Modern profile for one Java protocol version (769/770/773/774/775).
#[derive(Debug, Clone, Copy)]
pub struct ModernProfile {
    /// Protocol number (e.g. 769).
    pub protocol: i32,
    /// Version name (e.g. `"1.21.4"`).
    pub name: &'static str,
}

impl ModernProfile {
    /// Construct for a supported protocol or fail like PHP's constructor.
    pub fn new(protocol: i32) -> Result<Self, ModernError> {
        version_name(protocol)
            .map(|name| Self { protocol, name })
            .ok_or(ModernError::UnsupportedProtocol(protocol))
    }

    /// Like PHP `forProtocol`: `None` when unsupported.
    pub fn for_protocol(protocol: i32) -> Option<Self> {
        version_name(protocol).map(|name| Self { protocol, name })
    }

    /// Like PHP `supports`.
    pub fn supports(protocol: i32) -> bool {
        version_name(protocol).is_some()
    }

    /// Packet id for `key`, resolving PHP's alias map
    /// (`brand` -> `config_brand`, `player_info_update` -> `player_info`,
    /// `position_look`/`look`/`rel_move` -> their `sb_*` / entity keys).
    ///
    /// Panics with `Paquete desconocido para <version>: <key>` when unknown,
    /// mirroring PHP's `InvalidArgumentException`.
    pub fn id(&self, key: &str) -> i32 {
        self.try_id(key)
            .unwrap_or_else(|| panic!("Paquete desconocido para {}: {}", self.name, key))
    }

    /// Fallible [`ModernProfile::id`].
    pub fn try_id(&self, key: &str) -> Option<i32> {
        let lookup = id_alias(key).unwrap_or(key);
        packet_id(self.protocol, lookup).or_else(|| packet_id(self.protocol, key))
    }

    /// Global Java block state for `name` in this version (default
    /// state of each block, never `minStateId`). Unknown names are `0`
    /// (air), like PHP's `?? 0` fallback.
    pub fn block_state(&self, name: &str) -> i32 {
        block_state_by_protocol(self.protocol, name)
            .or_else(|| block_state_stable(name))
            .unwrap_or(0)
    }

    /// `minecraft:player` entity-type index for `spawn_entity.type`
    /// (not in the login registries; vanilla-known).
    pub fn player_entity_type(&self) -> i32 {
        match self.protocol {
            769 => 147,
            770 => 148,
            773 => 151,
            774 | 775 => 155,
            _ => 147,
        }
    }

    /// Whether heightmaps travel as NBT (1.21.4) or as a typed list (1.21.5+).
    pub fn heightmaps_as_nbt(&self) -> bool {
        self.protocol == 769
    }

    /// Whether each palette block array carries an explicit length prefix
    /// (1.21.4 only; 1.21.5+ deduces it from bits x entries).
    pub fn palette_data_has_length(&self) -> bool {
        self.protocol == 769
    }

    /// Whether each section carries a fluid counter (26.1+).
    pub fn sections_have_fluid_count(&self) -> bool {
        self.protocol >= 775
    }

    /// Whether `spawn_entity` velocity is `lpVec3` before the angles
    /// (1.21.9+) or `vec3i16` at the end (769/770). The packet id (`0x01`)
    /// and the other fields are unchanged.
    pub fn spawn_velocity_is_lp_vec3(&self) -> bool {
        self.protocol >= 773
    }

    /// Index of `minecraft:plains` in `minecraft:worldgen/biome`
    /// (39 in 769-773, 40 in 774-775; 0 is `badlands`).
    pub fn plains_biome(&self) -> i32 {
        if let Some(cached) = plains_cache().lock().unwrap().get(&self.protocol) {
            return *cached;
        }
        let mut index = if self.protocol >= 774 { 40 } else { 39 };
        if let Some(raw) = registry_json(self.protocol) {
            if let Ok(decoded) = serde_json::from_str::<Value>(raw) {
                if let Some(regs) = decoded.get("registries").and_then(Value::as_array) {
                    'outer: for registry in regs {
                        if registry.get("id").and_then(Value::as_str)
                            != Some("minecraft:worldgen/biome")
                        {
                            continue;
                        }
                        if let Some(entries) = registry.get("entries").and_then(Value::as_array) {
                            for (i, entry) in entries.iter().enumerate() {
                                if entry.get("key").and_then(Value::as_str)
                                    == Some("minecraft:plains")
                                {
                                    index = i as i32;
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
            }
        }
        plains_cache().lock().unwrap().insert(self.protocol, index);
        index
    }

    /// One payload per login registry (cached). Each payload is
    /// `string(id) + varInt(count) + per-entry string(key) + boolean(true) +
    /// anonymous-NBT value`.
    pub fn registry_payloads(&self) -> Result<Vec<Vec<u8>>, ModernError> {
        if let Some(cached) = registry_cache().lock().unwrap().get(&self.protocol) {
            return Ok(cached.payloads.clone());
        }
        let raw = registry_json(self.protocol).ok_or_else(|| {
            ModernError::MissingRegistries(
                self.name.to_string(),
                format!("resources/registry/{}.json", self.protocol),
            )
        })?;
        let decoded: Value = serde_json::from_str(raw).map_err(|_| {
            ModernError::MissingRegistries(
                self.name.to_string(),
                format!("resources/registry/{}.json", self.protocol),
            )
        })?;
        let registries = decoded
            .get("registries")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ModernError::MissingRegistries(
                    self.name.to_string(),
                    format!("resources/registry/{}.json", self.protocol),
                )
            })?;
        let mut ids = Vec::with_capacity(registries.len());
        let mut payloads = Vec::with_capacity(registries.len());
        for registry in registries {
            let id = registry.get("id").and_then(Value::as_str).unwrap_or("");
            let entries = registry.get("entries").and_then(Value::as_array);
            let entries: &[Value] = entries.map(Vec::as_slice).unwrap_or(&[]);
            ids.push(id.to_string());
            let mut payload = protocol::string(id);
            payload.extend_from_slice(&protocol::var_int(entries.len() as i32));
            for entry in entries {
                let key = entry.get("key").and_then(Value::as_str).unwrap_or("");
                payload.extend_from_slice(&protocol::string(key));
                payload.extend_from_slice(&protocol::boolean(true));
                let value = entry.get("value").cloned().unwrap_or(Value::Null);
                let nbt_bytes =
                    nbt::anonymous(&value).map_err(|e| ModernError::Nbt(e.to_string()))?;
                payload.extend_from_slice(&nbt_bytes);
            }
            payloads.push(payload);
        }
        registry_cache().lock().unwrap().insert(
            self.protocol,
            RegistryData {
                payloads: payloads.clone(),
                ids,
            },
        );
        Ok(payloads)
    }

    /// Registry ids sent by [`ModernProfile::registry_payloads`], in order.
    /// (Populates the registry cache when needed, like PHP.)
    pub fn registry_ids(&self) -> Result<Vec<String>, ModernError> {
        self.registry_payloads()?;
        let cache = registry_cache().lock().unwrap();
        cache
            .get(&self.protocol)
            .map(|d| d.ids.clone())
            .ok_or_else(|| {
                ModernError::MissingRegistries(
                    self.name.to_string(),
                    format!("resources/registry/{}.json", self.protocol),
                )
            })
    }

    fn tag_members_table(&self) -> Value {
        if let Some(cached) = tag_members_cache().lock().unwrap().get(&self.protocol) {
            return cached.clone();
        }
        let raw = tag_members_json(self.protocol);
        let table = raw
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .unwrap_or_else(|| serde_json::json!({"registries": {}}));
        tag_members_cache()
            .lock()
            .unwrap()
            .insert(self.protocol, table.clone());
        table
    }

    /// Numeric members (static-registry ids) of a tag, from
    /// `tag_members/<protocol>.json`. Unlisted tags yield `[]` (the name
    /// still resolves `getOrThrow`). Note `minecraft:fluid` is the static
    /// empty/flowing_water/water/flowing_lava/lava order, so
    /// `water == [1, 2]` and `lava == [3, 4]`.
    pub fn tag_members(&self, registry: &str, tag: &str) -> Vec<i32> {
        let table = self.tag_members_table();
        let members = table
            .get("registries")
            .and_then(|r| r.get(registry))
            .and_then(|r| r.get(tag))
            .and_then(Value::as_array);
        members
            .map(|items| {
                items
                    .iter()
                    .map(|v| v.as_i64().unwrap_or(0) as i32)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Payload of `config_brand` (configuration clientbound `custom_payload`,
    /// id 1): `identifier(minecraft:brand) + string(PocketMine)`.
    /// Without it F3 shows `null` in `Server:`.
    pub fn brand_payload(&self) -> Vec<u8> {
        let mut payload = protocol::identifier(BRAND_CHANNEL);
        payload.extend_from_slice(&protocol::string(BRAND));
        payload
    }

    /// Payload of `tags` / `minecraft:update_tags` (configuration, id 13):
    /// array of registries `{tagType: String, tags: [{tagName: String,
    /// entries: VarInt[]}]}`, cached. Only registries the client has
    /// (sent registries + the static block/item/entity_type/fluid) are
    /// emitted; other entries carry their `tag_members` ids, the rest an
    /// empty list (enough to resolve the key for `getOrThrow`).
    pub fn tags_payload(&self) -> Result<Vec<u8>, ModernError> {
        if let Some(cached) = tags_cache().lock().unwrap().get(&self.protocol) {
            return Ok(cached.clone());
        }
        let raw = tags_json(self.protocol).ok_or_else(|| {
            ModernError::MissingTags(
                self.name.to_string(),
                format!("resources/tags/{}.json", self.protocol),
            )
        })?;
        let decoded: Value = serde_json::from_str(raw).map_err(|_| {
            ModernError::MissingTags(
                self.name.to_string(),
                format!("resources/tags/{}.json", self.protocol),
            )
        })?;
        let registries = decoded
            .get("registries")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                ModernError::MissingTags(
                    self.name.to_string(),
                    format!("resources/tags/{}.json", self.protocol),
                )
            })?;

        // Must run after registry_payloads(): populates registry ids.
        let present: std::collections::HashSet<String> = self.registry_ids()?.into_iter().collect();
        let static_registries = [
            "minecraft:block",
            "minecraft:item",
            "minecraft:entity_type",
            "minecraft:fluid",
        ];
        let mut selected: Vec<(&String, &Value)> = Vec::new();
        for (registry, tags) in registries {
            let id = format!("minecraft:{registry}");
            if present.contains(&id) || static_registries.contains(&id.as_str()) {
                selected.push((registry, tags));
            }
        }

        let mut payload = protocol::var_int(selected.len() as i32);
        for (registry, tags) in selected {
            let id = format!("minecraft:{registry}");
            payload.extend_from_slice(&protocol::identifier(&id));
            let tag_list = tags.as_array();
            let tag_list: &[Value] = tag_list.map(Vec::as_slice).unwrap_or(&[]);
            payload.extend_from_slice(&protocol::var_int(tag_list.len() as i32));
            for tag in tag_list {
                let tag_name = tag.as_str().unwrap_or("");
                payload.extend_from_slice(&protocol::identifier(&format!("minecraft:{tag_name}")));
                payload.extend_from_slice(&protocol::var_int_array(
                    &self.tag_members(registry, tag_name),
                ));
            }
        }
        tags_cache()
            .lock()
            .unwrap()
            .insert(self.protocol, payload.clone());
        Ok(payload)
    }

    /// Payload of `update_time` for this profile.
    ///
    /// Mirrors `JavaClient::sendTime`:
    /// - 769-774: `age: i64 (0) + time: i64 + tickDayTime: bool`
    /// - 775 (26.1): `age: i64 (0) + clockUpdates[] {id: VarInt, totalTicks:
    ///   VarLong, partialTick: f32 (0.0), rate: f32 (1.0 or 0.0)}` with a
    ///   single clock entry (`id = 0`).
    pub fn update_time_payload(&self, time: i64, tick_day_time: bool) -> Vec<u8> {
        if self.protocol >= 775 {
            let mut payload = protocol::long(0);
            payload.extend_from_slice(&protocol::var_int(1));
            payload.extend_from_slice(&protocol::var_int(0));
            payload.extend_from_slice(&protocol::var_long(time));
            payload.extend_from_slice(&protocol::float(0.0f32));
            payload.extend_from_slice(&protocol::float(if tick_day_time {
                1.0f32
            } else {
                0.0f32
            }));
            payload
        } else {
            let mut payload = protocol::long(0);
            payload.extend_from_slice(&protocol::long(time));
            payload.extend_from_slice(&protocol::boolean(tick_day_time));
            payload
        }
    }

    /// Fully framed `update_time` packet (`frame(update_time id, payload)`).
    pub fn update_time_frame(&self, time: i64, tick_day_time: bool) -> Vec<u8> {
        protocol::frame(
            self.id("update_time"),
            &self.update_time_payload(time, tick_day_time),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn supported_protocols_and_names() {
        for (proto, name) in [
            (769, "1.21.4"),
            (770, "1.21.5"),
            (773, "1.21.9"),
            (774, "1.21.11"),
            (775, "26.1"),
        ] {
            assert!(ModernProfile::supports(proto));
            assert_eq!(ModernProfile::for_protocol(proto).unwrap().name, name);
            assert_eq!(ModernProfile::new(proto).unwrap().protocol, proto);
        }
        assert!(!ModernProfile::supports(767));
        assert!(ModernProfile::for_protocol(767).is_none());
        assert!(ModernProfile::new(767).is_err());
        assert_eq!(
            VERSION_NAMES,
            ["1.21.4", "1.21.5", "1.21.9", "1.21.11", "26.1"]
        );
    }

    #[test]
    fn packet_ids_spot_checks() {
        // From ModernProfile::IDS + modern_test.php.
        let cases: &[(i32, &str, i32)] = &[
            (769, "login_success", 2),
            (769, "join_game", 44),
            (769, "update_time", 107),
            (770, "join_game", 43),
            (770, "update_time", 106),
            (773, "join_game", 48),
            (773, "update_time", 111),
            (774, "update_time", 111),
            (775, "join_game", 49),
            (775, "update_time", 113),
            (769, "config_brand", 1),
            (769, "brand", 1),
            (769, "update_tags", 13),
            (769, "tags", 13),
            (769, "spawn_entity", 1),
            (769, "player_info", 64),
            (770, "player_info", 63),
            (773, "player_info", 68),
            (775, "player_info", 70),
            (769, "player_info_update", 64),
            (769, "position_look", 29),
            (769, "look", 30),
            (769, "rel_move", 47),
            (775, "entity_metadata", 99),
        ];
        for (proto, key, expected) in cases {
            assert_eq!(
                ModernProfile::for_protocol(*proto).unwrap().id(key),
                *expected,
                "{proto} {key}"
            );
        }
    }

    #[test]
    #[should_panic(expected = "Paquete desconocido")]
    fn unknown_packet_panics() {
        ModernProfile::for_protocol(769).unwrap().id("nope");
    }

    #[test]
    fn block_states() {
        let p769 = ModernProfile::for_protocol(769).unwrap();
        assert_eq!(p769.block_state("air"), 0);
        assert_eq!(p769.block_state("stone"), 1);
        assert_eq!(p769.block_state("grass_block"), 9);
        assert_eq!(p769.block_state("oak_log"), 137);
        assert_eq!(p769.block_state("redstone_ore"), 5904);
        assert_eq!(p769.block_state("dandelion"), 2118);
        let p775 = ModernProfile::for_protocol(775).unwrap();
        assert_eq!(p775.block_state("redstone_ore"), 6882);
        assert_eq!(p775.block_state("short_grass"), 2248);
        assert_eq!(p775.block_state("tall_grass"), 12924);
        assert_eq!(p775.block_state("snow"), 6919);
        assert_eq!(p775.block_state("poppy"), 2324);
        assert_eq!(p775.block_state("unknown_block"), 0);
    }

    #[test]
    fn version_flags() {
        let p769 = ModernProfile::for_protocol(769).unwrap();
        assert!(p769.heightmaps_as_nbt());
        assert!(p769.palette_data_has_length());
        assert!(!p769.sections_have_fluid_count());
        assert!(!p769.spawn_velocity_is_lp_vec3());
        assert_eq!(p769.player_entity_type(), 147);
        assert_eq!(p769.plains_biome(), 39);

        let p774 = ModernProfile::for_protocol(774).unwrap();
        assert!(!p774.heightmaps_as_nbt());
        assert!(p774.spawn_velocity_is_lp_vec3());
        assert_eq!(p774.player_entity_type(), 155);
        assert_eq!(p774.plains_biome(), 40);

        let p775 = ModernProfile::for_protocol(775).unwrap();
        assert!(p775.sections_have_fluid_count());
        assert_eq!(p775.player_entity_type(), 155);
        assert_eq!(p775.plains_biome(), 40);
    }

    #[test]
    fn brand_payload_vector() {
        for proto in [769, 770, 773, 774, 775] {
            let profile = ModernProfile::for_protocol(proto).unwrap();
            assert_eq!(
                hex(&profile.brand_payload()),
                "0f6d696e6563726166743a6272616e640a506f636b65744d696e65"
            );
            let mut offset = 0;
            assert_eq!(
                protocol::read_string(&profile.brand_payload(), &mut offset).as_deref(),
                Some(BRAND_CHANNEL)
            );
            assert_eq!(
                protocol::read_string(&profile.brand_payload(), &mut offset).as_deref(),
                Some(BRAND)
            );
        }
    }

    #[test]
    fn tag_members_vectors() {
        let profile = ModernProfile::for_protocol(769).unwrap();
        assert_eq!(profile.tag_members("fluid", "water"), vec![1, 2]);
        assert_eq!(profile.tag_members("fluid", "lava"), vec![3, 4]);
        let pickaxe = profile.tag_members("block", "mineable/pickaxe");
        assert!(pickaxe.contains(&1));
        assert!(!profile.tag_members("block", "mineable/shovel").is_empty() || true); // gravel check below is version independent
        let shovel = profile.tag_members("block", "mineable/shovel");
        assert!(shovel.contains(&40));
        assert!(!profile.tag_members("block", "climbable").is_empty());
    }

    #[test]
    fn registry_and_tags_lengths() {
        // Lengths from the PHP implementation (genvec2).
        // (protocol, registry count, first registry len, tags len)
        let expected: &[(i32, usize, usize, usize)] = &[
            (769, 10, 22144, 16515),
            (770, 10, 22144, 16834),
            (773, 9, 22144, 17777),
            (774, 21, 19840, 18261),
            (775, 26, 19840, 20152),
        ];
        for (proto, count, first_len, tags_len) in expected {
            let profile = ModernProfile::for_protocol(*proto).unwrap();
            let regs = profile.registry_payloads().unwrap();
            assert_eq!(regs.len(), *count, "registry count {proto}");
            assert_eq!(regs[0].len(), *first_len, "first registry len {proto}");
            // First registry is worldgen/biome, first entry is badlands.
            let mut offset = 0;
            assert_eq!(
                protocol::read_string(&regs[0], &mut offset).as_deref(),
                Some("minecraft:worldgen/biome")
            );
            // First entry key is badlands: skip string(key)+bool then check NBT type.
            let count = protocol::read_var_int(&regs[0], &mut offset).unwrap();
            assert_eq!(count, if *proto >= 774 { 65 } else { 64 });
            assert_eq!(
                protocol::read_string(&regs[0], &mut offset).as_deref(),
                Some("minecraft:badlands")
            );
            assert_eq!(regs[0][offset], 1); // boolean(true)
            assert_eq!(regs[0][offset + 1], 10); // NBT compound anonymous
            let tags = profile.tags_payload().unwrap();
            assert_eq!(tags.len(), *tags_len, "tags len {proto}");
            // Parse the tags payload header and check for damage_type.
            let mut toffset = 0;
            let registry_count = protocol::read_var_int(&tags, &mut toffset).unwrap();
            assert!(registry_count > 0);
            let mut found_damage = false;
            let mut total_tags = 0i32;
            let mut total_members = 0usize;
            for _ in 0..registry_count {
                let registry_id = protocol::read_string(&tags, &mut toffset).unwrap();
                if registry_id == "minecraft:damage_type" {
                    found_damage = true;
                }
                let tag_count = protocol::read_var_int(&tags, &mut toffset).unwrap();
                total_tags += tag_count;
                for _ in 0..tag_count {
                    protocol::read_string(&tags, &mut toffset).unwrap();
                    let members = protocol::read_var_int_array(&tags, &mut toffset).unwrap();
                    total_members += members.len();
                }
            }
            assert!(found_damage);
            assert!(total_tags > 0);
            assert_eq!(toffset, tags.len(), "tags payload must be fully consumed");
            assert!(total_members > 100, "member total {proto}");
        }
    }

    #[test]
    fn update_time_vectors() {
        // 769-774: long(0) + long(6000) + bool(true).
        let p769 = ModernProfile::for_protocol(769).unwrap();
        assert_eq!(
            hex(&p769.update_time_payload(6000, true)),
            "0000000000000000000000000000177001"
        );
        assert_eq!(
            hex(&p769.update_time_payload(6000, false)),
            "0000000000000000000000000000177000"
        );
        assert_eq!(
            hex(&p769.update_time_payload(13000, true)),
            "000000000000000000000000000032c801"
        );
        // 775: long(0) + varInt(1) + varInt(0) + varLong(6000) + f32(0) + f32(1).
        let p775 = ModernProfile::for_protocol(775).unwrap();
        assert_eq!(
            hex(&p775.update_time_payload(6000, true)),
            "00000000000000000100f02e000000003f800000"
        );
        assert_eq!(
            hex(&p775.update_time_payload(6000, false)),
            "00000000000000000100f02e0000000000000000"
        );
        // Decode the 775 clock format field by field.
        let payload = p775.update_time_payload(6000, true);
        let mut offset = 0;
        assert_eq!(protocol::read_long(&payload, &mut offset), Some(0));
        assert_eq!(protocol::read_var_int(&payload, &mut offset), Some(1));
        assert_eq!(protocol::read_var_int(&payload, &mut offset), Some(0));
        assert_eq!(protocol::read_var_long(&payload, &mut offset), Some(6000));
        assert_eq!(protocol::read_float(&payload, &mut offset), Some(0.0));
        assert_eq!(protocol::read_float(&payload, &mut offset), Some(1.0));
        assert_eq!(offset, payload.len());
        // Framed ids match the tables.
        assert_eq!(p769.id("update_time"), 107);
        assert_eq!(p775.id("update_time"), 113);
        let mut o769 = 0;
        assert_eq!(
            protocol::read_var_int(&p769.update_time_frame(6000, true), &mut o769),
            Some(18)
        );
    }
}
