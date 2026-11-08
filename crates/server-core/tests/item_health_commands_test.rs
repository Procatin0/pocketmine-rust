use std::net::SocketAddr;

use bedrock_protocol::packets::{
    InventoryTransactionPacket, NetworkInventoryAction, TransactionData,
};
use bedrock_protocol::Writer;
use server_core::inventory::WINDOW_ID_INVENTORY;
use server_core::{PlayerSession, SessionManager, SessionState};

#[test]
fn kill_sends_death_packets_and_respawn_restores_health() {
    let addr: SocketAddr = "127.0.0.1:54340".parse().unwrap();
    let mut session = PlayerSession::new(70, addr);
    session.username = "Steve".to_string();
    session.state = SessionState::InGame;

    session.kill_player();
    assert_eq!(session.health, 0.0);
    assert!(session.is_dead);

    let ids: Vec<u32> = session
        .pending_outgoing
        .iter()
        .map(|packet| packet.packet_id())
        .collect();
    assert!(ids.contains(&0x2A), "SetHealthPacket is missing");
    assert!(ids.contains(&0x1B), "ActorEventPacket is missing");
    assert!(ids.contains(&0x2D), "RespawnPacket is missing");

    // The client accepts the respawn with RespawnPacket state=2.
    let mut writer = Writer::new();
    writer.write_f32_le(0.0);
    writer.write_f32_le(0.0);
    writer.write_f32_le(0.0);
    writer.write_byte(2);
    writer.write_var_u64(70);
    let payload = writer.into_bytes().to_vec();

    let responses = session
        .handle_packet(0x2D, &payload)
        .expect("handle the client respawn");

    assert!(!session.is_dead);
    assert_eq!(session.health, 20.0);
    let ids: Vec<u32> = responses.iter().map(|packet| packet.packet_id()).collect();
    assert!(ids.contains(&0x2A), "health is not restored");
    assert!(ids.contains(&0x2D), "READY_TO_SPAWN is missing");
    // 0x13 is MovePlayerPacket; 0x12 is MoveActorAbsolute (other entities).
    assert!(ids.contains(&0x13), "the teleport packet is missing");

    // A second, unsolicited respawn packet is ignored.
    assert!(session.handle_packet(0x2D, &payload).unwrap().is_empty());
}

#[test]
fn give_and_clear_change_the_server_inventory() {
    let manager = SessionManager::new();
    let addr: SocketAddr = "127.0.0.1:54341".parse().unwrap();
    let mut session = PlayerSession::new(71, addr);
    session.username = "Steve".to_string();
    session.state = SessionState::InGame;
    session.joined = true;
    manager.add_session(session);

    manager
        .execute_console_command("/give Steve stone 3")
        .expect("/give is native");

    let items = manager.player_inventory("Steve").expect("inventory exists");
    let held: Vec<_> = items.iter().filter(|item| !item.is_empty()).collect();
    assert_eq!(held.len(), 1, "the stack should land in a single slot");
    assert_eq!(held[0].network_id, 1, "minecraft:stone");
    assert_eq!(held[0].count, 3);

    // Giving more merges into the same stack.
    manager
        .execute_console_command("/give Steve minecraft:stone 5")
        .expect("/give is native");
    let items = manager.player_inventory("Steve").expect("inventory exists");
    assert_eq!(items.iter().map(|item| item.count as u32).sum::<u32>(), 8);

    manager
        .execute_console_command("/clear Steve minecraft:stone")
        .expect("/clear is native");
    let items = manager.player_inventory("Steve").expect("inventory exists");
    assert!(items.iter().all(|item| item.is_empty()));
}

