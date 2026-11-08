use server_core::SessionManager;
use server_core::command::CommandAction;
use php_plugin_bridge::PlayerContext;

#[test]
fn tp_console_updates_virtual_player_registry() {
    let manager = SessionManager::new();
    let mut v = PlayerContext::new("Procatin0", "127.0.0.1:25565".parse().unwrap());
    v.is_virtual = true;
    v.x = 10.0; v.y = 64.0; v.z = 10.0; v.yaw = 0.0; v.pitch = 0.0;
    manager.registry().upsert(v);

    let out = manager.execute_console_command("/tp Procatin0 0 100 0").expect("native");
    assert!(out.iter().any(|l| l.contains("Teleporting")), "feedback: {:?}", out);
    let p = manager.registry().get("Procatin0").expect("still registered");
    assert_eq!((p.x, p.y, p.z), (0.0, 100.0, 0.0), "registry after /tp");
}

#[test]
fn teleport_action_virtual_yaw_pitch_and_plugin_path() {
    let manager = SessionManager::new();
    let mut v = PlayerContext::new("Procatin0", "127.0.0.1:25565".parse().unwrap());
    v.is_virtual = true;
    v.x = 1.0; v.y = 64.0; v.z = 1.0;
    manager.registry().upsert(v);

    manager.apply_command_actions(&[CommandAction::Teleport {
        target: "Procatin0".to_string(),
        position: (5.0, 70.0, -3.0),
        yaw: Some(90.0),
        pitch: Some(-15.0),
    }]);
    let p = manager.registry().get("Procatin0").expect("registered");
    assert_eq!((p.x, p.y, p.z, p.yaw, p.pitch), (5.0, 70.0, -3.0, 90.0, -15.0));
}
