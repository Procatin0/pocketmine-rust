use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;
use crate::OFFLINE_MESSAGE_DATA_ID;

pub const ID_UNCONNECTED_PING: u8 = 0x01;
pub const ID_UNCONNECTED_PING_OPEN_CONNECTIONS: u8 = 0x02;
pub const ID_OPEN_CONNECTION_REQUEST_1: u8 = 0x05;
pub const ID_OPEN_CONNECTION_REPLY_1: u8 = 0x06;
pub const ID_OPEN_CONNECTION_REQUEST_2: u8 = 0x07;
pub const ID_OPEN_CONNECTION_REPLY_2: u8 = 0x08;
pub const ID_UNCONNECTED_PONG: u8 = 0x1c;

#[derive(Debug, Error)]
pub enum PacketError {
    #[error("Buffer too short: needed {needed}, available {available}")]
    BufferTooShort { needed: usize, available: usize },
    #[error("Invalid magic in offline message")]
    InvalidMagic,
    #[error("Invalid packet ID: {0:#04x}")]
    InvalidPacketId(u8),
    #[error("Invalid IP version: {0}")]
    InvalidIpVersion(u8),
    #[error("UTF-8 decode error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

/// Helper function to format default Bedrock MOTD string.
pub fn format_bedrock_motd(server_id: u64) -> String {
    format!("MCPE;Dedicated PocketMine-Rust Server;748;1.21.20;0;1000;{server_id};Survival;1;19132;19133;")
}

pub fn read_address(buf: &mut impl Buf) -> Result<SocketAddr, PacketError> {
    if buf.remaining() < 1 {
        return Err(PacketError::BufferTooShort {
            needed: 1,
            available: buf.remaining(),
        });
    }
    let version = buf.get_u8();
    match version {
        4 => {
            if buf.remaining() < 6 {
                return Err(PacketError::BufferTooShort {
                    needed: 6,
                    available: buf.remaining(),
                });
            }
            let a = buf.get_u8();
            let b = buf.get_u8();
            let c = buf.get_u8();
            let d = buf.get_u8();
            let port = buf.get_u16();
            Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(a, b, c, d)), port))
        }
        6 => {
            if buf.remaining() >= 28 {
                let _family = buf.get_u16_le();
                let port = buf.get_u16();
                let _flow = buf.get_u32();
                let mut octets = [0u8; 16];
                buf.copy_to_slice(&mut octets);
                let _scope = buf.get_u32();
                Ok(SocketAddr::new(IpAddr::V6(Ipv6Addr::from(octets)), port))
            } else if buf.remaining() >= 18 {
                let mut octets = [0u8; 16];
                buf.copy_to_slice(&mut octets);
                let port = buf.get_u16();
                Ok(SocketAddr::new(IpAddr::V6(Ipv6Addr::from(octets)), port))
            } else {
                Err(PacketError::BufferTooShort {
                    needed: 18,
                    available: buf.remaining(),
                })
            }
        }
        v => Err(PacketError::InvalidIpVersion(v)),
    }
}

pub fn write_address(buf: &mut impl BufMut, addr: &SocketAddr) {
    match addr {
        SocketAddr::V4(v4) => {
            buf.put_u8(4);
            buf.put_slice(&v4.ip().octets());
            buf.put_u16(v4.port());
        }
        SocketAddr::V6(v6) => {
            buf.put_u8(6);
            buf.put_u16_le(23); // AF_INET6
            buf.put_u16(v6.port());
            buf.put_u32(0); // FlowInfo
            buf.put_slice(&v6.ip().octets());
            buf.put_u32(0); // ScopeId
        }
    }
}

/// RakNet Unconnected Ping packet (0x01 or 0x02)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnconnectedPing {
    pub ping_time: u64,
    pub client_guid: u64,
}

