use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bedrock_protocol::binary::{CodecError, Reader, Writer};
use bedrock_protocol::{
    AvailableCommandsPacket, BlockPos, ChunkRadiusUpdatedPacket, CommandRequestPacket, CommandSpec,
    DisconnectPacket, LevelChunkPacket, LoginPacket, NetworkChunkPublisherUpdatePacket,
    NetworkSettingsPacket, Packet, PacketBatch, PacketViolationWarningPacket, PlayStatusPacket,
    PlayerAuthInputPacket, RequestAbilityPacket, RequestChunkRadiusPacket,
    RequestNetworkSettingsPacket, ResourcePackClientResponsePacket, ResourcePackStackPacket,
    ResourcePacksInfoPacket, SetPlayerGameTypePacket, SetSpawnPositionPacket, StartGamePacket,
    ActorEventPacket, InventoryContentPacket, InventorySlotPacket, InventoryTransactionPacket,
    ItemStack, RespawnPacket, SetDifficultyPacket, SetHealthPacket, SetTitlePacket, TextPacket,
    TransactionData, TransferPacket,
    UpdateAbilitiesPacket, UpdatePlayerGameTypePacket, Vector3f,
};
use bedrock_protocol::packets::actor_event::EVENT_DEATH_ANIMATION;
use bedrock_protocol::packets::player_auth_input::{
    parse_block_actions, BLOCK_ACTION_PREDICT_DESTROY, BLOCK_ACTION_START_BREAK,
};
use bedrock_world::{block_id::AIR, FlatGenerator, WorldGenerator};
use parking_lot::RwLock;
use php_plugin_bridge::{
    BlockBreakEvent, CommandSender, Event, PlayerChatEvent, PlayerCommandPreprocessEvent,
    PlayerContext, PlayerInteractEvent, PlayerJoinEvent, PlayerMoveEvent, PlayerQuitEvent,
    PluginAction, PluginManager,
};
use rayon::prelude::*;
use thiserror::Error;
use tracing::{debug, error, info, warn};

use crate::command::{
    CommandAction, CommandSource, OnlinePlayer, TitleAction, WeatherState, DEFAULT_WEATHER_TICKS,
};
use crate::identity::parse_login_identity;
use crate::inventory::{
    PlayerInventory, WINDOW_ID_ARMOR, WINDOW_ID_CURSOR, WINDOW_ID_INVENTORY, WINDOW_ID_OFFHAND,
};
use crate::plugin_host::PlayerRegistry;
use crate::static_packets::StaticPackets;
use crate::world_state::{WorldState, WORLD_MAX_Y, WORLD_MIN_Y};

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Protocol codec error: {0}")]
    Codec(#[from] CodecError),
    #[error("Unexpected packet for state {state:?}: packet_id=0x{packet_id:02X}")]
    UnexpectedPacket {
        state: SessionState,
        packet_id: u32,
    },
    #[error("Unsupported client protocol: {0}")]
    UnsupportedProtocol(u32),
    #[error("Session is disconnected")]
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Connected, waiting for RequestNetworkSettings (0xC1) or Login (0x01)
    Connected,
    /// Login received, waiting for ResourcePacksClientResponse (0x08)
    ResourcePacks,
    /// Stack sent, waiting for final ResourcePacksClientResponse COMPLETED
    ResourcePackStack,
    /// In world, spawned, chunks sent
    InGame,
    /// Session closed
    Disconnected,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutgoingPacket {
    NetworkSettings(NetworkSettingsPacket),
    PlayStatus(PlayStatusPacket),
    ResourcePacksInfo(ResourcePacksInfoPacket),
    ResourcePackStack(ResourcePackStackPacket),
    StartGame(StartGamePacket),
    SetSpawnPosition(SetSpawnPositionPacket),
    NetworkChunkPublisherUpdate(NetworkChunkPublisherUpdatePacket),
    ChunkRadiusUpdated(ChunkRadiusUpdatedPacket),
    LevelChunk(LevelChunkPacket),
    Disconnect(DisconnectPacket),
    SetPlayerGameType(SetPlayerGameTypePacket),
    UpdatePlayerGameType(UpdatePlayerGameTypePacket),
    UpdateAbilities(UpdateAbilitiesPacket),
    Text(TextPacket),
    SetTitle(SetTitlePacket),
    Transfer(TransferPacket),
    RawWire(Vec<u8>),
}

impl OutgoingPacket {
    pub fn packet_id(&self) -> u32 {
        match self {
            Self::NetworkSettings(p) => p.packet_id(),
            Self::PlayStatus(p) => p.packet_id(),
            Self::ResourcePacksInfo(p) => p.packet_id(),
            Self::ResourcePackStack(p) => p.packet_id(),
            Self::StartGame(p) => p.packet_id(),
            Self::SetSpawnPosition(p) => p.packet_id(),
            Self::NetworkChunkPublisherUpdate(p) => p.packet_id(),
            Self::ChunkRadiusUpdated(p) => p.packet_id(),
            Self::LevelChunk(p) => p.packet_id(),
            Self::Disconnect(p) => p.packet_id(),
            Self::SetPlayerGameType(p) => p.packet_id(),
            Self::UpdatePlayerGameType(p) => p.packet_id(),
            Self::UpdateAbilities(p) => p.packet_id(),
            Self::Text(p) => p.packet_id(),
            Self::SetTitle(p) => p.packet_id(),
            Self::Transfer(p) => p.packet_id(),
            Self::RawWire(bytes) => {
                let mut reader = Reader::new(bytes);
                reader.read_var_u32().unwrap_or(0) & 0x3FF
            }
        }
    }

    pub fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::NetworkSettings(p) => p.encode(writer),
            Self::PlayStatus(p) => p.encode(writer),
            Self::ResourcePacksInfo(p) => p.encode(writer),
            Self::ResourcePackStack(p) => p.encode(writer),
            Self::StartGame(p) => p.encode(writer),
            Self::SetSpawnPosition(p) => p.encode(writer),
            Self::NetworkChunkPublisherUpdate(p) => p.encode(writer),
            Self::ChunkRadiusUpdated(p) => p.encode(writer),
            Self::LevelChunk(p) => p.encode(writer),
            Self::Disconnect(p) => p.encode(writer),
            Self::SetPlayerGameType(p) => p.encode(writer),
            Self::UpdatePlayerGameType(p) => p.encode(writer),
            Self::UpdateAbilities(p) => p.encode(writer),
            Self::Text(p) => p.encode(writer),
            Self::SetTitle(p) => p.encode(writer),
            Self::Transfer(p) => p.encode(writer),
            Self::RawWire(bytes) => {
                writer.write_bytes(bytes);
                Ok(())
            }
        }
    }

    pub fn to_bytes(&self) -> Result<bytes::Bytes, CodecError> {
        let mut writer = Writer::new();
        self.encode(&mut writer)?;
        Ok(writer.into_bytes())
    }
}

pub struct PlayerSession {
    pub session_id: u64,
    pub client_addr: SocketAddr,
    pub state: SessionState,
    pub protocol_version: u32,
    pub username: String,
    pub xuid: String,
    pub entity_unique_id: i64,
    pub entity_runtime_id: u64,
    pub position: Vector3f,
    pub pitch: f32,
    pub yaw: f32,
    pub gamemode: i32,
    pub is_flying: bool,
    pub chunk_radius: i32,
    pub max_view_distance: i32,
    pub sent_chunks: HashSet<(i32, i32)>,
    pub compression_enabled: bool,
    /// When true (set by teleport()), ignore client-reported position in
    /// PlayerAuthInput until the client confirms the server's position.
    /// Matches PocketMine's `forceMoveSync` to prevent desync after /tp.
    pub force_move_sync: bool,
    pub generator: Arc<dyn WorldGenerator>,
    /// Shared world state (terrain + plugin edits).
    pub world: Arc<WorldState>,
    /// Plugin runtime, when PHP plugins are loaded.
    pub plugins: Option<Arc<PluginManager>>,
    /// Player name registry used by the PHP host bridge.
    pub registry: Option<Arc<PlayerRegistry>>,
    /// Operators, used when building player snapshots.
    pub ops: Arc<RwLock<HashSet<String>>>,
    /// Actions emitted by plugins while handling the current packet.
    pub pending_actions: Vec<PluginAction>,
    /// Packets queued asynchronously (broadcasts, plugin actions).
    pub pending_outgoing: Vec<OutgoingPacket>,
    /// Chat/system lines to broadcast on the next session dispatch.
    pub pending_broadcasts: Vec<String>,
    /// Whether PlayerJoinEvent has already been fired.
    pub joined: bool,
    /// Move-event throttle/statistics.
    pub move_events: u64,
    /// Core-command actions waiting for `SessionManager` to apply them.
    pub pending_core: Vec<CommandAction>,
    /// `allow-cheats` from the server configuration.
    pub allow_cheats: bool,
    /// `max-players` from the server configuration, reported by `/list`.
    pub max_players: i32,
    /// Current health in half-hearts; `0.0` means dead.
    pub health: f32,
    pub max_health: f32,
    /// Set between death and the client's respawn request.
    pub is_dead: bool,
    /// Server-side inventory (36 main slots plus armor, offhand and cursor).
    pub inventory: PlayerInventory,
    /// Respawn position set with `/spawnpoint`; `None` means the world spawn.
    pub spawn_point: Option<(f64, f64, f64)>,
    /// Server ban and whitelist lists, shared with every session.
    pub access: Arc<crate::access::AccessList>,
    /// Directory for `players/<name>.json`; `None` disables persistence.
    pub data_dir: Option<std::path::PathBuf>,
    /// A saved position was loaded and must not be replaced by the generator.
    pub has_saved_data: bool,
    /// Tick sampler, shared with the server tick loop (`/tps`).
    pub metrics: Option<Arc<crate::metrics::TickMetrics>>,
    /// Permission registry and this player's attachments.
    pub permissions: Option<Arc<crate::permission::PermissionManager>>,
}

impl PlayerSession {
    pub fn new(session_id: u64, client_addr: SocketAddr) -> Self {
        let generator: Arc<dyn WorldGenerator> = Arc::new(FlatGenerator::new());
        Self::build(session_id, client_addr, generator, 0)
    }

    pub fn with_generator(
        session_id: u64,
        client_addr: SocketAddr,
        generator: Arc<dyn WorldGenerator>,
    ) -> Self {
        Self::with_generator_and_gamemode(session_id, client_addr, generator, 0)
    }

    pub fn with_generator_and_gamemode(
        session_id: u64,
        client_addr: SocketAddr,
        generator: Arc<dyn WorldGenerator>,
        gamemode: i32,
    ) -> Self {
        Self::build(session_id, client_addr, generator, gamemode)
    }

    fn build(
        session_id: u64,
        client_addr: SocketAddr,
        generator: Arc<dyn WorldGenerator>,
        gamemode: i32,
    ) -> Self {
        let (px, py, pz) = generator.player_position();
        let world = Arc::new(WorldState::new("world", Arc::clone(&generator)));
        Self {
            session_id,
            client_addr,
            state: SessionState::Connected,
            protocol_version: 0,
            username: format!("Player_{}", session_id),
            xuid: String::new(),
            entity_unique_id: session_id as i64,
            entity_runtime_id: session_id,
            position: Vector3f::new(px, py, pz),
            pitch: 0.0,
            yaw: 0.0,
            gamemode,
            is_flying: false,
            chunk_radius: 8,
            max_view_distance: 12,
            sent_chunks: HashSet::new(),
            compression_enabled: false,
            force_move_sync: false,
            generator,
            world,
            plugins: None,
            registry: None,
            ops: Arc::new(RwLock::new(HashSet::new())),
            pending_actions: Vec::new(),
            pending_outgoing: Vec::new(),
            pending_broadcasts: Vec::new(),
            joined: false,
            move_events: 0,
            pending_core: Vec::new(),
            allow_cheats: true,
            max_players: 1000,
            health: 20.0,
            max_health: 20.0,
            is_dead: false,
            inventory: PlayerInventory::new(),
            spawn_point: None,
            access: Arc::new(crate::access::AccessList::new()),
            data_dir: None,
            has_saved_data: false,
            metrics: None,
            permissions: None,
        }
    }

    /// Attaches the shared world, plugin runtime and player registry.
    pub fn attach_runtime(
        &mut self,
        world: Arc<WorldState>,
        plugins: Option<Arc<PluginManager>>,
        registry: Arc<PlayerRegistry>,
        ops: Arc<RwLock<HashSet<String>>>,
    ) {
        self.world = world;
        self.plugins = plugins;
        self.registry = Some(registry);
        self.ops = ops;
    }

    /// Shares the server ban/whitelist lists with this session.
    pub fn attach_access(&mut self, access: Arc<crate::access::AccessList>) {
        self.access = access;
    }

    /// Shares the tick sampler used by `/tps`.
    pub fn attach_metrics(&mut self, metrics: Arc<crate::metrics::TickMetrics>) {
        self.metrics = Some(metrics);
    }

    /// Shares the permission registry used to gate commands.
    pub fn attach_permissions(&mut self, permissions: Arc<crate::permission::PermissionManager>) {
        self.permissions = Some(permissions);
    }

    /// `players/<name>.json` for this player, or `None` without persistence.
    pub fn player_data_path(&self) -> Option<std::path::PathBuf> {
        let dir = self.data_dir.as_ref()?;
        Some(
            dir.join("players")
                .join(format!("{}.json", self.username.to_lowercase())),
        )
    }

    /// Writes position, rotation, gamemode, health and spawn point.
    ///
    /// PocketMine uses `players/<name>.dat` (NBT); JSON keeps the same data
    /// without an NBT serializer.
    pub fn save_player_data(&self) -> std::io::Result<()> {
        let Some(path) = self.player_data_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let value = serde_json::json!({
            "name": self.username,
            "x": self.position.x as f64,
            "y": self.position.y as f64,
            "z": self.position.z as f64,
            "yaw": self.yaw,
            "pitch": self.pitch,
            "gamemode": self.gamemode,
            "health": self.health,
            "spawn_point": self.spawn_point.map(|(x, y, z)| vec![x, y, z]),
        });
        let bytes = serde_json::to_vec(&value)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
        std::fs::write(path, bytes)
    }

