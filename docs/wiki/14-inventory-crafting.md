# 14 — Inventario y crafteo

Inventario autoritativo del jugador, armadura, rejillas de crafteo,
recetas, yunque, contenedores dinámicos y el motor `ItemStackRequest`.
Port de PocketMine-MP (`PlayerInventory`, `ArmorInventory`,
`CraftingGrid`, `CraftingManager`) al núcleo Rust; el cliente Bedrock
nunca es la fuente de verdad: toda acción se valida contra el estado
del servidor y, si no coincide, se rechaza y se reenvía el inventario
completo.

## Mapa rápido

| Archivo | Responsabilidad |
|---|---|
| `crates/server-core/src/inventory.rs` | `PlayerInventory` (36 + armadura + offhand + cursor + crafteo), `ArmorInventory`, registro de ítems, `apply_actions` |
| `crates/server-core/src/crafting.rs` | `CraftingManager`, `RecipeIngredient`, recetas shaped/shapeless/horno/cervecería, `CraftingGrid`, `CraftingInventory`, creativo, motor `ItemStackRequest` |
| `crates/server-core/src/anvil.rs` | `AnvilState`, `AnvilInventory`, `anvil_combine`, `apply_repair` |
| `crates/server-core/src/packets_wire.rs` | Wire de `ContainerOpen`/`ContainerClose`/`ItemStackResponse`, sentinel de tags en `CraftingData` |
| `crates/server-core/src/session.rs` | `PlayerSession::inventory` / `crafting_table` / `anvil_inventory` / `open_container`, dispatch 0x1E/0x21/0x2F/0x93, `resync_inventory`, `crafting_data_wire`, `creative_content_wire` |
| `crates/php-plugin-bridge/php/player.php` | `pocketmine\inventory\PlayerInventory` (stub en memoria, ver §9) |
| `docs/plugin-api-contract.md` | Acciones de plugin y límites del milestone |

Datos embebidos en compilación con `include_str!`:

- `data/item_ids.txt` — diccionario BDS 1.26.60 (protocolo 2225),
  un par `identificador<TAB>id numérico` por línea.
- `data/recipes/{shaped_crafting,shaped_crafting_asymmetric,shapeless_crafting,potion_type,potion_container_change}.json`
- `data/item_tags.json` — tags de ingredientes.
- `data/creative/{items,construction,equipment,nature}.json` — catálogo creativo.

## 1. `PlayerInventory`: 36 + armadura 4 + offhand + cursor

```rust
// crates/server-core/src/inventory.rs
pub const INVENTORY_SIZE: usize = 36;   // 9 hotbar + 27 almacenamiento
pub const HOTBAR_SIZE: usize = 9;
pub const ARMOR_SIZE: usize = 4;        // cabeza, pecho, piernas, botas
pub const MAX_STACK_SIZE: u16 = 64;

pub const WINDOW_ID_INVENTORY: u32 = 0;
pub const WINDOW_ID_OFFHAND:   u32 = 119;
pub const WINDOW_ID_ARMOR:     u32 = 120;
pub const WINDOW_ID_CURSOR:    u32 = 124;
pub const WINDOW_ID_CRAFT_SMALL: u32 = 125;  // 2x2 del jugador
pub const WINDOW_ID_CRAFT_BIG:   u32 = 126;  // 3x3 de mesa de crafteo
pub const CRAFT_SMALL_RESULT: usize = 4;     // 4 entradas + resultado
pub const CRAFT_BIG_RESULT:   usize = 9;     // 9 entradas + resultado
```

`PlayerInventory` guarda, además de los 36 slots principales:

- `armor: Vec<ItemStack>` (4), `offhand: Vec<ItemStack>` (1),
  `cursor: Vec<ItemStack>` (1): la pila entre clic y soltar.
- `craft_small` (5) y `craft_big` (10): rejilla + slot de resultado.
  Se acceden con `craft_slot(small, slot)` / `set_craft_slot(small, slot, item)`;
  `small = true` es la ventana 2x2, `false` la 3x3.
- `selected_hotbar_slot: u8` (recortado a `0..8` por
  `set_selected_hotbar_slot`). `held_item()` devuelve el slot de la
  mano principal; es lo que se envía en `AddPlayerPacket` y en
  `MobEquipmentPacket` (0x1F).

