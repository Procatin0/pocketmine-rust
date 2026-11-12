//! Packet ID 0x93 (147): ItemStackRequestPacket.
//!
//! Modern Bedrock inventory editing (1.26) does not use `InventoryTransaction`
//! 0x1E for container moves: the client sends Take/Place/Swap/Drop/Destroy/
//! CreativeCreate actions here and the server answers with
//! `ItemStackResponse` 0x94.
//!
//! Layout (ported field by field from `axolotl-pm/BedrockProtocol`
//! `types/inventory/stackrequest/*`, protocol 2225 / 1.26.60):
//!   requests: var_u32 count, each:
//!     request_id: var_i32 (zigzag)
//!     actions: var_u32 count, each:
//!       type: var_u32 (outer `ItemStackRequestActionType`)
//!       inner_type: u8 (`ItemStackRequestActionType::INNER_TYPES[type]`)
//!       payload by type
//!     filter_strings: var_u32 count + strings
//!     filter_string_cause: i32 LE
//!
//! Two details broke every real request before: the action type is a
//! **var_u32 followed by a second inner-type byte** (a single u8 made the
//! decoder read the inner byte as the type and desync), and the stack net id
//! of a slot is a plain **i32 LE**, not a varint.
//!
//! `ContainerSlot` (`ItemStackRequestSlotInfo`): container u8 (UI id, see
//! [`UI_*`] below), optional dynamic id (bool + u32 LE), slot u8 and the
//! item stack net id (i32 LE) the client believes the slot holds.

use crate::binary::{CodecError, Reader, Writer};

/// `ItemStackRequestActionType` ids (outer numbering).
pub const ACTION_TAKE: u32 = 0;
pub const ACTION_PLACE: u32 = 1;
pub const ACTION_SWAP: u32 = 2;
pub const ACTION_DROP: u32 = 3;
pub const ACTION_DESTROY: u32 = 4;
pub const ACTION_CRAFTING_CONSUME_INPUT: u32 = 5;
pub const ACTION_CRAFTING_CREATE_SPECIFIC_RESULT: u32 = 6;
pub const ACTION_LAB_TABLE_COMBINE: u32 = 7;
pub const ACTION_BEACON_PAYMENT: u32 = 8;
pub const ACTION_MINE_BLOCK: u32 = 9;
pub const ACTION_CRAFTING_RECIPE: u32 = 10;
pub const ACTION_CRAFTING_RECIPE_AUTO: u32 = 11;
pub const ACTION_CREATIVE_CREATE: u32 = 12;
pub const ACTION_CRAFTING_RECIPE_OPTIONAL: u32 = 13;
pub const ACTION_CRAFTING_GRINDSTONE: u32 = 14;
pub const ACTION_CRAFTING_LOOM: u32 = 15;
pub const ACTION_CRAFTING_RESERVED: u32 = 16;
pub const ACTION_CRAFTING_NON_IMPLEMENTED: u32 = 17;
pub const ACTION_CRAFTING_RESULTS_DEPRECATED: u32 = 18;

/// Inner type byte written after the outer action type
/// (`ItemStackRequestActionType::INNER_TYPES`). It is not the same numbering:
/// 7 and 8 are the two deprecated container actions, so everything from
/// `LAB_TABLE_COMBINE` on is shifted by two.
pub const fn action_inner_type(action_type: u32) -> u8 {
    match action_type {
        ACTION_TAKE => 0,
        ACTION_PLACE => 1,
        ACTION_SWAP => 2,
        ACTION_DROP => 3,
        ACTION_DESTROY => 4,
        ACTION_CRAFTING_CONSUME_INPUT => 5,
        ACTION_CRAFTING_CREATE_SPECIFIC_RESULT => 6,
        ACTION_LAB_TABLE_COMBINE => 9,
        ACTION_BEACON_PAYMENT => 10,
        ACTION_MINE_BLOCK => 11,
        ACTION_CRAFTING_RECIPE => 12,
        ACTION_CRAFTING_RECIPE_AUTO => 13,
        ACTION_CREATIVE_CREATE => 14,
        ACTION_CRAFTING_RECIPE_OPTIONAL => 15,
        ACTION_CRAFTING_GRINDSTONE => 16,
        ACTION_CRAFTING_LOOM => 17,
        ACTION_CRAFTING_RESERVED => 18,
        ACTION_CRAFTING_NON_IMPLEMENTED => 19,
        ACTION_CRAFTING_RESULTS_DEPRECATED => 20,
        other => other as u8,
    }
}

