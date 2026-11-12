//! Modern packet-id table ported from `ModernProfile.php` (minecraft-data
//! `protocol.json` per version). Chunk *bytes* are not here: they come from
//! [`crate::hooks::ServerHooks::modern_chunk`] so the `ModernChunk` agent
//! can supply them without this crate depending on that crate yet.

use crate::wire::{identifier, string};

/// Supported modern protocols: protocol -> version name.
pub fn version_name(protocol: i32) -> Option<&'static str> {
    match protocol {
        769 => Some("1.21.4"),
        770 => Some("1.21.5"),
        773 => Some("1.21.9"),
        774 => Some("1.21.11"),
        775 => Some("26.1"),
        _ => None,
    }
}

pub fn is_modern(protocol: i32) -> bool {
    version_name(protocol).is_some()
}

pub const VERSION_NAMES: [&str; 5] = ["1.21.4", "1.21.5", "1.21.9", "1.21.11", "26.1"];

/// Primary / prioritized Java version protocol (26.1).
pub const PRIMARY_PROTOCOL: i32 = 775;
pub const PRIMARY_VERSION_NAME: &str = "26.1";

/// Server brand shown in F3 (`Server:`), sent in configuration.
pub const BRAND: &str = "PocketMine";
pub const BRAND_CHANNEL: &str = "minecraft:brand";

/// Packet id for a logical name in a modern protocol. Returns `None` for
/// unknown names/protocols (PHP throws `InvalidArgumentException`).
pub fn packet_id(protocol: i32, key: &str) -> Option<i32> {
    let lookup = match key {
        "position_look" => "sb_position_look",
        "look" => "sb_look",
        "player_info_update" => "player_info",
        "rel_move" => "rel_entity_move",
        "brand" => "config_brand",
        other => other,
    };
    let table = match protocol {
        769 => TABLE_769,
        770 => TABLE_770,
        773 => TABLE_773,
        774 => TABLE_774,
        775 => TABLE_775,
        _ => return None,
    };
    table
        .iter()
        .find(|(name, _)| *name == lookup)
        .map(|(_, id)| *id)
        .or_else(|| {
            table
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, id)| *id)
        })
}

/// `minecraft:player` entity-type index per version (entities.json).
pub fn player_entity_type(protocol: i32) -> i32 {
    match protocol {
        769 => 147,
        770 => 148,
        773 => 151,
        774 | 775 => 155,
        _ => 147,
    }
}

/// 1.21.4 sends heightmaps as NBT; 1.21.5+ as typed lists.
pub fn heightmaps_as_nbt(protocol: i32) -> bool {
    protocol == 769
}

/// 1.21.5 dropped the length prefix of palette long arrays.
pub fn palette_data_has_length(protocol: i32) -> bool {
    protocol == 769
}

/// 26.1 added a fluid counter per section.
pub fn sections_have_fluid_count(protocol: i32) -> bool {
    protocol >= 775
}

/// In `spawn_entity`, velocity is `lpVec3` (middle) from 1.21.9 (773+);
/// 769/770 carry it as trailing `vec3i16`.
pub fn spawn_velocity_is_lp_vec3(protocol: i32) -> bool {
    protocol >= 773
}

/// `config_brand` payload: channel + brand string (`custom_payload` id 1
/// in configuration, 769-775).
pub fn brand_payload() -> Vec<u8> {
    let mut out = identifier(BRAND_CHANNEL);
    out.extend_from_slice(&string(BRAND));
    out
}

