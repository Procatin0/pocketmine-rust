# PocketMine-Rust — Matriz de compatibilidad de clases (PM5)

Última revisión: 2026-11-17
Referencia normativa: `target/axolotl-pm` (PocketMine-MP 5 / axolotl, branch `stable`)
Alcance: **clases** de PocketMine-MP 5 y su port, en dos planos:

- **Nativo** — módulo Rust de `crates/server-core`, `crates/bedrock-world`,
  `crates/bedrock-protocol`.
- **API** — clase PHP equivalente en `crates/php-plugin-bridge/php/`, tal como la
  ve un plugin (`pocketmine\entity\Entity`, `pocketmine\block\Block`, ...).

Leyenda: 🟢 completo · 🟡 parcial/stub documentado · 🔴 ausente o desconectado.

> Este documento es la matriz que `HANDOFF.md` daba por existente y que faltaba en
> el repo. Se actualiza al cerrar cada clase.

---

## 0. Resumen ejecutivo

| Plano | Estado |
|---|---|
| Nativo | Entidades, bloques con estado, inventario, crafteo, efectos, comandos, permisos, scoreboard y bossbar tienen port real. |
| API PHP | Chat, comandos, permisos, scheduler, config, NBT, matemáticas, scoreboard, bossbar, entidades, inventario, efectos, encantamientos, formularios y crafteo están conectados al motor. |

**Los tres agujeros estructurales de la matriz inicial están cerrados:**

1. **`Entity` PHP ↔ entidades nativas** — fase 1: host calls `entity.*`,
   `EntityFactory::create()` devuelve un `EntityProxy` hidratado del mundo.
2. **`PlayerInventory` PHP ↔ `inventory.rs`** — fase 4: proxy con snapshot y
   cola de ediciones; desde la fase 7b el NBT viaja en ambos sentidos.
3. **Formularios** — fase 8: `Form`/`SimpleForm`/`CustomForm`/`ModalForm`,
   `ModalFormRequestPacket` (0x64) / `ModalFormResponsePacket` (0x65) y
   `Form::handleResponse()` por IPC.

**Hueco transversal que queda:** el PHP declara **48** clases `*Event` y Rust
despacha **27**. Entidad completa (`EntitySpawnEvent`, `EntityDamageEvent`,
`EntityDamageByEntityEvent`, `EntityDamageByChildEntityEvent`,
`EntityDeathEvent`, `EntityDespawnEvent`, `EntityExplodeEvent`), mundo
(`WorldSaveEvent`, `SpawnChangeEvent`, `WorldDifficultyChangeEvent`,
`ChunkLoadEvent`, `WorldSoundEvent`, `WorldParticleEvent`) y bloque con
mecánica real (`BlockSpreadEvent`, `BlockFormEvent`) ya se disparan; siguen sin
emisión los de bloque sin mecánica o sin cancelación previa
(`BlockGrowEvent`, `LeavesDecayEvent`, `BlockBurnEvent`, ...), ver §4.16/§4.18.

**Límites estructurales restantes (documentados en §4):** no hay
`MobEffectPacket`, así que los efectos son autoritativos pero no se dibujan;
los efectos de jugador viven solo en PHP; `getCollisionBoxes()` no conoce
formas parciales; no hay luz por bloque ni block entities.

---

## 1. Cifras medidas

| Métrica | Valor |
|---|---|
| Clases/traits/interfaces declaradas en el API PHP | 929 (53 295 líneas, 46 archivos en php/ + 4 generados, 50 en runtime) |
| Clases PM5 en `src/` | ~3 000 |
| Módulos de paquete Bedrock implementados | 55 |
| Tipos de entidad en la tabla nativa | 70 |
| Structs de evento despachados desde Rust | 27 |
| Clases `*Event` declaradas en PHP | 48 |

Cobertura por **nombre de clase** (no por firma):

| Módulo PM5 | Cubiertas / total |
|---|---:|
| `permission/` | 9/14 |
| `scheduler/` | 11/15 |
| `plugin/` | 13/21 |
| `event/` | 48/152 |
| `entity/` | 34/77 (entidades + `effect/*` + `object/*` 4/4 + `projectile/*` 9/11) |
| `player/` | 3/14 |
| `world/` | 87/276 (+ biomas 17/17, partículas 38/38, sonidos 21/119, `format` 4/4) |
| `inventory/` | 2/34 |
| `item/` | 5/146 (+`item/enchantment/` 6/12) |
| `block/` | 402/402 (completo: 276 raíz, 39 `tile/`, 19 `inventory/`, 68 `utils/`) |
| `crafting/` | 11/23 |
| `form/` | 2/2 core (+12 de ext-forms empaquetadas) |
| `lang/` | 2/3 |

> En `item/` PM tiene una clase por item; aquí son data-driven (registro de 834
> items). En `block/`, el módulo completo ha sido migrado al 100% (402/402
> declaraciones en 9 archivos de runtime) manteniendo la jerarquía de herencia
> real de PM5 (`Air extends Flowable extends Transparent extends Block`), todos
> los traits/enums de `utils/`, tiles, inventarios de bloque y las 833
> constantes y getters de `BlockTypeIds` y `VanillaBlocks`.

Cobertura por **método público** de las clases grandes:

| Clase | Métodos PM5 | Cubiertos | Faltan |
|---|---:|---:|---:|
| `Entity` + `Living` | 146 | 28 | 118 |
| `Player` + `IPlayer` | 148 | 54 | 94 |
| `Block` | 70 | 70 | 0 |
| `World` | 141 | 85 | 56 |
| `Item` | 62 | 17 | 45 |

> `World` es el único caso con métodos propios: el `World` del bridge declara
> 99 métodos públicos, pero 14 no existen en PM5 (`tick`, `getName`,
> `getTiles`, `isDaytime`…). La cobertura se cuenta sobre la intersección:
> 85 de 141, faltan 56 (§4.22).

---

## 2. Tabla maestra por clase

