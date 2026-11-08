//! GS4 Query listener (UT3 Query Protocol, `0xFE 0xFD`).
//!
//! Port of `pocketmine\network\query\QueryHandler` + `QueryInfo` +
//! `DedicatedQueryNetworkInterface`:
//! - `HANDSHAKE (9)`: returns a server-bound challenge token.
//! - `STATISTICS (0)`: returns the short or long server info string.
//!
//! The token is `BE32(substr(SHA512(salt . ":" . token), 7, 4))` where
//! `salt` is the requester's IP string, exactly like `QueryHandler::getTokenString`.
//!
//! This module is transport-agnostic: `handle_packet` is pure and testable,
//! `QueryListener` owns a UDP socket like `DedicatedQueryNetworkInterface`
//! (non-blocking, same-port as RakNet or a dedicated port).
//!
//! Wiring (`main.rs` `start_network_interfaces`): when RakLib is listening
//! on the gameplay port, the handler rides on the RakNet socket via
//! `RakNetListener::set_raw_handler` (PM's `Network::registerRawPacketHandler`);
//! otherwise a dedicated `QueryListener` binds its own UDP socket
//! (`DedicatedQueryNetworkInterface`).

use std::net::SocketAddr;
use std::sync::Arc;

use parking_lot::RwLock;
use sha2::{Digest, Sha512};

/// Handshake packet type (9).
pub const HANDSHAKE: u8 = 9;
/// Statistics packet type (0).
pub const STATISTICS: u8 = 0;

/// `QueryInfo::GAME_ID`.
pub const GAME_ID: &str = "MINECRAFTPE";

/// Snapshot the query reports. Mirrors `QueryInfo`.
#[derive(Debug, Clone)]
pub struct QueryInfo {
    pub server_name: String,
    pub list_plugins: bool,
    pub plugins: Vec<String>,
    pub players: Vec<String>,
    pub gametype: String,
    pub version: String,
    pub server_engine: String,
    pub map: String,
    pub max_players: i32,
    pub whitelist: bool,
    pub port: i32,
    pub ip: String,
}

impl QueryInfo {
    pub fn new(
        server_name: impl Into<String>,
        plugins: Vec<String>,
        players: Vec<String>,
        max_players: i32,
        port: i32,
        ip: impl Into<String>,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            list_plugins: true,
            plugins,
            players,
            gametype: "SMP".to_string(),
            version: "1.26.60".to_string(),
            server_engine: "Axolotl-PM 5.51.2-dev".to_string(),
            map: "world".to_string(),
            max_players,
            whitelist: false,
            port,
            ip: ip.into(),
        }
    }

    pub fn player_count(&self) -> i32 {
        self.players.len() as i32
    }

    /// Long query (`\x00\x01player_\x00\x00` trailer + player list).
    pub fn long_query(&self) -> Vec<u8> {
        let mut plist = self.server_engine.clone();
        if !self.plugins.is_empty() && self.list_plugins {
            plist.push(':');
            for plugin in &self.plugins {
                let clean: String = plugin
                    .chars()
                    .map(|c| if c == ';' || c == ':' || c == ' ' { '_' } else { c })
                    .collect();
                plist.push(' ');
                plist.push_str(&clean);
                plist.push(';');
            }
            plist.pop();
        }

        let mut query = Vec::new();
        let push_kv = |query: &mut Vec<u8>, key: &str, value: &str| {
            query.extend_from_slice(key.as_bytes());
            query.push(0);
            query.extend_from_slice(value.as_bytes());
            query.push(0);
        };
        push_kv(&mut query, "splitnum", "\u{80}");
        push_kv(&mut query, "hostname", &self.server_name);
        push_kv(&mut query, "gametype", &self.gametype);
        push_kv(&mut query, "game_id", GAME_ID);
        push_kv(&mut query, "version", &self.version);
        push_kv(&mut query, "server_engine", &self.server_engine);
        push_kv(&mut query, "plugins", &plist);
        push_kv(&mut query, "map", &self.map);
        push_kv(
            &mut query,
            "numplayers",
            &self.player_count().to_string(),
        );
        push_kv(&mut query, "maxplayers", &self.max_players.to_string());
        push_kv(
            &mut query,
            "whitelist",
            if self.whitelist { "on" } else { "off" },
        );
        push_kv(&mut query, "hostip", &self.ip);
        push_kv(&mut query, "hostport", &self.port.to_string());
        query.extend_from_slice(b"\x00\x01player_\x00\x00");
        for player in &self.players {
            query.extend_from_slice(player.as_bytes());
            query.push(0);
        }
        query.push(0);
        query
    }

    /// Short query: `hostname\0 gametype\0 map\0 num\0 max\0 port(LE u16) ip\0`.
    pub fn short_query(&self) -> Vec<u8> {
        let mut query = Vec::new();
        query.extend_from_slice(self.server_name.as_bytes());
        query.push(0);
        query.extend_from_slice(self.gametype.as_bytes());
        query.push(0);
        query.extend_from_slice(self.map.as_bytes());
        query.push(0);
        query.extend_from_slice(self.player_count().to_string().as_bytes());
        query.push(0);
        query.extend_from_slice(self.max_players.to_string().as_bytes());
        query.push(0);
        query.extend_from_slice(&(self.port as u16).to_le_bytes());
        query.extend_from_slice(self.ip.as_bytes());
        query.push(0);
        query
    }
}

