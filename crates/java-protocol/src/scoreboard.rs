//! Java Edition scoreboard payloads, ported from minecraft-data
//! `protocol.json` (`packet_scoreboard_objective`,
//! `packet_scoreboard_score`, `packet_scoreboard_display_objective`,
//! `packet_reset_score`) plus the legacy 1.12.2 (protocol 340) layout
//! in `target/javamc/protocol.json`.
//!
//! Packet ids are per-protocol (see `ModernProfile` tables and
//! `java-bridge::profile`): `scoreboard_objective` is 100/99/104/104/106
//! and `scoreboard_score` 104/103/108/108/110 for
//! 769/770/773/774/775. Legacy 340 uses 0x42/0x45/0x3B (not 0x49/0x4A/
//! 0x4B: 0x49 is `sound_effect` in 340).
//!
//! Display positions use the Java ordering (list=0, sidebar=1,
//! below-name=2); `server-core::scoreboard::ScoreboardSlot` maps it.

use crate::nbt;
use crate::protocol;

// --- legacy 1.12.2 (340) clientbound ids -------------------------------
pub const S2C_SCOREBOARD_OBJECTIVE: i32 = 0x42;
pub const S2C_SCOREBOARD_SCORE: i32 = 0x45;
pub const S2C_SCOREBOARD_DISPLAY_OBJECTIVE: i32 = 0x3B;

// --- modern action bytes ------------------------------------------------
pub const ACTION_CREATE: i32 = 0;
pub const ACTION_REMOVE: i32 = 1;
pub const ACTION_UPDATE_DISPLAY: i32 = 2;

/// Render type `integer` (scores are numbers, not hearts).
pub const RENDER_TYPE_INTEGER: i32 = 0;
/// Criteria PocketMine sends for plugin objectives (legacy only).
pub const CRITERIA_DUMMY: &str = "dummy";

// --- modern payloads -----------------------------------------------------

/// `scoreboard_objective` create (action 0) / update (action 2) for
/// 769-775: name, action, displayText (anonymous NBT text), render
/// type (varint), number_format (absent => default integer formatting,
/// which also means no styling NBT follows).
pub fn modern_objective_payload(name: &str, display_name: &str, action: i32) -> Vec<u8> {
    let mut payload = protocol::string(name);
    payload.extend_from_slice(&protocol::byte(action));
    payload.extend_from_slice(&nbt::text(display_name));
    payload.extend_from_slice(&protocol::var_int(RENDER_TYPE_INTEGER));
    payload.extend_from_slice(&protocol::boolean(false));
    payload
}

pub fn modern_objective_create(name: &str, display_name: &str) -> Vec<u8> {
    modern_objective_payload(name, display_name, ACTION_CREATE)
}

pub fn modern_objective_update(name: &str, display_name: &str) -> Vec<u8> {
    modern_objective_payload(name, display_name, ACTION_UPDATE_DISPLAY)
}

/// `scoreboard_objective` remove (action 1): name only.
pub fn modern_objective_remove(name: &str) -> Vec<u8> {
    let mut payload = protocol::string(name);
    payload.extend_from_slice(&protocol::byte(ACTION_REMOVE));
    payload
}

/// `scoreboard_score` (769-775): objective, entry name, value, no
/// display_name override, no number_format (default integer).
pub fn modern_score_payload(objective: &str, entry: &str, value: i32) -> Vec<u8> {
    let mut payload = protocol::string(entry);
    payload.extend_from_slice(&protocol::string(objective));
    payload.extend_from_slice(&protocol::var_int(value));
    payload.extend_from_slice(&protocol::boolean(true));
    payload.extend_from_slice(&nbt::text(entry));
    payload.extend_from_slice(&protocol::boolean(false));
    payload
}

/// `reset_score` (769-775): entry name + objective (present). This is
/// the modern score-removal packet (the old `scoreboard_score` action
/// field no longer exists).
pub fn modern_reset_score_payload(entry: &str, objective: &str) -> Vec<u8> {
    let mut payload = protocol::string(entry);
    payload.extend_from_slice(&protocol::boolean(true));
    payload.extend_from_slice(&protocol::string(objective));
    payload
}

/// `scoreboard_display_objective`: Java position (list=0, sidebar=1,
/// below-name=2) + objective name. An empty name hides the slot.
pub fn modern_display_payload(java_position: i32, objective: &str) -> Vec<u8> {
    let mut payload = protocol::var_int(java_position);
    payload.extend_from_slice(&protocol::string(objective));
    payload
}

// --- legacy 1.12.2 (340) payloads ----------------------------------------

/// `scoreboard_objective` (0x42) create/update: name, action,
/// displayText string, criteria string (`"dummy"`).
pub fn legacy_objective_payload(name: &str, display_name: &str, action: i32) -> Vec<u8> {
    let mut payload = protocol::string(name);
    payload.extend_from_slice(&protocol::byte(action));
    payload.extend_from_slice(&protocol::string(display_name));
    payload.extend_from_slice(&protocol::string(CRITERIA_DUMMY));
    payload
}

pub fn legacy_objective_create(name: &str, display_name: &str) -> Vec<u8> {
    legacy_objective_payload(name, display_name, ACTION_CREATE)
}

pub fn legacy_objective_update(name: &str, display_name: &str) -> Vec<u8> {
    legacy_objective_payload(name, display_name, ACTION_UPDATE_DISPLAY)
}

