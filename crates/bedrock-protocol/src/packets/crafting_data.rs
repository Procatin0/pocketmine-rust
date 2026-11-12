//! Packet ID 0x34 (52): CraftingDataPacket.
//!
//! Wire format for protocol 2225 (1.26.60), ported field by field from
//! BedrockProtocol's `CraftingDataPacket.php` + the `types/recipe/*`
//! serializers and `serializer/CommonTypes.php` (`getRecipeIngredient`,
//! `getItemStackWithoutStackId`, `readRecipeNetId`), as used by Axolotl's
//! `CraftingDataCache::buildCraftingDataCache` (only shaped, shapeless,
//! potion-type and potion-container lists are populated; every other list
//! is empty there, so this server emits zero counts for them too).
//!
//! Wire (payload, after the packet id varu): one list per recipe class,
//! each a varu count followed by that many entries — there is NO
//! interleaved recipe-type tag in this protocol revision:
//!   shaped count varu, then per ShapedRecipe:
//!     recipe_id string, width var_i32, height var_i32,
//!     ingredient_count varu (= width*height), width*height descriptors,
//!     output_count varu + ItemStackWithoutStackId items, uuid 16 bytes
//!     (two reversed 8-byte halves), block string, priority var_i32,
//!     symmetric bool, optional unlocking requirement, network_id varu
//!   shapeless count varu, then per ShapelessRecipe:
//!     recipe_id string, ingredient_count varu + descriptors,
//!     output_count varu + items, uuid, block, priority var_i32,
//!     optional unlocking requirement, network_id varu
//!   multi count varu (uuid + network_id), user-data shapeless count varu,
//!   shapeless-chemistry count varu, shaped-chemistry count varu,
//!   smithing-transform count varu (recipe_id string, template/input/
//!     addition descriptors, output item, block string, network_id varu),
//!   smithing-trim count varu (same minus the output item),
//!   potion-type count varu (6 var_i32: input id/meta, ingredient id/meta,
//!     output id/meta), potion-container count varu (3 var_i32), then
//!   material-reducer count varu (packed input var_i32
//!     `(id << 16) | meta`, output list varu of (id var_i32, count
//!     var_i32)), and finally clear_recipes bool.
//!
//! A recipe ingredient ("messy" descriptor, `CommonTypes::
//! getRecipeIngredient`): variant byte 0 for the empty descriptor (followed
//! by var_i32 32767), or variant byte 1 + a type string (`"name"`,
//! `"item_tag"` or `"molang"`) + payload + count var_i32. Payloads:
//! `name` = item string id + meta var_i32; `item_tag` = tag string + meta
//! var_i32; `molang` = expression string + u16 LE version.
//!
//! An output item is `ItemStackWithoutStackId`: network_id var_i32, count
//! u16 LE, meta varu, block_runtime_id var_i32, extra-data blob as a
//! length-prefixed byte string (the `ItemStackExtraData` serialization:
//! i16 NBT length + optional versioned LE NBT, then canPlaceOn /
//! canDestroy as u32 counts of u16-length-prefixed byte strings). The
//! empty extra data is 10 zero bytes (i16 0 + u32 0 + u32 0);
//! [`CraftingItemStack::from_item_stack`] reproduces it for an empty
//! inventory NBT, exactly like `TypeConverter::coreItemStackToNet`.
//!
//! The unlocking requirement is itself an optional: a present flag bool,
//! then context var_i32 (0 none, 1 always unlocked, 2 player in water,
//! 3 player has many items) and an optional list of ingredient
//! descriptors (only allowed when the context is none). Axolotl always
//! sends context 1 with no ingredient list. See
//! `RecipeUnlockingRequirement.php` / `RecipeUnlockingContext.php`.
//!
//! NOTE: the old interleaved layout (recipe-count then `type varu` per
//! recipe, numeric int-id descriptors, varu counts, a stack-id in output
//! items) was protocol <= 1.20 and does NOT parse on 2225 clients: the
//! client reads the first recipe as a shaped one and eventually hits a
//! garbage string length (`invalid string readNoHeader failed!` on
//! 0x34), even for the pre-1.21 BDS capture under
//! `data/bds-capture/crafting_data/`.

use crate::binary::{CodecError, Reader, Writer};
use crate::packets::item_stack::ItemStack;

/// Unlocking context: explicit unlocking ingredients are listed.
pub const UNLOCK_CONTEXT_NONE: i32 = 0;
/// Unlocking context: the recipe is always known to the player.
pub const UNLOCK_CONTEXT_ALWAYS_UNLOCKED: i32 = 1;
/// Unlocking context: unlocked while the player is in water.
pub const UNLOCK_CONTEXT_PLAYER_IN_WATER: i32 = 2;
/// Unlocking context: unlocked once the player holds many items.
pub const UNLOCK_CONTEXT_PLAYER_HAS_MANY_ITEMS: i32 = 3;

