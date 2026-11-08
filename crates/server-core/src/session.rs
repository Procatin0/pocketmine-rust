use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bedrock_protocol::binary::{CodecError, Reader, Writer};
use bedrock_protocol::{
    AddPlayerPacket, AvailableCommandsPacket, BlockPos, ChunkRadiusUpdatedPacket,
    CommandRequestPacket, CommandSpec, DisconnectPacket, LevelChunkPacket, LoginPacket,
    MetadataEntry, MetadataValue, MoveActorAbsolutePacket, MovePlayerPacket,
    NetworkChunkPublisherUpdatePacket,
    NetworkSettingsPacket, Packet, PacketBatch, PacketViolationWarningPacket, PlayStatusPacket,
    PlayerAuthInputPacket, PlayerListEntry, PlayerListPacket, RemoveActorPacket,
    RequestAbilityPacket, RequestChunkRadiusPacket, RequestNetworkSettingsPacket,
    ResourcePackClientResponsePacket, ResourcePackStackPacket, ResourcePacksInfoPacket,
    SetActorDataPacket, SetPlayerGameTypePacket, SetSpawnPositionPacket, SkinData,
    StartGamePacket, ActorEventPacket, InventoryContentPacket, InventorySlotPacket,
    InventoryTransactionPacket, ItemStack, RespawnPacket, SetDifficultyPacket, SetHealthPacket,
    SetTitlePacket, TextPacket, TransactionData, TransferPacket, UpdateAbilitiesPacket,
    UpdatePlayerGameTypePacket, Vector3f,
};
use bedrock_protocol::packets::actor_event::EVENT_DEATH_ANIMATION;
use bedrock_protocol::packets::inventory_transaction::{
    ACTION_CLICK_BLOCK, UseItemData,
};
use bedrock_protocol::packets::player_auth_input::{
    has_input_flag, parse_block_actions, parse_input_flags, BLOCK_ACTION_PREDICT_DESTROY,
    BLOCK_ACTION_START_BREAK, INPUT_FLAG_MISSED_SWING, INPUT_FLAG_START_FLYING,
    INPUT_FLAG_START_GLIDING, INPUT_FLAG_START_JUMPING, INPUT_FLAG_START_SNEAKING,
    INPUT_FLAG_START_SPRINTING, INPUT_FLAG_START_SWIMMING, INPUT_FLAG_STOP_FLYING,
    INPUT_FLAG_STOP_GLIDING, INPUT_FLAG_STOP_SNEAKING, INPUT_FLAG_STOP_SPRINTING,
    INPUT_FLAG_STOP_SWIMMING,
};
use bedrock_protocol::packets::player_list::uuid_from_str;
use bedrock_world::{block_id::AIR, FlatGenerator, WorldGenerator};
use parking_lot::RwLock;
use php_plugin_bridge::{
    BlockBreakEvent, BlockPlaceEvent, CommandSender, Event, PlayerChatEvent, PlayerCommandPreprocessEvent,
    PlayerContext, PlayerDeathEvent, PlayerInteractEvent, PlayerJoinEvent, PlayerMoveEvent,
    PlayerQuitEvent, PlayerRespawnEvent, PluginAction, PluginManager,
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
use crate::world_state::{BlockChange, WorldState, WORLD_MAX_Y, WORLD_MIN_Y};

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

/// Player eye height above the feet (Human::getOffsetPosition, 1.62 in InGamePacketHandler).
pub const EYE_HEIGHT: f32 = 1.62;
/// Entity metadata flags (bits of DATA_FLAGS long).
pub const FLAG_SNEAKING: i64 = 1 << 1;
pub const FLAG_SPRINTING: i64 = 1 << 3;
pub const FLAG_GLIDING: i64 = 1 << 32;
pub const FLAG_SWIMMING: i64 = 1 << 57;
pub const FLAG_CAN_CLIMB: i64 = 1 << 19;
pub const FLAG_HAS_COLLISION: i64 = 1 << 48;
pub const FLAG_AFFECTED_BY_GRAVITY: i64 = 1 << 49;

/// How long a session may sit in PreSpawn without `0x71` before the server
/// forces `PLAYER_SPAWN` anyway (Bedrock 1.26 clients that only send
/// `RequestChunkRadius` / `PlayerAuthInput`).
pub const PRESPAWN_SPAWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// `UpdateBlockPacket` id (Fase 2, Java -> Bedrock).
///
/// NOTE: this is `0x15` (21), *not* `0x0B`. `0x0B` is `StartGamePacket`
/// (see `data/protocol_info.json`: `"UpdateBlockPacket": 21`,
/// `"StartGamePacket": 11`); sending `0x0B` here would make the client
/// try to parse a StartGame. The plan's "0x0B" label is that mix-up.
pub const UPDATE_BLOCK_PACKET_ID: u32 = 0x15;
/// Update flags for `UpdateBlockPacket`: neighbor updates + send to
/// clients (`BLOCK_UPDATE_ALL` in PocketMine).
pub const UPDATE_BLOCK_FLAGS_ALL: u8 = 0x03;

/// Ticks between periodic `SetTimePacket` broadcasts to Bedrock
/// clients (20 ticks = 1 second at 20 TPS). Clients advance the sun
/// locally between packets, so 1 Hz keeps them in lock-step with the
/// server clock without a packet per tick; jumps (`/time set`) are
/// broadcast immediately instead of waiting for this interval.
pub const TIME_BROADCAST_INTERVAL_TICKS: i32 = 20;

/// `UpdateBlockPacket` (0x15) wire bytes for one block.
///
/// Layout mirrors PocketMine's `UpdateBlockPacket` serializer:
/// packet id (var_u32), `BlockPos` (var_i32 x/y/z), runtime block id
/// (zigzag var_i32 — the same encoding the chunk palette uses for these
/// FNV-1a hash ids, so `block_id as i32` round-trips the stored `u32`),
/// flags (byte, [`UPDATE_BLOCK_FLAGS_ALL`]).
pub fn update_block_wire(x: i32, y: i32, z: i32, block_id: u32) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(UPDATE_BLOCK_PACKET_ID);
    BlockPos::new(x, y, z).write(&mut writer);
    writer.write_var_i32(block_id as i32);
    writer.write_byte(UPDATE_BLOCK_FLAGS_ALL);
    writer.into_bytes().to_vec()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Connected,
    ResourcePacks,
    ResourcePackStack,
    PreSpawn,
    InGame,
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
    AddPlayer(AddPlayerPacket),
    PlayerList(PlayerListPacket),
    SetActorData(SetActorDataPacket),
    MoveActorAbsolute(MoveActorAbsolutePacket),
    RemoveActor(RemoveActorPacket),
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
            Self::AddPlayer(p) => p.packet_id(),
            Self::PlayerList(p) => p.packet_id(),
            Self::SetActorData(p) => p.packet_id(),
            Self::MoveActorAbsolute(p) => p.packet_id(),
            Self::RemoveActor(p) => p.packet_id(),
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
            Self::AddPlayer(p) => p.encode(writer),
            Self::PlayerList(p) => p.encode(writer),
            Self::SetActorData(p) => p.encode(writer),
            Self::MoveActorAbsolute(p) => p.encode(writer),
            Self::RemoveActor(p) => p.encode(writer),
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
    /// Bedrock UUID used in AddPlayer / PlayerList.
    pub uuid: [u8; 16],
    /// PLAYER_SPAWN has been sent and doFirstSpawn completed.
    pub spawned: bool,
    /// When PreSpawn was entered (`pub` for tests); `None` once spawned.
    pub prespawn_started_at: Option<std::time::Instant>,
    /// A move happened and must be broadcast to viewers by the manager.
    pub pending_move_broadcast: bool,
    /// Entity flags changed and must be broadcast to viewers by the manager.
    pub pending_data_broadcast: bool,
    /// A first spawn happened and must be broadcast by the manager.
    pub pending_spawn_broadcast: bool,
    /// Sessions that have spawned this player (Entity::hasSpawned).
    pub viewers: HashSet<u64>,
    /// Players this session hides (Player::hiddenPlayers -> canSee).
    pub hidden_players: HashSet<u64>,
    /// Movement toggles driven by PlayerAuthInputFlags.
    pub is_sneaking: bool,
    pub is_sprinting: bool,
    pub is_swimming: bool,
    pub is_gliding: bool,
    /// Last decoded PlayerAuthInput flag mask, for edge detection.
    pub last_input_flags: u128,
    /// Hunger (0..20) and saturation (0..20), HungerManager.
    pub food: f32,
    pub saturation: f32,
    /// Experience level/progress, ExperienceManager.
    pub xp_level: i32,
    pub xp_progress: f32,
    /// Breath ticks, Living::breathTicks (max 300).
    pub air: i16,
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
    /// `Player::getFirstPlayed()` (0.0 = never played, loaded from JSON v2).
    pub first_played: f64,
    /// `Player::getLastPlayed()` (0.0 = never played, loaded from JSON v2).
    pub last_played: f64,
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
            uuid: derive_uuid_for_session(session_id, &format!("Player_{}", session_id)),
            spawned: false,
            prespawn_started_at: None,
            pending_move_broadcast: false,
            pending_data_broadcast: false,
            pending_spawn_broadcast: false,
            viewers: HashSet::new(),
            hidden_players: HashSet::new(),
            is_sneaking: false,
            is_sprinting: false,
            is_swimming: false,
            is_gliding: false,
            last_input_flags: 0,
            food: 20.0,
            saturation: 20.0,
            xp_level: 0,
            xp_progress: 0.0,
            air: 300,
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
            first_played: 0.0,
            last_played: 0.0,
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

    /// `Player::hasPlayedBefore(): bool`: data existed before this session
    /// (`has_saved_data` from load) or `last_played` exceeds `first_played`
    /// by more than 1ms (axolotl threshold).
    pub fn has_played_before(&self) -> bool {
        self.has_saved_data
            || (self.first_played > 0.0 && self.last_played > self.first_played + 0.001)
    }

    /// Epoch seconds with sub-second precision (`microtime(true)` in PHP).
    pub fn now_played_secs() -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs_f64())
            .unwrap_or(0.0)
    }

    /// Writes JSON v2 (26 campos) directly to `players/<name>.json`.
    ///
    /// PocketMine uses `players/<name>.dat` (NBT); JSON keeps the same data
    /// without an NBT serializer. `y` is canonical Bedrock world Y (never
    /// legacy `y+64`). Virtual (Java) saves write this exact object to the
    /// same path via `player.save` — no staging file.
    pub fn save_player_data(&self) -> std::io::Result<()> {
        let Some(path) = self.player_data_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let now = Self::now_played_secs();
        // Preserve previously saved first/last when this session never
        // loaded them (e.g. autosave before login finished).
        let existing_first_last = if self.first_played <= 0.0 {
            std::fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        } else {
            None
        };
        let first_played = if self.first_played > 0.0 {
            self.first_played
        } else if let Some(existing) = existing_first_last.as_ref() {
            existing.get("first_played").and_then(|entry| entry.as_f64()).filter(|v| *v > 0.0).unwrap_or(now)
        } else {
            now
        };
        let mut snapshot = self.player_context();
        snapshot.first_played = first_played;
        snapshot.last_played = now;
        let value = snapshot.to_save_json();
        let bytes = serde_json::to_vec(&value)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
        std::fs::write(path, bytes)
    }

    /// Restores a snapshot written by [`PlayerSession::save_player_data`].
    /// Old files (8-field v1) load with defaults for the new fields.
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
        let first_number = |keys: &[&str]| keys.iter().filter_map(|key| number(key)).next();
        let first_bool = |keys: &[&str]| {
            keys.iter()
                .filter_map(|key| value.get(*key).and_then(|entry| entry.as_bool()))
                .next()
        };

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
        if let Some(max_health) = first_number(&["max_health", "maxHealth"]) {
            self.max_health = max_health.max(1.0) as f32;
        }
        if let Some(food) = number("food") {
            self.food = food as f32;
        }
        if let Some(saturation) = number("saturation") {
            self.saturation = saturation as f32;
        }
        if let Some(xp) = number("xp") {
            self.xp_progress = xp as f32;
        }
        if let Some(level) = value
            .get("xp_level")
            .or_else(|| value.get("xpLevel"))
            .and_then(|entry| entry.as_i64())
        {
            self.xp_level = level as i32;
        }
        if let Some(air) = value.get("air").and_then(|entry| entry.as_i64()) {
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
        if let Some(xuid) = value.get("xuid").and_then(|entry| entry.as_str()) {
            if !xuid.is_empty() {
                self.xuid = xuid.to_string();
            }
        }
        if let Some(first) = first_number(&["first_played", "firstPlayed"]) {
            self.first_played = first;
        }
        if let Some(last) = first_number(&["last_played", "lastPlayed"]) {
            self.last_played = last;
        }
        if let Some(point) = value
            .get("spawn_point")
            .or_else(|| value.get("spawnPoint"))
            .and_then(|entry| entry.as_array())
        {
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
            is_virtual: false,
            xuid: self.xuid.clone(),
            first_played: self.first_played,
            last_played: self.last_played,
            food: self.food,
            saturation: self.saturation,
            xp: self.xp_progress,
            xp_level: self.xp_level,
            air: self.air,
            is_flying: self.is_flying,
            spawn_point: self.spawn_point.map(|(x, y, z)| [x, y, z]),
            is_sneaking: self.is_sneaking,
            is_sprinting: self.is_sprinting,
            is_swimming: self.is_swimming,
            is_gliding: self.is_gliding,
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
        // Viewers must see the same teleport as a MoveActorAbsolute.
        self.pending_move_broadcast = true;
    }

    fn register_self(&self) {
        if let Some(registry) = &self.registry {
            // A virtual player already owns this name: do not downgrade the
            // entry to Bedrock (keeps /list counting it exactly once).
            if let Some(existing) = registry.get(&self.username) {
                if existing.is_virtual {
                    return;
                }
            }
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
        self.state == SessionState::InGame || self.state == SessionState::PreSpawn
    }

    pub fn is_spawned(&self) -> bool {
        self.spawned
    }

    /// Time since PreSpawn was entered, when still waiting for `0x71`.
    pub fn prespawn_elapsed(&self) -> Option<std::time::Duration> {
        self.prespawn_started_at.map(|t| t.elapsed())
    }

    /// True when this session is stuck in PreSpawn past `threshold` without
    /// `0x71` and must be force-spawned (chunk-radius / input / timeout fallback).
    pub fn is_prespawn_timed_out(&self, threshold: std::time::Duration) -> bool {
        self.state == SessionState::PreSpawn
            && !self.spawned
            && self
                .prespawn_started_at
                .map(|t| t.elapsed() >= threshold)
                .unwrap_or(false)
    }

    pub fn feet_position(&self) -> Vector3f {
        self.position
    }

    pub fn eye_position(&self) -> Vector3f {
        Vector3f::new(self.position.x, self.position.y + EYE_HEIGHT, self.position.z)
    }

    pub fn can_see(&self, other_session_id: u64) -> bool {
        !self.hidden_players.contains(&other_session_id)
    }

    pub fn hide_player(&mut self, other_session_id: u64) -> bool {
        self.hidden_players.insert(other_session_id)
    }

    pub fn show_player(&mut self, other_session_id: u64) -> bool {
        self.hidden_players.remove(&other_session_id)
    }

    pub fn data_flags(&self) -> i64 {
        let mut flags: i64 = FLAG_HAS_COLLISION | FLAG_AFFECTED_BY_GRAVITY | FLAG_CAN_CLIMB;
        if self.is_sneaking {
            flags |= FLAG_SNEAKING;
        }
        if self.is_sprinting {
            flags |= FLAG_SPRINTING;
        }
        if self.is_swimming {
            flags |= FLAG_SWIMMING;
        }
        if self.is_gliding {
            flags |= FLAG_GLIDING;
        }
        flags
    }

    pub fn add_player_packet(&self) -> AddPlayerPacket {
        let metadata = vec![
            MetadataEntry::new(0, MetadataValue::Long(self.data_flags())),
            MetadataEntry::new(4, MetadataValue::String(self.username.clone())),
            MetadataEntry::new(7, MetadataValue::Short(self.air)),
            MetadataEntry::new(42, MetadataValue::Short(self.air)),
            MetadataEntry::new(38, MetadataValue::Float(1.0)),
            MetadataEntry::new(53, MetadataValue::Float(0.6)),
            MetadataEntry::new(54, MetadataValue::Float(1.8)),
        ];
        AddPlayerPacket::new(
            self.uuid,
            self.username.clone(),
            self.entity_runtime_id,
            self.feet_position(),
            self.pitch,
            self.yaw,
            self.yaw,
            self.inventory.held_item(),
            self.gamemode,
            metadata,
            bedrock_protocol::AbilitiesData::visitor(self.entity_unique_id),
        )
    }

    pub fn skin_data(&self) -> SkinData {
        SkinData::default_for(&self.uuid)
    }

    pub fn player_list_addition(&self) -> PlayerListEntry {
        PlayerListEntry::addition(
            self.uuid,
            self.entity_unique_id,
            self.username.clone(),
            self.skin_data(),
            self.xuid.clone(),
        )
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
            0x71 => {
                let mut reader = Reader::new(payload);
                let _runtime_id = reader.read_var_u64().ok();
                self.handle_set_local_player_as_initialized()
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
        self.uuid = uuid_from_str(&identity.uuid)
            .unwrap_or_else(|| derive_uuid_for_session(self.session_id, &self.username));
        debug!(
            "Session {} login attempt with protocol version {} as '{}' (xuid: {})",
            self.session_id, self.protocol_version, self.username, self.xuid
        );
        // `NetworkSession`: the name is known after login.
        info!(
            "{}",
            crate::translations::Translator::english()
                .fill_named("pocketmine.network.session.playerName", &[("playerName", &format!("\u{a7}b{}\u{a7}r", self.username))])
                .unwrap_or_else(|| format!("Player: {}", self.username))
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
                debug!("Session {} refused resource packs; disconnecting.", self.session_id);
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
                debug!("Session {} accepted packs, sending ResourcePackStack", self.session_id);
                Ok(vec![OutgoingPacket::ResourcePackStack(stack)])
            }
            3 | 4 if self.state == SessionState::ResourcePackStack || self.state == SessionState::ResourcePacks => {
                self.state = SessionState::PreSpawn;
                self.spawned = false;
                self.prespawn_started_at = Some(std::time::Instant::now());
                debug!("Session {} entered world (InGame) - sending pre-spawn declaration and world packets", self.session_id);

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
                start_game.player_position = self.eye_position();
                start_game.level_settings.seed = self.world.seed();
                // Per-player spawn (`/spawnpoint`) wins over the world spawn,
                // like `Player::getSpawn()` vs `World::getSpawnLocation()`.
                let (sx, sy, sz) = match self.spawn_point {
                    Some((x, y, z)) => (x.floor() as i32, y.floor() as i32, z.floor() as i32),
                    None => self.world.spawn(),
                };
                start_game.level_settings.spawn_position = BlockPos::new(sx, sy, sz);
                start_game.level_settings.generator_type = self.generator.generator_type();
                start_game.level_settings.world_gamemode = self.gamemode;
                start_game.level_settings.difficulty = self.world.difficulty();
                start_game.level_settings.time = self.world.time();
                start_game.level_settings.dimension = 0;
                start_game.world_name = "Axolotl-PM World".to_string();
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
                for window_wire in self.inventory_window_wires() {
                    out.push(OutgoingPacket::RawWire(window_wire));
                }

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

                // 10d. SetDifficultyPacket (0x3C / 60): live value, not the
                // captured static (easy) snapshot.
                out.push(OutgoingPacket::RawWire(set_difficulty_wire(
                    self.world.difficulty() as u32,
                )));

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

                // NOTE: PLAYER_SPAWN is intentionally NOT sent here. It is sent
                // by handle_set_local_player_as_initialized once the client
                // confirms it is initialized (0x71), or by the chunk-radius /
                // auth-input / timeout fallback when `0x71` never arrives.

                // Visible at default log level so a silent login (StartGame
                // queued but never acknowledged) is distinguishable from a
                // malformed StartGame the client dropped.
                info!(
                    "Session {} sent StartGame (unique={}, runtime={}, gamemode={}) and entered PreSpawn (awaiting 0x71; chunk-radius/input and {:?} timeout will force spawn)",
                    self.session_id,
                    self.entity_unique_id,
                    self.entity_runtime_id,
                    self.gamemode,
                    PRESPAWN_SPAWN_TIMEOUT,
                );

                Ok(out)
            }
            other => {
                debug!("Session {} sent unknown resource pack status: {}", self.session_id, other);
                Ok(Vec::new())
            }
        }
    }

    /// NetworkSession::onEnterWorld: syncs Time / Difficulty / Spawn.
    fn on_enter_world_packets(&self) -> Vec<OutgoingPacket> {
        let (sx, sy, sz) = self.world.spawn();
        vec![
            OutgoingPacket::RawWire(set_time_wire(self.world.time())),
            OutgoingPacket::RawWire(set_difficulty_wire(self.world.difficulty() as u32)),
            OutgoingPacket::SetSpawnPosition(SetSpawnPositionPacket::new_player_spawn(
                BlockPos::new(sx, sy, sz),
                0,
            )),
        ]
    }

    /// Player::doFirstSpawn equivalent: sends PLAYER_SPAWN, enters the
    /// world and fires PlayerJoinEvent exactly once. Visibility
    /// (spawnToAll) is performed by the manager after this returns.
    pub fn handle_set_local_player_as_initialized(
        &mut self,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        if self.state != SessionState::PreSpawn && self.state != SessionState::InGame {
            return Ok(Vec::new());
        }
        if self.spawned {
            return Ok(Vec::new());
        }
        self.spawned = true;
        self.prespawn_started_at = None;
        self.state = SessionState::InGame;
        self.pending_spawn_broadcast = true;
        debug!(
            "Session {} handle_set_local_player_as_initialized: PLAYER_SPAWN sent (InGame)",
            self.session_id
        );

        let mut out = vec![OutgoingPacket::PlayStatus(PlayStatusPacket {
            status: PlayStatusPacket::PLAYER_SPAWN,
        })];

        out.extend(self.on_enter_world_packets());

        if !self.joined {
            self.joined = true;
            self.register_self();
            let mut event = PlayerJoinEvent::new(
                self.player_context(),
                format!("{} joined the game", self.username),
            );
            self.fire_event(&mut event);
            // `Server::addOnlinePlayer`: `pocketmine.player.logIn` (AQUA name).
            {
                let fmt_coord = |v: f64| -> String {
                    let s = format!("{:.4}", v);
                    let s = s.trim_end_matches('0').trim_end_matches('.');
                    if s.is_empty() || s == "-0" { "0".to_string() } else { s.to_string() }
                };
                let name = format!("\u{a7}b{}\u{a7}r", self.username);
                let ip = self.client_addr.ip().to_string();
                let port = self.client_addr.port().to_string();
                let id = self.entity_runtime_id.to_string();
                let world = self.world.name.clone();
                let x = fmt_coord(self.position.x as f64);
                let y = fmt_coord(self.position.y as f64);
                let z = fmt_coord(self.position.z as f64);
                info!(
                    "{}",
                    crate::translations::Translator::english()
                        .fill("pocketmine.player.logIn", &[&name, &ip, &port, &id, &world, &x, &y, &z])
                        .unwrap_or_else(|| format!("{}[/{}:{}] logged in with entity id {} at ({}, {}, {}, {})", self.username, ip, port, id, world, x, y, z))
                );
            }
            if !event.join_message.is_empty() {
                self.pending_broadcasts.push(event.join_message);
            }
        }

        Ok(out)
    }

    /// Handles RequestChunkRadiusPacket (0x45).
    ///
    /// Fallback: newer Bedrock clients send `RequestChunkRadius` right after
    /// `StartGame` but never send `SetLocalPlayerAsInitialized` (`0x71`).
    /// Waiting for `0x71` there left the session in PreSpawn forever
    /// (no `PLAYER_SPAWN`, no `PlayerJoinEvent`, TestKit saw 0 online).
    /// The first chunk-radius request therefore force-spawns, idempotently:
    /// when `0x71` arrives later it is a no-op because `spawned` is set.
    pub fn handle_chunk_radius_request(
        &mut self,
        packet: RequestChunkRadiusPacket,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        if !(self.state == SessionState::InGame || self.state == SessionState::PreSpawn) {
            return Err(SessionError::UnexpectedPacket {
                state: self.state,
                packet_id: 0x45,
            });
        }

        let mut spawn_fallback: Vec<OutgoingPacket> = Vec::new();
        if self.state == SessionState::PreSpawn && !self.spawned {
            info!(
                "Session {} RequestChunkRadius arrived before 0x71; forcing PLAYER_SPAWN fallback",
                self.session_id
            );
            spawn_fallback = self.handle_set_local_player_as_initialized()?;
        }

        let clamped_radius = packet.radius.clamp(4, self.max_view_distance);
        self.chunk_radius = clamped_radius;
        debug!(
            "Session {} handle_chunk_radius_request: requested={}, clamped={}",
            self.session_id, packet.radius, clamped_radius
        );

        let center_pos = BlockPos::new(
            self.position.x.floor() as i32,
            self.position.y.floor() as i32,
            self.position.z.floor() as i32,
        );

        let publisher_radius_blocks = (((clamped_radius as f64 * std::f64::consts::SQRT_2) + 2.0).ceil() as u32) * 16;

        let mut out = spawn_fallback;
        out.push(OutgoingPacket::ChunkRadiusUpdated(ChunkRadiusUpdatedPacket::new(clamped_radius)));
        out.push(OutgoingPacket::NetworkChunkPublisherUpdate(
            NetworkChunkPublisherUpdatePacket::new(center_pos, publisher_radius_blocks),
        ));

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
        debug!(
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
    /// Handles PlayerAuthInputPacket (0x90).
    ///
    /// Mirrors InGamePacketHandler::handlePlayerAuthInput:
    /// - NaN/Inf in position/rotation is discarded,
    /// - the client position is the eye (- 1.62 gives the feet),
    /// - rotation is folded into [0, 360),
    /// - sneak/sprint/swim/glide/fly toggles are edge-detected and flagged
    ///   for a SetActorData broadcast by the manager.
    pub fn handle_player_auth_input(
        &mut self,
        packet: PlayerAuthInputPacket,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        if !(self.state == SessionState::InGame || self.state == SessionState::PreSpawn) {
            return Ok(Vec::new());
        }
        // Same `0x71`-missing fallback as `RequestChunkRadius`: a client that
        // starts sending movement inputs has clearly received `StartGame`
        // but will never confirm init, so spawn it now instead of dropping
        // every input forever.
        let mut spawn_fallback: Vec<OutgoingPacket> = Vec::new();
        if self.state == SessionState::PreSpawn && !self.spawned {
            info!(
                "Session {} PlayerAuthInput arrived before 0x71; forcing PLAYER_SPAWN fallback",
                self.session_id
            );
            spawn_fallback = self.handle_set_local_player_as_initialized()?;
        }
        for component in [
            packet.position.x,
            packet.position.y,
            packet.position.z,
            packet.pitch,
            packet.yaw,
            packet.head_yaw,
            packet.move_vec_x,
            packet.move_vec_z,
        ] {
            if !component.is_finite() {
                return Ok(spawn_fallback);
            }
        }
        let raw_eye = packet.position;
        let raw_yaw = packet.yaw;
        let raw_pitch = packet.pitch;
        let mut yaw = raw_yaw % 360.0;
        if yaw < 0.0 {
            yaw += 360.0;
        }
        let mut pitch = raw_pitch % 360.0;
        if pitch < 0.0 {
            pitch += 360.0;
        }
        let new_feet = Vector3f::new(
            (raw_eye.x * 10_000.0).round() / 10_000.0,
            ((raw_eye.y - EYE_HEIGHT) * 10_000.0).round() / 10_000.0,
            (raw_eye.z * 10_000.0).round() / 10_000.0,
        );

        // If a teleport is pending (force_move_sync), ignore the client's
        // reported position until it matches the server's position. This
        // matches PocketMine's forceMoveSync and prevents desync after /tp.
        if self.force_move_sync {
            let dx = new_feet.x - self.position.x;
            let dy = new_feet.y - self.position.y;
            let dz = new_feet.z - self.position.z;
            let distance_sq = dx * dx + dy * dy + dz * dz;
            // Client has confirmed the teleported position (PM tolerates 1
            // block for client-side physics at spawn).
            if distance_sq <= 1.0 {
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
                return Ok(spawn_fallback);
            }
        }

        let old_cx = (self.position.x.floor() as i32) >> 4;
        let old_cz = (self.position.z.floor() as i32) >> 4;

        let from = (
            self.position.x as f64,
            self.position.y as f64,
            self.position.z as f64,
        );
        let to = (new_feet.x as f64, new_feet.y as f64, new_feet.z as f64);
        let has_moved_for_move = (from.0 - to.0).abs() > 0.0001
            || (from.1 - to.1).abs() > 0.0001
            || (from.2 - to.2).abs() > 0.0001
            || (self.yaw - yaw).abs() > 0.0001
            || (self.pitch - pitch).abs() > 0.0001;

        // PlayerMoveEvent is only dispatched when a plugin actually listens to
        // it, and throttled to every other input to keep the IPC round-trips
        // bounded. Cancelling restores the previous position.
        if self
            .plugins
            .as_ref()
            .map(|plugins| plugins.has_listeners("PlayerMoveEvent"))
            .unwrap_or(false)
            && has_moved_for_move
        {
            self.move_events = self.move_events.wrapping_add(1);
            if self.move_events % 2 == 0 {
                let mut event = PlayerMoveEvent::new(self.player_context(), from, to);
                self.fire_event(&mut event);
                if event.cancelled {
                    return Ok(spawn_fallback);
                }
            }
        }

        // Rotation is always accepted; position follows PM's
        // if (!$this->forceMoveSync && $hasMoved).
        let position_changed = !self.force_move_sync && has_moved_for_move;
        self.yaw = yaw;
        self.pitch = pitch;
        if position_changed {
            self.position = new_feet;
            self.pending_move_broadcast = true;
        }
        self.register_self();

        // Movement toggles (PlayerAuthInputFlags -> Living flags).
        let flags = parse_input_flags(&packet.input_data);
        if flags != self.last_input_flags {
            let mut data_changed = false;
            if has_input_flag(flags, INPUT_FLAG_START_SNEAKING) && !self.is_sneaking {
                self.is_sneaking = true;
                data_changed = true;
            } else if has_input_flag(flags, INPUT_FLAG_STOP_SNEAKING) && self.is_sneaking {
                self.is_sneaking = false;
                data_changed = true;
            }
            if has_input_flag(flags, INPUT_FLAG_START_SPRINTING) && !self.is_sprinting {
                self.is_sprinting = true;
                data_changed = true;
            } else if has_input_flag(flags, INPUT_FLAG_STOP_SPRINTING) && self.is_sprinting {
                self.is_sprinting = false;
                data_changed = true;
            }
            if has_input_flag(flags, INPUT_FLAG_START_SWIMMING) && !self.is_swimming {
                self.is_swimming = true;
                data_changed = true;
            } else if has_input_flag(flags, INPUT_FLAG_STOP_SWIMMING) && self.is_swimming {
                self.is_swimming = false;
                data_changed = true;
            }
            if has_input_flag(flags, INPUT_FLAG_START_GLIDING) && !self.is_gliding {
                self.is_gliding = true;
                data_changed = true;
            } else if has_input_flag(flags, INPUT_FLAG_STOP_GLIDING) && self.is_gliding {
                self.is_gliding = false;
                data_changed = true;
            }
            if has_input_flag(flags, INPUT_FLAG_START_FLYING) {
                if self.gamemode == 1 && !self.is_flying {
                    self.is_flying = true;
                    self.pending_outgoing
                        .push(OutgoingPacket::RawWire(self.create_update_abilities_packet()));
                }
            } else if has_input_flag(flags, INPUT_FLAG_STOP_FLYING) {
                if self.is_flying {
                    self.is_flying = false;
                    self.pending_outgoing
                        .push(OutgoingPacket::RawWire(self.create_update_abilities_packet()));
                }
            }
            let _ = has_input_flag(flags, INPUT_FLAG_START_JUMPING);
            let _ = has_input_flag(flags, INPUT_FLAG_MISSED_SWING);
            if data_changed {
                self.pending_data_broadcast = true;
            }
            self.last_input_flags = flags;
        }

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

        let mut out = spawn_fallback;
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
            debug!(
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
        // above. Parsing is best-effort and never affects movement. It is
        // NOT gated on plugin listeners: vanilla break/place must edit the
        // world even when no plugin listens (Fase 1/2 sync Bedrock -> Java).
        // `handle_block_actions` itself fires the cancellable events only
        // when a plugin manager is installed (`fire_event` is a no-op
        // otherwise) and queues the authoritative `SetBlock` either way.
        self.handle_block_actions(&packet.input_data);

        Ok(out)
    }

    /// Fires block events from `PlayerAuthInput` block actions.
    ///
    /// - `START_BREAK` fires a cancellable `PlayerInteractEvent`
    ///   (left click). Nothing is applied, so cancellation needs no undo.
    /// - `PREDICT_DESTROY_BLOCK` fires a cancellable `BlockBreakEvent` with
    ///   the current block id. When not cancelled the break is applied
    ///   authoritatively (air edit queued as a manager-level `SetBlock` so
    ///   every viewer gets the change); when cancelled the chunk is
    ///   resent to this session to undo the client's prediction.
    ///   That `SetBlock` goes through `apply_actions` -> `world.set_block`,
    ///   which is what enqueues the edit for Fase 1 (Java
    ///   `world.take_block_changes`) and Fase 2 (`poll_block_changes`
    ///   broadcasts the `UpdateBlockPacket` to chunk viewers).
    /// - `BlockPlaceEvent` is NOT fired here: placement travels in
    ///   `InventoryTransactionPacket` (`UseItem` CLICK_BLOCK) and is applied
    ///   by [`Self::handle_use_item_place`], not by block actions.
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

    /// Sends health + full attributes to the client.
    pub fn sync_health(&mut self) {
        self.pending_outgoing
            .push(OutgoingPacket::RawWire(set_health_wire(self.health)));
        self.pending_outgoing
            .push(OutgoingPacket::RawWire(self.create_update_attributes_packet()));
    }

    /// Sends every inventory window (main, armor, offhand, cursor).
    pub fn sync_inventory(&mut self) {
        for wire in self.inventory_window_wires() {
            self.pending_outgoing.push(OutgoingPacket::RawWire(wire));
        }
    }

    /// InventoryContentPacket wire bytes for every modelled window.
    pub fn inventory_window_wires(&self) -> Vec<Vec<u8>> {
        let mut wires = Vec::new();
        for (window_id, items) in self.inventory.windows() {
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
            wires.push(writer.into_bytes().to_vec());
        }
        wires
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

        // Bedrock block placement (Fase 1/2 sync): a `UseItem` CLICK_BLOCK
        // transaction places `item_in_hand.block_runtime_id` against the
        // clicked block. Without this the Java bridge never saw Bedrock
        // placements (only breaks via `PREDICT_DESTROY_BLOCK` arrived).
        if let TransactionData::UseItem(use_item) = &packet.data {
            self.handle_use_item_place(use_item);
        }

        out
    }

    /// Authoritative Bedrock block placement from a `UseItem` transaction.
    ///
    /// Mirrors `handle_block_actions` for breaks and the Java bridge's
    /// `placeBlock()`: the target cell is `block_position + face offset`
    /// (0 abajo, 1 arriba, 2 norte, 3 sur, 4 oeste, 5 este), it must be
    /// empty (air, like Java's `isEmpty` gate) and inside world Y bounds,
    /// and the placed id is the held block's runtime id
    /// (`item_in_hand.block_runtime_id`, the same FNV-1a hash `world.set_block`
    /// accepts). `data.block_runtime_id` is the clicked block's own id
    /// (prediction/validation), NOT the block to place.
    ///
    /// When a plugin listens to `BlockPlaceEvent`, the event is cancellable:
    /// cancellation resends the affected `LevelChunk` to this session alone
    /// so the client's predicted placement is undone. The edit itself is
    /// queued as a `PluginAction::SetBlock` so the manager broadcasts the
    /// `UpdateBlockPacket` (0x15) to chunk viewers (Fase 2) and the Java
    /// queue is filled (Fase 1) exactly like a break.
    fn handle_use_item_place(&mut self, use_item: &UseItemData) {
        if use_item.action_type != ACTION_CLICK_BLOCK {
            return;
        }
        let block_id = use_item.item_in_hand.block_runtime_id;
        if block_id == 0 {
            return;
        }
        let (bx, by, bz) = use_item.block_position;
        let offset: (i32, i32, i32) = match use_item.face {
            0 => (0, -1, 0),
            1 => (0, 1, 0),
            2 => (0, 0, -1),
            3 => (0, 0, 1),
            4 => (-1, 0, 0),
            5 => (1, 0, 0),
            _ => return,
        };
        let (x, y, z) = (bx + offset.0, by + offset.1, bz + offset.2);
        if !(WORLD_MIN_Y..WORLD_MAX_Y).contains(&y) {
            return;
        }
        // Only place into an empty cell, like `JavaServer::placeBlock()`.
        if self.world.block_at(x, y, z) != AIR {
            return;
        }

        // Cancellable plugin hook when a listener exists. `fire_event` is a
        // no-op without a plugin manager, so vanilla placement also works
        // with no plugins (unlike the old listener-gated break path).
        let has_place_listeners = self
            .plugins
            .as_ref()
            .map(|plugins| plugins.has_listeners("BlockPlaceEvent"))
            .unwrap_or(false);
        if has_place_listeners {
            let mut event = BlockPlaceEvent::new(
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
                return;
            }
        }

        self.pending_actions.push(PluginAction::SetBlock {
            world: self.world.name.clone(),
            x,
            y,
            z,
            block_id,
        });
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
        // `PlayerDeathEvent` (simplified bridge payload: player + message).
        // Only fired when a plugin listens, so death-message broadcasting
        // stays opt-in (see EVENT_TRIGGERS.md).
        if self
            .plugins
            .as_ref()
            .map(|plugins| plugins.has_listeners("PlayerDeathEvent"))
            .unwrap_or(false)
        {
            let mut event = PlayerDeathEvent::new(
                self.player_context(),
                format!("{} died", self.username),
            );
            self.fire_event(&mut event);
            if !event.death_message.is_empty() {
                self.pending_broadcasts.push(event.death_message);
            }
        }
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

        let (spawn_x, spawn_y, spawn_z) = match self.spawn_point {
            Some(position) => position,
            None => {
                let (x, y, z) = self.world.spawn();
                (x as f64 + 0.5, y as f64, z as f64 + 0.5)
            }
        };

        // `PlayerRespawnEvent` (bridge: cancellable, position mutable). PM
        // upstream is not cancellable; the bridge keeps the flag so plugins
        // can veto the respawn. Fired only when a plugin listens.
        if self
            .plugins
            .as_ref()
            .map(|plugins| plugins.has_listeners("PlayerRespawnEvent"))
            .unwrap_or(false)
        {
            let mut event =
                PlayerRespawnEvent::new(self.player_context(), (spawn_x, spawn_y, spawn_z));
            self.fire_event(&mut event);
            if event.cancelled {
                return Vec::new();
            }
            let (spawn_x, spawn_y, spawn_z) = event.position;
            self.is_dead = false;
            self.health = self.max_health;
            self.teleport(spawn_x, spawn_y, spawn_z, None, None);
        } else {
            self.is_dead = false;
            self.health = self.max_health;
            self.teleport(spawn_x, spawn_y, spawn_z, None, None);
        }
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
                "§aGame mode changed to {}",
                crate::command::gamemode_translatable(self.gamemode)
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

    /// Creates a SetActorDataPacket (0x27) - data flags (incl. sneak/sprint/
    /// swim/glide), nametag, scale, bounding box and air.
    pub fn create_set_actor_data_packet(&self) -> Vec<u8> {
        let packet = SetActorDataPacket::new(
            self.entity_runtime_id,
            vec![
                MetadataEntry::new(0, MetadataValue::Long(self.data_flags())),
                MetadataEntry::new(4, MetadataValue::String(self.username.clone())),
                MetadataEntry::new(7, MetadataValue::Short(self.air)),
                MetadataEntry::new(42, MetadataValue::Short(self.air)),
                MetadataEntry::new(38, MetadataValue::Float(1.0)),
                MetadataEntry::new(53, MetadataValue::Float(0.6)),
                MetadataEntry::new(54, MetadataValue::Float(1.8)),
            ],
            bedrock_protocol::PropertySyncData::empty(),
            0,
        );
        let mut writer = Writer::new();
        writer.write_var_u32(SetActorDataPacket::PACKET_ID);
        packet.encode_payload(&mut writer);
        writer.into_bytes().to_vec()
    }

    /// Creates an UpdateAttributesPacket (0x1D) with the player set: health,
    /// hunger, saturation, movement, xp level and xp progress.
    ///
    /// Ranges mirror AttributeFactory (health/hunger/saturation 0..20,
    /// xp level 0..24791, xp progress 0..1). There is no `minecraft:air`
    /// attribute: BDS syncs breath via `SetActorData` metadata 7/42
    /// (`create_set_actor_data_packet`), and sending an unknown attribute
    /// id makes the client log `Cannot find attribute minecraft:air`.
    pub fn create_update_attributes_packet(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_var_u32(0x1d);
        writer.write_var_u64(self.entity_runtime_id);
        writer.write_var_u32(6);
        let write_attr = |writer: &mut Writer, min: f32, max: f32, value: f32, def_min: f32, def_max: f32, def: f32, id: &str| {
            writer.write_f32_le(min);
            writer.write_f32_le(max);
            writer.write_f32_le(value);
            writer.write_f32_le(def_min);
            writer.write_f32_le(def_max);
            writer.write_f32_le(def);
            writer.write_string(id);
            writer.write_var_u32(0);
        };
        write_attr(&mut writer, 0.0, self.max_health, self.health, 0.0, self.max_health, self.max_health, "minecraft:health");
        write_attr(&mut writer, 0.0, 20.0, self.food, 0.0, 20.0, 20.0, "minecraft:player.hunger");
        write_attr(&mut writer, 0.0, 20.0, self.saturation, 0.0, 20.0, 20.0, "minecraft:player.saturation");
        write_attr(&mut writer, 0.0, f32::MAX, 0.1, 0.0, f32::MAX, 0.1, "minecraft:movement");
        write_attr(&mut writer, 0.0, 24791.0, self.xp_level as f32, 0.0, 24791.0, 0.0, "minecraft:player.level");
        write_attr(&mut writer, 0.0, 1.0, self.xp_progress, 0.0, 1.0, 0.0, "minecraft:player.experience");
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

    /// Builds a MovePlayerPacket (0x13) to move/teleport the local player.
    ///
    /// Mode 2 is `Teleport`, which is accepted even when the client predicts a
    /// different position. If `yaw`/`pitch` are provided they override the
    /// session's current values, matching PocketMine's `/tp x y z yaw pitch`.
    ///
    /// The wire format comes from `MovePlayerPacket` in bedrock-protocol
    /// (axolotl `MovePlayerPacket.php`, protocol 2225): header `0x13` — not
    /// `0x12`, which is MoveActorAbsolute and made Bedrock clients disconnect
    /// with `BinaryStream read() incomplete / packetId: 18` on `/tp` — then
    /// actor VarUInt64, eye Vector3, pitch/yaw/headYaw f32, mode as a single
    /// byte, onGround, riding VarUInt64, the optional telemetry bool plus
    /// (for teleports) cause/source as u32 LE, and the tick VarUInt64.
    /// The position is the eye position, matching PocketMine's
    /// `Human::getOffsetPosition()` (+1.62) in `NetworkSession::syncMovement()`.
    pub fn create_move_player_packet(
        &self,
        mode: u32,
        yaw_override: Option<f32>,
        pitch_override: Option<f32>,
    ) -> Vec<u8> {
        let yaw = yaw_override.unwrap_or(self.yaw);
        let pitch = pitch_override.unwrap_or(self.pitch);
        let packet = MovePlayerPacket::simple(
            self.entity_runtime_id,
            self.eye_position(),
            pitch,
            yaw,
            yaw, // headYaw: PocketMine sends yaw here too (TODO upstream)
            mode as u8,
            true, // onGround: session has no ground tracking; a valid bool,
            // not the cause of the /tp disconnect. Client physics resolves it.
            0,    // ridingActorRuntimeId
            0,    // tick (PocketMine also sends 0 here, TODO upstream)
        );
        let mut writer = Writer::new();
        writer.write_var_u32(packet.packet_id());
        // Encoding a MovePlayerPacket cannot fail: every field is a plain
        // numeric/bool/vector write.
        packet
            .encode(&mut writer)
            .expect("MovePlayerPacket encode is infallible");
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
/// Bedrock-visible view of a virtual (Java bridge) player.
///
/// Virtual players have no `PlayerSession`, so they are not in the sessions
/// map that `spawn_to_locked` walks. This table keeps their Bedrock entity
/// identity (deterministic UUID, allocated runtime/unique ids) plus the set
/// of Bedrock viewers that already received `PlayerList` + `AddPlayer`, so
/// [`SessionManager::sync_virtuals_locked`] can emit spawn / move / despawn
/// packets for them exactly like Bedrock players.
#[derive(Debug, Clone)]
struct VirtualEntity {
    name: String,
    uuid: [u8; 16],
    unique_id: i64,
    runtime_id: u64,
    /// Sessions that received `AddPlayerPacket` for this entity.
    viewers: HashSet<u64>,
    /// Sessions that received the `PlayerListPacket` addition for this entity.
    listed_to: HashSet<u64>,
    /// Feet position (`PlayerContext` stores feet, like Bedrock sessions).
    x: f64,
    y: f64,
    z: f64,
    yaw: f64,
    pitch: f64,
    gamemode: i32,
    /// Last position/rotation broadcast as `MoveActorAbsolutePacket`.
    sent_x: f64,
    sent_y: f64,
    sent_z: f64,
    sent_yaw: f64,
    sent_pitch: f64,
}

impl VirtualEntity {
    fn new(name: String, runtime_id: u64, snapshot: &PlayerContext) -> Self {
        Self {
            uuid: virtual_uuid_for(&name),
            unique_id: runtime_id as i64,
            runtime_id,
            viewers: HashSet::new(),
            listed_to: HashSet::new(),
            name,
            x: snapshot.x,
            y: snapshot.y,
            z: snapshot.z,
            yaw: snapshot.yaw,
            pitch: snapshot.pitch,
            gamemode: snapshot.gamemode as i32,
            sent_x: snapshot.x,
            sent_y: snapshot.y,
            sent_z: snapshot.z,
            sent_yaw: snapshot.yaw,
            sent_pitch: snapshot.pitch,
        }
    }

    fn feet_position(&self) -> Vector3f {
        Vector3f::new(self.x as f32, self.y as f32, self.z as f32)
    }

    fn eye_position(&self) -> Vector3f {
        Vector3f::new(
            self.x as f32,
            self.y as f32 + EYE_HEIGHT,
            self.z as f32,
        )
    }

    /// Mirrors [`PlayerSession::add_player_packet`] for a virtual snapshot.
    fn add_player_packet(&self) -> AddPlayerPacket {
        let flags: i64 = FLAG_HAS_COLLISION | FLAG_AFFECTED_BY_GRAVITY | FLAG_CAN_CLIMB;
        let metadata = vec![
            MetadataEntry::new(0, MetadataValue::Long(flags)),
            MetadataEntry::new(4, MetadataValue::String(self.name.clone())),
            MetadataEntry::new(7, MetadataValue::Short(300)),
            MetadataEntry::new(42, MetadataValue::Short(300)),
            MetadataEntry::new(38, MetadataValue::Float(1.0)),
            MetadataEntry::new(53, MetadataValue::Float(0.6)),
            MetadataEntry::new(54, MetadataValue::Float(1.8)),
        ];
        AddPlayerPacket::new(
            self.uuid,
            self.name.clone(),
            self.runtime_id,
            self.feet_position(),
            self.pitch as f32,
            self.yaw as f32,
            self.yaw as f32,
            ItemStack::empty(),
            self.gamemode,
            metadata,
            bedrock_protocol::AbilitiesData::visitor(self.unique_id),
        )
    }

    fn player_list_addition(&self) -> PlayerListEntry {
        PlayerListEntry::addition(
            self.uuid,
            self.unique_id,
            self.name.clone(),
            SkinData::default_for(&self.uuid),
            String::new(),
        )
    }

    fn moved_since_sent(&self) -> bool {
        (self.x - self.sent_x).abs() > 0.0001
            || (self.y - self.sent_y).abs() > 0.0001
            || (self.z - self.sent_z).abs() > 0.0001
            || (self.yaw - self.sent_yaw).abs() > 0.0001
            || (self.pitch - self.sent_pitch).abs() > 0.0001
    }

    fn mark_sent(&mut self) {
        self.sent_x = self.x;
        self.sent_y = self.y;
        self.sent_z = self.z;
        self.sent_yaw = self.yaw;
        self.sent_pitch = self.pitch;
    }
}

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
    virtuals: RwLock<HashMap<String, VirtualEntity>>,
    next_virtual_runtime: AtomicU64,
    /// Ticks since the last periodic `SetTime` broadcast
    /// (`tick_world_time`); reset on every broadcast.
    time_tick_counter: std::sync::atomic::AtomicI32,
    /// World time of the last `SetTime` broadcast, or -1 before the
    /// first one. Used to spot jumps (`/time set`, plugin
    /// `world.set_time`) that differ from normal +1/tick progression.
    last_time_broadcast: std::sync::atomic::AtomicI32,
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
            virtuals: RwLock::new(HashMap::new()),
            // Bedrock runtime ids are session ids (small ints); keep virtual
            // runtime ids in a disjoint range so packets never confuse them.
            next_virtual_runtime: AtomicU64::new(10_000_000),
            time_tick_counter: std::sync::atomic::AtomicI32::new(0),
            last_time_broadcast: std::sync::atomic::AtomicI32::new(-1),
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
        // `NetworkSession::onSessionStart`: "Session opened".
        info!(
            "{}",
            crate::translations::Translator::english()
                .translate("pocketmine.network.session.open", &[])
        );
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
            // Only drop the entry we own; a virtual player with the same
            // name must survive a Bedrock disconnect.
            registry.remove_if_bedrock(&session.username);
        }

        if !session.joined && !session.spawned {
            session.joined = false;
            session.spawned = false;
            self.forget_viewer(session.session_id);
            // `NetworkSession::close`: "Session closed: {%reason}".
            info!(
                "{}",
                crate::translations::Translator::english()
                    .fill_named("pocketmine.network.session.close", &[("reason", reason)])
                    .unwrap_or_else(|| format!("Session closed: {reason}"))
            );
            return Vec::new();
        }
        session.joined = false;
        session.spawned = false;

        // Despawn the entity + player-list removal for every viewer
        // (despawnFromAll + onPlayerRemoved).
        let (uuid, unique_id, quitter_id, viewers) = (
            session.uuid,
            session.entity_unique_id,
            session.session_id,
            session.viewers.clone(),
        );
        session.viewers.clear();
        self.broadcast_remove_actor_snapshot(uuid, unique_id, quitter_id, &viewers);
        // The quitter no longer views virtual (Java) entities either.
        self.forget_viewer(quitter_id);

        let mut event = PlayerQuitEvent::new(
            session.player_context(),
            format!("{} left the game", session.username),
            reason,
        );
        // `NetworkSession::close`: "Session closed: {%reason}".
        info!(
            "{}",
            crate::translations::Translator::english()
                .fill_named("pocketmine.network.session.close", &[("reason", reason)])
                .unwrap_or_else(|| format!("Session closed: {reason}"))
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
            // Virtual players (Java bridge) have no Bedrock session: their
            // chat lines wait in the registry inbox for `player.take_messages`.
            if let OutgoingPacket::Text(text) = &packet {
                let chat_like = matches!(
                    text.text_type,
                    TextPacket::TYPE_RAW
                        | TextPacket::TYPE_CHAT
                        | TextPacket::TYPE_SYSTEM
                        | TextPacket::TYPE_WHISPER
                );
                if chat_like && self.registry.get(name).is_some() {
                    self.registry.push_message(name, &text.message);
                }
            }
            return;
        };
        if let Some(session) = self.sessions.write().get_mut(&id) {
            session.pending_outgoing.push(packet);
        }
    }

    pub fn broadcast_text(&self, message: &str) {
        let packet = OutgoingPacket::Text(TextPacket::raw(message));
        // Any live session (joined or not) means a Bedrock client will read
        // the packet; only names without one are virtual players.
        let live: HashSet<String> = {
            let mut map = self.sessions.write();
            let live = map
                .values()
                .map(|session| session.username.to_lowercase())
                .collect();
            for session in map.values_mut() {
                if session.joined {
                    session.pending_outgoing.push(packet.clone());
                }
            }
            live
        };
        // Virtual players (Java bridge) have no session for the packet; their
        // copy waits in the registry inbox instead.
        for name in self.registry.names() {
            if !live.contains(&name) {
                self.registry.push_message(&name, message);
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

    /// Entity::spawnTo: spawns target_id for viewer_id (idempotent).
    fn spawn_to_locked(map: &mut HashMap<u64, PlayerSession>, viewer_id: u64, target_id: u64) -> bool {
        if viewer_id == target_id {
            return false;
        }
        let (viewer_spawned, target_spawned, viewer_dead, target_dead, can_see, already) = {
            let Some(viewer) = map.get(&viewer_id) else {
                return false;
            };
            let Some(target) = map.get(&target_id) else {
                return false;
            };
            (
                viewer.spawned && viewer.joined,
                target.spawned && target.joined,
                viewer.is_dead,
                target.is_dead,
                viewer.can_see(target_id),
                target.viewers.contains(&viewer_id),
            )
        };
        if !viewer_spawned || !target_spawned || viewer_dead || target_dead || !can_see || already {
            return false;
        }
        let add_packet = {
            let target = map.get(&target_id).expect("checked above");
            target.add_player_packet()
        };
        if let Some(target) = map.get_mut(&target_id) {
            target.viewers.insert(viewer_id);
        }
        if let Some(viewer) = map.get_mut(&viewer_id) {
            viewer
                .pending_outgoing
                .push(OutgoingPacket::AddPlayer(add_packet));
        }
        true
    }

    /// Entity::despawnFrom: despawns target_id for viewer_id (idempotent).
    fn despawn_from_locked(
        map: &mut HashMap<u64, PlayerSession>,
        viewer_id: u64,
        target_id: u64,
    ) -> bool {
        if viewer_id == target_id {
            return false;
        }
        let unique_id = {
            let Some(target) = map.get(&target_id) else {
                return false;
            };
            if !target.viewers.contains(&viewer_id) {
                return false;
            }
            target.entity_unique_id
        };
        if let Some(target) = map.get_mut(&target_id) {
            target.viewers.remove(&viewer_id);
        }
        if let Some(viewer) = map.get_mut(&viewer_id) {
            viewer
                .pending_outgoing
                .push(OutgoingPacket::RemoveActor(RemoveActorPacket::new(unique_id)));
        }
        true
    }

    fn broadcast_add_player_locked(&self, map: &mut HashMap<u64, PlayerSession>, player_id: u64) {
        let others: Vec<u64> = map
            .values()
            .filter(|s| s.session_id != player_id && s.joined && s.spawned)
            .map(|s| s.session_id)
            .collect();
        let addition = {
            let Some(target) = map.get(&player_id) else {
                return;
            };
            target.player_list_addition()
        };
        let other_entries: Vec<PlayerListEntry> = others
            .iter()
            .filter_map(|id| map.get(id).map(|s| s.player_list_addition()))
            .collect();
        if let Some(newbie) = map.get_mut(&player_id) {
            if !other_entries.is_empty() {
                let packet = PlayerListPacket::with_additions(other_entries);
                newbie.pending_outgoing.push(OutgoingPacket::PlayerList(packet));
            }
        }
        for other_id in &others {
            if let Some(viewer) = map.get_mut(other_id) {
                let packet = PlayerListPacket::addition(addition.clone());
                viewer.pending_outgoing.push(OutgoingPacket::PlayerList(packet));
            }
        }
        for other_id in others {
            Self::spawn_to_locked(map, other_id, player_id);
            Self::spawn_to_locked(map, player_id, other_id);
        }
        // Virtual (Java bridge) players have no session, so the loop above
        // never spots them: sync them here so a Bedrock newbie immediately
        // sees every Java body (spawn + player-list) and the other Bedrock
        // viewers learn about any Java player that registered earlier.
        self.sync_virtuals_locked(map);
    }

    /// Reconciles Bedrock visibility of virtual (Java bridge) players.
    ///
    /// The shared registry is the source of truth: every virtual snapshot
    /// (`is_virtual`) becomes a Bedrock entity for every live Bedrock viewer
    /// (spawned + joined + not dead, same gate as [`Self::spawn_to_locked`]).
    /// New snapshots emit `PlayerListPacket` addition + `AddPlayerPacket`
    /// (feet position, like `PlayerSession::add_player_packet`), position
    /// changes emit `MoveActorAbsolutePacket` (eye position), and vanished
    /// snapshots emit `RemoveActorPacket` + `PlayerListPacket` removal
    /// (`player.unregister`).
    ///
    /// Snapshots whose name is owned by a live Bedrock session are skipped
    /// so one name never renders twice. Returns how many packets were
    /// queued. Caller must hold the sessions write lock.
    fn sync_virtuals_locked(&self, map: &mut HashMap<u64, PlayerSession>) -> usize {
        let snapshots = self.registry.all();
        let bedrock_names: HashSet<String> = map
            .values()
            .map(|s| s.username.to_lowercase())
            .collect();
        let mut current: HashMap<String, PlayerContext> = HashMap::new();
        for snapshot in snapshots {
            if !snapshot.is_virtual || snapshot.name.is_empty() {
                continue;
            }
            let lower = snapshot.name.to_lowercase();
            if bedrock_names.contains(&lower) {
                continue;
            }
            current.insert(lower, snapshot);
        }

        let mut queued = 0usize;
        let mut virtuals = self.virtuals.write();

        // Despawn: virtuals that left the registry (`player.unregister`).
        let removed: Vec<String> = virtuals
            .keys()
            .filter(|key| !current.contains_key(*key))
            .cloned()
            .collect();
        for key in removed {
            if let Some(entity) = virtuals.remove(&key) {
                let recipients: HashSet<u64> = entity
                    .listed_to
                    .union(&entity.viewers)
                    .copied()
                    .collect();
                for viewer_id in recipients {
                    if let Some(viewer) = map.get_mut(&viewer_id) {
                        viewer.pending_outgoing.push(OutgoingPacket::RemoveActor(
                            RemoveActorPacket::new(entity.unique_id),
                        ));
                        viewer.pending_outgoing.push(OutgoingPacket::PlayerList(
                            PlayerListPacket::removal(entity.uuid),
                        ));
                        queued += 2;
                    }
                }
            }
        }

        // Spawn / move: every current virtual for every eligible viewer.
        for (lower, snapshot) in &current {
            let runtime_id = match virtuals.get(lower) {
                Some(existing) => existing.runtime_id,
                None => self.next_virtual_runtime.fetch_add(1, Ordering::SeqCst),
            };
            let entity = virtuals
                .entry(lower.clone())
                .or_insert_with(|| VirtualEntity::new(snapshot.name.clone(), runtime_id, snapshot));
            entity.name = snapshot.name.clone();
            entity.x = snapshot.x;
            entity.y = snapshot.y;
            entity.z = snapshot.z;
            entity.yaw = snapshot.yaw;
            entity.pitch = snapshot.pitch;
            entity.gamemode = snapshot.gamemode as i32;

            let viewers: Vec<u64> = map
                .values()
                .filter(|s| {
                    s.spawned && s.joined && !s.is_dead && s.username.to_lowercase() != *lower
                })
                .map(|s| s.session_id)
                .collect();

            // `moved` is decided once for the whole entity, before the
            // viewer loop: `mark_sent` must not run per-viewer, or only
            // the first Bedrock viewer would ever receive the move and
            // every later viewer would keep seeing a frozen body.
            let moved = entity.moved_since_sent();
            let mut moved_packets = 0;

            for viewer_id in viewers {
                if !entity.viewers.contains(&viewer_id) {
                    if !entity.listed_to.contains(&viewer_id) {
                        if let Some(viewer) = map.get_mut(&viewer_id) {
                            viewer.pending_outgoing.push(OutgoingPacket::PlayerList(
                                PlayerListPacket::addition(entity.player_list_addition()),
                            ));
                            queued += 1;
                        }
                        entity.listed_to.insert(viewer_id);
                    }
                    if let Some(viewer) = map.get_mut(&viewer_id) {
                        viewer
                            .pending_outgoing
                            .push(OutgoingPacket::AddPlayer(entity.add_player_packet()));
                        queued += 1;
                    }
                    entity.viewers.insert(viewer_id);
                } else if moved {
                    // Java `player.update_position` publishes x/y/z/yaw/pitch;
                    // Bedrock viewers see them as `MoveActorAbsolutePacket`
                    // on the eye position, like Bedrock players.
                    let packet = MoveActorAbsolutePacket::new(
                        entity.runtime_id as i64,
                        entity.eye_position(),
                        entity.pitch as f32,
                        entity.yaw as f32,
                        entity.yaw as f32,
                        0,
                    );
                    if let Some(viewer) = map.get_mut(&viewer_id) {
                        viewer
                            .pending_outgoing
                            .push(OutgoingPacket::MoveActorAbsolute(packet.clone()));
                        queued += 1;
                        moved_packets += 1;
                    }
                }
            }

            if moved {
                if moved_packets > 0 {
                    debug!(
                        "Virtual player {} moved to ({:.2}, {:.2}, {:.2}); queued MoveActorAbsolute for {} Bedrock viewer(s)",
                        entity.name,
                        entity.x,
                        entity.y,
                        entity.z,
                        moved_packets,
                    );
                }
                entity.mark_sent();
            }

            // Drop viewers that disconnected while this entity was visible.
            entity.viewers.retain(|id| map.contains_key(id));
            entity.listed_to.retain(|id| map.contains_key(id));
        }

        queued
    }

    /// Public entry for virtual sync, usable from the tick loop and from the
    /// `ServerPluginHost` virtual notify callback (which runs on the PHP
    /// worker thread). Uses a non-blocking lock so a host call running
    /// inside a session dispatch can never deadlock on the sessions lock;
    /// a missed cycle is retried by the next poll (`poll_prespawn_timeouts`).
    pub fn poll_virtual_players(&self) -> usize {
        let Some(mut map) = self.sessions.try_write() else {
            return 0;
        };
        self.sync_virtuals_locked(&mut map)
    }

    /// Forgets a disconnected Bedrock session as a virtual viewer.
    fn forget_viewer(&self, viewer_id: u64) {
        let mut virtuals = self.virtuals.write();
        for entity in virtuals.values_mut() {
            entity.viewers.remove(&viewer_id);
            entity.listed_to.remove(&viewer_id);
        }
    }

    fn broadcast_remove_actor_locked(
        &self,
        map: &mut HashMap<u64, PlayerSession>,
        uuid: [u8; 16],
        unique_id: i64,
        viewers: &HashSet<u64>,
    ) {
        for viewer_id in viewers {
            if let Some(viewer) = map.get_mut(viewer_id) {
                viewer
                    .pending_outgoing
                    .push(OutgoingPacket::RemoveActor(RemoveActorPacket::new(unique_id)));
                viewer
                    .pending_outgoing
                    .push(OutgoingPacket::PlayerList(PlayerListPacket::removal(uuid)));
            }
        }
    }

    fn broadcast_move_actor_locked(&self, map: &mut HashMap<u64, PlayerSession>, player_id: u64) {
        let (runtime_id, eye, pitch, yaw, viewers) = {
            let Some(target) = map.get(&player_id) else {
                return;
            };
            (
                target.entity_runtime_id as i64,
                target.eye_position(),
                target.pitch,
                target.yaw,
                target.viewers.clone(),
            )
        };
        let packet = MoveActorAbsolutePacket::new(runtime_id, eye, pitch, yaw, yaw, 0);
        for viewer_id in viewers {
            if viewer_id == player_id {
                continue;
            }
            if let Some(viewer) = map.get_mut(&viewer_id) {
                if !viewer.can_see(player_id) {
                    continue;
                }
                viewer
                    .pending_outgoing
                    .push(OutgoingPacket::MoveActorAbsolute(packet.clone()));
            }
        }
    }

    fn broadcast_actor_data_locked(&self, map: &mut HashMap<u64, PlayerSession>, player_id: u64) {
        let (packet, viewers) = {
            let Some(target) = map.get(&player_id) else {
                return;
            };
            let data = SetActorDataPacket::new(
                target.entity_runtime_id,
                vec![
                    MetadataEntry::new(0, MetadataValue::Long(target.data_flags())),
                    MetadataEntry::new(4, MetadataValue::String(target.username.clone())),
                    MetadataEntry::new(7, MetadataValue::Short(target.air)),
                    MetadataEntry::new(42, MetadataValue::Short(target.air)),
                    MetadataEntry::new(38, MetadataValue::Float(1.0)),
                    MetadataEntry::new(53, MetadataValue::Float(0.6)),
                    MetadataEntry::new(54, MetadataValue::Float(1.8)),
                ],
                bedrock_protocol::PropertySyncData::empty(),
                0,
            );
            (data, target.viewers.clone())
        };
        for viewer_id in viewers {
            if viewer_id == player_id {
                continue;
            }
            if let Some(viewer) = map.get_mut(&viewer_id) {
                if !viewer.can_see(player_id) {
                    continue;
                }
                viewer
                    .pending_outgoing
                    .push(OutgoingPacket::SetActorData(packet.clone()));
            }
        }
    }

    fn drain_pending_broadcasts_locked(&self, map: &mut HashMap<u64, PlayerSession>, player_id: u64) {
        let (spawn, move_, data) = {
            let Some(session) = map.get(&player_id) else {
                return;
            };
            (
                session.pending_spawn_broadcast,
                session.pending_move_broadcast,
                session.pending_data_broadcast,
            )
        };
        if spawn {
            self.broadcast_add_player_locked(map, player_id);
            if let Some(session) = map.get_mut(&player_id) {
                session.pending_spawn_broadcast = false;
            }
        }
        if move_ {
            self.broadcast_move_actor_locked(map, player_id);
            if let Some(session) = map.get_mut(&player_id) {
                session.pending_move_broadcast = false;
            }
        }
        if data {
            self.broadcast_actor_data_locked(map, player_id);
            if let Some(session) = map.get_mut(&player_id) {
                session.pending_data_broadcast = false;
            }
        }
    }

    /// Spawns player_id for every other joined player and vice versa.
    pub fn broadcast_add_player(&self, player_id: u64) {
        let mut map = self.sessions.write();
        self.broadcast_add_player_locked(&mut map, player_id);
    }

    /// Despawns player_id for every viewer and removes it from the player list.
    pub fn broadcast_remove_actor(&self, player_id: u64) {
        let mut map = self.sessions.write();
        let Some(target) = map.get(&player_id) else {
            return;
        };
        let (uuid, unique_id, viewers) =
            (target.uuid, target.entity_unique_id, target.viewers.clone());
        for (id, session) in map.iter_mut() {
            if *id != player_id {
                session.viewers.remove(&player_id);
            }
        }
        self.broadcast_remove_actor_locked(&mut map, uuid, unique_id, &viewers);
    }

    /// Snapshot variant used when the session has already been removed.
    pub fn broadcast_remove_actor_snapshot(
        &self,
        uuid: [u8; 16],
        unique_id: i64,
        quitter_id: u64,
        viewers: &HashSet<u64>,
    ) {
        let mut map = self.sessions.write();
        for (_, session) in map.iter_mut() {
            session.viewers.remove(&quitter_id);
            session.hidden_players.remove(&quitter_id);
        }
        self.broadcast_remove_actor_locked(&mut map, uuid, unique_id, viewers);
    }

    /// Broadcasts the current position/rotation to viewers.
    pub fn broadcast_move_actor(&self, player_id: u64) {
        let mut map = self.sessions.write();
        self.broadcast_move_actor_locked(&mut map, player_id);
    }

    /// Broadcasts entity flags (sneak/sprint/swim/glide) to viewers.
    pub fn broadcast_actor_data(&self, player_id: u64) {
        let mut map = self.sessions.write();
        self.broadcast_actor_data_locked(&mut map, player_id);
    }

    /// Player::canSee(viewer, target) exposed for the Java bridge.
    pub fn can_see(&self, viewer: &str, target: &str) -> bool {
        let (Some(viewer_id), Some(target_id)) =
            (self.session_id_for(viewer), self.session_id_for(target))
        else {
            return false;
        };
        if viewer_id == target_id {
            return true;
        }
        let map = self.sessions.read();
        map.get(&viewer_id)
            .map(|s| s.can_see(target_id))
            .unwrap_or(true)
    }

    /// Player::hidePlayer(viewer, target) exposed for the Java bridge.
    pub fn hide_player(&self, viewer: &str, target: &str) -> bool {
        let (Some(viewer_id), Some(target_id)) =
            (self.session_id_for(viewer), self.session_id_for(target))
        else {
            return false;
        };
        if viewer_id == target_id {
            return false;
        }
        let mut map = self.sessions.write();
        if let Some(session) = map.get_mut(&viewer_id) {
            session.hide_player(target_id);
        }
        Self::despawn_from_locked(&mut map, viewer_id, target_id)
    }

    /// Player::showPlayer(viewer, target) exposed for the Java bridge.
    pub fn show_player(&self, viewer: &str, target: &str) -> bool {
        let (Some(viewer_id), Some(target_id)) =
            (self.session_id_for(viewer), self.session_id_for(target))
        else {
            return false;
        };
        if viewer_id == target_id {
            return false;
        }
        let mut map = self.sessions.write();
        if let Some(session) = map.get_mut(&viewer_id) {
            session.show_player(target_id);
        }
        Self::spawn_to_locked(&mut map, viewer_id, target_id)
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
    pub fn handle_session_packet(
        &self,
        session_id: u64,
        packet_id: u32,
        payload: &[u8],
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        let mut map = self.sessions.write();
        let out_result = {
            let Some(session) = map.get_mut(&session_id) else {
                return Err(SessionError::Disconnected);
            };
            let result = session.handle_packet(packet_id, payload);
            let mut out = match result {
                Ok(out) => out,
                Err(e) => return Err(e),
            };
            if packet_id == 0x01 && !session.username.is_empty() {
                let is_op = session.is_op();
                let ip = session.client_addr.ip().to_string();
                if let Some(reason) = self.access.denial(&session.username, &ip, is_op) {
                    debug!(
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
            out
        };
        self.drain_pending_broadcasts_locked(&mut map, session_id);
        let mut out = out_result;
        if let Some(session) = map.get_mut(&session_id) {
            out.append(&mut session.pending_outgoing);
        }
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
    ///
    /// Virtual (Java) players in the registry are saved directly to
    /// `players/<name>.json` with the same JSON v2 as Bedrock — no staging
    /// file. `first_played`/`xuid` from disk win over empty live values so a
    /// save never resets a returning player's timers.
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
        for player in self.registry.all() {
            if !player.is_virtual {
                continue;
            }
            let path = dir
                .join("players")
                .join(format!("{}.json", player.name.to_lowercase()));
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let existing = std::fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
            let now = PlayerSession::now_played_secs();
            let mut merged = existing.clone().unwrap_or(serde_json::json!({"name": player.name}));
            if let (Some(object), Some(snap)) = (merged.as_object_mut(), player.to_save_json().as_object()) {
                for (key, value) in snap {
                    object.insert(key.clone(), value.clone());
                }
                // Live empty times must not erase on-disk timers.
                if player.first_played <= 0.0 {
                    if let Some(existing_first) = existing.as_ref().and_then(|entry| entry.get("first_played").and_then(|entry| entry.as_f64())) {
                        if existing_first > 0.0 {
                            object.insert("first_played".to_string(), serde_json::json!(existing_first));
                        }
                    }
                }
                if player.xuid.is_empty() {
                    if let Some(existing_xuid) = existing.as_ref().and_then(|entry| entry.get("xuid").and_then(|entry| entry.as_str())) {
                        if !existing_xuid.is_empty() {
                            object.insert("xuid".to_string(), serde_json::json!(existing_xuid));
                        }
                    }
                }
                object.insert("last_played".to_string(), serde_json::json!(now));
                object.insert("name".to_string(), serde_json::json!(player.name));
            }
            let bytes = serde_json::to_vec(&merged).map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            std::fs::write(path, bytes)?;
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

    /// Forces `PLAYER_SPAWN` for sessions stuck in PreSpawn past
    /// [`PRESPAWN_SPAWN_TIMEOUT`] without `0x71`.
    ///
    /// Spawn packets are queued as `pending_outgoing` and delivered on the
    /// next client message (there is no server-initiated DataChannel push);
    /// registration/join already happens here so plugins (TestKit heartbeat,
    /// `/list`) see the player as online after the timeout even if the
    /// client has gone silent. Returns how many sessions were forced.
    pub fn poll_prespawn_timeouts(&self) -> usize {
        // Fase 2 tick poll (main.rs already calls this every 20 ticks):
        // Java edits made only via the `world.set_block` host call have no
        // `apply_actions` of their own, so they are forwarded to Bedrock
        // viewers here even when no Bedrock packet is in flight.
        self.poll_block_changes();
        // Virtual (Java bridge) players mutate the registry through host
        // calls (`player.register` / `player.update_position` /
        // `player.unregister`) without any Bedrock packet in flight. The
        // host notify callback covers them immediately; this 20-tick poll
        // is the safety net for anything missed (e.g. no notify wired).
        self.poll_virtual_players();
        let candidates: Vec<u64> = {
            let map = self.sessions.read();
            map.values()
                .filter(|session| session.is_prespawn_timed_out(PRESPAWN_SPAWN_TIMEOUT))
                .map(|session| session.session_id)
                .collect()
        };
        let mut forced = 0usize;
        for session_id in candidates {
            let mut session = {
                let mut map = self.sessions.write();
                match map.remove(&session_id) {
                    Some(session) => session,
                    None => continue,
                }
            };
            if !session.is_prespawn_timed_out(PRESPAWN_SPAWN_TIMEOUT) {
                self.sessions.write().insert(session_id, session);
                continue;
            }
            info!(
                "Session {} prespawn timed out (> {:?} without 0x71); forcing PLAYER_SPAWN fallback",
                session_id, PRESPAWN_SPAWN_TIMEOUT
            );
            match session.handle_set_local_player_as_initialized() {
                Ok(packets) => {
                    if !packets.is_empty() {
                        forced += 1;
                        session.pending_outgoing.extend(packets);
                    }
                }
                Err(error) => {
                    warn!("Prespawn timeout spawn failed for session {}: {}", session_id, error);
                }
            }
            // Fire join broadcasts now so other players learn about the
            // spawn even if this session never sends another packet.
            let broadcasts = std::mem::take(&mut session.pending_broadcasts);
            let actions = std::mem::take(&mut session.pending_actions);
            self.sessions.write().insert(session_id, session);
            {
                let mut map = self.sessions.write();
                self.drain_pending_broadcasts_locked(&mut map, session_id);
            }
            for message in broadcasts {
                self.broadcast_text(&message);
            }
            self.apply_actions(&actions, 0);
        }
        forced
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

        // Timeout fallback: if this session already sat in PreSpawn past the
        // threshold, spawn it before processing the new batch so *any* next
        // client packet (not just `0x45`/`0x90`) unblocks the login.
        if session.is_prespawn_timed_out(PRESPAWN_SPAWN_TIMEOUT) {
            info!(
                "Session {} prespawn timed out (> {:?} without 0x71) on dispatch; forcing PLAYER_SPAWN fallback",
                session_id, PRESPAWN_SPAWN_TIMEOUT
            );
            if let Ok(spawn_packets) = session.handle_set_local_player_as_initialized() {
                outgoing.extend(spawn_packets);
            }
        }

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

        // Visibility broadcasts for this peer (spawn/move/flags).
        {
            let mut map = self.sessions.write();
            self.drain_pending_broadcasts_locked(&mut map, session_id);
        }

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
                    if let Some(id) = self.session_id_for(target_player) {
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
                            // Si la acción no trae yaw/pitch, se conservan
                            // los del jugador (la sesión no cambia rotación).
                            session.teleport(
                                *x,
                                *y,
                                *z,
                                yaw.map(|v| v as f32),
                                pitch.map(|v| v as f32),
                            );
                            registry_upsert(&self.registry, session);
                        }
                        self.drain_pending_broadcasts_locked(&mut map, id);
                    } else if let Some(mut player) = self.registry.get(target_player) {
                        // Virtual Java player (bridge): no Bedrock session,
                        // keep the shared snapshot so the bridge polls it
                        // and sends the Java position/teleport packet.
                        // A portado de `syncTeleport`: se conserva el
                        // yaw/pitch Bedrock de la acción cuando viene.
                        player.apply_position(
                            *x,
                            *y,
                            *z,
                            yaw.unwrap_or(player.yaw),
                            pitch.unwrap_or(player.pitch),
                        );
                        self.registry.upsert(player);
                        // Bedrock viewers see the teleport as a move.
                        self.poll_virtual_players();
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
                    // `set_block` enqueues the edit for both sync
                    // directions (Fase 1: Java `take_block_changes`; Fase
                    // 2: Bedrock broadcast queue). The Bedrock side is
                    // delivered by the `poll_block_changes` at the end of
                    // `apply_actions` as a single `UpdateBlockPacket`
                    // (0x15) per viewer holding the chunk — not by
                    // resending a whole `LevelChunk` as before.
                    let _ = self.world.set_block(*x, *y, *z, *block_id);
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
                    if let Some(id) = self.session_id_for(target_player) {
                        let mut map = self.sessions.write();
                        if let Some(session) = map.get_mut(&id) {
                            session.gamemode = gamemode;
                            session.pending_outgoing.push(OutgoingPacket::SetPlayerGameType(
                                SetPlayerGameTypePacket::new(gamemode),
                            ));
                            let abilities = session.create_update_abilities_packet();
                            session.pending_outgoing.push(OutgoingPacket::RawWire(abilities));
                            registry_upsert(&self.registry, session);
                        }
                    } else if let Some(mut player) = self.registry.get(target_player) {
                        // Virtual Java player: no Bedrock session, keep the
                        // shared snapshot so the bridge polls it and sends
                        // the Java gamemode packet (player_info_update /
                        // player_info) to the client.
                        player.gamemode = gamemode.max(0) as u32;
                        self.registry.upsert(player);
                    }
                }
                // ---- Fase A1 (Server+Player API): acceso Server/Player ----
                PluginAction::SetHealth {
                    target_player,
                    health,
                    max_health,
                } => {
                    let Some(id) = self.session_id_for(target_player) else {
                        continue;
                    };
                    let mut map = self.sessions.write();
                    if let Some(session) = map.get_mut(&id) {
                        if let Some(max) = max_health {
                            session.max_health = max.max(1.0);
                        }
                        if session.health > session.max_health && session.health > 0.0 {
                            // `Living::setMaxHealth` clamps current health.
                            session.health = session.max_health;
                        }
                        session.health = health.clamp(0.0, session.max_health);
                        if session.health <= 0.0 {
                            session.is_dead = true;
                        } else if session.is_dead && session.health > 0.0 {
                            session.is_dead = false;
                        }
                        session
                            .pending_outgoing
                            .push(OutgoingPacket::RawWire(set_health_wire(session.health)));
                        let attributes = session.create_update_attributes_packet();
                        session
                            .pending_outgoing
                            .push(OutgoingPacket::RawWire(attributes));
                        registry_upsert(&self.registry, session);
                    }
                }
                PluginAction::SetXp {
                    target_player,
                    xp,
                    level,
                } => {
                    let Some(id) = self.session_id_for(target_player) else {
                        continue;
                    };
                    let mut map = self.sessions.write();
                    if let Some(session) = map.get_mut(&id) {
                        session.xp_progress = xp.clamp(0.0, 1.0);
                        if let Some(lvl) = level {
                            session.xp_level = (*lvl).max(0);
                        }
                        let attributes = session.create_update_attributes_packet();
                        session
                            .pending_outgoing
                            .push(OutgoingPacket::RawWire(attributes));
                    }
                }
                PluginAction::HidePlayer { viewer, target } => {
                    self.hide_player(viewer, target);
                }
                PluginAction::ShowPlayer { viewer, target } => {
                    self.show_player(viewer, target);
                }
                PluginAction::SendSubTitle {
                    target_player,
                    subtitle,
                } => {
                    self.queue_packet(
                        target_player,
                        OutgoingPacket::SetTitle(SetTitlePacket::subtitle(subtitle.clone())),
                    );
                }
                PluginAction::SendActionBar {
                    target_player,
                    message,
                } => {
                    self.queue_packet(
                        target_player,
                        OutgoingPacket::SetTitle(SetTitlePacket::action_bar_message(
                            message.clone(),
                        )),
                    );
                }
            }
        }

        if !follow_up.is_empty() {
            let nested = self.apply_actions(&follow_up, depth + 1);
            follow_up.extend(nested);
        }

        // Fase 2: forward every queued block change (Bedrock breaks via
        // `PluginAction::SetBlock` above, plus Java edits made through the
        // `world.set_block` host call since the last poll) to Bedrock
        // viewers as `UpdateBlockPacket`s. Polling here, once per
        // `apply_actions`, is what makes the `dispatch_webrtc_packet` /
        // plugin-tick paths deliver in the same dispatch; a no-change
        // poll is an empty drain plus an empty session scan.
        self.poll_block_changes();

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

    /// Authoritative list of players in the world: live Bedrock sessions
    /// plus every virtual (Java bridge) entry in the shared registry, so
    /// console `/list` counts Procatin0-like players too.
    fn online_players_snapshot(&self) -> Vec<OnlinePlayer> {
        use std::collections::HashSet;
        let mut seen: HashSet<String> = HashSet::new();
        let mut players: Vec<OnlinePlayer> = self
            .sessions
            .read()
            .values()
            .filter(|session| session.joined)
            .map(|session| {
                seen.insert(session.username.to_lowercase());
                OnlinePlayer {
                    name: session.username.clone(),
                    position: (
                        session.position.x as f64,
                        session.position.y as f64,
                        session.position.z as f64,
                    ),
                    gamemode: session.gamemode,
                }
            })
            .collect();
        for entry in self.registry.all() {
            if seen.insert(entry.name.to_lowercase()) {
                players.push(OnlinePlayer {
                    name: entry.name,
                    position: (entry.x, entry.y, entry.z),
                    gamemode: entry.gamemode as i32,
                });
            }
        }
        players
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
                    if let Some(session_id) = self.session_id_for(target) {
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
                    } else if let Some(mut player) = self.registry.get(target) {
                        // Virtual Java player (bridge): no Bedrock session.
                        // Update the shared registry snapshot so the bridge
                        // polls it and sends the Java gamemode packet.
                        player.gamemode = (*gamemode).max(0) as u32;
                        self.registry.upsert(player);
                    } else {
                        continue;
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
                    debug!("Kicked {}: {}", target, crate::logging::to_ansi(reason));
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
                CommandAction::SetTimeRunning {
                    running,
                    message,
                    notify,
                } => {
                    if *running {
                        self.world.start_time();
                    } else {
                        self.world.stop_time();
                    }
                    // Snap every client to the (now frozen / resumed)
                    // time immediately; the periodic `tick_world_time`
                    // broadcast would otherwise take up to 1 s, and a
                    // frozen client would keep drifting locally.
                    self.broadcast_time(self.world.time());
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::ApplyEffect {
                    target,
                    effect,
                    duration_secs,
                    amplifier,
                    particles,
                    remove,
                    message,
                    notify,
                } => {
                    // No MobEffect packet path yet: log + broadcast so the
                    // command feedback is observable. The canonical effect
                    // registry (`StringToEffectParser`) is still TODO.
                    if *remove {
                        debug!("Effect removed from {}: {}", target, effect);
                    } else {
                        debug!(
                            "Effect {} * {} on {} ({}s, particles={})",
                            effect,
                            amplifier,
                            target,
                            duration_secs.map(|s| s.to_string()).unwrap_or_else(|| "infinite".into()),
                            particles
                        );
                    }
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::Enchant {
                    target,
                    enchantment,
                    level,
                    message,
                    notify,
                } => {
                    // Inventory enchantment NBT path is not implemented;
                    // log the request (session owns the inventory).
                    debug!("Enchant {} lvl {} on {}", enchantment, level, target);
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::AddXp {
                    target,
                    amount,
                    is_levels,
                    message,
                    notify,
                } => {
                    // XP attribute path is not implemented; log the request.
                    debug!(
                        "XP {} {} on {}",
                        if *is_levels { "levels" } else { "points" },
                        amount,
                        target
                    );
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::SpawnParticle {
                    name,
                    position,
                    delta,
                    count,
                    data,
                    message,
                } => {
                    debug!(
                        "Particle {} x{} at ({:.1},{:.1},{:.1}) delta ({:.1},{:.1},{:.1}) data={:?}",
                        name, count, position.0, position.1, position.2, delta.0, delta.1, delta.2, data
                    );
                    // The sender already got the success line as feedback;
                    // particles have no broadcast in PM either.
                    let _ = message;
                }
                CommandAction::Timings {
                    mode,
                    message,
                    notify,
                } => {
                    debug!("Timings mode: {}", mode);
                    self.broadcast_command_message(notify, message);
                }
                CommandAction::GarbageCollect { notify } => {
                    // Chunk/entity unload counting needs the world manager;
                    // report zeroes honestly until then.
                    debug!("Garbage collection requested by {}", notify);
                    self.notify_sender(
                        notify,
                        &crate::command::fill_named(
                            crate::command::lang::GC_CHUNKS,
                            &[("chunksCollected", "0")],
                        ),
                    );
                    self.notify_sender(
                        notify,
                        &crate::command::fill_named(
                            crate::command::lang::GC_ENTITIES,
                            &[("entitiesCollected", "0")],
                        ),
                    );
                    self.notify_sender(
                        notify,
                        &crate::command::fill_named(
                            crate::command::lang::GC_CYCLES,
                            &[("cyclesCollected", "0")],
                        ),
                    );
                    self.notify_sender(
                        notify,
                        &crate::command::fill_named(
                            crate::command::lang::GC_MEMORY_FREED,
                            &[("memoryFreed", "0.00")],
                        ),
                    );
                }
                CommandAction::DumpMemory { path } => {
                    let default_path = format!("memory_dumps/{}.txt", time_now_for_dump());
                    let output = path.clone().unwrap_or(default_path);
                    let _ = std::fs::create_dir_all(
                        std::path::Path::new(&output).parent().unwrap_or(std::path::Path::new(".")),
                    );
                    let body = format!(
                        "Axolotl-PM memory dump\nsessions: {}\nworld: {}\n",
                        self.session_count(),
                        self.world.name
                    );
                    if let Err(error) = std::fs::write(&output, body) {
                        warn!("Could not write memory dump to {}: {}", output, error);
                    } else {
                        debug!("Memory dump written to {}", output);
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
        if let Some(session_id) = self.session_id_for(name) {
            let mut map = self.sessions.write();
            let Some(session) = map.get_mut(&session_id) else {
                return false;
            };
            session.teleport(x, y, z, yaw, pitch);
            registry_upsert(&self.registry, session);
            self.drain_pending_broadcasts_locked(&mut map, session_id);
            return true;
        }
        // Virtual Java player (bridge): no Bedrock session. Keep the shared
        // registry snapshot in sync so the bridge polls it (via `player.get`)
        // and sends the Java position/teleport packet to the client.
        if let Some(mut player) = self.registry.get(name) {
            player.apply_position(
                x,
                y,
                z,
                yaw.map(|v| v as f64).unwrap_or(player.yaw),
                pitch.map(|v| v as f64).unwrap_or(player.pitch),
            );
            self.registry.upsert(player);
            // Bedrock viewers see the teleport as a `MoveActorAbsolute`.
            self.poll_virtual_players();
            return true;
        }
        false
    }

    fn broadcast_time(&self, time: i32) {
        self.last_time_broadcast
            .store(time, std::sync::atomic::Ordering::Relaxed);
        self.time_tick_counter
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.queue_all(OutgoingPacket::RawWire(set_time_wire(time)));
    }

    /// Advances the world clock by one tick and broadcasts `SetTime`
    /// to joined Bedrock sessions every
    /// [`TIME_BROADCAST_INTERVAL_TICKS`] ticks (1 s), or immediately
    /// when the time jumped by more than that interval since the last
    /// broadcast (`/time set`/`add` via a plugin/`world.set_time`
    /// host call, which does not go through `broadcast_time`).
    ///
    /// Called once per server tick from `main.rs`. Returns the
    /// current world time. The Java bridge polls the same
    /// `WorldState::time()` (via `world.get_time`) in its own tick,
    /// so both platforms share this single clock and stay synced:
    /// `/time set 6000` on Bedrock reaches Java within one Java tick
    /// (jump detection there) and vice versa.
    pub fn tick_world_time(&self) -> i32 {
        let time = self.world.tick();
        let elapsed = self
            .time_tick_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        let last = self
            .last_time_broadcast
            .load(std::sync::atomic::Ordering::Relaxed);
        let jumped = last >= 0 && (time - last).abs() > TIME_BROADCAST_INTERVAL_TICKS;
        if elapsed >= TIME_BROADCAST_INTERVAL_TICKS || jumped {
            self.broadcast_time(time);
        }
        time
    }

    /// Resends a chunk to every session that already has it loaded.
    ///
    /// Kept as the `LevelChunk` fallback of
    /// [`SessionManager::broadcast_block_change`]: a whole chunk is only
    /// resent when the lightweight `UpdateBlockPacket` could not be
    /// delivered; the normal single-block path never resends a chunk.
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

    /// Fase 2 (Java -> Bedrock): queues a single-block
    /// `UpdateBlockPacket` (0x15, [`UPDATE_BLOCK_PACKET_ID`]) for every
    /// joined Bedrock session that already has the affected chunk in view
    /// (`sent_chunks` contains `(x >> 4, z >> 4)`), via
    /// `OutgoingPacket::RawWire`. Sessions without that chunk get nothing:
    /// when they stream it, `chunk_payload` already includes the edit.
    ///
    /// Returns how many sessions received the packet. When nobody has the
    /// chunk there is no stale client to fix, so the `LevelChunk` fallback
    /// ([`SessionManager::refresh_chunk_at`], invoked by
    /// [`SessionManager::poll_block_changes`]) is a no-op by construction;
    /// a new viewer always gets a chunk generated from the edited world.
    pub fn broadcast_block_change(&self, x: i32, y: i32, z: i32, block_id: u32) -> usize {
        self.broadcast_block_change_ref(&BlockChange::new(x, y, z, block_id))
    }

    fn broadcast_block_change_ref(&self, change: &BlockChange) -> usize {
        let chunk = (change.x >> 4, change.z >> 4);
        let wire = update_block_wire(change.x, change.y, change.z, change.block_id);
        let mut map = self.sessions.write();
        let mut recipients = 0usize;
        for session in map.values_mut() {
            if !session.joined || !session.sent_chunks.contains(&chunk) {
                continue;
            }
            session
                .pending_outgoing
                .push(OutgoingPacket::RawWire(wire.clone()));
            recipients += 1;
        }
        recipients
    }

    /// Fase 2 poll: drains the Bedrock broadcast queue
    /// ([`WorldState::take_broadcast_changes`]) and broadcasts every queued
    /// change as an `UpdateBlockPacket` (see
    /// [`SessionManager::broadcast_block_change`]). Changes with no viewer
    /// holding the chunk fall back to a `LevelChunk` refresh attempt
    /// ([`SessionManager::refresh_chunk_at`]), which is a no-op when, as
    /// usual, no session has that chunk yet.
    ///
    /// Returns the number of changes that reached at least one viewer.
    /// Safe to call when the queue is empty (the common case) and from the
    /// existing tick/message paths: `apply_actions` polls at its end (so a
    /// Bedrock break, a `PluginAction::SetBlock` or a core command is
    /// forwarded in the same dispatch), and `poll_prespawn_timeouts`
    /// polls them at the 20-tick cadence `main.rs` already drives — which
    /// is also how a Java edit made purely through the
    /// `world.set_block` host call (no plugin action) reaches Bedrock on
    /// the next tick/packet. The Java queue
    /// (`world.take_block_changes`) is independent and untouched here.
    pub fn poll_block_changes(&self) -> usize {
        let changes = self.world.take_broadcast_changes();
        let mut delivered = 0usize;
        for change in &changes {
            if self.broadcast_block_change_ref(change) > 0 {
                delivered += 1;
            } else {
                self.refresh_chunk_at(change.x, change.z);
            }
        }
        delivered
    }
}

/// `date("D_M_j-H.i.s-T_Y")` equivalent for `/dumpmemory` default path.
fn time_now_for_dump() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("dump-{secs}")
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

#[cfg(test)]
mod block_sync_tests {
    use super::*;
    use std::net::SocketAddr;

    fn joined_session(id: u64, chunks: &[(i32, i32)]) -> PlayerSession {
        let addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
        let mut session = PlayerSession::new(id, addr);
        session.state = SessionState::InGame;
        session.joined = true;
        session.spawned = true;
        session.sent_chunks = chunks.iter().copied().collect();
        session
    }

    #[test]
    fn update_block_wire_is_0x15_blockpos_id_flags() {
        let wire = update_block_wire(1, 64, -3, 0xDEAD_BEEF);
        let mut reader = Reader::new(&wire);
        assert_eq!(reader.read_var_u32().unwrap(), UPDATE_BLOCK_PACKET_ID);
        // 0x15, not 0x0B (StartGame).
        assert_eq!(UPDATE_BLOCK_PACKET_ID, 21);
        assert_eq!(BlockPos::read(&mut reader).unwrap(), BlockPos::new(1, 64, -3));
        assert_eq!(reader.read_var_i32().unwrap(), 0xDEAD_BEEF_u32 as i32);
        assert_eq!(reader.read_byte().unwrap(), UPDATE_BLOCK_FLAGS_ALL);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn broadcast_only_reaches_sessions_holding_the_chunk() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, &[(0, 0)]));
        manager.add_session(joined_session(2, &[(9, 9)]));
        manager.add_session(joined_session(3, &[]));

        let recipients = manager.broadcast_block_change(1, 64, 2, 42);
        assert_eq!(recipients, 1);

        let map = manager.sessions.read();
        let viewer = &map[&1];
        assert_eq!(viewer.pending_outgoing.len(), 1);
        assert_eq!(viewer.pending_outgoing[0].packet_id(), UPDATE_BLOCK_PACKET_ID);
        let OutgoingPacket::RawWire(bytes) = &viewer.pending_outgoing[0] else {
            panic!("expected RawWire UpdateBlockPacket");
        };
        assert_eq!(bytes, &update_block_wire(1, 64, 2, 42));
        assert!(map[&2].pending_outgoing.is_empty());
        assert!(map[&3].pending_outgoing.is_empty());
    }

    #[test]
    fn poll_forwards_host_set_block_once_and_keeps_java_queue() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, &[(0, 0)]));

        // Java -> world.set_block (host call path, no plugin action).
        assert!(manager.world().set_block(2, 64, 3, 77));

        assert_eq!(manager.poll_block_changes(), 1);
        {
            let map = manager.sessions.read();
            assert_eq!(map[&1].pending_outgoing.len(), 1);
            assert_eq!(
                map[&1].pending_outgoing[0].packet_id(),
                UPDATE_BLOCK_PACKET_ID
            );
        }
        // Bedrock queue drained; a second poll sends nothing.
        assert_eq!(manager.poll_block_changes(), 0);
        // Fase 1 queue is independent: Java still gets the change.
        assert_eq!(
            manager.world().take_changes(),
            vec![BlockChange::new(2, 64, 3, 77)]
        );
    }

    #[test]
    fn apply_actions_set_block_sends_update_block_not_level_chunk() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, &[(0, 0)]));

        let actions = vec![PluginAction::SetBlock {
            world: "world".to_string(),
            x: 5,
            y: 64,
            z: 6,
            block_id: 123,
        }];
        manager.apply_actions(&actions, 0);

        {
            let map = manager.sessions.read();
            let viewer = &map[&1];
            assert_eq!(viewer.pending_outgoing.len(), 1);
            assert!(matches!(
                viewer.pending_outgoing[0],
                OutgoingPacket::RawWire(_)
            ));
            assert_eq!(
                viewer.pending_outgoing[0].packet_id(),
                UPDATE_BLOCK_PACKET_ID
            );
        }
        // Fase 1: the same edit is queued for Java.
        assert_eq!(
            manager.world().take_changes(),
            vec![BlockChange::new(5, 64, 6, 123)]
        );
    }

    #[test]
    fn poll_with_no_viewers_falls_back_without_sending() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, &[(9, 9)]));

        // Chunk (0,0) is held by nobody: UpdateBlock has no recipient and
        // the LevelChunk fallback must not invent a chunk for session 1.
        assert!(manager.world().set_block(1, 64, 1, 55));
        assert_eq!(manager.poll_block_changes(), 0);
        let map = manager.sessions.read();
        assert!(map[&1].pending_outgoing.is_empty());
        assert!(map[&1].sent_chunks.contains(&(9, 9)));
    }

    #[test]
    fn bedrock_use_item_place_reaches_both_queues_with_real_id() {
        use bedrock_protocol::packets::inventory_transaction::{
            TransactionData, ACTION_CLICK_BLOCK, UseItemData,
        };
        use bedrock_protocol::ItemStack;

        let manager = SessionManager::new();
        let mut placer = joined_session(1, &[(0, 0)]);
        // Production shares one WorldState between the manager and every
        // session (dispatch_webrtc_packet re-points session.world): the
        // occupancy check needs the same world the manager edits.
        placer.world = Arc::clone(manager.world());
        manager.add_session(placer);

        // Clicked block (2,79,3) face 1 (up) -> target (2,80,3) in the sky,
        // air in both worlds. Placed id 999 is deliberately NOT stone/air:
        // both sync directions must forward the real id, never a fallback.
        let mut held = ItemStack::new(1, 1);
        held.block_runtime_id = 999;
        let packet = InventoryTransactionPacket {
            request_id: 0,
            request_changed_slots: None,
            actions: vec![],
            data: TransactionData::UseItem(UseItemData {
                action_type: ACTION_CLICK_BLOCK,
                trigger_type: 0,
                block_position: (2, 79, 3),
                face: 1,
                hotbar_slot: 0,
                hand: 0,
                item_in_hand: held,
                player_position: (2.5, 80.06, 3.5),
                click_position: (2.5, 80.0, 3.5),
                block_runtime_id: 123, // clicked block's id, not the placed one
                client_interact_prediction: 0,
                client_cooldown_state: 0,
            }),
        };
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let payload = writer.into_bytes().to_vec();
        manager
            .handle_session_packet(1, 0x1E, &payload)
            .expect("use item");

        // handle_session_packet holds the session lock and cannot apply
        // actions inline (what dispatch_webrtc_packet does after releasing
        // it): drain the queued SetBlock and apply it like dispatch does.
        let actions: Vec<PluginAction> = {
            let mut map = manager.sessions.write();
            let session = map.get_mut(&1).expect("placer session");
            std::mem::take(&mut session.pending_actions)
        };
        assert_eq!(
            actions,
            vec![PluginAction::SetBlock {
                world: "world".to_string(),
                x: 2,
                y: 80,
                z: 3,
                block_id: 999,
            }]
        );
        manager.apply_actions(&actions, 0);

        // Manager world holds the real id.
        assert_eq!(manager.world().block_at(2, 80, 3), 999);
        // Fase 2 (Java -> Bedrock style, here Bedrock placer -> viewers):
        // exactly one UpdateBlock 0x15 with the real id, payload byte-exact.
        {
            let map = manager.sessions.read();
            let viewer = &map[&1];
            assert_eq!(viewer.pending_outgoing.len(), 1);
            assert_eq!(
                viewer.pending_outgoing[0],
                OutgoingPacket::RawWire(update_block_wire(2, 80, 3, 999))
            );
        }
        // Fase 1: the same real id is queued for the Java bridge.
        assert_eq!(
            manager.world().take_changes(),
            vec![BlockChange::new(2, 80, 3, 999)]
        );
        // A second poll re-sends nothing (no echo duplication).
        assert_eq!(manager.poll_block_changes(), 0);
    }

    #[test]
    fn bedrock_use_item_place_into_occupied_cell_is_ignored() {
        use bedrock_protocol::packets::inventory_transaction::{
            TransactionData, ACTION_CLICK_BLOCK, UseItemData,
        };
        use bedrock_protocol::ItemStack;

        let manager = SessionManager::new();
        let mut placer = joined_session(1, &[(0, 0)]);
        placer.world = Arc::clone(manager.world());
        manager.add_session(placer);

        // Occupy the target first (direct edit, both queues drained).
        assert!(manager.world().set_block(2, 80, 3, 111));
        assert_eq!(manager.poll_block_changes(), 1);
        assert!(manager.world().take_changes().len() == 1);
        {
            let mut map = manager.sessions.write();
            map.get_mut(&1).unwrap().pending_outgoing.clear();
        }

        let mut held = ItemStack::new(1, 1);
        held.block_runtime_id = 999;
        let packet = InventoryTransactionPacket {
            request_id: 0,
            request_changed_slots: None,
            actions: vec![],
            data: TransactionData::UseItem(UseItemData {
                action_type: ACTION_CLICK_BLOCK,
                trigger_type: 0,
                block_position: (2, 79, 3),
                face: 1,
                hotbar_slot: 0,
                hand: 0,
                item_in_hand: held,
                player_position: (2.5, 80.06, 3.5),
                click_position: (2.5, 80.0, 3.5),
                block_runtime_id: 123,
                client_interact_prediction: 0,
                client_cooldown_state: 0,
            }),
        };
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let payload = writer.into_bytes().to_vec();
        manager
            .handle_session_packet(1, 0x1E, &payload)
            .expect("use item");

        // Occupied -> no SetBlock queued, world keeps 111, no UpdateBlock.
        let actions: Vec<PluginAction> = {
            let mut map = manager.sessions.write();
            let session = map.get_mut(&1).expect("placer session");
            std::mem::take(&mut session.pending_actions)
        };
        assert!(actions.is_empty());
        assert_eq!(manager.world().block_at(2, 80, 3), 111);
        assert_eq!(manager.poll_block_changes(), 0);
        assert!(manager.world().take_changes().is_empty());
    }

    #[test]
    fn tick_world_time_advances_and_broadcasts_every_interval_and_on_jump() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, &[(0, 0)]));
        let start = manager.world().time();

        // No broadcast before the interval elapses.
        for _ in 0..(TIME_BROADCAST_INTERVAL_TICKS - 1) {
            manager.tick_world_time();
        }
        assert!(manager.sessions.read()[&1].pending_outgoing.is_empty());
        assert_eq!(manager.world().time(), start + TIME_BROADCAST_INTERVAL_TICKS - 1);

        // Interval reached: one SetTime (0x0A) with the current time.
        let time = manager.tick_world_time();
        {
            let map = manager.sessions.read();
            let viewer = &map[&1];
            assert_eq!(viewer.pending_outgoing.len(), 1);
            assert_eq!(viewer.pending_outgoing[0].packet_id(), 0x0A);
            let OutgoingPacket::RawWire(bytes) = &viewer.pending_outgoing[0] else {
                panic!("expected RawWire SetTime");
            };
            assert_eq!(bytes, &set_time_wire(time));
        }

        // A jump (`/time set` via host/plugin) broadcasts on the next
        // tick, without waiting for the interval.
        manager.world().set_time(6_000);
        {
            let mut map = manager.sessions.write();
            map.get_mut(&1).unwrap().pending_outgoing.clear();
        }
        let time = manager.tick_world_time();
        assert_eq!(time, 6_001);
        {
            let map = manager.sessions.read();
            let viewer = &map[&1];
            assert_eq!(viewer.pending_outgoing.len(), 1);
            let OutgoingPacket::RawWire(bytes) = &viewer.pending_outgoing[0] else {
                panic!("expected RawWire SetTime after jump");
            };
            assert_eq!(bytes, &set_time_wire(6_001));
        }

        // Stopped time does not advance, but the periodic broadcast
        // still re-sends it (clients would otherwise drift locally).
        manager.world().stop_time();
        for _ in 0..TIME_BROADCAST_INTERVAL_TICKS {
            assert_eq!(manager.tick_world_time(), 6_001);
        }
        let map = manager.sessions.read();
        assert!(!map[&1].pending_outgoing.is_empty());
    }
}

#[cfg(test)]
mod virtual_visibility_tests {
    use super::*;
    use std::net::SocketAddr;

    fn joined_session(id: u64, username: &str) -> PlayerSession {
        let addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
        let mut session = PlayerSession::new(id, addr);
        session.username = username.to_string();
        session.state = SessionState::InGame;
        session.joined = true;
        session.spawned = true;
        session.position = Vector3f::new(0.5, 63.0, 0.5);
        session
    }

    fn register_virtual(manager: &SessionManager, name: &str, x: f64, y: f64, z: f64) {
        let mut player =
            PlayerContext::new(name, "127.0.0.1:25565".parse().unwrap());
        player.is_virtual = true;
        player.x = x;
        player.y = y;
        player.z = z;
        manager.registry().upsert(player);
    }

    fn move_virtual(manager: &SessionManager, name: &str, x: f64, y: f64, z: f64) {
        let mut player = manager.registry().get(name).expect("virtual registered");
        player.apply_position(x, y, z, player.yaw, player.pitch);
        manager.registry().upsert(player);
    }

    #[test]
    fn virtual_spawn_emits_player_list_and_add_player() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Steve"));
        register_virtual(&manager, "Procatin0", 2.5, 63.0, -1.5);

        // PlayerList addition + AddPlayer for the single Bedrock viewer.
        assert_eq!(manager.poll_virtual_players(), 2);
        {
            let map = manager.sessions.read();
            let viewer = &map[&1];
            assert_eq!(viewer.pending_outgoing.len(), 2);
            assert!(matches!(
                &viewer.pending_outgoing[0],
                OutgoingPacket::PlayerList(packet)
                    if packet.entries.len() == 1
                        && packet.entries[0].username == "Procatin0"
            ));
            let OutgoingPacket::AddPlayer(packet) = &viewer.pending_outgoing[1] else {
                panic!("expected AddPlayer for virtual");
            };
            assert_eq!(packet.username, "Procatin0");
            assert_eq!(packet.actor_runtime_id, 10_000_000);
            // Feet position straight from the virtual snapshot.
            assert_eq!(packet.position, Vector3f::new(2.5, 63.0, -1.5));
            assert_eq!(packet.game_mode, 0);
        }
        // Idempotent: a second poll without changes queues nothing.
        assert_eq!(manager.poll_virtual_players(), 0);
    }

    #[test]
    fn virtual_move_emits_move_actor_absolute() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Steve"));
        register_virtual(&manager, "Procatin0", 2.5, 63.0, -1.5);
        assert_eq!(manager.poll_virtual_players(), 2);

        move_virtual(&manager, "Procatin0", 4.5, 64.0, -1.5);
        assert_eq!(manager.poll_virtual_players(), 1);
        let map = manager.sessions.read();
        let viewer = &map[&1];
        let OutgoingPacket::MoveActorAbsolute(packet) = &viewer.pending_outgoing[2] else {
            panic!("expected MoveActorAbsolute for virtual move");
        };
        assert_eq!(packet.actor_runtime_id, 10_000_000);
        // Eye position (feet + 1.62), like Bedrock players.
        assert_eq!(
            packet.position,
            Vector3f::new(4.5, 64.0 + EYE_HEIGHT, -1.5)
        );
        // No change, no packet.
        assert_eq!(manager.poll_virtual_players(), 0);
    }

    #[test]
    fn virtual_unregister_emits_remove_actor() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Steve"));
        register_virtual(&manager, "Procatin0", 2.5, 63.0, -1.5);
        assert_eq!(manager.poll_virtual_players(), 2);

        // `player.unregister` drops the registry entry.
        assert!(manager.registry().remove_if_virtual("Procatin0"));
        assert_eq!(manager.poll_virtual_players(), 2);
        let map = manager.sessions.read();
        let viewer = &map[&1];
        assert_eq!(viewer.pending_outgoing.len(), 4);
        assert!(matches!(
            &viewer.pending_outgoing[2],
            OutgoingPacket::RemoveActor(packet)
                if packet.actor_unique_id == 10_000_000
        ));
        assert!(matches!(
            &viewer.pending_outgoing[3],
            OutgoingPacket::PlayerList(packet)
                if packet.entries.len() == 1
                    && packet.entries[0].entry_type == PlayerListPacket::TYPE_REMOVE
        ));
        assert_eq!(manager.poll_virtual_players(), 0);
    }

    #[test]
    fn bedrock_join_sees_existing_virtual_and_vice_versa() {
        let manager = SessionManager::new();
        register_virtual(&manager, "Procatin0", 2.5, 63.0, -1.5);
        manager.add_session(joined_session(1, "Steve"));
        // No other Bedrock player yet: Bedrock->Bedrock broadcast is empty,
        // but the virtual sync inside it must spawn the Java body.
        manager.broadcast_add_player(1);
        let map = manager.sessions.read();
        let viewer = &map[&1];
        assert!(viewer
            .pending_outgoing
            .iter()
            .any(|packet| matches!(packet, OutgoingPacket::PlayerList(_))));
        assert!(viewer
            .pending_outgoing
            .iter()
            .any(|packet| matches!(
                packet,
                OutgoingPacket::AddPlayer(packet) if packet.username == "Procatin0"
            )));
    }

    #[test]
    fn unspawned_viewer_gets_no_virtual_spawn() {
        let manager = SessionManager::new();
        let addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
        let mut session = PlayerSession::new(1, addr);
        session.username = "Steve".to_string();
        session.state = SessionState::PreSpawn;
        // joined/spawned stay false.
        manager.add_session(session);
        register_virtual(&manager, "Procatin0", 2.5, 63.0, -1.5);
        assert_eq!(manager.poll_virtual_players(), 0);
        assert!(manager.sessions.read()[&1].pending_outgoing.is_empty());
    }

    #[test]
    fn same_name_bedrock_session_wins_over_virtual_snapshot() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Procatin0"));
        register_virtual(&manager, "Procatin0", 2.5, 63.0, -1.5);
        // One name, one entity: the live Bedrock session owns it.
        assert_eq!(manager.poll_virtual_players(), 0);
    }

    #[test]
    fn virtual_move_reaches_every_bedrock_viewer() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Steve"));
        manager.add_session(joined_session(2, "Alex"));
        register_virtual(&manager, "Procatin0", 2.5, 63.0, -1.5);

        // Two viewers: PlayerList + AddPlayer for each.
        assert_eq!(manager.poll_virtual_players(), 4);
        {
            let map = manager.sessions.read();
            for id in [1, 2] {
                assert_eq!(map[&id].pending_outgoing.len(), 2, "spawn for viewer {id}");
            }
        }

        // One Java move must emit one MoveActorAbsolute for EVERY viewer,
        // not just the first one (`mark_sent` used to run per-viewer, so
        // the second Bedrock client kept seeing a frozen Java body).
        move_virtual(&manager, "Procatin0", 4.5, 64.0, -1.5);
        assert_eq!(manager.poll_virtual_players(), 2);
        let map = manager.sessions.read();
        for id in [1, 2] {
            let viewer = &map[&id];
            assert_eq!(
                viewer.pending_outgoing.len(),
                3,
                "viewer {id} must get exactly one move packet"
            );
            let OutgoingPacket::MoveActorAbsolute(packet) = &viewer.pending_outgoing[2] else {
                panic!("expected MoveActorAbsolute for viewer {id}");
            };
            assert_eq!(packet.actor_runtime_id, 10_000_000);
            assert_eq!(
                packet.position,
                Vector3f::new(4.5, 64.0 + EYE_HEIGHT, -1.5)
            );
        }
        // Idempotent: the move was delivered to all viewers.
        assert_eq!(manager.poll_virtual_players(), 0);
    }
}

