use std::net::SocketAddr;

use bedrock_protocol::{
    LoginPacket, Packet, PlayStatusPacket, PlayerAuthInputPacket, RequestChunkRadiusPacket,
    RequestNetworkSettingsPacket, ResourcePackClientResponsePacket, Vector3f, Writer,
};
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

    assert_eq!(session.state, SessionState::InGame);
    assert!(session.is_in_game());
    assert!(responses.len() >= 3);

    // Verify StartGamePacket is present
    let start_game = responses.iter().find_map(|r| match r {
        OutgoingPacket::StartGame(p) => Some(p),
        _ => None,
    }).expect("Expected StartGamePacket in responses");
    assert_eq!(start_game.actor_unique_id, 42);
    assert_eq!(start_game.actor_runtime_id, 42);
    assert_eq!(start_game.player_gamemode, 0);
    assert_eq!(start_game.world_name, "PocketMine-Rust World");

    // Verify spawn LevelChunkPacket is present
    let chunk = responses.iter().find_map(|r| match r {
        OutgoingPacket::LevelChunk(c) if c.chunk_x == 0 && c.chunk_z == 0 => Some(c),
        _ => None,
    }).expect("Expected LevelChunk (0,0) in responses");
    assert_eq!(chunk.chunk_x, 0);
    assert_eq!(chunk.chunk_z, 0);
    assert_eq!(chunk.sub_chunk_count, 5);
    assert!(!chunk.payload.is_empty());

    // Verify PlayStatusPacket (PLAYER_SPAWN) is present
    let play_status = responses.iter().find_map(|r| match r {
        OutgoingPacket::PlayStatus(p) => Some(p),
        _ => None,
    }).expect("Expected PlayStatusPacket in responses");
    assert_eq!(play_status.status, PlayStatusPacket::PLAYER_SPAWN);

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

    assert_eq!(session.position, Vector3f::new(320.0, 65.0, 0.0));
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

    assert!(session.is_in_game());
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
    assert_eq!(session.position, Vector3f::new(2.0, 65.0, 2.0));
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

    // Max 16 chunks generated in first packet + 1 NetworkChunkPublisherUpdate
    let chunks_gen_1 = responses_1.iter().filter(|r| matches!(r, OutgoingPacket::LevelChunk(_))).count();
    assert_eq!(chunks_gen_1, 16);
    assert!(responses_1.iter().any(|r| matches!(r, OutgoingPacket::NetworkChunkPublisherUpdate(_))));
    assert_eq!(session.sent_chunks.len(), 25 + 16);

    // 3. Second auth input generates the remaining 9 chunks
    let responses_2 = session.handle_player_auth_input(move_to_10_10.clone()).unwrap();
    let chunks_gen_2 = responses_2.iter().filter(|r| matches!(r, OutgoingPacket::LevelChunk(_))).count();
    assert_eq!(chunks_gen_2, 9);
    assert!(responses_2.iter().any(|r| matches!(r, OutgoingPacket::NetworkChunkPublisherUpdate(_))));
    assert_eq!(session.sent_chunks.len(), 25 + 25);

    // 4. Third auth input generates 0 chunks since all 25 chunks in the window are now sent
    let responses_3 = session.handle_player_auth_input(move_to_10_10).unwrap();
    assert!(responses_3.is_empty());
}
