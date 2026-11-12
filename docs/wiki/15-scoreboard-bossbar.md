# 15 — Scoreboard y BossBar

Paneles laterales (`scoreboard`) y barras de jefe (`bossbar`):
objetivos con líneas puntuadas, tint de fondo, barra compartida o
por jugador, y su entrega en Bedrock y Java. Port de
`pocketmine\scoreboard` y del virion `xenialdan\apibossbar`
(apiBossBar) con el mismo patrón que el resto del bridge: **PHP es
dueño del objeto y reenvía el estado completo en cada mutación;
Rust hace diff contra su copia y emite los paquetes mínimos**.

## Mapa rápido

| Archivo | Responsabilidad |
|---|---|
| `crates/server-core/src/scoreboard.rs` | `Scoreboard`, `ScoreboardObjective`, `ScoreboardLine`, `ScoreboardSlot`, `ColorRange`, paquetes Bedrock, `ScoreboardFile` |
| `crates/server-core/src/bossbar.rs` | `BossBar`, `DiverseBossBar`, `BossBarManager`, paquetes Bedrock/Java, colas Java |
| `crates/server-core/src/session.rs` | Entrega y diff: `bossbar_apply_data`, `handle_boss_event`, `apply_scoreboard_operation`, `save/load_scoreboards`, `take_scoreboard_java_frames`, `take_bossbar_java_frames` |
| `crates/server-core/src/command.rs` | Comando nativo `/scoreboard` (alias `sb`) |
| `crates/java-protocol/src/scoreboard.rs` | Payloads Java `scoreboard_objective`/`scoreboard_score`/`display_objective`/`reset_score` |
| `crates/java-protocol/src/bossbar.rs` | Payloads Java `boss_bar` (la referencia de formato) |
| `crates/java-bridge/src/bossbar.rs` | Copia byte-idéntica para `java-bridge` (no depende de `java-protocol`) |
| `crates/php-plugin-bridge/php/scoreboard.php` | `pocketmine\scoreboard\*` (objeto PHP) |
| `crates/php-plugin-bridge/php/bossbar.php` | `xenialdan\apibossbar\*` + `BossBarColor`/`BossBarOverlay` |
| `crates/php-plugin-bridge/src/context.rs` | DTOs `ScoreboardData`, `BossBarData`, `BossBarOverrideData` y acciones `set_scoreboard`/`set_bossbar` |
| `docs/plugin-api-contract.md` | Forma exacta de las acciones |

## 1. Scoreboard en Rust

### `ScoreboardSlot`: la trampa de numeración

```rust
// crates/server-core/src/scoreboard.rs
pub enum ScoreboardSlot { Sidebar, BelowName, List }  // Default = Sidebar

slot.bedrock_value()  // Sidebar=1, BelowName=2, List=3   (DisplaySlot Bedrock)
slot.java_position()  // List=0, Sidebar=1, BelowName=2   (orden Java)
ScoreboardSlot::from_bedrock(u8) / from_java(i32) / from_str_name("sidebar"|"side"|"below_name"|"below"|"list")
```

Las dos numeraciones **no son una permutación la de la otra**:
Bedrock es sidebar/below/list = 1/2/3 y Java es list/sidebar/below =
0/1/2. Cada plataforma usa su función; mezclarlas es el error
clásico que el test `slot_values_are_not_interchangeable_between_platforms`
fija. Los slots "ObjectScreen" del comando Bedrock no se modelan:
PocketMine solo expone los tres vanilla.

### `ColorRange`: tint de fondo

```rust
pub struct ColorRange { pub r: u8, pub g: u8, pub b: u8 }  // Default = red (255,85,85)
ColorRange::rgb(r,g,b) / red() / black() / white()
from_slice(&[r,g,b]) -> Option<ColorRange>   // forma wire de PHP: [r,g,b]
color_code() -> char                          // código §0-§f vanilla más cercano (distancia euclídea)
prefix() -> String                            // "§c" para rojo
```

Bedrock no tiene campo de color en `SetDisplayObjectivePacket`, así
que el núcleo codifica el tint como código `§` antepuesto al nombre
visible (`wire_display_name()`); Java lo ignora (nombres como
componentes de texto plano).

### Líneas y objetivos