/// Stateful handler: keeps a rotating challenge token like `QueryHandler`.
#[derive(Debug)]
pub struct QueryHandler {
    info: RwLock<QueryInfo>,
    token: RwLock<Vec<u8>>,
    last_token: RwLock<Vec<u8>>,
}

impl QueryHandler {
    pub fn new(info: QueryInfo) -> Self {
        let token = random_token();
        Self {
            info: RwLock::new(info),
            token: RwLock::new(token.clone()),
            last_token: RwLock::new(token),
        }
    }

    pub fn info(&self) -> QueryInfo {
        self.info.read().clone()
    }

    pub fn set_info(&self, info: QueryInfo) {
        *self.info.write() = info;
    }

    pub fn regenerate_token(&self) {
        let mut token = self.token.write();
        let mut last = self.last_token.write();
        *last = token.clone();
        *token = random_token();
    }

    fn token_string(token: &[u8], salt: &str) -> i32 {
        let mut hasher = Sha512::new();
        hasher.update(salt.as_bytes());
        hasher.update(b":");
        hasher.update(token);
        let hash = hasher.finalize();
        i32::from_be_bytes([hash[7], hash[8], hash[9], hash[10]])
    }

    /// Returns `Some(response)` for a recognised query packet.
    pub fn handle(&self, requester: SocketAddr, packet: &[u8]) -> Option<Vec<u8>> {
        if packet.len() < 7 || packet[0] != 0xFE || packet[1] != 0xFD {
            return None;
        }
        let packet_type = packet[2];
        let session_id = u32::from_be_bytes([packet[3], packet[4], packet[5], packet[6]]);
        let salt = requester.ip().to_string();

        match packet_type {
            HANDSHAKE => {
                let token = self.token.read();
                let value = Self::token_string(&token, &salt);
                let mut out = Vec::with_capacity(8);
                out.push(HANDSHAKE);
                out.extend_from_slice(&session_id.to_be_bytes());
                out.extend_from_slice(&value.to_be_bytes());
                out.push(0);
                Some(out)
            }
            STATISTICS => {
                if packet.len() < 11 {
                    return Some(Vec::new());
                }
                let token = i32::from_be_bytes([packet[7], packet[8], packet[9], packet[10]]);
                let current = Self::token_string(&self.token.read(), &salt);
                let previous = Self::token_string(&self.last_token.read(), &salt);
                if token != current && token != previous {
                    return Some(Vec::new());
                }
                let remaining = packet.len() - 11;
                let info = self.info();
                let payload = if remaining == 4 {
                    // Spec: long query trailing `FF FF FF 01`.
                    info.long_query()
                } else {
                    info.short_query()
                };
                let mut out = Vec::with_capacity(8 + payload.len());
                out.push(STATISTICS);
                out.extend_from_slice(&session_id.to_be_bytes());
                out.extend_from_slice(&payload);
                Some(out)
            }
            _ => None,
        }
    }
}

