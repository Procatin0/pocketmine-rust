//! `JavaSession`: one Java Edition TCP connection.
//!
//! Port of `JavaClient.php`, structured as the mirror of
//! `server-core::session::PlayerSession`: same field roles (identity,
//! position, gamemode, sent chunks, viewers/seen entities, tick counters,
//! pending output) plus the Java state machine
//! handshake -> status | login -> configuration -> play (legacy 340 goes
//! login -> play directly; modern 769-775 go through configuration).
//!
//! The session owns no socket: callers feed bytes with [`JavaSession::poll`]
//! (or [`JavaSession::poll_bytes`]) and drain [`JavaSession::take_out`].
//! `server.rs` does that over Tokio; unit tests do it in memory. Periodic
//! work mirrors `JavaClient::tick` + `JavaServer::tick*` gating:
//! keep-alive every 10 s, `maybeUpdatePosition` throttled to ~2 ticks,
//! `deliverPluginMessages` every 5 ticks, gamemode/teleport sync staggered
//! every 10 ticks, chunk streaming under a time budget.
//!
//! Virtual-player visibility mirrors `SessionManager::sync_virtuals_locked`
//! in reverse: there, Bedrock viewers get spawn/move/despawn for registry
//! virtuals; here, a Java session gets `spawn_entity` / move / destroy for
//! every snapshot in [`crate::hooks::ServerHooks::snapshots`] that is not
//! itself and whose chunk has been sent (the `hasReceivedChunk` gate).

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::hooks::{ChatOutcome, PlayerSnapshot, ServerHooks};
use crate::profile;
use crate::wire;

pub const KEEP_ALIVE_SECONDS: f64 = 10.0;
pub const TIMEOUT_SECONDS: f64 = 30.0;
/// Per-tick chunk budget (`CHUNK_BUDGET_SECONDS` in PHP).
pub const CHUNK_BUDGET: Duration = Duration::from_millis(12);
/// Columns per batch (`CHUNKS_PER_BATCH`).
pub const CHUNKS_PER_BATCH: usize = 64;
/// Pause streaming when more than this is queued (`CHUNK_BACKPRESSURE`).
pub const CHUNK_BACKPRESSURE: usize = 262_144;
pub const WORLD_NAME: &str = "minecraft:overworld";
pub const SEA_LEVEL: i32 = 63;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Handshake,
    Status,
    Login,
    Config,
    Play,
    Closed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SeenEntity {
    pub name: String,
    pub entity: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Java yaw/pitch (wire convention).
    pub yaw: f64,
    pub pitch: f64,
    pub gamemode: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TabEntry {
    pub name: String,
    pub gamemode: i32,
    pub ping: i32,
}

/// Entity target computed by the server (`entityTargetsFor`), Java yaw/pitch.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityTarget {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f64,
    pub pitch: f64,
    pub gamemode: i32,
    pub ping: i32,
}

/// TAB target (`tabTargets`): every online player, no chunk gate.
#[derive(Debug, Clone, PartialEq)]
pub struct TabTarget {
    pub name: String,
    pub gamemode: i32,
    pub ping: i32,
}

pub struct JavaSession {
    pub session_id: u64,
    pub peer: SocketAddr,
    pub state: SessionState,
    /// Wire protocol (340 legacy, 769-775 modern).
    pub protocol: i32,
    pub username: Option<String>,
    /// Canonical Bedrock position (feet); legacy wire adds +64 itself.
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Java yaw/pitch (wire convention).
    pub yaw: f64,
    pub pitch: f64,
    pub gamemode: i32,
    pub health: f32,
    /// Chunks already sent (`sentChunks`).
    pub sent_chunks: HashSet<(i32, i32)>,
    pub pending_chunks: VecDeque<(i32, i32)>,
    pub pending_attempts: HashMap<(i32, i32), u32>,
    pub last_chunk: Option<(i32, i32)>,
    pub chunks_per_tick: Option<f32>,
    pub view_announced: bool,
    pub teleport_id: i32,
    pub keep_alive_id: i64,
    pub last_keep_alive: Instant,
    pub last_seen: Instant,
    pub tick_count: u64,
    pub last_pos_update_tick: u64,
    /// Last position shared with the server `[x, y, z, yaw, pitch]` (Java).
    pub last_sent_pos: Option<[f64; 5]>,
    pub own_entity_id: Option<i32>,
    /// True once `joinApi` ran for this TCP session (once per session).
    pub api_joined: bool,
    /// Entities this client has spawned (`seenEntities`).
    pub seen_entities: HashMap<String, SeenEntity>,
    /// Players announced in this client's TAB (`tabListed`).
    pub tab_listed: HashMap<String, TabEntry>,
    in_buf: Vec<u8>,
    out_buf: Vec<u8>,
    pub closed: bool,
    pub playing: bool,
    spawn_chunks_at: Instant,
    spawn_chunks_logged: bool,
}

impl JavaSession {
    pub fn new(session_id: u64, peer: SocketAddr) -> Self {
        let now = Instant::now();
        Self {
            session_id,
            peer,
            state: SessionState::Handshake,
            protocol: 0,
            username: None,
            x: 0.5,
            y: 4.0,
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
            gamemode: 0,
            health: 20.0,
            sent_chunks: HashSet::new(),
            pending_chunks: VecDeque::new(),
            pending_attempts: HashMap::new(),
            last_chunk: None,
            chunks_per_tick: None,
            view_announced: false,
            teleport_id: 1,
            keep_alive_id: 0,
            last_keep_alive: now,
            last_seen: now,
            tick_count: 0,
            last_pos_update_tick: u64::MAX,
            last_sent_pos: None,
            own_entity_id: None,
            api_joined: false,
            seen_entities: HashMap::new(),
            tab_listed: HashMap::new(),
            in_buf: Vec::new(),
            out_buf: Vec::new(),
            closed: false,
            playing: false,
            spawn_chunks_at: now,
            spawn_chunks_logged: false,
        }
    }

    pub fn is_modern(&self) -> bool {
        profile::is_modern(self.protocol)
    }

    pub fn is_playing(&self) -> bool {
        self.playing && !self.closed
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn has_sent_chunk(&self, cx: i32, cz: i32) -> bool {
        self.sent_chunks.contains(&(cx, cz))
    }

    pub fn pending_bytes(&self) -> usize {
        self.out_buf.len()
    }

    pub fn take_out(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.out_buf)
    }

    /// Feed raw socket bytes; returns false when the connection must close.
    pub fn poll_bytes(&mut self, hooks: &dyn ServerHooks, data: &[u8]) -> bool {
        if self.closed {
            return false;
        }
        if !data.is_empty() {
            self.in_buf.extend_from_slice(data);
            self.last_seen = Instant::now();
        }
        while !self.closed {
            let Some((packet_id, payload, consumed)) = wire::try_split_frame(&self.in_buf) else {
                break;
            };
            self.in_buf.drain(..consumed);
            self.handle_packet(hooks, packet_id, &payload);
        }
        !self.closed
    }

    /// Periodic work (`JavaClient::tick`): timeout, keep-alive, position
    /// push, plugin messages, gamemode/teleport sync, chunk streaming.
    pub fn tick(&mut self, hooks: &dyn ServerHooks, deadline: Option<Instant>) {
        if self.closed {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.last_seen).as_secs_f64() > TIMEOUT_SECONDS {
            self.kick("Tiempo de espera agotado");
            return;
        }
        if self.playing {
            self.tick_count += 1;
            self.keep_alive(now);
            self.ensure_registered(hooks);
            self.maybe_update_position(hooks);
            if self.tick_count % 5 == 1 {
                self.deliver_plugin_messages(hooks);
            }
            if self.tick_count % 10 == 2 {
                self.sync_gamemode(hooks);
            }
            if self.tick_count % 10 == 7 {
                self.sync_teleport(hooks);
            }
            self.stream_chunks(hooks, deadline);
        }
    }

    // ------------------------------------------------------------------
    // Packet dispatch
    // ------------------------------------------------------------------

    fn handle_packet(&mut self, hooks: &dyn ServerHooks, packet_id: i32, payload: &[u8]) {
        match self.state {
            SessionState::Handshake => self.handle_handshake(hooks, packet_id, payload),
            SessionState::Status => self.handle_status(hooks, packet_id, payload),
            SessionState::Login => {
                if self.is_modern() {
                    self.handle_modern_login(hooks, packet_id, payload)
                } else {
                    self.handle_login(hooks, packet_id, payload)
                }
            }
            SessionState::Config => self.handle_configuration(hooks, packet_id),
            SessionState::Play => {
                if self.is_modern() {
                    self.handle_modern_play(hooks, packet_id, payload)
                } else {
                    self.handle_play(hooks, packet_id, payload)
                }
            }
            SessionState::Closed => {}
        }
    }

