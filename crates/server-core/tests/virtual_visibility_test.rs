use std::net::SocketAddr;
use std::sync::Arc;

use php_plugin_bridge::{PlayerContext, PluginHost};
use server_core::{PlayerSession, ServerPluginHost, SessionManager, SessionState};

fn joined_bedrock(id: u64, username: &str) -> PlayerSession {
    let addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
    let mut session = PlayerSession::new(id, addr);
    session.username = username.to_string();
    session.state = SessionState::InGame;
    session.joined = true;
    session.spawned = true;
    session
}

fn register_virtual(manager: &SessionManager, name: &str) {
    let mut player = PlayerContext::new(name, "127.0.0.1:25565".parse().unwrap());
    player.is_virtual = true;
    player.x = 2.5;
    player.y = 63.0;
    player.z = -1.5;
    manager.registry().upsert(player);
}

#[test]
fn virtual_player_spawn_move_and_remove_reach_bedrock_viewer() {
    let manager = SessionManager::new();
    manager.add_session(joined_bedrock(1, "Steve"));

    // Java joins: spawn (PlayerList + AddPlayer) for the Bedrock viewer.
    register_virtual(&manager, "Procatin0");
    assert_eq!(manager.poll_virtual_players(), 2);
    // Idempotent without changes.
    assert_eq!(manager.poll_virtual_players(), 0);

    // Java moves: one MoveActorAbsolute.
    let mut player = manager
        .registry()
        .get("Procatin0")
        .expect("virtual still registered");
    player.apply_position(4.5, 64.0, -1.5, player.yaw, player.pitch);
    manager.registry().upsert(player);
    assert_eq!(manager.poll_virtual_players(), 1);
    assert_eq!(manager.poll_virtual_players(), 0);

    // Java leaves: RemoveActor + PlayerList removal.
    assert!(manager.registry().remove_if_virtual("Procatin0"));
    assert_eq!(manager.poll_virtual_players(), 2);
    assert_eq!(manager.poll_virtual_players(), 0);
}

#[test]
fn host_notify_wires_virtual_spawn_immediately() {
    let manager = Arc::new(SessionManager::new());
    manager.add_session(joined_bedrock(1, "Steve"));

    let host = Arc::new(ServerPluginHost::new(
        Arc::clone(manager.world()),
        Arc::clone(manager.registry()),
        Arc::clone(manager.ops()),
        "test motd",
        100,
    ));
    let sync = Arc::clone(&manager);
    host.set_virtual_notify(move || {
        sync.poll_virtual_players();
    });

    // The host call itself triggers the sync (no explicit poll needed).
    let result = host
        .call(
            "player.register",
            &serde_json::json!({
                "name": "Procatin0",
                "display_name": "Procatin0",
                "address": "127.0.0.1:25565",
                "gamemode": 1,
                "x": 2.5, "y": 63.0, "z": -1.5,
                "yaw": 0.0, "pitch": 0.0,
                "world": "world",
                "health": 20.0, "max_health": 20.0,
                "is_op": false,
            }),
        )
        .expect("player.register");
    assert_eq!(result, serde_json::json!({ "ok": true }));
    // Already consumed by the notify: an explicit poll has nothing to do.
    assert_eq!(manager.poll_virtual_players(), 0);

    // Movement via the host call also syncs immediately.
    host.call(
        "player.update_position",
        &serde_json::json!({ "name": "Procatin0", "x": 4.5, "y": 64.0, "z": -1.5 }),
    )
    .expect("player.update_position");
    assert_eq!(manager.poll_virtual_players(), 0);
}

#[test]
fn bedrock_newcomer_sees_existing_virtual_before_any_poll() {
    let manager = SessionManager::new();
    register_virtual(&manager, "Procatin0");
    manager.add_session(joined_bedrock(1, "Steve"));

    // The join broadcast alone must spawn the Java body for the new Bedrock.
    manager.broadcast_add_player(1);
    // Nothing left for a later poll (broadcast already synced virtuals).
    assert_eq!(manager.poll_virtual_players(), 0);
}
