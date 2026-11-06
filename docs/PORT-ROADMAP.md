# PocketMine-Rust — Estado del port de axolotl-pm y hoja de ruta

Última actualización: 2026-11-05
Referencia de comportamiento: `target/axolotl-pm` (PocketMine-MP 5 / axolotl) y
`target/axolotl-bedrock-protocol` (protocolo 2225). Ambos clones viven en
`target/` y **no se versionan**: se recrean cuando hacen falta.

Regla del port: cada comando y cada paquete se traduce **uno a uno** desde el
PHP. Mismos nombres, mismos argumentos, mismos límites, mismos mensajes
(claves de traducción de PocketMine, locale español) y mismos efectos. Si algo
no se puede replicar todavía, se documenta en la fase correspondiente en vez de
inventar un comportamiento distinto.

---

## 1. Árbol actual del repositorio

```
pocketmine-rust/
├── Cargo.toml                     workspace: 6 crates, resolver 2
├── HANDOFF.md                     estado previo del puente PHP
├── server.properties              config del servidor (allow-cheats, view-distance, ...)
├── pocketmine.yml                 config estilo PM (generadores, etc.)
├── allowlist.json / white-list.txt / ops.txt (se crea al usar /op)
├── docs/
│   ├── plugin-api-contract.md     contrato del puente Rust ↔ PHP
│   └── PORT-ROADMAP.md            ESTE documento
├── data/                          datos extraídos de BDS / BedrockData
│   ├── item_ids.txt               [nuevo] 834 items clásicos: identifier<TAB>id
│   ├── bds-capture/               38 capturas reales de paquetes (login, chunks,
│   │                              inventario, respawn, comandos, item registry...)
│   ├── canonical_block_states.nbt, block_properties_table.json, item_tags.json,
│   │   biome_definitions.json, biome_id_map.json, entity_id_map.json,
│   │   required_item_list.json, creative/, recipes/, enums/
│   └── README.md                  procedencia de cada blob
├── plugins/                       plugins PHP de prueba
│   ├── ExamplePlugin/  ChatGuard/  TestKit/  EconomyLite/
├── plugin_data/                   datos generados por los plugins (no versionar)
└── crates/
    ├── bedrock-protocol/          (≈3.700 líneas) protocolo 2225
    │   ├── src/lib.rs
    │   ├── src/binary.rs          Reader/Writer (varint, zigzag, LE, strings)
    │   ├── src/batch.rs           batch/zlib por frame
    │   ├── src/types.rs           BlockPos, Vector3f, ...
    │   ├── src/packets/           [25 módulos]
    │   │   ├── mod.rs
    │   │   ├── login.rs                    0x01 Login
    │   │   ├── play_status.rs              0x02
    │   │   ├── resource_packs.rs           0x06 / 0x07 / 0x08
    │   │   ├── start_game.rs               0x0B
    │   │   ├── set_spawn_position.rs       0x43
    │   │   ├── level_chunk.rs              0x3A
    │   │   ├── network_chunk_publisher_update.rs  0x79
    │   │   ├── chunk_radius.rs             0x5A
    │   │   ├── network_settings.rs         0x8F
    │   │   ├── request_ability.rs          0xBA
    │   │   ├── player_auth_input.rs        0x90
    │   │   ├── set_player_game_type.rs     0x3E
    │   │   ├── update_player_game_type.rs  0x97
    │   │   ├── update_abilities.rs         0xBC
    │   │   ├── text.rs                     0x09
    │   │   ├── command_request.rs          0x4D
    │   │   ├── available_commands.rs       0x4C
    │   │   ├── disconnect.rs               0x05
    │   │   ├── packet_violation_warning.rs 0x9C
    │   │   ├── item_stack.rs               ItemStack de red (2225)
    │   │   ├── inventory_content.rs        0x31
    │   │   ├── inventory_slot.rs           0x32
    │   │   ├── inventory_transaction.rs    0x1E
    │   │   ├── set_health.rs               0x2A
    │   │   ├── set_title.rs                0x58
    │   │   ├── remove_actor.rs             0x0E
    │   │   ├── move_actor_absolute.rs      0x12
    │   │   ├── transfer.rs                0x55
    │   │   ├── actor_event.rs              0x1B (DEATH_ANIMATION)
    │   │   └── respawn.rs                  0x2D
    │   └── tests/session_packets_test.rs   (11 tests de roundtrip)
    ├── bedrock-world/             (≈3.400 líneas)
    │   ├── src/chunk.rs, subchunk.rs, paletted_block_array.rs
    │   ├── src/block_id.rs        registro de bloques
    │   ├── src/network.rs         serialización de subchunks
    │   ├── src/generator/         flat, normal, biome, noise/simplex,
    │   │                          populators/{tree,ore,ground_cover,tall_grass}
    │   └── tests/{block_access,generator,subchunk_network}_test.rs
    ├── server-core/               (≈6.500 líneas) servidor
    │   ├── src/main.rs            arranque, config, consola, tick loop 20 TPS
    │   ├── src/session.rs         PlayerSession + SessionManager (login → juego)
    │   ├── src/command.rs         framework + 22 comandos nativos
    │   ├── src/inventory.rs       PlayerInventory (36 slots) + registro de items
    │   ├── src/world_state.rs     mundo mutable, spawn, hora, clima, ediciones
    │   ├── src/config.rs          server.properties / pocketmine.yml
    │   ├── src/plugin_host.rs     PlayerRegistry + ServerPluginHost
    │   ├── src/identity.rs        JWT de login → nombre/XUID
    │   ├── src/static_packets.rs  StartGame y compañía
    │   ├── src/logging.rs
    │   └── tests/{session_lifecycle,item_health_commands}_test.rs
    ├── nethernet-rs/              (≈1.500 líneas) transporte NetherNet
    │   ├── src/signaling.rs, negotiation.rs, identity.rs, info.rs
    │   └── tests/{signaling,nat,reconnect}_test.rs
    ├── raklib-rs/                 (≈890 líneas) RakNet (unconnected ping/pong)
    │   └── src/{lib,offline}.rs + tests/offline_test.rs
    └── php-plugin-bridge/         (≈3.700 líneas Rust + PHP embebido)
        ├── php/*.php              API PM5 en PHP (math, text, block, world,
        │                          player, item, command, scheduler, event,
        │                          plugin, ipc, bootstrap)
        ├── src/runtime/{php,bootstrap,native}.rs
        ├── src/{manager,plugin,event,context,host,command,error}.rs
        └── src/events/{player,block,server,mod}.rs
```