    fn handle_handshake(&mut self, hooks: &dyn ServerHooks, packet_id: i32, payload: &[u8]) {
        if packet_id != wire::C2S_HANDSHAKE {
            self.kick("Se esperaba un handshake");
            return;
        }
        let mut r = wire::Reader::new(payload);
        let (Some(protocol), Some(address), Some(port), Some(next)) = (
            r.read_varint(),
            r.read_string(),
            r.read_u16(),
            r.read_varint(),
        ) else {
            self.kick("Handshake inválido");
            return;
        };
        self.protocol = protocol;
        hooks.log(format!(
            "Handshake de {}: protocolo {protocol}, destino {address}:{port}, estado {next}",
            self.peer
        ));
        if next == 1 {
            self.state = SessionState::Status;
            return;
        }
        if next != 2 {
            self.kick("Estado de handshake desconocido");
            return;
        }
        if profile::is_modern(protocol) || protocol == wire::PROTOCOL_VERSION {
            self.state = SessionState::Login;
            return;
        }
        let supported = format!(
            "Versiones soportadas: Java {} (protocolo {}) y {}. Tu cliente usa el protocolo {protocol}.",
            wire::VERSION_NAME,
            wire::PROTOCOL_VERSION,
            profile::VERSION_NAMES.join(", ")
        );
        self.send(
            wire::S2C_LOGIN_DISCONNECT,
            &wire::string(&wire::chat_json(&supported)),
        );
        self.closed = true;
    }

    fn handle_status(&mut self, hooks: &dyn ServerHooks, packet_id: i32, payload: &[u8]) {
        match packet_id {
            wire::C2S_STATUS_REQUEST => {
                let json = crate::server::status_json(hooks, Some(self.protocol));
                self.send(wire::S2C_STATUS_RESPONSE, &wire::string(&json));
            }
            wire::C2S_STATUS_PING => {
                let mut r = wire::Reader::new(payload);
                let time = r.read_i64().unwrap_or(0);
                self.send(wire::S2C_STATUS_PONG, &wire::long(time));
                self.closed = true;
            }
            _ => {}
        }
    }

    fn valid_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 16
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    /// Shared legacy/modern login-start validation. Returns the name when
    /// accepted (and records it); sends login disconnect + closes otherwise.
    fn accept_login_name(&mut self, hooks: &dyn ServerHooks, payload: &[u8]) -> Option<String> {
        let mut r = wire::Reader::new(payload);
        let name = r.read_string().unwrap_or_default().trim().to_string();
        if !Self::valid_name(&name) {
            self.send(
                wire::S2C_LOGIN_DISCONNECT,
                &wire::string(&wire::chat_json("Nombre de jugador inválido.")),
            );
            self.closed = true;
            return None;
        }
        if !hooks.is_name_available(&name, self.session_id) {
            self.send(
                wire::S2C_LOGIN_DISCONNECT,
                &wire::string(&wire::chat_json("Ese nombre ya está conectado.")),
            );
            self.closed = true;
            return None;
        }
        self.username = Some(name.clone());
        Some(name)
    }

    fn handle_login(&mut self, hooks: &dyn ServerHooks, packet_id: i32, payload: &[u8]) {
        if packet_id != wire::C2S_LOGIN_START {
            self.kick("Se esperaba login_start");
            return;
        }
        let Some(name) = self.accept_login_name(hooks, payload) else {
            return;
        };
        let uuid = wire::offline_uuid(&name);
        let mut payload = wire::string(&wire::uuid_string(&uuid));
        payload.extend_from_slice(&wire::string(&name));
        self.send(wire::S2C_LOGIN_SUCCESS, &payload);
        self.enter_play(hooks);
    }

    fn handle_modern_login(&mut self, hooks: &dyn ServerHooks, packet_id: i32, payload: &[u8]) {
        if packet_id == profile::packet_id(self.protocol, "login_acknowledged").unwrap_or(-1) {
            self.begin_configuration(hooks);
            return;
        }
        if packet_id != wire::C2S_LOGIN_START {
            self.kick("Se esperaba login_start");
            return;
        }
        let Some(name) = self.accept_login_name(hooks, payload) else {
            return;
        };
        let mut payload = wire::offline_uuid(&name).to_vec();
        payload.extend_from_slice(&wire::string(&name));
        payload.extend_from_slice(&wire::varint(0));
        let id = profile::packet_id(self.protocol, "login_success").unwrap_or(2);
        self.send(id, &payload);
    }

    fn begin_configuration(&mut self, hooks: &dyn ServerHooks) {
        self.state = SessionState::Config;
        let registry_id = profile::packet_id(self.protocol, "registry_data").unwrap_or(7);
        for payload in hooks.registry_payloads(self.protocol) {
            self.send(registry_id, &payload);
        }
        let feature_flags = profile::packet_id(self.protocol, "feature_flags").unwrap_or(12);
        let mut flags = wire::varint(1);
        flags.extend_from_slice(&wire::string("minecraft:vanilla"));
        self.send(feature_flags, &flags);
        // Tags (0x0d) must exist before finish_configuration or the client
        // aborts with "Missing tag" building item components.
        let tags_id = profile::packet_id(self.protocol, "update_tags").unwrap_or(13);
        self.send(tags_id, &hooks.tags_payload(self.protocol));
        // Brand (0x01): custom_payload clientbound, channel + brand string.
        let brand_id = profile::packet_id(self.protocol, "config_brand").unwrap_or(1);
        self.send(brand_id, &profile::brand_payload());
        let finish = profile::packet_id(self.protocol, "finish_configuration").unwrap_or(3);
        self.send(finish, &[]);
    }

    fn handle_configuration(&mut self, hooks: &dyn ServerHooks, packet_id: i32) {
        if packet_id == profile::packet_id(self.protocol, "config_finish").unwrap_or(-1) {
            self.enter_play_modern(hooks);
        }
    }

    fn snapshot(&self) -> PlayerSnapshot {
        PlayerSnapshot {
            name: self.username.clone().unwrap_or_default(),
            x: self.x,
            y: self.y,
            z: self.z,
            yaw: wire::java_yaw_to_bedrock(self.yaw),
            pitch: self.pitch,
            gamemode: self.gamemode,
            ping: 0,
            is_virtual: true,
        }
    }

    fn load_or_spawn(&mut self, hooks: &dyn ServerHooks) {
        let spawn = hooks.spawn();
        let loaded = self
            .username
            .as_deref()
            .and_then(|name| hooks.load_player_data(name));
        self.x = loaded.as_ref().map(|d| d.x).unwrap_or(spawn.0);
        self.y = loaded.as_ref().map(|d| d.y).unwrap_or(spawn.1);
        self.z = loaded.as_ref().map(|d| d.z).unwrap_or(spawn.2);
        if let Some(d) = &loaded {
            self.yaw = wire::bedrock_yaw_to_java(d.yaw);
            self.pitch = wire::bedrock_pitch_to_java(d.pitch);
            self.health = d.health;
            self.gamemode = d.gamemode.clamp(0, 3);
        } else {
            self.gamemode = hooks.default_gamemode().clamp(0, 3);
        }
    }

    fn enter_play(&mut self, hooks: &dyn ServerHooks) {
        self.state = SessionState::Play;
        self.playing = true;
        self.load_or_spawn(hooks);
        let spawn = hooks.spawn();
        let gamemode = self.gamemode;
        let health = self.health;
        let difficulty = hooks.difficulty();
        let max_players = hooks.max_players().clamp(1, 255);
        let own_id = self.entity_id(hooks);
        let mut join = Vec::new();
        join.extend_from_slice(&(own_id as u32).to_be_bytes());
        join.extend_from_slice(&wire::ubyte(gamemode));
        join.extend_from_slice(&0u32.to_be_bytes());
        join.extend_from_slice(&wire::ubyte(difficulty));
        join.extend_from_slice(&wire::ubyte(max_players));
        join.extend_from_slice(&wire::string(&hooks.level_type()));
        join.extend_from_slice(&wire::boolean(false));
        self.send(wire::S2C_JOIN_GAME, &join);
        self.send(wire::S2C_DIFFICULTY, &wire::ubyte(difficulty));
        self.send(
            wire::S2C_SPAWN_POSITION,
            &wire::position(spawn.0 as i32, spawn.1 as i32 + 64, spawn.2 as i32),
        );
        self.send(wire::S2C_ABILITIES, &abilities_payload(gamemode));
        let mut health_payload = wire::float(health);
        health_payload.extend_from_slice(&wire::varint(health.round() as i32));
        health_payload.extend_from_slice(&wire::float(5.0));
        self.send(wire::S2C_UPDATE_HEALTH, &health_payload);
        self.send_time(hooks.world_time(), hooks.world_time_running());
        self.send_position(true);
        self.join_api(hooks);
        let targets = crate::server::tab_targets(hooks, None);
        self.sync_tab(&targets);
        self.send_chat(&format!(
            "Conectado a PocketMine-Rust. {} jugador(es) online.",
            crate::server::online_count(hooks)
        ));
        self.reset_chunk_streaming();
    }

