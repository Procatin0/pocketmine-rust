# Plan de compatibilidad con axolotl-pm

**Proyecto:** pocketmine-rust
**Referencia de comportamiento:** `target/axolotl-pm` (PocketMine-MP 5 / Axolotl, PHP) y
`target/axolotl-bedrock-protocol` (protocolo 2225)
**Protocolo objetivo:** Bedrock 1.26.60 (protocolo 2225)
**Estado medido:** 33 de los 41 comandos de PM portados, 29 de 248 paquetes, permisos y
`/gamemode` paritarios; entidades, crafteo y persistencia de chunks pendientes
**Fecha:** 2026-11-06
**Verificación base:** `cargo check --workspace --all-targets` OK (0 warnings); 59 tests de
`server-core` (lib) + 12 (`item_health_commands_test`) + 10 (`session_lifecycle_test`) +
38 y 11 (`bedrock-protocol`) en verde, ejecutados con `CARGO_TARGET_DIR=target/test-parity`.

Regla del port: cada comando y cada paquete se traduce **uno a uno** desde el PHP.
Mismos nombres, alias, argumentos, límites, nodos de permiso, mensajes (claves de
`resources/translations/spa.ini`) y efectos. Cuando algo no se puede replicar todavía se
documenta como deuda explícita; no se inventa comportamiento.

Convención de estados usada en todo el documento:

- 🟢 implementado con textos y efectos de PM, con test que fija el comportamiento.
- 🟡 implementado pero con deuda de paridad concreta (se lista).
- 🔴 no implementado (bloqueo indicado).

> 🟢 no significa "comparado byte a byte contra un servidor PM en vivo": todavía no
> existe un harness de comparación automática (§12). Significa que la clave de
> traducción, los argumentos y los efectos salen del PHP y están cubiertos por tests.

---

## 1. Cómo usar este documento

- Cada sección corresponde a una fase de `docs/PORT-ROADMAP.md`.
- Antes de marcar algo como cerrado: `cargo check --workspace --all-targets`, los tests
  de los crates tocados y, si aplica, `php -l` sobre los PHP nuevos.
- `target/axolotl-pm` es la fuente de verdad; `target/` está fuera de git y se recrea
  clonando el fork.
- El estado de §2 y §3 se recalcula con:

```bash
ls target/axolotl-pm/src/command/defaults/*.php    # comandos PM de referencia
grep -c "CommandInfo {" crates/server-core/src/command.rs   # comandos registrados
grep -n "pub const COMMAND_" crates/server-core/src/permission.rs   # nodos
ls crates/bedrock-protocol/src/packets/*.rs        # paquetes
```

- Ninguna fila de §3 se pone 🟢 sin un test que fije el texto exacto de `spa.ini`.

---

## 2. Estado medido

| Fase | Nombre | Estado | Causa del bloqueo |
|---|---|---|---|
| 0 | Cimientos | ✅ | — |
| 1 | Inventario y objetos | 🟡 | falta `ItemStackRequest`, NBT y items data-driven |
| 2 | Comandos restantes | 🟡 17/25 | 8 comandos dependen de sistemas inexistentes |
| 3 | Permisos, traducciones y consola | 🟡 3/4 | permisos, `AvailableCommands`, `ServerCommandEvent` y difusión administrativa a la consola listos; falta el motor de traducciones |
| 4 | Entidades, combate y XP | 🟡 | `RemoveActorPacket` y `MoveActorAbsolutePacket` modelados; faltan `AddPlayerPacket`, `PlayerListPacket`, `AddActorPacket` y `SetEntityData` |
| 5 | Crafteo y contenedores | 🔴 | sin `CraftingDataPacket` ni ventanas múltiples |
| 6 | Mundo completo | 🔴 | sin luz, estados de bloque ni persistencia de chunks |
| 7 | API de plugins | 🔴 | depende de 4, 5 y 6 |
| 8 | Red y escalado | 🔴 | sin multiworld, rate limiting ni `TransferPacket` |
| 9 | Operación y cierre | 🔴 | sin CI ni crash dumps |

Cifras concretas:

| Métrica | Valor |
|---|---|
| Comandos de PM existentes (`command/defaults/*.php`, sin `VanillaCommand`) | 41 |
| Comandos de PM portados | 31 |
| Comandos propios (superconjunto Bedrock/Paper) | 7 |
| Comandos registrados en `COMMANDS` | 38 |
| Comandos de PM pendientes | 10 |
| Paquetes implementados | 27 de 248 |
| Nodos de permiso registrados | 46 (+16 reservados de comandos no portados) |
| Tests en verde (workspace `server-core` + `bedrock-protocol`) | 130 |

