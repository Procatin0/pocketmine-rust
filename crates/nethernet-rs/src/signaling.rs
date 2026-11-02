use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info};

use crate::identity::NetherNetIdentity;
use crate::info::ServerInfo;

pub struct SignalingServer {
    addrs: Vec<SocketAddr>,
    info: Arc<ServerInfo>,
    udp_ports: Vec<u16>,
    identity: Arc<NetherNetIdentity>,
}

impl SignalingServer {
    pub fn new(addr: SocketAddr, info: ServerInfo, udp_port: u16, identity: Arc<NetherNetIdentity>) -> Self {
        Self {
            addrs: vec![addr],
            info: Arc::new(info),
            udp_ports: vec![udp_port],
            identity,
        }
    }

    pub fn with_multiple_ports(
        addrs: Vec<SocketAddr>,
        info: ServerInfo,
        udp_ports: Vec<u16>,
        identity: Arc<NetherNetIdentity>,
    ) -> Self {
        Self {
            addrs,
            info: Arc::new(info),
            udp_ports,
            identity,
        }
    }

    pub async fn run(&self) -> std::io::Result<()> {
        for &addr in &self.addrs {
            let info = Arc::clone(&self.info);
            let udp_ports = self.udp_ports.clone();
            let identity = Arc::clone(&self.identity);
            tokio::spawn(async move {
                if let Err(e) = Self::listen_loop(addr, info, udp_ports, identity).await {
                    error!("Signaling listener failed on {}: {}", addr, e);
                }
            });
            info!("NetherNet HTTP signaling listening on http://{}", addr);
        }
        Ok(())
    }