```rust
pub struct ScoreboardLine { pub score: u32, pub text: String }
line.fake_player_id()  // "§r{score}§{text}" — id de fake-player en el wire
line.identity() -> i64  // FNV-1a de fake_player_id, nunca 0 (SetScoreboardIdentity)

pub struct ScoreboardObjective {
    pub name: String, pub display_name: String, pub slot: ScoreboardSlot,
    pub sort_order: u8,                     // default DESCENDING
    pub background: Option<ColorRange>, pub lines: Vec<ScoreboardLine>,
}
objective.add_score(score, text) -> bool    // reemplaza por score; false si el texto ya era ese
objective.remove_score(score) -> bool / get_score(score) / sorted_lines() / clear_lines()
```

El `score` es la clave de ordenación: con orden descendente (el
defecto), el score mayor se renderiza arriba; los empates se
desempitan por texto. Las líneas son únicas por score dentro de un
objetivo.

### `Scoreboard`

```rust
pub struct Scoreboard { pub title: String, pub objectives: Vec<ScoreboardObjective> }

add_objective(objective)          // nombres únicos: un segundo objetivo con el mismo nombre lo reemplaza
remove_objective(name) -> bool / get_objective(name) / get_objective_mut(name)
add_score(objective, score, text) -> bool   // false si el objetivo no existe (PM lanza; aquí no se pánico)
remove_score(objective, score) -> bool
clear() / is_empty()
```

### Paquetes Bedrock (0x6A / 0x6B / 0x6C / 0x70)

`bedrock_packets()` genera, **por objetivo**, en este orden:

1. `SetDisplayObjectivePacket` (0x6B): `objective_name`,
   `display_name` (con el prefijo `§` del tint), `criteria = "dummy"`,
   `sort_order`, `display_slot` (`slot.bedrock_value()`).
2. Un `SetScoreboardIdentityPacket` (0x70) por línea:
   `fake_player_id()` → `identity()`.
3. Un único `SetScorePacket` (0x6C) con todas las líneas del
   objetivo (`ScoreEntry::set(fake_id, objective_name, score)`).

`bedrock_remove_packets()`: un `RemoveObjectivePacket` (0x6A) por
objetivo. `bedrock_diff_packets(old)`: primero los objetivos que
desaparecieron y después el estado completo nuevo (Bedrock no tiene
actualización in-place de objetivos; re-añadir uno lo sobrescribe).

Los paquetes salen tipados como `PacketBytes` (enum con
`packet_id()`/`encode()`); `SessionManager` los mapea a
`OutgoingPacket` para no ensamblar bytes a mano.

### Persistencia: `scoreboard.json`

PocketMine guarda scoreboards solo en memoria; este port añade un
snapshot JSON, como `world.json`:

```rust
pub struct ScoreboardFile {
    pub version: i32,                    // SCOREBOARD_FILE_VERSION = 1
    pub global: Option<Scoreboard>,      // tablero por defecto
    pub players: HashMap<String, Scoreboard>,  // copia personal por jugador
}
Scoreboard::to_json() / from_json()      // roundtrip por objetivo
```

`SessionManager::save_scoreboards()` escribe `scoreboard.json` en
pretty-JSON dentro de `data_dir` (sin `data_dir`, no-op);
`load_scoreboards()` lo lee al arrancar: fichero ausente = vacío,
fichero ilegible = aviso y arranque sin objetivos persistidos.
Cada mutación de `/scoreboard` guarda al momento
(`apply_scoreboard_operation` → `save_scoreboards`), y el cierre del
servidor también.

Resolución por sesión:

- Al crear la sesión se siembra el tablero global.
- Cuando el nombre de login es definitivo (spawn), `sync_scoreboard_after_spawn`
  resuelve la copia personal persistida —una sola vez por sesión
  (`scoreboard_synced`)— y encola el diff contra lo ya enviado.
- `get_scoreboard(name)`: copia personal si existe, si no el global.
- `set_global_scoreboard(...)` reenvía el nuevo global solo a las
  sesiones sin copia personal.

### Scoreboard en Java

`crates/java-protocol/src/scoreboard.rs` (payloads) +
`take_scoreboard_java_frames` (diff por jugador en `session.rs`):

