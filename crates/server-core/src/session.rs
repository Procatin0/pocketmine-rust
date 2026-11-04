use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bedrock_protocol::binary::{CodecError, Reader, Writer};
use bedrock_protocol::{
    BlockPos, ChunkRadiusUpdatedPacket, DisconnectPacket, LevelChunkPacket,
    LoginPacket, NetworkChunkPublisherUpdatePacket, NetworkSettingsPacket, Packet, PacketBatch,
    PacketViolationWarningPacket, PlayStatusPacket, PlayerAuthInputPacket, RequestAbilityPacket,
    RequestChunkRadiusPacket, RequestNetworkSettingsPacket, ResourcePackClientResponsePacket,
    ResourcePackStackPacket, ResourcePacksInfoPacket, SetPlayerGameTypePacket, SetSpawnPositionPacket,
    StartGamePacket, TextPacket, UpdateAbilitiesPacket, UpdatePlayerGameTypePacket, Vector3f,
};
use bedrock_world::{FlatGenerator, WorldGenerator};
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
    SetSpawnPosition(SetSpawnPositionPacket),
    NetworkChunkPublisherUpdate(NetworkChunkPublisherUpdatePacket),
    ChunkRadiusUpdated(ChunkRadiusUpdatedPacket),
    LevelChunk(LevelChunkPacket),
    Disconnect(DisconnectPacket),
    SetPlayerGameType(SetPlayerGameTypePacket),
    UpdatePlayerGameType(UpdatePlayerGameTypePacket),
    UpdateAbilities(UpdateAbilitiesPacket),
    Text(TextPacket),
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
    pub pitch: f32,
    pub yaw: f32,
    pub gamemode: i32,
    pub is_flying: bool,
    pub chunk_radius: i32,
    pub sent_chunks: HashSet<(i32, i32)>,
    pub compression_enabled: bool,
    pub generator: Arc<dyn WorldGenerator>,
}