impl UnconnectedPing {
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(33);
        buf.put_u8(ID_UNCONNECTED_PING);
        buf.put_u64(self.ping_time);
        buf.put_slice(&OFFLINE_MESSAGE_DATA_ID);
        buf.put_u64(self.client_guid);
        buf.freeze()
    }

    pub fn decode(data: &[u8]) -> Result<Self, PacketError> {
        if data.len() < 25 {
            return Err(PacketError::BufferTooShort {
                needed: 25,
                available: data.len(),
            });
        }
        let id = data[0];
        if id != ID_UNCONNECTED_PING && id != ID_UNCONNECTED_PING_OPEN_CONNECTIONS {
            return Err(PacketError::InvalidPacketId(id));
        }
        let mut slice = &data[1..];
        let ping_time = slice.get_u64();
        let mut magic = [0u8; 16];
        slice.copy_to_slice(&mut magic);
        if magic != OFFLINE_MESSAGE_DATA_ID {
            return Err(PacketError::InvalidMagic);
        }
        let client_guid = if slice.remaining() >= 8 {
            slice.get_u64()
        } else {
            0
        };
        Ok(Self {
            ping_time,
            client_guid,
        })
    }
}

/// RakNet Unconnected Pong packet (0x1c)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnconnectedPong {
    pub ping_time: u64,
    pub server_id: u64,
    pub motd: String,
}

impl UnconnectedPong {
    pub fn encode(&self) -> Bytes {
        let motd_bytes = self.motd.as_bytes();
        let mut buf = BytesMut::with_capacity(35 + motd_bytes.len());
        buf.put_u8(ID_UNCONNECTED_PONG);
        buf.put_u64(self.ping_time);
        buf.put_u64(self.server_id);
        buf.put_slice(&OFFLINE_MESSAGE_DATA_ID);
        buf.put_u16(motd_bytes.len() as u16);
        buf.put_slice(motd_bytes);
        buf.freeze()
    }

    pub fn decode(data: &[u8]) -> Result<Self, PacketError> {
        if data.len() < 35 {
            return Err(PacketError::BufferTooShort {
                needed: 35,
                available: data.len(),
            });
        }
        let id = data[0];
        if id != ID_UNCONNECTED_PONG {
            return Err(PacketError::InvalidPacketId(id));
        }
        let mut slice = &data[1..];
        let ping_time = slice.get_u64();
        let server_id = slice.get_u64();
        let mut magic = [0u8; 16];
        slice.copy_to_slice(&mut magic);
        if magic != OFFLINE_MESSAGE_DATA_ID {
            return Err(PacketError::InvalidMagic);
        }
        let str_len = slice.get_u16() as usize;
        if slice.remaining() < str_len {
            return Err(PacketError::BufferTooShort {
                needed: str_len,
                available: slice.remaining(),
            });
        }
        let motd = String::from_utf8(slice[..str_len].to_vec())?;
        Ok(Self {
            ping_time,
            server_id,
            motd,
        })
    }
}

/// RakNet Open Connection Request 1 (0x05)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenConnectionRequest1 {
    pub protocol_version: u8,
    pub mtu_size: u16,
}

impl OpenConnectionRequest1 {
    pub fn encode(&self) -> Bytes {
        let padding_len = if self.mtu_size > 18 {
            (self.mtu_size - 18) as usize
        } else {
            0
        };
        let mut buf = BytesMut::with_capacity(18 + padding_len);
        buf.put_u8(ID_OPEN_CONNECTION_REQUEST_1);
        buf.put_slice(&OFFLINE_MESSAGE_DATA_ID);
        buf.put_u8(self.protocol_version);
        buf.put_bytes(0, padding_len);
        buf.freeze()
    }

    pub fn decode(data: &[u8]) -> Result<Self, PacketError> {
        if data.len() < 18 {
            return Err(PacketError::BufferTooShort {
                needed: 18,
                available: data.len(),
            });
        }
        let id = data[0];
        if id != ID_OPEN_CONNECTION_REQUEST_1 {
            return Err(PacketError::InvalidPacketId(id));
        }
        let mut magic = [0u8; 16];
        magic.copy_from_slice(&data[1..17]);
        if magic != OFFLINE_MESSAGE_DATA_ID {
            return Err(PacketError::InvalidMagic);
        }
        let protocol_version = data[17];
        let mtu_size = data.len() as u16;
        Ok(Self {
            protocol_version,
            mtu_size,
        })
    }
}