    /// Restores a snapshot written by [`PlayerSession::save_player_data`].
    pub fn load_player_data(&mut self) -> bool {
        let Some(path) = self.player_data_path() else {
            return false;
        };
        let Ok(bytes) = std::fs::read(path) else {
            return false;
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return false;
        };
        let number = |key: &str| value.get(key).and_then(|entry| entry.as_f64());

        if let (Some(x), Some(y), Some(z)) = (number("x"), number("y"), number("z")) {
            self.position = Vector3f::new(x as f32, y as f32, z as f32);
            self.has_saved_data = true;
        }
        if let Some(yaw) = number("yaw") {
            self.yaw = yaw as f32;
        }
        if let Some(pitch) = number("pitch") {
            self.pitch = pitch as f32;
        }
        if let Some(gamemode) = value.get("gamemode").and_then(|entry| entry.as_i64()) {
            self.gamemode = gamemode as i32;
        }
        if let Some(health) = number("health") {
            self.health = health as f32;
        }
        if let Some(point) = value.get("spawn_point").and_then(|entry| entry.as_array()) {
            let coords: Vec<f64> = point
                .iter()
                .filter_map(|entry| entry.as_f64())
                .collect();
            if coords.len() == 3 {
                self.spawn_point = Some((coords[0], coords[1], coords[2]));
            }
        }
        true
    }

    pub fn is_op(&self) -> bool {
        self.ops.read().contains(&self.username.to_lowercase())
    }

    /// Snapshot passed to PHP plugins.
    pub fn player_context(&self) -> PlayerContext {
        PlayerContext {
            name: self.username.clone(),
            display_name: self.username.clone(),
            address: self.client_addr,
            runtime_id: self.entity_runtime_id,
            unique_id: self.entity_unique_id,
            ping: 0,
            gamemode: self.gamemode.max(0) as u32,
            x: self.position.x as f64,
            y: self.position.y as f64,
            z: self.position.z as f64,
            yaw: self.yaw as f64,
            pitch: self.pitch as f64,
            world: self.world.name.clone(),
            health: self.health,
            max_health: self.max_health,
            is_op: self.is_op(),
        }
    }

    /// Snapshot of who ran the command currently being processed.
    pub fn command_source(&self) -> CommandSource {
        CommandSource {
            name: self.username.clone(),
            position: (
                self.position.x as f64,
                self.position.y as f64,
                self.position.z as f64,
            ),
            is_op: self.is_op(),
            allow_cheats: self.allow_cheats,
            is_console: false,
            max_players: self.max_players,
            world_time: self.world.time(),
            world_seed: self.world.seed(),
            world_spawn: {
                let (x, y, z) = self.world.spawn();
                (x as f64, y as f64, z as f64)
            },
            access: Some(Arc::clone(&self.access)),
            plugins: self
                .plugins
                .as_ref()
                .map(|manager| manager.load_order())
                .unwrap_or_default(),
            tps: self
                .metrics
                .as_ref()
                .map(|metrics| metrics.current())
                .unwrap_or((20.0, 20.0, 20.0)),
            permissions: self.permissions.clone(),
        }
    }

    /// Online players known by the registry, used to resolve command targets.
    ///
    /// The sender is always included, so `/tp <name>` still resolves before the
    /// plugin registry has caught up (for example right after joining).
    pub fn online_players(&self) -> Vec<OnlinePlayer> {
        let mut players: Vec<OnlinePlayer> = self
            .registry
            .as_ref()
            .map(|registry| {
                registry
                    .all()
                    .into_iter()
                    .map(|player| OnlinePlayer {
                        name: player.name,
                        position: (player.x, player.y, player.z),
                        gamemode: player.gamemode as i32,
                    })
                    .collect()
            })
            .unwrap_or_default();

        if !players
            .iter()
            .any(|player| player.name.eq_ignore_ascii_case(&self.username))
        {
            players.push(OnlinePlayer {
                name: self.username.clone(),
                position: (
                    self.position.x as f64,
                    self.position.y as f64,
                    self.position.z as f64,
                ),
                gamemode: self.gamemode,
            });
        }

        players
    }

    /// Authoritative teleport: moves the player and tells the client.
    ///
    /// Chunks around the destination are NOT sent here. They will be sent
    /// naturally when the client confirms the new position via PlayerAuthInput
    /// (handle_player_auth_input). Sending chunks immediately would race with
    /// the client's current position and cause protocol violations / bad-packet
    /// disconnects, because the client hasn't actually moved yet.
    pub fn teleport(&mut self, x: f64, y: f64, z: f64, yaw: Option<f32>, pitch: Option<f32>) {
        let y = y.clamp(WORLD_MIN_Y as f64, WORLD_MAX_Y as f64);
        self.position = Vector3f::new(x as f32, y as f32, z as f32);
        // Canonical Bedrock range (see `java_sync`): yaw folds to
        // `[0, 360)` and pitch to `(-180, 180]`, matching the values
        // a Java (virtual) player publishes in the registry — the PHP
        // bridge does the inverse `+180` dance in `syncTeleport`.
        if let Some(yaw) = yaw {
            self.yaw = crate::java_sync::normalize_bedrock_yaw(yaw);
        }
        if let Some(pitch) = pitch {
            self.pitch = crate::java_sync::bedrock_pitch_to_java(pitch);
        }
        if self.gamemode != 1 {
            self.is_flying = false;
        }
        let packet = self.create_move_player_packet(2, yaw, pitch);
        self.pending_outgoing.push(OutgoingPacket::RawWire(packet));
        self.register_self();
        // Mark that a teleport is pending so we ignore client-reported
        // position until the client confirms the server's position.
        // Matches PocketMine's forceMoveSync.
        self.force_move_sync = true;
    }

    fn register_self(&self) {
        if let Some(registry) = &self.registry {
            registry.upsert(self.player_context());
        }
    }

    /// Dispatches an event to native and PHP listeners, collecting actions.
    pub fn fire_event(&mut self, event: &mut dyn Event) -> Vec<PluginAction> {
        let Some(plugins) = self.plugins.clone() else {
            return Vec::new();
        };
        match plugins.dispatch_event_sync(event) {
            Ok(actions) => {
                self.pending_actions.extend(actions.iter().cloned());
                actions
            }
            Err(error) => {
                warn!("Plugin event '{}' failed: {}", event.name(), error);
                Vec::new()
            }
        }
    }

    pub fn is_in_game(&self) -> bool {
        self.state == SessionState::InGame
    }

    pub fn is_disconnected(&self) -> bool {
        self.state == SessionState::Disconnected
    }

    /// Primary dispatch method: processes an incoming raw Bedrock packet payload by ID.
    pub fn handle_packet(
        &mut self,
        packet_id: u32,
        payload: &[u8],
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        if self.is_disconnected() {
            return Err(SessionError::Disconnected);
        }

        match packet_id {
            0xC1 => {
                let mut reader = Reader::new(payload);
                let packet = RequestNetworkSettingsPacket::decode(&mut reader)?;
                self.handle_network_settings_request(packet)
            }
            0x01 => {
                let mut reader = Reader::new(payload);
                let packet = LoginPacket::decode(&mut reader)?;
                self.handle_login(packet)
            }
            0x08 => {
                let mut reader = Reader::new(payload);
                let packet = ResourcePackClientResponsePacket::decode(&mut reader)?;
                self.handle_resource_pack_response(packet)
            }
            0x45 => {
                let mut reader = Reader::new(payload);
                let packet = RequestChunkRadiusPacket::decode(&mut reader)?;
                self.handle_chunk_radius_request(packet)
            }
            0x90 => {
                let mut reader = Reader::new(payload);
                let packet = PlayerAuthInputPacket::decode(&mut reader)?;
                self.handle_player_auth_input(packet)
            }
            0x09 => {
                let mut reader = Reader::new(payload);
                let packet = TextPacket::decode(&mut reader)?;
                self.handle_text(packet)
            }
            0x1E => {
                let mut reader = Reader::new(payload);
                match InventoryTransactionPacket::decode_payload(&mut reader) {
                    Ok(packet) => Ok(self.handle_inventory_transaction(packet)),
                    Err(error) => {
                        warn!(
                            "Failed to decode InventoryTransactionPacket from session {}: {}",
                            self.session_id, error
                        );
                        Ok(self.resync_inventory())
                    }
                }
            }
            0x2D => {
                let mut reader = Reader::new(payload);
                let packet = RespawnPacket::decode_payload(&mut reader)?;
                if packet.state == RespawnPacket::CLIENT_READY_TO_SPAWN {
                    Ok(self.handle_respawn())
                } else {
                    Ok(Vec::new())
                }
            }
            0x4D => {
                let mut reader = Reader::new(payload);
                match CommandRequestPacket::decode(&mut reader) {
                    Ok(packet) => self.handle_command_request(packet),
                    Err(error) => {
                        warn!(
                            "Failed to decode CommandRequestPacket from session {}: {}",
                            self.session_id, error
                        );
                        Ok(Vec::new())
                    }
                }
            }
            0x3E => {
                let mut reader = Reader::new(payload);
                let packet = SetPlayerGameTypePacket::decode(&mut reader)?;
                self.handle_set_player_game_type(packet)
            }
            0xB8 => {
                let mut reader = Reader::new(payload);
                let packet = RequestAbilityPacket::decode(&mut reader)?;
                self.handle_request_ability(packet)
            }
            0x9C => {
                let mut reader = Reader::new(payload);
                match PacketViolationWarningPacket::decode(&mut reader) {
                    Ok(p) => {
                        error!(
                            "[:WebRTC] >>> CLIENT PACKET VIOLATION (0x9C) from session {}: type={}, severity={}, packet_id=0x{:02X}, reason='{}'",
                            self.session_id, p.violation_type, p.severity, p.packet_id, p.message
                        );
                    }
                    Err(e) => {
                        error!("[:WebRTC] Failed to decode 0x9C packet violation from session {}: {}", self.session_id, e);
                    }
                }
                Ok(Vec::new())
            }
            _ => {
                debug!(
                    "Unhandled packet ID 0x{:02X} (len {}) from session {}",
                    packet_id,
                    payload.len(),
                    self.session_id
                );
                Ok(Vec::new())
            }
        }
    }

    /// Handles RequestNetworkSettingsPacket (0xC1).
    pub fn handle_network_settings_request(
        &mut self,
        packet: RequestNetworkSettingsPacket,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        debug!(
            "Session {} requested network settings (client protocol: {})",
            self.session_id, packet.client_protocol_version
        );

        let response = NetworkSettingsPacket {
            compression_threshold: 512,
            compression_algorithm: 0, // 0 = Deflate (Standard Bedrock)
            client_throttle: false,
            client_throttle_threshold: 0,
            client_throttle_scalar: 0.0,
        };

        Ok(vec![OutgoingPacket::NetworkSettings(response)])
    }

    /// Handles LoginPacket (0x01).
    pub fn handle_login(
        &mut self,
        packet: LoginPacket,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        if self.state != SessionState::Connected {
            return Err(SessionError::UnexpectedPacket {
                state: self.state,
                packet_id: 0x01,
            });
        }

        self.protocol_version = packet.protocol_version;
        let identity = parse_login_identity(&packet.connection_request);
        self.username = identity.display_name.clone();
        let _ = self.load_player_data();
        self.xuid = identity.xuid.clone();
        if let Some(xuid) = identity.xuid.parse::<i64>().ok() {
            self.entity_unique_id = xuid;
        } else {
            self.entity_unique_id = identity.unique_id();
        }
        info!(
            "Session {} login attempt with protocol version {} as '{}' (xuid: {})",
            self.session_id, self.protocol_version, self.username, self.xuid
        );

        // Transition state to ResourcePacks
        self.state = SessionState::ResourcePacks;

        let play_status = PlayStatusPacket {
            status: PlayStatusPacket::LOGIN_SUCCESS,
        };
        let packs_info = ResourcePacksInfoPacket {
            must_accept: false,
            has_addons: false,
            has_scripts: false,
            disable_vibrant_visuals: false,
            world_template_uuid: [0u8; 16],
            world_template_version: String::new(),
            behavior_packs: Vec::new(),
            resource_packs: Vec::new(),
        };

        Ok(vec![
            OutgoingPacket::PlayStatus(play_status),
            OutgoingPacket::ResourcePacksInfo(packs_info),
        ])
    }