/// Container UI ids the client uses inside `ItemStackRequest`
/// (`ContainerUIIds`). They are *not* window ids: they must be translated
/// before touching an inventory (see `crafting::container_slot`).
pub const UI_ANVIL_INPUT: u8 = 0;
pub const UI_ARMOR: u8 = 6;
pub const UI_COMBINED_HOTBAR_AND_INVENTORY: u8 = 12;
pub const UI_CRAFTING_INPUT: u8 = 13;
pub const UI_CRAFTING_OUTPUT_PREVIEW: u8 = 14;
pub const UI_HOTBAR: u8 = 28;
pub const UI_INVENTORY: u8 = 29;
pub const UI_OFFHAND: u8 = 34;
pub const UI_CURSOR: u8 = 59;
pub const UI_CREATED_OUTPUT: u8 = 60;
/// Only slot of [`UI_CREATED_OUTPUT`]: the crafted/creative "created item".
pub const CREATED_ITEM_SLOT: u8 = 50;
/// First UI slot of the 2x2 crafting grid (`UIInventorySlotOffset`).
pub const UI_CRAFT_2X2_SLOT: u8 = 28;
/// First UI slot of the 3x3 crafting grid.
pub const UI_CRAFT_3X3_SLOT: u8 = 32;

/// Cap for client-sent list lengths, mirroring the protocol limits.
const MAX_LIST: u32 = 128;

/// Reference to one container slot inside a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemStackContainerSlot {
    /// Container UI id ([`UI_INVENTORY`], [`UI_HOTBAR`], ...).
    pub container: u8,
    pub dynamic_id: Option<u32>,
    pub slot: u8,
    /// Item stack net id the client expects in the slot; `0` for empty.
    /// Ignored by the server (it keeps no stack-id bookkeeping).
    pub stack_id: i32,
}

impl ItemStackContainerSlot {
    pub fn new(container: u8, slot: u8) -> Self {
        Self { container, dynamic_id: None, slot, stack_id: 0 }
    }

    pub fn encode(&self, writer: &mut Writer) {
        writer.write_byte(self.container);
        writer.write_bool(self.dynamic_id.is_some());
        if let Some(id) = self.dynamic_id {
            writer.write_u32_le(id);
        }
        writer.write_byte(self.slot);
        // `ItemStackNetIdVariant` is a raw i32 LE, not a varint.
        writer.write_i32_le(self.stack_id);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let container = reader.read_byte()?;
        let dynamic_id = if reader.read_bool()? {
            Some(reader.read_u32_le()?)
        } else {
            None
        };
        let slot = reader.read_byte()?;
        let stack_id = reader.read_i32_le()?;
        Ok(Self { container, dynamic_id, slot, stack_id })
    }
}

/// `ItemDescriptorType`: 0 empty, 1 name+meta, 2 molang, 3 item tag.
const DESCRIPTOR_EMPTY: u32 = 0;
const DESCRIPTOR_STRING_ID_META: u32 = 1;
const DESCRIPTOR_MOLANG: u32 = 2;
const DESCRIPTOR_TAG: u32 = 3;

/// Item descriptor used by the recipe-book actions
/// (`CommonTypes::readItemDescriptorNormal`). Only decoded to keep the stream
/// aligned; the server does not resolve recipe-book ingredients.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemDescriptor {
    Empty,
    StringIdMeta { id: String, meta: i32 },
    Molang { expression: String, version: u16 },
    Tag { tag: String },
}