Comprobaciones actuales (2026-11-05):

```
cargo check --workspace                      → OK
cargo test -p bedrock-protocol -p server-core → 85 tests OK
php -l sobre crates/php-plugin-bridge/php/*.php → OK (ver HANDOFF.md)
```

---

## 2. Cobertura por subsistema

| Subsistema de axolotl-pm | Archivos PHP | Dónde vive en Rust | Estado |
|---|---:|---|---|
| `network/` (sesiones, sesión de jugador, raknet, compresión) | 109 | `server-core/session.rs`, `nethernet-rs`, `raklib-rs` | 🟡 login, chunks y chat funcionan; falta el resto |
| `protocol/` (248 paquetes) | 248 | `bedrock-protocol/packets/` | 🟡 29 de 248 |
| `world/` (chunks, generadores, luz, formatos) | 278 | `bedrock-world` | 🟡 generación sin luz ni persistencia |
| `block/` (402 bloques) | 402 | `bedrock-world/block_id.rs` | 🔴 ids planos, sin estados ni comportamientos |
| `entity/` + `player/` | 93 | parcial en `session.rs` | 🔴 un solo jugador visible para sí mismo |
| `inventory/` | 35 | `server-core/inventory.rs` | 🟡 cuatro ventanas + `InventoryTransaction` |
| `item/` | 154 | `inventory.rs` (registro) | 🟡 834 items por id, sin NBT/data-driven |
| `command/` (41 comandos + framework) | 53 | `server-core/command.rs` | 🟡 22 comandos, sin nodos de permiso |
| `permission/` | 14 | `CommandSource::can_use_privileged` | 🔴 op/allow-cheats, sin `PermissionManager` real |
| `event/` | 152 | `php-plugin-bridge/events/` | 🟡 subconjunto (join, quit, move, chat, command) |
| `plugin/`, `scheduler/` | 37 | `php-plugin-bridge` | 🟢 funcional para plugins PHP simples |
| `crafting/` | 25 | — | 🔴 sin empezar |
| `form/` | 2 | — | 🔴 sin empezar |
| `resourcepacks/` | 9 | `resource_packs.rs` | 🟡 handshake básico |
| `lang/` (traducciones) | 3 + .ini | `translations.rs` + `resources/spa.ini` | 🟡 locale embebido y 25 constantes verificadas; migración completa pendiente |
| `utils/`, `console/`, `timings/`, `crash/`, `wizard/`, `data/` | 240 | disperso | 🔴 mínimo |