| Clase PM5 | Nativo | API PHP | Estado |
|---|---|---|---|
| `entity\Entity` | ✅ `entity.rs` (1 502 líneas): AABB, `move()`, `tryChangeMovement`, gravedad/drag, `entityBaseTick`, metadata, runtime ids | ✅ `player.php:11` con host calls `entity.*` (`spawnToAll`, `kill`, `teleport`, `attack`, `setNameTag`); `EntityFactory::create` devuelve `EntityProxy` | 🟢 fase 1 |
| `entity\Living` | ✅ salud, `attack()`, invulnerabilidad, `onDeath`, loot, XP | ✅ `Living` con health/maxHealth, `getDrops()`, `getXpDropAmount()`, `knockBack()`, motion y AABB | 🟢 fase 2 |
| `entity\Human`, `Skin` | 🟡 skins de jugador en `session.rs`; `Skin` PHP stub | 🟡 | 🟡 |
| `Zombie`, `Villager`, `Squid`, `WaterAnimal` (los únicos mobs de PM5 core) | ✅ tabla nativa de 70 tipos con tamaño/salud/XP + wander | ✅ portados con firmas y drops de PM; `Ageable` y `EntitySizeInfo` incluidos | 🟢 fase 2 |
| Resto de tipos Bedrock (70) | ✅ invocables con `/summon` y por host | 🟡 `EntityFactory::create()` devuelve `EntityProxy` | 🟡 |
| `projectile\*` (Arrow, Egg, Snowball, EnderPearl, Trident, IceBomb, SplashPotion, XP bottle) | ✅ impacto bloque/entidad, daño por velocidad | ✅ `Projectile` + 8 subclases con `getResultDamage()` y velocidad inicial | 🟢 fase 12 (§4.14) |
| `object\ItemEntity` | ✅ pickup delay, despawn 6000, merge | ✅ `ItemEntity` + `World::dropItem()`/`dropItemNaturally()` reales | 🟢 fase 11 (§4.13) |
| `object\FallingBlock` | ✅ + física de arena/grava | ✅ `FallingBlock` con el id de estado del bloque | 🟢 fase 11 |
| `object\PrimedTNT` | ✅ mecha 80, radio 4; los bloques se destruyen en `apply_explosions()` tras `EntityExplodeEvent` | ✅ `PrimedTNT` con `getFuse()`/`setFuse()` | 🟢 fase 11; evento en fase 14 |
| `object\ExperienceOrb` | ✅ | ✅ `ExperienceOrb` + `World::dropExperience()` | 🟢 fase 11 |
| `entity\effect\*` (`Effect`, `EffectInstance`, `EffectCollection`, `EffectManager`, `VanillaEffects`) | ✅ `WorldState::entity_effects` + tick de expiración | ✅ `addEffect`/`removeEffect`/`hasEffect`/`getEffects` sobre `entity.*` | 🟢 fase 7b (sin paquete de cliente, §4.9) |
| `entity\AttributeFactory` / `AttributeMap` | 🟡 solo salud/tamaño | 🔴 | 🔴 |
| `block\Block` | ✅ `block_states.rs`: estados, propiedades, colisión, `interact` | ✅ 70/70 métodos públicos de PM5 cubiertos al 100% (+23 métodos bridge para compatibilidad) | 🟢 fase 21 (§4.23) |
| `block\BlockTypeIds` / `VanillaBlocks` | ✅ 833 tipos de bloque registrados | ✅ 833 constantes en `BlockTypeIds` (registro dinámico `newId`/`forName`) y 833 getters en `VanillaBlocks` + `__callStatic` (alias legacy) + `getAll()` | 🟢 fase 21 (§4.23) |
| Formas y geometría (`Transparent`, `Opaque`, `Thin`, `Wall`, `Stair`, `Slab`, `Fence`, `Rail`...) | ✅ Colisión sólida básica en core | ✅ 115 clases en `block_shapes.php`, jerarquía de herencia PM5, `recalculateCollisionBoxes()` analítico | 🟢 fase 21 (§4.23) |
| Interactivos / contenedores / redstone (`Chest`, `Door`, `Button`, `Furnace`, `Hopper`, `Bed`...) | ✅ Interacción y estado básico | ✅ 57 clases en `block_interactive.php` con contratos de activación, inventarios y señales | 🟢 fase 21 (§4.23) |
| Fluidos y fuego (`Flowable`, `Air`, `Liquid`, `Water`, `Lava`, `Fire`, `SoulFire`) | ✅ `fluid::flow_into`, colisiones, hooks | ✅ 10 clases en `block_fluids.php`, `BlockSpreadEvent`/`BlockFormEvent` integrados | 🟢 fase 21 (§4.23) |
| Plantas (`Leaves`, `Crops`, `Sapling`, `Bamboo`, `Cactus`, `Sugarcane`...) | ✅ Random tick decae/crece en core | ✅ 59 clases en `block_plants.php`, contratos `Ageable`, soporte estático | 🟢 fase 21 (§4.23) |
| `block\tile\*` (39 clases: `Tile`, `TileFactory`, `Container`, `Chest`, `Sign`...) | 🟡 No materializados en Rust (`World::getTileAt() = null`) | ✅ 39 clases en `block_tiles.php`, serialización NBT completa, mapeo bidireccional en `TileFactory` | 🟢 fase 21 (§4.23) |
| `block\inventory\*` (19 clases: `BlockInventory`, `ChestInventory`, `FurnaceInventory`...) | 🟡 Integrado con inventario en memoria | ✅ 19 clases en `block_inventory.php`, listeners y contratos PM5 | 🟢 fase 21 (§4.23) |
| `block\utils\*` (68 clases/traits/enums: `SupportType`, `DyeColor`, `WoodType`...) | — | ✅ 68 declaraciones en `block_utils.php` con enums, traits y helpers | 🟢 fase 21 (§4.23) |
| `block\BlockFactory` | ✅ | ✅ legacy↔network | 🟢 |
| `world\World` | 🟡 `world_state.rs`: 1 mundo, ediciones, chunks, hora, clima, dificultad, biomas por columna, colisiones, actualizaciones programadas | 🟢 85/141 (consultas, chunk cache, safe spawn, sonido/partículas, biomas, colisiones, vecinos) | 🟡 sin luz/multimundo/tiles/chunk-loaders (§4.17, §4.22) |
| `world\biome\*` (`Biome`, `BiomeRegistry`, 15 biomas + `UnknownBiome`) | ✅ `biome_at`/`set_biome` sobre el generador + `world.get_biome`/`set_biome` | ✅ ids de `BiomeIds`, temperatura/lluvia/elevación/ground cover | 🟢 fase 15 (§4.17) |
| `world\particle\*` (38) | ✅ `add_level_event_at` → `LevelEventPacket` | ✅ `Particle` + `ParticleBase` con `getPacketEvent()`/`encodeData()` | 🟢 fase 15 (salvo `FloatingText`) |
| `world\sound\*` (119) | ✅ `add_sound_at` → `LevelSoundEventPacket` + cola `LevelEvent` | 🟡 21/119 con `SoundBase` | 🟡 fase 15, resto sin portar |
| `world\WorldManager` | 🟡 un solo `WorldState` | 🟡 multi-mundo por nombre | 🟡 |
| `world\format\*` (`Chunk`, `SubChunk`, `HeightArray`, `ChunkException`) | ✅ Bedrock + Java | ✅ `Chunk` decodifica el payload, coordenadas, capa local y subsecciones; `HeightArray` completo | 🟢 fase 17 (§4.19) |
| `world\BlockTransaction` | ✅ `World::setBlockAt()` por bloque | ✅ `addBlock`/`addBlockAt`/`fetchBlock`/`getBlocks`/validadores/`apply()` atómico | 🟢 fase 18 (§4.20) |
| `world\Explosion` | ✅ `explosion_blocks` (preview) + `queue_explosion` (TNT y plugins) | ✅ `Explosion` con `explodeA`/`explodeB`/`explode`, `affectedBlocks`, `setFireChance` | 🟡 fase 19 (§4.21) |
| `player\Player` (+`IPlayer`) | 🟡 `session.rs`: login, movimiento, chat, vida, XP, comida, vuelo, sneak | 🟡 54/148 | 🟡 |
| `player\OfflinePlayer` | ✅ `players/*.json` | ✅ `player.load/save` | 🟢 |
| `player\GameMode` | ✅ ids 0-3 | ✅ clase final con `SURVIVAL()`, `fromString`, `fromId`, `id()`; `Player::getGamemode(): GameMode` | 🟢 (clase, no `enum`) |
| `PlayerInventory` / `inventory\*` | ✅ `inventory.rs` + transacciones atómicas + `ItemStackRequest` | 🔴 array privado en memoria | 🔴 doble fuente de verdad |
| `inventory\InventoryTransaction` | ✅ | 🔴 | 🔴 |
| `item\Item` / `ItemFactory` | ✅ registro 834 + NBT de red | 🟡 17/62 + NBT (`ench`, `display`) en ida y vuelta | 🟡 |
| `item\enchantment\*` (`Enchantment`, `EnchantmentInstance`, `VanillaEnchantments`) | ✅ ids Bedrock por NBT | ✅ `Item::addEnchantment`/`getEnchantments` sobre `ench` | 🟢 fase 7b |
| `crafting\CraftingManager` | ✅ `crafting.rs` (2 261) | ✅ proxy sobre `crafting.*` (`getCraftingManager()` real) | 🟢 fase 9 |
| `event\*` (152) | 🟡 27 despachados (12 previos + entidad/daño + mundo + bloque/chunk + sonido/partícula) | 🟡 48 declarados | 🟡 ver §4.5 |
| `event\world\WorldSoundEvent`, `WorldParticleEvent` | ✅ cola `PendingWorldEffect` drenada en el tick + `dispatch_event_sync`; cancelar evita el packet y `setSound`/`setParticle`/`setRecipients` cambian lo enviado | ✅ clases con `getSound()`/`getParticle()`/`getPosition()`/`getRecipients()` sobre `WireSound`/`WireParticle` | 🟢 fase 16 (§4.18) |
| `event\world\*` (`WorldSaveEvent`, `SpawnChangeEvent`, `WorldDifficultyChangeEvent`, `ChunkLoadEvent` + base `WorldEvent`/`ChunkEvent`) | ✅ cola `PendingWorldEvent` drenada en el tick; `ChunkLoadEvent` una vez por columna | ✅ clases con `getWorld()`/`getPreviousSpawn()`/`getOldDifficulty()`/`getChunk()` e `EventFactory` | 🟢 fases 13-14 (§4.15/§4.16) |
| `event\block\*` de cambio (`BlockSpreadEvent`, `BlockFormEvent`) | ✅ hook `BlockEventHook` consultado por `fluid::flow_into`/`liquid_collide` antes de escribir | ✅ `WorldBlockChangeEvent` + `getSource()`/`getCausingBlock()`/`getNewState()` sobre `Block::fromStateId()` | 🟢 fase 14 (§4.16) |
| `event\entity\EntityExplodeEvent` | ✅ TNT (`explode_into`) y `Explosion` de PHP (`world.explode`) encolan; `apply_explosions()` dispara antes de destruir | ✅ `getEntity`/`getPosition`/`getBlockList`/`setBlockList`/`getYield`/`setYield` | 🟢 fase 19 (§4.21) |
| `event\block\BlockGrowEvent`, `LeavesDecayEvent` | 🟡 el random tick crece/decae sin evento previo (`TickScene::set_block`) | 🔴 sin clase PHP ni struct Rust | 🟡 siguiente paso (§4.16) |
| `command\*` (53) | ✅ 49 comandos + framework | ✅ `Command`, `SimpleCommand`, `ClosureCommand`, `PluginCommand`, `CommandMap`, `VanillaCommand` | 🟢 |
| `permission\*` (14) | ✅ nodos + attachments | ✅ 9/14 | 🟢 |
| `scheduler\*` (15) | ✅ tick desde Rust | ✅ 11/15 (`AsyncPool` sin pool real) | 🟢/🟡 |
| `plugin\*` (22) | ✅ manager, orden topológico, `.phar` | ✅ 13/21 | 🟢 |
| `form\Form` (+ ext-forms) | ✅ `ModalFormRequestPacket` 0x64 / `ModalFormResponsePacket` 0x65 | ✅ `Form`, `SimpleForm`, `CustomForm`, `ModalForm`, `Button` y 6 elementos | 🟢 fase 8 |
| `Server`, `Logger`, `Config`, `TextFormat`, `Utils` | — | ✅ / 🟡 (`Utils` 6 de ~30) | 🟢/🟡 |
| `math\Vector3/Vector2/AABB`, `Position`, `Location` | ✅ `Vector3f` + `Aabb` | ✅ completos | 🟢 |
| `nbt\*` | ✅ NBT de red | ✅ árbol + serializadores | 🟢 |
| `scoreboard`, `apiBossBar` (plugins externos) | ✅ ambos lados | ✅ | 🟢 |
| `resourcepacks\*`, `lang\*`, `promise\*`, `thread\*`, `data\*`, `timings` | 🟡 handshake / `translations.rs` / crash / metrics | 🟡 `thread.php`, `Language` parciales | 🟡 |

---

## 3. Host calls PHP → Rust (estado real)

`crates/server-core/src/plugin_host.rs` implementa:

- `world.*`: `get_block`, `set_block`, `take_block_changes`, `drain_changes`,
  `get_highest_block`, `get_spawn`, `set_spawn` (fase 13, desde
  `World::setSpawnLocation()`), `get_time`, `get_time_running`,
  `set_time`, `set_time_running`, `get_difficulty`, `set_difficulty`,
  `get_seed`, `save`, `generate`, `get_auto_save`, `set_auto_save`,
  `get_auto_save_interval`, `set_auto_save_interval`, `do_auto_save`,
  `tick`, `get_chunk` y desde la fase 15 `add_sound` (nombre de
  `LevelSoundEvent` o `event_id` de `LevelEvent`), `add_particle`
  (`LevelEventPacket`), `get_biome` y `set_biome`.
- Eventos del tick del mundo (fases 13-14): `WorldSaveEvent`,
  `SpawnChangeEvent` y `WorldDifficultyChangeEvent` se **encolan** en
  `WorldState::pending_world_events` (una host call no puede despachar: el
  runtime PHP está dentro de la llamada) y `SessionManager` los entrega en el
  tick; `BlockSpreadEvent`/`BlockFormEvent` y `EntityExplodeEvent` sí se
  despachan en línea, desde el hook `BlockEventHook` y desde
  `apply_explosions()`.
- `player.*`: `get`, `load`, `save`, `save_data`, `has_data`,
  `has_played_before`, `get_by_uuid`, `get_by_raw_uuid`, `update_health`.
- `server.*`: `get_players`, `get_players_full`, `get_max_players`, `get_motd`,
  `is_op`, `has_permission`, `get_query_info`, `save_all`, `set_op`.
- `entity.*`: `spawn`, `get`, `list`, `despawn`/`remove`, `teleport`,
  `set_health`, `damage`, `set_name_tag`, `set_motion`, `knock_back`, desde
  la fase 7b `effects`, `add_effect`, `remove_effect`, `clear_effects` y desde
  las fases 11-12 `spawn_item`, `spawn_falling_block`, `spawn_tnt`,
  `spawn_xp_orb`, `spawn_projectile`.
- `item.id_by_name`: resuelve `minecraft:rotten_flesh` & co. contra
  `data/item_ids.txt` (necesario para `getDrops()`, que no son bloques).
- `inventory.*` (fase 4): `get` (snapshot de las 4 ventanas), `set_slot`
  (valida ventana y rango) y `clear`. Las escrituras se **encolan** y el tick
  las aplica al `PlayerInventory` autoritativo, reenviando
  `InventoryContentPacket` de las ventanas tocadas.
- `block.network_id`, `block.state` (fase 5: incluye `id`, `solid` y `meta`
  legacy, `null` cuando BDS no la tiene) y `block.break_info` (`breakable`,
  `hardness`, `instant`, `bare_hand`, `seconds`).
- `inventory.get` / `inventory.set_slot` (fase 7b): el stack lleva `nbt` en
  hexadecimal en ambos sentidos, así que encantamientos, `display.Name` y
  `display.Lore` cruzan el puente.