/// Wildcard meta (`0x7fff`) marking "any meta" in name/tag descriptors.
pub const WILDCARD_META: i32 = 32767;

/// One crafting ingredient descriptor (item-string based, `CommonTypes::
/// readItemDescriptorMess`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeDescriptor {
    /// Empty grid cell (variant byte 0 + var_i32 32767, then count).
    Invalid { count: i32 },
    /// `name` descriptor: canonical item string id + meta.
    StringIdMeta {
        id: String,
        meta: i32,
        count: i32,
    },
    /// `item_tag` descriptor: tag string + meta.
    ItemTag {
        tag: String,
        meta: i32,
        count: i32,
    },
    /// `molang` descriptor: expression string + u16 LE version.
    Molang {
        expression: String,
        version: u16,
        count: i32,
    },
}

impl RecipeDescriptor {
    /// Empty grid cell (`invalid`, count 0).
    pub fn invalid() -> Self {
        Self::Invalid { count: 0 }
    }

    pub fn encode(&self, writer: &mut Writer) {
        match self {
            Self::Invalid { count } => {
                writer.write_byte(0);
                writer.write_var_i32(WILDCARD_META);
                writer.write_var_i32(*count);
            }
            Self::StringIdMeta { id, meta, count } => {
                writer.write_byte(1);
                writer.write_string("name");
                writer.write_string(id);
                writer.write_var_i32(*meta);
                writer.write_var_i32(*count);
            }
            Self::ItemTag { tag, meta, count } => {
                writer.write_byte(1);
                writer.write_string("item_tag");
                writer.write_string(tag);
                writer.write_var_i32(*meta);
                writer.write_var_i32(*count);
            }
            Self::Molang {
                expression,
                version,
                count,
            } => {
                writer.write_byte(1);
                writer.write_string("molang");
                writer.write_string(expression);
                writer.write_u16_le(*version);
                writer.write_var_i32(*count);
            }
        }
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        Ok(match reader.read_byte()? {
            0 => {
                let meta = reader.read_var_i32()?;
                if meta != WILDCARD_META {
                    return Err(CodecError::Custom(format!(
                        "Expected meta 32767 for empty item descriptor, got {meta}"
                    )));
                }
                Self::Invalid {
                    count: reader.read_var_i32()?,
                }
            }
            1 => match reader.read_string()?.as_str() {
                "name" => {
                    let id = reader.read_string()?;
                    let meta = reader.read_var_i32()?;
                    Self::StringIdMeta {
                        id,
                        meta,
                        count: reader.read_var_i32()?,
                    }
                }
                "item_tag" => {
                    let tag = reader.read_string()?;
                    let meta = reader.read_var_i32()?;
                    Self::ItemTag {
                        tag,
                        meta,
                        count: reader.read_var_i32()?,
                    }
                }
                "molang" => {
                    let expression = reader.read_string()?;
                    let version = reader.read_u16_le()?;
                    Self::Molang {
                        expression,
                        version,
                        count: reader.read_var_i32()?,
                    }
                }
                other => {
                    return Err(CodecError::Custom(format!(
                        "Unknown crafting descriptor type \"{other}\""
                    )))
                }
            },
            other => {
                return Err(CodecError::Custom(format!(
                    "Expected 0 or 1 for item descriptor variant, got {other}"
                )))
            }
        })
    }
}

/// One output item of a recipe: `ItemStackWithoutStackId` on the wire.
/// `extra_data` is the `ItemStackExtraData` blob (`CommonTypes::
/// putItemStackWithoutStackId` writes it as a byte string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftingItemStack {
    pub network_id: i32,
    pub count: u16,
    pub meta: u32,
    pub block_runtime_id: i32,
    pub extra_data: Vec<u8>,
}

impl CraftingItemStack {
    pub fn new(network_id: i32, count: u16, meta: u32) -> Self {
        Self {
            network_id,
            count,
            meta,
            block_runtime_id: 0,
            extra_data: vec![0; 10],
        }
    }

    /// Converts an inventory [`ItemStack`] (the format `CraftingManager`
    /// stores). An empty NBT becomes the 10 zero bytes Axolotl serializes
    /// for a plain output; non-empty NBT is carried verbatim (crafting
    /// results from the recipe JSON carry none).
    pub fn from_item_stack(item: &ItemStack) -> Self {
        Self {
            network_id: item.network_id as i32,
            count: item.count,
            meta: item.meta,
            block_runtime_id: item.block_runtime_id as i32,
            extra_data: if item.nbt.is_empty() {
                vec![0; 10]
            } else {
                item.nbt.clone()
            },
        }
    }