### Ventanas

```rust
window(window_id) -> Option<&[ItemStack]>       // 0 / 119 / 120 / 124
window_slot(window_id, slot) -> Option<&ItemStack>
set_window_slot(window_id, slot, item) -> bool  // false si no existe
windows() -> Vec<(u32, Vec<ItemStack>)>          // orden vanilla: inv, armor, offhand, cursor
contents() -> Vec<ItemStack>                    // los 36, para InventoryContentPacket
```

`window(999)` y slots fuera de rango devuelven `None`/`false`: nunca
se indexa fuera de límites.

### Operaciones sobre los 36 slots

| Método | Semántica |
|---|---|
| `add_item(&item) -> u16` | Apila primero sobre pilas iguales (`network_id`, `meta` y `nbt` iguales, hueco hasta `max_stack_size`), luego en slots vacíos. Devuelve el resto que no cupo. |
| `clear() -> u32` | Vacía solo los 36 principales (no cursor/armadura/offhand) y devuelve cuántos ítems quitó. |
| `remove_item(network_id, meta?, limit?) -> u32` | Quita ítems coincidentes, con límite opcional. |
| `count_item(network_id, meta?) -> u32` | Cuenta ítems coincidentes. |
| `total_items()` / `is_empty()` | Suma de `count` / todo vacío (solo principales). |
| `drop_from_slot(window, slot, count) -> Option<ItemStack>` | Drop con tecla Q: quita hasta `count` y devuelve la pila soltada; `None` si el slot está vacío o la ventana no existe. |
| `swap_inventory_slots(a, b)` | Intercambio de slots principales (scroll de hotbar / tecla numérica). |
| `armor_slots()` / `offhand()` / `offhand_item()` / `cursor_item()` / `set_cursor(item)` | Acceso a las demás ventanas. |

`max_stack_size(network_id)`: 64 por defecto; los ids `325..=350`
( cubos, herramientas y armas clásicas) son de pila única (`1`).

### `clear_matching`: la semántica de `/clear`

```rust
pub fn clear_matching(&mut self, item: Option<&ItemStack>, max_count: i32)
    -> (u32, bool)  // (cantidad, counted_only)
```

- Recorre las ventanas en orden vanilla: inventario, cursor, armadura,
  offhand (`CLEAR_ORDER`).
- El filtro es un ítem: `Item::equals()` compara tipo, daño **y** tag
  nombrado, así que una pila con NBT no coincide con el ítem sin tag
  que construye `/clear` desde su argumento (`matches_filter`).
- `max_count == 0`: solo cuenta, no borra (`counted_only = true`).
- `max_count < 0`: borra todo lo coincidente.
- `max_count > 0`: borra como máximo esa cantidad, con el límite
  compartido entre ventanas.
- `item = None`: borra todo, en todas las ventanas.

### Registro de ítems

Todo se resuelve contra `data/item_ids.txt` (cargado una vez con
`OnceLock`; los ítems data-driven sin id numérico no existen para
este protocolo: `/give` solo puede dar ítems clásicos con id).

```rust
resolve_item_name("stone" | "Stone" | "minecraft:stone") -> Option<&str>
resolve_item("stone" | "minecraft:stone" | "1" | "1:2" | "diamond_sword:5")
    -> Option<(network_id, meta, "minecraft:...")>
item_network_id("minecraft:stone") -> Option<i16>   // = Some(1)
item_display_name(347) -> Some("Diamond Sword")     // línea de éxito de /give
name_for_network_id(347) -> Some("minecraft:diamond_sword")
known_item_count() -> usize                          // > 800 en el capture
```

Alias: `grass -> minecraft:grass_block`, `slab -> minecraft:oak_slab`.
Los ids numéricos solo resuelven si el registro los conoce
(`resolve_item("9999") -> None`).

## 2. `ArmorInventory`: validación de la pieza

```rust
// inventory.rs — orden de red de la ventana 120
//   0 cabeza (_helmet) · 1 pecho (_chestplate) · 2 piernas (_leggings) · 3 botas (_boots)
pub struct ArmorInventory { pub slots: Vec<ItemStack> }  // 4

armor.equip(slot: u8, item: ItemStack) -> Option<ItemStack>
inv.equip_armor(slot: u8, item: ItemStack) -> Option<ItemStack>  // misma lógica
```

