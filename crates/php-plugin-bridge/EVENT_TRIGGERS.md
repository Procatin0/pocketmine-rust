# Event triggers — pendientes de cableado (equipo visibilidad)

> Los eventos ya existen como **stubs correctos** (PHP + Rust + `EventFactory`) y
> compilan (`php -l`, `cargo check -p php-plugin-bridge`). **No se han tocado**
> `session.rs`, `command.rs`, `world_state.rs` ni `plugin_host.rs`.
> Este documento deja el TODO con `file:line` sugerido para que el equipo de
> visibilidad dispare cada evento desde el core.

Convención en cada stub: `// TODO trigger: <file> ... (visibility team)` en Rust
y `@trigger TODO: ...` en el docblock PHP.

## Player (session.rs)

| Evento | Cancellable | Trigger sugerido | Estado |
| --- | --- | --- | --- |
| PlayerPreLoginEvent | — | `session.rs` handshake pre-auth `TODO: session.rs:PlayerPreLoginEvent` | stub |
| PlayerLoginEvent | ✅ | `session.rs` auth ok `TODO: session.rs:PlayerLoginEvent` | stub |
| PlayerKickEvent | ✅ | `session.rs` kick `TODO: session.rs:PlayerKickEvent` | stub |
| PlayerDuplicateLoginEvent | ✅ | `session.rs` duplicate login `TODO: session.rs:PlayerDuplicateLoginEvent` | stub |
| PlayerCreationEvent | — | `session.rs` creación `TODO: session.rs:PlayerCreationEvent` | stub |
| PlayerDataSaveEvent | ✅ | `session.rs`/`world_state.rs` save `TODO: world_state.rs:PlayerDataSaveEvent` | stub |
| PlayerPostChunkSendEvent | — | `session.rs` chunk send `TODO: session.rs:PlayerPostChunkSendEvent` | stub |
| PlayerViewDistanceChangeEvent | — | `session.rs` view distance `TODO: session.rs:PlayerViewDistanceChangeEvent` | stub |
| PlayerToggleFlight/Sprint/Sneak/Swim/GlideEvent | ✅ | `session.rs` movement toggles `TODO: session.rs:PlayerToggle*Event` | stub |
| PlayerJumpEvent | — | `session.rs` jump `TODO: session.rs:PlayerJumpEvent` | stub |
| PlayerGameModeChangeEvent | ✅ | `session.rs`/`command.rs` gamemode `TODO: command.rs:PlayerGameModeChangeEvent` | stub |
| PlayerTransferEvent | ✅ | `session.rs` transfer `TODO: session.rs:PlayerTransferEvent` | stub |
| PlayerItemUse/Held/Consume/EnchantEvent | ✅ | `session.rs` inventario `TODO: session.rs:PlayerItem*Event` | stub |
| PlayerExperienceChangeEvent | ✅ | `world_state.rs` XP `TODO: world_state.rs:PlayerExperienceChangeEvent` | stub |
| PlayerExhaustEvent | ✅ | `world_state.rs` exhaust `TODO: world_state.rs:PlayerExhaustEvent` | stub |
| PlayerDropItemEvent | ✅ | `session.rs` drop `TODO: session.rs:PlayerDropItemEvent` | stub |
| PlayerBucket/Fill/EmptyEvent | ✅ | `session.rs` bucket `TODO: session.rs:PlayerBucket*Event` | stub |
| PlayerBlockPick/EntityInteract/EntityPickEvent | ✅ | `session.rs` pick/interact `TODO: session.rs:PlayerBlockPickEvent` | stub |
| PlayerEditBook/EmoteEvent | ✅ | `session.rs` book/emote `TODO: session.rs:PlayerEditBookEvent` | stub |
| PlayerBedEnter/LeaveEvent | ✅/— | `session.rs` bed `TODO: session.rs:PlayerBed*Event` | stub |
| PlayerChangeSkin/DisplayNameChangeEvent | ✅/— | `session.rs` skin/name `TODO: session.rs:PlayerChangeSkinEvent` | stub |
| PlayerEnchantingOptionsRequest/MissSwing/RespawnAnchorUseEvent | ✅ | `session.rs` `TODO: session.rs:PlayerEnchantingOptionsRequestEvent` | stub |
| PlayerIdentityVerify/ResourcePackOfferEvent | ✅/— | `session.rs` `TODO: session.rs:PlayerIdentityVerifyEvent` | stub |
| Ya cableados (integración): Join/Quit/Chat/CommandPreprocess/Move/Interact/BlockBreak + Death/Respawn (gated `has_listeners`) + ServerCommand (consola) | — | `session.rs`/`command.rs`/`main.rs` | ✅ |