    pub fn encode(&self, writer: &mut Writer) {
        writer.write_var_i32(self.network_id);
        writer.write_u16_le(self.count);
        writer.write_var_u32(self.meta);
        writer.write_var_i32(self.block_runtime_id);
        writer.write_var_u32(self.extra_data.len() as u32);
        writer.write_bytes(&self.extra_data);
    }

    pub fn decode(reader: &mut Reader) -> Result<Self, CodecError> {
        let network_id = reader.read_var_i32()?;
        let count = reader.read_u16_le()?;
        let meta = reader.read_var_u32()?;
        let block_runtime_id = reader.read_var_i32()?;
        let len = reader.read_var_u32()? as usize;
        Ok(Self {
            network_id,
            count,
            meta,
            block_runtime_id,
            extra_data: reader.read_bytes(len)?.to_vec(),
        })
    }
}

/// How a recipe becomes known to the player
/// (`RecipeUnlockingRequirement`): context var_i32 plus an optional
/// ingredient list (only with [`UNLOCK_CONTEXT_NONE`]).
#[derive(Debug, Clone, PartialEq)]
pub struct UnlockingRequirement {
    pub context: i32,
    pub ingredients: Option<Vec<RecipeDescriptor>>,
}

impl UnlockingRequirement {
    /// What Axolotl sends for every recipe: always unlocked, no list.
    pub fn always_unlocked() -> Self {
        Self {
            context: UNLOCK_CONTEXT_ALWAYS_UNLOCKED,
            ingredients: None,
        }
    }

    pub fn none(ingredients: Vec<RecipeDescriptor>) -> Self {
        Self {
            context: UNLOCK_CONTEXT_NONE,
            ingredients: Some(ingredients),
        }
    }

    fn encode(&self, writer: &mut Writer) {
        writer.write_bool(true);
        writer.write_var_i32(self.context);
        match &self.ingredients {
            None => writer.write_bool(false),
            Some(ingredients) => {
                writer.write_bool(true);
                writer.write_var_u32(ingredients.len() as u32);
                for ingredient in ingredients {
                    ingredient.encode(writer);
                }
            }
        }
    }

    fn decode(reader: &mut Reader) -> Result<Option<Self>, CodecError> {
        if !reader.read_bool()? {
            return Ok(None);
        }
        let context = reader.read_var_i32()?;
        let ingredients = if reader.read_bool()? {
            let count = reader.read_var_u32()?;
            let mut list = Vec::with_capacity(count as usize);
            for _ in 0..count {
                list.push(RecipeDescriptor::decode(reader)?);
            }
            Some(list)
        } else {
            None
        };
        if context != UNLOCK_CONTEXT_NONE && ingredients.is_some() {
            return Err(CodecError::Custom(
                "Unlocking ingredients should only be set when context is CONTEXT_NONE".into(),
            ));
        }
        Ok(Some(Self {
            context,
            ingredients,
        }))
    }
}

fn write_optional_unlocking(
    writer: &mut Writer,
    unlocking: &Option<UnlockingRequirement>,
) {
    match unlocking {
        None => writer.write_bool(false),
        Some(requirement) => requirement.encode(writer),
    }
}

fn read_optional_unlocking(
    reader: &mut Reader,
) -> Result<Option<UnlockingRequirement>, CodecError> {
    UnlockingRequirement::decode(reader)
}

/// A shaped (grid) recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedRecipe {
    pub recipe_id: String,
    pub width: u32,
    pub height: u32,
    pub inputs: Vec<RecipeDescriptor>,
    pub outputs: Vec<CraftingItemStack>,
    pub uuid: [u8; 16],
    pub block: String,
    pub priority: i32,
    pub assume_symmetry: bool,
    pub unlocking: Option<UnlockingRequirement>,
    pub network_id: u32,
}

/// A shapeless recipe. Furnace / blast-furnace / smoker / campfire
/// smelting recipes are shapeless on the wire with `block` naming the
/// furnace, exactly like Axolotl sends them.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapelessRecipe {
    pub recipe_id: String,
    pub inputs: Vec<RecipeDescriptor>,
    pub outputs: Vec<CraftingItemStack>,
    pub uuid: [u8; 16],
    pub block: String,
    pub priority: i32,
    pub unlocking: Option<UnlockingRequirement>,
    pub network_id: u32,
}

/// A "multi" recipe (repair / maps / banners / fireworks): just an
/// identity UUID plus the network id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiRecipe {
    pub uuid: [u8; 16],
    pub network_id: u32,
}

