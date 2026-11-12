//! Production [`ServerHooks`](java_bridge::ServerHooks) implementation.
//!
//! The Java listener (`java-bridge`'s `JavaServer`) owns frames, ids and
//! tick pacing; everything stateful is delegated here to the canonical
//! store: [`SessionManager`] (sessions + [`PlayerRegistry`]) and its
//! [`WorldState`]. This replaces the PHP `plugins/JavaBridge` server:
//! chat routes through [`crate::chat::handle_java_chat`], chunk bytes are
//! produced by [`bedrock_world::java_chunk`] over the shared chunk column,
//! and block edits enqueue in the same `WorldState` queues that already
//! feed Bedrock (`poll_block_changes`) — so one edit reaches both
//! platforms without any new IPC.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::net::SocketAddr;
use std::sync::Arc;

use bedrock_world::block_id;
use bedrock_world::java_chunk::{ChunkBuilder, ModernChunk, ModernProfile};
use java_bridge::{
    BlockChange, BossBarFrame, ChatOutcome, LegacyChunk, LoadedData, ModernChunkData,
    PlayerSnapshot, ScoreboardFrame, ServerHooks,
};
use java_bridge::display::bedrock_view_display_name;
use parking_lot::Mutex;
use php_plugin_bridge::{PlayerContext, PlayerJoinEvent, PlayerQuitEvent};

use crate::session::SessionManager;

/// `ServerHooks` backed by the live server core.
pub struct CoreHooks {
    manager: Arc<SessionManager>,
    motd: String,
    view_distance: i32,
    default_gamemode: i32,
    level_type: String,
    /// Converted legacy columns keyed by `(cx, cz, world version)`.
    legacy_cache: Mutex<PayloadCache<(i32, i32, u64), LegacyChunk>>,
    /// Converted modern columns keyed by `(protocol, cx, cz, version)`.
    modern_cache: Mutex<PayloadCache<(i32, i32, i32, u64), ModernChunkData>>,
}

/// Bound for converted Java chunk payloads. One modern payload is
/// ~62 KB (measured: 8.4 KB data + 53 KB light + heightmaps), and the
/// view area at radius 10 is 441 columns per protocol, so 512 entries
/// thrashed as soon as two protocols (or a moving window) competed.
/// 1024 covers two full view areas of one protocol.
const JAVA_PAYLOAD_CACHE_CAP: usize = 1024;
/// Cold columns one `prefetch_blocking` call generates at most. Warm
/// columns are skipped with a cache lookup, so a pass over the whole
/// view area (441 columns) is cheap; the budget bounds the blocking
/// job (~48 x ~20 ms / cores) and the next 500 ms pass continues with
/// the following batch instead of re-warming the nearest columns.
const PREFETCH_COLD_BUDGET: usize = 48;

/// FIFO cache of converted Java chunk bytes. First miss pays the
/// generator + conversion; afterwards `streamChunks` serves a memory
/// clone, so two Java players (and every re-stream) never regenerate a
/// column. A [`WorldState::set_block`] bumps the column version, so a
/// stale payload is never served after an edit.
struct PayloadCache<K, V> {
    entries: HashMap<K, V>,
    order: VecDeque<K>,
}

impl<K, V> PayloadCache<K, V>
where
    K: Eq + Hash + Clone,
{
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        self.entries.get(key).cloned()
    }

    fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    fn insert(&mut self, key: K, value: V) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
            while self.order.len() > JAVA_PAYLOAD_CACHE_CAP {
                if let Some(oldest) = self.order.pop_front() {
                    self.entries.remove(&oldest);
                }
            }
        }
        self.entries.insert(key, value);
    }
}

impl CoreHooks {
    pub fn new(
        manager: Arc<SessionManager>,
        motd: String,
        view_distance: i32,
        default_gamemode: i32,
        level_type: String,
    ) -> Self {
        Self {
            manager,
            motd,
            view_distance,
            default_gamemode,
            level_type,
            legacy_cache: Mutex::new(PayloadCache::new()),
            modern_cache: Mutex::new(PayloadCache::new()),
        }
    }

