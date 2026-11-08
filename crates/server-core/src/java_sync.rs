//! Gameplay sync for Java (virtual) players, ported from
//! `plugins/JavaBridge/src/javabridge/JavaServer.php` (`syncGamemode`,
//! `syncTeleport`, `tickTimeSync`, `tickTab`, `tickEntities`/
//! `entityTargetsFor`) and `JavaClient.php` (`setGamemode`, `teleport`,
//! `sendTime`, `syncTab`), with the rotation math from `Protocol.php`
//! (`bedrockYawToJava`, `javaYawToBedrock`, `bedrockPitchToJava`,
//! `angle`).
//!
//! The Java wire packets live in that PHP bridge; this module is the
//! canonical, side-effect-free half of the port so the Rust core and
//! its tests speak exactly the same yaw/pitch, gamemode, time, TAB and
//! entity-visibility decisions as the bridge (byte layouts verified
//! against `tests/tp_sync_test.php` and the `modern_test.php`
//! gamemode / `update_time` sections):
//!
//! * gamemode bytes: `player_info_update` flags `0x04` (19 bytes for
//!   gamemode 0-3), `game_state_change` reason `3` + `f32`,
//!   `abilities` flags + `0.05`/`0.1` — see
//!   [`encode_player_info_gamemode`], [`encode_game_state_gamemode`],
//!   [`encode_abilities`];
//! * teleport rotation: Bedrock<->Java yaw is `+180` modulo 360
//!   ([`bedrock_yaw_to_java`] / [`java_yaw_to_bedrock`]) and pitch
//!   folds back to `(-180, 180]` ([`bedrock_pitch_to_java`]), with
//!   an extra `[-90, 90]` clip for Java payloads
//!   ([`clip_java_pitch`]);
//! * `update_time` ids 769=107, 770=106, 773/774=111, 775=113, the
//!   775 form being `age + clockUpdates[]` ([`update_time_packet_id`],
//!   [`encode_update_time_payload`]);
//! * TAB add/remove in one `0x1D` batch ([`encode_tab_add_batch`],
//!   [`encode_player_remove_batch`]) and the diff that drives
//!   `syncTab` ([`diff_tab`]);
//! * entity visibility: only entities whose chunk was already sent
//!   ([`chunk_visible`]) and the spawn/move/despawn thresholds of
//!   `syncEntities` ([`classify_entity_move`]).
//!
//! Everything else in the bridge (sockets, chunk streaming, chat,
//! block place/break) stays on its own track and is deliberately not
//! mirrored here.

use std::collections::HashMap;

/// `player_info_update` bitflags (`JavaClient.php`).
pub const PLAYER_INFO_ADD_PLAYER: u8 = 0x01;
/// `player_info_update` bitflags. Note: `0x02` is `initialize_chat`,
/// never `update_game_mode`.
pub const PLAYER_INFO_UPDATE_GAME_MODE: u8 = 0x04;
pub const PLAYER_INFO_UPDATE_LISTED: u8 = 0x08;
pub const PLAYER_INFO_UPDATE_LATENCY: u8 = 0x10;
/// TAB add batch: add + gamemode + listed + latency (`0x1D`).
pub const PLAYER_INFO_TAB_ADD: u8 = PLAYER_INFO_ADD_PLAYER
    | PLAYER_INFO_UPDATE_GAME_MODE
    | PLAYER_INFO_UPDATE_LISTED
    | PLAYER_INFO_UPDATE_LATENCY;
/// `game_state_change` reason `change_game_mode` — the only packet
/// that updates the local player's UI (hearts/hunger).
pub const GAME_EVENT_CHANGE_GAMEMODE: u8 = 3;

/// Broadcast `update_time` every 20 ticks (1 Hz, like
/// `SetTimePacket` on the Bedrock side). A time jump larger than
/// one broadcast interval is sent on the next reading instead.
pub const TIME_SYNC_INTERVAL_TICKS: u64 = 20;
/// Between broadcasts the world clock is read every 5 ticks so a
/// `/time set` jump is detected within 250 ms.
pub const TIME_SYNC_PROBE_INTERVAL_TICKS: u64 = 5;

