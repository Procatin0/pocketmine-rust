use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::SocketAddr;

/// An action emitted by a plugin (PHP or native) to be executed by the server core.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", content = "data")]
pub enum PluginAction {
    #[serde(rename = "send_message")]
    SendMessage {
        target_player: String,
        message: String,
    },
    #[serde(rename = "broadcast_message")]
    BroadcastMessage { message: String },
    #[serde(rename = "console_message")]
    ConsoleMessage { message: String },
    #[serde(rename = "kick_player")]
    KickPlayer {
        target_player: String,
        #[serde(default)]
        reason: String,
    },
    #[serde(rename = "teleport")]
    Teleport {
        target_player: String,
        x: f64,
        y: f64,
        z: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        world: Option<String>,
        /// Bedrock-canonical yaw/pitch (the values `Player::teleport`
        /// stores on the PHP copy). Absent on the plain
        /// `teleport(x, y, z)` form; the Java bridge strips yaw/pitch
        /// when neither is provided.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        yaw: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pitch: Option<f64>,
    },
    #[serde(rename = "set_block")]
    SetBlock {
        world: String,
        x: i32,
        y: i32,
        z: i32,
        block_id: u32,
    },
    #[serde(rename = "dispatch_command")]
    DispatchCommand {
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sender: Option<String>,
    },
    #[serde(rename = "send_tip")]
    SendTip {
        target_player: String,
        message: String,
    },
    #[serde(rename = "send_popup")]
    SendPopup {
        target_player: String,
        message: String,
    },
    #[serde(rename = "send_title")]
    SendTitle {
        target_player: String,
        title: String,
        #[serde(default)]
        subtitle: String,
        #[serde(default)]
        fade_in: i32,
        #[serde(default)]
        stay: i32,
        #[serde(default)]
        fade_out: i32,
    },
    #[serde(rename = "broadcast_tip")]
    BroadcastTip { message: String },
    #[serde(rename = "broadcast_popup")]
    BroadcastPopup { message: String },
    #[serde(rename = "broadcast_title")]
    BroadcastTitle {
        title: String,
        #[serde(default)]
        subtitle: String,
        #[serde(default)]
        fade_in: i32,
        #[serde(default)]
        stay: i32,
        #[serde(default)]
        fade_out: i32,
    },
    #[serde(rename = "set_gamemode")]
    SetGamemode {
        target_player: String,
        gamemode: u32,
    },
    // Fase A1 (Server+Player API): `Player::setHealth/setMaxHealth/setXp/
    // setXpLevel` via `addAction`. `SetHealth` bundles `health` + optional
    // `max_health` so both PHP setters share one variant; `SetXp` bundles
    // `xp` (progress 0..1) + optional `level` (xp_level). Tolerant of
    // partial payloads (optionals default to `None`).
    #[serde(rename = "set_health")]
    SetHealth {
        target_player: String,
        health: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_health: Option<f32>,
    },
    #[serde(rename = "set_xp")]
    SetXp {
        target_player: String,
        xp: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<i32>,
    },
    // Fase A1: `Player::hidePlayer/showPlayer` -> `SessionManager::
    // hide_player/show_player` (`canSee`). `viewer` hides/shows `target`.
    #[serde(rename = "hide_player")]
    HidePlayer { viewer: String, target: String },
    #[serde(rename = "show_player")]
    ShowPlayer { viewer: String, target: String },
    // Fase A1: `Player::sendSubTitle` (subtitle only, no title packet).
    #[serde(rename = "send_subtitle")]
    SendSubTitle {
        target_player: String,
        subtitle: String,
    },
    // Fase A1: `Player::sendActionBarMessage` (`SetTitlePacket`
    // action-bar message, NOT `TextPacket` popup).
    #[serde(rename = "send_action_bar")]
    SendActionBar {
        target_player: String,
        message: String,
    },
    // Scoreboard (Fase 7, `pocketmine\scoreboard\Scoreboard`): the PHP
    // side owns the full per-player scoreboard object and re-sends it on
    // every mutation (same full-state pattern as `set_health`/`set_xp`).
    // Rust diffs it against the stored copy and emits the Bedrock
    // `RemoveObjective`/`SetDisplayObjective`/`SetScore`/
    // `SetScoreboardIdentity` packets plus the Java scoreboard packets
    // (via the Java `take_scoreboard` hook poll).
    #[serde(rename = "set_scoreboard")]
    SetScoreboard {
        target_player: String,
        #[serde(default)]
        scoreboard: ScoreboardData,
    },
    /// `Player::removeScoreboard()`: drop every objective of the player.
    #[serde(rename = "remove_scoreboard")]
    RemoveScoreboard { target_player: String },
    // apiBossBar (`xenialdan\apibossbar`): the PHP side owns the full
    // `BossBar`/`DiverseBossBar` object and re-sends its whole state on
    // every mutation (same full-state pattern as `set_scoreboard`).
    // Rust diffs it against the stored copy in `BossBarManager` and
    // emits Bedrock `BossEventPacket` (0x4A) + Java `boss_bar` frames.
    #[serde(rename = "set_bossbar")]
    SetBossBar {
        #[serde(default)]
        bossbar: BossBarData,
    },
    /// `BossBar` destruction: hides the bar from every viewer and
    /// unregisters it (upstream `removeAllPlayers` + drop).
    #[serde(rename = "remove_bossbar")]
    RemoveBossBar { boss_id: i64 },
    /// PM `World::broadcastPacketToViewers($pos, LevelEventPacket)`.
    /// Emitted by the server-auth dig flow: `SurvivalBlockBreakHandler`
    /// broadcasts `BLOCK_START_BREAK`/`BLOCK_STOP_BREAK`/`BLOCK_BREAK_SPEED`
    /// (LevelEvent 3600/3601/3602) so the client can render dig progress
    /// and send `PREDICT_DESTROY_BLOCK` when it completes.
    #[serde(rename = "broadcast_level_event")]
    BroadcastLevelEvent {
        event_id: i32,
        event_data: i32,
        x: i32,
        y: i32,
        z: i32,
    },
}

