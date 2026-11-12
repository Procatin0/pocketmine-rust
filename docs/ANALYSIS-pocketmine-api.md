# Análisis completo — API de PocketMine-MP 5 (axolotl-pm) vs. pocketmine-rust

**Fecha:** 2026-11-08
**Referencia normativa:** PocketMine-MP 5 (`pmmp/PocketMine-MP`, rama `stable`, HEAD `6a7cc02`), clonado temporalmente para este análisis (ver §1).
**Objeto comparado:**
- `target/axolotl-pm/src/*` — *ausente en este checkout* (ver §1)
- `crates/php-plugin-bridge/php/*.php` — bridge PHP (≈8 000 líneas, 19 ficheros)
- `crates/php-plugin-bridge/src/**` — runtime Rust del bridge
- `crates/server-core/src/*.rs` — núcleo Rust (≈32 000 líneas)
- `crates/bedrock-protocol/src/packets/*.rs` — 48 módulos de paquetes
- `crates/bedrock-world/src/**` — mundo/generación

**Alcance:** solo análisis. No se toca código. Este documento **complementa** `docs/wiki/` (manual de uso de la API *existente*), `docs/plugin-api-contract.md` (contrato IPC) y `docs/pendientes-axolotl/plan-compatibilidad.md` (paridad de comandos/paquetes). Donde estos documentos han quedado desactualizados se señala la corrección en §5.

**Convención de prioridades:**
- **P0** — rompe (crash por clase/método faltante o estado divergente) un plugin de `plugins/` o el gameloop central.
- **P1** — necesario para plugins PM de uso común (economía, staff, minijuegos, kits, protección de bloques).
- **P2** — paridad secundaria: el plugin funciona pero con funciones degradadas.
- **P3** — nicho o interno (red profunda, aync, dumps, telemetría).

**Leyenda por fila:** ✅ existe y funciona · 🟡 existe parcial/stub · 🔴 ausente.

---

## 0. Resumen ejecutivo

El bridge cubre muy bien **lo que los plugins instalados usan hoy** (chat, permisos, comandos, scheduler, scoreboard, bossbar), pero hay **dos agujeros estructurales P0** y una **cadena de herencia de Player del 76 % ausente**:

| # | Hallazgo | Impacto |
|---|---|---|
| 1 | **Inventario con doble fuente de verdad (P0):** el `PlayerInventory` de PHP (`player.php`) es un stub en memoria que **no habla con** el inventario autoritativo Rust (`inventory.rs`, 36+armadura+offhand+cursor+crafteo, con `InventoryTransactionPacket` y `ItemStackRequest`). No existen host calls `inventory.*`. Cualquier plugin que lea/escriba inventario ve un inventario vacío/desincronizado. | Cualquier plugin de economía/tienda/kit |
| 2 | **Formularios inexistentes (P0):** no hay `pocketmine\form\*` (ni siquiera la clase abstracta `Form` de PM5). `Player::sendForm()` es un **no-op**. Un plugin que use `SimpleForm` (patrón estándar PM) muere con *Class not found*. Tampoco hay paquete `ModalForm` en `bedrock-protocol`. | Menús, confirmaciones, UIs |
| 3 | **Entidades inexistentes (P0/P1):** no hay `Entity`/`Living`/`Human` en PHP ni modelo de entidad en Rust más allá de la visibilidad de jugadores (`AddPlayerPacket`, `RemoveActorPacket`, `MoveActorAbsolutePacket`, `SetActorData`). Daño, muerte con drops, proyectiles, NPCs, experiencia y hambre no existen como sistema. | Minijuegos, combate, drops, mascotas |
| 4 | **Cadena Player: 238/313 métodos ausentes (76 %).** El bridge implementa 114 métodos únicos de PHP frente a 313 de la cadena PM5 (`Player` 146 + `Human` 25 + `Living` 46 + `Entity` 105 + `ExperienceManager` 22 + `HungerManager` 17, con duplicados resueltos). Ver §2.2 para la lista exhaustiva. | La mayoría de plugins “de jugador” |
| 5 | **Comandos: 41/41 parseados, 8 con efecto stub (P1).** `effect`, `enchant`, `xp`, `particle`, `status`, `timings`, `gc`, `dumpmemory` responden con los mensajes de `spa.ini` pero **no tienen efecto real** (solo `debug!` en `session.rs`; `gc` reporta ceros honestos). `plan-compatibilidad.md` §3 los marca 🔴 “no implementado”: ya no es exacto, pero tampoco funcionan de verdad. | Paridad de consola |
| 6 | **Todo lo demás, bien encaminado:** permisos (ambos lados completos, 63 nodos Rust), scheduler PHP completo, chat con `ChatFormatter`, scoreboard y bossbar **completos en ambos lados** (Bedrock + Java). | BetterChat, ChatGuard, ScoreboardDemo ✅ |
| 7 | **Mundo de un solo mapa, sin luz ni estados (P1):** `WorldState` es un único mundo sin `level.dat`, sin luz cielo/bloque, sin estados de bloque (ids planos) y sin tiles. `World` PHP carece de 98/132 métodos PM; `dropItem` devuelve `null`, `addParticle`/`addSound` son no-ops documentados. | Construcción, teleports multi-mundo, visual |
| 8 | **Bug en el wire: títulos de plugins con id de paquete incorrecto (P1).** `create_set_title_packet()` en `session.rs` escribe `write_var_u32(0x59)`; el id real de `SetTitlePacket` en protocolo 2225 es **0x58** (el tipado `bedrock-protocol/src/packets/set_title.rs` sí devuelve 0x58). La ruta manual **sí se usa en producción** (líneas 5872/5882/5915/5925, `PluginAction::SendTitle`/`BroadcastTitle`): los clientes Bedrock no muestran los títulos que mandan los plugins. Ver §6 corrección 4 y §5 P1-9. | `send_title`/`broadcast_title` |

**Compatibilidad con los plugins instalados hoy: 6/6 cargan y operan** (BetterChat, ChatGuard, EconomyLite, ExamplePlugin, TestKit, ScoreboardDemo), pero con el caveat de que ninguno de ellos ejercita inventario, entidades ni formularios. La matriz por tipo de plugin se ver en §3.

---

## 1. Método, fuentes y verificación

### 1.1 `target/axolotl-pm` no existe en este checkout

El enunciado pide comparar contra `target/axolotl-pm/src/*`, pero ese directorio **no está presente**: `target/` en este equipo contiene solo artefactos de build (`debug/`, `release/`, `test-parity/` de `docs/pendientes-axolotl/`, jars, capturas, fixtures), no clones del fork. `git remote -v` está vacío y `.gitignore` ignora `/target/` (esperado: los docs del proyecto lo definen como “fuera de git, se recrea clonando el fork”).

