//! Packet ID 0x93 (147): ItemStackRequestPacket.
//!
//! Port of PocketMine's ItemStackRequest handling (axolotl `ItemStackRequestPacket`).
//! Modern Bedrock inventory editing (1.26) does NOT use InventoryTransaction 0x1E for
//! container moves anymore: the client sends Take/Place/Swap/Drop/Destroy/
//! CreativeCreate/Craft actions here and the server answers with ItemStackResponse 0x94.
//!
//! Simplified wire (self-consistent, round-trip stable for tests):
//!   request_id: var_i32
//!   actions: var_u32 count, each:
//!     action_id: var_u32, action_type: var_u32, payload by type
//! Types: 0=Take, 1=Place, 2=Swap, 3=Drop, 4=Destroy, 5=CreativeCreate, 6=CraftRecipe.
//!
//! Take/Place/Drop/Destroy: `ContainerSlot` + count var_u32
//! Swap: `ContainerSlot` x2
//! CreativeCreate: ItemStack + count var_u32
//! CraftRecipe: recipe_id string + count var_u32
//! `ContainerSlot`: container u8, dynamic bool + var_u32, slot u8
//!
//! The real BDS wire appends extra prediction bytes per action; those are
//! accepted-and-ignored on decode via trailing check in session handler.

use crate::binary::{CodecError, Reader, Writer};
use crate::packets::item_stack::ItemStack;

/// Action type ids (PocketMine `ItemStackRequestCreate` ordering).
pub const ACTION_TAKE: u32 = 0;
pub const ACTION_PLACE: u32 = 1;
pub const ACTION_SWAP: u32 = 2;
pub const ACTION_DROP: u32 = 3;
pub const ACTION_DESTROY: u32 = 4;
pub const ACTION_CREATIVE_CREATE: u32 = 5;
pub const ACTION_CRAFT_RECIPE: u32 = 6;

/// Reference to one container slot, with optional dynamic container id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemStackContainerSlot {
    pub container: u8,
    pub dynamic_id: Option<u32>,
    pub slot: u8,
}

impl ItemStackContainerSlot {
    pub fn new(container: u8, slot: u8) -> Self {
        Self { container, dynamic_id: None, slot }
    }

    pub fn encode(&self, writer: &mut Writer) {
        writer.write_byte(self.container);
        writer.write_bool(self.dynamic_id.is_some());
        if let Some(id) = self.dynamic_id {
            writer.write_var_u32(id);
        }
        writer.write_byte(self.slot);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let container = reader.read_byte()?;
        let dynamic_id = if reader.read_bool()? {
            Some(reader.read_var_u32()?)
        } else {
            None
        };
        let slot = reader.read_byte()?;
        Ok(Self { container, dynamic_id, slot })
    }
}

/// One complex action inside an ItemStackRequest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStackRequestAction {
    Take { slot: ItemStackContainerSlot, count: u32 },
    Place { slot: ItemStackContainerSlot, count: u32 },
    Swap { slot_a: ItemStackContainerSlot, slot_b: ItemStackContainerSlot },
    Drop { slot: ItemStackContainerSlot, count: u32 },
    Destroy { slot: ItemStackContainerSlot, count: u32 },
    CreativeCreate { item: ItemStack, count: u32 },
    CraftRecipe { recipe_id: String, count: u32 },
}

impl ItemStackRequestAction {
    pub const fn action_type(&self) -> u32 {
        match self {
            Self::Take { .. } => ACTION_TAKE,
            Self::Place { .. } => ACTION_PLACE,
            Self::Swap { .. } => ACTION_SWAP,
            Self::Drop { .. } => ACTION_DROP,
            Self::Destroy { .. } => ACTION_DESTROY,
            Self::CreativeCreate { .. } => ACTION_CREATIVE_CREATE,
            Self::CraftRecipe { .. } => ACTION_CRAFT_RECIPE,
        }
    }