---

## 3. Estado de los comandos nativos

### Portados (40 registrados, con alias)

| Comando | Alias | Notas |
|---|---|---|
| `/help` | `?` | tabla local |
| `/tp` | `teleport` | coordenadas absolutas y relativas |
| `/gamemode` | `gm` | survival/creative/adventure/spectator; `gameMode.changed` al objetivo y `chat.type.admin` a ops, como `GamemodeCommand` |
| `/gmc` `/gms` `/gma` `/gmsp` | — | atajos |
| `/spawn` | — | propios de Bedrock, no de PM |
| `/setworldspawn` | — | persiste en `WorldState` |
| `/say` `/me` | — | chat |
| `/msg` | `tell`, `w`, `whisper` | privado |
| `/list` | `players` | |
| `/kick` | — | |
| `/give` | — | claves vanilla, `item[:meta]`, 1..32767 |
| `/clear` | — | `maxCount -1/0/n`, cuatro ventanas; `clear.self`/`clear.other` |
| `/kill` | `suicide` | mensaje `commands.kill.successful` |
| `/time` | — | set/add/query |
| `/weather` | — | propio de Bedrock |
| `/op` `/deop` | — | escribe `ops.txt` |
| `/version` | `ver` | |
| `/spawnpoint` | — | `getRelativeDouble`, reaparición por sesión |
| `/defaultgamemode` | — | gamemode por defecto de sesiones nuevas |
| `/difficulty` | — | `SetDifficultyPacket` (0x3C) a todos |
| `/seed` | — | semilla del `WorldGenerator` |
| `/ban` | — | `banned-players.txt`, expulsa si está en línea |
| `/ban-ip` | — | `banned-ips.txt`, expulsa la IP |
| `/banlist` | — | `ips` / `players` |
| `/pardon` | `unban` | |
| `/pardon-ip` | `unban-ip` | |
| `/whitelist` | — | `on/off/list/add/remove/reload` + expulsión, un nodo por subcomando |
| `/stop` | — | activa el apagado ordenado |
| `/plugins` | `pl` | `load_order()` del `PluginManager` |
| `/save-all` | — | `world.json` + `players/<nombre>.json` |
| `/save-on` `/save-off` | — | bandera de autoguardado (6000 ticks) |
| `/tps` | — | extensión nativa estilo PaperMC (1m/5m/15m) |
| `/title` | — | `SetTitlePacket` (0x58): `clear`, `reset`, `title`, `subtitle`, `actionbar`, `times` |
| `/transferserver` | — | `TransferPacket` (0x55); sólo jugadores y sin mensaje de éxito, como `TransferServerCommand` |

### Pendientes de axolotl-pm (8)

`dumpmemory`, `effect`, `enchant`, `gc`, `particle`, `status`, `timings`, `xp`.
Estos necesitan sistemas que aún no existen: métricas de tick/red,
`TransferPacket`, efectos de entidad, experiencia, encantamientos (NBT),
partículas y volcados de memoria.

---

## 4. Fases

Cada fase termina con código compilando, tests en verde y una nota de paridad
(qué se puede y qué no). El orden respeta dependencias: no se puede portar
`/effect` sin entidades, ni `/xp` sin salud/experiencia, ni `ItemStackRequest`
sin inventario completo.

### Fase 0 — Cimientos (✅ completada)