    /// Generates (off the tick path) the chunk columns around the
    /// current players and the converted Java bytes for `protocols`, so
    /// `streamChunks` only ever consumes already-generated cache.
    ///
    /// Must run on the blocking pool (`spawn_blocking`): a cold column
    /// costs the generator (~18 ms). Columns are processed in the
    /// caller's (nearest-first) order; already-warm columns are
    /// skipped with cache lookups and at most
    /// [`PREFETCH_COLD_BUDGET`] cold columns are generated per call —
    /// the next pass continues with the following batch. The old
    /// caller-side `truncate(32)` after sorting nearest-first meant
    /// the prefetch re-warmed the same nearest 32 columns forever and
    /// outer columns stayed cold, so the Java tick kept paying one
    /// ~18 ms generation per tick for the entire initial load.
    /// Cold generation + conversion run in parallel (rayon): the
    /// generator and converters hold no lock while working, the
    /// caches are only locked for the lookup/insert.
    /// Returns how many cold columns were generated.
    pub fn prefetch_blocking(&self, columns: &[(i32, i32)], protocols: &[i32]) -> usize {
        use rayon::prelude::*;
        // Cold = object missing, or a payload for a requested protocol
        // missing. Warm columns cost only the `contains` lookups.
        let mut cold: Vec<(i32, i32)> = Vec::new();
        for &(cx, cz) in columns {
            let world = self.manager.world();
            let version = world.chunk_version(cx, cz);
            let object_warm = world.has_chunk_cached(cx, cz);
            let legacy_warm = self.legacy_cache.lock().contains(&(cx, cz, version));
            let modern_warm = protocols.iter().all(|&protocol| {
                ModernProfile::for_protocol(protocol).is_none()
                    || self
                        .modern_cache
                        .lock()
                        .contains(&(protocol, cx, cz, version))
            });
            if !(object_warm && legacy_warm && modern_warm) {
                cold.push((cx, cz));
                if cold.len() >= PREFETCH_COLD_BUDGET {
                    break;
                }
            }
        }
        let generated = cold.len();
        cold.par_iter().for_each(|&(cx, cz)| {
            let world = self.manager.world();
            if !world.has_chunk_cached(cx, cz) {
                let _ = world.get_chunk_cached(cx, cz);
            }
            self.warm_payloads(cx, cz, protocols);
        });
        generated
    }

    /// Converts and caches only the legacy payload for one column
    /// whose object is already cached (no-op when warm). Split out
    /// of [`Self::warm_payloads`] so the cache-only tick lookups
    /// (`try_legacy_chunk`) do not pay for a modern conversion they
    /// will never send (and vice versa).
    fn warm_legacy_payload(&self, cx: i32, cz: i32) {
        let world = self.manager.world();
        let version = world.chunk_version(cx, cz);
        if !self.legacy_cache.lock().contains(&(cx, cz, version)) {
            let chunk = world.get_chunk_cached(cx, cz);
            let (bitmap, data) = ChunkBuilder::from_chunk(&chunk);
            self.legacy_cache.lock().insert(
                (cx, cz, version),
                LegacyChunk {
                    bitmap: bitmap as i32,
                    data,
                },
            );
        }
    }

    /// Converts and caches only the modern payload for `protocol`
    /// (no-op when warm or the protocol is unknown).
    fn warm_modern_payload(&self, protocol: i32, cx: i32, cz: i32) {
        let Some(profile) = ModernProfile::for_protocol(protocol) else {
            return;
        };
        let world = self.manager.world();
        let version = world.chunk_version(cx, cz);
        let key = (protocol, cx, cz, version);
        if !self.modern_cache.lock().contains(&key) {
            let chunk = world.get_chunk_cached(cx, cz);
            let data = ModernChunk::from_chunk(&chunk, &profile);
            let heights = ModernChunk::heights_from_chunk(&chunk);
            self.modern_cache.lock().insert(
                key,
                ModernChunkData {
                    heightmaps: ModernChunk::heightmaps(&heights, &profile),
                    light: ModernChunk::light(),
                    data,
                },
            );
        }
    }

    /// Converts and caches the legacy + modern payloads for one column
    /// whose object is already cached, without cloning anything out.
    fn warm_payloads(&self, cx: i32, cz: i32, protocols: &[i32]) {
        self.warm_legacy_payload(cx, cz);
        for &protocol in protocols {
            self.warm_modern_payload(protocol, cx, cz);
        }
    }
}

