use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tracing::info;
use nethernet_rs::{NetherNetIdentity, ServerInfo, SignalingServer};
use raklib_rs::{RakNetEvent, RakNetListener};
use server_core::{ServerConfig, SessionManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

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
    let session_manager = Arc::new(SessionManager::with_generator(world_generator));
    info!("SessionManager initialized (active sessions: {})", session_manager.session_count());

    // 2. Transporte principal de señalización: TCP 19132 EXCLUSIVAMENTE (HTTP / HTTPS demux)
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

    // 3. Inicializar motor y puente de plugins (php-plugin-bridge)
    let plugin_manager = Arc::new(php_plugin_bridge::PluginManager::new());
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

    // 4. Iniciar listener de RakNet en UDP 19132 (fallback para clientes sin NetherNet)
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
                                let _ = session_mgr.create_session(src);
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
    tokio::signal::ctrl_c().await?;
    info!("Shutting down server gracefully.");
    let _ = plugin_manager.disable_plugins().await;
    Ok(())
}
