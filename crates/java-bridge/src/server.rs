//! `JavaServer`: the TCP listener (port 25565 by default) and the
//! per-tick phases [`JavaServer::tick`] drives.
//!
//! The tick is **not** scheduled here: there is exactly one server clock
//! — the central 20 TPS loop in `server-core::main`. Every 50 ms it calls
//! `SessionManager::tick` (Bedrock) and then `JavaServer::tick_at` with
//! the same tick number (Java), so both platforms mutate state on the
//! same pulse and share `WorldState::time`. `JavaServer::run_arc` only
//! accepts connections and flushes sockets; `handle_stream` feeds bytes
//! with [`JavaSession::poll_bytes`] and drains [`JavaSession::take_out`]
//! (immediate flush + 20 ms drain), exactly like the Bedrock transport
//! drains `pending_outgoing` on its dispatch path. The tick itself only
//! queues envelopes in `out_buf` under a shared deadline — it never
//! writes to a socket.
//!
//! Tick gating (mirrors PHP `JavaServer::tick`):
//! - per-session `tick()` (keep-alive, position push, plugin messages,
//!   gamemode/teleport sync, chunk streaming under a shared deadline),
//! - `tick_tab()` every tick (memory-only diff, no host call),
//! - `tick_time_sync()` every 20 ticks or on a jump,
//! - `tick_entities()` every tick (20 Hz, one snapshots read): remote
//!   players move on `rel_entity_move` / `entity_move_look`, which the
//!   Java client interpolates, so a 5 Hz entity cadence showed up as
//!   stepping. One registry snapshot per tick is a small Vec clone —
//!   the tick budget is spent on chunk bytes, not on this diff,
//! - `tick_block_changes()` every 2 ticks (10 Hz drain). Tail heavy
//!   work — cold chunk builds (~18 ms) — is warmed off the tick by the
//!   `CoreHooks::prefetch_blocking` sidecar in `main.rs`.
//
//! Entity/TAB targets mirror `entityTargetsFor` / `tabTargets` and the
//! reverse of `SessionManager::sync_virtuals_locked`: snapshots from
//! [`ServerHooks::snapshots`] are the source of truth, Bedrock snapshots
//! win over virtual ones with the same name, Java yaw/pitch conversion
//! happens here (snapshots are Bedrock-canonical), and the chunk gate
//! (`has_sent_chunk`) keeps spawns out of unloaded columns.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::display::java_view_display_name;
use crate::hooks::{PlayerSnapshot, ServerHooks};
use crate::profile;
use crate::session::{EntityTarget, JavaSession, TabTarget};
use crate::wire;

pub const DEFAULT_PORT: u16 = 25565;
pub const TICK_BUDGET: Duration = Duration::from_millis(10);
pub const ENTITY_SYNC_INTERVAL_TICKS: u64 = 1;
pub const BLOCK_CHANGES_INTERVAL_TICKS: u64 = 2;
pub const TIME_SYNC_INTERVAL_TICKS: u64 = 20;
/// Central entity-id registry starts here and never reuses ids.
pub const FIRST_ENTITY_ID: i32 = 1_000_000;

/// `statusJson`: version/players/description for the status ping.
pub fn status_json(hooks: &dyn ServerHooks, protocol: Option<i32>) -> String {
    let (version_name, version_protocol) = match protocol {
        Some(p) if profile::is_modern(p) => (profile::version_name(p).unwrap_or("").to_string(), p),
        Some(p) if p == wire::PROTOCOL_VERSION => (wire::VERSION_NAME.to_string(), p),
        Some(p) => (
            format!(
                "{} / {}",
                wire::VERSION_NAME,
                profile::VERSION_NAMES.join(" / ")
            ),
            p,
        ),
        None => (
            profile::PRIMARY_VERSION_NAME.to_string(),
            profile::PRIMARY_PROTOCOL,
        ),
    };
    let sample: Vec<serde_json::Value> = hooks
        .snapshots()
        .iter()
        .take(10)
        .map(|s| {
            let display = java_view_display_name(&s.name, s.is_virtual);
            serde_json::json!({
                "name": display,
                "id": wire::uuid_string(&wire::offline_uuid(&display)),
            })
        })
        .collect();
    serde_json::json!({
        "version": { "name": version_name, "protocol": version_protocol },
        "players": {
            "max": hooks.max_players(),
            "online": online_count(hooks),
            "sample": sample,
        },
        "description": { "text": hooks.motd() },
    })
    .to_string()
}