El servidor es un **superconjunto**: lo que existe en PM existe aquí con el mismo
comportamiento, y además hay comandos propios de Bedrock y una extensión de TPS (§4).

---

## 3. Matriz de paridad de los 41 comandos de PM

Referencia: `target/axolotl-pm/src/command/defaults/`. Los alias son los del
constructor PHP; los mensajes son claves de `spa.ini`.

| Comando | Args (PM) | Nodos | Mensajes / efectos | Estado |
|---|---|---|---|---|
| `ban` | `<nombre> [razón...]` | `ban.player` | `commands.ban.success` a ops; `pocketmine.disconnect.ban` o `.noReason` al expulsar | 🟡 |
| `ban-ip` | `<dirección\|nombre> [razón...]` | `ban.ip` | `commands.banip.success`, `commands.banip.invalid` | 🟡 |
| `banlist` | `[ips\|players]` | `ban.list` | `commands.banlist.*` | 🟢 |
| `clear` | `[jugador] [objeto] [maxCount]` | `clear.self`, `clear.other` | `commands.clear.success`, `.failure.no.items`, `.testing` | 🟡 |
| `defaultgamemode` | `<modo>` | `defaultgamemode` | `commands.defaultgamemode.success` | 🟢 |
| `deop` | `<jugador>` | `op.take` | `commands.deop.success`, escribe `ops.txt` | 🟢 |
| `difficulty` | `<dificultad>` | `difficulty` | `commands.difficulty.success`, `SetDifficultyPacket` (0x3C) | 🟢 |
| `dumpmemory` | — | `dumpmemory` | volcado de memoria | 🔴 |
| `effect` | `<modo> <efecto> [segundos] [amplificador] [jugador]` | `effect.self`, `effect.other` | `commands.effect.*` | 🔴 |
| `enchant` | `<jugador> <encantamiento> [nivel]` | `enchant.self`, `enchant.other` | `commands.enchant.*` | 🔴 |
| `gamemode` | `<modo> [jugador]` | `gamemode.self`, `gamemode.other` | `commands.gamemode.success.self/.other`, `gameMode.changed` al objetivo, `chat.type.admin` a ops, `pocketmine.command.gamemode.unknown/.failure` | 🟢 |
| `gc` | — | `gc` | `pocketmine.command.gc.*` | 🔴 |
| `give` | `<jugador> <objeto[:meta]> [cantidad] [tags...]` | `give.self`, `give.other` | `commands.give.success.single` | 🟡 |
| `help` | `[página\|comando]` | `help` | `commands.help.header`, `pocketmine.command.help.specificCommand.*`, `pocketmine.command.notFound`; 7 líneas por página y filtrado por `testPermissionSilent` | 🟢 |
| `kick` | `<jugador> [razón...]` | `kick` | `commands.kick.success` / `.success.reason`; sin motivo el cliente muestra su texto por defecto | 🟢 |
| `kill` | `[jugador]` | `kill.self`, `kill.other` | `commands.kill.successful` | 🟢 |
| `list` | — | `list` | `commands.players.list` con `max-players` y segunda línea con los nombres en orden `SORT_STRING` | 🟢 |
| `me` | `<acción...>` | `me` | `chat.type.emote` a todos, sin acuse al emisor | 🟢 |
| `op` | `<jugador>` | `op.give` | `commands.op.success`, escribe `ops.txt` | 🟢 |
| `pardon` | `<nombre>` | `unban.player` | `commands.unban.success` | 🟢 |
| `pardon-ip` | `<dirección>` | `unban.ip` | `commands.unbanip.success`, `.invalid` | 🟢 |
| `particle` | `<partícula> [x y z] [dx dy dz] [cantidad] [jugador]` | `particle` | `commands.particle.success`, `LevelEventPacket` | 🔴 |
| `plugins` | — (alias `pl`) | `plugins` | `pocketmine.command.plugins.success`, verde/rojo por estado, orden alfabético | 🟢 |
| `save-all` | — | `save.perform` | `commands.save.success` | 🟡 |
| `save-off` | — | `save.disable` | `commands.save.disabled` | 🟡 |
| `save-on` | — | `save.enable` | `commands.save.enabled` | 🟡 |
| `say` | `<mensaje...>` | `say` | `chat.type.announcement` a todos con el nombre real del emisor, sin acuse | 🟢 |
| `seed` | — | `seed` | `commands.seed.success` | 🟢 |
| `setworldspawn` | `[<x y z>]` | `setworldspawn` | `commands.setworldspawn.success`, `SetSpawnPosition` a todos | 🟢 |
| `spawnpoint` | `[jugador] [<x y z>]` | `spawnpoint.self`, `spawnpoint.other` | `commands.spawnpoint.success.*`, `SetSpawnPosition` al objetivo | 🟢 |
| `status` | — | `status` | métricas de red (`BandwidthStatsTracker`) | 🔴 |
| `stop` | — | `stop` | `pocketmine.command.stop` | 🟢 |
| `teleport` | `[jugador] <destino\|x y z> [y-rot x-rot]` | `teleport.self`, `teleport.other` | `commands.tp.success.*`, `commands.tp.notFound` | 🟢 |
| `tell` | `<jugador> <mensaje...>` (alias `msg`, `w`, `whisper`) | `tell` | `commands.message.display.outgoing/.incoming` y `commands.message.sameTarget` | 🟢 |
| `time` | `<set\|add\|query> [valor]` | `time.set`, `time.add`, `time.query` | `commands.time.set`, `.added`, `.query`, con un nodo por subcomando | 🟢 |
| `timings` | `[on\|off\|paste\|report]` | `timings` | `TimingsHandler` | 🔴 |
| `title` | `<jugador> <clear\|reset\|title\|subtitle\|actionbar\|times> ...` | `title.self`, `title.other` | `SetTitlePacket` (0x58) con varints con signo; `commands.title.success` y `commands.title.usage`; `times` negativos se ignoran como `setTitleDuration()` | 🟢 |
| `transferserver` | `<servidor> [puerto]` | `transferserver` | `TransferPacket` (0x55); puerto `(int)` truncado a u16, sólo jugadores (`pocketmine.command.error.playerUserOnly`) y cierre sin pantalla de desconexión | 🟢 |
| `version` | — (alias `ver`) | `version` | `pocketmine.command.version.serverSoftwareName`, `.serverSoftwareVersion`, `.minecraftVersion` | 🟢 |
| `whitelist` | `<on\|off\|list\|add\|remove\|reload>` | `whitelist.enable/.disable/.list/.add/.remove/.reload` | `commands.whitelist.*` a ops, expulsión de no incluidos, `white-list` en `server.properties` | 🟢 |
| `xp` | `<cantidad> [jugador]` | `xp.self`, `xp.other` | `commands.xp.*` | 🔴 |

