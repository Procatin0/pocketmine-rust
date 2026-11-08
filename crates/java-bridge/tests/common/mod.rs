//! Shared stub server for the wire/modern integration tests.
//!
//! Ports the PocketMine stubs of `wire_test.php` / `modern_test.php`:
//! a flat world with spawn (0.5, 5.62, 0.5), time 6000, max players 20,
//! one stone block at (0,4,0), chunk columns with bitmap 3 / 2 KiB of
//! data, and a registry/tags stub. Chunk *contents* are dummy bytes:
//! real columns belong to the `ModernChunk` agent (this crate only
//! frames them).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use java_bridge::hooks::{
    BlockChange, ChatOutcome, LegacyChunk, ModernChunkData, PlayerSnapshot, ServerHooks,
};
use java_bridge::server::JavaServer;
use java_bridge::wire;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Default)]
pub struct StubState {
    pub snapshots: Vec<PlayerSnapshot>,
    pub registrations: Vec<String>,
    pub quits: Vec<String>,
    pub inbox: Vec<String>,
    pub broadcasts: Vec<String>,
    pub blocks: HashMap<(i32, i32, i32), u32>,
    pub changes: Vec<BlockChange>,
}

pub struct StubHooks {
    pub state: Mutex<StubState>,
    pub view_distance: i32,
}

impl StubHooks {
    pub fn new(view_distance: i32) -> Self {
        let mut blocks = HashMap::new();
        blocks.insert((0, 4, 0), 2_150_698_529u32); // stone
        Self {
            state: Mutex::new(StubState {
                blocks,
                ..Default::default()
            }),
            view_distance,
        }
    }

    pub fn arc(self) -> Arc<StubHooks> {
        Arc::new(self)
    }
}

impl ServerHooks for StubHooks {
    fn view_distance(&self) -> i32 {
        self.view_distance
    }
    fn spawn(&self) -> (f64, f64, f64) {
        (0.5, 5.62, 0.5)
    }
    fn motd(&self) -> String {
        "JavaBridge test".to_string()
    }
    fn max_players(&self) -> i32 {
        20
    }
    fn world_time(&self) -> i64 {
        6000
    }
    fn is_name_available(&self, name: &str, _except: u64) -> bool {
        !self
            .state
            .lock()
            .unwrap()
            .snapshots
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(name))
    }
    fn snapshots(&self) -> Vec<PlayerSnapshot> {
        self.state.lock().unwrap().snapshots.clone()
    }
    fn join_api(&self, _id: u64, _peer: SocketAddr, snapshot: &PlayerSnapshot) {
        let mut state = self.state.lock().unwrap();
        state.registrations.push(snapshot.name.clone());
        state
            .snapshots
            .retain(|s| !s.name.eq_ignore_ascii_case(&snapshot.name));
        state.snapshots.push(snapshot.clone());
    }
    fn leave_api(&self, name: &str) {
        let mut state = self.state.lock().unwrap();
        state.quits.push(name.to_string());
        state
            .snapshots
            .retain(|s| !(s.is_virtual && s.name.eq_ignore_ascii_case(name)));
    }
    fn update_virtual_position(&self, name: &str, x: f64, y: f64, z: f64, yaw: f64, pitch: f64) {
        for snapshot in self.state.lock().unwrap().snapshots.iter_mut() {
            if snapshot.name.eq_ignore_ascii_case(name) {
                snapshot.x = x;
                snapshot.y = y;
                snapshot.z = z;
                snapshot.yaw = yaw;
                snapshot.pitch = pitch;
            }
        }
    }
    fn take_plugin_messages(&self, _name: &str) -> Vec<String> {
        std::mem::take(&mut self.state.lock().unwrap().inbox)
    }
    fn handle_chat(&self, _id: u64, name: &str, message: &str) -> ChatOutcome {
        let mut state = self.state.lock().unwrap();
        if !message.starts_with('/') {
            state.broadcasts.push(format!("<{name}> {message}"));
            return ChatOutcome::default();
        }
        let mut parts = message[1..].split_whitespace();
        match parts.next().unwrap_or("").to_lowercase().as_str() {
            "list" => {
                let names: Vec<String> = state.snapshots.iter().map(|s| s.name.clone()).collect();
                ChatOutcome::lines(vec![format!(
                    "Online ({}): {}",
                    names.len(),
                    names.join(", ")
                )])
            }
            "spawn" => ChatOutcome {
                lines: vec!["Teleportado al spawn del mundo Bedrock.".to_string()],
                teleport: Some((0.5, 5.62, 0.5, None, None)),
                gamemode: None,
            },
            "tp" => {
                let coords: Vec<f64> = parts.take(3).map(|p| p.parse().unwrap_or(0.0)).collect();
                if coords.len() == 3 {
                    ChatOutcome {
                        lines: vec![],
                        teleport: Some((coords[0], coords[1], coords[2], None, None)),
                        gamemode: None,
                    }
                } else {
                    ChatOutcome::default()
                }
            }
            "gamemode" => {
                let mode = match parts.next().unwrap_or("") {
                    "creative" | "1" | "c" => 1,
                    "survival" | "0" | "s" => 0,
                    _ => 0,
                };
                ChatOutcome {
                    lines: vec![],
                    teleport: None,
                    gamemode: Some(mode),
                }
            }
            other => {
                ChatOutcome::lines(vec![format!("Comando no disponible desde Java: /{other}")])
            }
        }
    }
    fn break_block(&self, x: i32, y: i32, z: i32) -> bool {
        let mut state = self.state.lock().unwrap();
        if state
            .blocks
            .get(&(x, y, z))
            .copied()
            .unwrap_or(3_690_217_760)
            == 3_690_217_760
        {
            return false;
        }
        state.blocks.insert((x, y, z), 3_690_217_760);
        true
    }
    fn place_block(&self, x: i32, y: i32, z: i32, network_id: u32) -> bool {
        let mut state = self.state.lock().unwrap();
        if state
            .blocks
            .get(&(x, y, z))
            .copied()
            .unwrap_or(3_690_217_760)
            != 3_690_217_760
        {
            return false;
        }
        state.blocks.insert((x, y, z), network_id);
        // Like PHP `placeBlock` -> `broadcastBlockChange`: the edit is
        // queued for the drain that forwards it to every Java client.
        state.changes.push(BlockChange {
            x,
            y,
            z,
            network_id: Some(network_id),
            kind: String::new(),
        });
        true
    }
    fn block_network_id(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        self.state
            .lock()
            .unwrap()
            .blocks
            .get(&(x, y, z))
            .copied()
            .or(Some(3_690_217_760))
    }
    fn legacy_chunk(&self, _cx: i32, _cz: i32) -> Option<LegacyChunk> {
        Some(LegacyChunk {
            bitmap: 3,
            data: vec![0u8; 2048],
        })
    }
    fn modern_chunk(&self, _protocol: i32, _cx: i32, _cz: i32) -> Option<ModernChunkData> {
        Some(ModernChunkData {
            data: vec![0u8; 1024],
            heightmaps: vec![0u8; 8],
            light: vec![0u8; 8],
        })
    }
    fn registry_payloads(&self, _protocol: i32) -> Vec<Vec<u8>> {
        [
            "minecraft:dimension_type",
            "minecraft:worldgen/biome",
            "minecraft:chat_type",
        ]
        .iter()
        .map(|id| {
            let mut payload = wire::string(id);
            payload.extend_from_slice(&wire::varint(0));
            payload
        })
        .collect()
    }
    fn tags_payload(&self, _protocol: i32) -> Vec<u8> {
        let mut payload = wire::varint(1);
        payload.extend_from_slice(&wire::identifier("minecraft:damage_type"));
        payload.extend_from_slice(&wire::varint(1));
        payload.extend_from_slice(&wire::identifier("minecraft:is_fire"));
        payload.extend_from_slice(&wire::varint(2));
        payload.extend_from_slice(&wire::varint(1));
        payload.extend_from_slice(&wire::varint(2));
        payload
    }
    fn drain_block_changes(&self) -> Vec<BlockChange> {
        std::mem::take(&mut self.state.lock().unwrap().changes)
    }
}