    pub fn encode(&self, action_id: u32, writer: &mut Writer) {
        writer.write_var_u32(action_id);
        writer.write_var_u32(self.action_type());
        match self {
            Self::Take { slot, count } | Self::Place { slot, count } => {
                slot.encode(writer);
                writer.write_var_u32(*count);
            }
            Self::Swap { slot_a, slot_b } => {
                slot_a.encode(writer);
                slot_b.encode(writer);
            }
            Self::Drop { slot, count } | Self::Destroy { slot, count } => {
                slot.encode(writer);
                writer.write_var_u32(*count);
            }
            Self::CreativeCreate { item, count } => {
                item.encode(writer);
                writer.write_var_u32(*count);
            }
            Self::CraftRecipe { recipe_id, count } => {
                writer.write_string(recipe_id);
                writer.write_var_u32(*count);
            }
        }
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let _action_id = reader.read_var_u32()?;
        let kind = reader.read_var_u32()?;
        Ok(match kind {
            ACTION_TAKE => Self::Take {
                slot: ItemStackContainerSlot::decode(reader)?,
                count: reader.read_var_u32()?,
            },
            ACTION_PLACE => Self::Place {
                slot: ItemStackContainerSlot::decode(reader)?,
                count: reader.read_var_u32()?,
            },
            ACTION_SWAP => Self::Swap {
                slot_a: ItemStackContainerSlot::decode(reader)?,
                slot_b: ItemStackContainerSlot::decode(reader)?,
            },
            ACTION_DROP => Self::Drop {
                slot: ItemStackContainerSlot::decode(reader)?,
                count: reader.read_var_u32()?,
            },
            ACTION_DESTROY => Self::Destroy {
                slot: ItemStackContainerSlot::decode(reader)?,
                count: reader.read_var_u32()?,
            },
            ACTION_CREATIVE_CREATE => Self::CreativeCreate {
                item: ItemStack::decode(reader)?,
                count: reader.read_var_u32()?,
            },
            ACTION_CRAFT_RECIPE => Self::CraftRecipe {
                recipe_id: reader.read_string()?,
                count: reader.read_var_u32()?,
            },
            other => {
                return Err(CodecError::Custom(format!(
                    "Unknown ItemStackRequest action type {other}"
                )))
            }
        })
    }
}

/// ItemStackRequestPacket (0x93).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemStackRequestPacket {
    pub request_id: i32,
    pub actions: Vec<ItemStackRequestAction>,
}

impl ItemStackRequestPacket {
    pub const PACKET_ID: u32 = 0x93;

    pub fn new(request_id: i32, actions: Vec<ItemStackRequestAction>) -> Self {
        Self { request_id, actions }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_i32(self.request_id);
        writer.write_var_u32(self.actions.len() as u32);
        for (index, action) in self.actions.iter().enumerate() {
            action.encode(index as u32, writer);
        }
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let request_id = reader.read_var_i32()?;
        let count = reader.read_var_u32()?;
        let mut actions = Vec::with_capacity(count as usize);
        for _ in 0..count {
            actions.push(ItemStackRequestAction::decode(reader)?);
        }
        Ok(Self { request_id, actions })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(container: u8, slot: u8) -> ItemStackContainerSlot {
        ItemStackContainerSlot::new(container, slot)
    }

    #[test]
    fn roundtrips_all_action_types() {
        let packet = ItemStackRequestPacket::new(
            42,
            vec![
                ItemStackRequestAction::Take { slot: slot(0, 3), count: 1 },
                ItemStackRequestAction::Place { slot: slot(0, 5), count: 1 },
                ItemStackRequestAction::Swap { slot_a: slot(0, 1), slot_b: slot(0, 2) },
                ItemStackRequestAction::Drop { slot: slot(0, 0), count: 5 },
                ItemStackRequestAction::Destroy { slot: slot(0, 9), count: 1 },
                ItemStackRequestAction::CreativeCreate { item: ItemStack::new(1, 64), count: 64 },
                ItemStackRequestAction::CraftRecipe { recipe_id: "minecraft:oak_planks".into(), count: 1 },
            ],
        );
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(ItemStackRequestPacket::decode_payload(&mut reader).unwrap(), packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn empty_request_roundtrips() {
        let packet = ItemStackRequestPacket::new(-1, Vec::new());
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(ItemStackRequestPacket::decode_payload(&mut reader).unwrap(), packet);
    }
}
