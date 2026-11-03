use bedrock_protocol::{
    BlockPos, ChunkRadiusUpdatedPacket, LevelChunkPacket, LevelSettings, LoginPacket, Packet,
    Reader, RequestChunkRadiusPacket, ResourcePackClientResponsePacket, ResourcePackInfoEntry,
    ResourcePackStackEntry, ResourcePackStackPacket, ResourcePacksInfoPacket, StartGamePacket,
    Vector3f, Writer,
};

#[test]
fn test_login_packet_roundtrip() {
    let packet = LoginPacket::new(685, "sample_jwt_auth_and_client_data_token_string");
    assert_eq!(packet.packet_id(), 0x01);

    let mut writer = Writer::new();
    packet.encode(&mut writer).expect("encode should succeed");

    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    let decoded = LoginPacket::decode(&mut reader).expect("decode should succeed");

    assert_eq!(reader.remaining(), 0);
    assert_eq!(packet, decoded);
    assert_eq!(decoded.protocol_version, 685);
    assert_eq!(
        decoded.connection_request,
        "sample_jwt_auth_and_client_data_token_string"
    );
}

#[test]
fn test_resource_packs_info_packet_empty_roundtrip() {
    let packet = ResourcePacksInfoPacket {
        must_accept: false,
        has_addons: false,
        has_scripts: false,
        disable_vibrant_visuals: false,
        world_template_uuid: [0u8; 16],
        world_template_version: String::new(),
        behavior_packs: Vec::new(),
        resource_packs: Vec::new(),
    };
    assert_eq!(packet.packet_id(), 0x06);

    let mut writer = Writer::new();
    packet.encode(&mut writer).expect("encode should succeed");

    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    let decoded = ResourcePacksInfoPacket::decode(&mut reader).expect("decode should succeed");

    assert_eq!(reader.remaining(), 0);
    assert_eq!(packet, decoded);
}

#[test]
fn test_resource_packs_info_packet_with_entries_roundtrip() {
    let packet = ResourcePacksInfoPacket {
        must_accept: true,
        has_addons: false,
        has_scripts: true,
        disable_vibrant_visuals: false,
        world_template_uuid: [1u8; 16],
        world_template_version: "1.0.0".to_string(),
        behavior_packs: Vec::new(),
        resource_packs: vec![ResourcePackInfoEntry {
            pack_id: "00000000-0000-0000-0000-000000000002".to_string(),
            version: "2.1.0".to_string(),
            pack_size: 2048576,
            content_key: "".to_string(),
            sub_pack_name: "".to_string(),
            content_identity: "identity_rp".to_string(),
            has_scripts: false,
            rtx_enabled: true,
        }],
    };

    let mut writer = Writer::new();
    packet.encode(&mut writer).expect("encode should succeed");

    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    let decoded = ResourcePacksInfoPacket::decode(&mut reader).expect("decode should succeed");

    assert_eq!(reader.remaining(), 0);
    assert_eq!(packet, decoded);
}

#[test]
fn test_resource_pack_stack_packet_roundtrip() {
    let packet = ResourcePackStackPacket {
        must_accept: true,
        behavior_packs: Vec::new(),
        resource_packs: vec![ResourcePackStackEntry {
            pack_id: "rp-uuid-2".to_string(),
            version: "1.2.0".to_string(),
            sub_pack_name: "high_res".to_string(),
        }],
        game_version: "1.26.60".to_string(),
        experiments_previously_enabled: false,
        has_editor_packs: false,
    };
    assert_eq!(packet.packet_id(), 0x07);

    let mut writer = Writer::new();
    packet.encode(&mut writer).expect("encode should succeed");

    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    let decoded = ResourcePackStackPacket::decode(&mut reader).expect("decode should succeed");

    assert_eq!(reader.remaining(), 0);
    assert_eq!(packet, decoded);
}