/// Start a `JavaServer` on an ephemeral port with a 20 ms tick loop.
pub async fn start_server(
    hooks: Arc<StubHooks>,
) -> (
    std::net::SocketAddr,
    Arc<JavaServer>,
    tokio::task::JoinHandle<()>,
) {
    let server = Arc::new(JavaServer::new(hooks));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let accept_server = Arc::clone(&server);
    let tick_server = Arc::clone(&server);
    let handle = tokio::spawn(async move {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(20));
            loop {
                interval.tick().await;
                tick_server.tick();
            }
        });
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let server = Arc::clone(&accept_server);
                    tokio::spawn(async move {
                        server.handle_stream(stream).await;
                    });
                }
                Err(_) => break,
            }
        }
    });
    (addr, server, handle)
}

pub struct TestClient {
    pub stream: TcpStream,
    pub buffer: Vec<u8>,
}

impl TestClient {
    pub async fn connect(addr: std::net::SocketAddr) -> Self {
        let stream = TcpStream::connect(addr).await.unwrap();
        Self {
            stream,
            buffer: Vec::new(),
        }
    }

    pub async fn send(&mut self, bytes: &[u8]) {
        self.stream.write_all(bytes).await.unwrap();
    }

    pub async fn send_frame(&mut self, id: i32, payload: &[u8]) {
        self.send(&wire::frame(id, payload)).await;
    }

    /// Read until `done` holds or the timeout elapses; returns all frames.
    pub async fn collect_until(
        &mut self,
        timeout: Duration,
        done: impl Fn(&[(i32, Vec<u8>)]) -> bool,
    ) -> Vec<(i32, Vec<u8>)> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut frames: Vec<(i32, Vec<u8>)> = Vec::new();
        loop {
            while let Some((id, payload, consumed)) = wire::try_split_frame(&self.buffer) {
                self.buffer.drain(..consumed);
                frames.push((id, payload));
            }
            if done(&frames) || tokio::time::Instant::now() >= deadline {
                return frames;
            }
            let mut chunk = [0u8; 65536];
            let remaining = deadline - tokio::time::Instant::now();
            match tokio::time::timeout(remaining, self.stream.read(&mut chunk)).await {
                Ok(Ok(0)) => return frames,
                Ok(Ok(n)) => self.buffer.extend_from_slice(&chunk[..n]),
                _ => return frames,
            }
        }
    }

    pub async fn collect(&mut self, timeout: Duration) -> Vec<(i32, Vec<u8>)> {
        self.collect_until(timeout, |_| false).await
    }
}

pub fn handshake(protocol: i32, next_state: i32, name: &str) -> Vec<u8> {
    let mut payload = wire::varint(protocol);
    payload.extend_from_slice(&wire::string("127.0.0.1"));
    payload.extend_from_slice(&25565u16.to_be_bytes());
    payload.extend_from_slice(&wire::varint(next_state));
    let mut out = wire::frame(0x00, &payload);
    if next_state == 2 {
        out.extend_from_slice(&wire::frame(0x00, &wire::string(name)));
    }
    out
}