    /// Handles ResourcePackClientResponsePacket (0x08).
    pub fn handle_resource_pack_response(
        &mut self,
        packet: ResourcePackClientResponsePacket,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        match packet.response_status {
            0 => {
                info!("Session {} refused resource packs; disconnecting.", self.session_id);
                self.state = SessionState::Disconnected;
                Ok(vec![OutgoingPacket::Disconnect(DisconnectPacket {
                    reason: 0,
                    skip_message: false,
                    message: Some("Resource packs were refused.".to_string()),
                })])
            }
            1 => {
                // Client requests packs downloading
                Ok(Vec::new())
            }
            2 | 3 if self.state == SessionState::ResourcePacks => {
                self.state = SessionState::ResourcePackStack;
                let stack = ResourcePackStackPacket {
                    must_accept: false,
                    behavior_packs: Vec::new(),
                    resource_packs: Vec::new(),
                    game_version: "1.26.60".to_string(),
                    experiments_previously_enabled: false,
                    has_editor_packs: false,
                };
                info!("Session {} accepted packs, sending ResourcePackStack", self.session_id);
                Ok(vec![OutgoingPacket::ResourcePackStack(stack)])
            }
            3 | 4 if self.state == SessionState::ResourcePackStack || self.state == SessionState::ResourcePacks => {
                self.state = SessionState::InGame;
                info!("Session {} entered world (InGame) - sending pre-spawn declaration and world packets", self.session_id);

                let mut out = Vec::new();

                // 1. JigsawStructureDataPacket (0x139 / 313) - Required before StartGame in 1.21+ / 1.26
                out.push(OutgoingPacket::RawWire(StaticPackets::JIGSAW_STRUCTURE_DATA.to_vec()));

                // 2. VoxelShapesPacket (0x151 / 337) - Required before StartGame in 1.21+ / 1.26
                out.push(OutgoingPacket::RawWire(StaticPackets::VOXEL_SHAPES.to_vec()));

                // 3. StartGamePacket (0x0B / 11) - Core world configuration
                let mut start_game = StartGamePacket::default();
                start_game.actor_unique_id = self.entity_unique_id;
                start_game.actor_runtime_id = self.entity_runtime_id;
                start_game.player_gamemode = self.gamemode;
                if !self.has_saved_data {
                    let (px, py, pz) = self.generator.player_position();
                    self.position = Vector3f::new(px, py, pz);
                }
                start_game.player_position = self.position;
                start_game.level_settings.seed = 12345678;
                let (sx, sy, sz) = self.world.spawn();
                start_game.level_settings.spawn_position = BlockPos::new(sx, sy, sz);
                start_game.level_settings.generator_type = self.generator.generator_type();
                start_game.level_settings.world_gamemode = self.gamemode;
                start_game.level_settings.difficulty = 1;
                start_game.level_settings.dimension = 0;
                start_game.world_name = "PocketMine-Rust World".to_string();
                out.push(OutgoingPacket::StartGame(start_game));

                // 4. ItemRegistryPacket (0xA2 / 162) - Dynamic session item dictionary
                out.push(OutgoingPacket::RawWire(StaticPackets::ITEM_REGISTRY.to_vec()));

                // 5. AvailableActorIdentifiersPacket (0x77 / 119) - Entity definitions
                out.push(OutgoingPacket::RawWire(StaticPackets::AVAILABLE_ENTITY_IDENTIFIERS.to_vec()));

                // 6. BiomeDefinitionListPacket (0x7A / 122) - World biomes
                out.push(OutgoingPacket::RawWire(StaticPackets::BIOME_DEFINITION_LIST.to_vec()));

                // 7. TrimDataPacket (0x12E / 302)
                out.push(OutgoingPacket::RawWire(StaticPackets::TRIM_DATA.to_vec()));

                // 8. UpdateAttributesPacket (0x1D / 29) - Health, hunger, movement speed
                out.push(OutgoingPacket::RawWire(self.create_update_attributes_packet()));

                // 8b. SetHealthPacket (0x2A) and the 36-slot inventory, so the
                // HUD and the item bar start from the server state.
                out.push(OutgoingPacket::RawWire(set_health_wire(self.health)));
                out.push(OutgoingPacket::RawWire(inventory_content_wire(
                    &self.inventory.contents(),
                )));

                // 9. SetActorDataPacket (0x27 / 39) - Collision, gravity, scale, air=300
                out.push(OutgoingPacket::RawWire(self.create_set_actor_data_packet()));

                // 10. UpdateAbilitiesPacket (0xBB / 187) - Survival abilities, flying=false
                out.push(OutgoingPacket::RawWire(self.create_update_abilities_packet()));

                // 10b. UpdateAdventureSettingsPacket (0xBC / 188)
                out.push(OutgoingPacket::RawWire(StaticPackets::UPDATE_ADVENTURE_SETTINGS.to_vec()));

                // 10c. SetCommandsEnabledPacket (0x3B / 59)
                out.push(OutgoingPacket::RawWire(StaticPackets::SET_COMMANDS_ENABLED.to_vec()));

                // 10c2. AvailableCommandsPacket (0x4C / 76): registers the
                // native commands so the client lists and autocompletes them.
                out.push(OutgoingPacket::RawWire(available_commands_wire()));

                // 10d. SetDifficultyPacket (0x3C / 60)
                out.push(OutgoingPacket::RawWire(StaticPackets::SET_DIFFICULTY.to_vec()));

                // 10e. GameRulesChangedPacket (0x48 / 72)
                out.push(OutgoingPacket::RawWire(StaticPackets::GAME_RULES_CHANGED.to_vec()));

                // 11. SetSpawnPositionPacket (0x2B / 43)
                let spawn_pos = BlockPos::new(sx, sy, sz);
                out.push(OutgoingPacket::SetSpawnPosition(
                    SetSpawnPositionPacket::new_player_spawn(spawn_pos, 0),
                ));

                // 11b. World time and weather, so /time and /weather survive
                // every new join.
                out.push(OutgoingPacket::RawWire(set_time_wire(self.world.time())));
                if let Some(state) = WeatherState::from_i32(self.world.weather()) {
                    if state != WeatherState::Clear {
                        for event_id in state.level_events() {
                            out.push(OutgoingPacket::RawWire(level_event_wire(
                                *event_id,
                                DEFAULT_WEATHER_TICKS,
                            )));
                        }
                    }
                }

                // 12. NetworkChunkPublisherUpdatePacket (0x79 / 121)
                let initial_radius_blocks = (((self.chunk_radius as f64 * std::f64::consts::SQRT_2) + 2.0).ceil() as u32) * 16;
                out.push(OutgoingPacket::NetworkChunkPublisherUpdate(
                    NetworkChunkPublisherUpdatePacket::new(spawn_pos, initial_radius_blocks.max(128)),
                ));

                // 13. Create initial 5x5 chunks around spawn
                let spawn_cx = sx >> 4;
                let spawn_cz = sz >> 4;
                for cx in (spawn_cx - 2)..=(spawn_cx + 2) {
                    for cz in (spawn_cz - 2)..=(spawn_cz + 2) {
                        self.sent_chunks.insert((cx, cz));
                        out.push(OutgoingPacket::LevelChunk(self.create_chunk_packet(cx, cz)));
                    }
                }

                // 14. Confirm spawn to client (PLAYER_SPAWN = 3)
                let play_status = PlayStatusPacket {
                    status: PlayStatusPacket::PLAYER_SPAWN,
                };
                out.push(OutgoingPacket::PlayStatus(play_status));

                // 15. PlayerJoinEvent (fires plugin listeners once the client is
                // actually in the world and can receive messages).
                if !self.joined {
                    self.joined = true;
                    self.register_self();
                    let mut event = PlayerJoinEvent::new(
                        self.player_context(),
                        format!("{} joined the game", self.username),
                    );
                    self.fire_event(&mut event);
                    if !event.join_message.is_empty() {
                        self.pending_broadcasts.push(event.join_message);
                    }
                }

                Ok(out)
            }
            other => {
                debug!("Session {} sent unknown resource pack status: {}", self.session_id, other);
                Ok(Vec::new())
            }
        }
    }

    /// Handles RequestChunkRadiusPacket (0x45).
    pub fn handle_chunk_radius_request(
        &mut self,
        packet: RequestChunkRadiusPacket,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        if self.state != SessionState::InGame {
            return Err(SessionError::UnexpectedPacket {
                state: self.state,
                packet_id: 0x45,
            });
        }

        let clamped_radius = packet.radius.clamp(4, self.max_view_distance);
        self.chunk_radius = clamped_radius;
        tracing::info!(
            "Session {} handle_chunk_radius_request: requested={}, clamped={}",
            self.session_id, packet.radius, clamped_radius
        );

        let center_pos = BlockPos::new(
            self.position.x.floor() as i32,
            self.position.y.floor() as i32,
            self.position.z.floor() as i32,
        );

        let publisher_radius_blocks = (((clamped_radius as f64 * std::f64::consts::SQRT_2) + 2.0).ceil() as u32) * 16;

        let mut out = vec![
            OutgoingPacket::ChunkRadiusUpdated(ChunkRadiusUpdatedPacket::new(clamped_radius)),
            OutgoingPacket::NetworkChunkPublisherUpdate(
                NetworkChunkPublisherUpdatePacket::new(center_pos, publisher_radius_blocks),
            ),
        ];

        // Send all chunks in the square around player so all 360 degrees (North, South, East, West)
        // are 100% loaded immediately at spawn, avoiding directional starvation or blank horizons.
        // Send in small batches (max 8 per response) to avoid overwhelming the WebRTC channel
        // and consuming too much CPU on chunk generation, which would lower TPS.
        const MAX_CHUNKS_PER_RESPONSE: usize = 8;

        let curr_cx = (self.position.x.floor() as i32) >> 4;
        let curr_cz = (self.position.z.floor() as i32) >> 4;
        let mut chunks_to_generate = Vec::new();
        for dx in -clamped_radius..=clamped_radius {
            for dz in -clamped_radius..=clamped_radius {
                let cx = curr_cx + dx;
                let cz = curr_cz + dz;
                if !self.sent_chunks.contains(&(cx, cz)) {
                    chunks_to_generate.push((cx, cz));
                }
            }
        }

        chunks_to_generate.sort_by_key(|&(cx, cz)| {
            (cx - curr_cx).pow(2) + (cz - curr_cz).pow(2)
        });

        let to_send = chunks_to_generate.len().min(MAX_CHUNKS_PER_RESPONSE);
        tracing::info!(
            "Session {} sending {} of {} pending chunks (radius={})",
            self.session_id, to_send, chunks_to_generate.len(), clamped_radius
        );

        // Mark chunks as sent first (synchronous, fast), then generate
        // their network payloads in parallel across all CPU cores using rayon.
        // This prevents chunk generation from blocking the WebRTC network thread
        // and stealing CPU from the 20 TPS tick loop.
        let chunks_to_send: Vec<(i32, i32)> =
            chunks_to_generate.into_iter().take(to_send).collect();
        for (cx, cz) in &chunks_to_send {
            self.sent_chunks.insert((*cx, *cz));
        }

        let world = Arc::clone(&self.world);
        let packets: Vec<LevelChunkPacket> = chunks_to_send
            .into_par_iter()
            .map(|(cx, cz)| {
                let (sub_chunk_count, payload) = world.chunk_payload(cx, cz);
                LevelChunkPacket::new(
                    cx,
                    cz,
                    0, // dimension: Overworld
                    sub_chunk_count,
                    None, // sub_chunk_request_limit
                    false, // cache_enabled
                    Vec::new(), // used_blob_hashes
                    payload,
                    false, // client_biome_update
                )
            })
            .collect();

        for packet in packets {
            out.push(OutgoingPacket::LevelChunk(packet));
        }

        Ok(out)
    }

    /// Handles PlayerAuthInputPacket (0x90).
    pub fn handle_player_auth_input(
        &mut self,
        packet: PlayerAuthInputPacket,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        if self.state != SessionState::InGame {
            return Ok(Vec::new());
        }

        // If a teleport is pending (force_move_sync), ignore the client's
        // reported position until it matches the server's position. This
        // matches PocketMine's forceMoveSync and prevents desync after /tp.
        if self.force_move_sync {
            let client_pos = packet.position;
            let server_pos = self.position;
            let dx = (client_pos.x - server_pos.x).abs();
            let dy = (client_pos.y - server_pos.y).abs();
            let dz = (client_pos.z - server_pos.z).abs();
            // Client has confirmed the teleported position.
            if dx < 0.5 && dy < 0.5 && dz < 0.5 {
                self.force_move_sync = false;
                // Fall through to normal handling with server position.
            } else {
                // Client hasn't confirmed yet: keep server position and
                // re-send MovePlayerPacket to correct the client.
                // Do NOT generate or stream chunks until the client confirms
                // the teleport, otherwise the client gets flooded with chunks
                // for a position it hasn't reached yet and disconnects.
                let packet = self.create_move_player_packet(2, None, None);
                self.pending_outgoing.push(OutgoingPacket::RawWire(packet));
                return Ok(Vec::new());
            }
        }

        let old_cx = (self.position.x.floor() as i32) >> 4;
        let old_cz = (self.position.z.floor() as i32) >> 4;

        let from = (
            self.position.x as f64,
            self.position.y as f64,
            self.position.z as f64,
        );
        let to = (
            packet.position.x as f64,
            packet.position.y as f64,
            packet.position.z as f64,
        );

        // PlayerMoveEvent is only dispatched when a plugin actually listens to
        // it, and throttled to every other input to keep the IPC round-trips
        // bounded. Cancelling restores the previous position.
        if self
            .plugins
            .as_ref()
            .map(|plugins| plugins.has_listeners("PlayerMoveEvent"))
            .unwrap_or(false)
            && ((from.0 - to.0).abs() > 0.0001
                || (from.1 - to.1).abs() > 0.0001
                || (from.2 - to.2).abs() > 0.0001)
        {
            self.move_events = self.move_events.wrapping_add(1);
            if self.move_events % 2 == 0 {
                let mut event = PlayerMoveEvent::new(self.player_context(), from, to);
                self.fire_event(&mut event);
                if event.cancelled {
                    return Ok(Vec::new());
                }
            }
        }

        // Only accept client position if teleport is not pending, or if the
        // client has already confirmed the server's position (force_move_sync
        // was cleared above). Otherwise keep the server's teleported position.
        if !self.force_move_sync {
            self.position = packet.position;
            self.pitch = packet.pitch;
            self.yaw = packet.yaw;
        }
        self.register_self();

        let radius = self.chunk_radius;
        let curr_cx = (self.position.x.floor() as i32) >> 4;
        let curr_cz = (self.position.z.floor() as i32) >> 4;
        let moved = curr_cx != old_cx || curr_cz != old_cz;

        // Prune chunks that are far away from the player (beyond radius + 12)
        // so that if the player returns to previously visited areas, they will be resent,
        // preventing empty transparent holes/voids when backtracking.
        if moved {
            let prune_margin = radius + 12;
            self.sent_chunks.retain(|&(cx, cz)| {
                (cx - curr_cx).abs() <= prune_margin && (cz - curr_cz).abs() <= prune_margin
            });
        }

        let mut out = Vec::new();
        let max_chunks_per_packet = 8;

        let mut chunks_to_generate = Vec::new();
        for cx in (curr_cx - radius)..=(curr_cx + radius) {
            for cz in (curr_cz - radius)..=(curr_cz + radius) {
                if !self.sent_chunks.contains(&(cx, cz)) {
                    chunks_to_generate.push((cx, cz));
                }
            }
        }

        // Prioritize generating chunks closest to the player
        chunks_to_generate.sort_by_key(|&(cx, cz)| {
            (cx - curr_cx).pow(2) + (cz - curr_cz).pow(2)
        });

        if moved || !chunks_to_generate.is_empty() {
            tracing::info!(
                "Session {} AuthInput: pos=({:.1}, {:.1}, {:.1}) chunk=({}, {}) old=({}, {}) radius={} chunks_to_gen={} sent_count={}",
                self.session_id, packet.position.x, packet.position.y, packet.position.z,
                curr_cx, curr_cz, old_cx, old_cz, radius,
                chunks_to_generate.len(), self.sent_chunks.len()
            );
        }

        // Publisher radius in blocks MUST encompass the entire square area (diagonal = radius * sqrt(2))
        // plus an additional margin so Bedrock client NEVER discards any boundary chunk!
        let publisher_radius_blocks = (((radius as f64 * std::f64::consts::SQRT_2) + 2.0).ceil() as u32) * 16;

        // CRITICAL: NetworkChunkPublisherUpdate MUST be added to `out` FIRST, BEFORE any LevelChunk packets!
        // In Bedrock, the client discards any chunk outside its current publisher radius.
        // Putting the update packet first ensures the client updates its publisher area BEFORE receiving chunks.
        if !chunks_to_generate.is_empty() || moved {
            let center_pos = BlockPos::new(
                self.position.x.floor() as i32,
                self.position.y.floor() as i32,
                self.position.z.floor() as i32,
            );
            out.push(OutgoingPacket::NetworkChunkPublisherUpdate(
                NetworkChunkPublisherUpdatePacket::new(center_pos, publisher_radius_blocks),
            ));
        }

        let chunks_to_send: Vec<(i32, i32)> = chunks_to_generate
            .into_iter()
            .take(max_chunks_per_packet)
            .collect();
        for (cx, cz) in &chunks_to_send {
            self.sent_chunks.insert((*cx, *cz));
        }

        // Generate chunk payloads in parallel across all CPU cores.
        let world = Arc::clone(&self.world);
        let packets: Vec<LevelChunkPacket> = chunks_to_send
            .into_par_iter()
            .map(|(cx, cz)| {
                let (sub_chunk_count, payload) = world.chunk_payload(cx, cz);
                LevelChunkPacket::new(
                    cx,
                    cz,
                    0,
                    sub_chunk_count,
                    None,
                    false,
                    Vec::new(),
                    payload,
                    false,
                )
            })
            .collect();

        for packet in packets {
            out.push(OutgoingPacket::LevelChunk(packet));
        }

        // Block break / interact actions ride in the input_data tail behind
        // the movement fields. Handling runs after chunk streaming so a
        // cancelled break can resend its chunk without it being regenerated
        // above. Parsing is best-effort and never affects movement.
        if self
            .plugins
            .as_ref()
            .map(|plugins| {
                plugins.has_listeners("BlockBreakEvent")
                    || plugins.has_listeners("PlayerInteractEvent")
            })
            .unwrap_or(false)
        {
            self.handle_block_actions(&packet.input_data);
        }

        Ok(out)
    }