impl PlayerSession {
    pub fn new(session_id: u64, client_addr: SocketAddr) -> Self {
        let generator = Arc::new(FlatGenerator::new());
        let (px, py, pz) = generator.player_position();
        Self {
            session_id,
            client_addr,
            state: SessionState::Connected,
            protocol_version: 0,
            username: format!("Player_{}", session_id),
            entity_unique_id: session_id as i64,
            entity_runtime_id: session_id,
            position: Vector3f::new(px, py, pz),
            pitch: 0.0,
            yaw: 0.0,
            gamemode: 0, // Survival
            is_flying: false,
            chunk_radius: 8,
            sent_chunks: HashSet::new(),
            compression_enabled: false,
            generator,
        }
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
        let (px, py, pz) = generator.player_position();
        Self {
            session_id,
            client_addr,
            state: SessionState::Connected,
            protocol_version: 0,
            username: format!("Player_{}", session_id),
            entity_unique_id: session_id as i64,
            entity_runtime_id: session_id,
            position: Vector3f::new(px, py, pz),
            pitch: 0.0,
            yaw: 0.0,
            gamemode,
            is_flying: false,
            chunk_radius: 8,
            sent_chunks: HashSet::new(),
            compression_enabled: false,
            generator,
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
                let (px, py, pz) = self.generator.player_position();
                self.position = Vector3f::new(px, py, pz);
                start_game.player_position = self.position;
                start_game.level_settings.seed = 12345678;
                let (sx, sy, sz) = self.generator.spawn_position();
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

                // 9. SetActorDataPacket (0x27 / 39) - Collision, gravity, scale, air=300
                out.push(OutgoingPacket::RawWire(self.create_set_actor_data_packet()));

                // 10. UpdateAbilitiesPacket (0xBB / 187) - Survival abilities, flying=false
                out.push(OutgoingPacket::RawWire(self.create_update_abilities_packet()));

                // 10b. UpdateAdventureSettingsPacket (0xBC / 188)
                out.push(OutgoingPacket::RawWire(StaticPackets::UPDATE_ADVENTURE_SETTINGS.to_vec()));

                // 10c. SetCommandsEnabledPacket (0x3B / 59)
                out.push(OutgoingPacket::RawWire(StaticPackets::SET_COMMANDS_ENABLED.to_vec()));

                // 10d. SetDifficultyPacket (0x3C / 60)
                out.push(OutgoingPacket::RawWire(StaticPackets::SET_DIFFICULTY.to_vec()));

                // 10e. GameRulesChangedPacket (0x48 / 72)
                out.push(OutgoingPacket::RawWire(StaticPackets::GAME_RULES_CHANGED.to_vec()));

                // 11. SetSpawnPositionPacket (0x2B / 43)
                let spawn_pos = BlockPos::new(sx, sy, sz);
                out.push(OutgoingPacket::SetSpawnPosition(
                    SetSpawnPositionPacket::new_player_spawn(spawn_pos, 0),
                ));

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

        let clamped_radius = packet.radius.clamp(2, 32);
        self.chunk_radius = clamped_radius;

        let center_pos = BlockPos::new(
            self.position.x as i32,
            self.position.y as i32,
            self.position.z as i32,
        );

        let publisher_radius_blocks = (((clamped_radius as f64 * std::f64::consts::SQRT_2) + 2.0).ceil() as u32) * 16;

        let mut out = vec![
            OutgoingPacket::ChunkRadiusUpdated(ChunkRadiusUpdatedPacket::new(clamped_radius)),
            OutgoingPacket::NetworkChunkPublisherUpdate(
                NetworkChunkPublisherUpdatePacket::new(center_pos, publisher_radius_blocks),
            ),
        ];

        // Send an immediate burst around player (clamped_radius.min(6)) so the local area is loaded immediately,
        // while remaining chunks stream smoothly via PlayerAuthInput without stalling the server event loop.
        let curr_cx = (self.position.x as i32) >> 4;
        let curr_cz = (self.position.z as i32) >> 4;
        let burst_radius = clamped_radius.min(6);
        let mut chunks_to_generate = Vec::new();
        for dx in -burst_radius..=burst_radius {
            for dz in -burst_radius..=burst_radius {
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

        for (cx, cz) in chunks_to_generate {
            self.sent_chunks.insert((cx, cz));
            out.push(OutgoingPacket::LevelChunk(self.create_chunk_packet(cx, cz)));
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

        let old_cx = (self.position.x as i32) >> 4;
        let old_cz = (self.position.z as i32) >> 4;

        self.position = packet.position;
        self.pitch = packet.pitch;
        self.yaw = packet.yaw;

        let radius = self.chunk_radius;
        let curr_cx = (self.position.x as i32) >> 4;
        let curr_cz = (self.position.z as i32) >> 4;

        // Prune chunks that are far away from the player (beyond radius + 16)
        // so that if the player returns to previously visited areas, they will be resent,
        // preventing empty transparent holes/voids when backtracking.
        let prune_margin = radius + 16;
        self.sent_chunks.retain(|&(cx, cz)| {
            (cx - curr_cx).abs() <= prune_margin && (cz - curr_cz).abs() <= prune_margin
        });

        let mut out = Vec::new();
        let max_chunks_per_packet = 16;

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

        // Publisher radius in blocks MUST encompass the entire square area (diagonal = radius * sqrt(2))
        // plus an additional margin so Bedrock client NEVER discards any boundary chunk!
        let publisher_radius_blocks = (((radius as f64 * std::f64::consts::SQRT_2) + 2.0).ceil() as u32) * 16;

        // CRITICAL: NetworkChunkPublisherUpdate MUST be added to `out` FIRST, BEFORE any LevelChunk packets!
        // In Bedrock, the client discards any chunk outside its current publisher radius.
        // Putting the update packet first ensures the client updates its publisher area BEFORE receiving chunks.
        if !chunks_to_generate.is_empty() || curr_cx != old_cx || curr_cz != old_cz {
            let center_pos = BlockPos::new(
                self.position.x as i32,
                self.position.y as i32,
                self.position.z as i32,
            );
            out.push(OutgoingPacket::NetworkChunkPublisherUpdate(
                NetworkChunkPublisherUpdatePacket::new(center_pos, publisher_radius_blocks),
            ));
        }

        for (cx, cz) in chunks_to_generate.into_iter().take(max_chunks_per_packet) {
            self.sent_chunks.insert((cx, cz));
            out.push(OutgoingPacket::LevelChunk(self.create_chunk_packet(cx, cz)));
        }

        Ok(out)
    }

    /// Handles TextPacket (0x09) for chat and in-game slash commands.
    pub fn handle_text(&mut self, packet: TextPacket) -> Result<Vec<OutgoingPacket>, SessionError> {
        let trimmed = packet.message.trim();
        if trimmed.starts_with('/') {
            if let Some(cmd_out) = crate::command::handle_command(trimmed) {
                let mut out = Vec::new();
                match cmd_out.action {
                    crate::command::CommandAction::SetGameMode(new_gm) => {
                        self.gamemode = new_gm;
                        if new_gm != 1 {
                            self.is_flying = false;
                        }
                        out.push(OutgoingPacket::SetPlayerGameType(SetPlayerGameTypePacket::new(new_gm)));
                        out.push(OutgoingPacket::RawWire(self.create_update_abilities_packet()));
                        out.push(OutgoingPacket::Text(TextPacket::raw(cmd_out.message)));
                    }
                    crate::command::CommandAction::None => {
                        out.push(OutgoingPacket::Text(TextPacket::raw(cmd_out.message)));
                    }
                }
                return Ok(out);
            }
        }

        // Normal chat: echo back to player
        let echo = TextPacket::chat(&self.username, &packet.message);
        Ok(vec![OutgoingPacket::Text(echo)])
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

    /// Generates a LevelChunkPacket for (chunk_x, chunk_z) using the active WorldGenerator.
    pub fn create_chunk_packet(&self, chunk_x: i32, chunk_z: i32) -> LevelChunkPacket {
        let (sub_chunk_count, payload) = self.generator.generate_chunk(chunk_x, chunk_z);
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

        // 1. minecraft:health (min: 0.0, max: 20.0, val: 20.0, def_min: 0.0, def_max: 20.0, def: 20.0)
        writer.write_f32_le(0.0);
        writer.write_f32_le(20.0);
        writer.write_f32_le(20.0);
        writer.write_f32_le(0.0);
        writer.write_f32_le(20.0);
        writer.write_f32_le(20.0);
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
}

/// Thread-safe player session manager.
pub struct SessionManager {
    sessions: RwLock<HashMap<u64, PlayerSession>>,
    next_session_id: AtomicU64,
    generator: Arc<dyn WorldGenerator>,
    default_gamemode: i32,
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
        Self {
            sessions: RwLock::new(HashMap::new()),
            next_session_id: AtomicU64::new(1),
            generator,
            default_gamemode,
        }
    }

    pub fn create_session(&self, addr: SocketAddr) -> Arc<RwLock<PlayerSession>> {
        let id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
        let session = PlayerSession::with_generator_and_gamemode(
            id,
            addr,
            Arc::clone(&self.generator),
            self.default_gamemode,
        );
        let mut map = self.sessions.write();
        map.insert(id, session);
        // Note: For direct Arc access, or callers can lookup by id
        Arc::new(RwLock::new(PlayerSession::with_generator_and_gamemode(
            id,
            addr,
            Arc::clone(&self.generator),
            self.default_gamemode,
        )))
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
                let new_session = PlayerSession::with_generator_and_gamemode(
                    id,
                    peer,
                    Arc::clone(&self.generator),
                    self.default_gamemode,
                );
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