/// `tickEntities` cadence: `server.get_players_full` is a synchronous
/// host call, so entities sync every 4 ticks (5 Hz). Clients
/// interpolate between updates.
pub const ENTITY_SYNC_INTERVAL_TICKS: u64 = 4;

/// Gamemodes are always clamped to `0..=3`
/// (`max(0, min(3, $gamemode))` in the bridge).
pub fn clamp_gamemode(gamemode: i32) -> i32 {
    gamemode.clamp(0, 3)
}

/// Ability flags byte of the Java `abilities` packet
/// (`JavaClient::abilityFlags`): creative gets
/// invulnerable|allowFlying|instabuild (`0x0D`), spectator
/// invulnerable|allowFlying (`0x05`), anything else `0`.
pub fn java_ability_flags(gamemode: i32) -> u8 {
    match clamp_gamemode(gamemode) {
        1 => 0x01 | 0x04 | 0x08,
        3 => 0x01 | 0x04,
        _ => 0,
    }
}

/// Bedrock yaw (`[0, 360)`) to Java yaw (`(-180, 180]`): `+180`
/// modulo 360, normalized. Same rotation direction, opposite zero.
pub fn bedrock_yaw_to_java(yaw: f32) -> f32 {
    let y = (yaw + 180.0).rem_euclid(360.0);
    if y > 180.0 {
        y - 360.0
    } else {
        y
    }
}

/// Java yaw (`(-180, 180]`) to Bedrock yaw (`[0, 360)`): the inverse
/// of [`bedrock_yaw_to_java`] (also `+180` modulo 360).
pub fn java_yaw_to_bedrock(yaw: f32) -> f32 {
    (yaw + 180.0).rem_euclid(360.0)
}

/// Fold any yaw into the canonical Bedrock range `[0, 360)` that the
/// Rust registry stores.
pub fn normalize_bedrock_yaw(yaw: f32) -> f32 {
    yaw.rem_euclid(360.0)
}

/// Bedrock pitch to Java pitch: same sign, but the Rust side
/// normalizes pitch to `[0, 360)`, so it is folded back to
/// `(-180, 180]`.
pub fn bedrock_pitch_to_java(pitch: f32) -> f32 {
    let p = pitch.rem_euclid(360.0);
    if p > 180.0 {
        p - 360.0
    } else {
        p
    }
}

/// Clip a pitch for a Java payload: Java clients only accept
/// `[-90, 90]` (up/down).
pub fn clip_java_pitch(pitch: f32) -> f32 {
    pitch.clamp(-90.0, 90.0)
}

/// Java angle byte (`Protocol::angle`): degrees scaled to
/// `deg * 256 / 360` modulo 256, returned as a signed byte.
pub fn angle_byte(degrees: f32) -> i8 {
    let scaled = (degrees as f64 * 256.0 / 360.0 + 360.0).rem_euclid(256.0) as i32;
    if scaled >= 128 {
        (scaled - 256) as i8
    } else {
        scaled as i8
    }
}

/// `player_info_update` packet id per protocol
/// (`ModernProfile::id('player_info_update')`, verified in
/// `modern_test.php`): 769=64, 770=63, 773=68, 774=68, 775=70.
pub fn player_info_update_packet_id(protocol: u32) -> Option<u32> {
    match protocol {
        769 => Some(64),
        770 => Some(63),
        773 | 774 => Some(68),
        775 => Some(70),
        _ => None,
    }
}

/// `game_state_change` packet id per protocol (verified in
/// `modern_test.php`): 769=35, 770=34, 773/774/775=38.
pub fn game_state_change_packet_id(protocol: u32) -> Option<u32> {
    match protocol {
        769 => Some(35),
        770 => Some(34),
        773 | 774 | 775 => Some(38),
        _ => None,
    }
}

/// `abilities` packet id per protocol (verified in
/// `modern_test.php`): 769=58, 770=57, 773/774=62, 775=64.
pub fn abilities_packet_id(protocol: u32) -> Option<u32> {
    match protocol {
        769 => Some(58),
        770 => Some(57),
        773 | 774 => Some(62),
        775 => Some(64),
        _ => None,
    }
}