pub fn online_count(hooks: &dyn ServerHooks) -> usize {
    hooks.snapshots().len()
}

/// TAB targets: every online player, including the observer itself.
///
/// Names are *display* names for this Java observer (Floodgate-style, see
/// [`crate::display`]): Bedrock snapshots become `"." + raw`, Java snapshots
/// stay verbatim. The lowercase key is also the display name's, so
/// `sync_tab` diffing and `offline_uuid(display)` stay coherent with
/// `spawn_entity` / nametag, which are built from the same `EntityTarget`.
pub fn tab_targets(hooks: &dyn ServerHooks, extra: Option<&[TabTarget]>) -> Vec<TabTarget> {
    let mut targets: HashMap<String, TabTarget> = HashMap::new();
    for snapshot in hooks.snapshots() {
        let display = java_view_display_name(&snapshot.name, snapshot.is_virtual);
        targets.insert(
            display.to_lowercase(),
            TabTarget {
                name: display,
                gamemode: snapshot.gamemode.clamp(0, 3),
                ping: snapshot.ping.max(0),
            },
        );
    }
    if let Some(extra) = extra {
        for target in extra {
            targets
                .entry(target.name.to_lowercase())
                .or_insert_with(|| target.clone());
        }
    }
    targets.into_values().collect()
}

/// Entity targets for one observer: everyone but itself, chunk-gated,
/// Bedrock yaw/pitch converted to Java. Bedrock snapshots win over
/// virtual ones with the same raw name (same rule as `sync_virtuals_locked`).
/// `name` is the Java-view display name (Bedrock -> `"." + raw`, Java verbatim),
/// so `spawn_entity` UUID, TAB UUID and nametag all derive from one string.
pub fn entity_targets_for(
    _hooks: &dyn ServerHooks,
    observer: &JavaSession,
    snapshots: &[PlayerSnapshot],
) -> Vec<EntityTarget> {
    let self_name = observer.username.as_deref().unwrap_or("");
    let mut by_name: HashMap<String, PlayerSnapshot> = HashMap::new();
    for snapshot in snapshots {
        if snapshot.name.is_empty() {
            continue;
        }
        let key = snapshot.name.to_lowercase();
        let replace = match by_name.get(&key) {
            None => true,
            Some(existing) => existing.is_virtual && !snapshot.is_virtual,
        };
        if replace {
            by_name.insert(key, snapshot.clone());
        }
    }
    let mut targets = Vec::new();
    for snapshot in by_name.values() {
        if snapshot.name.eq_ignore_ascii_case(self_name) {
            continue;
        }
        let cx = (snapshot.x / 16.0).floor() as i32;
        let cz = (snapshot.z / 16.0).floor() as i32;
        if !observer.has_sent_chunk(cx, cz) {
            continue;
        }
        targets.push(EntityTarget {
            name: java_view_display_name(&snapshot.name, snapshot.is_virtual),
            x: snapshot.x,
            y: snapshot.y,
            z: snapshot.z,
            yaw: wire::bedrock_yaw_to_java(snapshot.yaw),
            pitch: wire::bedrock_pitch_to_java(snapshot.pitch),
            head_yaw: wire::bedrock_yaw_to_java(snapshot.head_yaw),
            is_sneaking: snapshot.is_sneaking,
            gamemode: snapshot.gamemode,
            ping: snapshot.ping,
        });
    }
    targets
}

/// Deterministic entity id for direct session tests (from 1_000_000).
/// Production sessions allocate through `JavaServer::entity_id_for` /
/// `ServerHooks::entity_id_for`; this helper never allocates.
pub fn entity_id_for(hooks: &dyn ServerHooks, name: &str) -> i32 {
    let _ = hooks;
    let mut hash: u32 = 0;
    for byte in name.to_lowercase().as_bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(*byte as u32);
    }
    FIRST_ENTITY_ID + (hash % 900_000) as i32
}