- `equip` devuelve la pieza que llevaba el slot (para intercambiarla
  con la mano) y `None` **sin tocar nada** si el slot está fuera de
  `0..3` o la pieza no encaja.
- `fits_armor_slot` decide por sufijo del nombre canónico
  (`name.ends_with("_helmet")`, etc.). Un casco no entra en el slot
  de botas; una espada no entra en ningún slot de armadura.
- Un ítem vacío siempre encaja: es la forma de desequipar.

La validación vive en Rust; el stub PHP de inventario no la replica
(§9).

## 3. `CraftingGrid`: 2x2 del jugador y 3x3 de mesa

```rust
// crates/server-core/src/crafting.rs
pub const SIZE_SMALL: usize = 2;   // CraftingGrid::SIZE_SMALL (jugador)
pub const SIZE_BIG: usize = 3;     // CraftingGrid::SIZE_BIG (mesa)

pub struct CraftingGrid { pub size: usize, pub slots: Vec<ItemStack> }
CraftingGrid::small()  // 2x2, 4 slots row-major
CraftingGrid::big()    // 3x3, 9 slots row-major
```

- `matches_shaped(recipe, tags)`: recorta los bordes vacíos de la
  rejilla (`crop_bounds`) y exige que el bounding box tenga exactamente
  el tamaño de la forma de la receta, celda a celda (`None` = vacía).
  Así una receta 2x2 se puede colocar en cualquier esquina de la 3x3.
  Devuelve `CraftOutcome { result, consume }` con `consume` = nº de
  slots no vacíos (regla de 1 por slot de PocketMine).
- `matches_shapeless(recipe, tags)`: multiconjunto — el orden no
  importa, cada ingrediente consume una pila distinta de la rejilla.

Inventarios con resultado:

```rust
pub struct CraftingInventory { pub grid: CraftingGrid, pub result: ItemStack }
pub struct PlayerCraftingInventory { pub inventory: CraftingInventory }  // 2x2
pub struct CraftingTableInventory { pub inventory: CraftingInventory }   // 3x3

set_input(slot, item) -> bool   // false si el slot no existe
update_result()                 // result = manager().craft_from_grid(grid) o vacío
take_result() -> Option<ItemStack>  // consume 1 de cada slot no vacío y recalcula
close_drops() -> Vec<ItemStack>     // entradas al cerrar el contenedor (mesa)
```

## 4. `CraftingManager`: shaped, shapeless y compañía

```rust
pub enum RecipeIngredient {
    Exact { network_id: i16, meta: u32 },       // id + meta exactos
    MetaWildcard { network_id: i16 },           // cualquier meta
    TagWildcard { tag: String },                // cualquier ítem del tag
}
pub const META_WILDCARD: u32 = 32767;           // marcador en el JSON Bedrock

pub struct ShapedRecipe   { id, shape: Vec<Vec<Option<RecipeIngredient>>>, result, priority }
pub struct ShapelessRecipe{ id, ingredients: Vec<RecipeIngredient>, result, priority }
pub struct FurnaceRecipe  { input: RecipeIngredient, output: ItemStack }
pub struct BrewingRecipe  { input: RecipeIngredient, ingredient: RecipeIngredient, output: ItemStack }
pub struct CraftOutcome   { result: ItemStack, consume: u16 }
```

`CraftingManager::build()` (expuesto como singleton perezoso
`manager()`) carga el JSON embebido:

- `shaped_crafting.json` + `shaped_crafting_asymmetric.json`: solo
  entradas con `block == "crafting_table"`. La forma (`shape` con
  letras `A`/`B`/…) se recorta de bordes vacíos igual que lo hace el
  JSON y cada letra se resuelve contra `input`.
- `shapeless_crafting.json`: `block == "crafting_table"` →
  shapeless; `furnace|blast_furnace|smoker|campfire|soul_campfire` →
  `FurnaceRecipe`.
- `potion_type.json` → `brewing`; `potion_container_change.json` →
  `potion_container` (nombres planos, meta 0).
- `item_tags.json` → `tags: HashMap<String, HashSet<i16>>`
  (`minecraft:stone_crafting_materials` y compañía).