### 3.1 Deuda de las filas 🟡

| Comando | Deuda concreta | Referencia |
|---|---|---|
| `ban` / `ban-ip` | Formato de almacenamiento propio (`banned-players.txt` / `banned-ips.txt` con `entrada\|razón`) en lugar de las listas YAML de PM; no hay `expires` ni `createdBy` tipados | `BanList`, `banned-players.yml` |
| `clear` | No compara NBT al filtrar por objeto (`Item` + `meta` sólo); el recuento por `maxCount` sí es paritario | `ClearCommand::execute` |
| `give` | `[... tags]` devuelve `commands.give.tagError` porque no hay NBT; el resto (clave vanilla, `item[:meta]`, 1..32767) es paritario | `GiveCommand::execute` |
| `save-all` / `save-on` / `save-off` | Guardan `world.json` y `players/<nombre>.json`, no chunks; `auto-save` ya se persiste en `server.properties` | `SaveCommand`, `ServerProperties::AUTO_SAVE` |

### 3.2 Deuda de las filas 🔴

| Comando | Bloqueo |
|---|---|
| `dumpmemory` | No hay gestión de memoria del runtime (el PHP vive en `php-plugin-bridge`); hace falta un volcado propio o exponer el del worker |
| `effect` | Requiere entidades vivas, `MobEffectPacket` y `EffectManager` (fase 4/5) |
| `enchant` | Requiere NBT de item y registro de encantamientos (fase 1) |
| `gc` | Requiere exponer el recolector del worker PHP (`GarbageCollectorManager`) |
| `particle` | Requiere `LevelEventPacket`/`SpawnParticleEffectPacket` y validación de partículas |
| `status` | Requiere `BandwidthStatsTracker` (subida/bajada por sesión) — fase 8 |
| `timings` | Requiere `TimingsHandler` con árbol de secciones; `metrics.rs` sólo mide TPS |
| `xp` | Requiere experiencia y `ExperienceManager` — fase 4 |

---

## 4. Comandos propios (no existen en PM)

Se declaran explícitamente para que la paridad sea verificable: el set es superconjunto.