/// `player_remove` packet id per protocol. The modern bridge sends
/// the legacy `player_info` alias for the add path
/// (`ModernProfile` maps `player_info` -> `player_info_update`);
/// removals go on `player_remove`.
/// 769=37, 770=36, 773=40, 774=40, 775=41 (minecraft-data).
pub fn player_remove_packet_id(protocol: u32) -> Option<u32> {
    match protocol {
        769 => Some(37),
        770 => Some(36),
        773 | 774 => Some(40),
        775 => Some(41),
        _ => None,
    }
}

/// `update_time` packet id per protocol (`packet_update_time` in
/// minecraft-data, verified in `modern_test.php`): 769=107,
/// 770=106, 773/774=111, 775=113.
pub fn update_time_packet_id(protocol: u32) -> Option<u32> {
    match protocol {
        769 => Some(107),
        770 => Some(106),
        773 | 774 => Some(111),
        775 => Some(113),
        _ => None,
    }
}

fn write_varint(out: &mut Vec<u8>, value: u32) {
    let mut v = value;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            break;
        }
    }
}

/// `player_info_update` with only `update_game_mode` (`0x04`):
/// `0x04 + varInt(1) + uuid(16B) + varInt(gamemode)` — 19 bytes for
/// gamemode 0-3. No `add_player` (name/properties) and no
/// `initialize_chat` (chat_session).
pub fn encode_player_info_gamemode(uuid: &[u8; 16], gamemode: i32) -> Vec<u8> {
    let mut out = Vec::with_capacity(19);
    out.push(PLAYER_INFO_UPDATE_GAME_MODE);
    write_varint(&mut out, 1);
    out.extend_from_slice(uuid);
    write_varint(&mut out, clamp_gamemode(gamemode) as u32);
    out
}

/// `game_state_change` for a gamemode change: reason byte
/// `change_game_mode` (`3`) + gamemode as `f32` (big-endian, like
/// the bridge's `Protocol::float`).
pub fn encode_game_state_gamemode(gamemode: i32) -> Vec<u8> {
    let mut out = Vec::with_capacity(5);
    out.push(GAME_EVENT_CHANGE_GAMEMODE);
    out.extend_from_slice(&(clamp_gamemode(gamemode) as f32).to_be_bytes());
    out
}

/// `abilities` payload: flags byte + `flySpeed 0.05` + `walkSpeed
/// 0.1` (big-endian floats, as in `JavaClient::setGamemode`).
pub fn encode_abilities(gamemode: i32) -> Vec<u8> {
    let mut out = Vec::with_capacity(9);
    out.push(java_ability_flags(gamemode));
    out.extend_from_slice(&0.05f32.to_be_bytes());
    out.extend_from_slice(&0.1f32.to_be_bytes());
    out
}

/// `update_time` payload:
/// * 769-774: `age i64 + time i64 + tickDayTime bool`;
/// * 775 (26.1): `age i64 + clockUpdates[]` with a single day clock
///   `{id 0, totalTicks varlong, partialTick f32 0, rate f32 1/0}`.
pub fn encode_update_time_payload(protocol: u32, age: i64, time: i64, tick_day_time: bool) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&age.to_be_bytes());
    if protocol >= 775 {
        write_varint(&mut out, 1);
        write_varint(&mut out, 0);
        write_varint(&mut out, time as u32);
        out.extend_from_slice(&0.0f32.to_be_bytes());
        out.extend_from_slice(&(if tick_day_time { 1.0f32 } else { 0.0f32 }).to_be_bytes());
    } else {
        out.extend_from_slice(&time.to_be_bytes());
        out.push(if tick_day_time { 1 } else { 0 });
    }
    out
}

/// Port of the broadcast decision in `JavaServer::tickTimeSync`:
/// broadcast when this is the first reading, when the clock jumped
/// (more than one broadcast interval between readings, e.g.
/// `/time set`), when `time_running` flipped, or when the 1 Hz
/// interval elapsed. Between broadcasts the caller still probes
/// every [`TIME_SYNC_PROBE_INTERVAL_TICKS`] ticks to catch jumps and
/// freezes within 250 ms.
pub fn should_broadcast_time(
    ticks_since_broadcast: u64,
    last_broadcast_time: Option<i64>,
    last_broadcast_running: Option<bool>,
    time: i64,
    running: bool,
) -> bool {
    let Some(last) = last_broadcast_time else {
        return true;
    };
    let jumped = (time - last).abs() > TIME_SYNC_INTERVAL_TICKS as i64;
    let running_changed = last_broadcast_running.is_some_and(|r| r != running);
    jumped || running_changed || ticks_since_broadcast >= TIME_SYNC_INTERVAL_TICKS
}

