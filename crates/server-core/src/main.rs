use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tracing::info;
use nethernet_rs::{ServerInfo, SignalingServer};
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
        online_auth: config.xbox_auth,
        ..Default::default()
    };

    // 1. Iniciar transporte primario: NetherNet HTTP Signaling (WebRTC)
    if config.builtin_signaling_enabled {
        let nethernet_addr: SocketAddr = format!("0.0.0.0:{}", config.signaling_port_tcp).parse()?;
        let signaling_server = SignalingServer::new(nethernet_addr, server_info, config.server_port_udp);
        tokio::spawn(async move {
            if let Err(e) = signaling_server.run().await {
                tracing::error!("NetherNet Signaling Server failed: {}", e);
            }
        });
        info!("NetherNet (Primary WebRTC Transport) active on TCP :{}", config.signaling_port_tcp);
    }

    // 2. Iniciar transporte secundario / fallback: RakNet UDP (puerto diferente al TCP)
    let raknet_addr: SocketAddr = format!("0.0.0.0:{}", config.server_port_udp).parse()?;
    let server_id: u64 = 0x12345678_aabbccdd;
    let raknet_listener = Arc::new(RakNetListener::bind(raknet_addr, server_id).await?);
    raknet_listener.set_motd(format!(
        "MCPE;{};748;1.21.20;0;{};{};Survival;1;{};{};",
        config.motd, config.max_players, server_id, config.server_port_udp, config.server_port_v6_udp
    ));

    // 3. Inicializar gestor de sesiones de jugadores
    let session_manager = Arc::new(SessionManager::new());
    info!("SessionManager initialized (active sessions: {})", session_manager.session_count());

    let raknet_runner = Arc::clone(&raknet_listener);
    let session_mgr = Arc::clone(&session_manager);
    tokio::spawn(async move {
        loop {
            match raknet_runner.poll_event().await {
                Ok(event) => match event {
                    RakNetEvent::Offline { packet, src, .. } => {
                        tracing::info!("RakNet Offline Handshake from {}: {:?}", src, packet);
                        let _ = session_mgr.create_session(src);
                    }
                },
                Err(e) => {
                    tracing::warn!("RakNet listener error: {}", e);
                }
            }
        }
    });
    info!("RakNet (UDP Transport) active on {}", raknet_listener.local_addr()?);

    info!("============================================================");
    info!(" Server READY: Connect your Bedrock client to:");
    info!(" -> UDP Gameplay (RakNet):  port {}", config.server_port_udp);
    info!(" -> TCP Signaling (NetherNet): port {}", config.signaling_port_tcp);
    info!("============================================================");

    // Mantener corriendo hasta Ctrl+C
    tokio::signal::ctrl_c().await?;
    info!("Shutting down server gracefully.");
    Ok(())
}