| Comando | Origen | Nodos | Nota |
|---|---|---|---|
| `gmc`, `gms`, `gma`, `gmsp` | Bedrock | `gamemode.self/.other` + `can_use_privileged` | Atajos de `/gamemode <modo> [jugador]` |
| `spawn` | Bedrock | `can_use_privileged` (sin nodo de PM) | Teletransporta al spawn del mundo |
| `weather` | Bedrock | `can_use_privileged` (sin nodo de PM) | `clear/rain/thunder` + duración, persiste en `WorldState` |
| `tps` | PaperMC | `pocketmine.command.tps` (op) | Ventanas 1m/5m/15m, formato `§6TPS from last 1m, 5m, 15m: …` |

Estos comandos no deben contar como paridad de PM en ninguna métrica.

---

## 5. Permisos

`crates/server-core/src/permission.rs` replica `PermissionManager`: registro con
`default: true|false|op|notop`, hijos, `PermissionAttachment` por jugador y los nodos de
`DefaultPermissionNames` que usan los comandos portados.

| Familia de nodos | Default en PM | Uso aquí |
|---|---|---|
| `help`, `me`, `tell`, `list`, `version` | `true` | despacho |
| `clear.self`, `kill.self` | `true` | despacho |
| `time.query` | `true` | despacho |
| `clear.other`, `kill.other`, `give.*`, `gamemode.*`, `teleport.*`, `spawnpoint.*` | `op` | despacho + resolución de objetivo |
| `ban.*`, `unban.*`, `whitelist.*`, `op.*`, `stop`, `plugins`, `save.*`, `difficulty`, `seed`, `defaultgamemode`, `setworldspawn` | `op` | despacho |
| `broadcast.admin` | `op` | difusión de mensajes de comando |

Comportamiento:

- Nodo no registrado ⇒ `is_op || allow_cheats` (equivalente a `Permission::$default`
  ausente en PM).
- `allow-cheats` levanta los defaults `op` para todos, igual que el servidor de prueba de PM.
- Denegación: `pocketmine.command.error.permission` (`No tienes permiso para ejecutar el
  comando: {%commandName}`), que es la clave de `spa.ini` línea 342.

### 5.1 Estado y deuda en permisos

- **Resuelto:** el gate usa `permission_nodes()` y acepta cualquiera del conjunto, como
  `Command::testPermissionSilent()`; después el handler aplica `.self` o `.other` según
  el objetivo resuelto (`/tp`, `/gamemode`, `/clear`, `/give`, `/kill`, `/spawnpoint`).
  Cubierto por `permission_gate_accepts_either_self_or_other`.
- **Resuelto:** `/time` gatea `set`, `add` y `query` con `time.set`/`time.add`/`time.query`.
- **Pendiente:** la consola se modela con `is_op = true`, así que un attachment que
  deniegue un nodo a la consola no se respeta (PM la trata como omnisciente).
- **Pendiente:** los comandos propios (`spawn`, `weather`) siguen usando
  `can_use_privileged()` en vez de un nodo; no bloquean la paridad de PM, pero conviene
  un nodo propio (`pocketmine.command.spawn`/`weather`).
- Los comandos no portados ya tienen nodo reservado (`effect`, `enchant`, `xp`, `title`,
  `particle`, `status`, `gc`, `timings`, `dumpmemory`, `transferserver`), para que el
  registro quede completo cuando lleguen.

---

## 6. Traducciones

Estado: las claves usadas están copiadas a mano en `command::lang` (constantes `&str`) con
el texto literal de `spa.ini`, y los parámetros se sustituyen con `fill_template`
(`{%0}`, `{%1}`, `{%commandName}`, `{%playerName}`, `{%gameModeName}`).

Pendiente (fase 3.2):

1. Motor que lea `spa.ini` completo (`include_str!`) y resuelva `Translatable` con
   parámetros, en vez de constantes sueltas.
2. `KnownTranslationFactory` equivalente para que los plugins PHP reciban las mismas
   claves que en PM.
3. Fallback a `eng.ini` cuando falte una clave en el locale activo, como `Language`.
4. Test que compare cada constante de `command::lang` con la línea correspondiente de
   `spa.ini` para que no vuelva a haber textos inventados.

Claves ya usadas (muestra): `pocketmine.command.error.permission`,
`commands.generic.usage`, `commands.gamemode.success.self|other`,
`pocketmine.command.gamemode.unknown|failure`, `gameMode.changed`, `chat.type.admin`,
`commands.clear.success|failure.no.items|testing`, `commands.kill.successful`,
`commands.whitelist.*`, `commands.save.enabled|disabled`,
`pocketmine.command.plugins.success`, `commands.banlist.*`, `commands.tp.*`.