pub struct JavaServer {
    hooks: Arc<dyn ServerHooks>,
    sessions: Mutex<HashMap<u64, JavaSession>>,
    next_session_id: AtomicU64,
    entity_ids: Mutex<HashMap<String, i32>>,
    next_entity_id: Mutex<i32>,
    tick_count: AtomicU64,
    ticks_since_time_sync: Mutex<u64>,
    last_broadcast_time: Mutex<Option<i64>>,
    last_broadcast_time_running: Mutex<Option<bool>>,
    last_known_blocks: Mutex<HashMap<(i32, i32, i32), Option<u32>>>,
    slow_tick_log_at: Mutex<Option<Instant>>,
}

impl JavaServer {
    pub fn new(hooks: Arc<dyn ServerHooks>) -> Self {
        Self {
            hooks,
            sessions: Mutex::new(HashMap::new()),
            next_session_id: AtomicU64::new(1),
            entity_ids: Mutex::new(HashMap::new()),
            next_entity_id: Mutex::new(FIRST_ENTITY_ID),
            tick_count: AtomicU64::new(0),
            ticks_since_time_sync: Mutex::new(0),
            last_broadcast_time: Mutex::new(None),
            last_broadcast_time_running: Mutex::new(None),
            last_known_blocks: Mutex::new(HashMap::new()),
            slow_tick_log_at: Mutex::new(None),
        }
    }

    pub fn hooks(&self) -> &dyn ServerHooks {
        self.hooks.as_ref()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.lock().len()
    }

    pub fn is_playing(&self, session_id: u64) -> bool {
        self.sessions
            .lock()
            .get(&session_id)
            .map(|s| s.is_playing())
            .unwrap_or(false)
    }

    /// Allocating entity id (`JavaServer::entityIdFor`).
    pub fn entity_id_for(&self, name: &str) -> i32 {
        let key = name.to_lowercase();
        let mut ids = self.entity_ids.lock();
        if let Some(id) = ids.get(&key) {
            return *id;
        }
        let mut next = self.next_entity_id.lock();
        let id = *next;
        *next += 1;
        ids.insert(key, id);
        id
    }

    /// Insert a session (Tokio accept loop + tests). Returns its id.
    pub fn add_session(&self, peer: SocketAddr) -> u64 {
        let id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
        self.sessions.lock().insert(id, JavaSession::new(id, peer));
        id
    }

    pub fn remove_session(&self, session_id: u64) -> Option<JavaSession> {
        self.sessions.lock().remove(&session_id)
    }

    /// One Java tick with the central clock's tick number.
    ///
    /// This is the entry point the single server tick in
    /// `server-core::main` calls right after
    /// `SessionManager::tick` (same `tick` number, so Java sees the
    /// world time Bedrock just advanced). Tests that drive their own
    /// clock may call [`JavaServer::tick`] instead, which increments
    /// the local counter.
    pub fn tick_at(&self, tick_count: u64) {
        // Keep the stored counter in sync with the central clock so
        // entity/time/block gating is anchored to the unified tick,
        // never to a drifting local interval.
        self.tick_count.fetch_max(tick_count, Ordering::SeqCst);
        self.tick_inner(tick_count)
    }

    /// Current central tick number (0 before the first tick).
    pub fn current_tick(&self) -> u64 {
        self.tick_count.load(Ordering::SeqCst)
    }

    /// One bridge tick: same gating as [`JavaServer::tick_at`] but with
    /// a locally-incremented tick number. Production uses `tick_at`
    /// from the central clock; this stays for tests / standalone use.
    pub fn tick(&self) {
        let tick_count = self.tick_count.fetch_add(1, Ordering::SeqCst) + 1;
        self.tick_inner(tick_count)
    }

