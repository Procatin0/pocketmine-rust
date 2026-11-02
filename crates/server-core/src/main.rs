use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;
use nethernet_rs::{ServerInfo, SignalingServer};
use raklib_rs::RakNetListener;
use server_core::SessionManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("============================================================");
    info!(" Starting PocketMine-Rust High Performance Bedrock Server   ");
    info!("============================================================");

    let server_info = ServerInfo {
        name: "Dedicated PocketMine-Rust Server".to_string(),
        max_players: 1000,
        ..Default::default()
    };

    // 1. Iniciar transporte primario: NetherNet HTTP Signaling (WebRTC)
    let nethernet_addr: SocketAddr = "0.0.0.0:19132".parse()?;
    let signaling_server = SignalingServer::new(nethernet_addr, server_info);
    tokio::spawn(async move {
        if let Err(e) = signaling_server.run().await {
            tracing::error!("NetherNet Signaling Server failed: {}", e);
        }
    });
    info!("NetherNet (Primary WebRTC Transport) active on {}", nethernet_addr);

    // 2. Iniciar transporte secundario / fallback: RakNet UDP
    let raknet_addr: SocketAddr = "0.0.0.0:19133".parse()?;
    let server_id: u64 = 0x12345678_aabbccdd;
    let raknet_listener = RakNetListener::bind(raknet_addr, server_id).await?;
    info!("RakNet (Legacy UDP Fallback) active on {}", raknet_listener.local_addr()?);

    // 3. Inicializar gestor de sesiones de jugadores
    let session_manager = Arc::new(SessionManager::new());
    info!("SessionManager initialized (active sessions: {})", session_manager.session_count());

    info!("Server is ready for Bedrock clients (NetherNet + RakNet fallback)");

    // Mantener corriendo hasta Ctrl+C
    tokio::signal::ctrl_c().await?;
    info!("Shutting down server gracefully.");
    Ok(())
}
