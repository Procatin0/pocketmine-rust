use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error};

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
use crate::negotiation::{AnswerBuilder, IceCandidate, IceCandidateFormatter};

pub trait NetherNetPacketHandler: Send + Sync {
    fn handle_message(&self, peer: SocketAddr, channel: &str, data: bytes::Bytes) -> Vec<bytes::Bytes>;

    /// Called when a peer's data channel closes. The default does nothing.
    fn on_disconnect(&self, _peer: SocketAddr) {}
}

/// Registry of the live `ReliableDataChannel` per WebRTC peer.
///
/// `NetherNetPacketHandler::handle_message` only ever runs in reply to an
/// inbound data-channel message, so without this registry the server
/// could never push bytes to a client that had gone quiet (e.g. Bedrock
/// in "Locating/Generating the world", waiting for terrain chunks and
/// sending nothing). The tick loop uses
/// [`PeerDataChannels::send_to_peer`] to flush the envelopes the central
/// tick queued (`NetworkSession::flushGamePacketQueue()`), framed
/// exactly like the replies the data-channel loop sends: one `0x00`
/// countdown header byte in front of each encoded batch.
pub struct PeerDataChannels {
    inner: Mutex<HashMap<SocketAddr, Arc<dyn DataChannel>>>,
}

impl PeerDataChannels {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Registers (or replaces) the reliable data channel for `peer`.
    pub async fn register(&self, peer: SocketAddr, dc: Arc<dyn DataChannel>) {
        self.inner.lock().await.insert(peer, dc);
    }

    /// Removes `peer`'s entry, but only when it still points at the
    /// channel that closed (`dc`): a reconnect may already have
    /// registered a fresh channel for the same peer address, and the
    /// old channel's close must not evict it.
    pub async fn unregister_if_same(&self, peer: SocketAddr, dc: &Arc<dyn DataChannel>) {
        let mut map = self.inner.lock().await;
        if map.get(&peer).is_some_and(|stored| Arc::ptr_eq(stored, dc)) {
            map.remove(&peer);
        }
    }

    /// Whether a reliable data channel is currently registered for `peer`.
    pub async fn has_peer(&self, peer: SocketAddr) -> bool {
        self.inner.lock().await.contains_key(&peer)
    }

    /// Frames `data` with the `0x00` countdown header and sends it on
    /// `peer`'s reliable data channel. Returns `false` when no channel
    /// is registered for the peer (not yet established, or already
    /// closed) or the send fails; the caller decides what dropping the
    /// batch means.
    pub async fn send_to_peer(&self, peer: SocketAddr, data: bytes::Bytes) -> bool {
        let dc = self.inner.lock().await.get(&peer).cloned();
        let Some(dc) = dc else {
            return false;
        };
        let mut framed = Vec::with_capacity(data.len() + 1);
        framed.push(0x00); // 0x00 NetherNet countdown header (unfragmented)
        framed.extend_from_slice(&data);
        let buf = bytes::BytesMut::from(&framed[..]);
        dc.send(buf).await.is_ok()
    }
}

impl Default for PeerDataChannels {
    fn default() -> Self {
        Self::new()
    }
}

struct NetherNetWebRtcHandler {
    gather_notify: Arc<tokio::sync::Notify>,
    peer_addr: SocketAddr,
    packet_handler: Option<Arc<dyn NetherNetPacketHandler>>,
    peer_conn: Arc<Mutex<Option<Arc<dyn PeerConnection>>>>,
    reliable_dc: Arc<Mutex<Option<Arc<dyn DataChannel>>>>,
    peer_data_channels: Arc<PeerDataChannels>,
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
        debug!("[:WebRTC] ICE connection state changed to {:?} for {}", state, self.peer_addr);
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        debug!("[:WebRTC] PeerConnection state changed to {:?} for {}", state, self.peer_addr);
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
        debug!("[:WebRTC] DataChannel '{}' (id: {:?}) established with {}", label, id, peer);