impl ServerHooks for CoreHooks {
    fn log(&self, message: String) {
        tracing::info!("Java bridge: {message}");
    }

    fn view_distance(&self) -> i32 {
        self.view_distance
    }

    fn spawn(&self) -> (f64, f64, f64) {
        let (x, y, z) = self.manager.world().spawn();
        (x as f64, y as f64, z as f64)
    }

    fn default_gamemode(&self) -> i32 {
        self.default_gamemode
    }

    fn difficulty(&self) -> i32 {
        self.manager.world().difficulty()
    }

    fn level_type(&self) -> String {
        self.level_type.clone()
    }

    fn motd(&self) -> String {
        self.motd.clone()
    }

    fn max_players(&self) -> i32 {
        self.manager.max_players()
    }

    fn world_time(&self) -> i64 {
        i64::from(self.manager.world().time())
    }

    fn world_time_running(&self) -> bool {
        self.manager.world().is_time_running()
    }

    fn load_player_data(&self, name: &str) -> Option<LoadedData> {
        // Same canonical `players/<name>.json` (JSON v2, 27 fields) that
        // `PlayerSession::save_player_data` writes.
        let path = std::path::Path::new("players").join(format!("{}.json", name.to_lowercase()));
        let bytes = std::fs::read(path).ok()?;
        let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        let number = |key: &str| value.get(key).and_then(serde_json::Value::as_f64);
        let (x, y, z) = (number("x")?, number("y")?, number("z")?);
        value_get(&value, "gamemode").map(|gamemode| LoadedData {
            x,
            y,
            z,
            yaw: number("yaw").unwrap_or(0.0),
            pitch: number("pitch").unwrap_or(0.0),
            gamemode: gamemode as i32,
            health: number("health").unwrap_or(20.0) as f32,
        })
    }

    fn is_name_available(&self, name: &str, _except_session: u64) -> bool {
        // A live Bedrock entry wins; Java-vs-Java duplicates are rejected
        // by the listener's own session registry before this hook.
        !matches!(
            self.manager.registry().get(name),
            Some(player) if !player.is_virtual
        )
    }

    fn snapshots(&self) -> Vec<PlayerSnapshot> {
        self.manager
            .registry()
            .all()
            .into_iter()
            .map(|player| PlayerSnapshot {
                name: player.name.clone(),
                x: player.x,
                y: player.y,
                z: player.z,
                yaw: player.yaw,
                pitch: player.pitch,
                head_yaw: player.head_yaw,
                is_sneaking: player.is_sneaking,
                gamemode: player.gamemode as i32,
                ping: player.ping as i32,
                is_virtual: player.is_virtual,
            })
            .collect()
    }

    fn get_snapshot(&self, name: &str) -> Option<PlayerSnapshot> {
        // Single registry lookup instead of the trait default
        // (`snapshots()` + linear scan): the Java tick calls this on
        // the position-forwarding path (`has_teleport_pending`) and
        // in `sync_teleport`/`sync_gamemode`, so the full clone used
        // to run several times per tick per player.
        self.manager.registry().get(name).map(|player| PlayerSnapshot {
            name: player.name.clone(),
            x: player.x,
            y: player.y,
            z: player.z,
            yaw: player.yaw,
            pitch: player.pitch,
            head_yaw: player.head_yaw,
            is_sneaking: player.is_sneaking,
            gamemode: player.gamemode as i32,
            ping: player.ping as i32,
            is_virtual: player.is_virtual,
        })
    }