#[test]
fn inventory_transaction_moves_items_between_slots() {
    let manager = SessionManager::new();
    let addr: SocketAddr = "127.0.0.1:54343".parse().unwrap();
    let mut session = PlayerSession::new(73, addr);
    session.username = "Steve".to_string();
    session.state = SessionState::InGame;
    session.joined = true;
    manager.add_session(session);
    let session_id = manager.session_id_for("Steve").expect("session");

    manager
        .execute_console_command("/give Steve stone 3")
        .expect("/give is native");

    let packet = InventoryTransactionPacket {
        request_id: 11,
        request_changed_slots: None,
        actions: vec![
            NetworkInventoryAction {
                source_type: 0, // SOURCE_CONTAINER
                window_id: Some(WINDOW_ID_INVENTORY as i8),
                source_flags: None,
                inventory_slot: 0,
                old_item: bedrock_protocol::ItemStack::new(1, 3),
                new_item: bedrock_protocol::ItemStack::empty(),
            },
            NetworkInventoryAction {
                source_type: 0,
                window_id: Some(WINDOW_ID_INVENTORY as i8),
                source_flags: None,
                inventory_slot: 5,
                old_item: bedrock_protocol::ItemStack::empty(),
                new_item: bedrock_protocol::ItemStack::new(1, 3),
            },
        ],
        data: TransactionData::Normal,
    };

    let mut writer = Writer::new();
    packet.encode_payload(&mut writer);
    let payload = writer.into_bytes().to_vec();

    let responses = manager
        .handle_session_packet(session_id, 0x1E, &payload)
        .expect("handle the transaction");
    let ids: Vec<u32> = responses.iter().map(|packet| packet.packet_id()).collect();
    assert!(ids.contains(&0x32), "the changed slots must be echoed");

    let inventory = manager.player_inventory("Steve").expect("inventory");
    assert!(inventory[0].is_empty());
    assert_eq!(inventory[5].network_id, 1);
    assert_eq!(inventory[5].count, 3);
}

#[test]
fn stale_inventory_transaction_resyncs_the_windows() {
    let manager = SessionManager::new();
    let addr: SocketAddr = "127.0.0.1:54344".parse().unwrap();
    let mut session = PlayerSession::new(74, addr);
    session.username = "Alex".to_string();
    session.state = SessionState::InGame;
    session.joined = true;
    manager.add_session(session);
    let session_id = manager.session_id_for("Alex").expect("session");

    manager
        .execute_console_command("/give Alex stone 3")
        .expect("/give is native");

    // The client claims to hold 64 stone in slot 0, the server has 3.
    let packet = InventoryTransactionPacket {
        request_id: 12,
        request_changed_slots: None,
        actions: vec![NetworkInventoryAction {
            source_type: 0,
            window_id: Some(WINDOW_ID_INVENTORY as i8),
            source_flags: None,
            inventory_slot: 0,
            old_item: bedrock_protocol::ItemStack::new(1, 64),
            new_item: bedrock_protocol::ItemStack::empty(),
        }],
        data: TransactionData::Normal,
    };
    let mut writer = Writer::new();
    packet.encode_payload(&mut writer);
    let payload = writer.into_bytes().to_vec();

    let responses = manager
        .handle_session_packet(session_id, 0x1E, &payload)
        .expect("handle the transaction");
    let content_packets = responses
        .iter()
        .filter(|packet| packet.packet_id() == 0x31)
        .count();
    // `/give` ya había dejado su propia sincronización pendiente, así que se
    // esperan al menos las cuatro ventanas del resync.
    assert!(
        content_packets >= 4,
        "all four windows are resent, got {content_packets}"
    );

    let inventory = manager.player_inventory("Alex").expect("inventory");
    assert_eq!(inventory[0].count, 3, "the server state wins");
    assert!(manager.player_inventory("Alex").unwrap()[0].network_id == 1);
}

#[test]
fn spawnpoint_changes_where_the_player_respawns() {
    let manager = SessionManager::new();
    let addr: SocketAddr = "127.0.0.1:54345".parse().unwrap();
    let mut session = PlayerSession::new(75, addr);
    session.username = "Alex".to_string();
    session.state = SessionState::InGame;
    session.joined = true;
    manager.add_session(session);
    let session_id = manager.session_id_for("Alex").expect("session");

    manager
        .execute_console_command("/spawnpoint Alex 100 70 -50")
        .expect("/spawnpoint is native");

    manager
        .execute_console_command("/kill Alex")
        .expect("/kill is native");

    let mut writer = Writer::new();
    writer.write_f32_le(0.0);
    writer.write_f32_le(0.0);
    writer.write_f32_le(0.0);
    writer.write_byte(2);
    writer.write_var_u64(75);
    let payload = writer.into_bytes().to_vec();

    manager
        .handle_session_packet(session_id, 0x2D, &payload)
        .expect("handle the client respawn");

    let position = manager.player_position("Alex").expect("player");
    assert_eq!(
        (position.0 as i32, position.1 as i32, position.2 as i32),
        (100, 70, -50)
    );
}