/// Wire form of one per-player `DiverseBossBar` override (only set
/// fields are overrides; missing fields fall back to shared defaults).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BossBarOverrideData {
    #[serde(default)]
    pub player: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub percentage: Option<f32>,
    #[serde(default)]
    pub color: Option<u8>,
}

/// Wire form of a whole boss bar (`BossBar`/`DiverseBossBar` in PHP).
/// Defaults mirror the upstream constructor: 100%, purple, progress,
/// no flags, shared (non-diverse) bar with no viewers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BossBarData {
    /// Bedrock actor unique id (`$actorId`; synthetic `nextRuntimeId()`
    /// unless `setEntity()` bound a real entity).
    #[serde(default)]
    pub boss_id: i64,
    #[serde(default)]
    pub diverse: bool,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default = "default_bossbar_percentage")]
    pub percentage: f32,
    #[serde(default = "default_bossbar_color")]
    pub color: u8,
    #[serde(default)]
    pub overlay: u8,
    /// Java-only flags (darken/music/fog bits).
    #[serde(default)]
    pub flags: u8,
    #[serde(default)]
    pub entity: Option<String>,
    /// Viewers (`addPlayer`...); `hidden` are viewers the bar is
    /// currently hidden from (`hideFrom` without `removePlayer`).
    #[serde(default)]
    pub players: Vec<String>,
    #[serde(default)]
    pub hidden: Vec<String>,
    #[serde(default)]
    pub overrides: Vec<BossBarOverrideData>,
}

impl Default for BossBarData {
    fn default() -> Self {
        Self {
            boss_id: 0,
            diverse: false,
            title: String::new(),
            subtitle: String::new(),
            percentage: default_bossbar_percentage(),
            color: default_bossbar_color(),
            overlay: 0,
            flags: 0,
            entity: None,
            players: Vec::new(),
            hidden: Vec::new(),
            overrides: Vec::new(),
        }
    }
}