    fn join_api(&self, _session_id: u64, peer: SocketAddr, snapshot: &PlayerSnapshot) {
        self.manager.clear_java_scoreboard_sent(&snapshot.name);
        let mut player = PlayerContext::new(snapshot.name.clone(), peer);
        player.x = snapshot.x;
        player.y = snapshot.y;
        player.z = snapshot.z;
        player.yaw = snapshot.yaw;
        player.pitch = snapshot.pitch;
        player.gamemode = snapshot.gamemode.max(0) as u32;
        player.is_virtual = true;
        player.display_name = bedrock_view_display_name(&snapshot.name, true);
        self.manager.registry().upsert(player.clone());
        // Let Bedrock viewers see the join immediately (spawn packet).
        self.manager.poll_virtual_players();
        if let Some(plugins) = self.manager.plugin_manager() {
            let mut event =
                PlayerJoinEvent::new(player, format!("{} joined the game", snapshot.name));
            match plugins.dispatch_event_sync(&mut event) {
                Ok(actions) => {
                    self.manager.apply_actions(&actions, 0);
                }
                Err(error) => tracing::warn!("Java PlayerJoinEvent failed: {error}"),
            }
        }
    }

    fn leave_api(&self, name: &str) {
        self.manager.clear_java_scoreboard_sent(name);
        let player = self.manager.registry().get(name);
        self.manager.registry().remove_if_virtual(name);
        if let Some(player) = player {
            if let Some(plugins) = self.manager.plugin_manager() {
                let mut event =
                    PlayerQuitEvent::new(player, format!("{} left the game", name), "left");
                if let Err(error) = plugins.dispatch_event_sync(&mut event) {
                    tracing::warn!("Java PlayerQuitEvent failed: {error}");
                }
            }
        }
        self.manager.poll_virtual_players();
    }

    fn update_virtual_position(&self, name: &str, x: f64, y: f64, z: f64, yaw: f64, pitch: f64) {
        if let Some(mut player) = self.manager.registry().get(name) {
            player.apply_position(x, y, z, yaw, pitch);
            player.head_yaw = yaw;
            self.manager.registry().upsert(player);
            self.manager.poll_virtual_players();
        }
    }

    fn update_virtual_sneak(&self, name: &str, sneaking: bool) {
        if let Some(mut player) = self.manager.registry().get(name) {
            if player.is_sneaking == sneaking {
                return;
            }
            player.is_sneaking = sneaking;
            self.manager.registry().upsert(player);
            self.manager.poll_virtual_players();
        }
    }

    fn take_plugin_messages(&self, name: &str) -> Vec<String> {
        self.manager.registry().take_messages(name)
    }

    /// apiBossBar Java side: ADD/UPDATE/REMOVE deltas for one virtual
    /// player, produced on demand by the manager (per-protocol).
    fn take_bossbar_frames(&self, name: &str, protocol: i32) -> Vec<BossBarFrame> {
        self.manager.take_bossbar_java_frames(name, protocol)
    }

    /// Scoreboard Java side: objective/score/display deltas for one
    /// virtual player, diffed by the manager against the last state
    /// sent to that client (per-protocol framing).
    fn take_scoreboard_frames(&self, name: &str, protocol: i32) -> Vec<ScoreboardFrame> {
        self.manager
            .take_scoreboard_java_frames(name, protocol)
            .into_iter()
            .map(|frame| ScoreboardFrame { frame })
            .collect()
    }

    fn handle_chat(&self, _session_id: u64, name: &str, message: &str) -> ChatOutcome {
        let sender = self.manager.registry().get(name).unwrap_or_else(|| {
            PlayerContext::new(
                name,
                "0.0.0.0:0"
                    .parse()
                    .unwrap_or(SocketAddr::from(([0, 0, 0, 0], 0))),
            )
        });
        let _outcome = crate::chat::handle_java_chat(&self.manager, &sender, message);
        // `handle_java_chat` already queued `feedback` into the sender's
        // registry inbox (drained once by `take_plugin_messages` via the
        // session's 5-tick `deliver_plugin_messages`). Returning the same
        // lines here would make the session send them immediately too,
        // so every Java command appeared twice. Keep single delivery.
        ChatOutcome {
            lines: Vec::new(),
            teleport: None,
            gamemode: None,
        }
    }

    fn break_block(&self, x: i32, y: i32, z: i32) -> bool {
        self.manager.world().set_block(x, y, z, block_id::AIR)
    }

    fn place_block(&self, x: i32, y: i32, z: i32, network_id: u32) -> bool {
        self.manager.world().set_block(x, y, z, network_id)
    }

    fn block_network_id(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        Some(self.manager.world().block_at(x, y, z))
    }