#[test]
fn test_resource_pack_client_response_packet_roundtrip() {
    let statuses = [
        ResourcePackClientResponsePacket::STATUS_REFUSED,
        ResourcePackClientResponsePacket::STATUS_SEND_PACKS,
        ResourcePackClientResponsePacket::STATUS_HAVE_ALL_PACKS,
        ResourcePackClientResponsePacket::STATUS_COMPLETED,
    ];

    for status in statuses {
        let packet = ResourcePackClientResponsePacket::new(
            status,
            vec!["pack-id-alpha".to_string(), "pack-id-beta".to_string()],
        );
        assert_eq!(packet.packet_id(), 0x08);

        let mut writer = Writer::new();
        packet.encode(&mut writer).expect("encode should succeed");

        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        let decoded =
            ResourcePackClientResponsePacket::decode(&mut reader).expect("decode should succeed");

        assert_eq!(reader.remaining(), 0);
        assert_eq!(packet, decoded);
        assert_eq!(decoded.response_status, status);
    }
}

#[test]
fn test_start_game_packet_roundtrip() {
    let packet = StartGamePacket {
        actor_unique_id: -123456789012345,
        actor_runtime_id: 987654321098765,
        player_gamemode: 1, // Creative
        player_position: Vector3f::new(100.5, 64.0, -250.75),
        pitch: 15.5,
        yaw: 180.0,
        level_settings: LevelSettings {
            seed: 9876543210123456,
            spawn_position: BlockPos::new(100, 64, -250),
            generator_type: 1,
            world_gamemode: 0,
            hardcore: false,
            difficulty: 2,
            dimension: 0,
            level_name: "PocketMine-Rust World".to_string(),
        },
    };
    assert_eq!(packet.packet_id(), 0x0B);

    let mut writer = Writer::new();
    packet.encode(&mut writer).expect("encode should succeed");

    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    let decoded = StartGamePacket::decode(&mut reader).expect("decode should succeed");

    assert_eq!(reader.remaining(), 0);
    assert_eq!(packet, decoded);
}

#[test]
fn test_chunk_radius_packets_roundtrip() {
    // RequestChunkRadiusPacket (0x45)
    let request_packet = RequestChunkRadiusPacket::new(12, 16);
    assert_eq!(request_packet.packet_id(), 0x45);

    let mut writer = Writer::new();
    request_packet
        .encode(&mut writer)
        .expect("encode should succeed");
    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    let decoded_request =
        RequestChunkRadiusPacket::decode(&mut reader).expect("decode should succeed");
    assert_eq!(reader.remaining(), 0);
    assert_eq!(request_packet, decoded_request);

    // ChunkRadiusUpdatedPacket (0x46)
    let updated_packet = ChunkRadiusUpdatedPacket::new(10);
    assert_eq!(updated_packet.packet_id(), 0x46);

    let mut writer2 = Writer::new();
    updated_packet
        .encode(&mut writer2)
        .expect("encode should succeed");
    let bytes2 = writer2.into_bytes();
    let mut reader2 = Reader::new(&bytes2);
    let decoded_updated =
        ChunkRadiusUpdatedPacket::decode(&mut reader2).expect("decode should succeed");
    assert_eq!(reader2.remaining(), 0);
    assert_eq!(updated_packet, decoded_updated);
    assert_eq!(decoded_updated.radius, 10);
}

#[test]
fn test_level_chunk_packet_roundtrip() {
    let dummy_subchunk_payload = vec![0x08, 0x01, 0x01, 0x00, 0x00, 0xAA, 0xBB, 0xCC, 0xDD];
    let packet = LevelChunkPacket::new(4, -8, 2, dummy_subchunk_payload);
    assert_eq!(packet.packet_id(), 0x3A);

    let mut writer = Writer::new();
    packet.encode(&mut writer).expect("encode should succeed");

    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes);
    let decoded = LevelChunkPacket::decode(&mut reader).expect("decode should succeed");

    assert_eq!(reader.remaining(), 0);
    assert_eq!(packet, decoded);
    assert_eq!(decoded.chunk_x, 4);
    assert_eq!(decoded.chunk_z, -8);
    assert_eq!(decoded.sub_chunk_count, 2);
    assert_eq!(decoded.payload.len(), 9);
}