| Paquete | 340 (1.12.2) | 769 (1.21.4) | 770/773/774/775 |
|---|---|---|---|
| `scoreboard_objective` | `0x42` | 100 | 99 / 104 / 104 / 106 |
| `scoreboard_score` | `0x45` | 104 | 103 / 108 / 108 / 110 |
| `scoreboard_display_objective` | `0x3B` | 92 | 91 / 96 / 96 / 98 |
| `reset_score` | — | 73 | 72 / 77 / 77 / 79 |

Funciones: `modern_objective_create/remove/update`,
`modern_score_payload`, `modern_reset_score_payload` (el borrado
moderno ya no usa el campo action de `scoreboard_score`),
`modern_display_payload(java_position, name)` —un nombre vacío
oculta el slot— y sus equivalentes `legacy_*` (criterios `"dummy"`,
título como string plano; en moderno el display name es un
componente NBT `{"text": …}` y el render type es `integer`).

Diferencias con Bedrock en el drain: la entrada es el **texto de la
línea** (Java no tiene fake-player ids) y el tint `ColorRange` se
ignora.

### Lado PHP

`crates/php-plugin-bridge/php/scoreboard.php`
(`pocketmine\scoreboard`):

- `ScoreboardSlot`: constantes `SIDEBAR = 1`, `BELOW_NAME = 2`,
  `LIST = 3` (valores wire Bedrock) + `fromInt`/`fromString`/`toString`.
- `ColorRange` (constructores `red/black/white`, `fromArray`/`toArray`).
- `ScoreboardLine::getFakePlayerId()` — debe coincidir con Rust
  (`"§r" . score . "§" . text`).
- `ScoreboardObjective::addScore/removeScore/getScore/getLines/clearLines`.
- `Scoreboard::addObjective` (reemplaza por nombre),
  `removeObjective`, `addScore`/`removeScore` (false si falta el
  objetivo), `toArray()`.

`Player::setScoreboard()` (`php/player.php`) instala el emisor
(`setOnChange`): **cada mutación reemite la acción completa**
`set_scoreboard`; `removeScoreboard()` emite `remove_scoreboard`.
Rust difumina… mejor: Rust difiere el board completo contra su
copia guardada y emite solo los paquetes que cambian.

### Comando `/scoreboard` (alias `sb`)

Extensión Rust sobre `Player::setScoreboard()` (el `/scoreboard` de
Bedrock gestiona objetivos del servidor, que este port no expone):

```
/scoreboard <player|global> <clear|add|remove|set> ...
```

| Operación | Efecto |
|---|---|
| `clear` | Borra todos los objetivos (`Player::removeScoreboard()`). |
| `add <obj> <score> <text...>` | Añade/reemplaza una línea; si el objetivo no existe lo crea con display name `= <obj>` en sidebar. `score` en `0..1_000_000_000`. |
| `remove <obj> <score>` | Quita una línea. |
| `set <obj> <display...>` | Crea el objetivo (sidebar) o actualiza su display name y slot, **conservando las líneas**. |

- `global` ataca el tablero por defecto persistido en
  `scoreboard.json`; un nombre ataca la copia personal de ese
  jugador (una primera mutación **no** copia el global en ella).
- Permisos: `COMMAND_SCOREBOARD_SELF` / `COMMAND_SCOREBOARD_OTHER`
  (nodos `[self, other]`, como el resto de comandos con objetivo).
- Éxito: `Scoreboard command successfully executed`.
- Hueco conocido: la ayuda anuncia también `list`, pero el handler
  solo implementa `clear|add|remove|set` — `list` cae en el mensaje
  de uso. `ScoreboardOperation::RemoveObjective` existe en el enum
  (y lo aplica `session.rs`) pero ningún subcomando lo genera.

## 2. BossBar en Rust

Port de `xenialdan\apibossbar` (`BossBar.php`, `DiverseBossBar.php`,
`API.php`/`PacketListener.php`). "Añadir jugador" siempre muestra;
"ocultar" solo oculta sin desuscribir; "quitar" oculta y desuscribe.

### `BossBar` compartida

```rust
// crates/server-core/src/bossbar.rs
pub struct BossBar {
    pub actor_id: i64,                    // $actorId; sintético si no hay entidad ligada
    pub bound_entity: Option<String>,     // set_entity()
    // título, subtítulo, percentage, color, overlay, flags, viewers (minúsculas)
}
BossBar::new(actor_id)                    // "" / "" / 1.0 / PURPLE / PROGRESS / 0 flags / sin espectadores, sin spawn
BossBar::create_for_entity(actor_id, entity)
```

