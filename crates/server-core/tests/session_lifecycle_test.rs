use std::net::SocketAddr;

use bedrock_protocol::{
    LoginPacket, Packet, PlayStatusPacket, RequestChunkRadiusPacket, RequestNetworkSettingsPacket,
    ResourcePackClientResponsePacket, Writer,
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
            assert_eq!(pkt.compression_threshold, 65535);
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
            assert_eq!(p.game_version, "1.21.0");
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
    assert_eq!(responses.len(), 3);

    // Verify StartGamePacket
    match &responses[0] {
        OutgoingPacket::StartGame(p) => {
            assert_eq!(p.actor_unique_id, 42);
            assert_eq!(p.actor_runtime_id, 42);
            assert_eq!(p.player_gamemode, 0);
            assert_eq!(p.level_settings.level_name, "PocketMine-Rust World");
        }
        other => panic!("Expected StartGame, got {:?}", other),
    }

    // Verify spawn LevelChunkPacket
    match &responses[1] {
        OutgoingPacket::LevelChunk(chunk) => {
            assert_eq!(chunk.chunk_x, 0);
            assert_eq!(chunk.chunk_z, 0);
            assert_eq!(chunk.sub_chunk_count, 1);
            assert!(!chunk.payload.is_empty());
        }
        other => panic!("Expected LevelChunk, got {:?}", other),
    }

    // Verify PlayStatusPacket (PLAYER_SPAWN)
    match &responses[2] {
        OutgoingPacket::PlayStatus(p) => {
            assert_eq!(p.status, PlayStatusPacket::PLAYER_SPAWN);
        }
        other => panic!("Expected PlayStatus(PLAYER_SPAWN), got {:?}", other),
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
    for resp in responses {
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