const TABLE_769: &[(&str, i32)] = &[
    ("login_success", 2),
    ("join_game", 44),
    ("position", 66),
    ("map_chunk", 40),
    ("game_state_change", 35),
    ("abilities", 58),
    ("system_chat", 115),
    ("block_change", 9),
    ("unload_chunk", 34),
    ("kick", 29),
    ("keep_alive", 39),
    ("chunk_batch_start", 13),
    ("chunk_batch_finished", 12),
    ("registry_data", 7),
    ("finish_configuration", 3),
    ("feature_flags", 12),
    ("tags", 13),
    ("update_tags", 13),
    ("config_brand", 1),
    ("login_acknowledged", 3),
    ("config_settings", 0),
    ("config_custom_payload", 2),
    ("config_select_known_packs", 7),
    ("config_finish", 3),
    ("chat_message", 7),
    ("chat_command", 5),
    ("sb_position", 28),
    ("sb_position_look", 29),
    ("sb_look", 30),
    ("client_command", 10),
    ("block_dig", 39),
    ("block_place", 60),
    ("held_item_slot", 51),
    ("set_creative_slot", 54),
    ("entity_action", 40),
    ("player_input", 41),
    ("teleport_confirm", 0),
    ("chunk_batch_received", 9),
    ("player_loaded", 42),
    ("spawn_entity", 1),
    ("rel_entity_move", 47),
    ("entity_move_look", 48),
    ("entity_look", 50),
    ("entity_head_rotation", 77),
    ("entity_destroy", 71),
    ("player_info", 64),
    ("player_remove", 63),
    ("entity_metadata", 93),
    ("entity_teleport", 119),
    ("sync_entity_position", 32),
    ("update_view_position", 88),
    ("update_view_distance", 89),
    ("update_time", 107),
    ("boss_bar", 0x0A),
    ("scoreboard_objective", 100),
    ("scoreboard_score", 104),
    ("scoreboard_display_objective", 92),
    ("reset_score", 73),
    ("world_event", 41),
];

const TABLE_770: &[(&str, i32)] = &[
    ("login_success", 2),
    ("join_game", 43),
    ("position", 65),
    ("map_chunk", 39),
    ("game_state_change", 34),
    ("abilities", 57),
    ("system_chat", 114),
    ("block_change", 8),
    ("unload_chunk", 33),
    ("kick", 28),
    ("keep_alive", 38),
    ("chunk_batch_start", 12),
    ("chunk_batch_finished", 11),
    ("registry_data", 7),
    ("finish_configuration", 3),
    ("feature_flags", 12),
    ("tags", 13),
    ("update_tags", 13),
    ("config_brand", 1),
    ("login_acknowledged", 3),
    ("config_settings", 0),
    ("config_custom_payload", 2),
    ("config_select_known_packs", 7),
    ("config_finish", 3),
    ("chat_message", 7),
    ("chat_command", 5),
    ("sb_position", 28),
    ("sb_position_look", 29),
    ("sb_look", 30),
    ("client_command", 10),
    ("block_dig", 39),
    ("block_place", 62),
    ("held_item_slot", 51),
    ("set_creative_slot", 54),
    ("entity_action", 40),
    ("player_input", 41),
    ("teleport_confirm", 0),
    ("chunk_batch_received", 9),
    ("player_loaded", 42),
    ("spawn_entity", 1),
    ("rel_entity_move", 46),
    ("entity_move_look", 47),
    ("entity_look", 49),
    ("entity_head_rotation", 76),
    ("entity_destroy", 70),
    ("player_info", 63),
    ("player_remove", 62),
    ("entity_metadata", 92),
    ("entity_teleport", 118),
    ("sync_entity_position", 31),
    ("update_view_position", 87),
    ("update_view_distance", 88),
    ("update_time", 106),
    ("boss_bar", 0x09),
    ("scoreboard_objective", 99),
    ("scoreboard_score", 103),
    ("scoreboard_display_objective", 91),
    ("reset_score", 72),
    ("world_event", 40),
];