impl ItemDescriptor {
    pub fn encode(&self, writer: &mut Writer) {
        let descriptor_type = match self {
            Self::Empty => DESCRIPTOR_EMPTY,
            Self::StringIdMeta { .. } => DESCRIPTOR_STRING_ID_META,
            Self::Molang { .. } => DESCRIPTOR_MOLANG,
            Self::Tag { .. } => DESCRIPTOR_TAG,
        };
        writer.write_var_u32(descriptor_type);
        writer.write_byte(descriptor_type as u8);
        match self {
            Self::Empty => {}
            Self::StringIdMeta { id, meta } => {
                writer.write_string(id);
                writer.write_var_i32(*meta);
            }
            Self::Molang { expression, version } => {
                writer.write_string(expression);
                writer.write_u16_le(*version);
            }
            Self::Tag { tag } => writer.write_string(tag),
        }
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let descriptor_type = reader.read_var_u32()?;
        let inner = reader.read_byte()?;
        if inner as u32 != descriptor_type {
            return Err(CodecError::Custom(format!(
                "Item descriptor type mismatch: outer {descriptor_type}, inner {inner}"
            )));
        }
        Ok(match descriptor_type {
            DESCRIPTOR_EMPTY => Self::Empty,
            DESCRIPTOR_STRING_ID_META => Self::StringIdMeta {
                id: reader.read_string()?,
                meta: reader.read_var_i32()?,
            },
            DESCRIPTOR_MOLANG => Self::Molang {
                expression: reader.read_string()?,
                version: reader.read_u16_le()?,
            },
            DESCRIPTOR_TAG => Self::Tag { tag: reader.read_string()? },
            other => {
                return Err(CodecError::Custom(format!(
                    "Unsupported item descriptor type {other}"
                )))
            }
        })
    }
}

/// One entry of a `CraftRecipeAuto` ingredient list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeIngredient {
    pub descriptor: ItemDescriptor,
    pub count: u16,
}

impl RecipeIngredient {
    fn encode(&self, writer: &mut Writer) {
        self.descriptor.encode(writer);
        writer.write_u16_le(self.count);
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            descriptor: ItemDescriptor::decode(reader)?,
            count: reader.read_u16_le()?,
        })
    }
}

/// One entry of the deprecated crafting-results action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeprecatedCraftingResult {
    pub descriptor: ItemDescriptor,
    pub count: u16,
    pub block_runtime_id: u32,
    pub extra_data: String,
}

impl DeprecatedCraftingResult {
    fn encode(&self, writer: &mut Writer) {
        self.descriptor.encode(writer);
        writer.write_u16_le(self.count);
        writer.write_var_u32(self.block_runtime_id);
        writer.write_string(&self.extra_data);
    }

    fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(Self {
            descriptor: ItemDescriptor::decode(reader)?,
            count: reader.read_u16_le()?,
            block_runtime_id: reader.read_var_u32()?,
            extra_data: reader.read_string()?,
        })
    }
}

/// One action inside an `ItemStackRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStackRequestAction {
    Take { count: u8, source: ItemStackContainerSlot, destination: ItemStackContainerSlot },
    Place { count: u8, source: ItemStackContainerSlot, destination: ItemStackContainerSlot },
    Swap { slot1: ItemStackContainerSlot, slot2: ItemStackContainerSlot },
    Drop { count: u8, source: ItemStackContainerSlot, randomly: bool },
    Destroy { count: u8, source: ItemStackContainerSlot },
    CraftingConsumeInput { count: u8, source: ItemStackContainerSlot },
    /// Multi-output recipes: selects which crafted result the created-item
    /// slot now holds.
    CraftingCreateSpecificResult { result_index: u8 },
    LabTableCombine,
    BeaconPayment { primary_effect: i32, secondary_effect: i32 },
    MineBlock { hotbar_slot: i32, predicted_durability: i32, stack_id: i32 },
    CraftingRecipe { recipe_network_id: u32, repetitions: u8 },
    /// Recipe-book craft with the ingredient list the client used.
    CraftingRecipeAuto {
        recipe_network_id: u32,
        repetitions: u8,
        ingredients: Vec<RecipeIngredient>,
    },
    /// Copies an entry of the creative catalog (by creative item network id)
    /// into the "created item" slot; the client then takes it with a
    /// [`ItemStackRequestAction::Take`] from [`UI_CREATED_OUTPUT`].
    CreativeCreate { creative_item_id: u32, repetitions: u8 },
    CraftingRecipeOptional { recipe_network_id: u32, filter_string_index: i32 },
    CraftingGrindstone { recipe_network_id: u32, repair_cost: i32, repetitions: u8 },
    CraftingLoom { pattern_id: String, repetitions: u8 },
    CraftingReserved { reserved_id: String, num_crafts: u8 },
    CraftingNonImplemented,
    CraftingResultsDeprecated { results: Vec<DeprecatedCraftingResult>, iterations: u8 },
}