    fn stone_network_id(&self) -> u32 {
        block_id::STONE
    }

    fn modern_block_state(&self, network_id: u32, protocol: i32) -> i32 {
        match ModernProfile::for_protocol(protocol) {
            Some(profile) => ModernChunk::bedrock_state(network_id, &profile),
            None => {
                if network_id == block_id::AIR {
                    0
                } else {
                    1
                }
            }
        }
    }

    fn legacy_block_state(&self, network_id: u32) -> i32 {
        ChunkBuilder::java_state(network_id)
    }

    fn legacy_chunk(&self, cx: i32, cz: i32) -> Option<LegacyChunk> {
        let world = self.manager.world();
        let key = (cx, cz, world.chunk_version(cx, cz));
        if let Some(cached) = self.legacy_cache.lock().get(&key) {
            return Some(cached);
        }
        self.warm_legacy_payload(cx, cz);
        self.legacy_cache.lock().get(&key)
    }

    fn try_legacy_chunk(&self, cx: i32, cz: i32) -> Option<LegacyChunk> {
        // Cache-only (tick path): never run the generator here.
        let world = self.manager.world();
        let key = (cx, cz, world.chunk_version(cx, cz));
        if let Some(cached) = self.legacy_cache.lock().get(&key) {
            return Some(cached);
        }
        if !world.has_chunk_cached(cx, cz) {
            return None; // not warm yet: wait for the prefetch
        }
        self.warm_legacy_payload(cx, cz);
        self.legacy_cache.lock().get(&key)
    }

    fn registry_payloads(&self, protocol: i32) -> Vec<Vec<u8>> {
        // Real `registry_data` payloads from the embedded per-protocol
        // JSON (byte-parity with PHP `ModernProfile::registryPayloads`).
        // The `java-bridge` trait default is empty, which made
        // `beginConfiguration` send zero `registry_data` packets.
        match java_protocol::ModernProfile::for_protocol(protocol) {
            Some(profile) => profile.registry_payloads().unwrap_or_else(|error| {
                tracing::warn!("Java registry_payloads({protocol}) failed: {error}");
                Vec::new()
            }),
            None => {
                tracing::warn!(
                    "Java registry_payloads: protocolo moderno no soportado: {protocol}"
                );
                Vec::new()
            }
        }
    }

    fn tags_payload(&self, protocol: i32) -> Vec<u8> {
        // Real `update_tags` payload (minecraft-data `packet_tags`:
        // varInt registry count + identifier tagType + varInt tag
        // count + identifier tagName + varIntArray member ids).
        // Without this override the session sent a 0-byte payload and
        // the 775 client aborted with
        // `DecoderException: Failed to decode packet
        // 'clientbound/minecraft:update_tags'`.
        match java_protocol::ModernProfile::for_protocol(protocol) {
            Some(profile) => profile.tags_payload().unwrap_or_else(|error| {
                tracing::warn!("Java tags_payload({protocol}) failed: {error}");
                Vec::new()
            }),
            None => {
                tracing::warn!("Java tags_payload: protocolo moderno no soportado: {protocol}");
                Vec::new()
            }
        }
    }

    fn modern_chunk(&self, protocol: i32, cx: i32, cz: i32) -> Option<ModernChunkData> {
        // Unknown protocol: no payload, and never generate for nothing.
        let _ = ModernProfile::for_protocol(protocol)?;
        let world = self.manager.world();
        let key = (protocol, cx, cz, world.chunk_version(cx, cz));
        if let Some(cached) = self.modern_cache.lock().get(&key) {
            return Some(cached);
        }
        self.warm_modern_payload(protocol, cx, cz);
        self.modern_cache.lock().get(&key)
    }

    fn try_modern_chunk(&self, protocol: i32, cx: i32, cz: i32) -> Option<ModernChunkData> {
        // Cache-only (tick path): never run the generator here.
        let _ = ModernProfile::for_protocol(protocol)?;
        let world = self.manager.world();
        let key = (protocol, cx, cz, world.chunk_version(cx, cz));
        if let Some(cached) = self.modern_cache.lock().get(&key) {
            return Some(cached);
        }
        if !world.has_chunk_cached(cx, cz) {
            return None; // not warm yet: wait for the prefetch
        }
        self.warm_modern_payload(protocol, cx, cz);
        self.modern_cache.lock().get(&key)
    }

