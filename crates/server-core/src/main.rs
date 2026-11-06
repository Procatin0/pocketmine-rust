use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{info, warn};
use nethernet_rs::{NetherNetIdentity, ServerInfo, SignalingServer};
use php_plugin_bridge::{CommandSender, Event, PluginManager, ServerCommandEvent};
use raklib_rs::{RakNetEvent, RakNetListener};
use server_core::{ServerConfig, ServerPluginHost, SessionManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    server_core::logging::init();

    info!("============================================================");
    info!(" Starting PocketMine-Rust High Performance Bedrock Server   ");
    info!("============================================================");

    // Cargar configuración desde server.properties y pocketmine.yml
    let config = ServerConfig::load_or_create(Path::new("."));

    let server_info = ServerInfo {
        name: config.motd.clone(),
        max_players: config.max_players,
        online_auth: true,
        self_signed_auth: true,
        whitelist: false,
        allow_list: false,
        ..Default::default()
    };

    // 1. Inicializar gestor de sesiones de jugadores con el generador de mundo configurado
    let world_generator = config.create_world_generator();
    info!(
        "World generator initialized (level-type: {}, generator-type: {}, spawn: {:?})",
        config.level_type,
        world_generator.generator_type(),
        world_generator.spawn_position()
    );
    let session_manager = Arc::new(SessionManager::with_generator_gamemode_and_view_distance(
        world_generator,
        config.gamemode,
        config.view_distance,
    ));
    session_manager.set_allow_cheats(config.allow_cheats);
    session_manager.set_max_players(config.max_players);
    session_manager.set_auto_save_enabled(config.auto_save);
    info!(
        "SessionManager initialized (active sessions: {}, allow-cheats: {})",
        session_manager.session_count(),
        if config.allow_cheats { "ON" } else { "OFF" }
    );
    info!(
        "Native commands ready: {} (usa /help para ver la lista completa)",
        server_core::command::COMMANDS.len()
    );

    // 2. Motor de plugins: se crea antes del transporte para que las sesiones
    // que entren ya tengan eventos y comandos disponibles.
    let ops = load_ops(Path::new("ops.txt"));
    session_manager.set_ops(ops);
    info!("Loaded {} operator(s) from ops.txt", session_manager.ops().read().len());
    session_manager.access().load_from_dir(Path::new("."));
    session_manager.enable_persistence(std::path::PathBuf::from("."));
    if session_manager.world_state().load_from(Path::new("world.json")) {
        info!("Loaded world.json");
    }
    info!(
        "Loaded {} name ban(s), {} IP ban(s) and {} whitelisted player(s)",
        session_manager.access().banned_names().len(),
        session_manager.access().banned_ips().len(),
        session_manager.access().whitelist_entries().len()
    );

    let plugin_manager = Arc::new(PluginManager::new());
    let host = Arc::new(ServerPluginHost::new(
        Arc::clone(session_manager.world()),
        Arc::clone(session_manager.registry()),
        Arc::clone(session_manager.ops()),
        config.motd.clone(),
        config.max_players,
    ));
    plugin_manager.set_host(host);
    session_manager.set_plugin_manager(Arc::clone(&plugin_manager));

    let _ = plugin_manager.init_runtime().await;
    match plugin_manager.load_plugins_from_directory("plugins").await {
        Ok(count) => {
            info!("PluginManager loaded {} plugin(s) from 'plugins/'", count);
            if let Err(e) = plugin_manager.enable_plugins().await {
                tracing::error!("Failed enabling plugins: {}", e);
            }
        }
        Err(e) => {
            tracing::warn!("Could not load plugins directory: {}", e);
        }
    }

    // 3. Tick loop: 20 TPS, ejecuta el scheduler PHP y aplica sus acciones.
    {
        let plugin_manager = Arc::clone(&plugin_manager);
        let session_manager = Arc::clone(&session_manager);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut tick = 0u64;
            loop {
                interval.tick().await;
                session_manager.metrics().record_tick();
                tick = tick.wrapping_add(1);
                let manager = Arc::clone(&plugin_manager);
                let result =
                    tokio::task::spawn_blocking(move || manager.tick_sync(tick)).await;
                match result {
                    Ok(Ok(actions)) => {
                        if !actions.is_empty() {
                            session_manager.apply_actions(&actions, 0);
                        }
                    }
                    Ok(Err(error)) => warn!("Plugin tick {} failed: {}", tick, error),
                    Err(error) => warn!("Plugin tick task panicked: {}", error),
                }

                // PocketMine's autosave interval is 6000 ticks (5 minutes).
                if tick % 6000 == 0 && session_manager.auto_save_enabled() {
                    let saver = Arc::clone(&session_manager);
                    let _ = tokio::task::spawn_blocking(move || {
                        if let Err(error) = saver.save_all() {
                            warn!("Autosave failed: {}", error);
                        }
                    })
                    .await;
                }
            }
        });
    }

    // 4. Consola: ServerCommandEvent + comandos de plugins.
    {
        let plugin_manager = Arc::clone(&plugin_manager);
        let session_manager = Arc::clone(&session_manager);
        tokio::spawn(async move {
            let mut lines = BufReader::new(tokio::io::stdin()).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let line = line.trim().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        let manager = Arc::clone(&plugin_manager);
                        let sessions = Arc::clone(&session_manager);
                        let result = tokio::task::spawn_blocking(move || {
                            let mut event = ServerCommandEvent::new(line.clone());
                            let mut actions = manager.dispatch_event_sync(&mut event).unwrap_or_default();
                            if !event.is_cancelled() {
                                let command = event.command;
                                // Native commands first (tp, gamemode, say, ...).
                                if let Some(feedback) = sessions.execute_console_command(&command) {
                                    for line in feedback {
                                        server_core::logging::console_message(&line);
                                    }
                                    return actions;
                                }
                                match manager.execute_command_sync(&CommandSender::Console, &command) {
                                    Ok(Some(outcome)) => actions.extend(outcome.actions),
                                    Ok(None) => server_core::logging::console_message(
                                        &server_core::command::not_found_message(
                                            command.split_whitespace().next().unwrap_or(""),
                                        ),
                                    ),
                                    Err(error) => {
                                        tracing::error!("Console command failed: {}", error)
                                    }
                                }
                            }
                            actions
                        })
                        .await;
                        if let Ok(actions) = result {
                            session_manager.apply_actions(&actions, 0);
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        warn!("Console input closed: {}", error);
                        break;
                    }
                }
            }
        });
    }

    // 5. Transporte principal de señalización: TCP 19132 EXCLUSIVAMENTE (HTTP / HTTPS demux)
    // El puerto 19133 es de WebRTC (UDP) y NUNCA debe escuchar en TCP (idéntico a Axolotl/PocketMine)
    let signaling_addr: SocketAddr = "0.0.0.0:19132".parse()?;
    let webrtc_udp_port: u16 = 19133;

    struct BedrockPacketHandler {
        session_manager: Arc<SessionManager>,
    }

    impl nethernet_rs::NetherNetPacketHandler for BedrockPacketHandler {
        fn handle_message(&self, peer: SocketAddr, channel: &str, data: bytes::Bytes) -> Vec<bytes::Bytes> {
            if channel != "ReliableDataChannel" && channel != "UnreliableDataChannel" {
                return Vec::new();
            }
            self.session_manager.dispatch_webrtc_packet(peer, &data)
        }

        fn on_disconnect(&self, peer: SocketAddr) {
            info!("[:WebRTC] Peer {} disconnected; firing PlayerQuitEvent", peer);
            self.session_manager.handle_peer_disconnect(peer);
        }
    }

    let identity = Arc::new(NetherNetIdentity::new());
    let mut signaling_server = SignalingServer::new(
        signaling_addr,
        server_info,
        webrtc_udp_port,
        identity,
    );
    signaling_server.set_packet_handler(Arc::new(BedrockPacketHandler {
        session_manager: Arc::clone(&session_manager),
    }));

    tokio::spawn(async move {
        if let Err(e) = signaling_server.run().await {
            tracing::error!("NetherNet Signaling Server failed: {}", e);
        }
    });

    // 6. Iniciar listener de RakNet en UDP 19132 (fallback para clientes sin NetherNet)
    let server_id: u64 = 0x12345678_aabbccdd;
    let raknet_port = 19132;
    let raknet_addr: SocketAddr = format!("0.0.0.0:{}", raknet_port).parse()?;
    match RakNetListener::bind(raknet_addr, server_id).await {
        Ok(listener) => {
            let listener = Arc::new(listener);
            listener.set_motd(format!(
                "MCPE;{};2225;1.26.60;0;{};{};Survival;1;{};{};",
                config.motd, config.max_players, server_id, raknet_port, raknet_port
            ));

            let raknet_runner = Arc::clone(&listener);
            let session_mgr = Arc::clone(&session_manager);
            tokio::spawn(async move {
                loop {
                    match raknet_runner.poll_event().await {
                        Ok(event) => match event {
                            RakNetEvent::Offline { packet, src, .. } => {
                                tracing::info!("RakNet Offline Handshake on :{} from {}: {:?}", raknet_port, src, packet);
                                let session_id = session_mgr.create_session(src);
                                tracing::debug!(
                                    "Registered session {} for RakNet peer {}",
                                    session_id,
                                    src
                                );
                            }
                        },
                        Err(e) => {
                            tracing::warn!("RakNet listener error on :{}: {}", raknet_port, e);
                        }
                    }
                }
            });
            info!("RakNet (UDP Fallback Transport) active on :{}", raknet_port);
        }
        Err(e) => {
            tracing::warn!("Could not bind RakNet on UDP port {}: {}", raknet_port, e);
        }
    }

    info!("============================================================");
    info!(" PocketMine-Rust READY (Axolotl / NetherNet Architecture):");
    info!(" -> TCP 19132: HTTP/HTTPS Signaling (GET / and /v1/join -> JSON)");
    info!(" -> UDP 19133: WebRTC Gameplay (ICE / DTLS / DataChannel)");
    info!(" -> UDP 19132: RakNet Fallback Transport");
    info!(" -> TCP 19133: CLOSED (WebRTC is UDP-only, matches Axolotl)");
    info!(" -> Whitelist: DISABLED");
    info!(" -> Plugins: READY ({} active)", plugin_manager.plugin_count());
    info!("============================================================");

    // Mantener corriendo hasta Ctrl+C
    tokio::select! {
        result = tokio::signal::ctrl_c() => result?,
        _ = wait_for_stop(Arc::clone(&session_manager)) => {
            info!("/stop requested, shutting down.");
        }
    }
    info!("Shutting down server gracefully.");
    let _ = plugin_manager.disable_plugins().await;
    // Run shutdown_sync in spawn_blocking so it doesn't block the Tokio runtime
    // while waiting for the PHP worker thread to exit.
    let worker = plugin_manager.php_worker().clone();
    let _ = tokio::task::spawn_blocking(move || worker.shutdown_sync()).await;
    Ok(())
}

/// Resolves once `/stop` sets the shutdown flag.
async fn wait_for_stop(manager: Arc<server_core::SessionManager>) {
    loop {
        if manager.stop_requested() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Reads `ops.txt`, one player name per line. Missing file means no operators.
fn load_ops(path: &Path) -> HashSet<String> {
    let mut ops = HashSet::new();
    let Ok(contents) = std::fs::read_to_string(path) else {
        return ops;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        ops.insert(line.to_lowercase());
    }
    ops
}