fn default_bossbar_percentage() -> f32 {
    1.0
}

fn default_bossbar_color() -> u8 {
    5 // BossBarColor::PURPLE (upstream constructor default)
}

/// Wire form of one scoreboard line (`ScoreboardLine` in PHP).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ScoreboardLineData {
    #[serde(default)]
    pub score: u32,
    #[serde(default)]
    pub text: String,
}

/// Wire form of one objective (`ScoreboardObjective` in PHP).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ScoreboardObjectiveData {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    /// `ScoreboardSlot` wire value: 1 sidebar, 2 below-name, 3 list.
    #[serde(default = "default_scoreboard_slot")]
    pub slot: u8,
    #[serde(default)]
    pub sort_order: u8,
    /// Background tint as `[r, g, b]` (PHP `ColorRange`), ignored when
    /// empty. Bedrock has no color field on the packet, so the core
    /// prepends the matching `§` code to the display name.
    #[serde(default)]
    pub background: Vec<u8>,
    #[serde(default)]
    pub lines: Vec<ScoreboardLineData>,
}

fn default_scoreboard_slot() -> u8 {
    1
}

/// Wire form of a whole per-player scoreboard (`Scoreboard` in PHP).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ScoreboardData {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub objectives: Vec<ScoreboardObjectiveData>,
}

fn default_socket_addr() -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], 0))
}

/// Information about a player passed to plugins.
///
/// Mirrors the JSON object documented in `docs/plugin-api-contract.md`. All
/// fields have serde defaults so older callers (and partial PHP payloads) keep
/// working.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerContext {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default = "default_socket_addr")]
    pub address: SocketAddr,
    #[serde(default)]
    pub runtime_id: u64,
    #[serde(default)]
    pub unique_id: i64,
    #[serde(default)]
    pub ping: u32,
    #[serde(default)]
    pub gamemode: u32,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub z: f64,
    #[serde(default)]
    pub yaw: f64,
    #[serde(default)]
    pub pitch: f64,
    /// Head yaw (Bedrock canonical `[0, 360)`), from
    /// `PlayerAuthInputPacket.headYaw`. Java virtuals report their body
    /// yaw here. Defaults to 0.0; older payloads omit it.
    #[serde(default)]
    pub head_yaw: f64,
    #[serde(default)]
    pub world: String,
    #[serde(default)]
    pub health: f32,
    #[serde(default)]
    pub max_health: f32,
    #[serde(default)]
    pub is_op: bool,
    /// True when this entry is owned by a virtual (Java bridge) player,
    /// false when it is owned by a live Bedrock session. Ownership matters
    /// for removal: quitting one side must not delete the other's entry
    /// when both scripts share a lowercase name.
    #[serde(default)]
    pub is_virtual: bool,
    /// Xbox XUID ("" when unknown, e.g. offline Java login).
    #[serde(default)]
    pub xuid: String,
    /// Microtime of first join (0.0 = never, `Player::getFirstPlayed`).
    #[serde(default)]
    pub first_played: f64,
    /// Microtime of last join/save (0.0 = never, `Player::getLastPlayed`).
    #[serde(default)]
    pub last_played: f64,
    #[serde(default = "default_food")]
    pub food: f32,
    #[serde(default = "default_food")]
    pub saturation: f32,
    #[serde(default)]
    pub xp: f32,
    #[serde(default)]
    pub xp_level: i32,
    #[serde(default = "default_air")]
    pub air: i16,
    #[serde(default)]
    pub is_flying: bool,
    #[serde(default)]
    pub spawn_point: Option<[f64; 3]>,
    #[serde(default)]
    pub is_sneaking: bool,
    #[serde(default)]
    pub is_sprinting: bool,
    #[serde(default)]
    pub is_swimming: bool,
    #[serde(default)]
    pub is_gliding: bool,
}

fn default_food() -> f32 {
    20.0
}

fn default_air() -> i16 {
    300
}