    fn enter_play_modern(&mut self, hooks: &dyn ServerHooks) {
        self.state = SessionState::Play;
        self.playing = true;
        self.load_or_spawn(hooks);
        let gamemode = self.gamemode;
        let view_distance = hooks.view_distance().clamp(2, 32);
        let max_players = hooks.max_players().clamp(1, 255);
        let own_id = self.entity_id(hooks);
        let mut join = Vec::new();
        join.extend_from_slice(&(own_id as u32).to_be_bytes());
        join.extend_from_slice(&wire::boolean(false));
        join.extend_from_slice(&wire::varint(1));
        join.extend_from_slice(&wire::string(WORLD_NAME));
        join.extend_from_slice(&wire::varint(max_players));
        join.extend_from_slice(&wire::varint(view_distance));
        join.extend_from_slice(&wire::varint(view_distance));
        join.extend_from_slice(&wire::boolean(false));
        join.extend_from_slice(&wire::boolean(true));
        join.extend_from_slice(&wire::boolean(false));
        // SpawnInfo
        join.extend_from_slice(&wire::varint(0));
        join.extend_from_slice(&wire::string(WORLD_NAME));
        join.extend_from_slice(&wire::long(0));
        join.push(gamemode as u8);
        join.push(0xFF);
        join.extend_from_slice(&wire::boolean(false));
        join.extend_from_slice(&wire::boolean(true));
        join.extend_from_slice(&wire::boolean(false));
        join.extend_from_slice(&wire::varint(0));
        join.extend_from_slice(&wire::varint(SEA_LEVEL));
        join.extend_from_slice(&wire::boolean(false));
        let id = profile::packet_id(self.protocol, "join_game").unwrap_or(44);
        self.send(id, &join);
        self.send_time(hooks.world_time(), hooks.world_time_running());
        self.send_position(true);
        let gsc = profile::packet_id(self.protocol, "game_state_change").unwrap_or(35);
        self.send(gsc, &[13u8, 0, 0, 0, 0]);
        self.join_api(hooks);
        let targets = crate::server::tab_targets(hooks, None);
        self.sync_tab(&targets);
        self.send_chat(&format!(
            "Conectado a PocketMine-Rust. {} jugador(es) online.",
            crate::server::online_count(hooks)
        ));
        self.reset_chunk_streaming();
    }

    fn join_api(&mut self, hooks: &dyn ServerHooks) {
        if self.api_joined {
            return;
        }
        let snapshot = self.snapshot();
        hooks.join_api(self.session_id, self.peer, &snapshot);
        self.api_joined = true;
    }

    pub fn ensure_registered(&mut self, hooks: &dyn ServerHooks) {
        if self.username.is_none() || !self.is_playing() {
            return;
        }
        if !self.api_joined {
            self.join_api(hooks);
        }
    }

    // ------------------------------------------------------------------
    // Play packets
    // ------------------------------------------------------------------

    fn apply_position_look(&mut self, hooks: &dyn ServerHooks, payload: &[u8], legacy: bool) {
        let mut r = wire::Reader::new(payload);
        let (Some(x), Some(y), Some(z)) = (r.read_f64(), r.read_f64(), r.read_f64()) else {
            return;
        };
        self.x = x;
        self.y = if legacy { y - 64.0 } else { y };
        self.z = z;
        self.request_chunks(hooks);
    }

    fn handle_play(&mut self, hooks: &dyn ServerHooks, packet_id: i32, payload: &[u8]) {
        match packet_id {
            wire::C2S_KEEP_ALIVE
            | wire::C2S_TELEPORT_CONFIRM
            | wire::C2S_FLYING
            | wire::C2S_ARM_ANIMATION => {}
            wire::C2S_CHAT => {
                let mut r = wire::Reader::new(payload);
                if let Some(message) = r.read_string() {
                    if !message.is_empty() {
                        self.route_chat(hooks, &message);
                    }
                }
            }
            wire::C2S_POSITION => self.apply_position_look(hooks, payload, true),
            wire::C2S_LOOK => {
                let mut r = wire::Reader::new(payload);
                if let (Some(yaw), Some(pitch)) = (r.read_f32(), r.read_f32()) {
                    self.yaw = yaw as f64;
                    self.pitch = pitch as f64;
                    self.request_chunks(hooks);
                }
            }
            wire::C2S_POSITION_LOOK => {
                let mut r = wire::Reader::new(payload);
                let pos = (r.read_f64(), r.read_f64(), r.read_f64());
                let rot = (r.read_f32(), r.read_f32());
                if let (Some(x), Some(y), Some(z)) = pos {
                    if let (Some(yaw), Some(pitch)) = rot {
                        self.x = x;
                        self.y = y - 64.0;
                        self.z = z;
                        self.yaw = yaw as f64;
                        self.pitch = pitch as f64;
                        self.request_chunks(hooks);
                    }
                }
            }
            wire::C2S_BLOCK_DIG => self.handle_block_dig(hooks, payload, true),
            wire::C2S_BLOCK_PLACE => self.handle_block_place(hooks, payload, true),
            wire::C2S_SETTINGS => {
                let mut r = wire::Reader::new(payload);
                let locale = r.read_string().unwrap_or_default();
                hooks.log(format!(
                    "Cliente Java {} configurado (locale {locale})",
                    self.peer
                ));
            }
            wire::C2S_CLIENT_COMMAND => {
                let mut r = wire::Reader::new(payload);
                if r.read_varint() == Some(0) {
                    self.teleport(hooks, self.x, self.y, self.z, None, None);
                }
            }
            _ => {}
        }
    }

    fn pid(&self, key: &str) -> i32 {
        profile::packet_id(self.protocol, key).unwrap_or(-1)
    }

    fn handle_modern_play(&mut self, hooks: &dyn ServerHooks, packet_id: i32, payload: &[u8]) {
        if packet_id == self.pid("chat_message") {
            let mut r = wire::Reader::new(payload);
            if let Some(message) = r.read_string() {
                if !message.is_empty() {
                    self.route_chat(hooks, &message);
                }
            }
        } else if packet_id == self.pid("chat_command") {
            let mut r = wire::Reader::new(payload);
            if let Some(command) = r.read_string() {
                if !command.is_empty() {
                    self.route_chat(hooks, &format!("/{command}"));
                }
            }
        } else if packet_id == self.pid("sb_position") {
            self.apply_position_look(hooks, payload, false);
        } else if packet_id == self.pid("sb_position_look") {
            let mut r = wire::Reader::new(payload);
            let pos = (r.read_f64(), r.read_f64(), r.read_f64());
            let rot = (r.read_f32(), r.read_f32());
            if let (Some(x), Some(y), Some(z)) = pos {
                if let (Some(yaw), Some(pitch)) = rot {
                    self.x = x;
                    self.y = y;
                    self.z = z;
                    self.yaw = yaw as f64;
                    self.pitch = pitch as f64;
                    self.request_chunks(hooks);
                }
            }
        } else if packet_id == self.pid("sb_look") {
            let mut r = wire::Reader::new(payload);
            if let (Some(yaw), Some(pitch)) = (r.read_f32(), r.read_f32()) {
                self.yaw = yaw as f64;
                self.pitch = pitch as f64;
                self.request_chunks(hooks);
            }
        } else if packet_id == self.pid("chunk_batch_received") {
            let mut r = wire::Reader::new(payload);
            if let Some(rate) = r.read_f32() {
                if rate > 0.0 {
                    self.chunks_per_tick = Some(rate);
                }
            }
        } else if packet_id == self.pid("client_command") {
            let mut r = wire::Reader::new(payload);
            if r.read_varint() == Some(0) {
                let spawn = hooks.spawn();
                self.teleport(hooks, spawn.0, spawn.1, spawn.2, None, None);
            }
        } else if packet_id == self.pid("block_dig") {
            self.handle_block_dig(hooks, payload, false);
        } else if packet_id == self.pid("block_place") {
            self.handle_block_place(hooks, payload, false);
        }
    }

    fn route_chat(&mut self, hooks: &dyn ServerHooks, message: &str) {
        let message = message.trim();
        if message.is_empty() {
            return;
        }
        let name = self.username.clone().unwrap_or_default();
        let outcome: ChatOutcome = hooks.handle_chat(self.session_id, &name, message);
        if let Some((x, y, z, yaw, pitch)) = outcome.teleport {
            self.teleport(hooks, x, y, z, yaw, pitch);
        }
        if let Some(gamemode) = outcome.gamemode {
            self.set_gamemode(gamemode);
        }
        for line in outcome.lines {
            if !line.is_empty() {
                self.send_chat(&line);
            }
        }
    }

    fn handle_block_dig(&mut self, hooks: &dyn ServerHooks, payload: &[u8], legacy: bool) {
        let mut r = wire::Reader::new(payload);
        let (Some(status), Some((x, y, z))) = (r.read_varint(), r.read_position()) else {
            return;
        };
        if status != 2 {
            return;
        }
        let canonical_y = if legacy { y - 64 } else { y };
        if (x as f64 + 0.5 - self.x).abs() > 6.0
            || (canonical_y as f64 + 0.5 - self.y).abs() > 6.0
            || (z as f64 + 0.5 - self.z).abs() > 6.0
        {
            return;
        }
        if hooks.break_block(x, canonical_y, z) {
            self.send_block_change(hooks, x, canonical_y, z, "air", None);
        } else {
            self.send_block_change(hooks, x, canonical_y, z, "", None);
        }
    }