Setters fluidos (devuelven `&mut Self`, como el PHP):

| Método | Detalle |
|---|---|
| `set_title` / `set_subtitle` | `getFullTitle()` = título + `"\n\n" + subtítulo` si el subtítulo no está vacío (`full_title()`) |
| `set_percentage(f32)` | Clamp exacto de `setPercentage`: `min(1.0, max(0.0, p))` (`clamp_percentage`) |
| `set_color(u8)` / `set_color_typed(BossBarColor)` / `set_overlay(u8)` / `set_style(color, overlay)` | Color/overlay verbatim; en Bedrock `set_color` reenvía `show` (no hay evento de color suelto en apiBossBar) |
| `set_flags(u8)` | Solo Java: `FLAG_DARKEN_SCREEN(1) \| FLAG_PLAY_MUSIC(2) \| FLAG_CREATE_WORLD_FOG(4)` |
| `add_player` / `add_players` / `remove_player` / `remove_players` / `remove_all_players` | Espectadores por nombre en minúsculas (Bedrock y Java comparten namespace); `viewers()` ordenado, `players()` = `getPlayers()`, `viewer_count()`, `is_viewer(name)` |
| `set_entity(Option<String>)` / `reset_entity(bool)` | Liga/desliga entidad; `reset_entity` no cierra entidades (el núcleo no tiene ciclo de vida de entidades) |

Colores Bedrock (`BossBarColor`): `PINK=0, BLUE=1, RED=2, GREEN=3,
YELLOW=4, PURPLE=5 (defecto), REBECCA_PURPLE=6, WHITE=7`.
Overlays (`BossBarOverlay`): `PROGRESS=0, TEN=1, TWENTY=2, SIX=3,
FOUR=4, THREE=5`.

Nota: el struct Rust **no guarda la lista de ocultos**. `hideFrom`/
`showTo` viven en el DTO (`hidden`) y en PHP; la capa de entrega
(`session.rs`, §4) los aplica al difuminar. El comentario de cabecera
de `bossbar.rs` menciona `hide_from`/`show_to` como parte del API
upstream, pero en el struct solo existen los constructores de
paquetes de abajo.

### Paquetes Bedrock: `BossEventPacket` (0x4A)

| Constructor | Evento | Cuándo (upstream) |
|---|---|---|
| `show_packet()` | `TYPE_SHOW` | `addPlayer` / `showTo` / `setColor` / `setEntity` |
| `hide_packet()` | `TYPE_HIDE` | `removePlayer` / `hideFrom` |
| `title_packet()` | `TYPE_TITLE` | `setTitle` / `setSubTitle` |
| `health_packet()` | `TYPE_HEALTH_PERCENT` | `setPercentage` |
| `properties_packet()` | `TYPE_PROPERTIES` | color + overlay |

Todos con `boss_actor_unique_id = actor_id` y el título completo.

### `DiverseBossBar`: overrides por jugador

Envuelve una `BossBar` compartida + mapas por jugador
(`titles`, `subtitles`, `percentages`, `colors`):

```rust
set_title_for(players, title) / title_for(player) -> String      // override o defecto compartido
set_subtitle_for(...) / subtitle_for(...) / set_percentage_for(...) / percentage_for(...)
set_color_for(...) / color_for(...) / reset_for(player) / reset_forAll()
remove_player(...)  // también borra los overrides de ese jugador
show_packet_for(player) / title_packet_for(player) / health_packet_for(player)
java_add_frame_for(player, protocol)
```

Quirk upstream conservado bug-for-bug en `full_title_for`: el texto
es el título **override** (o `""`) más el subtítulo override; solo
si el resultado queda vacío cae al `full_title()` compartido. No
interpola el título compartido con un subtítulo override. Lo mismo
en `getFullTitleFor` de PHP.

### `BossBarManager`: registro thread-safe