## Block (world_state.rs)

| Evento | Cancellable | Trigger sugerido | Estado |
| --- | --- | --- | --- |
| BlockBurn/Melt/Spread/Grow/Form/DeathEvent | ✅ (vía Base) | `world_state.rs` fire/tick `TODO: world_state.rs:BlockBurnEvent` | stub |
| BlockExplode/PreExplode/Teleport/UpdateEvent | ✅ | `world_state.rs` `TODO: world_state.rs:BlockExplodeEvent` | stub |
| BlockItemPickupEvent | ✅ | `world_state.rs` pickup `TODO: world_state.rs:BlockItemPickupEvent` | stub |
| BrewingFuelUse/BrewItem/CampfireCookEvent | ✅ | `world_state.rs` `TODO: world_state.rs:BrewingFuelUseEvent` | stub |
| ChestPair/FarmlandHydrationChange/GeyserEruptionPulse/LeavesDecay/StructureGrowEvent | ✅ | `world_state.rs` `TODO: world_state.rs:StructureGrowEvent` | stub |
| SignChangeEvent | ✅ | `session.rs`/`world_state.rs` `TODO: world_state.rs:SignChangeEvent` | stub |
| PressurePlateUpdateEvent | ✅ (vía Base) | `world_state.rs` `TODO: world_state.rs:PressurePlateUpdateEvent` | stub |
| Ya cableados: BlockBreakEvent (+ Interact). BlockPlaceEvent NO (requiere `InventoryTransactionPacket`) | ✅ | `session.rs` | parcial |

## Entity (world_state.rs)

| Evento | Cancellable | Trigger sugerido | Estado |
| --- | --- | --- | --- |
| EntityDamageEvent | ✅ | golpe melee, proyectil, caída/explosión del jugador (`SessionManager::damage_chain`) | disparado |
| EntityDamageByEntityEvent | ✅ | golpe melee del jugador y `Player::attack()` | disparado |
| EntityDamageByChildEntityEvent | ✅ | impacto de proyectil contra mob (`child` = proyectil) | disparado |
| EntityDamageByBlockEvent | ✅ | — (el bloque no es damager todavía) | stub |
| EntityCombust/ByBlock/ByEntityEvent | ✅ | `world_state.rs` `TODO: world_state.rs:EntityCombustEvent` | stub |
| EntityEffect/Add/RemoveEvent | ✅ | `world_state.rs` `TODO: world_state.rs:EntityEffectEvent` | stub |
| EntitySpawnEvent | — | `WorldState::spawn_entity()` | disparado |
| EntityDeathEvent | — | `Living::onDeath()` del tick | disparado |
| EntityDespawnEvent | — | `WorldState::remove_entity()` vivo y barrido de `closed` | disparado |
| EntityExplodeEvent | ✅ | TNT (`explode_into`) y `Explosion` de PHP (`world.explode` → `queue_explosion`) encolan; `SessionManager::apply_explosions()` dispara antes de destruir bloques | disparado |
| EntityPreExplode/FrostWalker/ItemPickup/Motion/RegainHealth/ShootBow/Teleport/Trample/BlockChange/AreaEffectCloudApply/Event | ✅ | `world_state.rs` `TODO: world_state.rs:EntityMotionEvent` | stub |
| EntityExtinguishEvent (—), ItemSpawnEvent (—), ProjectileHit/HitBlock/HitEntityEvent (—), ItemDespawn/Merge/LaunchEvent (✅) | —/✅ | `world_state.rs` `TODO: world_state.rs:ProjectileLaunchEvent` | stub |

