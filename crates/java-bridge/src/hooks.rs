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
    /// Bedrock head yaw ([0, 360)); Java virtuals report body yaw here.
    pub head_yaw: f64,
    /// Sneak state (Bedrock `is_sneaking` / Java shift), shown to Java
    /// viewers via the entity metadata flags byte (`sneaking = 0x02`).
    pub is_sneaking: bool,
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
            head_yaw: 0.0,
            is_sneaking: false,
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
    /// Bedrock network id of the previous block before this edit.
    pub previous_network_id: Option<u32>,
}

/// One queued Java boss-bar frame for a player (apiBossBar Java side).
/// Produced by the server core (`SessionManager::take_bossbar_java_frames`,
/// encoded with this crate's [`crate::bossbar`]) and drained by the
/// session every tick, exactly like chat lines (`take_plugin_messages`).
/// `protocol` is the target client's wire protocol so codecs stay
/// per-version (340 string vs 769+ NBT titles); frames are already
/// length-prefixed and can be appended verbatim.
#[derive(Debug, Clone, PartialEq)]
pub struct BossBarFrame {
    pub frame: Vec<u8>,
}

/// One queued Java scoreboard frame for a player (scoreboard Java
/// side). Produced by the server core
/// (`SessionManager::take_scoreboard_java_frames`, encoded with
/// `java-protocol::scoreboard`) and drained by the session every tick,
/// exactly like boss-bar frames. Frames are already length-prefixed
/// for the target `protocol` and can be appended verbatim.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreboardFrame {
    pub frame: Vec<u8>,
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
    /// Java sneak/sprint toggle (`entity_action` / `player_input` shift bit):
    /// flips `PlayerContext.is_sneaking` in the registry so Bedrock viewers
    /// receive the `SetActorDataPacket` sneak flag for this virtual player.
    fn update_virtual_sneak(&self, _name: &str, _sneaking: bool) {}
    /// `player.take_messages`, drained by `deliverPluginMessages`.
    fn take_plugin_messages(&self, _name: &str) -> Vec<String> {
        Vec::new()
    }

    /// Queued Java `boss_bar` frames for `name` on `protocol`
    /// (apiBossBar Java side), drained by the session every tick and
    /// appended verbatim to the output buffer (already framed). Default
    /// (in-memory test hooks) is empty — no bars.
    fn take_bossbar_frames(&self, _name: &str, _protocol: i32) -> Vec<BossBarFrame> {
        Vec::new()
    }

    /// Queued Java scoreboard frames for `name` on `protocol`
    /// (`scoreboard_objective` / `scoreboard_score` /
    /// `scoreboard_display_objective` / `reset_score`), drained by the
    /// session every tick and appended verbatim (already framed).
    /// Default (in-memory test hooks) is empty — no objectives.
    fn take_scoreboard_frames(&self, _name: &str, _protocol: i32) -> Vec<ScoreboardFrame> {
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
    /// Cache-only chunk lookup for the tick path (`stream_chunks`).
    ///
    /// Unlike [`Self::modern_chunk`] / [`Self::legacy_chunk`], this must
    /// never run the world generator: a cold column costs ~18 ms, which
    /// blows the whole Java tick budget (the "17 ms tick" report). It
    /// returns the already-converted payload when one is cached — or
    /// converts one from an already-generated chunk object (sub-ms) —
    /// and `None` when the column is not warm yet, so the session can
    /// wait for the background prefetch instead of generating inline.
    /// The default (in-memory test hooks) delegates to the generating
    /// lookups, preserving their on-demand behaviour.
    fn try_modern_chunk(&self, protocol: i32, cx: i32, cz: i32) -> Option<ModernChunkData> {
        self.modern_chunk(protocol, cx, cz)
    }
    fn try_legacy_chunk(&self, cx: i32, cz: i32) -> Option<LegacyChunk> {
        self.legacy_chunk(cx, cz)
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