impl ItemStackRequestAction {
    pub const fn action_type(&self) -> u32 {
        match self {
            Self::Take { .. } => ACTION_TAKE,
            Self::Place { .. } => ACTION_PLACE,
            Self::Swap { .. } => ACTION_SWAP,
            Self::Drop { .. } => ACTION_DROP,
            Self::Destroy { .. } => ACTION_DESTROY,
            Self::CraftingConsumeInput { .. } => ACTION_CRAFTING_CONSUME_INPUT,
            Self::CraftingCreateSpecificResult { .. } => ACTION_CRAFTING_CREATE_SPECIFIC_RESULT,
            Self::LabTableCombine => ACTION_LAB_TABLE_COMBINE,
            Self::BeaconPayment { .. } => ACTION_BEACON_PAYMENT,
            Self::MineBlock { .. } => ACTION_MINE_BLOCK,
            Self::CraftingRecipe { .. } => ACTION_CRAFTING_RECIPE,
            Self::CraftingRecipeAuto { .. } => ACTION_CRAFTING_RECIPE_AUTO,
            Self::CreativeCreate { .. } => ACTION_CREATIVE_CREATE,
            Self::CraftingRecipeOptional { .. } => ACTION_CRAFTING_RECIPE_OPTIONAL,
            Self::CraftingGrindstone { .. } => ACTION_CRAFTING_GRINDSTONE,
            Self::CraftingLoom { .. } => ACTION_CRAFTING_LOOM,
            Self::CraftingReserved { .. } => ACTION_CRAFTING_RESERVED,
            Self::CraftingNonImplemented => ACTION_CRAFTING_NON_IMPLEMENTED,
            Self::CraftingResultsDeprecated { .. } => ACTION_CRAFTING_RESULTS_DEPRECATED,
        }
    }