    fn handle_block_place(&mut self, hooks: &dyn ServerHooks, payload: &[u8], legacy: bool) {
        if self.gamemode != 1 {
            self.send_chat(
                "Colocar bloques desde Java solo está disponible en modo creativo por ahora (sin inventario). Usa /gamemode creative.",
            );
            return;
        }
        let mut r = wire::Reader::new(payload);
        let (position, face, block_id) = if !legacy {
            let (Some(_hand), Some(pos), Some(face)) =
                (r.read_varint(), r.read_position(), r.read_varint())
            else {
                return;
            };
            (pos, face, hooks.stone_network_id())
        } else {
            let Some(pos) = r.read_position() else { return };
            let Some(&face_byte) = payload.get(r.pos) else {
                return;
            };
            r.pos += 1;
            let Some(item_id) = r.read_u16() else { return };
            let item_id = item_id as i16 as i32;
            if item_id <= 0 {
                self.send_chat("No tienes ningún bloque en la mano para colocar.");
                return;
            }
            (pos, face_byte as i32, item_id as u32)
        };
        let offsets = [
            (0, -1, 0),
            (0, 1, 0),
            (0, 0, -1),
            (0, 0, 1),
            (-1, 0, 0),
            (1, 0, 0),
        ];
        let Some(offset) = offsets.get(face as usize) else {
            return;
        };
        let (tx, mut ty, tz) = (
            position.0 + offset.0,
            position.1 + offset.1,
            position.2 + offset.2,
        );
        if legacy {
            ty -= 64;
        }
        if !hooks.place_block(tx, ty, tz, block_id) {
            self.send_block_change(hooks, tx, ty, tz, "", None);
        }
    }

    // ------------------------------------------------------------------
    // Position / teleport / gamemode / time
    // ------------------------------------------------------------------

    fn maybe_update_position(&mut self, hooks: &dyn ServerHooks) {
        let current = [self.x, self.y, self.z, self.yaw, self.pitch];
        let Some(last) = self.last_sent_pos else {
            self.update_player_position(hooks);
            self.last_sent_pos = Some(current);
            self.last_pos_update_tick = self.tick_count;
            return;
        };
        if last == current {
            if self.tick_count.saturating_sub(self.last_pos_update_tick) >= 100
                && !self.has_teleport_pending(hooks)
            {
                self.update_player_position(hooks);
                self.last_pos_update_tick = self.tick_count;
            }
            return;
        }
        let chunk_changed = (last[0] / 16.0).floor() != (current[0] / 16.0).floor()
            || (last[2] / 16.0).floor() != (current[2] / 16.0).floor();
        if chunk_changed || self.tick_count.saturating_sub(self.last_pos_update_tick) >= 2 {
            if self.has_teleport_pending(hooks) {
                return;
            }
            self.update_player_position(hooks);
            self.last_sent_pos = Some(current);
            self.last_pos_update_tick = self.tick_count;
        }
    }

    pub fn update_player_position(&mut self, hooks: &dyn ServerHooks) {
        let Some(name) = self.username.clone() else {
            return;
        };
        hooks.update_virtual_position(
            &name,
            self.x,
            self.y,
            self.z,
            wire::java_yaw_to_bedrock(self.yaw),
            self.pitch,
        );
    }

    fn pos_differs(x: f64, y: f64, z: f64, yaw: f64, pitch: f64, last: [f64; 5]) -> bool {
        (x - last[0]).abs() > 0.001
            || (y - last[1]).abs() > 0.001
            || (z - last[2]).abs() > 0.001
            || (yaw - last[3]).abs() > 0.01
            || (pitch - last[4]).abs() > 0.01
    }

    fn has_teleport_pending(&self, hooks: &dyn ServerHooks) -> bool {
        let (Some(name), Some(last)) = (&self.username, self.last_sent_pos) else {
            return false;
        };
        let Some(entry) = hooks.get_snapshot(name) else {
            return false;
        };
        if !entry.is_virtual {
            return false;
        }
        let yaw = wire::bedrock_yaw_to_java(entry.yaw);
        let pitch = wire::bedrock_pitch_to_java(entry.pitch);
        Self::pos_differs(entry.x, entry.y, entry.z, yaw, pitch, last)
    }

    fn sync_teleport(&mut self, hooks: &dyn ServerHooks) {
        if !self.is_playing() {
            return;
        }
        let (Some(name), Some(last)) = (self.username.clone(), self.last_sent_pos) else {
            return;
        };
        let Some(entry) = hooks.get_snapshot(&name) else {
            return;
        };
        if !entry.is_virtual {
            return;
        }
        let yaw = wire::bedrock_yaw_to_java(entry.yaw);
        let pitch = wire::bedrock_pitch_to_java(entry.pitch);
        if Self::pos_differs(entry.x, entry.y, entry.z, yaw, pitch, last) {
            self.teleport(hooks, entry.x, entry.y, entry.z, Some(yaw), Some(pitch));
        }
    }

    fn sync_gamemode(&mut self, hooks: &dyn ServerHooks) {
        if !self.is_playing() {
            return;
        }
        let Some(name) = self.username.clone() else {
            return;
        };
        let Some(entry) = hooks.get_snapshot(&name) else {
            return;
        };
        let mode = entry.gamemode.clamp(0, 3);
        if mode != self.gamemode {
            self.set_gamemode(mode);
        }
    }

    pub fn deliver_plugin_messages(&mut self, hooks: &dyn ServerHooks) {
        let Some(name) = self.username.clone() else {
            return;
        };
        for line in hooks.take_plugin_messages(&name) {
            if !line.is_empty() {
                self.send_chat(&line);
            }
        }
    }

    pub fn teleport(
        &mut self,
        hooks: &dyn ServerHooks,
        x: f64,
        y: f64,
        z: f64,
        yaw: Option<f64>,
        pitch: Option<f64>,
    ) {
        self.x = x;
        self.y = y;
        self.z = z;
        if let Some(yaw) = yaw {
            self.yaw = yaw;
        }
        if let Some(pitch) = pitch {
            self.pitch = pitch;
        }
        self.last_chunk = None;
        self.send_position(true);
        self.request_chunks(hooks);
        self.last_sent_pos = Some([self.x, self.y, self.z, self.yaw, self.pitch]);
        self.last_pos_update_tick = self.tick_count;
        self.update_player_position(hooks);
    }

    fn send_position(&mut self, _force: bool) {
        if self.is_modern() {
            self.teleport_id += 1;
            let mut payload = wire::varint(self.teleport_id);
            for v in [self.x, self.y, self.z, 0.0, 0.0, 0.0] {
                payload.extend_from_slice(&wire::double(v));
            }
            payload.extend_from_slice(&wire::float(self.yaw as f32));
            payload.extend_from_slice(&wire::float(self.pitch as f32));
            payload.extend_from_slice(&0u32.to_be_bytes());
            let id = self.pid("position");
            self.send(id, &payload);
            return;
        }
        self.teleport_id += 1;
        let teleport_id = self.teleport_id;
        let mut payload = wire::double(self.x);
        payload.extend_from_slice(&wire::double(self.y + 64.0));
        payload.extend_from_slice(&wire::double(self.z));
        payload.extend_from_slice(&wire::float(self.yaw as f32));
        payload.extend_from_slice(&wire::float(self.pitch as f32));
        payload.push(0);
        payload.extend_from_slice(&wire::varint(teleport_id));
        self.send(wire::S2C_POSITION, &payload);
    }

    pub fn send_time(&mut self, time: i64, tick_day_time: bool) {
        if !self.is_playing() {
            return;
        }
        if self.is_modern() {
            let payload = if self.protocol >= 775 {
                let mut p = wire::long(0);
                p.extend_from_slice(&wire::varint(1));
                p.extend_from_slice(&wire::varint(0));
                p.extend_from_slice(&wire::var_long(time));
                p.extend_from_slice(&wire::float(0.0));
                p.extend_from_slice(&wire::float(if tick_day_time { 1.0 } else { 0.0 }));
                p
            } else {
                let mut p = wire::long(0);
                p.extend_from_slice(&wire::long(time));
                p.extend_from_slice(&wire::boolean(tick_day_time));
                p
            };
            let id = self.pid("update_time");
            self.send(id, &payload);
        } else {
            let mut p = wire::long(0);
            p.extend_from_slice(&wire::long(time));
            self.send(wire::S2C_UPDATE_TIME, &p);
        }
    }

    pub fn set_gamemode(&mut self, gamemode: i32) {
        let gamemode = gamemode.clamp(0, 3);
        if gamemode == self.gamemode {
            return;
        }
        self.gamemode = gamemode;
        if let Some(name) = &self.username {
            if let Some(entry) = self.tab_listed.get_mut(&lower(name)) {
                entry.gamemode = gamemode;
            }
        }
        if !self.is_playing() {
            return;
        }
        if self.is_modern() {
            let id = self.pid("player_info_update");
            self.send(id, &self.modern_player_info_gamemode(gamemode, None));
            let gsc = self.pid("game_state_change");
            self.send(
                gsc,
                &[wire::GAME_EVENT_CHANGE_GAMEMODE, gamemode as u8, 0, 0, 0],
            );
            let abilities = self.pid("abilities");
            self.send(abilities, &abilities_payload(gamemode));
        } else {
            self.send(
                wire::S2C_PLAYER_INFO,
                &self.legacy_player_info_gamemode(gamemode, None),
            );
            self.send(wire::S2C_ABILITIES, &abilities_payload(gamemode));
        }
    }

