use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

use crate::info::ServerInfo;

pub struct SignalingServer {
    addr: SocketAddr,
    info: Arc<ServerInfo>,
}

impl SignalingServer {
    pub fn new(addr: SocketAddr, info: ServerInfo) -> Self {
        Self {
            addr,
            info: Arc::new(info),
        }
    }

    pub async fn run(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        info!("NetherNet HTTP signaling listening on http://{}", self.addr);

        loop {
            let (stream, peer_addr) = listener.accept().await?;
            let info = Arc::clone(&self.info);
            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(stream, peer_addr, info).await {
                    warn!("Signaling connection error from {}: {}", peer_addr, e);
                }
            });
        }
    }

    async fn handle_client(
        mut stream: TcpStream,
        peer_addr: SocketAddr,
        info: Arc<ServerInfo>,
    ) -> std::io::Result<()> {
        let mut buffer = [0u8; 4096];
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(());
        }

        let request = String::from_utf8_lossy(&buffer[..bytes_read]);
        let mut lines = request.lines();
        let request_line = lines.next().unwrap_or("");
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("");

        info!("Incoming signaling request: {} {} from {}", method, path, peer_addr);

        if method == "GET" && (path == "/v1/join" || path == "/v1/join/") {
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

            // Placeholder SDP answer (WebRTC peer connection integration will negotiate here)
            let response = "HTTP/1.1 501 Not Implemented\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response.as_bytes()).await?;
        } else {
            let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response.as_bytes()).await?;
        }

        stream.flush().await?;
        Ok(())
    }
}
