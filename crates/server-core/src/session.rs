use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bedrock_protocol::binary::{CodecError, Reader, Writer};
use bedrock_protocol::{
    AddPlayerPacket, AvailableCommandsPacket, BlockPos, BossEventPacket, ChunkRadiusUpdatedPacket,
    CommandRequestPacket, CommandSpec, DisconnectPacket, LevelChunkPacket, LoginPacket,
    MetadataEntry, MetadataValue, MoveActorAbsolutePacket, MoveActorDeltaPacket, MovePlayerPacket,
    NetworkChunkPublisherUpdatePacket,
    NetworkSettingsPacket, Packet, PacketBatch, PacketViolationWarningPacket, PlayStatusPacket,
    PlayerAuthInputPacket, PlayerListEntry, PlayerListPacket, RemoveActorPacket,
    RequestAbilityPacket, RequestChunkRadiusPacket, RequestNetworkSettingsPacket,
    ResourcePackClientResponsePacket, ResourcePackStackPacket, ResourcePacksInfoPacket,
    SetActorDataPacket, SetPlayerGameTypePacket, SetSpawnPositionPacket, SkinData,
    StartGamePacket, ActorEventPacket, InventoryContentPacket, InventorySlotPacket,
    InventoryTransactionPacket, ItemStack, RemoveObjectivePacket, RespawnPacket,
    SetDifficultyPacket, SetDisplayObjectivePacket, SetHealthPacket,
    SetScorePacket, SetScoreboardIdentityPacket, SetTitlePacket, TextPacket, TransactionData, TransferPacket, UpdateAbilitiesPacket,
    UpdatePlayerGameTypePacket, Vector3f,
};
use bedrock_protocol::packets::actor_event::EVENT_DEATH_ANIMATION;
use bedrock_protocol::packets::inventory_transaction::{
    ACTION_CLICK_BLOCK, UseItemData,
};
use bedrock_protocol::packets::player_auth_input::{
    has_input_flag, parse_block_actions_result, parse_input_flags, PlayerBlockAction,
    BLOCK_ACTION_ABORT_BREAK,
    BLOCK_ACTION_CONTINUE_DESTROY, BLOCK_ACTION_CRACK_BREAK, BLOCK_ACTION_PREDICT_DESTROY,
    BLOCK_ACTION_START_BREAK, BLOCK_ACTION_STOP_BREAK, INPUT_FLAG_MISSED_SWING,
    INPUT_FLAG_PERFORM_BLOCK_ACTIONS,
    INPUT_FLAG_START_FLYING, INPUT_FLAG_START_GLIDING, INPUT_FLAG_START_JUMPING,
    INPUT_FLAG_START_SNEAKING, INPUT_FLAG_START_SPRINTING, INPUT_FLAG_START_SWIMMING,
    INPUT_FLAG_STOP_FLYING, INPUT_FLAG_STOP_GLIDING, INPUT_FLAG_STOP_SNEAKING,
    INPUT_FLAG_STOP_SPRINTING, INPUT_FLAG_STOP_SWIMMING,
};
use bedrock_protocol::packets::level_event::{
    level_event_wire as level_packet_wire, LEVEL_EVENT_BLOCK_START_BREAK,
    LEVEL_EVENT_BLOCK_STOP_BREAK, LEVEL_EVENT_PACKET_ID, LEVEL_EVENT_PARTICLE_DESTROY_BLOCK,
    LEVEL_EVENT_SOUND_DOOR,
};
use bedrock_protocol::packets::level_sound_event::{
    level_sound_event_wire, LEVEL_SOUND_EVENT_PACKET_ID, LEVEL_SOUND_PLACE, LEVEL_SOUND_POWER_OFF,
    LEVEL_SOUND_POWER_ON,
};
use bedrock_protocol::packets::player_list::uuid_from_str;
use bedrock_world::{block_id::AIR, FlatGenerator, WorldGenerator};
use parking_lot::RwLock;
use php_plugin_bridge::{
    BlockBreakEvent, BlockPlaceEvent, BossBarData, CommandSender, Event, MoveLocation,
    PlayerChatEvent, PlayerCommandPreprocessEvent, PlayerContext, PlayerDeathEvent,
    PlayerInteractEvent, PlayerJoinEvent, PlayerMoveEvent, PlayerQuitEvent, PlayerRespawnEvent,
    PluginAction, PluginManager,
};
use rayon::prelude::*;
use thiserror::Error;
use tracing::{debug, error, info, warn};

use crate::bossbar::{BossBar, BossBarManager, DiverseBossBar};
use crate::command::{
    CommandAction, CommandSource, OnlinePlayer, TitleAction, WeatherState, DEFAULT_WEATHER_TICKS,
};
use crate::identity::parse_login_identity;
use crate::inventory::{
    PlayerInventory, WINDOW_ID_ARMOR, WINDOW_ID_CURSOR, WINDOW_ID_INVENTORY, WINDOW_ID_OFFHAND,
};
use crate::plugin_host::PlayerRegistry;
use crate::scoreboard::{PacketBytes as ScoreboardPacketBytes, Scoreboard};
use crate::static_packets::StaticPackets;
use crate::world_state::{BlockChange, WorldState, WORLD_MAX_Y, WORLD_MIN_Y};
use java_bridge::display::bedrock_view_display_name;

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
///
/// Kept for diagnostics only (`is_prespawn_timed_out`): PocketMine has no
/// prespawn-timeout spawn — a Bedrock session simply awaits
/// `RequestChunkRadius`, the chunk-streaming terrain threshold
/// (`notifyTerrainReady`), then `0x71`. No forcing path uses this any more.
pub const PRESPAWN_SPAWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// `YmlServerProperties::CHUNK_SENDING_PER_TICK` (default 4): how many
/// `LevelChunk` packets one `Player::doChunkRequests()` pass sends.
pub const CHUNK_SENDING_PER_TICK: usize = 4;

/// `YmlServerProperties::CHUNK_SENDING_SPAWN_RADIUS` (default 4): the
/// chunk radius feeding the terrain-ready spawn threshold
/// (`min(viewDistance, radius) ** 2 * pi`).
pub const CHUNK_SENDING_SPAWN_RADIUS: i32 = 4;

/// `UpdateBlockPacket` id (Fase 2, Java -> Bedrock).
///
/// NOTE: this is `0x15` (21), *not* `0x0B`. `0x0B` is `StartGamePacket`
/// (see `data/protocol_info.json`: `"UpdateBlockPacket": 21`,
/// `"StartGamePacket": 11`); sending `0x0B` here would make the client
/// try to parse a StartGame. The plan's "0x0B" label is that mix-up.
pub const UPDATE_BLOCK_PACKET_ID: u32 = 0x15;
/// Update flags for `UpdateBlockPacket` as sent by PocketMine's
/// `World::createBlockUpdatePackets` (`UpdateBlockPacket::FLAG_NETWORK`).
pub const UPDATE_BLOCK_FLAGS_NETWORK: u32 = 0b0010;
/// Data layer as sent by PocketMine (`UpdateBlockPacket::DATA_LAYER_NORMAL`).
pub const UPDATE_BLOCK_DATA_LAYER_NORMAL: u32 = 0;

/// Ticks between periodic `SetTimePacket` broadcasts to Bedrock
/// clients (20 ticks = 1 second at 20 TPS). Clients advance the sun
/// locally between packets, so 1 Hz keeps them in lock-step with the
/// server clock without a packet per tick; jumps (`/time set`) are
/// broadcast immediately instead of waiting for this interval.
pub const TIME_BROADCAST_INTERVAL_TICKS: i32 = 20;

/// `UpdateBlockPacket` (0x15) wire bytes for one block.
///
/// Layout mirrors PocketMine's `UpdateBlockPacket::encodePayload`
/// (`target/BedrockProtocol/src/UpdateBlockPacket.php`): packet id (var_u32),
/// `BlockPosition` (signed var_i32 x/y/z), runtime block id (**unsigned**
/// var_u32 — unlike the LevelChunk palette, which uses zigzag var_i32),
/// flags (unsigned var_u32, [`UPDATE_BLOCK_FLAGS_NETWORK`]) and data layer
/// (unsigned var_u32, [`UPDATE_BLOCK_DATA_LAYER_NORMAL`]).
///
/// The previous layout used a zigzag id plus a single flags byte and no data
/// layer, so a real Bedrock client could not decode the packet (wrong block
/// id, truncated tail) and the break was invisible to every other Bedrock
/// viewer. `World::createBlockUpdatePackets` additionally sends a
/// `BlockActorDataPacket` for container tiles; this port has no tiles, so
/// there is nothing to add here.
/// PM `BlockBreakInfo::getBreakTime` (reference
/// `target/axolotl-pm/src/block/BlockBreakInfo.php`):
/// `hardness * (isToolCompatible ? 1.5 : 5.0)`. The bare hand is compatible
/// for blocks with `toolHarvestLevel == 0` (`isToolCompatible` returns true
/// when `toolHarvestLevel === 0`) and incompatible for tool-required blocks
/// (stone family, ores). We keep no item/tool model yet, so bare hand is
/// the faithful default.
///
/// Returns `None` for blocks whose `isBreakable()` is false (DEV: bedrock
/// is negative; air has no break info) and `Some(0.0)` for
/// `breaksInstantly()` blocks (hardness 0: tall grass, flowers, snow layer).
///
/// TODO(parity): extend to the full vanilla break-info table; hashed ids
/// outside the generator's output default to stone (1.5, incompatible).
fn block_break_time_secs(block_id: u32) -> Option<f32> {
    use bedrock_world::block_id as bid;
    if block_id == bid::AIR {
        return None;
    }
    let (hardness, bare_hand_compatible) = match block_id {
        bid::STONE => (1.5, false),
        bid::DIRT => (0.5, true),
        bid::GRASS_BLOCK => (0.6, true),
        bid::SAND => (0.5, true),
        bid::SANDSTONE => (0.8, false),
        bid::GRAVEL => (0.6, true),
        bid::COAL_ORE | bid::IRON_ORE | bid::GOLD_ORE | bid::REDSTONE_ORE
        | bid::LAPIS_ORE | bid::DIAMOND_ORE | bid::EMERALD_ORE => (3.0, false),
        bid::OAK_LOG | bid::SPRUCE_LOG | bid::BIRCH_LOG => (2.0, true),
        bid::OAK_LEAVES | bid::SPRUCE_LEAVES | bid::BIRCH_LEAVES => (0.2, true),
        bid::SHORT_GRASS | bid::TALL_GRASS | bid::DANDELION | bid::POPPY
        | bid::SNOW_LAYER => (0.0, true),
        // Unbreakable (hardness < 0 / not breakable): bedrock, water.
        bid::BEDROCK | bid::WATER => return None,
        // Unknown hashed id: conservative stone default.
        _ => (1.5, false),
    };
    let secs = if bare_hand_compatible {
        hardness * 1.5
    } else {
        hardness * 5.0
    };
    Some(secs)
}

pub fn update_block_wire(x: i32, y: i32, z: i32, block_id: u32) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.write_var_u32(UPDATE_BLOCK_PACKET_ID);
    BlockPos::new(x, y, z).write(&mut writer);
    writer.write_var_u32(block_id);
    writer.write_var_u32(UPDATE_BLOCK_FLAGS_NETWORK);
    writer.write_var_u32(UPDATE_BLOCK_DATA_LAYER_NORMAL);
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
    MoveActorDelta(MoveActorDeltaPacket),
    RemoveActor(RemoveActorPacket),
    RemoveObjective(RemoveObjectivePacket),
    SetDisplayObjective(SetDisplayObjectivePacket),
    SetScore(SetScorePacket),
    SetScoreboardIdentity(SetScoreboardIdentityPacket),
    /// `BossEventPacket` (0x4A): Bedrock boss bars (apiBossBar).
    BossEvent(BossEventPacket),
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
            Self::MoveActorDelta(p) => p.packet_id(),
            Self::RemoveActor(p) => p.packet_id(),
            Self::RemoveObjective(p) => p.packet_id(),
            Self::SetDisplayObjective(p) => p.packet_id(),
            Self::SetScore(p) => p.packet_id(),
            Self::SetScoreboardIdentity(p) => p.packet_id(),
            Self::BossEvent(p) => p.packet_id(),
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
            Self::MoveActorDelta(p) => p.encode(writer),
            Self::RemoveActor(p) => p.encode(writer),
            Self::RemoveObjective(p) => p.encode(writer),
            Self::SetDisplayObjective(p) => p.encode(writer),
            Self::SetScore(p) => p.encode(writer),
            Self::SetScoreboardIdentity(p) => p.encode(writer),
            Self::BossEvent(p) => p.encode(writer),
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

impl From<ScoreboardPacketBytes> for OutgoingPacket {
    fn from(packet: ScoreboardPacketBytes) -> Self {
        match packet {
            ScoreboardPacketBytes::RemoveObjective(p) => Self::RemoveObjective(p),
            ScoreboardPacketBytes::SetDisplayObjective(p) => Self::SetDisplayObjective(p),
            ScoreboardPacketBytes::SetScore(p) => Self::SetScore(p),
            ScoreboardPacketBytes::SetScoreboardIdentity(p) => Self::SetScoreboardIdentity(p),
        }
    }
}

/// PM `SurvivalBlockBreakHandler` state (src/player/SurvivalBlockBreakHandler.php):
/// everything the per-tick `update()` needs. `break_speed` is progress
/// percentage per game tick; `break_progress` accumulates it; the handler
/// breaks the block at `progress >= 1.0` (PM `update()` return).
#[derive(Debug, Clone, PartialEq)]
pub struct BreakHandlerState {
    /// Target block position.
    pub pos: (i32, i32, i32),
    /// Currently targeted face (`setTargetedFace` on CRACK_BREAK).
    pub targeted_face: i32,
    /// Calculated progress per tick (`calculateBreakProgressPerTick`).
    pub break_speed: f32,
    /// Accumulated break progress `0.0..=1.0`.
    pub break_progress: f32,
    /// FX ticker (`$this->fxTicker++ % fxTickInterval === 0`).
    pub fx_ticker: u32,
    /// PM `$maxPlayerDistance` (constructor arg passed by `attackBlock`).
    pub max_player_distance: i32,
}

