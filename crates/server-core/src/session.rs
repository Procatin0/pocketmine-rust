use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bedrock_protocol::binary::{CodecError, Reader, Writer};
use bedrock_protocol::{
    BlockPos, ChunkRadiusUpdatedPacket, DisconnectPacket, LevelChunkPacket, LevelSettings,
    LoginPacket, NetworkSettingsPacket, Packet, PacketBatch, PlayStatusPacket, RequestChunkRadiusPacket,
    RequestNetworkSettingsPacket, ResourcePackClientResponsePacket, ResourcePackStackPacket,
    ResourcePacksInfoPacket, StartGamePacket, Vector3f,
};
use bedrock_world::SubChunk;
use parking_lot::RwLock;
use thiserror::Error;
use tracing::{debug, info};

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
    ChunkRadiusUpdated(ChunkRadiusUpdatedPacket),
    LevelChunk(LevelChunkPacket),
    Disconnect(DisconnectPacket),
}

impl OutgoingPacket {
    pub fn packet_id(&self) -> u32 {
        match self {
            Self::NetworkSettings(p) => p.packet_id(),
            Self::PlayStatus(p) => p.packet_id(),
            Self::ResourcePacksInfo(p) => p.packet_id(),
            Self::ResourcePackStack(p) => p.packet_id(),
            Self::StartGame(p) => p.packet_id(),
            Self::ChunkRadiusUpdated(p) => p.packet_id(),
            Self::LevelChunk(p) => p.packet_id(),
            Self::Disconnect(p) => p.packet_id(),
        }
    }

