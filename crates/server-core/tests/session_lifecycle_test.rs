use std::net::SocketAddr;

use bedrock_protocol::{
    LoginPacket, MovePlayerPacket, MovePlayerTeleportData, Packet, PlayStatusPacket,
    PlayerAuthInputPacket, Reader, RequestChunkRadiusPacket, RequestNetworkSettingsPacket,
    ResourcePackClientResponsePacket, TextPacket, Vector3f, Writer,
};
use server_core::command::{CommandAction, WeatherState};
use server_core::{OutgoingPacket, PlayerSession, SessionManager, SessionState};

#[test]
fn test_session_full_login_lifecycle() {
    let addr: SocketAddr = "127.0.0.1:54321".parse().unwrap();
    let mut session = PlayerSession::new(42, addr);

    assert_eq!(session.state, SessionState::Connected);
    assert!(!session.is_in_game());

    // Step 1: Client sends RequestNetworkSettings (0xC1)
    let req_net = RequestNetworkSettingsPacket {
        client_protocol_version: 685,
    };
    let mut writer = Writer::new();
    req_net.encode(&mut writer).unwrap();
    let responses = session
        .handle_packet(0xC1, &writer.into_bytes())
        .expect("handle 0xC1 should succeed");

    assert_eq!(responses.len(), 1);
    match &responses[0] {
        OutgoingPacket::NetworkSettings(pkt) => {
            assert_eq!(pkt.compression_threshold, 512);
            assert_eq!(pkt.compression_algorithm, 0);
            assert_eq!(pkt.client_throttle, false);
        }
        other => panic!("Expected NetworkSettings, got {:?}", other),
    }

    // Step 2: Client sends LoginPacket (0x01)
    let login_pkt = LoginPacket::new(685, "auth_client_payload_data");
    let mut writer = Writer::new();
    login_pkt.encode(&mut writer).unwrap();
    let responses = session
        .handle_packet(0x01, &writer.into_bytes())
        .expect("handle 0x01 should succeed");

    assert_eq!(session.state, SessionState::ResourcePacks);
    assert_eq!(responses.len(), 2);
    match &responses[0] {
        OutgoingPacket::PlayStatus(p) => {
            assert_eq!(p.status, PlayStatusPacket::LOGIN_SUCCESS);
        }
        other => panic!("Expected PlayStatus(LOGIN_SUCCESS), got {:?}", other),
    }
    match &responses[1] {
        OutgoingPacket::ResourcePacksInfo(p) => {
            assert!(!p.must_accept);
        }
        other => panic!("Expected ResourcePacksInfo, got {:?}", other),
    }

    // Step 3: Client responds with ResourcePackClientResponse (HAVE_ALL_PACKS)
    let pack_response_1 = ResourcePackClientResponsePacket::new(
        ResourcePackClientResponsePacket::STATUS_HAVE_ALL_PACKS,
        Vec::new(),
    );
    let mut writer = Writer::new();
    pack_response_1.encode(&mut writer).unwrap();
    let responses = session
        .handle_packet(0x08, &writer.into_bytes())
        .expect("handle 0x08 should succeed");

    assert_eq!(session.state, SessionState::ResourcePackStack);
    assert_eq!(responses.len(), 1);
    match &responses[0] {
        OutgoingPacket::ResourcePackStack(p) => {
            assert_eq!(p.game_version, "1.26.60");
        }
        other => panic!("Expected ResourcePackStack, got {:?}", other),
    }

    // Step 4: Client responds with ResourcePackClientResponse (COMPLETED)
    let pack_response_2 = ResourcePackClientResponsePacket::new(
        ResourcePackClientResponsePacket::STATUS_COMPLETED,
        Vec::new(),
    );
    let mut writer = Writer::new();
    pack_response_2.encode(&mut writer).unwrap();
    let responses = session
        .handle_packet(0x08, &writer.into_bytes())
        .expect("handle 0x08 COMPLETED should succeed");

    assert_eq!(session.state, SessionState::PreSpawn);
    assert!(session.is_in_game());
    assert!(responses.len() >= 3);
    // PLAYER_SPAWN is NOT sent until the client confirms init.
    assert!(
        !responses
            .iter()
            .any(|r| matches!(r, OutgoingPacket::PlayStatus(p) if p.status == PlayStatusPacket::PLAYER_SPAWN)),
        "PLAYER_SPAWN must wait for 0x71"
    );

    // Client sends SetLocalPlayerAsInitializedPacket (0x71) with runtime id.
    let mut writer = Writer::new();
    writer.write_var_u64(42);
    let spawn_responses = session
        .handle_packet(0x71, &writer.into_bytes())
        .expect("handle 0x71 should succeed");
    assert_eq!(session.state, SessionState::InGame);
    assert!(session.is_spawned());
    let play_status = spawn_responses.iter().find_map(|r| match r {
        OutgoingPacket::PlayStatus(p) => Some(p),
        _ => None,
    }).expect("Expected PLAYER_SPAWN after 0x71");
    assert_eq!(play_status.status, PlayStatusPacket::PLAYER_SPAWN);

    // Verify StartGamePacket is present
    let start_game = responses.iter().find_map(|r| match r {
        OutgoingPacket::StartGame(p) => Some(p),
        _ => None,
    }).expect("Expected StartGamePacket in responses");
    // Login now derives the entity id from the real login identity (see
    // `parse_login_identity`): the fake payload has no XUID, so the id is
    // FNV-1a("Player"), not the raw session id. The runtime id stays bound
    // to the session.
    assert_eq!(session.username, "Player");
    assert_eq!(start_game.actor_unique_id, session.entity_unique_id);
    assert_eq!(start_game.actor_runtime_id, 42);
    assert_eq!(start_game.player_gamemode, 0);
    assert_eq!(start_game.world_name, "Axolotl-PM World");

    // Verify spawn LevelChunkPacket is present
    let chunk = responses.iter().find_map(|r| match r {
        OutgoingPacket::LevelChunk(c) if c.chunk_x == 0 && c.chunk_z == 0 => Some(c),
        _ => None,
    }).expect("Expected LevelChunk (0,0) in responses");
    assert_eq!(chunk.chunk_x, 0);
    assert_eq!(chunk.chunk_z, 0);
    assert_eq!(chunk.sub_chunk_count, 5);
    assert!(!chunk.payload.is_empty());

    // Verify PlayStatusPacket (PLAYER_SPAWN) is present (after 0x71).
    let play_status = spawn_responses.iter().find_map(|r| match r {
        OutgoingPacket::PlayStatus(p) => Some(p),
        _ => None,
    }).expect("Expected PlayStatusPacket in responses");
    assert_eq!(play_status.status, PlayStatusPacket::PLAYER_SPAWN);

    // Verify the native commands are advertised to the client (0x4C).
    let commands_packet = responses
        .iter()
        .find(|r| matches!(r, OutgoingPacket::RawWire(bytes) if bytes.first() == Some(&0x4Cu8)))
        .expect("Expected AvailableCommandsPacket in responses");
    match commands_packet {
        OutgoingPacket::RawWire(bytes) => assert!(bytes.len() > 100),
        _ => unreachable!(),
    }

    // Step 5: Client sends RequestChunkRadius (0x45)
    let radius_req = RequestChunkRadiusPacket::new(12, 16);
    let mut writer = Writer::new();
    radius_req.encode(&mut writer).unwrap();
    let responses = session
        .handle_packet(0x45, &writer.into_bytes())
        .expect("handle 0x45 should succeed");

    assert_eq!(session.chunk_radius, 12);
    assert!(!responses.is_empty());
    match &responses[0] {
        OutgoingPacket::ChunkRadiusUpdated(p) => {
            assert_eq!(p.radius, 12);
        }
        other => panic!("Expected ChunkRadiusUpdated, got {:?}", other),
    }

    // All outgoing packets should be serializable into bytes
    for resp in &responses {
        let bytes = resp.to_bytes().expect("to_bytes should succeed");
        assert!(!bytes.is_empty());
    }

    // Step 6: Client sends PlayerAuthInputPacket (0x90) with new position
    // Move to (320.0, 65.0, 0.0) -> chunk (20, 0), well outside the previous window.
    let auth_input = PlayerAuthInputPacket::new(
        10.0,
        90.0,
        Vector3f::new(320.0, 65.0, 0.0),
        0.0,
        0.0,
        90.0,
    );
    let mut writer = Writer::new();
    auth_input.encode(&mut writer).unwrap();
    let move_responses = session
        .handle_packet(0x90, &writer.into_bytes())
        .expect("handle 0x90 should succeed");

    // Client position is the eye; server stores the feet (eye - 1.62).
    assert_eq!(session.position, Vector3f::new(320.0, 65.0 - 1.62, 0.0));
    assert_eq!(session.pitch, 10.0);
    assert_eq!(session.yaw, 90.0);
    assert!(!move_responses.is_empty());

    let has_chunks = move_responses
        .iter()
        .any(|r| matches!(r, OutgoingPacket::LevelChunk(_)));
    let has_pub_update = move_responses
        .iter()
        .any(|r| matches!(r, OutgoingPacket::NetworkChunkPublisherUpdate(_)));
    assert!(has_chunks);
    assert!(has_pub_update);
    assert!(session.sent_chunks.contains(&(20, 0)));

    for resp in &move_responses {
        let bytes = resp.to_bytes().expect("to_bytes should succeed");
        assert!(!bytes.is_empty());
    }
}