impl PlayerContext {
    pub fn new(name: impl Into<String>, address: SocketAddr) -> Self {
        let name = name.into();
        Self {
            display_name: name.clone(),
            name,
            address,
            runtime_id: 1,
            unique_id: 1,
            ping: 0,
            gamemode: 0,
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            world: String::new(),
            health: 20.0,
            max_health: 20.0,
            is_op: false,
            is_virtual: false,
            xuid: String::new(),
            first_played: 0.0,
            last_played: 0.0,
            food: 20.0,
            saturation: 20.0,
            xp: 0.0,
            xp_level: 0,
            air: 300,
            is_flying: false,
            spawn_point: None,
            is_sneaking: false,
            is_sprinting: false,
            is_swimming: false,
            is_gliding: false,
        }
    }

    /// Serialize the player snapshot for the PHP runtime.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Hydrate a player snapshot returned by PHP.
    ///
    /// Returns `None` when `value` is not an object (or contains invalid
    /// field types). Missing fields fall back to their serde defaults.
    pub fn from_json(value: &Value) -> Option<Self> {
        let mut player: Self = serde_json::from_value(value.clone()).ok()?;
        if player.display_name.is_empty() {
            player.display_name = player.name.clone();
        }
        Some(player)
    }

    /// Updates position/rotation in place (`player.update_position`).
    pub fn apply_position(&mut self, x: f64, y: f64, z: f64, yaw: f64, pitch: f64) {
        self.x = x;
        self.y = y;
        self.z = z;
        self.yaw = yaw;
        self.pitch = pitch;
    }

    /// PM `Player::hasPlayedBefore(): bool`: saved data existed before this
    /// session (registry carries `first_played`/`last_played` from disk), or
    /// `last_played` exceeds `first_played` by more than 1ms (axolotl).
    pub fn has_played_before(&self) -> bool {
        self.first_played > 0.0 && self.last_played > self.first_played + 0.001
    }

    /// Snapshot in the `players/<name>.json` format shared with Bedrock
    /// sessions (`PlayerSession::save_player_data`).
    ///
    /// JSON v2 (27 campos, canónico): `y` is the Bedrock world Y (never the
    /// legacy Java `y+64`; the +64 lives only in legacy Java packets).
    /// Bedrock and virtual (Java) saves write this exact object directly to
    /// `players/<lowercase>.json` — no staging file.
    pub fn to_save_json(&self) -> Value {
        serde_json::json!({
            "name": self.name,
            "xuid": self.xuid,
            "display_name": self.display_name,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "yaw": self.yaw,
            "pitch": self.pitch,
            "head_yaw": self.head_yaw,
            "gamemode": self.gamemode as i64,
            "health": self.health as f64,
            "max_health": self.max_health as f64,
            "food": self.food as f64,
            "saturation": self.saturation as f64,
            "xp": self.xp as f64,
            "xp_level": self.xp_level,
            "air": self.air as i64,
            "flying": self.is_flying,
            "sneaking": self.is_sneaking,
            "sprinting": self.is_sprinting,
            "swimming": self.is_swimming,
            "gliding": self.is_gliding,
            "world": self.world,
            "is_op": self.is_op,
            "spawn_point": self.spawn_point.map(|point| vec![point[0], point[1], point[2]]),
            "first_played": self.first_played,
            "last_played": self.last_played,
        })
    }