    fn drain_block_changes(&self) -> Vec<BlockChange> {
        // Fase 1 queue (Bedrock -> Java); the Bedrock-facing duplicate
        // (`take_broadcast_changes`) is drained separately by
        // `poll_block_changes`, so draining here starves nobody.
        self.manager
            .world()
            .take_changes()
            .into_iter()
            .map(|change| BlockChange {
                x: change.x,
                y: change.y,
                z: change.z,
                network_id: Some(change.block_id),
                kind: ModernChunk::block_name(change.block_id)
                    .unwrap_or("block")
                    .to_string(),
                previous_network_id: change.previous_block_id,
            })
            .collect()
    }
}

fn value_get(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(serde_json::Value::as_i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionManager;

    fn core_hooks() -> CoreHooks {
        CoreHooks::new(
            Arc::new(SessionManager::new()),
            "test".to_string(),
            12,
            0,
            "flat".to_string(),
        )
    }

    /// Converted Java payloads are cached per column + world version:
    /// a second request (e.g. a second Java player) serves the clone
    /// without regenerating, and a block edit bumps the version so the
    /// next build carries the new block instead of serving stale bytes.
    #[test]
    fn modern_and_legacy_chunks_are_cached_and_invalidated_by_edits() {
        let hooks = core_hooks();
        let first = hooks.modern_chunk(775, 2, -1).expect("modern build");
        let second = hooks.modern_chunk(775, 2, -1).expect("modern cache hit");
        assert_eq!(first, second);
        // Legacy is cached under its own key.
        let legacy = hooks.legacy_chunk(2, -1).expect("legacy build");
        assert_eq!(hooks.legacy_chunk(2, -1), Some(legacy.clone()));

        // Breaking a block in the column bumps its version: both payloads
        // must be rebuilt and must differ from the stale ones.
        let world = hooks.manager.world().clone();
        let before = world.block_at(2 * 16 + 3, 64, -1 * 16 + 5);
        assert!(world.set_block(2 * 16 + 3, 64, -1 * 16 + 5, 12345));
        assert_ne!(before, 12345);
        let rebuilt = hooks.modern_chunk(775, 2, -1).expect("modern rebuild");
        assert_ne!(rebuilt, first);
        assert_ne!(hooks.legacy_chunk(2, -1).expect("legacy rebuild"), legacy);

        // Unknown protocol still builds nothing.
        assert!(hooks.modern_chunk(999, 2, -1).is_none());
    }

    /// The background prefetch warms a column off the tick path:
    /// afterwards `modern_chunk`/`legacy_chunk` hit cache and the warm
    /// payloads are identical to on-demand builds.
    #[test]
    fn prefetch_warms_cache_for_stream_chunks() {
        let hooks = core_hooks();
        assert_eq!(hooks.prefetch_blocking(&[(5, 5)], &[775]), 1);
        // Second pass generates nothing (all warm).
        assert_eq!(hooks.prefetch_blocking(&[(5, 5)], &[775]), 0);
        let on_demand = hooks.modern_chunk(775, 5, 5).expect("warm hit");
        assert!(!on_demand.data.is_empty());
        assert!(hooks.legacy_chunk(5, 5).is_some());
    }

    /// Regression for the persistent "17 ms Java tick": the cache-only
    /// tick lookups must never run the generator. A cold column
    /// returns `None` (and stays cold — no generation happened), a
    /// prefetched column is served, and a block edit makes the column
    /// cold again until the prefetch re-warms it.
    #[test]
    fn try_chunks_never_generate_and_wait_for_prefetch() {
        let hooks = core_hooks();
        assert!(
            hooks.try_modern_chunk(775, 7, 7).is_none(),
            "cold column: tick must wait, not generate"
        );
        assert!(
            hooks.try_legacy_chunk(7, 7).is_none(),
            "cold column: tick must wait, not generate"
        );
        assert!(
            !hooks.manager.world().has_chunk_cached(7, 7),
            "try_* must not have generated the column"
        );
        assert_eq!(hooks.prefetch_blocking(&[(7, 7)], &[775]), 1);
        let warm = hooks.try_modern_chunk(775, 7, 7).expect("warm hit");
        assert!(!warm.data.is_empty());
        assert!(hooks.try_legacy_chunk(7, 7).is_some());
        // A block edit invalidates the object: try_* goes cold again
        // instead of serving a stale payload or generating inline.
        let world = hooks.manager.world().clone();
        assert!(world.set_block(7 * 16 + 1, 64, 7 * 16 + 1, 4242));
        assert!(hooks.try_modern_chunk(775, 7, 7).is_none());
        assert_eq!(hooks.prefetch_blocking(&[(7, 7)], &[775]), 1);
        assert!(hooks.try_modern_chunk(775, 7, 7).is_some());
    }

    /// Regression for the prefetch progression bug: with more cold
    /// columns than one pass generates, the next pass must continue
    /// with the following batch — not re-warm the same nearest
    /// columns forever (the old caller-side `truncate(32)`).
    #[test]
    fn prefetch_progresses_through_batches_instead_of_repeating_nearest() {
        let hooks = core_hooks();
        let columns: Vec<(i32, i32)> = (0..60).map(|i| (100 + i, 100)).collect();
        let first = hooks.prefetch_blocking(&columns, &[775]);
        assert_eq!(first, PREFETCH_COLD_BUDGET, "first pass bounded");
        let second = hooks.prefetch_blocking(&columns, &[775]);
        assert_eq!(second, 60 - PREFETCH_COLD_BUDGET, "second pass continues");
        let third = hooks.prefetch_blocking(&columns, &[775]);
        assert_eq!(third, 0, "all warm now");
        // And the warmed payloads are servable from the tick path.
        assert!(hooks.try_modern_chunk(775, 100, 100).is_some());
        assert!(hooks.try_modern_chunk(775, 159, 100).is_some());
    }

    /// Regression: `beginConfiguration` (775) used to send an EMPTY
    /// `update_tags` payload because `CoreHooks` never overrode the
    /// trait defaults in `java-bridge` (`Vec::new()`). The 26.1 client
    /// aborts with `DecoderException: Failed to decode packet
    /// 'clientbound/minecraft:update_tags'`. The payload must be the
    /// real `ModernProfile::tags_payload` bytes (per-protocol JSON).
    #[test]
    fn core_hooks_tags_payload_775_is_decodable_and_non_empty() {
        let hooks = core_hooks();
        let regs = hooks.registry_payloads(775);
        assert_eq!(regs.len(), 26, "registry count 775: {regs:?}");
        assert_eq!(regs[0].len(), 19840, "first registry len 775");
        let tags = hooks.tags_payload(775);
        assert_eq!(tags.len(), 20152, "tags len 775");
        // Full decode must consume every byte (minecraft-data
        // packet_tags schema: varInt(count) + per-registry
        // identifier(tagType) + varInt(count) + identifier(tagName)
        // + varIntArray(entries)).
        let mut offset = 0;
        let registry_count = java_protocol::protocol::read_var_int(&tags, &mut offset).unwrap();
        assert!(registry_count > 0);
        for _ in 0..registry_count {
            java_protocol::protocol::read_string(&tags, &mut offset).unwrap();
            let tag_count = java_protocol::protocol::read_var_int(&tags, &mut offset).unwrap();
            for _ in 0..tag_count {
                java_protocol::protocol::read_string(&tags, &mut offset).unwrap();
                java_protocol::protocol::read_var_int_array(&tags, &mut offset).unwrap();
            }
        }
        assert_eq!(offset, tags.len(), "tags payload must be fully consumed");
        // Fluid ids are stable (empty=0, flowing_water=1, water=2,
        // flowing_lava=3, lava=4) for every protocol.
        for proto in [769, 770, 773, 774, 775] {
            let profile = java_protocol::ModernProfile::for_protocol(proto).unwrap();
            assert_eq!(
                profile.tag_members("fluid", "water"),
                vec![1, 2],
                "water {proto}"
            );
            assert_eq!(
                profile.tag_members("fluid", "lava"),
                vec![3, 4],
                "lava {proto}"
            );
        }
    }
}
