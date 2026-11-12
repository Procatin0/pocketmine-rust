//! Packet ID 0x2E (46): ContainerOpenPacket.
//! Tells the client to open a container screen (anvil, crafting table ...,
//! and since 1.16 also the player's own inventory screen).
//!
//! Layout (Mojang `ContainerOpenPacket`, protocol 2225 / 1.26):
//! container id u8, container type u8, position BlockPos (3x var_i32),
//! target actor id `ActorUniqueID` (var_i64, `-1` for block containers).
//!
//! The previous layout (window id, `window_name` string, BlockPos,
//! var_u32 dynamic id) is not the wire shape of the current protocol, so a
//! real client could not decode it: neither the crafting table nor the
//! inventory screen could be opened.

use crate::binary::{CodecError, Reader, Writer};
use crate::types::BlockPos;

/// `WindowTypes::INVENTORY` (PocketMine `WindowTypes`): `-1` on the wire.
pub const CONTAINER_TYPE_INVENTORY: u8 = 0xFF;
/// `WindowTypes::CONTAINER`: chests, hoppers, ...
pub const CONTAINER_TYPE_CONTAINER: u8 = 0x00;
/// `WindowTypes::WORKBENCH`: crafting table.
pub const CONTAINER_TYPE_WORKBENCH: u8 = 0x01;
/// `WindowTypes::ANVIL`.
pub const CONTAINER_TYPE_ANVIL: u8 = 0x05;

/// `ActorUniqueID` used by `ContainerOpenPacket::blockInv` for block
/// containers (no actor owns the screen).
pub const BLOCK_CONTAINER_ACTOR_ID: i64 = -1;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContainerOpenPacket {
    pub window_id: u8,
    pub container_type: u8,
    pub position: BlockPos,
    pub actor_unique_id: i64,
}

impl ContainerOpenPacket {
    pub const PACKET_ID: u32 = 0x2E;

    pub fn new(
        window_id: u8,
        container_type: u8,
        position: BlockPos,
        actor_unique_id: i64,
    ) -> Self {
        Self { window_id, container_type, position, actor_unique_id }
    }

    /// PocketMine `ContainerOpenPacket::blockInv`: a block container screen.
    pub fn block(window_id: u8, container_type: u8, position: BlockPos) -> Self {
        Self::new(window_id, container_type, position, BLOCK_CONTAINER_ACTOR_ID)
    }

    /// PocketMine `ContainerOpenPacket::entityInv`: the player inventory
    /// screen, owned by the player entity itself.
    pub fn inventory(window_id: u8, actor_unique_id: i64) -> Self {
        Self::new(
            window_id,
            CONTAINER_TYPE_INVENTORY,
            BlockPos::new(0, 0, 0),
            actor_unique_id,
        )
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_byte(self.window_id);
        writer.write_byte(self.container_type);
        self.position.write(writer);
        writer.write_var_i64(self.actor_unique_id);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            window_id: reader.read_byte()?,
            container_type: reader.read_byte()?,
            position: BlockPos::read(reader)?,
            actor_unique_id: reader.read_var_i64()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_block_container() {
        let packet = ContainerOpenPacket::block(10, CONTAINER_TYPE_WORKBENCH, BlockPos::new(1, 64, -3));
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(ContainerOpenPacket::decode_payload(&mut reader).unwrap(), packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn roundtrips_inventory_screen() {
        let packet = ContainerOpenPacket::inventory(0, 7);
        assert_eq!(packet.container_type, CONTAINER_TYPE_INVENTORY);
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        // id, type, x, y, z, actor id
        assert_eq!(&bytes[..2], &[0x00, 0xFF]);
        let mut reader = Reader::new(&bytes);
        assert_eq!(ContainerOpenPacket::decode_payload(&mut reader).unwrap(), packet);
        assert_eq!(reader.remaining(), 0);
    }
}
