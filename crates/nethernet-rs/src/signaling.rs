use std::net::SocketAddr;
use std::sync::Arc;
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error, info};

use rtc::peer_connection::transport::RTCDtlsRole;
use webrtc::data_channel::{DataChannel, DataChannelEvent};
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler,
    RTCConfigurationBuilder, RTCIceConnectionState, RTCIceGatheringState,
    RTCPeerConnectionState, RTCSessionDescription, SettingEngineBuilder,
};

use crate::identity::NetherNetIdentity;
use crate::info::ServerInfo;

struct NetherNetWebRtcHandler {
    gather_notify: Arc<tokio::sync::Notify>,
    peer_addr: SocketAddr,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for NetherNetWebRtcHandler {
    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        debug!("[:WebRTC] ICE gathering state changed to {:?} for {}", state, self.peer_addr);
        if state == RTCIceGatheringState::Complete {
            self.gather_notify.notify_waiters();
        }
    }

    async fn on_ice_connection_state_change(&self, state: RTCIceConnectionState) {
        info!("[:WebRTC] ICE connection state changed to {:?} for {}", state, self.peer_addr);
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        info!("[:WebRTC] PeerConnection state changed to {:?} for {}", state, self.peer_addr);
    }

    async fn on_data_channel(&self, dc: Arc<dyn DataChannel>) {
        let peer = self.peer_addr;
        let label = dc.label().await.unwrap_or_default();
        let id = dc.id();
        info!("[:WebRTC] DataChannel '{}' (id: {:?}) established with {}", label, id, peer);

        tokio::spawn(async move {
            loop {
                match dc.poll().await {
                    Some(DataChannelEvent::OnMessage(msg)) => {
                        if !msg.data.is_empty() {
                            let countdown = msg.data[0];
                            let payload = &msg.data[1..];
                            debug!(
                                "[:WebRTC] DataChannel '{}' message from {} (countdown={}, len={})",
                                label, peer, countdown, payload.len()
                            );
                        }
                    }
                    Some(DataChannelEvent::OnClose) | None => {
                        info!("[:WebRTC] DataChannel '{}' closed for {}", label, peer);
                        break;
                    }
                    _ => {}
                }
            }
        });
    }
}

pub struct SignalingServer {
    addrs: Vec<SocketAddr>,
    info: Arc<ServerInfo>,
    udp_ports: Vec<u16>,
    identity: Arc<NetherNetIdentity>,
    tls_acceptor: Arc<TlsAcceptor>,
}

impl SignalingServer {
    pub fn new(
        addr: SocketAddr,
        info: ServerInfo,
        udp_port: u16,
        identity: Arc<NetherNetIdentity>,
    ) -> Self {
        let tls_acceptor = Arc::new(Self::create_self_signed_tls_acceptor());
        Self {
            addrs: vec![addr],
            info: Arc::new(info),
            udp_ports: vec![udp_port],
            identity,
            tls_acceptor,
        }
    }

    pub fn with_multiple_ports(
        addrs: Vec<SocketAddr>,
        info: ServerInfo,
        udp_ports: Vec<u16>,
        identity: Arc<NetherNetIdentity>,
    ) -> Self {
        let tls_acceptor = Arc::new(Self::create_self_signed_tls_acceptor());
        Self {
            addrs,
            info: Arc::new(info),
            udp_ports,
            identity,
            tls_acceptor,
        }
    }

    fn create_self_signed_tls_acceptor() -> TlsAcceptor {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let subject_alt_names = vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "192.168.100.51".to_string(),
        ];
        let certified_key = rcgen::generate_simple_self_signed(subject_alt_names)
            .expect("Failed to generate self-signed TLS cert");
        let cert_der = certified_key.cert.der().to_vec();
        let certs = vec![CertificateDer::from(cert_der)];
        let key: PrivateKeyDer = certified_key.signing_key.into();

        let server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .expect("Valid TLS server config");