/// RakNet Open Connection Reply 1 (0x06)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenConnectionReply1 {
    pub server_id: u64,
    pub server_security: bool,
    pub mtu_size: u16,
}

impl OpenConnectionReply1 {
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(28);
        buf.put_u8(ID_OPEN_CONNECTION_REPLY_1);
        buf.put_slice(&OFFLINE_MESSAGE_DATA_ID);
        buf.put_u64(self.server_id);
        buf.put_u8(if self.server_security { 1 } else { 0 });
        buf.put_u16(self.mtu_size);
        buf.freeze()
    }

    pub fn decode(data: &[u8]) -> Result<Self, PacketError> {
        if data.len() < 28 {
            return Err(PacketError::BufferTooShort {
                needed: 28,
                available: data.len(),
            });
        }
        let id = data[0];
        if id != ID_OPEN_CONNECTION_REPLY_1 {
            return Err(PacketError::InvalidPacketId(id));
        }
        let mut magic = [0u8; 16];
        magic.copy_from_slice(&data[1..17]);
        if magic != OFFLINE_MESSAGE_DATA_ID {
            return Err(PacketError::InvalidMagic);
        }
        let mut slice = &data[17..];
        let server_id = slice.get_u64();
        let server_security = slice.get_u8() != 0;
        let mtu_size = slice.get_u16();
        Ok(Self {
            server_id,
            server_security,
            mtu_size,
        })
    }
}

/// RakNet Open Connection Request 2 (0x07)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenConnectionRequest2 {
    pub server_address: SocketAddr,
    pub mtu_size: u16,
    pub client_id: u64,
}

impl OpenConnectionRequest2 {
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(34);
        buf.put_u8(ID_OPEN_CONNECTION_REQUEST_2);
        buf.put_slice(&OFFLINE_MESSAGE_DATA_ID);
        write_address(&mut buf, &self.server_address);
        buf.put_u16(self.mtu_size);
        buf.put_u64(self.client_id);
        buf.freeze()
    }

    pub fn decode(data: &[u8]) -> Result<Self, PacketError> {
        if data.len() < 17 {
            return Err(PacketError::BufferTooShort {
                needed: 17,
                available: data.len(),
            });
        }
        let id = data[0];
        if id != ID_OPEN_CONNECTION_REQUEST_2 {
            return Err(PacketError::InvalidPacketId(id));
        }
        let mut magic = [0u8; 16];
        magic.copy_from_slice(&data[1..17]);
        if magic != OFFLINE_MESSAGE_DATA_ID {
            return Err(PacketError::InvalidMagic);
        }
        let mut slice = &data[17..];
        let server_address = read_address(&mut slice)?;
        if slice.remaining() < 10 {
            return Err(PacketError::BufferTooShort {
                needed: 10,
                available: slice.remaining(),
            });
        }
        let mtu_size = slice.get_u16();
        let client_id = slice.get_u64();
        Ok(Self {
            server_address,
            mtu_size,
            client_id,
        })
    }
}

/// RakNet Open Connection Reply 2 (0x08)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenConnectionReply2 {
    pub server_id: u64,
    pub client_address: SocketAddr,
    pub mtu_size: u16,
    pub server_security: bool,
}