#[test]
fn test_session_resource_packs_refused() {
    let addr: SocketAddr = "127.0.0.1:54322".parse().unwrap();
    let mut session = PlayerSession::new(100, addr);

    // Move to ResourcePacks state
    let login_pkt = LoginPacket::new(685, "test");
    session.handle_login(login_pkt).unwrap();
    assert_eq!(session.state, SessionState::ResourcePacks);

    // Client refuses
    let pack_response = ResourcePackClientResponsePacket::new(
        ResourcePackClientResponsePacket::STATUS_REFUSED,
        Vec::new(),
    );
    let responses = session
        .handle_resource_pack_response(pack_response)
        .unwrap();

    assert_eq!(session.state, SessionState::Disconnected);
    assert!(session.is_disconnected());
    assert_eq!(responses.len(), 1);
    match &responses[0] {
        OutgoingPacket::Disconnect(p) => {
            assert_eq!(p.message.as_deref(), Some("Resource packs were refused."));
        }
        other => panic!("Expected Disconnect, got {:?}", other),
    }
}

#[test]
fn test_auth_input_with_unparsable_tail_still_moves() {
    // Block actions ride in the PlayerAuthInput input_data tail. Whatever
    // bytes a client sends there must never break movement handling.
    let addr: SocketAddr = "127.0.0.1:54324".parse().unwrap();
    let mut session = PlayerSession::new(78, addr);

    session.handle_login(LoginPacket::new(685, "test")).unwrap();
    session.handle_resource_pack_response(ResourcePackClientResponsePacket::new(
        ResourcePackClientResponsePacket::STATUS_HAVE_ALL_PACKS,
        Vec::new(),
    )).unwrap();
    session.handle_resource_pack_response(ResourcePackClientResponsePacket::new(
        ResourcePackClientResponsePacket::STATUS_COMPLETED,
        Vec::new(),
    )).unwrap();
    // Client confirms init before movement is accepted.
    session.handle_set_local_player_as_initialized().unwrap();
    assert!(session.is_in_game());
    assert!(session.is_spawned());
    session.chunk_radius = 2;

    // 1. Random garbage tail: movement must work exactly as without a tail.
    let mut garbage = PlayerAuthInputPacket::new(
        5.0, 180.0, Vector3f::new(2.0, 65.0, 2.0), 0.0, 0.0, 180.0,
    );
    garbage.input_data = vec![0xFF, 0x00, 0x13, 0x77, 0x42, 0x80, 0x80, 0x80];
    let responses = session.handle_player_auth_input(garbage).unwrap();
    assert!(responses.is_empty());
    assert_eq!(session.position, Vector3f::new(2.0, 65.0 - 1.62, 2.0));
    assert!(session.pending_actions.is_empty());

    // 2. Well-formed legacy START_BREAK tail at (0, 65, 0) face 1, but with
    // no plugins attached the block path stays dormant and movement wins.
    let mut with_tail = PlayerAuthInputPacket::new(
        0.0, 0.0, Vector3f::new(3.0, 65.0, 3.0), 0.0, 0.0, 0.0,
    );
    with_tail.input_data = vec![
        0x80, 0x80, 0x80, 0x80, 0x80, 0x01, // flags: bit 35
        0x00, // inputMode
        0x00, // playMode
        0x00, // tick
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // delta
        0x01, // count = 1
        0x00, // START_BREAK
        0x00, // x = 0
        0x82, 0x01, // y = 65 (zigzag)
        0x00, // z = 0
        0x02, // face = 1 (zigzag)
    ];
    let responses = session.handle_player_auth_input(with_tail).unwrap();
    assert!(responses.is_empty());
    assert_eq!(session.position, Vector3f::new(3.0, 65.0 - 1.62, 3.0));
    assert!(session.pending_actions.is_empty());
}