---

## 7. Consola y difusión de mensajes de comando

Implementado:

- `SessionManager::broadcast_command_message(source, message)` envía
  `§7§o[fuente: mensaje]` a los operadores, que es la forma coloreada de
  `chat.type.admin=[{%0}: {%1}]` con `TextFormat::GRAY . TextFormat::ITALIC`.
- `CommandOutput::feedback` es el mensaje directo a quien ejecuta, equivalente al
  `sendToSource` de PM.
- Lo usan `/say`, `/me`, bans, `/gamemode`, `/whitelist` y `/kick`.

Pendiente (fase 3.3):

1. Incluir la consola en la difusión (PM la añade como `BroadcastLoggerForwarder` con el
   texto sin colorear).
2. `ServerCommandEvent` con cancelación y `CommandEvent` para plugins PHP.
3. Diferenciar canal administrativo (`broadcast.admin`) del de usuarios
   (`broadcast.user`) con sus nodos.

---

## 8. Persistencia y listas de acceso

| Elemento | PM | Aquí | Impacto |
|---|---|---|---|
| Mundo | `level.dat` + regiones NBT | `world.json` (hora, clima, dificultad, spawn, semilla, ediciones) | Sin chunks ni entidades; se regenera el terreno |
| Jugador | `player.dat` (NBT) | `players/<nombre>.json` (posición, rotación, gamemode, salud, spawn) | Inventario no persistido todavía |
| Bans de nombre | `banned-players.yml` | `banned-players.txt` (`entrada\|razón`) | Sin `expires`/`createdBy`; no legible por PM |
| Bans de IP | `banned-ips.yml` | `banned-ips.txt` (`entrada\|razón`) | Ídem |
| Whitelist | `white-list.txt` (nombres) + propiedad `white-list` | `white-list.txt` + `server.properties` vía `config::set_server_property` | Paritario: `/whitelist on\|off` escribe y arranca desde la propiedad |
| Autoguardado | propiedad `auto-save` | `server.properties` + `ServerConfig::auto_save` | Paritario: `/save-on\|off` escribe y se restaura al arrancar |

Plan: introducir NBT mínimo (fase 1) y migrar `level.dat`/`player.dat` en fase 6;
mientras tanto, `save-on/off` y `whitelist on/off` deben escribir
`server.properties` con las mismas claves que `ServerProperties` (`auto-save`,
`white-list`), que es deuda de dos líneas por comando y no depende de NBT.

---

## 9. Fase 1 — Inventario y objetos completos (🟡 en curso)

Lo que ya funciona: `InventoryTransactionPacket` (0x1E) con los cinco tipos de acción y
límite de 50 acciones, `InventorySlotPacket` (0x32), las cuatro ventanas (0, 36 / 120, 4 /
119, 1 / 124, 1) y aplicación atómica con resync de las cuatro ventanas al rechazar.

Falta:

| Elemento | Detalle | Referencia |
|---|---|---|
| `ItemStackRequestPacket` (0x93) | Solicitudes de item de contenedores; no existe | `NetworkSession`, capturas `data/bds-capture/` |
| NBT mínimo | `TAG_Compound/String/Int/List` para `/give [... tags]` y conservar `nbt` al mover | `crates/bedrock-protocol/src/nbt.rs` (nuevo) |
| Items data-driven | Mapear identificadores string del registro de BDS a `network_id` + NBT | `data/required_item_list.json`, `inventory.rs` |
| Drops como entidad | Depende de fase 4 | `session.rs` |

Criterio de salida: mover un stack en el cliente se refleja en el servidor, `/give
Steve diamond_sword 1` funciona con meta y con NBT simple, y hay roundtrip contra
capturas reales.

---

## 10. Fases 4 a 9 (detalle)

### Fase 4 — Entidades, combate y experiencia (🟡)

En curso: `RemoveActorPacket` (0x0E) y `MoveActorAbsolutePacket` (0x12) ya están en
`bedrock-protocol` con tests de ida y vuelta. Faltan `AddPlayerPacket` (0x0C),
`PlayerListPacket` (0x3F) —que arrastra el serializador de skins—, `AddActorPacket`,
`SetEntityData`,
`UpdateAttributes` (0x1D).

1. `entity.rs`: `Entity`, `Living`, `Human`, `Player` con runtime ID, posición, skin y
   nametag.
2. Visibilidad: `AddPlayerPacket` a todos al entrar y al salir `RemoveActorPacket`;
   `PlayerListPacket` para la lista de jugadores.