    pub fn encode(&self, writer: &mut Writer) {
        let action_type = self.action_type();
        writer.write_var_u32(action_type);
        writer.write_byte(action_inner_type(action_type));
        match self {
            Self::Take { count, source, destination }
            | Self::Place { count, source, destination } => {
                writer.write_byte(*count);
                source.encode(writer);
                destination.encode(writer);
            }
            Self::Swap { slot1, slot2 } => {
                slot1.encode(writer);
                slot2.encode(writer);
            }
            Self::Drop { count, source, randomly } => {
                writer.write_byte(*count);
                source.encode(writer);
                writer.write_bool(*randomly);
            }
            Self::Destroy { count, source } | Self::CraftingConsumeInput { count, source } => {
                writer.write_byte(*count);
                source.encode(writer);
            }
            Self::CraftingCreateSpecificResult { result_index } => writer.write_byte(*result_index),
            Self::LabTableCombine | Self::CraftingNonImplemented => {}
            Self::BeaconPayment { primary_effect, secondary_effect } => {
                writer.write_var_i32(*primary_effect);
                writer.write_var_i32(*secondary_effect);
            }
            Self::MineBlock { hotbar_slot, predicted_durability, stack_id } => {
                writer.write_var_i32(*hotbar_slot);
                writer.write_var_i32(*predicted_durability);
                writer.write_i32_le(*stack_id);
            }
            Self::CraftingRecipe { recipe_network_id, repetitions } => {
                writer.write_var_u32(*recipe_network_id);
                writer.write_byte(*repetitions);
            }
            Self::CreativeCreate { creative_item_id, repetitions } => {
                writer.write_var_u32(*creative_item_id);
                writer.write_byte(*repetitions);
            }
            Self::CraftingRecipeAuto { recipe_network_id, repetitions, ingredients } => {
                writer.write_var_u32(*recipe_network_id);
                writer.write_byte(*repetitions);
                writer.write_var_u32(ingredients.len() as u32);
                for ingredient in ingredients {
                    ingredient.encode(writer);
                }
            }
            Self::CraftingRecipeOptional { recipe_network_id, filter_string_index } => {
                writer.write_var_u32(*recipe_network_id);
                writer.write_i32_le(*filter_string_index);
            }
            Self::CraftingGrindstone { recipe_network_id, repair_cost, repetitions } => {
                // Bedrock writes the recipe id as u32 LE and the repair cost
                // as a varint, with the repetitions in between.
                writer.write_u32_le(*recipe_network_id);
                writer.write_byte(*repetitions);
                writer.write_var_i32(*repair_cost);
            }
            Self::CraftingLoom { pattern_id, repetitions } => {
                writer.write_string(pattern_id);
                writer.write_byte(*repetitions);
            }
            Self::CraftingReserved { reserved_id, num_crafts } => {
                writer.write_string(reserved_id);
                writer.write_byte(*num_crafts);
            }
            Self::CraftingResultsDeprecated { results, iterations } => {
                writer.write_var_u32(results.len() as u32);
                for result in results {
                    result.encode(writer);
                }
                writer.write_byte(*iterations);
            }
        }
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let action_type = reader.read_var_u32()?;
        let inner = reader.read_byte()?;
        let expected = action_inner_type(action_type);
        if inner != expected {
            return Err(CodecError::Custom(format!(
                "ItemStackRequest action type mismatch: outer {action_type}, expected inner {expected}, actual inner {inner}"
            )));
        }
        Ok(match action_type {
            ACTION_TAKE => Self::Take {
                count: reader.read_byte()?,
                source: ItemStackContainerSlot::decode(reader)?,
                destination: ItemStackContainerSlot::decode(reader)?,
            },
            ACTION_PLACE => Self::Place {
                count: reader.read_byte()?,
                source: ItemStackContainerSlot::decode(reader)?,
                destination: ItemStackContainerSlot::decode(reader)?,
            },
            ACTION_SWAP => Self::Swap {
                slot1: ItemStackContainerSlot::decode(reader)?,
                slot2: ItemStackContainerSlot::decode(reader)?,
            },
            ACTION_DROP => Self::Drop {
                count: reader.read_byte()?,
                source: ItemStackContainerSlot::decode(reader)?,
                randomly: reader.read_bool()?,
            },
            ACTION_DESTROY => Self::Destroy {
                count: reader.read_byte()?,
                source: ItemStackContainerSlot::decode(reader)?,
            },
            ACTION_CRAFTING_CONSUME_INPUT => Self::CraftingConsumeInput {
                count: reader.read_byte()?,
                source: ItemStackContainerSlot::decode(reader)?,
            },
            ACTION_CRAFTING_CREATE_SPECIFIC_RESULT => Self::CraftingCreateSpecificResult {
                result_index: reader.read_byte()?,
            },
            ACTION_LAB_TABLE_COMBINE => Self::LabTableCombine,
            ACTION_BEACON_PAYMENT => Self::BeaconPayment {
                primary_effect: reader.read_var_i32()?,
                secondary_effect: reader.read_var_i32()?,
            },
            ACTION_MINE_BLOCK => Self::MineBlock {
                hotbar_slot: reader.read_var_i32()?,
                predicted_durability: reader.read_var_i32()?,
                stack_id: reader.read_i32_le()?,
            },
            ACTION_CRAFTING_RECIPE => Self::CraftingRecipe {
                recipe_network_id: reader.read_var_u32()?,
                repetitions: reader.read_byte()?,
            },
            ACTION_CRAFTING_RECIPE_AUTO => {
                let recipe_network_id = reader.read_var_u32()?;
                let repetitions = reader.read_byte()?;
                let count = reader.read_var_u32()?;
                if count > MAX_LIST {
                    return Err(CodecError::Custom(format!(
                        "Too many recipe ingredients: {count}"
                    )));
                }
                let mut ingredients = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    ingredients.push(RecipeIngredient::decode(reader)?);
                }
                Self::CraftingRecipeAuto { recipe_network_id, repetitions, ingredients }
            }
            ACTION_CREATIVE_CREATE => Self::CreativeCreate {
                creative_item_id: reader.read_var_u32()?,
                repetitions: reader.read_byte()?,
            },
            ACTION_CRAFTING_RECIPE_OPTIONAL => Self::CraftingRecipeOptional {
                recipe_network_id: reader.read_var_u32()?,
                filter_string_index: reader.read_i32_le()?,
            },
            ACTION_CRAFTING_GRINDSTONE => {
                let recipe_network_id = reader.read_u32_le()?;
                let repetitions = reader.read_byte()?;
                let repair_cost = reader.read_var_i32()?;
                Self::CraftingGrindstone { recipe_network_id, repair_cost, repetitions }
            }
            ACTION_CRAFTING_LOOM => Self::CraftingLoom {
                pattern_id: reader.read_string()?,
                repetitions: reader.read_byte()?,
            },
            ACTION_CRAFTING_RESERVED => Self::CraftingReserved {
                reserved_id: reader.read_string()?,
                num_crafts: reader.read_byte()?,
            },
            ACTION_CRAFTING_NON_IMPLEMENTED => Self::CraftingNonImplemented,
            ACTION_CRAFTING_RESULTS_DEPRECATED => {
                let count = reader.read_var_u32()?;
                if count > MAX_LIST {
                    return Err(CodecError::Custom(format!(
                        "Too many deprecated crafting results: {count}"
                    )));
                }
                let mut results = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    results.push(DeprecatedCraftingResult::decode(reader)?);
                }
                Self::CraftingResultsDeprecated {
                    results,
                    iterations: reader.read_byte()?,
                }
            }
            other => {
                return Err(CodecError::Custom(format!(
                    "Unsupported ItemStackRequest action type {other}"
                )))
            }
        })
    }
}