#[test]
fn test_player_tp_command_queues_a_teleport() {
    let addr: SocketAddr = "127.0.0.1:54330".parse().unwrap();
    let mut session = PlayerSession::new(60, addr);
    session.username = "Steve".to_string();

    let responses = session
        .handle_text(TextPacket::raw("/tp 12 70 -4"))
        .expect("handle /tp");
    assert_eq!(responses.len(), 1, "only the feedback line is sent directly");
    assert_eq!(
        session.pending_core,
        vec![CommandAction::Teleport {
            target: "Steve".to_string(),
            position: (12.0, 70.0, -4.0),
            yaw: None,
            pitch: None,
        }]
    );

    // Relative coordinates resolve against the sender position.
    let start = session.position;
    let _ = session
        .handle_text(TextPacket::raw("/tp ~2 ~ ~-1"))
        .expect("handle /tp relative");
    assert_eq!(
        session.pending_core.last(),
        Some(&CommandAction::Teleport {
            target: "Steve".to_string(),
            position: (start.x as f64 + 2.0, start.y as f64, start.z as f64 - 1.0),
            yaw: None,
            pitch: None,
        })
    );

    // Unknown commands still report the fallback message.
    let responses = session
        .handle_text(TextPacket::raw("/definitely_not_a_command"))
        .expect("handle unknown command");
    assert_eq!(responses.len(), 1);
}