- Las entradas cuyo ítem es data-driven y no tiene id numérico se
  saltan y se cuentan en `skipped_unknown`; `total_entries` cuenta
  todas las entradas de mesa (también las saltadas).
- Las recetas se ordenan por `priority` descendente: una receta
  específica gana a un fallback genérico de tag.

Búsqueda y crafteo:

```rust
find_shaped(grid) / find_shapeless(grid) -> Option<&Recipe>
craft_from_grid(grid) -> Option<CraftOutcome>   // shaped gana a shapeless, como BDS
find_furnace(&item) / find_brewing(&potion, &ingredient)
recipe_by_id(id) -> Option<CraftOutcome>        // para CraftRecipe (ver §5)
recipe_count() -> usize                          // shaped + shapeless + furnace (> 800 en tests)
recipe_summary(mgr) -> String                    // "shaped=…, shapeless=…, furnace=…, brewing=…, skipped_unknown=…"
```

Limitaciones conocidas, por diseño: `block_states` (base64) no se
decodifica — los resultados conservan `meta: 0` y `block_runtime_id: 0`,
igual que el resto del servidor modela ítems de bloque; los ítems
data-driven quedan fuera (§1).

## 5. `ItemStackRequest`: 0x93 → 0x94

El cliente moderno no manda `InventoryTransactionPacket` para mover
pilas: manda un `ItemStackRequestPacket` (0x93) con acciones y el
servidor responde con `ItemStackResponsePacket` (0x94) más
actualizaciones de slot. El motor vive en `crafting.rs` y lo despacha
`PlayerSession::handle_item_stack_request`.

Contenedores modelados (los ids coinciden con los de ventana):

```rust
pub const CONTAINER_INVENTORY: u8 = 0;
pub const CONTAINER_OFFHAND:   u8 = 119;
pub const CONTAINER_ARMOR:     u8 = 120;
pub const CONTAINER_CURSOR:    u8 = 124;
pub const CONTAINER_CRAFT_SMALL: u8 = 125;
pub const CONTAINER_CRAFT_BIG:   u8 = 126;
```

`container_slot(inventory, slot)` lee cualquier slot modelado
(incluidos los slots de resultado de crafteo 125/126);
`set_container_slot` escribe. Un contenedor desconocido → `false`.

```rust
pub fn apply_item_stack_request(
    inventory: &mut PlayerInventory,
    mgr: &CraftingManager,
    packet: &ItemStackRequestPacket,
    is_creative: bool,
) -> ItemStackRequestOutcome
// ItemStackRequestOutcome { response, dropped: Vec<ItemStack>, changed: Vec<ItemStackResponseSlot> }
```

Regla de oro: se clona el inventario antes de tocar nada; **si una
acción falla, todo el request se revierte** (`*inventory = snapshot`)
y la respuesta es `STATUS_ERROR` (sin `changed`/`dropped`). Esto es
el port de `Player::handleItemStackRequest`.

| Acción | Qué hace |
|---|---|
| `Take { slot, count }` | Mueve hasta `count` del slot al cursor. Falla si el cursor lleva otro ítem o superaría `max_stack_size`. |
| `Place { slot, count }` | Deja hasta `count` del cursor en el slot. Falla si difieren los ítems o supera la pila máxima. |
| `Swap { slot_a, slot_b }` | Intercambia dos slots de cualquier ventana modelada. |
| `Drop { slot, count }` | Quita ítems del slot (tecla Q) vía `drop_from_slot`; la pila va a `dropped`. |
| `Destroy { slot, count }` | Borra ítems del slot (desaparecen: borrado creativo). |
| `CreativeCreate { item, count }` | Solo si `is_creative` (la sesión pasa `gamemode == 1`): añade con `add_item`, recortado a la pila máxima; si no cabe, falla. |
| `CraftRecipe { recipe_id, count }` | Crafteo desde el libro de recetas (abajo). |

`CraftRecipe`: `mgr.recipe_by_id(recipe_id)` resuelve por id exacto
(`minecraft:oak_planks#123`) o por nombre de resultado
(`minecraft:oak_planks`). Para shapeless consume cada ingrediente
una vez por unidad producida desde el inventario principal
(`consume_ingredient`); para shaped consume una de cada celda no
vacía de la forma. El resultado se multiplica por `count` y debe
caber entero con `add_item`, o el request entero falla.

