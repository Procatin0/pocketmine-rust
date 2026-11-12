//! Crafting system: port of PocketMine's `src/crafting/` (Axolotl) to Rust.
//!
//! Covers `CraftingManager` (shaped/shapeless/furnace/brewing/potion-type
//! recipes), `RecipeIngredient` (`Exact` / `MetaWildcard` / `TagWildcard`),
//! `ShapedRecipe` / `ShapelessRecipe`, `CraftingGrid` (2x2 player crafting and
//! 3x3 crafting-table) and the ItemStackRequest engine (Take/Place/Swap/
//! Drop/Destroy/CreativeCreate/CraftRecipe).
//!
//! Recipes are loaded from the Bedrock recipe JSON in `data/recipes/`
//! (`shaped_crafting.json`, `shaped_crafting_asymmetric.json`,
//! `shapeless_crafting.json`, `potion_type.json`,
//! `potion_container_change.json`) plus `data/item_tags.json` for tags.
//! Item names are resolved through the classic numeric-id registry
//! (`inventory::item_network_id`): entries for data-driven items the registry
//! does not know are skipped (counted in `skipped_unknown`).
//!
//! `meta: 32767` in the JSON is PocketMine's `MetaWildcard` marker. Block
//! states (`block_states` base64) are not decoded: result stacks keep
//! `meta: 0` and `block_runtime_id: 0`, which matches how this server already
//! models block items in `ItemStack`.

use std::collections::{HashMap, HashSet};

use bedrock_protocol::ItemStack;
use serde_json::Value;

use crate::inventory::{
    item_network_id, max_stack_size, PlayerInventory, WINDOW_ID_ARMOR,
    WINDOW_ID_INVENTORY, WINDOW_ID_OFFHAND,
};

/// PocketMine `CraftingGrid::SIZE_SMALL` (player 2x2 crafting).
pub const SIZE_SMALL: usize = 2;
/// PocketMine `CraftingGrid::SIZE_BIG` (crafting table 3x3).
pub const SIZE_BIG: usize = 3;
/// `meta: 32767` marks a `MetaWildcard` ingredient in Bedrock recipe JSON.
pub const META_WILDCARD: u32 = 32767;

/// PocketMine `CraftingDataCache::RECIPE_ID_OFFSET`: the client rejects
/// recipes with network id 0, so every wire recipe id is `index + 1` and the
/// `ItemStackRequestExecutor::beginCrafting` lookup is
/// `recipeIndex = recipeId - RECIPE_ID_OFFSET`.
pub const RECIPE_ID_OFFSET: u32 = 1;
/// PocketMine `ItemStackRequestExecutor::beginCrafting`: repetitions must be
/// in `1..=256` (a `Byte` on the wire is hard-limited to 256).
pub const MAX_CRAFT_REPETITIONS: u32 = 256;

/// One crafting ingredient, port of `RecipeIngredient`:
/// `Exact` matches one id + meta, `MetaWildcard` one id with any meta and
/// `TagWildcard` any item in a tag (`data/item_tags.json`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeIngredient {
    Exact { network_id: i16, meta: u32 },
    MetaWildcard { network_id: i16 },
    TagWildcard { tag: String },
}

impl RecipeIngredient {
    pub fn matches(&self, item: &ItemStack, tags: &HashMap<String, HashSet<i16>>) -> bool {
        if item.is_empty() {
            return false;
        }
        match self {
            Self::Exact { network_id, meta } => {
                item.network_id == *network_id && item.meta == *meta
            }
            Self::MetaWildcard { network_id } => item.network_id == *network_id,
            Self::TagWildcard { tag } => tags
                .get(tag)
                .map(|ids| ids.contains(&item.network_id))
                .unwrap_or(false),
        }
    }
}

/// Shaped recipe: grid rows as `A`/`B`.. letters or `None` for empty cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapedRecipe {
    pub id: String,
    pub shape: Vec<Vec<Option<RecipeIngredient>>>,
    pub result: ItemStack,
    pub priority: i32,
}

/// Shapeless recipe: the multiset of ingredients, order irrelevant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapelessRecipe {
    pub id: String,
    pub ingredients: Vec<RecipeIngredient>,
    pub result: ItemStack,
    pub priority: i32,
}

/// Furnace (smelting) recipe, from `shapeless_crafting.json` with
/// `block: furnace|blast_furnace|smoker|campfire`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FurnaceRecipe {
    pub id: String,
    pub block: String,
    pub input: RecipeIngredient,
    pub output: ItemStack,
}

/// Brewing recipe (`PotionTypeRecipe`): `input` potion + `ingredient` -> `output`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrewingRecipe {
    pub input: RecipeIngredient,
    pub ingredient: RecipeIngredient,
    pub output: ItemStack,
}

/// One recipe resolved by wire network id (see
/// [`CraftingManager::recipe_by_network_id`]): the shared input list lets
/// the `CraftRecipe` engine consume ingredients without re-deciding the
/// recipe kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CraftingRecipeKind {
    Shaped(ShapedRecipe),
    Shapeless(ShapelessRecipe),
    Furnace(FurnaceRecipe),
}

impl CraftingRecipeKind {
    pub fn result(&self) -> ItemStack {
        match self {
            Self::Shaped(recipe) => recipe.result.clone(),
            Self::Shapeless(recipe) => recipe.result.clone(),
            Self::Furnace(recipe) => recipe.output.clone(),
        }
    }

    /// Every ingredient the recipe consumes once per repetition.
    pub fn ingredients(&self) -> Vec<RecipeIngredient> {
        match self {
            Self::Shaped(recipe) => recipe
                .shape
                .iter()
                .flatten()
                .filter_map(|cell| cell.clone())
                .collect(),
            Self::Shapeless(recipe) => recipe.ingredients.clone(),
            Self::Furnace(recipe) => vec![recipe.input.clone()],
        }
    }
}

/// Result of a craft attempt: the matched recipe's output plus the number of
/// input items consumed from every non-empty grid slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftOutcome {
    pub result: ItemStack,
    pub consume: u16,
}

/// Crafting grid of PocketMine: `SIZE_SMALL` (2x2) or `SIZE_BIG` (3x3),
/// row-major slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftingGrid {
    pub size: usize,
    pub slots: Vec<ItemStack>,
}

impl CraftingGrid {
    pub fn new(size: usize) -> Self {
        Self { size, slots: vec![ItemStack::empty(); size * size] }
    }

    pub fn small() -> Self {
        Self::new(SIZE_SMALL)
    }

    pub fn big() -> Self {
        Self::new(SIZE_BIG)
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(ItemStack::is_empty)
    }

    /// Bounding box of the non-empty slots (PocketMine trims empty borders so
    /// a recipe can be placed anywhere in the grid).
    fn crop_bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let mut min_x = self.size;
        let mut min_y = self.size;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        let mut any = false;
        for (index, slot) in self.slots.iter().enumerate() {
            if slot.is_empty() {
                continue;
            }
            any = true;
            let x = index % self.size;
            let y = index / self.size;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        any.then_some((min_x, min_y, max_x, max_y))
    }

    pub fn matches_shaped(
        &self,
        recipe: &ShapedRecipe,
        tags: &HashMap<String, HashSet<i16>>,
    ) -> Option<CraftOutcome> {
        if recipe.shape.is_empty() || recipe.shape.iter().all(|row| row.iter().all(|c| c.is_none())) {
            return None;
        }
        let (min_x, min_y, max_x, max_y) = self.crop_bounds()?;
        let (width, height) = (max_x - min_x + 1, max_y - min_y + 1);
        let shape_h = recipe.shape.len();
        let shape_w = recipe.shape.first().map(|row| row.len()).unwrap_or(0);
        if width != shape_w || height != shape_h {
            return None;
        }
        // PocketMine `ShapedRecipe::matchesCraftingGrid` accepts both the
        // direct orientation and the horizontally mirrored one (an asymmetric
        // shape placed left-to-right or right-to-left is the same craft).
        if !self.match_shaped_cells(recipe, tags, min_x, min_y, false)
            && !self.match_shaped_cells(recipe, tags, min_x, min_y, true)
        {
            return None;
        }
        // Cropping guarantees no stray items outside the box; count consumed
        // from the 1-per-slot rule PocketMine crafting uses.
        let consume = self.slots.iter().filter(|s| !s.is_empty()).count() as u16;
        Some(CraftOutcome { result: recipe.result.clone(), consume })
    }