- `crafting.*` (fase 9): `recipes` (índice completo con resultado,
  ingredientes y forma), `match` (rejilla 2x2/3x3 → resultado),
  `furnace` (bloque + item → fundido), `brewing`, `register_shaped` y
  `register_shapeless` (recetas de plugin, consultadas antes que las nativas).

**Formularios** no usan host call: `Player::sendForm()` emite la acción
`send_form` (`PluginAction::SendForm`), el núcleo manda
`ModalFormRequestPacket` (0x64) y la respuesta 0x65 vuelve por el IPC
`form_response` a `Form::handleResponse()`.

**Ausentes por completo:** `scoreboard.*` (el scoreboard PHP es local y se
sincroniza por otro camino) y `tile.*`.

---

## 4. Detalle de los huecos

### 4.1 `Entity` de PHP ↔ `entity.rs` — ✅ resuelto en la fase 1

`plugin_host.rs` implementa `entity.spawn|get|list|despawn|teleport|set_health|
damage|set_name_tag` sobre `WorldState` (`spawn_mob_configured`, `entity_by_id`,
`set_entity_position`, `set_entity_health`, `set_entity_name_tag`,
`damage_entity`, `remove_entity`).

En PHP, `Entity` gana `getNetworkTypeId()`, `applyNativeSnapshot()`,
`spawnToAll()`/`spawnTo()`, `kill()`, `teleport()`, `attack()`, `setNameTag()`,
`setHealth()` conectados al host, y `EntityFactory::create()` devuelve
`EntityProxy`. La difusión a clientes la sigue haciendo
`SessionManager::sync_entities()` (spawn/despawn/movimiento por chunk), sin
paquetes nuevos.

Queda para la fase 2: `setMotion`/`getMotion`, `getBoundingBox`, `getDrops`,
`getDirectionVector`, `despawnFrom`, y una clase PHP por tipo de mob.

### 4.2 Inventario: dos fuentes de verdad — ✅ resuelto en la fase 4

`PlayerInventory` (`player.php`) ya no es una lista local: lee el inventario
autoritativo (`inventory.get`) y escribe por `inventory.set_slot` /
`inventory.clear` (ver §4.4).

### 4.3 Formularios — ✅ resuelto en la fase 8

`Form`, `SimpleForm`, `CustomForm`, `ModalForm` y los elementos existen en
`php/form.php`, con los paquetes 0x64/0x65 y el callback de respuesta (ver §4.10).

### 4.4 `PlayerInventory` — ✅ resuelto en la fase 4

`PlayerInventory` dejó de ser una lista local: lee el inventario autoritativo
(`inventory.get`, snapshot de las ventanas 0/120/119/124) y escribe por
`inventory.set_slot` / `inventory.clear`. `SessionManager` drena la cola al
inicio del tick, aplica con `set_window_slot`/`clear`, reenvía las ventanas
tocadas y republica el snapshot.

Límites conocidos:

- El NBT de los stacks no viaja (solo id/meta/count): un item con nombre o
  encantamientos vuelve sin ellos al leerlo desde PHP.
- `getItemInHand()` usa el slot 0; el slot de hotbar seleccionado aún no se
  expone por el puente.
- `enderInventory`, `cursorInventory`, `craftingGrid` y `creativeInventory`
  siguen siendo locales (sus nombres llevan `#` y el proxy los ignora).
- Los snapshots se republican cada 20 ticks (1 s) además de tras cada edición:
  un cambio del cliente se ve en PHP con hasta 1 s de retraso, salvo que el
  plugin llame a `refresh()`.
- `addItem()`/`removeItem()` resuelven apilado y sobrante en PHP y empujan cada
  slot tocado; no hay host call atómico todavía.

### 4.5 Eventos declarados que nunca se disparan

**Fase 3 (2026-11-17):** los eventos de entidad ya tienen contrato completo
(`fromArray`/`toArray` + alta en `EventFactory`) y se despachan:

| Evento | Cuándo | Cancelable |
|---|---|---|
| `EntitySpawnEvent` | cola de `WorldState::spawn_entity`, drenada por tick | no |
| `EntityDamageByEntityEvent` | golpe melee, **antes** de aplicar daño | sí |
| `EntityDeathEvent` | `Living::onDeath()` del tick, con drops y XP | no (como PM) |
| `EntityDamageByEntityEvent` (proyectil) | cola `PendingProjectileDamage`, resuelta al inicio del tick siguiente | sí |

El impacto de proyectil **no se aplica** dentro de `tick_entities`: el mundo
solo encola (`PendingProjectileDamage`) y `SessionManager` decide el evento
antes de `tick_entities()`, así que cancelar deja al mob intacto (sin loot ni
animación). El `damager` es el jugador que disparó (si sigue conectado).

**Fase 10 (2026-11-17):** la familia de daño está completa y el despawn se
dispara.

| Evento | Cuándo | Cancelable |
|---|---|---|
| `EntityDamageEvent` (base) | antes de cada golpe, con la causa (`CAUSE_FALL`, `CAUSE_PROJECTILE`, ...) | sí |
| `EntityDamageByChildEntityEvent` | impacto de proyectil: `damager` = tirador, `child` = el proyectil | sí |
| `EntityDespawnEvent` | la entidad sale del mundo sin morir (`entity.despawn`, despawn del tick) | no |

La cadena se reproduce explícitamente porque el puente despacha por nombre
exacto: primero `EntityDamageEvent` (los listeners de la clase padre, que en PM
reciben la instancia derivada por `instanceof`), después
`EntityDamageByEntityEvent` / `EntityDamageByChildEntityEvent` con el daño que
dejó el primer evento. El valor devuelto en `final_damage` es el que se aplica.

Límites: un listener de `EntityDamageEvent` no ve los campos del evento
derivado (`getDamager()`/`getChild()` no existen en su clase), y `Living::attack()`
llamado desde PHP (`$entity->attack($event)`) no re-despacha al host.

Los eventos de mundo con un punto de disparo real ya se emiten (fase 13, ver
§4.15): `WorldSaveEvent`, `SpawnChangeEvent` y `WorldDifficultyChangeEvent`.
Los que tienen mecánica real ya se emiten (fase 14, ver §4.16):
`BlockSpreadEvent` y `BlockFormEvent` (fluidos), `EntityExplodeEvent` (TNT) y
`ChunkLoadEvent` (generación de columnas). El resto sigue sin dispararse porque
la mecánica no existe en el core (fuego, hoppers, yunques, brewing, campfire,
placas de presión, signos, geysers) o porque el random tick aplica el cambio sin
punto de cancelación previo (`BlockGrow`, `LeavesDecay`).

### 4.6 `World` con entidades — ✅ resuelto en la fase 6

`getEntities()` hidrata el snapshot nativo (`entity.list`) y suma los
jugadores del mundo; `getEntity(int)` indexa por id; `getNearbyEntities()`
filtra por intersección de `AxisAlignedBB` excluyendo la entidad indicada;
`addEntity()` es `spawnToAll()` (`entity.spawn`) y `removeEntity()` cierra la
entidad. Todo sobre host calls que ya existían: no hizo falta Rust nuevo.

Sigue faltando `getBlockLightAt()` y los tiles (`getTileAt()` devuelve
`null`), porque el core no guarda luz por bloque ni block entities.

### 4.7 `Block` — ✅ resuelto en la fase 5

`block.state` ahora devuelve `id`, `name`, `properties` y `solid` (de
`block_states::has_collision`, la misma tabla que usa el mundo), y hay un
host call nuevo `block.break_info` alimentado por `crates/server-core/src/
block_info.rs` (la tabla que antes vivía dentro del handler de minado, ahora
compartida por los dos caminos).

En PHP:

- `getProperties()` / `hasProperty()` / `getProperty()` leen del host y se
  cachean por id de estado.
- `isSolid()` consulta la paleta en vez de devolver `true` (aire sigue siendo
  no sólido por su clase, así que `Air`/`UnknownBlock` no cambian).
- `getCollisionBoxes()` devuelve el cubo completo para un bloque sólido y la
  lista vacía para uno no sólido.
- `asItem()` resuelve la forma de item por nombre (`ItemFactory`).
- `getBreakInfo()` construye `BlockBreakInfo` real: `instant()` con dureza 0,
  `byHand(hardness)` para el resto e `indestructible()` para aire/bedrock/agua.
  `isBreakable()` e `isInstantBreak()` ahora coinciden con PM (dureza `-1` no
  es instantánea).
- `setStateId()` valida el id contra la paleta y devuelve `false` si no existe,
  como PM; actualiza nombre y type id al aceptar.

Límites:

- `getMeta()` sale del mapa de BDS (`data/block_state_meta_map.json`, una
  entrada por estado en el orden de `canonical_block_states.nbt`) a través de
  `block.state`; `block_states::state_index()` da la posición de paleta que ese
  mapa indexa. Sigue devolviendo 0 cuando el id no está en la paleta (bloque
  custom), cuando BDS marca `-1` (sin equivalente legacy) o sin puente activo;
  y es 0 de forma legítima en bloques de un solo estado (piedra, lana, tierra),
  porque ahí la variante vive en el id y no en la meta.
- `getCollisionBoxes()` no conoce formas parciales (losas, escaleras, vallas):
  todo bloque sólido es un cubo.
- La tabla de dureza solo cubre los bloques que emite el generador; el resto
  cae en el valor de piedra, igual que el handler de minado.
- El modelo de herramientas sigue ausente, así que `getBreakInfo()` siempre
  describe la rotura a mano.

### 4.8 `GameMode` — ✅ resuelto en la fase 7

`pocketmine\player\GameMode` existe con los ids de PM (`SURVIVAL` 0,
`CREATIVE` 1, `ADVENTURE` 2, `SPECTATOR` 3), alias (`s`/`c`/`a`/`v`/`view`/+el
número), `fromString()`, `fromId()`, `id()`, `getEnglishName()`, `getAliases()`,
`equals()` y `__toString()`. `Player::getGamemode()` devuelve `GameMode` y
`setGamemode()` acepta `GameMode` o el id numérico; `isSurvival()` /
`isCreative()` / `isAdventure()` / `isSpectator()` usan las constantes.

Divergencia: PM 5 lo declara `enum` con `LegacyEnumShimTrait`. El puente usa una
clase final con constantes **y** métodos estáticos, así que
`GameMode::CREATIVE()` (lo que usan los plugins) funciona igual, pero
`GameMode::CREATIVE` no es un `case` (no hay `Match` exhaustivo ni
`instanceof` por caso).

**Efectos y encantamientos:** resueltos en la fase 7b (§4.9).

### 4.9 `Effect` / `Enchantment` — ✅ resuelto en la fase 7b

**Estado nativo.** `WorldState::entity_effects` guarda un `ActiveEffect` por
entidad (`id` Bedrock, `amplifier`, `duration`, `visible`, `ambient`,
`infinite`), con las operaciones de `EffectCollection`:
`entity_effects()`, `add_entity_effect()` (reemplaza por tipo),
`remove_entity_effect()`, `clear_entity_effects()` y `tick_entity_effects()`
(una pasada por tick desde `tick_entities()`, que descuenta duración y descarta
los expirados; `infinite` no decae). `remove_entity()` borra la colección con la
entidad, como `Entity::close()`.

