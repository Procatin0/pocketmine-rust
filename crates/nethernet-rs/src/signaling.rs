use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

use crate::info::ServerInfo;

pub struct SignalingServer {
    addr: SocketAddr,
    info: Arc<ServerInfo>,
    udp_port: u16,
}

impl SignalingServer {
    pub fn new(addr: SocketAddr, info: ServerInfo, udp_port: u16) -> Self {
        Self {
            addr,
            info: Arc::new(info),
            udp_port,
        }
    }

    pub async fn run(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        info!("NetherNet HTTP signaling listening on http://{}", self.addr);

        loop {
            let (stream, peer_addr) = listener.accept().await?;
            let info = Arc::clone(&self.info);
            let udp_port = self.udp_port;
            let signaling_port = self.addr.port();
            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(stream, peer_addr, info, udp_port, signaling_port).await {
                    warn!("Signaling connection error from {}: {}", peer_addr, e);
                }
            });
        }
    }

    async fn handle_client(
        mut stream: TcpStream,
        peer_addr: SocketAddr,
        info: Arc<ServerInfo>,
        udp_port: u16,
        signaling_port: u16,
    ) -> std::io::Result<()> {
        let mut buffer = [0u8; 8192];
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(());
        }

        // Sniff for TLS Handshake Record (ContentType 0x16, TLS Major Version 0x03)
        if buffer[0] == 0x16 && bytes_read >= 3 && buffer[1] == 0x03 {
            warn!(
                "TLS ClientHello received on plaintext HTTP signaling from {}. NetherNet is serving plain HTTP.",
                peer_addr
            );
            // Close connection cleanly
            return Ok(());
        }

        let request = String::from_utf8_lossy(&buffer[..bytes_read]);
        let mut lines = request.lines();
        let request_line = lines.next().unwrap_or("");
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("");

        info!("Incoming signaling request: {} {} from {}", method, path, peer_addr);

        if method == "GET" && (path == "/" || path == "/index.html") {
            let html = format!(
                r#"<!DOCTYPE html>
<html lang="es">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>PocketMine-Rust Server</title>
  <style>
    * {{ box-sizing: border-box; }}
    body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; background: #0b0f19; color: #f1f5f9; margin: 0; padding: 2rem 1rem; display: flex; justify-content: center; align-items: center; min-height: 90vh; }}
    .card {{ background: #131d31; border-radius: 16px; padding: 2rem; max-width: 520px; width: 100%; border: 1px solid #1e293b; box-shadow: 0 20px 30px -10px rgba(0,0,0,0.5); }}
    .header {{ display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem; }}
    h1 {{ color: #38bdf8; margin: 0; font-size: 1.4rem; }}
    .badge {{ background: #059669; color: #ecfdf5; font-size: 0.75rem; font-weight: 700; padding: 0.3rem 0.65rem; border-radius: 9999px; text-transform: uppercase; letter-spacing: 0.05em; }}
    .subtitle {{ color: #94a3b8; font-size: 0.9rem; margin-top: 0; margin-bottom: 1.5rem; }}
    .metric {{ display: flex; justify-content: space-between; align-items: center; padding: 0.8rem 0; border-bottom: 1px solid #1e293b; }}
    .metric:last-child {{ border-bottom: none; }}
    .metric-label {{ color: #94a3b8; font-size: 0.9rem; }}
    .metric-value {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-weight: 600; color: #f8fafc; }}
    .pill {{ background: #1e293b; padding: 0.2rem 0.5rem; border-radius: 6px; border: 1px solid #334155; }}
    a.link {{ color: #38bdf8; text-decoration: none; font-weight: 600; }}
    a.link:hover {{ text-decoration: underline; }}
  </style>
</head>
<body>
  <div class="card">
    <div class="header">
      <h1>PocketMine-Rust</h1>
      <span class="badge">Activo</span>
    </div>
    <p class="subtitle">Servidor Minecraft Bedrock de alto rendimiento en Rust</p>
    <div class="metric"><span class="metric-label">Nombre del Servidor</span><span class="metric-value">{}</span></div>
    <div class="metric"><span class="metric-label">Versión de Juego</span><span class="metric-value">{} (Protocolo {})</span></div>
    <div class="metric"><span class="metric-label">Señalización NetherNet (TCP)</span><span class="metric-value pill">:{}</span></div>
    <div class="metric"><span class="metric-label">Gameplay RakNet (UDP)</span><span class="metric-value pill">:{}</span></div>
    <div class="metric"><span class="metric-label">Probe NetherNet</span><a class="link metric-value" href="/v1/join">/v1/join</a></div>
  </div>
</body>
</html>"#,
                info.name, info.version, info.protocol, signaling_port, udp_port
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: text/html; charset=utf-8\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n\
                 {}",
                html.len(),
                html
            );
            stream.write_all(response.as_bytes()).await?;
        } else if method == "GET" && (path == "/v1/join" || path == "/v1/join/") {
            // Client probe: return ServerInfo JSON
            let json = info.to_json().map_err(|e| {
                error!("Failed to serialize ServerInfo: {}", e);
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })?;

            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/json\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n\
                 {}",
                json.len(),
                json
            );
            stream.write_all(response.as_bytes()).await?;
            info!("Responded to NetherNet probe from {}", peer_addr);
        } else if method == "POST" && path.starts_with("/v1/join/") {
            let network_id = path.trim_start_matches("/v1/join/");
            info!("Received NetherNet WebRTC offer from {} for networkId {}", peer_addr, network_id);

            // Construct standard NetherNet SDP DataChannel Answer pointing to UDP gameplay port
            let sdp_answer = format!(
                "v=0\r\n\
                 o=- 1 2 IN IP4 {server_ip}\r\n\
                 s=-\r\n\
                 t=0 0\r\n\
                 a=group:BUNDLE 0\r\n\
                 m=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n\
                 c=IN IP4 {server_ip}\r\n\
                 a=ice-ufrag:rustnet\r\n\
                 a=ice-pwd:pocketminerustpwd123\r\n\
                 a=fingerprint:sha-256 AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99\r\n\
                 a=setup:active\r\n\
                 a=mid:0\r\n\
                 a=sctp-port:5000\r\n\
                 a=max-message-size:262144\r\n\
                 a=candidate:1 1 UDP 2122260223 {server_ip} {udp_port} typ host\r\n",
                server_ip = peer_addr.ip(),
                udp_port = udp_port
            );

            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/sdp\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n\
                 {}",
                sdp_answer.len(),
                sdp_answer
            );
            stream.write_all(response.as_bytes()).await?;
            info!("Responded with WebRTC SDP Answer to {}", peer_addr);
        } else {
            let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response.as_bytes()).await?;
        }

        stream.flush().await?;
        Ok(())
    }
}