3. Daño: `damage.rs` con causas (caída, ahogo, fuego, vacío, ataque), armadura,
   invulnerabilidad y regeneración (`health.rs`).
4. Muerte: drops del inventario, `ActorEvent`, respawn en el spawn real y
   `SetSpawnPosition`.
5. Experiencia: niveles, `AddPlayerPacket` con XP y `/xp`.
6. Eventos PHP `EntityDamageEvent` y `EntityDeathEvent`.

Criterio: dos clientes se ven, se dañan, mueren con drops y respawnean; `/kill` y `/xp`
coherentes.

### Fase 5 — Crafteo, contenedores y uso de objetos (🔴)

`CraftingDataPacket` desde `data/recipes/` y `data/creative/`; contenedores con
`ContainerOpen/Close` y `InventoryContent` por ventana (cofre 27, horno 3, mesa 9);
validación de `InventoryTransaction` para ventanas no jugador; durabilidad, comida y
pociones base; colocar bloques con el `block_runtime_id` correcto.

Criterio: craftear en mesa, abrir un cofre y comer sin desincronizar el inventario.

### Fase 6 — Mundo completo (🔴)

Luz de cielo/bloque, `UpdateBlockPacket` y `BlockEventPacket`, estados de bloque reales
(`canonical_block_states.nbt`, `block_properties_table.json`), física barata (arena,
grava, agua/lava), persistencia de chunks y entidades, biomas reales
(`biome_definitions.json`).

Criterio: romper y colocar bloques sobrevive al reinicio, con luz y bioma correctos.

### Fase 7 — API de plugins a la par de PM (🔴)

Inventario y mundo reales desde PHP, entidades y eventos de combate/muerte, formularios
(`ModalForm`, `SimpleForm`, `CustomForm` + respuesta), scoreboard, boss bar, `Title`,
partículas, `AsyncPool` real.

Criterio: los 4 plugins de prueba usan inventario y formularios sin cambios y un plugin
nuevo de PM 5 arranca sin adaptaciones.

### Fase 8 — Red, escalado y compatibilidad (🔴)

`InventoryTransaction` bajo carga, límites de batch y compresión configurable, rate
limiting con `PacketViolationWarning`, multiworld
(`HashMap<String, Arc<WorldState>>`), `transfer-server` real, `timings` con datos reales
y benchmark de generación.

Criterio: 20 bots sin fugas ni desincronización.

### Fase 9 — Operación y cierre (🔴)

Recarga en caliente de `ops.txt`/`white-list.txt`, crash dumps legibles, logs rotativos,
CI (`cargo fmt --check`, `clippy`, `cargo test --workspace`, `php -l`) y documentación de
paridad por comando y paquete (esta matriz es el punto de partida).

Criterio: CI verde y paridad documentada.

---

## 11. Registro técnico de incidencias