#[test]
fn test_tp_to_0_100_0_sends_a_byte_exact_move_player_packet() {
    // Regression test for the Bedrock `/tp` disconnect
    // (`CLIENT PACKET VIOLATION packet_id=0x12 ... read() incomplete`):
    // the teleport used to go out as 0x12 (MoveActorAbsolute) with a
    // MovePlayer payload, missing the telemetry optional bool and writing
    // the teleport source as a VarInt instead of u32 LE.
    let addr: SocketAddr = "127.0.0.1:54333".parse().unwrap();
    let mut session = PlayerSession::new(63, addr);
    session.username = "Steve".to_string();
    session.state = SessionState::InGame;

    session.teleport(0.0, 100.0, 0.0, None, None);

    assert_eq!(session.position, Vector3f::new(0.0, 100.0, 0.0));
    assert!(session.force_move_sync);
    assert_eq!(session.pending_outgoing.len(), 1);
    assert_eq!(session.pending_outgoing[0].packet_id(), 0x13);

    let OutgoingPacket::RawWire(bytes) = &session.pending_outgoing[0] else {
        panic!("teleport must queue raw wire bytes");
    };
    let mut reader = Reader::new(bytes);
    assert_eq!(reader.read_var_u32().unwrap(), 0x13);
    let packet = MovePlayerPacket::decode(&mut reader).expect("decode MovePlayer");
    assert_eq!(reader.remaining(), 0, "no trailing bytes");
    assert_eq!(packet.actor_runtime_id, 63);
    // Eye position, like PocketMine's syncMovement (feet + 1.62).
    assert_eq!(packet.position, Vector3f::new(0.0, 101.62, 0.0));
    assert_eq!(packet.mode, MovePlayerPacket::MODE_TELEPORT);
    assert!(packet.on_ground);
    assert_eq!(packet.riding_actor_runtime_id, 0);
    assert_eq!(
        packet.telemetry_data,
        Some(MovePlayerTeleportData::new(0, 0))
    );
    assert_eq!(packet.tick, 0);
    // 1 header + 1 actor + 12 position + 12 rotation + 1 mode + 1 onGround
    // + 1 riding + 1 optional + 8 telemetry + 1 tick.
    assert_eq!(bytes.len(), 39);

    // forceMoveSync: a client input that has not reached the destination
    // yet must not move the server position, and the corrective re-send
    // must be the same well-formed 0x13 packet (not 0x12).
    let stale = PlayerAuthInputPacket::new(
        0.0,
        0.0,
        Vector3f::new(32.0, 66.62, 32.0),
        0.0,
        0.0,
        0.0,
    );
    let responses = session.handle_player_auth_input(stale).unwrap();
    assert!(responses.is_empty());
    assert_eq!(session.position, Vector3f::new(0.0, 100.0, 0.0));
    assert!(session.force_move_sync, "still waiting for confirmation");
    assert_eq!(session.pending_outgoing.len(), 2);
    assert_eq!(session.pending_outgoing[1].packet_id(), 0x13);
    let OutgoingPacket::RawWire(resend) = &session.pending_outgoing[1] else {
        panic!("forceMoveSync re-send must queue raw wire bytes");
    };
    let mut reader = Reader::new(resend);
    assert_eq!(reader.read_var_u32().unwrap(), 0x13);
    let resent = MovePlayerPacket::decode(&mut reader).expect("decode re-send");
    assert_eq!(reader.remaining(), 0);
    assert_eq!(resent.position, Vector3f::new(0.0, 101.62, 0.0));
    assert_eq!(resent.mode, MovePlayerPacket::MODE_TELEPORT);

    // Once the client confirms (within 1 block), forceMoveSync clears.
    let confirmed = PlayerAuthInputPacket::new(
        0.0,
        0.0,
        Vector3f::new(0.0, 101.62, 0.0),
        0.0,
        0.0,
        0.0,
    );
    let _ = session.handle_player_auth_input(confirmed).unwrap();
    assert!(!session.force_move_sync);
}