Transporte NetherNet y RakNet, login con identidad real, chunks y generación,
sesión de jugador, chat, comandos base, salud, inventario de 36 slots,
`/give`, `/clear`, `/kill`.

**Criterio de salida:** un cliente Bedrock 1.26.60 entra, ve terreno, recibe
mensajes del chat, ejecuta comandos con `CommandRequestPacket` y muere/resucita
con paquetes reales. Cumplido y cubierto por 74 tests.

### Fase 1 — Inventario y objetos completos (🟡 en curso)

Objetivo: que mover cosas en el cliente cambie el estado del servidor.

- [x] `InventoryTransactionPacket` (0x1E) con los cinco tipos de transacción y
  `NetworkInventoryAction`; `request_id`, `requestChangedSlots` y límite de 50
  acciones como PM.
- [x] `InventorySlotPacket` (0x32) para corregir ranuras sueltas.
- [x] `PlayerInventory` con las cuatro ventanas: inventario (0, 36), armadura
  (120, 4), offhand (119, 1) y cursor (124, 1).
- [x] Aplicación atómica de transacciones: si una acción no coincide con el
  estado del servidor se rechaza la transacción entera y se reenvían las cuatro
  ventanas; las acciones creativas también fuerzan resync.
- [x] `/clear` recorre las cuatro ventanas con el límite compartido, como el
  `ClearCommand` de PM.
- [ ] `ItemStackRequestPacket` (0x93) y respuestas, si el cliente lo usa para
  contenedores.
- [ ] NBT mínimo (TAG_Compound/String/Int/List) para `/give [... tags]` y para
  conservar `nbt` al mover objetos.
- [ ] Items data-driven: mapear identificadores string del registro de BDS a
  `network_id` + NBT, en vez de rechazarlos.
- [ ] Entidades de objeto para que los drops no desaparezcan (depende de fase 4).

**Archivos nuevos:** `packets/inventory_transaction.rs`,
`packets/item_stack_request.rs`, `packets/inventory_slot.rs`,
`crates/bedrock-protocol/src/nbt.rs`, ampliaciones de `inventory.rs`.
**Criterio de salida:** mover un stack en el cliente y leerlo igual desde el
servidor; `/give Steve diamond_sword 1` y `/clear` cubren armadura y offhand;
tests de roundtrip con capturas reales de `data/bds-capture/`.

### Fase 2 — Los 25 comandos restantes (🟡 17/25)

Traducir uno a uno `command/defaults/*.php`:

- **Jugador:** `spawnpoint` ✅, `defaultgamemode` ✅, `difficulty` ✅,
  `seed` ✅, `title` ✅, `effect`, `enchant`, `xp`, `particle`.
- **Servidor:** `stop` ✅, `plugins` ✅, `save-all` ✅, `save-on` ✅,
  `save-off` ✅, `status`, `gc`, `dumpmemory`, `timings`, `transfer-server` ✅.
- **Administración:** `ban` ✅, `ban-ip` ✅, `banlist` ✅, `pardon` ✅,
  `pardon-ip` ✅, `whitelist` ✅.

Notas de paridad de lo portado: `/difficulty` guarda el valor en `WorldState`
y envía `SetDifficultyPacket` (0x3C) a todos; `/spawnpoint` guarda un punto de
reaparición por sesión que `handle_respawn` respeta; `/seed` lee la semilla del
`WorldGenerator` (`NormalGenerator` la expone, `FlatGenerator` devuelve 0);
las listas de acceso viven en `server-core/src/access.rs`, se persisten en
`banned-players.txt`, `banned-ips.txt` y `white-list.txt` (formato
`entrada|razón`, desviación del YAML de PM), se comprueban en el login y
expulsan a los jugadores afectados. Los operadores siguen saltándose la
whitelist, como `Server::isWhitelisted()`.

Persistencia: `WorldState::save_to` escribe `world.json` (hora, clima,
dificultad, spawn, semilla y ediciones), `PlayerSession::save_player_data`
escribe `players/<nombre>.json` (posición, rotación, gamemode, salud y punto de
spawn), se cargan al hacer login y al arrancar, y `/save-all` más el
autoguardado de 6000 ticks las escriben. PM usa NBT (`level.dat`, `player.dat`);
aquí es JSON a propósito, para no necesitar serializador NBT todavía. El
inventario aún no se persiste.