    // ------------------------------------------------------------------
    // TAB (player_info)
    // ------------------------------------------------------------------

    fn modern_player_info_add_batch(entries: &[TabEntry]) -> Vec<u8> {
        let flags = wire::PLAYER_INFO_ADD_PLAYER
            | wire::PLAYER_INFO_UPDATE_GAME_MODE
            | wire::PLAYER_INFO_UPDATE_LISTED
            | wire::PLAYER_INFO_UPDATE_LATENCY;
        let mut payload = vec![flags];
        payload.extend_from_slice(&wire::varint(entries.len() as i32));
        for entry in entries {
            payload.extend_from_slice(&wire::offline_uuid(&entry.name));
            payload.extend_from_slice(&wire::string(&entry.name));
            payload.extend_from_slice(&wire::varint(0));
            payload.extend_from_slice(&wire::varint(entry.gamemode.clamp(0, 3)));
            payload.extend_from_slice(&wire::varint(1));
            payload.extend_from_slice(&wire::varint(entry.ping.max(0)));
        }
        payload
    }

    fn legacy_player_info_add_batch(entries: &[TabEntry]) -> Vec<u8> {
        let mut payload = wire::varint(0);
        payload.extend_from_slice(&wire::varint(entries.len() as i32));
        for entry in entries {
            payload.extend_from_slice(&wire::offline_uuid(&entry.name));
            payload.extend_from_slice(&wire::string(&entry.name));
            payload.extend_from_slice(&wire::varint(0));
            payload.extend_from_slice(&wire::varint(entry.gamemode.clamp(0, 3)));
            payload.extend_from_slice(&wire::varint(entry.ping.max(0)));
            payload.extend_from_slice(&wire::boolean(true));
            payload.extend_from_slice(&wire::string(&entry.name));
        }
        payload
    }

    fn modern_player_info_gamemode_batch(entries: &[TabEntry]) -> Vec<u8> {
        let mut payload = vec![wire::PLAYER_INFO_UPDATE_GAME_MODE];
        payload.extend_from_slice(&wire::varint(entries.len() as i32));
        for entry in entries {
            payload.extend_from_slice(&wire::offline_uuid(&entry.name));
            payload.extend_from_slice(&wire::varint(entry.gamemode.clamp(0, 3)));
        }
        payload
    }

    fn legacy_player_info_gamemode_batch(entries: &[TabEntry]) -> Vec<u8> {
        let mut payload = wire::varint(3);
        payload.extend_from_slice(&wire::varint(entries.len() as i32));
        for entry in entries {
            payload.extend_from_slice(&wire::offline_uuid(&entry.name));
            payload.extend_from_slice(&wire::varint(entry.gamemode.clamp(0, 3)));
        }
        payload
    }

    fn modern_player_info_gamemode(&self, gamemode: i32, name: Option<&str>) -> Vec<u8> {
        let name = name.unwrap_or_else(|| self.username.as_deref().unwrap_or(""));
        let mut payload = vec![wire::PLAYER_INFO_UPDATE_GAME_MODE];
        payload.extend_from_slice(&wire::varint(1));
        payload.extend_from_slice(&wire::offline_uuid(name));
        payload.extend_from_slice(&wire::varint(gamemode));
        payload
    }

    fn legacy_player_info_gamemode(&self, gamemode: i32, name: Option<&str>) -> Vec<u8> {
        let name = name.unwrap_or_else(|| self.username.as_deref().unwrap_or(""));
        let mut payload = wire::varint(3);
        payload.extend_from_slice(&wire::varint(1));
        payload.extend_from_slice(&wire::offline_uuid(name));
        payload.extend_from_slice(&wire::varint(gamemode));
        payload
    }

    /// Diff `targets` against `tab_listed` (modern_test: add uses
    /// `player_info_update` 0x01+0x04+0x08+0x10, gamemode change uses the
    /// 0x04-only form; removals use `player_remove` / legacy action 4).
    pub fn sync_tab(&mut self, targets: &[TabTarget]) {
        if !self.is_playing() {
            return;
        }
        let mut normalized: HashMap<String, TabEntry> = HashMap::new();
        for target in targets {
            normalized.insert(
                lower(&target.name),
                TabEntry {
                    name: target.name.clone(),
                    gamemode: target.gamemode.clamp(0, 3),
                    ping: target.ping.max(0),
                },
            );
        }
        let removed: Vec<TabEntry> = self
            .tab_listed
            .keys()
            .filter(|key| !normalized.contains_key(*key))
            .map(|key| self.tab_listed[key].clone())
            .collect();
        for entry in &removed {
            self.tab_listed.remove(&lower(&entry.name));
        }
        if !removed.is_empty() {
            if self.is_modern() {
                let mut payload = wire::varint(removed.len() as i32);
                for entry in &removed {
                    payload.extend_from_slice(&wire::offline_uuid(&entry.name));
                }
                let id = self.pid("player_remove");
                self.send(id, &payload);
            } else {
                let mut payload = wire::varint(4);
                payload.extend_from_slice(&wire::varint(removed.len() as i32));
                for entry in &removed {
                    payload.extend_from_slice(&wire::offline_uuid(&entry.name));
                }
                self.send(wire::S2C_PLAYER_INFO, &payload);
            }
        }
        let mut added: Vec<TabEntry> = Vec::new();
        let mut gamemode_changed: Vec<TabEntry> = Vec::new();
        for (key, target) in &normalized {
            match self.tab_listed.get(key) {
                None => added.push(target.clone()),
                Some(current) => {
                    if current.gamemode != target.gamemode {
                        gamemode_changed.push(target.clone());
                    }
                    if let Some(entry) = self.tab_listed.get_mut(key) {
                        entry.gamemode = target.gamemode;
                        entry.ping = target.ping;
                    }
                }
            }
        }
        if !added.is_empty() {
            if self.is_modern() {
                let id = self.pid("player_info_update");
                self.send(id, &Self::modern_player_info_add_batch(&added));
            } else {
                self.send(
                    wire::S2C_PLAYER_INFO,
                    &Self::legacy_player_info_add_batch(&added),
                );
            }
            for entry in &added {
                self.tab_listed.insert(lower(&entry.name), entry.clone());
            }
        }
        if !gamemode_changed.is_empty() {
            if self.is_modern() {
                let id = self.pid("player_info_update");
                self.send(
                    id,
                    &Self::modern_player_info_gamemode_batch(&gamemode_changed),
                );
            } else {
                self.send(
                    wire::S2C_PLAYER_INFO,
                    &Self::legacy_player_info_gamemode_batch(&gamemode_changed),
                );
            }
        }
    }

    // ------------------------------------------------------------------
    // Chunks
    // ------------------------------------------------------------------

    fn reset_chunk_streaming(&mut self) {
        self.sent_chunks.clear();
        self.pending_chunks.clear();
        self.pending_attempts.clear();
        self.last_chunk = None;
        self.view_announced = false;
        self.spawn_chunks_at = Instant::now();
        self.spawn_chunks_logged = false;
    }