#[test]
fn test_in_game_command_request_runs_native_commands() {
    let addr: SocketAddr = "127.0.0.1:54332".parse().unwrap();
    let mut session = PlayerSession::new(62, addr);
    session.username = "Steve".to_string();
    session.state = SessionState::InGame;

    // CommandRequestPacket (0x4D) exactly as a 1.26 client encodes it.
    let mut writer = Writer::new();
    writer.write_string("/tp Steve 8 70 -3");
    writer.write_string("player");
    writer.write_bytes(&[0u8; 16]);
    writer.write_string("");
    writer.write_i64_le(0);
    writer.write_bool(false);
    writer.write_string("");
    let payload = writer.into_bytes().to_vec();

    let responses = session
        .handle_packet(0x4D, &payload)
        .expect("handle CommandRequestPacket");
    assert_eq!(responses.len(), 1, "the teleport feedback line");
    assert_eq!(
        session.pending_core,
        vec![CommandAction::Teleport {
            target: "Steve".to_string(),
            position: (8.0, 70.0, -3.0),
            yaw: None,
            pitch: None,
        }]
    );

    // Requests without a leading slash are not commands (Axolotl behaviour).
    let mut writer = Writer::new();
    writer.write_string("hola");
    writer.write_string("player");
    writer.write_bytes(&[0u8; 16]);
    writer.write_string("");
    writer.write_i64_le(0);
    writer.write_bool(false);
    writer.write_string("");
    let payload = writer.into_bytes().to_vec();
    let responses = session.handle_packet(0x4D, &payload).expect("handle plain text");
    assert!(responses.is_empty());
}