| # | Incidencia | Causa | Fix | Estado |
|---|---|---|---|---|
| 1 | Solo 27 de 248 paquetes | port incremental desde capturas de BDS | fases por subsistema | abierto |
| 2 | Un solo mundo (`world: Arc<WorldState>`) | diseño inicial | `HashMap<String, Arc<WorldState>>` en fase 8 | abierto |
| 3 | Jugadores no se ven entre sí | `session.rs` no envía `AddPlayerPacket` | fase 4 | abierto |
| 4 | Inventario sólo 4 ventanas | `inventory.rs` sin contenedores | fase 5 | abierto |
| 5 | Persistencia JSON en vez de NBT | evitar serializador NBT | fase 6 | decisión consciente |
| 6 | `create_session` creaba dos sesiones con el mismo id | insertaba una y devolvía otra distinta | registra una sola sesión y devuelve su id, que es con el que se le envían paquetes | completo |
| 7 | Desconexión "bad packet" al entrar | 441 chunks en una sola respuesta | límite de 8-16 chunks por respuesta, ordenados por distancia | aplicado; verificación en vivo pendiente |
| 8 | Congelación al apagar con Ctrl+C | `shutdown_sync` no esperaba al worker y bloqueaba Tokio | `join` del worker + `spawn_blocking` | aplicado; verificación en vivo pendiente |
| 9 | TPS iniciales en 0 y bajos al generar | denominador de ventana completa + generación síncrona | `start_time` en `TickMetrics`; menos chunks por respuesta | aplicado; verificación en vivo pendiente |
| 10 | Generación sin multihilo | todo en el hilo de red | `rayon` en los tres caminos de streaming de chunks | aplicado; verificación en vivo pendiente |
| 11 | TP desconecta por `PacketViolationWarning` | chunks de zona lejana enviados antes de confirmar el TP | `force_move_sync`, reenvío de `MovePlayerPacket` y retorno temprano sin chunks | aplicado; verificación en vivo pendiente |
| 12 | Paridad de `/tp` | faltaban yaw/pitch y coordenadas `^` | `teleport()` con 5-6 args, `parse_coordinate` con `^`, clamp de Y | completo |
| 13 | Nodos `.self`/`.other` incompletos | `clear`/`gamemode`/`tp` usaban `can_use_privileged` | nodos por objetivo en `clear`, `gamemode`, `tp`, y nodo por subcomando en `whitelist` | completo |
| 14 | Gate `self\|other` y nodos de `/time` | el gate exigía un solo nodo del par; `/time` usaba `can_use_privileged` | `permission_nodes()` + gate `any()` + nodos por subcomando | completo |
| 15 | Textos propios en `say`, `me`, `tell`, `kick`, `time` y `version` | no se usaban las claves de `spa.ini` | constantes `CHAT_*`, `MESSAGE_*`, `KICK_SUCCESS*`, `TIME_*`, `VERSION_*` | completo |
| 16 | `/whitelist on\|off` y `/save-on\|off` no persistían | banderas en memoria | `config::set_server_property` + `ServerConfig::auto_save` | completo |
| 17 | `/spawnpoint` no actualizaba el cliente | faltaba el paquete | `SetSpawnPosition` al objetivo | completo |
| 18 | Un login no-JWT aceptaba cualquier cadena como nombre | estrategia 3 sin validar | validación tipo `Player::isValidUserName` | completo |
| 19 | Tests de chunks y de métricas desactualizados | las constantes cambiaron (8 chunks, clamp de ventanas) | tests ajustados al comportamiento documentado | completo |
| 20 | `/list` y `/help` con texto propio y sin paginar | implementación preliminar sin claves ni semántica de PM | `commands.players.list`, `commands.help.header`, `pocketmine.command.help.specificCommand.*`, `pocketmine.command.notFound`; `max_players` en `CommandSource` | completo |
| 21 | Comando desconocido en inglés y consola fuera de la difusión administrativa | texto propio en `session.rs` y `tracing::warn` en la consola | `command::not_found_message` (`pocketmine.command.notFound`) y `chat.type.admin` registrado por la consola | completo |
| 22 | `/title` y su paquete no existían | comando pendiente de la fase 2 | `SetTitlePacket` (0x58) con varints con signo, `TitleAction` y `CommandAction::SendTitle` | completo |
| 23 | `transferserver` bloqueado | faltaba `TransferPacket` | `TransferPacket` (0x55) y `CommandAction::Transfer`, que cierra la sesión como `NetworkSession::transfer()` | completo |
| 24 | Textos sin verificación contra el locale | constantes copiadas a mano sin red | `translations::Translator` con `spa.ini` embebido y test constante-por-clave | completo |

### 11.1 `create_session` duplicaba sesiones (resuelto)

`create_session` construía `session`, la insertaba en el mapa y después construía una
segunda instancia con el mismo `id` que devolvía envuelta en `Arc<RwLock<_>>`. El único
llamador (`main.rs`, el listener de RakNet) descartaba ese valor, así que la copia se
perdía y el trabajo de construcción se duplicaba: un manejador futuro habría encolado
paquetes en una sesión que el mapa nunca veía.

Arreglo: el `SessionManager` es el único dueño de la sesión. `create_session` construye
**una** sesión, la inserta y devuelve su `u64`, que es justo el parámetro con el que el
transporte direcciona los paquetes en `handle_session_packet`. Un `Arc<RwLock<PlayerSession>>`
en el mapa habría exigido bloquear cada sesión individualmente en los ~15 puntos que
hoy recorren el mapa con un solo candado, con riesgo de interbloqueo en el bucle de red,
sin ganancia mientras no exista un consumidor del handle. Cubierto por
`create_session_registers_a_single_session_per_client`, que además comprueba que el id
devuelto responde por sí mismo.

### 11.2 Comprobaciones de paridad ejecutadas (2026-11-06)

```
cargo check --workspace --all-targets                          → OK (0 warnings)
cargo test -p server-core --lib                                → 59 OK
cargo test -p server-core --test item_health_commands_test     → 12 OK
cargo test -p server-core --test session_lifecycle_test        → 10 OK
cargo test -p bedrock-protocol                                 → 38 + 11 OK
```

---

## 12. Backlog ordenado por impacto en paridad