/// A smithing-transform recipe (template + input + addition -> output).
#[derive(Debug, Clone, PartialEq)]
pub struct SmithingTransformRecipe {
    pub recipe_id: String,
    pub template: RecipeDescriptor,
    pub input: RecipeDescriptor,
    pub addition: RecipeDescriptor,
    pub output: CraftingItemStack,
    pub block: String,
    pub network_id: u32,
}

/// A smithing-trim recipe (template + input + addition, no output item).
#[derive(Debug, Clone, PartialEq)]
pub struct SmithingTrimRecipe {
    pub recipe_id: String,
    pub template: RecipeDescriptor,
    pub input: RecipeDescriptor,
    pub addition: RecipeDescriptor,
    pub block: String,
    pub network_id: u32,
}

/// One material-reducer output (numeric item id + count).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialReducerOutput {
    pub item_id: i32,
    pub count: i32,
}

/// One material-reducer recipe: a packed input (`(id << 16) | meta`)
/// plus its output list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialReducerRecipe {
    pub input_id: i32,
    pub input_meta: i32,
    pub outputs: Vec<MaterialReducerOutput>,
}

/// One recipe in the recipe book, grouped by class on the wire (there is
/// no per-recipe type tag in protocol 2225).
#[derive(Debug, Clone, PartialEq)]
pub enum CraftingRecipe {
    Shaped(ShapedRecipe),
    Shapeless(ShapelessRecipe),
    Multi(MultiRecipe),
    SmithingTransform(SmithingTransformRecipe),
    SmithingTrim(SmithingTrimRecipe),
}

fn write_outputs(writer: &mut Writer, outputs: &[CraftingItemStack]) {
    writer.write_var_u32(outputs.len() as u32);
    for output in outputs {
        output.encode(writer);
    }
}

fn read_outputs(reader: &mut Reader) -> Result<Vec<CraftingItemStack>, CodecError> {
    let count = reader.read_var_u32()?;
    let mut outputs = Vec::with_capacity(count as usize);
    for _ in 0..count {
        outputs.push(CraftingItemStack::decode(reader)?);
    }
    Ok(outputs)
}

fn read_descriptors_counted(
    reader: &mut Reader,
) -> Result<Vec<RecipeDescriptor>, CodecError> {
    let count = reader.read_var_u32()?;
    let mut inputs = Vec::with_capacity(count as usize);
    for _ in 0..count {
        inputs.push(RecipeDescriptor::decode(reader)?);
    }
    Ok(inputs)
}

/// UUID wire order (`CommonTypes::putUUID` / `getUUID`): two little-endian
/// longs — each 8-byte half byte-reversed.
fn write_uuid(writer: &mut Writer, uuid: &[u8; 16]) {
    let mut first = [0u8; 8];
    first.copy_from_slice(&uuid[..8]);
    let mut second = [0u8; 8];
    second.copy_from_slice(&uuid[8..]);
    writer.write_bytes(&first.iter().rev().copied().collect::<Vec<_>>());
    writer.write_bytes(&second.iter().rev().copied().collect::<Vec<_>>());
}

fn read_uuid(reader: &mut Reader) -> Result<[u8; 16], CodecError> {
    let mut uuid = [0u8; 16];
    let first = reader.read_bytes(8)?;
    for (index, byte) in first.iter().rev().enumerate() {
        uuid[index] = *byte;
    }
    let second = reader.read_bytes(8)?;
    for (index, byte) in second.iter().rev().enumerate() {
        uuid[8 + index] = *byte;
    }
    Ok(uuid)
}

fn encode_shaped(writer: &mut Writer, recipe: &ShapedRecipe) {
    writer.write_string(&recipe.recipe_id);
    writer.write_var_i32(recipe.width as i32);
    writer.write_var_i32(recipe.height as i32);
    writer.write_var_u32((recipe.width * recipe.height) as u32);
    for input in &recipe.inputs {
        input.encode(writer);
    }
    write_outputs(writer, &recipe.outputs);
    write_uuid(writer, &recipe.uuid);
    writer.write_string(&recipe.block);
    writer.write_var_i32(recipe.priority);
    writer.write_bool(recipe.assume_symmetry);
    write_optional_unlocking(writer, &recipe.unlocking);
    writer.write_var_u32(recipe.network_id);
}

fn encode_shapeless(writer: &mut Writer, recipe: &ShapelessRecipe) {
    writer.write_string(&recipe.recipe_id);
    writer.write_var_u32(recipe.inputs.len() as u32);
    for input in &recipe.inputs {
        input.encode(writer);
    }
    write_outputs(writer, &recipe.outputs);
    write_uuid(writer, &recipe.uuid);
    writer.write_string(&recipe.block);
    writer.write_var_i32(recipe.priority);
    write_optional_unlocking(writer, &recipe.unlocking);
    writer.write_var_u32(recipe.network_id);
}

