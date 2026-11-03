use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bedrock_protocol::binary::{CodecError, Reader, Writer};
use bedrock_protocol::{
    BlockPos, ChunkRadiusUpdatedPacket, DisconnectPacket, LevelChunkPacket,
    LoginPacket, NetworkSettingsPacket, Packet, PacketBatch, PacketViolationWarningPacket, PlayStatusPacket,
    RequestChunkRadiusPacket, RequestNetworkSettingsPacket, ResourcePackClientResponsePacket,
    ResourcePackStackPacket, ResourcePacksInfoPacket, StartGamePacket, Vector3f,
};
use bedrock_world::SubChunk;
use parking_lot::RwLock;
use thiserror::Error;
use tracing::{debug, error, info};

use crate::static_packets::StaticPackets;

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
            Self::ChunkRadiusUpdated(p) => p.packet_id(),
            Self::LevelChunk(p) => p.packet_id(),
            Self::Disconnect(p) => p.packet_id(),
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
            Self::ChunkRadiusUpdated(p) => p.encode(writer),
            Self::LevelChunk(p) => p.encode(writer),
            Self::Disconnect(p) => p.encode(writer),
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
                start_game.player_position = self.position;
                start_game.level_settings.seed = 12345678;
                start_game.level_settings.spawn_position = BlockPos::new(0, 64, 0);
                start_game.level_settings.generator_type = 1;
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

                // 8. SetCommandsEnabledPacket (0x3B / 59)
                out.push(OutgoingPacket::RawWire(StaticPackets::SET_COMMANDS_ENABLED.to_vec()));

                // 9. SetDifficultyPacket (0x3C / 60)
                out.push(OutgoingPacket::RawWire(StaticPackets::SET_DIFFICULTY.to_vec()));

                // 10. GameRulesChangedPacket (0x48 / 72)
                out.push(OutgoingPacket::RawWire(StaticPackets::GAME_RULES_CHANGED.to_vec()));

                // 11. SetSpawnPositionPacket (0x2B / 43)
                out.push(OutgoingPacket::RawWire(StaticPackets::SET_SPAWN_POSITION.to_vec()));

                // 12. NetworkChunkPublisherUpdatePacket (0x79 / 121)
                out.push(OutgoingPacket::RawWire(StaticPackets::NETWORK_CHUNK_PUBLISHER_UPDATE.to_vec()));

                // 13. Create initial spawn chunk (0, 0)
                let spawn_chunk = self.create_chunk_packet(0, 0);
                out.push(OutgoingPacket::LevelChunk(spawn_chunk));

                // 14. Confirm spawn to client (PLAYER_SPAWN = 3)
                let play_status = PlayStatusPacket {
                    status: PlayStatusPacket::PLAYER_SPAWN,
                };
                out.push(OutgoingPacket::PlayStatus(play_status));

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
        let mut writer = Writer::new();

        // 1 subchunk (at y = 0)
        let subchunk = SubChunk::new(1); // grass block or air
        subchunk.serialize_network_to(&mut writer);

        // 24 biome palettes (dimension bounds -4..=19 in 1.18+)
        for _ in 0..24 {
            // bpb = 0 -> header = (0 << 1) | 1 = 1
            writer.write_byte(1);
            // 0 words
            // no count since bpb == 0
            // 1 palette entry: plains = 1
            writer.write_var_i32(1);
        }

        // Border blocks count: 0
        writer.write_byte(0);

        // Block entities: 0 (no tiles)

        let payload = writer.into_bytes().to_vec();
        LevelChunkPacket::new(
            chunk_x,
            chunk_z,
            0,     // dimension: 0 (Overworld)
            1,     // sub_chunk_count: 1
            None,  // sub_chunk_request_limit: None
            false, // cache_enabled: false
            Vec::new(), // used_blob_hashes: empty
            payload,
            false, // client_biome_update: false
        )
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

        let mut batches: Vec<Vec<u8>> = Vec::new();
        let mut current_batch = Vec::new();

        for (packet_id, payload) in packets {
            info!(
                "[:WebRTC] Dispatching Bedrock packet 0x{:02X} (len {}) from {}",
                packet_id, payload.len(), peer
            );

            match session.handle_packet(packet_id, &payload) {
                Ok(responses) => {
                    for resp in responses {
                        let pkt_bytes = match &resp {
                            OutgoingPacket::RawWire(bytes) => bytes::Bytes::copy_from_slice(bytes),
                            _ => {
                                let mut pkt_writer = Writer::new();
                                let header = resp.packet_id() & 0x3FF;
                                pkt_writer.write_var_u32(header);
                                if let Err(e) = resp.encode(&mut pkt_writer) {
                                    tracing::error!("Failed to encode outgoing packet 0x{:02X}: {}", resp.packet_id(), e);
                                    continue;
                                }
                                pkt_writer.into_bytes()
                            }
                        };

                        let mut entry_writer = Writer::new();
                        entry_writer.write_var_u32(pkt_bytes.len() as u32);
                        entry_writer.write_bytes(&pkt_bytes);
                        let framed_entry = entry_writer.into_bytes();

                        // If response was NetworkSettingsPacket, send uncompressed immediately
                        if resp.packet_id() == 0x8F {
                            session.compression_enabled = true;
                            return vec![bytes::Bytes::from(framed_entry)];
                        }

                        // Split batches at ~16KB threshold to keep SCTP message size optimal
                        if !current_batch.is_empty() && current_batch.len() + framed_entry.len() > 16384 {
                            batches.push(std::mem::take(&mut current_batch));
                        }

                        current_batch.extend_from_slice(&framed_entry);
                    }
                }
                Err(e) => {
                    tracing::warn!("[:WebRTC] Session error handling 0x{:02X} from {}: {}", packet_id, peer, e);
                }
            }
        }

        if !current_batch.is_empty() {
            batches.push(current_batch);
        }

        let mut out = Vec::with_capacity(batches.len());
        for batch in batches {
            if session.compression_enabled {
                match PacketBatch::compress(&batch, 512) {
                    Ok(compressed) => out.push(bytes::Bytes::from(compressed)),
                    Err(e) => {
                        tracing::error!("Failed to compress outgoing batch for {}: {}", peer, e);
                        out.push(bytes::Bytes::from(batch));
                    }
                }
            } else {
                out.push(bytes::Bytes::from(batch));
            }
        }
        out
    }
}