const TABLE_773: &[(&str, i32)] = &[
    ("login_success", 2),
    ("join_game", 48),
    ("position", 70),
    ("map_chunk", 44),
    ("game_state_change", 38),
    ("abilities", 62),
    ("system_chat", 119),
    ("block_change", 8),
    ("unload_chunk", 37),
    ("kick", 32),
    ("keep_alive", 43),
    ("chunk_batch_start", 12),
    ("chunk_batch_finished", 11),
    ("registry_data", 7),
    ("finish_configuration", 3),
    ("feature_flags", 12),
    ("tags", 13),
    ("update_tags", 13),
    ("config_brand", 1),
    ("login_acknowledged", 3),
    ("config_settings", 0),
    ("config_custom_payload", 2),
    ("config_select_known_packs", 7),
    ("config_finish", 3),
    ("chat_message", 8),
    ("chat_command", 6),
    ("sb_position", 29),
    ("sb_position_look", 30),
    ("sb_look", 31),
    ("client_command", 11),
    ("block_dig", 40),
    ("block_place", 63),
    ("held_item_slot", 52),
    ("set_creative_slot", 55),
    ("entity_action", 41),
    ("player_input", 42),
    ("teleport_confirm", 0),
    ("chunk_batch_received", 10),
    ("player_loaded", 43),
    ("spawn_entity", 1),
    ("rel_entity_move", 51),
    ("entity_move_look", 52),
    ("entity_look", 54),
    ("entity_head_rotation", 81),
    ("entity_destroy", 75),
    ("player_info", 68),
    ("player_remove", 67),
    ("entity_metadata", 97),
    ("entity_teleport", 123),
    ("sync_entity_position", 35),
    ("update_view_position", 92),
    ("update_view_distance", 93),
    ("update_time", 111),
    ("boss_bar", 0x09),
    ("scoreboard_objective", 104),
    ("scoreboard_score", 108),
    ("scoreboard_display_objective", 96),
    ("reset_score", 77),
    ("world_event", 45),
];

const TABLE_774: &[(&str, i32)] = &[
    ("login_success", 2),
    ("join_game", 48),
    ("position", 70),
    ("map_chunk", 44),
    ("game_state_change", 38),
    ("abilities", 62),
    ("system_chat", 119),
    ("block_change", 8),
    ("unload_chunk", 37),
    ("kick", 32),
    ("keep_alive", 43),
    ("chunk_batch_start", 12),
    ("chunk_batch_finished", 11),
    ("registry_data", 7),
    ("finish_configuration", 3),
    ("feature_flags", 12),
    ("tags", 13),
    ("update_tags", 13),
    ("config_brand", 1),
    ("login_acknowledged", 3),
    ("config_settings", 0),
    ("config_custom_payload", 2),
    ("config_select_known_packs", 7),
    ("config_finish", 3),
    ("chat_message", 8),
    ("chat_command", 6),
    ("sb_position", 29),
    ("sb_position_look", 30),
    ("sb_look", 31),
    ("client_command", 11),
    ("block_dig", 40),
    ("block_place", 63),
    ("held_item_slot", 52),
    ("set_creative_slot", 55),
    ("entity_action", 41),
    ("player_input", 42),
    ("teleport_confirm", 0),
    ("chunk_batch_received", 10),
    ("player_loaded", 43),
    ("spawn_entity", 1),
    ("rel_entity_move", 51),
    ("entity_move_look", 52),
    ("entity_look", 54),
    ("entity_head_rotation", 81),
    ("entity_destroy", 75),
    ("player_info", 68),
    ("player_remove", 67),
    ("entity_metadata", 97),
    ("entity_teleport", 123),
    ("sync_entity_position", 35),
    ("update_view_position", 92),
    ("update_view_distance", 93),
    ("update_time", 111),
    ("boss_bar", 0x09),
    ("scoreboard_objective", 104),
    ("scoreboard_score", 108),
    ("scoreboard_display_objective", 96),
    ("reset_score", 77),
    ("world_event", 45),
];