fn encode_multi(writer: &mut Writer, recipe: &MultiRecipe) {
    write_uuid(writer, &recipe.uuid);
    writer.write_var_u32(recipe.network_id);
}

fn encode_smithing_transform(writer: &mut Writer, recipe: &SmithingTransformRecipe) {
    writer.write_string(&recipe.recipe_id);
    recipe.template.encode(writer);
    recipe.input.encode(writer);
    recipe.addition.encode(writer);
    recipe.output.encode(writer);
    writer.write_string(&recipe.block);
    writer.write_var_u32(recipe.network_id);
}

fn encode_smithing_trim(writer: &mut Writer, recipe: &SmithingTrimRecipe) {
    writer.write_string(&recipe.recipe_id);
    recipe.template.encode(writer);
    recipe.input.encode(writer);
    recipe.addition.encode(writer);
    writer.write_string(&recipe.block);
    writer.write_var_u32(recipe.network_id);
}

fn decode_shaped(reader: &mut Reader) -> Result<ShapedRecipe, CodecError> {
    let recipe_id = reader.read_string()?;
    let width = reader.read_var_i32()? as u32;
    let height = reader.read_var_i32()? as u32;
    let count = reader.read_var_u32()?;
    if count != width * height {
        return Err(CodecError::Custom(format!(
            "Provided ingredient count {count} does not match width {width} * height {height}"
        )));
    }
    let mut inputs = Vec::with_capacity(count as usize);
    for _ in 0..count {
        inputs.push(RecipeDescriptor::decode(reader)?);
    }
    let outputs = read_outputs(reader)?;
    let uuid = read_uuid(reader)?;
    let block = reader.read_string()?;
    let priority = reader.read_var_i32()?;
    let assume_symmetry = reader.read_bool()?;
    let unlocking = read_optional_unlocking(reader)?;
    let network_id = reader.read_var_u32()?;
    Ok(ShapedRecipe {
        recipe_id,
        width,
        height,
        inputs,
        outputs,
        uuid,
        block,
        priority,
        assume_symmetry,
        unlocking,
        network_id,
    })
}

fn decode_shapeless(reader: &mut Reader) -> Result<ShapelessRecipe, CodecError> {
    let recipe_id = reader.read_string()?;
    let inputs = read_descriptors_counted(reader)?;
    let outputs = read_outputs(reader)?;
    let uuid = read_uuid(reader)?;
    let block = reader.read_string()?;
    let priority = reader.read_var_i32()?;
    let unlocking = read_optional_unlocking(reader)?;
    let network_id = reader.read_var_u32()?;
    Ok(ShapelessRecipe {
        recipe_id,
        inputs,
        outputs,
        uuid,
        block,
        priority,
        unlocking,
        network_id,
    })
}

fn decode_multi(reader: &mut Reader) -> Result<MultiRecipe, CodecError> {
    Ok(MultiRecipe {
        uuid: read_uuid(reader)?,
        network_id: reader.read_var_u32()?,
    })
}

fn decode_smithing_transform(
    reader: &mut Reader,
) -> Result<SmithingTransformRecipe, CodecError> {
    Ok(SmithingTransformRecipe {
        recipe_id: reader.read_string()?,
        template: RecipeDescriptor::decode(reader)?,
        input: RecipeDescriptor::decode(reader)?,
        addition: RecipeDescriptor::decode(reader)?,
        output: CraftingItemStack::decode(reader)?,
        block: reader.read_string()?,
        network_id: reader.read_var_u32()?,
    })
}

fn decode_smithing_trim(reader: &mut Reader) -> Result<SmithingTrimRecipe, CodecError> {
    Ok(SmithingTrimRecipe {
        recipe_id: reader.read_string()?,
        template: RecipeDescriptor::decode(reader)?,
        input: RecipeDescriptor::decode(reader)?,
        addition: RecipeDescriptor::decode(reader)?,
        block: reader.read_string()?,
        network_id: reader.read_var_u32()?,
    })
}

fn encode_material_reducer(writer: &mut Writer, recipe: &MaterialReducerRecipe) {
    writer.write_var_i32((recipe.input_id << 16) | recipe.input_meta);
    writer.write_var_u32(recipe.outputs.len() as u32);
    for output in &recipe.outputs {
        writer.write_var_i32(output.item_id);
        writer.write_var_i32(output.count);
    }
}

fn decode_material_reducer(reader: &mut Reader) -> Result<MaterialReducerRecipe, CodecError> {
    let packed = reader.read_var_i32()?;
    let count = reader.read_var_u32()?;
    let mut outputs = Vec::with_capacity(count as usize);
    for _ in 0..count {
        outputs.push(MaterialReducerOutput {
            item_id: reader.read_var_i32()?,
            count: reader.read_var_i32()?,
        });
    }
    Ok(MaterialReducerRecipe {
        input_id: packed >> 16,
        input_meta: packed & 0x7fff,
        outputs,
    })
}