```rust
pub struct BossBarManager {
    bars: RwLock<HashMap<i64, BossBar>>,
    diverse: RwLock<HashMap<i64, DiverseBossBar>>,
    next_actor_id: Mutex<i64>,          // empieza en 1_000_000 (Entity::nextRuntimeId())
    java_queues: RwLock<HashMap<String, VecDeque<BossBarFrame>>>,
    java_sent: RwLock<HashMap<(String, i64), JavaSentState>>,  // último estado Java por (espectador, barra)
}
create() -> i64 / create_for_entity(actor_id, entity) -> i64 / create_diverse() -> i64
remove(actor_id) -> bool / contains(actor_id) / get(actor_id) / get_diverse(actor_id)
all_actor_ids() / with_bar(actor_id, f) / with_diverse(actor_id, f)
shared_insert(...) / shared_remove_quiet(...) / diverse_insert(...) / diverse_remove_quiet(...)
queue_java_frame(name, frame) / take_java_frames(name) / queued_java_frames(name)
java_sent_get/set/remove/ids/clear(viewer)   // diff ADD/UPDATE_*/REMOVE + limpieza al salir
```

Los ids sintéticos (desde 1 000 000) no colisionan con los
`entity_unique_id` de sesión Bedrock (1..N). Para el caso real de
jefe-mob, pasa el `entity_unique_id` de la sesión como `actor_id`
al construir.

### BossBar en Java: `boss_bar`

Payload `UUID(16) + VarInt(action)`; ids por protocolo (la pregunta
"¿0x0C?" es solo el legacy):

| Protocolo | Versión | Id |
|---|---|---|
| 340 | 1.12.2 | `0x0C` |
| 769 | 1.21.4 | `0x0A` |
| 770–775 | 1.21.5+ | `0x09` |

Acciones: `0 ADD` (title, health f32, color, dividers, flags),
`1 REMOVE`, `2 UPDATE_HEALTH`, `3 UPDATE_TITLE`, `4 UPDATE_STYLE`
(color, dividers), `5 UPDATE_FLAGS`. En `BossBar`:
`java_add_frame(protocol)`, `java_remove_frame(protocol)`,
`java_update_health/title/style/flags_frame(protocol)`.

- Título legacy (340): string plano; moderno (769+): componente
  NBT anónimo `{"text": …}`.
- Color: Bedrock `0..7` → Java `0..6` (`bedrock_color_to_java`):
  `rebecca_purple (6) → purple (5)`, el resto por índice.
- UUID determinista por `actor_id` (`bossbar_uuid`, SplitMix64 con
  bits de versión 3): la misma barra conserva UUID entre ADD y los
  UPDATE/REMOVE posteriores.
- El formato está duplicado byte a byte en `java-protocol/src/bossbar.rs`
  (referencia) y `java-bridge/src/bossbar.rs` (el que usa el bridge,
  que no depende de `java-protocol`).

## 3. Lado PHP: `xenialdan\apibossbar`

`crates/php-plugin-bridge/php/bossbar.php`:

- `pocketmine\network\mcpe\protocol\types\BossBarColor` (0..7) y
  `BossBarOverlay` (`PROGRESS=0, TEN=1, TWENTY=2, SIX=3, FOUR=4, THREE=5`).
- `xenialdan\apibossbar\BossBar`: `$actorId` público con
  `nextRuntimeId` estático desde 1 000 000 (misma base que Rust).
  `createForEntity`, `addPlayer(s)`/`removePlayer(s)`/`removeAllPlayers`,
  `setTitle`/`setSubTitle`/`getFullTitle`, `setPercentage` (clamp),
  `setColor`/`setOverlay`/`setStyle(color, overlay)`/`setFlags`,
  `hideFrom`/`hideFromAll`/`showTo`/`showToAll`,
  `getEntity`/`setEntity`/`resetEntity`, `destroy()` → acción
  `remove_bossbar`, `__toString()`.
  - `setEntity` con una entidad cerrada o marcada para despawn
    lanza `InvalidArgumentException` (paridad upstream); con un
    `Player`, su id numérico pasa a ser el `actorId`.
  - `removePlayer` de alguien no suscrito registra un debug en el
    log de `Server` y no hace nada.
- `DiverseBossBar extends BossBar`: `setTitleFor`/`getTitleFor`,
  `setSubTitleFor`/`getSubTitleFor`, `getFullTitleFor` (quirk),
  `setPercentageFor`/`getPercentageFor`, `setColorFor`/`getColorFor`,
  `resetFor`/`resetForAll`. Al quitar jugadores también borra sus
  overrides. Los overrides de jugadores no suscritos se envían igual
  (clave en minúsculas como nombre; Rust es case-insensitive).
- `API::load(Plugin)` y `PacketListener::register`: no-op de
  paridad — el listener real corre en Rust
  (`SessionManager::handle_boss_event`).