    fn request_chunks(&mut self, hooks: &dyn ServerHooks) {
        let cx = (self.x / 16.0).floor() as i32;
        let cz = (self.z / 16.0).floor() as i32;
        if self.last_chunk == Some((cx, cz)) {
            return;
        }
        self.last_chunk = Some((cx, cz));
        let radius = if self.is_modern() {
            hooks.view_distance().clamp(2, 32)
        } else {
            hooks.view_distance()
        };
        self.pending_chunks.clear();
        let mut wanted: HashSet<(i32, i32)> = HashSet::new();
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                wanted.insert((cx + dx, cz + dz));
                if !self.sent_chunks.contains(&(cx + dx, cz + dz)) {
                    self.pending_chunks.push_back((cx + dx, cz + dz));
                }
            }
        }
        let mut decorated: Vec<(i32, i32, i32)> = self
            .pending_chunks
            .iter()
            .map(|(px, pz)| ((px - cx) * (px - cx) + (pz - cz) * (pz - cz), *px, *pz))
            .collect();
        decorated.sort_unstable();
        self.pending_chunks = decorated.into_iter().map(|(_, px, pz)| (px, pz)).collect();
        let outside: Vec<(i32, i32)> = self
            .sent_chunks
            .iter()
            .copied()
            .filter(|(sx, sz)| {
                !wanted.contains(&(*sx, *sz))
                    && ((*sx - cx).abs() > radius + 1 || (*sz - cz).abs() > radius + 1)
            })
            .collect();
        for (sx, sz) in outside {
            self.sent_chunks.remove(&(sx, sz));
            self.send_unload_chunk(sx, sz);
            self.seen_entities.retain(|_, seen| {
                (
                    (seen.x / 16.0).floor() as i32,
                    (seen.z / 16.0).floor() as i32,
                ) != (sx, sz)
            });
        }
        if self.is_modern() {
            let id = self.pid("update_view_position");
            let mut payload = wire::varint(cx);
            payload.extend_from_slice(&wire::varint(cz));
            self.send(id, &payload);
            if !self.view_announced {
                let id = self.pid("update_view_distance");
                self.send(id, &wire::varint(radius));
                self.view_announced = true;
            }
        }
    }

    fn send_unload_chunk(&mut self, cx: i32, cz: i32) {
        if self.is_modern() {
            let id = self.pid("unload_chunk");
            let mut payload = (cz as u32).to_be_bytes().to_vec();
            payload.extend_from_slice(&(cx as u32).to_be_bytes());
            self.send(id, &payload);
        } else {
            let mut payload = (cx as u32).to_be_bytes().to_vec();
            payload.extend_from_slice(&(cz as u32).to_be_bytes());
            self.send(wire::S2C_UNLOAD_CHUNK, &payload);
        }
    }

    /// Send pending columns until the per-tick budget (`CHUNK_BUDGET` or the
    /// shared `deadline` from `JavaServer::tick`), backpressure, or the
    /// client ceiling (`chunk_batch_received`) is hit. A modern batch is
    /// opened lazily so a `chunk_batch_start` never lacks its
    /// `chunk_batch_finished`; a column that cannot be built after 20
    /// attempts rotates to the end of the queue instead of blocking it.
    pub fn stream_chunks(&mut self, hooks: &dyn ServerHooks, deadline: Option<Instant>) {
        if self.last_chunk.is_none() {
            self.request_chunks(hooks);
        }
        if self.pending_chunks.is_empty() {
            return;
        }
        let ceiling = self
            .chunks_per_tick
            .map(|rate| (rate.floor() as usize).max(1))
            .unwrap_or(CHUNKS_PER_BATCH);
        let per_tick = ceiling.min(CHUNKS_PER_BATCH);
        let deadline = deadline.unwrap_or_else(|| Instant::now() + CHUNK_BUDGET);
        let mut sent = 0usize;
        let mut batch_open = false;
        while let Some((cx, cz)) = self.pending_chunks.pop_front() {
            if self.pending_bytes() > CHUNK_BACKPRESSURE {
                self.pending_chunks.push_front((cx, cz));
                break;
            }
            if sent >= per_tick || Instant::now() > deadline {
                self.pending_chunks.push_front((cx, cz));
                break;
            }
            if self.sent_chunks.contains(&(cx, cz)) {
                self.pending_attempts.remove(&(cx, cz));
                continue;
            }
            let built = self.build_chunk(hooks, cx, cz);
            let Some(built) = built else {
                let attempts = self.pending_attempts.get(&(cx, cz)).copied().unwrap_or(0) + 1;
                if attempts >= 20 {
                    self.pending_chunks.push_back((cx, cz));
                    self.pending_attempts.insert((cx, cz), 0);
                } else {
                    self.pending_chunks.push_front((cx, cz));
                    self.pending_attempts.insert((cx, cz), attempts);
                }
                break;
            };
            self.pending_attempts.remove(&(cx, cz));
            if self.is_modern() {
                if !batch_open {
                    let id = self.pid("chunk_batch_start");
                    self.send(id, &[]);
                    batch_open = true;
                }
                let id = self.pid("map_chunk");
                let mut payload = (cx as u32).to_be_bytes().to_vec();
                payload.extend_from_slice(&(cz as u32).to_be_bytes());
                payload.extend_from_slice(&built.heightmaps);
                payload.extend_from_slice(&wire::varint(built.data.len() as i32));
                payload.extend_from_slice(&built.data);
                payload.extend_from_slice(&wire::varint(0));
                payload.extend_from_slice(&built.light);
                self.send(id, &payload);
            } else {
                let mut payload = (cx as u32).to_be_bytes().to_vec();
                payload.extend_from_slice(&(cz as u32).to_be_bytes());
                payload.extend_from_slice(&wire::boolean(true));
                payload.extend_from_slice(&wire::varint(built.bitmap));
                payload.extend_from_slice(&wire::varint(built.data.len() as i32));
                payload.extend_from_slice(&built.data);
                payload.extend_from_slice(&wire::varint(0));
                self.send(wire::S2C_MAP_CHUNK, &payload);
            }
            self.sent_chunks.insert((cx, cz));
            sent += 1;
        }
        if batch_open && self.is_modern() {
            let id = self.pid("chunk_batch_finished");
            self.send(id, &wire::varint(sent as i32));
        }
        if !self.spawn_chunks_logged && self.pending_chunks.is_empty() {
            self.spawn_chunks_logged = true;
        }
    }

    /// Unified chunk build result (legacy bitmap/data or modern column).
    fn build_chunk(&self, hooks: &dyn ServerHooks, cx: i32, cz: i32) -> Option<BuiltChunk> {
        if self.is_modern() {
            let data = hooks.modern_chunk(self.protocol, cx, cz)?;
            Some(BuiltChunk {
                bitmap: 0,
                data: data.data,
                heightmaps: data.heightmaps,
                light: data.light,
            })
        } else {
            let chunk = hooks.legacy_chunk(cx, cz)?;
            Some(BuiltChunk {
                bitmap: chunk.bitmap,
                data: chunk.data,
                heightmaps: Vec::new(),
                light: Vec::new(),
            })
        }
    }

    // ------------------------------------------------------------------
    // Entities (mirror of sync_virtuals_locked, Java observer side)
    // ------------------------------------------------------------------

    pub fn sync_entities(&mut self, hooks: &dyn ServerHooks, targets: &[EntityTarget]) {
        let wanted: HashMap<String, &EntityTarget> =
            targets.iter().map(|t| (lower(&t.name), t)).collect();
        let stale: Vec<String> = self
            .seen_entities
            .keys()
            .filter(|key| !wanted.contains_key(*key))
            .cloned()
            .collect();
        for key in stale {
            self.despawn_entity(hooks, &key);
        }
        for target in targets {
            let key = lower(&target.name);
            if self.is_modern() {
                self.sync_modern_entity(hooks, &key, target);
            } else {
                self.sync_legacy_entity(hooks, &key, target);
            }
        }
    }

    fn announce_in_tab(&mut self, target: &EntityTarget) {
        let key = lower(&target.name);
        if self.tab_listed.contains_key(&key) {
            return;
        }
        let entry = TabEntry {
            name: target.name.clone(),
            gamemode: target.gamemode,
            ping: target.ping,
        };
        if self.is_modern() {
            let id = self.pid("player_info_update");
            self.send(
                id,
                &Self::modern_player_info_add_batch(std::slice::from_ref(&entry)),
            );
        } else {
            self.send(
                wire::S2C_PLAYER_INFO,
                &Self::legacy_player_info_add_batch(std::slice::from_ref(&entry)),
            );
        }
        self.tab_listed.insert(key, entry);
    }

    fn sync_modern_entity(&mut self, hooks: &dyn ServerHooks, key: &str, target: &EntityTarget) {
        let entity = self.entity_id_for(hooks, &target.name);
        if !self.seen_entities.contains_key(key) {
            self.announce_in_tab(target);
            let id = self.pid("spawn_entity");
            self.send(id, &self.modern_spawn_payload(entity, target));
            let id = self.pid("entity_metadata");
            self.send(id, &modern_nametag_payload(entity, &target.name));
            self.seen_entities.insert(
                key.to_string(),
                SeenEntity {
                    name: target.name.clone(),
                    entity,
                    x: target.x,
                    y: target.y,
                    z: target.z,
                    yaw: target.yaw,
                    pitch: target.pitch,
                    gamemode: target.gamemode,
                },
            );
            return;
        }
        let prev = self.seen_entities[key].clone();
        let dist =
            (target.x - prev.x).abs() + (target.y - prev.y).abs() + (target.z - prev.z).abs();
        let rot_changed =
            (target.yaw - prev.yaw).abs() > 0.01 || (target.pitch - prev.pitch).abs() > 0.01;
        if dist > 8.0 {
            let id = self.pid("entity_teleport");
            self.send(id, &modern_teleport_payload(entity, target));
        } else if dist > 0.0001 {
            let (dx, dy, dz) = (
                wire::rel_delta(target.x - prev.x),
                wire::rel_delta(target.y - prev.y),
                wire::rel_delta(target.z - prev.z),
            );
            if rot_changed {
                let id = self.pid("entity_move_look");
                self.send(id, &move_look_payload(entity, dx, dy, dz, target));
            } else {
                let id = self.pid("rel_entity_move");
                self.send(id, &rel_move_payload(entity, dx, dy, dz));
            }
        } else if rot_changed {
            let id = self.pid("entity_look");
            self.send(id, &look_payload(entity, target));
        }
        if (target.yaw - prev.yaw).abs() > 0.01 {
            let id = self.pid("entity_head_rotation");
            let mut payload = wire::varint(entity);
            payload.push(wire::angle(target.yaw) as u8);
            self.send(id, &payload);
        }
        self.seen_entities.insert(
            key.to_string(),
            SeenEntity {
                name: target.name.clone(),
                entity,
                x: target.x,
                y: target.y,
                z: target.z,
                yaw: target.yaw,
                pitch: target.pitch,
                gamemode: target.gamemode,
            },
        );
    }

    fn sync_legacy_entity(&mut self, hooks: &dyn ServerHooks, key: &str, target: &EntityTarget) {
        let entity = self.entity_id_for(hooks, &target.name);
        if !self.seen_entities.contains_key(key) {
            self.announce_in_tab(target);
            self.send(
                wire::S2C_NAMED_ENTITY_SPAWN,
                &legacy_spawn_payload(entity, target),
            );
            self.send(
                wire::S2C_ENTITY_METADATA,
                &legacy_nametag_payload(entity, &target.name),
            );
            self.seen_entities.insert(
                key.to_string(),
                SeenEntity {
                    name: target.name.clone(),
                    entity,
                    x: target.x,
                    y: target.y,
                    z: target.z,
                    yaw: target.yaw,
                    pitch: target.pitch,
                    gamemode: target.gamemode,
                },
            );
            return;
        }
        let prev = self.seen_entities[key].clone();
        let dist =
            (target.x - prev.x).abs() + (target.y - prev.y).abs() + (target.z - prev.z).abs();
        let rot_changed =
            (target.yaw - prev.yaw).abs() > 0.01 || (target.pitch - prev.pitch).abs() > 0.01;
        if dist > 8.0 {
            let mut payload = wire::varint(entity);
            payload.extend_from_slice(&wire::double(target.x));
            payload.extend_from_slice(&wire::double(target.y + 64.0));
            payload.extend_from_slice(&wire::double(target.z));
            payload.push(wire::angle(target.yaw) as u8);
            payload.push(wire::angle(target.pitch) as u8);
            payload.extend_from_slice(&wire::boolean(false));
            self.send(wire::S2C_ENTITY_TELEPORT, &payload);
        } else if dist > 0.0001 {
            let (dx, dy, dz) = (
                wire::rel_delta(target.x - prev.x),
                wire::rel_delta(target.y - prev.y),
                wire::rel_delta(target.z - prev.z),
            );
            if rot_changed {
                self.send(
                    wire::S2C_ENTITY_MOVE_LOOK,
                    &move_look_payload(entity, dx, dy, dz, target),
                );
            } else {
                self.send(
                    wire::S2C_REL_ENTITY_MOVE,
                    &rel_move_payload(entity, dx, dy, dz),
                );
            }
        } else if rot_changed {
            self.send(wire::S2C_ENTITY_LOOK, &look_payload(entity, target));
        }
        if (target.yaw - prev.yaw).abs() > 0.01 {
            let mut payload = wire::varint(entity);
            payload.push(wire::angle(target.yaw) as u8);
            self.send(wire::S2C_ENTITY_HEAD_ROTATION, &payload);
        }
        self.seen_entities.insert(
            key.to_string(),
            SeenEntity {
                name: target.name.clone(),
                entity,
                x: target.x,
                y: target.y,
                z: target.z,
                yaw: target.yaw,
                pitch: target.pitch,
                gamemode: target.gamemode,
            },
        );
    }

    fn despawn_entity(&mut self, hooks: &dyn ServerHooks, key: &str) {
        let Some(seen) = self.seen_entities.remove(key) else {
            return;
        };
        let in_tab = self.tab_listed.contains_key(key);
        if self.is_modern() {
            let id = self.pid("entity_destroy");
            let mut payload = wire::varint(1);
            payload.extend_from_slice(&wire::varint(seen.entity));
            self.send(id, &payload);
            if !in_tab {
                let mut payload = wire::varint(1);
                payload.extend_from_slice(&wire::offline_uuid(&seen.name));
                let id = self.pid("player_remove");
                self.send(id, &payload);
            }
        } else {
            let mut payload = wire::varint(1);
            payload.extend_from_slice(&wire::varint(seen.entity));
            self.send(wire::S2C_ENTITY_DESTROY, &payload);
            if !in_tab {
                let mut payload = wire::varint(4);
                payload.extend_from_slice(&wire::varint(1));
                payload.extend_from_slice(&wire::offline_uuid(&seen.name));
                self.send(wire::S2C_PLAYER_INFO, &payload);
            }
        }
        let _ = hooks;
    }

    fn modern_spawn_payload(&self, entity: i32, target: &EntityTarget) -> Vec<u8> {
        let mut payload = wire::varint(entity);
        payload.extend_from_slice(&wire::offline_uuid(&target.name));
        payload.extend_from_slice(&wire::varint(profile::player_entity_type(self.protocol)));
        for v in [target.x, target.y, target.z] {
            payload.extend_from_slice(&wire::double(v));
        }
        let angles = [
            wire::angle(target.pitch) as u8,
            wire::angle(target.yaw) as u8,
            wire::angle(target.yaw) as u8,
        ];
        if profile::spawn_velocity_is_lp_vec3(self.protocol) {
            payload.extend_from_slice(&wire::lp_vec3(0.0, 0.0, 0.0));
            payload.extend_from_slice(&angles);
            payload.extend_from_slice(&wire::varint(0));
        } else {
            payload.extend_from_slice(&angles);
            payload.extend_from_slice(&wire::varint(0));
            for _ in 0..3 {
                payload.extend_from_slice(&wire::short(0));
            }
        }
        payload
    }

    // ------------------------------------------------------------------
    // Chat / blocks / misc sends
    // ------------------------------------------------------------------

    pub fn send_chat(&mut self, message: &str) {
        if self.closed {
            return;
        }
        if self.is_modern() {
            let id = self.pid("system_chat");
            let mut payload = wire::nbt_text(message);
            payload.extend_from_slice(&wire::boolean(false));
            self.send(id, &payload);
        } else {
            let mut payload = wire::string(&wire::chat_json(message));
            payload.push(0);
            self.send(wire::S2C_CHAT, &payload);
        }
    }

    pub fn send_block_change(
        &mut self,
        hooks: &dyn ServerHooks,
        x: i32,
        y: i32,
        z: i32,
        kind: &str,
        network_id: Option<u32>,
    ) {
        if self.is_modern() {
            let state = match network_id {
                Some(id) => hooks.modern_block_state(id, self.protocol),
                None => {
                    if kind.is_empty() || kind == "air" {
                        0
                    } else {
                        1
                    }
                }
            };
            let id = self.pid("block_change");
            let mut payload = wire::position(x, y, z);
            payload.extend_from_slice(&wire::varint(state));
            self.send(id, &payload);
        } else {
            let state = match network_id {
                Some(id) => hooks.legacy_block_state(id),
                None => {
                    if kind == "air" {
                        0
                    } else {
                        1
                    }
                }
            };
            let mut payload = wire::position(x, y + 64, z);
            payload.extend_from_slice(&wire::varint(state));
            self.send(wire::S2C_BLOCK_CHANGE, &payload);
        }
    }

    fn keep_alive(&mut self, now: Instant) {
        if now.duration_since(self.last_keep_alive).as_secs_f64() < KEEP_ALIVE_SECONDS {
            return;
        }
        self.last_keep_alive = now;
        self.keep_alive_id += 1;
        let id = if self.is_modern() {
            self.pid("keep_alive")
        } else {
            wire::S2C_KEEP_ALIVE
        };
        self.send(id, &wire::long(self.keep_alive_id));
    }

    pub fn entity_id(&mut self, hooks: &dyn ServerHooks) -> i32 {
        if let Some(id) = self.own_entity_id {
            return id;
        }
        if let Some(name) = self.username.clone() {
            let id = hooks.entity_id_for(&name);
            self.own_entity_id = Some(id);
            return id;
        }
        1000 + ((self.peer.to_string().len() as i32) % 100_000)
    }

    pub fn entity_id_for(&mut self, hooks: &dyn ServerHooks, name: &str) -> i32 {
        hooks.entity_id_for(name)
    }

    pub fn kick(&mut self, reason: &str) {
        if self.closed {
            return;
        }
        if self.is_modern() && self.playing {
            let id = self.pid("kick");
            self.send(id, &wire::nbt_text(reason));
        } else if self.state == SessionState::Login {
            self.send(
                wire::S2C_LOGIN_DISCONNECT,
                &wire::string(&wire::chat_json(reason)),
            );
        } else if self.playing {
            self.send(wire::S2C_KICK, &wire::string(&wire::chat_json(reason)));
        }
        self.close();
    }

    pub fn close(&mut self) {
        self.out_buf.clear();
        self.closed = true;
        self.playing = false;
    }

    fn send(&mut self, packet_id: i32, payload: &[u8]) {
        if self.closed {
            return;
        }
        self.out_buf
            .extend_from_slice(&wire::frame(packet_id, payload));
    }
}