Cada uno necesita su acción en `CommandAction` y su rama en
`SessionManager::apply_command_actions`, más los mensajes de `spa.ini` que use.
Antes de empezar la fase, portar `PermissionManager` (fase 3.1) para que los
nodos `pocketmine.command.*.self/.other` dejen de ser un booleano.

**Criterio de salida:** `/help` lista los 41 comandos de PM + los propios de
Bedrock, y cada uno tiene test de parseo con sus errores exactos.

### Fase 3 — Permisos, traducciones y consola (🟡 2/4)

1. [x] `PermissionManager` real: registro, herencia, `default: true|false|op|notop`,
   `PermissionAttachment` para plugins, `DefaultPermissionNames`.
   (`crates/server-core/src/permission.rs`: nodos, hijos, defaults de PM y
   adjuntos por jugador; el despacho comprueba el nodo antes de ejecutar y los
   comandos de objetivo usan `.self`/`.other`. `/clear`, `/gamemode` y `/tp`
   comprueban el nodo `other` sólo cuando el objetivo no es quien ejecuta, y
   `/whitelist` elige el nodo por subcomando en vez de un guard global, como
   `WhitelistCommand::onRun`.)
2. [ ] Motor de traducciones: leer un subconjunto de `spa.ini` (o tabla embebida)
   en vez de constantes sueltas; `Translatable` con parámetros.
3. [ ] Consola completa: `ServerCommandEvent`, `BroadcastLoggerForwarder`,
   colores por canal (`§7§o[fuente: mensaje]`, `chat.type.admin`). Ya existe
   el reenvío a operadores (`SessionManager::broadcast_command_message`) usado
   por `/say`, `/me`, bans y cambios de modo de juego.
4. [x] `AvailableCommandsPacket` generado desde `COMMANDS` con alias por
   comando (ya se construye desde la tabla; faltan los flags de permiso que
   PocketMine envía al cliente, que hoy no son necesarios).

**Criterio de salida:** un jugador sin permisos recibe exactamente el mensaje
de PM; los plugins pueden conceder permisos y eso cambia lo que ve el cliente.

### Fase 4 — Entidades, combate y experiencia (🟡 en curso)

Ya modelados: `RemoveActorPacket` (0x0E) y `MoveActorAbsolutePacket` (0x12),
con tests de ida y vuelta. Siguen faltando `AddPlayerPacket` (0x0C),
`PlayerListPacket` (0x3F) —que necesita el serializador de skins— y
`AddActorPacket`. Sin ellos no hay visibilidad entre jugadores ni entidades de
objeto para drops.

1. Entidades en el mundo: `AddPlayerPacket`, `AddActorPacket`,
   `RemoveActorPacket`, `MoveActorAbsolutePacket`, `PlayerListPacket`.
2. Visibilidad entre jugadores (skin, nametag, lista de jugadores).
3. Daño real: `EntityDamageEvent` con causas (caída, ahogo, fuego, vacío,
   ataque), armadura, invulnerabilidad tras golpe, regeneración de salud.
4. Muerte completa: drops del inventario, `ActorEvent`, pantalla de muerte,
   respawn en el spawn real, `SetSpawnPosition`.
5. Experiencia: `SetPlayerGameType` no basta; `UpdateAbilities`, niveles y
   `AddPlayerPacket` con `xp` real.
6. Eventos PHP correspondientes (`EntityDamageEvent`, `EntityDeathEvent`).

**Criterio de salida:** dos clientes se ven, se hacen daño y mueren con drops
y respawn; `/kill` y `/xp` coherentes con el estado.

### Fase 5 — Crafteo, contenedores y uso de objetos

1. `CraftingDataPacket` desde `data/recipes/` y `data/creative/`.
2. Contenedores: cofres, hornos, mesas de crafteo (`ContainerOpen/Close`,
   `ContainerSetData`, `InventoryContent` por ventana).