/// One TAB target: a player shown in the global player list.
#[derive(Debug, Clone, PartialEq)]
pub struct TabTarget {
    pub name: String,
    pub gamemode: i32,
    pub ping: i32,
}

/// Result of the `syncTab` diff against the already-listed players:
/// who left (removals), who joined (adds) and who changed gamemode.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TabDiff {
    pub removed: Vec<TabTarget>,
    pub added: Vec<TabTarget>,
    pub gamemode_changed: Vec<TabTarget>,
}

fn tab_key(name: &str) -> String {
    name.to_lowercase()
}

/// Port of the diff in `JavaClient::syncTab`: keys are lower-cased
/// by name (so a capital-letter key cannot re-announce every tick),
/// removals are what is listed but no longer wanted, adds are what
/// is wanted but not yet listed, and gamemode changes are
/// wanted-but-listed entries whose gamemode moved.
pub fn diff_tab(listed: &HashMap<String, TabTarget>, targets: &[TabTarget]) -> TabDiff {
    let mut wanted: HashMap<String, TabTarget> = HashMap::new();
    for target in targets {
        let gamemode = clamp_gamemode(target.gamemode);
        let ping = target.ping.max(0);
        wanted.insert(
            tab_key(&target.name),
            TabTarget {
                name: target.name.clone(),
                gamemode,
                ping,
            },
        );
    }
    let mut diff = TabDiff::default();
    for (key, old) in listed {
        if !wanted.contains_key(key) {
            diff.removed.push(old.clone());
        }
    }
    for (key, target) in &wanted {
        match listed.get(key) {
            None => diff.added.push(target.clone()),
            Some(old) if old.gamemode != target.gamemode => {
                diff.gamemode_changed.push(target.clone())
            }
            _ => {}
        }
    }
    diff
}

/// `player_info_update` TAB add batch (`0x1D`): flags + count +
/// per entry `uuid + name + 0 properties + gamemode + listed(1) +
/// latency(ping)`. No `initialize_chat`, no `display_name`.
pub fn encode_tab_add_batch(entries: &[TabTarget], uuids: &[[u8; 16]]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(PLAYER_INFO_TAB_ADD);
    write_varint(&mut out, entries.len() as u32);
    for (i, entry) in entries.iter().enumerate() {
        let uuid = uuids.get(i).cloned().unwrap_or([0u8; 16]);
        out.extend_from_slice(&uuid);
        write_varint(&mut out, entry.name.len() as u32);
        out.extend_from_slice(entry.name.as_bytes());
        write_varint(&mut out, 0);
        write_varint(&mut out, clamp_gamemode(entry.gamemode) as u32);
        write_varint(&mut out, 1);
        write_varint(&mut out, entry.ping.max(0) as u32);
    }
    out
}

/// `player_remove` payload: count + uuids.
pub fn encode_player_remove_batch(uuids: &[[u8; 16]]) -> Vec<u8> {
    let mut out = Vec::new();
    write_varint(&mut out, uuids.len() as u32);
    for uuid in uuids {
        out.extend_from_slice(uuid);
    }
    out
}

/// Port of `JavaServer::posDiffers`: position diverges past
/// `0.001`, yaw/pitch past `0.01` — the walking noise floor.
/// `last_sent` is `[x, y, z, yaw_java, pitch_java]`.
pub fn pos_differs(x: f64, y: f64, z: f64, yaw: f64, pitch: f64, last_sent: [f64; 5]) -> bool {
    (x - last_sent[0]).abs() > 0.001
        || (y - last_sent[1]).abs() > 0.001
        || (z - last_sent[2]).abs() > 0.001
        || (yaw - last_sent[3]).abs() > 0.01
        || (pitch - last_sent[4]).abs() > 0.01
}

/// Where a teleport should land, decided by `syncTeleport`: the
/// ping-side source (plugin copy first, host registry second) wins,
/// exactly like the bridge — local walking never counts because the
/// reference is `last_sent`, not the client's current position.
#[derive(Debug, Clone, PartialEq)]
pub enum TeleportDecision {
    /// Nothing diverged: the client walked, do not teleport.
    None,
    /// Teleport to `x, y, z` with Java yaw/pitch already converted.
    Teleport { x: f64, y: f64, z: f64, yaw: f64, pitch: f64 },
}

