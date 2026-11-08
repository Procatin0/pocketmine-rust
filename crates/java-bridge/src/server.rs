//! `JavaServer`: the TCP listener (port 25565 by default) and the periodic
//! phases `JavaServer::tick` drives in PHP.
//!
//! PHP runs everything on the server tick from non-blocking sockets;
//! Rust runs a Tokio `TcpListener`: one task per connection feeds bytes to
//! its [`JavaSession`](crate::session::JavaSession), and a tick task every
//! 50 ms runs the same gating as PHP `tick()`:
//! - per-session `tick()` (keep-alive, position push, plugin messages,
//!   gamemode/teleport sync, chunk streaming under a shared deadline),
//! - `tick_tab()` every tick (memory-only diff, no host call),
//! - `tick_time_sync()` every 20 ticks or on a jump,
//! - `tick_entities()` every 4 ticks (5 Hz, one snapshots read),
//! - `tick_block_changes()` every 2 ticks (10 Hz drain).
//!
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

use crate::hooks::{PlayerSnapshot, ServerHooks};
use crate::profile;
use crate::session::{EntityTarget, JavaSession, TabTarget};
use crate::wire;

pub const DEFAULT_PORT: u16 = 25565;
pub const TICK_BUDGET: Duration = Duration::from_millis(10);
pub const ENTITY_SYNC_INTERVAL_TICKS: u64 = 4;
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
        None => (wire::VERSION_NAME.to_string(), wire::PROTOCOL_VERSION),
    };
    let sample: Vec<serde_json::Value> = hooks
        .snapshots()
        .iter()
        .take(10)
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "id": wire::uuid_string(&wire::offline_uuid(&s.name)),
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
pub fn tab_targets(hooks: &dyn ServerHooks, extra: Option<&[TabTarget]>) -> Vec<TabTarget> {
    let mut targets: HashMap<String, TabTarget> = HashMap::new();
    for snapshot in hooks.snapshots() {
        targets.insert(
            snapshot.name.to_lowercase(),
            TabTarget {
                name: snapshot.name.clone(),
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
/// virtual ones with the same name (same rule as `sync_virtuals_locked`).
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
            name: snapshot.name.clone(),
            x: snapshot.x,
            y: snapshot.y,
            z: snapshot.z,
            yaw: wire::bedrock_yaw_to_java(snapshot.yaw),
            pitch: wire::bedrock_pitch_to_java(snapshot.pitch),
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

    /// One bridge tick: per-session work under a shared deadline, then
    /// TAB, then (budget permitting) time / entities / block changes.
    /// Mirrors `JavaServer::tick` gating, minus socket accepts (the Tokio
    /// loop owns those).
    pub fn tick(&self) {
        let tick_count = self.tick_count.fetch_add(1, Ordering::SeqCst) + 1;
        let started = Instant::now();
        let deadline = started + TICK_BUDGET;
        {
            let mut sessions = self.sessions.lock();
            for session in sessions.values_mut() {
                session.tick(self.hooks.as_ref(), Some(deadline));
            }
        }
        self.tick_tab();
        if Instant::now() <= deadline {
            self.tick_time_sync();
            if tick_count.is_multiple_of(ENTITY_SYNC_INTERVAL_TICKS) {
                self.tick_entities();
            }
            if tick_count.is_multiple_of(BLOCK_CHANGES_INTERVAL_TICKS) {
                self.tick_block_changes();
            }
        }
        let elapsed = started.elapsed();
        if elapsed > Duration::from_millis(15) {
            let mut last = self.slow_tick_log_at.lock();
            let quiet = last
                .map(|t| t.elapsed() > Duration::from_secs(10))
                .unwrap_or(true);
            if quiet {
                *last = Some(Instant::now());
                self.hooks.log(format!(
                    "Tick del puente Java lento: {} ms",
                    elapsed.as_millis()
                ));
            }
        }
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
        for change in self.hooks.drain_block_changes() {
            let key = (change.x, change.y, change.z);
            let known = self
                .last_known_blocks
                .lock()
                .get(&key)
                .copied()
                .unwrap_or(None);
            if change.network_id.is_some() && known == change.network_id {
                continue; // echo of our own place/break
            }
            self.broadcast_block_change(
                change.x,
                change.y,
                change.z,
                &change.kind,
                change.network_id,
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
        let network_id = network_id.or_else(|| self.hooks.block_network_id(x, y, z));
        self.note_known_block(x, y, z, network_id);
        let (cx, cz) = (
            (x as f64 / 16.0).floor() as i32,
            (z as f64 / 16.0).floor() as i32,
        );
        for session in self.sessions.lock().values_mut() {
            if session.is_playing() && session.has_sent_chunk(cx, cz) {
                session.send_block_change(self.hooks.as_ref(), x, y, z, kind, network_id);
            }
        }
    }

    fn note_known_block(&self, x: i32, y: i32, z: i32, network_id: Option<u32>) {
        let mut known = self.last_known_blocks.lock();
        known.insert((x, y, z), network_id);
        if known.len() > 8192 {
            if let Some(first) = known.keys().next().copied() {
                known.remove(&first);
            }
        }
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
                    // Drain periodic output queued by the tick task
                    // (chunks, TAB, entities) without a client write.
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

    /// Production entry: bind 25565 (or `addr`), accept + tick forever.
    /// Tests drive `tick()` directly and use `handle_stream` instead.
    pub async fn run_arc(self: Arc<Self>, addr: &str) {
        let listener = TcpListener::bind(addr).await.expect("bind Java listener");
        let tick_server = Arc::clone(&self);
        let tick_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            loop {
                interval.tick().await;
                tick_server.tick();
            }
        });
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
        tick_task.abort();
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
    use crate::hooks::ServerHooks;

    struct Stub {
        snapshots: Vec<PlayerSnapshot>,
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
    }

    #[test]
    fn entity_targets_skip_self_and_chunk_gate() {
        let hooks = Stub {
            snapshots: vec![
                PlayerSnapshot::new("Steve", 0.5, 5.0, 0.5),
                PlayerSnapshot {
                    name: "Alex".to_string(),
                    x: 100.0,
                    y: 64.0,
                    z: 100.0,
                    yaw: 90.0,
                    pitch: 0.0,
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
        assert_eq!(targets[0].name, "Alex");
        // Bedrock yaw 90 (east) -> Java -90.
        assert!((targets[0].yaw - (-90.0)).abs() < 0.001);
    }

    #[test]
    fn entity_id_registry_allocates_from_million() {
        let server = JavaServer::new(Arc::new(Stub { snapshots: vec![] }));
        assert_eq!(server.entity_id_for("Steve"), FIRST_ENTITY_ID);
        assert_eq!(server.entity_id_for("steve"), FIRST_ENTITY_ID);
        assert_eq!(server.entity_id_for("Alex"), FIRST_ENTITY_ID + 1);
    }

    #[test]
    fn status_json_shape() {
        let hooks = Stub {
            snapshots: vec![PlayerSnapshot::new("Steve", 0.0, 64.0, 0.0)],
        };
        let json = status_json(&hooks, Some(340));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["version"]["protocol"], 340);
        assert_eq!(value["players"]["online"], 1);
        let modern = status_json(&hooks, Some(775));
        let value: serde_json::Value = serde_json::from_str(&modern).unwrap();
        assert_eq!(value["version"]["name"], "26.1");
    }
}