## Inventory (session.rs / world_state.rs)

| Evento | Cancellable | Trigger sugerido | Estado |
| --- | --- | --- | --- |
| InventoryTransactionEvent | ✅ | `session.rs` inventario `TODO: session.rs:InventoryTransactionEvent` | stub |
| InventoryOpenEvent | ✅ | `session.rs` `TODO: session.rs:InventoryOpenEvent` | stub |
| InventoryCloseEvent | — | `session.rs` `TODO: session.rs:InventoryCloseEvent` | stub |
| CraftItemEvent | ✅ | `session.rs` `TODO: session.rs:CraftItemEvent` | stub |
| FurnaceBurn/SmeltEvent | ✅ | `world_state.rs` `TODO: world_state.rs:FurnaceBurnEvent` | stub |

## World / Chunk (world_state.rs)

El mundo no conoce el gestor de plugins, así que las mutaciones encolan
(`WorldState::queue_world_event()`) y `SessionManager::dispatch_world_events()`
entrega en el tick siguiente (`tick.world_events`).

| Evento | Cancellable | Trigger | Estado |
| --- | --- | --- | --- |
| WorldSaveEvent | — | `WorldState::save()` (manual, `/save-all`, autoguardado) | disparado (aplazado al tick) |
| SpawnChangeEvent | — | `WorldState::set_spawn()` (`/setworldspawn`, `world.set_spawn` de `World::setSpawnLocation()`) | disparado (aplazado al tick) |
| WorldDifficultyChangeEvent | — | `WorldState::set_difficulty()` (`/difficulty`, `world.set_difficulty`) | disparado (aplazado al tick) |
| WorldLoad/Init/UnloadEvent | Unload ✅ | `WorldManager::loadWorld`/`unloadWorld` (mundos espejo, sin tick) | stub |
| ChunkLoadEvent | — | primera vez que la columna se envía a un jugador (`SessionManager::loaded_chunks`, una vez por chunk) | disparado |
| ChunkPopulate/UnloadEvent | Unload ✅ | no hay cola de pobladores ni descarga de columnas | stub |
| WorldDisplayNameChangeEvent | — | `World::setDisplayName()` es local (no hay nombre de mundo nativo) | stub |
| WorldSoundEvent / WorldParticleEvent | ✅ | `World::addSound/addParticle` encolan `PendingWorldEffect`; `SessionManager::dispatch_world_effects()` dispara el evento y emite lo que devuelva (un tick después) | disparado |

`TODO: world_state.rs:ChunkLoadEvent`, etc.

### Bloque

| Evento | Cancellable | Trigger | Estado |
| --- | --- | --- | --- |
| BlockBreakEvent / BlockPlaceEvent | ✅ | hilo de sesión (`PREDICT_DESTROY_BLOCK`, `handle_use_item_place`) | disparado |
| BlockSpreadEvent | ✅ | `fluid::flow_into()` vía `BlockEventHook`, antes de escribir | disparado |
| BlockFormEvent | ✅ | `fluid::liquid_collide()` (lava + agua → piedra/cobblestone/obsidiana) | disparado |
| BlockGrowEvent | ✅ | random tick (cultivos, saplings, sugar cane, cactus, vines) | stub: `TickScene::set_block` crece sin evento previo |
| LeavesDecayEvent | ✅ | `random_tick::leaves_tick()` | stub: mismo motivo |
| BlockUpdateEvent | ✅ | cola de actualizaciones por vecino (`World::updateBlockAt`) | stub |
| BlockExplode/PreExplodeEvent | ✅ | en PM 5 los dispara `RespawnAnchor`, no la TNT | stub |
| BlockBurn/Melt/DeathEvent | ✅ | fuego/nieve (mecánica ausente) | stub |
| BlockItemPickupEvent | ✅ | hoppers (mecánica ausente) | stub |
| SignChange/PressurePlateUpdate/ChestPair/FarmlandHydrationChange/GeyserEruptionPulse/StructureGrow/BlockTeleport | ✅ | mecánica ausente | stub |