**API PHP** (`php/effect.php`): `Effect`, `InstantEffect` y los efectos con
lógica real (`InstantDamageEffect`, `InstantHealthEffect`,
`RegenerationEffect`, `PoisonEffect`, `WitherEffect`, `HealthBoostEffect`), más
`SpeedEffect`, `SlownessEffect`, `LevitationEffect`, `InvisibilityEffect`,
`AbsorptionEffect`, `HungerEffect` y `SaturationEffect` con las firmas de PM
(su efecto de juego lo aplica el motor o queda documentado como inerte).
También `EffectInstance` (duración infinita con el wrap de `Limits::INT32_MAX`),
`EffectCollection`, `EffectManager`, `VanillaEffects` (27 efectos con sus ids
Bedrock), `StringToEffectParser` y `pocketmine\color\Color`.

Los ids Bedrock viven en `pocketmine\data\bedrock\effect\EffectIdMap` (como en
PM5): un `Effect` custom no registrado no tiene id y `Living::addEffect()`
devuelve `false` en vez de mentir.

**Encantamientos** (`php/enchantment.php`): `Enchantment`, `EnchantmentInstance`,
`Rarity`, `VanillaEnchantments` (39 encantamientos, ids 0-37) y
`StringToEnchantmentParser`. `Item` los guarda en el NBT `ench`
(`[{id, lvl}]`): `addEnchantment()`/`getEnchantments()`/`getEnchantmentLevel()`
leen y escriben ese tag, y `inventory.set_slot` acepta el NBT en hexadecimal,
así que un encantamiento puesto por un plugin llega al stack autoritativo y
vuelve al leerlo.

Límites:

- **El cliente no dibuja la burbuja del efecto**: `bedrock-protocol` no tiene
  `MobEffectPacket`, así que el efecto es autoritativo en el servidor (estado,
  duración y `applyEffect`) pero no se envía al cliente.
- Los efectos de **jugador** no tienen estado de sesión: `Player::addEffect()`
  los guarda en la colección PHP y los aplica si son instantáneos, pero no
  los tickea el motor. Solo las entidades nativas (`entity.*`) los persisten.
- `SpeedEffect`/`SlownessEffect`/`LevitationEffect`/`InvisibilityEffect`/
  `AbsorptionEffect`/`HungerEffect`/`SaturationEffect` no cambian la física, el
  metadata ni la comida: el motor no lee efectos al moverse/tickear.
- `InstantDamageEffect` usa `setHealth()` directamente (no hay
  `EntityDamageEvent` que cancelar).
- Un `Item` con NBT que el puente no sepa leer conserva los bytes crudos y
  viaja intacto, pero `getEnchantments()` no ve encantamientos fuera de `ench`.

### 4.10 Formularios — ✅ resuelto en la fase 8

`crates/bedrock-protocol/src/packets/modal_form.rs` implementa
`ModalFormRequestPacket` (0x64: `form_id` var_u32 + JSON) y
`ModalFormResponsePacket` (0x65: `form_id`, `data` o `null`, `cancel_reason`).

- PHP (`php/form.php`): la interfaz `Form` del core (`handleResponse`,
  `JsonSerializable`) más las clases de ext-forms (`SimpleForm`, `CustomForm`,
  `ModalForm`, `Button`, `Label`, `Input`, `Dropdown`, `Slider`, `StepSlider`,
  `Toggle`) y `FormValidationException`. `handleResponse()` valida el tipo y el
  rango de la respuesta como upstream.
- `Player::sendForm(Form)`: registra el formulario en
  `pocketmine\runtime\FormRegistry` bajo un id y emite la acción `send_form`.
- `PluginAction::SendForm` → `OutboundPacket::ModalFormRequest` al jugador.
- El 0x65 se encola en `PlayerSession::pending_form_responses` y
  `SessionManager` lo resuelve **sin el lock de sesiones**, por el IPC
  `form_response` → `Form::handleResponse($player, $data)`; las acciones que
  emita el handler se aplican en el mismo dispatch.

Límites: el formulario se guarda en un registro estático (no hay objeto de
sesión PHP), un `form_id` desconocido se ignora (0.65 tardío o plugin
recargado), y `Player::sendForm()` no valida que el jugador esté conectado.

### 4.11 `CraftingManager` — ✅ resuelto en la fase 9

`crafting.*` expone el motor nativo:

| Host call | Respuesta |
|---|---|
| `crafting.recipes` | índice completo: `shaped` (alto, ancho, celdas), `shapeless` (ingredientes) y `furnace` (bloque, ticks), con resultado y `index` |
| `crafting.match` | `{matched, result, consume}` para una rejilla 2x2/3x3 |
| `crafting.furnace` | `{matched, result, ticks}` para un bloque de fundición |
| `crafting.brewing` | `{matched, result}` siguiendo el orden de PM (container-change primero) |
| `crafting.register_shaped` / `crafting.register_shapeless` | añade una receta de plugin al motor y devuelve su `index` |

PHP (`php/crafting.php`): `CraftingRecipe`, `ShapedRecipe`, `ShapelessRecipe`,
`FurnaceRecipe`, `RecipeIngredient` + `ExactRecipeIngredient` /
`MetaWildcardRecipeIngredient`, `CraftingGrid`, `FurnaceType`,
`FurnaceRecipeManager` y `CraftingManager` (con `matchRecipe()`,
`getCraftingRecipeFromIndex()`, `registerShapedRecipe()`,
`registerShapelessRecipe()` y los callbacks de registro). `Server::getCraftingManager()`
ya no devuelve `null`.

Límites:

- El índice va **shaped, shapeless, horno, recetas de plugin**; PM registra
  shapeless antes que shaped, así que los `index` no coinciden con los de PM
  (el `ItemStackRequest` del cliente usa el suyo y no se ve afectado).
- `TagWildcard` no tiene clase PHP: una receta con tag se materializa como
  `MetaWildcardRecipeIngredient(0)`, que nunca coincide. Las recetas de plugin
  con tags necesitan la clase `TagWildcardRecipeIngredient` (pendiente).
- Las recetas de plugin guardan **un** resultado (PM acepta lista); se toma el
  primero.
- `getRecipeRegisteredCallbacks()` devuelve un `array`, no el `ObjectSet` de PM.

### 4.12 Cadena de daño y `EntityDespawnEvent` — ✅ resuelto en la fase 10

Rust (`php-plugin-bridge/src/events/entity.rs`) gana
`EntityDamageByChildEntityEvent` (entity, damager, child, cause, base/final
damage) y las 16 constantes `CAUSE_*` de PM. `SessionManager::damage_chain()`
reparte el golpe:

1. `EntityDamageEvent` a sus listeners (clase padre, como el `instanceof` de PM).
2. `EntityDamageByEntityEvent` o `EntityDamageByChildEntityEvent` con el daño
   que dejó el paso 1.
3. Si ninguno cancela, se aplica `final_damage`.

Se usa en el golpe melee, en el impacto de proyectil contra mob (el `child` es
el proyectil, capturado antes de que `despawnsOnEntityHit` lo borre), en el daño
ambiental al jugador (caída → `CAUSE_FALL`, explosión → `CAUSE_ENTITY_EXPLOSION`,
proyectil → `CAUSE_PROJECTILE`) y en `Player::attack()`.

`EntityDespawnEvent` se dispara desde `WorldState::remove_entity()` cuando la
entidad seguía viva, y desde el barrido de `closed` del tick (item que expira,
proyectil que se consume, bloque que aterriza). Una entidad que murió ya había
emitido `EntityDeathEvent` y no emite despawn, como PM.

Límites:

- El daño a un jugador por proyectil no resuelve el tirador (el mapa de sesiones
  está bloqueado en ese bucle): el evento va sin `damager`.
- `$entity->attack($event)` desde PHP (host call `entity.damage`) no dispara
  eventos: el host se ejecuta dentro de la llamada PHP y el puente no re-entra.
- El jugador herido se hidrata como `Player`; sin `Server` cargado (pruebas de
  arranque) `applyNativeSnapshot()` cae al estado del constructor.

### 4.13 Entidades objeto — ✅ resuelto en la fase 11

Host calls nuevos (todos sobre los spawners que ya existían en `world_state.rs`):

| Host call | Nativo | PHP |
|---|---|---|
| `entity.spawn_item` | `World::spawn_item()` (pickup delay, motion aleatorio, merge) | `object\ItemEntity` + `World::dropItem()` / `dropItemNaturally()` |
| `entity.spawn_falling_block` | `spawn_falling_block()` (gravedad, aterrizaje) | `object\FallingBlock` (`getBlock()`) |
| `entity.spawn_tnt` | `spawn_primed_tnt()` (mecha, explosión radio 4) | `object\PrimedTNT` (`getFuse()`/`setFuse()`) |
| `entity.spawn_xp_orb` | `spawn_experience_orb()` | `object\ExperienceOrb` + `World::dropExperience()` |

`Entity::spawnToAll()` ya no cablea `entity.spawn`: llama al hook
`spawnHostCall()`, que cada subclase sobrescribe (mob → `entity.spawn`, objeto →
su spawner, proyectil → `entity.spawn_projectile`).

Límites: el stack del `ItemEntity` viaja id/meta/count (el NBT del stack no
entra en el actor nativo, así que un item con nombre o encantamientos pierde el
NBT al mostrarse en el suelo); `getPickupDelay()` es estado PHP (el motor usa su
propio delay al recoger).

### 4.14 Proyectiles — ✅ resuelto en la fase 12

`ProjectileKind::from_network_type_id()` traduce el tipo de red al enum nativo y
`entity.spawn_projectile` crea el actor con posición, velocidad, tirador, daño y
`critical`. PHP (`php/projectile.php`): `Projectile` (tirador, `isCritical()`,
`getResultDamage()`, `getOwningEntityName()`) más `Arrow` (daño 2, velocidad 3),
`Snowball`, `Egg`, `EnderPearl`, `SplashPotion`, `ExperienceBottle`,
`ThrownTrident` (daño 8) e `IceBomb`.

La velocidad inicial sale de la dirección de mirada del tirador
(`Living::getDirectionVector()` × `getInitialSpeed()`); sin tirador el
proyectil nace quieto y el plugin debe llamar a `setMotion()`.

Límites: la velocidad de un arco cargado y la dispersión no se modelan (se usa
la velocidad fija de la clase), el `owner_runtime_id` va en 0 (el motor solo
usa el nombre del tirador), y el NBT del item (poción, flecha con efectos) no
viaja en el actor.

### 4.15 Eventos de mundo — ✅ resuelto en la fase 13

`WorldState` no conoce el gestor de plugins (lo posee `SessionManager`) y una
host call se ejecuta con el runtime PHP bloqueado, así que el puente no puede
despachar un evento desde dentro de `World`. Las mutaciones del mundo encolan el
evento (`WorldState::queue_world_event()`, enum `PendingWorldEvent`) y
`SessionManager::dispatch_world_events()` lo entrega en el tick siguiente.