Respuesta (`session.rs::handle_item_stack_request`):

1. `ItemStackResponsePacket` (0x94) vía
   `packets_wire::item_stack_response_wire` (una entrada por `request_id`).
2. Un `InventorySlotPacket` (0x32) por cada slot en `changed`.
3. `MobEquipmentPacket` (0x1F) con el ítem en mano, para los demás
   espectadores.

Nota: `outcome.dropped` hoy se descarta en la sesión
(`let _ = outcome.dropped`): el drop actualiza el inventario pero
todavía no spawnea la entidad de ítem para plugins.

### La vía clásica: `InventoryTransactionPacket` (0x1E)

`PlayerInventory::apply_actions(&[NetworkInventoryAction]) ->
AppliedTransaction { changed, rejected }` replica
`InventoryTransaction::execute` de PocketMine: cada acción debe
describir el slot **exactamente como el servidor lo tiene**
(`same_item`: `network_id`, `meta`, `count` y `block_runtime_id`;
el id de pila de red y el NBT se ignoran, como en PocketMine).
Si alguna difiere, o alguna acción es `SOURCE_CREATIVE`, o el
`window_id` es negativo/desconocido, la transacción entera se
rechaza sin tocar nada. Las demás fuentes (drops, pickups, ventanas
de cliente) todavía no se modelan y se ignoran.

La sesión (`handle_inventory_transaction`) rechaza además
transacciones con más de 50 acciones y las de tipo `Mismatch`, y en
cualquier rechazo devuelve `resync_inventory()`: un
`InventoryContentPacket` por cada ventana modelada (0, 120, 119, 124).

## 6. `ContainerOpen` / `ContainerClose` (0x2E / 0x2F)

```rust
// session.rs
pub struct OpenContainer {
    pub window_id: u8,       // 10 = mesa de crafteo, 16 = yunque (BDS)
    pub name: String,        // "minecraft:crafting_table" | "minecraft:anvil"
    pub position: (i32, i32, i32),
    pub unique_id: u32,      // 10_000 + session_id % 10_000 (determinista por sesión)
    pub kind: String,        // "crafting_table" | "anvil"
}
```

- `open_crafting_table(pos)` / `open_anvil(pos)`: guardan el
  `OpenContainer` y emiten `ContainerOpenPacket` (0x2E) con
  `packets_wire::container_open_wire`.
- El cliente responde con `ContainerClosePacket` (0x2F), despachado
  en el bucle de sesión a `handle_container_close`: las entradas de
  la mesa o del yunque (`close_drops()`) se devuelven al inventario
  principal con `add_item` (el resto que no cabe se registra en log),
  se reenvía el inventario y se confirma con `container_close_wire`
  (`server_initiated = false`).
- `InteractPacket` (0x21) con `OPEN_INVENTORY` simplemente
  re-sincroniza la pantalla de inventario del jugador.

Solo existen dos contenedores dinámicos (`crafting_table`, `anvil`):
no hay cofres ni ventanas genéricas de plugin todavía.

## 7. `CreativeContent` (0x91) y `CraftingData` (0x34)

Ambos se envían una vez, al spawn (pasos 8c de la secuencia de
spawn de `session.rs`), generados desde los datos embebidos.

**Catálogo creativo** (`crafting.rs`, `session::creative_content_wire`):

```rust
creative_items()  -> Vec<ItemStack>   // data/creative/*.json, deduplicados por (network_id, meta), count = max_stack_size
creative_groups() -> Vec<CreativeGroup>  // group_name + group_icon (fallback "minecraft:stick")
```

**Libro de recetas** (`session::crafting_data_wire`): una entrada
`CraftingRecipeEntry` por receta shaped (`KIND_SHAPED`) y shapeless
(`KIND_SHAPELESS`) del manager, con sus inputs como pares
`(network_id, meta)`:

- `Exact` → su `(id, meta)`; `MetaWildcard` → `(id, 32767)`.
- `TagWildcard` → un sentinel negativo estable
  (`packets_wire::tag_network_id`: hash FNV-1a del tag, en
  `-1..-2000`) que el cliente ignora para el matching pero que sirve
  para distinguir tags.