/// One brewing-stand potion-type recipe (6 ids/metas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PotionTypeRecipe {
    pub input_id: i32,
    pub input_meta: i32,
    pub ingredient_id: i32,
    pub ingredient_meta: i32,
    pub output_id: i32,
    pub output_meta: i32,
}

/// One potion container change (3 ids, no metas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PotionContainerRecipe {
    pub input_id: i32,
    pub ingredient_id: i32,
    pub output_id: i32,
}

/// `CraftingDataPacket` (0x34). Recipes are held as one flat list on this
/// side; on the wire they are emitted grouped by class (shaped first,
/// then shapeless, multi, smithing) with empty lists for the classes the
/// server never fills (user-data, chemistry).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CraftingDataPacket {
    pub recipes: Vec<CraftingRecipe>,
    pub potion_types: Vec<PotionTypeRecipe>,
    pub potion_containers: Vec<PotionContainerRecipe>,
    pub material_reducers: Vec<MaterialReducerRecipe>,
    pub clear_recipes: bool,
}

impl CraftingDataPacket {
    pub const PACKET_ID: u32 = 0x34;

    pub fn new(
        recipes: Vec<CraftingRecipe>,
        potion_types: Vec<PotionTypeRecipe>,
        potion_containers: Vec<PotionContainerRecipe>,
    ) -> Self {
        Self {
            clear_recipes: true,
            recipes,
            potion_types,
            potion_containers,
            material_reducers: Vec::new(),
        }
    }

    pub fn encode_payload(&self, writer: &mut Writer) {
        // Each class is written under its own count in wire order; the
        // classes this server never fills (user-data, chemistry) get a
        // zero count, and any out-of-order recipe is still kept in its
        // own group.
        let mut shaped = Vec::new();
        let mut shapeless = Vec::new();
        let mut multi = Vec::new();
        let mut smithing_transforms = Vec::new();
        let mut smithing_trims = Vec::new();
        for recipe in &self.recipes {
            match recipe {
                CraftingRecipe::Shaped(_) => shaped.push(recipe),
                CraftingRecipe::Shapeless(_) => shapeless.push(recipe),
                CraftingRecipe::Multi(_) => multi.push(recipe),
                CraftingRecipe::SmithingTransform(_) => smithing_transforms.push(recipe),
                CraftingRecipe::SmithingTrim(_) => smithing_trims.push(recipe),
            }
        }

        writer.write_var_u32(shaped.len() as u32);
        for recipe in &shaped {
            if let CraftingRecipe::Shaped(recipe) = recipe {
                encode_shaped(writer, recipe);
            }
        }
        writer.write_var_u32(shapeless.len() as u32);
        for recipe in &shapeless {
            if let CraftingRecipe::Shapeless(recipe) = recipe {
                encode_shapeless(writer, recipe);
            }
        }
        writer.write_var_u32(multi.len() as u32);
        for recipe in &multi {
            if let CraftingRecipe::Multi(recipe) = recipe {
                encode_multi(writer, recipe);
            }
        }
        // user-data shapeless, shapeless chemistry, shaped chemistry.
        writer.write_var_u32(0);
        writer.write_var_u32(0);
        writer.write_var_u32(0);
        writer.write_var_u32(smithing_transforms.len() as u32);
        for recipe in &smithing_transforms {
            if let CraftingRecipe::SmithingTransform(recipe) = recipe {
                encode_smithing_transform(writer, recipe);
            }
        }
        writer.write_var_u32(smithing_trims.len() as u32);
        for recipe in &smithing_trims {
            if let CraftingRecipe::SmithingTrim(recipe) = recipe {
                encode_smithing_trim(writer, recipe);
            }
        }
        writer.write_var_u32(self.potion_types.len() as u32);
        for potion in &self.potion_types {
            writer.write_var_i32(potion.input_id);
            writer.write_var_i32(potion.input_meta);
            writer.write_var_i32(potion.ingredient_id);
            writer.write_var_i32(potion.ingredient_meta);
            writer.write_var_i32(potion.output_id);
            writer.write_var_i32(potion.output_meta);
        }
        writer.write_var_u32(self.potion_containers.len() as u32);
        for container in &self.potion_containers {
            writer.write_var_i32(container.input_id);
            writer.write_var_i32(container.ingredient_id);
            writer.write_var_i32(container.output_id);
        }
        writer.write_var_u32(self.material_reducers.len() as u32);
        for reducer in &self.material_reducers {
            encode_material_reducer(writer, reducer);
        }
        writer.write_bool(self.clear_recipes);
    }