#[cfg(test)]
mod attributes_tests {
    use super::*;
    use std::net::SocketAddr;

    /// Decodes the ids written by `create_update_attributes_packet`:
    /// packet id, runtime id, count, then per attribute six f32s, id and
    /// a trailing var_u32 flag.
    fn attribute_ids(wire: &[u8]) -> Vec<String> {
        let mut reader = Reader::new(wire);
        assert_eq!(reader.read_var_u32().unwrap(), 0x1d);
        reader.read_var_u64().unwrap();
        let count = reader.read_var_u32().unwrap();
        let mut ids = Vec::new();
        for _ in 0..count {
            for _ in 0..6 {
                reader.read_f32_le().unwrap();
            }
            ids.push(reader.read_string().unwrap());
            reader.read_var_u32().unwrap();
        }
        assert_eq!(reader.read_var_u64().unwrap(), 0);
        assert_eq!(reader.remaining(), 0);
        ids
    }

    #[test]
    fn update_attributes_never_sends_air() {
        // Regression: `minecraft:air` is not a Bedrock attribute (BDS syncs
        // breath via SetActorData 7/42). Sending it made the client log
        // `Cannot find attribute minecraft:air`.
        let session = PlayerSession::new(7, "127.0.0.1:19132".parse::<SocketAddr>().unwrap());
        let ids = attribute_ids(&session.create_update_attributes_packet());
        assert_eq!(
            ids,
            vec![
                "minecraft:health",
                "minecraft:player.hunger",
                "minecraft:player.saturation",
                "minecraft:movement",
                "minecraft:player.level",
                "minecraft:player.experience",
            ]
        );
        assert!(!ids.contains(&"minecraft:air".to_string()));
        assert!(!ids.iter().any(|id| id.ends_with("air")));
    }