#[test]
fn test_console_teleport_moves_the_player() {
    let manager = SessionManager::new();
    let addr: SocketAddr = "127.0.0.1:54331".parse().unwrap();
    let mut session = PlayerSession::new(61, addr);
    session.username = "Steve".to_string();
    session.joined = true;
    manager.add_session(session);

    let feedback = manager
        .execute_console_command("/tp Steve 40 70 -25")
        .expect("/tp is a native command");
    assert!(!feedback.is_empty());

    let player = manager
        .registry()
        .get("Steve")
        .expect("teleported player is registered");
    assert_eq!((player.x, player.y, player.z), (40.0, 70.0, -25.0));

    // Non-native commands are left for the plugin command map.
    assert!(manager.execute_console_command("/hello").is_none());
}

#[test]
fn console_and_in_game_commands_have_the_same_effect() {
    let manager = SessionManager::new();

    // Console: world-level commands are applied immediately.
    manager
        .execute_console_command("/time set noon")
        .expect("/time is native in the console");
    assert_eq!(manager.world().time(), 6_000);
    manager
        .execute_console_command("/weather rain")
        .expect("/weather is native in the console");
    assert_eq!(manager.world().weather(), 1);

    // In game: the same lines go through the same parser and queue the same
    // actions, which the manager then applies exactly like the console ones.
    let addr: SocketAddr = "127.0.0.1:54333".parse().unwrap();
    let mut session = PlayerSession::new(63, addr);
    session.username = "Alex".to_string();
    session.state = SessionState::InGame;

    let responses = session
        .handle_text(TextPacket::raw("/time set noon"))
        .expect("handle /time");
    assert_eq!(responses.len(), 1);
    assert_eq!(session.pending_core, vec![CommandAction::SetTime(6_000)]);

    let responses = session
        .handle_text(TextPacket::raw("/weather rain"))
        .expect("handle /weather");
    assert_eq!(responses.len(), 1);
    assert_eq!(
        session.pending_core.last(),
        Some(&CommandAction::SetWeather {
            state: WeatherState::Rain,
            duration_ticks: 12_000,
        })
    );
}

#[test]
fn test_session_manager() {
    let manager = SessionManager::new();
    assert_eq!(manager.session_count(), 0);

    let addr1: SocketAddr = "127.0.0.1:11111".parse().unwrap();
    let session1 = PlayerSession::new(1, addr1);
    manager.add_session(session1);
    assert_eq!(manager.session_count(), 1);

    let addr2: SocketAddr = "127.0.0.1:22222".parse().unwrap();
    let session2 = PlayerSession::new(2, addr2);
    manager.add_session(session2);
    assert_eq!(manager.session_count(), 2);

    let removed = manager.remove_session(1);
    assert!(removed.is_some());
    assert_eq!(manager.session_count(), 1);
}