    fn tick_inner(&self, tick_count: u64) {
        let started = Instant::now();
        let deadline = started + TICK_BUDGET;
        // Per-session locks: a cold chunk build (~18 ms) under a global
        // sessions lock starves the connection tasks of every other
        // player, so each session ticks under its own lock. The central
        // 20 TPS clock (server-core::main) is the only thing pacing us —
        // `run_arc` runs no interval of its own.
        // `streamChunks` consumes the background-warmed payload cache,
        // so a warm tick never pays generation at all.
        let ids: Vec<u64> = self.sessions.lock().keys().copied().collect();
        for id in ids {
            if let Some(session) = self.sessions.lock().get_mut(&id) {
                session.tick(self.hooks.as_ref(), Some(deadline));
            }
        }
        let after_sessions = Instant::now();
        self.tick_tab();
        let after_tab = Instant::now();
        if Instant::now() <= deadline {
            self.tick_time_sync();
            if tick_count.is_multiple_of(ENTITY_SYNC_INTERVAL_TICKS) {
                self.tick_entities();
            }
        }
        // `tick_block_changes` (Bedrock -> Java block sync) is a bounded
        // queue drain (512 entries max), so it must not depend on the
        // session-tick deadline: a slow chunk-streaming pass used to skip
        // it entirely, and with the queue capped the oldest edits were
        // then dropped by `push_capped` without ever reaching Java
        // viewers. It mirrors the Bedrock side, where `poll_block_changes`
        // runs first in `SessionManager::tick`.
        if tick_count.is_multiple_of(BLOCK_CHANGES_INTERVAL_TICKS) {
            self.tick_block_changes();
        }
        let elapsed = started.elapsed();
        if elapsed > Duration::from_millis(10) {
            // Phase profile: sessions = streamChunks/keepAlive/position
            // push, tab = player_info diff, rest = time/entities/blocks.
            tracing::debug!(
                "Java bridge tick {tick_count}: total={elapsed:?} sessions={:?} tab={:?} rest={:?} deadline_exceeded={}",
                after_sessions - started,
                after_tab - after_sessions,
                after_tab.elapsed(),
                Instant::now() > deadline,
            );
        }
        if elapsed > Duration::from_millis(15) {
            let mut last = self.slow_tick_log_at.lock();
            let quiet = last
                .map(|t| t.elapsed() > Duration::from_secs(10))
                .unwrap_or(true);
            if quiet {
                *last = Some(Instant::now());
                self.hooks.log(format!(
                    "Tick del puente Java lento: {} ms (sessions={:?} tab={:?} rest={:?})",
                    elapsed.as_millis(),
                    after_sessions - started,
                    after_tab - after_sessions,
                    after_tab.elapsed(),
                ));
            }
        }
    }

    /// Distinct wire protocols of the Java clients currently in play.
    /// The chunk prefetch (main.rs) warms converted payloads only for
    /// these — warming all five modern protocols per column cost
    /// ~5x the conversion CPU and thrashed the shared FIFO payload
    /// cache (512 entries) during the initial load.
    pub fn active_protocols(&self) -> Vec<i32> {
        let mut protocols: Vec<i32> = self
            .sessions
            .lock()
            .values()
            .filter(|s| s.is_playing())
            .map(|s| s.protocol)
            .filter(|p| *p != 0)
            .collect();
        protocols.sort_unstable();
        protocols.dedup();
        protocols
    }

    fn tick_tab(&self) {
        if !self.any_playing() {
            return;
        }
        let targets = tab_targets(self.hooks.as_ref(), None);
        for session in self.sessions.lock().values_mut() {
            session.sync_tab(&targets);
        }
    }

    fn tick_entities(&self) {
        if !self.any_playing() {
            return;
        }
        let snapshots = self.hooks.snapshots();
        let mut sessions = self.sessions.lock();
        for session in sessions.values_mut() {
            if !session.is_playing() {
                continue;
            }
            let targets = entity_targets_for(self.hooks.as_ref(), session, &snapshots);
            session.sync_entities(self.hooks.as_ref(), &targets);
        }
    }

