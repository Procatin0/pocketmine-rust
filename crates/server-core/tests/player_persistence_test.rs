use std::net::SocketAddr;
use std::sync::Arc;

use php_plugin_bridge::{PlayerContext, PluginHost};
use server_core::{PlayerSession, ServerPluginHost, SessionManager, SessionState};

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pm-rust-persist-{}-{}",
        std::process::id(),
        tag
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("players")).unwrap();
    dir
}

fn test_host(dir: &std::path::Path) -> (ServerPluginHost, Arc<server_core::PlayerRegistry>) {
    let manager = SessionManager::new();
    let host = ServerPluginHost::new(
        Arc::clone(manager.world()),
        Arc::clone(manager.registry()),
        Arc::clone(manager.ops()),
        "test motd",
        100,
    );
    host.set_data_dir(dir.to_path_buf());
    let registry = manager.registry().clone();
    // Leak the manager's world/registry Arcs are shared; manager itself is
    // dropped but the Arcs keep them alive.
    (host, registry)
}

#[test]
fn load_then_register_restores_canonical_position_and_times() {
    let dir = temp_dir("load-register");
    // Seed a previous quit save (JSON v2, canonical Y=65, no +64).
    let seed = serde_json::json!({
        "name": "Procatin0",
        "xuid": "2535412345678901",
        "display_name": "Procatin0",
        "x": 12.5, "y": 65.0, "z": -3.25,
        "yaw": 90.0, "pitch": -15.0,
        "gamemode": 1, "health": 18.0, "max_health": 20.0,
        "food": 18.0, "saturation": 16.0,
        "xp": 0.25, "xp_level": 5, "air": 280,
        "flying": false, "sneaking": false, "sprinting": true, "swimming": false, "gliding": false,
        "world": "world", "is_op": false,
        "spawn_point": [0.0, 64.0, 0.0],
        "first_played": 1700000000.5, "last_played": 1700001000.5,
    });
    std::fs::write(dir.join("players/procatin0.json"), serde_json::to_vec(&seed).unwrap()).unwrap();

    let (host, registry) = test_host(&dir);

    // joinApi path: player.load BEFORE player.register.
    let loaded = host.call("player.load", &serde_json::json!({ "name": "Procatin0" })).unwrap();
    assert_eq!(loaded["has_data"], true);
    assert_eq!(loaded["y"], 65.0);
    assert_eq!(loaded["xuid"], "2535412345678901");

    let registered = host
        .call(
            "player.register",
            &serde_json::json!({
                "name": "Procatin0",
                "display_name": "Procatin0",
                "address": "127.0.0.1:25565",
                "gamemode": loaded["gamemode"].as_i64().unwrap(),
                "x": loaded["x"], "y": loaded["y"], "z": loaded["z"],
                "yaw": loaded["yaw"], "pitch": loaded["pitch"],
                "world": "world",
                "health": loaded["health"], "max_health": loaded["max_health"],
                "is_op": false,
                "xuid": loaded["xuid"],
                "first_played": loaded["first_played"],
                "last_played": loaded["last_played"],
            }),
        )
        .unwrap();
    assert_eq!(registered, serde_json::json!({ "ok": true }));

    let snapshot = registry.get("procatin0").expect("registered");
    assert_eq!(snapshot.y, 65.0, "Y canónico sin offset +64");
    assert_eq!(snapshot.x, 12.5);
    assert_eq!(snapshot.xuid, "2535412345678901");
    assert_eq!(snapshot.first_played, 1700000000.5);
    assert!(snapshot.has_played_before());

    // register without times still inherits them from disk.
    registry.remove("Procatin0");
    host.call(
        "player.register",
        &serde_json::json!({
            "name": "Procatin0", "address": "127.0.0.1:25565",
            "x": 0.0, "y": 64.0, "z": 0.0,
        }),
    )
    .unwrap();
    let snapshot = registry.get("procatin0").expect("re-registered");
    assert_eq!(snapshot.first_played, 1700000000.5, "hereda tiempos de disco");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quit_save_writes_direct_json_v2_without_staging() {
    let dir = temp_dir("quit-save");
    let (host, registry) = test_host(&dir);

    host.call(
        "player.register",
        &serde_json::json!({
            "name": "Alex", "display_name": "Alex", "address": "127.0.0.1:25565",
            "gamemode": 0, "x": 1.5, "y": 63.0, "z": -2.5,
            "yaw": 90.0, "pitch": 0.0, "world": "world",
            "health": 20.0, "max_health": 20.0, "is_op": false,
        }),
    )
    .unwrap();

    // leaveApi: PlayerQuitEvent fires in PHP, THEN player.save runs.
    host.call(
        "player.save",
        &serde_json::json!({
            "name": "Alex", "x": 3.5, "y": 64.0, "z": 0.5,
            "yaw": 180.0, "pitch": 5.0, "gamemode": 0, "health": 19.0,
            "max_health": 20.0, "world": "world",
        }),
    )
    .unwrap();

    let path = dir.join("players/alex.json");
    assert!(path.exists(), "guarda directo en players/<name>.json");
    assert!(!dir.join("players/alex.json.staging").exists());
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let object = saved.as_object().expect("v2 object");
    assert_eq!(object.len(), 26, "JSON v2 = 26 campos: {:?}", object.keys().collect::<Vec<_>>());
    assert_eq!(saved["x"], 3.5);
    assert_eq!(saved["y"], 64.0);

    // Registry is still in sync and reload finds the data.
    let loaded = host.call("player.load", &serde_json::json!({ "name": "Alex" })).unwrap();
    assert_eq!(loaded["has_data"], true);
    let snapshot = registry.get("alex").expect("registered");
    assert_eq!(snapshot.x, 3.5);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn has_played_before_false_then_true() {
    let dir = temp_dir("has-played");
    let addr: SocketAddr = "127.0.0.1:54349".parse().unwrap();

    // New Bedrock player: no file yet -> false.
    let mut fresh = PlayerSession::new(90, addr);
    fresh.username = "Newbie".to_string();
    fresh.data_dir = Some(dir.clone());
    assert!(!fresh.has_played_before());

    // First quit save (first == last).
    fresh.position = bedrock_protocol::Vector3f::new(1.0, 64.0, 1.0);
    fresh.save_player_data().unwrap();
    assert!(dir.join("players/newbie.json").exists());

    // Reload in a new session: file existed -> true.
    let mut returning = PlayerSession::new(91, addr);
    returning.username = "Newbie".to_string();
    returning.data_dir = Some(dir.clone());
    assert!(returning.load_player_data());
    assert!(returning.has_played_before());
    assert!(returning.first_played > 0.0);

    // Host view agrees.
    let (host, _registry) = test_host(&dir);
    let played = host
        .call("player.has_played_before", &serde_json::json!({ "name": "Newbie" }))
        .unwrap();
    assert_eq!(played["has_played_before"], false, "primera sesión: first==last");

    // Manually age the file (second join later than the first).
    let mut saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("players/newbie.json")).unwrap()).unwrap();
    let first = saved["first_played"].as_f64().unwrap();
    saved["last_played"] = serde_json::json!(first + 3600.0);
    std::fs::write(dir.join("players/newbie.json"), serde_json::to_vec(&saved).unwrap()).unwrap();
    let played = host
        .call("player.has_played_before", &serde_json::json!({ "name": "Newbie" }))
        .unwrap();
    assert_eq!(played["has_played_before"], true);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn list_counts_virtual_players_once() {
    let manager = SessionManager::new();
    let mut virtual_player =
        PlayerContext::new("Procatin0", "127.0.0.1:25565".parse().unwrap());
    virtual_player.is_virtual = true;
    virtual_player.x = 0.5;
    virtual_player.y = 65.0;
    virtual_player.z = 0.5;
    manager.registry().upsert(virtual_player);
    manager
        .registry()
        .upsert(PlayerContext::new("Steve", "127.0.0.1:19132".parse().unwrap()));

    // Same name in different case must not double-count.
    let mut duplicate =
        PlayerContext::new("procatin0", "127.0.0.1:25565".parse().unwrap());
    duplicate.is_virtual = true;
    manager.registry().upsert(duplicate);

    let names = manager.registry().names();
    assert_eq!(names.len(), 2, "names: {:?}", names);

    let feedback = manager.execute_console_command("/list").expect("/list is native");
    assert_eq!(feedback[0], "There are 2/1000 players online:");
    assert!(feedback[1].to_lowercase().contains("procatin0"));
}

#[test]
fn save_all_persists_virtuals_directly() {
    let dir = temp_dir("save-all");
    let manager = SessionManager::new();
    manager.enable_persistence(dir.clone());

    let mut virtual_player =
        PlayerContext::new("Procatin0", "127.0.0.1:25565".parse().unwrap());
    virtual_player.is_virtual = true;
    virtual_player.x = 2.5;
    virtual_player.y = 63.0;
    virtual_player.z = -1.5;
    virtual_player.first_played = 1700000000.5;
    manager.registry().upsert(virtual_player);

    let addr: SocketAddr = "127.0.0.1:54350".parse().unwrap();
    let mut session = PlayerSession::new(92, addr);
    session.username = "Alex".to_string();
    session.state = SessionState::InGame;
    session.joined = true;
    session.data_dir = Some(dir.clone());
    session.position = bedrock_protocol::Vector3f::new(7.5, 70.0, -3.25);
    manager.add_session(session);

    manager.save_all().unwrap();

    assert!(dir.join("players/procatin0.json").exists(), "virtual saved by save_all");
    assert!(dir.join("players/alex.json").exists(), "bedrock saved by save_all");
    assert!(dir.join("world.json").exists());

    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("players/procatin0.json")).unwrap()).unwrap();
    assert_eq!(saved.as_object().unwrap().len(), 26);
    assert_eq!(saved["first_played"], 1700000000.5, "timers preserved");
    assert!(saved["last_played"].as_f64().unwrap() >= 1700000000.5);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn update_position_never_writes_disk_until_quit_save() {
    let dir = temp_dir("update-position");
    let (host, _registry) = test_host(&dir);
    host.call(
        "player.register",
        &serde_json::json!({
            "name": "Alex", "address": "127.0.0.1:25565",
            "x": 0.0, "y": 64.0, "z": 0.0,
        }),
    )
    .unwrap();
    host.call(
        "player.update_position",
        &serde_json::json!({ "name": "Alex", "x": 9.5, "y": 66.0, "z": 9.5 }),
    )
    .unwrap();
    assert!(
        !dir.join("players/alex.json").exists(),
        "heartbeat no escribe disco"
    );
    host.call(
        "player.save",
        &serde_json::json!({ "name": "Alex", "x": 9.5, "y": 66.0, "z": 9.5 }),
    )
    .unwrap();
    assert!(dir.join("players/alex.json").exists());

    let _ = std::fs::remove_dir_all(&dir);
}
