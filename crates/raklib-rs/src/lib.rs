//! RakLib-rs: Asynchronous RakNet UDP transport in Rust

pub mod offline;

pub use offline::*;

use bytes::Bytes;
use std::net::SocketAddr;
use std::sync::RwLock;
use thiserror::Error;
use tokio::net::UdpSocket;
use tracing::{debug, info};

pub const RAKNET_PROTOCOL_VERSION: u8 = 11;
pub const OFFLINE_MESSAGE_DATA_ID: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];

#[derive(Debug, Error)]
pub enum RakNetError {
    #[error("Packet error: {0}")]
    Packet(#[from] PacketError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RakNetEvent {
    Offline {
        packet: OfflinePacket,
        src: SocketAddr,
        response: Option<Bytes>,
    },
}

pub struct RakNetListener {
    socket: UdpSocket,
    pub server_id: u64,
    motd: RwLock<String>,
}

impl RakNetListener {
    pub async fn bind(addr: SocketAddr, server_id: u64) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        info!("RakNet listener bound to {}", addr);
        let motd = format_bedrock_motd(server_id);
        Ok(Self {
            socket,
            server_id,
            motd: RwLock::new(motd),
        })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }

    pub fn motd(&self) -> String {
        self.motd.read().unwrap().clone()
    }

    pub fn set_motd(&self, motd: impl Into<String>) {
        *self.motd.write().unwrap() = motd.into();
    }

    /// Pure offline packet processor that produces an offline response packet when applicable.
    pub fn handle_offline_packet(
        &self,
        packet: &OfflinePacket,
        src: SocketAddr,
    ) -> Option<OfflinePacket> {
        match packet {
            OfflinePacket::UnconnectedPing(ping) => {
                Some(OfflinePacket::UnconnectedPong(UnconnectedPong {
                    ping_time: ping.ping_time,
                    server_id: self.server_id,
                    motd: self.motd(),
                }))
            }
            OfflinePacket::OpenConnectionRequest1(req) => {
                let negotiated_mtu = req.mtu_size.min(1492);
                Some(OfflinePacket::OpenConnectionReply1(OpenConnectionReply1 {
                    server_id: self.server_id,
                    server_security: false,
                    mtu_size: negotiated_mtu,
                }))
            }
            OfflinePacket::OpenConnectionRequest2(req) => {
                Some(OfflinePacket::OpenConnectionReply2(OpenConnectionReply2 {
                    server_id: self.server_id,
                    client_address: src,
                    mtu_size: req.mtu_size,
                    server_security: false,
                }))
            }
            _ => None,
        }
    }

    /// Handles an incoming raw UDP datagram. If it contains a recognized offline packet,
    /// sends the response back to `src` and returns `Ok(Some(response_bytes))`.
    pub async fn handle_packet(
        &self,
        data: &[u8],
        src: SocketAddr,
    ) -> Result<Option<Bytes>, RakNetError> {
        let packet = OfflinePacket::decode(data)?;
        if let Some(reply) = self.handle_offline_packet(&packet, src) {
            let bytes = reply.encode();
            self.socket.send_to(&bytes, src).await?;
            Ok(Some(bytes))
        } else {
            Ok(None)
        }
    }

    /// Polls for the next datagram from the UDP socket and handles any offline packets.
    /// Loops continuously over non-offline or malformed datagrams until an event occurs.
    pub async fn poll_event(&self) -> Result<RakNetEvent, RakNetError> {
        let mut buf = [0u8; 2048];
        loop {
            let (len, src) = self.socket.recv_from(&mut buf).await?;
            match OfflinePacket::decode(&buf[..len]) {
                Ok(packet) => {
                    let response = if let Some(reply) = self.handle_offline_packet(&packet, src) {
                        let bytes = reply.encode();
                        self.socket.send_to(&bytes, src).await?;
                        Some(bytes)
                    } else {
                        None
                    };
                    return Ok(RakNetEvent::Offline {
                        packet,
                        src,
                        response,
                    });
                }
                Err(e) => {
                    debug!("Ignored packet from {}: {}", src, e);
                    continue;
                }
            }
        }
    }
}