/// One request inside an `ItemStackRequestPacket`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemStackRequest {
    pub request_id: i32,
    pub actions: Vec<ItemStackRequestAction>,
    pub filter_strings: Vec<String>,
    pub filter_string_cause: i32,
}

impl ItemStackRequest {
    pub fn new(request_id: i32, actions: Vec<ItemStackRequestAction>) -> Self {
        Self { request_id, actions, filter_strings: Vec::new(), filter_string_cause: 0 }
    }

    pub fn encode(&self, writer: &mut Writer) {
        writer.write_var_i32(self.request_id);
        writer.write_var_u32(self.actions.len() as u32);
        for action in &self.actions {
            action.encode(writer);
        }
        writer.write_var_u32(self.filter_strings.len() as u32);
        for string in &self.filter_strings {
            writer.write_string(string);
        }
        writer.write_i32_le(self.filter_string_cause);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let request_id = reader.read_var_i32()?;
        let count = reader.read_var_u32()?;
        if count > MAX_LIST {
            return Err(CodecError::Custom(format!(
                "Too many ItemStackRequest actions: {count}"
            )));
        }
        let mut actions = Vec::with_capacity(count as usize);
        for _ in 0..count {
            actions.push(ItemStackRequestAction::decode(reader)?);
        }
        let string_count = reader.read_var_u32()?;
        if string_count > MAX_LIST {
            return Err(CodecError::Custom(format!(
                "Too many ItemStackRequest filter strings: {string_count}"
            )));
        }
        let mut filter_strings = Vec::with_capacity(string_count as usize);
        for _ in 0..string_count {
            filter_strings.push(reader.read_string()?);
        }
        let filter_string_cause = reader.read_i32_le()?;
        Ok(Self { request_id, actions, filter_strings, filter_string_cause })
    }
}

/// ItemStackRequestPacket (0x93).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemStackRequestPacket {
    pub requests: Vec<ItemStackRequest>,
}

impl ItemStackRequestPacket {
    pub const PACKET_ID: u32 = 0x93;

    /// Single-request packet (the shape a client uses for one inventory move).
    pub fn new(request_id: i32, actions: Vec<ItemStackRequestAction>) -> Self {
        Self { requests: vec![ItemStackRequest::new(request_id, actions)] }
    }