1. ✅ ~~Gate de permisos por conjunto (`self OR other`)~~: `permission_nodes()` + gate
   `any()` + nodo por objetivo en los handlers, con test (§5.1).
2. ✅ ~~Persistir `white-list` y `auto-save`~~: `config::set_server_property`,
   `ServerConfig::auto_save` y restauración al arrancar (§8).
3. ✅ ~~`SetSpawnPosition` al objetivo de `/spawnpoint`~~ (paquete enviado al jugador afectado).
4. ✅ ~~Textos propios de `say`, `me`, `tell`, `kick`, `time` y `version`~~: ahora usan
   `chat.type.*`, `commands.message.*`, `commands.kick.*`, `commands.time.*` y
   `pocketmine.command.version.*`.
5. ✅ ~~`/list` con `commands.players.list`~~: `max_players` viaja en `CommandSource`
   (`max-players` de `server.properties`) y los nombres van en una segunda línea.
6. ✅ ~~Motor de traducciones desde `spa.ini` + test constante-por-clave~~:
   `resources/spa.ini` embebido, `translations::Translator` con parámetros posicionales
   y nombrados, y test que compara 25 constantes con el locale (§6).
7. ✅ ~~`/help` con paginación y filtrado por permisos~~: `commands.help.header`,
   `pocketmine.command.help.specificCommand.*` y `pocketmine.command.notFound`, con
   7 líneas por página (la consola no pagina) y gate `testPermissionSilent` en el listado.
8. **NBT mínimo** y `/give [... tags]` (fase 1).
9. ✅ ~~`SetTitlePacket` (0x59) y `/title`~~: el paquete real es 0x58; `/title` cubre
   `clear|reset|title|subtitle|actionbar|times` con nodos `.self`/`.other` (fase 2).
10. ✅ ~~Arreglar `create_session`~~: registra una sola sesión y devuelve el `u64` con el
    que el transporte le envía paquetes (§11.1).
11. ✅ ~~Consola en `broadcast_command_message` y `ServerCommandEvent`~~: la consola
    registra `chat.type.admin` sin color y el evento de consola ya se despachaba (fase 3.3).
12. **Harness de comparación con PM**: mismo comando contra PM y contra este servidor,
   comparando mensajes de `spa.ini` (§13 de cierre).

---

## 13. Checklist de cierre al 100 %

- [ ] Los 41 comandos de PM con sus alias, argumentos, nodos y mensajes exactos.
- [ ] Ningún comando propio aparece en la lista de PM (`/help` lo marca como extra).
- [ ] Gate de permisos con `self OR other` y attachments probados.
- [ ] Traducciones leídas de `spa.ini`, sin constantes huérfanas.
- [ ] `white-list`, `auto-save`, bans y whitelist con el mismo formato que PM o migración
      documentada y probada.
- [ ] Persistencia NBT (`level.dat`, `player.dat`) o decisión final documentada.
- [ ] Dos clientes se ven, se dañan, mueren con drops y respawnean.
- [ ] Crafteo, contenedores y uso de objetos sin desincronizar.
- [ ] Plugins PM 5 sin adaptaciones (inventario, entidades, formularios).
- [ ] CI: `cargo fmt --check`, `clippy`, `cargo test --workspace`, `php -l`.
- [ ] Harness de comparación con PM en verde para los 41 comandos.

---

## 14. Comandos útiles

```bash
export PATH="$HOME/.cargo/bin:$PATH"

# Compilar y testear (usar un target aparte si target/debug está bloqueado)
CARGO_TARGET_DIR=target/test-parity cargo check --workspace --all-targets
CARGO_TARGET_DIR=target/test-parity cargo test -p server-core --lib
CARGO_TARGET_DIR=target/test-parity cargo test -p bedrock-protocol

# Servidor completo
cargo run -p server-core

# Lint de los PHP del puente
for f in crates/php-plugin-bridge/php/*.php; do php -l "$f"; done

# Puente PHP aislado
php target/harness3.php | php crates/php-plugin-bridge/php/bootstrap.php
```

---

## 15. Criterio de salida global

Cuando se completen las fases 1-9:

1. Clientes Bedrock 1.26.60 entran, ven terreno y juegan.
2. Multijugador real con daño, muerte y drops.
3. Crafteo, contenedores y uso de objetos.
4. Mundo y jugadores persistentes al reiniciar.
5. Plugins PHP de PM 5 sin modificaciones.
6. Los 41 comandos de PM más los extras de Bedrock, con mensajes de `spa.ini`.
7. CI completa en verde y paridad documentada comando a comando.