    /// PocketMine `ShapedRecipe::matchInputMap`: a `None` recipe cell (space)
    /// must be empty in the (cropped) grid, otherwise the recipe ingredient
    /// must `accepts` the grid item. `reverse` mirrors columns, exactly like
    /// `$grid->getIngredient($reverse ? $width - $x - 1 : $x, $y)`.
    fn match_shaped_cells(
        &self,
        recipe: &ShapedRecipe,
        tags: &HashMap<String, HashSet<i16>>,
        min_x: usize,
        min_y: usize,
        reverse: bool,
    ) -> bool {
        let width = recipe.shape.first().map(|row| row.len()).unwrap_or(0);
        for (row, expected_row) in recipe.shape.iter().enumerate() {
            for (col, expected) in expected_row.iter().enumerate() {
                let grid_col = if reverse { width - col - 1 } else { col };
                let grid_item = &self.slots[(min_y + row) * self.size + (min_x + grid_col)];
                match expected {
                    None => {
                        if !grid_item.is_empty() {
                            return false;
                        }
                    }
                    Some(ingredient) => {
                        if !ingredient.matches(grid_item, tags) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    pub fn matches_shapeless(
        &self,
        recipe: &ShapelessRecipe,
        tags: &HashMap<String, HashSet<i16>>,
    ) -> Option<CraftOutcome> {
        let items: Vec<&ItemStack> =
            self.slots.iter().filter(|slot| !slot.is_empty()).collect();
        if items.is_empty() || items.len() != recipe.ingredients.len() {
            return None;
        }
        let mut remaining: Vec<bool> = vec![false; items.len()];
        for ingredient in &recipe.ingredients {
            let mut matched = None;
            for (index, candidate) in items.iter().enumerate() {
                if remaining[index] {
                    continue;
                }
                if ingredient.matches(candidate, tags) {
                    matched = Some(index);
                    break;
                }
            }
            match matched {
                Some(index) => remaining[index] = true,
                None => return None,
            }
        }
        Some(CraftOutcome {
            result: recipe.result.clone(),
            consume: items.len() as u16,
        })
    }
}

/// Registry of every craftable recipe, built once from `data/recipes/`.
#[derive(Debug, Clone, Default)]
pub struct CraftingManager {
    pub shaped: Vec<ShapedRecipe>,
    pub shapeless: Vec<ShapelessRecipe>,
    pub furnace: Vec<FurnaceRecipe>,
    pub brewing: Vec<BrewingRecipe>,
    pub potion_container: Vec<BrewingRecipe>,
    pub tags: HashMap<String, HashSet<i16>>,
    /// Recipes skipped because an item name is data-driven and has no
    /// numeric network id in this protocol.
    pub skipped_unknown: usize,
    /// Numeric network id -> canonical item string id, recorded for every
    /// ingredient/output name resolved from the recipe JSON in `build`.
    /// `CraftingDataPacket` descriptors are string based
    /// (`StringIdMetaItemDescriptor`), so the wire builder uses this map
    /// (mirrors Axolotl's `ItemTypeDictionary::fromIntId`).
    pub item_names: HashMap<i16, String>,
    /// All entries, including crafting-table ones, for `CraftingDataPacket`.
    pub total_entries: usize,
}

fn parse_ingredient(
    value: &Value,
    names: &mut HashMap<i16, String>,
) -> Option<RecipeIngredient> {
    if let Some(tag) = value.get("tag").and_then(Value::as_str) {
        return Some(RecipeIngredient::TagWildcard { tag: tag.to_string() });
    }
    let name = value.get("name").and_then(Value::as_str)?;
    let network_id = item_network_id(name)?;
    names.entry(network_id).or_insert_with(|| name.to_string());
    let meta = value.get("meta").and_then(Value::as_u64).unwrap_or(0) as u32;
    if meta == META_WILDCARD {
        Some(RecipeIngredient::MetaWildcard { network_id })
    } else {
        Some(RecipeIngredient::Exact { network_id, meta })
    }
}

/// Parses one recipe/creative JSON entry into an `ItemStack`. `block_states`
/// (base64 NBT) and `nbt` fields are ignored: the stack keeps `meta` as given
/// and `block_runtime_id: 0`, which for creative emission is exactly why the
/// `generated` 0x91 wire is disabled (see `creative_items`).
fn parse_item_stack(
    value: &Value,
    names: &mut HashMap<i16, String>,
) -> Option<ItemStack> {
    let name = value.as_str().or_else(|| value.get("name").and_then(Value::as_str))?;
    let network_id = item_network_id(name)?;
    names.entry(network_id).or_insert_with(|| name.to_string());
    let count = value
        .get("count")
        .and_then(Value::as_u64)
        .unwrap_or(1) as u16;
    let meta = value.get("meta").and_then(Value::as_u64).unwrap_or(0) as u32;
    Some(ItemStack { network_id, count, meta, ..ItemStack::empty() })
}

/// Wire recipe id for one loaded entry. The `#<index>` suffix is kept so
/// `CraftingManager::recipe_by_id` can map a client craft request onto one
/// concrete entry. (Axolotl uses random UUIDs as wire recipe ids instead;
/// its network-id offset of 1 is applied by the wire builder in
/// `session.rs`, matching `CraftingDataCache::RECIPE_ID_OFFSET`.)
fn recipe_id(name: &str, index: usize) -> String {
    format!("{name}#{index}")
}

impl CraftingManager {
    /// Builds the manager from the embedded Bedrock recipe JSON.
    pub fn build() -> Self {
        let mut manager = Self::default();

        // Item tags (`data/item_tags.json`): name (without `minecraft:` in the
        // JSON keys of this capture) -> network ids.
        let tags: Value = serde_json::from_str(include_str!("../../../data/item_tags.json"))
            .unwrap_or(Value::Array(Vec::new()));
        let mut raw_tags: HashMap<String, Vec<String>> = HashMap::new();
        if let Some(map) = tags.as_object() {
            for (key, values) in map {
                let tag = if let Some(stripped) = key.strip_prefix("minecraft:") {
                    format!("minecraft:{stripped}")
                } else {
                    key.clone()
                };
                let names: Vec<String> = values
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                raw_tags.insert(tag, names);
            }
        }
        for (tag, names) in &raw_tags {
            let ids: HashSet<i16> = names.iter().filter_map(|n| item_network_id(n)).collect();
            manager.tags.insert(tag.clone(), ids);
        }

        fn load_shaped(
            manager: &mut CraftingManager,
            source: &str,
            offset: usize,
        ) {
            let entries: Vec<Value> =
                serde_json::from_str(source).unwrap_or_default();
            for (index, entry) in entries.into_iter().enumerate() {
                let block = entry.get("block").and_then(Value::as_str).unwrap_or("");
                if block != "crafting_table" {
                    continue;
                }
                manager.total_entries += 1;
                let priority = entry.get("priority").and_then(Value::as_i64).unwrap_or(0) as i32;
                let result_value = entry.get("output").and_then(|v| v.as_array()).and_then(|a| a.first());
                let Some(result) = result_value
                    .and_then(|v| parse_item_stack(v, &mut manager.item_names))
                else {
                    manager.skipped_unknown += 1;
                    continue;
                };
                let inputs = entry.get("input").and_then(Value::as_object).cloned().unwrap_or_default();
                let shape_rows: Vec<&str> = entry
                    .get("shape")
                    .and_then(Value::as_array)
                    .map(|rows| rows.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();
                let mut shape: Vec<Vec<Option<RecipeIngredient>>> = Vec::new();
                let mut unknown = false;
                // Trim trailing empty borders exactly like the JSON does:
                // rows/cols that are all spaces are dropped.
                let width = shape_rows.iter().map(|row| row.chars().count()).max().unwrap_or(0);
                let grid: Vec<Vec<char>> = shape_rows
                    .iter()
                    .map(|row| {
                        let mut chars: Vec<char> = row.chars().collect();
                        chars.resize(width, ' ');
                        chars
                    })
                    .collect();
                let mut min_row = grid.len();
                let mut max_row = 0usize;
                let mut min_col = width;
                let mut max_col = 0usize;
                let mut any = false;
                for (row, line) in grid.iter().enumerate() {
                    for (col, ch) in line.iter().enumerate() {
                        if *ch != ' ' {
                            any = true;
                            min_row = min_row.min(row);
                            max_row = max_row.max(row);
                            min_col = min_col.min(col);
                            max_col = max_col.max(col);
                        }
                    }
                }
                if !any {
                    manager.skipped_unknown += 1;
                    continue;
                }
                for row in min_row..=max_row {
                    let mut out_row = Vec::new();
                    for col in min_col..=max_col {
                        let ch = grid[row][col];
                        if ch == ' ' {
                            out_row.push(None);
                        } else {
                            let key = ch.to_string();
                            let Some(parsed) = inputs
                                .get(&key)
                                .and_then(|v| parse_ingredient(v, &mut manager.item_names))
                            else {
                                unknown = true;
                                break;
                            };
                            out_row.push(Some(parsed));
                        }
                    }
                    if unknown {
                        break;
                    }
                    shape.push(out_row);
                }
                if unknown {
                    manager.skipped_unknown += 1;
                    continue;
                }
                let name = format!(
                    "minecraft:{}",
                    result_name(&entry)
                );
                manager.shaped.push(ShapedRecipe {
                    id: recipe_id(&name, offset + index),
                    shape,
                    result,
                    priority,
                });
            }
        }

        load_shaped(&mut manager, include_str!("../../../data/recipes/shaped_crafting.json"), 0);
        load_shaped(
            &mut manager,
            include_str!("../../../data/recipes/shaped_crafting_asymmetric.json"),
            100_000,
        );

        // Shapeless + furnace: `shapeless_crafting.json`.
        let entries: Vec<Value> = serde_json::from_str(
            include_str!("../../../data/recipes/shapeless_crafting.json"),
        )
        .unwrap_or_default();
        for (index, entry) in entries.into_iter().enumerate() {
            let block = entry.get("block").and_then(Value::as_str).unwrap_or("");
            match block {
                "crafting_table" => {
                    manager.total_entries += 1;
                    let result_value = entry
                        .get("output")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first());
                    let Some(result) = result_value
                        .and_then(|v| parse_item_stack(v, &mut manager.item_names))
                    else {
                        manager.skipped_unknown += 1;
                        continue;
                    };
                    let Some(inputs) = entry.get("input").and_then(Value::as_array) else {
                        manager.skipped_unknown += 1;
                        continue;
                    };
                    let mut ingredients = Vec::new();
                    let mut unknown = false;
                    for input in inputs {
                        match parse_ingredient(input, &mut manager.item_names) {
                            Some(parsed) => ingredients.push(parsed),
                            None => {
                                unknown = true;
                                break;
                            }
                        }
                    }
                    if unknown {
                        manager.skipped_unknown += 1;
                        continue;
                    }
                    let priority = entry.get("priority").and_then(Value::as_i64).unwrap_or(0) as i32;
                    manager.shapeless.push(ShapelessRecipe {
                        id: recipe_id(&result_name(&entry), index),
                        ingredients,
                        result,
                        priority,
                    });
                }
                "furnace" | "blast_furnace" | "smoker" | "campfire" | "soul_campfire" => {
                    manager.total_entries += 1;
                    let Some(input) = entry
                        .get("input")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                        .and_then(|v| parse_ingredient(v, &mut manager.item_names))
                    else {
                        manager.skipped_unknown += 1;
                        continue;
                    };
                    let Some(output) = entry
                        .get("output")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                        .and_then(|v| parse_item_stack(v, &mut manager.item_names))
                    else {
                        manager.skipped_unknown += 1;
                        continue;
                    };
                    manager.furnace.push(FurnaceRecipe {
                        id: recipe_id(
                            &format!("furnace_{}", result_name(&entry)),
                            index,
                        ),
                        block: block.to_string(),
                        input,
                        output,
                    });
                }
                _ => {}
            }
        }

        // Brewing (`potion_type.json`) and potion container changes.
        let brewing: Vec<Value> = serde_json::from_str(
            include_str!("../../../data/recipes/potion_type.json"),
        )
        .unwrap_or_default();
        for entry in brewing.into_iter() {
            manager.total_entries += 1;
            if let (Some(input), Some(ingredient), Some(output)) = (
                entry
                    .get("input")
                    .and_then(|v| parse_ingredient(v, &mut manager.item_names)),
                entry
                    .get("ingredient")
                    .and_then(|v| parse_ingredient(v, &mut manager.item_names)),
                entry
                    .get("output")
                    .and_then(|v| parse_item_stack(v, &mut manager.item_names)),
            ) {
                manager.brewing.push(BrewingRecipe { input, ingredient, output });
            } else {
                manager.skipped_unknown += 1;
            }
        }
        let container: Vec<Value> = serde_json::from_str(
            include_str!("../../../data/recipes/potion_container_change.json"),
        )
        .unwrap_or_default();
        for entry in container.into_iter() {
            // `input_item_name`/`output_item_name` are plain names without meta.
            // PocketMine `PotionContainerChangeRecipe` matches the input by
            // item *type* only (meta is carried into the output), so the
            // stored input is a `MetaWildcard`, not an `Exact` meta-0 match
            // that would reject every real potion (meta != 0).
            let stack_of = |key: &str| -> Option<ItemStack> {
                let name = entry.get(key).and_then(Value::as_str)?;
                let network_id = item_network_id(name)?;
                Some(ItemStack { network_id, count: 1, meta: 0, ..ItemStack::empty() })
            };
            let input_name = entry.get("input_item_name").and_then(Value::as_str).unwrap_or("");
            let input = item_network_id(input_name).map(|network_id| RecipeIngredient::MetaWildcard {
                network_id,
            });
            let ingredient_name = entry
                .get("ingredient")
                .and_then(|v| {
                    v.as_str()
                        .or_else(|| v.get("name").and_then(Value::as_str))
                })
                .unwrap_or("");
            let ingredient = crate::crafting::ingredient_from_name(ingredient_name);
            if let (Some(input), Some(ingredient), Some(output)) = (
                input,
                ingredient,
                stack_of("output_item_name"),
            ) {
                manager.potion_container.push(BrewingRecipe { input, ingredient, output });
            } else {
                manager.skipped_unknown += 1;
            }
        }

        // PocketMine sorts by priority descending, so a specific recipe wins
        // over a generic one (e.g. a named tool over a tag fallback). These
        // priorities are server-side only: on the wire Axolotl's
        // `CraftingDataCache::buildCraftingDataCache` always sends the
        // constant 50 for shaped/shapeless (incl. furnace) recipes, and the
        // wire builder in `session.rs` mirrors that.
        manager.shaped.sort_by_key(|r| std::cmp::Reverse(r.priority));
        manager.shapeless.sort_by_key(|r| std::cmp::Reverse(r.priority));
        manager
    }

    /// Finds a shaped recipe matching a grid of `size` (2 or 3).
    pub fn find_shaped(&self, grid: &CraftingGrid) -> Option<&ShapedRecipe> {
        self.shaped.iter().find(|recipe| grid.matches_shaped(recipe, &self.tags).is_some())
    }

    /// Finds a shapeless recipe matching a grid of `size` (2 or 3).
    pub fn find_shapeless(&self, grid: &CraftingGrid) -> Option<&ShapelessRecipe> {
        self.shapeless
            .iter()
            .find(|recipe| grid.matches_shapeless(recipe, &self.tags).is_some())
    }

    /// Crafts from a grid: shaped wins over shapeless, like BDS.
    pub fn craft_from_grid(&self, grid: &CraftingGrid) -> Option<CraftOutcome> {
        if let Some(recipe) = self.find_shaped(grid) {
            return grid.matches_shaped(recipe, &self.tags);
        }
        if let Some(recipe) = self.find_shapeless(grid) {
            return grid.matches_shapeless(recipe, &self.tags);
        }
        None
    }

    /// Finds a furnace recipe for one input item in the `furnace` block.
    /// PocketMine keeps one manager per `FurnaceType`, so a smoker-only
    /// input must not smelt in a plain furnace; `find_furnace_for` with the
    /// block name is the precise lookup. This helper prefers the plain
    /// `furnace` block and only falls back to any matching block so old
    /// callers that smelt in any furnace keep working.
    pub fn find_furnace(&self, item: &ItemStack) -> Option<&FurnaceRecipe> {
        self.find_furnace_for("furnace", item).or_else(|| {
            self.furnace.iter().find(|recipe| recipe.input.matches(item, &self.tags))
        })
    }

    /// Finds a furnace recipe for one input item, restricted to one block
    /// (`furnace` / `blast_furnace` / `smoker` / `campfire` /
    /// `soul_campfire`), like PocketMine's per-type
    /// `FurnaceRecipeManager::match`.
    pub fn find_furnace_for(&self, block: &str, item: &ItemStack) -> Option<&FurnaceRecipe> {
        self.furnace
            .iter()
            .filter(|recipe| recipe.block == block)
            .find(|recipe| recipe.input.matches(item, &self.tags))
    }

    /// Cook duration in ticks of one furnace block, from
    /// PocketMine `FurnaceType` metadata (FURNACE 200, BLAST_FURNACE 100,
    /// SMOKER 100, CAMPFIRE/SOUL_CAMPFIRE 600).
    pub fn furnace_cook_ticks(block: &str) -> u32 {
        match block {
            "furnace" => 200,
            "blast_furnace" | "smoker" => 100,
            "campfire" | "soul_campfire" => 600,
            _ => 200,
        }
    }

    /// Finds a brewing recipe for a potion + ingredient pair (potion-type
    /// recipes only; see [`CraftingManager::brewing_result`] for the
    /// container-change priority PocketMine uses).
    pub fn find_brewing(&self, input: &ItemStack, ingredient: &ItemStack) -> Option<&BrewingRecipe> {
        self.brewing.iter().find(|recipe| {
            recipe.input.matches(input, &self.tags)
                && recipe.ingredient.matches(ingredient, &self.tags)
        })
    }

    /// Brewing result for a potion + ingredient pair, in PocketMine
    /// `CraftingManager::matchBrewingRecipe` order: container-change
    /// recipes first, then potion-type recipes. A container-change recipe
    /// keeps the input potion's meta (`getResultFor` re-serializes the input
    /// type with its meta), so the output meta is the input meta.
    pub fn brewing_result(&self, input: &ItemStack, ingredient: &ItemStack) -> Option<ItemStack> {
        for recipe in &self.potion_container {
            if recipe.ingredient.matches(ingredient, &self.tags)
                && recipe.input.matches(input, &self.tags)
            {
                let mut output = recipe.output.clone();
                output.meta = input.meta;
                return Some(output);
            }
        }
        self.brewing.iter().find_map(|recipe| {
            if recipe.input.matches(input, &self.tags)
                && recipe.ingredient.matches(ingredient, &self.tags)
            {
                Some(recipe.output.clone())
            } else {
                None
            }
        })
    }

    /// Finds a recipe by its wire network id, the exact lookup
    /// PocketMine's `beginCrafting` performs (`recipeId - RECIPE_ID_OFFSET`
    /// into `getCraftingRecipeIndex`). This server serializes the recipe
    /// book as shaped, then shapeless, then furnace recipes (see
    /// `PlayerSession::generated_crafting_data_wire`), so ids are assigned
    /// in that order starting at 1. PocketMine instead registers shapeless
    /// recipes before shaped ones; its ids therefore differ from these, but
    /// relative `id - 1` indexing and the furnace `recipeNetId++` tail match.
    pub fn recipe_by_network_id(&self, network_id: u32) -> Option<CraftingRecipeKind> {
        let index = (network_id.checked_sub(RECIPE_ID_OFFSET)?) as usize;
        let shaped_len = self.shaped.len();
        if index < shaped_len {
            return Some(CraftingRecipeKind::Shaped(self.shaped[index].clone()));
        }
        let shapeless_index = index - shaped_len;
        if shapeless_index < self.shapeless.len() {
            return Some(CraftingRecipeKind::Shapeless(self.shapeless[shapeless_index].clone()));
        }
        let furnace_index = shapeless_index - self.shapeless.len();
        if furnace_index < self.furnace.len() {
            return Some(CraftingRecipeKind::Furnace(self.furnace[furnace_index].clone()));
        }
        None
    }

    /// Convenience: is this network id a known crafting recipe?
    pub fn has_network_id(&self, network_id: u32) -> bool {
        self.recipe_by_network_id(network_id).is_some()
    }

    /// Finds a recipe by id (used by `CraftRecipe` requests) or by its
    /// result name (the client sends the result identifier).
    pub fn recipe_by_id(&self, id: &str) -> Option<CraftOutcome> {
        let wanted = id.strip_prefix("minecraft:").unwrap_or(id).to_string();
        for recipe in &self.shaped {
            if recipe.id == id
                || recipe.id.strip_prefix(&format!("minecraft:{wanted}#")).is_some()
            {
                return Some(CraftOutcome {
                    result: recipe.result.clone(),
                    consume: count_ingredients(&recipe.shape),
                });
            }
        }
        for recipe in &self.shapeless {
            if recipe.id == id
                || recipe.id.strip_prefix(&format!("minecraft:{wanted}#")).is_some()
            {
                return Some(CraftOutcome {
                    result: recipe.result.clone(),
                    consume: 0, // supplied by the caller from the ingredient list
                });
            }
        }
        None
    }

    /// Total recipes loaded (skipped data-driven entries excluded).
    pub fn recipe_count(&self) -> usize {
        self.shaped.len() + self.shapeless.len() + self.furnace.len()
    }
}

fn count_ingredients(shape: &[Vec<Option<RecipeIngredient>>]) -> u16 {
    shape.iter().flatten().filter(|cell| cell.is_some()).count() as u16
}

/// Result recipe name used as the first part of the recipe id.
fn result_name(entry: &Value) -> String {
    let output = entry.get("output").and_then(|v| v.as_array()).and_then(|a| a.first());
    let name = output
        .and_then(|v| v.as_str().or_else(|| v.get("name").and_then(Value::as_str)))
        .unwrap_or("minecraft:unknown");
    name.strip_prefix("minecraft:").unwrap_or(name).to_string()
}

/// Parses a bare `minecraft:name` / `name` into an exact ingredient.
pub fn ingredient_from_name(name: &str) -> Option<RecipeIngredient> {
    let canonical = if name.starts_with("minecraft:") {
        name.to_string()
    } else {
        format!("minecraft:{name}")
    };
    let network_id = item_network_id(&canonical)?;
    Some(RecipeIngredient::Exact { network_id, meta: 0 })
}

/// Shared manager, built lazily on first use.
pub fn manager() -> &'static CraftingManager {
    static MANAGER: std::sync::OnceLock<CraftingManager> = std::sync::OnceLock::new();
    MANAGER.get_or_init(CraftingManager::build)
}

/// Crafting inventory for one grid size: `size*size` input slots plus a result
/// slot, port of PocketMine `CraftingInventory` (crafting table = 3x3).
#[derive(Debug, Clone)]
pub struct CraftingInventory {
    pub grid: CraftingGrid,
    pub result: ItemStack,
}

impl CraftingInventory {
    pub fn new(size: usize) -> Self {
        Self { grid: CraftingGrid::new(size), result: ItemStack::empty() }
    }

    pub fn small() -> Self {
        Self::new(SIZE_SMALL)
    }

    pub fn big() -> Self {
        Self::new(SIZE_BIG)
    }

    pub fn set_input(&mut self, slot: usize, item: ItemStack) -> bool {
        let Some(target) = self.grid.slots.get_mut(slot) else {
            return false;
        };
        *target = item;
        true
    }

    /// Recomputes the result slot from the current inputs.
    pub fn update_result(&mut self) {
        self.result = manager()
            .craft_from_grid(&self.grid)
            .map(|outcome| outcome.result)
            .unwrap_or_else(ItemStack::empty);
    }

    /// Takes one result stack (shift-click parity): consumes one item from
    /// every non-empty input slot and returns the crafted output.
    pub fn take_result(&mut self) -> Option<ItemStack> {
        self.update_result();
        if self.result.is_empty() {
            return None;
        }
        let result = self.result.clone();
        for slot in self.grid.slots.iter_mut() {
            if slot.is_empty() {
                continue;
            }
            slot.count -= 1;
            if slot.count == 0 {
                *slot = ItemStack::empty();
            }
        }
        self.update_result();
        Some(result)
    }

    /// Returns every input item, for closing the container.
    pub fn close_drops(&mut self) -> Vec<ItemStack> {
        let drops: Vec<ItemStack> = self
            .grid
            .slots
            .iter()
            .filter(|slot| !slot.is_empty())
            .cloned()
            .collect();
        for slot in self.grid.slots.iter_mut() {
            *slot = ItemStack::empty();
        }
        self.result = ItemStack::empty();
        drops
    }
}

/// The 2x2 crafting grid every player carries (`PlayerCraftingInventory`).
#[derive(Debug, Clone)]
pub struct PlayerCraftingInventory {
    pub inventory: CraftingInventory,
}

impl Default for PlayerCraftingInventory {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerCraftingInventory {
    pub fn new() -> Self {
        Self { inventory: CraftingInventory::small() }
    }

    pub fn update_result(&mut self) {
        self.inventory.update_result();
    }

    pub fn take_result(&mut self) -> Option<ItemStack> {
        self.inventory.take_result()
    }
}

/// The 3x3 crafting-table inventory (`CraftingTable` block inventory).
#[derive(Debug, Clone)]
pub struct CraftingTableInventory {
    pub inventory: CraftingInventory,
}

impl Default for CraftingTableInventory {
    fn default() -> Self {
        Self::new()
    }
}

impl CraftingTableInventory {
    pub fn new() -> Self {
        Self { inventory: CraftingInventory::big() }
    }

    pub fn update_result(&mut self) {
        self.inventory.update_result();
    }

    pub fn take_result(&mut self) -> Option<ItemStack> {
        self.inventory.take_result()
    }

    pub fn close_drops(&mut self) -> Vec<ItemStack> {
        self.inventory.close_drops()
    }
}

/// Creator-selected items for `CreativeContentPacket` (0x91), from
/// `data/creative/*.json`. This list feeds the disabled `generated` wire
/// only (`PlayerSession::creative_content_wire` sends the BDS capture
/// unless `PM_CORE_CREATIVE_MODE=generated`): the capture stays default
/// because this function cannot be faithful — 787 of the 1875 captured
/// items use negative (data-driven) ids absent from `item_ids.txt` and are
/// dropped here (only ~53% of the 2233 JSON entries resolve), `block_states`
/// (base64 NBT) are ignored so block stacks go out with runtime hash 0
/// instead of the BDS FNV-1a hash (`Invalid enum value` on real clients),
/// and `ItemStackExtraData` extras (NBT, 176 captured entries) are never
/// emitted. The `count = 1` set here matches the capture; widening it to
/// `max_stack_size` (64) previously diverged from every captured entry.
pub fn creative_items() -> Vec<ItemStack> {
    const FILES: &[&str] = &[
        include_str!("../../../data/creative/items.json"),
        include_str!("../../../data/creative/construction.json"),
        include_str!("../../../data/creative/equipment.json"),
        include_str!("../../../data/creative/nature.json"),
    ];
    let mut items = Vec::new();
    let mut seen: HashSet<(i16, u32)> = HashSet::new();
    // `parse_item_stack` records canonical names as a side effect; the
    // creative list does not need them, so this map is thrown away.
    let mut names: HashMap<i16, String> = HashMap::new();
    for source in FILES {
        let groups: Vec<Value> = serde_json::from_str(source).unwrap_or_default();
        for group in groups {
            let Some(list) = group.get("items").and_then(Value::as_array) else {
                continue;
            };
            for entry in list {
                // Entries are either "minecraft:name" or {"name": .., "meta": ..}.
                let entry_value = if let Some(name) = entry.as_str() {
                    serde_json::json!({ "name": name })
                } else {
                    entry.clone()
                };
                if let Some(mut stack) = parse_item_stack(&entry_value, &mut names) {
                    if !seen.insert((stack.network_id, stack.meta)) {
                        continue;
                    }
                    stack.count = 1;
                    items.push(stack);
                }
            }
        }
    }
    items
}

/// Creative groups for `CreativeContentPacket` (0x91): the same JSON files
/// grouped by `group_name`. Disabled together with `creative_items` (see its
/// doc): a null `group_icon` stays `ItemStack::empty()` (the null icon of
/// the 36 anonymous groups in the capture) instead of falling back to
/// `minecraft:stick`, group ids also have no source here (BDS: 123 groups,
/// not the 134 this JSON contains), and the category id of every group is
/// not modelled by `CreativeGroup`.
pub fn creative_groups() -> Vec<bedrock_protocol::packets::creative_content::CreativeGroup> {
    use bedrock_protocol::packets::creative_content::CreativeGroup;
    const FILES: &[&str] = &[
        include_str!("../../../data/creative/items.json"),
        include_str!("../../../data/creative/construction.json"),
        include_str!("../../../data/creative/equipment.json"),
        include_str!("../../../data/creative/nature.json"),
    ];
    let mut groups = Vec::new();
    for source in FILES {
        let parsed: Vec<Value> = serde_json::from_str(source).unwrap_or_default();
        for group in parsed {
            let name = group.get("group_name").and_then(Value::as_str).unwrap_or("").to_string();
            let icon_name = group.get("group_icon").and_then(Value::as_str);
            let icon = icon_name
                .and_then(crate::crafting::ingredient_from_name)
                .and_then(|ingredient| match ingredient {
                    RecipeIngredient::Exact { network_id, meta } => Some(ItemStack {
                        network_id,
                        count: 1,
                        meta,
                        ..ItemStack::empty()
                    }),
                    _ => None,
                })
                .unwrap_or_else(ItemStack::empty);
            groups.push(CreativeGroup { name, icon });
        }
    }
    groups
}

// ---------------------------------------------------------------------------
// ItemStackRequest engine (0x93 -> 0x94)
// ---------------------------------------------------------------------------

pub use bedrock_protocol::packets::item_stack_request::{
    ItemStackContainerSlot, ItemStackRequestAction, ItemStackRequestPacket,
    ACTION_CRAFT_RECIPE, ACTION_CREATIVE_CREATE, ACTION_DESTROY, ACTION_DROP,
    ACTION_PLACE, ACTION_SWAP, ACTION_TAKE,
};
pub use bedrock_protocol::packets::item_stack_response::{
    ItemStackResponseEntry, ItemStackResponsePacket, ItemStackResponseSlot, STATUS_ERROR,
    STATUS_OK,
};

/// Containers the server models, by `ItemStackContainerSlot.container`.
/// The ids are the window ids BDS uses (0 = inventory, 119 = offhand,
/// 120 = armor, 124 = cursor, 125 = 2x2 crafting, 126 = 3x3 crafting).
pub const CONTAINER_INVENTORY: u8 = 0;
pub const CONTAINER_OFFHAND: u8 = 119;
pub const CONTAINER_ARMOR: u8 = 120;
pub const CONTAINER_CURSOR: u8 = 124;
pub const CONTAINER_CRAFT_SMALL: u8 = 125;
pub const CONTAINER_CRAFT_BIG: u8 = 126;

/// Reads one slot of a modelled container (crafting result slots included).
pub fn container_slot(inventory: &PlayerInventory, slot: &ItemStackContainerSlot) -> Option<ItemStack> {
    match slot.container {
        CONTAINER_INVENTORY => inventory.window_slot(WINDOW_ID_INVENTORY, slot.slot as u32).cloned(),
        CONTAINER_OFFHAND => inventory.window_slot(WINDOW_ID_OFFHAND, slot.slot as u32).cloned(),
        CONTAINER_ARMOR => inventory.window_slot(WINDOW_ID_ARMOR, slot.slot as u32).cloned(),
        CONTAINER_CURSOR => Some(inventory.cursor_item()),
        CONTAINER_CRAFT_SMALL => inventory.craft_slot(true, slot.slot as usize),
        CONTAINER_CRAFT_BIG => inventory.craft_slot(false, slot.slot as usize),
        _ => None,
    }
}

fn set_container_slot(
    inventory: &mut PlayerInventory,
    slot: &ItemStackContainerSlot,
    item: ItemStack,
) -> bool {
    match slot.container {
        CONTAINER_INVENTORY => inventory.set_window_slot(WINDOW_ID_INVENTORY, slot.slot as u32, item),
        CONTAINER_OFFHAND => inventory.set_window_slot(WINDOW_ID_OFFHAND, slot.slot as u32, item),
        CONTAINER_ARMOR => inventory.set_window_slot(WINDOW_ID_ARMOR, slot.slot as u32, item),
        CONTAINER_CURSOR => inventory.set_cursor(item),
        CONTAINER_CRAFT_SMALL => inventory.set_craft_slot(true, slot.slot as usize, item),
        CONTAINER_CRAFT_BIG => inventory.set_craft_slot(false, slot.slot as usize, item),
        _ => false,
    }
}

/// Outcome of one `ItemStackRequestPacket`: the response entry to send plus
/// every item the server dropped into the world (Q key / container close).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStackRequestOutcome {
    pub response: ItemStackResponseEntry,
    pub dropped: Vec<ItemStack>,
    pub changed: Vec<ItemStackResponseSlot>,
}

impl Default for ItemStackRequestOutcome {
    fn default() -> Self {
        Self {
            response: ItemStackResponseEntry::error(0),
            dropped: Vec::new(),
            changed: Vec::new(),
        }
    }
}

fn empty_slot(slot: ItemStackContainerSlot) -> ItemStackResponseSlot {
    ItemStackResponseSlot { slot, item: ItemStack::empty() }
}

/// Applies one `ItemStackRequestPacket` to a player inventory, port of
/// PocketMine's `Player::handleItemStackRequest`. Take/Place/Swap move the
/// cursor, Drop/Destroy remove items, CreativeCreate adds them and
/// CraftRecipe consumes ingredients from the main inventory. Any failure
/// rolls the whole request back and answers with an error status.
pub fn apply_item_stack_request(
    inventory: &mut PlayerInventory,
    mgr: &CraftingManager,
    packet: &ItemStackRequestPacket,
    is_creative: bool,
) -> ItemStackRequestOutcome {
    let snapshot = inventory.clone();
    let mut outcome = ItemStackRequestOutcome {
        response: ItemStackResponseEntry::ok(packet.request_id),
        ..Default::default()
    };

    for action in &packet.actions {
        if !apply_one_action(inventory, mgr, action, is_creative, &mut outcome) {
            *inventory = snapshot;
            outcome.response = ItemStackResponseEntry::error(packet.request_id);
            outcome.changed.clear();
            outcome.dropped.clear();
            return outcome;
        }
    }
    outcome
}

fn apply_one_action(
    inventory: &mut PlayerInventory,
    mgr: &CraftingManager,
    action: &ItemStackRequestAction,
    is_creative: bool,
    outcome: &mut ItemStackRequestOutcome,
) -> bool {
    match action {
        ItemStackRequestAction::Take { slot, count } => {
            take_from_slot(inventory, slot, *count, &mut outcome.changed)
        }
        ItemStackRequestAction::Place { slot, count } => {
            place_to_slot(inventory, slot, *count, &mut outcome.changed)
        }
        ItemStackRequestAction::Swap { slot_a, slot_b } => {
            let Some(a) = container_slot(inventory, slot_a) else { return false };
            let Some(b) = container_slot(inventory, slot_b) else { return false };
            if !set_container_slot(inventory, slot_a, b) {
                return false;
            }
            set_container_slot(inventory, slot_b, a);
            outcome.changed.push(ItemStackResponseSlot { slot: *slot_a, item: container_slot(inventory, slot_a).unwrap_or_else(ItemStack::empty) });
            outcome.changed.push(ItemStackResponseSlot { slot: *slot_b, item: container_slot(inventory, slot_b).unwrap_or_else(ItemStack::empty) });
            true
        }
        ItemStackRequestAction::Drop { slot, count } => {
            // PocketMine `removeItemFromSlot` requires `count >= 1` and
            // `count <= stack count`; it routes through the same slot
            // translation as every other action, so the crafting windows
            // (125/126) must be handled here too instead of `drop_from_slot`
            // (which only knows the 0/119/120/124 windows and always
            // errored for the craft windows).
            if *count == 0 {
                return false;
            }
            let Some(item) = container_slot(inventory, slot) else { return false };
            if item.is_empty() || (*count as u16) > item.count {
                return false;
            }
            let dropped = ItemStack { count: *count as u16, ..item.clone() };
            let mut updated = item;
            updated.count -= *count as u16;
            if !set_container_slot(inventory, slot, if updated.count == 0 { ItemStack::empty() } else { updated }) {
                return false;
            }
            notify(outcome, *slot, inventory);
            outcome.dropped.push(dropped);
            true
        }
        ItemStackRequestAction::Destroy { slot, count } => {
            if *count == 0 {
                return false;
            }
            let Some(item) = container_slot(inventory, slot) else { return false };
            if item.is_empty() || (*count as u16) > item.count {
                return false;
            }
            // destroyed items vanish (creative delete)
            let remaining = item.count.saturating_sub(*count as u16);
            let updated = if remaining == 0 { ItemStack::empty() } else { ItemStack { count: remaining, ..item } };
            if !set_container_slot(inventory, slot, updated) {
                return false;
            }
            notify(outcome, *slot, inventory);
            true
        }
        ItemStackRequestAction::CreativeCreate { item, count } => {
            if !is_creative {
                // Creative-create is only trusted from creative players.
                return false;
            }
            let mut stack = item.clone();
            stack.count = (*count as u16).min(max_stack_size(stack.network_id));
            if inventory.add_item(&stack) == 0 {
                // `add_item` already moved it; report the cursor as changed.
                outcome.changed.push(empty_slot(ItemStackContainerSlot::new(CONTAINER_CURSOR, 0)));
                true
            } else {
                false
            }
        }
        ItemStackRequestAction::CraftRecipe { recipe_id, count } => {
            craft_recipe(inventory, mgr, recipe_id, *count, outcome)
        }
    }
}

fn take_from_slot(
    inventory: &mut PlayerInventory,
    slot: &ItemStackContainerSlot,
    count: u32,
    changed: &mut Vec<ItemStackResponseSlot>,
) -> bool {
    // PocketMine `removeItemFromSlot`: count < 1 or taking more than the
    // stack holds is an error (not a silently clamped no-op).
    if count == 0 {
        return false;
    }
    let Some(mut item) = container_slot(inventory, slot) else { return false };
    if item.is_empty() || (count as u16) > item.count {
        return false;
    }
    let taken = count as u16;
    item.count -= taken;
    let cursor = inventory.cursor_item();
    let cursor_updated = if cursor.is_empty() {
        ItemStack { count: taken, ..item.clone().with_count(taken) }
    } else if cursor.network_id == item.network_id && cursor.meta == item.meta && cursor.nbt == item.nbt {
        ItemStack { count: cursor.count + taken, ..cursor.clone() }
    } else {
        return false;
    };
    if cursor_updated.count > max_stack_size(cursor_updated.network_id) {
        return false;
    }
    if !set_container_slot(inventory, slot, if item.count == 0 { ItemStack::empty() } else { item }) {
        return false;
    }
    inventory.set_cursor(cursor_updated.clone());
    changed.push(ItemStackResponseSlot {
        slot: *slot,
        item: container_slot(inventory, slot).unwrap_or_else(ItemStack::empty),
    });
    changed.push(ItemStackResponseSlot {
        slot: ItemStackContainerSlot::new(CONTAINER_CURSOR, 0),
        item: cursor_updated,
    });
    true
}

fn place_to_slot(
    inventory: &mut PlayerInventory,
    slot: &ItemStackContainerSlot,
    count: u32,
    changed: &mut Vec<ItemStackResponseSlot>,
) -> bool {
    // PocketMine `addItemToSlot`: count < 1 or placing more than the cursor
    // holds is an error (not a silently clamped no-op).
    if count == 0 {
        return false;
    }
    let cursor = inventory.cursor_item();
    if cursor.is_empty() || (count as u16) > cursor.count {
        return false;
    }
    let Some(mut target) = container_slot(inventory, slot) else { return false };
    let placed = count as u16;
    if target.is_empty() {
        target = ItemStack { count: placed, ..cursor.clone() };
    } else if target.network_id == cursor.network_id
        && target.meta == cursor.meta
        && target.nbt == cursor.nbt
    {
        target.count += placed;
        if target.count > max_stack_size(target.network_id) {
            return false;
        }
    } else {
        return false;
    }
    let mut cursor = cursor;
    cursor.count -= placed;
    if !set_container_slot(inventory, slot, target) {
        return false;
    }
    inventory.set_cursor(if cursor.count == 0 { ItemStack::empty() } else { cursor.clone() });
    changed.push(ItemStackResponseSlot {
        slot: *slot,
        item: container_slot(inventory, slot).unwrap_or_else(ItemStack::empty),
    });
    changed.push(ItemStackResponseSlot {
        slot: ItemStackContainerSlot::new(CONTAINER_CURSOR, 0),
        item: cursor,
    });
    true
}

fn notify(outcome: &mut ItemStackRequestOutcome, slot: ItemStackContainerSlot, inventory: &PlayerInventory) {
    // Report the *actual* final contents: a Destroy/Drop that only takes
    // part of a stack leaves the remainder in the slot. The old code always
    // reported an empty stack and desynced the client.
    outcome.changed.push(ItemStackResponseSlot {
        slot,
        item: container_slot(inventory, &slot).unwrap_or_else(ItemStack::empty),
    });
}

fn craft_recipe(
    inventory: &mut PlayerInventory,
    mgr: &CraftingManager,
    recipe_id: &str,
    count: u32,
    outcome: &mut ItemStackRequestOutcome,
) -> bool {
    // PocketMine `beginCrafting`: repetitions < 1 or > 256 are rejected.
    // The old code clamped 0 to 1 and accepted anything else, so a crafted
    // stack could be handed out without the client actually consuming its
    // inputs.
    if count == 0 || count > MAX_CRAFT_REPETITIONS {
        return false;
    }
    let Some(craft) = mgr.recipe_by_id(recipe_id) else { return false };
    // For shapeless recipes the ingredient list tells what to consume.
    let wanted = recipe_id.strip_prefix("minecraft:").unwrap_or(recipe_id);
    let recipe = mgr
        .shapeless
        .iter()
        .find(|r| r.id.strip_prefix(&format!("minecraft:{wanted}#")).is_some());
    let mut result = craft.result.clone();
    let times = count;
    if let Some(recipe) = recipe {
        // Consume one of every ingredient, then add `times` results.
        for ingredient in &recipe.ingredients {
            if !consume_ingredient(inventory, ingredient, &mgr.tags, times) {
                return false;
            }
        }
        result.count = result.count.saturating_mul(times as u16);
    } else {
        // Shaped recipe: consume one of every pattern cell from the main
        // inventory (the grid path `take_result` stays authoritative for
        // manual grid crafting).
        let wanted = recipe_id.strip_prefix("minecraft:").unwrap_or(recipe_id);
        let shaped = mgr
            .shaped
            .iter()
            .find(|r| r.id.strip_prefix(&format!("minecraft:{wanted}#")).is_some());
        if let Some(shaped) = shaped {
            let mut pending: Vec<RecipeIngredient> = shaped
                .shape
                .iter()
                .flatten()
                .filter_map(|cell| cell.clone())
                .collect();
            for ingredient in pending.drain(..) {
                if !consume_ingredient(inventory, &ingredient, &mgr.tags, times) {
                    return false;
                }
            }
        }
        result.count = result.count.saturating_mul(times as u16);
    }
    if inventory.add_item(&result) != 0 {
        return false;
    }
    outcome.changed.push(empty_slot(ItemStackContainerSlot::new(CONTAINER_INVENTORY, 0)));
    true
}

fn consume_ingredient(
    inventory: &mut PlayerInventory,
    ingredient: &RecipeIngredient,
    tags: &HashMap<String, HashSet<i16>>,
    times: u32,
) -> bool {
    if times == 0 {
        return true;
    }
    // PocketMine's `CraftingTransaction` checks the *total* balance across
    // the inventory, not one stack: the caller may own e.g. 1+1 in two
    // slots and need 2. Require the total first, then consume across stacks.
    let available: u32 = inventory
        .slots()
        .iter()
        .filter(|slot| ingredient.matches(slot, tags))
        .map(|slot| slot.count as u32)
        .sum();
    if available < times {
        return false;
    }
    let mut remaining = times;
    for index in 0..inventory.slots().len() {
        if remaining == 0 {
            break;
        }
        let Some(slot) = inventory.window_slot(WINDOW_ID_INVENTORY, index as u32).cloned() else {
            continue;
        };
        if !ingredient.matches(&slot, tags) || slot.is_empty() {
            continue;
        }
        let taken = (slot.count as u32).min(remaining) as u16;
        let mut updated = slot;
        updated.count -= taken;
        remaining -= taken as u32;
        inventory.set_window_slot(
            WINDOW_ID_INVENTORY,
            index as u32,
            if updated.count == 0 { ItemStack::empty() } else { updated },
        );
    }
    remaining == 0
}

/// Human-readable list of recipes for diagnostics.
pub fn recipe_summary(mgr: &CraftingManager) -> String {
    format!(
        "shaped={}, shapeless={}, furnace={}, brewing={}, skipped_unknown={}",
        mgr.shaped.len(),
        mgr.shapeless.len(),
        mgr.furnace.len(),
        mgr.brewing.len(),
        mgr.skipped_unknown,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(network_id: i16, count: u16) -> ItemStack {
        ItemStack::new(network_id, count)
    }

    fn slot(container: u8, slot: u8) -> ItemStackContainerSlot {
        ItemStackContainerSlot::new(container, slot)
    }

    #[test]
    fn manager_loads_bedrock_recipes() {
        let mgr = manager();
        assert!(mgr.recipe_count() > 800, "got {}", mgr.recipe_count());
        assert!(mgr.shaped.len() > 200);
        assert!(mgr.shapeless.len() > 100);
        assert!(!mgr.furnace.is_empty());
        assert!(!mgr.tags.is_empty());
        // stone_crafting_materials must resolve (brewing_stand tag recipe).
        assert!(mgr.tags.get("minecraft:stone_crafting_materials").is_some());
    }

    /// Scratch diagnostic: when `PM_CORE_DUMP_WIRE` is set, writes the
    /// full `CraftingDataPacket` wire bytes to that path so the actual
    /// wire can be byte-compared against the BDS capture offline.
    #[test]
    fn dump_wire_when_env_set() {
        let Ok(path) = std::env::var("PM_CORE_DUMP_WIRE") else {
            return;
        };
        let wire = crate::PlayerSession::generated_crafting_data_wire();
        std::fs::write(&path, &wire).unwrap();
        eprintln!(
            "dumped crafting_data_wire: {} bytes -> {}",
            wire.len(),
            path
        );
    }

    #[test]
    fn crafting_data_wire_decodes() {
        let wire = crate::PlayerSession::generated_crafting_data_wire();
        let mut reader = bedrock_protocol::binary::Reader::new(&wire);
        assert_eq!(reader.read_var_u32().unwrap(), 0x34);
        let packet =
            bedrock_protocol::packets::crafting_data::CraftingDataPacket::decode_payload(
                &mut reader,
            )
            .unwrap();
        assert_eq!(reader.remaining(), 0);
        let mgr = manager();
        assert_eq!(
            packet.recipes.len(),
            mgr.shaped.len() + mgr.shapeless.len() + mgr.furnace.len()
        );
        assert_eq!(packet.potion_types.len(), mgr.brewing.len());
        assert_eq!(packet.potion_containers.len(), mgr.potion_container.len());
        eprintln!(
            "recipes={} potions={} containers={} wire_len={}",
            packet.recipes.len(),
            packet.potion_types.len(),
            packet.potion_containers.len(),
            wire.len()
        );
        assert!(!packet.potion_containers.is_empty());
    }

    #[test]
    fn crafts_oak_planks_in_2x2() {
        let mgr = manager();
        let mut grid = CraftingGrid::small();
        let oak_log = item_network_id("minecraft:oak_log").expect("oak_log has an id");
        grid.slots[0] = stack(oak_log, 1);
        let outcome = mgr.craft_from_grid(&grid).expect("log -> planks");
        let planks = item_network_id("minecraft:oak_planks").unwrap();
        assert_eq!(outcome.result.network_id, planks);
        assert_eq!(outcome.result.count, 4);
    }

    #[test]
    fn crafts_crafting_table_in_3x3() {
        let mgr = manager();
        let mut grid = CraftingGrid::big();
        let planks = item_network_id("minecraft:oak_planks").unwrap();
        // Bottom-left 2x2 block of planks -> crafting table.
        grid.slots[6] = stack(planks, 1);
        grid.slots[7] = stack(planks, 1);
        grid.slots[3] = stack(planks, 1);
        grid.slots[4] = stack(planks, 1);
        let outcome = mgr.craft_from_grid(&grid).expect("2x2 planks -> table");
        assert_eq!(
            outcome.result.network_id,
            item_network_id("minecraft:crafting_table").unwrap()
        );
    }

    #[test]
    fn shapeless_redye_matches_any_layout() {
        let mgr = manager();
        let mut grid = CraftingGrid::small();
        let mut bed = stack(item_network_id("minecraft:bed").unwrap(), 1);
        bed.meta = 1;
        grid.slots[1] = bed;
        grid.slots[2] = stack(item_network_id("minecraft:black_dye").unwrap(), 1);
        let outcome = mgr.craft_from_grid(&grid).expect("bed + dye -> redyed bed (shapeless)");
        assert_eq!(outcome.result.network_id, item_network_id("minecraft:bed").unwrap());
        assert_eq!(outcome.result.meta, 15);
    }

    #[test]
    fn shaped_bread_in_a_row() {
        let mgr = manager();
        let mut grid = CraftingGrid::big();
        let wheat = item_network_id("minecraft:wheat").unwrap();
        grid.slots[3] = stack(wheat, 1);
        grid.slots[4] = stack(wheat, 1);
        grid.slots[5] = stack(wheat, 1);
        let outcome = mgr.craft_from_grid(&grid).expect("AAA wheat -> bread (shaped)");
        assert_eq!(outcome.result.network_id, item_network_id("minecraft:bread").unwrap());
    }

    #[test]
    fn furnace_smelt() {
        let mgr = manager();
        let cobble = item_network_id("minecraft:cobblestone").unwrap();
        let recipe = mgr.find_furnace(&stack(cobble, 1)).expect("cobble smelts to stone");
        assert_eq!(recipe.output.network_id, item_network_id("minecraft:stone").unwrap());
    }

    #[test]
    fn take_place_swap_move_items_with_cursor() {
        let mut inventory = PlayerInventory::new();
        inventory.set_window_slot(WINDOW_ID_INVENTORY, 0, stack(1, 64));
        let packet = ItemStackRequestPacket::new(
            1,
            vec![
                ItemStackRequestAction::Take { slot: slot(0, 0), count: 32 },
                ItemStackRequestAction::Place { slot: slot(0, 5), count: 32 },
            ],
        );
        let outcome = apply_item_stack_request(&mut inventory, manager(), &packet, false);
        assert_eq!(outcome.response.status, STATUS_OK);
        assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 0).unwrap().count, 32);
        assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 5).unwrap().count, 32);
        assert!(inventory.cursor_item().is_empty());

        let packet = ItemStackRequestPacket::new(
            2,
            vec![ItemStackRequestAction::Swap { slot_a: slot(0, 0), slot_b: slot(0, 5) }],
        );
        let outcome = apply_item_stack_request(&mut inventory, manager(), &packet, false);
        assert_eq!(outcome.response.status, STATUS_OK);
        assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 0).unwrap().count, 32);
        assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 5).unwrap().count, 32);
    }

    #[test]
    fn drop_removes_items_and_reports_them() {
        let mut inventory = PlayerInventory::new();
        inventory.set_window_slot(WINDOW_ID_INVENTORY, 2, stack(1, 64));
        let packet = ItemStackRequestPacket::new(
            3,
            vec![ItemStackRequestAction::Drop { slot: slot(0, 2), count: 10 }],
        );
        let outcome = apply_item_stack_request(&mut inventory, manager(), &packet, false);
        assert_eq!(outcome.response.status, STATUS_OK);
        assert_eq!(outcome.dropped, vec![stack(1, 10)]);
        assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 2).unwrap().count, 54);
    }

    #[test]
    fn creative_create_adds_items_for_creative_players() {
        let mut inventory = PlayerInventory::new();
        let packet = ItemStackRequestPacket::new(
            4,
            vec![ItemStackRequestAction::CreativeCreate { item: stack(1, 64), count: 64 }],
        );
        let outcome = apply_item_stack_request(&mut inventory, manager(), &packet, true);
        assert_eq!(outcome.response.status, STATUS_OK);
        assert_eq!(inventory.total_items(), 64);
    }

    #[test]
    fn inventory_helpers_for_drop_and_hotbar() {
        let mut inventory = PlayerInventory::new();
        inventory.set_window_slot(WINDOW_ID_INVENTORY, 3, stack(1, 16));
        // Q drop parity: remove 4 from slot 3.
        assert_eq!(inventory.drop_from_slot(WINDOW_ID_INVENTORY, 3, 4), Some(stack(1, 4)));
        assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 3).unwrap().count, 12);
        // Hotbar move parity: swap slots 3 and 7.
        inventory.swap_inventory_slots(3, 7);
        assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 7).unwrap().count, 12);
        assert!(inventory.window_slot(WINDOW_ID_INVENTORY, 3).unwrap().is_empty());
    }

    #[test]
    fn crafting_inventory_take_result_consumes_inputs() {
        let planks = item_network_id("minecraft:oak_planks").unwrap();
        let mut table = CraftingTableInventory::new();
        table.inventory.set_input(6, stack(planks, 2));
        table.inventory.set_input(7, stack(planks, 2));
        table.inventory.set_input(3, stack(planks, 2));
        table.inventory.set_input(4, stack(planks, 2));
        table.update_result();
        let result = table.take_result().expect("2x2 planks in 3x3 grid crafts");
        assert_eq!(result.network_id, item_network_id("minecraft:crafting_table").unwrap());
    }

    #[test]
    fn creative_catalog_is_populated() {
        let items = creative_items();
        assert!(items.len() > 100, "got {}", items.len());
        assert!(!creative_groups().is_empty());
    }

    // --- Regression tests for the Axolotl reference parity fixes ----------

    /// PocketMine `ShapedRecipe::matchesCraftingGrid` tries the direct and
    /// the mirrored orientation. The old engine only tried the direct one,
    /// so an asymmetric shape placed mirrored never crafted.
    #[test]
    fn shaped_mirror_accepts_left_right_flip() {
        let mgr = manager();
        // Find a shaped recipe whose shape is not horizontally symmetric so
        // the mirror check is really exercised.
        let recipe = mgr
            .shaped
            .iter()
            .find(|r| {
                let width = r.shape[0].len();
                r.shape.iter().any(|row| {
                    row.iter().enumerate().any(|(col, cell)| {
                        let mirror = &row[width - col - 1];
                        cell.is_some() != mirror.is_some()
                    })
                })
            })
            .expect("there is an asymmetric shaped recipe");
        // Build the cropped grid from the *mirrored* shape: the direct
        // orientation must fail (shape differs) but the mirror must match.
        let mut grid = CraftingGrid::new(3);
        for (row, shape_row) in recipe.shape.iter().enumerate() {
            for (col, cell) in shape_row.iter().enumerate() {
                let Some(ingredient) = cell else { continue };
                let mirrored_col = shape_row.len() - col - 1;
                grid.slots[row * 3 + mirrored_col] = stack_for_ingredient(ingredient);
            }
        }
        assert!(grid.matches_shaped(recipe, &mgr.tags).is_some());
    }

    fn stack_for_ingredient(ingredient: &RecipeIngredient) -> ItemStack {
        match ingredient {
            RecipeIngredient::Exact { network_id, meta } => {
                let mut s = ItemStack::new(*network_id, 1);
                s.meta = *meta;
                s
            }
            RecipeIngredient::MetaWildcard { network_id } => ItemStack::new(*network_id, 1),
            RecipeIngredient::TagWildcard { tag } => {
                let id = manager()
                    .tags
                    .get(tag)
                    .and_then(|ids| ids.iter().next().copied())
                    .expect("tag used in a recipe resolves to an item");
                ItemStack::new(id, 1)
            }
        }
    }

    /// PocketMine has one furnace manager per `FurnaceType`. `find_furnace`
    /// must prefer the plain `furnace` block and must not let e.g. a smoker
    /// recipe smelt in a furnace. `find_furnace_for` is block-exact.
    #[test]
    fn furnace_lookup_is_per_block() {
        let mgr = manager();
        let cobble = item_network_id("minecraft:cobblestone").unwrap();
        let furnace = mgr.find_furnace_for("furnace", &stack(cobble, 1));
        assert!(furnace.is_some(), "cobble smelts in a furnace");
        assert_eq!(furnace.unwrap().output.network_id, item_network_id("minecraft:stone").unwrap());
        // Whatever the first recipe for the input is, `find_furnace` must
        // return a furnace-block recipe for it.
        let any = mgr.find_furnace(&stack(cobble, 1)).expect("cobble smelts");
        assert_eq!(any.block, "furnace");
        // Cook durations follow PocketMine `FurnaceType` metadata.
        assert_eq!(CraftingManager::furnace_cook_ticks("furnace"), 200);
        assert_eq!(CraftingManager::furnace_cook_ticks("blast_furnace"), 100);
        assert_eq!(CraftingManager::furnace_cook_ticks("smoker"), 100);
        assert_eq!(CraftingManager::furnace_cook_ticks("campfire"), 600);
        assert_eq!(CraftingManager::furnace_cook_ticks("soul_campfire"), 600);
    }

    /// PocketMine checks container-change recipes *before* potion-type
    /// recipes and preserves the input meta. Potion (meta 4) + dragon's
    /// breath is not a container change in the data (splash potion is), but
    /// potion (meta 4) + gunpowder must travel Potion -> Splash Potion
    /// with meta 4 kept.
    #[test]
    fn brewing_container_change_keeps_meta_and_wins() {
        let mgr = manager();
        let potion_id = item_network_id("minecraft:potion").unwrap();
        let mut potion = ItemStack::new(potion_id, 1);
        potion.meta = 4;
        let gunpowder = item_network_id("minecraft:gunpowder").unwrap();
        let out = mgr
            .brewing_result(&potion, &ItemStack::new(gunpowder, 1))
            .expect("potion + gunpowder is a container change");
        assert_eq!(out.network_id, item_network_id("minecraft:splash_potion").unwrap());
        assert_eq!(out.meta, 4, "container changes keep the input meta");
        // A known potion-type recipe still works and keeps its own meta.
        let nether_wart = item_network_id("minecraft:nether_wart").unwrap();
        if let Some(type_out) = mgr.brewing_result(&potion, &ItemStack::new(nether_wart, 1)) {
            assert_eq!(type_out.network_id, potion_id);
            assert_ne!(type_out.meta, potion.meta);
        }
    }

    /// `CraftingManager::recipe_by_network_id` implements
    /// `recipeId - RECIPE_ID_OFFSET` against the shaped/shapeless/furnace
    /// serialization order.
    #[test]
    fn network_id_lookup_roundtrips_the_book() {
        let mgr = manager();
        let total = mgr.recipe_count() as u32;
        assert!(mgr.has_network_id(1));
        assert!(mgr.has_network_id(total));
        assert!(!mgr.has_network_id(0), "the client rejects recipe id 0");
        assert!(!mgr.has_network_id(total + 1));
        let first = mgr.recipe_by_network_id(1).expect("id 1 is the first shaped recipe");
        assert_eq!(first.result(), mgr.shaped[0].result);
        let kind = mgr.recipe_by_network_id(1 + mgr.shaped.len() as u32).unwrap();
        assert_eq!(kind.result(), mgr.shapeless[0].result);
        let kind = mgr
            .recipe_by_network_id((1 + mgr.shaped.len() as u32) + mgr.shapeless.len() as u32)
            .unwrap();
        assert_eq!(kind.result(), mgr.furnace[0].output);
        assert_eq!(kind.ingredients().len(), 1);
    }

    /// PocketMine `beginCrafting` rejects repetitions outside `1..=256`.
    #[test]
    fn craft_recipe_rejects_zero_and_over_256_repetitions() {
        let mgr = manager();
        let recipe = mgr
            .shapeless
            .iter()
            .find(|r| r.ingredients.len() >= 1)
            .expect("a shapeless recipe");
        // Build a request that would craft without the guard: count 0.
        let mut inventory = PlayerInventory::new();
        let packet = ItemStackRequestPacket::new(
            90,
            vec![ItemStackRequestAction::CraftRecipe {
                recipe_id: recipe.id.clone(),
                count: 0,
            }],
        );
        let outcome = apply_item_stack_request(&mut inventory, mgr, &packet, false);
        assert_eq!(outcome.response.status, STATUS_ERROR);
        let mut inventory = PlayerInventory::new();
        let packet = ItemStackRequestPacket::new(
            91,
            vec![ItemStackRequestAction::CraftRecipe {
                recipe_id: recipe.id.clone(),
                count: 300,
            }],
        );
        let outcome = apply_item_stack_request(&mut inventory, mgr, &packet, false);
        assert_eq!(outcome.response.status, STATUS_ERROR);
    }

    /// PocketMine's `CraftingTransaction` balances across the whole
    /// inventory: 1+1 in two slots must satisfy a 2x consumption. The old
    /// single-stack check failed that case.
    #[test]
    fn consume_ingredient_spans_multiple_stacks() {
        let planks = item_network_id("minecraft:oak_planks").unwrap();
        let mut inventory = PlayerInventory::new();
        inventory.set_window_slot(WINDOW_ID_INVENTORY, 0, stack(planks, 1));
        inventory.set_window_slot(WINDOW_ID_INVENTORY, 1, stack(planks, 1));
        let ingredient = RecipeIngredient::Exact { network_id: planks, meta: 0 };
        assert!(consume_ingredient(&mut inventory, &ingredient, &manager().tags, 2));
        assert!(inventory.slots().iter().all(ItemStack::is_empty));
        // Insufficient total must fail without touching anything.
        let mut inventory = PlayerInventory::new();
        inventory.set_window_slot(WINDOW_ID_INVENTORY, 0, stack(planks, 1));
        assert!(!consume_ingredient(&mut inventory, &ingredient, &manager().tags, 2));
        assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 0).unwrap().count, 1);
    }

    /// PocketMine `removeItemFromSlot` rejects count 0 / too-large counts,
    /// and `addItemToSlot` rejects placing more than the cursor holds; the
    /// old code silently clamped to a no-op (count 0 succeeded).
    #[test]
    fn take_place_reject_impossible_counts() {
        let mut inventory = PlayerInventory::new();
        inventory.set_window_slot(WINDOW_ID_INVENTORY, 0, stack(1, 5));
        let packet = ItemStackRequestPacket::new(
            92,
            vec![ItemStackRequestAction::Take { slot: slot(0, 0), count: 0 }],
        );
        let outcome = apply_item_stack_request(&mut inventory, manager(), &packet, false);
        assert_eq!(outcome.response.status, STATUS_ERROR);
        assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 0).unwrap().count, 5);
        let packet = ItemStackRequestPacket::new(
            93,
            vec![ItemStackRequestAction::Take { slot: slot(0, 0), count: 6 }],
        );
        let outcome = apply_item_stack_request(&mut inventory, manager(), &packet, false);
        assert_eq!(outcome.response.status, STATUS_ERROR);
    }

    /// A Drop must be reportable in the crafting windows too (container
    /// 125/126); the old code routed drops through `drop_from_slot`, which
    /// only knows windows 0/119/120/124 and always errored there. A partial
    /// drop must also report the remainder, not an empty stack.
    #[test]
    fn drop_reports_remainder_and_works_in_craft_windows() {
        let mut inventory = PlayerInventory::new();
        inventory.set_craft_slot(true, 0, stack(1, 64));
        let packet = ItemStackRequestPacket::new(
            94,
            vec![ItemStackRequestAction::Drop { slot: slot(CONTAINER_CRAFT_SMALL, 0), count: 10 }],
        );
        let outcome = apply_item_stack_request(&mut inventory, manager(), &packet, false);
        assert_eq!(outcome.response.status, STATUS_OK);
        assert_eq!(outcome.dropped, vec![stack(1, 10)]);
        assert_eq!(inventory.craft_slot(true, 0).unwrap().count, 54);
        assert_eq!(outcome.changed.len(), 1);
        assert_eq!(outcome.changed[0].item.count, 54);
    }
}

// `ItemStack::with_count` is a test-only helper kept here so the Take path
// reads cleanly; it mirrors `ItemStack { count, ..clone() }`.
trait WithCount {
    fn with_count(self, count: u16) -> Self;
}
impl WithCount for ItemStack {
    fn with_count(self, count: u16) -> Self {
        ItemStack { count, ..self }
    }
}
