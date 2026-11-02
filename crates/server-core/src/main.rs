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
        online_auth: false,
        self_signed_auth: true,
        whitelist: false,
        allow_list: false,
        ..Default::default()
    };

    // 1. Iniciar transporte primario: NetherNet HTTP/HTTPS Signaling en el puerto principal 19132
    // y fallback en 19133
    let signaling_addrs: Vec<SocketAddr> = vec![
        "0.0.0.0:19132".parse()?,
        "0.0.0.0:19133".parse()?,
    ];
    let udp_ports = vec![19132, 19133];

    let identity = Arc::new(NetherNetIdentity::new());
    let signaling_server = SignalingServer::with_multiple_ports(
        signaling_addrs,
        server_info,
        udp_ports.clone(),
        identity,
    );
    tokio::spawn(async move {
        if let Err(e) = signaling_server.run().await {
            tracing::error!("NetherNet Signaling Server failed: {}", e);
        }
    });

    // 2. Inicializar gestor de sesiones de jugadores
    let session_manager = Arc::new(SessionManager::new());
    info!("SessionManager initialized (active sessions: {})", session_manager.session_count());

    // 3. Iniciar listeners de UDP gameplay (RakNet / WebRTC) en ambos puertos
    let server_id: u64 = 0x12345678_aabbccdd;
    for &port in &udp_ports {
        let raknet_addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        let listener = match RakNetListener::bind(raknet_addr, server_id).await {
            Ok(l) => Arc::new(l),
            Err(e) => {
                tracing::warn!("Could not bind RakNet on UDP port {}: {}", port, e);
                continue;
            }
        };

        listener.set_motd(format!(
            "MCPE;{};2225;1.26.60;0;{};{};Survival;1;{};{};",
            config.motd, config.max_players, server_id, port, port
        ));

        let raknet_runner = Arc::clone(&listener);
        let session_mgr = Arc::clone(&session_manager);
        tokio::spawn(async move {
            loop {
                match raknet_runner.poll_event().await {
                    Ok(event) => match event {
                        RakNetEvent::Offline { packet, src, .. } => {
                            tracing::info!("RakNet Offline Handshake on :{} from {}: {:?}", port, src, packet);
                            let _ = session_mgr.create_session(src);
                        }
                    },
                    Err(e) => {
                        tracing::warn!("RakNet listener error on : {}: {}", port, e);
                    }
                }
            }
        });
        info!("RakNet (UDP Transport) active on :{}", port);
    }

    info!("============================================================");
    info!(" Server READY on ALL Bedrock ports:");
    info!(" -> TCP Signaling: listening on 19132 & 19133 (returns JSON)");
    info!(" -> UDP Gameplay:  listening on 19132 & 19133 (WebRTC/RakNet)");
    info!(" -> Whitelist:     DISABLED (OFF)");
    info!("============================================================");

    // Mantener corriendo hasta Ctrl+C
    tokio::signal::ctrl_c().await?;
    info!("Shutting down server gracefully.");
    Ok(())
}