fn random_token() -> Vec<u8> {
    // `random_bytes(16)`: read from the OS CSPRNG via `/dev/urandom` when
    // available, falling back to a time-seeded hash.
    #[cfg(unix)]
    {
        if let Ok(bytes) = std::fs::read("/dev/urandom") {
            if bytes.len() >= 16 {
                return bytes[..16].to_vec();
            }
        }
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = Sha512::new();
    hasher.update(now.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    hasher.finalize()[..16].to_vec()
}

/// UDP listener mirroring `DedicatedQueryNetworkInterface`.
pub struct QueryListener {
    socket: std::sync::Arc<tokio::net::UdpSocket>,
    handler: Arc<QueryHandler>,
}

impl QueryListener {
    /// Binds the query socket. `ip`/`port` are the RakNet port (or a
    /// dedicated port when `query-port` differs in `server.properties`).
    pub async fn bind(
        ip: &str,
        port: u16,
        info: QueryInfo,
    ) -> std::io::Result<Self> {
        Self::bind_with_handler(ip, port, Arc::new(QueryHandler::new(info))).await
    }

    /// Binds the query socket sharing an existing handler, so live
    /// `QueryInfo` refreshes (the RakNet raw-handler path) also reach the
    /// dedicated listener.
    pub async fn bind_with_handler(
        ip: &str,
        port: u16,
        handler: Arc<QueryHandler>,
    ) -> std::io::Result<Self> {
        let addr_text = if ip.contains(':') && !ip.starts_with('[') {
            format!("[{ip}]:{port}")
        } else {
            format!("{ip}:{port}")
        };
        let addr: SocketAddr = addr_text.parse().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid query address")
        })?;
        let socket = tokio::net::UdpSocket::bind(addr).await?;
        Ok(Self {
            socket: Arc::new(socket),
            handler,
        })
    }

    pub fn handler(&self) -> &Arc<QueryHandler> {
        &self.handler
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Serves until the socket is closed. Polls with a timeout so a shutdown
    /// flag can stop the loop (PM ticks the interface every server tick).
    pub async fn serve(&self) {
        let mut buffer = vec![0u8; 65_535];
        loop {
            let (len, peer) = match tokio::time::timeout(
                std::time::Duration::from_millis(250),
                self.socket.recv_from(&mut buffer),
            )
            .await
            {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => continue,
                Err(_) => continue,
            };
            if let Some(response) = self.handler.handle(peer, &buffer[..len]) {
                if !response.is_empty() {
                    let _ = self.socket.send_to(&response, peer).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler() -> QueryHandler {
        let mut info = QueryInfo::new("Test", vec!["TestKit 1.0".to_string()], vec!["Steve".to_string()], 100, 19132, "127.0.0.1");
        info.map = "world".to_string();
        QueryHandler::new(info)
    }

    #[test]
    fn handshake_and_short_query_round_trip() {
        let handler = handler();
        let peer: SocketAddr = "127.0.0.1:5000".parse().unwrap();

        let mut handshake = vec![0xFE, 0xFD, HANDSHAKE, 0, 0, 0, 1];
        let response = handler.handle(peer, &handshake).expect("handshake");
        assert_eq!(response[0], HANDSHAKE);
        assert_eq!(&response[1..5], &[0, 0, 0, 1]);
        assert_eq!(response[len_of(&response) - 1], 0);

        let token = i32::from_be_bytes([response[5], response[6], response[7], response[8]]);
        let mut stat = vec![0xFE, 0xFD, STATISTICS, 0, 0, 0, 2];
        stat.extend_from_slice(&token.to_be_bytes());
        let short = handler.handle(peer, &stat).expect("short query");
        assert_eq!(short[0], STATISTICS);
        let body = &short[5..];
        assert!(body.starts_with(b"Test\x00SMP\x00world\x001\x00100"));

        // Long query: trailing `FF FF FF 01` (4 unread bytes).
        let mut long = vec![0xFE, 0xFD, STATISTICS, 0, 0, 0, 3];
        long.extend_from_slice(&token.to_be_bytes());
        long.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0x01]);
        // Recompute: original QueryInfo test above consumed? No, token still valid.
        let long_response = handler.handle(peer, &long).expect("long query");
        assert_eq!(long_response[0], STATISTICS);
        assert!(long_response.windows(9).any(|w| w == b"hostname\x00"));
        handshake.clear();
    }

    fn len_of(v: &[u8]) -> usize {
        v.len()
    }

    #[test]
    fn bad_token_gets_no_payload() {
        let handler = handler();
        let peer: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let mut stat = vec![0xFE, 0xFD, STATISTICS, 0, 0, 0, 1];
        stat.extend_from_slice(&12345_i32.to_be_bytes());
        let response = handler.handle(peer, &stat).expect("handled");
        assert!(response.is_empty());
    }

    #[test]
    fn token_is_bound_to_requester_ip() {
        let handler = handler();
        let a: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let b: SocketAddr = "192.168.0.5:5000".parse().unwrap();
        let handshake_a = handler.handle(a, &[0xFE, 0xFD, HANDSHAKE, 0, 0, 0, 1]).unwrap();
        let token_a = i32::from_be_bytes([
            handshake_a[5],
            handshake_a[6],
            handshake_a[7],
            handshake_a[8],
        ]);
        let mut stat_b = vec![0xFE, 0xFD, STATISTICS, 0, 0, 0, 1];
        stat_b.extend_from_slice(&token_a.to_be_bytes());
        assert!(handler.handle(b, &stat_b).unwrap().is_empty());
    }
}