const TABLE_775: &[(&str, i32)] = &[
    ("login_success", 2),
    ("join_game", 49),
    ("position", 72),
    ("map_chunk", 45),
    ("game_state_change", 38),
    ("abilities", 64),
    ("system_chat", 121),
    ("block_change", 8),
    ("unload_chunk", 37),
    ("kick", 32),
    ("keep_alive", 44),
    ("chunk_batch_start", 12),
    ("chunk_batch_finished", 11),
    ("registry_data", 7),
    ("finish_configuration", 3),
    ("feature_flags", 12),
    ("tags", 13),
    ("update_tags", 13),
    ("config_brand", 1),
    ("login_acknowledged", 3),
    ("config_settings", 0),
    ("config_custom_payload", 2),
    ("config_select_known_packs", 7),
    ("config_finish", 3),
    ("chat_message", 9),
    ("chat_command", 7),
    ("sb_position", 30),
    ("sb_position_look", 31),
    ("sb_look", 32),
    ("client_command", 12),
    ("block_dig", 41),
    ("block_place", 66),
    ("held_item_slot", 53),
    ("set_creative_slot", 56),
    ("entity_action", 42),
    ("player_input", 43),
    ("teleport_confirm", 0),
    ("chunk_batch_received", 11),
    ("player_loaded", 44),
    ("spawn_entity", 1),
    ("rel_entity_move", 53),
    ("entity_move_look", 54),
    ("entity_look", 56),
    ("entity_head_rotation", 83),
    ("entity_destroy", 77),
    ("player_info", 70),
    ("player_remove", 69),
    ("entity_metadata", 99),
    ("entity_teleport", 125),
    ("sync_entity_position", 35),
    ("update_view_position", 94),
    ("update_view_distance", 95),
    ("update_time", 113),
    ("boss_bar", 0x09),
    ("scoreboard_objective", 106),
    ("scoreboard_score", 110),
    ("scoreboard_display_objective", 98),
    ("reset_score", 79),
    ("world_event", 46),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_ids_spot_checks() {
        // Tags 0x0d / brand 0x01 in every modern version.
        for protocol in [769, 770, 773, 774, 775] {
            assert_eq!(packet_id(protocol, "update_tags"), Some(13));
            assert_eq!(packet_id(protocol, "config_brand"), Some(1));
            assert_eq!(packet_id(protocol, "brand"), Some(1));
            assert_eq!(packet_id(protocol, "spawn_entity"), Some(1));
            // player_info_update is the alias of player_info (0x04 is its
            // gamemode bitflag, asserted on the wire in session tests).
            assert_eq!(
                packet_id(protocol, "player_info_update"),
                packet_id(protocol, "player_info")
            );
        }
        assert_eq!(packet_id(769, "player_info"), Some(64));
        assert_eq!(packet_id(775, "player_info"), Some(70));
        assert_eq!(packet_id(769, "entity_metadata"), Some(93));
        assert_eq!(packet_id(775, "entity_metadata"), Some(99));
        assert_eq!(packet_id(775, "update_time"), Some(113));
        assert_eq!(packet_id(340, "join_game"), None);
    }

    #[test]
    fn spawn_velocity_and_entity_type() {
        assert!(!spawn_velocity_is_lp_vec3(769));
        assert!(!spawn_velocity_is_lp_vec3(770));
        assert!(spawn_velocity_is_lp_vec3(773));
        assert!(spawn_velocity_is_lp_vec3(775));
        assert_eq!(player_entity_type(769), 147);
        assert_eq!(player_entity_type(775), 155);
        assert!(sections_have_fluid_count(775));
        assert!(!sections_have_fluid_count(774));
    }

    #[test]
    fn brand_payload_shape() {
        let payload = brand_payload();
        let mut r = crate::wire::Reader::new(&payload);
        assert_eq!(r.read_string().as_deref(), Some(BRAND_CHANNEL));
        assert_eq!(r.read_string().as_deref(), Some(BRAND));
        assert_eq!(r.remaining(), 0);
    }
}