/// A dynamic container opened with `ContainerOpenPacket` (0x2E), port of
/// PocketMine `Player::openInventory` for block inventories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenContainer {
    /// Client container id (10 = crafting table, 16 = anvil in BDS).
    pub window_id: u8,
    /// Bedrock `ContainerType` byte sent in `ContainerOpenPacket`
    /// (`CONTAINER_TYPE_WORKBENCH`, `CONTAINER_TYPE_ANVIL`, ...).
    pub container_type: u8,
    /// Block position of the container block.
    pub position: (i32, i32, i32),
    /// Inventory kind: `crafting_table` or `anvil`.
    pub kind: String,
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
    /// Independent head yaw (`PlayerAuthInputPacket.headYaw`), Bedrock
    /// canonical `[0, 360)`. The body yaw/pitch above drive movement;
    /// head yaw drives `EntityMetadata` head packets to Java viewers.
    pub head_yaw: f32,
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
    /// `NetworkSession::notifyTerrainReady()` has fired: `PLAYER_SPAWN` was
    /// sent and the session is in the SpawnResponse phase, awaiting
    /// `SetLocalPlayerAsInitialized` (`0x71`) before `doFirstSpawn`.
    pub terrain_ready: bool,
    /// `Player::$viewDistance !== -1`: the client sent `RequestChunkRadius`
    /// (`Player::setViewDistance`). Until then no chunks stream.
    pub view_distance_received: bool,
    /// `Player::$spawnThreshold`: chunks sent from spawn before
    /// `notifyTerrainReady` fires (`(min(viewDistance, 4) ** 2) * pi`).
    pub spawn_threshold: i32,
    /// `Player::$spawnChunkLoadCount`: chunks sent since spawn, `0..threshold`;
    /// `-1` once the threshold fired (never re-fired).
    pub spawn_chunk_load_count: i32,
    /// `Player::$nextChunkOrderRun`: `doChunkRequests` passes until the next
    /// `orderChunks()` run; `i32::MAX` means "not scheduled".
    pub next_chunk_order_run: i32,
    /// Pending chunk coordinates to stream, populated on `orderChunks()`.
    pub chunk_load_queue: Vec<(i32, i32)>,
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
    /// Length of the last `PlayerAuthInputPacket::input_data` tail, for the
    /// "auth_input tail len=…" tail-shape diagnostic (fires on change only).
    pub last_input_len: usize,
    /// Last instant the rate-limited "zero block actions with
    /// PERFORM_BLOCK_ACTIONS" diagnostic fired (at most one per 2 seconds).
    pub last_block_actions_diag: Option<std::time::Instant>,
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
    /// `InGamePacketHandler::$lastBlockAttacked` (PM): the block the client
    /// last attacked via START_BREAK/CONTINUE — the client sends CONTINUE
    /// for the current target right before `PREDICT_DESTROY`, which must
    /// not count as a new left-click.
    pub last_block_attacked: Option<(i32, i32, i32)>,
    /// PM `SurvivalBlockBreakHandler` (src/player/...): server-side dig
    /// progress for non-instantly-breakable blocks in non-creative modes.
    /// Creative clients break via `PREDICT_DESTROY` only (`attackBlock`
    /// never creates the handler in creative).
    pub break_handler: Option<BreakHandlerState>,
    /// Incoming `BossEventPacket`s (0x4A) for the manager to answer
    /// (apiBossBar `PacketListener`: register/unregister/query).
    pub pending_boss_events: Vec<BossEventPacket>,
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
    /// Currently opened dynamic container (crafting table, anvil, ...).
    pub open_container: Option<OpenContainer>,
    /// Crafting-table inventory (3x3 + result), `CraftingTable` block.
    pub crafting_table: crate::crafting::CraftingTableInventory,
    /// Anvil inventory (left, right, result + XP cost).
    pub anvil_inventory: crate::anvil::AnvilInventory,
    /// Anvil world-block damage stage for the anvil this player last hit.
    pub anvil_damage: u8,
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
    /// Per-player scoreboard (`Player::getScoreboard()`), `None` until a
    /// plugin or `/scoreboard` sets one. Re-sent after `PLAYER_SPAWN`.
    pub scoreboard: Option<Scoreboard>,
    /// The manager resolved the persisted personal scoreboard for the
    /// (login-known) username and queued it. Set once per session by
    /// `sync_scoreboard_after_spawn`.
    pub scoreboard_synced: bool,
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
            head_yaw: 0.0,
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
            terrain_ready: false,
            view_distance_received: false,
            // `Player::__construct`: `(CHUNK_SENDING_SPAWN_RADIUS(4) ** 2) * pi`
            spawn_threshold: ((CHUNK_SENDING_SPAWN_RADIUS as f64).powi(2)
                * std::f64::consts::PI) as i32,
            spawn_chunk_load_count: 0,
            next_chunk_order_run: 5,
            chunk_load_queue: Vec::new(),
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
            last_input_len: 0,
            last_block_actions_diag: None,
            food: 20.0,
            saturation: 20.0,
            xp_level: 0,
            xp_progress: 0.0,
            air: 300,
            pending_actions: Vec::new(),
            last_block_attacked: None,
            break_handler: None,
            pending_boss_events: Vec::new(),
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
            open_container: None,
            crafting_table: crate::crafting::CraftingTableInventory::new(),
            anvil_inventory: Default::default(),
            anvil_damage: 0,
            spawn_point: None,
            access: Arc::new(crate::access::AccessList::new()),
            data_dir: None,
            has_saved_data: false,
            first_played: 0.0,
            last_played: 0.0,
            metrics: None,
            permissions: None,
            scoreboard: None,
            scoreboard_synced: false,
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

    /// Writes JSON v2 (27 campos) directly to `players/<name>.json`.
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
            head_yaw: self.head_yaw as f64,
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
                        display_name: java_bridge::display::bedrock_view_display_name(
                            &player.name,
                            player.is_virtual,
                        ),
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
                display_name: self.username.clone(),
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
            self.head_yaw = self.yaw;
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
        // `Player::teleport`: re-order chunks around the destination and
        // restart the terrain-ready count unless it already fired.
        self.next_chunk_order_run = 0;
        if self.spawn_chunk_load_count != -1 {
            self.spawn_chunk_load_count = 0;
        }
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
        let ipc_started = std::time::Instant::now();
        let result = plugins.dispatch_event_sync(event);
        // Plugin events cross the process boundary synchronously, so their
        // round-trip is charged to the caller (the packet or tick thread).
        crate::metrics::ipc().record(ipc_started.elapsed());
        match result {
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
    /// `0x71` (diagnostics only — PocketMine never forces a spawn; the
    /// session simply awaits `RequestChunkRadius` and terrain-ready
    /// chunks).
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
        // Geyser/Floodgate-style cross-visibility (see `java_bridge::display`):
        // a Bedrock viewer sees every player verbatim, so the AddPlayer
        // username, PlayerList username and the metadata-4 nametag all use
        // the Bedrock-view display name (currently the raw name). Java ->
        // Bedrock is deliberately prefix-free, like Geyser's Java nick.
        let display = self.bedrock_display_name();
        let metadata = vec![
            MetadataEntry::new(0, MetadataValue::Long(self.data_flags())),
            MetadataEntry::new(4, MetadataValue::String(display.clone())),
            MetadataEntry::new(7, MetadataValue::Short(self.air)),
            MetadataEntry::new(42, MetadataValue::Short(self.air)),
            MetadataEntry::new(38, MetadataValue::Float(1.0)),
            MetadataEntry::new(53, MetadataValue::Float(0.6)),
            MetadataEntry::new(54, MetadataValue::Float(1.8)),
        ];
        AddPlayerPacket::new(
            self.uuid,
            display,
            self.entity_runtime_id,
            self.feet_position(),
            self.pitch,
            self.yaw,
            self.yaw,
            self.inventory.held_item(),
            self.gamemode,
            metadata,
            // `Human::spawnTo` passes `$this->getId()` (entity id), not the
            // XUID stored in `entity_unique_id`.
            bedrock_protocol::AbilitiesData::visitor(self.entity_runtime_id as i64),
        )
    }

    pub fn skin_data(&self) -> SkinData {
        SkinData::default_for(&self.uuid)
    }

    pub fn player_list_addition(&self) -> PlayerListEntry {
        // `NetworkSession::syncPlayerList` (axolotl `NetworkSession.php:1374`):
        // `PlayerListEntry::createAdditionEntry($player->getUniqueId(),
        // $player->getId(), ...)` — `actorUniqueId` is the small entity id
        // (`getId()` == `entity_runtime_id`), NOT the XUID. The XUID travels
        // in the `xboxUserId` string field (`self.xuid`). Using the XUID /
        // FNV hash here duplicated the Xbox id in two fields and did not
        // match `StartGamePacket`'s `actorUniqueId` semantics in PHP
        // (`PreSpawnPacketHandler::setUp()` passes `$player->getId()` for
        // both `actorUniqueId` and `actorRuntimeId`).
        // The username is the Bedrock-view display name (verbatim, see
        // `java_bridge::display`), matching `add_player_packet` and the
        // metadata-4 nametag.
        PlayerListEntry::addition(
            self.uuid,
            self.entity_runtime_id as i64,
            self.bedrock_display_name(),
            self.skin_data(),
            self.xuid.clone(),
        )
    }

    /// Name as seen by Bedrock viewers (verbatim, Floodgate does not prefix
    /// Java in Bedrock; see `java_bridge::display`).
    pub fn bedrock_display_name(&self) -> String {
        bedrock_view_display_name(&self.username, false)
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

        // Deadlock instrumentation (INFO, visible in `server.log` at the
        // default level): the Bedrock login deadlock is silent — after
        // `StartGame` the server logs nothing when `RequestChunkRadius`
        // (`0x45`) never arrives. This line makes "client sent / client
        // never sent" distinguishable for the three spawn-gating packets
        // without spamming for high-frequency packets (`0x90` is still
        // logged at dispatch, but its handler stays silent in PreSpawn).
        // `0x90` (PlayerAuthInput) arrives 20x/second: only log it while the
        // client is still spawning. Logging it forever produced ~130k lines
        // per run and measurably slowed the tick/bridge.
        if matches!(packet_id, 0x45 | 0x71) || (packet_id == 0x90 && !self.spawned) {
            info!(
                "Session {} handle_packet dispatch: packet=0x{:02X} len={} state={:?} terrain_ready={} spawned={} view_distance_received={}",
                self.session_id,
                packet_id,
                payload.len(),
                self.state,
                self.terrain_ready,
                self.spawned,
                self.view_distance_received
            );
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
            // `MovePlayerPacket` (0x13) serverbound: `InGamePacketHandler`
            // is `#[SilentDiscard(MovePlayerPacket::class)]` — echoed
            // movement packets carry no new information (movement travels in
            // `PlayerAuthInput` 0x90) and are ignored without a response.
            0x13 => {
                let _ = payload;
                self.handle_move_player()
            }
            0x09 => {
                let mut reader = Reader::new(payload);
                let packet = TextPacket::decode(&mut reader)?;
                self.handle_text(packet)
            }
            // BossEventPacket (0x4A): only register/unregister/query
            // travel client -> server (apiBossBar `PacketListener`).
            // Queued for the manager (`handle_boss_event`), which
            // owns the registry needed to answer `TYPE_QUERY`.
            0x4A => {
                let mut reader = Reader::new(payload);
                match BossEventPacket::decode(&mut reader) {
                    Ok(packet) => {
                        debug!(
                            "BossEventPacket from session {}: boss={} player={} type={}",
                            self.session_id,
                            packet.boss_actor_unique_id,
                            packet.player_actor_unique_id,
                            packet.event_type
                        );
                        self.pending_boss_events.push(packet);
                        Ok(Vec::new())
                    }
                    Err(error) => {
                        warn!(
                            "Failed to decode BossEventPacket from session {}: {}",
                            self.session_id, error
                        );
                        Ok(Vec::new())
                    }
                }
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
            0x93 => {
                let mut reader = Reader::new(payload);
                match bedrock_protocol::packets::item_stack_request::ItemStackRequestPacket::decode_payload(&mut reader) {
                    Ok(packet) => Ok(self.handle_item_stack_request(packet)),
                    Err(error) => {
                        warn!(
                            "Failed to decode ItemStackRequestPacket from session {}: {}",
                            self.session_id, error
                        );
                        // Reject the requests so the client rolls its
                        // prediction back instead of diverging silently.
                        Ok(self.item_stack_error_response(payload))
                    }
                }
            }
            0x2F => {
                let mut reader = Reader::new(payload);
                match bedrock_protocol::packets::container_close::ContainerClosePacket::decode_payload(&mut reader) {
                    Ok(packet) => Ok(self.handle_container_close(packet)),
                    Err(error) => {
                        warn!(
                            "Failed to decode ContainerClosePacket from session {}: {}",
                            self.session_id, error
                        );
                        Ok(Vec::new())
                    }
                }
            }
            0x21 => {
                let mut reader = Reader::new(payload);
                match bedrock_protocol::packets::interact::InteractPacket::decode_payload(&mut reader) {
                    Ok(packet) => Ok(self.handle_interact(packet)),
                    Err(error) => {
                        warn!(
                            "Failed to decode InteractPacket from session {}: {}",
                            self.session_id, error
                        );
                        Ok(Vec::new())
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

                let bisect_no_static = std::env::var("PM_CORE_BISECT_NOSTATIC").as_deref() == Ok("1");
                // 1. JigsawStructureDataPacket (0x139 / 313) - Required before StartGame in 1.21+ / 1.26
                if !bisect_no_static {
                    out.push(OutgoingPacket::RawWire(StaticPackets::JIGSAW_STRUCTURE_DATA.to_vec()));
                }

                // 2. VoxelShapesPacket (0x151 / 337) - Required before StartGame in 1.21+ / 1.26
                if !bisect_no_static {
                    out.push(OutgoingPacket::RawWire(StaticPackets::VOXEL_SHAPES.to_vec()));
                }

                // 3. StartGamePacket (0x0B / 11) - Core world configuration
                // `PreSpawnPacketHandler::setUp()` (LevelSettings.php + StartGamePacket.php):
                // seed = -1, SpawnSettings(BIOME_TYPE_DEFAULT=0, "", OVERWORLD=0),
                // hasEduFeaturesEnabled = true, isTexturePacksRequired = true
                // (LevelSettings default), limitedWorld 0, PlayerMovementSettings(0, true),
                // levelId = "". The world spawn / time / difficulty / gamemode /
                // generator are the only live fields.
                // `blockNetworkIdsAreHashes` is deliberately NOT taken from
                // `PreSpawnPacketHandler`: that line passes `false` because
                // PMMP resolves ids as indices into the *full* canonical
                // palette it sends in the same packet
                // (`StaticPacketCache::getBlockPaletteEntries()` +
                // `BlockTranslator::internalIdToNetworkId` -> canonical
                // index in `data/canonical_block_states.nbt`). This server
                // sends an empty palette (like vanilla BDS, see
                // `data/bds-capture/start_game/1.json`:
                // `"block_properties": []`,
                // `"block_network_ids_are_hashes": true`) and every chunk /
                // UpdateBlock / inventory runtime id it emits is the
                // FNV-1a hash from `bedrock-world/src/block_id.rs`
                // (verified against BDS `creative_content` / `crafting_data`
                // captures for 25 of the 27 generator blocks; AIR and WATER
                // come from `ModernChunk::BEDROCK_BLOCKS`, the same table
                // `java_chunk.rs` uses for Bedrock<->Java sync, and the
                // client renders sky and water from them). Sending
                // `false` with an empty palette and hash ids is the
                // `Block / ClientDisconnection-90` signature (hash values
                // read as palette indices, far outside the 0..23960
                // canonical range). `StartGamePacket::default()` already
                // uses `true`; keeping the default restores consistency.
                let mut start_game = StartGamePacket::default();
                // `PreSpawnPacketHandler::setUp()` (`StartGamePacket::create`):
                // `actorUniqueId = $player->getId()` and
                // `actorRuntimeId = $player->getId()` — the same small entity
                // id for both. `entity_unique_id` here holds the Xbox XUID
                // (or an FNV fallback hash, see `handle_login`), which in PHP
                // is sent only as `PlayerList.xboxUserId`, never as an actor
                // id. Use the runtime id (== session id) for `actorUniqueId`
                // so `StartGame` and the `PlayerList` addition above agree
                // with PHP (`unique == runtime == getId()`).
                start_game.actor_unique_id = self.entity_runtime_id as i64;
                start_game.actor_runtime_id = self.entity_runtime_id;
                start_game.player_gamemode = self.gamemode;
                if !self.has_saved_data {
                    let (px, py, pz) = self.generator.player_position();
                    self.position = Vector3f::new(px, py, pz);
                }
                start_game.player_position = self.eye_position();
                start_game.pitch = self.pitch;
                start_game.yaw = self.yaw;
                start_game.level_settings.seed = u64::MAX;
                start_game.level_settings.spawn_biome_type = 0;
                start_game.level_settings.spawn_biome_name = String::new();
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
                start_game.level_settings.has_edu_features_enabled = true;
                start_game.level_settings.is_texture_packs_required = true;
                start_game.level_settings.limited_world_width = 0;
                start_game.level_settings.limited_world_length = 0;
                start_game.level_id = String::new();
                start_game.world_name = "Axolotl-PM World".to_string();
                start_game.rewind_history_size = 0;
                // Keep `StartGamePacket::default()` (`true`): must match the
                // FNV-1a hash runtime ids in every chunk palette (see
                // `bedrock-world/src/block_id.rs` and the BDS capture note
                // above). Removing this override (previously `false`,
                // copied from PMMP's index-mode `StartGamePacket::create`)
                // fixes the out-of-range palette ids the client rejects
                // with `Block / ClientDisconnection-90`.
                // Snapshot for the INFO diagnostic below: `start_game` is
                // moved into `out`, so reading its fields after the push
                // would be a use-after-move.
                let sg_seed = start_game.level_settings.seed;
                let sg_generator = start_game.level_settings.generator_type;
                let sg_difficulty = start_game.level_settings.difficulty;
                let sg_time = start_game.level_settings.time;
                let sg_dimension = start_game.level_settings.dimension;
                let sg_rewind = start_game.rewind_history_size;
                let sg_hashes = start_game.block_network_ids_are_hashes;
                if bisect_no_static {
                    // BISSECT: send verbatim BDS StartGame (1025 B) to isolate LevelChunk
                    out.push(OutgoingPacket::RawWire(StaticPackets::START_GAME.to_vec()));
                    let _ = start_game;
                } else {
                    out.push(OutgoingPacket::StartGame(start_game));
                }

                // 4. ItemRegistryPacket (0xA2 / 162) - Dynamic session item dictionary
                if !bisect_no_static {
                    out.push(OutgoingPacket::RawWire(StaticPackets::ITEM_REGISTRY.to_vec()));
                }

                // 5. AvailableActorIdentifiersPacket (0x77 / 119) - Entity definitions
                if !bisect_no_static {
                    out.push(OutgoingPacket::RawWire(StaticPackets::AVAILABLE_ENTITY_IDENTIFIERS.to_vec()));
                }

                // 6. BiomeDefinitionListPacket (0x7A / 122) - World biomes
                if !bisect_no_static {
                    out.push(OutgoingPacket::RawWire(StaticPackets::BIOME_DEFINITION_LIST.to_vec()));
                }

                // 7. UpdateAttributesPacket (0x1D / 29) -
                // `entityEventBroadcaster->syncAttributes`: health, hunger,
                // movement speed. No `SetHealthPacket` (0x2A) here: PM
                // never sends it (health is the `minecraft:health`
                // attribute) and no `TrimDataPacket` (0x12E): PM never
                // sends it at login.
                if !bisect_no_static {
                    out.push(OutgoingPacket::RawWire(self.create_update_attributes_packet()));
                }

                // 8. AvailableCommandsPacket (0x4C / 76) -
                // `syncAvailableCommands()`: registers the native commands
                // so the client lists and autocompletes them. Before
                // abilities, like the reference.
                out.push(OutgoingPacket::RawWire(available_commands_wire()));

                // 9. UpdateAbilitiesPacket (0xBB / 187) + AdventureSettings
                // (0xBC / 188) - `syncAbilities()` then
                // `syncAdventureSettings()`.
                if !bisect_no_static {
                    out.push(OutgoingPacket::RawWire(self.create_update_abilities_packet()));
                    out.push(OutgoingPacket::RawWire(StaticPackets::UPDATE_ADVENTURE_SETTINGS.to_vec()));
                }

                // 10. Effects: `onEntityEffectAdded` per active effect.
                // A fresh join has none, so nothing is queued here.

                // 11. SetActorDataPacket (0x27 / 39) -
                // `player->sendData([$player])`: collision, gravity,
                // scale, air.
                if !bisect_no_static {
                    out.push(OutgoingPacket::RawWire(self.create_set_actor_data_packet()));
                }

                // 12. Inventory `syncAll()` (36-slot windows) +
                // `syncSelectedHotbarSlot()` (`MobEquipmentPacket` 0x1F).
                if !bisect_no_static {
                    for window_wire in self.inventory_window_wires() {
                        out.push(OutgoingPacket::RawWire(window_wire));
                    }
                    out.push(OutgoingPacket::RawWire(self.mob_equipment_wire()));
                }

                // 13. Creative catalog (`0x91`) - `syncCreative()` -
                // then 14. recipe book (`0x34`) - `CraftingDataCache::
                // getCache()`, before `syncPlayerList`. Both wires come
                // from `data/recipes/` / `data/creative/` (generated) or
                // the `data/bds-capture/` re-encodings (capture, the
                // default) — see `static_packets.rs`.
                // `PM_CORE_CREATIVE=off` omits `0x91` entirely; the
                // generated vs capture choice is `PM_CORE_CREATIVE_MODE`
                // (read inside `creative_content_wire`).
                if std::env::var("PM_CORE_CREATIVE").as_deref() != Ok("off") {
                    let creative_mode = std::env::var("PM_CORE_CREATIVE_MODE")
                        .unwrap_or_else(|_| "capture".to_string());
                    let wire = Self::creative_content_wire();
                    info!(
                        "CreativeContent 0x91: mode={} len={}",
                        creative_mode,
                        wire.len()
                    );
                    out.push(OutgoingPacket::RawWire(wire));
                } else {
                    info!("CreativeContent 0x91: mode=off (not sending 0x91)")
                }
                // Diagnostic: `PM_CORE_CRAFTING_MODE=off` omits 0x34 entirely to
                // isolate the "invalid string" crash on Bedrock login.
                let crafting_mode = std::env::var("PM_CORE_CRAFTING_MODE").unwrap_or_else(|_| "capture".to_string());
                match crafting_mode.as_str() {
                    "off" => {
                        info!("CraftingData 0x34: mode=off (not sending 0x34)")
                    }
                    mode => {
                        let wire = Self::crafting_data_wire();
                        info!("CraftingData 0x34: mode={} len={}", mode, wire.len());
                        out.push(OutgoingPacket::RawWire(wire));
                    }
                }

                // 15. PlayerListPacket (0x3F) - `syncPlayerList(online)`.
                // LAST packet of `setUp()`: the 1.26 client only leaves
                // "Localizando servidor..." and sends `RequestChunkRadius`
                // after it. Without it the session deadlocks in PreSpawn
                // (no chunks, no `PLAYER_SPAWN`). Self is the first
                // entry; later joins are broadcast via
                // `broadcast_add_player_locked` after `doFirstSpawn`.
                out.push(OutgoingPacket::PlayerList(PlayerListPacket::with_additions(
                    vec![self.player_list_addition()],
                )));

                // NOTE: `PreSpawnPacketHandler::setUp()` sends no chunk
                // packets at all — no `NetworkChunkPublisherUpdate`, no
                // `LevelChunk`. Chunks stream later via
                // `do_chunk_requests()` (`Player::doChunkRequests`, driven by
                // `NetworkSession::tick()`): `RequestChunkRadius` ->
                // `ChunkRadiusUpdated`, then `orderChunks()` emits the
                // publisher update and `LevelChunk`s (up to
                // `CHUNK_SENDING_PER_TICK` per pass), and after
                // `spawn_threshold` chunks `notifyTerrainReady()` sends
                // `PLAYER_SPAWN`. Sending chunks inside this bundle made the
                // 1.26 client close the session right after login.
                //
                // NOTE: PLAYER_SPAWN is intentionally NOT sent here either. It
                // is sent by `do_chunk_requests()` once the terrain-ready
                // threshold is reached; `0x71` then completes `doFirstSpawn`.

                // Visible at default log level so a silent login (StartGame
                // queued but never acknowledged) is distinguishable from a
                // malformed StartGame the client dropped. Includes the
                // PreSpawn bundle size + wire order (must mirror
                // `PreSpawnPacketHandler::setUp()` lines 66-171), the
                // `StartGame` identity fields, and the `PlayerList` self
                // entry (`syncPlayerList`): if the client never answers with
                // `0x45`, the next `handle_packet dispatch` line will simply
                // never appear — the silent-discard signature.
                {
                    let packet_ids: Vec<String> = out
                        .iter()
                        .map(|packet| format!("0x{:02X}", packet.packet_id()))
                        .collect();
                    let uuid_hex: String = self
                        .uuid
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect();
                    info!(
                        "Session {} handle_resource_pack_response: PreSpawn bundle out={} packets=[{}] StartGame(unique={}, runtime={}, xuid_entity={}, gamemode={}, seed=0x{:016X}, generator={}, difficulty={}, time={}, dimension={}, rewind={}, hashes={}) PlayerList(uuid={}, username='{}', xuid='{}', actorUniqueId={}) entered PreSpawn (awaiting RequestChunkRadius 0x45 and terrain-ready chunks for PLAYER_SPAWN, then 0x71)",
                        self.session_id,
                        out.len(),
                        packet_ids.join(","),
                        self.entity_runtime_id as i64,
                        self.entity_runtime_id,
                        self.entity_unique_id,
                        self.gamemode,
                        sg_seed,
                        sg_generator,
                        sg_difficulty,
                        sg_time,
                        sg_dimension,
                        sg_rewind,
                        sg_hashes,
                        uuid_hex,
                        self.username,
                        self.xuid,
                        self.entity_runtime_id as i64,
                    );
                }

                Ok(out)
            }
            other => {
                debug!("Session {} sent unknown resource pack status: {}", self.session_id, other);
                Ok(Vec::new())
            }
        }
    }

    /// NetworkSession::onEnterWorld: syncs Time / Difficulty / Spawn (+
    /// weather, so `/weather` survives every join; PM leaves a TODO there).
    /// `PreSpawnPacketHandler::setUp()` sends NONE of these — time,
    /// difficulty and spawn live in `StartGamePacket` (`LevelSettings`)
    /// and are re-synced here on world change / first spawn.
    fn on_enter_world_packets(&self) -> Vec<OutgoingPacket> {
        let (sx, sy, sz) = self.world.spawn();
        let mut out = vec![
            OutgoingPacket::RawWire(set_time_wire(self.world.time())),
            OutgoingPacket::RawWire(set_difficulty_wire(self.world.difficulty() as u32)),
            OutgoingPacket::SetSpawnPosition(SetSpawnPositionPacket::new_player_spawn(
                BlockPos::new(sx, sy, sz),
                0,
            )),
        ];
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
        out
    }

    /// `SpawnResponsePacketHandler` + `Player::doFirstSpawn` equivalent:
    /// the client answered `PLAYER_SPAWN` with
    /// `SetLocalPlayerAsInitialized` (`0x71`). Fires the join event exactly
    /// once and enters InGame. It does NOT send `PLAYER_SPAWN` — that
    /// belongs to `notifyTerrainReady()`, fired by the chunk-streaming
    /// threshold in [`Self::do_chunk_requests`]. A `0x71` arriving before
    /// terrain-ready is ignored (the SpawnResponse handler does not exist
    /// yet in the reference), as is a duplicate one after `doFirstSpawn`.
    pub fn handle_set_local_player_as_initialized(
        &mut self,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        info!(
            "Session {} handle_set_local_player_as_initialized dispatch: state={:?} terrain_ready={} spawned={} prespawn_elapsed_ms={}",
            self.session_id,
            self.state,
            self.terrain_ready,
            self.spawned,
            self.prespawn_elapsed().map(|elapsed| elapsed.as_millis()).unwrap_or(0)
        );
        if self.state != SessionState::PreSpawn && self.state != SessionState::InGame {
            return Ok(Vec::new());
        }
        if !self.terrain_ready || self.spawned {
            info!(
                "Session {} handle_set_local_player_as_initialized ignored: terrain_ready={} spawned={} (0x71 before terrain-ready is ignored, like SpawnResponsePacketHandler)",
                self.session_id, self.terrain_ready, self.spawned
            );
            return Ok(Vec::new());
        }
        self.spawned = true;
        self.prespawn_started_at = None;
        self.state = SessionState::InGame;
        self.pending_spawn_broadcast = true;
        info!(
            "Session {} handle_set_local_player_as_initialized: doFirstSpawn (InGame)",
            self.session_id
        );

        let mut out = self.on_enter_world_packets();

        // Scoreboard objectives are client-side state: re-send the
        // current board after every spawn (PocketMine re-sends its
        // objectives on respawn). The manager syncs the persisted
        // personal copy right after this returns (see
        // `sync_scoreboard_after_spawn`), so the global seed sent here
        // is overwritten by the personal board when one exists.
        if let Some(scoreboard) = &self.scoreboard {
            out.extend(
                scoreboard
                    .bedrock_packets()
                    .into_iter()
                    .map(OutgoingPacket::from),
            );
        }

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

    /// `Player::setViewDistance` equivalent: clamps via
    /// `Server::getAllowedViewDistance` (`max(2, min(distance, max_view_distance))`),
    /// recomputes the spawn threshold, schedules chunk ordering and sends
    /// `ChunkRadiusUpdatedPacket` — and only that. Chunks stream from
    /// [`Self::do_chunk_requests`]; nothing else is sent here.
    pub fn set_view_distance(&mut self, distance: i32) -> Vec<OutgoingPacket> {
        // (PocketMine fires `PlayerViewDistanceChangeEvent` on change; the
        // plugin bridge has no such event, so no event is fired here.)
        // `Server::getViewDistance()` is `max(2, config view-distance)` and
        // `getAllowedViewDistance()` is `max(2, min(distance, memory
        // .getViewDistance(getViewDistance())))`; `MemoryManager` is
        // identity outside low memory, so the ceiling is `max(2, max)`.
        let clamped = distance.clamp(2, self.max_view_distance.max(2));
        self.chunk_radius = clamped;
        self.view_distance_received = true;
        self.spawn_threshold =
            ((clamped.min(CHUNK_SENDING_SPAWN_RADIUS) as f64).powi(2) * std::f64::consts::PI) as i32;
        self.next_chunk_order_run = 0;
        info!(
            "Session {} handle_chunk_radius_request: RequestChunkRadius 0x45 radius={} clamped={} spawn_threshold={} state={:?} max_view_distance={}",
            self.session_id, distance, clamped, self.spawn_threshold, self.state, self.max_view_distance
        );
        vec![OutgoingPacket::ChunkRadiusUpdated(
            ChunkRadiusUpdatedPacket::new(clamped),
        )]
    }

    /// Handles RequestChunkRadiusPacket (0x45).
    ///
    /// `PreSpawnPacketHandler::handleRequestChunkRadius` and
    /// `InGamePacketHandler::handleRequestChunkRadius` both delegate to
    /// `Player::setViewDistance()` — no spawn forcing, no publisher update,
    /// no chunks. The old fallback that force-spawned on the first radius
    /// request inverted the protocol (PLAYER_SPAWN before terrain-ready)
    /// and the 1.26 client closed the session ~0.3 s after login.
    pub fn handle_chunk_radius_request(
        &mut self,
        packet: RequestChunkRadiusPacket,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        info!(
            "Session {} handle_chunk_radius_request dispatch: radius={} state={:?} (PreSpawn handleRequestChunkRadius -> setViewDistance)",
            self.session_id, packet.radius, self.state
        );
        if !(self.state == SessionState::InGame || self.state == SessionState::PreSpawn) {
            return Err(SessionError::UnexpectedPacket {
                state: self.state,
                packet_id: 0x45,
            });
        }
        Ok(self.set_view_distance(packet.radius))
    }

    /// `ChunkSelector::selectChunks` port (nearest-first rings over the
    /// circle of `radius` chunks centered on the given chunk). Mirrors the
    /// reference exactly: ring `subRadius` covers
    /// `subRadius^2 <= dist^2 < (subRadius+1)^2`, the left bound is
    /// `(int)(subRadius / M_SQRT2)`, and coordinates are mirrored into four
    /// quadrants (plus the diagonal mirror when `x != z`).
    fn select_chunks(radius: i32, center_x: i32, center_z: i32) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        if radius <= 0 {
            return out;
        }
        for sub_radius in 0..radius {
            let sub_sq = sub_radius * sub_radius;
            let next_sq = (sub_radius + 1) * (sub_radius + 1);
            let min_x = (sub_radius as f64 / std::f64::consts::SQRT_2) as i32;
            let mut last_z = 0;
            let mut x = sub_radius;
            while x >= min_x {
                let mut z = last_z;
                while z <= x {
                    let dist_sq = x * x + z * z;
                    if dist_sq >= next_sq {
                        break;
                    }
                    if dist_sq >= sub_sq {
                        last_z = z;
                        /* Top right quadrant */
                        out.push((center_x + x, center_z + z));
                        /* Top left quadrant */
                        out.push((center_x - x - 1, center_z + z));
                        /* Bottom right quadrant */
                        out.push((center_x + x, center_z - z - 1));
                        /* Bottom left quadrant */
                        out.push((center_x - x - 1, center_z - z - 1));
                        if x != z {
                            /* Top right quadrant mirror */
                            out.push((center_x + z, center_z + x));
                            /* Top left quadrant mirror */
                            out.push((center_x - z - 1, center_z + x));
                            /* Bottom right quadrant mirror */
                            out.push((center_x + z, center_z - x - 1));
                            /* Bottom left quadrant mirror */
                            out.push((center_x - z - 1, center_z - x - 1));
                        }
                    }
                    z += 1;
                }
                x -= 1;
            }
        }
        out
    }

    /// Publisher-update radius in blocks for the current view distance
    /// (`NetworkSession::syncViewAreaCenterPoint`: `viewDistance * 16`).
    fn publisher_radius_blocks(view_distance: i32) -> u32 {
        (view_distance.max(0) as u32) * 16
    }

    /// `Player::doChunkRequests` pass (driven by `NetworkSession::tick()`
    /// in the reference; here driven once per central tick by
    /// [`SessionManager::drive_chunk_requests`], so streaming progresses
    /// even when the client sends no further packets after
    /// `RequestChunkRadius`):
    ///
    /// - when an ordering pass is scheduled, `orderChunks()` runs: chunks
    ///   outside the new radius are unloaded and, if the load/unload sets are
    ///   non-empty, a `NetworkChunkPublisherUpdate` (center = player block,
    ///   radius = viewDistance * 16) is queued before any chunk;
    /// - up to `CHUNK_SENDING_PER_TICK` missing `LevelChunk`s are sent
    ///   nearest-first, each with dimension 0, `sub_chunk_request_limit`
    ///   `None`, cache disabled, no blob hashes, `client_biome_update`
    ///   `false` (`ChunkRequestTask` creation);
    /// - each sent chunk advances the terrain-ready counter; the chunk that
    ///   reaches `spawn_threshold` fires `notifyTerrainReady()` — the
    ///   `PlayStatusPacket::PLAYER_SPAWN` is queued right after that chunk.
    ///
    /// No-op until `RequestChunkRadius` set the view distance.
    /// True when [`Self::do_chunk_requests`] would stream terrain to this
    /// session (the manager uses it to warm payloads outside its lock).
    pub fn wants_chunk_streaming(&self) -> bool {
        self.view_distance_received
            && (self.state == SessionState::PreSpawn || self.state == SessionState::InGame)
    }

    /// Columns the next [`Self::do_chunk_requests`] pass will stream.
    ///
    /// Generating a cold column costs ~18 ms, so the manager warms this list
    /// in parallel *before* taking the sessions lock (see
    /// [`SessionManager::drive_chunk_requests`]): while the payload tables were
    /// built under the lock, every packet handler — a block break's particles
    /// and sound included — waited behind them for 60-160 ms, which the client
    /// also felt as lag and, under fast breaking, as a dropped connection.
    pub fn next_chunk_columns(&self) -> Vec<(i32, i32)> {
        if !self.chunk_load_queue.is_empty() {
            return self
                .chunk_load_queue
                .iter()
                .take(CHUNK_SENDING_PER_TICK)
                .copied()
                .collect();
        }
        let allowed_radius = self.chunk_radius.clamp(2, self.max_view_distance.max(2));
        let curr_cx = (self.position.x.floor() as i32) >> 4;
        let curr_cz = (self.position.z.floor() as i32) >> 4;
        Self::select_chunks(allowed_radius, curr_cx, curr_cz)
            .into_iter()
            .filter(|chunk| !self.sent_chunks.contains(chunk))
            .take(CHUNK_SENDING_PER_TICK)
            .collect()
    }

    pub fn do_chunk_requests(&mut self) -> Vec<OutgoingPacket> {
        if !self.view_distance_received {
            return Vec::new();
        }
        if !(self.state == SessionState::PreSpawn || self.state == SessionState::InGame) {
            return Vec::new();
        }

        let mut out = Vec::new();

        // `Player::doChunkRequests()`: `if (next !== PHP_INT_MAX &&
        // next-- <= 0) { next = PHP_INT_MAX; orderChunks(); }` — trigger
        // when the *old* value is <= 0, not when the decremented value is.
        let mut ordered = false;
        if self.next_chunk_order_run != i32::MAX {
            let old = self.next_chunk_order_run;
            self.next_chunk_order_run -= 1;
            if old <= 0 {
                self.next_chunk_order_run = i32::MAX;
                ordered = true;
            }
        }

        // `orderChunks()`: selection uses `Server::getAllowedViewDistance(
        // viewDistance)` while `syncViewAreaCenterPoint(location,
        // viewDistance)` (radius `*16`) uses the stored value. They coincide
        // once `set_view_distance` clamped, but the split keeps the port
        // exact if the ceiling moves between calls.
        let stored_view_distance = self.chunk_radius;
        if ordered || (self.chunk_load_queue.is_empty() && self.sent_chunks.is_empty()) {
            let allowed_radius =
                stored_view_distance.clamp(2, self.max_view_distance.max(2));
            let curr_cx = (self.position.x.floor() as i32) >> 4;
            let curr_cz = (self.position.z.floor() as i32) >> 4;
            let needed = Self::select_chunks(allowed_radius, curr_cx, curr_cz);
            let needed_set: HashSet<(i32, i32)> = needed.iter().copied().collect();

            let before = self.sent_chunks.len();
            self.sent_chunks.retain(|chunk| needed_set.contains(chunk));
            let unloaded = before - self.sent_chunks.len();

            self.chunk_load_queue = needed
                .into_iter()
                .filter(|chunk| !self.sent_chunks.contains(chunk))
                .collect();

            // `orderChunks()` syncs the view-area center point when the
            // load/unload sets are non-empty — always before the chunks.
            if ordered && (!self.chunk_load_queue.is_empty() || unloaded > 0) {
                let center = BlockPos::new(
                    self.position.x.floor() as i32,
                    self.position.y.floor() as i32,
                    self.position.z.floor() as i32,
                );
                out.push(OutgoingPacket::NetworkChunkPublisherUpdate(
                    NetworkChunkPublisherUpdatePacket::new(
                        center,
                        Self::publisher_radius_blocks(stored_view_distance),
                    ),
                ));
            }
        }

        if self.chunk_load_queue.is_empty() {
            // Deadlock instrumentation: `do_chunk_requests` ran (client sent
            // *something* after the radius) but there was nothing to stream.
            if ordered || !out.is_empty() {
                debug!(
                    "Session {} do_chunk_requests: view_distance_received={} chunk_radius={} spawn_threshold={} spawn_chunk_load_count={} sent_chunks={} missing=0 out={} ordered={} state={:?}",
                    self.session_id,
                    self.view_distance_received,
                    self.chunk_radius,
                    self.spawn_threshold,
                    self.spawn_chunk_load_count,
                    self.sent_chunks.len(),
                    out.len(),
                    ordered,
                    self.state
                );
            }
            return out;
        }

        let take_count = self.chunk_load_queue.len().min(CHUNK_SENDING_PER_TICK);
        let to_send: Vec<(i32, i32)> = self.chunk_load_queue.drain(..take_count).collect();
        let to_send_len = to_send.len();
        for chunk in &to_send {
            self.sent_chunks.insert(*chunk);
        }

        // Generate chunk payloads in parallel across all CPU cores using
        // rayon (same pattern as the old packet-handler streaming).
        let world = Arc::clone(&self.world);
        let packets: Vec<LevelChunkPacket> = to_send
            .into_par_iter()
            .map(|(cx, cz)| {
                let (sub_chunk_count, payload) = world.chunk_payload(cx, cz);
                LevelChunkPacket::new(
                    cx,
                    cz,
                    0, // DimensionIds::OVERWORLD
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
            // `Player::requestChunks()` completion callback: count toward the
            // terrain-ready threshold.
            if self.spawn_chunk_load_count == -1 {
                // Already fired: keep streaming normally.
            } else if self.spawn_chunk_load_count == self.spawn_threshold {
                self.spawn_chunk_load_count = -1;
                if !self.terrain_ready {
                    self.terrain_ready = true;
                    info!(
                        "Session {} do_chunk_requests: notifyTerrainReady PLAYER_SPAWN sent (threshold={} chunk_radius={} state={:?})",
                        self.session_id, self.spawn_threshold, self.chunk_radius, self.state
                    );
                    out.push(OutgoingPacket::PlayStatus(PlayStatusPacket {
                        status: PlayStatusPacket::PLAYER_SPAWN,
                    }));
                }
            } else {
                self.spawn_chunk_load_count += 1;
            }
        }

        debug!(
            "Session {} do_chunk_requests: sent {} chunks (spawn_chunk_load_count={}/{} terrain_ready={} sent_chunks={} out={})",
            self.session_id,
            to_send_len,
            self.spawn_chunk_load_count,
            self.spawn_threshold,
            self.terrain_ready,
            self.sent_chunks.len(),
            out.len()
        );

        out
    }

    /// Handles serverbound `MovePlayerPacket` (0x13).
    ///
    /// Mirrors `InGamePacketHandler`'s
    /// `#[SilentDiscard(MovePlayerPacket::class)]`: the packet is discarded
    /// without processing or response. Movement arrives in
    /// `PlayerAuthInput` (0x90); serverbound `MovePlayer` echoes add no
    /// information (per the reference comment: "Not needed, noisy debug
    /// when landing on ground").
    pub fn handle_move_player(&mut self) -> Result<Vec<OutgoingPacket>, SessionError> {
        Ok(Vec::new())
    }

    /// Handles PlayerAuthInputPacket (0x90).
    ///
    /// Mirrors `InGamePacketHandler::handlePlayerAuthInput`:
    /// - NaN/Inf in position/rotation is discarded,
    /// - the client position is the eye (- 1.62 gives the feet),
    /// - rotation is folded into [0, 360),
    /// - sneak/sprint/swim/glide/fly toggles are edge-detected and flagged
    ///   for a SetActorData broadcast by the manager.
    ///
    /// In PreSpawn / SpawnResponse the reference handlers
    /// (`PreSpawnPacketHandler`, `SpawnResponsePacketHandler`) silently
    /// discard `PlayerAuthInput` ("spammed after StartGame even though the
    /// player has no controls"), so movement and chunk streaming here only
    /// apply in InGame. Chunk streaming itself belongs to
    /// [`Self::do_chunk_requests`] (`Player::doChunkRequests`), not to
    /// each input packet.
    pub fn handle_player_auth_input(
        &mut self,
        packet: PlayerAuthInputPacket,
    ) -> Result<Vec<OutgoingPacket>, SessionError> {
        if self.state != SessionState::InGame {
            return Ok(Vec::new());
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
                return Ok(Vec::new());
            }
        }
        let raw_eye = packet.position;
        let raw_yaw = packet.yaw;
        let raw_pitch = packet.pitch;
        // Canonical Bedrock storage, identical to `teleport`: yaw folds to
        // `[0, 360)` and pitch to `(-180, 180]` via `java_sync`. The old
        // manual wrap stored pitch in `[0, 360)`, so after any teleport a
        // looking-up pitch of -10 was stored as 350 and every subsequent
        // input looked like a 360-degree movement (and broadcast wrong).
        // `mut`: a plugin may rewrite the `PlayerMoveEvent` destination
        // (`setTo`), which then replaces these before they are applied.
        let mut yaw = crate::java_sync::normalize_bedrock_yaw(raw_yaw);
        let mut pitch = crate::java_sync::bedrock_pitch_to_java(raw_pitch);
        let mut new_feet = Vector3f::new(
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
                return Ok(Vec::new());
            }
        }

        let old_cx = (self.position.x.floor() as i32) >> 4;
        let old_cz = (self.position.z.floor() as i32) >> 4;

        // PM `Player::processMostRecentMovements`: `from` is the last
        // location (feet + yaw/pitch + world), `to` the new one. The
        // PlayerAuthInput position is the eye; `new_feet` already removed
        // `EYE_HEIGHT` above, so both endpoints are feet-based `Location`s
        // like `Player::getLocation()` returns.
        let world_name = self.world.name.clone();
        let from = MoveLocation::new(
            self.position.x as f64,
            self.position.y as f64,
            self.position.z as f64,
            self.yaw as f64,
            self.pitch as f64,
            world_name.clone(),
        );
        let to = MoveLocation::new(
            new_feet.x as f64,
            new_feet.y as f64,
            new_feet.z as f64,
            yaw as f64,
            pitch as f64,
            world_name,
        );
        let has_moved_for_move = (from.x - to.x).abs() > 0.0001
            || (from.y - to.y).abs() > 0.0001
            || (from.z - to.z).abs() > 0.0001
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
                // Upstream the player is already standing at `to` when the
                // event fires, so the snapshot carries the new position.
                let mut context = self.player_context();
                context.apply_position(to.x, to.y, to.z, to.yaw, to.pitch);
                let mut event = PlayerMoveEvent::new(context, from.clone(), to.clone());
                self.fire_event(&mut event);
                if event.cancelled {
                    return Ok(Vec::new());
                }
                // PM: `if ($to->distanceSquared($ev->getTo()) > 0.01)` the
                // plugin rewrote the destination (`setTo`), so teleport there.
                // World changes via `setTo` are ignored: this server has a
                // single world, so only position/rotation are adopted.
                if to.distance_squared(&event.to) > 0.01 {
                    new_feet = Vector3f::new(
                        event.to.x as f32,
                        event.to.y as f32,
                        event.to.z as f32,
                    );
                    yaw = event.to.yaw as f32;
                    pitch = event.to.pitch as f32;
                }
            }
        }

        // Rotation is always accepted; position follows PM's
        // if (!$this->forceMoveSync && $hasMoved).
        let position_changed = !self.force_move_sync && has_moved_for_move;
        self.yaw = yaw;
        self.pitch = pitch;
        // Head rotation is independent of the body rotation (Bedrock
        // `PlayerAuthInputPacket.headYaw`): Java viewers render it with
        // `EntityHeadRotationPacket`, so it must survive the registry
        // round-trip instead of being dropped here.
        self.head_yaw = crate::java_sync::normalize_bedrock_yaw(packet.head_yaw);
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

        // Chunk streaming itself lives in `do_chunk_requests()`
        // (`Player::doChunkRequests`); movement only schedules the next
        // `orderChunks()` run when the player crosses a chunk boundary.
        let curr_cx = (self.position.x.floor() as i32) >> 4;
        let curr_cz = (self.position.z.floor() as i32) >> 4;
        let moved = curr_cx != old_cx || curr_cz != old_cz;
        if moved {
            self.next_chunk_order_run = 0;
        }

        let out: Vec<OutgoingPacket> = Vec::new();

        // Tail-shape diagnostic: the input_data tail (item payloads +
        // block actions) only appears when it rides a real interaction, so
        // log its length only when it changes — never per packet.
        if packet.input_data.len() != self.last_input_len {
            debug!(
                "auth_input tail len={} (was {})",
                packet.input_data.len(),
                self.last_input_len
            );
            self.last_input_len = packet.input_data.len();
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
    /// Decodes the tail with [`parse_block_actions_result`], which also
    /// returns the embedded `ItemInteractionData` when the client put the
    /// item payloads in this packet. Reference order
    /// (`target/axolotl-pm` `InGamePacketHandler::handlePlayerAuthInput`):
    /// `handleUseItemTransaction` (the embedded interaction) runs FIRST,
    /// then `itemStackRequest` (skipped), then `blockActions` LAST — so the
    /// embedded `UseItem` CLICK_BLOCK is applied through
    /// [`Self::handle_use_item_place`] before the actions below run. The
    /// standalone `InventoryTransactionPacket` path still works for clients
    /// that keep sending the legacy packet.
    ///
    /// Break flow: `handlePlayerActionFromData(PREDICT_DESTROY_BLOCK)` calls
    /// `Player::breakBlock`, which runs `World::useBreakOn` ->
    /// `World::setBlockAt(AIR)`. `setBlockAt` only records the edit in
    /// `World::$changedBlocks`; the broadcast happens on the **next**
    /// `World::actuallyDoTick`: for each chunk with changes,
    /// `createBlockUpdatePackets` builds one `UpdateBlockPacket`
    /// (`FLAG_NETWORK`, `DATA_LAYER_NORMAL`, plus a `BlockActorDataPacket`
    /// for container tiles) and `broadcastPacketToPlayersUsingChunk`
    /// delivers it to exactly the players with that chunk in view
    /// (at most once per viewer per tick, buffered by chunk). Above 512
    /// changed blocks in one chunk PM instead resends the whole `LevelChunk`
    /// via `Player::onChunkChanged`; this port always sends per-block
    /// `UpdateBlockPacket`s, which converges to the same state.
    ///
    /// Rust equivalent here:
    /// - `START_BREAK` fires a cancellable `PlayerInteractEvent`
    ///   (left click). Nothing is applied, so cancellation needs no undo.
    ///   In PM `START_BREAK` also drives `attackBlock` /
    ///   `SurvivalBlockBreakHandler`; the authoritative break still arrives
    ///   as `PREDICT_DESTROY_BLOCK`, which is what this port acts on.
    /// - `PREDICT_DESTROY_BLOCK` fires a cancellable `BlockBreakEvent` with
    ///   the current block id. When not cancelled the break is applied
    ///   authoritatively (air edit queued as a manager-level `SetBlock` so
    ///   every viewer gets the change); when cancelled the chunk is
    ///   resent to this session to undo the client's prediction. PM
    ///   instead `syncBlocksNearby`s the 6 neighbours to the breaker only;
    ///   a whole-chunk resend to the breaker is the same undo.
    ///   That `SetBlock` goes through `apply_actions` -> `world.set_block`,
    ///   which is what enqueues the edit for Fase 1 (Java
    ///   `world.take_block_changes`) and Fase 2 (`poll_block_changes`
    ///   broadcasts the `UpdateBlockPacket` to chunk viewers, matching
    ///   PM's next-tick delivery except the dispatch path polls in the
    ///   same dispatch instead of waiting one tick).
    /// - `BlockPlaceEvent` is NOT fired from block actions: placement
    ///   travels in `InventoryTransactionPacket` (`UseItem` CLICK_BLOCK) and
    ///   is applied by [`Self::handle_use_item_place`], not by block
    ///   actions. PM also accepts the same `UseItemTransactionData`
    ///   embedded in `PlayerAuthInputPacket::itemInteractionData`
    ///   (`handleUseItemTransaction` runs BEFORE the block actions); this
    ///   port now decodes that embedded form (`parse_block_actions_result`
    ///   returns the [`ItemInteractionData`] too) and reuses the same
    ///   placement path, including the cancellable `BlockPlaceEvent`.
    /// - `CREATIVE_PLAYER_DESTROY_BLOCK` is intentionally ignored here too:
    ///   PM notes the client sends `PREDICT_DESTROY_BLOCK` anyway.
    fn handle_block_actions(&mut self, input_data: &[u8]) {
        let (actions, interaction) = parse_block_actions_result(input_data);

        // Reference order: the embedded item interaction (the same
        // `UseItemTransactionData` the legacy packet carries) is handled
        // BEFORE the block actions. Placement reuses the exact
        // standalone-packet path, including the cancellable BlockPlaceEvent.
        if let Some(interaction) = interaction {
            self.handle_use_item_place(&interaction.transaction);
        }

        if actions.is_empty() {
            // A client that flags PERFORM_BLOCK_ACTIONS but decodes to zero
            // actions used to be invisible (old code bailed silently and all
            // embedded payloads were dropped before the fix). Report it at
            // most once per 2 seconds, with the tail length and the first
            // bytes so a wire-layout regression is easy to spot in the log.
            if has_input_flag(
                parse_input_flags(input_data),
                INPUT_FLAG_PERFORM_BLOCK_ACTIONS,
            ) {
                let now = std::time::Instant::now();
                let due = self
                    .last_block_actions_diag
                    .map_or(true, |last| now.duration_since(last).as_secs_f32() >= 2.0);
                if due {
                    self.last_block_actions_diag = Some(now);
                    let preview_len = input_data.len().min(24);
                    let preview: Vec<String> = input_data[..preview_len]
                        .iter()
                        .map(|byte| format!("{:02x}", byte))
                        .collect();
                    debug!(
                        "block actions flagged but none decoded: input_data len={} hex[0..{}]={}",
                        input_data.len(),
                        preview_len,
                        preview.join(" ")
                    );
                }
            }
            return;
        }

        let types: Vec<String> = actions
            .iter()
            .map(|action| action.action_type.to_string())
            .collect();
        debug!("block_actions types={}", types.join(","));

        self.apply_block_actions(actions);
    }

    /// PM `InGamePacketHandler::handlePlayerActionFromData` dispatch for the
    /// validated block actions of one `PlayerAuthInputPacket`. Split from
    /// [`Self::handle_block_actions`] so tests can drive the exact PM state
    /// machine without rebuilding the wire tail.
    fn apply_block_actions(&mut self, actions: Vec<PlayerBlockAction>) {
        for action in actions {
            match action.action_type {
                BLOCK_ACTION_START_BREAK | BLOCK_ACTION_CONTINUE_DESTROY => {
                    let Some((x, y, z)) = action.position else {
                        continue;
                    };
                    // PM `InGamePacketHandler`: the client sends
                    // CONTINUE_DESTROY_BLOCK for the block it is already
                    // destroying right before `PREDICT_DESTROY_BLOCK`; it
                    // must not count as a new left-click
                    // (`$lastBlockAttacked` dedup).
                    if self.last_block_attacked == Some((x, y, z)) {
                        continue;
                    }
                    // PM `Player::attackBlock`: cancellable LEFT_CLICK event.
                    let block_id = self.world.block_at(x, y, z);
                    let mut event = PlayerInteractEvent::new(
                        self.player_context(),
                        self.world.name.clone(),
                        x,
                        y,
                        z,
                        PlayerInteractEvent::ACTION_LEFT_CLICK,
                    );
                    self.fire_event(&mut event);
                    self.last_block_attacked = Some((x, y, z));
                    if event.cancelled {
                        // PM: `attackBlock` returning false triggers
                        // `syncBlocksNearby` (authoritative resend).
                        let chunk = (x >> 4, z >> 4);
                        if self.sent_chunks.remove(&chunk) {
                            self.pending_outgoing.push(OutgoingPacket::LevelChunk(
                                self.create_chunk_packet(chunk.0, chunk.1),
                            ));
                        }
                        continue;
                    }
                    // PM `attackBlock` returns before creating the handler
                    // in creative because the client forecasts the break and
                    // sends `PREDICT_DESTROY_BLOCK` itself. A client that
                    // *digs* (START/CRACK) needs server-driven progress even
                    // when the session is flagged creative (its own gamemode
                    // view can lag behind `syncAbilities`), so creative digs
                    // are served instantly (instabuild semantics: one tick)
                    // instead of being dropped.
                    let creative = self.gamemode == 1;
                    if let Some(secs) = block_break_time_secs(block_id) {
                        // `calculateBreakProgressPerTick`: seconds * 20
                        // = ticks, progress = 1 / ticks. The airborne x5
                        // penalty is not modeled (no ground tracking yet).
                        let ticks = secs * 20.0;
                        let break_speed = if creative {
                            1.0
                        } else if ticks > 0.0 {
                            1.0 / ticks
                        } else {
                            1.0
                        };
                        self.break_handler = Some(BreakHandlerState {
                            pos: (x, y, z),
                            targeted_face: action.face.unwrap_or(0),
                            break_speed,
                            break_progress: 0.0,
                            fx_ticker: 0,
                            max_player_distance: 16,
                        });
                        // PM handler constructor: broadcast
                        // `BLOCK_START_BREAK` with `65535 * breakSpeed` to
                        // the viewers of the block (the digger included) —
                        // this drives the client's crack rendering in
                        // server-auth breaking; the client sends
                        // `PREDICT_DESTROY_BLOCK` when its crack completes.
                        self.pending_actions.push(PluginAction::BroadcastLevelEvent {
                            event_id: LEVEL_EVENT_BLOCK_START_BREAK,
                            event_data: (65535.0 * break_speed) as i32,
                            x,
                            y,
                            z,
                        });
                        debug!(
                            "break handler started: ({},{},{}) speed/tick={:.4}",
                            x, y, z, break_speed
                        );
                    }
                }
                BLOCK_ACTION_CRACK_BREAK => {
                    let Some((x, y, z)) = action.position else {
                        continue;
                    };
                    // PM `continueBreakBlock`: refresh the targeted face on
                    // the current target only.
                    if let Some(handler) = self.break_handler.as_mut() {
                        if handler.pos == (x, y, z) {
                            if let Some(face) = action.face {
                                handler.targeted_face = face;
                            }
                        }
                    }
                    self.last_block_attacked = Some((x, y, z));
                }
                BLOCK_ACTION_STOP_BREAK | BLOCK_ACTION_ABORT_BREAK => {
                    let Some((x, y, z)) = action.position else {
                        continue;
                    };
                    // PM `stopBreakBlock`: drop the handler for the current
                    // target (`__destruct` broadcasts BLOCK_STOP_BREAK).
                    if let Some(handler) = self
                        .break_handler
                        .take_if(|handler| handler.pos == (x, y, z))
                    {
                        self.pending_actions.push(PluginAction::BroadcastLevelEvent {
                            event_id: LEVEL_EVENT_BLOCK_STOP_BREAK,
                            event_data: 0,
                            x: handler.pos.0,
                            y: handler.pos.1,
                            z: handler.pos.2,
                        });
                    }
                    if self.last_block_attacked == Some((x, y, z)) {
                        self.last_block_attacked = None;
                    }
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
                        self.pending_actions.push(PluginAction::BroadcastLevelEvent {
                            event_id: LEVEL_EVENT_PARTICLE_DESTROY_BLOCK,
                            event_data: block_id as i32,
                            x,
                            y,
                            z,
                        });
                    }
                    // PM `breakBlock` starts by calling `stopBreakBlock($pos)`.
                    if let Some(handler) = self
                        .break_handler
                        .take_if(|handler| handler.pos == (x, y, z))
                    {
                        self.pending_actions.push(PluginAction::BroadcastLevelEvent {
                            event_id: LEVEL_EVENT_BLOCK_STOP_BREAK,
                            event_data: 0,
                            x: handler.pos.0,
                            y: handler.pos.1,
                            z: handler.pos.2,
                        });
                    }
                    self.last_block_attacked = None;
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
        // PM `World::useItemOn()`: `$blockClicked->onInteract()` runs *before*
        // item placement and consumes the click. Without this a right click on
        // a button placed the held block next to it instead of pressing it.
        if self.handle_block_interact(use_item) {
            return;
        }
        let block_id = if use_item.item_in_hand.block_runtime_id != 0 {
            use_item.item_in_hand.block_runtime_id
        } else {
            match Self::resolve_block_id_from_item(use_item.item_in_hand.network_id) {
                Some(id) => id,
                None => return,
            }
        };
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
        // PM `World::useItemOn` refuses the placement when the placed block's
        // collision boxes intersect an entity (`getCollidingEntities`): without
        // this a player could place a block inside themselves and get stuck.
        if self.placement_blocked_by_entity(x, y, z, block_id) {
            self.revert_predicted_placement(x, z);
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
                self.revert_predicted_placement(x, z);
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
        // PM `World::addSound(new BlockPlaceSound($placed))`: the client only
        // plays the block's place sound when the server broadcasts
        // `LevelSoundEventPacket(PLACE, blockId)`. Without it the block appeared
        // with no sound at all.
        self.pending_actions.push(PluginAction::BroadcastSound {
            sound: LEVEL_SOUND_PLACE.to_string(),
            extra_data: block_id as i32,
            x,
            y,
            z,
        });

        if self.gamemode == 0 {
            let slot = self.inventory.selected_hotbar_slot() as u32;
            if let Some(item) = self.inventory.window_slot(WINDOW_ID_INVENTORY, slot).cloned() {
                if !item.is_empty() {
                    let mut new_item = item;
                    new_item.count = new_item.count.saturating_sub(1);
                    if new_item.count == 0 {
                        new_item = ItemStack::empty();
                    }
                    self.inventory.set_window_slot(WINDOW_ID_INVENTORY, slot, new_item);
                }
            }
        }
    }

    /// PM `Block::onInteract()` for the blocks the reference gives one: buttons
    /// (`Button::onInteract`: press, then a scheduled update releases it),
    /// levers (`Lever::onInteract`: toggle) and fence gates
    /// (`FenceGate::onInteract`: open/close).
    ///
    /// Returns true when the click was consumed — either because the block
    /// changed, because it was already in the requested state, or because the
    /// client's prediction was refused. Sneaking only skips the interaction
    /// when the hand holds an item (`PlayerInteractEvent::setUseBlock($item->isNull())`
    /// in `World::useItemOn`), so a gate can still be opened empty-handed.
    fn handle_block_interact(&mut self, use_item: &UseItemData) -> bool {
        let (x, y, z) = use_item.block_position;
        let current = self.world.block_at(x, y, z);
        if bedrock_world::block_states::interact(current).is_none() {
            return false;
        }
        if self.is_sneaking && use_item.item_in_hand.network_id != 0 {
            return false;
        }

        let has_interact_listeners = self
            .plugins
            .as_ref()
            .map(|plugins| plugins.has_listeners("PlayerInteractEvent"))
            .unwrap_or(false);
        if has_interact_listeners {
            let mut event = PlayerInteractEvent::new(
                self.player_context(),
                self.world.name.clone(),
                x,
                y,
                z,
                PlayerInteractEvent::ACTION_RIGHT_CLICK,
            );
            self.fire_event(&mut event);
            if event.cancelled {
                // PM: a cancelled `PlayerInteractEvent` makes `useItemOn`
                // return false without placing anything.
                return false;
            }
        }

        // PM `onInteract()` edits the world inline (`World::setBlock`), which
        // also schedules a pressed button's release.
        let Some(interaction) = self.world.interact_block(x, y, z) else {
            return false;
        };
        if interaction.new_state == current {
            // Already pressed/activated: PM's `Button::onInteract` still
            // returns true, so the click never reaches the item.
            return true;
        }

        if interaction.door_sound {
            // PM `FenceGate::onInteract` -> `DoorSound` ->
            // `LevelEventPacket(SOUND_DOOR)` at the block itself.
            self.pending_actions.push(PluginAction::BroadcastLevelEvent {
                event_id: LEVEL_EVENT_SOUND_DOOR,
                event_data: 0,
                x,
                y,
                z,
            });
        } else {
            let sound = if interaction.powered {
                LEVEL_SOUND_POWER_ON
            } else {
                LEVEL_SOUND_POWER_OFF
            };
            self.pending_actions.push(PluginAction::BroadcastSound {
                sound: sound.to_string(),
                extra_data: -1,
                x,
                y,
                z,
            });
        }
        true
    }

    /// Undoes the block this client has already predicted (a refused placement):
    /// resending the column makes the ghost block disappear, exactly like PM
    /// (which never lets the client keep a placement the server rejected).
    fn revert_predicted_placement(&mut self, x: i32, z: i32) {
        let chunk = (x >> 4, z >> 4);
        if self.sent_chunks.remove(&chunk) {
            self.pending_outgoing.push(OutgoingPacket::LevelChunk(
                self.create_chunk_packet(chunk.0, chunk.1),
            ));
        }
    }

    /// PM `World::useItemOn`: the placement is refused when an entity's
    /// bounding box intersects the block being placed, i.e.
    /// `count($world->getCollidingEntities($collisionBox)) > 0`.
    ///
    /// The canonical state table carries no collision geometry, so passable
    /// blocks are recognised by name and everything else is treated as a solid
    /// cell (conservative: it refuses rather than letting a player stand inside
    /// a newly placed block).
    fn placement_blocked_by_entity(&self, x: i32, y: i32, z: i32, block_id: u32) -> bool {
        const HALF_WIDTH: f64 = 0.3; // player hitbox is 0.6 x 1.8
        const HEIGHT: f64 = 1.8;

        let name = bedrock_world::block_states::state_name(block_id).unwrap_or("air");
        if !bedrock_world::block_states::has_collision(name) {
            return false;
        }

        let (cell_min_x, cell_max_x) = (x as f64, x as f64 + 1.0);
        let (cell_min_y, cell_max_y) = (y as f64, y as f64 + 1.0);
        let (cell_min_z, cell_max_z) = (z as f64, z as f64 + 1.0);
        let intersects = |px: f64, py: f64, pz: f64| {
            let (min_x, max_x) = (px - HALF_WIDTH, px + HALF_WIDTH);
            let (min_y, max_y) = (py, py + HEIGHT);
            let (min_z, max_z) = (pz - HALF_WIDTH, pz + HALF_WIDTH);
            min_x < cell_max_x
                && max_x > cell_min_x
                && min_y < cell_max_y
                && max_y > cell_min_y
                && min_z < cell_max_z
                && max_z > cell_min_z
        };

        if intersects(
            self.position.x as f64,
            self.position.y as f64,
            self.position.z as f64,
        ) {
            return true;
        }
        self.registry.as_ref().is_some_and(|registry| {
            registry
                .all()
                .iter()
                .any(|player| intersects(player.x, player.y, player.z))
        })
    }

/// Resolves a Bedrock block runtime id from a numeric item network id.
pub fn resolve_block_id_from_item(network_id: i16) -> Option<u32> {
    use bedrock_world::block_id::*;
    match network_id {
        1 => Some(STONE),
        2 => Some(GRASS_BLOCK),
        3 => Some(DIRT),
        4 => Some(STONE), // Cobblestone fallback
        7 => Some(BEDROCK),
        12 => Some(SAND),
        13 => Some(GRAVEL),
        14 => Some(GOLD_ORE),
        15 => Some(IRON_ORE),
        16 => Some(COAL_ORE),
        17 => Some(OAK_LOG),
        18 => Some(OAK_LEAVES),
        24 => Some(SANDSTONE),
        56 => Some(DIAMOND_ORE),
        73 => Some(REDSTONE_ORE),
        129 => Some(EMERALD_ORE),
        id if id > 0 => Some(STONE),
        _ => None,
    }
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

    // -----------------------------------------------------------------
    // Crafting / containers / ItemStackRequest
    // -----------------------------------------------------------------

    /// Opens the crafting table at `(x, y, z)` and sends
    /// `ContainerOpenPacket` (0x2E, window 10, workbench container type).
    pub fn open_crafting_table(&mut self, position: (i32, i32, i32)) -> Vec<OutgoingPacket> {
        let container_type = crate::packets_wire::CONTAINER_TYPE_WORKBENCH;
        self.open_container = Some(OpenContainer {
            window_id: 10,
            container_type,
            position,
            kind: "crafting_table".to_string(),
        });
        vec![OutgoingPacket::RawWire(crate::packets_wire::container_open_wire(
            10,
            container_type,
            position,
            crate::packets_wire::BLOCK_CONTAINER_ACTOR_ID,
        ))]
    }

    /// Opens the anvil at `(x, y, z)`: window 16, anvil container type.
    pub fn open_anvil(&mut self, position: (i32, i32, i32)) -> Vec<OutgoingPacket> {
        let container_type = crate::packets_wire::CONTAINER_TYPE_ANVIL;
        self.open_container = Some(OpenContainer {
            window_id: 16,
            container_type,
            position,
            kind: "anvil".to_string(),
        });
        vec![OutgoingPacket::RawWire(crate::packets_wire::container_open_wire(
            16,
            container_type,
            position,
            crate::packets_wire::BLOCK_CONTAINER_ACTOR_ID,
        ))]
    }

    /// Handles `ContainerClosePacket` (0x2F): returns crafting/anvil inputs
    /// to the main inventory and resyncs it.
    pub fn handle_container_close(
        &mut self,
        packet: bedrock_protocol::packets::container_close::ContainerClosePacket,
    ) -> Vec<OutgoingPacket> {
        let Some(container) = self.open_container.take() else {
            // The player inventory screen (window 0) is opened directly by
            // `handle_interact`; the client still expects its close to be
            // acknowledged, or it refuses to open another window later.
            if packet.window_id as u32 == WINDOW_ID_INVENTORY {
                return vec![OutgoingPacket::RawWire(
                    crate::packets_wire::container_close_wire(
                        packet.window_id,
                        packet.container_type,
                        false,
                    ),
                )];
            }
            return Vec::new();
        };
        let drops = match container.kind.as_str() {
            "crafting_table" => self.crafting_table.close_drops(),
            "anvil" => self.anvil_inventory.close_drops(),
            _ => Vec::new(),
        };
        for drop in drops {
            let leftover = self.inventory.add_item(&drop);
            debug!(
                "Session {}: {} items of the closed {} container did not fit",
                self.session_id, leftover, container.kind
            );
        }
        let mut out = self.resync_inventory();
        out.push(OutgoingPacket::RawWire(
            crate::packets_wire::container_close_wire(
                container.window_id,
                container.container_type,
                false,
            ),
        ));
        out
    }

    /// Handles `InteractPacket` (0x21): `ACTION_OPEN_INVENTORY` (action 6)
    /// opens the player inventory screen; other actions are ignored.
    ///
    /// Since 1.16 the client waits for `ContainerOpenPacket` before showing
    /// *any* container screen, its own inventory included (Mojang
    /// "Server Auth Inventory / Item Stack packets disabled" guide), and BDS
    /// does not re-send `InventoryContentPacket` when that screen opens. So
    /// the open packet is what was missing: without it the inventory button
    /// did nothing, in creative as much as in survival.
    pub fn handle_interact(
        &mut self,
        packet: bedrock_protocol::packets::interact::InteractPacket,
    ) -> Vec<OutgoingPacket> {
        if packet.action != bedrock_protocol::packets::interact::ACTION_OPEN_INVENTORY {
            return Vec::new();
        }
        let mut out = vec![OutgoingPacket::RawWire(crate::packets_wire::container_open_wire(
            WINDOW_ID_INVENTORY as u8,
            crate::packets_wire::CONTAINER_TYPE_INVENTORY,
            (0, 0, 0),
            // `ContainerOpenPacket::entityInv(..., $player->getId())`: the
            // screen belongs to the player entity itself.
            self.entity_runtime_id as i64,
        ))];
        // The server owns the contents, so re-send them for window 0 (the
        // id the open packet uses); harmless when the client already has
        // them and needed after creative catalog clicks.
        out.extend(self.resync_inventory());
        out
    }

    /// Handles `ItemStackRequestPacket` (0x93), answers with
    /// `ItemStackResponsePacket` (0x94) plus inventory slot updates, one
    /// response per request (the packet may batch several).
    pub fn handle_item_stack_request(
        &mut self,
        packet: bedrock_protocol::packets::item_stack_request::ItemStackRequestPacket,
    ) -> Vec<OutgoingPacket> {
        let is_creative = self.gamemode == 1;
        let mut out = Vec::new();
        for request in &packet.requests {
            let outcome = crate::crafting::apply_item_stack_request(
                &mut self.inventory,
                crate::crafting::manager(),
                request,
                is_creative,
                crate::crafting::creative_catalog(),
            );
            out.push(OutgoingPacket::RawWire(crate::packets_wire::item_stack_response_wire(
                &outcome.response,
            )));
            // Tell the client the final state of every slot we touched (the
            // request names slots with container UI ids, not window ids).
            for (slot, item) in &outcome.changed {
                let Some((window, index)) =
                    crate::crafting::ui_container_window(slot.container, slot.slot)
                else {
                    continue;
                };
                out.push(OutgoingPacket::RawWire(inventory_slot_wire(window, index, item)));
            }
            // Dropped items (Q key) become entities for plugin visibility; the
            // packet contract only requires the inventory delta here.
            let _ = outcome.dropped;
        }
        // Held-item changes must reach viewers.
        out.push(OutgoingPacket::RawWire(self.mob_equipment_wire()));
        out
    }

    /// Answers an undecodable `ItemStackRequestPacket` (0x93) with an error
    /// result for every request id it could read, so the client drops its
    /// prediction instead of being left waiting for a response.
    pub fn item_stack_error_response(&self, payload: &[u8]) -> Vec<OutgoingPacket> {
        let mut reader = Reader::new(payload);
        let Ok(count) = reader.read_var_u32() else {
            return Vec::new();
        };
        let mut responses = Vec::new();
        for _ in 0..count.min(64) {
            let Ok(request_id) = reader.read_var_i32() else {
                break;
            };
            responses.push(crate::crafting::ItemStackResponseEntry::error(request_id));
        }
        if responses.is_empty() {
            return Vec::new();
        }
        let mut writer = Writer::new();
        writer.write_var_u32(bedrock_protocol::packets::item_stack_response::ItemStackResponsePacket::PACKET_ID);
        bedrock_protocol::packets::item_stack_response::ItemStackResponsePacket::new(responses)
            .encode_payload(&mut writer);
        vec![OutgoingPacket::RawWire(writer.into_bytes().to_vec())]
    }

    /// `MobEquipmentPacket` (0x1F) for the held item in the main hand.
    pub fn mob_equipment_wire(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_var_u32(bedrock_protocol::packets::mob_equipment::MobEquipmentPacket::PACKET_ID);
        bedrock_protocol::packets::mob_equipment::MobEquipmentPacket::new(
            self.entity_runtime_id,
            self.inventory.held_item(),
            self.inventory.selected_hotbar_slot() as i32,
            bedrock_protocol::packets::mob_equipment::HAND_MAIN,
            0,
        )
        .encode_payload(&mut writer);
        writer.into_bytes().to_vec()
    }

    /// `CraftingDataPacket` (0x34) wire bytes.
    ///
    /// Default is the *generated* wire (`921` classic recipes from
    /// [`crate::crafting::manager`]), now translated from the real 2225
    /// wire (`target/BedrockProtocol/src/CraftingDataPacket.php`):
    /// 8 lists without type-tag, messy descriptors, `ItemStackWithoutStackId`.
    /// The BDS capture (`PM_CORE_CRAFTING_MODE=capture`) is the **old**
    /// interleaved format (type varu per recipe, 3660 unified) and is kept
    /// only for reference — it still causes `invalid string` on 1.26.60.
    pub fn crafting_data_wire() -> Vec<u8> {
        if std::env::var("PM_CORE_CRAFTING_MODE").as_deref() == Ok("capture") {
            return StaticPackets::CRAFTING_DATA.to_vec();
        }
        Self::generated_crafting_data_wire()
    }

    /// Generated `CraftingDataPacket` (0x34) from the loaded recipe manager
    /// (~921 classic recipes). Currently NOT sent to clients by default —
    /// see [`PlayerSession::crafting_data_wire`].
    pub fn generated_crafting_data_wire() -> Vec<u8> {
        use bedrock_protocol::packets::crafting_data as cd;
        use crate::crafting::RecipeIngredient;

        // NOTE (crafting wire fix, protocol 2225): descriptors are item
        // string ids now (`StringIdMetaItemDescriptor`), resolved from the
        // numeric ids via `CraftingManager::item_names` (recorded while the
        // recipe JSON is loaded). Tag ingredients use `item_tag` with the
        // wildcard meta, empty grid cells use the empty descriptor — all
        // as `TypeConverter::coreRecipeIngredientToNet` does. If a name is
        // ever missing, the cell degrades to an empty descriptor instead
        // of emitting a wrong id.
        fn descriptor(
            mgr: &crate::crafting::CraftingManager,
            ingredient: &RecipeIngredient,
        ) -> cd::RecipeDescriptor {
            let name_of = |id: i16| mgr.item_names.get(&id);
            match ingredient {
                RecipeIngredient::Exact { network_id, meta } => {
                    match name_of(*network_id) {
                        Some(id) => cd::RecipeDescriptor::StringIdMeta {
                            id: id.clone(),
                            meta: *meta as i32,
                            count: 1,
                        },
                        None => cd::RecipeDescriptor::invalid(),
                    }
                }
                RecipeIngredient::MetaWildcard { network_id } => {
                    match name_of(*network_id) {
                        Some(id) => cd::RecipeDescriptor::StringIdMeta {
                            id: id.clone(),
                            meta: cd::WILDCARD_META,
                            count: 1,
                        },
                        None => cd::RecipeDescriptor::invalid(),
                    }
                }
                RecipeIngredient::TagWildcard { tag } => cd::RecipeDescriptor::ItemTag {
                    tag: tag.clone(),
                    meta: cd::WILDCARD_META,
                    count: 1,
                },
            }
        }

        fn output(item: &bedrock_protocol::ItemStack) -> cd::CraftingItemStack {
            cd::CraftingItemStack::from_item_stack(item)
        }

        fn uuid_of(recipe_id: &str) -> [u8; 16] {
            // FNV-1a (64-bit) twice with different seeds, so equal ids
            // always map to the same UUID without external state.
            fn hash(seed: u64, bytes: &[u8]) -> u64 {
                let mut h = seed;
                for &b in bytes {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x100000001b3);
                }
                h
            }
            let mut uuid = [0u8; 16];
            uuid[0..8].copy_from_slice(&hash(0xcbf29ce484222325, recipe_id.as_bytes()).to_le_bytes());
            uuid[8..16].copy_from_slice(&hash(0x84222325cbf29ce4, recipe_id.as_bytes()).to_le_bytes());
            uuid
        }

        /// Potion id/meta for one ingredient; tag ingredients cannot be
        /// represented in the brewing tables and skip the recipe.
        fn ingredient_id_meta(
            ingredient: &RecipeIngredient,
        ) -> Option<(i32, i32)> {
            match ingredient {
                RecipeIngredient::Exact { network_id, meta } => {
                    Some((*network_id as i32, *meta as i32))
                }
                RecipeIngredient::MetaWildcard { network_id } => {
                    Some((*network_id as i32, 32767))
                }
                RecipeIngredient::TagWildcard { .. } => None,
            }
        }

        const RECIPE_ID_OFFSET: u32 = 1;

        let mgr = crate::crafting::manager();
        let mut recipes = Vec::new();
        let mut next_id = RECIPE_ID_OFFSET;
        for recipe in &mgr.shaped {
            let height = recipe.shape.len() as u32;
            let width = recipe
                .shape
                .first()
                .map(|row| row.len() as u32)
                .unwrap_or(0);
            let inputs = recipe
                .shape
                .iter()
                .flatten()
                .map(|cell| match cell {
                    Some(ingredient) => descriptor(mgr, ingredient),
                    None => cd::RecipeDescriptor::invalid(),
                })
                .collect();
            recipes.push(cd::CraftingRecipe::Shaped(cd::ShapedRecipe {
                recipe_id: recipe.id.clone(),
                width,
                height,
                inputs,
                outputs: vec![output(&recipe.result)],
                uuid: uuid_of(&recipe.id),
                block: "crafting_table".to_string(),
                // Axolotl's CraftingDataCache sends the constant 50 here.
                priority: 50,
                assume_symmetry: true,
                unlocking: Some(cd::UnlockingRequirement::always_unlocked()),
                network_id: next_id,
            }));
            next_id += 1;
        }
        for recipe in &mgr.shapeless {
            recipes.push(cd::CraftingRecipe::Shapeless(cd::ShapelessRecipe {
                recipe_id: recipe.id.clone(),
                inputs: recipe
                    .ingredients
                    .iter()
                    .map(|i| descriptor(mgr, i))
                    .collect(),
                outputs: vec![output(&recipe.result)],
                uuid: uuid_of(&recipe.id),
                block: "crafting_table".to_string(),
                // Axolotl's CraftingDataCache sends the constant 50 here.
                priority: 50,
                unlocking: Some(cd::UnlockingRequirement::always_unlocked()),
                network_id: next_id,
            }));
            next_id += 1;
        }
        for recipe in &mgr.furnace {
            recipes.push(cd::CraftingRecipe::Shapeless(cd::ShapelessRecipe {
                recipe_id: recipe.id.clone(),
                inputs: vec![descriptor(mgr, &recipe.input)],
                outputs: vec![output(&recipe.output)],
                uuid: uuid_of(&recipe.id),
                block: recipe.block.clone(),
                // Axolotl's CraftingDataCache sends the constant 50 here.
                priority: 50,
                unlocking: Some(cd::UnlockingRequirement::always_unlocked()),
                network_id: next_id,
            }));
            next_id += 1;
        }

        let potion_types = mgr
            .brewing
            .iter()
            .filter_map(|recipe| {
                let (input_id, input_meta) = ingredient_id_meta(&recipe.input)?;
                let (ingredient_id, ingredient_meta) =
                    ingredient_id_meta(&recipe.ingredient)?;
                Some(cd::PotionTypeRecipe {
                    input_id,
                    input_meta,
                    ingredient_id,
                    ingredient_meta,
                    output_id: recipe.output.network_id as i32,
                    output_meta: recipe.output.meta as i32,
                })
            })
            .collect();
        let potion_containers = mgr
            .potion_container
            .iter()
            .filter_map(|recipe| {
                let (input_id, _) = ingredient_id_meta(&recipe.input)?;
                let (ingredient_id, _) = ingredient_id_meta(&recipe.ingredient)?;
                Some(cd::PotionContainerRecipe {
                    input_id,
                    ingredient_id,
                    output_id: recipe.output.network_id as i32,
                })
            })
            .collect();

        let mut writer = Writer::new();
        writer.write_var_u32(cd::CraftingDataPacket::PACKET_ID);
        cd::CraftingDataPacket::new(recipes, potion_types, potion_containers)
            .encode_payload(&mut writer);
        writer.into_bytes().to_vec()
    }

    /// `CreativeContentPacket` (0x91) wire bytes.
    ///
    /// Default is the BDS capture (`StaticPackets::CREATIVE_CONTENT`),
    /// byte-identical to what BDS 1.26.60 sends, so real clients parse it
    /// with certainty. The generated wire (`PM_CORE_CREATIVE_MODE=generated`,
    /// from `data/creative/*.json`) currently fails on real clients with
    /// `Invalid enum value` because `block_states` are ignored and every
    /// stack is sent with `block_runtime_id=0` (invalid for blocks with
    /// states). The generated path is kept behind the flag until the
    /// block-state runtime resolution is implemented.
    pub fn creative_content_wire() -> Vec<u8> {
        if std::env::var("PM_CORE_CREATIVE_MODE").as_deref() != Ok("generated") {
            return StaticPackets::CREATIVE_CONTENT.to_vec();
        }
        Self::generated_creative_content_wire()
    }

    /// Generated `CreativeContentPacket` (0x91) from `data/creative/*.json`.
    /// Currently NOT sent to clients by default — see
    /// [`PlayerSession::creative_content_wire`].
    pub fn generated_creative_content_wire() -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_var_u32(bedrock_protocol::packets::creative_content::CreativeContentPacket::PACKET_ID);
        bedrock_protocol::packets::creative_content::CreativeContentPacket {
            groups: crate::crafting::creative_groups(),
            items: crate::crafting::creative_items(),
        }
        .encode_payload(&mut writer);
        writer.into_bytes().to_vec()
    }

    /// Damages the anvil at `(x, y, z)` after a block break, firing the
    /// cancellable `AnvilDamageEvent`. Returns `true` when the anvil broke.
    pub fn damage_anvil_at(&mut self, x: i32, y: i32, z: i32, roll: u8) -> bool {
        let old_damage = self.anvil_damage;
        let mut state = crate::anvil::AnvilState::new(old_damage);
        let hit = state.damage(roll);
        if self
            .plugins
            .as_ref()
            .map(|plugins| plugins.has_listeners("AnvilDamageEvent"))
            .unwrap_or(false)
        {
            let mut event = php_plugin_bridge::AnvilDamageEvent::new(
                self.player_context(),
                self.world.name.clone(),
                x,
                y,
                z,
                old_damage,
                hit.damage,
                hit.broken,
            );
            self.fire_event(&mut event);
            if event.cancelled {
                return false;
            }
        }
        self.anvil_damage = hit.damage;
        hit.broken
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

    /// Creates an UpdateAbilitiesPacket (0xBB) setting player abilities
    /// (survival or creative with flight).
    ///
    /// Port of `NetworkSession::syncAbilities($for)` (axolotl
    /// `src/network/mcpe/NetworkSession.php:1149`):
    /// - `targetActorUniqueId` is `$for->getId()` — the small entity id,
    ///   i.e. [`Self::entity_runtime_id`]. NOT `entity_unique_id`, which for
    ///   authenticated clients holds the Xbox XUID: the client silently
    ///   drops an abilities packet addressed to an actor it does not own,
    ///   which left every Bedrock player with default (survival) abilities
    ///   despite StartGame's creative mode.
    /// - `playerPermission` = MEMBER (1), `commandPermission` = NORMAL (0).
    /// - The base layer SETs every ability PM sets (`ABILITY_*` 0..=19,
    ///   including `VERTICAL_FLY_SPEED` at index 19): PM notes "ALL of these
    ///   need to be set for the base layer, otherwise the client will cry".
    /// - Creative: `INFINITE_RESOURCES` (instabuild) + `INVULNERABLE` +
    ///   `ALLOW_FLIGHT`; survival: neither.
    pub fn create_update_abilities_packet(&self) -> Vec<u8> {
        // PM `syncAbilities`: `ABILITY_*` bit indices.
        const ABILITY_BUILD: u32 = 0;
        const ABILITY_MINE: u32 = 1;
        const ABILITY_DOORS_AND_SWITCHES: u32 = 2;
        const ABILITY_OPEN_CONTAINERS: u32 = 3;
        const ABILITY_ATTACK_PLAYERS: u32 = 4;
        const ABILITY_ATTACK_MOBS: u32 = 5;
        const ABILITY_OPERATOR: u32 = 6;
        const ABILITY_TELEPORT: u32 = 7;
        const ABILITY_INVULNERABLE: u32 = 8;
        const ABILITY_FLYING: u32 = 9;
        const ABILITY_ALLOW_FLIGHT: u32 = 10;
        const ABILITY_INFINITE_RESOURCES: u32 = 11;
        const ABILITY_LIGHTNING: u32 = 12;
        const ABILITY_FLY_SPEED: u32 = 13;
        const ABILITY_WALK_SPEED: u32 = 14;
        const ABILITY_MUTED: u32 = 15;
        const ABILITY_WORLD_BUILDER: u32 = 16;
        const ABILITY_NO_CLIP: u32 = 17;
        const ABILITY_PRIVILEGED_BUILDER: u32 = 18;
        const ABILITY_VERTICAL_FLY_SPEED: u32 = 19;

        let creative = self.gamemode == 1;

        let mut set = 0u32;
        let mut values = 0u32;
        let mut set_ability = |ability: u32, value: bool| {
            set |= 1 << ability;
            if value {
                values |= 1 << ability;
            }
        };

        // Every ability PM lists for LAYER_BASE (all of them must be set).
        set_ability(ABILITY_ALLOW_FLIGHT, creative);
        set_ability(ABILITY_FLYING, self.is_flying);
        set_ability(ABILITY_NO_CLIP, false);
        set_ability(ABILITY_OPERATOR, false);
        set_ability(ABILITY_TELEPORT, false);
        set_ability(ABILITY_INVULNERABLE, creative);
        set_ability(ABILITY_MUTED, false);
        set_ability(ABILITY_WORLD_BUILDER, false);
        set_ability(ABILITY_INFINITE_RESOURCES, creative);
        set_ability(ABILITY_LIGHTNING, false);
        set_ability(ABILITY_BUILD, true);
        set_ability(ABILITY_MINE, true);
        set_ability(ABILITY_DOORS_AND_SWITCHES, true);
        set_ability(ABILITY_OPEN_CONTAINERS, true);
        set_ability(ABILITY_ATTACK_PLAYERS, true);
        set_ability(ABILITY_ATTACK_MOBS, true);
        set_ability(ABILITY_PRIVILEGED_BUILDER, false);
        // Speed abilities carry their value in the trailing floats; the
        // encoder marks them set.
        set |= (1 << ABILITY_FLY_SPEED) | (1 << ABILITY_WALK_SPEED) | (1 << ABILITY_VERTICAL_FLY_SPEED);

        let mut writer = Writer::new();
        writer.write_var_u32(0xbb); // UpdateAbilities packet ID
        writer.write_i64_le(self.entity_runtime_id as i64); // PM `$for->getId()`
        writer.write_byte(1); // player_permission: MEMBER
        writer.write_byte(0); // command_permission: NORMAL
        writer.write_byte(1); // layer count: 1
        writer.write_u16_le(1); // LAYER_BASE
        writer.write_u32_le(set);
        writer.write_u32_le(values);
        writer.write_f32_le(0.05); // fly_speed
        writer.write_f32_le(1.0); // vertical_fly_speed (PM passes 1)
        writer.write_f32_le(0.1); // walk_speed
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

    /// Builds the wire bytes of a `BossEventPacket` (0x4A): id header +
    /// payload. Queued via `OutgoingPacket::BossEvent` on the typed
    /// path; this helper is kept for `RawWire` callers (tests).
    pub fn create_boss_event_wire(&self, packet: &BossEventPacket) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.write_var_u32(packet.packet_id());
        packet
            .encode(&mut writer)
            .expect("BossEventPacket encode is infallible");
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
        writer.write_var_u32(0x58);
        writer.write_var_i32(title_type);
        writer.write_string(text);
        writer.write_var_i32(fade_in);
        writer.write_var_i32(stay);
        writer.write_var_i32(fade_out);
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
    /// Sneak state (`PlayerContext.is_sneaking`, Bedrock
    /// `EntityMetadataFlags::SNEAKING` bit 1 = value 2). Carried in
    /// `AddPlayer` metadata at spawn and re-broadcast as `SetActorData`
    /// on change, so Bedrock viewers see Java players crouch.
    is_sneaking: bool,
    /// Last position/rotation broadcast as `MoveActorAbsolutePacket`
    /// (or `MoveActorDeltaPacket` for small moves).
    sent_x: f64,
    sent_y: f64,
    sent_z: f64,
    sent_yaw: f64,
    sent_pitch: f64,
    /// Last sneak state sent as `SetActorDataPacket`.
    sent_sneaking: bool,
}

impl VirtualEntity {
    fn new(name: String, runtime_id: u64, snapshot: &PlayerContext) -> Self {
        // `name` is the raw registry name; the Bedrock-view display name
        // (see `bedrock_display_name`) derives from it. The UUID is derived
        // from the display name, which is currently identical to the raw
        // name (Java -> Bedrock is prefix-free, like Geyser).
        let display = bedrock_view_display_name(&name, true);
        Self {
            uuid: virtual_uuid_for(&display),
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
            is_sneaking: snapshot.is_sneaking,
            sent_x: snapshot.x,
            sent_y: snapshot.y,
            sent_z: snapshot.z,
            sent_yaw: snapshot.yaw,
            sent_pitch: snapshot.pitch,
            sent_sneaking: snapshot.is_sneaking,
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
    /// Username, metadata-4 nametag and `PlayerListEntry.username` all use
    /// the Bedrock-view display name (verbatim Java nick, like Geyser).
    fn bedrock_display_name(&self) -> String {
        bedrock_view_display_name(&self.name, true)
    }

    fn add_player_packet(&self) -> AddPlayerPacket {
        let mut flags: i64 = FLAG_HAS_COLLISION | FLAG_AFFECTED_BY_GRAVITY | FLAG_CAN_CLIMB;
        if self.is_sneaking {
            flags |= FLAG_SNEAKING;
        }
        let display = self.bedrock_display_name();
        let metadata = vec![
            MetadataEntry::new(0, MetadataValue::Long(flags)),
            MetadataEntry::new(4, MetadataValue::String(display.clone())),
            MetadataEntry::new(7, MetadataValue::Short(300)),
            MetadataEntry::new(42, MetadataValue::Short(300)),
            MetadataEntry::new(38, MetadataValue::Float(1.0)),
            MetadataEntry::new(53, MetadataValue::Float(0.6)),
            MetadataEntry::new(54, MetadataValue::Float(1.8)),
        ];
        AddPlayerPacket::new(
            self.uuid,
            display,
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

    /// Mirrors [`PlayerSession::data_flags`] for a virtual snapshot.
    fn data_flags(&self) -> i64 {
        let mut flags: i64 = FLAG_HAS_COLLISION | FLAG_AFFECTED_BY_GRAVITY | FLAG_CAN_CLIMB;
        if self.is_sneaking {
            flags |= FLAG_SNEAKING;
        }
        flags
    }

    /// Sneak/state broadcast for viewers that already spawned this entity
    /// (`PlayerSession::create_set_actor_data_packet` mirror).
    fn set_actor_data_packet(&self) -> SetActorDataPacket {
        let display = self.bedrock_display_name();
        SetActorDataPacket::new(
            self.runtime_id,
            vec![
                MetadataEntry::new(0, MetadataValue::Long(self.data_flags())),
                MetadataEntry::new(4, MetadataValue::String(display)),
                MetadataEntry::new(7, MetadataValue::Short(300)),
                MetadataEntry::new(42, MetadataValue::Short(300)),
                MetadataEntry::new(38, MetadataValue::Float(1.0)),
                MetadataEntry::new(53, MetadataValue::Float(0.6)),
                MetadataEntry::new(54, MetadataValue::Float(1.8)),
            ],
            bedrock_protocol::PropertySyncData::empty(),
            0,
        )
    }

    fn player_list_addition(&self) -> PlayerListEntry {
        PlayerListEntry::addition(
            self.uuid,
            self.unique_id,
            self.bedrock_display_name(),
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

/// Report of one unified server tick (`SessionManager::tick`).
///
/// The central 20 TPS clock in `main.rs` calls this once per 50 ms and
/// then drives `JavaServer::tick_at` with the same `tick` number, so
/// Bedrock and Java observe the same world time in the same tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickReport {
    /// 1-based tick number (first tick is 1).
    pub tick: u64,
    /// World time after this tick's [`WorldState::tick`].
    pub world_time: i32,
    /// Block changes forwarded to Bedrock viewers this tick.
    pub block_changes: usize,
    /// Virtual-entity packets queued for Bedrock viewers this tick.
    pub virtual_queued: usize,
    /// Chunk packets queued by the tick's `doChunkRequests` pass
    /// (`NetworkSession::tick()` -> `Player::doChunkRequests()`):
    /// `NetworkChunkPublisherUpdate` + up to `CHUNK_SENDING_PER_TICK`
    /// `LevelChunk`s (plus the terrain-ready `PLAYER_SPAWN` when the
    /// threshold chunk lands) per eligible session.
    pub chunk_packets_queued: usize,
    /// PreSpawn sessions force-spawned this tick (always 0: PocketMine has
    /// no prespawn-timeout spawn; kept for report compatibility).
    pub prespawn_forced: usize,
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
    /// Per-stage latency samples reported once per second (`[lag]` lines).
    lag: crate::metrics::LagProbe,
    permissions: Arc<crate::permission::PermissionManager>,
    virtuals: RwLock<HashMap<String, VirtualEntity>>,
    next_virtual_runtime: AtomicU64,
    /// Central tick number (see [`TickReport`]); incremented once per
    /// [`SessionManager::tick`]. This is the single Bedrock-side clock
    /// the Java connector ticks against — no second interval exists.
    tick_count: AtomicU64,
    /// Ticks since the last periodic `SetTime` broadcast
    /// (`tick_world_time`); reset on every broadcast.
    time_tick_counter: std::sync::atomic::AtomicI32,
    /// World time of the last `SetTime` broadcast, or -1 before the
    /// first one. Used to spot jumps (`/time set`, plugin
    /// `world.set_time`) that differ from normal +1/tick progression.
    last_time_broadcast: std::sync::atomic::AtomicI32,
    /// Per-player scoreboards (`Player::setScoreboard`), keyed by
    /// lowercase name. Shared with the Java bridge through the
    /// `take_scoreboard` hook so virtual players render the same
    /// objectives as Bedrock ones.
    scoreboards: RwLock<HashMap<String, Scoreboard>>,
    /// Global/default scoreboard applied to players without a personal
    /// copy (persisted in `scoreboard.json`).
    global_scoreboard: RwLock<Option<Scoreboard>>,
    /// Last scoreboard state framed for each Java client, so
    /// `take_scoreboard_java_frames` only emits the diff. Keyed by
    /// (lowercase name, wire protocol): a 340 client and a 775 client
    /// each get their own full first send.
    java_scoreboard_sent: RwLock<HashMap<(String, i32), Scoreboard>>,
    /// Boss bars (apiBossBar `BossBar`/`DiverseBossBar`): shared state
    /// in `BossBarManager`, delivered as Bedrock `BossEventPacket`
    /// (0x4A) to Bedrock sessions and Java `boss_bar` frames to
    /// virtual players through the `take_bossbar_frames` hook poll.
    bossbars: BossBarManager,
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
            lag: crate::metrics::LagProbe::default(),
            permissions: Arc::new(crate::permission::PermissionManager::new()),
            virtuals: RwLock::new(HashMap::new()),
            // Bedrock runtime ids are session ids (small ints); keep virtual
            // runtime ids in a disjoint range so packets never confuse them.
            next_virtual_runtime: AtomicU64::new(10_000_000),
            tick_count: AtomicU64::new(0),
            time_tick_counter: std::sync::atomic::AtomicI32::new(0),
            last_time_broadcast: std::sync::atomic::AtomicI32::new(-1),
            scoreboards: RwLock::new(HashMap::new()),
            global_scoreboard: RwLock::new(None),
            java_scoreboard_sent: RwLock::new(HashMap::new()),
            bossbars: BossBarManager::new(),
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

    /// `allow-cheats` read by the Java chat router to build command sources.
    pub fn allow_cheats(&self) -> bool {
        *self.allow_cheats.read()
    }

    /// `max-players` reported by `/list` for sources built outside a session.
    pub fn max_players(&self) -> i32 {
        self.max_players.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn plugin_manager(&self) -> Option<Arc<PluginManager>> {
        self.plugins.read().clone()
    }

    /// `Player::setScoreboard()`: stores the player's scoreboard and
    /// queues the Bedrock diff (removed objectives + full new state)
    /// when a live Bedrock session exists. Virtual (Java) players have
    /// no Bedrock session; the Java bridge polls the same store through
    /// the `take_scoreboard` hook.
    pub fn set_scoreboard(&self, name: &str, scoreboard: Option<Scoreboard>) {
        let key = name.to_lowercase();
        match &scoreboard {
            Some(board) => {
                self.scoreboards.write().insert(key.clone(), board.clone());
            }
            None => {
                self.scoreboards.write().remove(&key);
            }
        }
        if let Some(id) = self.session_id_for(name) {
            let mut map = self.sessions.write();
            if let Some(session) = map.get_mut(&id) {
                let old = session.scoreboard.clone();
                let packets = match (&old, &scoreboard) {
                    (None, None) => Vec::new(),
                    (Some(old), None) => old.bedrock_remove_packets(),
                    (Some(old), Some(new)) => new.bedrock_diff_packets(old),
                    (None, Some(new)) => new.bedrock_packets(),
                };
                for packet in packets {
                    session.pending_outgoing.push(OutgoingPacket::from(packet));
                }
                session.scoreboard = scoreboard;
            }
        }
    }

    /// The player's effective scoreboard: personal copy first, then the
    /// global/default one.
    pub fn get_scoreboard(&self, name: &str) -> Option<Scoreboard> {
        if let Some(board) = self.scoreboards.read().get(&name.to_lowercase()) {
            return Some(board.clone());
        }
        self.global_scoreboard.read().clone()
    }

    /// Applies one `/scoreboard` mutation and persists the result.
    /// `global` targets the default scoreboard; players mutate their
    /// personal copy only (a first mutation does not copy the global
    /// board into it).
    fn apply_scoreboard_operation(
        &self,
        target: &str,
        operation: &crate::command::ScoreboardOperation,
    ) {
        use crate::command::ScoreboardOperation;
        use crate::scoreboard::ScoreboardObjective;

        fn apply(board: &mut Scoreboard, operation: &ScoreboardOperation) {
            match operation {
                ScoreboardOperation::Clear => board.clear(),
                ScoreboardOperation::AddObjective(objective) => {
                    if let Some(existing) = board.get_objective_mut(&objective.name) {
                        // `set` updates the display name but keeps lines.
                        existing.display_name = objective.display_name.clone();
                        existing.slot = objective.slot;
                    } else {
                        board.add_objective(objective.clone());
                    }
                }
                ScoreboardOperation::RemoveObjective(name) => {
                    board.remove_objective(name);
                }
                ScoreboardOperation::AddScore {
                    objective_name,
                    score,
                    text,
                } => {
                    if board.get_objective(objective_name).is_none() {
                        board.add_objective(ScoreboardObjective::sidebar(
                            objective_name.clone(),
                            objective_name.clone(),
                        ));
                    }
                    board.add_score(objective_name, *score, text.clone());
                }
                ScoreboardOperation::RemoveScore {
                    objective_name,
                    score,
                } => {
                    board.remove_score(objective_name, *score);
                }
            }
        }

        if target.eq_ignore_ascii_case("global") {
            let mut board = self.global_scoreboard.read().clone().unwrap_or_default();
            apply(&mut board, operation);
            self.set_global_scoreboard(Some(board));
        } else {
            let mut board = self
                .scoreboards
                .read()
                .get(&target.to_lowercase())
                .cloned()
                .unwrap_or_default();
            apply(&mut board, operation);
            self.set_scoreboard(target, Some(board));
        }
        if let Err(error) = self.save_scoreboards() {
            warn!("scoreboard.json save failed: {error}");
        }
    }

    /// After a session spawns, resolve its persisted personal
    /// scoreboard by the (now login-known) username and queue the diff
    /// against whatever the session already sent (the global seed).
    /// Runs once per session (`scoreboard_synced`); the queued packets
    /// land in `pending_outgoing` and are drained by the caller.
    fn sync_scoreboard_after_spawn(&self, session: &mut PlayerSession) {
        if !session.spawned || session.scoreboard_synced || session.username.is_empty() {
            return;
        }
        session.scoreboard_synced = true;
        let Some(personal) = self
            .scoreboards
            .read()
            .get(&session.username.to_lowercase())
            .cloned()
        else {
            return;
        };
        if session.scoreboard.as_ref() == Some(&personal) {
            return;
        }
        let packets = match &session.scoreboard {
            Some(old) => personal.bedrock_diff_packets(old),
            None => personal.bedrock_packets(),
        };
        for packet in packets {
            session
                .pending_outgoing
                .push(OutgoingPacket::from(packet));
        }
        session.scoreboard = Some(personal);
    }

    /// Java scoreboard drain (`take_scoreboard_frames` hook): framed
    /// `scoreboard_objective` / `scoreboard_score` /
    /// `scoreboard_display_objective` / `reset_score` packets for the
    /// diff between the player's effective board and the last state
    /// sent to their Java client. Modern (769-775) uses per-protocol
    /// ids from `ModernProfile`; legacy (340) uses 0x42/0x45/0x3B.
    /// Entry names are the line texts (Java has no fake-player ids);
    /// Bedrock's background tint has no Java counterpart and is
    /// ignored here.
    ///
    /// Observer-relative names (Geyser/Floodgate-style, `java_bridge::display`):
    /// objective display names and line texts are rendered for this Java
    /// viewer with live Bedrock raw names prefixed by `.`. E.g. a line
    /// `"Nombre: Steve"` stored verbatim shows to a Java player as
    /// `"Nombre: .Steve"`, while Bedrock viewers of the same board keep the
    /// verbatim text. Scoreboard lines stay opaque otherwise (no invented
    /// text), so plugins also see the dot and should use
    /// `java_bridge::display::display_name` when they build `Nombre: ...`
    /// lines themselves.
    /// Clears the scoreboard diff cache for `name`. Called on Java player
    /// join and leave so reconnecting sends the entire scoreboard cleanly.
    pub fn clear_java_scoreboard_sent(&self, name: &str) {
        let name_lower = name.to_lowercase();
        self.java_scoreboard_sent
            .write()
            .retain(|(n, _), _| n != &name_lower);
    }

    pub fn take_scoreboard_java_frames(&self, name: &str, protocol: i32) -> Vec<Vec<u8>> {
        let key = (name.to_lowercase(), protocol);
        let current = self.get_scoreboard(name).unwrap_or_default();
        let sent = self
            .java_scoreboard_sent
            .read()
            .get(&key)
            .cloned()
            .unwrap_or_default();
        if current == sent {
            return Vec::new();
        }
        let java_text = |raw: &str| raw.to_string();
        let modern = java_protocol::ModernProfile::for_protocol(protocol);
        let mut frames = Vec::new();
        let mut push = |id: i32, payload: Vec<u8>| {
            frames.push(java_protocol::protocol::frame(id, &payload));
        };
        // Removed objectives first.
        for objective in &sent.objectives {
            if current.get_objective(&objective.name).is_none() {
                match &modern {
                    Some(profile) => push(
                        profile.id("scoreboard_objective"),
                        java_protocol::scoreboard::modern_objective_remove(&objective.name),
                    ),
                    None => push(
                        java_protocol::scoreboard::S2C_SCOREBOARD_OBJECTIVE,
                        java_protocol::scoreboard::legacy_objective_remove(&objective.name),
                    ),
                }
            }
        }
        for objective in &current.objectives {
            let position = objective.slot.java_position();
            let old_opt = sent.get_objective(&objective.name);
            match &modern {
                Some(profile) => {
                    if let Some(old) = old_opt {
                        if java_text(&old.display_name) != java_text(&objective.display_name) {
                            push(
                                profile.id("scoreboard_objective"),
                                java_protocol::scoreboard::modern_objective_update(
                                    &objective.name,
                                    &java_text(&objective.display_name),
                                ),
                            );
                        }
                        if old.slot != objective.slot {
                            push(
                                profile.id("scoreboard_display_objective"),
                                java_protocol::scoreboard::modern_display_payload(
                                    position,
                                    &objective.name,
                                ),
                            );
                        }
                    } else {
                        push(
                            profile.id("scoreboard_objective"),
                            java_protocol::scoreboard::modern_objective_create(
                                &objective.name,
                                &java_text(&objective.display_name),
                            ),
                        );
                        push(
                            profile.id("scoreboard_display_objective"),
                            java_protocol::scoreboard::modern_display_payload(
                                position,
                                &objective.name,
                            ),
                        );
                    }
                    // Lines dropped or changed from a kept objective need a reset.
                    if let Some(old) = old_opt {
                        for old_line in &old.lines {
                            let old_text = java_text(&old_line.text);
                            let still_exists = objective.lines.iter().any(|l| {
                                java_text(&l.text) == old_text && l.score == old_line.score
                            });
                            if !still_exists {
                                push(
                                    profile.id("reset_score"),
                                    java_protocol::scoreboard::modern_reset_score_payload(
                                        &old_text,
                                        &objective.name,
                                    ),
                                );
                            }
                        }
                    }
                    for line in objective.sorted_lines() {
                        let cur_text = java_text(&line.text);
                        let is_same = old_opt.map_or(false, |old| {
                            old.lines
                                .iter()
                                .any(|ol| java_text(&ol.text) == cur_text && ol.score == line.score)
                        });
                        if !is_same {
                            push(
                                profile.id("scoreboard_score"),
                                java_protocol::scoreboard::modern_score_payload(
                                    &objective.name,
                                    &cur_text,
                                    line.score as i32,
                                ),
                            );
                        }
                    }
                }
                None => {
                    if let Some(old) = old_opt {
                        if java_text(&old.display_name) != java_text(&objective.display_name) {
                            push(
                                java_protocol::scoreboard::S2C_SCOREBOARD_OBJECTIVE,
                                java_protocol::scoreboard::legacy_objective_update(
                                    &objective.name,
                                    &java_text(&objective.display_name),
                                ),
                            );
                        }
                        if old.slot != objective.slot {
                            push(
                                java_protocol::scoreboard::S2C_SCOREBOARD_DISPLAY_OBJECTIVE,
                                java_protocol::scoreboard::legacy_display_payload(
                                    position,
                                    &objective.name,
                                ),
                            );
                        }
                    } else {
                        push(
                            java_protocol::scoreboard::S2C_SCOREBOARD_OBJECTIVE,
                            java_protocol::scoreboard::legacy_objective_create(
                                &objective.name,
                                &java_text(&objective.display_name),
                            ),
                        );
                        push(
                            java_protocol::scoreboard::S2C_SCOREBOARD_DISPLAY_OBJECTIVE,
                            java_protocol::scoreboard::legacy_display_payload(
                                position,
                                &objective.name,
                            ),
                        );
                    }
                    if let Some(old) = old_opt {
                        for old_line in &old.lines {
                            let old_text = java_text(&old_line.text);
                            let still_exists = objective.lines.iter().any(|l| {
                                java_text(&l.text) == old_text && l.score == old_line.score
                            });
                            if !still_exists {
                                push(
                                    java_protocol::scoreboard::S2C_SCOREBOARD_SCORE,
                                    java_protocol::scoreboard::legacy_score_remove_payload(
                                        &objective.name,
                                        &old_text,
                                    ),
                                );
                            }
                        }
                    }
                    for line in objective.sorted_lines() {
                        let cur_text = java_text(&line.text);
                        let is_same = old_opt.map_or(false, |old| {
                            old.lines
                                .iter()
                                .any(|ol| java_text(&ol.text) == cur_text && ol.score == line.score)
                        });
                        if !is_same {
                            push(
                                java_protocol::scoreboard::S2C_SCOREBOARD_SCORE,
                                java_protocol::scoreboard::legacy_score_payload(
                                    &objective.name,
                                    &cur_text,
                                    line.score as i32,
                                ),
                            );
                        }
                    }
                }
            }
        }
        self.java_scoreboard_sent
            .write()
            .insert(key, current);
        frames
    }

    // -- Boss bars (apiBossBar `BossBar` / `DiverseBossBar`) -------------

    /// The shared boss-bar registry (`BossBarManager`).
    pub fn bossbar_manager(&self) -> &BossBarManager {
        &self.bossbars
    }

    /// Queue one Bedrock `BossEventPacket` for `name` when a live
    /// Bedrock session exists. Java delivery needs no work here:
    /// `take_bossbar_java_frames` re-derives ADD/UPDATE/REMOVE frames
    /// from the bar state + last-sent snapshot at drain time (same
    /// split as chat: Bedrock packets vs Java poll).
    fn bossbar_queue(&self, name: &str, bedrock: BossEventPacket) {
        if let Some(id) = self.session_id_for(name) {
            if let Some(session) = self.sessions.write().get_mut(&id) {
                session.pending_outgoing.push(OutgoingPacket::BossEvent(bedrock));
            }
        }
    }

    /// Creates an empty shared bar (`new BossBar()`). Returns its
    /// Bedrock actor id; the bar shows nothing until `bossbar_add_player`.
    pub fn bossbar_create(&self) -> i64 {
        self.bossbars.create()
    }

    /// Creates an empty diverse bar (`new DiverseBossBar()`).
    pub fn bossbar_create_diverse(&self) -> i64 {
        self.bossbars.create_diverse()
    }

    /// `BossBar::createForEntity($entity)`: `actor_id` is the entity's
    /// Bedrock unique id (e.g. a session's `entity_unique_id`).
    pub fn bossbar_create_for_entity(&self, actor_id: i64, entity: impl Into<String>) -> i64 {
        self.bossbars.create_for_entity(actor_id, entity)
    }

    pub fn bossbar_get(&self, boss_id: i64) -> Option<BossBar> {
        self.bossbars.get(boss_id)
    }

    pub fn bossbar_get_diverse(&self, boss_id: i64) -> Option<DiverseBossBar> {
        self.bossbars.get_diverse(boss_id)
    }

    /// `BossBar::addPlayer($player)`: subscribe `name` and show the bar.
    /// Returns `false` when the bar is unknown.
    pub fn bossbar_add_player(&self, boss_id: i64, name: &str) -> bool {
        if let Some(bar) = self.bossbars.get(boss_id) {
            let packet = bar.show_packet();
            self.bossbars.with_bar(boss_id, |bar| {
                bar.add_player(name);
            });
            self.bossbar_queue(name, packet);
            return true;
        }
        if let Some(bar) = self.bossbars.get_diverse(boss_id) {
            let packet = bar.show_packet_for(name);
            self.bossbars.with_diverse(boss_id, |bar| {
                bar.add_player(name);
            });
            self.bossbar_queue(name, packet);
            return true;
        }
        false
    }

    /// `BossBar::addPlayers($players)`.
    pub fn bossbar_add_players(&self, boss_id: i64, names: &[String]) -> bool {
        let mut added = false;
        for name in names {
            added |= self.bossbar_add_player(boss_id, name);
        }
        added
    }

    /// `BossBar::removePlayer($player)`: hide + unsubscribe.
    pub fn bossbar_remove_player(&self, boss_id: i64, name: &str) -> bool {
        let Some(bar) = self.bossbars.get(boss_id) else {
            if self.bossbars.get_diverse(boss_id).is_some() {
                let packet = BossEventPacket::hide(boss_id);
                self.bossbars.with_diverse(boss_id, |bar| {
                    bar.remove_player(name);
                });
                self.bossbar_queue(name, packet);
                return true;
            }
            return false;
        };
        let _ = bar;
        let packet = BossEventPacket::hide(boss_id);
        self.bossbars.with_bar(boss_id, |bar| {
            bar.remove_player(name);
        });
        self.bossbar_queue(name, packet);
        true
    }

    /// `BossBar::removePlayers($players)`.
    pub fn bossbar_remove_players(&self, boss_id: i64, names: &[String]) -> bool {
        let mut removed = false;
        for name in names {
            removed |= self.bossbar_remove_player(boss_id, name);
        }
        removed
    }

    /// `BossBar::removeAllPlayers()`.
    pub fn bossbar_remove_all_players(&self, boss_id: i64) -> bool {
        let viewers = self
            .bossbars
            .get(boss_id)
            .map(|bar| bar.viewers())
            .unwrap_or_else(|| {
                self.bossbars
                    .get_diverse(boss_id)
                    .map(|bar| bar.viewers())
                    .unwrap_or_default()
            });
        if viewers.is_empty()
            && !self.bossbars.contains(boss_id)
        {
            return false;
        }
        let packet = BossEventPacket::hide(boss_id);
        if self.bossbars.with_bar(boss_id, |bar| { bar.remove_all_players(); }).is_some() {
            for viewer in &viewers {
                self.bossbar_queue(viewer, packet.clone());
            }
            return true;
        }
        if self.bossbars.with_diverse(boss_id, |bar| { bar.remove_all_players(); }).is_some() {
            for viewer in &viewers {
                self.bossbar_queue(viewer, packet.clone());
            }
            return true;
        }
        false
    }

    /// `BossBar::hideFrom($players)`: hide without unsubscribing.
    pub fn bossbar_hide_from(&self, boss_id: i64, names: &[String]) -> bool {
        if !self.bossbars.contains(boss_id) {
            return false;
        }
        let packet = BossEventPacket::hide(boss_id);
        for name in names {
            self.bossbar_queue(name, packet.clone());
        }
        true
    }

    /// `BossBar::showTo($players)`: show without subscribing new viewers.
    pub fn bossbar_show_to(&self, boss_id: i64, names: &[String]) -> bool {
        if let Some(bar) = self.bossbars.get(boss_id) {
            let packet = bar.show_packet();
            for name in names {
                self.bossbar_queue(name, packet.clone());
            }
            return true;
        }
        if let Some(bar) = self.bossbars.get_diverse(boss_id) {
            for name in names {
                self.bossbar_queue(name, bar.show_packet_for(name));
            }
            return true;
        }
        false
    }

    /// `BossBar::hideFromAll()`.
    pub fn bossbar_hide_all(&self, boss_id: i64) -> bool {
        let viewers = self
            .bossbars
            .get(boss_id)
            .map(|bar| bar.viewers())
            .unwrap_or_else(|| {
                self.bossbars
                    .get_diverse(boss_id)
                    .map(|bar| bar.viewers())
                    .unwrap_or_default()
            });
        if !self.bossbars.contains(boss_id) {
            return false;
        }
        let packet = BossEventPacket::hide(boss_id);
        for viewer in &viewers {
            self.bossbar_queue(viewer, packet.clone());
        }
        true
    }

    /// `BossBar::showToAll()`.
    pub fn bossbar_show_all(&self, boss_id: i64) -> bool {
        if let Some(bar) = self.bossbars.get(boss_id) {
            let packet = bar.show_packet();
            for viewer in bar.viewers() {
                self.bossbar_queue(&viewer, packet.clone());
            }
            return true;
        }
        if let Some(bar) = self.bossbars.get_diverse(boss_id) {
            for viewer in bar.viewers() {
                self.bossbar_queue(&viewer, bar.show_packet_for(&viewer));
            }
            return true;
        }
        false
    }

    /// `BossBar::setTitle($title)` (shared default; sends the title
    /// event to every viewer).
    pub fn bossbar_set_title(&self, boss_id: i64, title: impl Into<String>) -> bool {
        if let Some(mut bar) = self.bossbars.get(boss_id) {
            bar.set_title(title);
            let viewers = bar.viewers();
            let packet = bar.title_packet();
            self.bossbars
                .with_bar(boss_id, |stored| *stored = bar.clone());
            for viewer in &viewers {
                self.bossbar_queue(viewer, packet.clone());
            }
            return true;
        }
        if let Some(mut bar) = self.bossbars.get_diverse(boss_id) {
            bar.set_title(title);
            let viewers = bar.viewers();
            let packets: Vec<(String, BossEventPacket)> = viewers
                .iter()
                .map(|viewer| (viewer.clone(), bar.title_packet_for(viewer)))
                .collect();
            self.bossbars
                .with_diverse(boss_id, |stored| *stored = bar.clone());
            for (viewer, packet) in packets {
                self.bossbar_queue(&viewer, packet);
            }
            return true;
        }
        false
    }

    /// `BossBar::setSubTitle($subTitle)`.
    pub fn bossbar_set_subtitle(&self, boss_id: i64, subtitle: impl Into<String>) -> bool {
        if let Some(mut bar) = self.bossbars.get(boss_id) {
            bar.set_subtitle(subtitle);
            let viewers = bar.viewers();
            let packet = bar.title_packet();
            self.bossbars
                .with_bar(boss_id, |stored| *stored = bar.clone());
            for viewer in &viewers {
                self.bossbar_queue(viewer, packet.clone());
            }
            return true;
        }
        if let Some(mut bar) = self.bossbars.get_diverse(boss_id) {
            bar.set_subtitle(subtitle);
            let viewers = bar.viewers();
            let packets: Vec<(String, BossEventPacket)> = viewers
                .iter()
                .map(|viewer| (viewer.clone(), bar.title_packet_for(viewer)))
                .collect();
            self.bossbars
                .with_diverse(boss_id, |stored| *stored = bar.clone());
            for (viewer, packet) in packets {
                self.bossbar_queue(&viewer, packet);
            }
            return true;
        }
        false
    }

    /// `BossBar::setPercentage($percentage)` (clamped 0..1).
    pub fn bossbar_set_percentage(&self, boss_id: i64, percentage: f32) -> bool {
        if let Some(mut bar) = self.bossbars.get(boss_id) {
            bar.set_percentage(percentage);
            let viewers = bar.viewers();
            let packet = bar.health_packet();
            self.bossbars
                .with_bar(boss_id, |stored| *stored = bar.clone());
            for viewer in &viewers {
                self.bossbar_queue(viewer, packet.clone());
            }
            return true;
        }
        if let Some(mut bar) = self.bossbars.get_diverse(boss_id) {
            bar.set_percentage(percentage);
            let viewers = bar.viewers();
            let packets: Vec<(String, BossEventPacket)> = viewers
                .iter()
                .map(|viewer| (viewer.clone(), bar.health_packet_for(viewer)))
                .collect();
            self.bossbars
                .with_diverse(boss_id, |stored| *stored = bar.clone());
            for (viewer, packet) in packets {
                self.bossbar_queue(&viewer, packet);
            }
            return true;
        }
        false
    }

    /// `BossBar::setColor($color)` (re-sends `show`, as upstream does).
    pub fn bossbar_set_color(&self, boss_id: i64, color: u8) -> bool {
        if let Some(mut bar) = self.bossbars.get(boss_id) {
            bar.set_color(color);
            let viewers = bar.viewers();
            let packet = bar.show_packet();
            self.bossbars
                .with_bar(boss_id, |stored| *stored = bar.clone());
            for viewer in &viewers {
                self.bossbar_queue(viewer, packet.clone());
            }
            return true;
        }
        if let Some(mut bar) = self.bossbars.get_diverse(boss_id) {
            bar.set_color(color);
            let viewers = bar.viewers();
            let packets: Vec<(String, BossEventPacket)> = viewers
                .iter()
                .map(|viewer| (viewer.clone(), bar.show_packet_for(viewer)))
                .collect();
            self.bossbars
                .with_diverse(boss_id, |stored| *stored = bar.clone());
            for (viewer, packet) in packets {
                self.bossbar_queue(&viewer, packet);
            }
            return true;
        }
        false
    }

    /// `setStyle(color, overlay)` (new in this port; sends `properties`).
    pub fn bossbar_set_style(&self, boss_id: i64, color: u8, overlay: u8) -> bool {
        if let Some(mut bar) = self.bossbars.get(boss_id) {
            bar.set_style(color, overlay);
            let viewers = bar.viewers();
            let packet = bar.properties_packet();
            self.bossbars
                .with_bar(boss_id, |stored| *stored = bar.clone());
            for viewer in &viewers {
                self.bossbar_queue(viewer, packet.clone());
            }
            return true;
        }
        if let Some(mut bar) = self.bossbars.get_diverse(boss_id) {
            bar.set_style(color, overlay);
            let viewers = bar.viewers();
            let packet = bar.bar.properties_packet();
            self.bossbars
                .with_diverse(boss_id, |stored| *stored = bar.clone());
            for viewer in &viewers {
                self.bossbar_queue(viewer, packet.clone());
            }
            return true;
        }
        false
    }

    /// Java-only flags (darken/music/fog); Bedrock ignores them.
    pub fn bossbar_set_flags(&self, boss_id: i64, flags: u8) -> bool {
        if let Some(mut bar) = self.bossbars.get(boss_id) {
            bar.set_flags(flags);
            self.bossbars
                .with_bar(boss_id, |stored| *stored = bar.clone());
            return true;
        }
        if let Some(mut bar) = self.bossbars.get_diverse(boss_id) {
            bar.set_flags(flags);
            self.bossbars
                .with_diverse(boss_id, |stored| *stored = bar.clone());
            return true;
        }
        false
    }

    /// `BossBar::setEntity($entity)` (re-shows to every viewer).
    pub fn bossbar_set_entity(&self, boss_id: i64, entity: Option<String>) -> bool {
        if let Some(mut bar) = self.bossbars.get(boss_id) {
            bar.set_entity(entity);
            let viewers = bar.viewers();
            let packet = bar.show_packet();
            self.bossbars
                .with_bar(boss_id, |stored| *stored = bar.clone());
            for viewer in &viewers {
                self.bossbar_queue(viewer, packet.clone());
            }
            return true;
        }
        if let Some(mut bar) = self.bossbars.get_diverse(boss_id) {
            bar.set_entity(entity);
            let viewers = bar.viewers();
            let packets: Vec<(String, BossEventPacket)> = viewers
                .iter()
                .map(|viewer| (viewer.clone(), bar.show_packet_for(viewer)))
                .collect();
            self.bossbars
                .with_diverse(boss_id, |stored| *stored = bar.clone());
            for (viewer, packet) in packets {
                self.bossbar_queue(&viewer, packet);
            }
            return true;
        }
        false
    }

    /// `BossBar::resetEntity($removeEntity = false)`: unbind and
    /// re-show. Upstream also closes the bound entity when
    /// `$removeEntity` is true; this core has no entity lifecycle,
    /// so the flag only documents intent (bound name is dropped).
    pub fn bossbar_reset_entity(&self, boss_id: i64, _remove_entity: bool) -> bool {
        self.bossbar_set_entity(boss_id, None)
    }

    /// Destroy a bar: hide from every viewer, then unregister.
    pub fn bossbar_destroy(&self, boss_id: i64) -> bool {
        if !self.bossbars.contains(boss_id) {
            return false;
        }
        self.bossbar_remove_all_players(boss_id);
        self.bossbars.remove(boss_id)
    }

    // -- Diverse per-player overrides --------------------------------

    /// `DiverseBossBar::setTitleFor($players, $title)`.
    pub fn bossbar_set_title_for(
        &self,
        boss_id: i64,
        names: &[String],
        title: impl Into<String>,
    ) -> bool {
        let title = title.into();
        let Some(mut bar) = self.bossbars.get_diverse(boss_id) else {
            return false;
        };
        bar.set_title_for(names.iter().cloned(), title);
        self.bossbars
            .with_diverse(boss_id, |stored| *stored = bar.clone());
        for name in names {
            self.bossbar_queue(name, bar.title_packet_for(name));
        }
        true
    }

    /// `DiverseBossBar::setSubTitleFor($players, $subTitle)`.
    pub fn bossbar_set_subtitle_for(
        &self,
        boss_id: i64,
        names: &[String],
        subtitle: impl Into<String>,
    ) -> bool {
        let subtitle = subtitle.into();
        let Some(mut bar) = self.bossbars.get_diverse(boss_id) else {
            return false;
        };
        bar.set_subtitle_for(names.iter().cloned(), subtitle);
        self.bossbars
            .with_diverse(boss_id, |stored| *stored = bar.clone());
        for name in names {
            self.bossbar_queue(name, bar.title_packet_for(name));
        }
        true
    }

    /// `DiverseBossBar::setPercentageFor($players, $percentage)`.
    pub fn bossbar_set_percentage_for(
        &self,
        boss_id: i64,
        names: &[String],
        percentage: f32,
    ) -> bool {
        let Some(mut bar) = self.bossbars.get_diverse(boss_id) else {
            return false;
        };
        bar.set_percentage_for(names.iter().cloned(), percentage);
        self.bossbars
            .with_diverse(boss_id, |stored| *stored = bar.clone());
        for name in names {
            self.bossbar_queue(name, bar.health_packet_for(name));
        }
        true
    }

    /// `DiverseBossBar::setColorFor($players, $color)`.
    pub fn bossbar_set_color_for(
        &self,
        boss_id: i64,
        names: &[String],
        color: u8,
    ) -> bool {
        let Some(mut bar) = self.bossbars.get_diverse(boss_id) else {
            return false;
        };
        bar.set_color_for(names.iter().cloned(), color);
        self.bossbars
            .with_diverse(boss_id, |stored| *stored = bar.clone());
        for name in names {
            self.bossbar_queue(name, bar.show_packet_for(name));
        }
        true
    }

    /// `DiverseBossBar::resetFor($player)` (re-shows with defaults).
    pub fn bossbar_reset_for(&self, boss_id: i64, name: &str) -> bool {
        let Some(mut bar) = self.bossbars.get_diverse(boss_id) else {
            return false;
        };
        bar.reset_for(name);
        self.bossbars
            .with_diverse(boss_id, |stored| *stored = bar.clone());
        self.bossbar_queue(name, bar.show_packet_for(name));
        true
    }

    /// `DiverseBossBar::resetForAll()`.
    pub fn bossbar_reset_for_all(&self, boss_id: i64) -> bool {
        let Some(mut bar) = self.bossbars.get_diverse(boss_id) else {
            return false;
        };
        bar.reset_for_all();
        let viewers = bar.viewers();
        self.bossbars
            .with_diverse(boss_id, |stored| *stored = bar.clone());
        for viewer in &viewers {
            self.bossbar_queue(viewer, bar.show_packet_for(viewer));
        }
        true
    }

    // -- wire -> state (PHP `set_bossbar` full-state pattern) -----------

    /// Applies a whole PHP `set_bossbar` payload: builds the bar from
    /// `data`, diffs it against the stored copy, and queues the
    /// minimal Bedrock/Java deltas exactly like `set_scoreboard`
    /// does for scoreboards. Unknown `boss_id` values create the bar.
    pub fn bossbar_apply_data(&self, data: &BossBarData) {
        let boss_id = data.boss_id;
        // Snapshot of the previous state (defaults when new).
        let previous_shared = self.bossbars.get(boss_id);
        let previous_diverse = self.bossbars.get_diverse(boss_id);

        // Build the new state.
        let mut shared = BossBar::new(boss_id);
        shared
            .set_title(data.title.clone())
            .set_subtitle(data.subtitle.clone())
            .set_percentage(data.percentage)
            .set_color(data.color)
            .set_overlay(data.overlay)
            .set_flags(data.flags)
            .set_entity(data.entity.clone());
        for player in &data.players {
            shared.add_player(player);
        }

        if data.diverse {
            let mut diverse = DiverseBossBar::new(boss_id);
            diverse.bar = shared.clone();
            for entry in &data.overrides {
                if !entry.player.is_empty() {
                    if let Some(title) = &entry.title {
                        diverse.set_title_for([entry.player.clone()], title.clone());
                    }
                    if let Some(subtitle) = &entry.subtitle {
                        diverse
                            .set_subtitle_for([entry.player.clone()], subtitle.clone());
                    }
                    if let Some(percentage) = entry.percentage {
                        diverse.set_percentage_for([entry.player.clone()], percentage);
                    }
                    if let Some(color) = entry.color {
                        diverse.set_color_for([entry.player.clone()], color);
                    }
                }
            }
            self.bossbars.diverse_insert(boss_id, diverse);
            self.bossbars.shared_remove_quiet(boss_id);
            self.bossbar_diff_diverse(
                boss_id,
                previous_diverse.as_ref(),
                data,
            );
        } else {
            self.bossbars.shared_insert(boss_id, shared);
            self.bossbars.diverse_remove_quiet(boss_id);
            self.bossbar_diff_shared(boss_id, previous_shared.as_ref(), data);
        }
    }

    fn bossbar_diff_shared(
        &self,
        boss_id: i64,
        previous: Option<&BossBar>,
        data: &BossBarData,
    ) {
        let Some(new) = self.bossbars.get(boss_id) else {
            return;
        };
        let hidden: std::collections::HashSet<String> =
            data.hidden.iter().map(|name| name.to_lowercase()).collect();
        let previous_viewers: std::collections::HashSet<String> = previous
            .map(|bar| bar.viewers().into_iter().collect())
            .unwrap_or_default();
        let new_viewers: std::collections::HashSet<String> =
            new.viewers().into_iter().collect();

        for viewer in new.viewers() {
            let is_new = !previous_viewers.contains(&viewer);
            let is_hidden = hidden.contains(&viewer);
            if is_new || !is_hidden {
                // New viewer (or un-hidden): full ADD/show.
                self.bossbar_queue(&viewer, new.show_packet());
            } else {
                // Stays hidden: nothing.
                continue;
            }
            // Field updates only apply to viewers that were already
            // showing the bar.
            if is_new {
                continue;
            }
            if previous.is_some_and(|old| old.full_title() != new.full_title()) {
                self.bossbar_queue(&viewer, new.title_packet());
            }
            if previous.is_some_and(|old| old.percentage() != new.percentage()) {
                self.bossbar_queue(&viewer, new.health_packet());
            }
            if previous.is_some_and(|old| old.color() != new.color() || old.overlay() != new.overlay())
            {
                self.bossbar_queue(&viewer, new.properties_packet());
            }
        }
        for viewer in previous_viewers {
            if !new_viewers.contains(&viewer) || hidden.contains(&viewer) {
                self.bossbar_queue(&viewer, BossEventPacket::hide(boss_id));
            }
        }
        // Brand-new bar with no field changes: viewers were handled
        // above as ADD. A bar whose `boss_id` never existed has
        // `previous == None`, so every viewer already got `show`.
    }

    fn bossbar_diff_diverse(
        &self,
        boss_id: i64,
        previous: Option<&DiverseBossBar>,
        data: &BossBarData,
    ) {
        let Some(new) = self.bossbars.get_diverse(boss_id) else {
            return;
        };
        let hidden: std::collections::HashSet<String> =
            data.hidden.iter().map(|name| name.to_lowercase()).collect();
        let previous_viewers: std::collections::HashSet<String> = previous
            .map(|bar| bar.viewers().into_iter().collect())
            .unwrap_or_default();

        for viewer in new.viewers() {
            if hidden.contains(&viewer) {
                continue;
            }
            let is_new = !previous_viewers.contains(&viewer);
            if is_new {
                self.bossbar_queue(&viewer, new.show_packet_for(&viewer));
                continue;
            }
            let Some(old) = previous else { continue };
            if old.full_title_for(&viewer) != new.full_title_for(&viewer) {
                self.bossbar_queue(&viewer, new.title_packet_for(&viewer));
            }
            if old.percentage_for(&viewer) != new.percentage_for(&viewer) {
                self.bossbar_queue(&viewer, new.health_packet_for(&viewer));
            }
            if old.color_for(&viewer) != new.color_for(&viewer) {
                self.bossbar_queue(&viewer, new.show_packet_for(&viewer));
            }
        }
        for viewer in previous_viewers {
            if !new.is_viewer(&viewer) || hidden.contains(&viewer) {
                self.bossbar_queue(&viewer, BossEventPacket::hide(boss_id));
            }
        }
    }

    /// Java frame callback for `java-bridge`: drains the ADD /
    /// UPDATE_* / REMOVE deltas for `name` on `protocol`, diffing the
    /// current bar state against the last-sent snapshot
    /// (`BossBarManager::java_sent_*`). Bedrock sessions get
    /// `BossEventPacket` via `pending_outgoing` instead (same
    /// session-or-virtual split as `queue_packet`/`take_plugin_messages`).
    pub fn take_bossbar_java_frames(
        &self,
        name: &str,
        protocol: i32,
    ) -> Vec<java_bridge::BossBarFrame> {
        if self.session_id_for(name).is_some() {
            return Vec::new();
        }
        use crate::bossbar::JavaSentState;
        let uuid = |boss_id: i64| java_bridge::bossbar::bossbar_uuid(boss_id);
        let bossbar = java_bridge::bossbar::boss_bar_packet_id(protocol);
        let mut frames = Vec::new();

        for boss_id in self.bossbars.all_actor_ids() {
            // Effective per-player view for this bar.
            let (title, percentage, color, overlay, flags, watching) =
                if let Some(bar) = self.bossbars.get(boss_id) {
                    (
                        bar.full_title(),
                        bar.percentage(),
                        bar.color(),
                        bar.overlay(),
                        bar.flags(),
                        bar.is_viewer(name),
                    )
                } else if let Some(bar) = self.bossbars.get_diverse(boss_id) {
                    (
                        bar.full_title_for(name),
                        bar.percentage_for(name),
                        bar.color_for(name),
                        bar.bar.overlay(),
                        bar.bar.flags(),
                        bar.is_viewer(name),
                    )
                } else {
                    continue;
                };
            let sent = self.bossbars.java_sent_get(name, boss_id);
            match (watching, sent) {
                (false, Some(_)) => {
                    if let (Some(id), Some(frame)) = (
                        bossbar,
                        java_bridge::bossbar::remove_frame(&uuid(boss_id), protocol),
                    ) {
                        let _ = id;
                        frames.push(crate::bossbar::java_frame(frame));
                    }
                    self.bossbars.java_sent_remove(name, boss_id);
                }
                (true, None) => {
                    if let Some(frame) = java_bridge::bossbar::add_frame(
                        &uuid(boss_id),
                        protocol,
                        &title,
                        percentage,
                        java_bridge::bossbar::bedrock_color_to_java(color),
                        overlay as i32,
                        flags,
                    ) {
                        frames.push(crate::bossbar::java_frame(frame));
                    }
                    self.bossbars.java_sent_set(
                        name,
                        boss_id,
                        JavaSentState {
                            title,
                            percentage,
                            color,
                            overlay,
                            flags,
                        },
                    );
                }
                (true, Some(previous)) => {
                    let Some(id) = bossbar else { continue };
                    if previous.title != title {
                        frames.push(crate::bossbar::java_frame(
                            java_bridge::wire::frame(
                                id,
                                &java_bridge::bossbar::update_title_payload(
                                    &uuid(boss_id),
                                    protocol,
                                    &title,
                                ),
                            ),
                        ));
                    }
                    if previous.percentage != percentage {
                        frames.push(crate::bossbar::java_frame(
                            java_bridge::wire::frame(
                                id,
                                &java_bridge::bossbar::update_health_payload(
                                    &uuid(boss_id),
                                    percentage,
                                ),
                            ),
                        ));
                    }
                    if previous.color != color || previous.overlay != overlay {
                        frames.push(crate::bossbar::java_frame(
                            java_bridge::wire::frame(
                                id,
                                &java_bridge::bossbar::update_style_payload(
                                    &uuid(boss_id),
                                    java_bridge::bossbar::bedrock_color_to_java(color),
                                    overlay as i32,
                                ),
                            ),
                        ));
                    }
                    if previous.flags != flags {
                        frames.push(crate::bossbar::java_frame(
                            java_bridge::wire::frame(
                                id,
                                &java_bridge::bossbar::update_flags_payload(
                                    &uuid(boss_id),
                                    flags,
                                ),
                            ),
                        ));
                    }
                    self.bossbars.java_sent_set(
                        name,
                        boss_id,
                        JavaSentState {
                            title,
                            percentage,
                            color,
                            overlay,
                            flags,
                        },
                    );
                }
                (false, None) => {}
            }
        }
        // Bars deleted since the last drain get a REMOVE too.
        for boss_id in self.bossbars.java_sent_ids(name) {
            if self.bossbars.contains(boss_id) {
                continue;
            }
            if let Some(frame) =
                java_bridge::bossbar::remove_frame(&uuid(boss_id), protocol)
            {
                frames.push(crate::bossbar::java_frame(frame));
                self.bossbars.java_sent_remove(name, boss_id);
            }
        }
        frames
    }

    /// Hides every bar from a leaving player (`PlayerQuitEvent` ->
    /// `removePlayer` on all bars that player watched).
    pub fn bossbar_remove_viewer(&self, name: &str) {
        let key = name.to_lowercase();
        for boss_id in self.bossbars.all_actor_ids() {
            let watched = self
                .bossbars
                .get(boss_id)
                .map(|bar| bar.is_viewer(name))
                .unwrap_or_else(|| {
                    self.bossbars
                        .get_diverse(boss_id)
                        .map(|bar| bar.is_viewer(name))
                        .unwrap_or(false)
                });
            if !watched {
                continue;
            }
            if self.bossbars.with_bar(boss_id, |bar| { bar.remove_player(name); }).is_some() {
                self.bossbar_queue(name, BossEventPacket::hide(boss_id));
            } else if self
                .bossbars
                .with_diverse(boss_id, |bar| { bar.remove_player(name); })
                .is_some()
            {
                self.bossbar_queue(name, BossEventPacket::hide(boss_id));
            }
        }
        self.bossbars.java_sent_clear(&key);
    }

    /// Incoming `BossEventPacket` (0x4A) from a Bedrock client, the
    /// `PacketListener` port: register/unregister from the client
    /// are acknowledged in debug logs, `TYPE_QUERY` resends the
    /// whole bar, and any other (server-only) event type is logged.
    /// Upstream kicks on unexpected types; this core logs instead so a
    /// vanilla client quirk never drops a player.
    pub fn handle_boss_event(
        &self,
        session_id: u64,
        packet: BossEventPacket,
    ) -> Vec<OutgoingPacket> {
        use bedrock_protocol::BossEventPacket as Packet;
        match packet.event_type {
            Packet::TYPE_REGISTER_PLAYER => {
                debug!(
                    "BossEventPacket register by client for boss {} player {} (session {})",
                    packet.boss_actor_unique_id, packet.player_actor_unique_id, session_id
                );
                Vec::new()
            }
            Packet::TYPE_UNREGISTER_PLAYER => {
                debug!(
                    "BossEventPacket unregister by client for boss {} player {} (session {})",
                    packet.boss_actor_unique_id, packet.player_actor_unique_id, session_id
                );
                Vec::new()
            }
            Packet::TYPE_QUERY => {
                if let Some(bar) = self.bossbars.get(packet.boss_actor_unique_id) {
                    vec![OutgoingPacket::BossEvent(bar.show_packet())]
                } else if let Some(bar) = self.bossbars.get_diverse(packet.boss_actor_unique_id) {
                    let name = self
                        .sessions
                        .read()
                        .get(&session_id)
                        .map(|session| session.username.clone())
                        .unwrap_or_default();
                    vec![OutgoingPacket::BossEvent(bar.show_packet_for(&name))]
                } else {
                    Vec::new()
                }
            }
            other => {
                debug!(
                    "BossEventPacket unknown event type {} from session {} (boss {})",
                    other, session_id, packet.boss_actor_unique_id
                );
                Vec::new()
            }
        }
    }

    /// Sets (or clears, with `None`) the global/default scoreboard.
    /// Live sessions without a personal copy are re-sent the new one.
    pub fn set_global_scoreboard(&self, scoreboard: Option<Scoreboard>) {
        *self.global_scoreboard.write() = scoreboard;
        let personal: HashSet<String> = self.scoreboards.read().keys().cloned().collect();
        let names: Vec<String> = {
            let map = self.sessions.read();
            map.values()
                .filter(|session| session.joined)
                .map(|session| session.username.clone())
                .collect()
        };
        for name in names {
            if !personal.contains(&name.to_lowercase()) {
                let board = self.global_scoreboard.read().clone();
                self.set_scoreboard(&name, board);
            }
        }
    }

    /// `scoreboard.json`: global scoreboard + per-player copies.
    pub fn save_scoreboards(&self) -> std::io::Result<()> {
        let Some(dir) = self.data_dir.read().clone() else {
            return Ok(());
        };
        let file = crate::scoreboard::ScoreboardFile::new(
            self.global_scoreboard.read().clone(),
            self.scoreboards.read().clone(),
        );
        let bytes = serde_json::to_vec_pretty(&file)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
        std::fs::write(dir.join("scoreboard.json"), bytes)
    }

    /// Loads `scoreboard.json` (missing file = empty, like `world.json`).
    /// Returns how many player scoreboards were restored.
    pub fn load_scoreboards(&self) -> usize {
        let dir = self
            .data_dir
            .read()
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let Ok(bytes) = std::fs::read(dir.join("scoreboard.json")) else {
            return 0;
        };
        let Ok(file) = serde_json::from_slice::<crate::scoreboard::ScoreboardFile>(&bytes)
        else {
            warn!("scoreboard.json is unreadable; starting without persisted objectives");
            return 0;
        };
        let count = file.players.len();
        *self.global_scoreboard.write() = file.global;
        *self.scoreboards.write() = file.players;
        count
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
        // Persisted/default objectives: personal copy first, then the
        // global scoreboard (`scoreboard.json`). The username is not
        // known yet (login assigns it), so resolution happens again in
        // `handle_set_local_player_as_initialized` where the identity
        // is final; this only seeds the global default.
        session.scoreboard = self.global_scoreboard.read().clone();
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
        // apiBossBar: the quitter unsubscribes from every bar it
        // watched so stale bars never linger on rejoin.
        self.bossbar_remove_viewer(&session.username);

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
            // Java-view copy: live Bedrock names gain the Floodgate dot, like
            // `broadcast_text`.
            if let OutgoingPacket::Text(text) = &packet {
                let chat_like = matches!(
                    text.text_type,
                    TextPacket::TYPE_RAW
                        | TextPacket::TYPE_CHAT
                        | TextPacket::TYPE_SYSTEM
                        | TextPacket::TYPE_WHISPER
                );
                if chat_like && self.registry.get(name).is_some() {
                    self.registry.push_message(name, text.message.clone());
                }
            }
            return;
        };
        if let Some(session) = self.sessions.write().get_mut(&id) {
            session.pending_outgoing.push(packet);
        }
    }

    /// Broadcasts one text line to every player.
    ///
    /// Per-viewer display names (Geyser/Floodgate-style, `java_bridge::display`):
    /// Bedrock sessions receive `message` verbatim, Java inboxes receive a
    /// variant where whole-word occurrences of live Bedrock raw names are
    /// prefixed with `.` (Floodgate `BedrockPrefix`). Thus a Bedrock sender's
    /// `"<Steve> hola"` / `"Steve joined the game"` / `"Nombre: Steve"`
    /// appears to Java as `"<.Steve> hola"` etc., while a Java sender's line
    /// (no Bedrock name inside) is identical for both audiences. The stored
    /// board/line is never mutated; only the delivered Java copy differs.
    pub fn broadcast_text(&self, message: &str) {
        let packet = OutgoingPacket::Text(TextPacket::raw(message));
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
            entity.is_sneaking = snapshot.is_sneaking;

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
            let sneak_changed = entity.is_sneaking != entity.sent_sneaking;
            // Large jumps stay on the absolute channel (teleport);
            // continuous small movement goes on the interpolated delta
            // channel (`MoveActorDeltaPacket`, `ticks = 1`) so Bedrock
            // viewers slide instead of stepping/teleporting every tick.
            // Bedrock -> Bedrock keeps `MoveActorAbsolutePacket` (same as
            // PM `Entity::broadcastMovement`) — only Java virtuals use
            // the delta channel.
            let dist_from_sent = ((entity.x - entity.sent_x).abs()
                + (entity.y - entity.sent_y).abs()
                + (entity.z - entity.sent_z).abs())
                > 8.0;
            let mut moved_packets = 0;
            let mut sneak_packets = 0;

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
                    // Bedrock viewers see small moves as interpolated
                    // `MoveActorDeltaPacket`s (eye position, head yaw =
                    // body yaw like PM `broadcastMovement`) and long jumps
                    // as `MoveActorAbsolutePacket` teleports.
                    if dist_from_sent {
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
                                .push(OutgoingPacket::MoveActorAbsolute(packet));
                            queued += 1;
                            moved_packets += 1;
                        }
                    } else {
                        let packet = MoveActorDeltaPacket::full_move(
                            entity.runtime_id,
                            entity.eye_position(),
                            entity.pitch as f32,
                            entity.yaw as f32,
                            entity.yaw as f32,
                        );
                        if let Some(viewer) = map.get_mut(&viewer_id) {
                            viewer
                                .pending_outgoing
                                .push(OutgoingPacket::MoveActorDelta(packet));
                            queued += 1;
                            moved_packets += 1;
                        }
                    }
                }
                if sneak_changed && entity.viewers.contains(&viewer_id) {
                    if let Some(viewer) = map.get_mut(&viewer_id) {
                        viewer
                            .pending_outgoing
                            .push(OutgoingPacket::SetActorData(entity.set_actor_data_packet()));
                        queued += 1;
                        sneak_packets += 1;
                    }
                }
            }

            if moved {
                if moved_packets > 0 {
                    debug!(
                        "Virtual player {} moved to ({:.2}, {:.2}, {:.2}); queued {} movement packet(s) for {} Bedrock viewer(s)",
                        entity.name,
                        entity.x,
                        entity.y,
                        entity.z,
                        moved_packets,
                        entity.viewers.len(),
                    );
                }
                entity.mark_sent();
            }
            if sneak_changed && sneak_packets > 0 {
                debug!(
                    "Virtual player {} sneak -> {}; queued SetActorData for {} Bedrock viewer(s)",
                    entity.name,
                    entity.is_sneaking,
                    sneak_packets,
                );
            }
            entity.sent_sneaking = entity.is_sneaking;

            // Drop viewers that disconnected while this entity was visible.
            entity.viewers.retain(|id| map.contains_key(id));
            entity.listed_to.retain(|id| map.contains_key(id));
        }

        queued
    }

    /// Public entry for virtual sync, usable from the tick loop and from the
    /// `ServerPluginHost` virtual notify callback (which runs on the PHP
    /// worker thread). Uses a bounded-retry non-blocking lock so a host
    /// call running inside a session dispatch can never deadlock on the
    /// sessions lock, while still riding out the brief write-lock bursts
    /// of a concurrent Bedrock dispatch instead of dropping the whole
    /// sync cycle: a single `try_write` that loses the race used to delay
    /// a Java move to Bedrock viewers by a full extra poll cycle (up to
    /// 50 ms of jitter at 20 Hz, visible as stepping). A cycle truly
    /// missed here is retried by the next central tick
    /// (`SessionManager::tick` polls every 50 ms).
    pub fn poll_virtual_players(&self) -> usize {
        for _ in 0..32 {
            if let Some(mut map) = self.sessions.try_write() {
                return self.sync_virtuals_locked(&mut map);
            }
            std::hint::spin_loop();
        }
        0
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
            // Metadata-4 nametag uses the same Bedrock-view display name as
            // AddPlayer/PlayerList (overhead name == TAB name).
            let data = SetActorDataPacket::new(
                target.entity_runtime_id,
                vec![
                    MetadataEntry::new(0, MetadataValue::Long(target.data_flags())),
                    MetadataEntry::new(4, MetadataValue::String(target.bedrock_display_name())),
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
            self.sync_scoreboard_after_spawn(session);
            out
        };
        self.drain_pending_broadcasts_locked(&mut map, session_id);
        let mut out = out_result;
        // apiBossBar `PacketListener`: answer queued C2S boss events
        // (register/unregister log, query resends the bar).
        let drained: Vec<BossEventPacket> = map
            .get_mut(&session_id)
            .map(|session| std::mem::take(&mut session.pending_boss_events))
            .unwrap_or_default();
        for packet in drained {
            out.extend(self.handle_boss_event(session_id, packet));
        }
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
        self.save_scoreboards()?;
        self.world.save_to(&dir.join("world.json"))
    }

    /// `save-on` / `save-off` state, checked by the autosave tick.
    pub fn auto_save_enabled(&self) -> bool {
        self.auto_save.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Tick sampler consumed by `/tps`.
    /// Per-stage latency probe (`[lag]` log lines once per second).
    pub fn lag(&self) -> &crate::metrics::LagProbe {
        &self.lag
    }

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

    /// Periodic safety net between packets: forwards Java host
    /// `world.set_block` edits and virtual-player mutations to Bedrock
    /// viewers. PocketMine has no prespawn-timeout spawn (a Bedrock
    /// session simply awaits `RequestChunkRadius` and the terrain-ready
    /// chunks), so this never forces `PLAYER_SPAWN`; it always returns 0.
    pub fn poll_prespawn_timeouts(&self) -> usize {
        // Java edits made only via the `world.set_block` host call have no
        // `apply_actions` of their own, so they are forwarded to Bedrock
        // viewers here even when no Bedrock packet is in flight. Same for
        // virtual (Java bridge) players that mutate the registry through
        // host calls without any Bedrock packet in flight: the host notify
        // callback covers them immediately, this poll is the safety net.
        self.poll_block_changes();
        self.poll_virtual_players();
        0
    }

    /// Dispatches a raw Bedrock network message received over NetherNet WebRtc DataChannels
    pub fn dispatch_webrtc_packet(&self, peer: SocketAddr, raw: &[u8]) -> Vec<bytes::Bytes> {
        let started = std::time::Instant::now();
        let out = self.dispatch_webrtc_packet_inner(peer, raw);
        self.lag.dispatch.record(started.elapsed());
        out
    }

    fn dispatch_webrtc_packet_inner(&self, peer: SocketAddr, raw: &[u8]) -> Vec<bytes::Bytes> {
        let batch_started = std::time::Instant::now();
        let ipc_before = crate::metrics::ipc().snapshot();
        // Take the session out of the map so plugin IPC (which may call back
        // into the host) never runs while the session lock is held.
        let lock_started = std::time::Instant::now();
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

        let lock_elapsed = lock_started.elapsed();
        self.lag.lock.record(lock_elapsed);

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

        let packet_count = packets.len();
        let mut slow_packet: Option<(std::time::Duration, u32)> = None;
        let packets_started = std::time::Instant::now();
        for (packet_id, payload) in packets {
            let packet_started = std::time::Instant::now();
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
            let elapsed = packet_started.elapsed();
            if slow_packet
                .as_ref()
                .map_or(true, |(slowest, _)| elapsed > *slowest)
            {
                slow_packet = Some((elapsed, packet_id));
            }
        }
        let packets_total = packets_started.elapsed();

        // Chunk streaming (`Player::doChunkRequests`) is *not* run here:
        // in the reference it runs once per `NetworkSession::tick()`,
        // i.e. on the central 50 ms clock
        // ([`SessionManager::drive_chunk_requests`], called from
        // [`SessionManager::tick`]), never per inbound batch. Driving it
        // per batch stalled the stream when the client went quiet after
        // `RequestChunkRadius` ("Locating/Generating the world"): the
        // first 4 chunks went out with this batch and no further batch
        // ever arrived to trigger the next 4. `ChunkRadiusUpdated` still
        // precedes the chunks it unlocks — it is encoded in this batch,
        // the tick flush that follows sends the PublisherUpdate and
        // LevelChunks after it, and `PLAYER_SPAWN` still follows its
        // threshold chunk inside the tick pass.

        self.sync_scoreboard_after_spawn(&mut session);

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
        self.lag.packets.record(packets_total);
        let actions_started = std::time::Instant::now();
        self.apply_actions(&actions, 0);
        self.apply_command_actions(&core_actions);
        let actions_total = actions_started.elapsed();
        self.lag.actions.record(actions_total);

        // Pick up packets queued by broadcasts/actions for this same session.
        if let Some(session) = self.sessions.write().get_mut(&session_id) {
            outgoing.append(&mut session.pending_outgoing);
        }

        let batch_total = batch_started.elapsed();
        // One line per slow inbound batch: enough to tell a slow handler from
        // a slow action, a slow IPC round-trip or a stalled thread (a large
        // `batch` with a small sum of the parts). See `metrics::LagProbe`.
        if batch_total > std::time::Duration::from_millis(20) {
            let ipc_after = crate::metrics::ipc().snapshot();
            warn!(
                "[lag-slow] batch={:.1}ms npk={} lock={:.1}ms packets={:.1}ms actions={:.1}ms ipc={:.1}ms/{} slowest={:.1}ms/0x{:02X}",
                Self::ms(batch_total),
                packet_count,
                Self::ms(lock_elapsed),
                Self::ms(packets_total),
                Self::ms(actions_total),
                Self::ms(ipc_after.1.saturating_sub(ipc_before.1)),
                ipc_after.0.saturating_sub(ipc_before.0),
                Self::ms(slow_packet.map(|(elapsed, _)| elapsed).unwrap_or_default()),
                slow_packet.map(|(_, id)| id).unwrap_or(0),
            );
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
            // A single packet is never split across batches: the batch entry
            // is the length-prefixed packet buffer, exactly like
            // `PacketBatch::encodeRaw` (`target/BedrockProtocol/src/serializer/
            // PacketBatch.php`), so a 425kB `0x34` always occupies one batch on
            // its own. The 16kB bound only caps how many packets share a batch
            // (the reference puts every packet buffered during one tick into a
            // single batch via `NetworkSession::flushGamePacketQueue`); the
            // excess batches are separate SCTP messages for the same stream and
            // preserve wire order. The largest raw batch (the 425kB capture)
            // compresses to ~108kB, below the WebRTC `SctpMaxMessageSize::
            // Bounded(262144)` in `nethernet-rs/src/signaling.rs`, so no
            // transport fragmentation is required for it.
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
                // `Server::prepareBatch` + `ZlibCompressor::
                // DEFAULT_THRESHOLD` (256) in the reference: batches below
                // 256 bytes are sent uncompressed but still get the `0xFF`
                // prefix byte; larger ones are raw-deflated behind `0x00`.
                // `PacketBatch::compress` always emits one of the two
                // algorithm bytes when compression is enabled, matching it.
                match PacketBatch::compress(&batch, bedrock_protocol::batch::COMPRESSION_THRESHOLD) {
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
        let actions_started = std::time::Instant::now();
        // Slowest single action of this call, filled on drop so `continue`
        // arms are measured too. The debug string is only built when needed.
        let mut slow_action: Option<(std::time::Duration, String)> = None;

        struct ActionTimer<'a, F: Fn() -> String> {
            started: std::time::Instant,
            slowest: &'a mut Option<(std::time::Duration, String)>,
            describe: F,
        }

        impl<F: Fn() -> String> Drop for ActionTimer<'_, F> {
            fn drop(&mut self) {
                let elapsed = self.started.elapsed();
                if elapsed > std::time::Duration::from_millis(5)
                    && self
                        .slowest
                        .as_ref()
                        .map_or(true, |(slowest, _)| elapsed > *slowest)
                {
                    *self.slowest = Some((elapsed, (self.describe)()));
                }
            }
        }

        for action in actions {
            let _action_timer = ActionTimer {
                started: std::time::Instant::now(),
                slowest: &mut slow_action,
                describe: || format!("{:?}", action),
            };
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
                    debug!(
                        "[SetBlock] world {} at ({},{},{}) id={}",
                        world, x, y, z, block_id
                    );
                    let _ = self.world.set_block(*x, *y, *z, *block_id);
                }
                PluginAction::BroadcastLevelEvent {
                    event_id,
                    event_data,
                    x,
                    y,
                    z,
                } => {
                    // PM `World::broadcastPacketToViewers`: dig progress
                    // events go to every viewer holding the chunk.
                    self.broadcast_level_event(*event_id, *event_data, *x, *y, *z);
                }
                PluginAction::BroadcastSound {
                    sound,
                    extra_data,
                    x,
                    y,
                    z,
                } => {
                    // PM `World::addSound`: block place sounds reach every
                    // viewer holding the chunk, not only the placer.
                    self.broadcast_sound(sound, *extra_data, *x, *y, *z);
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
                                2,
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
                                2,
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
                PluginAction::SetScoreboard {
                    target_player,
                    scoreboard,
                } => {
                    let board = Scoreboard::from_data(scoreboard);
                    self.set_scoreboard(target_player, Some(board));
                }
                PluginAction::RemoveScoreboard { target_player } => {
                    // An explicit empty board masks the global default:
                    // removing must not resurrect it on the next poll.
                    self.set_scoreboard(target_player, Some(Scoreboard::default()));
                }
                // apiBossBar: PHP owns the full `BossBar`/
                // `DiverseBossBar` object; Rust diffs it against the
                // stored copy and queues Bedrock `BossEventPacket`
                // (0x4A) + Java `boss_bar` deltas.
                PluginAction::SetBossBar { bossbar } => {
                    self.bossbar_apply_data(bossbar);
                }
                PluginAction::RemoveBossBar { boss_id } => {
                    self.bossbar_destroy(*boss_id);
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

        let actions_total = actions_started.elapsed();
        // A slow `apply_actions` is either one pathological action (addressed
        // by the debug string) or many cheap ones (the count gives that away).
        if actions_total > std::time::Duration::from_millis(20) {
            match &slow_action {
                Some((elapsed, label)) => warn!(
                    "[lag-slow] actions={} total={:.1}ms slowest={:.1}ms: {}",
                    actions.len(),
                    Self::ms(actions_total),
                    Self::ms(*elapsed),
                    label
                ),
                None => warn!(
                    "[lag-slow] actions={} total={:.1}ms, none over 5ms",
                    actions.len(),
                    Self::ms(actions_total)
                ),
            }
        }

        follow_up
    }

    /// Milliseconds for the `[lag-slow]` lines.
    fn ms(duration: std::time::Duration) -> f64 {
        (duration.as_secs_f64() * 10000.0).round() / 10.0
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

    /// Public alias of [`Self::online_players_snapshot`] for the Java
    /// chat router (`crates/server-core/src/chat.rs`).
    pub fn online_players_snapshot_public(&self) -> Vec<OnlinePlayer> {
        self.online_players_snapshot()
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
                    display_name: session.username.clone(),
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
                    display_name: java_bridge::display::bedrock_view_display_name(
                        &entry.name,
                        entry.is_virtual,
                    ),
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
                CommandAction::Scoreboard { target, operation } => {
                    self.apply_scoreboard_operation(target, operation);
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

    /// Current central tick number (0 before the first [`SessionManager::tick`]).
    pub fn current_tick(&self) -> u64 {
        self.tick_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The one Bedrock-side tick, driven by the central 20 TPS clock in
    /// `main.rs`.
    ///
    /// This is the *only* server tick: there is no second Tokio interval
    /// for Bedrock anymore (the old 20 Hz flush sidecar and the old Java
    /// `run_arc` interval are gone). `main.rs` calls this once per 50 ms
    /// and then drives the Java connector with the same tick number
    /// (`JavaServer::tick_at`), so both platforms mutate state on the
    /// same pulse.
    ///
    /// What runs here is state mutation and envelope queueing only — no
    /// socket I/O and no blocking IPC:
    /// - the world clock ([`SessionManager::tick_world_time`]: `SetTime`
    ///   is queued as `RawWire` envelope, not sent),
    /// - Fase 2 block forwarding ([`SessionManager::poll_block_changes`]:
    ///   Java `world.set_block` edits reach Bedrock viewers as
    ///   `UpdateBlockPacket`, queued in `pending_outgoing`),
    /// - virtual-player sync ([`SessionManager::poll_virtual_players`]:
    ///   Java registry moves/spawns become `MoveActorAbsolute` /
    ///   `AddPlayer` envelopes for Bedrock viewers),
    /// - chunk streaming ([`SessionManager::drive_chunk_requests`]:
    ///   `Player::doChunkRequests`, exactly `NetworkSession::tick()`'s
    ///   per-tick call in the reference — up to
    ///   `CHUNK_SENDING_PER_TICK` `LevelChunk`s per eligible session,
    ///   queued in `pending_outgoing`).
    ///
    /// Chunk streaming deliberately does *not* run in
    /// `dispatch_webrtc_packet` any more: it used to, so a client that
    /// sent nothing after `RequestChunkRadius` (Bedrock sits in
    /// "Locating/Generating the world" until it sees terrain and sends
    /// no `0x71`/`0x90`) stalled the stream after the first 4 chunks.
    /// There is also no prespawn-timeout spawn: PocketMine simply awaits
    /// `RequestChunkRadius` and the terrain-ready chunks.
    ///
    /// Envelopes sit in `pending_outgoing` until the transport flushes
    /// them: `main.rs`' tick loop calls
    /// [`SessionManager::drain_pending_outgoing_bytes`] right after this
    /// tick (the `NetworkSession::flushGamePacketQueue()` half of the
    /// reference tick) and writes the encoded batches to each peer's
    /// WebRTC data channel, so a slow network never stalls the clock.
    /// `dispatch_webrtc_packet` still drains `pending_outgoing` on its
    /// way through, so anything queued between ticks also goes out on
    /// the next inbound batch. Plugin IPC (`tick_sync`) is deliberately
    /// *not* here: `main.rs` queues the tick on a sidecar that
    /// `spawn_blocking`s it, exactly like the chunk prefetch.
    /// PM `World::broadcastPacketToViewers($blockPos, $packet)` for
    /// `LevelEventPacket` dig events: queue the raw wire for every joined
    /// session holding the block's chunk — same recipient rule as
    /// [`Self::broadcast_block_change_ref`]. Returns recipients count.
    /// `World::addSound()` for a non-actor sound: one `LevelSoundEventPacket`
    /// (0x7B) to every session that holds the chunk.
    pub(crate) fn broadcast_sound(
        &self,
        sound: &str,
        extra_data: i32,
        x: i32,
        y: i32,
        z: i32,
    ) -> usize {
        let chunk = (x >> 4, z >> 4);
        let mut wire = Vec::with_capacity(32);
        {
            let mut writer = Writer::new();
            writer.write_var_u32(LEVEL_SOUND_EVENT_PACKET_ID);
            wire.extend_from_slice(&writer.into_bytes());
        }
        // PM plays the sound at the block centre.
        wire.extend_from_slice(&level_sound_event_wire(
            sound,
            x as f32 + 0.5,
            y as f32 + 0.5,
            z as f32 + 0.5,
            extra_data,
        ));
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

    pub(crate) fn broadcast_level_event(
        &self,
        event_id: i32,
        event_data: i32,
        x: i32,
        y: i32,
        z: i32,
    ) -> usize {
        let chunk = (x >> 4, z >> 4);
        // `RawWire` carries the packet id inside the buffer (same convention
        // as [`update_block_wire`]); the crate helper emits the payload only.
        let mut wire = Vec::with_capacity(16);
        {
            let mut writer = Writer::new();
            writer.write_var_u32(LEVEL_EVENT_PACKET_ID);
            wire.extend_from_slice(&writer.into_bytes());
        }
        wire.extend_from_slice(&level_packet_wire(event_id, event_data, x, y, z));
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
        drop(map);
        if recipients > 0 {
            debug!(
                "level_event: id={} data={} at ({},{},{}) recipients={}",
                event_id, event_data, x, y, z, recipients
            );
        }
        recipients
    }

    /// Per-tick PM `SurvivalBlockBreakHandler::update()` (Player.php:1564
    /// runs it every server tick while spawned). The handler never breaks
    /// the block itself: it accumulates server-driven dig progress, and the
    /// digger's client forecasts the final break via `PREDICT_DESTROY_BLOCK`
    /// (`CREATIVE_PLAYER_DESTROY_BLOCK` needs no handling for the same
    /// reason). Drops on distance > `maxPlayerDistance`² and at completion,
    /// each drop broadcasting `LevelEvent::BLOCK_STOP_BREAK` (the PM
    /// handler's `__destruct`).
    pub fn update_break_handlers(&self) {
        let mut stops: Vec<(i32, i32, i32)> = Vec::new();
        let mut completed: Vec<(String, i32, i32, i32)> = Vec::new();
        {
            let mut map = self.sessions.write();
            for session in map.values_mut() {
                if !session.joined || session.state != SessionState::InGame {
                    continue;
                }
                let Some(handler) = session.break_handler.as_mut() else {
                    continue;
                };
                // PM: position.distanceSquared(block + 0.5) >
                // maxPlayerDistance ** 2 stops the dig.
                let center = (
                    handler.pos.0 as f32 + 0.5,
                    handler.pos.1 as f32 + 0.5,
                    handler.pos.2 as f32 + 0.5,
                );
                let pos = session.position;
                let dist2 = (pos.x - center.0).powi(2)
                    + (pos.y - center.1).powi(2)
                    + (pos.z - center.2).powi(2);
                if dist2 > (handler.max_player_distance as f32).powi(2) {
                    let (x, y, z) = handler.pos;
                    session.break_handler = None;
                    debug!("break handler dropped: player walked away from ({},{},{})", x, y, z);
                    stops.push((x, y, z));
                    continue;
                }
                handler.break_progress += handler.break_speed;
                handler.fx_ticker += 1;
                if handler.break_progress < 1.0 {
                    continue;
                }
                // progress >= 1.0: PM `update()` returns false and `Player`
                // drops the handler. PM waits for the client's
                // `PREDICT_DESTROY_BLOCK`; this port breaks the block here
                // too (server-authoritative, like BDS in server-auth
                // breaking) so the world converges even if the client's
                // prediction never arrives. A later `PREDICT_DESTROY` for
                // the same position is a no-op (`set_block` sees AIR).
                let (x, y, z) = handler.pos;
                let world_name = session.world.name.clone();
                session.break_handler = None;
                debug!("break handler complete: ({},{},{}) breaking server-side", x, y, z);
                completed.push((world_name, x, y, z));
                stops.push((x, y, z));
            }
        }
        for (x, y, z) in stops {
            self.broadcast_level_event(LEVEL_EVENT_BLOCK_STOP_BREAK, 0, x, y, z);
        }
        // Authoritative edit: `set_block` enqueues for both sync consumers
        // (Bedrock broadcast queue + Java `take_block_changes`), so the
        // other clients converge from the next `poll_block_changes` pass.
        for (world_name, x, y, z) in completed {
            if world_name.is_empty() || world_name.eq_ignore_ascii_case(&self.world.name) {
                let _ = self.world.set_block(x, y, z, AIR);
            }
        }
    }

    pub fn tick(&self) -> TickReport {
        let tick = self
            .tick_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        let world_time = self.tick_world_time();
        // Sounds produced by scheduled block updates, e.g. a button releasing
        // itself (`onScheduledUpdate` -> `World::addSound`).
        for (sound, x, y, z) in self.world.take_pending_sounds() {
            self.broadcast_sound(&sound, -1, x, y, z);
        }
        // Dig handlers first: a completed dig edits the world, and the
        // broadcast poll below must ship that edit in this same tick.
        self.update_break_handlers();
        let block_changes = self.poll_block_changes();
        let virtual_queued = self.poll_virtual_players();
        let chunk_packets_queued = self.drive_chunk_requests();
        TickReport {
            tick,
            world_time,
            block_changes,
            virtual_queued,
            chunk_packets_queued,
            // Always 0: PocketMine has no prespawn-timeout spawn.
            prespawn_forced: 0,
        }
    }

    /// `NetworkSession::tick()` -> `Player::doChunkRequests()` for every
    /// session the reference would tick: `PreSpawn`/`InGame` with a view
    /// distance already received (`RequestChunkRadius` set it). Each
    /// [`PlayerSession::do_chunk_requests`] pass queues its
    /// `NetworkChunkPublisherUpdate` (when an ordering pass runs) plus up
    /// to `CHUNK_SENDING_PER_TICK` `LevelChunk`s — and the terrain-ready
    /// `PLAYER_SPAWN` right after the threshold chunk — into that
    /// session's `pending_outgoing`, in queue order behind anything the
    /// tick already queued (`SetTime`, block forwards, virtual sync).
    ///
    /// This runs on the central 50 ms clock (and in tests via
    /// [`SessionManager::tick`]), independently of inbound packets, so
    /// chunk streaming progresses while the client is still in
    /// "Locating/Generating the world" and sends nothing. Returns the
    /// total number of packets queued across all sessions. Sessions not
    /// eligible are skipped without touching `next_chunk_order_run`
    /// (`do_chunk_requests` itself is a no-op for them, this guard just
    /// keeps the driver explicit).
    pub fn drive_chunk_requests(&self) -> usize {
        let started = std::time::Instant::now();
        let queued = self.drive_chunk_requests_inner();
        self.lag.chunk_stream.record(started.elapsed());
        queued
    }

    fn drive_chunk_requests_inner(&self) -> usize {
        // Warm the columns each session is about to stream *before* locking
        // the sessions map. `do_chunk_requests` generates payloads with rayon
        // (~18 ms per cold column) and `drain_pending_outgoing_bytes` compresses
        // them; doing either under `sessions.write()` blocked every packet
        // handler in between — a break's particles/sound waited 60-160 ms, the
        // tick drifted and fast breaking dropped the peer.
        let pending: Vec<(i32, i32)> = {
            let map = self.sessions.read();
            let mut columns: Vec<(i32, i32)> = Vec::new();
            for session in map.values() {
                if !session.wants_chunk_streaming() {
                    continue;
                }
                columns.extend(session.next_chunk_columns());
            }
            columns
        };
        if !pending.is_empty() {
            pending.par_iter().for_each(|(cx, cz)| {
                self.world.chunk_payload(*cx, *cz);
            });
        }

        let mut map = self.sessions.write();
        let mut queued = 0usize;
        for session in map.values_mut() {
            if !session.wants_chunk_streaming() {
                continue;
            }
            let out = session.do_chunk_requests();
            queued += out.len();
            session.pending_outgoing.extend(out);
        }
        queued
    }

    /// `NetworkSession::flushGamePacketQueue()` for every session:
    /// takes each session's `pending_outgoing` (everything the tick
    /// queued, chunks included), encodes it with
    /// [`SessionManager::encode_outgoing`] under that session's
    /// compression state (the same `0x8F` rule `dispatch_webrtc_packet`
    /// applies), and returns the encoded batches keyed by peer address
    /// for the caller (the `main.rs` tick loop) to write to each peer's
    /// WebRTC data channel.
    ///
    /// Taking under the session lock makes this exclusive with
    /// `dispatch_webrtc_packet`'s drain: a packet is encoded by exactly
    /// one of the two paths, never both. Sessions with an empty queue
    /// produce no entry.
    pub fn drain_pending_outgoing_bytes(&self) -> Vec<(SocketAddr, Vec<bytes::Bytes>)> {
        let started = std::time::Instant::now();
        let drained = self.drain_pending_outgoing_bytes_inner();
        self.lag.drain.record(started.elapsed());
        drained
    }

    fn drain_pending_outgoing_bytes_inner(&self) -> Vec<(SocketAddr, Vec<bytes::Bytes>)> {
        // Take the queues under the lock, encode/compress outside it: a
        // terrain chunk is ~62 KB and deflating it under `sessions.write()`
        // stalled every packet handler (and so every block break ack) for
        // milliseconds per batch.
        let taken: Vec<(SocketAddr, Vec<OutgoingPacket>, bool)> = {
            let mut map = self.sessions.write();
            map.values_mut()
                .filter(|session| !session.pending_outgoing.is_empty())
                .map(|session| {
                    let outgoing = std::mem::take(&mut session.pending_outgoing);
                    let compression_enabled = session.compression_enabled
                        || outgoing.iter().any(|packet| packet.packet_id() == 0x8F);
                    session.compression_enabled = compression_enabled;
                    (session.client_addr, outgoing, compression_enabled)
                })
                .collect()
        };
        let mut drained = Vec::with_capacity(taken.len());
        for (peer, outgoing, compression_enabled) in taken {
            let bytes = self.encode_outgoing(outgoing, compression_enabled);
            if !bytes.is_empty() {
                drained.push((peer, bytes));
            }
        }
        drained
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
        // Only the `UpdateBlock`: PM's `changedBlocks` flush never carries the
        // destroy particles. Whoever breaks the block broadcasts
        // `PARTICLE_DESTROY_BLOCK` itself (`Session::handle_block_actions` for a
        // Bedrock dig, `CoreHooks::break_block` for a Java one), exactly like
        // `World::useBreakOn`. Emitting it here as well sent the client two
        // particle bursts for the same block, one tick apart.
        let level_event_wire: Option<Vec<u8>> = None;
        let mut map = self.sessions.write();
        let mut recipients = 0usize;
        for session in map.values_mut() {
            if !session.joined || !session.sent_chunks.contains(&chunk) {
                continue;
            }
            session
                .pending_outgoing
                .push(OutgoingPacket::RawWire(wire.clone()));
            if let Some(ref le_wire) = level_event_wire {
                session
                    .pending_outgoing
                    .push(OutgoingPacket::RawWire(le_wire.clone()));
            }
            recipients += 1;
        }
        // Always log: `recipients=0` is the failure mode where the edit
        // exists but no viewer holds the chunk (nothing is sent, silently).
        tracing::debug!(
            "broadcast_block_change: x={} y={} z={} block_id={} chunk={:?} recipients={}",
            change.x, change.y, change.z, change.block_id, chunk, recipients
        );
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
        let started = std::time::Instant::now();
        let delivered = self.poll_block_changes_inner();
        self.lag.poll.record(started.elapsed());
        delivered
    }

    fn poll_block_changes_inner(&self) -> usize {
        let changes = self.world.take_broadcast_changes();
        if !changes.is_empty() {
            tracing::debug!("poll_block_changes: drained {} changes", changes.len());
        }
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

/// InventorySlot wire bytes from a `ContainerSlot` (ItemStackRequest
/// container id -> InventorySlot window id).
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

    /// Chunk streaming must not hold the sessions lock while it builds
    /// columns: a block break's `UpdateBlock` and destroy particles are queued
    /// by a packet handler that needs that same lock, so a slow column used to
    /// delay them by the whole build (60-160 ms in the production log, where
    /// the tick held the lock while generating terrain).
    #[test]
    fn chunk_streaming_does_not_hold_the_sessions_lock() {
        struct SlowGenerator;

        impl WorldGenerator for SlowGenerator {
            fn generator_type(&self) -> i32 {
                1
            }
            fn spawn_position(&self) -> (i32, i32, i32) {
                (0, 64, 0)
            }
            fn player_position(&self) -> (f32, f32, f32) {
                (0.5, 64.0, 0.5)
            }
            fn generate_chunk(&self, _chunk_x: i32, _chunk_z: i32) -> (u32, Vec<u8>) {
                std::thread::sleep(std::time::Duration::from_millis(80));
                (0, Vec::new())
            }
        }

        let manager = Arc::new(SessionManager::with_generator(Arc::new(SlowGenerator)));
        let mut session = joined_session(1, &[]);
        session.world = Arc::clone(manager.world());
        session.view_distance_received = true;
        session.chunk_radius = 4;
        session.chunk_load_queue = vec![(0, 0), (1, 0)];
        manager.add_session(session);

        // Stand-in for a packet handler: it only needs the sessions lock.
        let waiter = {
            let manager = Arc::clone(&manager);
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(10));
                let started = std::time::Instant::now();
                let _guard = manager.sessions.write();
                started.elapsed()
            })
        };

        let queued = manager.drive_chunk_requests();
        assert_eq!(queued, 2, "both queued columns are streamed");

        let waited = waiter.join().expect("waiter thread");
        assert!(
            waited < std::time::Duration::from_millis(40),
            "a packet handler waited {waited:?} for the sessions lock while chunks were built"
        );
    }

    #[test]
    fn update_block_wire_is_0x15_blockpos_id_flags() {
        let wire = update_block_wire(1, 64, -3, 0xDEAD_BEEF);
        let mut reader = Reader::new(&wire);
        assert_eq!(reader.read_var_u32().unwrap(), UPDATE_BLOCK_PACKET_ID);
        // 0x15, not 0x0B (StartGame).
        assert_eq!(UPDATE_BLOCK_PACKET_ID, 21);
        assert_eq!(BlockPos::read(&mut reader).unwrap(), BlockPos::new(1, 64, -3));
        // `UpdateBlockPacket::encodePayload`: runtime id is an unsigned
        // var_u32, then flags and data layer as unsigned var_u32s.
        assert_eq!(reader.read_var_u32().unwrap(), 0xDEAD_BEEF);
        assert_eq!(reader.read_var_u32().unwrap(), UPDATE_BLOCK_FLAGS_NETWORK);
        assert_eq!(reader.read_var_u32().unwrap(), UPDATE_BLOCK_DATA_LAYER_NORMAL);
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

    fn scoreboard_fixture() -> Scoreboard {
        use crate::scoreboard::{ScoreboardObjective, ScoreboardSlot};
        let mut objective =
            ScoreboardObjective::new("main", "Números", ScoreboardSlot::Sidebar);
        objective.add_score(100, "Oro");
        objective.add_score(10, "Cobre");
        let mut board = Scoreboard::new("Economía");
        board.add_objective(objective);
        board
    }

    #[test]
    fn set_scoreboard_queues_bedrock_packets_and_stores_the_board() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, &[(0, 0)]));
        // `joined_session` names the player `Player_1`.
        let board = scoreboard_fixture();
        manager.set_scoreboard("Player_1", Some(board.clone()));

        assert_eq!(manager.get_scoreboard("Player_1"), Some(board.clone()));
        let map = manager.sessions.read();
        let session = &map[&1];
        assert_eq!(session.scoreboard, Some(board));
        let ids: Vec<u32> = session
            .pending_outgoing
            .iter()
            .map(OutgoingPacket::packet_id)
            .collect();
        // ScoreFactory order: 1 remove + 1 display + 1 score batch (no 0x70).
        assert_eq!(ids, vec![0x6A, 0x6B, 0x6C]);
    }

    #[test]
    fn remove_scoreboard_queues_remove_objective_per_objective() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, &[(0, 0)]));
        manager.set_scoreboard("Player_1", Some(scoreboard_fixture()));
        {
            let mut map = manager.sessions.write();
            map.get_mut(&1).unwrap().pending_outgoing.clear();
        }
        manager.set_scoreboard("Player_1", Some(Scoreboard::default()));
        let map = manager.sessions.read();
        let session = &map[&1];
        assert_eq!(session.pending_outgoing.len(), 1);
        assert_eq!(session.pending_outgoing[0].packet_id(), 0x6A);
    }

    #[test]
    fn plugin_action_set_scoreboard_applies_like_set_scoreboard() {
        use php_plugin_bridge::ScoreboardData;
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, &[(0, 0)]));
        let board = scoreboard_fixture();
        let actions = vec![PluginAction::SetScoreboard {
            target_player: "Player_1".to_string(),
            scoreboard: board.to_data(),
        }];
        manager.apply_actions(&actions, 0);
        assert_eq!(manager.get_scoreboard("Player_1"), Some(board));
        let _ = ScoreboardData::default();
    }

    #[test]
    fn java_frames_diff_modern_and_legacy_and_then_go_quiet() {
        let manager = SessionManager::new();
        manager.set_scoreboard("Steve", Some(scoreboard_fixture()));

        // Modern 775: objective + display + 2 scores = 4 frames.
        let modern = manager.take_scoreboard_java_frames("Steve", 775);
        assert_eq!(modern.len(), 4, "{modern:?}");
        // No change: nothing more to send.
        assert!(manager.take_scoreboard_java_frames("Steve", 775).is_empty());
        // Legacy 340 tracks its own sent state.
        let legacy = manager.take_scoreboard_java_frames("Steve", 340);
        assert_eq!(legacy.len(), 4, "{legacy:?}");
        assert!(manager.take_scoreboard_java_frames("Steve", 340).is_empty());

        // Removing the board frames one objective removal per protocol.
        manager.set_scoreboard("Steve", Some(Scoreboard::default()));
        assert_eq!(manager.take_scoreboard_java_frames("Steve", 775).len(), 1);
        assert_eq!(manager.take_scoreboard_java_frames("Steve", 340).len(), 1);
    }

    #[test]
    fn java_frames_update_existing_objective_does_not_recreate() {
        let manager = SessionManager::new();
        let mut board = scoreboard_fixture();
        manager.set_scoreboard("Steve", Some(board.clone()));

        // Initial frames for modern 775: create objective (action 0) + display + 2 scores = 4 frames.
        let initial = manager.take_scoreboard_java_frames("Steve", 775);
        assert_eq!(initial.len(), 4);

        // Bedrock player joins or plugin updates scoreline -> objective already exists!
        board.get_objective_mut("main").unwrap().add_score(50, "Plata");
        manager.set_scoreboard("Steve", Some(board));

        let update_frames = manager.take_scoreboard_java_frames("Steve", 775);
        // Only the new score is sent (1 frame), NO duplicate objective create!
        assert_eq!(update_frames.len(), 1);
        let profile = java_protocol::ModernProfile::for_protocol(775).unwrap();
        let (id, _, _) = java_bridge::wire::try_split_frame(&update_frames[0]).expect("frame");
        assert_eq!(id, profile.id("scoreboard_score"));
        assert_ne!(id, profile.id("scoreboard_objective"));
    }

    #[test]
    fn scoreboard_file_roundtrips_global_and_players() {
        let dir = std::env::temp_dir().join(format!(
            "pocketmine-scoreboard-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let manager = SessionManager::new();
        manager.enable_persistence(dir.clone());
        manager.set_global_scoreboard(Some(scoreboard_fixture()));
        manager.set_scoreboard("Steve", Some(scoreboard_fixture()));
        manager.save_scoreboards().unwrap();
        assert!(dir.join("scoreboard.json").exists());

        let restored = SessionManager::new();
        restored.enable_persistence(dir.clone());
        let count = restored.load_scoreboards();
        assert_eq!(count, 1);
        assert_eq!(
            restored.get_scoreboard("Steve"),
            Some(scoreboard_fixture())
        );
        std::fs::remove_dir_all(&dir).ok();
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

    /// Dispatches the `UseItem` (CLICK_BLOCK) packet a client sends when it
    /// right-clicks a block and returns the actions the session queued.
    fn right_click_actions(
        manager: &SessionManager,
        block_position: (i32, i32, i32),
        face: u8,
        held: ItemStack,
    ) -> Vec<PluginAction> {
        use bedrock_protocol::packets::inventory_transaction::{
            TransactionData, ACTION_CLICK_BLOCK, UseItemData,
        };

        let packet = InventoryTransactionPacket {
            request_id: 0,
            request_changed_slots: None,
            actions: vec![],
            data: TransactionData::UseItem(UseItemData {
                action_type: ACTION_CLICK_BLOCK,
                trigger_type: 0,
                block_position,
                face,
                hotbar_slot: 0,
                hand: 0,
                item_in_hand: held,
                player_position: (2.5, 80.06, 3.5),
                click_position: (2.5, 80.0, 3.5),
                block_runtime_id: 0,
                client_interact_prediction: 0,
                client_cooldown_state: 0,
            }),
        };
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        manager
            .handle_session_packet(1, 0x1E, &writer.into_bytes())
            .expect("use item");
        let mut map = manager.sessions.write();
        let session = map.get_mut(&1).expect("session");
        std::mem::take(&mut session.pending_actions)
    }

    /// PM `Lever::onInteract()`: the click toggles the lever and plays the
    /// redstone sound; nothing is placed next to it.
    #[test]
    fn bedrock_click_on_a_lever_toggles_it() {
        let manager = SessionManager::new();
        let mut placer = joined_session(1, &[(0, 0)]);
        placer.world = Arc::clone(manager.world());
        manager.add_session(placer);

        let lever = bedrock_world::block_states::network_id("lever").unwrap();
        assert!(manager.world().set_block(2, 80, 3, lever));
        let on = bedrock_world::block_states::with_property(lever, "open_bit", "true").unwrap();

        let actions = right_click_actions(&manager, (2, 80, 3), 1, ItemStack::new(1, 1));
        // `onInteract()` edits the world inline, like PM's `World::setBlock`.
        assert_eq!(manager.world().block_at(2, 80, 3), on);
        assert_eq!(
            actions,
            vec![PluginAction::BroadcastSound {
                sound: LEVEL_SOUND_POWER_ON.to_string(),
                extra_data: -1,
                x: 2,
                y: 80,
                z: 3,
            }]
        );

        // The held block was not placed on the face that was clicked.
        assert_eq!(manager.world().block_at(2, 81, 3), AIR);
    }

    /// PM `Button::onInteract()` + `onScheduledUpdate()`: press, `power.on`,
    /// and after `getActivationTime()` ticks the button releases itself with
    /// `power.off`.
    #[test]
    fn a_pressed_button_releases_itself_through_the_tick() {
        let manager = SessionManager::new();
        let mut placer = joined_session(1, &[(0, 0)]);
        placer.world = Arc::clone(manager.world());
        manager.add_session(placer);

        let button = bedrock_world::block_states::network_id("stone_button").unwrap();
        let pressed =
            bedrock_world::block_states::with_property(button, "button_pressed_bit", "true")
                .unwrap();
        assert!(manager.world().set_block(2, 80, 3, button));

        let actions = right_click_actions(&manager, (2, 80, 3), 1, ItemStack::new(1, 1));
        assert_eq!(manager.world().block_at(2, 80, 3), pressed);
        assert_eq!(
            actions,
            vec![PluginAction::BroadcastSound {
                sound: LEVEL_SOUND_POWER_ON.to_string(),
                extra_data: -1,
                x: 2,
                y: 80,
                z: 3,
            }]
        );
        assert_eq!(manager.world().scheduled_block_update_count(), 1);

        for _ in 0..19 {
            manager.tick();
        }
        assert_eq!(manager.world().block_at(2, 80, 3), pressed);

        manager.tick();
        assert_eq!(manager.world().block_at(2, 80, 3), button);
        assert_eq!(manager.world().scheduled_block_update_count(), 0);
        let power_off = {
            let map = manager.sessions.read();
            map.get(&1)
                .map(|session| {
                    session.pending_outgoing.iter().any(|packet| match packet {
                        OutgoingPacket::RawWire(bytes) => {
                            bytes.windows(9).any(|window| window == b"power.off")
                        }
                        _ => false,
                    })
                })
                .unwrap_or(false)
        };
        assert!(power_off, "the release must broadcast RedstonePowerOffSound");
    }

    /// PM `useItemOn`: sneaking with an item in hand skips `onInteract()` and
    /// places the block instead.
    #[test]
    fn sneaking_with_a_block_places_against_a_lever() {
        let manager = SessionManager::new();
        let mut placer = joined_session(1, &[(0, 0)]);
        placer.world = Arc::clone(manager.world());
        placer.is_sneaking = true;
        manager.add_session(placer);

        let lever = bedrock_world::block_states::network_id("lever").unwrap();
        assert!(manager.world().set_block(2, 80, 3, lever));

        let mut held = ItemStack::new(1, 1);
        held.block_runtime_id = 999;
        let actions = right_click_actions(&manager, (2, 80, 3), 1, held);
        assert_eq!(
            actions,
            vec![
                PluginAction::SetBlock {
                    world: "world".to_string(),
                    x: 2,
                    y: 81,
                    z: 3,
                    block_id: 999,
                },
                PluginAction::BroadcastSound {
                    sound: LEVEL_SOUND_PLACE.to_string(),
                    extra_data: 999,
                    x: 2,
                    y: 81,
                    z: 3,
                },
            ]
        );
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
            vec![
                PluginAction::SetBlock {
                    world: "world".to_string(),
                    x: 2,
                    y: 80,
                    z: 3,
                    block_id: 999,
                },
                // PM `BlockPlaceSound`: the place sound is broadcast too.
                PluginAction::BroadcastSound {
                    sound: LEVEL_SOUND_PLACE.to_string(),
                    extra_data: 999,
                    x: 2,
                    y: 80,
                    z: 3,
                },
            ]
        );
        manager.apply_actions(&actions, 0);

        // Manager world holds the real id.
        assert_eq!(manager.world().block_at(2, 80, 3), 999);
        // Fase 2 (Java -> Bedrock style, here Bedrock placer -> viewers):
        // exactly one UpdateBlock 0x15 with the real id, payload byte-exact.
        {
            let map = manager.sessions.read();
            let viewer = &map[&1];
            assert_eq!(viewer.pending_outgoing.len(), 2, "block + place sound");
            // The sound is broadcast while the action is applied, the block
            // update is flushed by the `poll_block_changes` at the end, so the
            // order is sound-then-update (assert by content, not index).
            assert!(viewer
                .pending_outgoing
                .contains(&OutgoingPacket::RawWire(update_block_wire(
                    2, 80, 3, 999
                ))));
            let mut sound = Vec::new();
            {
                let mut writer = Writer::new();
                writer.write_var_u32(LEVEL_SOUND_EVENT_PACKET_ID);
                sound.extend_from_slice(&writer.into_bytes());
            }
            sound.extend_from_slice(&level_sound_event_wire(
                LEVEL_SOUND_PLACE,
                2.5,
                80.5,
                3.5,
                999,
            ));
            assert!(viewer.pending_outgoing.contains(&OutgoingPacket::RawWire(sound)));
        }
        // Fase 1: the same real id is queued for the Java bridge.
        assert_eq!(
            manager.world().take_changes(),
            vec![BlockChange::new(2, 80, 3, 999)]
        );
        // A second poll re-sends nothing (no echo duplication).
        assert_eq!(manager.poll_block_changes(), 0);
    }

    /// PM `World::useItemOn` refuses a placement whose block boxes intersect an
    /// entity: a player standing inside the target cell must not be able to
    /// place a block into itself (the reported "player trapped inside the
    /// block"). The client's prediction is undone by resending the column.
    #[test]
    fn bedrock_use_item_place_inside_the_player_is_refused() {
        use bedrock_protocol::packets::inventory_transaction::{
            TransactionData, ACTION_CLICK_BLOCK, UseItemData,
        };
        use bedrock_protocol::ItemStack;

        let manager = SessionManager::new();
        let mut placer = joined_session(1, &[(0, 0)]);
        placer.world = Arc::clone(manager.world());
        // Standing in (2,80,3): the own hitbox overlaps the target cell.
        placer.position = Vector3f::new(2.5, 80.0, 3.5);
        manager.add_session(placer);

        let mut held = ItemStack::new(1, 1);
        held.block_runtime_id = bedrock_world::block_id::STONE;
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
                player_position: (2.5, 80.0, 3.5),
                click_position: (2.5, 80.0, 3.5),
                block_runtime_id: 123,
                client_interact_prediction: 0,
                client_cooldown_state: 0,
            }),
        };
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let payload = writer.into_bytes().to_vec();
        let out = manager
            .handle_session_packet(1, 0x1E, &payload)
            .expect("use item");

        {
            let mut map = manager.sessions.write();
            let session = map.get_mut(&1).expect("placer session");
            assert!(
                session.pending_actions.is_empty(),
                "no SetBlock/sound for a refused placement: {:?}",
                session.pending_actions
            );
            // The refused cell is no longer marked as sent, so the column is
            // streamed again and the client's predicted block disappears.
            assert!(!session.sent_chunks.contains(&(0, 0)));
        }
        assert!(
            out.iter()
                .any(|outgoing| matches!(outgoing, OutgoingPacket::LevelChunk(_))),
            "the predicted ghost block is undone with the column"
        );
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

    #[test]
    fn unified_tick_advances_world_and_forwards_block_once() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, &[(0, 0)]));
        let start_tick = manager.current_tick();
        let start_time = manager.world().time();

        // A Java-style host edit with no plugin action must reach the
        // Bedrock viewer on the unified tick, exactly once.
        assert!(manager.world().set_block(2, 64, 3, 77));
        let report = manager.tick();
        assert_eq!(report.tick, start_tick + 1);
        assert_eq!(manager.current_tick(), report.tick);
        assert_eq!(report.world_time, start_time + 1);
        assert_eq!(manager.world().time(), start_time + 1);
        assert_eq!(report.block_changes, 1);
        assert_eq!(report.virtual_queued, 0);
        assert_eq!(report.prespawn_forced, 0);
        {
            let map = manager.sessions.read();
            let viewer = &map[&1];
            assert_eq!(viewer.pending_outgoing.len(), 1);
            assert_eq!(viewer.pending_outgoing[0].packet_id(), UPDATE_BLOCK_PACKET_ID);
            assert_eq!(
                viewer.pending_outgoing[0],
                OutgoingPacket::RawWire(update_block_wire(2, 64, 3, 77))
            );
        }
        // Second tick with no new edits sends nothing (idempotent poll).
        let report = manager.tick();
        assert_eq!(report.block_changes, 0);
        assert_eq!(report.world_time, start_time + 2);
        // Fase 1 queue untouched by the Bedrock tick: Java still gets it.
        assert_eq!(
            manager.world().take_changes(),
            vec![BlockChange::new(2, 64, 3, 77)]
        );
    }

    #[test]
    fn java_break_via_hooks_reaches_bedrock_viewer_on_tick() {
        use crate::java_hooks::CoreHooks;
        use java_bridge::ServerHooks;

        let manager = Arc::new(SessionManager::new());
        manager.add_session(joined_session(1, &[(0, 0)]));
        let hooks = CoreHooks::new(
            Arc::clone(&manager),
            "test".to_string(),
            12,
            0,
            "flat".to_string(),
        );

        // Seed a solid block and drain both sync queues, so only the
        // break below is forwarded.
        assert!(manager.world().set_block(2, 64, 3, 999));
        assert_eq!(manager.poll_block_changes(), 1);
        assert_eq!(manager.world().take_changes().len(), 1);
        manager
            .sessions
            .write()
            .get_mut(&1)
            .unwrap()
            .pending_outgoing
            .clear();

        // Java breaks it through the production hook (`world.set_block`).
        assert!(hooks.break_block(2, 64, 3));
        assert_eq!(manager.world().block_at(2, 64, 3), AIR);

        // `World::useBreakOn` order: the destroy particles go out with the
        // break itself, and the tick forwards the AIR edit as a single
        // `UpdateBlockPacket` (the `changedBlocks` flush never repeats the
        // particles, which used to arrive twice, one tick apart).
        let report = manager.tick();
        assert_eq!(report.block_changes, 1);
        {
            let map = manager.sessions.read();
            let viewer = &map[&1];
            assert_eq!(viewer.pending_outgoing.len(), 2);
            assert_eq!(
                viewer.pending_outgoing[0].packet_id(),
                LEVEL_EVENT_PACKET_ID
            );
            assert_eq!(
                viewer.pending_outgoing[1],
                OutgoingPacket::RawWire(update_block_wire(2, 64, 3, AIR))
            );
        }
        // Fase 1 queue untouched by the Bedrock tick: Java viewers still
        // get the same edit through `drain_block_changes`.
        let drained = hooks.drain_block_changes();
        assert_eq!(drained.len(), 1);
        assert_eq!((drained[0].x, drained[0].y, drained[0].z), (2, 64, 3));
        assert_eq!(drained[0].network_id, Some(AIR));
        assert_eq!(drained[0].kind, "air");
    }

    #[test]
    fn bedrock_break_reaches_java_drain_with_air_id() {
        use crate::java_hooks::CoreHooks;
        use java_bridge::ServerHooks;

        let manager = Arc::new(SessionManager::new());
        manager.add_session(joined_session(1, &[(0, 0)]));
        let hooks = CoreHooks::new(
            Arc::clone(&manager),
            "test".to_string(),
            12,
            0,
            "flat".to_string(),
        );

        assert!(manager.world().set_block(2, 64, 3, 999));
        assert_eq!(manager.poll_block_changes(), 1);
        assert_eq!(hooks.drain_block_changes().len(), 1);

        // Bedrock breaks it (`handle_block_actions` -> `SetBlock(AIR)`).
        let actions = vec![PluginAction::SetBlock {
            world: "world".to_string(),
            x: 2,
            y: 64,
            z: 3,
            block_id: AIR,
        }];
        manager.apply_actions(&actions, 0);
        assert_eq!(manager.world().block_at(2, 64, 3), AIR);

        // The Java-facing queue carries the same edit with the AIR id.
        let drained = hooks.drain_block_changes();
        assert_eq!(drained.len(), 1);
        assert_eq!((drained[0].x, drained[0].y, drained[0].z), (2, 64, 3));
        assert_eq!(drained[0].network_id, Some(AIR));
        assert_eq!(drained[0].kind, "air");
    }

    #[test]
    fn viewer_without_chunk_misses_update_but_chunk_carries_edit() {
        let manager = SessionManager::new();
        // Session 1 only holds chunk (9, 9); the edit lands in (0, 0).
        manager.add_session(joined_session(1, &[(9, 9)]));

        assert!(manager.world().set_block(1, 64, 1, 999));
        assert_eq!(manager.poll_block_changes(), 0);
        assert_eq!(manager.world().take_changes().len(), 1);

        // Break it while session 1 has no (0, 0) in view: no UpdateBlock
        // recipient and no LevelChunk fallback (same gate), so nothing is
        // queued for the viewer right now.
        assert!(manager.world().set_block(1, 64, 1, AIR));
        assert_eq!(manager.poll_block_changes(), 0);
        assert!(manager.sessions.read()[&1].pending_outgoing.is_empty());
        assert_eq!(manager.world().take_changes().len(), 1);

        // The chunk generated afterwards (what the viewer will stream)
        // already carries the edit, so the viewer converges without ever
        // receiving an UpdateBlockPacket.
        let chunk = manager.world().get_chunk(0, 0);
        assert_eq!(chunk.get_block(1, 64, 1), AIR);
    }

    fn action(action_type: i32, pos: (i32, i32, i32), face: i32) -> PlayerBlockAction {
        PlayerBlockAction {
            action_type,
            position: Some(pos),
            face: Some(face),
        }
    }

    /// PM `Player::attackBlock` (survival): START_BREAK creates the
    /// `SurvivalBlockBreakHandler` and broadcasts `BLOCK_START_BREAK` with
    /// `65535 * breakSpeed`, which is what lets the client render the dig
    /// progress and eventually send `PREDICT_DESTROY_BLOCK`.
    #[test]
    fn survival_start_break_creates_handler_and_broadcasts_start_break() {
        let mut session = joined_session(1, &[(0, 0)]);
        session.gamemode = 0; // survival
        session.position = Vector3f::new(0.5, 4.62, 0.5);

        session.apply_block_actions(vec![action(BLOCK_ACTION_START_BREAK, (0, 3, 0), 1)]);

        let handler = session
            .break_handler
            .as_ref()
            .expect("survival START_BREAK must create the dig handler");
        assert_eq!(handler.pos, (0, 3, 0));
        // Grass (hardness 0.6), bare hand compatible (x1.5) -> 0.9 s = 18
        // ticks -> 1/18 progress per tick.
        assert!((handler.break_speed - 1.0 / 18.0).abs() < 1e-6);

        let queued = session
            .pending_actions
            .iter()
            .find_map(|action| match action {
                PluginAction::BroadcastLevelEvent {
                    event_id,
                    event_data,
                    x,
                    y,
                    z,
                } => Some((*event_id, *event_data, *x, *y, *z)),
                _ => None,
            })
            .expect("BLOCK_START_BREAK must be queued");
        assert_eq!(queued.0, LEVEL_EVENT_BLOCK_START_BREAK);
        assert_eq!(queued.1, (65535.0 / 18.0) as i32);
        assert_eq!((queued.2, queued.3, queued.4), (0, 3, 0));
    }

    /// A creative client that still *digs* (instead of predicting the break
    /// immediately) must get instabuild progress: the handler is created with
    /// `break_speed = 1.0` and `BLOCK_START_BREAK` carries the maximum speed.
    /// PM skips the handler in creative because its client predicts the
    /// break; a client whose creative abilities have not settled digs, and
    /// dropping the dig left every break unsynced.
    #[test]
    fn creative_start_break_serves_instant_dig_progress() {
        let mut session = joined_session(1, &[(0, 0)]);
        session.gamemode = 1; // creative

        session.apply_block_actions(vec![action(BLOCK_ACTION_START_BREAK, (0, 3, 0), 1)]);

        let handler = session
            .break_handler
            .as_ref()
            .expect("creative dig must still get a handler");
        assert!((handler.break_speed - 1.0).abs() < 1e-6);
        let speed = session
            .pending_actions
            .iter()
            .find_map(|action| match action {
                PluginAction::BroadcastLevelEvent {
                    event_id,
                    event_data,
                    ..
                } if *event_id == LEVEL_EVENT_BLOCK_START_BREAK => Some(*event_data),
                _ => None,
            })
            .expect("BLOCK_START_BREAK must be queued");
        assert_eq!(speed, 65535);
    }

    /// CONTINUE_DESTROY on the same target does not restart the dig and does
    /// not fire a second left-click (PM `$lastBlockAttacked` dedup).
    #[test]
    fn continue_destroy_on_same_block_is_ignored() {
        let mut session = joined_session(1, &[(0, 0)]);
        session.gamemode = 0;
        session.position = Vector3f::new(0.5, 4.62, 0.5);

        session.apply_block_actions(vec![action(BLOCK_ACTION_START_BREAK, (0, 3, 0), 1)]);
        session.break_handler.as_mut().unwrap().break_progress = 0.4;
        session.pending_actions.clear();

        session.apply_block_actions(vec![action(
            BLOCK_ACTION_CONTINUE_DESTROY,
            (0, 3, 0),
            1,
        )]);

        let handler = session.break_handler.as_ref().expect("dig continues");
        assert!((handler.break_progress - 0.4).abs() < 1e-6);
        assert!(session.pending_actions.is_empty());
    }

    /// STOP_BREAK drops the handler and queues `BLOCK_STOP_BREAK`
    /// (PM handler `__destruct`), resetting the client's crack.
    #[test]
    fn stop_break_drops_handler_and_broadcasts_stop_break() {
        let mut session = joined_session(1, &[(0, 0)]);
        session.gamemode = 0;
        session.break_handler = Some(BreakHandlerState {
            pos: (0, 3, 0),
            targeted_face: 1,
            break_speed: 0.5,
            break_progress: 0.2,
            fx_ticker: 0,
            max_player_distance: 16,
        });

        session.apply_block_actions(vec![action(BLOCK_ACTION_STOP_BREAK, (0, 3, 0), 1)]);

        assert!(session.break_handler.is_none());
        assert!(session.pending_actions.iter().any(|action| matches!(
            action,
            PluginAction::BroadcastLevelEvent { event_id, .. }
                if *event_id == LEVEL_EVENT_BLOCK_STOP_BREAK
        )));
    }

    /// PREDICT_DESTROY is the authoritative break: it queues AIR for the
    /// world and drops the handler, broadcasting `BLOCK_STOP_BREAK`
    /// (PM `breakBlock` calls `stopBreakBlock` first).
    #[test]
    fn predict_destroy_sets_air_and_stops_handler() {
        let mut session = joined_session(1, &[(0, 0)]);
        session.gamemode = 0;
        session.break_handler = Some(BreakHandlerState {
            pos: (0, 3, 0),
            targeted_face: 1,
            break_speed: 0.5,
            break_progress: 0.9,
            fx_ticker: 0,
            max_player_distance: 16,
        });

        session.apply_block_actions(vec![action(
            BLOCK_ACTION_PREDICT_DESTROY,
            (0, 3, 0),
            1,
        )]);

        assert!(session.break_handler.is_none());
        assert!(session
            .pending_actions
            .iter()
            .any(|action| matches!(action, PluginAction::SetBlock { block_id, .. } if *block_id == AIR)));
        assert!(session.pending_actions.iter().any(|action| matches!(
            action,
            PluginAction::BroadcastLevelEvent { event_id, .. }
                if *event_id == LEVEL_EVENT_BLOCK_STOP_BREAK
        )));
        assert!(session.pending_actions.iter().any(|action| matches!(
            action,
            PluginAction::BroadcastLevelEvent { event_id, .. }
                if *event_id == LEVEL_EVENT_PARTICLE_DESTROY_BLOCK
        )));
    }

    /// The central tick advances dig progress and drops the handler at
    /// completion (PM `update()` returns false -> `$blockBreakHandler =
    /// null` -> `__destruct` broadcasts `BLOCK_STOP_BREAK`).
    #[test]
    fn tick_advances_break_handler_and_stops_at_completion() {
        let manager = SessionManager::new();
        let mut session = joined_session(1, &[(0, 0)]);
        session.gamemode = 0;
        session.position = Vector3f::new(0.5, 4.62, 0.5);
        session.break_handler = Some(BreakHandlerState {
            pos: (0, 3, 0),
            targeted_face: 1,
            break_speed: 0.5,
            break_progress: 0.0,
            fx_ticker: 0,
            max_player_distance: 16,
        });
        manager.add_session(session);

        manager.tick(); // +0.5
        {
            let map = manager.sessions.read();
            let handler = map[&1].break_handler.as_ref().expect("still digging");
            assert!((handler.break_progress - 0.5).abs() < 1e-6);
        }
        manager.tick(); // +0.5 -> complete
        {
            let map = manager.sessions.read();
            assert!(map[&1].break_handler.is_none());
            assert!(map[&1].pending_outgoing.iter().any(|packet| {
                let OutgoingPacket::RawWire(bytes) = packet else {
                    return false;
                };
                bytes.first() == Some(&(LEVEL_EVENT_PACKET_ID as u8))
            }));
        }
        // Completion is server-authoritative: the block is gone in the world
        // (and therefore enqueued for both Bedrock and Java sync).
        assert_eq!(manager.world().block_at(0, 3, 0), AIR);
    }

    /// Walking away drops the handler (PM distance gate).
    #[test]
    fn tick_drops_break_handler_when_player_walks_away() {
        let manager = SessionManager::new();
        let mut session = joined_session(1, &[(0, 0)]);
        session.gamemode = 0;
        session.position = Vector3f::new(500.0, 4.62, 500.0);
        session.break_handler = Some(BreakHandlerState {
            pos: (0, 3, 0),
            targeted_face: 1,
            break_speed: 0.5,
            break_progress: 0.0,
            fx_ticker: 0,
            max_player_distance: 16,
        });
        manager.add_session(session);
        manager.tick();
        let map = manager.sessions.read();
        assert!(map[&1].break_handler.is_none());
        assert!(map[&1].pending_outgoing.iter().any(|packet| {
            let OutgoingPacket::RawWire(bytes) = packet else {
                return false;
            };
            bytes.first() == Some(&(LEVEL_EVENT_PACKET_ID as u8))
        }));
    }

    #[test]
    fn tick_drives_chunk_requests_without_inbound_packets() {
        // `NetworkSession::tick()` -> `Player::doChunkRequests()`: a
        // PreSpawn session that received `RequestChunkRadius` (radius 10
        // clamped from the client's 80) must stream PublisherUpdate +
        // `CHUNK_SENDING_PER_TICK` LevelChunks per central tick, with no
        // inbound packet ever arriving (the client sits in
        // "Locating/Generating the world" until it sees terrain).
        let manager = SessionManager::new();
        let addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
        let mut session = PlayerSession::new(1, addr);
        session.state = SessionState::PreSpawn;
        session.view_distance_received = true;
        session.chunk_radius = 10;
        session.next_chunk_order_run = 0;
        manager.add_session(session);
        // A session that never received a view distance is skipped.
        let idle_addr: SocketAddr = "127.0.0.1:19133".parse().unwrap();
        let mut idle = PlayerSession::new(2, idle_addr);
        idle.state = SessionState::PreSpawn;
        manager.add_session(idle);

        let report = manager.tick();
        assert_eq!(report.chunk_packets_queued, 1 + CHUNK_SENDING_PER_TICK);
        {
            let map = manager.sessions.read();
            let viewer = &map[&1];
            assert_eq!(
                viewer.pending_outgoing.len(),
                1 + CHUNK_SENDING_PER_TICK
            );
            // PublisherUpdate precedes the chunks it unlocks.
            assert!(matches!(
                viewer.pending_outgoing[0],
                OutgoingPacket::NetworkChunkPublisherUpdate(_)
            ));
            assert_eq!(
                viewer.pending_outgoing
                    .iter()
                    .filter(|p| matches!(p, OutgoingPacket::LevelChunk(_)))
                    .count(),
                CHUNK_SENDING_PER_TICK
            );
            assert_eq!(viewer.sent_chunks.len(), CHUNK_SENDING_PER_TICK);
            assert!(map[&2].pending_outgoing.is_empty());
            assert!(map[&2].sent_chunks.is_empty());
        }

        // The tick flush (`flushGamePacketQueue`) encodes what the tick
        // queued and empties the queue; the next tick queues the next
        // batch of 4 with no inbound packet in between.
        let drained = manager.drain_pending_outgoing_bytes();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].0, addr);
        assert!(!drained[0].1.is_empty());
        assert!(manager.sessions.read()[&1].pending_outgoing.is_empty());

        let report = manager.tick();
        assert_eq!(report.chunk_packets_queued, CHUNK_SENDING_PER_TICK);
        let map = manager.sessions.read();
        assert_eq!(map[&1].sent_chunks.len(), 2 * CHUNK_SENDING_PER_TICK);
        assert_eq!(
            map[&1]
                .pending_outgoing
                .iter()
                .filter(|p| matches!(p, OutgoingPacket::LevelChunk(_)))
                .count(),
            CHUNK_SENDING_PER_TICK
        );
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
                        && packet.entries[0].username == ".Procatin0"
            ));
            let OutgoingPacket::AddPlayer(packet) = &viewer.pending_outgoing[1] else {
                panic!("expected AddPlayer for virtual");
            };
            assert_eq!(packet.username, ".Procatin0");
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

        // Small moves go on the interpolated delta channel
        // (`MoveActorDeltaPacket`, `ticks = 1`), not the snap absolute one.
        move_virtual(&manager, "Procatin0", 4.5, 64.0, -1.5);
        assert_eq!(manager.poll_virtual_players(), 1);
        let map = manager.sessions.read();
        let viewer = &map[&1];
        let OutgoingPacket::MoveActorDelta(packet) = &viewer.pending_outgoing[2] else {
            panic!("expected MoveActorDelta for virtual move");
        };
        assert_eq!(packet.actor_runtime_id, 10_000_000);
        assert_eq!(packet.ticks, 1);
        assert!(!packet.teleport);
        // Eye position (feet + 1.62), like Bedrock players.
        assert_eq!(packet.x_pos, Some(4.5));
        assert_eq!(packet.y_pos, Some(64.0 + EYE_HEIGHT));
        assert_eq!(packet.z_pos, Some(-1.5));

        // Long jumps stay on the absolute channel (teleport).
        drop(map);
        move_virtual(&manager, "Procatin0", 40.5, 64.0, -1.5);
        assert_eq!(manager.poll_virtual_players(), 1);
        let map = manager.sessions.read();
        let viewer = &map[&1];
        let OutgoingPacket::MoveActorAbsolute(packet) = &viewer.pending_outgoing[3] else {
            panic!("expected MoveActorAbsolute for virtual teleport");
        };
        assert_eq!(packet.actor_runtime_id, 10_000_000);
        assert_eq!(
            packet.position,
            Vector3f::new(40.5, 64.0 + EYE_HEIGHT, -1.5)
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
                OutgoingPacket::AddPlayer(packet) if packet.username == ".Procatin0"
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

        // One Java move must emit one movement packet for EVERY viewer,
        // not just the first one (`mark_sent` used to run per-viewer, so
        // the second Bedrock client kept seeing a frozen Java body).
        // Small moves use the interpolated delta channel.
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
            let OutgoingPacket::MoveActorDelta(packet) = &viewer.pending_outgoing[2] else {
                panic!("expected MoveActorDelta for viewer {id}");
            };
            assert_eq!(packet.actor_runtime_id, 10_000_000);
            assert_eq!(packet.x_pos, Some(4.5));
            assert_eq!(packet.y_pos, Some(64.0 + EYE_HEIGHT));
            assert_eq!(packet.z_pos, Some(-1.5));
        }
        // Idempotent: the move was delivered to all viewers.
        assert_eq!(manager.poll_virtual_players(), 0);
    }

    #[test]
    fn virtual_sneak_toggle_emits_set_actor_data_with_sneak_flag() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Steve"));
        register_virtual(&manager, "Procatin0", 2.5, 63.0, -1.5);
        assert_eq!(manager.poll_virtual_players(), 2);

        let set_sneak = |manager: &SessionManager, sneaking: bool| {
            let mut player = manager.registry().get("Procatin0").expect("virtual registered");
            player.is_sneaking = sneaking;
            manager.registry().upsert(player);
            manager.poll_virtual_players()
        };
        let flags_of = |manager: &SessionManager| -> i64 {
            let map = manager.sessions.read();
            let viewer = &map[&1];
            let packet = viewer
                .pending_outgoing
                .iter()
                .rev()
                .find_map(|packet| match packet {
                    OutgoingPacket::SetActorData(packet) => Some(packet),
                    _ => None,
                })
                .expect("SetActorData for sneak toggle");
            let flags = packet
                .metadata
                .iter()
                .find(|entry| entry.key == 0)
                .expect("flags metadata entry");
            match flags.value {
                MetadataValue::Long(value) => value,
                _ => panic!("flags must be Long"),
            }
        };

        // Java holds shift: one SetActorData carrying the sneak flag.
        assert_eq!(set_sneak(&manager, true), 1);
        // SNEAKING is bit 1 (value 2) of the data-flags long.
        assert_eq!(flags_of(&manager) & FLAG_SNEAKING, FLAG_SNEAKING);
        assert_eq!(manager.poll_virtual_players(), 0);

        // Releasing shift: one more without the flag.
        assert_eq!(set_sneak(&manager, false), 1);
        assert_eq!(flags_of(&manager) & FLAG_SNEAKING, 0);
        assert_eq!(manager.poll_virtual_players(), 0);
    }

    #[test]
    fn bedrock_view_names_are_verbatim_for_both_platforms() {
        // Geyser/Floodgate-style: Java -> Bedrock TAB/AddPlayer/nametag
        // carry the Java nick as-is (no prefix), and a Bedrock player's own
        // packets are equally verbatim for Bedrock viewers. Only the Java
        // viewer side prefixes Bedrock names (see `java_bridge::display`).
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Steve"));
        register_virtual(&manager, "Procatin0", 2.5, 63.0, -1.5);
        assert_eq!(manager.poll_virtual_players(), 2);
        let map = manager.sessions.read();
        let viewer = &map[&1];
        // Virtual Java player: PlayerList + AddPlayer both verbatim.
        let OutgoingPacket::PlayerList(list) = &viewer.pending_outgoing[0] else {
            panic!("expected PlayerList for virtual");
        };
        assert_eq!(list.entries[0].username, ".Procatin0");
        assert!(list.entries[0].username.starts_with('.'));
        let OutgoingPacket::AddPlayer(add) = &viewer.pending_outgoing[1] else {
            panic!("expected AddPlayer for virtual");
        };
        assert_eq!(add.username, ".Procatin0");
        let nametag = add
            .metadata
            .iter()
            .find(|entry| entry.key == 4)
            .expect("metadata 4 nametag");
        assert!(matches!(&nametag.value, MetadataValue::String(s) if s == ".Procatin0"));
        // Bedrock player's own PlayerList/AddPlayer are verbatim too.
        let session = PlayerSession::new(7, "127.0.0.1:19132".parse().unwrap());
        let mut bedrock = session;
        bedrock.username = "Alex".to_string();
        assert_eq!(bedrock.player_list_addition().username, "Alex");
        let bedrock_add = bedrock.add_player_packet();
        assert_eq!(bedrock_add.username, "Alex");
        let nametag = bedrock_add
            .metadata
            .iter()
            .find(|entry| entry.key == 4)
            .expect("metadata 4 nametag (bedrock)");
        assert!(matches!(&nametag.value, MetadataValue::String(s) if s == "Alex"));
        assert_eq!(bedrock.bedrock_display_name(), "Alex");
    }

    #[test]
    fn broadcast_text_delivers_verbatim_to_all() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Steve"));
        register_virtual(&manager, "Procatin0", 2.5, 63.0, -1.5);
        // Bedrock sender chat: Bedrock sees raw, Java sees raw (Bedrock has no prefix).
        manager.broadcast_text("<Steve> hola");
        {
            let map = manager.sessions.read();
            let viewer = &map[&1];
            let text = viewer
                .pending_outgoing
                .iter()
                .find_map(|packet| match packet {
                    OutgoingPacket::Text(text) => Some(text.message.clone()),
                    _ => None,
                })
                .expect("bedrock text packet");
            assert_eq!(text, "<Steve> hola");
        }
        assert_eq!(
            crate::chat::deliver_plugin_messages(&manager, "Procatin0"),
            vec!["<Steve> hola".to_string()]
        );
        // Join/scoreboard-style lines follow the same rule.
        manager.broadcast_text("Steve joined the game");
        manager.broadcast_text("Nombre: Steve");
        assert_eq!(
            crate::chat::deliver_plugin_messages(&manager, "Procatin0"),
            vec![
                "Steve joined the game".to_string(),
                "Nombre: Steve".to_string()
            ]
        );
        // Java sender: verbatim for both audiences.
        manager.broadcast_text("<.Procatin0> hey");
        assert_eq!(
            crate::chat::deliver_plugin_messages(&manager, "Procatin0"),
            vec!["<.Procatin0> hey".to_string()]
        );
    }

    #[test]
    fn java_scoreboard_frames_and_bedrock_packets_keep_bedrock_names_verbatim() {
        use crate::scoreboard::{Scoreboard, ScoreboardObjective, ScoreboardSlot};
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Steve"));
        let mut objective = ScoreboardObjective::new("main", "Números", ScoreboardSlot::Sidebar);
        objective.add_score(100, "Nombre: Steve");
        let mut board = Scoreboard::new("Economía");
        board.add_objective(objective);
        manager.set_scoreboard("Procatin0", Some(board.clone()));
        // Bedrock packets for a Bedrock viewer keep the verbatim line.
        let bedrock_board = board.clone();
        let bedrock_packets = bedrock_board.bedrock_packets();
        let bedrock_texts: Vec<String> = match &bedrock_packets[2] {
            crate::scoreboard::PacketBytes::SetScore(packet) => packet
                .entries
                .iter()
                .map(|entry| entry.custom_name.clone().unwrap_or_default())
                .collect(),
            other => panic!("expected SetScore, got {other:?}"),
        };
        assert_eq!(bedrock_texts, vec!["Nombre: Steve".to_string()]);
        // Java frames for the virtual viewer also keep the verbatim line (no dot for Bedrock).
        let frames = manager.take_scoreboard_java_frames("Procatin0", 775);
        let joined: Vec<u8> = frames.concat();
        assert!(
            joined
                .windows(b"Nombre: Steve".len())
                .any(|window| window == b"Nombre: Steve"),
            "java frames must contain the verbatim line: {frames:?}"
        );
        assert!(
            manager.take_scoreboard_java_frames("Procatin0", 775).is_empty(),
            "no change: no more frames"
        );
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

#[cfg(test)]
mod bossbar_tests {
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

    fn boss_event_ids(session: &PlayerSession) -> Vec<(u8, i64)> {
        session
            .pending_outgoing
            .iter()
            .filter_map(|packet| match packet {
                OutgoingPacket::BossEvent(packet) => {
                    Some((packet.event_type, packet.boss_actor_unique_id))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn bossbar_create_add_set_remove_flows_bedrock_packets() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Steve"));
        let boss_id = manager.bossbar_create();
        assert!(manager.bossbar_manager().contains(boss_id));

        assert!(manager.bossbar_add_player(boss_id, "Steve"));
        assert!(manager.bossbar_set_title(boss_id, "Hello world!"));
        assert!(manager.bossbar_set_subtitle(boss_id, "Foo Bar"));
        assert!(manager.bossbar_set_percentage(boss_id, 0.5));
        assert!(manager.bossbar_set_color(boss_id, 2));
        assert!(manager.bossbar_remove_player(boss_id, "Steve"));

        let map = manager.sessions.read();
        let session = &map[&1];
        // show, title, title, health, show (setColor re-sends show), hide.
        assert_eq!(
            boss_event_ids(session),
            vec![
                (BossEventPacket::TYPE_SHOW, boss_id),
                (BossEventPacket::TYPE_TITLE, boss_id),
                (BossEventPacket::TYPE_TITLE, boss_id),
                (BossEventPacket::TYPE_HEALTH_PERCENT, boss_id),
                (BossEventPacket::TYPE_SHOW, boss_id),
                (BossEventPacket::TYPE_HIDE, boss_id),
            ]
        );
        let bar = manager.bossbar_get(boss_id).expect("bar stored");
        assert_eq!(bar.full_title(), "Hello world!\n\nFoo Bar");
        assert_eq!(bar.percentage(), 0.5);
        assert_eq!(bar.color(), 2);
        assert!(bar.viewers().is_empty());
    }

    #[test]
    fn bossbar_style_flags_entity_and_queries() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Steve"));
        let boss_id = manager.bossbar_create_for_entity(77, "Steve");
        assert_eq!(boss_id, 77);

        assert!(manager.bossbar_add_player(boss_id, "Steve"));
        assert!(manager.bossbar_set_style(boss_id, 7, 5));
        assert!(manager.bossbar_set_flags(boss_id, 0x01));
        assert!(manager.bossbar_set_entity(boss_id, Some("Boss".to_string())));
        assert_eq!(
            manager.bossbar_get(boss_id).expect("bar").entity(),
            Some("Boss")
        );
        assert!(manager.bossbar_reset_entity(boss_id, false));
        assert_eq!(manager.bossbar_get(boss_id).expect("bar").entity(), None);

        // TYPE_QUERY resends the whole bar; register just logs.
        let query = manager.handle_boss_event(
            1,
            BossEventPacket::query(boss_id, manager.sessions.read()[&1].entity_unique_id),
        );
        assert_eq!(query.len(), 1);
        assert!(matches!(
            &query[0],
            OutgoingPacket::BossEvent(packet)
                if packet.event_type == BossEventPacket::TYPE_SHOW
                    && packet.boss_actor_unique_id == boss_id
        ));
        let register = manager.handle_boss_event(
            1,
            BossEventPacket::register_player(boss_id, 1),
        );
        assert!(register.is_empty());
        assert!(!manager.bossbar_add_player(9_999_999, "Steve"));
    }

    #[test]
    fn bossbar_diverse_per_player_overrides_reach_each_viewer() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Steve"));
        manager.add_session(joined_session(2, "Alex"));
        let boss_id = manager.bossbar_create_diverse();
        manager.bossbar_get_diverse(boss_id); // exists as diverse
        assert!(manager.bossbar_add_player(boss_id, "Steve"));
        assert!(manager.bossbar_add_player(boss_id, "Alex"));
        assert!(manager.bossbar_set_title_for(
            boss_id,
            &["Steve".to_string()],
            "Steve's bar"
        ));
        assert!(manager.bossbar_set_percentage_for(
            boss_id,
            &["Steve".to_string()],
            0.25
        ));

        let map = manager.sessions.read();
        let steve = &map[&1];
        let titles: Vec<String> = steve
            .pending_outgoing
            .iter()
            .filter_map(|packet| match packet {
                OutgoingPacket::BossEvent(packet)
                    if packet.event_type == BossEventPacket::TYPE_TITLE =>
                {
                    Some(packet.title.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(titles, vec!["Steve's bar".to_string()]);
        let health: Vec<f32> = steve
            .pending_outgoing
            .iter()
            .filter_map(|packet| match packet {
                OutgoingPacket::BossEvent(packet)
                    if packet.event_type == BossEventPacket::TYPE_HEALTH_PERCENT =>
                {
                    Some(packet.health_percent)
                }
                _ => None,
            })
            .collect();
        assert_eq!(health, vec![0.25]);
        // Alex never received Steve's override title.
        let alex = &map[&2];
        assert!(alex
            .pending_outgoing
            .iter()
            .all(|packet| !matches!(
                packet,
                OutgoingPacket::BossEvent(packet)
                    if packet.event_type == BossEventPacket::TYPE_TITLE
                        && packet.title == "Steve's bar"
            )));
        drop(map);
        // resetFor(steve) re-shows Steve with shared defaults.
        assert!(manager.bossbar_reset_for(boss_id, "Steve"));
    }

    #[test]
    fn bossbar_apply_data_diffs_and_java_poll_delivers_frames() {
        let manager = SessionManager::new();
        // Virtual (Java) player: no Bedrock session, registry only.
        let mut virtual_player =
            PlayerContext::new("Procatin0", "127.0.0.1:25565".parse().unwrap());
        virtual_player.is_virtual = true;
        manager.registry().upsert(virtual_player);

        let data = BossBarData {
            boss_id: 424242,
            title: "Boss".to_string(),
            percentage: 0.75,
            color: 2,
            players: vec!["Procatin0".to_string()],
            ..Default::default()
        };
        manager.bossbar_apply_data(&data);
        // Legacy 340: first poll ADDs, second poll is silent.
        let first = manager.take_bossbar_java_frames("Procatin0", 340);
        assert_eq!(first.len(), 1);
        let (id, _, _) =
            java_bridge::wire::try_split_frame(&first[0].frame).expect("frame");
        assert_eq!(id, 0x0C);
        assert!(manager.take_bossbar_java_frames("Procatin0", 340).is_empty());
        // Update percentage: UPDATE_HEALTH (action 2) delta.
        let mut updated = data.clone();
        updated.percentage = 0.5;
        manager.bossbar_apply_data(&updated);
        let delta = manager.take_bossbar_java_frames("Procatin0", 340);
        assert_eq!(delta.len(), 1);
        let (_, payload, _) =
            java_bridge::wire::try_split_frame(&delta[0].frame).expect("frame");
        assert_eq!(payload[16], 2, "UPDATE_HEALTH action");
        // Modern protocol ids differ (775 -> 0x09).
        manager.bossbar_destroy(424242);
        let removal = manager.take_bossbar_java_frames("Procatin0", 775);
        assert_eq!(removal.len(), 1);
        let (id, _, _) =
            java_bridge::wire::try_split_frame(&removal[0].frame).expect("frame");
        assert_eq!(id, 0x09);
        assert!(!manager.bossbar_manager().contains(424242));
    }

    #[test]
    fn bossbar_quit_cleanup_hides_every_bar() {
        let manager = SessionManager::new();
        manager.add_session(joined_session(1, "Steve"));
        let boss_id = manager.bossbar_create();
        assert!(manager.bossbar_add_player(boss_id, "Steve"));
        manager.bossbar_remove_viewer("Steve");
        let bar = manager.bossbar_get(boss_id).expect("bar stored");
        assert!(bar.viewers().is_empty());
        let map = manager.sessions.read();
        let events = boss_event_ids(&map[&1]);
        assert!(events.contains(&(BossEventPacket::TYPE_HIDE, boss_id)));
    }

    #[test]
    fn boss_event_packet_decodes_from_0x4a_payload() {
        let packet = BossEventPacket::show(
            42,
            "Hi",
            0.5,
            bedrock_protocol::BossBarColor::Red as u8,
            0,
        );
        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded = BossEventPacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn create_boss_event_wire_is_0x4a_plus_payload() {
        let session = joined_session(1, "Steve");
        let packet = BossEventPacket::hide(42);
        let wire = session.create_boss_event_wire(&packet);
        let mut reader = Reader::new(&wire);
        assert_eq!(reader.read_var_u32().expect("id"), 0x4A);
        let decoded = BossEventPacket::decode(&mut reader).expect("decode");
        assert_eq!(decoded, packet);
        assert_eq!(reader.remaining(), 0);
    }
}
