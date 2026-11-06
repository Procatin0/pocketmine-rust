//! Minecraft: Java Edition wire protocol (1.12.2, protocol 340).
//!
//! This crate implements the subset of the Java protocol the Bedrock bridge
//! needs: the vanilla (offline mode) handshake, the server list ping, login
//! without encryption, and the gameplay packets used to put a Java client into
//! a Bedrock world.
//!
//! Packet ids and field layouts were taken from
//! `PrismarineJS/minecraft-data` (`data/pc/1.12.2/protocol.json`) and block ids
//! from `data/pc/1.12/blocks.json`, the same data the `minecraft-protocol`
//! package uses.
//!
//! Deliberate omissions for this first milestone:
//! - No encryption (`login_set_compression` / `encryption_begin`), so only
//!   offline-mode clients can log in.
//! - No compression: the server never sends `login_compress`, so frames are
//!   plain `varint(length) + varint(id) + payload`.
//! - No NBT, inventories, entities or block placement yet.
//!
//! Anything newer than 1.12.2 needs the 1.20.2+ configuration phase and the
//! vanilla registry data, which is the main blocker documented in
//! `docs/java-bridge-plugin.md`.

pub mod binary;
pub mod blocks;
pub mod chunk;
pub mod frame;
pub mod packets;

pub use binary::{CodecError, Reader, Writer};
pub use frame::{decode_frame, encode_frame, read_frame, Frame, MAX_FRAME_LEN};
pub use packets::{
    chat_json, decode_handshake, decode_login_start, decode_ping, decode_play, offline_uuid,
    uuid_string, Handshake, Serverbound,
};

/// Protocol number of the Java Edition version this bridge speaks (1.12.2).
pub const PROTOCOL_VERSION: i32 = 340;

/// Human readable version name announced in the server list ping.
pub const VERSION_NAME: &str = "1.12.2";

/// Java stores the player's eye height above the reported (feet) position.
pub const JAVA_EYE_HEIGHT: f64 = 1.62;