#[test]
fn create_session_registers_a_single_session_per_client() {
    let manager = SessionManager::new();
    let addr: SocketAddr = "127.0.0.1:54341".parse().unwrap();

    let id = manager.create_session(addr);
    assert_eq!(manager.session_count(), 1);

    // The returned id addresses the session that is actually registered, so
    // its replies are not queued on a detached copy.
    let mut writer = Writer::new();
    RequestNetworkSettingsPacket {
        client_protocol_version: 685,
    }
    .encode(&mut writer)
    .unwrap();
    let responses = manager
        .handle_session_packet(id, 0xC1, &writer.into_bytes())
        .expect("the registered session answers its own id");
    assert_eq!(responses.len(), 1);
    assert!(matches!(responses[0], OutgoingPacket::NetworkSettings(_)));

    let second = manager.create_session("127.0.0.1:54342".parse().unwrap());
    assert_ne!(second, id);
    assert_eq!(manager.session_count(), 2);
}

#[test]
fn test_dynamic_chunk_streaming_on_player_movement() {
    let addr: SocketAddr = "127.0.0.1:54323".parse().unwrap();
    let mut session = PlayerSession::new(77, addr);

    // Progress session to InGame
    session.handle_login(LoginPacket::new(685, "test")).unwrap();
    session.handle_resource_pack_response(ResourcePackClientResponsePacket::new(
        ResourcePackClientResponsePacket::STATUS_HAVE_ALL_PACKS,
        Vec::new(),
    )).unwrap();
    session.handle_resource_pack_response(ResourcePackClientResponsePacket::new(
        ResourcePackClientResponsePacket::STATUS_COMPLETED,
        Vec::new(),
    )).unwrap();
    session.handle_set_local_player_as_initialized().unwrap();

    assert!(session.is_in_game());
    assert!(session.is_spawned());
    assert_eq!(session.sent_chunks.len(), 25); // 5x5 initial spawn chunks

    // 1. Set radius to 2 (matching initial 5x5 spawn chunks) and move slightly within same chunk (0, 0)
    session.chunk_radius = 2;
    let move_same_chunk = PlayerAuthInputPacket::new(
        5.0,
        180.0,
        Vector3f::new(2.0, 65.0, 2.0),
        0.1,
        0.0,
        180.0,
    );
    let responses = session.handle_player_auth_input(move_same_chunk).unwrap();
    assert!(responses.is_empty(), "No new chunks should be sent within the already generated spawn area");
    assert_eq!(session.position, Vector3f::new(2.0, 65.0 - 1.62, 2.0));
    assert_eq!(session.pitch, 5.0);
    assert_eq!(session.yaw, 180.0);

    // 2. Move to chunk (10, 10) -> coordinate (160.0, 65.0, 160.0)
    // Window: 8..=12, 8..=12 -> 5x5 = 25 chunks to generate
    let move_to_10_10 = PlayerAuthInputPacket::new(
        0.0,
        0.0,
        Vector3f::new(160.0, 65.0, 160.0),
        0.0,
        0.0,
        0.0,
    );
    let responses_1 = session.handle_player_auth_input(move_to_10_10.clone()).unwrap();

    // 8 chunks per response, the cap that keeps the WebRTC channel alive.
    let chunks_gen_1 = responses_1.iter().filter(|r| matches!(r, OutgoingPacket::LevelChunk(_))).count();
    assert_eq!(chunks_gen_1, 8);
    assert!(responses_1.iter().any(|r| matches!(r, OutgoingPacket::NetworkChunkPublisherUpdate(_))));
    assert_eq!(session.sent_chunks.len(), 25 + 8);

    // 3. Following auth inputs stream the rest of the 5x5 window, 8 at a time,
    //    and go idle once all 25 chunks of the window are sent.
    let mut sent = 25 + chunks_gen_1;
    for step in 0..8 {
        let responses = session.handle_player_auth_input(move_to_10_10.clone()).unwrap();
        let chunks = responses
            .iter()
            .filter(|r| matches!(r, OutgoingPacket::LevelChunk(_)))
            .count();
        if chunks == 0 {
            assert!(responses.is_empty(), "step {step} should be idle");
            break;
        }
        assert!(chunks <= 8, "step {step} sent {chunks} chunks");
        assert!(responses.iter().any(|r| matches!(r, OutgoingPacket::NetworkChunkPublisherUpdate(_))));
        sent += chunks;
    }
    assert_eq!(sent, 25 + 25);
    assert_eq!(session.sent_chunks.len(), 25 + 25);
}