struct BuiltChunk {
    bitmap: i32,
    data: Vec<u8>,
    heightmaps: Vec<u8>,
    light: Vec<u8>,
}

fn lower(name: &str) -> String {
    name.to_lowercase()
}

fn ability_flags(gamemode: i32) -> i32 {
    match gamemode {
        1 => 0x01 | 0x04 | 0x08,
        3 => 0x01 | 0x04,
        _ => 0,
    }
}

fn abilities_payload(gamemode: i32) -> Vec<u8> {
    let mut payload = vec![ability_flags(gamemode) as u8];
    payload.extend_from_slice(&wire::float(0.05));
    payload.extend_from_slice(&wire::float(0.1));
    payload
}

fn rel_move_payload(entity: i32, dx: i32, dy: i32, dz: i32) -> Vec<u8> {
    let mut payload = wire::varint(entity);
    for d in [dx, dy, dz] {
        payload.extend_from_slice(&wire::short(d));
    }
    payload.extend_from_slice(&wire::boolean(false));
    payload
}

fn move_look_payload(entity: i32, dx: i32, dy: i32, dz: i32, target: &EntityTarget) -> Vec<u8> {
    let mut payload = rel_move_payload(entity, dx, dy, dz);
    // rel_move ends with on_ground; move_look inserts angles before it.
    payload.truncate(payload.len() - 1);
    payload.push(wire::angle(target.yaw) as u8);
    payload.push(wire::angle(target.pitch) as u8);
    payload.extend_from_slice(&wire::boolean(false));
    payload
}