    /// Fires block events from `PlayerAuthInput` block actions.
    ///
    /// - `START_BREAK` fires a cancellable `PlayerInteractEvent`
    ///   (left click). Nothing is applied, so cancellation needs no undo.
    /// - `PREDICT_DESTROY_BLOCK` fires a cancellable `BlockBreakEvent` with
    ///   the current block id. When not cancelled the break is applied
    ///   authoritatively (air edit queued as a manager-level `SetBlock` so
    ///   every viewer gets the chunk refresh); when cancelled the chunk is
    ///   resent to this session to undo the client's prediction.
    /// - `BlockPlaceEvent` is intentionally not fired here: placement
    ///   travels in `InventoryTransactionPacket`, which this server does not
    ///   decode yet.
    fn handle_block_actions(&mut self, input_data: &[u8]) {
        let actions = parse_block_actions(input_data);
        if actions.is_empty() {
            return;
        }
        for action in actions {
            match action.action_type {
                BLOCK_ACTION_START_BREAK => {
                    let Some((x, y, z)) = action.position else {
                        continue;
                    };
                    let mut event = PlayerInteractEvent::new(
                        self.player_context(),
                        self.world.name.clone(),
                        x,
                        y,
                        z,
                        PlayerInteractEvent::ACTION_LEFT_CLICK,
                    );
                    self.fire_event(&mut event);
                }
                BLOCK_ACTION_PREDICT_DESTROY => {
                    let Some((x, y, z)) = action.position else {
                        continue;
                    };
                    let block_id = self.world.block_at(x, y, z);
                    let mut event = BlockBreakEvent::new(
                        self.player_context(),
                        self.world.name.clone(),
                        x,
                        y,
                        z,
                        block_id,
                    );
                    self.fire_event(&mut event);
                    if event.cancelled {
                        let chunk = (x >> 4, z >> 4);
                        if self.sent_chunks.remove(&chunk) {
                            self.pending_outgoing.push(OutgoingPacket::LevelChunk(
                                self.create_chunk_packet(chunk.0, chunk.1),
                            ));
                        }
                    } else {
                        self.pending_actions.push(PluginAction::SetBlock {
                            world: self.world.name.clone(),
                            x,
                            y,
                            z,
                            block_id: AIR,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    /// Handles TextPacket (0x09) for chat and in-game slash commands.
    pub fn handle_text(&mut self, packet: TextPacket) -> Result<Vec<OutgoingPacket>, SessionError> {
        let mut trimmed = packet.message.trim().to_string();

        if trimmed.starts_with('/') {
            // 1. PlayerCommandPreprocessEvent (cancellable, message mutable).
            if self
                .plugins
                .as_ref()
                .map(|plugins| plugins.has_listeners("PlayerCommandPreprocessEvent"))
                .unwrap_or(false)
            {
                let mut event =
                    PlayerCommandPreprocessEvent::new(self.player_context(), trimmed.clone());
                self.fire_event(&mut event);
                if event.cancelled {
                    return Ok(Vec::new());
                }
                trimmed = event.message;
            }

            // 2. Built-in core commands (tp, gamemode, spawn, time, ...).
            // Their side effects are queued for the SessionManager, which owns
            // the other sessions, the world clock and the operator list.
            if let Some(output) =
                crate::command::handle_command(&trimmed, &self.command_source(), &self.online_players())
            {
                self.pending_core.extend(output.actions);
                return Ok(output
                    .feedback
                    .into_iter()
                    .map(|line| OutgoingPacket::Text(TextPacket::raw(line)))
                    .collect());
            }

            // 3. Plugin commands.
            if let Some(plugins) = self.plugins.clone() {
                let sender = CommandSender::Player(self.player_context());
                match plugins.execute_command_sync(&sender, &trimmed) {
                    Ok(Some(outcome)) => {
                        self.pending_actions.extend(outcome.actions);
                        return Ok(Vec::new());
                    }
                    Ok(None) => {}
                    Err(error) => {
                        warn!("Plugin command '{}' failed: {}", trimmed, error);
                    }
                }
            }

            return Ok(vec![OutgoingPacket::Text(TextPacket::raw(
                crate::command::not_found_message(
                    trimmed
                        .trim_start_matches('/')
                        .split_whitespace()
                        .next()
                        .unwrap_or(""),
                ),
            ))]);
        }

        // Normal chat: PlayerChatEvent (cancellable, message/format mutable).
        let recipients = self
            .registry
            .as_ref()
            .map(|registry| registry.names())
            .unwrap_or_else(|| vec![self.username.clone()]);

        let mut event = PlayerChatEvent::new(self.player_context(), trimmed.clone());
        event.recipients = recipients;
        self.fire_event(&mut event);

        if event.cancelled {
            return Ok(Vec::new());
        }

        let format = if event.format.trim().is_empty() {
            format!("<{}> {}", self.username, event.message)
        } else {
            event.format
        };

        if event.recipients.is_empty() {
            Ok(vec![OutgoingPacket::Text(TextPacket::raw(format))])
        } else {
            self.pending_broadcasts.push(format);
            Ok(Vec::new())
        }
    }

    /// Handles CommandRequestPacket (0x4D): modern clients send slash commands
    /// through this packet instead of `TextPacket`.
    ///
    /// Mirrors Axolotl's `InGamePacketHandler::handleCommandRequest`, which
    /// only forwards lines that start with `/` into the chat pipeline.
    pub fn handle_command_request(
        &mut self,
        packet: CommandRequestPacket,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        if !self.is_in_game() || !packet.command.starts_with('/') {
            return Ok(Vec::new());
        }
        self.handle_text(TextPacket::raw(packet.command))
    }

    fn position_tuple(&self) -> (f32, f32, f32) {
        (self.position.x, self.position.y, self.position.z)
    }

    /// Sends the current health to the client.
    pub fn sync_health(&mut self) {
        self.pending_outgoing
            .push(OutgoingPacket::RawWire(set_health_wire(self.health)));
    }

    /// Sends the whole 36-slot inventory to the client.
    pub fn sync_inventory(&mut self) {
        let packet = inventory_content_wire(&self.inventory.contents());
        self.pending_outgoing.push(OutgoingPacket::RawWire(packet));
    }

    /// Handles InventoryTransactionPacket (0x1E): applies the container actions
    /// and answers with the slots that changed, or resyncs everything when the
    /// client and the server disagree.
    pub fn handle_inventory_transaction(
        &mut self,
        packet: InventoryTransactionPacket,
    ) -> Vec<OutgoingPacket> {
        if packet.actions.len() > 50 {
            warn!(
                "Session {}: {} actions in an inventory transaction",
                self.session_id,
                packet.actions.len()
            );
            return self.resync_inventory();
        }
        if matches!(packet.data, TransactionData::Mismatch) {
            return self.resync_inventory();
        }

        let applied = self.inventory.apply_actions(&packet.actions);
        if applied.rejected {
            debug!(
                "Session {}: rejected inventory transaction, resyncing",
                self.session_id
            );
            return self.resync_inventory();
        }

        let mut out = Vec::new();
        for (window_id, slot) in &applied.changed {
            if let Some(item) = self.inventory.window_slot(*window_id, *slot) {
                out.push(OutgoingPacket::RawWire(inventory_slot_wire(
                    *window_id, *slot, item,
                )));
            }
        }

        // `requestChangedSlots` asks for an unconditional resend of those slots.
        for group in packet.request_changed_slots.iter().flatten() {
            if group.container_id != 0 {
                continue;
            }
            for net_slot in &group.slot_indexes {
                let slot = *net_slot as u32;
                if let Some(item) = self.inventory.window_slot(WINDOW_ID_INVENTORY, slot) {
                    out.push(OutgoingPacket::RawWire(inventory_slot_wire(
                        WINDOW_ID_INVENTORY,
                        slot,
                        item,
                    )));
                }
            }
        }

        out
    }

    /// Sends every modelled window so client and server agree again.
    pub fn resync_inventory(&mut self) -> Vec<OutgoingPacket> {
        let mut out = Vec::new();
        for window_id in [
            WINDOW_ID_INVENTORY,
            WINDOW_ID_ARMOR,
            WINDOW_ID_OFFHAND,
            WINDOW_ID_CURSOR,
        ] {
            let items = self.inventory.window(window_id).unwrap_or(&[]).to_vec();
            let mut writer = Writer::new();
            writer.write_var_u32(InventoryContentPacket::PACKET_ID);
            InventoryContentPacket {
                window_id,
                items,
                container_id: 0,
                dynamic_container_id: None,
                storage: ItemStack::empty(),
            }
            .encode_payload(&mut writer);
            out.push(OutgoingPacket::RawWire(writer.into_bytes().to_vec()));
        }
        out
    }

    /// Kills the player: health drops to zero, the death animation plays and
    /// the client is told to look for a spawn (0x2D state 0), like BDS does.
    pub fn kill_player(&mut self) {
        self.health = 0.0;
        self.is_dead = true;

        self.pending_outgoing
            .push(OutgoingPacket::RawWire(set_health_wire(0.0)));
        self.pending_outgoing.push(OutgoingPacket::RawWire(actor_event_wire(
            self.entity_runtime_id,
            EVENT_DEATH_ANIMATION,
        )));
        let attributes = self.create_update_attributes_packet();
        self.pending_outgoing
            .push(OutgoingPacket::RawWire(attributes));
        self.pending_outgoing
            .push(OutgoingPacket::RawWire(respawn_wire(
                self.position_tuple(),
                RespawnPacket::SEARCHING_FOR_SPAWN,
                self.entity_runtime_id,
            )));
    }

    /// Handles the client's respawn request (0x2D state 2): restores health,
    /// moves the player to the world spawn and confirms with `READY_TO_SPAWN`.
    pub fn handle_respawn(&mut self) -> Vec<OutgoingPacket> {
        if !self.is_dead {
            return Vec::new();
        }
        self.is_dead = false;
        self.health = self.max_health;

        let (spawn_x, spawn_y, spawn_z) = match self.spawn_point {
            Some(position) => position,
            None => {
                let (x, y, z) = self.world.spawn();
                (x as f64 + 0.5, y as f64, z as f64 + 0.5)
            }
        };
        self.teleport(spawn_x, spawn_y, spawn_z, None, None);
        self.register_self();

        let mut out = vec![
            OutgoingPacket::RawWire(set_health_wire(self.health)),
            OutgoingPacket::RawWire(self.create_update_attributes_packet()),
            OutgoingPacket::RawWire(respawn_wire(
                self.position_tuple(),
                RespawnPacket::READY_TO_SPAWN,
                self.entity_runtime_id,
            )),
        ];
        // The teleport queued the move packet and the chunk refresh.
        out.append(&mut self.pending_outgoing);
        out
    }

    /// Handles SetPlayerGameTypePacket (0x3E) from client settings.
    pub fn handle_set_player_game_type(
        &mut self,
        packet: SetPlayerGameTypePacket,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        self.gamemode = packet.gamemode;
        if self.gamemode != 1 {
            self.is_flying = false;
        }
        let out = vec![
            OutgoingPacket::SetPlayerGameType(packet),
            OutgoingPacket::RawWire(self.create_update_abilities_packet()),
            OutgoingPacket::Text(TextPacket::raw(format!(
                "§aModo de juego cambiado a {}",
                if self.gamemode == 1 { "Creativo" } else { "Supervivencia" }
            ))),
        ];
        Ok(out)
    }

    /// Handles RequestAbilityPacket (0xB8) such as toggling flight.
    pub fn handle_request_ability(
        &mut self,
        packet: RequestAbilityPacket,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        if packet.ability_id == RequestAbilityPacket::ABILITY_FLYING {
            if self.gamemode == 1 {
                self.is_flying = packet.bool_value;
            } else {
                self.is_flying = false;
            }
            return Ok(vec![OutgoingPacket::RawWire(self.create_update_abilities_packet())]);
        }
        Ok(Vec::new())
    }

    /// Generates a LevelChunkPacket for (chunk_x, chunk_z) using the active
    /// world state (terrain plus plugin edits).
    pub fn create_chunk_packet(&self, chunk_x: i32, chunk_z: i32) -> LevelChunkPacket {
        let (sub_chunk_count, payload) = self.world.chunk_payload(chunk_x, chunk_z);
        LevelChunkPacket::new(
            chunk_x,
            chunk_z,
            0,     // dimension: 0 (Overworld)
            sub_chunk_count,
            None,  // sub_chunk_request_limit: None
            false, // cache_enabled: false
            Vec::new(), // used_blob_hashes: empty
            payload,
            false, // client_biome_update: false
        )
    }

    /// Creates a SetActorDataPacket (0x27) setting collision, gravity, scale, and air supply.
    pub fn create_set_actor_data_packet(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_var_u32(0x27); // SetActorData packet ID
        writer.write_var_u64(self.entity_runtime_id);
        // Metadata count: 6 entries
        writer.write_var_u32(6);

        // 1. FLAGS (0) -> type: long (7), legacy_type: 7
        writer.write_var_u32(0);
        writer.write_var_u32(7);
        writer.write_byte(7);
        // has_collision (bit 48) | affected_by_gravity (bit 49)
        let flags: i64 = (1i64 << 48) | (1i64 << 49);
        writer.write_var_i64(flags);

        // 2. AIR (7) -> type: short (1), legacy_type: 1 -> 400
        writer.write_var_u32(7);
        writer.write_var_u32(1);
        writer.write_byte(1);
        writer.write_i16_le(400);

        // 3. MAX_AIR (42) -> type: short (1), legacy_type: 1 -> 400
        writer.write_var_u32(42);
        writer.write_var_u32(1);
        writer.write_byte(1);
        writer.write_i16_le(400);

        // 4. SCALE (38) -> type: float (3), legacy_type: 3 -> 1.0
        writer.write_var_u32(38);
        writer.write_var_u32(3);
        writer.write_byte(3);
        writer.write_f32_le(1.0);

        // 5. BOUNDING_BOX_WIDTH (53) -> type: float (3), legacy_type: 3 -> 0.6
        writer.write_var_u32(53);
        writer.write_var_u32(3);
        writer.write_byte(3);
        writer.write_f32_le(0.6);

        // 6. BOUNDING_BOX_HEIGHT (54) -> type: float (3), legacy_type: 3 -> 1.8
        writer.write_var_u32(54);
        writer.write_var_u32(3);
        writer.write_byte(3);
        writer.write_f32_le(1.8);

        // EntityProperties (ints: 0, floats: 0)
        writer.write_var_u32(0);
        writer.write_var_u32(0);
        // Tick: 0
        writer.write_var_u64(0);

        writer.into_bytes().to_vec()
    }

    /// Creates an UpdateAttributesPacket (0x1D) setting player attributes (health, hunger, movement speed, etc.).
    pub fn create_update_attributes_packet(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_var_u32(0x1d); // UpdateAttributes packet ID
        writer.write_var_u64(self.entity_runtime_id);
        // 3 attributes: health, movement, player.hunger
        writer.write_var_u32(3);

        // 1. minecraft:health with the player's current value.
        writer.write_f32_le(0.0);
        writer.write_f32_le(self.max_health);
        writer.write_f32_le(self.health);
        writer.write_f32_le(0.0);
        writer.write_f32_le(self.max_health);
        writer.write_f32_le(self.max_health);
        writer.write_string("minecraft:health");
        writer.write_var_u32(0); // modifier count: 0

        // 2. minecraft:movement (min: 0.0, max: f32::MAX, val: 0.1, def_min: 0.0, def_max: f32::MAX, def: 0.1)
        writer.write_f32_le(0.0);
        writer.write_f32_le(f32::MAX);
        writer.write_f32_le(0.1);
        writer.write_f32_le(0.0);
        writer.write_f32_le(f32::MAX);
        writer.write_f32_le(0.1);
        writer.write_string("minecraft:movement");
        writer.write_var_u32(0); // modifier count: 0

        // 3. minecraft:player.hunger (min: 0.0, max: 20.0, val: 20.0, def_min: 0.0, def_max: 20.0, def: 20.0)
        writer.write_f32_le(0.0);
        writer.write_f32_le(20.0);
        writer.write_f32_le(20.0);
        writer.write_f32_le(0.0);
        writer.write_f32_le(20.0);
        writer.write_f32_le(20.0);
        writer.write_string("minecraft:player.hunger");
        writer.write_var_u32(0); // modifier count: 0

        // tick: 0
        writer.write_var_u64(0);

        writer.into_bytes().to_vec()
    }

    /// Creates an UpdateAbilitiesPacket (0xBB) setting player abilities (survival or creative with flight).
    pub fn create_update_abilities_packet(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_var_u32(0xbb); // UpdateAbilities packet ID
        writer.write_i64_le(self.entity_unique_id);

        if self.gamemode == 1 {
            // Creative Mode: operator permissions, allow flight, instabuild, invulnerable!
            writer.write_byte(2); // permission_level: member (2)
            writer.write_byte(1); // command_permission: operator (1)
            writer.write_byte(1); // layer count: 1

            // Layer 0: Base layer (1)
            writer.write_u16_le(1);
            let set_mask: u32 = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 5)
                | (1 << 6) | (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10) | (1 << 11)
                | (1 << 13) | (1 << 14);
            writer.write_u32_le(set_mask);

            let mut val_mask: u32 = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 5)
                | (1 << 6) | (1 << 7) | (1 << 8) | (1 << 10) | (1 << 11);
            if self.is_flying {
                val_mask |= 1 << 9;
            }
            writer.write_u32_le(val_mask);
            writer.write_f32_le(0.05); // fly_speed
            writer.write_f32_le(0.05); // vertical_fly_speed
            writer.write_f32_le(0.1);  // walk_speed
        } else {
            // Survival Mode
            writer.write_byte(1); // permission_level: member (1)
            writer.write_byte(0); // command_permission: normal (0)
            writer.write_byte(1); // layer count: 1

            // Layer 0: Base layer (1)
            writer.write_u16_le(1);
            writer.write_u32_le(0x0000663f);
            writer.write_u32_le(0x0000003f);
            writer.write_f32_le(0.05); // fly_speed
            writer.write_f32_le(0.0);  // vertical_fly_speed
            writer.write_f32_le(0.1);  // walk_speed
        }

        writer.into_bytes().to_vec()
    }

    /// Builds a MovePlayerPacket (0x12) to teleport the local player.
    ///
    /// Mode 2 is `Teleport`, which is accepted even when the client predicts a
    /// different position. If `yaw`/`pitch` are provided they override the
    /// session's current values, matching PocketMine's `/tp x y z yaw pitch`.
    pub fn create_move_player_packet(
        &self,
        mode: u32,
        yaw_override: Option<f32>,
        pitch_override: Option<f32>,
    ) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_var_u32(0x12);
        writer.write_var_u64(self.entity_runtime_id);
        writer.write_f32_le(self.position.x);
        writer.write_f32_le(self.position.y);
        writer.write_f32_le(self.position.z);
        writer.write_f32_le(pitch_override.unwrap_or(self.pitch));
        writer.write_f32_le(yaw_override.unwrap_or(self.yaw));
        writer.write_f32_le(yaw_override.unwrap_or(self.yaw));
        writer.write_var_u32(mode);
        writer.write_bool(true); // on_ground
        writer.write_var_u64(0); // ridden_runtime_id
        if mode == 2 {
            // Teleport extras: cause, source entity type, tick.
            writer.write_i32_le(0);
            writer.write_var_u32(0);
            writer.write_var_u64(0);
        }
        writer.into_bytes().to_vec()
    }

    /// Builds a SetTitlePacket (0x59).
    ///
    /// Title types: 0 clear, 1 reset, 2 title, 3 subtitle, 4 action bar,
    /// 5 times, 6 title+subtitle.
    pub fn create_set_title_packet(
        &self,
        title_type: i32,
        text: &str,
        fade_in: i32,
        stay: i32,
        fade_out: i32,
    ) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_var_u32(0x59);
        writer.write_var_i32(title_type);
        writer.write_string(text);
        writer.write_i32_le(fade_in);
        writer.write_i32_le(stay);
        writer.write_i32_le(fade_out);
        writer.write_string(""); // xuid
        writer.write_string(""); // platform online id
        writer.write_string(""); // filtered title
        writer.into_bytes().to_vec()
    }
}

/// Thread-safe player session manager.
pub struct SessionManager {
    sessions: RwLock<HashMap<u64, PlayerSession>>,
    next_session_id: AtomicU64,
    generator: Arc<dyn WorldGenerator>,
    world: Arc<WorldState>,
    registry: Arc<PlayerRegistry>,
    plugins: RwLock<Option<Arc<PluginManager>>>,
    ops: Arc<RwLock<HashSet<String>>>,
    default_gamemode: std::sync::atomic::AtomicI32,
    default_view_distance: i32,
    max_players: std::sync::atomic::AtomicI32,
    allow_cheats: RwLock<bool>,
    access: Arc<crate::access::AccessList>,
    stop_requested: std::sync::atomic::AtomicBool,
    auto_save: std::sync::atomic::AtomicBool,
    data_dir: RwLock<Option<std::path::PathBuf>>,
    metrics: Arc<crate::metrics::TickMetrics>,
    permissions: Arc<crate::permission::PermissionManager>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self::with_generator(Arc::new(bedrock_world::NormalGenerator::new(12345678)))
    }

    pub fn with_generator(generator: Arc<dyn WorldGenerator>) -> Self {
        Self::with_generator_and_gamemode(generator, 0)
    }

    pub fn with_generator_and_gamemode(generator: Arc<dyn WorldGenerator>, default_gamemode: i32) -> Self {
        Self::with_generator_gamemode_and_view_distance(generator, default_gamemode, 12)
    }

    pub fn with_generator_gamemode_and_view_distance(
        generator: Arc<dyn WorldGenerator>,
        default_gamemode: i32,
        default_view_distance: i32,
    ) -> Self {
        let world = Arc::new(WorldState::new("world", Arc::clone(&generator)));
        Self {
            sessions: RwLock::new(HashMap::new()),
            next_session_id: AtomicU64::new(1),
            generator,
            world,
            registry: Arc::new(PlayerRegistry::new()),
            plugins: RwLock::new(None),
            ops: Arc::new(RwLock::new(HashSet::new())),
            default_gamemode: std::sync::atomic::AtomicI32::new(default_gamemode),
            default_view_distance,
            max_players: std::sync::atomic::AtomicI32::new(1000),
            allow_cheats: RwLock::new(true),
            access: Arc::new(crate::access::AccessList::new()),
            stop_requested: std::sync::atomic::AtomicBool::new(false),
            auto_save: std::sync::atomic::AtomicBool::new(true),
            data_dir: RwLock::new(None),
            metrics: Arc::new(crate::metrics::TickMetrics::new()),
            permissions: Arc::new(crate::permission::PermissionManager::new()),
        }
    }

    /// Installs the plugin runtime used for events, commands and the tick loop.
    pub fn set_plugin_manager(&self, plugins: Arc<PluginManager>) {
        *self.plugins.write() = Some(plugins);
    }

    /// Enables or disables administrative commands for non-operators
    /// (`allow-cheats` in `server.properties`).
    pub fn set_allow_cheats(&self, allow: bool) {
        *self.allow_cheats.write() = allow;
        for session in self.sessions.write().values_mut() {
            session.allow_cheats = allow;
        }
    }

    /// Sets `max-players`, reported by `/list` like `Server::getMaxPlayers()`.
    pub fn set_max_players(&self, max_players: i32) {
        self.max_players
            .store(max_players, std::sync::atomic::Ordering::Relaxed);
        for session in self.sessions.write().values_mut() {
            session.max_players = max_players;
        }
    }

    pub fn plugin_manager(&self) -> Option<Arc<PluginManager>> {
        self.plugins.read().clone()
    }

    pub fn world(&self) -> &Arc<WorldState> {
        &self.world
    }

    pub fn registry(&self) -> &Arc<PlayerRegistry> {
        &self.registry
    }

    pub fn ops(&self) -> &Arc<RwLock<HashSet<String>>> {
        &self.ops
    }

    pub fn set_ops(&self, ops: HashSet<String>) {
        *self.ops.write() = ops;
    }

    fn new_session(&self, id: u64, addr: SocketAddr) -> PlayerSession {
        let mut session = PlayerSession::with_generator_and_gamemode(
            id,
            addr,
            Arc::clone(&self.generator),
            self.default_gamemode
                .load(std::sync::atomic::Ordering::Relaxed),
        );
        session.max_view_distance = self.default_view_distance;
        session.allow_cheats = *self.allow_cheats.read();
        session.max_players = self.max_players.load(std::sync::atomic::Ordering::Relaxed);
        session.attach_access(Arc::clone(&self.access));
        session.attach_metrics(Arc::clone(&self.metrics));
        session.attach_permissions(Arc::clone(&self.permissions));
        session.data_dir = self.data_dir.read().clone();
        session.attach_runtime(
            Arc::clone(&self.world),
            self.plugin_manager(),
            Arc::clone(&self.registry),
            Arc::clone(&self.ops),
        );
        session
    }

    /// Registers a new session and returns its id.
    ///
    /// The manager owns the only copy: transports address the session through
    /// [`Self::handle_session_packet`] with the returned id, so there is no
    /// detached handle whose queued packets could be lost.
    pub fn create_session(&self, addr: SocketAddr) -> u64 {
        let id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
        let session = self.new_session(id, addr);
        self.sessions.write().insert(id, session);
        id
    }

    pub fn add_session(&self, session: PlayerSession) {
        let mut map = self.sessions.write();
        map.insert(session.session_id, session);
    }

    /// Removes a session and fires `PlayerQuitEvent`, broadcasting its message.
    pub fn remove_session(&self, session_id: u64) -> Option<PlayerSession> {
        let mut session = self.sessions.write().remove(&session_id)?;
        self.finalize_session(&mut session, "Disconnected");
        Some(session)
    }

    /// Handles a WebRTC peer disconnect (data channel closed).
    pub fn handle_peer_disconnect(&self, peer: SocketAddr) -> Vec<PluginAction> {
        let session_id = {
            let map = self.sessions.read();
            map.values()
                .find(|session| session.client_addr == peer)
                .map(|session| session.session_id)
        };

        let Some(session_id) = session_id else {
            return Vec::new();
        };

        let Some(mut session) = self.sessions.write().remove(&session_id) else {
            return Vec::new();
        };

        let actions = self.finalize_session(&mut session, "Disconnected");
        self.apply_actions(&actions, 0);
        actions
    }

    fn finalize_session(&self, session: &mut PlayerSession, reason: &str) -> Vec<PluginAction> {
        let _ = session.save_player_data();
        if let Some(registry) = &session.registry {
            registry.remove(&session.username);
        }

        if !session.joined {
            return Vec::new();
        }
        session.joined = false;

        let mut event = PlayerQuitEvent::new(
            session.player_context(),
            format!("{} left the game", session.username),
            reason,
        );
        let actions = session.fire_event(&mut event);
        if !event.quit_message.is_empty() {
            self.broadcast_text(&event.quit_message);
        }
        actions
    }

    pub fn session_id_for(&self, name: &str) -> Option<u64> {
        let name = name.to_lowercase();
        self.sessions
            .read()
            .values()
            .find(|session| session.username.to_lowercase() == name)
            .map(|session| session.session_id)
    }

    /// Mirrors `ConfigGroup::setConfigBool()` + `save()` for `server.properties`.
    fn persist_server_property(&self, key: &str, value: &str) {
        let Some(dir) = self.data_dir.read().clone() else {
            return;
        };
        crate::config::set_server_property(&dir, key, value);
    }

    fn queue_packet(&self, name: &str, packet: OutgoingPacket) {
        let Some(id) = self.session_id_for(name) else {
            return;
        };
        if let Some(session) = self.sessions.write().get_mut(&id) {
            session.pending_outgoing.push(packet);
        }
    }

    pub fn broadcast_text(&self, message: &str) {
        let packet = OutgoingPacket::Text(TextPacket::raw(message));
        let mut map = self.sessions.write();
        for session in map.values_mut() {
            if session.joined {
                session.pending_outgoing.push(packet.clone());
            }
        }
    }

    fn queue_all(&self, packet: OutgoingPacket) {
        let mut map = self.sessions.write();
        for session in map.values_mut() {
            if session.joined {
                session.pending_outgoing.push(packet.clone());
            }
        }
    }

    fn queue_targeted(&self, target: &str, packet: OutgoingPacket) {
        if target.eq_ignore_ascii_case("@all") || target.eq_ignore_ascii_case("@a") {
            self.queue_all(packet);
            return;
        }
        self.queue_packet(target, packet);
    }

    /// Delivers a result line to the player who ran a command, or prints it to
    /// the server console when the console ran it. PocketMine shows command
    /// feedback directly on its console, so we use [`console_message`] instead
    /// of `tracing::info!` to avoid the tracing event wrapper.
    fn notify_sender(&self, who: &str, message: &str) {
        if who.is_empty() || who.eq_ignore_ascii_case("CONSOLE") {
            crate::logging::console_message(message);
        } else {
            self.queue_packet(who, OutgoingPacket::Text(TextPacket::raw(message)));
        }
    }

    /// `Command::broadcastCommandMessage`: the sender already received the
    /// plain line, administrators get the grey italic `[source: message]` one.
    fn broadcast_command_message(&self, source: &str, message: &str) {
        // `chat.type.admin`: the console subscribes to the administrative
        // channel as a logger forwarder and receives the uncoloured line, while
        // players get it in grey italics.
        let admin = crate::command::fill_template(crate::command::lang::CHAT_ADMIN, &[source, message]);
        crate::logging::console_message(&admin);
        let line = format!("§7§o{admin}");
        let ops: Vec<String> = self.ops.read().iter().cloned().collect();
        for name in ops {
            if name.eq_ignore_ascii_case(source) {
                continue;
            }
            self.queue_packet(&name, OutgoingPacket::Text(TextPacket::raw(&line)));
        }
    }

    pub fn session_count(&self) -> usize {
        self.sessions.read().len()
    }

    /// Sends the disconnect screen to a player and marks the session closed.
    fn kick_player(&self, name: &str, message: &str) {
        let Some(session_id) = self.session_id_for(name) else {
            return;
        };
        if let Some(session) = self.sessions.write().get_mut(&session_id) {
            if session.state == SessionState::Disconnected {
                return;
            }
            session.state = SessionState::Disconnected;
            session
                .pending_outgoing
                .push(OutgoingPacket::Disconnect(DisconnectPacket {
                    reason: 0,
                    skip_message: false,
                    message: Some(message.to_string()),
                }));
        }
    }

    /// Kicks every session connected from `ip` (`/ban-ip`).
    fn kick_ip(&self, ip: &str, message: &str) {
        let ids: Vec<u64> = self
            .sessions
            .read()
            .iter()
            .filter(|(_, session)| session.client_addr.ip().to_string() == ip)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if let Some(session) = self.sessions.write().get_mut(&id) {
                session.state = SessionState::Disconnected;
                session
                    .pending_outgoing
                    .push(OutgoingPacket::Disconnect(DisconnectPacket {
                        reason: 0,
                        skip_message: false,
                        message: Some(message.to_string()),
                    }));
            }
        }
    }

    /// `WhitelistCommand::kickNonWhitelistedPlayers()`; operators are exempt.
    fn kick_non_whitelisted(&self) {
        let names: Vec<String> = self
            .sessions
            .read()
            .values()
            .filter(|session| session.joined)
            .map(|session| session.username.clone())
            .collect();
        for name in names {
            if self.ops.read().contains(&name.to_lowercase()) {
                continue;
            }
            if !self.access.is_whitelisted(&name) {
                self.kick_player(
                    &name,
                    &crate::command::lang::DISCONNECT_KICK
                        .replace("{%reason}", crate::command::lang::DISCONNECT_WHITELISTED),
                );
            }
        }
    }

    /// Routes a raw packet to a session and collects everything it queued.
    ///
    /// Transports own the session id, so this is the entry point for packets
    /// addressed to a specific player.
    pub fn handle_session_packet(
        &self,
        session_id: u64,
        packet_id: u32,
        payload: &[u8],
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        let mut map = self.sessions.write();
        let Some(session) = map.get_mut(&session_id) else {
            return Err(SessionError::Disconnected);
        };
        let mut out = session.handle_packet(packet_id, payload)?;

        // PocketMine checks the bans and the whitelist right after login.
        if packet_id == 0x01 && !session.username.is_empty() {
            let is_op = session.is_op();
            let ip = session.client_addr.ip().to_string();
            if let Some(reason) = self.access.denial(&session.username, &ip, is_op) {
                info!(
                    "Refusing login for '{}' ({}): {}",
                    session.username, ip, reason
                );
                session.state = SessionState::Disconnected;
                out = vec![OutgoingPacket::Disconnect(DisconnectPacket {
                    reason: 0,
                    skip_message: false,
                    message: Some(reason),
                })];
            }
        }

        out.append(&mut session.pending_outgoing);
        Ok(out)
    }

    /// Read-only snapshot of a player's 36-slot inventory.
    pub fn player_inventory(&self, name: &str) -> Option<Vec<ItemStack>> {
        let id = self.session_id_for(name)?;
        let map = self.sessions.read();
        map.get(&id).map(|session| session.inventory.contents())
    }

    /// Position of a live player, mainly for diagnostics and tests.
    pub fn player_position(&self, name: &str) -> Option<(f32, f32, f32)> {
        let id = self.session_id_for(name)?;
        let map = self.sessions.read();
        map.get(&id)
            .map(|session| (session.position.x, session.position.y, session.position.z))
    }

    /// World difficulty, as changed by `/difficulty`.
    pub fn world_difficulty(&self) -> i32 {
        self.world.difficulty()
    }

    /// Ban and whitelist lists.
    pub fn access(&self) -> &Arc<crate::access::AccessList> {
        &self.access
    }

    /// Shared world state (terrain, time, spawn and block edits).
    pub fn world_state(&self) -> &Arc<WorldState> {
        &self.world
    }

    /// Restores the persisted `auto-save` property on startup.
    pub fn set_auto_save_enabled(&self, enabled: bool) {
        self.auto_save
            .store(enabled, std::sync::atomic::Ordering::SeqCst);
    }

    /// Enables player and world persistence in `dir`.
    pub fn enable_persistence(&self, dir: std::path::PathBuf) {
        *self.data_dir.write() = Some(dir.clone());
        for session in self.sessions.write().values_mut() {
            session.data_dir = Some(dir.clone());
        }
    }

    /// `Server::saveAll()`: player data plus the world snapshot.
    pub fn save_all(&self) -> std::io::Result<()> {
        let Some(dir) = self.data_dir.read().clone() else {
            return Ok(());
        };
        {
            let map = self.sessions.read();
            for session in map.values().filter(|session| session.joined) {
                let _ = session.save_player_data();
            }
        }
        self.world.save_to(&dir.join("world.json"))
    }

    /// `save-on` / `save-off` state, checked by the autosave tick.
    pub fn auto_save_enabled(&self) -> bool {
        self.auto_save.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Tick sampler consumed by `/tps`.
    pub fn metrics(&self) -> &Arc<crate::metrics::TickMetrics> {
        &self.metrics
    }

    /// Permission registry shared with every session.
    pub fn permissions(&self) -> &Arc<crate::permission::PermissionManager> {
        &self.permissions
    }

    /// `(1m, 5m, 15m)` TPS averages.
    pub fn tps(&self) -> (f64, f64, f64) {
        self.metrics.current()
    }

    /// `/stop` asked the server to shut down.
    pub fn request_stop(&self) {
        self.stop_requested
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn stop_requested(&self) -> bool {
        self.stop_requested
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Lifecycle state of a live session, used by tests and diagnostics.
    pub fn session_state(&self, name: &str) -> Option<SessionState> {
        let id = self.session_id_for(name)?;
        let map = self.sessions.read();
        map.get(&id).map(|session| session.state)
    }

    /// Dispatches a raw Bedrock network message received over NetherNet WebRtc DataChannels
    pub fn dispatch_webrtc_packet(&self, peer: SocketAddr, raw: &[u8]) -> Vec<bytes::Bytes> {
        // Take the session out of the map so plugin IPC (which may call back
        // into the host) never runs while the session lock is held.
        let (session_id, mut session) = {
            let mut map = self.sessions.write();
            match map
                .iter()
                .find(|(_, session)| session.client_addr == peer)
                .map(|(id, _)| *id)
            {
                Some(id) => {
                    let session = map.remove(&id).expect("session exists");
                    (id, session)
                }
                None => {
                    let id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
                    let session = self.new_session(id, peer);
                    (id, session)
                }
            }
        };

        // A session may have been created before plugins finished loading.
        if session.plugins.is_none() {
            session.plugins = self.plugin_manager();
            session.world = Arc::clone(&self.world);
            session.registry = Some(Arc::clone(&self.registry));
            session.ops = Arc::clone(&self.ops);
        }

        let mut outgoing: Vec<OutgoingPacket> = std::mem::take(&mut session.pending_outgoing);

        // Decompress batch or treat as raw uncompressed batch
        let decompressed = match PacketBatch::decompress(raw) {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("[:WebRTC] Batch decompress failed from {}: {}, using raw", peer, e);
                raw.to_vec()
            }
        };

        // Decode individual packets from the uncompressed buffer
        let packets = match PacketBatch::decode_packets(&decompressed) {
            Ok(pkts) if !pkts.is_empty() => pkts,
            _ => {
                // Fallback: direct unbatched packet format (like uncompressed RequestNetworkSettingsPacket)
                let mut reader = Reader::new(&decompressed);
                let header = reader.read_var_u32().unwrap_or(0);
                let packet_id = header & 0x3FF;
                let payload = reader.read_remaining().to_vec();
                vec![(packet_id, payload)]
            }
        };

        for (packet_id, payload) in packets {
            debug!(
                "[:WebRTC] Dispatching Bedrock packet 0x{:02X} (len {}) from {}",
                packet_id, payload.len(), peer
            );

            match session.handle_packet(packet_id, &payload) {
                Ok(responses) => outgoing.extend(responses),
                Err(e) => {
                    tracing::warn!(
                        "[:WebRTC] Session error handling 0x{:02X} from {}: {}",
                        packet_id, peer, e
                    );
                }
            }
        }

        let broadcasts = std::mem::take(&mut session.pending_broadcasts);
        let actions = std::mem::take(&mut session.pending_actions);
        let core_actions = std::mem::take(&mut session.pending_core);
        let compression_enabled = session.compression_enabled
            || outgoing.iter().any(|packet| packet.packet_id() == 0x8F);
        session.compression_enabled = compression_enabled;

        self.sessions.write().insert(session_id, session);

        for message in broadcasts {
            self.broadcast_text(&message);
        }
        self.apply_actions(&actions, 0);
        self.apply_command_actions(&core_actions);

        // Pick up packets queued by broadcasts/actions for this same session.
        if let Some(session) = self.sessions.write().get_mut(&session_id) {
            outgoing.append(&mut session.pending_outgoing);
        }

        self.encode_outgoing(outgoing, compression_enabled)
    }

    /// Encodes outgoing packets into compressed (or raw) Bedrock batches.
    fn encode_outgoing(
        &self,
        packets: Vec<OutgoingPacket>,
        compression_enabled: bool,
    ) -> Vec<bytes::Bytes> {
        let mut batches: Vec<Vec<u8>> = Vec::new();
        let mut current_batch = Vec::new();

        for resp in packets {
            let pkt_bytes = match &resp {
                OutgoingPacket::RawWire(bytes) => bytes::Bytes::copy_from_slice(bytes),
                _ => {
                    let mut pkt_writer = Writer::new();
                    let header = resp.packet_id() & 0x3FF;
                    pkt_writer.write_var_u32(header);
                    if let Err(e) = resp.encode(&mut pkt_writer) {
                        tracing::error!(
                            "Failed to encode outgoing packet 0x{:02X}: {}",
                            resp.packet_id(),
                            e
                        );
                        continue;
                    }
                    pkt_writer.into_bytes()
                }
            };

            let mut entry_writer = Writer::new();
            entry_writer.write_var_u32(pkt_bytes.len() as u32);
            entry_writer.write_bytes(&pkt_bytes);
            let framed_entry = entry_writer.into_bytes();

            // NetworkSettings must travel uncompressed and on its own.
            if resp.packet_id() == 0x8F {
                return vec![bytes::Bytes::from(framed_entry)];
            }

            // Split batches at ~16KB threshold to keep SCTP message size optimal.
            if !current_batch.is_empty() && current_batch.len() + framed_entry.len() > 16384 {
                batches.push(std::mem::take(&mut current_batch));
            }

            current_batch.extend_from_slice(&framed_entry);
        }

        if !current_batch.is_empty() {
            batches.push(current_batch);
        }

        let mut out = Vec::with_capacity(batches.len());
        for batch in batches {
            if compression_enabled {
                match PacketBatch::compress(&batch, 512) {
                    Ok(compressed) => out.push(bytes::Bytes::from(compressed)),
                    Err(e) => {
                        tracing::error!("Failed to compress outgoing batch: {}", e);
                        out.push(bytes::Bytes::from(batch));
                    }
                }
            } else {
                out.push(bytes::Bytes::from(batch));
            }
        }

        out
    }

    /// Executes actions emitted by plugins against the live sessions.
    ///
    /// `depth` prevents unbounded recursion when a plugin dispatches a command
    /// that itself emits actions.
    pub fn apply_actions(&self, actions: &[PluginAction], depth: usize) -> Vec<PluginAction> {
        if depth > 4 {
            warn!("Plugin action recursion limit reached");
            return Vec::new();
        }

        let mut follow_up = Vec::new();

        for action in actions {
            match action {
                PluginAction::SendMessage {
                    target_player,
                    message,
                } => {
                    self.queue_targeted(
                        target_player,
                        OutgoingPacket::Text(TextPacket::raw(message.clone())),
                    );
                }
                PluginAction::BroadcastMessage { message } => {
                    self.broadcast_text(message);
                }
                PluginAction::ConsoleMessage { message } => {
                    info!("[Plugin] {}", crate::logging::to_ansi(&message));
                }
                PluginAction::KickPlayer {
                    target_player,
                    reason,
                } => {
                    self.queue_targeted(
                        target_player,
                        OutgoingPacket::Disconnect(DisconnectPacket {
                            reason: 0,
                            skip_message: false,
                            message: Some(reason.clone()),
                        }),
                    );
                    if let Some(registry) = Some(&self.registry) {
                        registry.remove(target_player);
                    }
                }
                PluginAction::Teleport {
                    target_player,
                    x,
                    y,
                    z,
                    world,
                    yaw,
                    pitch,
                } => {
                    let Some(id) = self.session_id_for(target_player) else {
                        continue;
                    };
                    let mut map = self.sessions.write();
                    if let Some(session) = map.get_mut(&id) {
                        if let Some(world) = world {
                            if !world.is_empty() {
                                session.world = Arc::new(WorldState::new(
                                    world.clone(),
                                    Arc::clone(&session.generator),
                                ));
                            }
                        }
                        // A portado de `syncTeleport`: el yaw/pitch
                        // Bedrock que trae la acción se conserva
                        // (normalizado en `teleport()`); sin ellos el
                        // jugador quedaba con la rotación anterior.
                        session.teleport(
                            *x,
                            *y,
                            *z,
                            yaw.map(|v| v as f32),
                            pitch.map(|v| v as f32),
                        );
                        registry_upsert(&self.registry, session);
                    }
                }
                PluginAction::SetBlock {
                    world,
                    x,
                    y,
                    z,
                    block_id,
                } => {
                    if !world.is_empty() && !world.eq_ignore_ascii_case(&self.world.name) {
                        continue;
                    }
                    if self.world.set_block(*x, *y, *z, *block_id) {
                        self.refresh_chunk_at(*x, *z);
                    }
                }
                PluginAction::DispatchCommand { command, sender } => {
                    let plugins = self.plugin_manager();
                    let Some(plugins) = plugins else {
                        continue;
                    };
                    let sender = match sender {
                        Some(name) if !name.eq_ignore_ascii_case("CONSOLE") => {
                            match self.registry.get(name) {
                                Some(player) => CommandSender::Player(player),
                                None => CommandSender::Console,
                            }
                        }
                        _ => CommandSender::Console,
                    };
                    match plugins.execute_command_sync(&sender, command) {
                        Ok(Some(outcome)) => follow_up.extend(outcome.actions),
                        Ok(None) => debug!("Dispatched plugin command '{}' (unhandled)", command),
                        Err(error) => warn!("Dispatched command '{}' failed: {}", command, error),
                    }
                }
                PluginAction::SendTip {
                    target_player,
                    message,
                } => {
                    self.queue_targeted(
                        target_player,
                        OutgoingPacket::Text(text_with_type(TextPacket::TYPE_TIP, message)),
                    );
                }
                PluginAction::SendPopup {
                    target_player,
                    message,
                } => {
                    self.queue_targeted(
                        target_player,
                        OutgoingPacket::Text(text_with_type(TextPacket::TYPE_POPUP, message)),
                    );
                }
                PluginAction::SendTitle {
                    target_player,
                    title,
                    subtitle,
                    fade_in,
                    stay,
                    fade_out,
                } => {
                    let Some(id) = self.session_id_for(target_player) else {
                        continue;
                    };
                    if let Some(session) = self.sessions.write().get_mut(&id) {
                        session.pending_outgoing.push(OutgoingPacket::RawWire(
                            session.create_set_title_packet(
                                6,
                                title,
                                *fade_in,
                                *stay,
                                *fade_out,
                            ),
                        ));
                        if !subtitle.is_empty() {
                            session.pending_outgoing.push(OutgoingPacket::RawWire(
                                session.create_set_title_packet(
                                    3,
                                    subtitle,
                                    *fade_in,
                                    *stay,
                                    *fade_out,
                                ),
                            ));
                        }
                    }
                }
                PluginAction::BroadcastTip { message } => {
                    self.queue_all(OutgoingPacket::Text(text_with_type(
                        TextPacket::TYPE_TIP,
                        message,
                    )));
                }
                PluginAction::BroadcastPopup { message } => {
                    self.queue_all(OutgoingPacket::Text(text_with_type(
                        TextPacket::TYPE_POPUP,
                        message,
                    )));
                }
                PluginAction::BroadcastTitle {
                    title,
                    subtitle,
                    fade_in,
                    stay,
                    fade_out,
                } => {
                    let mut map = self.sessions.write();
                    for session in map.values_mut().filter(|session| session.joined) {
                        session.pending_outgoing.push(OutgoingPacket::RawWire(
                            session.create_set_title_packet(
                                6,
                                title,
                                *fade_in,
                                *stay,
                                *fade_out,
                            ),
                        ));
                        if !subtitle.is_empty() {
                            session.pending_outgoing.push(OutgoingPacket::RawWire(
                                session.create_set_title_packet(
                                    3,
                                    subtitle,
                                    *fade_in,
                                    *stay,
                                    *fade_out,
                                ),
                            ));
                        }
                    }
                }
                PluginAction::SetGamemode {
                    target_player,
                    gamemode,
                } => {
                    let gamemode = *gamemode as i32;
                    let Some(id) = self.session_id_for(target_player) else {
                        continue;
                    };
                    let mut map = self.sessions.write();
                    if let Some(session) = map.get_mut(&id) {
                        session.gamemode = gamemode;
                        session.pending_outgoing.push(OutgoingPacket::SetPlayerGameType(
                            SetPlayerGameTypePacket::new(gamemode),
                        ));
                        let abilities = session.create_update_abilities_packet();
                        session.pending_outgoing.push(OutgoingPacket::RawWire(abilities));
                    }
                }
            }
        }

        if !follow_up.is_empty() {
            let nested = self.apply_actions(&follow_up, depth + 1);
            follow_up.extend(nested);
        }

        follow_up
    }

    /// Runs a native command from the server console.
    ///
    /// Returns the feedback lines to print when the command is native, or
    /// `None` when it is not, so the caller can try plugin commands.
    pub fn execute_console_command(&self, line: &str) -> Option<Vec<String>> {
        let mut source = CommandSource::console(self.world.time());
        source.world_seed = self.world.seed();
        source.world_spawn = {
            let (x, y, z) = self.world.spawn();
            (x as f64, y as f64, z as f64)
        };
        source.plugins = self
            .plugin_manager()
            .map(|manager| manager.load_order())
            .unwrap_or_default();
        source.tps = self.tps();
        source.permissions = Some(Arc::clone(&self.permissions));
        let players = self.online_players_snapshot();

        let output = crate::command::handle_command(line, &source, &players)?;
        self.apply_command_actions(&output.actions);
        Some(output.feedback)
    }

    /// Authoritative list of players in the world, taken from the live
    /// sessions (the plugin registry may lag behind or not exist at all).
    fn online_players_snapshot(&self) -> Vec<OnlinePlayer> {
        self.sessions
            .read()
            .values()
            .filter(|session| session.joined)
            .map(|session| OnlinePlayer {
                name: session.username.clone(),
                position: (
                    session.position.x as f64,
                    session.position.y as f64,
                    session.position.z as f64,
                ),
                gamemode: session.gamemode,
            })
            .collect()
    }

    /// Applies the side effects of native commands against the live sessions.
    pub fn apply_command_actions(&self, actions: &[CommandAction]) {
        for action in actions {
            match action {
                CommandAction::SetGamemode {
                    target,
                    gamemode,
                    message,
                    notify,
                } => {
                    let Some(session_id) = self.session_id_for(target) else {
                        continue;
                    };
                    {
                        let mut map = self.sessions.write();
                        let Some(session) = map.get_mut(&session_id) else {
                            continue;
                        };
                        session.gamemode = *gamemode;
                        if *gamemode != 1 {
                            session.is_flying = false;
                        }
                        session.pending_outgoing.push(OutgoingPacket::SetPlayerGameType(
                            SetPlayerGameTypePacket::new(*gamemode),
                        ));
                        let abilities = session.create_update_abilities_packet();
                        session.pending_outgoing.push(OutgoingPacket::RawWire(abilities));
                        registry_upsert(&self.registry, session);
                    }
                    // Paper broadcasts verified command changes to operators and
                    // tells the affected player; PocketMine only messages the
                    // source, so the broadcast is additive parity.
                    self.broadcast_command_message(notify, message);
                    if !target.eq_ignore_ascii_case(notify) {
                        let changed = crate::command::lang::GAMEMODE_CHANGED.replace(
                            "{%0}",
                            crate::command::gamemode_translatable(*gamemode),
                        );
                        self.queue_packet(target, OutgoingPacket::Text(TextPacket::raw(&changed)));
                    }
                }
                CommandAction::Teleport { target, position, yaw, pitch } => {
                    self.teleport_player(target, position.0, position.1, position.2, *yaw, *pitch);
                }
                CommandAction::TeleportToSpawn { target } => {
                    let (spawn_x, spawn_y, spawn_z) = self.world.spawn();
                    self.teleport_player(
                        target,
                        spawn_x as f64 + 0.5,
                        spawn_y as f64,
                        spawn_z as f64 + 0.5,
                        None,
                        None,
                    );
                }
                CommandAction::Broadcast { message } => self.broadcast_text(message),
                CommandAction::Message { target, message } => self.queue_packet(
                    target,
                    OutgoingPacket::Text(TextPacket::raw(message.clone())),
                ),
                CommandAction::Kick { target, reason } => {
                    self.queue_packet(
                        target,
                        OutgoingPacket::Disconnect(DisconnectPacket {
                            reason: 0,
                            skip_message: false,
                            message: Some(reason.clone()),
                        }),
                    );
                    info!("Kicked {}: {}", target, crate::logging::to_ansi(reason));
                }
                CommandAction::SendTitle { target, action } => {
                    let packet = match action {
                        TitleAction::Clear => SetTitlePacket::clear_title(),
                        TitleAction::Reset => SetTitlePacket::reset_title_options(),
                        TitleAction::Title(text) => SetTitlePacket::title(text.clone()),
                        TitleAction::Subtitle(text) => SetTitlePacket::subtitle(text.clone()),
                        TitleAction::ActionBar(text) => {
                            SetTitlePacket::action_bar_message(text.clone())
                        }
                        TitleAction::Times {
                            fade_in,
                            stay,
                            fade_out,
                        } => SetTitlePacket::set_animation_times(*fade_in, *stay, *fade_out),
                    };
                    self.queue_packet(target, OutgoingPacket::SetTitle(packet));
                }
                CommandAction::Transfer {
                    target,
                    address,
                    port,
                } => {
                    // `NetworkSession::transfer()` sends the packet and closes
                    // the session without a disconnect screen.
                    self.queue_packet(
                        target,
                        OutgoingPacket::Transfer(TransferPacket::new(
                            address.clone(),
                            *port,
                            false,
                        )),
                    );
                    if let Some(session_id) = self.session_id_for(target) {
                        if let Some(session) = self.sessions.write().get_mut(&session_id) {
                            session.state = SessionState::Disconnected;
                        }
                    }
                }
                CommandAction::SetTime(ticks) => {
                    let time = self.world.set_time(*ticks);
                    self.broadcast_time(time);
                }
                CommandAction::AddTime(ticks) => {
                    let time = self.world.add_time(*ticks);
                    self.broadcast_time(time);
                }
                CommandAction::SetWeather {
                    state,
                    duration_ticks,
                } => {
                    self.world.set_weather(state.as_i32());
                    for event_id in state.level_events() {
                        self.queue_all(OutgoingPacket::RawWire(level_event_wire(
                            *event_id,
                            *duration_ticks,
                        )));
                    }
                }
                CommandAction::SetWorldSpawn { x, y, z } => {
                    self.world.set_spawn(*x, *y, *z);
                    let position = BlockPos::new(*x, *y, *z);
                    self.queue_all(OutgoingPacket::SetSpawnPosition(
                        SetSpawnPositionPacket::new_player_spawn(position, 0),
                    ));
                }
                CommandAction::Kill {
                    target,
                    message,
                    broadcast,
                    notify,
                } => {
                    let Some(session_id) = self.session_id_for(target) else {
                        continue;
                    };
                    {
                        let mut map = self.sessions.write();
                        let Some(session) = map.get_mut(&session_id) else {
                            continue;
                        };
                        session.kill_player();
                        registry_upsert(&self.registry, session);
                    }
                    if *broadcast {
                        self.broadcast_command_message(notify, message);
                    }
                }
                CommandAction::Give {
                    target,
                    network_id,
                    meta,
                    count,
                    nbt,
                    message,
                    notify,
                } => {
                    let Some(session_id) = self.session_id_for(target) else {
                        continue;
                    };
                    let leftover = {
                        let mut map = self.sessions.write();
                        let Some(session) = map.get_mut(&session_id) else {
                            continue;
                        };
                        let mut stack = ItemStack::new(*network_id, *count);
                        stack.meta = *meta;
                        stack.nbt = nbt.clone();
                        let leftover = session.inventory.add_item(&stack);
                        session.sync_inventory();
                        leftover
                    };
                    if leftover > 0 {
                        // PocketMine drops the overflow too ("TODO: overflow").
                        warn!("{} items did not fit in {}'s inventory", leftover, target);
                    }
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::ClearInventory {
                    target,
                    item,
                    max_count,
                    notify,
                } => {
                    let Some(session_id) = self.session_id_for(target) else {
                        continue;
                    };
                    let (count, counted_only, name) = {
                        let mut map = self.sessions.write();
                        let Some(session) = map.get_mut(&session_id) else {
                            continue;
                        };
                        // `ClearCommand` filters with the tagless item parsed
                        // from its argument, so tagged stacks never match.
                        let filter = item.map(|(network_id, meta)| {
                            let mut stack = ItemStack::new(network_id, 1);
                            stack.meta = meta;
                            stack
                        });
                        let (count, counted_only) =
                            session.inventory.clear_matching(filter.as_ref(), *max_count);
                        if !counted_only && count > 0 {
                            session.sync_inventory();
                        }
                        (count, counted_only, session.username.clone())
                    };

                    let count_text = count.to_string();
                    if counted_only {
                        if count > 0 {
                            self.notify_sender(
                                notify,
                                &crate::command::fill_template(
                                    crate::command::lang::CLEAR_TESTING,
                                    &[&name, &count_text],
                                ),
                            );
                        } else {
                            self.notify_sender(
                                notify,
                                &crate::command::red(crate::command::fill_template(
                                    crate::command::lang::CLEAR_FAILURE_NO_ITEMS,
                                    &[&name],
                                )),
                            );
                        }
                    } else if count > 0 {
                        self.broadcast_command_message(
                            notify,
                            &crate::command::fill_template(
                                crate::command::lang::CLEAR_SUCCESS,
                                &[&name, &count_text],
                            ),
                        );
                    } else {
                        self.notify_sender(
                            notify,
                            &crate::command::red(crate::command::fill_template(
                                crate::command::lang::CLEAR_FAILURE_NO_ITEMS,
                                &[&name],
                            )),
                        );
                    }
                }
                CommandAction::SaveAll { message, notify } => {
                    self.broadcast_command_message(notify, message);
                    let started = std::time::Instant::now();
                    if let Err(error) = self.save_all() {
                        warn!("Could not save the server: {}", error);
                    }
                    let elapsed = started.elapsed().as_secs_f64();
                    let success = crate::command::lang::SAVE_SUCCESS
                        .replace("{%0}", &format_seconds(elapsed));
                    self.broadcast_command_message(notify, &success);
                }
                CommandAction::SetAutoSave {
                    enabled,
                    message,
                    notify,
                } => {
                    self.auto_save
                        .store(*enabled, std::sync::atomic::Ordering::SeqCst);
                    // `SaveOnCommand`/`SaveOffCommand` persist `auto-save`.
                    self.persist_server_property(
                        "auto-save",
                        if *enabled { "on" } else { "off" },
                    );
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::BanName {
                    name,
                    reason,
                    message,
                    notify,
                } => {
                    self.access.ban_name(name, reason);
                    let kick_message = if reason.is_empty() {
                        crate::command::lang::DISCONNECT_BAN_NOREASON.to_string()
                    } else {
                        crate::command::lang::DISCONNECT_BAN.replace("{%reason}", reason)
                    };
                    self.kick_player(name, &kick_message);
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::BanIp {
                    ip,
                    reason,
                    message,
                    notify,
                } => {
                    self.access.ban_ip(ip, reason);
                    let kick_message = if reason.is_empty() {
                        crate::command::lang::DISCONNECT_BAN_IP.to_string()
                    } else {
                        crate::command::lang::DISCONNECT_BAN.replace("{%reason}", reason)
                    };
                    self.kick_ip(ip, &kick_message);
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::BanIpPlayer {
                    player,
                    reason,
                    notify,
                } => {
                    let Some(session_id) = self.session_id_for(player) else {
                        continue;
                    };
                    let ip = {
                        let map = self.sessions.read();
                        map.get(&session_id)
                            .map(|session| session.client_addr.ip().to_string())
                    };
                    let Some(ip) = ip else {
                        continue;
                    };
                    self.access.ban_ip(&ip, reason);
                    let kick_message = if reason.is_empty() {
                        crate::command::lang::DISCONNECT_BAN_IP.to_string()
                    } else {
                        crate::command::lang::DISCONNECT_BAN.replace("{%reason}", reason)
                    };
                    self.kick_ip(&ip, &kick_message);
                    let message = crate::command::lang::BANIP_SUCCESS_PLAYERS
                        .replace("{%0}", &ip)
                        .replace("{%1}", player);
                    self.broadcast_command_message(notify, &message);
                }
                CommandAction::UnbanName {
                    name,
                    message,
                    notify,
                } => {
                    self.access.pardon_name(name);
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::UnbanIp {
                    ip,
                    message,
                    notify,
                } => {
                    self.access.pardon_ip(ip);
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::SetWhitelist {
                    enabled,
                    message,
                    notify,
                } => {
                    self.access.set_whitelist_enabled(*enabled);
                    // `WhitelistCommand` persists the `white-list` property.
                    self.persist_server_property(
                        "white-list",
                        if *enabled { "on" } else { "off" },
                    );
                    if *enabled {
                        self.kick_non_whitelisted();
                    }
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::WhitelistReload { notify } => {
                    self.access.reload_whitelist();
                    if self.access.whitelist_enabled() {
                        self.kick_non_whitelisted();
                    }
                    self.broadcast_command_message(
                        notify,
                        crate::command::lang::WHITELIST_RELOADED,
                    );
                }
                CommandAction::AddWhitelist {
                    name,
                    message,
                    notify,
                } => {
                    self.access.add_whitelist(name);
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::RemoveWhitelist {
                    name,
                    message,
                    notify,
                } => {
                    self.access.remove_whitelist(name);
                    if !self.access.is_whitelisted(name) {
                        self.kick_player(
                            name,
                            &crate::command::lang::DISCONNECT_KICK.replace(
                                "{%reason}",
                                crate::command::lang::DISCONNECT_WHITELISTED,
                            ),
                        );
                    }
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::StopServer { message, notify } => {
                    self.broadcast_command_message(notify, message);
                    self.request_stop();
                }
                CommandAction::SetSpawnPoint {
                    target,
                    position,
                    message,
                    notify,
                } => {
                    let Some(session_id) = self.session_id_for(target) else {
                        continue;
                    };
                    if let Some(session) = self.sessions.write().get_mut(&session_id) {
                        session.spawn_point = Some(*position);
                    }
                    // `Player::setSpawn()` tells the affected client about it.
                    self.queue_packet(
                        target,
                        OutgoingPacket::SetSpawnPosition(SetSpawnPositionPacket::new_player_spawn(
                            BlockPos::new(
                                position.0.floor() as i32,
                                position.1.floor() as i32,
                                position.2.floor() as i32,
                            ),
                            0,
                        )),
                    );
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::SetDefaultGamemode(gamemode) => {
                    self.default_gamemode
                        .store(*gamemode, std::sync::atomic::Ordering::Relaxed);
                }
                CommandAction::SetDifficulty {
                    difficulty,
                    message,
                    notify,
                } => {
                    self.world.set_difficulty(*difficulty);
                    self.queue_all(OutgoingPacket::RawWire(set_difficulty_wire(
                        *difficulty as u32,
                    )));
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::SetOp { target, op } => {
                    let key = target.to_lowercase();
                    let names = {
                        let mut ops = self.ops.write();
                        if *op {
                            ops.insert(key.clone());
                        } else {
                            ops.remove(&key);
                        }
                        let mut names: Vec<String> = ops.iter().cloned().collect();
                        names.sort();
                        names
                    };
                    let mut contents = names.join("\n");
                    if !contents.is_empty() {
                        contents.push('\n');
                    }
                    if let Err(error) = std::fs::write("ops.txt", contents) {
                        warn!("Could not persist ops.txt: {}", error);
                    }
                    if let Some(mut player) = self.registry.get(target) {
                        player.is_op = *op;
                        self.registry.upsert(player);
                    }
                }
            }
        }
    }

    /// Moves an online player and streams the chunks around the destination.
    fn teleport_player(
        &self,
        name: &str,
        x: f64,
        y: f64,
        z: f64,
        yaw: Option<f32>,
        pitch: Option<f32>,
    ) -> bool {
        let Some(session_id) = self.session_id_for(name) else {
            return false;
        };
        let mut map = self.sessions.write();
        let Some(session) = map.get_mut(&session_id) else {
            return false;
        };
        session.teleport(x, y, z, yaw, pitch);
        registry_upsert(&self.registry, session);
        true
    }

    fn broadcast_time(&self, time: i32) {
        self.queue_all(OutgoingPacket::RawWire(set_time_wire(time)));
    }

    /// Resends a chunk to every session that already has it loaded.
    fn refresh_chunk_at(&self, x: i32, z: i32) {
        let chunk = (x >> 4, z >> 4);
        let mut map = self.sessions.write();
        for session in map.values_mut() {
            if !session.joined || !session.sent_chunks.contains(&chunk) {
                continue;
            }
            session.sent_chunks.remove(&chunk);
            session
                .pending_outgoing
                .push(OutgoingPacket::LevelChunk(session.create_chunk_packet(chunk.0, chunk.1)));
        }
    }
}

/// InventorySlotPacket (0x32) wire bytes.
fn inventory_slot_wire(window_id: u32, slot: u32, item: &ItemStack) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(InventorySlotPacket::PACKET_ID);
    InventorySlotPacket {
        window_id,
        inventory_slot: slot,
        container_id: None,
        dynamic_container_id: None,
        storage: None,
        item: item.clone(),
    }
    .encode_payload(&mut writer);
    writer.into_bytes().to_vec()
}

/// PHP's `(string) round($value, 3)`.
fn format_seconds(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    if rounded.fract() == 0.0 {
        return format!("{}", rounded as i64);
    }
    let mut text = format!("{:.3}", rounded);
    while text.ends_with('0') {
        text.pop();
    }
    text
}

/// SetDifficultyPacket (0x3C) wire bytes.
fn set_difficulty_wire(difficulty: u32) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(SetDifficultyPacket::PACKET_ID);
    SetDifficultyPacket::new(difficulty).encode_payload(&mut writer);
    writer.into_bytes().to_vec()
}

/// SetHealthPacket (0x2A) wire bytes.
fn set_health_wire(health: f32) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(SetHealthPacket::PACKET_ID);
    SetHealthPacket::new(health.round() as i32).encode_payload(&mut writer);
    writer.into_bytes().to_vec()
}

/// InventoryContentPacket (0x31) with the player's 36 main slots.
fn inventory_content_wire(items: &[ItemStack]) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(InventoryContentPacket::PACKET_ID);
    InventoryContentPacket::inventory(items.to_vec()).encode_payload(&mut writer);
    writer.into_bytes().to_vec()
}

/// ActorEventPacket (0x1B) wire bytes.
fn actor_event_wire(runtime_id: u64, event_id: u8) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(ActorEventPacket::PACKET_ID);
    ActorEventPacket::new(runtime_id, event_id, 0).encode_payload(&mut writer);
    writer.into_bytes().to_vec()
}

/// RespawnPacket (0x2D) wire bytes.
fn respawn_wire(position: (f32, f32, f32), state: u8, runtime_id: u64) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(RespawnPacket::PACKET_ID);
    RespawnPacket::new(position, state, runtime_id).encode_payload(&mut writer);
    writer.into_bytes().to_vec()
}

/// AvailableCommandsPacket (0x4C) carrying the native command table.
fn available_commands_wire() -> Vec<u8> {
    let commands: Vec<CommandSpec> = crate::command::COMMANDS
        .iter()
        // The client ships its own /help, exactly like Axolotl skips it.
        .filter(|info| info.name != "help")
        .map(|info| CommandSpec {
            name: info.name.to_string(),
            description: info.description.to_string(),
            aliases: info.aliases.iter().map(|alias| alias.to_string()).collect(),
        })
        .collect();

    let packet = AvailableCommandsPacket { commands };
    let mut writer = Writer::with_capacity(4096);
    writer.write_var_u32(AvailableCommandsPacket::PACKET_ID);
    packet.encode_payload(&mut writer);
    writer.into_bytes().to_vec()
}

/// SetTimePacket (0x0A): absolute world time in ticks.
fn set_time_wire(time: i32) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(0x0A);
    writer.write_var_i32(time);
    writer.into_bytes().to_vec()
}

/// LevelEventPacket (0x19) at the origin, used for weather changes.
fn level_event_wire(event_id: i32, duration_ticks: i32) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(0x19);
    writer.write_var_i32(event_id);
    writer.write_f32_le(0.0);
    writer.write_f32_le(0.0);
    writer.write_f32_le(0.0);
    writer.write_var_i32(duration_ticks);
    writer.into_bytes().to_vec()
}

/// Updates the registry snapshot for a session (used after teleports).
fn registry_upsert(registry: &PlayerRegistry, session: &PlayerSession) {
    registry.upsert(session.player_context());
}

/// Builds a TextPacket of any category from public fields.
fn text_with_type(text_type: u8, message: &str) -> TextPacket {
    TextPacket {
        needs_translation: false,
        text_type,
        source_name: String::new(),
        message: message.to_string(),
        parameters: Vec::new(),
        xbox_user_id: String::new(),
        platform_chat_id: String::new(),
    }
}
