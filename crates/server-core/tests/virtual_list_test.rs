use server_core::SessionManager;
use php_plugin_bridge::PlayerContext;

#[test]
fn console_list_counts_virtual_players() {
    let manager = SessionManager::new();
    // Simula el alta del JavaBridge (`player.register`): entrada virtual
    // sin sesión Bedrock.
    let mut virtual_player = PlayerContext::new("Procatin0", "127.0.0.1:25565".parse().unwrap());
    virtual_player.is_virtual = true;
    virtual_player.x = 0.5;
    virtual_player.y = 65.0;
    virtual_player.z = 0.5;
    manager.registry().upsert(virtual_player);
    manager.registry().upsert(PlayerContext::new("Steve", "127.0.0.1:19132".parse().unwrap()));

    // El snapshot de consola debe incluir ambos sin duplicar el nombre.
    // Se verifica vía el registro compartido (fuente de online_players_snapshot).
    let names = manager.registry().names();
    assert!(names.iter().any(|n| n.eq_ignore_ascii_case("procatin0")));
    assert!(names.iter().any(|n| n.eq_ignore_ascii_case("steve")));

    let feedback = manager
        .execute_console_command("/list")
        .expect("/list is native");

    assert_eq!(feedback[0], "There are 2/1000 players online:");
}