/// `scoreboard_objective` (0x42) remove (action 1): name only.
pub fn legacy_objective_remove(name: &str) -> Vec<u8> {
    let mut payload = protocol::string(name);
    payload.extend_from_slice(&protocol::byte(ACTION_REMOVE));
    payload
}

/// `scoreboard_score` (0x45) create/update: objective, action varint
/// (0), entry name, value varint. Removal omits the value.
pub fn legacy_score_payload(objective: &str, entry: &str, value: i32) -> Vec<u8> {
    let mut payload = protocol::string(entry);
    payload.extend_from_slice(&protocol::var_int(0));
    payload.extend_from_slice(&protocol::string(objective));
    payload.extend_from_slice(&protocol::var_int(value));
    payload
}

pub fn legacy_score_remove_payload(objective: &str, entry: &str) -> Vec<u8> {
    let mut payload = protocol::string(entry);
    payload.extend_from_slice(&protocol::var_int(1));
    payload.extend_from_slice(&protocol::string(objective));
    payload
}

/// `scoreboard_display_objective` (0x3B): position byte (Java order)
/// + objective name.
pub fn legacy_display_payload(java_position: i32, objective: &str) -> Vec<u8> {
    let mut payload = protocol::byte(java_position);
    payload.extend_from_slice(&protocol::string(objective));
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModernProfile;

    #[test]
    fn modern_ids_match_minecraft_data() {
        let expected = [
            (769, 100, 104, 92, 73),
            (770, 99, 103, 91, 72),
            (773, 104, 108, 96, 77),
            (774, 104, 108, 96, 77),
            (775, 106, 110, 98, 79),
        ];
        for (proto, objective, score, display, reset) in expected {
            let profile = ModernProfile::for_protocol(proto).unwrap();
            assert_eq!(profile.id("scoreboard_objective"), objective, "objective {proto}");
            assert_eq!(profile.id("scoreboard_score"), score, "score {proto}");
            assert_eq!(
                profile.id("scoreboard_display_objective"),
                display,
                "display {proto}"
            );
            assert_eq!(profile.id("reset_score"), reset, "reset {proto}");
        }
    }

    #[test]
    fn legacy_ids_are_340_layout_not_0x49() {
        assert_eq!(S2C_SCOREBOARD_OBJECTIVE, 0x42);
        assert_eq!(S2C_SCOREBOARD_SCORE, 0x45);
        assert_eq!(S2C_SCOREBOARD_DISPLAY_OBJECTIVE, 0x3B);
    }

    #[test]
    fn modern_objective_create_decodes() {
        let payload = modern_objective_create("main", "Números");
        let mut offset = 0;
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "main");
        assert_eq!(payload[offset], 0, "action create");
        offset += 1;
        // Anonymous NBT compound: 0x0A ... 0x00 terminator.
        assert_eq!(payload[offset], 0x0A);
        assert!(payload.ends_with(&[0x00, 0x00]));
    }

    #[test]
    fn modern_objective_remove_is_name_plus_action() {
        let payload = modern_objective_remove("main");
        let mut offset = 0;
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "main");
        assert_eq!(payload[offset], 1);
        assert_eq!(offset + 1, payload.len());
    }

    #[test]
    fn modern_score_and_reset_and_display_decode() {
        let payload = modern_score_payload("main", "Oro", 100);
        let mut offset = 0;
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "Oro");
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "main");
        assert_eq!(protocol::read_var_int(&payload, &mut offset).unwrap(), 100);
        assert_eq!(payload[offset], 1, "display_name override present");
        offset += 1;
        assert_eq!(payload[offset], 0x0A, "anonymous NBT compound start");
        let nbt_bytes = nbt::text("Oro");
        assert_eq!(&payload[offset..offset + nbt_bytes.len()], &nbt_bytes);
        offset += nbt_bytes.len();
        assert_eq!(payload[offset], 0, "no number_format");
        assert_eq!(offset + 1, payload.len());

        let payload = modern_reset_score_payload("Oro", "main");
        let mut offset = 0;
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "Oro");
        assert_eq!(payload[offset], 1, "objective present");
        offset += 1;
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "main");
        assert_eq!(offset, payload.len());

        let payload = modern_display_payload(1, "main");
        let mut offset = 0;
        assert_eq!(protocol::read_var_int(&payload, &mut offset).unwrap(), 1);
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "main");
    }

    #[test]
    fn legacy_payloads_decode() {
        let payload = legacy_objective_create("main", "Números");
        let mut offset = 0;
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "main");
        assert_eq!(payload[offset], 0);
        offset += 1;
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "Números");
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "dummy");
        assert_eq!(offset, payload.len());

        let payload = legacy_score_payload("main", "Oro", 100);
        let mut offset = 0;
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "Oro");
        assert_eq!(protocol::read_var_int(&payload, &mut offset).unwrap(), 0);
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "main");
        assert_eq!(protocol::read_var_int(&payload, &mut offset).unwrap(), 100);

        let payload = legacy_score_remove_payload("main", "Oro");
        let mut offset = 0;
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "Oro");
        assert_eq!(protocol::read_var_int(&payload, &mut offset).unwrap(), 1);
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "main");
        assert_eq!(offset, payload.len());

        let payload = legacy_display_payload(0, "main");
        assert_eq!(payload[0], 0, "list position byte");
        let mut offset = 1;
        assert_eq!(protocol::read_string(&payload, &mut offset).unwrap(), "main");
    }
}