fn drive_to_prespawn(session: &mut PlayerSession) {
    session.handle_login(LoginPacket::new(685, "test")).unwrap();
    session
        .handle_resource_pack_response(ResourcePackClientResponsePacket::new(
            ResourcePackClientResponsePacket::STATUS_HAVE_ALL_PACKS,
            Vec::new(),
        ))
        .unwrap();
    session
        .handle_resource_pack_response(ResourcePackClientResponsePacket::new(
            ResourcePackClientResponsePacket::STATUS_COMPLETED,
            Vec::new(),
        ))
        .unwrap();
    assert_eq!(session.state, SessionState::PreSpawn);
    assert!(!session.is_spawned());
    assert!(session.prespawn_started_at.is_some());
}

#[test]
fn chunk_radius_forces_spawn_without_0x71() {
    // Regression: Bedrock clients that send RequestChunkRadius but never
    // 0x71 must still reach PLAYER_SPAWN + PlayerJoinEvent (joined).
    let addr: SocketAddr = "127.0.0.1:54401".parse().unwrap();
    let mut session = PlayerSession::new(101, addr);
    drive_to_prespawn(&mut session);

    let radius_req = RequestChunkRadiusPacket::new(8, 16);
    let responses = session.handle_chunk_radius_request(radius_req).unwrap();

    assert!(session.is_spawned());
    assert!(session.joined);
    assert_eq!(session.state, SessionState::InGame);
    assert!(session.prespawn_started_at.is_none());
    assert!(
        responses
            .iter()
            .any(|r| matches!(r, OutgoingPacket::PlayStatus(p) if p.status == PlayStatusPacket::PLAYER_SPAWN)),
        "chunk radius fallback must emit PLAYER_SPAWN"
    );
    assert!(responses
        .iter()
        .any(|r| matches!(r, OutgoingPacket::ChunkRadiusUpdated(_))));
    // A late 0x71 is a no-op (spawned already).
    assert!(session
        .handle_set_local_player_as_initialized()
        .unwrap()
        .is_empty());
}

#[test]
fn auth_input_forces_spawn_without_0x71() {
    let addr: SocketAddr = "127.0.0.1:54402".parse().unwrap();
    let mut session = PlayerSession::new(102, addr);
    drive_to_prespawn(&mut session);

    let input = PlayerAuthInputPacket::new(
        0.0,
        0.0,
        Vector3f::new(0.5, 65.0, 0.5),
        0.0,
        0.0,
        0.0,
    );
    let responses = session.handle_player_auth_input(input).unwrap();

    assert!(session.is_spawned());
    assert!(session.joined);
    assert!(
        responses
            .iter()
            .any(|r| matches!(r, OutgoingPacket::PlayStatus(p) if p.status == PlayStatusPacket::PLAYER_SPAWN)),
        "auth input fallback must emit PLAYER_SPAWN"
    );
    assert_eq!(session.position, Vector3f::new(0.5, 65.0 - 1.62, 0.5));
}

#[test]
fn prespawn_timeout_forces_spawn_via_manager_poll() {
    use std::time::{Duration, Instant};
    // No 0x71, no chunk radius: after the timeout the manager must spawn
    // so TestKit/plugins see 1 online.
    let addr: SocketAddr = "127.0.0.1:54403".parse().unwrap();
    let manager = SessionManager::new();
    let mut session = PlayerSession::new(103, addr);
    drive_to_prespawn(&mut session);
    session.prespawn_started_at = Some(Instant::now() - Duration::from_secs(11));
    manager.add_session(session);
    assert_eq!(manager.poll_prespawn_timeouts(), 1);
    assert_eq!(manager.poll_prespawn_timeouts(), 0);
}