impl OpenConnectionReply2 {
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(35);
        buf.put_u8(ID_OPEN_CONNECTION_REPLY_2);
        buf.put_slice(&OFFLINE_MESSAGE_DATA_ID);
        buf.put_u64(self.server_id);
        write_address(&mut buf, &self.client_address);
        buf.put_u16(self.mtu_size);
        buf.put_u8(if self.server_security { 1 } else { 0 });
        buf.freeze()
    }

    pub fn decode(data: &[u8]) -> Result<Self, PacketError> {
        if data.len() < 25 {
            return Err(PacketError::BufferTooShort {
                needed: 25,
                available: data.len(),
            });
        }
        let id = data[0];
        if id != ID_OPEN_CONNECTION_REPLY_2 {
            return Err(PacketError::InvalidPacketId(id));
        }
        let mut magic = [0u8; 16];
        magic.copy_from_slice(&data[1..17]);
        if magic != OFFLINE_MESSAGE_DATA_ID {
            return Err(PacketError::InvalidMagic);
        }
        let mut slice = &data[17..];
        let server_id = slice.get_u64();
        let client_address = read_address(&mut slice)?;
        if slice.remaining() < 3 {
            return Err(PacketError::BufferTooShort {
                needed: 3,
                available: slice.remaining(),
            });
        }
        let mtu_size = slice.get_u16();
        let server_security = slice.get_u8() != 0;
        Ok(Self {
            server_id,
            client_address,
            mtu_size,
            server_security,
        })
    }
}

/// Enum encompassing all RakNet offline packets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfflinePacket {
    UnconnectedPing(UnconnectedPing),
    UnconnectedPong(UnconnectedPong),
    OpenConnectionRequest1(OpenConnectionRequest1),
    OpenConnectionReply1(OpenConnectionReply1),
    OpenConnectionRequest2(OpenConnectionRequest2),
    OpenConnectionReply2(OpenConnectionReply2),
}

impl OfflinePacket {
    pub fn packet_id(&self) -> u8 {
        match self {
            Self::UnconnectedPing(_) => ID_UNCONNECTED_PING,
            Self::UnconnectedPong(_) => ID_UNCONNECTED_PONG,
            Self::OpenConnectionRequest1(_) => ID_OPEN_CONNECTION_REQUEST_1,
            Self::OpenConnectionReply1(_) => ID_OPEN_CONNECTION_REPLY_1,
            Self::OpenConnectionRequest2(_) => ID_OPEN_CONNECTION_REQUEST_2,
            Self::OpenConnectionReply2(_) => ID_OPEN_CONNECTION_REPLY_2,
        }
    }

    pub fn encode(&self) -> Bytes {
        match self {
            Self::UnconnectedPing(p) => p.encode(),
            Self::UnconnectedPong(p) => p.encode(),
            Self::OpenConnectionRequest1(p) => p.encode(),
            Self::OpenConnectionReply1(p) => p.encode(),
            Self::OpenConnectionRequest2(p) => p.encode(),
            Self::OpenConnectionReply2(p) => p.encode(),
        }
    }

    pub fn decode(data: &[u8]) -> Result<Self, PacketError> {
        if data.is_empty() {
            return Err(PacketError::BufferTooShort {
                needed: 1,
                available: 0,
            });
        }
        match data[0] {
            ID_UNCONNECTED_PING | ID_UNCONNECTED_PING_OPEN_CONNECTIONS => {
                Ok(Self::UnconnectedPing(UnconnectedPing::decode(data)?))
            }
            ID_UNCONNECTED_PONG => Ok(Self::UnconnectedPong(UnconnectedPong::decode(data)?)),
            ID_OPEN_CONNECTION_REQUEST_1 => Ok(Self::OpenConnectionRequest1(
                OpenConnectionRequest1::decode(data)?,
            )),
            ID_OPEN_CONNECTION_REPLY_1 => Ok(Self::OpenConnectionReply1(
                OpenConnectionReply1::decode(data)?,
            )),
            ID_OPEN_CONNECTION_REQUEST_2 => Ok(Self::OpenConnectionRequest2(
                OpenConnectionRequest2::decode(data)?,
            )),
            ID_OPEN_CONNECTION_REPLY_2 => Ok(Self::OpenConnectionReply2(
                OpenConnectionReply2::decode(data)?,
            )),
            id => Err(PacketError::InvalidPacketId(id)),
        }
    }
}