Procedimiento usado:
1. Clonado superficial de `pmmp/PocketMine-MP@stable` (HEAD `6a7cc02`) con sparse-checkout de `src/` + `resources/` en `%TEMP%\pm5ref\`.
2. Toda comparación de métodos es contra esa fuente, con el mismo nombre de módulo y firma pública.
3. Verificación cruzada con la documentación interna del proyecto (`PORT-ROADMAP.md`, `plan-compatibilidad.md`, wiki, `EVENT_TRIGGERS.md`), que citan `target/axolotl-pm` como origen de traducción.

### 1.2 Cuentas verificadas (útil para revalidar tras clonar el fork)

| Módulo PM5 | Enunciado | Medido en `stable@6a7cc02` | Observación |
|---|---|---:|---|
| `Server.php` | 91 métodos | **91** `public function` únicos | coincide exacto |
| `Player.php` | ~300 | **146** propios; **313** con la cadena `Human`+`Living`+`Entity`+`ExperienceManager`+`HungerManager` | el “~300” es la cadena completa, no el fichero solo |
| `Event/*` | 142 | **150** ficheros | skew de versión: el HEAD actual es más nuevo que el fork clonado en 2026-11-06; la wiki dice 143. Para el análisis se usa 150 y se citan las clases. |
| `Command/*` | 42 | **53** ficheros (framework 7 + defaults 41 + utils 5) | el “42” ≈ 41 defaults + `VanillaCommand` |
| `block/` | — | 390 ficheros |  |
| `item/` | — | 154 ficheros | coincide con el enunciado implícito |
| `entity/` | — | 77 ficheros |  |
| `world/` | — | 271 ficheros |  |
| `inventory/` | — | 35 ficheros |  |
| `network/` | — | 85 ficheros |  |
| `data/` | — | 99 ficheros |  |
| `scheduler/` | — | 15 |  |
| `permission/` | — | 14 |  |
| `plugin/` | — | 22 |  |
| `form/` | — | 2 (`Form.php` abstracta + `FormValidationException.php`) | los formularios concretos (`SimpleForm`, `CustomForm`, `ModalForm`) viven en el plugin **ext-forms**, no en el core |
| `player/` | — | 16 | incluido `PlayerInfo`, `GameMode`, `Skin` (vía `entity\Skin`) |

> **Scoreboard y BossBar no son de PM5 core.** Son plugins externos: `pocketmine\scoreboard\*` (plugin Scoreboard, no confundir con el `scoreboard` de Bukkit) y `xenialdan\apibossbar` (apiBossBar). El bridge los porta como módulos propios (`scoreboard.php`, `bossbar.php`), lo cual es correcto pero debe leerse como “compatibilidad con dos plugins populares”, no como paridad de core.

### 1.3 Qué es “existe” en este informe

- **Bridge PHP:** el método/clase existe en `crates/php-plugin-bridge/php/*.php` con la misma signatura pública. Si devuelve un valor fijo/`false`/`null`/no-op, se marca **🟡 stub** aunque la signatura exista.
- **Rust:** existe modelo o paquete en `server-core`/`bedrock-protocol`/`bedrock-world` con efecto observable en el cliente Bedrock (o Java cuando aplica).
- **Host calls PHP→Rust:** la lista completa está en el Anexo A (22 `world.*`, 12 `player.*`, ~10 `server.*`). Si un método PHP dice “stub” porque no hay host call, se cita el archivo de Rust a crear.

---

## 2. Tabla maestra por módulo

| # | Módulo | axolotl-pm (cantidad) | Nuestro bridge/Rust (cantidad) | Falta (stub o ausente) | Prioridad | Archivo(s) a tocar | Plugins PM afectados |
|---|---|---|---|---|---|---|---|
| 2.1 | `Server` | 91 métodos | `core.php` 104 funciones; 67/91 PM presentes | 24 métodos ausentes; 8 stubs `null` (`getLoader`, `getUpdater`, `getCraftingManager`, `getResourcePackManager`, `getAsyncPool`, `getMemoryManager`, `getAuthKeyProvider`, `getNetwork`) | P1 | `crates/php-plugin-bridge/php/core.php` | Todos (diagnóstico) |
| 2.2 | `Player` (+cadena) | 313 métodos (cadena) | `player.php` 114 métodos únicos; Rust: `session.rs` + `context.rs` | 238 métodos ausentes; inventario/hambre/XP/efectos/entities stub | **P0** | `crates/php-plugin-bridge/php/player.php`, `crates/server-core/src/session.rs`, `plugin_host.rs`, nuevos `crates/server-core/src/entity.rs` | Todos los de jugador |
| 2.3 | `Event` | 150 ficheros (143 clases + infra) | bridge PHP 15 clases; Rust 9 structs + `EventDispatcher`; 11 eventos cableados | ~132 eventos sin trigger (catálogo en `EVENT_TRIGGERS.md`); sin clase `Entity` | P1 | `crates/server-core/src/session.rs`, `world_state.rs`, `crates/php-plugin-bridge/php/event.php` | Protección, combate, economía avanzada |
| 2.4 | `Command` | framework 7 + 41 defaults | `command.php` 64 métodos; Rust `command.rs` 4 896 líneas, 49 comandos (41 PM + 8 propios) | 8 comandos con efecto stub; `FormattedCommandAlias`, `VanillaCommand` incompletos (ver §2.4) | P1 | `crates/server-core/src/command.rs`, `session.rs`, `crates/bedrock-protocol/src/packets/mob_effect.rs` (nuevo) | Staff/administración |
| 2.5 | `Plugin` | 22 ficheros | `plugin.php` 2 683 líneas (el mayor fichero del bridge); `description.rs` 170 | Detalles de `PluginDescription` (`requiredExtensions`, `libraries`, `accessProtocol`); `PluginEnableOrder` parcial | P2 | `crates/php-plugin-bridge/php/plugin.php`, `src/description.rs` | Plugins grandes (dependencias) |
| 2.6 | `Scheduler` | 15 ficheros (`TaskScheduler` 10 métodos) | `scheduler.php` 1 291 líneas, 127 métodos (incl. `AsyncTask`, `AsyncPool`) | `AsyncPool` sin pool real (todo inline/sync) | P2 | `crates/php-plugin-bridge/php/scheduler.php`, `src/runtime/php.rs` | Plugins con I/O async |
| 2.7 | `Permission` | 14 ficheros | `permission.php` 778 líneas; Rust `permission.rs` 568 + `access.rs` 524 (63 nodos) | Gate de plugins limitado a `is_op` (`server.has_permission` ignora `PermissionManager`); `group.*` sin efecto | P1 | `crates/server-core/src/plugin_host.rs`, `permission.rs`, `crates/php-plugin-bridge/php/permission.php` | BetterChat/ChatGuard (ya ok) y cualquier plugin con nodos no-op |
| 2.8 | `World` | 271 ficheros; `World` 132 métodos; `WorldManager` 18 | `world.php` 71 métodos (World+Block+Chunk+Manager); Rust `world_state.rs` (1 mundo) + `bedrock-world` | **98/132 métodos**; luz, tiles, multi-mundo, `dropItem`→`null`, partículas/sonido no-op | P1 | `crates/server-core/src/world_state.rs`, `crates/bedrock-world/src/block_id.rs` + `chunk.rs`, `crates/php-plugin-bridge/php/world.php` | Construcción, teleports, redstone |
| 2.9 | `Item` | 154 ficheros; `Item` 59 métodos | `item.php` 37 métodos; Rust `inventory.rs` (registro 834 items) + `nbt_json.rs` | 41/59 métodos; `StringToItemParser`, encantamientos (sistema), comportamientos (comida, herramientas) | P1 | `crates/php-plugin-bridge/php/item.php`, `crates/server-core/src/inventory.rs`, nuevo `crates/server-core/src/enchant.rs` | Tiendas, kits, herramientas |
| 2.10 | `Block` | 390 ficheros; `Block` 70 métodos | `world.php`: `BlockFactory` 110+ nombres legacy + `Block` mínimo; Rust: ids planos | 62/70 métodos; estados reales, comportamientos, drops, luz | P1 | `crates/bedrock-world/src/block_id.rs`, `crates/server-core/src/world_state.rs`, `crates/php-plugin-bridge/php/world.php` | Protección, grief, build |
| 2.11 | `Entity` | 77 ficheros; `Entity` 105 + `Living` 46 + efectos 20 | Sin clases. Rust: paquetes de visibilidad (6) + `ApplyEffect` stub | Todo el sistema: daño, muerte, drops, proyectiles, efectos, atributos, mobs | P1 (/P0 para minijuegos) | Nuevos `crates/server-core/src/entity.rs`, `effect.rs`, `crates/bedrock-protocol/src/packets/add_actor.rs`, `mob_effect.rs` (nuevos) | Minijuegos, combate, mascotas |
| 2.12 | `Scoreboard` / `BossBar` | plugins externos (`pocketmine\scoreboard`, `xenialdan\apibossbar`) | **Completos ambos lados**: `scoreboard.rs` 670 + `bossbar.rs` 916 + PHP + paquetes Bedrock/Java + persistencia `scoreboard.json` | Solo pulido (P3) | P3 | — (verificar overrides `DiverseBossBar` por jugador) | ScoreboardDemo ✅ |
| 2.13 | `Inventory` | 35 ficheros; `Inventory` 25 métodos + 11 subclases + transacciones | Rust: inventario real + crafteo + yunque + `ItemStackRequest`; PHP: stub en memoria | **Sincronización PHP↔Rust inexistente** (0 host calls `inventory.*`); 10/25 métodos del `Inventory` base | **P0** | `crates/php-plugin-bridge/php/player.php`, `crates/server-core/src/plugin_host.rs`, `inventory.rs`, `session.rs` | Economía, tiendas, kits, cofres |
| 2.14 | `Crafting` | 25 ficheros; `CraftingManager` 16 métodos | Rust `crafting.rs` 1 272 líneas (motor real); `getCraftingManager()` PHP → `null` | Exposición PHP del motor; recetas post-defecto de plugin (callbacks) | P2 | `crates/php-plugin-bridge/php/core.php`, `crates/server-core/src/crafting.rs` + host calls `crafting.*` | Plugins que registran recetas |
| 2.15 | `Chat` | `PlayerChatEvent` + `ChatFormatter` + recipients | `chat.php` (interfaz + `PlaceholderFormatter`) + `event.php`; Rust `chat.rs` + `session.rs` chat | Sin métodos faltantes relevantes | P3 / ✅ | — | BetterChat ✅, ChatGuard ✅ |
| 2.16 | `Form` | core: `Form` abstracta + `FormValidationException`; resto en ext-forms | **Nada.** `sendForm(mixed)` = no-op | Clases `Form`/`SimpleForm`/`CustomForm`/`ModalForm` + paquete `ModalForm` Bedrock + `onFormSubmit` | **P0** | Nuevo `crates/php-plugin-bridge/php/form.php`; `crates/bedrock-protocol/src/packets/modal_form.rs` (nuevo); `session.rs` | Menús/UIs (crash sin el plugin de formularios) |
| 2.17 | `Data` | 99 ficheros (serializers NBT, mapas de ids) | `data/` JSON + NBT de red mínimo | `SavedData`, `ItemSerializer`, `BlockStateSerializer`, persistencia `level.dat`/`player.dat` | P3 | Nuevo `crates/server-core/src/data.rs` (o aceptar JSON como formato propio) | Plugins que migran saves de PM |
| 2.18 | `Utils` | 33 ficheros; `TextFormat` completa | `text.php`: `TextFormat` ✅ completa; `Utils` 6 métodos; `Config` 16 métodos; `math.php` 77 | `Utils` (PM tiene ~30 métodos estáticos), constantes `Config::MODE_*`, `KnownTranslationFactory` | P2 | `crates/php-plugin-bridge/php/text.php`, `config.php`, `core.php` | Plugins que usan `Utils::` |
| 2.19 | Red/`Thread`/`Timings`/`Console`/`ResourcePacks`/`Lang`/`Crash`/`Updater`/`Stats`/`Promise` | 85+13+3+5+9+3+5+3+1+3 | Paquetes 48 módulos; `resource_packs.rs` handshake; `translations.rs` (spa/eng); `crash.rs`; `metrics.rs`; `query.rs` | `DataPacket*Event`, `AsyncPool` real, `timings` con datos, `Lang` PHP-side, firmas de recursos | P3 | Diverso (ver §2.19) | Plugins avanzados |

---

## 3. Detalle por módulo

### 2.1 Server — 91 métodos PM, 67 presentes, 24 ausentes, 8 stubs

`crates/php-plugin-bridge/php/core.php` merece nota: tiene **más** funciones que PM (104) porque incluye `Logger`, jugadores y bans (que en PM5 viven en otros ficheros). Comparando contra `Server.php` puro:

**Ausentes (24):** `crashDump`, `createPlayer`, `enablePlugins`, `exceptionHandler`, `getAllowedViewDistance`, `getDifficulty`, `getFilePath`, `getIpV6`, `getOfflinePlayerData`, `getPluginCommand`, `getPocketMineVersion`, `getPortV6`, `getTickSleeper`, `getTicksPerSecondAverage`, `getTickUsage`, `getTickUsageAverage`, `getViewDistance`, `hasOfflinePlayerData`, `isHardcore`, `isLanguageForced`, `isRunning`, `saveOfflinePlayerData`, `sendUsage`, `shouldSavePlayerData`.

**Stubs documentados (`null`/`false` en `core.php`, marcador `TODO(A1)`):** `getLoader`, `getUpdater`, `getCraftingManager`, `getResourcePackManager`, `getAsyncPool`, `getMemoryManager`, `getAuthKeyProvider`, `getNetwork`. Además `getConfigGroup()` y `getQueryInformation()` devuelven arrays/`mixed` en vez de objetos tipados; `prepareBatch()` es compresión sync sin `AsyncPool`; `getNameBans()`/`getIPBans()` devuelven `Config` en vez de `BanList` tipado (el tipado sí existe en el bridge: `permission.php` define `BanList`/`BanEntry`, pero `Server` no lo usa — oportunidad rápida).

**Divergencias a vigilar (no son ausencia, son forma distinta):** `getPlayer()` implementa búsqueda por **prefijo** (`stripos === 0`, exacto primero), que es la semántica de `Server::getPlayerByPrefix()` — compatible. `getOnlinePlayers()` filtra por `isOnline()`; en PM la lista es de `Player` conectados. `getPlayerCount()` == `getOnlinePlayerCount()` (PM los distingue: online vs. en el servidor).

- **Prioridad: P1.** Ningún plugin instalado llama a los stubs de forma letal (TestKit usa `getTick()`/`getTicksPerSecond()`, que existen y pasan por host calls). Pero `getViewDistance()`/`getDifficulty()` y los `*PlayerData*` los usan plugins de staff. Un plugin que llame `getUpdater()` en PM recibe un objeto; aquí recibe `null` → *error on null* si no comprueba.
- **Archivos:** `core.php` (métodos), `plugin_host.rs` (host `server.*` nuevos), `server-core/src/access.rs` (bans tipados).

### 2.2 Player — 313 métodos de cadena, 114 en el bridge, **238 ausentes**

`crates/php-plugin-bridge/php/player.php` (1 161 líneas) cubre bien lo que hoy usan los plugins instalados: identidad (`getName`, `getDisplayName`, `getXuid` —ojo: devuelve `(string)$this->uniqueId`, no un XUID real, ver divergencia §3—), posición, gamemode, salud, permisos/op, mensajes/titles/tips/popups, teleport, kick, scoreboard,flags de movimiento (toggle*), `sleepOn`/`stopSleep` simplificados, `getFirstPlayed`/`getLastPlayed`/`hasPlayedBefore`, `getServer`, `Skin` (identidad, no datos visuales reales: `getSkinData()` deriva de `xuid`).

**Ausentes (agrupados por subsistema para fijar el backlog, lista completa abajo):**

| Subsistema | Métodos PM ausentes (muestra) | Coalescencia en Rust hoy |
|---|---|---|
| Inventario | `getOffHandInventory`, `getCurrentWindow`, `setCurrentWindow`, `removeCurrentWindow`, `selectHotbarSlot`, `getCreativeInventory(set)`, `getCraftingGrid(set)`, `getEnderInventory` — el stub existe pero **sin sync** | `inventory.rs` completo, pero sin host calls |
| Daño/muerte | `attack`, `attackEntity`, `damageArmor`, `heal`, `kill`, `respawn`, `getDrops`, `getDeathPosition/setDeathPosition`, `getXpDropAmount`, `getLastDamageCause/setLastDamageCause` | `SetHealth` vía `PlayerContext.health`; `/kill` existe; sin sistema de daño |
| Hambre/aire | `getFood/setFood`, `addFood`, `getSaturation/setSaturation`, `addSaturation`, `getExhaustion/setExhaustion`, `exhaust`, `getHungerManager`, `getMaxFood`, `isHungry`, `getAirSupplyTicks`, `canBreathe` (presente, simplificado) | `PlayerContext.food/saturation/air: f32` existen como datos, sin tick de hambre |
| XP | `addXp`, `subtractXp`, `addXpLevels`, `subtractXpLevels`, `getXpManager`, `getCurrentTotalXp`, `getLifetimeTotalXp`, `getRemainderXp`, `setXpAndProgress`, `setXpProgress`, `getXpProgress`, `canAttractXpOrbs`, `canPickupXp`, `onPickupXp` | `xp_progress`+`xp_level` + `UpdateAttributes` vía `SetXp` (ver §2.11); `/xp` es stub de comando |
| Efectos | `getEffects` | `/effect` parsea pero `ApplyEffect` es log-only (nodo `StringToEffectParser` inexistente) |
| Movimiento/estado | `getMotion/setMotion`, `addMotion`, `getGravity/setGravity`, `hasGravity/setHasGravity`, `getFallDistance/setFallDistance`, `resetFallDistance`, `setFireTicks`, `getFireTicks`, `isOnFire/setOnFire`, `extinguish`, `isUnderwater`, `isOnGround`, `getBoundingBox`, `getEyeHeight/getEyePos`, `onUpdate`, `onCollideWithPlayer`, `jump`, `lookAt`, `knockBack` | Flags `data_flags()` en sesión; sin física |
| Sesión/red | `chat()`, `emote`, `disconnect`, `transfer`, `getNetworkSession`, `getPlayerInfo`, `sendData`, `sendSkin`, `changeSkin`, `getUsedChunkStatus`, `isUsingChunk`, `hasReceivedChunk`, `getViewDistance/setViewDistance`, `getScreenLineHeight/setScreenLineHeight`, `getLanguage`, `getLocale` (presente como string), `isAuthenticated`, `getAllowFlight/setAllowFlight`, `doChunkRequests`, `doFirstSpawn`, `spawnTo/spawnToAll`, `getViewers`, `onPostDisconnect`, `handleMovement` | Login/primera aparición reales en `session.rs`; chunks reales |
| Form/spawn | `closeAllForms`, `hasPendingForm`, `onFormSubmit`, `getSpawn/setSpawn`, `hasValidCustomSpawn` | `spawn_point` en `PlayerContext` (JSON) pero `setSpawn()` PM sin host call |
| Interacción | `breakBlock`, `continueBreakBlock`, `stopBreakBlock`, `attackBlock`, `interactBlock`, `interactEntity`, `pickEntity`, `consumeHeldItem`, `useHeldItem`, `releaseHeldItem`, `getItemUseDuration`, `isUsingItem`, `setUsingItem`, `getItemCooldownExpiry`, `hasItemCooldown`, `resetItemCooldown`, `openSignEditor`, `missSwing`, `onBlockChanged`, `onChunkChanged`, `onChunkUnloaded`, `getInAirTicks`, `getFlightSpeedMultiplier/setFlightSpeedMultiplier`, `setAutoJump/hasAutoJump`, `hasBlockCollision/setHasBlockCollision`, `isSneakPressed/setSneakPressed`, `getDeathPosition`, `removeTitles/resetTitles/removeCurrentWindow`, `sendJukeboxPopup`, `broadcastAnimation/broadcastSound`, `canBeRenamed`, `canBeMovedByCurrents`, `canBeCollidedWith`, `canCollideWith`, `canSaveWithChunk/setCanSaveWithChunk`, `canInteract`, `getDirectionPlane/Vector`, `getHorizontalFacing`, `getId`, `getNameTag/setNameTag`, `isNameTagVisible`, `getScale/setScale`, `getScoreTag`, `getTargetEntity`, `getOwningEntity`, `getPickedItem`, `getFireProof`, `saveNBT`, `tick`, `getAttributeMap`, `scheduleUpdate`, `flagForDespawn/isFlaggedForDespawn`, `despawnFrom/All`, `setRotation`, `setStepHeight`, `setNoClientPredictions`, `setForceMovementUpdate`, `hasMovementUpdate`, `getMovementSpeed`, `setMovementSpeed`, `getAbsorption`, `getFrostWalkerLevel`, `getLineOfSight`, `getTargetBlock`, `getSneakOffset`, `isEnabled`, `setEnabled`, `applyDamageModifiers`, `consumeObject`, `onAirExpired`, `onRandomUpdate`, `onNearbyBlockChange`, `onInteract`, `addVelocityToEntity`, `setSprinting/setSneaking/setSwimming/setGliding/setBreathing` (los `toggle*` sí existen; los `set*` no), `setHasBlockCollision`, `isInvisible/setInvisible`, `isSilent/setSilent`, `isInsideOfSolid`, `respawnToAll`, `setCanClimb`, `setMaxHealth` (presente), `getEnchantmentSeed/setEnchantmentSeed`, `regenerateEnchantmentSeed`, `resetXpCooldown`, `canAttractXpOrbs`, `setCurrentTotalXp`, `setLifetimeTotalXp`, `getMaxAirSupplyTicks`, `getFoodTickTimer`…

*(la línea superior es el barrido íntegro de los 238: sirve como checklist literal)*

- **Prioridad: P0** por inventario (§2.13) y **P1** por el resto (un plugin “completo” de PM toca `setHealth`, `getWorld()->getBlock()`, `getInventory()`, `damage()` en algún momento).
- **Archivos:** `crates/php-plugin-bridge/php/player.php` (métodos), `crates/server-core/src/session.rs` (estado real), `plugin_host.rs` (host calls `player.*`), nuevo `entity.rs` (daño/efectos/XP/hambre).

### 2.3 Event — 150 ficheros PM, 15 clases en el bridge, 11 cableados

Infraestructura (`Event`, `Cancellable`, `EventPriority`, `RegisteredListener`, `EventFactory`, reflexión por docblock `@priority`/`@ignoreCancelled`) está completa y coincide con PM. La distancia está en dos ejes:

1. **Clases:** bridge PHP define 14 clases en `event.php` (`PlayerJoin/Quit/Chat/CommandPreprocess/Move/Death/Respawn`, `PlayerInteract`, `BlockBreak`, `BlockPlace`, `CommandEvent`, `ServerCommandEvent`, + base `PlayerEvent`/`BlockEvent`) + `PlayerDataSaveEvent.php` aparte. Rust tiene 9 structs (`events/player|block|server.rs`, incluye `AnvilDamageEvent` sin contraparte PM directa y `PlayerDeath/Respawn`). **Faltan ~130 clases** (catálogo por familia en `docs/wiki/10-eventos.md` y `EVENT_TRIGGERS.md`, que ya detallan trigger sugerido y cancelabilidad — no se duplica aquí).
2. **Triggers (lo que el core dispara):** cableados **Join, Quit, Chat, CommandPreprocess, Move (throttle 1/2 inputs), Interact (START_BREAK), BlockBreak, BlockPlace (gated `has_listeners`), Death, Respawn, ServerCommand (consola)**. El resto está en estado *stub*: la clase existe, compila y el roundtrip `fromArray/toArray` pasa (141/141 según `EVENT_TRIGGERS.md`), pero el core nunca la dispara.

**Divergencias conocidas (ya documentadas en `EVENT_TRIGGERS.md`, se confirman):** `EntityEvent` se modela como arrays + `__call`/`extra` por no existir clase `Entity`; `PlayerExperienceChange/Exhaust` extienden `PlayerEvent` en el bridge (PM: `EntityEvent<Human>`); `BaseBlockChangeEvent` y eventos `Block*` nuevos usan coords en vez de `BlockEvent(Player|Block)` para no inventar un jugador ficticio. `FurnaceBurn/SmeltEvent` viven en namespace `inventory` (como PM) pero con coords de bloque.

- **Prioridad: P1.** Los plugins instalados usan Join/Quit/Chat/CommandEvent/BlockBreak/Interact/Move — todos cableados ✅. Los que duelen primero en PM real: `PlayerLoginEvent`/`PlayerPreLoginEvent` (control de acceso antes de entrar), `PlayerKickEvent`, `EntityDamageEvent`/`EntityDeathEvent` (economía por actividad, kits), `InventoryTransactionEvent`/`CraftItemEvent` (tiendas), `PlayerGameModeChangeEvent` (minigames). 
- **Archivos:** `crates/server-core/src/session.rs` (triggers de jugador), `world_state.rs` (bloque/entidad/inventario/mundo), `crates/php-plugin-bridge/php/event.php` (clases que falten).

### 2.4 Command — framework completo, 41/41 comandos parseados, 8 con efecto stub

**Bridge PHP (`command.php`, 64 métodos):** cubre `Command`, `SimpleCommand`, `ClosureCommand`, `PluginCommand`, `CommandMap` (+`registerAll`, `registerAlias`), `FormattedCommandAlias`, `VanillaCommand`, excepciones (`InvalidCommandSyntaxException`, `PermissionCommandException`, `CommandException`/`CommandStringHelper` en PM `command/utils` equivalentes). `testPermission`/`testPermissionSilent` con `permissionNodes()` (semántica `self|other` correcta).

**Rust (`command.rs`, 4 896 líneas):** 49 entradas en `COMMANDS`: los 41 defaults de PM (`ban`, `ban-ip`, `banlist`, `clear`, `defaultgamemode`, `deop`, `difficulty`, `dumpmemory`, `effect`, `enchant`, `gamemode`, `gc`, `give`, `help`, `kick`, `kill`, `list`, `me`, `op`, `pardon`, `pardon-ip`, `particle`, `plugins`, `save-all`, `save-off`, `save-on`, `say`, `seed`, `setworldspawn`, `spawnpoint`, `status`, `stop`, `teleport`, `tell`, `time`, `timings`, `title`, `transfer-server` (alias de `transferserver`), `version`, `whitelist`, `xp`) + 7 propios (`gmc`, `gms`, `gma`, `gmsp`, `spawn`, `weather`, `tps`, `scoreboard` —8 propios, superconjunto declarado en `plan-compatibilidad.md` §4—). Mensajes desde `spa.ini` vía `translations.rs` + constantes espejo verificadas por test constante-por-clave; permisos por nodo (63 `COMMAND_*` en `permission.rs`).

**Los 8 con efecto stub** (feedback correcto, efecto = `debug!` en `session.rs`):

| Comando | `CommandAction` | Estado real en `session.rs` |
|---|---|---|
| `effect` | `ApplyEffect` | log; sin paquete `MobEffect`; id opaco (sin `StringToEffectParser`) |
| `enchant` | `Enchant` | log; sin NBT de encantamiento |
| `xp` | `AddXp` | log; **inconsistente** con `PluginAction::SetXp`, que sí envía `UpdateAttributes` |
| `particle` | `SpawnParticle` | log; sin `LevelEventPacket`/`SpawnParticleEffectPacket` |
| `status` | — | mensajes de métricas limitadas (TPS reales vía `metrics.rs`, sin `BandwidthStatsTracker`) |
| `gc` | `GarbageCollect` | reporta 4 ceros honestos |
| `timings` | `Timings` | `debug!` del modo; sin `TimingsHandler` |
| `dumpmemory` | `DumpMemory` | escribe `memory_dumps/*.txt` (verificar formato); sin volcado de memoria real |

**Corrección a `plan-compatibilidad.md`:** §3 los marca 🔴 y §2 cuenta “31 de 41”. En el árbol actual son **41/41 parseados** (33 con efecto real + 8 stub). La matriz de paridad de mensajes/nodos de §3 sigue válida para los 33; los 8 hay que pasarlos de 🔴 a 🟡 “parseado, sin efecto”.

- **Prioridad: P1** (las 8 acciones stub) porque el comando *existe* en `/help` y cualquier test de paridad lo detecta. Para `effect`/`xp` el camino es corto: `MobEffectPacket` (módulo nuevo en `bedrock-protocol`) + `UpdateAttributes` ya funciona para XP.
- **Archivos:** `crates/bedrock-protocol/src/packets/` (nuevos `mob_effect.rs`, `level_event.rs`), `session.rs` (aplicar acciones), `command.rs` (mensajes ya ok).

### 2.5 Plugin — casi completo; huecos en metadata y orden

`plugin.php` (2 683 líneas, el fichero más grande del bridge) + `description.rs` + `manager.rs` (1 086 líneas, orden topológico por dependencias, `PluginEnableOrder`) cubren el ciclo de vida PM#: `onLoad`/`onEnable`/`onDisable`, `PluginBase::{getDataFolder, saveResource, getConfig, saveConfig, reloadConfig, saveDefaultConfig, registerEvents, getCommand, getLogger, getScheduler, getServer}`, `PluginManager` (reflexión de listeners, `unregisterEvents` al desactivar), autoloader PSR-4 con soporte `phar://` y `vendor/autoload.php`, `plugin.yml` con `api`/`depend`/`softdepend`/`load-before`/`commands`/`permissions`.

**Falta (P2):**
- `PluginDescription`: campos `requiredExtensions`, `libraries` (descarga de libs), `accessProtocol`, `order`/`prefix`/`website`/`map` (procesados de forma parcial en `description.rs` — el test de `description.rs` cubre `commands`+`permissions`+aliases, no las extensiones req.).
- `PluginCommand`: existe, pero su sincronización con `plugin.yml → CommandMap` y con el cliente (`AvailableCommandsPacket` desde `COMMANDS` sin flags de permiso) está documentada como deuda en `plan-compatibilidad.md` §3 (fase 3.4).
- `PluginLogger`/`PluginException`/`PluginEnableEvent`/`PluginDisableEvent` existen (ver wiki `10-eventos.md`); `PluginEvent` existe.

- **Prioridad: P2.** EconomyLite/TestKit declaran `softdepend` y cargan; un plugin con dependencias duras o librerías externas (p. ej. EconomyAPI) fallaría en la fase de carga, no en runtime.
- **Archivos:** `crates/php-plugin-bridge/src/description.rs`, `src/manager.rs`, `crates/php-plugin-bridge/php/plugin.php`.

### 2.6 Scheduler — 10 métodos PM todos presentes; `AsyncPool` sin pool real

`TaskScheduler` PM: `scheduleTask`, `scheduleDelayedTask`, `scheduleRepeatingTask`, `scheduleDelayedRepeatingTask`, `cancelAllTasks`, `isQueued`, `mainThreadHeartbeat`, `setEnabled`, `shutdown` (+ ctor). El bridge los tiene todos, con las firmas PM5 y la forma abreviada `(Task, delay/period)` que usan los plugins instalados (`scheduleRepeatingTask(new ClosureTask(...))` en BetterChat/ChatGuard/EconomyLite/TestKit, verificado en §3.2). El reloj Rust es **solo un contador**: `main.rs` emite un tick cada 50 ms (20 TPS, `MissedTickBehavior::Skip`) por IPC; todo el scheduling corre en el hilo PHP. `scheduler.php` además define `AsyncTask`, `AsyncTaskHandler`, `AsyncWorker`, `AsyncPool` y `BulkCurlTask` (127 métodos en total) — pero sin workers reales: la compresión (`Server::prepareBatch`) es sync y `getAsyncPool()` devuelve `null` (ver §2.1).

- **Prioridad: P2.** Los 4 plugins instalados usan solo tareas sync ✅. Un plugin con I/O async (DB, HTTP) se ejecutará inline en el hilo principal: funciona, pero bloquea el tick. No es un crash, es un problema de rendimiento.
- **Archivos:** `crates/php-plugin-bridge/php/scheduler.php`, `src/runtime/php.rs` (workers), `src/manager.rs` (`tick_sync`).

### 2.7 Permission — el módulo más paritario; un gate P1

`permission.php` (778 líneas) espeja PM5: `Permission` con `default: true|false|op|notop`, hijos, `PermissibleInternal` con attachments, `PermissionManager` (`addPermission`/`removePermission`/`getPermission`/`getPermissions`/`getDefaultPermissions`/`recalculatePermissionDefaults`/`subscribeToPermission`/`unsubscribeFromPermission`/`clearPermissions`/`hasPermission`), `DefaultPermissionNames` con los **61 nodos** (56 de comando + 2 broadcast + 3 raíces `group.*`), `BanEntry`/`BanList` tipados. Rust replica: `permission.rs` (568 líneas, 63 `pub const COMMAND_*` + `PermissionManager` con `default`/hijos/attachments), `access.rs` (524 líneas, `BanEntry::from_string`/`get_string`, `AccessList` con bans de nombre/IP, whitelist, `denial()` en login), `CommandSource::can()` delegando en el manager y gate `self|other` correcto (incl. nodos por subcomando en `/whitelist` y `/time`).

**Falta (P1):**
1. **El gate desde plugins ignora el manager:** el host `server.has_permission` en `plugin_host.rs` solo comprueba `is_op` (`self.ops.contains(name)`). Un `PermissionAttachment` concedido por un plugin (p. ej. BetterChat concede `betterchat.*`) **no cambia** `Server::has_permission()` del host, aunque el gate de comandos de Rust sí lo respeta. Los permisos “de plugin” funcionan por un camino (comandos) y no por otro (host calls).
2. `group.operator`/`group.user`/`group.console` sin efecto (PM les asigna attachments op/user). La consola se modela `is_op = true`, así que un attachment que deniegue un nodo a la consola no se respeta (documentado en `plan-compatibilidad.md` §5.1).
3. Comandos propios (`spawn`, `weather`) aún con `can_use_privileged()` en vez de nodo propio (deuda menor, no rompe paridad PM).

- **Prioridad: P1** por el punto 1 (un plugin que concede permisos y luego los consulta por host ve `false`). El resto P2.
- **Archivos:** `crates/server-core/src/plugin_host.rs` (`server.has_permission`), `permission.rs` (attachments por jugador puenteados al host), `session.rs` (inspección del manager al despachar acciones).

### 2.8 World — 132 métodos PM, 34 presentes; un solo mapa sin luz ni estados

**Rust:** `world_state.rs` (770 líneas) = un único `WorldState` (“world”): reloj (`time`, `start/stop_time`), clima (`0=clear/1=rain/2=thunder`), dificultad, spawn/seed, **ediciones** (`set_block`/`block_at`, colas `BLOCK_CHANGE_QUEUE_CAP 512` para Bedrock `UpdateBlock` + puente Java), caché de chunks, persistencia `world.json` (formato propio v2: hora, clima, dificultad, spawn, semilla, ediciones — **no** `level.dat`). `bedrock-world`: `chunk`/`subchunk`/`paletted_block_array` + `network.rs` (serialización de red) + generadores `flat`/`normal`/`biome`/`caves`/`noise/simplex` + populators (árboles, ores, hierba) + `java_chunk.rs` (`ModernChunk`/`ChunkBuilder` 1 007 líneas). **Sin modelo de luz** (ni cielo ni bloque), **sin estados de bloque** (`block_id.rs` es un mapa de ids planos; `data/canonical_block_states.nbt` existe pero sin lector), **sin tiles** (block entities), **sin multi-mundo** (`target/` dice “hash map de mundos” como fase 8; el cross-world reinstancia un `WorldState` limpio best-effort según wiki `17-mundo-persistencia.md`).

**PHP:** `world.php` (1 351 líneas) con `BlockFactory` (110+ nombres legacy verificados contra `block_id.rs`, fichero honesto sobre el fallback a ids legacy), `Block`, `Chunk`, `World`, `WorldManager` (71 métodos públicos en total). `World::getBlock/setBlock` van por host calls (`world.get_block/set_block`); `getHighestBlockAt`, `getTime/setTime`, `getDifficulty/setDifficulty`, `getSeed`, `save`, `getAutoSave/setAutoSave*`, `getChunk` (hex de subchunks, para el bridge Java) reales. **Stubs:** `dropItem()` → `null`; `addParticle()`/`addSound()` no-op documentado; `getBiomeId()` local.

**Ausentes en PHP (98/132), agrupados:**
- **Entidades/tiles:** `addEntity`, `removeEntity`, `addTile`, `removeTile`, `getTile`, `getTileAt`, `getEntity`, `getEntities`, `getChunkEntities`, `getNearbyEntities`, `getNearestEntity`, `getCollidingEntities`, `getCollisionBlocks`, `onEntityMoved`.
- **Chunks/loaders:** `getLoadedChunks`, `getChunkPlayers`, `getChunkLoaders`, `getChunkListeners`, `registerChunkListener/Loader`, `unregisterChunkListener(All)`, `loadChunk`, `unloadChunk(Request)`, `lockChunk/unlockChunk`, `isChunkLoaded/Generated/Populated/Locked/InUse`, `requestChunkPopulation`, `getOrLoadChunkAtPosition`, `getAdjacentChunks`, `setChunk`, `cancelUnloadChunkRequest`, `orderChunkPopulation`, `registerTickingChunk/unregisterTickingChunk`, `isInWorld/isInLoadedTerrain/isSpawnChunk`, `isChunkInUse`.
- **Luz:** `getBlockLightAt`, `getFullLight`, `getFullLightAt`, `getPotentialLight`, `getPotentialLightAt`, `getPotentialBlockSkyLightAt`, `getRealBlockSkyLightAt`, `getHighestAdjacent*` (x6), `computeSkyLightReduction`, `computeSunAnglePercentage`, `getSkyLightReduction`, `getSunAngleDegrees/Percentage/Radians`, `updateAllLight`, `getMaxY/getMinY`, `getBiome` (sí `getBiomeId` local), `setBiomeId`, `sendTime`, `getTimeOfDay`, `getProvider`, `getServer`, `getLogger`, `getTickRateTime`, `isDoingTick`, `doChunkGarbageCollection`, `getTickingChunks`, `getRandomTickedBlocks`, `addRandomTickedBlock/removeRandomTickedBlock`, `notifyNeighbourBlockUpdate`, `scheduleDelayedBlockUpdate`, `createBlockUpdatePackets`, `setSleepTicks`, `checkSleep`, `broadcastPacketToViewers`, `getViewersForPosition`, `addOnUnloadCallback/removeOnUnloadCallback`, `onUnload`, `clearCache`, `saveChunks`, `useBreakOn`, `useItemOn`, `getBlockAt` (sí existe), `dropExperience`.

- **Prioridad: P1** (multi-mundo y luz por separado podrían ser P2, pero `dropItem`/`addParticle`/`addSound` y los chunks/loaders son P1 porque los plugins los llaman para efectos visibles y reciben “nada”).
- **Archivos:** `crates/server-core/src/world_state.rs` (multi-mundo = `HashMap<String, Arc<WorldState>>`, luz, tiles), `crates/bedrock-world/src/block_id.rs` (estados), `crates/php-plugin-bridge/php/world.php` (métodos), `crates/server-core/src/plugin_host.rs` (host de partículas/sonido/tile).

### 2.9 Item — 59 métodos PM, 37 presentes; sin comportamientos ni `StringToItemParser`

`item.php` (288 líneas): `Item` con estado (id, meta, count, custom name, lore, NBT, damage, encantamientos como array) + `ItemFactory` (`registerItem`, `getFromString`, `isRegistered`, `getCreativeInventory`, `clearCreativeItems`…). **Ausentes (41/59):** `canBePlaced`, `canStackWith`, `equalsExact`, `getAttackPoints`, `getBlock`, `getBlockToolHarvestLevel/Type`, `getCanDestroy/setCanDestroy`, `getCanPlaceOn/setCanPlaceOn`, `getCooldownTag/Ticks`, `getDefensePoints`, `getEnchantability`, `getEnchantmentTags`, `getMaxStackSize`, `getMiningEfficiency`, `getFuelTime`, `getFuelResidue`, `getStateId`, `jsonSerialize`, `nbtSerialize`, `pop`, `keepOnDeath/setKeepOnDeath`, `onAttackEntity`, `onClickAir`, `onDestroyBlock`, `onInteractBlock`, `onInteractEntity`, `onReleaseUsing`, `onTickWorn`, `getNamedTag/setNamedTag`, `hasNamedTag`, `clearNamedTag`, `getPlacementTransaction`, `isFireProof`, `getEnchantmentTags`, `hasCustomBlockData`, `clearCustomBlockData` (`set/getCustomBlockData` sí existen).

En PM5 los items son **~150 clases con comportamiento** (herramientas, comida, cubos, pociones…); aquí solo hay datos. El registro Rust (`inventory.rs` + `data/item_ids.txt`, 834 entradas BDS, meta, NBT vía `nbt_json.rs`) resuelve `resolve_item`/`item_network_id`/`item_display_name` y `/give` con NBT de red — suficiente para dar items, insuficiente para usarlos con su comportamiento (comer, picar, pociones).

**Ausente estructural:** `StringToItemParser` (PM5) — el bridge deja `ItemFactory::getFromString` legacy; un plugin PM5 que haga `StringToItemParser::parse("diamond_sword")` recibe *Class not found*.

- **Prioridad: P1** (parsing + comportamiento mínimo de comida/herramientas; los encantamientos como sistema son P2).
- **Archivos:** `crates/php-plugin-bridge/php/item.php`, nuevo `crates/server-core/src/enchant.rs`, `inventory.rs` (consumables).

### 2.10 Block — 70 métodos PM, 8 presentes; sin estados ni comportamientos

`world.php` implementa `BlockFactory` (mapa legacy 110+ entradas, header honesto: solo las hashes `block_id.rs` verificadas se renderizan bien; el resto se graba como edición y puede renderizar mal) y `Block` mínimo (`getId`/`getMeta`/`getTypeId`/`getName`/`getPosition`/`getX/Y/Z`/`getWorld`/`setTypeId`/`setType`/`setBlock`/`isSolid`). **Ausentes (62/70):** coordenadas/posiciones con estado (`getStateId`, `position`, `readStateFromWorld`, `writeStateToWorld`, `asItem`, `getPickedItem`), drops (`getDrops`, `getDropsForCompatibleTool`, `getSilkTouchDrops`, `getXpDropForTool`, `getAffectedBlocks`), física/colisión (`getCollisionBoxes`, `calculateIntercept`, `canBePlacedAt`, `canBeReplaced`, `canClimb`, `collidesWithBB`, `hasEntityCollision`, `isFullCube`, `isTransparent`, `blocksDirectSkyLight`), interacciones (`onBreak`, `onInteract`, `onPostPlace`, `onAttack`, `onEntityLand/Inside`, `onNearbyBlockChange`, `onRandomTick`, `onScheduledUpdate`, `onIncinerate`, `onProjectileHit`, `place`, `getSide`, `getAllSides`, `getHorizontalSides`), redstone/fuego (`getFlammability`, `burnsForever`…), metadatos (`getIdInfo`, `getTypeTags`, `hasTypeTag`, `getLightLevel/Filter`, `getMaxStackSize`, `getFuelTime`, `getEnchantmentTags`, `getModelPositionOffset`, `isFireProofAsItem`, `getSupportType`, `describeBlockItemState`, `generateStatePermutations`, `hasSameTypeId`, `isSameState`, `getBreakInfo`, `ticksRandomly`, `canBeFlowedInto`, `canBePlaced`, `isAffectedBySilkTouch`, `getHorizontalSides`…).

En Rust hay **ids planos** sin estados (`block_id.rs`, 79 líneas) y persistencia por ediciones (`world.json`). Los datos necesarios (`data/canonical_block_states.nbt`, `block_properties_table.json`, `block_state_meta_map.json`) ya están en `data/` pero sin lector registrado en el bridge.

- **Prioridad: P1** (estados + drops + `BlockBreak/Place` con `getDrops` reales; el resto P2/P3). TestKit solo usa `getName`/`getTypeId`/`getId` ✅, por eso no lo nota hoy.
- **Archivos:** `crates/bedrock-world/src/block_id.rs`, `crates/server-core/src/world_state.rs`, `crates/php-plugin-bridge/php/world.php`, `crates/server-core/src/inventory.rs` (drops).

### 2.11 Entity — 0 clases en el bridge; el sistema entero falta

PM5: `Entity` (105 métodos), `Living` (46), `Human` (25), `ExperienceManager` (22), `HungerManager` (17), efectos (20 ficheros: `EffectInstance`, `EffectManager`, `StringToEffectParser`, 12 efectos concretos), `Attribute`/`AttributeMap`/`AttributeFactory` (12 atributos vanilla), proyectiles, objetos (`ItemEntity`, `XpOrb`, `FallingBlock`), animales/hostiles (Squid, Villager, Zombie), `Location` (en `entity`, no `world`).

Aquí: **ninguna clase**. En Rust existen 6 paquetes de visibilidad (`add_player`, `remove_actor`, `move_actor_absolute`, `set_actor_data`, `actor_event`, `mob_equipment` parcial) + `session.rs` con `canSee`/`hidePlayer`/`showPlayer` (visibilidad jugador↔jugador) + `PluginAction::SetXp` que sí envía `UpdateAttributes`. Todo lo demás es stub:

- `CommandAction::ApplyEffect` → solo `debug!` (efecto con feedback, sin efecto).
- `CommandAction::AddXp` → solo `debug!` (XP de comando sin efecto, inconsistente con `SetXp` de plugin).
- `World::dropItem()` → `null` (sin `ItemEntity`).

**Puente PHP↔Rust incompleto:** la clase hipotética `pocketmine\entity\Entity` no existe; `EntityEvent` usa arrays genéricos (ver §2.3). Un plugin que haga `new Entity(...)`, `getServer()->createPlayer()` o `$world->addEntity()` recibe *Class not found*.

- **Prioridad: P1** (sube a P0 para cualquier minijuego que spawnee entidades). Orden de ataque sugerido: 1) `mob_effect.rs` (efecto real, unlock `/effect`), 2) `ApplyEffect`+`AddXp` cableados (unlock `/xp`), 3) `Entity` mínimo (`spawnTo`, `getWorld`, `getPosition`, `teleport`, `close`) + `ItemEntity` (unlock `dropItem`), 4) daño/muerte (`EntityDamage/DeathEvent`).
- **Archivos:** nuevos `crates/server-core/src/entity.rs`, `effect.rs`; `crates/bedrock-protocol/src/packets/add_actor.rs` (nuevo), `mob_effect.rs` (nuevo), `session.rs` (spawnear al haber listeners), `plugin_host.rs`.

### 2.12 Scoreboard / BossBar — ✅ completos (plugins externos, ambos lados)

Portados desde `pocketmine\scoreboard\*` + `xenialdan\apibossbar` con el patrón “PHP dueño del objeto, Rust difunde”. Rust: `scoreboard.rs` (670 líneas: `Scoreboard`, `ScoreboardObjective`, `ScoreboardLine`, `ScoreboardSlot`, `ColorRange`, paquetes Bedrock `0x6A/0x6B/0x6C/0x70` + diff + persistencia `scoreboard.json`), `bossbar.rs` (916: `BossBar`, `DiverseBossBar` con overrides por jugador, paquetes Bedrock `BossEventPacket 0x4A` + Java `boss_bar` para 769-775 y 340). PHP: `scoreboard.php` (332, 37 métodos) + `bossbar.php` (525, 45) + acciones `set_scoreboard`/`remove_scoreboard`/`set_bossbar`/`remove_bossbar` en `PluginAction`. Comando nativo `/scoreboard` (`sb`). `ScoreboardDemo` lo ejercita ✅. El wiki `15-scoreboard-bossbar.md` lo documenta en profundidad.

- **Prioridad: P3** (solo verificar edge cases: overrides `DiverseBossBar` tras reconnect, `ColorRange::color_code()` en Java). No requiere cambios estructurales.

### 2.13 Inventory — P0: el inventario Rust y el PHP no están conectados

Rust (`inventory.rs` 857 + `crafting.rs` 1 272 + `anvil.rs` 393 + `packets_wire.rs`) es **autoritativo y real**: 4 ventanas (inventario 36, armadura 4, offhand 1, cursor 1) + crafteo 2x2/3x3, `apply_actions` atómico (rechazo total + resync de las 4 ventanas si el cliente diverge), límite 50 acciones, `clear_matching` con NBT (paridad con `Item::equals()`), `ItemStackRequest` (motor en `crafting.rs`), creativo desde `data/creative/`, yunque (dureza, reparación, `prior_work`) y `CraftingDataPacket` embebido (`static_packets.rs`). El cliente se sincroniza contra esto.

PHP (`player.php`, clases `PlayerInventory` + `PlayerCraftingInventory` + `Skin`): **stub en memoria** declarado en `plugin-api-contract.md` (“está en `player.php` con stub en memoria”). **No hay host calls `inventory.*`** (la lista completa en el Anexo A no contiene ninguna) ni `PluginAction` de inventario. En consecuencia:

- `Player::getInventory()` devuelve un `PlayerInventory` PHP que **no refleja** lo que el cliente mueve (y viceversa).
- `Inventory::swap()`, `contains()`, `isSlotEmpty()`, `getAddableItemQuantity()`, `all()`, `slotExists()`, `getListeners()`, `getViewers()`, `onOpen`/`onClose`, `getMaxStackSize`, `canAddItem`, `setMaxStackSize`, `clearAll` (parcial), `getHotbarSize`/`getItemInHand/setItemInHand` (presentes pero locales), `firstEmpty`/`first` (presentes pero locales)… de los 25 métodos de `Inventory`, 15 existen en PHP pero **sin efecto sobre el inventario del cliente**.
- Clases ausentes: `ArmorInventory`, `PlayerOffHandInventory`, `PlayerEnderInventory`, `PlayerCursorInventory`, `PlayerCraftingInventory` (sí como clase simple, sin sync), `CreativeInventory` (presente como objeto), `SimpleInventory`, `DelegateInventory`, `TemporaryInventory`, `CallbackInventoryListener`, transacciones (`InventoryTransaction`, `CreateItemAction`, `DestroyItemAction`, `DropItemAction`, `SlotChangeAction`, `CraftingTransaction`, `EnchantingTransaction`, `TransactionBuilder*`).

- **Prioridad: P0.** Es la mitad del servidor para plugins reales. El camino: host calls `inventory.get_contents/set_contents/clear/add_item/remove_item` + `PluginAction::SetInventory` (o reutilizar `InventoryTransactionPacket` en sentido servidor) + `PluginAction::DropItem` (con `ItemEntity`, ver §2.11).
- **Archivos:** `crates/php-plugin-bridge/php/player.php` (`PlayerInventory`), nuevo `crates/server-core/src/inventory_php.rs` (o host en `plugin_host.rs`), `crates/server-core/src/session.rs` (resync tras escritura PHP).

### 2.14 Crafting — motor Rust real que PHP no puede consultar

`crafting.rs` implementa `CraftingManager` (shaped×2 tamaños, shapeless, horno, cervecería, recetas por índice), `CraftingGrid`, `CraftingInventory`, `PlayerCraftingInventory`, `CraftingTableInventory`, `creative_items()`/`creative_groups()`, `apply_item_stack_request()`, `container_slot()`. PM `CraftingManager` tiene 16 métodos; aquí `getCraftingManager()` devuelve `null` (ver §2.1) y no hay host `crafting.*`. Un plugin que registre recetas (`registerShapedRecipe`) o las consulte no tiene dónde ir. `FurnaceRecipeManager`, recetas de pociones (`registerPotionTypeRecipe`, `matchBrewingRecipe`) y callbacks de registro (`getRecipeRegisteredCallbacks`) también ausentes en PHP.

- **Prioridad: P2.**
- **Archivos:** `crates/php-plugin-bridge/php/core.php`, `crates/server-core/src/crafting.rs`, host `crafting.*` en `plugin_host.rs`.

### 2.15 Chat — ✅ completo

`chat.php` (58 líneas): `ChatFormatter` + `PlaceholderFormatter` (paridad con la firma `{key}` de BetterChat). `PlayerChatEvent` con `getFormatter/setFormatter`, `getRecipients/setRecipients`, `getMessage/setMessage` (verificado en `event.php`, y BetterChat lo usa en §3.2). Rust: `chat.rs` (ruta Bedrock+Java con `handle_java_chat`/`handle_java_command`), `session.rs` (chat → `PlayerChatEvent` → broadcast respetando recipients/cancelación). Wiki: contrato en `plugin-api-contract.md`. Sin huecos.

### 2.16 Form — P0: clase inexistente + paquete inexistente

**PM5 core:** `form/Form.php` (abstracta: constructor con `onSubmit` callable, `handleResponse`, `getTitle`, `getTiles`…) + `FormValidationException.php`. **Los formularios concretos** (`SimpleForm`, `CustomForm`, `ModalForm`, `Button`, `Input`, `Dropdown`, etc.) **no son del core**: viven en el plugin externo `pmmp/ext-forms` (`pocketmine\form\SimpleForm` y aprox. 25 clases). En PM real, un plugin que usa formularios declara dependencia de ext-forms y el servidor carga ese plugin como uno más.

**Aquí:** cero clases. `Player::sendForm(mixed $form): void` es el no-op documentado en `player.php` (“No-op stub: no se emite acción”). En Rust no hay paquete `ModalForm` (`bedrock-protocol` no mod en la lista de 48) ni `PluginAction::SendForm`. Conclusión: un plugin que copie-pega el patrón PM (`new SimpleForm(...)`, `$player->sendForm($form)`) **no carga** (si ext-forms no está instalado) o **parece funcionar pero no hace nada** (si el plugin trae sus propias clases de form sin paquete real detrás).

- **Prioridad: P0** (la clase de la faceta “form” es la que más plugins PM famosos usan — menús, confirmaciones, lobbies).
- **Camino sugerido:** `crates/php-plugin-bridge/php/form.php` con `Form` (paridad `handleResponse`/`onSubmit`), `SimpleForm`, `CustomForm`, `ModalForm` (copiar ext-forms es viable: son clases de datos + cancelable); paquete nuevo `crates/bedrock-protocol/src/packets/modal_form.rs` (0x68 petición cliente, 0x69 respuesta servidor); `session.rs` (enviar + enrutar respuesta a `onFormSubmit`); `PluginAction::SendForm`.
- **Archivos:** esos 4 + `crates/php-plugin-bridge/src/context.rs` (acción).

### 2.17 Data — P3: formato propio JSON, sin NBT ni serializers PM

PM5 `data/` (99 ficheros): `SavedData` (`SavedDataLoadingException`), serializers de items/bloques (`ItemSerializer`, `ItemDeserializer`, `BlockStateSerializer`, `BlockStateDeserializer`, recursos de upgrade `ItemIdMetaUpgrader`/`BlockStateUpgrader`), mapas de ids (`EnchantmentIdMap`, `EffectIdMap`, `PotionTypeIdMap`, `BannerPatternTypeIdMap`, `DyeColorIdMap`…).
Aquí: `data/` del repo contiene JSON efímeros (`item_ids.txt`, `item_tags.json`, `recipes/`, `creative/`, `biome_definitions.json`, `canonical_block_states.nbt` sin lector, `block_properties_table.json` sin uso) + persistencia JSON propia (`world.json` v2, `players/<nombre>.json` con 26 campos, `scoreboard.json`, `balances.json` de plugins). La API no expone `SavedData` ni serializers: un plugin que guarde `Item` con `nbtSerialize()`/`getNamedTag()` (NBT tipo PM) se topa con NBT de red propio de `bedrock-protocol/nbt.rs` (subconjunto: `Tag` + `write_network_nbt`) y `nbt_json.rs` (inferencia `JsonNbtParser`).

- **Prioridad: P3.** Decidir si se adopta NBT (`player.dat`/`level.dat`) o se declara el JSON como formato propio (honesto y ya en uso). Los plugins que lean saves PM externos fallarán en ambos casos hasta la migración.
- **Archivos:** nuevo `crates/server-core/src/data.rs` + `crates/server-core/src/nbt_json.rs` si se adopta NBT.

### 2.18 Utils / TextFormat / Config / Math — mayormente ✅

- `TextFormat` (`text.php`): constantes completas PM5 (incl. `MINECOIN_GOLD` y materiales `h–v`), `colorize`, `clean`, `stripColors`, `toANSI`, `getByChar/Name`, `addFormat`, `tokenize`, `toHTML`. ✅
- `Utils` (PM ≈30 métodos): bridge 6 (`getRandomFloat`, `getRandomInt`, `getCurrentTicks`, `getNiceTickCount`, `getNiceClassName`, `validateArgumentCount`, `assocToIndexed` — en realidad 7 contando `validateArgumentCount`). Faltan `logRotation`/`tidyPath`, `strEndsWith`/`strStartsWith`, `compare`/`json_encode` helpers y los utilidades de fechas. **P3** (los plugins suelen reimplementarlos localmente).
- `Config` (`config.php`, 16 métodos): YAML/JSON/properties, `get`/`getNested`/`set`/`setNested`/`remove`/`removeNested`/`exists`/`getAll`/`setAll`/`setDefaults`/`getPath`/`getType`/`save`/`reload`/`load`. Faltan las constantes `Config::MODE_*` (detected, yaml, json) y `save()` atómico sobre el fichero real ya documentado como deuda en `core.php`. BetterChat/ChatGuard/EconomyLite usan `get`, `getNested`, `save`, `saveResource`, `getDataFolder` ✅ (ver §3.2).
- `math.php` (77 métodos): `Vector3`, `Vector2`, `AxisAlignedBB`, `Position`, `Location` (incl. alias `pocketmine\math\Position`) completos. ✅
- `Skin` (en `player.php`, ns `pocketmine\entity`): identidad (`skinId`, cape, `SkinData`) derivada de xuid; sin skin real del cliente (el paquete `PlayerListPacket` con serializador de skins está en deuda según `plan-compatibilidad.md` fase 4). **P2**, pero pega a la visibilidad de jugadores (wiki fase 4).
- `Translations`: Rust `translations.rs` lee `spa.ini`+`eng.ini` embebidos + test constante-por-clave (25 constantes). PHP-side no hay `Language`/`KnownTranslationFactory` (p. ej. plugins que hagan `$player->translateString()` o `LocalizedTranslation`). **P2.**

### 2.19 Red, Thread, Timings, Console, ResourcePacks, Lang, Crash, Updater, Stats, Promise — P3

| Subsistema | PM | Aquí | Nota |
|---|---|---|---|
| `network/` (85) | `PlayerNetworkSession`, sesiones RakNet, compresión, paquetes `DataPacket` | `bedrock-protocol` (27/248 paquetes efectivos; 48 módulos), `raklib-rs`, `nethernet-rs` | Eventos `DataPacketDecode/Receive/SendEvent` imposibles de disparar sin intercepción; plugins de seguridad/red avanzados no aplican |
| `crafting/` expuesto | (ver §2.14) | motor Rust real | P2 |
| `thread/` (13) | `AsyncTask`, workers | `scheduler.php` define las clases, ejecución inline | P2 (ver §2.6) |
| `timings/` (3) | `TimingsHandler` con árbol | `metrics.rs` (TPS 1m/5m/15m) + `/tps`; `/timings` stub | P3 |
| `console/` (5) | consola interactiva | se lee de stdin (`main.rs`); `ServerCommandEvent` cableado | P3 |
| `resourcepacks/` (9) | packs con accept/refuse | handshake básico (`resource_packs.rs`) | P3 |
| `lang/` (3) | `Language`, `Translation` | `translations.rs` Rust-only | P2 (ver §2.18) |
| `crash/` (5) | dumps | `crash.rs` (mensajes + archivo, `MIN_UPTIME_SECS 120`) | P3 |
| `updater/` (3) | auto-update | `updater.rs` (stub `has_update=false`, `do_check(){}`) | P3 |
| `stats/` (1) | `Statistics` | `query.rs` (GameSpy query) — distinto pero útil | P3 |
| `promise/` (3) | `Promise` | ausente | P3 |
| `wizard/` (1) | setup interactivo | `ServerConfig::load_or_create` materializa `pocketmine.yml`/`server.properties` | P3 |

---

## 4. Compatibilidad con plugins

### 4.1 Plugins **normalmente** usados en PM (matriz cualitativa)

| Tipo de plugin PM | Estado hoy | Qué lo bloquea | Prioridad de desbloqueo |
|---|---|---|---|
| Chat/moderación (BetterChat, ChatGuard, SimpleChat, anti-spam) | ✅ cargan y operan | — | — |
| Utilidades staff (Kits, Warps, Homes, Spawn — con `WorldManager` de 1 mapa) | 🟡 | solo 1 mundo; `World::getBlock` con ids planos | P2 (multi-mundo) |
| Economía (EconomyAPI, MultiWorld-Economy, PurePerms) | 🟡 | inventario stub (§2.13); sin `StringToItemParser` (§2.9) | **P0/P1** |
| Protección de bloques (GriefPrevention/Island-related) | 🟡 | `BlockBreak/Place` cableados ✅; estados/drops §2.10 limitan | P1 |
| Minijuegos (arena, kits, efectos, NPCs) | 🔴 | sin entidades (§2.11), sin efectos (§2.4), sin forms (§2.16) | **P1** |
| Tiendas (ChestShop, QuickShop) | 🔴 | inventario PHP↔Rust (§2.13), transacciones, entidades `ItemEntity` | P0/P1 |
| Mini-juegos single/duo locales (dados, votación por chat) | ✅ (BetterChat lo trae) | — | — |
| Plugins con `AsyncPool` (DB, webhooks) | 🟡 (funcionan, bloquean) | sin pool real (§2.6) | P2 |
| Plugins con UI (menús, confirmaciones) | 🔴 | sin `Form` (§2.16) | **P0** |
| Plugins de rank/permisos (PurePerms) | 🟡 | gate host ignora attachments (§2.7) | P1 |
| Plugins que lean migraciones PM (`player.dat`) | 🔴 | JSON propio (§2.17) | P3 |
| Plugins de red avanzada (interceptar paquetes) | 🔴 | sin `DataPacket*Event` (§2.19) | P3 |

### 4.2 Plugins instalados en `plugins/` (verificación de uso real)

Método: extracción de todas las llamadas `$obj->metodo()` y `use pocketmine\*` en `plugins/**/*.php`.

**BetterChat** — `use pocketmine\command\Command, CommandSender; console\ConsoleCommandSender; event\Listener, event\player\PlayerChatEvent, PlayerJoinEvent, PlayerQuitEvent, event\server\CommandEvent; player\Player; plugin\PluginBase; scheduler\ClosureTask, TaskHandler; utils\Config, TextFormat`. El `ChatListener` usa: `getPlayer`, `getMessage`, `setMessage`, `cancel`, `setRecipients`, `setFormatter(…$chatFormatter)` + `getSender`, `getCommand`, `setQuitMessage`, `setJoinMessage`, `getWorld()->getFolderName()`, `getDisplayName`, `hasPermission`, `hasOpPermission`, `sendMessage`, `getOnlinePlayers`, `getNested`, `msg` (método propio). Todos existen en el bridge (incl. `setFormatter` tipado contra `pocketmine\player\chat\ChatFormatter`, que BetterChat satisface con su propia implementación de la interfaz ✅). `getOnlineMembers`/`getActiveGroup` son métodos **propios del plugin**, no de PM. BetterChat no usa inventario, entidades, forms ni AsyncPool. **Estado: cargado y funcional** (verificado en HANDOFF con el harness histórico).

**ChatGuard** — `PlayerChatEvent` (`getPlayer`, `getMessage`, `setMessage`, `hasPermission`), `getCommandMap()->register("chatguard", …)` (existe en `command.php` ✅), `getScheduler()->scheduleRepeatingTask(new ClosureTask(...))` ✅, `saveDefaultConfig`, `Config::get` ✅. **Funciona.**

**EconomyLite** — `PluginBase`, `PlayerJoinEvent`, `CommandExecutor`, `ClosureTask`, `getOnlinePlayers`/`getName`, `getDataFolder` + `balances.json` propio, `getConfig`/`save`/`getLogger`. No toca inventario ni entidades. **Funciona.**

**ExamplePlugin** — join + `/hello` ✅. **TestKit** — `PlayerJoin/Quit/Move/Interact/BlockBreak` + `getBlock()->{getX,getY,getZ}`, `getPosition()->add()`, `getWorld()` (`getName`/`getDisplayName` — PosCommand hace `method_exists($world, "getName") ? $world->getName() : $world->getDisplayName()`, y `World::getName()` existe ✅), `getDescription()->getCompatibleApis()/getApi()` (existen en `description.rs` y en `PluginDescription` PHP ✅), `getPluginManager()->getPlugin()`, `getTick`, `getTicksPerSecond`. **Funciona.** Nota: `TestKit/Main.php::getPluginApiVersions()` es **método propio del plugin** (no es API PM), no confundir con la API.

**ScoreboardDemo** — `pocketmine\scoreboard\{ColorRange,Scoreboard,ScoreboardObjective,ScoreboardSlot}` + `getScoreboard/setScoreboard` en `player.php` ✅ (completo, ver §2.12).

**Conclusión compatibilidad 6/6** en lo que ejercitan hoy; el siguiente plugin PM real que uses pega con §2.13/P0 o §2.16/P0 en ≤10 minutos de prueba.

---

## 5. Backlog ordenado (P0 → P3) con archivos a tocar

### P0 — ahora

| # | Tarea | Por qué | Archivos |
|---|---|---|---|
| P0-1 | **Sincronizar inventario PHP↔Rust** (host `inventory.*` + `PluginAction::SetInventory` + resync) | Doble fuente de verdad; rompe economía/tiendas | `player.php`, `plugin_host.rs`, `inventory.rs`, `session.rs`, `context.rs` |
| P0-2 | **Clases `pocketmine\form\*` + paquete `ModalForm`** | Hoy no-op; plugin con UI = crash | Nuevo `php/form.php`, nuevo `bedrock-protocol/.../modal_form.rs`, `session.rs`, `context.rs` |
| P0-3 | **Efectos reales de `effect`/`xp`/`enchant`/`particle`** (hoy log-only) | Comando visible en `/help` sin efecto; `AddXp` inconsistente con `SetXp` que sí funciona | Nuevos `bedrock-protocol/.../mob_effect.rs`, `level_event.rs`; `session.rs`, `command.rs` |
| P0-4 | **Clase `Entity` mínima + `ItemEntity`** (desbloquea `dropItem` y cualquier `spawnTo`) | Sin ella no hay drops ni NPCs | `php/entity.php` (nuevo), `server-core/src/entity.rs` (nuevo), `add_actor.rs` (nuevo), `world.php` |

### P1 — siguiente iteración

| # | Tarea | Archivos |
|---|---|---|
| P1-1 | `BlockPlaceEvent` ya cableado ✅ (corregir `EVENT_TRIGGERS.md`, que dice NO — ver §6). Verificar con test live. | `session.rs` (solo test) |
| P1-2 | Gate `server.has_permission` contra `PermissionManager` (attachments incluidos) | `plugin_host.rs`, `permission.rs`, `session.rs` |
| P1-3 | `StringToItemParser` + comportamientos básicos (comida/herramienta) + `getMaxStackSize` | `item.php`, `inventory.rs`, nuevo `enchant.rs` (sistema de encantamientos) |
| P1-4 | Estados de bloque (lector `canonical_block_states.nbt`) + `getDrops` reales | `bedrock-world/block_id.rs`, `world_state.rs`, `world.php` |
| P1-5 | Multi-mundo (`HashMap<String, Arc<WorldState>>`) + `WorldManager` real | `world_state.rs`, `main.rs`, `world.php` |
| P1-6 | `EntityDamageEvent`/`EntityDeathEvent` + sistema de daño/muerte (drops, respawn) | `entity.rs`, `session.rs`, `event.php` |
| P1-7 | `Server::getUpdater/getAsyncPool/...` (null → objetos mínimos o excepciones claras) | `core.php` |
| P1-8 | `Player::getLanguage()` + `LocalizedTranslation` (idioma por jugador) | `player.php`, `core.php`, `translations.rs` |
| P1-9 | **Bug wire `SetTitlePacket`: `create_set_title_packet` escribe 0x59, el id real es 0x58** (afecta `send_title` de plugins; ver §6 corrección 4) | `crates/server-core/src/session.rs` (usar el tipado `SetTitlePacket`) |

### P2

| # | Tarea | Archivos |
|---|---|---|
| P2-1 | `AsyncPool` real (workers fuera del hilo principal) | `scheduler.php`, `manager.rs`, `runtime/php.rs` |
| P2-2 | Flags de permiso en `AvailableCommandsPacket` (fase 3.4 de `plan-compatibilidad.md`) | `command.rs`, `bedrock-protocol/.../available_commands.rs` |
| P2-3 | `getCraftingManager()` PHP con el motor Rust (host `crafting.*`) | `core.php`, `crafting.rs`, `plugin_host.rs` |
| P2-4 | `PluginDescription`: `requiredExtensions`, `libraries`, `accessProtocol` | `description.rs`, `manager.rs` |
| P2-5 | Compatibilidad `getPlayerCount()` vs `getOnlinePlayerCount()` + `Skin` real (paquete `PlayerListPacket` con skin) | `core.php`, `bedrock-protocol/.../player_list.rs` |
| P2-6 | `Utils::` faltantes + constantes `Config::MODE_*` | `text.php`, `config.php` |

### P3

| # | Tarea | Archivos |
|---|---|---|
| P3-1 | Luz (cielo/bloque) + `UpdateBlockPacket` completo | `bedrock-world/*`, `world_state.rs` |
| P3-2 | `SavedData` + decisión NBT/JSON (`level.dat`/`player.dat`) | Nuevo `server-core/src/data.rs`, `nbt_json.rs` |
| P3-3 | `DataPacketDecode/Receive/SendEvent` + `Promise` + `timings` con datos + `Stats` | `session.rs`, `metrics.rs` |
| P3-4 | Crash dumps legibles + CI (`cargo fmt --check`, `clippy`, `cargo test --workspace`, `php -l`) | `crash.rs`, CI (fase 9) |
| P3-5 | Scoreboard/BossBar: casos borde (reconnect con overrides `DiverseBossBar`) | `bossbar.rs`, `session.rs` |

---

## 6. Relación con la documentación existente (y correcciones)

| Documento | Qué cubre | Relación con este informe |
|---|---|---|
| `docs/wiki/10-eventos.md` (633 líneas) | Catálogo documentado de eventos (triggers, prioridades, cancelación) | No se duplica: aquí solo contamos **clases y triggers reales** y citamos la wiki como fuente del catálogo |
| `docs/wiki/11-permisos.md` | Los 61 nodos, attachments, bans | Se confirma y se añade el hueco del gate por host (§2.7 P1-2) |
| `docs/wiki/12-comandos.md` | Dos capas (parser Rust + jerarquía PHP), `CommandSource` | Se confirma; §2.4 detecta las 8 acciones stub que la wiki no lista |
| `docs/wiki/13-scheduler.md` | Tick 20 TPS, tareas, async | Se confirma; §2.6 añade “`getAsyncPool()` = null” |
| `docs/wiki/14-inventory-crafting.md` | `inventory.rs`, `crafting.rs`, yunque, `ItemStackRequest` | Vista Rust-centrada: el hueco PHP↔Rust (§2.13) no aparece; aquí sí |
| `docs/wiki/15-scoreboard-bossbar.md` | Todo scoreboard/bossbar | Se confirma completo (§2.12) |
| `docs/wiki/17-mundo-persistencia.md` | Un solo mundo, JSON, regeneración | Se confirma; §2.8 cuantifica (98/132 métodos) |
| `docs/pendientes-axolotl/plan-compatibilidad.md` | Paridad de los 41 comandos y 248 paquetes | **Desactualizado en dos puntos** (correcciones abajo) |
| `docs/plugin-api-contract.md` | Contrato IPC (host calls, acciones, eventos) | La tabla de `PluginAction` va corta (ver abajo) |
| `crates/php-plugin-bridge/EVENT_TRIGGERS.md` | Estado de cada evento (stub vs cableado) | **Desactualizado en un punto** (ver abajo) |

**Correcciones detectadas (verificar y aplicar a los docs):**

1. **`plan-compatibilidad.md` §2–§3** dice “31 de 41 comandos portados”, “8 pendientes (`effect`, `enchant`, `xp`, `particle`, `status`, `gc`, `timings`, `dumpmemory` 🔴)”. **Hoy son 41/41 parseados, 33 con efecto real y 8 con mensaje correcto pero efecto stub** (`CommandAction::ApplyEffect/AddXp/Enchant/SpawnParticle/Timings/GarbageCollect/DumpMemory` → solo log/`debug!` en `session.rs`, con `gc` reportando ceros honestos). Pasar las 8 filas de 🔴 a 🟡 y actualizar las cifras de §2.
2. **`EVENT_TRIGGERS.md`** (sección Block) dice “`BlockPlaceEvent` NO (requiere `InventoryTransactionPacket`)”. **Hoy sí se dispara**: `session.rs` línea 2251, gateado por `has_listeners("BlockPlaceEvent")`. Corregir a “cableado” con el TODO cerrado o a un test E2E que lo fije.
3. **`plugin-api-contract.md`** § “Acciones de plugin” lista 18 acciones; el enum `PluginAction` en `context.rs` ya tiene **24** (faltan en la doc: `SetHealth`, `SetXp`, `HidePlayer`, `ShowPlayer`, `SendSubTitle`, `SendActionBar`; `SetBossBar`/`RemoveBossBar` ya están pero el DTO `BossBarData` cambió con `entity`/`hidden`/`overrides`). Actualizar la tabla del contrato.
4. **`HANDOFF.md`** §1.3 tabla de `php/*.php` dice “Inventarios, entidades, efectos y encantamientos son stubs en memoria” (sigue válido) pero §1.5 atribuye la terminología `SetTitlePacket 0x59`: **el id real es 0x58 (88)**, como declara `bedrock-protocol/src/packets/set_title.rs` (`packet_id() = 0x58`) y `PORT-ROADMAP.md`. El constructor manual de `session.rs` (`create_set_title_packet`) escribe `write_var_u32(0x59)` y **sí se usa en producción** (líneas 5872/5882/5915/5925, ruta `PluginAction::SendTitle`/`BroadcastTitle`): los títulos enviados por plugins salen con id de paquete incorrecto y los clientes Bedrock los ignoran o responden con error. Corregir a 0x58 o, mejor, eliminar el constructor manual y usar el tipado `SetTitlePacket` (patrón que ya emplean `SendSubTitle`/`SendActionBar`). No es solo un comentario: es un bug en el wire con efecto en plugins. Prioridad **P1** (las filas P0-1…P0-4 de §5 se mantienen porque implican sistemas enteros).

---

## Anexo A — Host calls PHP → Rust (reales, `plugin_host.rs`)

| Método | Efecto |
|---|---|
| `world.get_block` | `id` de `WorldState::block_at` (id Bedrock runtime) |
| `world.set_block` | `WorldState::set_block` + doble encolado (Java poll + broadcast Bedrock) |
| `world.take_block_changes` / `world.drain_changes` | drain de la cola (para el bridge Java) |
| `world.get_highest_block` | `highest_block_at` |
| `world.get_spawn` | spawn + `player_position` |
| `world.get_time`, `world.set_time`, `world.get_time_running`, `world.set_time_running` | reloj del mundo |
| `world.get_difficulty`, `world.set_difficulty`, `world.get_seed` | dificultad/semilla |
| `world.save`, `world.generate`, `world.do_auto_save`, `world.tick` | persistencia y tick |
| `world.get_auto_save`, `world.set_auto_save`, `world.get_auto_save_interval`, `world.set_auto_save_interval` | autoguardado |
| `world.get_chunk` | hex de subchunks (necesario para el puente Java) |
| `player.get` | snapshot `PlayerContext` por nombre |
| `player.has_data`, `player.has_played_before`, `player.load`, `player.save`, `player.save_data` | persistencia JSON v2 (26 campos) |
| `player.update_position`, `player.register`, `player.unregister`, `player.take_messages` | registry virtual (Java) + outbox de mensajes |
| `player.get_by_uuid`, `player.get_by_raw_uuid`, `player.update_health` | identidad y salud |
| `server.get_players`, `server.get_players_full`, `server.get_max_players`, `server.get_motd`, `server.is_op`, `server.has_permission` ⚠️ (solo op — §2.7), `server.save_all`, `server.get_query_info`, `server.get_ops`, `server.set_op` | servidor |

**Sin host para:** inventario (`inventory.*`), entidades (`entity.*`), efectos (`effect.*`), XP (`xp.*`), partículas/sonido (`particle.*`, `sound.*`), crafteo (`crafting.*`), permisos por nodo (`permission.*`), forms (`form.*`), tiles (`tile.*`). Cada uno de esos vacíos es un candidato a los archivos citados en §5.

---

## Anexo B — Cómo revalidar estos números

```powershell
# PM5 de referencia (cuando alguien clone el fork en target/axolotl-pm):
Select-String -Path target\axolotl-pm\src\Server.php -Pattern 'public function' | Sort-Object
Select-String -Path target\axolotl-pm\src\player\Player.php -Pattern 'public function' | Measure-Object | % Count
(Get-ChildItem target\axolotl-pm\src\event -Recurse -Filter *.php).Count

# Bridge PHP:
(Select-String -Path crates\php-plugin-bridge\php\core.php -Pattern 'public function' | % { $_.Line.Trim() }) -join ', '
Select-String -Path crates\php-plugin-bridge\php\player.php -Pattern 'class (\w+)' | % { $_.Line.Trim() }

# Rust:
(grep -c 'pub const COMMAND_' crates/server-core/src/permission.rs)
(Get-ChildItem crates/bedrock-protocol/src/packets -Filter *.rs).Count
Select-String -Path crates/server-core/src/session.rs -Pattern 'AddXp|ApplyEffect' | % LineNumber
```

Requisitos para la revisión: `cargo check --workspace --all-targets`, `cargo test -p server-core --lib`, `cargo test -p bedrock-protocol`, `php -l` sobre los PHP tocados (criterio ya exigido por `PORT-ROADMAP.md`). Este análisis **no** deja código: los conteos son reproducible tal cual con los comandos del Anexo B.