    pub fn with_requests(requests: Vec<ItemStackRequest>) -> Self {
        Self { requests }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        writer.write_var_u32(self.requests.len() as u32);
        for request in &self.requests {
            request.encode(writer);
        }
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let count = reader.read_var_u32()?;
        if count > MAX_LIST {
            return Err(CodecError::Custom(format!(
                "Too many ItemStackRequests: {count}"
            )));
        }
        let mut requests = Vec::with_capacity(count as usize);
        for _ in 0..count {
            requests.push(ItemStackRequest::decode(reader)?);
        }
        Ok(Self { requests })
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
                ItemStackRequestAction::Take {
                    count: 1,
                    source: slot(UI_CREATED_OUTPUT, CREATED_ITEM_SLOT),
                    destination: slot(UI_HOTBAR, 3),
                },
                ItemStackRequestAction::Place {
                    count: 2,
                    source: slot(UI_INVENTORY, 5),
                    destination: slot(UI_CRAFTING_INPUT, UI_CRAFT_2X2_SLOT),
                },
                ItemStackRequestAction::Swap { slot1: slot(UI_HOTBAR, 1), slot2: slot(UI_INVENTORY, 20) },
                ItemStackRequestAction::Drop { count: 5, source: slot(UI_HOTBAR, 0), randomly: false },
                ItemStackRequestAction::Destroy { count: 1, source: slot(UI_INVENTORY, 9) },
                ItemStackRequestAction::CraftingConsumeInput {
                    count: 1,
                    source: slot(UI_CRAFTING_INPUT, UI_CRAFT_3X3_SLOT),
                },
                ItemStackRequestAction::CraftingCreateSpecificResult { result_index: 1 },
                ItemStackRequestAction::LabTableCombine,
                ItemStackRequestAction::BeaconPayment { primary_effect: 1, secondary_effect: 0 },
                ItemStackRequestAction::MineBlock {
                    hotbar_slot: 2,
                    predicted_durability: 55,
                    stack_id: -3,
                },
                ItemStackRequestAction::CraftingRecipe { recipe_network_id: 100, repetitions: 1 },
                ItemStackRequestAction::CraftingRecipeAuto {
                    recipe_network_id: 7,
                    repetitions: 2,
                    ingredients: vec![RecipeIngredient {
                        descriptor: ItemDescriptor::StringIdMeta {
                            id: "minecraft:log".into(),
                            meta: 1,
                        },
                        count: 3,
                    }],
                },
                ItemStackRequestAction::CreativeCreate { creative_item_id: 7, repetitions: 1 },
                ItemStackRequestAction::CraftingRecipeOptional {
                    recipe_network_id: 12,
                    filter_string_index: -1,
                },
                ItemStackRequestAction::CraftingGrindstone {
                    recipe_network_id: 3,
                    repair_cost: 4,
                    repetitions: 1,
                },
                ItemStackRequestAction::CraftingLoom {
                    pattern_id: "cre".into(),
                    repetitions: 1,
                },
                ItemStackRequestAction::CraftingReserved {
                    reserved_id: "minecraft:stone".into(),
                    num_crafts: 2,
                },
                ItemStackRequestAction::CraftingNonImplemented,
                ItemStackRequestAction::CraftingResultsDeprecated {
                    results: vec![DeprecatedCraftingResult {
                        descriptor: ItemDescriptor::Tag { tag: "minecraft:planks".into() },
                        count: 4,
                        block_runtime_id: 9,
                        extra_data: String::new(),
                    }],
                    iterations: 1,
                },
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

    /// The wire shape of one creative take, byte by byte: request id,
    /// action count, action type (`var_u32` outer + inner byte), count, source
    /// slot info, destination slot info, then the empty filter-string list and
    /// its cause.
    #[test]
    fn creative_take_matches_the_documented_wire() {
        let packet = ItemStackRequestPacket::new(
            1,
            vec![ItemStackRequestAction::Take {
                count: 1,
                source: slot(UI_CREATED_OUTPUT, CREATED_ITEM_SLOT),
                destination: slot(UI_HOTBAR, 0),
            }],
        );
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        assert_eq!(
            bytes,
            vec![
                0x01, // request count
                0x02, // request id 1 (zigzag)
                0x01, // action count
                0x00, // action type TAKE (var_u32)
                0x00, // inner action type TAKE
                0x01, // count
                UI_CREATED_OUTPUT,
                0x00, // no dynamic id
                50,   // created item slot
                0x00, 0x00, 0x00, 0x00, // stack id (i32 LE)
                UI_HOTBAR,
                0x00, // no dynamic id
                0,    // hotbar slot 0
                0x00, 0x00, 0x00, 0x00, // stack id (i32 LE)
                0x00, // no filter strings
                0x00, 0x00, 0x00, 0x00, // filter string cause
            ]
        );
    }

    /// A drop as a real client sends it: the inner type byte is the *inner*
    /// numbering, which differs from the outer one for the crafting actions.
    #[test]
    fn inner_type_byte_follows_the_inner_enum() {
        let packet = ItemStackRequestPacket::new(
            3,
            vec![ItemStackRequestAction::CreativeCreate { creative_item_id: 1, repetitions: 1 }],
        );
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        assert_eq!(
            bytes,
            vec![
                0x01, 0x06, // one request, id 3
                0x01, // one action
                0x0C, // outer CREATIVE_CREATE (12)
                0x0E, // inner CREATIVE_CREATE (14)
                0x01, // creative item id
                0x01, // repetitions
                0x00, // no filter strings
                0x00, 0x00, 0x00, 0x00, // cause
            ]
        );
    }
}