- Cada mutación llama a `sync()`: si el bridge está activo, emite la
  acción `set_bossbar` con el estado **completo** (`toData()`):
  `boss_id`, `diverse`, `title`, `subtitle`, `percentage`, `color`,
  `overlay`, `flags`, `entity`, `players`, `hidden`, `overrides`.

## 4. Contrato IPC y entrega en Rust

Acciones (`docs/plugin-api-contract.md`, `context.rs`):

```json
{"action":"set_bossbar","data":{"bossbar":{
  "boss_id":1000000,"diverse":false,"title":"…","subtitle":"…",
  "percentage":0.5,"color":2,"overlay":0,"flags":0,
  "entity":null,"players":["Steve"],"hidden":[],
  "overrides":[{"player":"Steve","title":"…","subtitle":null,"percentage":null,"color":null}]}}}
{"action":"remove_bossbar","data":{"boss_id":1000000}}
```

Defectos del DTO (constructor upstream): `percentage = 1.0`,
`color = 5 (PURPLE)`, `overlay = 0`, `flags = 0`, barra compartida
sin espectadores. `remove_bossbar` oculta la barra a todos los
espectadores y la desregistra.

`SessionManager::bossbar_apply_data(data)`:

1. Construye la barra nueva desde el DTO (compartida o diversa según
   `diverse`; un `boss_id` desconocido crea la barra) y la inserta
   en el manager (reemplaza la variante contraria en silencio).
2. `bossbar_diff_shared` / `bossbar_diff_diverse`: por espectador,
   espectador nuevo o des-oculto → `show` completo; espectador
   existente → solo lo que cambió (`title` → `TYPE_TITLE`,
   `percentage` → `TYPE_HEALTH_PERCENT`, color/overlay → en
   compartida `TYPE_PROPERTIES`, en diversa reenvío de `show`
   porque el override de color no tiene evento propio); quien deja
   de verla o pasa a `hidden` → `hide`.
3. Java: `take_bossbar_java_frames(name, protocol)` difumina contra
   `java_sent_*` y emite ADD / UPDATE_* / REMOVE encolados por
   jugador (las sesiones Bedrock reciben `BossEventPacket` por
   `pending_outgoing` en su lugar).

Paquetes entrantes del cliente (`handle_boss_event`, port de
`PacketListener`): `TYPE_REGISTER_PLAYER`/`TYPE_UNREGISTER_PLAYER`
solo se registran en log; `TYPE_QUERY` reenvía la barra completa
(`show_packet` o `show_packet_for`); cualquier otro tipo se loguea
en lugar de expulsar al jugador (upstream expulsa; aquí un quirk de
cliente vanilla nunca tumba una sesión).

`remove_bossbar` y la salida de un jugador limpian también el
estado `java_sent` del espectador, para que un reingreso empiece
con ADD limpio.

## 5. Verificación

```powershell
cargo test -p server-core scoreboard:: bossbar::
cargo test -p java-protocol scoreboard:: bossbar::
cargo test -p java-bridge bossbar::
```

Tests que fijan el contrato:

- `scoreboard.rs`: `slot_values_are_not_interchangeable_between_platforms`,
  `color_code_maps_to_nearest_vanilla_code`,
  `add_objective_replaces_by_name_and_remove_reports_presence`,
  `bedrock_packets_are_display_then_identity_then_scores`
  (0x6B → 0x70×N → 0x6C, display name con prefijo `§c`),
  `diff_removes_only_dropped_objectives_then_resends`,
  `json_roundtrip_preserves_objectives_lines_and_slot`,
  `scoreboard_file_roundtrip`.
- `bossbar.rs`: `defaults_match_upstream_constructor` (1.0 / PURPLE /
  PROGRESS), `percentage_clamps_and_viewer_mutation`,
  `show_packet_is_boss_event_show` (0x4A), 
  `diverse_falls_back_to_shared_defaults`,
  `diverse_full_title_for_keeps_upstream_quirk`,
  `manager_allocates_unique_ids_and_queues_java`.
- `session.rs`: `bossbar_apply_data_diffs_and_java_poll_delivers_frames`.
- Java: `packet_ids_match_minecraft_data` (0x0C/0x0A/0x09),
  `uuid_is_deterministic_and_versioned`, `add_payload_layout_legacy_vs_modern_title`.