    #[test]
    fn breaching_air_still_syncs_via_set_actor_data() {
        // The breath value must keep reaching the client through metadata
        // 7 (DATA_AIR) and 42 (max air) after dropping the attribute.
        let session = PlayerSession::new(7, "127.0.0.1:19132".parse::<SocketAddr>().unwrap());
        let wire = session.create_set_actor_data_packet();
        let mut reader = Reader::new(&wire);
        assert_eq!(reader.read_var_u32().unwrap(), SetActorDataPacket::PACKET_ID);
        let packet = SetActorDataPacket::decode_payload(&mut reader).expect("decode");
        let air = packet.metadata.iter().find(|entry| entry.key == 7).expect("air");
        assert_eq!(air.value, MetadataValue::Short(session.air));
        let max_air = packet
            .metadata
            .iter()
            .find(|entry| entry.key == 42)
            .expect("max air");
        assert_eq!(max_air.value, MetadataValue::Short(session.air));
        assert_eq!(reader.remaining(), 0);
    }
}

/// Deterministic offline UUID for a session when the client did not send a
/// parseable `identity` claim. Uses FNV-1a over `<session_id>:<name>` so the
/// same name always yields the same UUID across restarts.
fn derive_uuid_for_session(session_id: u64, name: &str) -> [u8; 16] {
    let seed = format!("{}:{}", session_id, name);
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in seed.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let mut out = [0u8; 16];
    out[0..8].copy_from_slice(&hash.to_le_bytes());
    let mut hash2 = hash ^ 0x9e3779b97f4a7c15;
    // Second mix for the low half so both halves differ.
    for byte in seed.as_bytes().iter().rev() {
        hash2 ^= *byte as u64;
        hash2 = hash2.wrapping_mul(0x100000001b3);
    }
    out[8..16].copy_from_slice(&hash2.to_le_bytes());
    out
}

/// Deterministic UUID for a virtual (Java bridge) entity. The `virtual:`
/// prefix keeps it disjoint from Bedrock session UUIDs derived from
/// `<session_id>:<name>`.
fn virtual_uuid_for(name: &str) -> [u8; 16] {
    derive_uuid_for_session(0, &format!("virtual:{}", name.to_lowercase()))
}