fn look_payload(entity: i32, target: &EntityTarget) -> Vec<u8> {
    let mut payload = wire::varint(entity);
    payload.push(wire::angle(target.yaw) as u8);
    payload.push(wire::angle(target.pitch) as u8);
    payload.extend_from_slice(&wire::boolean(false));
    payload
}

fn modern_teleport_payload(entity: i32, target: &EntityTarget) -> Vec<u8> {
    let mut payload = wire::varint(entity);
    for v in [target.x, target.y, target.z, 0.0, 0.0, 0.0] {
        payload.extend_from_slice(&wire::double(v));
    }
    payload.extend_from_slice(&wire::float(target.yaw as f32));
    payload.extend_from_slice(&wire::float(target.pitch as f32));
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&wire::boolean(false));
    payload
}

fn legacy_spawn_payload(entity: i32, target: &EntityTarget) -> Vec<u8> {
    let mut payload = wire::varint(entity);
    payload.extend_from_slice(&wire::offline_uuid(&target.name));
    payload.extend_from_slice(&wire::double(target.x));
    payload.extend_from_slice(&wire::double(target.y + 64.0));
    payload.extend_from_slice(&wire::double(target.z));
    payload.push(wire::angle(target.yaw) as u8);
    payload.push(wire::angle(target.pitch) as u8);
    payload.push(0xFF);
    payload
}

/// Modern nametag: `custom_name` (index 2, `optional_component` type 6)
/// with the presence byte `0x01` before the anonymous NBT, plus
/// `custom_name_visible` (3, boolean type 8). Sending type 5 (direct
/// component) instead makes 26.1 abort with IllegalStateException.
fn modern_nametag_payload(entity: i32, name: &str) -> Vec<u8> {
    let mut metadata = vec![2u8];
    metadata.extend_from_slice(&wire::varint(6));
    metadata.extend_from_slice(&wire::boolean(true));
    metadata.extend_from_slice(&wire::nbt_text(name));
    metadata.push(3);
    metadata.extend_from_slice(&wire::varint(8));
    metadata.extend_from_slice(&wire::boolean(true));
    metadata.push(0xFF);
    let mut payload = wire::varint(entity);
    payload.extend_from_slice(&metadata);
    payload
}

fn legacy_nametag_payload(entity: i32, name: &str) -> Vec<u8> {
    let mut metadata = vec![2u8, 3u8];
    metadata.extend_from_slice(&wire::string(name));
    metadata.push(3);
    metadata.push(6);
    metadata.extend_from_slice(&wire::boolean(true));
    metadata.push(0xFF);
    let mut payload = wire::varint(entity);
    payload.extend_from_slice(&metadata);
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::Reader;

    pub(crate) struct StubHooks {
        pub snapshots: Vec<PlayerSnapshot>,
    }

    impl ServerHooks for StubHooks {
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

    fn modern_session(protocol: i32) -> JavaSession {
        let mut session = JavaSession::new(1, "127.0.0.1:25565".parse().unwrap());
        session.protocol = protocol;
        session.username = Some("Steve".to_string());
        session.state = SessionState::Play;
        session.playing = true;
        session
    }

    fn frames(session: &mut JavaSession) -> Vec<(i32, Vec<u8>)> {
        let bytes = session.take_out();
        let mut out = Vec::new();
        let mut rest = bytes.as_slice();
        while let Some((id, payload, consumed)) = wire::try_split_frame(rest) {
            out.push((id, payload));
            rest = &rest[consumed..];
        }
        out
    }

    #[test]
    fn modern_spawn_uses_lp_vec3_on_775_and_vec3i16_on_769() {
        let hooks = StubHooks { snapshots: vec![] };
        for (protocol, lp) in [(769, false), (775, true)] {
            let mut session = modern_session(protocol);
            let target = EntityTarget {
                name: "Alex".to_string(),
                x: 1.0,
                y: 64.0,
                z: 2.0,
                yaw: 0.0,
                pitch: 0.0,
                gamemode: 0,
                ping: 0,
            };
            session.sync_entities(&hooks, &[target]);
            let sent = frames(&mut session);
            let spawn = sent
                .iter()
                .find(|(id, _)| *id == profile::packet_id(protocol, "spawn_entity").unwrap())
                .expect("spawn_entity sent");
            let payload = &spawn.1;
            if lp {
                // After entityId+uuid+type+xyz comes lpVec3(0,0,0) = 0x00,
                // then pitch/yaw/headPitch bytes.
                let offset = wire::varint(1_000_000).len() + 16 + 1 + 24;
                assert_eq!(payload[offset], 0x00, "lpVec3 zero byte at {offset}");
            } else {
                // Trailing vec3i16 zeros after angles + objectData.
                assert!(payload.ends_with(&[0, 0, 0, 0, 0, 0]), "vec3i16 tail");
            }
        }
    }

    #[test]
    fn modern_nametag_uses_optional_component() {
        let hooks = StubHooks { snapshots: vec![] };
        let mut session = modern_session(775);
        session.sync_entities(
            &hooks,
            &[EntityTarget {
                name: "Alex".to_string(),
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                gamemode: 0,
                ping: 0,
            }],
        );
        let sent = frames(&mut session);
        let meta = sent
            .iter()
            .find(|(id, _)| *id == profile::packet_id(775, "entity_metadata").unwrap())
            .expect("entity_metadata sent");
        let mut r = Reader::new(&meta.1);
        let _entity = r.read_varint();
        assert_eq!(r.data[r.pos], 2, "custom_name index");
        r.pos += 1;
        assert_eq!(r.read_varint(), Some(6), "optional_component type, not 5");
        assert_eq!(r.data[r.pos], 1, "Optional presence byte");
    }

    #[test]
    fn player_info_update_gamemode_bitflag_is_0x04() {
        let mut session = modern_session(769);
        session.tab_listed.insert(
            lower("Steve"),
            TabEntry {
                name: "Steve".to_string(),
                gamemode: 0,
                ping: 0,
            },
        );
        session.set_gamemode(1);
        let sent = frames(&mut session);
        let info = sent
            .iter()
            .find(|(id, _)| *id == profile::packet_id(769, "player_info").unwrap())
            .expect("player_info_update sent");
        assert_eq!(info.1[0], wire::PLAYER_INFO_UPDATE_GAME_MODE);
        assert_eq!(info.1[0], 0x04);
        // game_state_change reason 3 follows.
        let gsc = sent
            .iter()
            .find(|(id, _)| *id == profile::packet_id(769, "game_state_change").unwrap())
            .expect("game_state_change sent");
        assert_eq!(gsc.1[0], 3);
    }

    #[test]
    fn legacy_spawn_and_nametag() {
        let hooks = StubHooks { snapshots: vec![] };
        let mut session = JavaSession::new(1, "127.0.0.1:25565".parse().unwrap());
        session.protocol = 340;
        session.username = Some("Steve".to_string());
        session.state = SessionState::Play;
        session.playing = true;
        session.sync_entities(
            &hooks,
            &[EntityTarget {
                name: "Alex".to_string(),
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                gamemode: 0,
                ping: 0,
            }],
        );
        let sent = frames(&mut session);
        assert!(sent
            .iter()
            .any(|(id, _)| *id == wire::S2C_NAMED_ENTITY_SPAWN));
        let meta = sent
            .iter()
            .find(|(id, _)| *id == wire::S2C_ENTITY_METADATA)
            .expect("legacy entity_metadata");
        // Legacy: index 2, type 3 (string), name...
        assert_eq!(meta.1[wire::varint(1_000_000).len()], 2);
        assert_eq!(meta.1[wire::varint(1_000_000).len() + 1], 3);
    }

    #[test]
    fn sync_tab_adds_once_and_removes() {
        let mut session = modern_session(769);
        let targets = vec![TabTarget {
            name: "Steve".to_string(),
            gamemode: 0,
            ping: 0,
        }];
        session.sync_tab(&targets);
        let first = frames(&mut session);
        assert_eq!(first.len(), 1, "one player_info_update add");
        assert_eq!(first[0].1[0], 0x01 | 0x04 | 0x08 | 0x10);
        session.sync_tab(&targets);
        assert!(frames(&mut session).is_empty(), "no duplicate add");
        session.sync_tab(&[]);
        let removed = frames(&mut session);
        assert_eq!(removed.len(), 1);
        assert_eq!(
            removed[0].0,
            profile::packet_id(769, "player_remove").unwrap()
        );
    }
}
