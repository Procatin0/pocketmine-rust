//! Java Edition protocol crate: byte-for-byte ports of the JavaBridge
//! plugin's `Protocol` (1.12.2 / 340 primitives), `Nbt` (network NBT
//! writer) and `ModernProfile` (769/770/773/774/775 packet tables,
//! registry/tags/brand payloads, block states, 26.1 `update_time`).
//!
//! ```rust
//! use java_protocol::{ModernProfile, protocol, nbt};
//! let profile = ModernProfile::for_protocol(775).unwrap();
//! let frame = protocol::frame(profile.id("update_time"),
//!     &profile.update_time_payload(6000, true));
//! ```
//!
//! This crate is intentionally isolated from `bedrock-protocol` (Bedrock
//! uses little-endian + zigzag varints + a different NBT flavor; see
//! `nbt.rs`) and from the server loop (`session.rs`, `JavaServer`,
//! `ModernChunk` live outside it and consume it via `pub use` only).

pub mod modern;
pub mod nbt;
pub mod protocol;

pub use modern::ModernProfile;
pub use protocol as javabridge_protocol;