    fn tick_time_sync(&self) {
        if !self.any_playing() {
            return;
        }
        let mut since = self.ticks_since_time_sync.lock();
        *since += 1;
        let last = *self.last_broadcast_time.lock();
        if last.is_some() && *since < TIME_SYNC_INTERVAL_TICKS && !(*since).is_multiple_of(5) {
            return;
        }
        let time = self.hooks.world_time();
        let running = self.hooks.world_time_running();
        let jumped = last
            .map(|prev| (time - prev).abs() > TIME_SYNC_INTERVAL_TICKS as i64)
            .unwrap_or(false);
        let running_changed = *self.last_broadcast_time_running.lock() != Some(running)
            && self.last_broadcast_time_running.lock().is_some();
        if last.is_some() && !jumped && !running_changed && *since < TIME_SYNC_INTERVAL_TICKS {
            return;
        }
        for session in self.sessions.lock().values_mut() {
            session.send_time(time, running);
        }
        *self.last_broadcast_time.lock() = Some(time);
        *self.last_broadcast_time_running.lock() = Some(running);
        *since = 0;
    }

    fn tick_block_changes(&self) {
        if !self.any_playing() {
            return;
        }
        let changes: Vec<_> = self.hooks.drain_block_changes();
        if !changes.is_empty() {
            tracing::debug!("tick_block_changes: drained {} changes", changes.len());
        }
        for change in changes {
            let key = (change.x, change.y, change.z);
            let known = self
                .last_known_blocks
                .lock()
                .get(&key)
                .copied()
                .unwrap_or(None);
            if change.network_id.is_some() && known == change.network_id {
                tracing::debug!("tick_block_changes: echo suppressed at ({},{},{})", change.x, change.y, change.z);
                continue; // echo of our own place/break
            }
            self.broadcast_block_change_with_prev(
                change.x,
                change.y,
                change.z,
                &change.kind,
                change.network_id,
                change.previous_network_id,
            );
        }
    }

    fn any_playing(&self) -> bool {
        self.sessions.lock().values().any(|s| s.is_playing())
    }

    /// Send a chat line to every Java client in play (`broadcastChat`).
    pub fn broadcast_chat(&self, line: &str) {
        for session in self.sessions.lock().values_mut() {
            if session.is_playing() {
                session.send_chat(line);
            }
        }
    }

    /// Forward a real block state to Java clients that already have the
    /// chunk (`broadcastBlockChange`); records it as the known state so
    /// the drain above suppresses our own echo.
    pub fn broadcast_block_change(
        &self,
        x: i32,
        y: i32,
        z: i32,
        kind: &str,
        network_id: Option<u32>,
    ) {
        self.broadcast_block_change_with_prev(x, y, z, kind, network_id, None);
    }

    pub fn broadcast_block_change_with_prev(
        &self,
        x: i32,
        y: i32,
        z: i32,
        kind: &str,
        network_id: Option<u32>,
        prev_network_id: Option<u32>,
    ) {
        let network_id = network_id.or_else(|| self.hooks.block_network_id(x, y, z));
        let known_before = {
            let mut known = self.last_known_blocks.lock();
            let old = known.insert((x, y, z), network_id);
            if known.len() > 8192 {
                if let Some(first) = known.keys().next().copied() {
                    known.remove(&first);
                }
            }
            old.flatten()
        };
        let prev_id = prev_network_id.or(known_before);
        let is_break = kind == "air" || network_id == Some(3_690_217_760);
        let (cx, cz) = (
            (x as f64 / 16.0).floor() as i32,
            (z as f64 / 16.0).floor() as i32,
        );
        let mut recipients = 0usize;
        for session in self.sessions.lock().values_mut() {
            if session.is_playing() && session.has_sent_chunk(cx, cz) {
                session.send_block_change(self.hooks.as_ref(), x, y, z, kind, network_id);
                if is_break {
                    let prev_state = match prev_id {
                        Some(pid) if session.is_modern() => self.hooks.modern_block_state(pid, session.protocol),
                        Some(pid) => self.hooks.legacy_block_state(pid),
                        None if session.is_modern() => 1,
                        None => 1 << 4,
                    };
                    session.send_world_event(self.hooks.as_ref(), 2001, x, y, z, prev_state, false);
                }
                recipients += 1;
            }
        }
        tracing::debug!(
            "broadcast_block_change: ({},{},{}) kind={} chunk=({},{}) recipients={}",
            x, y, z, kind, cx, cz, recipients
        );
    }