| Evento | Mutación que lo encola | PHP |
|---|---|---|
| `WorldSaveEvent` | `WorldState::save()` (guardado manual, `/save-all`, autoguardado) | `world\WorldSaveEvent` |
| `SpawnChangeEvent` | `WorldState::set_spawn()` (`/setworldspawn`, `World::setSpawnLocation()` vía `world.set_spawn`) | `world\SpawnChangeEvent` (`getPreviousSpawn()`) |
| `WorldDifficultyChangeEvent` | `WorldState::set_difficulty()` (`/difficulty`, `World::setDifficulty()`) | `world\WorldDifficultyChangeEvent` (`getOldDifficulty()`/`getNewDifficulty()`) |

Los tres son **no cancelables** en PocketMine, así que aplazarlos no puede
cambiar el resultado: el listener observa el cambio, no lo impide.
`WorldState::set_spawn()` encola el evento en **cada** llamada, aunque las
coordenadas no cambien, como `World::setSpawnLocation()` (que solo compara para
poder pasar el spawn anterior); `set_difficulty()` hace lo mismo con el mismo
valor.

Límites:

- El evento llega después del cambio (PM lo dispara dentro de la mutación); solo
  cambia el orden respecto a la escritura, no el estado final.
- Cargar un mundo (`load_from`) escribe dificultad y spawn directamente, sin
  encolar: PM solo dispara desde `setDifficulty()`/`setSpawnLocation()`.
- El guardado de apagado no tiene tick siguiente, así que su `WorldSaveEvent` se
  pierde.
- `getWorld()` resuelve por nombre contra `WorldManager`, que en modo
  single-world devuelve el mundo por defecto: un `WorldSaveEvent` de otro
  nombre llega con el `World` cargado.
- Siguen sin dispararse `WorldLoad/Init/Unload`, `ChunkLoad/Populate/Unload`
  y `WorldDisplayNameChange`: no hay gancho (el chunk se genera a demanda por
  sesión, no hay registro de chunks cargados). `WorldParticle`/`WorldSound`
  pasaron a dispararse en la fase 16 (§4.18).
- En bloque no se añadió nada en esta fase: `BlockBreak`/`BlockPlace` ya se
  disparaban; los de mecánica real llegan en la fase 14 (§4.16).

### 4.16 Eventos de bloque y de chunk — ✅ resuelto en la fase 14

**Fluidos — `BlockSpreadEvent` y `BlockFormEvent`.** `WorldState` expone un
`BlockEventHook` (`spread`/`form`) que `SessionManager` instala en
`set_plugin_manager()` sobre el mismo slot de plugins. `fluid::flow_into()` y
`fluid::liquid_collide()` lo consultan **antes** de escribir y sin ningún lock
del mundo tomado, así que un listener puede llamar de vuelta al mundo
(`$world->setBlock()`) sin bloquearse, y cancelar deja la celda intacta como
`Liquid::flowIntoBlock()` / `BlockEventHelper::form()`.

**TNT — `EntityExplodeEvent`.** `explode_into()` ya no destruye bloques: filtra
los resistentes a la explosión y encola `Explosion { entity, blocks, radius }`.
`SessionManager::apply_explosions()` dispara el evento (orden de PM: impacto
calculado, `explodeB()` dispara antes de destruir), aplica la lista y el `yield`
que devuelva el listener, y solo entonces `WorldState::apply_explosion_blocks()`
destruye los supervivientes y suelta el `yield`% como items. Un evento cancelado
no destruye nada ni hace daño, como `Explosion::explodeB()`.

**Generación de chunks — `ChunkLoadEvent`.** Se dispara una vez por columna, la
primera vez que se envía a un jugador (`SessionManager::loaded_chunks`), no por
viewer. Este port no tiene almacenamiento de chunks: toda columna la genera el
generador bajo demanda, así que `isNewChunk()` es siempre `true`.

| Evento (PHP) | Getters portados | Notas |
|---|---|---|
| `entity\EntityExplodeEvent` | `getEntity`, `getPosition`, `getBlockList`/`setBlockList`, `getYield`/`setYield`, `getIgnitions`/`setIgnitions` | `yield` por defecto `1/radius*100` (25 con radio 4); `ignitions` siempre vacío (el core no crea fuego) |
| `block\BlockSpreadEvent` | `getSource`, `getSourceId`, `getBlock`, `getNewState`, `getBlockId`, coords, `getWorld` | agua/lava que fluyen a una celda libre |
| `block\BlockFormEvent` | `getCausingBlock`, `getCausingBlockId`, `getBlock`, `getNewState`, coords, `getWorld` | lava que endurece al tocar agua (piedra/cobblestone/obsidiana) |
| `world\ChunkLoadEvent` | `getChunkX`, `getChunkZ`, `isNewChunk`, `getChunk`, `getWorld` | `getChunk()` pide la columna al host la primera vez; sin host, columna vacía |

Límites:

- Los eventos de bloque nuevos **no** extienden `BlockEvent(Player|Block)` (no
  hay jugador en el tick del mundo): extienden `Event` con coordenadas y
  reconstruyen los `Block` en `getBlock()`/`getNewState()` con
  `Block::fromStateId()`; `getWorld()` resuelve por nombre contra
  `WorldManager`, que en single-world devuelve el mundo por defecto.
- `BlockFormEvent::getCausingBlock()` se aproxima con el bloque reemplazado: el
  fluido que colisiona no llega hasta la host call de la colisión.
- El tick del mundo solo dispara eventos mientras haya un `PluginManager`
  instalado (`set_plugin_manager`); sin plugins nada cambia.
- `explode_into()` captura el snapshot del `PrimedTNT` **antes** de que la
  entidad salga del mundo, para que `getEntity()` lo vea vivo.
- Siguen sin dispararse (motivo): `BlockGrowEvent` y `LeavesDecayEvent` —ni
  siquiera declarados todavía— porque el random tick crece/decae con
  `TickScene::set_block` sin evento previo; falta el mismo tratamiento que
  fluidos, `BlockBurnEvent`/`BlockMeltEvent` (no hay
  fuego ni nieve), `BlockItemPickupEvent` (no hay hoppers), `BlockExplodeEvent`
  y `BlockPreExplodeEvent` (en PM 5 los dispara `RespawnAnchor`, no la TNT),
  `SignChangeEvent` (no se decodifica la edición de carteles),
  `PressurePlateUpdateEvent`/`ChestPairEvent`/`FarmlandHydrationChangeEvent`/
  `GeyserEruptionPulseEvent`/`StructureGrowEvent`/`BlockTeleportEvent`/
  `BlockDeathEvent` (sin mecánica), `ChunkPopulateEvent`/`ChunkUnloadEvent`
  (no hay cola de pobladores ni descarga de columnas), `WorldLoad/Init/Unload`
  (el mundo se carga antes que los plugins) y `WorldDisplayNameChange`
  (nombre local). `WorldParticleEvent`/`WorldSoundEvent` se disparan desde la
  fase 16 con cola + hook (§4.18).

---

### 4.17 Módulo `world`: sonidos, partículas, biomas y consultas — ✅ fase 15

**Sonidos (`world\sound`)** — `Sound` con la misma interfaz
`encode(Vector3): array`, más `SoundBase` que expone la representación de red:
`getSoundName()` (`LevelSoundEventPacket`), `getEventId()`
(`LevelEventPacket`) y `getExtraData()` (id de bloque o `pitch * 1000`).
Portados 20 sonidos del core: `BlockBreak`/`BlockPlace`/`BlockPunch`,
`ChestOpen`/`ChestClose`, `Explode`, `BowShoot`, `ArrowHit`, `Burp`,
`Click`, `Door`, `Fizz`, `Pop`, `AnvilBreak`/`AnvilUse`/`AnvilFall`,
`BlazeShoot`, `EndermanTeleport`, `GhastShoot`. `World::addSound()` llama a
`world.add_sound` y el tick emite el packet en la **posición exacta**
(`add_sound_at()`), sin el `+0.5` de los sonidos de bloque del core.

**Partículas (`world\particle`)** — `Particle` + `ParticleBase` con
`getPacketEvent()` (id de `LevelEventPacket`, ya con `ADD_PARTICLE_MASK` para
las estándar) y `encodeData()`. Portadas 37: todas las estándar de PM más
`BlockBreak`, `BlockPunch`, `DragonEggTeleport`, `EndermanTeleport`,
`MobSpawn`, `PotionSplash`, `Terrain` e `ItemBreak`. `World::addParticle()`
→ `world.add_particle` → `LevelEventPacket` en el tick.

**Biomas (`world\biome`)** — `Biome` (id, nombre, elevación, temperatura,
lluvia, ground cover), `BiomeRegistry` (singleton) y los 15 biomas vanilla
más `GrassyBiome`/`SandyBiome`/`SnowyBiome` y `UnknownBiome`, con
`BiomeIds` en `pocketmine\data\bedrock` con el **catálogo completo** de PM5
(90 constantes, de `OCEAN = 0` a `ICE_CAVES = 196`). `World::getBiomeId()`,
`getBiome()` y `setBiomeId()` usan `world.get_biome`/`world.set_biome`; los
ids sin clase portada devuelven `UnknownBiome` (id correcto, nombre
`Unknown`).

**Consultas de `World`** — 85 de los 141 métodos públicos de PM5 (el bridge
declara 99; 14 son propios):
constantes `Y_MIN`/`Y_MAX`/`TIME_*`/`DIFFICULTY_*`, `isInWorld`,
`getMinY`/`getMaxY`, `getTimeOfDay`, `isDaytime`/`isNighttime`,
`getSunAnglePercentage`/`Radians`/`Degrees` y `computeSunAnglePercentage`,
`getFullLight*`, `getBlockLightAt`, `getPotentialLight*`,
`getRealBlockSkyLightAt`, `getBiomeId`/`getBiome`/`setBiomeId`,
`isChunkLoaded`/`isChunkGenerated`/`isChunkPopulated`/`isChunkInUse`,
`loadChunk`/`unloadChunk`/`getLoadedChunks`/`getOrLoadChunkAtPosition`,
`getChunkEntities`, `isInLoadedTerrain`, `getChunkPlayers`,
`getViewersForPosition`, `getCollidingEntities`, `getNearestEntity`,
`getTile`/`getTileAt`/`getTiles`, `getSafeSpawn()` real (baja hasta un cubo
completo y sube al primer hueco de dos bloques), `getAutoSave`/`setAutoSave`,
`getTickRateTime` e `isDoingTick`. `Chunk` ya no se descarta: `getChunk()`
lo cachea y `unloadChunk()` libera esa copia.

Límites documentados:

- `Sound::encode()`/`Particle::encode()` devuelven `[]`: el runtime PHP no
expone `ClientboundPacket`. Un `Sound`/`Particle` propio de un plugin no se
emite (solo las clases del bridge). `FloatingTextParticle` no se emite: PM
usa `SpawnParticleEffectPacket` y entidades de texto, ninguno portado.
- `$players` de `addSound()`/`addParticle()` viaja al host como nombres y
`WorldSoundEvent`/`WorldParticleEvent` lo expone; el broadcast filtra por esos
nombres (vacío = viewers del chunk, como `getViewersForPosition()`).
- El override de bioma es por **columna** (`x,z`): el payload Bedrock lleva
una paleta por subchunk y el generador solo escribe biomas 2D. `setBiomeId()`
no lanza `WorldException` en columna no generada (aquí la generación es bajo
demanda).
- Los pobladores (`Populator`, `TallGrass`) no existen y `populateChunk()` es
no-op: el generador es nativo. El ground cover de los biomas usa una posición
dummy porque todo `Block` del puente lleva posición.
- No hay motor de luz: `getBlockLightAt()` = 0 y la luz de cielo es 15/0
según `getHighestBlockAt()`, así que `getFullLightAt()` es una aproximación
solar (sin lluvia/tormenta). Tampoco hay block entities: `getTile*()` = null.
- `getViewersForPosition()` usa un radio fijo de 64 bloques porque el
`view-distance` por jugador no llega al PHP; `getMinY`/`getMaxY` son las
constantes de PM5 (`-64`/`320`).

---

### 4.18 `WorldSoundEvent` / `WorldParticleEvent` — ✅ fase 16

PM los dispara dentro de `World::addSound()`/`addParticle()` y emite el packet
en la misma llamada. Aquí `addSound()` corre dentro de una host call
PHP→Rust, así que el puente no puede re-entrar al intérprete para despachar el
evento. Se usa el patrón cola + hook:

1. PHP `World::addSound()`/`addParticle()` manda `world.add_sound` /
   `world.add_particle` con la descripción de red y los nombres de `$players`.
2. `ServerPluginHost` encola un `PendingWorldEffect` (sonido o partícula) en
   `WorldState`; no emite nada.
3. `SessionManager::dispatch_world_effects()` (cada tick, tras
   `dispatch_world_events()`) construye el evento Rust, llama a
   `PluginManager::dispatch_event_sync()` y, si no fue cancelado, hace el
   broadcast con los campos que devolvió el listener.

El evento PHP lleva un sonido/partícula "de cable" (`WireSound`/
`WireParticle`) porque un objeto `Sound`/`Particle` no cruza el IPC:
`getSound()` expone `getSoundName()`/`getEventId()`/`getExtraData()` y
`getParticle()` expone `getPacketEvent()`/`encodeData()`. `toArray()` devuelve
los campos ya editados; si un listener pone un `Sound`/`Particle` que no es del
bridge, el host no puede codificarlo y no se envía nada (PM tampoco enviaría
un `encode()` vacío).

Límites:

- El paquete sale un tick después de la llamada (PM lo envía de forma síncrona
  dentro de `addSound()`): el evento se dispara y se emite en el tick
  siguiente, no dentro del comando/tarea que llamó a `addSound()`.
- `getRecipients(): Player[]` resuelve los nombres guardados contra
  `Server::getPlayerExact()`; un jugador desconectado se omite. `setRecipients()`
  acepta `Player[]` o nombres y el broadcast filtra por nombre (vacío = viewers
  del chunk).
- El `$players` que el plugin pasa a `addSound()`/`addParticle()` se respeta
  como lista de destinatarios inicial (antes se ignoraba).

---

### 4.19 `world\format` — ✅ fase 17

`World::getChunk()` devolvía `pocketmine\world\Chunk`, que no existe en PM5
(el namespace real es `pocketmine\world\format\Chunk`): cualquier plugin con
`instanceof \pocketmine\world\format\Chunk` fallaba. Ahora el parser vive en
`world\Chunk` (compatibilidad con el bridge Java) y `format\Chunk` lo extiende
con la forma de PM5; `World::getChunk()`, `loadChunk()`,
`getOrLoadChunkAtPosition()` y `ChunkEvent::getChunk()` devuelven
`format\Chunk`.

Portado en `php/world_format.php` (38.º archivo del runtime):

- `Chunk`: `getX()`/`getZ()`/`getHeight()`/`getHeightMap()`/`getHighestBlockAt()`,
  `getSubChunk()`/`getSubChunks()` (índice `-4..19`, como PM),
  `isPopulated()`/`setPopulated()`, `getTiles()`/`getTile()`, `getChunkCoordHash()`,
  `MIN_SUBCHUNK_INDEX`/`MAX_SUBCHUNK_INDEX` y `COORD_BIT_SIZE`/`COORD_MASK`.
- `SubChunk`: `getSubChunkIndex()`, `getBlockStateId()`/`setBlockStateId()` en
  coordenadas locales, `isEmptyFast()`/`isEmptyAuthoritative()`,
  `getEmptyBlockId()`, `getHighestBlockAt()`, `getBlockLayers()`.
- `HeightArray`: `fill()`, `get()`, `set()`, `getValues()` y validación `0-15`.
- `ChunkException extends \RuntimeException`.

Límites:

- `setBlockStateId()`/`setSubChunk()` escriben en una **capa local** del objeto
  (el chunk es un snapshot): no cambian el terreno nativo. Para eso está
  `World::setBlockAt()`/`setBlockAtPosition()`. PM escribe en la columna viva y
  el provider la persiste después; aquí la separación evita host calls por
  bloque.
- `getBiomeId()` sigue siendo por subsección (el payload solo trae una paleta
  de bioma por sección), no por bloque.
- `getBlockLayers()` devuelve `[]` (sin modelo multi-capa) y `getTiles()` `[]`
  (los tiles Bedrock no se materializan como objetos PHP).
- No hay `LightArray`: el core no tiene motor de luz, así que
  `getBlockSkyLightArray()`/`getBlockLightArray()` no se portaron.

---

### 4.20 `BlockTransaction` — ✅ fase 18

PM lo usa para lotes de bloques (crecimiento, explosiones, `useItemOn`). Aquí
está portado sobre `World`:

- `addBlock(Vector3, Block)`/`addBlockAt()` acumulan por coordenada (la última
escritura gana), `fetchBlock()`/`fetchBlockAt()` leen el lote con caída al
mundo, `getBlocks()` genera `[x, y, z, Block]` como PM5.
- `addValidator(Closure)` acumula validadores; `apply()` los evalúa **antes**
de escribir y devuelve `false` sin tocar nada si alguno rechaza. Devuelve
`false` también si ningún bloque cambia (como PM).
- `dummyValidator()` replica el chequeo de posición dentro del mundo
(`Y_MIN <= y < Y_MAX`).

Límite: PM aplica sobre `ChunkManager` y aquí el tipo es `World` (no hay
interfaz `ChunkManager`); cada bloque cambiado es una host call
`Block::setStateId()`, no una escritura de chunk en lote.

---

### 4.21 `Explosion` — 🟡 fase 19

PM crea explosiones desde PHP (`new Explosion($pos, 4.0, $tnt)`) y hace el
raycast de bloques en el propio `Explosion`. Aquí se reutiliza la mecánica del
core (la de la TNT) con dos host calls nuevas:

- `world.explosion_blocks` (`Explosion::explodeA()`): devuelve la lista
  `{x,y,z,state_id}` de bloques que el estallido destruiría. PHP los guarda
  como `Block[]` en `$affectedBlocks` (clave `x:y:z`); el plugin puede borrar o
  sustituir entradas antes de `explodeB()`.
- `world.explode` (`Explosion::explodeB()`): encola el estallido con el radio
  y la lista actual. En el tick, `SessionManager::apply_explosions()` dispara
  `EntityExplodeEvent` cuando `$what` es una `Entity` (snapshot resuelto por
  `entity_id`), aplica los bloques/yield que deje el listener y daña a los
  jugadores dentro del radio.

Portado: constructor con validación de radio y `fireChance` (0-1),
`explodeA()`/`explodeB()`/`explode()`, `$source`/`$radius`/`$rays`/`$stepLen`,
`$affectedBlocks`, `getAffectedBlocks()`, `getSource()`, `getRadius()`,
`getWhat()`, `setFireChance()`/`getFireChance()` y `DEFAULT_FIRE_CHANCE`.

Límites (divergencias documentadas):

- El modelo de bloques es la **esfera del core**, no el raycast de PM: incluye
  todo bloque no aire dentro del radio salvo bedrock/barrier/obsidian/command
  blocks; no consulta la blast resistance real de cada bloque.
- El evento y el daño ocurren en el **tick siguiente** a `explodeB()`:
  `explodeB()` devuelve `true` aunque un listener cancele después (PM
  devuelve `false` de forma síncrona).
- `fireChance` se valida y se guarda, pero el core no tiene ignición por
  explosión: no se crea fuego. `getAffectedBlocks()` tampoco incluye
  `fireIgnitions`.
- `$what` como `Block` debería disparar `BlockExplodeEvent`, que no está
  portado: se acepta y el estallido se aplica sin evento.
- Sin `Entity` de origen no hay `EntityExplodeEvent` (PM tampoco lo dispara)
  y el daño a jugadores usa `CAUSE_BLOCK_EXPLOSION`.
- El core no aplica knocback a entidades ni reproduce
  `ExplodeSound`/`HugeExplodeSeedParticle` en este camino.

---

### 4.22 Auditoría de `World`: colisiones y actualizaciones — ✅ fase 20

`World.php` de PM5 tiene **141 métodos públicos**. El `World` del bridge
declara **99**, de los que **85** existen en PM5 y 14 son propios del port
(`__toString`, `addPlayer`, `dropItemNaturally`, `getEntityAt`, `getName`,
`getPlayerCount`, `getPlayerSpawnLocation`, `getTick`, `getTiles`, `isDaytime`,
`isNighttime`, `isTimeRunning`, `removePlayer` y `tick`), que no cuentan como
cobertura: faltan **56 métodos de PM5**. De los que faltaban, se completaron
los que tienen mecánica nativa real (no `generator`/`light`, ya documentados
como no portados):

- `getBlockCollisionBoxes(AxisAlignedBB)` / `getCollisionBlocks(AxisAlignedBB)`
  → host calls `world.collision_boxes` / `world.collision_blocks` sobre
  `WorldState::collision_boxes()` / `collision_blocks()`.
- `getCollisionBoxes(Entity, AxisAlignedBB, bool $entities)` → cajas del core
  más (opcionalmente) las bounding boxes de `getCollidingEntities()`.
- `scheduleDelayedBlockUpdate(Vector3, int)` → `world.schedule_block_update`
  encola en `WorldState::scheduled_block_updates` el bloque actual; el tick lo
  re-aplica.
- `notifyNeighbourBlockUpdate(Vector3)` → `world.notify_neighbour` ejecuta la
  cascada de conexiones, física (arena/grava/soporte), fluidos y redstone sin
  editar el bloque.
- Derivados de estado local: `isSpawnChunk()`, `chunkHash()`,
  `getAdjacentChunks()`, `getServer()`, `getLogger()` y
  `AxisAlignedBB::expandedCopy()` (que faltaba para `getCollisionBoxes()`).

Límites:

- El core modela **cada bloque sólido como un cubo entero** (no la forma real
  de escaleras/losas/plantas): `getBlockCollisionBoxes()` devuelve un cubo por
  bloque sólido, no `Block::getCollisionBoxes()`.
- `getCollisionBlocks($bb, $targetFirst)` ignora `$targetFirst`: el core
  devuelve las coordenadas ordenadas.
- `getAdjacentChunks()` delega en `loadChunk()` (host `world.get_chunk`), así
  que devuelve `null` donde el bridge no puede materializar la columna.