3. `ItemStackRequest` completo (si 2225 lo requiere) y validación servidor.
4. Durabilidad, comida, pociones base, colocar bloques desde el inventario con
   el `block_runtime_id` correcto.

**Criterio de salida:** craftear en una mesa, abrir un cofre y comer funcionan
sin desincronizar el inventario.

### Fase 6 — Mundo completo

1. Luz de cielo y de bloque (`data/bds-capture/level_chunk` ya trae la
   estructura); `UpdateBlockPacket`, `BlockEventPacket`.
2. Estados de bloque reales: `canonical_block_states.nbt`,
   `block_properties_table.json`, `block_state_meta_map.json`.
3. Física barata: arena/grava que caen, agua y lava básicas, crecimiento.
4. Persistencia: guardar/cargar chunks y entidades (`world/format`), `level.dat`.
5. Biomas reales (`biome_definitions.json`) y populators que los respeten.

**Criterio de salida:** romper y colocar bloques se guarda al reiniciar, con
luz y bioma correctos.

### Fase 7 — API de plugins a la par de PM

Ampliar el puente ya existente (hoy cubre join/quit/move/chat/comando):

1. Inventario y mundo reales desde PHP (`PlayerInventory`, `Block`, `World`).
2. Entidades y eventos de combate/muerte.
3. Formularios: `ModalForm`, `SimpleForm`, `CustomForm` + paquete de respuesta.
4. Scoreboard, boss bar, `Title`, `Particle`.
5. Tareas asíncronas reales y `AsyncPool`.

**Criterio de salida:** los 4 plugins de prueba usan inventario y formularios
sin cambios en su código, y un plugin nuevo de PM 5 arranca sin adaptaciones.

### Fase 8 — Red, escalado y compatibilidad

1. `InventoryTransaction` bajo carga, límites de batch, compresión configurable.
2. Rate limiting de paquetes y `PacketViolationWarning` (ya existe el paquete).
3. Multiworld y `transfer-server` reales.
4. Negociación de versión y mensajes de rechazo limpios.
5. Métricas (`timings`) y benchmark de generación.

**Criterio de salida:** 20 bots conectados sin fugas ni desincronización,
`timings` con datos reales.

### Fase 9 — Operación y cierre

1. Comandos de administración persistentes: `ban`, `whitelist`, `ops.txt`,
   `allowlist.json` con recarga en caliente.
2. Crash dumps legibles y logs rotativos.
3. CI: `cargo fmt --check`, `clippy`, `cargo test --workspace`, `php -l`.
4. Documentación final de paridad por comando y por paquete.

---

## 5. Convenciones de trabajo

- Sin dependencias nuevas salvo que una fase las justifique explícitamente
  (hoy: tokio, bytes, thiserror, serde, serde_json, flate2, tracing,
  parking_lot).
- Todo paquete nuevo se valida contra una captura real de `data/bds-capture/`
  antes de escribir el codificador.
- Los mensajes al jugador salen de las claves de PocketMine; nada de textos
  inventados.
- Los comandos devuelven `CommandOutput`/`CommandAction`; sólo
  `SessionManager` toca sesiones, mundo y registro.
- Antes de dar una fase por cerrada: `cargo check --workspace` y los tests de
  los crates tocados en verde.
- `target/axolotl-pm` y `target/axolotl-bedrock-protocol` son la fuente de
  verdad; si no están, clonarlos de nuevo.

---

## 6. Riesgos y decisiones abiertas

| Tema | Riesgo | Decisión actual |
|---|---|---|
| Items data-driven | No tienen id numérico en 2225 | Rechazados por `/give`; se resuelven en fase 1 con id + NBT |
| `InventoryTransaction` | Sin él, el cliente y el servidor divergen | Fase 1, prioridad máxima |
| Luz | Chunks oscuros o con luz incorrecta | Fase 6 |
| Plugins PHP | Un plugin que use inventario/entidades falla | Fase 7; hoy el contrato lo excluye |
| Persistencia | El mundo se regenera al reiniciar | Fase 6 |
| Permisos | `allow-cheats` simula op para todo | Fase 3 con `PermissionManager` real |