La TNT dispara `EntityExplodeEvent` (no `BlockExplodeEvent`): `explode_into()`
encola los bloques afectados y `SessionManager::apply_explosions()` dispara el
evento antes de destruirlos, así que cancelar evita destrucción y daño.

## Server / Plugin

- `DataPacketDecode/Receive/SendEvent` (✅) → `session.rs` `TODO: session.rs:DataPacketReceiveEvent`
- `LowMemoryEvent`, `QueryRegenerateEvent`, `UpdateNotifyEvent` (—) → `server-core` `TODO: server-core:LowMemoryEvent`
- `NetworkInterface/Register/UnregisterEvent` → `session.rs` `TODO: session.rs:NetworkInterfaceRegisterEvent`
- `PluginEnable/DisableEvent` (—) → `plugin_host.rs` lifecycle (no tocar por ownership) `TODO: plugin_host.rs:PluginEnableEvent`
- `CommandEvent`/`ServerCommandEvent` ya cableados en `command.rs`.

## Divergencias documentadas (bridge vs PM upstream)

- `BaseBlockChangeEvent` y los `Block*` nuevos extienden `Event` con coords en vez
  de `BlockEvent(Player|Block)`, para no requerir `Player` ficticio en eventos de
  mundo. Getters `getWorld()/getX()/getY()/getZ()/getBlockId()` compatibles.
  `BlockBreak/Place` originales sí extienden `BlockEvent` (sin cambios).
- `PlayerDataSave/Creation/DuplicateLogin/IdentityVerify/ResourcePackOffer/PreLogin`
  extienden `Event` (no `PlayerEvent`) igual que PM; el resto de player extiende `PlayerEvent`.
- `PlayerExperienceChange/Exhaust` extienden `PlayerEvent` en el bridge (PM usa
  `EntityEvent<Human>`); se expone `getPlayer()` + payload genérico.
- `FurnaceBurn/SmeltEvent` viven en `inventory` namespace (PM) pero con coords de bloque.
- `EntityEvent` guarda `entity_id/type/name + world/x/y/z` como arrays (no hay clase
  `Entity` en el bridge). Getters genéricos vía `__call` + `extra`.

## Qué queda

1. Cablear triggers en `session.rs` / `command.rs` / `world_state.rs` (equipo visibilidad).
2. Enriquecer payloads Rust con tipos reales (`Entity`, `Block`, `Position`, `Item`)
   cuando el core los tenga; hoy son `String`/`Value` genéricos + `extra`.
3. Tests de plugins reales contra cada evento (roundtrip PHP ya verificado: 141/141).
4. `scheduler.php` / `plugin.php`: sin cambios (ya completos; no se tocó `manager.rs`/logs).

## Anexo integración (2026-11-06)

Triggers cableados por el equipo de integración (gated: solo si `has_listeners`):

- `PlayerDeathEvent` en `PlayerSession::kill_player` (`session.rs`): payload
  simplificado `player + death_message` ("{name} died"). Broadcast del mensaje
  solo cuando hay listeners (preserva el comportamiento sin plugins; `/kill`
  sigue usando `broadcastCommandMessage`).
- `PlayerRespawnEvent` en `PlayerSession::handle_respawn` (`session.rs`):
  posición de respawn mutable; cancelar deja al jugador muerto. Divergencia:
  el bridge lo marca `Cancellable` (PM upstream no lo es).
- `PlayerJoin/Quit`, `PlayerChatEvent`, `PlayerCommandPreprocessEvent`,
  `PlayerMoveEvent`, `PlayerInteractEvent`, `BlockBreakEvent` y
  `ServerCommandEvent` ya estaban cableados antes de la integración.

Pendientes (no triviales / sin struct Rust): `PlayerKickEvent`,
`PlayerGameModeChangeEvent`, `PlayerToggle*Event`, `PlayerDataSaveEvent`,
`PlayerLoginEvent`, `BlockPlaceEvent` (decodificar `InventoryTransactionPacket`),
entidades, inventario, mundo/chunk y eventos de red. Ver la tabla para el
`TODO` exacto por evento.