        TlsAcceptor::from(Arc::new(server_config))
    }

    pub async fn run(&self) -> std::io::Result<()> {
        for &addr in &self.addrs {
            let info = Arc::clone(&self.info);
            let udp_ports = self.udp_ports.clone();
            let identity = Arc::clone(&self.identity);
            let tls_acceptor = Arc::clone(&self.tls_acceptor);
            tokio::spawn(async move {
                if let Err(e) = Self::listen_loop(addr, info, udp_ports, identity, tls_acceptor).await {
                    error!("Signaling listener failed on {}: {}", addr, e);
                }
            });
            info!("NetherNet HTTP/HTTPS signaling listening on {}", addr);
        }
        Ok(())
    }

    async fn listen_loop(
        addr: SocketAddr,
        info: Arc<ServerInfo>,
        udp_ports: Vec<u16>,
        identity: Arc<NetherNetIdentity>,
        tls_acceptor: Arc<TlsAcceptor>,
    ) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        loop {
            let (stream, peer_addr) = listener.accept().await?;
            let info = Arc::clone(&info);
            let udp_ports = udp_ports.clone();
            let signaling_port = addr.port();
            let identity = Arc::clone(&identity);
            let tls_acceptor = Arc::clone(&tls_acceptor);
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(
                    stream,
                    peer_addr,
                    info,
                    udp_ports,
                    signaling_port,
                    identity,
                    tls_acceptor,
                )
                .await
                {
                    debug!("Signaling connection closed from {}: {}", peer_addr, e);
                }
            });
        }
    }

    async fn handle_connection(
        stream: TcpStream,
        peer_addr: SocketAddr,
        info: Arc<ServerInfo>,
        udp_ports: Vec<u16>,
        signaling_port: u16,
        identity: Arc<NetherNetIdentity>,
        tls_acceptor: Arc<TlsAcceptor>,
    ) -> std::io::Result<()> {
        let mut peek_buf = [0u8; 1];
        let n = stream.peek(&mut peek_buf).await?;
        if n == 0 {
            return Ok(());
        }

        // Check if incoming connection is TLS (0x16 = ContentType Handshake)
        if peek_buf[0] == 0x16 {
            debug!("[:{}] TLS Handshake (HTTPS) initiated by {}", signaling_port, peer_addr);
            match tls_acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    Self::handle_http(tls_stream, peer_addr, info, udp_ports, signaling_port, identity).await
                }
                Err(e) => {
                    debug!("[:{}] TLS handshake failed from {}: {}", signaling_port, peer_addr, e);
                    Ok(())
                }
            }
        } else {
            // Plaintext HTTP connection
            Self::handle_http(stream, peer_addr, info, udp_ports, signaling_port, identity).await
        }
    }

    async fn handle_http<S: AsyncRead + AsyncWrite + Unpin>(
        mut stream: S,
        peer_addr: SocketAddr,
        info: Arc<ServerInfo>,
        udp_ports: Vec<u16>,
        signaling_port: u16,
        identity: Arc<NetherNetIdentity>,
    ) -> std::io::Result<()> {
        let mut buffer = [0u8; 16384];
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
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

        // ANY GET request returns ServerInfo JSON (strictly matching PocketMine ServerInfoCodec schema)
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

            // EXACT PocketMine ServerInfo JSON schema (15 properties, no extra fields)
            let json = serde_json::json!({
                "dataVersion": 7,
                "name": probe_info.name,
                "protocol": probe_info.protocol,
                "version": probe_info.version,
                "level": probe_info.level,
                "players": probe_info.players,
                "maxPlayers": probe_info.max_players,
                "gameType": probe_info.game_type,
                "editor": false,
                "hardcore": false,
                "onlineAuth": true,
                "selfSignedAuth": true,
                "nonce": "",
                "transportLayer": 2,
                "connection": 4
            }).to_string();

            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/json; charset=utf-8\r\n\
                 Cache-Control: no-cache, no-store, must-revalidate\r\n\
                 Pragma: no-cache\r\n\
                 Expires: 0\r\n\
                 Access-Control-Allow-Origin: *\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n\
                 {}",
                json.len(),
                json
            );
            stream.write_all(response.as_bytes()).await?;
            info!("[:{}] Responded with exact JSON ServerInfo (v{}, p{}, onlineAuth: true) to {}", signaling_port, probe_info.version, probe_info.protocol, peer_addr);
        } else if method == "POST" && raw_path.contains("/v1/join/") {
            let network_id = raw_path.split("/v1/join/").nth(1).unwrap_or("").split('?').next().unwrap_or("");
            info!("[:{}] Received NetherNet WebRTC offer from {} for networkId {}", signaling_port, peer_addr, network_id);

            // Extract SDP body from HTTP request
            let sdp_body = if let Some((_, body)) = request.split_once("\r\n\r\n") {
                body.trim()
            } else if let Some((_, body)) = request.split_once("\n\n") {
                body.trim()
            } else {
                ""
            };

            let primary_udp = udp_ports.first().copied().unwrap_or(19133);
            let sdp_answer = match Self::generate_sdp_answer(sdp_body, &client_host, primary_udp, peer_addr, &identity).await {
                Ok(answer) => answer,
                Err(e) => {
                    error!("[:{}] WebRTC negotiation fallback due to: {}", signaling_port, e);
                    Self::fallback_sdp_answer(&client_host, primary_udp, &identity)
                }
            };

            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/sdp\r\n\
                 Cache-Control: no-cache, no-store, must-revalidate\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n\
                 {}",
                sdp_answer.len(),
                sdp_answer
            );
            stream.write_all(response.as_bytes()).await?;
            info!("[:{}] Responded with WebRTC SDP Answer (setup:passive, a=identity, target UDP {}:{}) to {}", signaling_port, client_host, primary_udp, peer_addr);
        } else {
            let response = "HTTP/1.1 404 Not Found\r\nCache-Control: no-cache\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response.as_bytes()).await?;
        }

        stream.flush().await?;
        Ok(())
    }

    async fn generate_sdp_answer(
        offer_sdp: &str,
        client_host: &str,
        udp_port: u16,
        peer_addr: SocketAddr,
        identity: &NetherNetIdentity,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let gather_notify = Arc::new(tokio::sync::Notify::new());
        let handler = Arc::new(NetherNetWebRtcHandler {
            gather_notify: Arc::clone(&gather_notify),
            peer_addr,
        });

        let setting_engine = SettingEngineBuilder::new()
            .with_answering_dtls_role(RTCDtlsRole::Server)
            .build();

        let config = RTCConfigurationBuilder::new().build();
        let pc = Arc::new(
            PeerConnectionBuilder::new()
                .with_configuration(config)
                .with_setting_engine(setting_engine)
                .with_handler(handler)
                .with_udp_addrs(vec![format!("0.0.0.0:{}", udp_port)])
                .build()
                .await?,
        );

        // Clean client offer: strip out any a=identity line before parsing
        let cleaned_offer = offer_sdp
            .lines()
            .filter(|line| !line.starts_with("a=identity:"))
            .collect::<Vec<_>>()
            .join("\r\n") + "\r\n";

        let offer = RTCSessionDescription::offer(cleaned_offer)?;
        pc.set_remote_description(offer).await?;

        let answer = pc.create_answer(None).await?;
        pc.set_local_description(answer).await?;

        // Wait for ICE gathering to complete (non-trickle ICE exchange)
        tokio::select! {
            _ = gather_notify.notified() => {
                debug!("[:WebRTC] ICE gathering completed for {}", peer_addr);
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                debug!("[:WebRTC] ICE gathering timed out after 500ms for {}", peer_addr);
            }
        }

        let local_desc = pc.local_description().await.ok_or("No local description")?;

        // Extract fingerprint from local description
        let mut fingerprint = String::new();
        for line in local_desc.sdp.lines() {
            if line.starts_with("a=fingerprint:sha-256 ") {
                fingerprint = line["a=fingerprint:sha-256 ".len()..].trim().to_string();
                break;
            }
        }
        if fingerprint.is_empty() {
            fingerprint = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99".to_string();
        }

        let identity_assertion = identity.create_assertion(&fingerprint);

        // Format final SDP answer: inject a=identity before m= line, ensure candidate points to reachable IP
        let mut final_sdp = String::new();
        let mut identity_inserted = false;
        let mut candidate_seen = false;

        for line in local_desc.sdp.lines() {
            if line.starts_with("m=") && !identity_inserted {
                final_sdp.push_str(&format!("a=identity:{}\r\n", identity_assertion));
                identity_inserted = true;
            }
            if line.starts_with("a=candidate:") {
                candidate_seen = true;
                // Add the candidate rewritten with the reachable client_host IP
                let rewritten = line
                    .replace("172.17.0.3", client_host)
                    .replace("0.0.0.0", client_host)
                    .replace("127.0.0.1", client_host);
                final_sdp.push_str(&rewritten);
                final_sdp.push_str("\r\n");

                if client_host != "192.168.100.51" {
                    let lan = line
                        .replace("172.17.0.3", "192.168.100.51")
                        .replace("0.0.0.0", "192.168.100.51");
                    final_sdp.push_str(&lan);
                    final_sdp.push_str("\r\n");
                }

                // Also retain the original candidate line
                final_sdp.push_str(line);
                final_sdp.push_str("\r\n");
                continue;
            }
            final_sdp.push_str(line);
            final_sdp.push_str("\r\n");
        }
        if !identity_inserted {
            final_sdp.push_str(&format!("a=identity:{}\r\n", identity_assertion));
        }

        if !candidate_seen {
            let mut cand_str = format!(
                "a=candidate:1 1 udp 2122260199 {} {} typ host generation 0 network-id 1 network-cost 10\r\n",
                client_host, udp_port
            );
            if client_host != "192.168.100.51" {
                cand_str.push_str(&format!(
                    "a=candidate:2 1 udp 2122260198 192.168.100.51 {} typ host generation 0 network-id 2 network-cost 10\r\n",
                    udp_port
                ));
            }
            final_sdp.push_str(&cand_str);
        }

        // Keep connection active for the game session
        let pc_keeper = Arc::clone(&pc);
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            let _ = pc_keeper.close().await;
        });

        Ok(final_sdp)
    }

    fn fallback_sdp_answer(
        client_host: &str,
        udp_port: u16,
        identity: &NetherNetIdentity,
    ) -> String {
        let fingerprint = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99";
        let identity_assertion = identity.create_assertion(fingerprint);

        format!(
            "v=0\r\n\
             o=- 1 2 IN IP4 {host}\r\n\
             s=-\r\n\
             t=0 0\r\n\
             a=group:BUNDLE 0\r\n\
             a=identity:{identity}\r\n\
             m=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n\
             c=IN IP4 0.0.0.0\r\n\
             a=ice-ufrag:rustnetufrag123\r\n\
             a=ice-pwd:pocketminerustpwd1234567890\r\n\
             a=fingerprint:sha-256 {fingerprint}\r\n\
             a=setup:passive\r\n\
             a=mid:0\r\n\
             a=sctp-port:5000\r\n\
             a=max-message-size:262144\r\n\
             a=candidate:1 1 udp 2122260199 {host} {udp_port} typ host generation 0 network-id 1 network-cost 10\r\n\
             a=candidate:2 1 udp 2122260198 192.168.100.51 {udp_port} typ host generation 0 network-id 2 network-cost 10\r\n",
            host = client_host,
            identity = identity_assertion,
            fingerprint = fingerprint,
            udp_port = udp_port
        )
    }
}