    pub fn decode_payload(reader: &mut Reader) -> Result<Self, CodecError> {
        let mut recipes = Vec::new();
        let shaped_count = reader.read_var_u32()?;
        for _ in 0..shaped_count {
            recipes.push(CraftingRecipe::Shaped(decode_shaped(reader)?));
        }
        let shapeless_count = reader.read_var_u32()?;
        for _ in 0..shapeless_count {
            recipes.push(CraftingRecipe::Shapeless(decode_shapeless(reader)?));
        }
        let multi_count = reader.read_var_u32()?;
        for _ in 0..multi_count {
            recipes.push(CraftingRecipe::Multi(decode_multi(reader)?));
        }
        // User-data shapeless and chemistry lists use the plain
        // shaped/shapeless layouts; keep them in the flat list.
        for _ in 0..reader.read_var_u32()? {
            recipes.push(CraftingRecipe::Shapeless(decode_shapeless(reader)?));
        }
        for _ in 0..reader.read_var_u32()? {
            recipes.push(CraftingRecipe::Shapeless(decode_shapeless(reader)?));
        }
        for _ in 0..reader.read_var_u32()? {
            recipes.push(CraftingRecipe::Shaped(decode_shaped(reader)?));
        }
        for _ in 0..reader.read_var_u32()? {
            recipes.push(CraftingRecipe::SmithingTransform(decode_smithing_transform(
                reader,
            )?));
        }
        for _ in 0..reader.read_var_u32()? {
            recipes.push(CraftingRecipe::SmithingTrim(decode_smithing_trim(reader)?));
        }
        let potion_count = reader.read_var_u32()?;
        let mut potion_types = Vec::with_capacity(potion_count as usize);
        for _ in 0..potion_count {
            potion_types.push(PotionTypeRecipe {
                input_id: reader.read_var_i32()?,
                input_meta: reader.read_var_i32()?,
                ingredient_id: reader.read_var_i32()?,
                ingredient_meta: reader.read_var_i32()?,
                output_id: reader.read_var_i32()?,
                output_meta: reader.read_var_i32()?,
            });
        }
        let container_count = reader.read_var_u32()?;
        let mut potion_containers = Vec::with_capacity(container_count as usize);
        for _ in 0..container_count {
            potion_containers.push(PotionContainerRecipe {
                input_id: reader.read_var_i32()?,
                ingredient_id: reader.read_var_i32()?,
                output_id: reader.read_var_i32()?,
            });
        }
        let reducer_count = reader.read_var_u32()?;
        let mut material_reducers = Vec::with_capacity(reducer_count as usize);
        for _ in 0..reducer_count {
            material_reducers.push(decode_material_reducer(reader)?);
        }
        let clear_recipes = reader.read_bool()?;
        Ok(Self {
            clear_recipes,
            recipes,
            potion_types,
            potion_containers,
            material_reducers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(id: &str, meta: i32) -> RecipeDescriptor {
        RecipeDescriptor::StringIdMeta {
            id: id.into(),
            meta,
            count: 1,
        }
    }

    fn always_unlocked() -> Option<UnlockingRequirement> {
        Some(UnlockingRequirement::always_unlocked())
    }

    #[test]
    fn roundtrips_shaped_shapeless_and_potions() {
        let shaped = CraftingRecipe::Shaped(ShapedRecipe {
            recipe_id: "minecraft:oak_planks".into(),
            width: 1,
            height: 1,
            inputs: vec![descriptor("minecraft:oak_log", WILDCARD_META)],
            outputs: vec![CraftingItemStack::new(5, 4, 0)],
            uuid: [1; 16],
            block: "crafting_table".into(),
            priority: 50,
            assume_symmetry: true,
            unlocking: always_unlocked(),
            network_id: 1,
        });
        let shapeless = CraftingRecipe::Shapeless(ShapelessRecipe {
            recipe_id: "minecraft:stick".into(),
            inputs: vec![
                descriptor("minecraft:oak_planks", 0),
                descriptor("minecraft:oak_planks", 0),
            ],
            outputs: vec![CraftingItemStack::new(352, 4, 0)],
            uuid: [2; 16],
            block: "crafting_table".into(),
            priority: 50,
            unlocking: always_unlocked(),
            network_id: 2,
        });
        let furnace = CraftingRecipe::Shapeless(ShapelessRecipe {
            recipe_id: "minecraft:furnace_cobblestone".into(),
            inputs: vec![descriptor("minecraft:cobblestone", WILDCARD_META)],
            outputs: vec![CraftingItemStack::new(1, 1, 0)],
            uuid: [3; 16],
            block: "furnace".into(),
            priority: 50,
            unlocking: always_unlocked(),
            network_id: 3,
        });
        let packet = CraftingDataPacket::new(
            vec![shaped, shapeless, furnace],
            vec![PotionTypeRecipe {
                input_id: 459,
                input_meta: 4,
                ingredient_id: 462,
                ingredient_meta: 0,
                output_id: 459,
                output_meta: 31,
            }],
            vec![PotionContainerRecipe {
                input_id: 459,
                ingredient_id: 360,
                output_id: 600,
            }],
        );
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            CraftingDataPacket::decode_payload(&mut reader).unwrap(),
            packet
        );
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn empty_tag_and_wildcard_descriptors_roundtrip() {
        let packet = CraftingDataPacket::new(
            vec![CraftingRecipe::Shapeless(ShapelessRecipe {
                recipe_id: "minecraft:barrel".into(),
                inputs: vec![
                    RecipeDescriptor::Invalid { count: 0 },
                    RecipeDescriptor::ItemTag {
                        tag: "minecraft:wooden_slabs".into(),
                        meta: WILDCARD_META,
                        count: 1,
                    },
                ],
                outputs: vec![CraftingItemStack::new(606, 1, 15)],
                uuid: [4; 16],
                block: "crafting_table".into(),
                priority: -1,
                unlocking: Some(UnlockingRequirement::none(Vec::new())),
                network_id: 4,
            })],
            Vec::new(),
            Vec::new(),
        );
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            CraftingDataPacket::decode_payload(&mut reader).unwrap(),
            packet
        );
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn smithing_multi_and_reducers_roundtrip() {
        let packet = CraftingDataPacket {
            recipes: vec![
                CraftingRecipe::Multi(MultiRecipe {
                    uuid: [7; 16],
                    network_id: 9,
                }),
                CraftingRecipe::SmithingTransform(SmithingTransformRecipe {
                    recipe_id: "minecraft:smithing_netherite_axe".into(),
                    template: descriptor("minecraft:netherite_upgrade_smithing_template", WILDCARD_META),
                    input: descriptor("minecraft:diamond_axe", WILDCARD_META),
                    addition: descriptor("minecraft:netherite_ingot", WILDCARD_META),
                    output: CraftingItemStack::new(649, 1, 0),
                    block: "smithing_table".into(),
                    network_id: 10,
                }),
                CraftingRecipe::SmithingTrim(SmithingTrimRecipe {
                    recipe_id: "minecraft:smithing_armor_trim".into(),
                    template: RecipeDescriptor::ItemTag {
                        tag: "minecraft:trim_templates".into(),
                        meta: WILDCARD_META,
                        count: 1,
                    },
                    input: RecipeDescriptor::ItemTag {
                        tag: "minecraft:trimmable_armors".into(),
                        meta: WILDCARD_META,
                        count: 1,
                    },
                    addition: RecipeDescriptor::ItemTag {
                        tag: "minecraft:trim_materials".into(),
                        meta: WILDCARD_META,
                        count: 1,
                    },
                    block: "smithing_table".into(),
                    network_id: 11,
                }),
            ],
            potion_types: Vec::new(),
            potion_containers: Vec::new(),
            material_reducers: vec![MaterialReducerRecipe {
                input_id: 17,
                input_meta: 2,
                outputs: vec![MaterialReducerOutput {
                    item_id: 5,
                    count: 4,
                }],
            }],
            clear_recipes: true,
        };
        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        // Decode flattens in wire group order (multi, then smithing
        // transform, then smithing trim), which is the same order used
        // above.
        assert_eq!(
            CraftingDataPacket::decode_payload(&mut reader).unwrap(),
            packet
        );
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn rejects_shaped_count_mismatch() {
        let mut writer = Writer::new();
        // Wire order: shaped count=1, recipe id, width+height=2 but then a
        // bogus ingredient count of 3 before any descriptor.
        writer.write_var_u32(1);
        writer.write_string("minecraft:bad");
        writer.write_var_i32(1);
        writer.write_var_i32(2);
        writer.write_var_u32(3);
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes);
        assert!(CraftingDataPacket::decode_payload(&mut reader).is_err());
    }

    #[test]
    fn uuid_wire_is_two_reversed_halves() {
        let mut writer = Writer::new();
        let mut uuid = [0u8; 16];
        for (index, byte) in uuid.iter_mut().enumerate() {
            *byte = index as u8;
        }
        write_uuid(&mut writer, &uuid);
        let bytes = writer.into_bytes();
        assert_eq!(
            &bytes[..],
            [7, 6, 5, 4, 3, 2, 1, 0, 15, 14, 13, 12, 11, 10, 9, 8]
        );
        let mut reader = Reader::new(&bytes);
        assert_eq!(read_uuid(&mut reader).unwrap(), uuid);
    }
}