    pub fn encode(&self, writer: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::NetworkSettings(p) => p.encode(writer),
            Self::PlayStatus(p) => p.encode(writer),
            Self::ResourcePacksInfo(p) => p.encode(writer),
            Self::ResourcePackStack(p) => p.encode(writer),
            Self::StartGame(p) => p.encode(writer),
            Self::ChunkRadiusUpdated(p) => p.encode(writer),
            Self::LevelChunk(p) => p.encode(writer),
            Self::Disconnect(p) => p.encode(writer),
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
    pub entity_unique_id: i64,
    pub entity_runtime_id: u64,
    pub position: Vector3f,
    pub gamemode: i32,
    pub chunk_radius: i32,
    pub compression_enabled: bool,
}

impl PlayerSession {
    pub fn new(session_id: u64, client_addr: SocketAddr) -> Self {
        Self {
            session_id,
            client_addr,
            state: SessionState::Connected,
            protocol_version: 0,
            username: format!("Player_{}", session_id),
            entity_unique_id: session_id as i64,
            entity_runtime_id: session_id,
            position: Vector3f::new(0.0, 68.0, 0.0),
            gamemode: 0, // Survival
            chunk_radius: 8,
            compression_enabled: false,
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
            compression_threshold: 0,
            compression_algorithm: 0, // 0 = Deflate / Zlib (Standard PocketMine/Bedrock)
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
        info!(
            "Session {} login attempt with protocol version {}",
            self.session_id, self.protocol_version
        );

        // Transition state to ResourcePacks
        self.state = SessionState::ResourcePacks;

        let play_status = PlayStatusPacket {
            status: PlayStatusPacket::LOGIN_SUCCESS,
        };
        let packs_info = ResourcePacksInfoPacket {
            must_accept: false,
            has_scripts: false,
            force_server_packs: false,
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
            ResourcePackClientResponsePacket::STATUS_REFUSED => {
                info!("Session {} refused resource packs; disconnecting.", self.session_id);
                self.state = SessionState::Disconnected;
                Ok(vec![OutgoingPacket::Disconnect(DisconnectPacket {
                    reason: 0,
                    skip_message: false,
                    message: Some("Resource packs were refused.".to_string()),
                })])
            }
            ResourcePackClientResponsePacket::STATUS_SEND_PACKS => {
                // Client requests packs downloading
                Ok(Vec::new())
            }
            ResourcePackClientResponsePacket::STATUS_HAVE_ALL_PACKS => {
                if self.state != SessionState::ResourcePacks {
                    return Err(SessionError::UnexpectedPacket {
                        state: self.state,
                        packet_id: 0x08,
                    });
                }

                self.state = SessionState::ResourcePackStack;
                let stack = ResourcePackStackPacket {
                    must_accept: false,
                    behavior_packs: Vec::new(),
                    resource_packs: Vec::new(),
                    game_version: "1.21.0".to_string(),
                    experiments_previously_enabled: false,
                };
                Ok(vec![OutgoingPacket::ResourcePackStack(stack)])
            }
            ResourcePackClientResponsePacket::STATUS_COMPLETED => {
                if self.state != SessionState::ResourcePackStack && self.state != SessionState::ResourcePacks {
                    return Err(SessionError::UnexpectedPacket {
                        state: self.state,
                        packet_id: 0x08,
                    });
                }

                self.state = SessionState::InGame;
                info!("Session {} entered world (InGame)", self.session_id);

                let start_game = StartGamePacket {
                    actor_unique_id: self.entity_unique_id,
                    actor_runtime_id: self.entity_runtime_id,
                    player_gamemode: self.gamemode,
                    player_position: self.position,
                    pitch: 0.0,
                    yaw: 0.0,
                    level_settings: LevelSettings {
                        seed: 12345678,
                        spawn_position: BlockPos::new(0, 64, 0),
                        generator_type: 1,
                        world_gamemode: self.gamemode,
                        hardcore: false,
                        difficulty: 1,
                        dimension: 0,
                        level_name: "PocketMine-Rust World".to_string(),
                    },
                };

                // Create initial spawn chunk (0, 0)
                let spawn_chunk = self.create_chunk_packet(0, 0);

                let play_status = PlayStatusPacket {
                    status: PlayStatusPacket::PLAYER_SPAWN,
                };

                Ok(vec![
                    OutgoingPacket::StartGame(start_game),
                    OutgoingPacket::LevelChunk(spawn_chunk),
                    OutgoingPacket::PlayStatus(play_status),
                ])
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

        let clamped_radius = packet.radius.clamp(2, 16);
        self.chunk_radius = clamped_radius;

        let mut out = vec![OutgoingPacket::ChunkRadiusUpdated(
            ChunkRadiusUpdatedPacket::new(clamped_radius),
        )];

        // Generate surrounding chunks around spawn for the client
        for x in -1..=1 {
            for z in -1..=1 {
                if x != 0 || z != 0 {
                    out.push(OutgoingPacket::LevelChunk(self.create_chunk_packet(x, z)));
                }
            }
        }

        Ok(out)
    }

    /// Generates a standard subchunk packet using bedrock-world serialization.
    pub fn create_chunk_packet(&self, chunk_x: i32, chunk_z: i32) -> LevelChunkPacket {
        let subchunk = SubChunk::new(1); // fill entry 1 (e.g. grass block)
        let payload = subchunk.serialize_network();
        LevelChunkPacket::new(chunk_x, chunk_z, 1, payload)
    }
}

/// Thread-safe player session manager.
pub struct SessionManager {
    sessions: RwLock<HashMap<u64, PlayerSession>>,
    next_session_id: AtomicU64,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            next_session_id: AtomicU64::new(1),
        }
    }

    pub fn create_session(&self, addr: SocketAddr) -> Arc<RwLock<PlayerSession>> {
        let id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
        let session = PlayerSession::new(id, addr);
        let mut map = self.sessions.write();
        map.insert(id, session);
        // Note: For direct Arc access, or callers can lookup by id
        Arc::new(RwLock::new(PlayerSession::new(id, addr)))
    }

    pub fn add_session(&self, session: PlayerSession) {
        let mut map = self.sessions.write();
        map.insert(session.session_id, session);
    }

    pub fn remove_session(&self, session_id: u64) -> Option<PlayerSession> {
        let mut map = self.sessions.write();
        map.remove(&session_id)
    }

    pub fn session_count(&self) -> usize {
        self.sessions.read().len()
    }

    /// Dispatches a raw Bedrock network message received over NetherNet WebRtc DataChannels
    pub fn dispatch_webrtc_packet(&self, peer: SocketAddr, raw: &[u8]) -> Vec<bytes::Bytes> {
        let mut map = self.sessions.write();
        let session = map.values_mut().find(|s| s.client_addr == peer);
        let session = match session {
            Some(s) => s,
            None => {
                let id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
                let new_session = PlayerSession::new(id, peer);
                map.insert(id, new_session);
                map.get_mut(&id).unwrap()
            }
        };

        // Determine if packet is compressed:
        // Either session.compression_enabled is true, OR payload starts with zlib header (0x00 0x78)
        let decompressed = if raw.len() >= 3 && raw[0] == 0x00 && raw[1] == 0x78 {
            session.compression_enabled = true;
            match PacketBatch::decompress(raw) {
                Ok(data) => data,
                Err(e) => {
                    tracing::error!("[:WebRTC] Failed to decompress batch from {}: {}", peer, e);
                    return Vec::new();
                }
            }
        } else if session.compression_enabled && !raw.is_empty() {
            match PacketBatch::decompress(raw) {
                Ok(data) => data,
                Err(e) => {
                    tracing::error!("[:WebRTC] Failed to decompress batch from {}: {}", peer, e);
                    return Vec::new();
                }
            }
        } else {
            raw.to_vec()
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

        let mut outgoing_batch = Vec::new();

        for (packet_id, payload) in packets {
            info!(
                "[:WebRTC] Dispatching Bedrock packet 0x{:02X} (len {}) from {}",
                packet_id, payload.len(), peer
            );

            match session.handle_packet(packet_id, &payload) {
                Ok(responses) => {
                    for resp in responses {
                        let mut pkt_writer = Writer::new();
                        let header = resp.packet_id() & 0x3FF;
                        pkt_writer.write_var_u32(header);
                        if let Err(e) = resp.encode(&mut pkt_writer) {
                            tracing::error!("Failed to encode outgoing packet 0x{:02X}: {}", resp.packet_id(), e);
                            continue;
                        }
                        let pkt_bytes = pkt_writer.into_bytes();

                        let mut entry_writer = Writer::new();
                        entry_writer.write_var_u32(pkt_bytes.len() as u32);
                        entry_writer.write_bytes(&pkt_bytes);
                        outgoing_batch.extend_from_slice(&entry_writer.into_bytes());

                        // If response was NetworkSettingsPacket, enable compression for following packets
                        if resp.packet_id() == 0x8F {
                            // NetworkSettingsPacket itself is sent uncompressed:
                            let uncompressed_resp = outgoing_batch.clone();
                            outgoing_batch.clear();
                            session.compression_enabled = true;
                            return vec![bytes::Bytes::from(uncompressed_resp)];
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("[:WebRTC] Session error handling 0x{:02X} from {}: {}", packet_id, peer, e);
                }
            }
        }

        if outgoing_batch.is_empty() {
            return Vec::new();
        }

        // Compress the outgoing batch if compression is active
        if session.compression_enabled {
            match PacketBatch::compress(&outgoing_batch, 0) {
                Ok(compressed) => vec![bytes::Bytes::from(compressed)],
                Err(e) => {
                    tracing::error!("Failed to compress outgoing batch for {}: {}", peer, e);
                    vec![bytes::Bytes::from(outgoing_batch)]
                }
            }
        } else {
            vec![bytes::Bytes::from(outgoing_batch)]
        }
    }
}