## 8. Yunque (`anvil.rs`)

El yunque es el segundo contenedor dinámico y tiene además estado de
mundo: daño del bloque y reparación de ítems.

- `AnvilState { damage: 0..=3 }`, `damage(roll)` con `roll` en
  `0..100` aportado por el llamante (determinista en tests):
  probabilidad de romperse por etapa `5% / 12% / 25% / 100%`
  (`break_chance`); al subir de etapa, `damage + 1` hasta 3.
  La sesión lo aplica en `damage_anvil_at`, con el evento
  cancelable `AnvilDamageEvent` si algún plugin escucha.
- Durabilidad en `ItemStack.meta` (bits bajos = daño, bits altos =
  `prior_work`, el coste de trabajos previos). `max_durability`
  cubre herramientas y armaduras de madera a netherita.
- `anvil_combine(left, right) -> Option<AnvilRepair>`: solo el mismo
  `network_id` y con daño > 0. Cada ítem material restaura 25% de la
  durabilidad máxima + 5% extra; el coste es base + penalización
  `2 * (prior_left + prior_right + 1)`, y `cost > 40`
  (`MAX_REPAIR_LEVEL`) es "demasiado caro" → `None`.
- `apply_repair(inventory, left_index, right_index, repair, xp_level)`:
  no hace nada si el jugador no tiene XP suficiente; si no, deduce el
  coste, consume ambas entradas y añade el resultado.
- `AnvilInventory { left, right, result, cost }`: `update_result()`
  recalcula desde las dos entradas, `take_result()` consume ambas y
  devuelve `(result, cost)`, `close_drops()` devuelve las entradas
  al cerrar. Se abre con ventana 16 (`minecraft:anvil`).

## 9. Lado PHP y contrato de plugins

El puente PHP expone `pocketmine\inventory\PlayerInventory`
(`crates/php-plugin-bridge/php/player.php`): copia **en memoria** de
36 slots con `getItem`/`setItem`/`addItem`/`removeItem`/`first`/
`firstEmpty`/`getContents`/`setContents`/`clear`/`clearAll`/
`getItemInHand` (devuelve el slot 0, sin hotbar seleccionable).
`Player::getInventory()`, `getCursorInventory()` y
`getCraftingGrid()` devuelven estos stubs (marcados `TODO(A1)`).

Consecuencias prácticas, tal como las recoge
`docs/plugin-api-contract.md`:

- No existe acción de plugin `set_inventory`: el inventario
  autoritativo es el de Rust. `/give` y `/clear` son comandos
  nativos del núcleo (`command.rs`), no acciones del bridge.
- "Inventarios, entidades, efectos y encantamientos son stubs en
  memoria" es la descripción del lado PHP en el milestone 1; en
  Rust, inventario, crafteo, ventanas y transacciones sí están
  implementados y validados (§1–§7).

## 10. Verificación

```powershell
cargo test -p server-core inventory:: crafting:: anvil:: packets_wire::
```

Tests que fijan el contrato (nombres reales del código):

- `inventory.rs`: `adds_and_stacks_items` (herramientas no apilan),
  `reports_leftovers_when_the_inventory_is_full`,
  `clear_matching_follows_the_vanilla_rules`,
  `clear_reaches_the_other_player_windows`,
  `applies_matching_transactions_and_rejects_stale_ones`,
  `creative_actions_force_a_resync`,
  `armor_equip_validates_piece_type`, `cursor_and_crafting_slots_roundtrip`.
- `crafting.rs`: `manager_loads_bedrock_recipes` (> 800 recetas),
  `crafts_oak_planks_in_2x2`, `crafts_crafting_table_in_3x3`,
  `shapeless_redye_matches_any_layout`, `shaped_bread_in_a_row`,
  `take_place_swap_move_items_with_cursor`, `drop_removes_items_and_reports_them`,
  `creative_create_adds_items_for_creative_players`.
- `anvil.rs`: `anvil_takes_damage_and_breaks`,
  `repairs_damaged_sword_with_second_sword`,
  `too_expensive_combinations_are_rejected`,
  `apply_repair_deducts_xp_and_consumes_inputs`.
