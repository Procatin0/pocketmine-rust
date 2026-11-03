//! Bedrock Protocol Crate - Zero-copy packet encoding/decoding

pub mod binary;
pub mod types;
pub mod packets;
pub mod batch;

pub use batch::PacketBatch;
pub use binary::{Reader, Writer};
pub use packets::*;
pub use types::*;

pub trait Packet: Send + Sync {
    fn packet_id(&self) -> u32;
    fn encode(&self, writer: &mut Writer) -> Result<(), binary::CodecError>;
    fn decode(reader: &mut Reader) -> Result<Self, binary::CodecError>
    where
        Self: Sized;
}