        if label == "ReliableDataChannel" {
            *self.reliable_dc.lock().await = Some(Arc::clone(&dc));
            // Publish it for tick-driven pushes: `handle_message` only
            // runs in reply to an inbound message, so without this the
            // central tick could never reach a client that went quiet
            // (e.g. waiting for terrain chunks in PreSpawn).
            self.peer_data_channels
                .register(peer, Arc::clone(&dc))
                .await;
        }

        let handler = self.packet_handler.clone();
        let peer_conn = Arc::clone(&self.peer_conn);
        let reliable_dc = Arc::clone(&self.reliable_dc);
        let peer_data_channels = Arc::clone(&self.peer_data_channels);
        let is_reliable = label == "ReliableDataChannel";
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
                        debug!("[:WebRTC] DataChannel '{}' closed for {}", label, peer);
                        if is_reliable {
                            // Stop tick-driven pushes to this peer; only
                            // the reliable channel's close unregisters, so
                            // an unreliable channel closing cannot evict
                            // the reliable entry.
                            peer_data_channels.unregister_if_same(peer, &dc).await;
                        }
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
    peer_data_channels: Arc<PeerDataChannels>,
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
            peer_data_channels: Arc::new(PeerDataChannels::new()),
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
            peer_data_channels: Arc::new(PeerDataChannels::new()),
        }
    }

    pub fn set_packet_handler(&mut self, handler: Arc<dyn NetherNetPacketHandler>) {
        self.packet_handler = Some(handler);
    }

    pub fn with_packet_handler(mut self, handler: Arc<dyn NetherNetPacketHandler>) -> Self {
        self.packet_handler = Some(handler);
        self
    }

    /// The registry of live reliable data channels this server publishes
    /// to as peers connect. The host keeps the returned `Arc` (e.g. the
    /// central tick loop) to push tick-queued batches to a peer without
    /// waiting for that peer to send a packet first.
    pub fn peer_data_channels(&self) -> Arc<PeerDataChannels> {
        Arc::clone(&self.peer_data_channels)
    }

    /// Replaces the peer data-channel registry (before `run()`), e.g. to
    /// share one registry created by the host with this server.
    pub fn set_peer_data_channels(&mut self, registry: Arc<PeerDataChannels>) {
        self.peer_data_channels = registry;
    }

    /// `172.16.0.0/12`: the range Hyper-V's Default Switch and docker bridges
    /// carve for virtual adapters. Those addresses have no route to the LAN.
    fn is_virtual_subnet(ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => {
                let [a, b, ..] = v4.octets();
                a == 172 && (16..=31).contains(&b)
            }
            IpAddr::V6(_) => false,
        }
    }

    /// Link-local unicast: `169.254.0.0/16` (RFC 3927) and `fe80::/10` (RFC 4291).
    fn is_link_local(ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => v4.is_link_local(),
            IpAddr::V6(v6) => (v6.segments()[0] & 0xffc0) == 0xfe80,
        }
    }

    /// Probes the local source the stack would use to reach `dest_ip`, by
    /// `connect`ing a throwaway UDP socket (no packet is sent). Returns
    /// `None` when the probe cannot be made.
    fn probe_local_ip(dest_ip: IpAddr, udp_port: u16) -> Option<IpAddr> {
        let dest = SocketAddr::new(dest_ip, udp_port);
        let probe = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
        if probe.connect(dest).is_err() {
            return None;
        }
        probe
            .local_addr()
            .ok()
            .map(|local| local.ip())
            .filter(|ip| !ip.is_unspecified())
    }

    /// Remote IPv4 addresses named by the offer's host candidates and `c=`
    /// lines, ranked most-usable first: LAN, then virtual (`172.16/12`), then
    /// loopback. A loopback-only offer (an emulator) keeps its loopback bind;
    /// a same-host client that signals over `127.0.0.1` but advertises its
    /// LAN adapters resolves to the Wi-Fi/LAN interface instead of the
    /// unusable loopback socket.
    fn ranked_remote_targets(offer_sdp: &str) -> Vec<IpAddr> {
        let mut with_rank: Vec<(u8, IpAddr)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut push = |ip: IpAddr| {
            // The ICE agent gathers `NetworkType::Udp4` only, so an IPv6
            // target can never be reached from the IPv4 socket.
            if ip.is_ipv6() || ip.is_unspecified() || Self::is_link_local(ip) {
                return;
            }
            let rank = if ip.is_loopback() {
                2
            } else if Self::is_virtual_subnet(ip) {
                1
            } else {
                0
            };
            if seen.insert(ip) {
                with_rank.push((rank, ip));
            }
        };
        for line in offer_sdp.lines() {
            let line = line.trim();
            if let Some(cand_str) = line
                .strip_prefix("a=candidate:")
                .or_else(|| line.strip_prefix("candidate:"))
            {
                if let Some(cand) = IceCandidate::parse(cand_str) {
                    if let Ok(ip) = cand.address.parse::<IpAddr>() {
                        push(ip);
                    }
                }
            } else if let Some(conn) = line.strip_prefix("c=IN IP4 ") {
                if let Ok(ip) = conn.trim().parse::<IpAddr>() {
                    push(ip);
                }
            }
        }
        with_rank.sort_by_key(|(rank, _)| *rank);
        with_rank.into_iter().map(|(_, ip)| ip).collect()
    }

    /// Picks the single local IP the ICE agent should bind for this peer.
    ///
    /// Binding the wildcard `0.0.0.0` fans out (webrtc docs) to one socket per
    /// local interface, including virtual adapters such as the Hyper-V
    /// Default Switch (`172.19.16.1`) which has no route to the LAN.
    /// A concrete address binds exactly one interface, so those pairs are
    /// never formed. The railgun detail: the *signaling* TCP peer is not
    /// evidence about the *data* plane. A Bedrock client on this host signals
    /// over `127.0.0.1:19132` (loopback), so probing the signaling source
    /// used to bind `127.0.0.1:19133` — a socket that cannot write to the
    /// client's LAN candidates (`10051`) and leaves ICE with no working pair.
    /// The source is therefore probed towards the offer's own remote
    /// candidates (LAN first), then the Host header, then the signaling peer,
    /// and never degrades to loopback for a non-loopback destination.
    fn choose_bind_addr(peer_addr: SocketAddr, client_host: &str, offer_sdp: &str, udp_port: u16) -> String {
        let acceptable = |local: IpAddr, target: IpAddr| -> bool {
            // Never send LAN checks from loopback (or via a virtual-only
            // route) when the destination sits on another interface.
            if local.is_loopback() && !target.is_loopback() {
                return false;
            }
            if Self::is_virtual_subnet(local)
                && !Self::is_virtual_subnet(target)
                && !target.is_loopback()
            {
                return false;
            }
            true
        };
        let probe_toward = |target: IpAddr| -> Option<String> {
            Self::probe_local_ip(target, udp_port)
                .filter(|local| acceptable(*local, target))
                .map(|local| format!("{}:{}", local, udp_port))
        };

        for target in Self::ranked_remote_targets(offer_sdp) {
            if let Some(bind) = probe_toward(target) {
                return bind;
            }
        }
        if let Ok(host_ip) = client_host.trim().parse::<IpAddr>() {
            if let Some(bind) = probe_toward(host_ip) {
                return bind;
            }
        }
        if let Some(bind) = probe_toward(peer_addr.ip()) {
            return bind;
        }
        format!("0.0.0.0:{}", udp_port)
    }

    /// Strips `a=identity:` lines and, when the ICE bind is a LAN address
    /// while the offer advertises a non-loopback (LAN/virtual) address,
    /// loopback remote candidates.
    ///
    /// A LAN socket (`192.168.100.51:19133`) writing to a loopback remote
    /// (`127.0.0.1:65335`) is an impossible pair on Windows (`10049`) and
    /// noise once the LAN->LAN pair already connects. Emulator peers that
    /// only offer loopback keep their candidates because the bind stays
    /// loopback in that case.
    fn strip_impossible_candidates(offer_sdp: &str, bind_ip: &str) -> String {
        let bind_is_loopback = bind_ip == "127.0.0.1" || bind_ip == "::1";
        let has_non_loopback = Self::ranked_remote_targets(offer_sdp)
            .iter()
            .any(|ip| !ip.is_loopback());
        let strip_loopback = !bind_is_loopback && has_non_loopback;
        offer_sdp
            .lines()
            .filter(|line| {
                let line = line.trim();
                if line.starts_with("a=identity:") {
                    return false;
                }
                if strip_loopback {
                    if let Some(cand_str) = line
                        .strip_prefix("a=candidate:")
                        .or_else(|| line.strip_prefix("candidate:"))
                    {
                        if let Some(cand) = IceCandidate::parse(cand_str) {
                            if let Ok(ip) = cand.address.parse::<IpAddr>() {
                                if ip.is_loopback() {
                                    return false;
                                }
                            }
                        }
                    }
                }
                true
            })
            .collect::<Vec<_>>()
            .join("\r\n")
            + "\r\n"
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
        // `NetherNetInterface::start()`: debug waiting/booted pair.
        debug!("Waiting for NetherNet to start...");
        for &addr in &self.addrs {
            let info = Arc::clone(&self.info);
            let udp_ports = self.udp_ports.clone();
            let identity = Arc::clone(&self.identity);
            let tls_acceptor = Arc::clone(&self.tls_acceptor);
            let active_peers = Arc::clone(&self.active_peers);
            let packet_handler = self.packet_handler.clone();
            let peer_data_channels = Arc::clone(&self.peer_data_channels);
            tokio::spawn(async move {
                if let Err(e) = Self::listen_loop(addr, info, udp_ports, identity, tls_acceptor, active_peers, packet_handler, peer_data_channels).await {
                    error!("Signaling listener failed on {}: {}", addr, e);
                }
            });
            debug!("NetherNet HTTP/HTTPS signaling listening on {}", addr);
        }
        debug!("NetherNet booted successfully");
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
        peer_data_channels: Arc<PeerDataChannels>,
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
            let peer_data_channels = Arc::clone(&peer_data_channels);
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
                    peer_data_channels,
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
        peer_data_channels: Arc<PeerDataChannels>,
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
                    Self::handle_http(tls_stream, peer_addr, info, udp_ports, signaling_port, identity, active_peers, packet_handler, peer_data_channels).await
                }
                Err(e) => {
                    debug!("[:{}] TLS handshake failed from {}: {}", signaling_port, peer_addr, e);
                    Ok(())
                }
            }
        } else {
            // Plaintext HTTP connection
            Self::handle_http(stream, peer_addr, info, udp_ports, signaling_port, identity, active_peers, packet_handler, peer_data_channels).await
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
        peer_data_channels: Arc<PeerDataChannels>,
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

        debug!("[:{}] Incoming signaling request: {} {} from {}", signaling_port, method, raw_path, peer_addr);

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
            debug!("[:{}] Responded with exact JSON ServerInfo (v{}, p{}) to {}", signaling_port, probe_info.version, probe_info.protocol, peer_addr);
        } else if method == "POST" && raw_path.contains("/v1/join/") {
            let network_id = raw_path.split("/v1/join/").nth(1).unwrap_or("").split('?').next().unwrap_or("");
            debug!("[:{}] Received NetherNet WebRTC offer from {} for networkId {}", signaling_port, peer_addr, network_id);

            let offer_sdp = String::from_utf8_lossy(&body_bytes).to_string();
            if offer_sdp.trim().is_empty() {
                debug!("[:{}] Empty offer SDP received from {}", signaling_port, peer_addr);
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
                &peer_data_channels,
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
                    debug!("[:{}] Responded with WebRTC SDP Answer (srflx advertised {}:{}) to {}", signaling_port, client_host, primary_udp, peer_addr);
                }
                Err(e) => {
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
        peer_data_channels: &Arc<PeerDataChannels>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if offer_sdp.trim().is_empty() {
            return Err("Offer SDP is empty".into());
        }

        // Concrete single-interface bind (see choose_bind_addr), not the
        // wildcard, so ICE never pairs virtual adapters.
        let bind_addr = Self::choose_bind_addr(peer_addr, client_host, offer_sdp, udp_port);
        let bind_ip_for_filter = bind_addr.split(':').next().unwrap_or("").to_string();
        let has_non_loopback_remote = Self::ranked_remote_targets(offer_sdp)
            .iter()
            .any(|ip| !ip.is_loopback());

        // Clean client offer before creating PC: drop identity lines and,
        // for a LAN bind with LAN remote candidates, loopback remote
        // candidates so a LAN socket never writes to `127.0.0.1` (10049).
        let cleaned_offer = Self::strip_impossible_candidates(offer_sdp, &bind_ip_for_filter);
        let offer = RTCSessionDescription::offer(cleaned_offer)?;

        // 1. Only retire the previous PeerConnection for this network id.
        // Draining every entry (previous behaviour) closed *other* players'
        // connections whenever a new join negotiated on the same port, so
        // those players never got to spawn.
        let had_prior = {
            let mut peers = active_peers.lock().await;
            match peers.remove(network_id) {
                Some(old_pc) => {
                    debug!(
                        "[:WebRTC] Closing prior PeerConnection for {} on port {}",
                        network_id, udp_port
                    );
                    let _ = old_pc.close().await;
                    true
                }
                None => false,
            }
        };
        if had_prior {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        let gather_notify = Arc::new(tokio::sync::Notify::new());
        let peer_conn_holder: Arc<Mutex<Option<Arc<dyn PeerConnection>>>> = Arc::new(Mutex::new(None));
        let reliable_dc: Arc<Mutex<Option<Arc<dyn DataChannel>>>> = Arc::new(Mutex::new(None));
        let handler: Arc<dyn PeerConnectionEventHandler> = Arc::new(NetherNetWebRtcHandler {
            gather_notify: Arc::clone(&gather_notify),
            peer_addr,
            packet_handler: packet_handler.clone(),
            peer_conn: Arc::clone(&peer_conn_holder),
            reliable_dc,
            peer_data_channels: Arc::clone(peer_data_channels),
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
                .with_udp_addrs(vec![bind_addr.clone()])
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

            // Check if client offer needs signalingPeer candidate (from Axolotl TransportNegotiator.php).
            // A same-host client signals over loopback (`127.0.0.1`) while its
            // data plane is LAN: re-adding loopback as a remote candidate
            // creates the impossible LAN->loopback pair (10049). Skip it in
            // that case; the LAN candidate already covers the client.
            let peer_ip = peer_addr.ip().to_string();
            let bind_ip = bind_addr.split(':').next().unwrap_or("").to_string();
            let bind_is_loopback = bind_ip == "127.0.0.1" || bind_ip == "::1";
            let skip_signaling_peer =
                peer_addr.ip().is_loopback() && !bind_is_loopback && has_non_loopback_remote;
            let signaling_cand = if skip_signaling_peer {
                None
            } else {
                IceCandidateFormatter::signaling_peer(offer_sdp, &peer_ip)
            };
            if let Some(cand_str) = signaling_cand {
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
            // Advertise the address the client actually connected to (Host
            // header), plus this peer's concrete ICE bind address when it
            // differs, so the client never pairs an impossible server IP.
            // The hardcoded LAN address is only a fallback for non-loopback
            // binds; loopback (emulator) peers must not be offered the LAN IP.
            let mut advertised_addrs = vec![client_host.to_string()];
            let bind_ip = bind_addr.split(':').next().unwrap_or("").to_string();
            let bind_is_loopback = bind_ip == "127.0.0.1" || bind_ip == "::1";
            if !bind_ip.is_empty() && bind_ip != "0.0.0.0" && bind_ip != *client_host {
                advertised_addrs.push(bind_ip.clone());
            }
            if !bind_is_loopback && !advertised_addrs.iter().any(|a| a == "192.168.100.51") {
                advertised_addrs.push("192.168.100.51".to_string());
            }
            // A LAN bind cannot be reached at a loopback or virtual
            // (`172.19.16.1`) address: never offer the client an address its
            // checks cannot complete on. Loopback stays advertised only when
            // it is all there is (loopback-only emulator peers).
            let has_lan = advertised_addrs.iter().any(|a| {
                a.parse::<IpAddr>()
                    .map(|ip| !ip.is_loopback() && !Self::is_virtual_subnet(ip) && !Self::is_link_local(ip))
                    .unwrap_or(false)
            });
            if has_lan {
                advertised_addrs.retain(|a| match a.parse::<IpAddr>() {
                    Ok(ip) => !ip.is_loopback() && !Self::is_virtual_subnet(ip) && !Self::is_link_local(ip),
                    Err(_) => true,
                });
            }
            {
                let mut seen = std::collections::HashSet::new();
                advertised_addrs.retain(|a| seen.insert(a.clone()));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn offer_with_candidates(candidates: &str) -> String {
        format!(
            "v=0\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n{candidates}"
        )
    }

    #[test]
    fn lan_offer_signaled_over_loopback_binds_lan_not_loopback() {
        // Bedrock on this host signals over 127.0.0.1 but advertises its LAN
        // adapters in the offer. The bind must follow the data plane.
        let offer = offer_with_candidates(
            "a=candidate:1 1 udp 2122260199 172.19.16.1 62046 typ host\r\n\
             a=candidate:2 1 udp 2122260199 192.168.100.51 62047 typ host\r\n",
        );
        let peer: SocketAddr = "127.0.0.1:54321".parse().unwrap();
        let bind = SignalingServer::choose_bind_addr(peer, "192.168.100.51", &offer, 19133);
        assert!(
            !bind.starts_with("127.0.0.1:") && !bind.starts_with("0.0.0.0:") && !bind.starts_with("172.19.16.1:"),
            "expected a LAN bind, got {bind}"
        );
    }

    #[test]
    fn loopback_only_offer_keeps_loopback_bind() {
        // Emulator parity: a loopback-only offer must still bind loopback.
        let offer = offer_with_candidates("a=candidate:1 1 udp 2122260199 127.0.0.1 39240 typ host\r\n");
        let peer: SocketAddr = "127.0.0.1:54321".parse().unwrap();
        let bind = SignalingServer::choose_bind_addr(peer, "127.0.0.1", &offer, 19133);
        assert_eq!(bind, "127.0.0.1:19133");
    }

    #[test]
    fn lan_bind_strips_loopback_remote_candidates() {
        // Regression for `10049` (`192.168.100.51 -> 127.0.0.1`): with a LAN
        // bind and a mixed LAN+loopback offer, loopback remote must be
        // stripped so ICE never attempts the impossible pair.
        let offer = offer_with_candidates(
            "a=candidate:1 1 udp 2122260199 192.168.100.51 65334 typ host\r\n\
             a=candidate:2 1 udp 2122260199 127.0.0.1 65335 typ host\r\n",
        );
        let filtered = SignalingServer::strip_impossible_candidates(&offer, "192.168.100.51");
        assert!(
            filtered.contains("192.168.100.51 65334"),
            "LAN candidate must survive: {filtered}"
        );
        assert!(
            !filtered.contains("127.0.0.1 65335"),
            "loopback candidate must be stripped for LAN bind: {filtered}"
        );
    }

    #[test]
    fn loopback_bind_keeps_loopback_remote_candidates() {
        // Emulator parity: loopback-only offers keep their candidates.
        let offer = offer_with_candidates("a=candidate:1 1 udp 2122260199 127.0.0.1 39240 typ host\r\n");
        let filtered = SignalingServer::strip_impossible_candidates(&offer, "127.0.0.1");
        assert!(
            filtered.contains("127.0.0.1 39240"),
            "loopback candidate must survive for loopback bind: {filtered}"
        );
    }
}