    async fn listen_loop(
        addr: SocketAddr,
        info: Arc<ServerInfo>,
        udp_ports: Vec<u16>,
        identity: Arc<NetherNetIdentity>,
    ) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        loop {
            let (stream, peer_addr) = listener.accept().await?;
            let info = Arc::clone(&info);
            let udp_ports = udp_ports.clone();
            let signaling_port = addr.port();
            let identity = Arc::clone(&identity);
            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(stream, peer_addr, info, udp_ports, signaling_port, identity).await {
                    debug!("Signaling connection closed from {}: {}", peer_addr, e);
                }
            });
        }
    }

    async fn handle_client(
        mut stream: TcpStream,
        peer_addr: SocketAddr,
        info: Arc<ServerInfo>,
        udp_ports: Vec<u16>,
        signaling_port: u16,
        identity: Arc<NetherNetIdentity>,
    ) -> std::io::Result<()> {
        let mut buffer = [0u8; 8192];
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(());
        }

        // Sniff for TLS Handshake Record (ContentType 0x16, TLS Major Version 0x03)
        // If received on plaintext HTTP port, close cleanly without invalid HTTP 400
        if buffer[0] == 0x16 && bytes_read >= 3 && buffer[1] == 0x03 {
            debug!(
                "[:{}] TLS ClientHello probe from {}. Closing cleanly for plaintext fallback.",
                signaling_port, peer_addr
            );
            return Ok(());
        }

        let request = String::from_utf8_lossy(&buffer[..bytes_read]);
        let mut lines = request.lines();
        let request_line = lines.next().unwrap_or("");
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("");
        let raw_path = parts.next().unwrap_or("");

        // Extract host header if available
        let mut client_host = "192.168.100.51".to_string();
        for line in lines {
            let lower = line.to_lowercase();
            if lower.starts_with("host:") {
                let host_val = line[5..].trim();
                let host_no_port = host_val.split(':').next().unwrap_or(host_val).trim();
                if !host_no_port.is_empty() && host_no_port != "127.0.0.1" && host_no_port != "localhost" {
                    client_host = host_no_port.to_string();
                }
            }
        }

        info!("[:{}] Incoming signaling request: {} {} from {}", signaling_port, method, raw_path, peer_addr);

        // ANY GET request returns ServerInfo JSON (required by user and Bedrock clients)
        // This guarantees NO 404 is ever returned on /, /v1/join, /index.html, etc.
        if method == "GET" || method == "HEAD" {
            let mut probe_info = (*info).clone();
            if let Some(query) = raw_path.split_once('?') {
                for pair in query.1.split('&') {
                    if let Some((k, v)) = pair.split_once('=') {
                        if k == "version" && !v.is_empty() {
                            probe_info.version = v.to_string();
                        } else if k == "protocol" {
                            if let Ok(p) = v.parse::<i32>() {
                                probe_info.protocol = p;
                            }
                        }
                    }
                }
            }
            probe_info.whitelist = false;
            probe_info.allow_list = false;

            // Generate comprehensive JSON payload matching PocketMine ServerInfoCodec
            let json = serde_json::json!({
                "dataVersion": probe_info.data_version,
                "name": probe_info.name,
                "serverName": probe_info.name,
                "protocol": probe_info.protocol,
                "version": probe_info.version,
                "gameVersion": probe_info.version,
                "level": probe_info.level,
                "levelName": probe_info.level,
                "players": probe_info.players,
                "playerCount": probe_info.players,
                "maxPlayers": probe_info.max_players,
                "maxPlayerCount": probe_info.max_players,
                "gameType": probe_info.game_type,
                "editor": probe_info.editor,
                "hardcore": probe_info.hardcore,
                "onlineAuth": true,
                "selfSignedAuth": true,
                "whitelist": false,
                "white_list": false,
                "allowList": false,
                "allow_list": false,
                "nonce": probe_info.nonce,
                "transportLayer": probe_info.transport_layer,
                "connection": probe_info.connection
            }).to_string();

            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/json; charset=utf-8\r\n\
                 Access-Control-Allow-Origin: *\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n\
                 {}",
                json.len(),
                json
            );
            stream.write_all(response.as_bytes()).await?;
            info!("[:{}] Responded with JSON ServerInfo (v{}, p{}, onlineAuth: true) to {}", signaling_port, probe_info.version, probe_info.protocol, peer_addr);
        } else if method == "POST" && raw_path.contains("/v1/join/") {
            let network_id = raw_path.split("/v1/join/").nth(1).unwrap_or("").split('?').next().unwrap_or("");
            info!("[:{}] Received NetherNet WebRTC offer from {} for networkId {}", signaling_port, peer_addr, network_id);
            debug!("[:{}] Client SDP offer:\n{}", signaling_port, request);

            // Construct standard NetherNet SDP DataChannel Answer
            let primary_udp = udp_ports.first().copied().unwrap_or(19133);
            let mut candidates = String::new();
            let mut cand_id = 1;
            for &port in &udp_ports {
                candidates.push_str(&format!(
                    "a=candidate:{} 1 UDP 2122260{:03} {} {} typ host\r\n",
                    cand_id, 200 - cand_id, client_host, port
                ));
                cand_id += 1;
                if client_host != "192.168.100.51" {
                    candidates.push_str(&format!(
                        "a=candidate:{} 1 UDP 2122260{:03} 192.168.100.51 {} typ host\r\n",
                        cand_id, 200 - cand_id, port
                    ));
                    cand_id += 1;
                }
            }

            let fingerprint = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99";
            let identity_assertion = identity.create_assertion(fingerprint);

            let sdp_answer = format!(
                "v=0\r\n\
                 o=- 1 2 IN IP4 {host}\r\n\
                 s=-\r\n\
                 t=0 0\r\n\
                 a=group:BUNDLE 0\r\n\
                 a=identity:{identity}\r\n\
                 m=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n\
                 c=IN IP4 {host}\r\n\
                 a=ice-ufrag:rustnet\r\n\
                 a=ice-pwd:pocketminerustpwd123\r\n\
                 a=fingerprint:sha-256 {fingerprint}\r\n\
                 a=setup:active\r\n\
                 a=mid:0\r\n\
                 a=sctp-port:5000\r\n\
                 a=max-message-size:262144\r\n\
                 {candidates}",
                host = client_host,
                identity = identity_assertion,
                fingerprint = fingerprint,
                candidates = candidates
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
            info!("[:{}] Responded with WebRTC SDP Answer (with a=identity, target UDP {}:{}) to {}", signaling_port, client_host, primary_udp, peer_addr);
        } else {
            let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response.as_bytes()).await?;
        }

        stream.flush().await?;
        Ok(())
    }
}
