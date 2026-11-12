//! Java Edition `boss_bar` packets (apiBossBar, Java side).
//!
//! Layout is `PrismarineJS/minecraft-data` `packet_boss_bar` (verified
//! against `target/javamc*/protocol.json`):
//!
//! ```text
//! entityUUID : UUID (16 raw bytes, big-endian)
//! action     : VarInt
//!   0 ADD            : title, health f32, color VarInt, dividers VarInt, flags u8
//!   1 REMOVE         : —
//!   2 UPDATE_HEALTH  : health f32
//!   3 UPDATE_TITLE   : title
//!   4 UPDATE_STYLE   : color VarInt, dividers VarInt
//!   5 UPDATE_FLAGS   : flags u8
//! ```
//!
//! `title` is a plain `string` on legacy 1.12.2 (340) and an
//! `anonymousNbt` text component (`{"text": ...}`) on modern 1.21.4+
//! (769-775), exactly like `system_chat`.
//!
//! Packet ids (S2C play, `play/toClient` mappings):
//!
//! | protocol | version | `boss_bar` id |
//! |----------|---------|---------------:|
//! | 340      | 1.12.2  | `0x0C` (12)   |
//! | 769      | 1.21.4  | `0x0A` (10)   |
//! | 770      | 1.21.5  | `0x09` (9)    |
//! | 773      | 1.21.9  | `0x09` (9)    |
//! | 774      | 1.21.11 | `0x09` (9)    |
//! | 775      | 26.1    | `0x09` (9)    |
//!
//! The task's "BossBar packet 0x0C java?" is therefore the legacy id only;
//! modern profiles use 0x0A/0x09. Colours here are the Java 7-colour set
//! (`0 pink, 1 blue, 2 red, 3 green, 4 yellow, 5 purple, 6 white`) —
//! Bedrock adds `6 rebecca_purple, 7 white`, so [`bedrock_color_to_java`]
//! maps `6 -> 5 (purple)` and clamps the rest.
//!
//! `dividers` (Bedrock `overlay`) are `0 progress, 1 10-seg, 2 20-seg,
//! 3 6-seg, 4 4-seg, 5 3-seg`. `flags` bits: `1 darken screen`,
//! `2 play music`, `4 create world fog` (`FLAG_*` below).

use crate::{nbt, protocol};

/// Legacy 1.12.2 / 340 `boss_bar` id.
pub const S2C_BOSS_BAR_LEGACY: i32 = 0x0C;
/// 1.21.4 / 769 `boss_bar` id.
pub const S2C_BOSS_BAR_769: i32 = 0x0A;
/// 1.21.5+ / 770-775 `boss_bar` id.
pub const S2C_BOSS_BAR_770_PLUS: i32 = 0x09;

/// `boss_bar` actions (`action` field).
pub const ACTION_ADD: i32 = 0;
pub const ACTION_REMOVE: i32 = 1;
pub const ACTION_UPDATE_HEALTH: i32 = 2;
pub const ACTION_UPDATE_TITLE: i32 = 3;
pub const ACTION_UPDATE_STYLE: i32 = 4;
pub const ACTION_UPDATE_FLAGS: i32 = 5;

/// Java boss-bar colours (`0 pink .. 6 white`).
pub const COLOR_PINK: i32 = 0;
pub const COLOR_BLUE: i32 = 1;
pub const COLOR_RED: i32 = 2;
pub const COLOR_GREEN: i32 = 3;
pub const COLOR_YELLOW: i32 = 4;
pub const COLOR_PURPLE: i32 = 5;
pub const COLOR_WHITE: i32 = 6;

/// `flags` bits.
pub const FLAG_DARKEN_SCREEN: u8 = 0x01;
pub const FLAG_PLAY_MUSIC: u8 = 0x02;
pub const FLAG_CREATE_WORLD_FOG: u8 = 0x04;

/// `boss_bar` packet id for `protocol`, or `None` when unknown.
pub fn boss_bar_packet_id(protocol: i32) -> Option<i32> {
    match protocol {
        340 => Some(S2C_BOSS_BAR_LEGACY),
        769 => Some(S2C_BOSS_BAR_769),
        770 | 773 | 774 | 775 => Some(S2C_BOSS_BAR_770_PLUS),
        _ => None,
    }
}

/// Whether `protocol` encodes titles as anonymous-NBT text components
/// (modern) instead of plain strings (legacy 340).
pub fn titles_are_nbt(protocol: i32) -> bool {
    protocol != 340
}

/// Map a Bedrock `BossBarColor` byte (0..7) to the Java colour set (0..6).
/// Bedrock `6 rebecca_purple` has no Java equivalent and falls back to
/// purple; out-of-range values clamp to pink..purple by index.
pub fn bedrock_color_to_java(bedrock: u8) -> i32 {
    match bedrock {
        0 => COLOR_PINK,
        1 => COLOR_BLUE,
        2 => COLOR_RED,
        3 => COLOR_GREEN,
        4 => COLOR_YELLOW,
        5 | 6 => COLOR_PURPLE,
        _ => COLOR_WHITE,
    }
}

