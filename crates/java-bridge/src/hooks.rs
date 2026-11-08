//! Server hooks: everything the listener needs from the Rust server core.
//!
//! In PHP these are `JavaServer`'s host calls (`player.register`,
//! `player.update_position`, `player.load`, `player.get`,
//! `player.take_messages`, `world.get_chunk`, `world.get_block`,
//! `world.set_block`, `server.get_players_full`, `world.take_block_changes`,
//! plus the PocketMine `Server`/`World` API). In Rust they are one trait so
//! `java-bridge` compiles without depending on `server-core` (which would
//! be a cycle once `server-core` owns the listener). The production
//! implementation in `server-core` delegates to:
//! - `SessionManager` / `PlayerRegistry` (`registry.all()` is the same
//!   source `sync_virtuals_locked` reads; `take_messages` is the inbox
//!   `deliverPluginMessages` drains),
//! - `WorldState` for spawn / time / blocks / chunks,
//! - chat + command dispatch (`PlayerChatEvent` in PHP).
//!
//! Chunk *bytes* and block-state translation are supplied here too, so the
//! `Protocol` / `ModernChunk` agents own those crates and this crate only
//! frames their output. [`ChatOutcome`] keeps command logic out of the
//! listener: the hook decides, the session only sends/teleports.

use std::net::SocketAddr;

/// One online player as the Java listener sees it. Positions, yaw and pitch
/// are canonical Bedrock (the `PlayerContext` / registry convention); the
/// session converts to Java yaw/pitch when it builds packets.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSnapshot {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Bedrock yaw.
    pub yaw: f64,
    /// Bedrock pitch.
    pub pitch: f64,
    pub gamemode: i32,
    pub ping: i32,
    /// True for virtual (Java bridge) players, false for Bedrock sessions.
    /// Mirrors `PlayerContext::is_virtual`: a Bedrock snapshot whose name
    /// collides with a virtual one wins for Bedrock viewers in
    /// `sync_virtuals_locked`; here the same rule keeps one body per name.
    pub is_virtual: bool,
}

impl PlayerSnapshot {
    pub fn new(name: impl Into<String>, x: f64, y: f64, z: f64) -> Self {
        Self {
            name: name.into(),
            x,
            y,
            z,
            yaw: 0.0,
            pitch: 0.0,
            gamemode: 0,
            ping: 0,
            is_virtual: false,
        }
    }
}

/// Saved position/gamemode/health (`player.load`), Bedrock-canonical.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedData {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f64,
    pub pitch: f64,
    pub gamemode: i32,
    pub health: f32,
}

/// Legacy 1.12 chunk column as `ChunkBuilder::fromChunk` returns it:
/// `[bitMap, data]` (data is the already-serialized section bytes).
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyChunk {
    pub bitmap: i32,
    pub data: Vec<u8>,
}

/// Modern 1.18+ chunk column as `ModernChunk::fromChunk` returns it.
#[derive(Debug, Clone, PartialEq)]
pub struct ModernChunkData {
    pub data: Vec<u8>,
    pub heightmaps: Vec<u8>,
    pub light: Vec<u8>,
}

/// One Bedrock -> Java block edit (`world.take_block_changes`).
#[derive(Debug, Clone, PartialEq)]
pub struct BlockChange {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Bedrock network id, when the queue carries one.
    pub network_id: Option<u32>,
    /// Fallback kind (`"air"`, ...) when there is no network id.
    pub kind: String,
}

/// Result of routing a Java chat line/command through the server.
/// The listener only sends `lines` and applies `teleport`/`gamemode` to
/// the sender session; everything else (events, broadcasts) stays in core.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatOutcome {
    pub lines: Vec<String>,
    pub teleport: Option<(f64, f64, f64, Option<f64>, Option<f64>)>,
    pub gamemode: Option<i32>,
}

impl ChatOutcome {
    pub fn lines(lines: Vec<String>) -> Self {
        Self {
            lines,
            ..Default::default()
        }
    }
}

pub trait ServerHooks: Send + Sync {
    fn log(&self, _message: String) {}

    fn view_distance(&self) -> i32;
    /// Bedrock spawn point (`World::getPlayerSpawnLocation`).
    fn spawn(&self) -> (f64, f64, f64);
    fn default_gamemode(&self) -> i32 {
        0
    }
    fn difficulty(&self) -> i32 {
        2
    }
    fn level_type(&self) -> String {
        "flat".to_string()
    }
    fn motd(&self) -> String {
        "PocketMine-Rust".to_string()
    }
    fn max_players(&self) -> i32 {
        1000
    }
    fn world_time(&self) -> i64 {
        6000
    }
    fn world_time_running(&self) -> bool {
        true
    }