#[test]
fn difficulty_through_the_console_updates_the_world() {
    let manager = SessionManager::new();

    manager
        .execute_console_command("/difficulty easy")
        .expect("/difficulty is native");

    assert_eq!(manager.world_difficulty(), 1);
}

#[test]
fn banning_a_player_kicks_and_blocks_them() {
    let manager = SessionManager::new();
    let addr: SocketAddr = "127.0.0.1:54346".parse().unwrap();
    let mut session = PlayerSession::new(76, addr);
    session.username = "Alex".to_string();
    session.state = SessionState::InGame;
    session.joined = true;
    manager.add_session(session);

    manager
        .execute_console_command("/ban Alex griefing")
        .expect("/ban is native");

    assert_eq!(
        manager.access().name_ban_reason("Alex"),
        Some("griefing".to_string())
    );
    assert_eq!(
        manager.session_state("Alex"),
        Some(SessionState::Disconnected)
    );

    manager
        .execute_console_command("/pardon Alex")
        .expect("/pardon is native");
    assert_eq!(manager.access().name_ban_reason("Alex"), None);
}

#[test]
fn whitelist_on_kicks_non_whitelisted_players() {
    let manager = SessionManager::new();
    let addr: SocketAddr = "127.0.0.1:54347".parse().unwrap();
    let mut session = PlayerSession::new(77, addr);
    session.username = "Alex".to_string();
    session.state = SessionState::InGame;
    session.joined = true;
    manager.add_session(session);

    manager
        .execute_console_command("/whitelist on")
        .expect("/whitelist is native");

    assert_eq!(
        manager.session_state("Alex"),
        Some(SessionState::Disconnected)
    );
}

#[test]
fn stop_sets_the_shutdown_flag() {
    let manager = SessionManager::new();
    assert!(!manager.stop_requested());
    manager
        .execute_console_command("/stop")
        .expect("/stop is native");
    assert!(manager.stop_requested());
}

#[test]
fn save_all_writes_world_and_player_data() {
    let dir = std::env::temp_dir().join(format!("pm-rust-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let manager = SessionManager::new();
    manager.enable_persistence(dir.clone());

    let addr: SocketAddr = "127.0.0.1:54348".parse().unwrap();
    let mut session = PlayerSession::new(78, addr);
    session.username = "Alex".to_string();
    session.state = SessionState::InGame;
    session.joined = true;
    session.data_dir = Some(dir.clone());
    session.position = bedrock_protocol::Vector3f::new(12.5, 70.0, -3.25);
    manager.add_session(session);

    manager
        .execute_console_command("/save-all")
        .expect("/save-all is native");

    assert!(dir.join("world.json").exists());
    assert!(dir.join("players/alex.json").exists());

    let mut restored = PlayerSession::new(79, addr);
    restored.username = "Alex".to_string();
    restored.data_dir = Some(dir.clone());
    assert!(restored.load_player_data());
    assert_eq!(restored.position.x, 12.5);
    assert_eq!(restored.position.y, 70.0);
    assert_eq!(restored.position.z, -3.25);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_off_disables_the_autosave_tick() {
    let manager = SessionManager::new();
    assert!(manager.auto_save_enabled());
    manager
        .execute_console_command("/save-off")
        .expect("/save-off is native");
    assert!(!manager.auto_save_enabled());
    manager
        .execute_console_command("/save-on")
        .expect("/save-on is native");
    assert!(manager.auto_save_enabled());
}

#[test]
fn kill_through_the_console_publishes_zero_health() {
    let manager = SessionManager::new();
    let addr: SocketAddr = "127.0.0.1:54342".parse().unwrap();
    let mut session = PlayerSession::new(72, addr);
    session.username = "Alex".to_string();
    session.state = SessionState::InGame;
    session.joined = true;
    manager.add_session(session);

    manager
        .execute_console_command("/kill Alex")
        .expect("/kill is native");

    let player = manager.registry().get("Alex").expect("player is registered");
    assert_eq!(player.health, 0.0);
}