/// Deterministic 16-byte boss-bar UUID for a Bedrock boss actor id, so
/// the same bar keeps the same Java UUID across the ADD and the later
/// UPDATE/REMOVE frames. SplitMix64 over `boss_actor_unique_id` with the
/// version-3 / variant bits set like [`protocol::offline_uuid`].
pub fn bossbar_uuid(boss_actor_unique_id: i64) -> [u8; 16] {
    fn splitmix(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    let high = splitmix(boss_actor_unique_id as u64);
    let low = splitmix(high ^ 0xB055_BA2B_1A1C_3E5D);
    let mut out = high.to_be_bytes().to_vec();
    out.extend_from_slice(&low.to_be_bytes());
    let mut uuid = [0u8; 16];
    uuid.copy_from_slice(&out);
    uuid[6] = (uuid[6] & 0x0F) | 0x30;
    uuid[8] = (uuid[8] & 0x3F) | 0x80;
    uuid
}

/// Legacy title field (plain string).
fn title_legacy(title: &str) -> Vec<u8> {
    protocol::string(title)
}

/// Modern title field (anonymous-NBT `{"text": ...}`).
fn title_modern(title: &str) -> Vec<u8> {
    nbt::text(title)
}

fn title_for(protocol: i32, title: &str) -> Vec<u8> {
    if titles_are_nbt(protocol) {
        title_modern(title)
    } else {
        title_legacy(title)
    }
}

fn header(uuid: &[u8; 16], action: i32) -> Vec<u8> {
    let mut out = uuid.to_vec();
    out.extend_from_slice(&protocol::var_int(action));
    out
}

/// `ADD` payload: title + health + color + dividers + flags.
pub fn add_payload(
    uuid: &[u8; 16],
    protocol: i32,
    title: &str,
    health: f32,
    color: i32,
    dividers: i32,
    flags: u8,
) -> Vec<u8> {
    let mut out = header(uuid, ACTION_ADD);
    out.extend_from_slice(&title_for(protocol, title));
    out.extend_from_slice(&protocol::float(health.clamp(0.0, 1.0)));
    out.extend_from_slice(&protocol::var_int(color));
    out.extend_from_slice(&protocol::var_int(dividers));
    out.extend_from_slice(&protocol::ubyte(flags as i32));
    out
}

/// `REMOVE` payload (header only).
pub fn remove_payload(uuid: &[u8; 16]) -> Vec<u8> {
    header(uuid, ACTION_REMOVE)
}

/// `UPDATE_HEALTH` payload.
pub fn update_health_payload(uuid: &[u8; 16], health: f32) -> Vec<u8> {
    let mut out = header(uuid, ACTION_UPDATE_HEALTH);
    out.extend_from_slice(&protocol::float(health.clamp(0.0, 1.0)));
    out
}

/// `UPDATE_TITLE` payload.
pub fn update_title_payload(uuid: &[u8; 16], protocol: i32, title: &str) -> Vec<u8> {
    let mut out = header(uuid, ACTION_UPDATE_TITLE);
    out.extend_from_slice(&title_for(protocol, title));
    out
}

/// `UPDATE_STYLE` payload (color + dividers).
pub fn update_style_payload(uuid: &[u8; 16], color: i32, dividers: i32) -> Vec<u8> {
    let mut out = header(uuid, ACTION_UPDATE_STYLE);
    out.extend_from_slice(&protocol::var_int(color));
    out.extend_from_slice(&protocol::var_int(dividers));
    out
}

/// `UPDATE_FLAGS` payload.
pub fn update_flags_payload(uuid: &[u8; 16], flags: u8) -> Vec<u8> {
    let mut out = header(uuid, ACTION_UPDATE_FLAGS);
    out.extend_from_slice(&protocol::ubyte(flags as i32));
    out
}

/// Full framed packet for `add` (len + id + payload).
pub fn add_frame(
    uuid: &[u8; 16],
    protocol: i32,
    title: &str,
    health: f32,
    color: i32,
    dividers: i32,
    flags: u8,
) -> Option<Vec<u8>> {
    let id = boss_bar_packet_id(protocol)?;
    Some(protocol::frame(
        id,
        &add_payload(uuid, protocol, title, health, color, dividers, flags),
    ))
}

/// Full framed packet for `remove`.
pub fn remove_frame(uuid: &[u8; 16], protocol: i32) -> Option<Vec<u8>> {
    let id = boss_bar_packet_id(protocol)?;
    Some(protocol::frame(id, &remove_payload(uuid)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid() -> [u8; 16] {
        bossbar_uuid(42)
    }

    #[test]
    fn packet_ids_match_minecraft_data() {
        assert_eq!(boss_bar_packet_id(340), Some(0x0C));
        assert_eq!(boss_bar_packet_id(769), Some(0x0A));
        assert_eq!(boss_bar_packet_id(770), Some(0x09));
        assert_eq!(boss_bar_packet_id(773), Some(0x09));
        assert_eq!(boss_bar_packet_id(774), Some(0x09));
        assert_eq!(boss_bar_packet_id(775), Some(0x09));
        assert_eq!(boss_bar_packet_id(1), None);
    }

    #[test]
    fn uuid_is_deterministic_and_versioned() {
        assert_eq!(bossbar_uuid(42), bossbar_uuid(42));
        assert_ne!(bossbar_uuid(42), bossbar_uuid(43));
        let uuid = uuid();
        assert_eq!(uuid[6] >> 4, 0x3);
        assert_eq!(uuid[8] >> 6, 0b10);
    }

    #[test]
    fn add_payload_layout_legacy_vs_modern_title() {
        let uuid = uuid();
        // Legacy: uuid(16) + action VarInt(0->00) + string len+bytes + f32 +
        // color VarInt + dividers VarInt + flags u8.
        let legacy = add_payload(&uuid, 340, "Hi", 0.5, COLOR_RED, 0, 0);
        assert_eq!(&legacy[0..16], &uuid);
        assert_eq!(legacy[16], 0x00, "action 0");
        assert_eq!(legacy[17], 2, "string length");
        assert_eq!(&legacy[18..20], b"Hi");
        assert_eq!(&legacy[20..24], 0.5f32.to_be_bytes());
        assert_eq!(legacy[24], COLOR_RED as u8);
        assert_eq!(legacy[25], 0, "dividers");
        assert_eq!(legacy[26], 0, "flags");
        // Modern: same header, but the title slot is anonymous NBT
        // (compound 10 + string tag 8 + "text"), not a plain string.
        let modern = add_payload(&uuid, 775, "Hi", 0.5, COLOR_RED, 0, 0);
        assert_eq!(&modern[0..16], &uuid);
        assert_eq!(modern[16], 0x00);
        assert_eq!(modern[17], 10, "NBT compound");
        assert_eq!(modern[18], 8, "NBT string tag");
        assert_eq!(&modern[21..25], b"text");
        // Health still big-endian f32 after the NBT title.
        // NBT: compound tag(1) + string tag(1) + name len(2) + "text"(4)
        // + value len(2) + "Hi"(2) + END(1) = 13 bytes.
        let nbt_len = 13;
        assert_eq!(&modern[17 + nbt_len..17 + nbt_len + 4], 0.5f32.to_be_bytes());
    }

    #[test]
    fn update_payloads_have_only_their_fields() {
        let uuid = uuid();
        assert_eq!(remove_payload(&uuid).len(), 17, "uuid + action");
        let health = update_health_payload(&uuid, 1.0);
        assert_eq!(health.len(), 17 + 4);
        assert_eq!(&health[17..21], 1.0f32.to_be_bytes());
        // Health clamps like the Bedrock setters.
        assert_eq!(
            &update_health_payload(&uuid, 9.0)[17..21],
            1.0f32.to_be_bytes()
        );
        let style = update_style_payload(&uuid, COLOR_BLUE, 5);
        assert_eq!(style.len(), 19);
        assert_eq!(style[17], COLOR_BLUE as u8);
        assert_eq!(style[18], 5);
        let flags = update_flags_payload(&uuid, FLAG_DARKEN_SCREEN);
        assert_eq!(flags.len(), 18);
        assert_eq!(flags[17], FLAG_DARKEN_SCREEN);
    }

    #[test]
    fn add_frame_is_length_prefixed() {
        let frame = add_frame(&uuid(), 775, "Hi", 1.0, COLOR_PURPLE, 0, 0).unwrap();
        let mut offset = 0;
        let len = protocol::read_var_int(&frame, &mut offset).unwrap();
        assert_eq!(len as usize, frame.len() - offset);
        assert_eq!(
            protocol::read_var_int(&frame, &mut offset),
            Some(S2C_BOSS_BAR_770_PLUS)
        );
        assert!(add_frame(&uuid(), 999, "Hi", 1.0, 0, 0, 0).is_none());
        assert!(remove_frame(&uuid(), 999).is_none());
    }

    #[test]
    fn bedrock_color_mapping() {
        assert_eq!(bedrock_color_to_java(0), COLOR_PINK);
        assert_eq!(bedrock_color_to_java(5), COLOR_PURPLE);
        assert_eq!(bedrock_color_to_java(6), COLOR_PURPLE);
        assert_eq!(bedrock_color_to_java(7), COLOR_WHITE);
    }
}