    fn load_player_data(&self, _name: &str) -> Option<LoadedData> {
        None
    }

    /// Name reservation (`JavaServer::registerPlayer`): false when another
    /// Java session or a live Bedrock player already owns the name.
    fn is_name_available(&self, _name: &str, _except_session: u64) -> bool {
        true
    }

    /// Central entity id for a player (`JavaServer::entityIdFor`, from
    /// 1_000_000, never reused). The default is a deterministic hash for
    /// in-memory tests; production delegates to the `JavaServer` counter.
    fn entity_id_for(&self, name: &str) -> i32 {
        let mut hash: u32 = 0;
        for byte in name.to_lowercase().as_bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(*byte as u32);
        }
        1_000_000 + (hash % 900_000) as i32
    }

    /// All online players (Bedrock + Java), `server.get_players_full`.
    fn snapshots(&self) -> Vec<PlayerSnapshot>;
    fn get_snapshot(&self, name: &str) -> Option<PlayerSnapshot> {
        self.snapshots()
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// `player.register` + `Server::addPlayer` + join announce (`joinApi`).
    fn join_api(&self, _session_id: u64, _peer: SocketAddr, _snapshot: &PlayerSnapshot) {}
    /// `player.unregister` + quit announce + save (`leaveApi`).
    fn leave_api(&self, _name: &str) {}
    /// `player.update_position` (yaw already converted to Bedrock).
    fn update_virtual_position(
        &self,
        _name: &str,
        _x: f64,
        _y: f64,
        _z: f64,
        _yaw: f64,
        _pitch: f64,
    ) {
    }
    /// `player.take_messages`, drained by `deliverPluginMessages`.
    fn take_plugin_messages(&self, _name: &str) -> Vec<String> {
        Vec::new()
    }

    fn handle_chat(&self, _session_id: u64, _name: &str, _message: &str) -> ChatOutcome {
        ChatOutcome::default()
    }

    fn break_block(&self, _x: i32, _y: i32, _z: i32) -> bool {
        false
    }
    /// Place a block (`JavaServer::placeBlock`). Implementations must
    /// enqueue the edit in [`ServerHooks::drain_block_changes`] (as PHP
    /// `broadcastBlockChange` did inline) so every Java client with the
    /// chunk sent receives `block_change`; the session itself only
    /// re-sends the real state on failure to undo client prediction.
    fn place_block(&self, _x: i32, _y: i32, _z: i32, _network_id: u32) -> bool {
        false
    }
    fn block_network_id(&self, _x: i32, _y: i32, _z: i32) -> Option<u32> {
        None
    }
    /// Bedrock network id of stone (default creative placement).
    fn stone_network_id(&self) -> u32 {
        2_150_698_529
    }
    /// Java global block state for a Bedrock network id
    /// (`ModernChunk::bedrockState`); default is stone/air only.
    fn modern_block_state(&self, network_id: u32, _protocol: i32) -> i32 {
        if network_id == 3_690_217_760 {
            0
        } else {
            1
        }
    }
    /// Legacy Java state for a Bedrock network id (`ChunkBuilder::javaState`).
    fn legacy_block_state(&self, network_id: u32) -> i32 {
        if network_id == 3_690_217_760 {
            0
        } else {
            1
        }
    }

    fn legacy_chunk(&self, _cx: i32, _cz: i32) -> Option<LegacyChunk> {
        None
    }
    fn modern_chunk(&self, _protocol: i32, _cx: i32, _cz: i32) -> Option<ModernChunkData> {
        None
    }
    /// `registry_data` payloads for configuration, one per registry.
    fn registry_payloads(&self, _protocol: i32) -> Vec<Vec<u8>> {
        Vec::new()
    }
    /// `update_tags` payload (tags 0x0d) for configuration.
    fn tags_payload(&self, _protocol: i32) -> Vec<u8> {
        Vec::new()
    }
    /// Bedrock -> Java block edits since the last drain
    /// (`SessionManager::poll_block_changes` / `world.take_block_changes`).
    fn drain_block_changes(&self) -> Vec<BlockChange> {
        Vec::new()
    }
}
