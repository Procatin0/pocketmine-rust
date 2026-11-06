use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error, info, warn};

use rtc::ice::network_type::NetworkType;
use rtc::peer_connection::transport::{RTCDtlsRole, RTCIceCandidateInit};
use webrtc::data_channel::{DataChannel, DataChannelEvent};
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler,
    RTCConfigurationBuilder, RTCIceConnectionState, RTCIceGatheringState,
    RTCPeerConnectionState, RTCSessionDescription, SettingEngineBuilder,
};

use crate::identity::NetherNetIdentity;
use crate::info::ServerInfo;
use crate::negotiation::{AnswerBuilder, IceCandidateFormatter};

pub trait NetherNetPacketHandler: Send + Sync {
    fn handle_message(&self, peer: SocketAddr, channel: &str, data: bytes::Bytes) -> Vec<bytes::Bytes>;

    /// Called when a peer's data channel closes. The default does nothing.
    fn on_disconnect(&self, _peer: SocketAddr) {}
}

struct NetherNetWebRtcHandler {
    gather_notify: Arc<tokio::sync::Notify>,
    peer_addr: SocketAddr,
    packet_handler: Option<Arc<dyn NetherNetPacketHandler>>,
    peer_conn: Arc<Mutex<Option<Arc<dyn PeerConnection>>>>,
    reliable_dc: Arc<Mutex<Option<Arc<dyn DataChannel>>>>,
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
        if state == RTCPeerConnectionState::Disconnected || state == RTCPeerConnectionState::Failed || state == RTCPeerConnectionState::Closed {
            if let Some(pc) = self.peer_conn.lock().await.take() {
                debug!("[:WebRTC] Closing PeerConnection on disconnect/fail for {}", self.peer_addr);
                let _ = pc.close().await;
            }
        }
    }

    async fn on_data_channel(&self, dc: Arc<dyn DataChannel>) {
        let peer = self.peer_addr;
        let label = dc.label().await.unwrap_or_default();
        let id = dc.id();
        info!("[:WebRTC] DataChannel '{}' (id: {:?}) established with {}", label, id, peer);

        if label == "ReliableDataChannel" {
            *self.reliable_dc.lock().await = Some(Arc::clone(&dc));
        }

        let handler = self.packet_handler.clone();
        let peer_conn = Arc::clone(&self.peer_conn);
        let reliable_dc = Arc::clone(&self.reliable_dc);
        tokio::spawn(async move {
            loop {
                match dc.poll().await {
                    Some(DataChannelEvent::OnMessage(msg)) => {
                        if !msg.data.is_empty() {
                            let countdown = msg.data[0];
                            let payload = bytes::Bytes::copy_from_slice(&msg.data[1..]);
                            debug!(
                                "[:WebRTC] DataChannel '{}' message from {} (countdown={}, len={})",
                                label, peer, countdown, payload.len()
                            );
                            if let Some(ref ph) = handler {
                                let responses = ph.handle_message(peer, &label, payload);
                                for resp in responses {
                                    let mut framed = Vec::with_capacity(resp.len() + 1);
                                    framed.push(0x00); // 0x00 NetherNet countdown header (unfragmented)
                                    framed.extend_from_slice(&resp);
                                    let mut buf = bytes::BytesMut::with_capacity(framed.len());
                                    buf.extend_from_slice(&framed);

                                    // Chunks and all core game packets MUST be delivered over ReliableDataChannel
                                    let target_dc = if let Some(ref rdc) = *reliable_dc.lock().await {
                                        rdc.clone()
                                    } else {
                                        dc.clone()
                                    };

                                    if let Err(e) = target_dc.send(buf).await {
                                        error!("[:WebRTC] Failed to send response on '{}': {}", label, e);
                                    }
                                }
                            }
                        }
                    }
                    Some(DataChannelEvent::OnClose) | None => {
                        info!("[:WebRTC] DataChannel '{}' closed for {}", label, peer);
                        if let Some(ph) = &handler {
                            ph.on_disconnect(peer);
                        }
                        if let Some(pc) = peer_conn.lock().await.take() {
                            debug!("[:WebRTC] Closing PeerConnection on DataChannel close for {}", peer);
                            let _ = pc.close().await;
                        }
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
    active_peers: Arc<Mutex<HashMap<String, Arc<dyn PeerConnection>>>>,
    packet_handler: Option<Arc<dyn NetherNetPacketHandler>>,
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
            active_peers: Arc::new(Mutex::new(HashMap::new())),
            packet_handler: None,
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
            active_peers: Arc::new(Mutex::new(HashMap::new())),
            packet_handler: None,
        }
    }

    pub fn set_packet_handler(&mut self, handler: Arc<dyn NetherNetPacketHandler>) {
        self.packet_handler = Some(handler);
    }

    pub fn with_packet_handler(mut self, handler: Arc<dyn NetherNetPacketHandler>) -> Self {
        self.packet_handler = Some(handler);
        self
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
            let active_peers = Arc::clone(&self.active_peers);
            let packet_handler = self.packet_handler.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::listen_loop(addr, info, udp_ports, identity, tls_acceptor, active_peers, packet_handler).await {
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
        active_peers: Arc<Mutex<HashMap<String, Arc<dyn PeerConnection>>>>,
        packet_handler: Option<Arc<dyn NetherNetPacketHandler>>,
    ) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        loop {
            let (stream, peer_addr) = listener.accept().await?;
            let info = Arc::clone(&info);
            let udp_ports = udp_ports.clone();
            let signaling_port = addr.port();
            let identity = Arc::clone(&identity);
            let tls_acceptor = Arc::clone(&tls_acceptor);
            let active_peers = Arc::clone(&active_peers);
            let packet_handler = packet_handler.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(
                    stream,
                    peer_addr,
                    info,
                    udp_ports,
                    signaling_port,
                    identity,
                    tls_acceptor,
                    active_peers,
                    packet_handler,
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
        active_peers: Arc<Mutex<HashMap<String, Arc<dyn PeerConnection>>>>,
        packet_handler: Option<Arc<dyn NetherNetPacketHandler>>,
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
                    Self::handle_http(tls_stream, peer_addr, info, udp_ports, signaling_port, identity, active_peers, packet_handler).await
                }
                Err(e) => {
                    debug!("[:{}] TLS handshake failed from {}: {}", signaling_port, peer_addr, e);
                    Ok(())
                }
            }
        } else {
            // Plaintext HTTP connection
            Self::handle_http(stream, peer_addr, info, udp_ports, signaling_port, identity, active_peers, packet_handler).await
        }
    }

    async fn handle_http<S: AsyncRead + AsyncWrite + Unpin>(
        mut stream: S,
        peer_addr: SocketAddr,
        info: Arc<ServerInfo>,
        udp_ports: Vec<u16>,
        signaling_port: u16,
        identity: Arc<NetherNetIdentity>,
        active_peers: Arc<Mutex<HashMap<String, Arc<dyn PeerConnection>>>>,
        packet_handler: Option<Arc<dyn NetherNetPacketHandler>>,
    ) -> std::io::Result<()> {
        // Read HTTP request headers in a loop until \r\n\r\n or \n\n
        let mut buffer = Vec::new();
        let mut temp = [0u8; 4096];
        let mut head_end = None;

        loop {
            let n = stream.read(&mut temp).await?;
            if n == 0 {
                if buffer.is_empty() {
                    return Ok(());
                }
                break;
            }
            buffer.extend_from_slice(&temp[..n]);

            if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                head_end = Some((pos, 4));
                break;
            } else if let Some(pos) = buffer.windows(2).position(|w| w == b"\n\n") {
                head_end = Some((pos, 2));
                break;
            }

            if buffer.len() > 16384 {
                // Request header fields too large
                let response = "HTTP/1.1 431 Request Header Fields Too Large\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(response.as_bytes()).await;
                return Ok(());
            }
        }

        let (header_pos, term_len) = match head_end {
            Some(h) => h,
            None => return Ok(()),
        };

        let header_str = String::from_utf8_lossy(&buffer[..header_pos]).to_string();
        let mut body_bytes = buffer[(header_pos + term_len)..].to_vec();

        let mut lines = header_str.lines();
        let request_line = lines.next().unwrap_or("");
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("");
        let raw_path = parts.next().unwrap_or("");

        let mut content_length = 0usize;
        let mut expects_100_continue = false;
        let mut client_host = "192.168.100.51".to_string();

        for line in lines {
            let lower = line.to_lowercase();
            if lower.starts_with("content-length:") {
                content_length = line["content-length:".len()..].trim().parse().unwrap_or(0);
            } else if lower.starts_with("expect:") && lower.contains("100-continue") {
                expects_100_continue = true;
            } else if lower.starts_with("host:") {
                let host_val = line["host:".len()..].trim();
                let host_no_port = host_val.split(':').next().unwrap_or(host_val).trim();
                if !host_no_port.is_empty() && host_no_port != "127.0.0.1" && host_no_port != "localhost" {
                    client_host = host_no_port.to_string();
                }
            }
        }

        // If client asked for Expect: 100-continue, respond immediately (RFC 9112 / Axolotl HttpConnection.php)
        if expects_100_continue && body_bytes.len() < content_length {
            stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").await?;
            stream.flush().await?;
        }

        // Read remaining body up to Content-Length
        while body_bytes.len() < content_length {
            let n = stream.read(&mut temp).await?;
            if n == 0 {
                break;
            }
            body_bytes.extend_from_slice(&temp[..n]);
        }

        info!("[:{}] Incoming signaling request: {} {} from {}", signaling_port, method, raw_path, peer_addr);

        // GET or HEAD /v1/join or / -> ServerInfo JSON
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

            // EXACT PocketMine ServerInfo JSON schema (15 properties, ServerInfoCodec compatible)
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
            info!("[:{}] Responded with exact JSON ServerInfo (v{}, p{}) to {}", signaling_port, probe_info.version, probe_info.protocol, peer_addr);
        } else if method == "POST" && raw_path.contains("/v1/join/") {
            let network_id = raw_path.split("/v1/join/").nth(1).unwrap_or("").split('?').next().unwrap_or("");
            info!("[:{}] Received NetherNet WebRTC offer from {} for networkId {}", signaling_port, peer_addr, network_id);

            let offer_sdp = String::from_utf8_lossy(&body_bytes).to_string();
            if offer_sdp.trim().is_empty() {
                warn!("[:{}] Empty offer SDP received from {}", signaling_port, peer_addr);
                let err_body = "5";
                let response = format!(
                    "HTTP/1.1 500 Internal Server Error\r\n\
                     Content-Type: text/plain; charset=utf-8\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\
                     \r\n\
                     {}",
                    err_body.len(),
                    err_body
                );
                stream.write_all(response.as_bytes()).await?;
                stream.flush().await?;
                return Ok(());
            }

            let primary_udp = udp_ports.first().copied().unwrap_or(19133);

            match Self::generate_sdp_answer(
                &offer_sdp,
                network_id,
                &client_host,
                primary_udp,
                peer_addr,
                &identity,
                &active_peers,
                &packet_handler,
            ).await {
                Ok(sdp_answer) => {
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
                    info!("[:{}] Responded with WebRTC SDP Answer (srflx advertised {}:{}) to {}", signaling_port, client_host, primary_udp, peer_addr);
                }
                Err(e) => {
                    eprintln!("[:{}] WebRTC negotiation failed for {}: {:?}", signaling_port, network_id, e);
                    error!("[:{}] WebRTC negotiation failed for {}: {}", signaling_port, network_id, e);
                    // Match Axolotl JoinHandler failure response: HTTP 500 with error code string
                    let err_body = "5"; // ErrorCode::ICE
                    let response = format!(
                        "HTTP/1.1 500 Internal Server Error\r\n\
                         Content-Type: text/plain; charset=utf-8\r\n\
                         Content-Length: {}\r\n\
                         Connection: close\r\n\
                         \r\n\
                         {}",
                        err_body.len(),
                        err_body
                    );
                    stream.write_all(response.as_bytes()).await?;
                }
            }
        } else {
            let response = "HTTP/1.1 404 Not Found\r\nCache-Control: no-cache\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response.as_bytes()).await?;
        }

        stream.flush().await?;
        Ok(())
    }

    async fn generate_sdp_answer(
        offer_sdp: &str,
        network_id: &str,
        client_host: &str,
        udp_port: u16,
        peer_addr: SocketAddr,
        identity: &NetherNetIdentity,
        active_peers: &Arc<Mutex<HashMap<String, Arc<dyn PeerConnection>>>>,
        packet_handler: &Option<Arc<dyn NetherNetPacketHandler>>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if offer_sdp.trim().is_empty() {
            return Err("Offer SDP is empty".into());
        }

        // Clean client offer before creating PC
        let cleaned_offer = offer_sdp
            .lines()
            .filter(|line| !line.starts_with("a=identity:"))
            .collect::<Vec<_>>()
            .join("\r\n") + "\r\n";

        let offer = RTCSessionDescription::offer(cleaned_offer)?;

        // 1. Clean up all previous PeerConnections on this UDP port to ensure the port is cleanly freed
        {
            let mut peers = active_peers.lock().await;
            for (id, old_pc) in peers.drain() {
                debug!("[:WebRTC] Closing prior PeerConnection for {} on port {}", id, udp_port);
                let _ = old_pc.close().await;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let gather_notify = Arc::new(tokio::sync::Notify::new());
        let peer_conn_holder: Arc<Mutex<Option<Arc<dyn PeerConnection>>>> = Arc::new(Mutex::new(None));
        let reliable_dc: Arc<Mutex<Option<Arc<dyn DataChannel>>>> = Arc::new(Mutex::new(None));
        let handler: Arc<dyn PeerConnectionEventHandler> = Arc::new(NetherNetWebRtcHandler {
            gather_notify: Arc::clone(&gather_notify),
            peer_addr,
            packet_handler: packet_handler.clone(),
            peer_conn: Arc::clone(&peer_conn_holder),
            reliable_dc,
        });

        // 2. Configure WebRTC SettingEngine strictly matching Axolotl WebRtcTransportFactory.php:
        let mut pc_res = None;
        for attempt in 1..=5 {
            let setting_engine = SettingEngineBuilder::new()
                .with_answering_dtls_role(RTCDtlsRole::Server)
                .with_multicast_dns_mode(rtc::ice::mdns::MulticastDnsMode::Disabled)
                .with_network_types(vec![NetworkType::Udp4])
                .with_sctp_max_message_size(rtc::peer_connection::configuration::setting_engine::SctpMaxMessageSize::Bounded(262144))
                .with_sctp_max_receive_buffer_size(1024 * 1024 * 4)
                .build();
            let config = RTCConfigurationBuilder::new().build();

            match PeerConnectionBuilder::new()
                .with_configuration(config)
                .with_setting_engine(setting_engine)
                .with_handler(Arc::clone(&handler))
                .with_udp_addrs(vec![format!("0.0.0.0:{}", udp_port)])
                .build()
                .await
            {
                Ok(built) => {
                    pc_res = Some(built);
                    break;
                }
                Err(e) => {
                    if attempt < 5 {
                        debug!("[:WebRTC] Bind attempt {} on UDP :{} failed ({}), retrying in 60ms...", attempt, udp_port, e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(60)).await;
                    } else {
                        return Err(Box::new(e));
                    }
                }
            }
        }
        let pc: Arc<dyn PeerConnection> = Arc::new(pc_res.unwrap());
        *peer_conn_holder.lock().await = Some(Arc::clone(&pc));
        active_peers.lock().await.insert(network_id.to_string(), Arc::clone(&pc));

        let negotiation = async {
            pc.set_remote_description(offer).await?;

            // Check if client offer needs signalingPeer candidate (from Axolotl TransportNegotiator.php)
            let peer_ip = peer_addr.ip().to_string();
            if let Some(cand_str) = IceCandidateFormatter::signaling_peer(offer_sdp, &peer_ip) {
                let init = RTCIceCandidateInit {
                    candidate: cand_str,
                    sdp_mid: Some("0".to_string()),
                    sdp_mline_index: Some(0),
                    username_fragment: None,
                    url: None,
                };
                if let Err(e) = pc.add_ice_candidate(init).await {
                    debug!("[:WebRTC] signalingPeer candidate add ignored: {}", e);
                }
            }

            let answer = pc.create_answer(None).await?;
            pc.set_local_description(answer).await?;

            // Wait for ICE candidate gathering to finish (up to 1000ms)
            tokio::select! {
                _ = gather_notify.notified() => {
                    debug!("[:WebRTC] ICE gathering completed for {}", peer_addr);
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(1000)) => {
                    debug!("[:WebRTC] ICE gathering wait elapsed for {}", peer_addr);
                }
            }

            let local_desc = pc.local_description().await.ok_or_else(|| std::io::Error::other("No local description produced"))?;

            // Build the final SDP answer using AnswerBuilder (translating Axolotl AnswerBuilder.php & IceCandidateFormatter.php)
            let mut advertised_addrs = vec![client_host.to_string()];
            if client_host != "192.168.100.51" {
                advertised_addrs.push("192.168.100.51".to_string());
            }
            let builder = AnswerBuilder::new(advertised_addrs);
            let final_sdp = builder
                .build(&local_desc.sdp, identity)
                .map_err(|e| std::io::Error::other(e))?;

            Ok::<String, Box<dyn std::error::Error + Send + Sync>>(final_sdp)
        }.await;

        match negotiation {
            Ok(final_sdp) => Ok(final_sdp),
            Err(e) => {
                let _ = pc.close().await;
                active_peers.lock().await.remove(network_id);
                Err(e)
            }
        }
    }

    pub fn fallback_sdp_answer(
        client_host: &str,
        udp_port: u16,
        identity: &NetherNetIdentity,
    ) -> String {
        let fingerprint = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99";
        let identity_assertion = identity.create_assertion(fingerprint);

        format!(
            "v=0\r\n\
             o=- 1 2 IN IP4 0.0.0.0\r\n\
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
             a=candidate:80000000 1 udp 1694498815 {host} {udp_port} typ srflx raddr 0.0.0.0 rport 0\r\n",
            host = client_host,
            identity = identity_assertion,
            fingerprint = fingerprint,
            udp_port = udp_port
        )
    }
}