    /// Restores position/rotation/gamemode/health and the rest of JSON v2
    /// from a saved JSON object. Missing fields leave values untouched.
    /// Accepts PHP aliases (`flying`/`is_flying`, `xp_level`/`xpLevel`,
    /// `sneaking`/`is_sneaking`, ...).
    pub fn apply_save_json(&mut self, value: &Value) {
        let number = |key: &str| value.get(key).and_then(Value::as_f64);
        let first_number = |keys: &[&str]| keys.iter().filter_map(|key| number(key)).next();
        let first_bool = |keys: &[&str]| {
            keys.iter()
                .filter_map(|key| value.get(*key).and_then(Value::as_bool))
                .next()
        };
        if let (Some(x), Some(y), Some(z)) = (number("x"), number("y"), number("z")) {
            self.x = x;
            self.y = y;
            self.z = z;
        }
        if let Some(yaw) = number("yaw") {
            self.yaw = yaw;
        }
        if let Some(pitch) = number("pitch") {
            self.pitch = pitch;
        }
        if let Some(head_yaw) = first_number(&["head_yaw", "headYaw"]) {
            self.head_yaw = head_yaw;
        }
        if let Some(gamemode) = value.get("gamemode").and_then(Value::as_i64) {
            self.gamemode = gamemode.max(0) as u32;
        }
        if let Some(health) = number("health") {
            self.health = health as f32;
        }
        if let Some(max_health) = first_number(&["max_health", "maxHealth"]) {
            self.max_health = max_health.max(1.0) as f32;
        }
        if let Some(world) = value.get("world").and_then(Value::as_str) {
            if !world.is_empty() {
                self.world = world.to_string();
            }
        }
        if let Some(xuid) = value.get("xuid").and_then(Value::as_str) {
            if !xuid.is_empty() {
                self.xuid = xuid.to_string();
            }
        }
        if let Some(display_name) = value
            .get("display_name")
            .or_else(|| value.get("displayName"))
            .and_then(Value::as_str)
        {
            if !display_name.is_empty() {
                self.display_name = display_name.to_string();
            }
        }
        if let Some(first) = first_number(&["first_played", "firstPlayed"]) {
            self.first_played = first;
        }
        if let Some(last) = first_number(&["last_played", "lastPlayed"]) {
            self.last_played = last;
        }
        if let Some(food) = number("food") {
            self.food = food as f32;
        }
        if let Some(saturation) = number("saturation") {
            self.saturation = saturation as f32;
        }
        if let Some(xp) = number("xp") {
            self.xp = xp as f32;
        }
        if let Some(level) = value
            .get("xp_level")
            .or_else(|| value.get("xpLevel"))
            .and_then(Value::as_i64)
        {
            self.xp_level = level as i32;
        }
        if let Some(air) = value.get("air").and_then(Value::as_i64) {
            self.air = air as i16;
        }
        if let Some(flying) = first_bool(&["flying", "is_flying", "isFlying"]) {
            self.is_flying = flying;
        }
        if let Some(v) = first_bool(&["sneaking", "is_sneaking", "isSneaking"]) {
            self.is_sneaking = v;
        }
        if let Some(v) = first_bool(&["sprinting", "is_sprinting", "isSprinting"]) {
            self.is_sprinting = v;
        }
        if let Some(v) = first_bool(&["swimming", "is_swimming", "isSwimming"]) {
            self.is_swimming = v;
        }
        if let Some(v) = first_bool(&["gliding", "is_gliding", "isGliding"]) {
            self.is_gliding = v;
        }
        if let Some(is_op) = value
            .get("is_op")
            .or_else(|| value.get("op"))
            .and_then(Value::as_bool)
        {
            self.is_op = is_op;
        }
        if let Some(point) = value
            .get("spawn_point")
            .or_else(|| value.get("spawnPoint"))
            .and_then(Value::as_array)
        {
            let coords: Vec<f64> = point.iter().filter_map(Value::as_f64).collect();
            if coords.len() == 3 {
                self.spawn_point = Some([coords[0], coords[1], coords[2]]);
            }
        } else if value.get("spawn_point").is_some_and(Value::is_null) {
            self.spawn_point = None;
        }
    }

    /// Builds a context from a saved JSON object returning `None` when the
    /// object has no usable position.
    pub fn from_save_json(name: &str, value: &Value) -> Option<Self> {
        let (x, y, z) = {
            let object = value.as_object()?;
            (
                object.get("x")?.as_f64()?,
                object.get("y")?.as_f64()?,
                object.get("z")?.as_f64()?,
            )
        };
        let mut player = Self::new(name, default_socket_addr());
        player.x = x;
        player.y = y;
        player.z = z;
        player.apply_save_json(value);
        Some(player)
    }
}