**Métodos que siguen sin portar (56) y por qué:**

| Grupo | Métodos | Motivo |
|---|---|---|
| Luz (6) | `getHighestAdjacentBlockLight`, `getHighestAdjacentFullLightAt`, `getHighestAdjacentPotentialBlockSkyLight`, `getHighestAdjacentPotentialLightAt`, `getHighestAdjacentRealBlockSkyLight`, `updateAllLight` | El core no tiene motor de luz (§4.17). |
| Generación/promesas (4) | `orderChunkPopulation`, `requestChunkPopulation`, `requestSafeSpawn`, `setChunk` | Generación nativa bajo demanda; no hay `Promise`/pobladores PHP (§4.19). |
| Chunk loaders/ticking/locks (15) | `getChunkListeners`, `getChunkLoaders`, `getChunkTickRadius`/`setChunkTickRadius`, `getTickingChunks`, `registerChunkListener`/`unregisterChunkListener`/`unregisterChunkListenerFromAll`, `registerChunkLoader`/`unregisterChunkLoader`, `registerTickingChunk`/`unregisterTickingChunk`, `lockChunk`/`unlockChunk`/`isChunkLocked` | Registro interno de `ChunkManager`; aquí no hay proveedor ni ticking por jugador (el tick es fijo). |
| Descarga/persistencia (8) | `unloadChunkRequest`, `cancelUnloadChunkRequest`, `unloadChunks`, `saveChunks`, `clearCache`, `onUnload`, `addOnUnloadCallback`, `removeOnUnloadCallback` | Sin almacenamiento en disco: las columnas se generan a demanda y no se persisten. |
| Tiles (2) | `addTile`, `removeTile` | Los block entities no se materializan como objetos PHP (§4.19). |
| Random tick (3) | `addRandomTickedBlock`, `removeRandomTickedBlock`, `getRandomTickedBlocks` | El random tick del core está fijado por tipo de bloque; un registro PHP no lo alteraría. |
| Sonidos con handle (3) | `getNextSoundHandleId`, `getSoundHandleId`, `removeSoundHandleId` | No hay sonidos en bucle en el core. |
| Sueño (2) | `checkSleep`, `setSleepTicks` | Sin camas/ciclo de sueño nativo. |
| Uso de ítems (2) | `useBreakOn`, `useItemOn` | La ruptura/colocación vive en el flujo de `PlayerAuthInput` de las sesiones, no en una API reutilizable por posición. |
| Hashes/coordenadas (5) | `blockHash`, `chunkBlockHash`, `getBlockXYZ`, `getXZ`, `getDifficultyFromString` | Helpers estáticos puros (Morton y parseo de dificultad): el bridge pasa posiciones como `x/y/z` y expone las constantes `DIFFICULTY_*`, así que ningún camino nativo los usa. |
| Internos (6) | `__debugInfo`, `broadcastPacketToViewers`, `createBlockUpdatePackets`, `sendTime`, `onEntityMoved`, `getProvider` | Helpers internos de PM (`sendTime` se cubre con `world.set_time`; no hay `WritableWorldProvider`). |

---

### 4.23 Módulo `block`: migración completa (402 declaraciones, 100% PM5) — ✅ fase 21

El módulo `pocketmine\block` ha sido migrado íntegramente desde la fuente normativa
`target/axolotl-pm` (PocketMine-MP 5 / branch `stable`). Las 402 declaraciones de
clase, interfaz, trait y enum (276 raíz, 39 `tile/`, 19 `inventory/`, 68 `utils/`)
se distribuyen en 9 archivos de runtime PHP ordenados topológicamente para
evitar archivos monolíticos y conflictos de carga:

1. `block_utils.php` (68 declaraciones): enums, traits y helpers (`SupportType`,
   `DyeColor`, `WoodType`, `WoodMaterial`, `SlabType`, `StairShape`,
   `WallConnectionType`, `PillarRotation`, `FortuneDropHelper`, `CropGrowthHelper`, etc.).
2. `block.php` (núcleo):
   - `Block`: 70/70 métodos públicos de PM5 cubiertos al 100% (`getSide`, `getOffset`,
     `getRelative`, `getDrops`, `getXpDropAmount`, `getFlameEncouragement`,
     `getFlammability`, `getLightLevel`, `getLightFilter`, `getSupportType`,
     `isAffectedBySilkTouch`, `getPickedItem`, `onInteract`, `onNearbyBlockChange`,
     `onRandomTick`, `onScheduledUpdate`, `onEntityLand`, `onProjectileHit`,
     `describeBlockItemState`, `describeBlockOnlyState`, `serialize`, `__clone`...) más
     23 métodos bridge para compatibilidad.
   - `BlockBreakInfo`, `BlockIdentifier`, `BlockTypeInfo`, `BlockTypeTags`, `BlockToolType`.
   - `BlockTypeIds`: 833 constantes de bloques vanilla + registro dinámico (`newId`, `forName`).
   - `VanillaBlocks`: 833 métodos estáticos de acceso + `__callStatic` con alias legacy
     (`grass` → `grass_block`, etc.) + `getAll()`.
   - `RuntimeBlockStateRegistry` y `VanillaBlocksInputs` (clase interna de registro).
3. `block_shapes.php` (115 declaraciones): jerarquía de formas (`Transparent`,
   `Opaque`, `Thin`, `Wall`, `Stair`, `Slab`, `Fence`, `FenceGate`, `Ladder`, `Chain`,
   `SimplePillar`, jerarquía `Rail`, `PressurePlate`, etc.) con cálculo analítico de
   cajas de colisión (`recalculateCollisionBoxes()`).
4. `block_fluids.php` (10 declaraciones): `Flowable`, `Air`, `Liquid`, `Water`,
   `Lava`, `BaseFire`, `Fire`, `SoulFire`, `Cobweb`, `Magma`.
5. `block_plants.php` (59 declaraciones): `Leaves`, `Crops`, `Wheat`, `Carrot`,
   `Potato`, `Beetroot`, `Stem`, `Sapling`, `Bamboo`, `Cactus`, `Sugarcane`,
   `NetherWartPlant`, `CocoaBlock`, `ChorusFlower`, `Vine`, `GlowLichen`, etc.
6. `block_tiles.php` (39 declaraciones): jerarquía `Tile`, `Spawnable`, `Container`,
   `Nameable`, `TileFactory` (32 tipos registrados con IDs Mojang y PocketMine),
   `Banner`, `Barrel`, `Beacon`, `Bed`, `Bell`, `BrewingStand`, `Campfire`, `Cauldron`,
   `Chest`, `ChiseledBookshelf`, `Comparator`, `DaylightSensor`, `EnchantTable`,
   `EnderChest`, `FlowerPot`, `Furnace` (`NormalFurnace`, `BlastFurnace`, `Smoker`),
   `GlowingItemFrame`, `HangingSign`, `Hopper`, `ItemFrame`, `Jukebox`, `Lectern`,
   `MobHead`, `MonsterSpawner`, `Note`, `PotentSulfur`, `ShulkerBox`, `Sign`, `SporeBlossom`.
7. `block_inventory.php` (19 declaraciones): `BlockInventory`, `ChestInventory`,
   `DoubleChestInventory`, `FurnaceInventory`, `BarrelInventory`, `HopperInventory`,
   `BrewingStandInventory`, `CampfireInventory`, `ChiseledBookshelfInventory`,
   `EnchantInventory`, `ShulkerBoxInventory`, etc.
8. `block_interactive.php` (57 declaraciones): `Door`, `Trapdoor`, `Button`, `Lever`,
   `Chest`, `EnderChest`, `Barrel`, `Furnace`, `Hopper`, `ShulkerBox`, `Campfire`,
   `Cauldron`, `Bed`, `RedstoneWire`, `RedstoneRepeater`, `RedstoneComparator`,
   `DaylightSensor`, `Note`, `Jukebox`, `Lectern`, `Beacon`, etc.
9. `block_concrete.php` (114 declaraciones): Bloques concretos ordenados
   topológicamente (bloques de minerales, piedra, tierra, césped, arena, hormigón,
   vidrio, velas, pasteles, corales, mesas de trabajo, etc.).

#### Tabla de límites de la mecánica (§4.23)

| Subsistema / Dominio | Contrato PHP | Realidad en el Core Rust | Fallback / Comportamiento |
|---|---|---|---|
| Block Entities / Tiles | 39 clases completas con serialización NBT (`saveNBT`/`readSaveData`) y `TileFactory` | No persistidos en disco ni materializados en memoria del mundo (`World::getTileAt() === null`). | Las instancias creadas en PHP manejan su NBT e inventarios locales de forma segura; `TileFactory::createFromData()` recrea cualquier tile sin lanzar excepciones. |
| Colisiones geométricas | `recalculateCollisionBoxes()` calcula AABBs analíticas exactas de PM5 (escaleras, losas, muros...) | El core simula colisiones AABB de cubos completos (1x1x1) para bloques sólidos en `isSolid()` (§4.22). | Los plugins obtienen las AABBs exactas en PHP; el motor físico nativo previene atravesar bloques sólidos. |
| Luz y propagación | `getLightLevel()`, `getLightFilter()` devuelven valores canónicos de PM5 | El core no implementa cálculo dinámico de iluminación (§4.17). | Consultas de plugins reciben los niveles estándar de Bedrock. |
| Redstone | Estados `isPowered()`, `getPowerLevel()`, conexiones y propiedades de cable | Sin motor nativo de simulación de circuitos ni propagación de señal. | Los métodos de activación (`onInteract`, `isPressed`) alternan los estados en PHP y disparan los eventos correspondientes. |
| Fuego y propagación | `getFlameEncouragement()`, `getFlammability()`, clases `Fire`/`SoulFire` | Sin propagación estocástica de incendios nativa. | `BlockSpreadEvent` y `BlockFormEvent` se disparan vía `BlockEventHook` cuando los fluidos interactúan. |
| Crecimiento y decaimiento | Contratos `Ageable`, `onRandomTick()`, `CropGrowthHelper` | El random tick nativo ejecuta envejecimiento/decaimiento a nivel de bloque en Rust sin invocar ticks PHP. | Métodos disponibles para plugins que simulen o fuercen crecimiento manual (`onRandomTick()`). |
| Inventarios de bloque | 19 clases de `block\inventory` con listeners de cambio | Los inventarios autoritativos de jugador residen en `PlayerInventory`; los contenedores de bloque no tienen sincronización multi-visor en disco. | Los inventarios operan en memoria con slots válidos para transacciones de plugins. |

---

## 5. Plan de implementación, clase por clase

Cada fila se cierra con: host calls nativos + clase PHP hidratada + test.