/// Shared rotation triple used by the decision: a Bedrock position
/// snapshot (registry / API copy) converted to Java angles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BedrockSnapshot {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw_bedrock: f32,
    pub pitch_bedrock: f32,
}

/// Port of `JavaServer::syncTeleport` / `hasTeleportPending` over the
/// two sources: `plugin_copy` (PHP `apiPlayer`, Bedrock yaw) first,
/// `registry` (host `player.get`, Bedrock yaw) second. The first
/// source whose converted position differs from `last_sent` wins.
/// Note the bridge also rejects a non-virtual (real Bedrock) registry
/// entry; callers must filter that before calling.
pub fn decide_teleport(
    last_sent: [f64; 5],
    plugin_copy: Option<BedrockSnapshot>,
    registry: Option<BedrockSnapshot>,
) -> TeleportDecision {
    for source in [plugin_copy, registry].into_iter().flatten() {
        let yaw_java = bedrock_yaw_to_java(source.yaw_bedrock) as f64;
        let pitch_java = bedrock_pitch_to_java(source.pitch_bedrock) as f64;
        if pos_differs(source.x, source.y, source.z, yaw_java, pitch_java, last_sent) {
            return TeleportDecision::Teleport {
                x: source.x,
                y: source.y,
                z: source.z,
                yaw: yaw_java,
                pitch: pitch_java,
            };
        }
    }
    TeleportDecision::None
}

/// Entity visibility gate of `entityTargetsFor`: an entity is only
/// visible to an observer once the chunk containing it was sent
/// (`hasSentChunk`). The TAB has no such gate — see `diff_tab`.
pub fn chunk_visible(x: f32, z: f32, sent_chunks: &[(i32, i32)]) -> bool {
    let cx = (x / 16.0).floor() as i32;
    let cz = (z / 16.0).floor() as i32;
    sent_chunks.contains(&(cx, cz))
}

/// Entity move classification of `syncEntities`: past 8 blocks the
/// Java client needs an absolute teleport, otherwise relative moves
/// (with look when the rotation moved); yaw alone goes to head
/// rotation + look. Angles compare at `0.01`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityMove {
    None,
    RelativeMove,
    RelativeMoveLook,
    Teleport,
    Look,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityPose {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f64,
    pub pitch: f64,
}