/// Parse a position from either `[x, y, z]` or `{"x":..,"y":..,"z":..}`.
pub(crate) fn position_from_json(value: &Value) -> Option<(f64, f64, f64)> {
    if let Some(array) = value.as_array() {
        if array.len() >= 3 {
            return Some((array[0].as_f64()?, array[1].as_f64()?, array[2].as_f64()?));
        }
        return None;
    }

    let object = value.as_object()?;
    Some((
        object.get("x")?.as_f64()?,
        object.get("y")?.as_f64()?,
        object.get("z")?.as_f64()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_context_json_roundtrip() {
        let player = PlayerContext {
            name: "Steve".to_string(),
            display_name: "Steve".to_string(),
            address: "127.0.0.1:19132".parse().unwrap(),
            runtime_id: 1,
            unique_id: 1,
            ping: 0,
            gamemode: 0,
            x: 1.5,
            y: 64.0,
            z: -2.5,
            yaw: 90.0,
            pitch: 10.0,
            head_yaw: 90.0,
            world: "world".to_string(),
            health: 20.0,
            max_health: 20.0,
            is_op: true,
            is_virtual: false,
            xuid: "2535412345678901".to_string(),
            first_played: 1700000000.5,
            last_played: 1700000100.25,
            food: 20.0,
            saturation: 20.0,
            xp: 0.0,
            xp_level: 0,
            air: 300,
            is_flying: false,
            spawn_point: None,
            is_sneaking: false,
            is_sprinting: false,
            is_swimming: false,
            is_gliding: false,
        };

        let value = player.to_json();
        assert_eq!(value["name"], "Steve");
        assert_eq!(value["address"], "127.0.0.1:19132");
        assert_eq!(value["world"], "world");

        let parsed = PlayerContext::from_json(&value).expect("valid player json");
        assert_eq!(parsed, player);
    }

    #[test]
    fn player_context_defaults_for_partial_json() {
        let parsed =
            PlayerContext::from_json(&serde_json::json!({"name": "Alex"})).expect("valid player");
        assert_eq!(parsed.name, "Alex");
        assert_eq!(parsed.display_name, "Alex");
        assert_eq!(parsed.y, 0.0);
        assert_eq!(parsed.max_health, 0.0);
    }

    #[test]
    fn save_json_is_v2_27_fields_and_roundtrips() {
        let mut player = PlayerContext::new("Steve", "127.0.0.1:19132".parse().unwrap());
        player.xuid = "2535412345678901".to_string();
        player.first_played = 1700000000.5;
        player.last_played = 1700000100.25;
        player.food = 15.0;
        player.saturation = 12.0;
        player.xp = 0.5;
        player.xp_level = 3;
        player.air = 250;
        player.y = 65.0; // canónico, sin +64
        let saved = player.to_save_json();
        let object = saved.as_object().expect("save json object");
        assert_eq!(object.len(), 27, "JSON v2 debe tener 27 campos");
        assert_eq!(saved["y"], 65.0);
        assert_eq!(saved["xuid"], "2535412345678901");
        assert_eq!(saved["first_played"], 1700000000.5);
        let mut restored =
            PlayerContext::new("Steve", "127.0.0.1:19132".parse().unwrap());
        restored.apply_save_json(&saved);
        assert_eq!(restored, player);
        assert!(restored.has_played_before());
        assert!(!PlayerContext::new("New", "127.0.0.1:19132".parse().unwrap()).has_played_before());
    }

    #[test]
    fn parses_position_arrays_and_objects() {
        assert_eq!(
            position_from_json(&serde_json::json!([1.0, 64.0, -3.0])),
            Some((1.0, 64.0, -3.0))
        );
        assert_eq!(
            position_from_json(&serde_json::json!({"x": 1, "y": 64, "z": -3})),
            Some((1.0, 64.0, -3.0))
        );
    }
}