    /// Names of Java players currently in play (`javaPlayerNames`).
    pub fn java_player_names(&self) -> Vec<String> {
        self.sessions
            .lock()
            .values()
            .filter(|s| s.is_playing())
            .filter_map(|s| s.username.clone())
            .collect()
    }

    /// Cleanup after a TCP disconnect (`onDisconnect`): leave the API
    /// unless another Java session with the same name is still playing.
    pub fn on_disconnect(&self, session: &JavaSession) {
        let Some(name) = session.username.clone() else {
            return;
        };
        if !session.api_joined && !session.is_playing() {
            return;
        }
        let still_playing = self.sessions.lock().values().any(|other| {
            other.session_id != session.session_id
                && other.is_playing()
                && other
                    .username
                    .as_deref()
                    .map(|n| n.eq_ignore_ascii_case(&name))
                    .unwrap_or(false)
        });
        if !still_playing {
            self.hooks.leave_api(&name);
        }
    }

    /// Serve one accepted TCP stream until it closes.
    pub async fn handle_stream(&self, stream: TcpStream) {
        let peer = stream
            .peer_addr()
            .unwrap_or_else(|_| "127.0.0.1:0".parse().unwrap());
        let session_id = self.add_session(peer);
        let (mut read, mut write) = stream.into_split();
        let mut buffer = [0u8; 65536];
        let mut drain = tokio::time::interval(Duration::from_millis(20));
        loop {
            tokio::select! {
                result = read.read(&mut buffer) => {
                    match result {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let (alive, out) = {
                                let mut sessions = self.sessions.lock();
                                let Some(session) = sessions.get_mut(&session_id) else {
                                    break;
                                };
                                let alive = session.poll_bytes(self.hooks.as_ref(), &buffer[..n]);
                                let out = session.take_out();
                                (alive, out)
                            };
                            if !out.is_empty() {
                                let _ = write.write_all(&out).await;
                                let _ = write.flush().await;
                            }
                            if !alive {
                                break;
                            }
                        }
                    }
                }
                _ = drain.tick() => {
                    // Flush sidecar: drain envelopes queued by the central
                    // tick (chunks, TAB, entities) without a client write.
                    // This is socket flush only — no logical tick runs here.
                    let out = {
                        let mut sessions = self.sessions.lock();
                        sessions
                            .get_mut(&session_id)
                            .map(|s| s.take_out())
                            .unwrap_or_default()
                    };
                    if !out.is_empty() && write.write_all(&out).await.is_err() {
                        break;
                    }
                }
            }
        }
        if let Some(session) = self.remove_session(session_id) {
            self.on_disconnect(&session);
        }
    }

    /// Production entry: bind 25565 (or `addr`), accept + flush forever.
    ///
    /// There is deliberately **no tick interval here**: the single server
    /// clock in `server-core::main` (20 TPS) calls `SessionManager::tick`
    /// (Bedrock) and then `JavaServer::tick_at` with the same tick number
    /// (Java). Keeping a second 50 ms `tokio::time::interval` here with
    /// `MissedTickBehavior::Skip` lost ticks after a slow chunk build and
    /// let Java drift from Bedrock time. Tokio's only jobs in this loop
    /// are `accept` + per-connection `handle_stream` (poll_bytes +
    /// take_out + 20 ms drain flush), so network stalls never block the
    /// logical clock, and the logical clock never blocks network I/O.
    /// Tests drive `tick()` / `tick_at()` directly and use
    /// `handle_stream` instead of this loop.
    pub async fn run_arc(self: Arc<Self>, addr: &str) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let server = Arc::clone(&self);
                    tokio::spawn(async move {
                        server.handle_stream(stream).await;
                    });
                }
                Err(_) => break,
            }
        }
        Ok(())
    }
}

