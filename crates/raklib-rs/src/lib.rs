//! RakLib-rs: Asynchronous RakNet UDP transport in Rust

use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tracing::info;

pub const RAKNET_PROTOCOL_VERSION: u8 = 11;
pub const OFFLINE_MESSAGE_DATA_ID: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];

pub struct RakNetListener {
    socket: UdpSocket,
    pub server_id: u64,
}

impl RakNetListener {
    pub async fn bind(addr: SocketAddr, server_id: u64) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        info!("RakNet listener bound to {}", addr);
        Ok(Self { socket, server_id })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}