pub fn classify_entity_move(prev: EntityPose, next: EntityPose) -> EntityMove {
    let dist = (next.x - prev.x).abs() + (next.y - prev.y).abs() + (next.z - prev.z).abs();
    let rot_changed = (next.yaw - prev.yaw).abs() > 0.01 || (next.pitch - prev.pitch).abs() > 0.01;
    if dist > 8.0 {
        return EntityMove::Teleport;
    }
    if dist > 0.0001 {
        return if rot_changed {
            EntityMove::RelativeMoveLook
        } else {
            EntityMove::RelativeMove
        };
    }
    if rot_changed {
        EntityMove::Look
    } else {
        EntityMove::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steve_uuid() -> [u8; 16] {
        // `Protocol::offlineUuid('Steve')` in the bridge: MD5("OfflinePlayer:Steve").
        [
            0x56, 0x27, 0xdd, 0x98, 0xe6, 0xbe, 0x3c, 0x21, 0xb8, 0xa8, 0xe9, 0x23, 0x44, 0x18,
            0x36, 0x41,
        ]
    }

    #[test]
    fn gamemode_bytes_match_modern_test() {
        // `modern_test.php`: setGamemode(1) emits player_info 0x04,
        // game_state_change reason 3 + f32, abilities 0x0D + 0.05/0.1.
        let p = encode_player_info_gamemode(&steve_uuid(), 1);
        assert_eq!(p.len(), 19);
        assert_eq!(p[0], 0x04);
        assert_eq!(&p[2..18], &steve_uuid());
        assert_eq!(p[18], 1);
        assert_eq!(
            encode_game_state_gamemode(1),
            [3u8, 0x3F, 0x80, 0x00, 0x00].to_vec()
        );
        let abilities = encode_abilities(1);
        assert_eq!(abilities[0], 0x0D);
        assert_eq!(&abilities[1..5], &0.05f32.to_be_bytes());
        assert_eq!(&abilities[5..9], &0.1f32.to_be_bytes());
        // Survival: same shape, abilities all-off.
        let survival = encode_player_info_gamemode(&steve_uuid(), 0);
        assert_eq!(survival.len(), 19);
        assert_eq!(survival[18], 0);
        assert_eq!(encode_abilities(0)[0], 0);
        assert_eq!(encode_abilities(3)[0], 0x05);
    }

    #[test]
    fn packet_ids_match_modern_test() {
        assert_eq!(player_info_update_packet_id(769), Some(64));
        assert_eq!(player_info_update_packet_id(770), Some(63));
        assert_eq!(player_info_update_packet_id(773), Some(68));
        assert_eq!(player_info_update_packet_id(774), Some(68));
        assert_eq!(player_info_update_packet_id(775), Some(70));
        assert_eq!(game_state_change_packet_id(769), Some(35));
        assert_eq!(game_state_change_packet_id(775), Some(38));
        assert_eq!(abilities_packet_id(769), Some(58));
        assert_eq!(abilities_packet_id(775), Some(64));
        assert_eq!(update_time_packet_id(769), Some(107));
        assert_eq!(update_time_packet_id(770), Some(106));
        assert_eq!(update_time_packet_id(773), Some(111));
        assert_eq!(update_time_packet_id(774), Some(111));
        assert_eq!(update_time_packet_id(775), Some(113));
        assert_eq!(update_time_packet_id(340), None);
    }

    #[test]
    fn update_time_bytes_match_modern_test() {
        // 769 sendTime(6000, true): age 0 + time 6000 + tickDayTime 1.
        let p = encode_update_time_payload(769, 0, 6000, true);
        assert_eq!(p.len(), 17);
        assert_eq!(&p[0..8], &0i64.to_be_bytes());
        assert_eq!(&p[8..16], &6000i64.to_be_bytes());
        assert_eq!(p[16], 1);
        // Frozen: tickDayTime false.
        let frozen = encode_update_time_payload(769, 0, 6000, false);
        assert_eq!(frozen[16], 0);
        // 775: age + clockUpdates[1] {0, 6000, 0.0, 1.0}.
        let modern = encode_update_time_payload(775, 0, 6000, true);
        assert_eq!(&modern[0..8], &0i64.to_be_bytes());
        assert_eq!(modern[8], 1);
        assert_eq!(modern[9], 0);
        let rate_frozen = encode_update_time_payload(775, 0, 6000, false);
        assert_eq!(&rate_frozen[13..17], &0.0f32.to_be_bytes());
    }

    #[test]
    fn yaw_conversions_match_protocol_php() {
        // Bedrock 0 (north) -> Java 180; Bedrock 90 (east) -> Java -90
        // (see `bedrock-vis` check in modern_test.php).
        assert!((bedrock_yaw_to_java(0.0) - 180.0).abs() < 1e-6);
        assert!((bedrock_yaw_to_java(90.0) - (-90.0)).abs() < 1e-6);
        assert!((java_yaw_to_bedrock(180.0) - 0.0).abs() < 1e-6);
        // Round-trip is stable.
        for yaw in [-270.0, -180.0, -90.0, 0.0, 45.0, 90.0, 179.0, 180.0, 400.0] {
            let back = java_yaw_to_bedrock(bedrock_yaw_to_java(yaw));
            let want = normalize_bedrock_yaw(yaw);
            assert!((back - want).abs() < 1e-5, "yaw {yaw}: {back} != {want}");
        }
        assert_eq!(normalize_bedrock_yaw(-10.0), 350.0);
        assert!((bedrock_pitch_to_java(350.0) - (-10.0)).abs() < 1e-6);
        assert_eq!(clip_java_pitch(120.0), 90.0);
        assert_eq!(clip_java_pitch(-120.0), -90.0);
    }

    #[test]
    fn teleport_decision_matches_tp_sync_test() {
        // Registry moved to (0,100,0) with Bedrock yaw 0; last sent
        // [10,64,10, Java 180, 0]. Converted yaw is Java 180 -> same,
        // position diverges -> teleport.
        let last = [10.0, 64.0, 10.0, 180.0, 0.0];
        let registry = BedrockSnapshot {
            x: 0.0,
            y: 100.0,
            z: 0.0,
            yaw_bedrock: 0.0,
            pitch_bedrock: 0.0,
        };
        match decide_teleport(last, None, Some(registry)) {
            TeleportDecision::Teleport { x, y, z, yaw, .. } => {
                assert_eq!(x, 0.0);
                assert_eq!(y, 100.0);
                assert_eq!(z, 0.0);
                assert!((yaw - 180.0).abs() < 1e-6);
            }
            other => panic!("expected teleport, got {other:?}"),
        }
        // Walking (client-only move) -> no teleport: the registry/api
        // copy still equals `last_sent` ([0,100,0] after step 1 of
        // tp_sync_test.php), only the client's own position moved.
        let still = BedrockSnapshot {
            x: 0.0,
            y: 100.0,
            z: 0.0,
            yaw_bedrock: 0.0,
            pitch_bedrock: 0.0,
        };
        let last_after = [0.0, 100.0, 0.0, 180.0, 0.0];
        assert_eq!(
            decide_teleport(last_after, Some(still), Some(still)),
            TeleportDecision::None
        );
    }

    #[test]
    fn tab_diff_and_add_batch() {
        let mut listed = HashMap::new();
        listed.insert(
            "steve".to_string(),
            TabTarget {
                name: "Steve".to_string(),
                gamemode: 0,
                ping: 10,
            },
        );
        let targets = vec![
            TabTarget {
                name: "Steve".to_string(),
                gamemode: 1,
                ping: 12,
            },
            TabTarget {
                name: "Alex".to_string(),
                gamemode: 0,
                ping: 0,
            },
        ];
        let diff = diff_tab(&listed, &targets);
        assert!(diff.removed.is_empty());
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].name, "Alex");
        assert_eq!(diff.gamemode_changed.len(), 1);
        // Add batch starts with the 0x1D flag byte.
        let batch = encode_tab_add_batch(&diff.added, &[steve_uuid()]);
        assert_eq!(batch[0], PLAYER_INFO_TAB_ADD);
        assert_eq!(batch[1], 1);
        let remove = encode_player_remove_batch(&[steve_uuid()]);
        assert_eq!(remove.len(), 17);
    }

    #[test]
    fn entity_move_classification() {
        let a = EntityPose {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        };
        assert_eq!(classify_entity_move(a, a), EntityMove::None);
        let near = EntityPose {
            x: 1.0,
            ..a
        };
        assert_eq!(classify_entity_move(a, near), EntityMove::RelativeMove);
        let far = EntityPose {
            x: 9.0,
            ..a
        };
        assert_eq!(classify_entity_move(a, far), EntityMove::Teleport);
        let turned = EntityPose { yaw: 45.0, ..a };
        assert_eq!(classify_entity_move(a, turned), EntityMove::Look);
        // Chunk gate: (10,10) is chunk (0,0).
        assert!(chunk_visible(10.0, 10.0, &[(0, 0)]));
        assert!(!chunk_visible(10.0, 10.0, &[(1, 1)]));
    }

    #[test]
    fn time_broadcast_thresholds() {
        assert!(should_broadcast_time(0, None, None, 6000, true));
        assert!(!should_broadcast_time(19, Some(6000), Some(true), 6001, true));
        assert!(should_broadcast_time(20, Some(6000), Some(true), 6020, true));
        // Jump >20 ticks -> immediate.
        assert!(should_broadcast_time(5, Some(6000), Some(true), 13000, true));
        // /time stop flips running -> immediate even without a jump.
        assert!(should_broadcast_time(5, Some(6000), Some(true), 6000, false));
    }

    #[test]
    fn angle_byte_matches_protocol() {
        // `Protocol::angle` verified with php-cli: 0->104, 90->-88,
        // 180->-24, -90->40 (the +360 before the modulo, kept exact).
        assert_eq!(angle_byte(0.0), 104);
        assert_eq!(angle_byte(90.0), -88);
        assert_eq!(angle_byte(180.0), -24);
        assert_eq!(angle_byte(-90.0), 40);
    }
}