/// Names currently reserved by playing Java sessions (test helper).
pub fn java_names(sessions: &HashMap<u64, JavaSession>) -> HashSet<String> {
    sessions
        .values()
        .filter(|s| s.is_playing())
        .filter_map(|s| s.username.clone().map(|n| n.to_lowercase()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::{BlockChange, ServerHooks};

    struct Stub {
        snapshots: Vec<PlayerSnapshot>,
        changes: Mutex<Vec<BlockChange>>,
    }

    impl ServerHooks for Stub {
        fn view_distance(&self) -> i32 {
            1
        }
        fn spawn(&self) -> (f64, f64, f64) {
            (0.5, 5.62, 0.5)
        }
        fn snapshots(&self) -> Vec<PlayerSnapshot> {
            self.snapshots.clone()
        }
        fn break_block(&self, x: i32, y: i32, z: i32) -> bool {
            // Mirrors CoreHooks -> WorldState::set_block: a real break
            // enqueues the AIR edit for the next tick drain.
            self.changes.lock().push(BlockChange {
                x,
                y,
                z,
                network_id: Some(3_690_217_760), // AIR
                kind: "air".to_string(),
                previous_network_id: None,
            });
            true
        }
        fn drain_block_changes(&self) -> Vec<BlockChange> {
            std::mem::take(&mut *self.changes.lock())
        }
    }

    #[test]
    fn entity_targets_skip_self_and_chunk_gate() {
        let hooks = Stub {
            changes: Mutex::new(vec![]),
            snapshots: vec![
                PlayerSnapshot::new("Steve", 0.5, 5.0, 0.5),
                PlayerSnapshot {
                    name: "Alex".to_string(),
                    x: 100.0,
                    y: 64.0,
                    z: 100.0,
                    yaw: 90.0,
                    pitch: 0.0,
                    head_yaw: 90.0,
                    is_sneaking: false,
                    gamemode: 1,
                    ping: 0,
                    is_virtual: false,
                },
            ],
        };
        let mut observer = JavaSession::new(1, "127.0.0.1:1".parse().unwrap());
        observer.username = Some("Steve".to_string());
        observer.sent_chunks.insert((0, 0));
        let targets = entity_targets_for(&hooks, &observer, &hooks.snapshots());
        assert!(targets.is_empty(), "Alex is outside the sent chunk");
        observer.sent_chunks.insert((6, 6));
        let targets = entity_targets_for(&hooks, &observer, &hooks.snapshots());
        assert_eq!(targets.len(), 1);
        // Bedrock player (is_virtual == false) appears in Java view verbatim (no dot).
        assert_eq!(targets[0].name, "Alex");
        // Bedrock yaw 90 (west) -> Java 90 (same compass).
        assert!((targets[0].yaw - 90.0).abs() < 0.001);
    }

    #[test]
    fn tab_targets_leave_both_bedrock_and_java_verbatim_for_java() {
        let hooks = Stub {
            changes: Mutex::new(vec![]),
            snapshots: vec![
                PlayerSnapshot {
                    name: "Steve".to_string(),
                    x: 0.5,
                    y: 64.0,
                    z: 0.5,
                    yaw: 0.0,
                    pitch: 0.0,
                    head_yaw: 0.0,
                    is_sneaking: false,
                    gamemode: 0,
                    ping: 7,
                    is_virtual: false, // Bedrock
                },
                PlayerSnapshot {
                    name: "Procatin0".to_string(),
                    x: 1.5,
                    y: 64.0,
                    z: 1.5,
                    yaw: 0.0,
                    pitch: 0.0,
                    head_yaw: 0.0,
                    is_sneaking: false,
                    gamemode: 1,
                    ping: 3,
                    is_virtual: true, // Java
                },
            ],
        };
        let targets = tab_targets(&hooks, None);
        let mut names: Vec<String> = targets.iter().map(|t| t.name.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["Procatin0".to_string(), "Steve".to_string()]);
    }

    #[test]
    fn entity_id_registry_allocates_from_million() {
        let server = JavaServer::new(Arc::new(Stub {
            changes: Mutex::new(vec![]),
            snapshots: vec![],
        }));
        assert_eq!(server.entity_id_for("Steve"), FIRST_ENTITY_ID);
        assert_eq!(server.entity_id_for("steve"), FIRST_ENTITY_ID);
        assert_eq!(server.entity_id_for("Alex"), FIRST_ENTITY_ID + 1);
    }

    #[test]
    fn status_json_shape() {
        let hooks = Stub {
            changes: Mutex::new(vec![]),
            snapshots: vec![PlayerSnapshot::new("Steve", 0.0, 64.0, 0.0)],
        };
        let json = status_json(&hooks, Some(340));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["version"]["protocol"], 340);
        assert_eq!(value["players"]["online"], 1);
        let modern = status_json(&hooks, Some(775));
        let value: serde_json::Value = serde_json::from_str(&modern).unwrap();
        assert_eq!(value["version"]["name"], "26.1");
        assert_eq!(value["version"]["protocol"], 775);
        let default_ping = status_json(&hooks, None);
        let value: serde_json::Value = serde_json::from_str(&default_ping).unwrap();
        assert_eq!(value["version"]["name"], "26.1");
        assert_eq!(value["version"]["protocol"], 775);
    }

    #[test]
    fn tick_at_anchors_to_central_clock() {
        // The central clock (server-core main.rs) is the single source:
        // it calls SessionManager::tick() then JavaServer::tick_at(n)
        // with the same n, so Java gating (entities every 4, blocks
        // every 2, time every 20) follows Bedrock time. A stray local
        // tick() must not rewind it.
        let server = JavaServer::new(Arc::new(Stub {
            changes: Mutex::new(vec![]),
            snapshots: vec![],
        }));
        assert_eq!(server.current_tick(), 0);
        server.tick_at(42);
        assert_eq!(server.current_tick(), 42);
        server.tick();
        assert_eq!(server.current_tick(), 43);
        server.tick_at(10);
        assert_eq!(server.current_tick(), 43, "stale tick number must not rewind");
        server.tick_at(100);
        assert_eq!(server.current_tick(), 100);
        assert_eq!(server.session_count(), 0);
    }

    #[test]
    fn tick_block_changes_broadcasts_break_to_chunk_viewers() {
        // Bedrock -> Java block sync: a break enqueued through the hooks
        // (what `handle_block_dig` calls) is broadcast by the 10 Hz drain
        // to every playing session that already has the chunk.
        let hooks: Arc<dyn ServerHooks> = Arc::new(Stub {
            changes: Mutex::new(vec![]),
            snapshots: vec![],
        });
        let server = JavaServer::new(Arc::clone(&hooks));
        let id = server.add_session("127.0.0.1:1".parse().unwrap());
        {
            let mut sessions = server.sessions.lock();
            let session = sessions.get_mut(&id).unwrap();
            session.protocol = 775; // modern
            session.username = Some("Steve".to_string());
            session.playing = true;
            session.sent_chunks.insert((0, 0));
        }

        assert!(hooks.break_block(1, 64, 2));
        // Tick 2 is a multiple of BLOCK_CHANGES_INTERVAL_TICKS.
        server.tick_at(2);

        let out = {
            let mut sessions = server.sessions.lock();
            sessions.get_mut(&id).unwrap().take_out()
        };
        let change_id = profile::packet_id(775, "block_change").unwrap();
        let mut frames = Vec::new();
        let mut rest = out.as_slice();
        while let Some((packet_id, payload, consumed)) = wire::try_split_frame(rest) {
            frames.push((packet_id, payload));
            rest = &rest[consumed..];
        }
        assert!(
            frames.iter().any(|(packet_id, payload)| {
                *packet_id == change_id
                    && {
                        let mut r = wire::Reader::new(payload);
                        r.read_position() == Some((1, 64, 2)) && r.read_varint() == Some(0)
                    }
            }),
            "expected block_change(1, 64, 2, air), got {frames:?}"
        );

        // Drained: the next drain tick sends nothing new.
        server.tick_at(4);
        let out = {
            let mut sessions = server.sessions.lock();
            sessions.get_mut(&id).unwrap().take_out()
        };
        assert!(out.is_empty());
    }
}