| # | Clase | Trabajo | Estado |
|---|---|---|---|
| 1 | `pocketmine\entity\Entity` | host calls `entity.*` (`spawn`, `get`, `list`, `despawn`, `teleport`, `set_health`, `damage`, `set_name_tag`), PHP `Entity` respaldada por el mundo, `EntityFactory::create` | ✅ hecho 2026-11-17 |
| 2 | `Living` / `Human` / `WaterAnimal` / `Zombie` / `Villager` / `Squid` | `setMotion`/`getMotion`, `getBoundingBox`, `getDirectionVector`, `knockBack`, `getDrops`, `getXpDropAmount`, `EntitySizeInfo`, `Ageable` | ✅ hecho 2026-11-17 |
| 3 | Eventos de entidad | `EntitySpawnEvent`, `EntityDeathEvent`, `EntityDamageByEntityEvent` + contrato PHP (`fromArray`/`toArray`) | ✅ hecho 2026-11-17 |
| 3b | Daño por proyectil | cola `PendingProjectileDamage` + `EntityDamageByEntityEvent` con `CAUSE_PROJECTILE` antes de aplicar | ✅ hecho 2026-11-17 |
| 4 | `PlayerInventory` / `inventory\*` | host calls `inventory.get/set_slot/clear` + proxy PHP con caché por tick | ✅ hecho 2026-11-17 |
| 5 | `block\Block` | `getProperties`, `isSolid` por paleta, `getCollisionBoxes`, `asItem`, `getBreakInfo` real (`block.break_info`), `setStateId` validado | ✅ hecho 2026-11-17 |
| 6 | `world\World` | `getEntities`, `getNearbyEntities`, `getEntity`, `addEntity`, `removeEntity` | ✅ hecho 2026-11-17 |
| 7 | `player\GameMode` | clase `GameMode` (ids, alias, `fromString`/`fromId`), `Player::getGamemode(): GameMode`, `setGamemode()` | ✅ hecho 2026-11-17 |
| 7b | `Effect`/`Enchantment` | estado de efectos en `WorldState` + `entity.*`, `Effect`/`EffectInstance`/`VanillaEffects`, `Enchantment` sobre el NBT `ench` | ✅ hecho 2026-11-17 (ver §4.9) |
| 8 | `form\*` | `Form`/`SimpleForm`/`CustomForm`/`ModalForm`, `ModalFormRequest`/`Response` y `form_response` por IPC | ✅ hecho 2026-11-17 (ver §4.10) |
| 9 | `crafting\CraftingManager` | `crafting.*` + proxy PHP con índice, `matchRecipe` y registro de recetas | ✅ hecho 2026-11-17 (ver §4.11) |
| 10 | Eventos de daño y despawn | `EntityDamageEvent` base + `ByChildEntity` + `EntityDespawnEvent`, cadena con `final_damage` | ✅ hecho 2026-11-17 (ver §4.12) |
| 11 | `object\*` | `ItemEntity`, `FallingBlock`, `PrimedTNT`, `ExperienceOrb`, `World::dropItem`/`dropExperience` y los `entity.spawn_*` correspondientes | ✅ hecho 2026-11-17 (ver §4.13) |
| 12 | `projectile\*` | `Projectile` + 8 subclases y `entity.spawn_projectile` | ✅ hecho 2026-11-17 (ver §4.14) |
| 13 | Eventos de mundo | `WorldSaveEvent`, `SpawnChangeEvent`, `WorldDifficultyChangeEvent` con cola `PendingWorldEvent`, `world.set_spawn` y `SpawnChangeEvent` en `World::setSpawnLocation()` | ✅ hecho 2026-11-17 (ver §4.15) |
| 14 | Eventos de bloque y chunk | `BlockSpreadEvent`/`BlockFormEvent` vía `BlockEventHook` (fluidos), `EntityExplodeEvent` (TNT, destrucción diferida) y `ChunkLoadEvent` (una vez por columna) | ✅ hecho 2026-11-17 (ver §4.16); growth/decay siguen sin cancelación previa |
| 15 | `world` (sonido, partículas, biomas, consultas) | `world.add_sound`/`world.add_particle` (posiciones exactas), `world.get_biome`/`world.set_biome`, `Sound`/`Particle`/`Biome` + registries, 20 sonidos, 37 partículas, 15 biomas y 89 métodos de `World` en el bridge | ✅ hecho 2026-11-18 (ver §4.17); quedan `generator`/`light` |
| 16 | `WorldSoundEvent` / `WorldParticleEvent` | cola `PendingWorldEffect` en el host call + hook en `SessionManager::dispatch_world_effects()`; cancelación, `setSound`/`setParticle` y `setRecipients` | ✅ hecho 2026-11-18 (ver §4.18) |
| 17 | `world\format` | `format\Chunk` (namespace PM5) + `SubChunk`, `HeightArray`, `ChunkException`; `World::getChunk()` y `ChunkEvent::getChunk()` devuelven `format\Chunk` | ✅ hecho 2026-11-18 (ver §4.19) |
| 18 | `world\BlockTransaction` | lote de bloques con `addBlock`/`fetchBlock`/`getBlocks`, validadores acumulativos y `apply()` atómico sobre `World::getBlockAt()`/`setBlockAt()` | ✅ hecho 2026-11-18 (ver §4.20) |
| 19 | `world\Explosion` | preview de bloques (`world.explosion_blocks`) + encolado (`world.explode`) reutilizando la mecánica de la TNT; `EntityExplodeEvent` en el tick y daño por radio | 🟡 hecho 2026-11-18 (ver §4.21; sin fuego/knockback/sonido) |
| 20 | `World`: colisiones y actualizaciones | `getBlockCollisionBoxes`/`getCollisionBlocks`/`getCollisionBoxes`, `scheduleDelayedBlockUpdate`, `notifyNeighbourBlockUpdate` sobre las APIs nativas; derivados `isSpawnChunk`/`getAdjacentChunks`/`chunkHash` | ✅ hecho 2026-11-18 (ver §4.22); 85/141 métodos de PM5 (99 en el bridge) |
| 21 | Módulo `block` completo | 402 declaraciones PM5 en 9 archivos (`block_utils.php`, `block.php`, `block_shapes.php`, `block_fluids.php`, `block_plants.php`, `block_tiles.php`, `block_inventory.php`, `block_interactive.php`, `block_concrete.php`); 70/70 métodos públicos en `Block`, 833 constantes en `BlockTypeIds`, 833 getters en `VanillaBlocks`, jerarquía PM5 exacta y 8 suites de tests de integración | ✅ hecho 2026-11-18 (ver §4.23) |

---

## 6. Notas de verificación

- Las cifras son estáticas (conteo por nombre de clase/método), no de runtime.
- `docs/ANALYSIS-pocketmine-api.md` (2026-11-08) está desactualizado en §2.11
  ("entidades inexistentes"): hoy `entity.rs` existe. Sus hallazgos de
  inventario (§2.13) y formularios (§2.16) siguen vigentes.
- Comando para recalcular el conteo de clases PM5:
  `grep -rhoE "^(final |abstract )?(class|interface|trait|enum) [A-Za-z0-9_]+" target/axolotl-pm/src/<dir> | wc -l`.
- Conteo del API PHP (929 clases, 53 295 líneas, 48 `*Event`, 50 archivos de runtime: 46 en `php/` + 4 generados) y de los structs
  despachados desde Rust (27):
  ```bash
  php -r '$c=0;$l=0;$e=0; foreach (glob("crates/php-plugin-bridge/php/*.php") as $f) {
    $s=file_get_contents($f); $l+=substr_count($s,"\n");
    preg_match_all("/^\\s*(?:final\\s+|abstract\\s+)?(?:class|interface|trait|enum)\\s+([A-Za-z0-9_]+)/m",$s,$m);
    $c+=count($m[1]); foreach($m[1] as $n){ if(str_ends_with($n,"Event")) $e++; } }
    echo "$c clases, $l lineas, $e eventos\n";'
  grep -rn "impl Event for" crates/php-plugin-bridge/src/events/*.rs | wc -l
  ```

Verificación de la última pasada (fase 21, 2026-11-18):

```bash
cargo test -p server-core --lib                        # 414 OK (0 fallos)
cargo test -p php-plugin-bridge --lib                  # 49 OK (bootstrap con 50 archivos de runtime sincronizados)
cargo test -p php-plugin-bridge --test block_core      # 2 OK (bootstrap_runtime_ready + block_core_classes_and_registries)
cargo test -p php-plugin-bridge --test block_shapes    # 1 OK (block_shapes_and_geometry)
cargo test -p php-plugin-bridge --test block_interactive # 1 OK (block_interactive_containers_redstone)
cargo test -p php-plugin-bridge --test block_fluids_plants # 1 OK (block_fluids_fire_and_plants)
cargo test -p php-plugin-bridge --test block_tiles     # 1 OK (block_tiles_and_factory: 32/32 tiles instanciables roundtrip)
cargo test -p php-plugin-bridge --test block_inventory # 1 OK (block_inventory_classes: 19/19 inventarios)
cargo test -p php-plugin-bridge --test block_utils     # 1 OK (block_utils_enums_and_traits: 68/68 declaraciones)
cargo test -p php-plugin-bridge --test block_state_classes # 1 OK (air_state_resolves_to_the_air_class)
cargo test -p bedrock-protocol --lib                   # 118 OK (incluye modal_form 2/2)
php -l crates/php-plugin-bridge/php/*.php              # 46 archivos PHP limpios (0 errores de sintaxis)
php target/php_world_check.php                         # 27 comprobaciones del módulo world (ALL OK)
php target/measure_block_coverage.php                  # 402/402 declaraciones PM5 encontradas (0 missing)
php target/check_block_methods.php                     # 70/70 métodos públicos PM5 en Block cubiertos (0 missing)
```

`tick_never_generates_chunks_on_its_thread` mide un presupuesto absoluto de
40 ms. En una pasada de esta máquina falló con ~500 ms y en la última pasó,
incluso sin los cambios de la fase 7b (comprobado comentando el tick de
efectos), así que es ruido del entorno y no del port.

Boot del runtime PHP completo (valida firmas entre los 50 archivos): copiar
`crates/php-plugin-bridge/php/*.php` + los `*.php` de
`target/debug/build/php-plugin-bridge-*/out/` a un directorio y ejecutar
`php bootstrap.php`; debe imprimir `[PHP/RUNTIME] Runtime ready`.

El harness PHP vive en `target/` porque el proyecto no tiene runner PHP; si se
pierde, se recrea con el mismo patrón: definir un `pocketmine\runtime\Bridge`
falso y requerir `text.php`, `math.php`, `permission.php`, `command.php`,
`item.php`, `world.php`, `player.php` en ese orden.

`tick_never_generates_chunks_on_its_thread` mide un presupuesto absoluto de
40 ms. En una pasada de esta máquina falló con ~500 ms y en la última pasó,
incluso sin los cambios de la fase 7b (comprobado comentando el tick de
efectos), así que es ruido del entorno y no del port.

Boot del runtime PHP completo (valida firmas entre los 42 archivos): copiar
`crates/php-plugin-bridge/php/*.php` + los `*.php` de
`target/debug/build/php-plugin-bridge-*/out/` a un directorio y ejecutar
`php bootstrap.php`; debe imprimir `[PHP/RUNTIME] Runtime ready`.

El harness PHP vive en `target/` porque el proyecto no tiene runner PHP; si se
pierde, se recrea con el mismo patrón: definir un `pocketmine\runtime\Bridge`
falso y requerir `text.php`, `math.php`, `permission.php`, `command.php`,
`item.php`, `world.php`, `player.php` en ese orden.
