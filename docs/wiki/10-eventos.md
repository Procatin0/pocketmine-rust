---
title: "10 — Eventos (Plugin API)"
description: "Catálogo de los 143 eventos de la Plugin API (Player 49, Block 25, Entity 33, Inventory 7, World 14, Server 12, Plugin 3): prioridades, Cancellable, cuándo se dispara cada uno, PlayerChatEvent con ChatFormatter y PluginEnable/DisableEvent."
nav_order: 10
---

# 10 — Eventos (Plugin API)

[← Hub de la wiki](index.md) · [01 — Introducción](01-introduccion.md) · [02 — Quickstart: tu primer plugin](02-quickstart.md)

> **Tutorial de plugins:** si aún no tienes un plugin cargado, empieza por
> [02 — Quickstart: tu primer plugin](02-quickstart.md) (`Main.php` con
> `PluginBase::onEnable` / `onDisable`, `plugins/Demo/plugin.yml` y cómo ver
> los logs). Esta página presupone ese plugin y explica solo el sistema de
> eventos: cómo registrar un *listener*, en qué orden se ejecutan, qué
> eventos se pueden cancelar y cuándo dispara el núcleo cada uno.
> Para el contrato IPC completo (JSON-lines, `PluginAction`, campos mutables)
> ver [`docs/plugin-api-contract.md`](../plugin-api-contract.md).

## Fuentes de esta página

Todo lo que sigue se extrae de tres fuentes del repositorio, sin inventar
triggers que no estén documentados:

| Fuente | Qué aporta |
| --- | --- |
| `crates/php-plugin-bridge/php/event.php` | `Event`, `Cancellable`, `Listener`, `EventPriority`, `RegisteredListener`, `EventFactory` y las clases PHP con `fromArray` / `toArray` completos (`Player*`, `BlockBreak` / `BlockPlace`, `CommandEvent` / `ServerCommandEvent`). |
| `crates/php-plugin-bridge/src/event.rs` + `src/events/*.rs` | `EventPriority` Rust, trait `Event` (`name`, `is_cancellable`, `is_cancelled`, `set_cancelled`, `to_json`, `apply_json`), `EventDispatcher` y las structs Rust cableadas (`Player*`, `BlockBreak` / `BlockPlace`, `PlayerInteractEvent`, `AnvilDamageEvent`, `ServerCommandEvent`). |
| `crates/php-plugin-bridge/EVENT_TRIGGERS.md` | Catálogo de eventos pendientes de cableado: qué evento es cancelable, en qué fichero del core se sugiere dispararlo (`session.rs`, `world_state.rs`, `command.rs`, `plugin_host.rs`, `server-core`) y con qué `TODO` exacto. Los grupos con `/` en ese documento (p. ej. `PlayerToggleFlight/Sprint/Sneak/Swim/GlideEvent`) se han expandido aquí en una fila por clase. |
| `crates/php-plugin-bridge/php/plugin.php` y `php/chat.php` | `PluginEnableEvent` / `PluginDisableEvent` (namespace `pocketmine\event\plugin`, ciclo de vida en el `PluginManager` PHP) y `pocketmine\player\chat\ChatFormatter` / `PlaceholderFormatter` que usa `PlayerChatEvent`. |
| `crates/php-plugin-bridge/src/chat.rs` | Equivalente Rust nativo de `ChatFormatter` (`PlaceholderFormatter`, `default_format`). |

### Nota de conteo (143)

El encargo de esta página es **143 clases**: Player 49, Block 25, Entity 33,
Inventory 7, World 14, Server 12 y Plugin 3 (49 + 25 + 33 + 7 + 14 + 12 + 3 = 143).
Las tablas de las secciones 5.1 a 5.7 contienen **exactamente una fila por
clase** con esos conteos, sin duplicar ningún nombre entre categorías.

Dos precisiones para que el conteo sea auditable:

1. **Las clases base cuentan como fila.** `PlayerEvent`, `BlockEvent`,
   `EntityEvent`, `InventoryEvent`, `WorldEvent` + `ChunkEvent`, `ServerEvent`
   y `PluginEvent` son las 8 filas base incluidas en el total de 143; las
   135 restantes son eventos concretos. `PluginEvent` es la base real de
   `php/plugin.php` (`pocketmine\event\plugin`), y `PlayerEvent` / `BlockEvent`
   son las bases reales de `php/event.php`.
2. **`PlayerAllowFlightChangeEvent` es la única fila no listada en
   `EVENT_TRIGGERS.md`.** Se incluye para cuadrar el conteo Player 49 de
   PM upstream y se marca con esa advertencia en su fila: antes de usarla
   como trigger, verifícala contra el árbol PM que uses. Todo el resto de
   filas sale literalmente de expandir los grupos de `EVENT_TRIGGERS.md`
   (más `AnvilDamageEvent`, que en el árbol actual solo existe en Rust, en
   `src/events/block.rs`, y ya está cableado en `session.rs`).
   `BaseBlockChangeEvent` no es fila de tabla: aparece en la sección 6
   (divergencias) como base de los `Block*` nuevos, igual que en
   `EVENT_TRIGGERS.md`.

### Leyenda de estado

| Estado | Significado |
| --- | --- |
| **Cableado** | El core ya dispara el evento hoy (con o sin *gating* `has_listeners`). |
| **Parcial** | Disparo a medias o solo en una capa (p. ej. solo PHP, o solo si hay listeners). |
| **Stub** | Clase/catálogo documentado en `EVENT_TRIGGERS.md` con su `TODO: <fichero>:<Evento>` pendiente; el equipo de visibilidad debe cablearlo en `session.rs`, `world_state.rs`, `command.rs`, `plugin_host.rs` o `server-core`. No asumas que un stub ya se dispara en juego. |

> **Importante:** que un evento aparezca en estas tablas no significa que ya
> se dispare en el servidor. En el árbol actual, las clases PHP con
> `fromArray` / `toArray` completos en `php/event.php` son las 12 de
> `EventFactory::EVENTS` (`PlayerJoinEvent`, `PlayerQuitEvent`,
> `PlayerChatEvent`, `PlayerCommandPreprocessEvent`, `PlayerMoveEvent`,
> `PlayerDeathEvent`, `PlayerRespawnEvent`, `PlayerInteractEvent`,
> `BlockBreakEvent`, `BlockPlaceEvent`, `ServerCommandEvent`,
> `CommandEvent`), más `PlayerDataSaveEvent` (`php/PlayerDataSaveEvent.php`)
> y `PluginEnableEvent` / `PluginDisableEvent` (`php/plugin.php`, que
> nunca cruzan el IPC). En Rust, `src/events/*.rs` define las structs de
> `Player*`, `BlockBreak` / `BlockPlace`, `PlayerInteractEvent`,
> `AnvilDamageEvent` y `ServerCommandEvent`. El resto del catálogo es el
> objetivo documentado en `EVENT_TRIGGERS.md`.

## 1. Fundamentos: Event, Cancellable, prioridades

### 1.1 `Event` y `Cancellable`

En PHP (`php/event.php`, namespace `pocketmine\event`):

```php
abstract class Event{
    public function isCancelled() : bool
    public function cancel() : void
    public function uncancel() : void
    public function setCancelled(bool $cancelled = true) : void
    public function isCancellable() : bool   // $this instanceof Cancellable
    public function getEventName() : string  // nombre corto de la clase
    public static function fromArray(array $data) : Event  // hidratación desde IPC
    public function toArray() : array        // estado final que vuelve a Rust
}

interface Cancellable{
}
```

`Cancellable` es un marcador: solo los eventos que lo implementan pueden
cancelarse de verdad. La base `Event` guarda el flag en cualquier caso
(`cancelled`, con `applyCancelled()` para hidratarlo desde el payload IPC),
pero `isCancellable()` devuelve `false` si la clase no implementa
`Cancellable`. En Rust el equivalente es el método `is_cancellable()` del
trait `Event` (`src/event.rs`), que por defecto devuelve `false`; cada
struct cancelable lo sobrescribe a `true` y guarda su propio campo
`cancelled` con `is_cancelled()` / `set_cancelled()`.

Cómo se propaga la cancelación:

1. Rust crea el evento y lo pasa a `EventDispatcher::dispatch()` (o, si hay
   worker PHP, por IPC `dispatch_event`).
2. Los listeners PHP pueden llamar `$event->cancel()`, `$event->setMessage(...)`,
   `$event->setFormat(...)`, etc.
3. PHP responde siempre con `event` (estado final) + `cancelled`
   (ver `docs/plugin-api-contract.md`, protocolo IPC).
4. Rust aplica el snapshot con `apply_json()` — que **solo actualiza los
   campos presentes** en el payload, para que payloads parciales y runtimes
   PHP antiguos sigan funcionando — y el core decide el efecto (p. ej.
   cancelar el movimiento, no romper el bloque, no difundir el chat).

### 1.2 Prioridades (`EventPriority`)

Definidas igual en PHP (`EventPriority`, `php/event.php`) y en Rust
(`EventPriority`, `src/event.rs`, `#[repr(u8)]`). Se ejecutan en orden
ascendente: primero `LOWEST`, último `MONITOR`.

| Constante | Valor | Cuándo usarla |
| --- | --- | --- |
| `LOWEST` | 0 | Filtros o ajustes muy tempranos (p. ej. censura en chat antes de que otros plugins lean el mensaje). |
| `LOW` | 1 | Ajustes tempranos. |
| `NORMAL` | 2 | **Defecto** si no declaras `@priority`. La mayoría de listeners. |
| `HIGH` | 3 | Ajustes tardíos, cuando ya casi todo el mundo ha actuado. |
| `HIGHEST` | 4 | Última oportunidad de modificar o cancelar antes de `MONITOR`. |
| `MONITOR` | 5 | Solo observación (logs, métricas, auditoría). **Siempre se ejecuta**, incluso si el evento está cancelado, y no debería modificar el evento. |

Detalles de implementación:

- `EventPriority::getPriority(int $priority)` **recorta** el valor a
  `[LOWEST, MONITOR]` (`max` / `min`), así que un entero fuera de rango
  nunca rompe el orden; `getPriorityName()` devuelve el nombre.
- En Rust, `EventDispatcher::register_listener()` ordena la lista de cada
  evento con `sort_by_key(|l| l.priority)` al registrar.
- En PHP, `PluginManager::dispatchEvent()` (`php/plugin.php`) recoge los
  listeners de la clase del evento **y de sus clases padre**, los ordena con
  `usort` por `$listener->getPriority()` y llama a
  `RegisteredListener::callEvent()` uno a uno. Un listener que lanza una
  excepción se registra en el log y no detiene a los demás.

### 1.3 `ignoreCancelled`, `@handleCancelled` y `MONITOR`

La regla de salto es la misma en las dos capas:

- PHP, `RegisteredListener::callEvent()` (`php/event.php`):
  si `$event->isCancelled() && !$this->ignoreCancelled && $this->priority !== EventPriority::MONITOR`,
  el listener **no se llama**.
- Rust, `EventDispatcher::dispatch()` (`src/event.rs`):
  si `event.is_cancelled() && !listener.ignore_cancelled && listener.priority != EventPriority::Monitor`,
  el listener **se salta** (`continue`).

Es decir, en este bridge:

- `ignoreCancelled = false` (defecto): no ves los eventos ya cancelados.
- `ignoreCancelled = true`: sí ves los eventos cancelados (útil para
  auditoría o para *des-cancelar* con `uncancel()`).
- `MONITOR`: siempre te ves el evento, cancelado o no.

⚠️ **Cuidado con el nombre al portar plugins PM upstream.** En PocketMine-MP,
`@ignoreCancelled true` significa «ignora los eventos cancelados» (no te
llaman). En este bridge, tal y como está implementado
(`php/plugin.php::registerEvent()`, parámetro `$handleCancelled`, que se
pasa tal cual como `ignoreCancelled` a `RegisteredListener`, y el
`callEvent()` de arriba), `@ignoreCancelled true` significa «ejecútame
también si está cancelado». El comentario del código lo documenta como
inversión deliberada del nombre. Prueba siempre tu listener cancelado con
y sin `@ignoreCancelled` antes de publicar, y no asumas la semántica PM.

Además, `registerEvent()` lanza `PluginException` si declaras
`@handleCancelled` (es decir, `ignoreCancelled = true`) para un evento que
**no** implementa `Cancellable`.

### 1.4 Gating por `has_listeners` (rendimiento)

Varios triggers del core solo construyen y despachan el evento **si hay al
menos un listener registrado** para ese nombre, p. ej. en `session.rs`:
`PlayerMoveEvent`, `PlayerCommandPreprocessEvent`, `BlockPlaceEvent`,
`AnvilDamageEvent`, `PlayerDeathEvent` y `PlayerRespawnEvent` están
protegidos por `plugins.has_listeners("<Nombre>Event")`. Sin plugins
escuchando, el comportamiento es idéntico al servidor sin plugins y no se
paga el coste de serializar el evento. Esto también explica por qué
`PlayerDeathEvent` solo difunde su mensaje cuando hay listeners (anexo de
integración 2026-11-06 de `EVENT_TRIGGERS.md`).

## 2. Cómo registrar un listener

El flujo completo está en
[02 — Quickstart](02-quickstart.md) para el plugin mínimo; aquí solo el
parte de eventos, con el código del bridge (`php/plugin.php` y
`php/event.php`).

### 2.1 Con `registerEvents()` y docblock (estilo PM)

```php
<?php

declare(strict_types=1);

namespace MiPlugin;

use pocketmine\event\Listener;
use pocketmine\event\player\PlayerChatEvent;
use pocketmine\event\player\PlayerJoinEvent;
use pocketmine\event\EventPriority;
use pocketmine\plugin\PluginBase;

final class Main extends PluginBase implements Listener{

    public function onEnable() : void{
        $this->registerEvents($this, $this);
    }

    /**
     * @priority HIGHEST
     * @ignoreCancelled false
     */
    public function onChat(PlayerChatEvent $event) : void{
        if(str_contains($event->getMessage(), "mala-palabra")){
            $event->cancel();
        }
    }

    /**
     * @priority MONITOR
     */
    public function onJoin(PlayerJoinEvent $event) : void{
        $this->getLogger()->info("Entró " . $event->getPlayer()->getName());
    }
}
```

Qué hace `PluginManager::registerEvents()` (`php/plugin.php`):

1. Recorre los métodos públicos del listener y toma el primer parámetro
   cuyo tipo sea una subclase de `Event`.
2. Lee el docblock: `@priority LOWEST|LOW|NORMAL|HIGH|HIGHEST|MONITOR`
   (también acepta el entero) y `@ignoreCancelled true|false`.
   Defecto: `NORMAL` + `ignoreCancelled false`.
3. Registra `[$listener, $method]` con el plugin dueño. Si el plugin
   implementa `Listener` y no llamó a `registerEvents()`, el runtime puede
   auto-registrar sus métodos al habilitar (compatibilidad hacia atrás).
4. Un listener sin ningún método de evento lanza `InvalidArgumentException`.

### 2.2 Con `registerEvent()` y un `Closure`

```php
use pocketmine\event\EventPriority;
use pocketmine\event\player\PlayerMoveEvent;

$manager = $this->getPluginManager();
$manager->registerEvent(
    PlayerMoveEvent::class,
    function(PlayerMoveEvent $event) : void{
        // ...
    },
    EventPriority::HIGH,
    $this,          // plugin dueño
    false           // handleCancelled: true = ver también los cancelados
);
```

`registerEvent()` valida que la clase sea un `Event`, que el plugin esté
habilitado, que la prioridad sea válida (acepta nombre en string como
extensión del bridge) y que no uses `handleCancelled` en un evento no
cancelable. Devuelve el `RegisteredListener` creado.

### 2.3 Desregistro y ciclo de vida

Al deshabilitar un plugin se eliminan **todos** sus listeners y comandos:
`unregisterEvents($plugin)` en PHP y
`EventDispatcher::unregister_plugin_listeners()` en Rust hacen lo mismo por
nombre de plugin. Por eso un listener solo vive mientras su plugin está
habilitado; ver la sección 4 para el orden exacto de
`PluginEnableEvent` / `PluginDisableEvent` respecto a `onEnable` /
`onDisable`.

### 2.4 `EventFactory` y qué clases se pueden hidratar

`EventFactory::classOf()` / `create()` (`php/event.php`) resuelven un
nombre de evento a su clase PHP:

- Mapa fijo `EVENTS` con las 12 clases de la nota de estado de arriba.
- Si el nombre no está en el mapa, acepta cualquier clase existente que
  sea subclase de `Event` (`class_exists` + `is_a`). Así entran
  `PlayerDataSaveEvent` (fichero propio) y `PluginEnableEvent` /
  `PluginDisableEvent` (definidos en `plugin.php`), aunque
  `PluginEnableEvent::fromArray()` lanza `BadMethodCallException` porque
  esos eventos nunca cruzan el IPC.
- `create()` llama a `fromArray()` y, si falla, escribe
  `[PHP/RUNTIME] Failed to create event ...` en stderr y devuelve `null`.

Consecuencia práctica: para los eventos **Stub** de las tablas, registrar
un listener en PHP solo funcionará cuando existan la clase PHP con
`fromArray` y el trigger en el core. Hasta entonces, el Rust ni siquiera
llega a despachar por IPC.

## 3. `PlayerChatEvent` y `ChatFormatter`

`PlayerChatEvent` es el evento más rico del bridge y el que más plugins
tocan (censura, rangos, canales, formato). Está cableado en `session.rs`
(chat normal de jugador) y es **cancelable**.

### 3.1 Campos y métodos (PHP)

De `php/event.php` (`pocketmine\event\player\PlayerChatEvent extends PlayerEvent implements Cancellable`):

| Campo / método | Tipo | Notas |
| --- | --- | --- |
| `getPlayer()` | `Player` | Heredado de `PlayerEvent`. |
| `getMessage()` / `setMessage(string)` | `string` | Mensaje ya filtrado, mutable. |
| `getFormat()` / `setFormat(string)` | `string` | Línea que se difunde; ver 3.2 (el formatter tiene precedencia). |
| `getFormatter()` / `setFormatter(?ChatFormatter)` | `?ChatFormatter` | `pocketmine\player\chat\ChatFormatter`, ver 3.2. |
| `getRecipients()` / `setRecipients(array)` | `Player[]` | Destinatarios. En el IPC viajan como **nombres** (`toArray()` serializa `$recipient->getName()`), no como blobs de jugador. |
| `cancel()` / `isCancelled()` | — | Cancelar impide la difusión. |
| `fromArray()` / `toArray()` | — | IPC: `player`, `message`, `format`, `recipients`, `cancelled`. |

En Rust (`src/events/player.rs`, `PlayerChatEvent`) los mismos campos:
`player: PlayerContext`, `message: String`, `format: String`
(defecto `"<{name}> {message}"` vía `format!("<{}> {}", player.name, msg)`),
`recipients: Vec<String>`, `cancelled: bool`. `apply_json()` acepta
destinatarios como strings **o** como objetos con `name`, porque el runtime
PHP puede devolver cualquiera de las dos formas.

### 3.2 `ChatFormatter`: el formato no es un string, es un render

En PocketMine-MP, `PlayerChatEvent` no lleva un formato *string* pelado:
lleva un objeto `ChatFormatter` con `format(string $username, string $message): string`.
El bridge lo replica en `php/chat.php`:

```php
namespace pocketmine\player\chat;

interface ChatFormatter{
    public function format(string $username, string $message) : string;
}

class PlaceholderFormatter implements ChatFormatter{
    public function __construct(
        private string $format,
        private array $placeholders = []   // array<string, string>
    ){}
    public function format(string $username, string $message) : string{
        // sustituye {key} de $placeholders + {player} y {message} implícitos
    }
}
```

Y `PlayerChatEvent::getFormat()` lo respeta con esta precedencia
(`php/event.php`):

```php
public function getFormat() : string{
    if($this->formatter !== null){
        return $this->formatter->format($this->player->getName(), $this->message);
    }
    return $this->format;   // defecto '<{PLAYER}> {MESSAGE}'
}
```

Es decir:

1. Si un plugin anterior (en prioridad más baja) llamó
   `$event->setFormatter(...)`, `getFormat()` **renderiza a través del
   formatter**, no devuelve el string crudo.
2. Si no hay formatter, se usa el string de `setFormat()` / el defecto.

En Rust hay un equivalente nativo sin worker PHP
(`src/chat.rs`): trait `ChatFormatter`, `PlaceholderFormatter` con
`format` + `placeholders: HashMap<String, String>` y `default_format(username, message)`
que devuelve `"<{username}> {message}"`. El JSON del evento lleva el
`format` ya renderizado, así que la línea difundida es la misma la
renderice PHP o Rust. El comentario de `src/chat.rs` cita el caso real:
**BetterChat** registra su propio `ChatFormatter(format, placeholders)` con
`{player}`, `{message}`, `{display_name}`, `{prefix}`, `{suffix}`,
`{world}` y `{group}`. En este repositorio puedes verlo en
`plugins/BetterChat/src/ChatFormatter.php` y su `ChatListener` llamando a
`$event->setFormatter(new ChatFormatter($formatString, $placeholders))`.

Ejemplo típico de plugin de rango (prioridad baja, antes que el resto):

```php
/**
 * @priority LOW
 */
public function onChat(PlayerChatEvent $event) : void{
    $player = $event->getPlayer();
    $event->setFormatter(new PlaceholderFormatter(
        "[{prefix}] {display_name} > {message}",
        [
            "prefix" => $this->rangoDe($player),
            "display_name" => $player->getDisplayName(),
        ]
    ));
}
```

Y un filtro de palabras (prioridad alta, última palabra):

```php
/**
 * @priority HIGHEST
 */
public function onChat(PlayerChatEvent $event) : void{
    if(str_contains($event->getMessage(), "mala-palabra")){
        $event->cancel();   // no se difunde nada, ni siquiera el formato
    }
}
```

En `session.rs`, tras despachar, si el evento está cancelado no se difunde;
si `format` viene vacío se usa `"<{username}> {message}"`; si
`recipients` viene vacío se difunde a todos los jugadores en línea.

## 4. `PluginEnableEvent` y `PluginDisableEvent`

Definidos en `php/plugin.php`, namespace `pocketmine\event\plugin`:

```php
abstract class PluginEvent extends Event{
    public function __construct(protected Plugin $plugin){}
    public function getPlugin() : Plugin
    public static function fromArray(array $data) : Event{
        throw new \BadMethodCallException(static::class . ' cannot be hydrated from IPC data');
    }
    public function toArray() : array{
        return ['plugin' => $this->plugin->getName(), 'cancelled' => $this->isCancelled()];
    }
}
final class PluginEnableEvent extends PluginEvent{}
final class PluginDisableEvent extends PluginEvent{}
```

Son **3 clases** (la base + las dos finales) y **no son cancelables**
(no implementan `Cancellable`). Características clave:

- **Nunca cruzan el IPC.** El comentario de la clase lo dice literal:
  «fired by `PluginManager::enablePlugin()` / `disablePlugin()` only; these
  events never cross the IPC boundary (Rust has no lifecycle events)».
  Por eso `fromArray()` lanza excepción: Rust no puede hidratarlos.
  Solo se despachan dentro del worker PHP, entre plugins PHP.
- **Orden en `enablePlugin()`** (`PluginManager` en `php/plugin.php`):
  primero la contabilidad de habilitado y dependencias, **después**
  `dispatchEvent(new PluginEnableEvent($plugin))`, y **después** el
  auto-registro de listeners si el plugin implementa `Listener` y no
  registró ninguno. Es decir, cuando tu `PluginEnableEvent` se ejecuta,
  `onEnable()` ya ha corrido y tus listeners ya están registrados.
- **Orden en `disablePlugin()`**: `PluginDisableEvent` se dispara
  **antes** de `onDisable()`, «so listeners can still observe it», y los
  registros se eliminan **después** (`unregisterEvents()` en el `finally`
  tras `onDisable()` y el shutdown del scheduler). Después se marca el
  plugin como deshabilitado.
- Si un listener de estos eventos lanza, el `PluginManager` lo captura,
  lo registra en el log (`PluginEnableEvent failed for ...`) y sigue con
  la habilitación/deshabilitación.
- En `EVENT_TRIGGERS.md` aparecen como «`PluginEnable/DisableEvent` (—) →
  `plugin_host.rs` lifecycle (no tocar por ownership)
  `TODO: plugin_host.rs:PluginEnableEvent`»: desde el lado Rust no hay
  trigger cableado, y no debes esperarlo en `server-core`.

> Para el ciclo de vida completo (`onLoad`, `onEnable`, `onDisable`,
> `load: startup|postworld`, orden de dependencias) ver
> [01 — Introducción](01-introduccion.md) y el quickstart de
> [02 — Quickstart](02-quickstart.md).

## 5. Catálogo completo (143 clases)

Las tablas van en el orden de `EVENT_TRIGGERS.md`. «Cuándo se dispara» es
el trigger sugerido en ese documento (fichero + momento), no una promesa
de que ya esté cableado: la columna Estado lo aclara. Namespace PM
abreviado: `pocketmine\event\<categoria>` (p. ej. `player` =
`pocketmine\event\player`).

### 5.1 Player — 49 clases (`pocketmine\event\player`)

| # | Evento | Cancellable | Estado | Cuándo se dispara / trigger sugerido |
| --- | --- | --- | --- | --- |
| 1 | `PlayerEvent` | — | Base | Clase base abstracta (`getPlayer()`); no se dispara, es el padre de los `Player*` de abajo. |
| 2 | `PlayerPreLoginEvent` | — | Stub | `session.rs`, handshake pre-auth. `TODO: session.rs:PlayerPreLoginEvent` |
| 3 | `PlayerLoginEvent` | ✅ | Stub | `session.rs`, auth ok. `TODO: session.rs:PlayerLoginEvent` |
| 4 | `PlayerKickEvent` | ✅ | Stub | `session.rs`, kick. `TODO: session.rs:PlayerKickEvent` |
| 5 | `PlayerDuplicateLoginEvent` | ✅ | Stub | `session.rs`, login duplicado. `TODO: session.rs:PlayerDuplicateLoginEvent` |
| 6 | `PlayerCreationEvent` | — | Stub | `session.rs`, creación del jugador. `TODO: session.rs:PlayerCreationEvent` |
| 7 | `PlayerDataSaveEvent` | ✅ | Parcial | `session.rs` / `world_state.rs`, guardado. `TODO: world_state.rs:PlayerDataSaveEvent`. En PHP, `Player::save()` ya lo despacha si la clase existe (`php/player.php`). |
| 8 | `PlayerPostChunkSendEvent` | — | Stub | `session.rs`, envío de chunk. `TODO: session.rs:PlayerPostChunkSendEvent` |
| 9 | `PlayerViewDistanceChangeEvent` | — | Stub | `session.rs`, cambio de distancia de visión. `TODO: session.rs:PlayerViewDistanceChangeEvent` |
| 10 | `PlayerToggleFlightEvent` | ✅ | Stub | `session.rs`, toggles de movimiento. `TODO: session.rs:PlayerToggle*Event` |
| 11 | `PlayerAllowFlightChangeEvent` | ✅ | Stub | `session.rs`, cambio de `allowFlight`. Ver nota de conteo: no está en `EVENT_TRIGGERS.md`, se incluye para cuadrar el 49 de PM upstream. |
| 12 | `PlayerToggleSprintEvent` | ✅ | Stub | `session.rs`, toggles de movimiento. `TODO: session.rs:PlayerToggle*Event` |
| 13 | `PlayerToggleSneakEvent` | ✅ | Stub | `session.rs`, toggles de movimiento. `TODO: session.rs:PlayerToggle*Event` |
| 14 | `PlayerToggleSwimEvent` | ✅ | Stub | `session.rs`, toggles de movimiento. `TODO: session.rs:PlayerToggle*Event` |
| 15 | `PlayerToggleGlideEvent` | ✅ | Stub | `session.rs`, toggles de movimiento. `TODO: session.rs:PlayerToggle*Event` |
| 16 | `PlayerJumpEvent` | — | Stub | `session.rs`, salto. `TODO: session.rs:PlayerJumpEvent` |
| 17 | `PlayerGameModeChangeEvent` | ✅ | Stub | `session.rs` / `command.rs`, cambio de gamemode. `TODO: command.rs:PlayerGameModeChangeEvent` |
| 18 | `PlayerTransferEvent` | ✅ | Stub | `session.rs`, transferencia a otro servidor. `TODO: session.rs:PlayerTransferEvent` |
| 19 | `PlayerItemUseEvent` | ✅ | Stub | `session.rs`, uso de item. `TODO: session.rs:PlayerItem*Event` |
| 20 | `PlayerItemHeldEvent` | ✅ | Stub | `session.rs`, cambio de item en mano. `TODO: session.rs:PlayerItem*Event` |
| 21 | `PlayerItemConsumeEvent` | ✅ | Stub | `session.rs`, consumo de item. `TODO: session.rs:PlayerItem*Event` |
| 22 | `PlayerItemEnchantEvent` | ✅ | Stub | `session.rs`, encantamiento. `TODO: session.rs:PlayerItem*Event`. Alias PM: `PlayerEnchantItemEvent`. |
| 23 | `PlayerExperienceChangeEvent` | ✅ | Stub | `world_state.rs`, XP. `TODO: world_state.rs:PlayerExperienceChangeEvent` |
| 24 | `PlayerExhaustEvent` | ✅ | Stub | `world_state.rs`, agotamiento (hambre). `TODO: world_state.rs:PlayerExhaustEvent` |
| 25 | `PlayerDropItemEvent` | ✅ | Stub | `session.rs`, soltar item. `TODO: session.rs:PlayerDropItemEvent` |
| 26 | `PlayerBucketFillEvent` | ✅ | Stub | `session.rs`, cubo llenado. `TODO: session.rs:PlayerBucket*Event` |
| 27 | `PlayerBucketEmptyEvent` | ✅ | Stub | `session.rs`, cubo vaciado. `TODO: session.rs:PlayerBucket*Event` |
| 28 | `PlayerBlockPickEvent` | ✅ | Stub | `session.rs`, pick de bloque (creativo). `TODO: session.rs:PlayerBlockPickEvent` |
| 29 | `PlayerEntityInteractEvent` | ✅ | Stub | `session.rs`, interactuar con entidad. Shorthand en `EVENT_TRIGGERS.md`: `EntityInteractEvent`. |
| 30 | `PlayerEntityPickEvent` | ✅ | Stub | `session.rs`, pick de entidad. Shorthand en `EVENT_TRIGGERS.md`: `EntityPickEvent`. |
| 31 | `PlayerEditBookEvent` | ✅ | Stub | `session.rs`, edición de libro. `TODO: session.rs:PlayerEditBookEvent` |
| 32 | `PlayerEmoteEvent` | ✅ | Stub | `session.rs`, emote (`/me`). `TODO: session.rs:PlayerEditBookEvent` |
| 33 | `PlayerBedEnterEvent` | ✅ | Stub | `session.rs`, entrar en cama. `TODO: session.rs:PlayerBed*Event` |
| 34 | `PlayerBedLeaveEvent` | — | Stub | `session.rs`, salir de cama. `TODO: session.rs:PlayerBed*Event`. Shorthand `✅/—` en `EVENT_TRIGGERS.md`. |
| 35 | `PlayerChangeSkinEvent` | ✅ | Stub | `session.rs`, cambio de skin. `TODO: session.rs:PlayerChangeSkinEvent` |
| 36 | `PlayerDisplayNameChangeEvent` | — | Stub | `session.rs`, cambio de nombre visible. Shorthand `✅/—` en `EVENT_TRIGGERS.md`. |
| 37 | `PlayerEnchantingOptionsRequestEvent` | ✅ | Stub | `session.rs`, petición de opciones de encantamiento. `TODO: session.rs:PlayerEnchantingOptionsRequestEvent` |
| 38 | `PlayerMissSwingEvent` | ✅ | Stub | `session.rs`, swing sin objetivo. `TODO: session.rs:PlayerEnchantingOptionsRequestEvent` |
| 39 | `PlayerRespawnAnchorUseEvent` | ✅ | Stub | `session.rs`, uso de ancla de respawn. `TODO: session.rs:PlayerEnchantingOptionsRequestEvent` |
| 40 | `PlayerIdentityVerifyEvent` | ✅ | Stub | `session.rs`, verificación de identidad. `TODO: session.rs:PlayerIdentityVerifyEvent` |
| 41 | `PlayerResourcePackOfferEvent` | — | Stub | `session.rs`, oferta de resource pack. Shorthand `✅/—` en `EVENT_TRIGGERS.md`. |
| 42 | `PlayerJoinEvent` | — | Cableado | `session.rs`, entrada en el mundo (una sola vez por sesión). Mutable: `join_message`. |
| 43 | `PlayerQuitEvent` | — | Cableado | `session.rs`, salida. Mutable: `quit_message`. `reason` informativa. |
| 44 | `PlayerChatEvent` | ✅ | Cableado | `session.rs`, chat normal. Ver sección 3 (`ChatFormatter`, `recipients`, `format`). |
| 45 | `PlayerCommandPreprocessEvent` | ✅ | Cableado | `session.rs`, mensaje que empieza por `/`. Mutable: `message`. Gated `has_listeners`. |
| 46 | `PlayerMoveEvent` | ✅ | Cableado | `session.rs`, movimiento (throttle cada 2 movimientos). `from`/`to` son `Location` (pies + yaw/pitch/world, como `getLocation()`); mutable: `to` (`setTo` teleporta allí). Gated `has_listeners`. Cancelar impide el movimiento. |
| 47 | `PlayerInteractEvent` | ✅ | Cableado | `session.rs`, `START_BREAK` en `PlayerAuthInput`. Constantes `ACTION_LEFT_CLICK` (0) y `ACTION_RIGHT_CLICK` (1). |
| 48 | `PlayerDeathEvent` | — | Cableado | `PlayerSession::kill_player` (`session.rs`). Mutable: `death_message`. Gated `has_listeners`; el broadcast solo ocurre si hay listeners. |
| 49 | `PlayerRespawnEvent` | ✅ | Cableado | `PlayerSession::handle_respawn` (`session.rs`). Mutable: `position`. Gated `has_listeners`. Divergencia: el bridge lo marca `Cancellable` (PM upstream no lo es); cancelar deja al jugador muerto. |

### 5.2 Block — 25 clases (`pocketmine\event\block`)

| # | Evento | Cancellable | Estado | Cuándo se dispara / trigger sugerido |
| --- | --- | --- | --- | --- |
| 1 | `BlockEvent` | — | Base | Clase base abstracta de `php/event.php` (`Player|Block`, coords, `getBlock()`, `getPosition()`); no se dispara. |
| 2 | `BlockBreakEvent` | ✅ | Cableado | `session.rs`, acción `PREDICT_DESTROY_BLOCK` en `PlayerAuthInput`. Cancelar impide romper. |
| 3 | `BlockPlaceEvent` | ✅ | Parcial | Colocación de bloque. `EVENT_TRIGGERS.md` lo marca NO cableado (requiere `InventoryTransactionPacket`); en `session.rs` ya hay disparo gated `has_listeners` en el flujo de colocación. Trátelo como parcial hasta verificar el paquete. |
| 4 | `AnvilDamageEvent` | ✅ | Cableado | `session.rs`, daño de yunque al romper (roll de daño estilo PM `Anvil::onBreak`). Campos Rust: `old_damage`, `new_damage`, `broken`. Cancelar deja el yunque intacto. Solo existe en Rust (`src/events/block.rs`), sin clase PHP en `event.php`. |
| 5 | `BlockBurnEvent` | ✅ | Stub | `world_state.rs`, fuego/tick. `TODO: world_state.rs:BlockBurnEvent`. Cancelable vía base. |
| 6 | `BlockMeltEvent` | ✅ | Stub | `world_state.rs`, fusión por calor. `TODO: world_state.rs:BlockBurnEvent` |
| 7 | `BlockSpreadEvent` | ✅ | Stub | `world_state.rs`, propagación (fuego, etc.). `TODO: world_state.rs:BlockBurnEvent` |
| 8 | `BlockGrowEvent` | ✅ | Stub | `world_state.rs`, crecimiento (cultivos, etc.). `TODO: world_state.rs:BlockBurnEvent` |
| 9 | `BlockFormEvent` | ✅ | Stub | `world_state.rs`, formación de bloque (agua, nieve, etc.). `TODO: world_state.rs:BlockBurnEvent` |
| 10 | `BlockDeathEvent` | ✅ | Stub | `world_state.rs`, muerte de bloque. `TODO: world_state.rs:BlockBurnEvent` |
| 11 | `BlockExplodeEvent` | ✅ | Stub | `world_state.rs`, explosión afectando bloque. `TODO: world_state.rs:BlockExplodeEvent` |
| 12 | `BlockPreExplodeEvent` | ✅ | Stub | `world_state.rs`, pre-explosión. `TODO: world_state.rs:BlockExplodeEvent` |
| 13 | `BlockTeleportEvent` | ✅ | Stub | `world_state.rs`, teletransporte de bloque. `TODO: world_state.rs:BlockExplodeEvent` |
| 14 | `BlockUpdateEvent` | ✅ | Stub | `world_state.rs`, actualización de bloque. `TODO: world_state.rs:BlockExplodeEvent` |
| 15 | `BlockItemPickupEvent` | ✅ | Stub | `world_state.rs`, recogida de item por bloque (tolva, etc.). `TODO: world_state.rs:BlockItemPickupEvent` |
| 16 | `BrewingFuelUseEvent` | ✅ | Stub | `world_state.rs`, uso de combustible de cervecería. `TODO: world_state.rs:BrewingFuelUseEvent` |
| 17 | `BrewItemEvent` | ✅ | Stub | `world_state.rs`, cervecería. `TODO: world_state.rs:BrewingFuelUseEvent` |
| 18 | `CampfireCookEvent` | ✅ | Stub | `world_state.rs`, cocina de hoguera. `TODO: world_state.rs:BrewingFuelUseEvent` |
| 19 | `ChestPairEvent` | ✅ | Stub | `world_state.rs`, par de cofres. `TODO: world_state.rs:StructureGrowEvent` |
| 20 | `FarmlandHydrationChangeEvent` | ✅ | Stub | `world_state.rs`, hidratación de tierra de cultivo. `TODO: world_state.rs:StructureGrowEvent` |
| 21 | `GeyserEruptionPulseEvent` | ✅ | Stub | `world_state.rs`, pulso de géiser. `TODO: world_state.rs:StructureGrowEvent` |
| 22 | `LeavesDecayEvent` | ✅ | Stub | `world_state.rs`, caída de hojas. `TODO: world_state.rs:StructureGrowEvent` |
| 23 | `StructureGrowEvent` | ✅ | Stub | `world_state.rs`, crecimiento de estructura. `TODO: world_state.rs:StructureGrowEvent` |
| 24 | `SignChangeEvent` | ✅ | Stub | `session.rs` / `world_state.rs`, cambio de cartel. `TODO: world_state.rs:SignChangeEvent` |
| 25 | `PressurePlateUpdateEvent` | ✅ | Stub | `world_state.rs`, placa de presión. `TODO: world_state.rs:PressurePlateUpdateEvent`. Cancelable vía base. |

### 5.3 Entity — 33 clases (`pocketmine\event\entity`)

> Divergencia del bridge (`EVENT_TRIGGERS.md`): no hay clase `Entity` en el
> bridge; `EntityEvent` guarda `entity_id` / `type` / `name` + `world`/`x`/`y`/`z`
> como arrays y expone getters genéricos vía `__call` + `extra`. Los payloads
> Rust de hoy son `String` / `Value` genéricos hasta que el core tenga tipos
> reales (`Entity`, `Block`, `Position`, `Item`).

| # | Evento | Cancellable | Estado | Cuándo se dispara / trigger sugerido |
| --- | --- | --- | --- | --- |
| 1 | `EntityEvent` | — | Base | Clase base de entidad (ver divergencia arriba); no se dispara. |
| 2 | `EntityDamageEvent` | ✅ | Stub | `world_state.rs`, daño. `TODO: world_state.rs:EntityDamageEvent` |
| 3 | `EntityDamageByBlockEvent` | ✅ | Stub | `world_state.rs`, daño por bloque. `TODO: world_state.rs:EntityDamageEvent` |
| 4 | `EntityDamageByEntityEvent` | ✅ | Stub | `world_state.rs`, daño por entidad. `TODO: world_state.rs:EntityDamageEvent` |
| 5 | `EntityDamageByChildEvent` | ✅ | Stub | `world_state.rs`, daño por entidad hija (proyectil, etc.). `TODO: world_state.rs:EntityDamageEvent` |
| 6 | `EntityCombustEvent` | ✅ | Stub | `world_state.rs`, combustión. `TODO: world_state.rs:EntityCombustEvent` |
| 7 | `EntityCombustByBlockEvent` | ✅ | Stub | `world_state.rs`, combustión por bloque. `TODO: world_state.rs:EntityCombustEvent` |
| 8 | `EntityCombustByEntityEvent` | ✅ | Stub | `world_state.rs`, combustión por entidad. `TODO: world_state.rs:EntityCombustEvent` |
| 9 | `EntityEffectEvent` | ✅ | Stub | `world_state.rs`, efecto. `TODO: world_state.rs:EntityEffectEvent` |
| 10 | `EntityEffectAddEvent` | ✅ | Stub | `world_state.rs`, añadir efecto. `TODO: world_state.rs:EntityEffectEvent`. Alias PM: `EntityAddEffectEvent`. |
| 11 | `EntityEffectRemoveEvent` | ✅ | Stub | `world_state.rs`, quitar efecto. `TODO: world_state.rs:EntityEffectEvent`. Alias PM: `EntityRemoveEffectEvent`. |
| 12 | `EntityDeathEvent` | — | Stub | `world_state.rs`, muerte de entidad. `TODO: world_state.rs:EntitySpawnEvent` |
| 13 | `EntityDespawnEvent` | — | Stub | `world_state.rs`, despawn. `TODO: world_state.rs:EntitySpawnEvent` |
| 14 | `EntitySpawnEvent` | — | Stub | `world_state.rs`, spawn. `TODO: world_state.rs:EntitySpawnEvent` |
| 15 | `EntityExplodeEvent` | ✅ | Stub | `world_state.rs`, explosión de entidad. `TODO: world_state.rs:EntityMotionEvent` |
| 16 | `EntityPreExplodeEvent` | ✅ | Stub | `world_state.rs`, pre-explosión de entidad. `TODO: world_state.rs:EntityMotionEvent` |
| 17 | `EntityFrostWalkerEvent` | ✅ | Stub | `world_state.rs`, frost walker. `TODO: world_state.rs:EntityMotionEvent` |
| 18 | `EntityItemPickupEvent` | ✅ | Stub | `world_state.rs`, recogida de item por entidad. `TODO: world_state.rs:EntityMotionEvent` |
| 19 | `EntityMotionEvent` | ✅ | Stub | `world_state.rs`, movimiento de entidad. `TODO: world_state.rs:EntityMotionEvent` |
| 20 | `EntityRegainHealthEvent` | ✅ | Stub | `world_state.rs`, recuperación de salud. `TODO: world_state.rs:EntityMotionEvent` |
| 21 | `EntityShootBowEvent` | ✅ | Stub | `world_state.rs`, disparo de arco. `TODO: world_state.rs:EntityMotionEvent` |
| 22 | `EntityTeleportEvent` | ✅ | Stub | `world_state.rs`, teletransporte de entidad. `TODO: world_state.rs:EntityMotionEvent` |
| 23 | `EntityTrampleEvent` | ✅ | Stub | `world_state.rs`, pisoteo (tierra de cultivo). `TODO: world_state.rs:EntityMotionEvent` |
| 24 | `EntityBlockChangeEvent` | ✅ | Stub | `world_state.rs`, cambio de bloque por entidad. `TODO: world_state.rs:EntityMotionEvent` |
| 25 | `EntityAreaEffectCloudApplyEvent` | ✅ | Stub | `world_state.rs`, nube de efecto de área. `TODO: world_state.rs:EntityMotionEvent` |
| 26 | `EntityExtinguishEvent` | — | Stub | `world_state.rs`, extinción. `TODO: world_state.rs:ProjectileLaunchEvent` |
| 27 | `ItemSpawnEvent` | — | Stub | `world_state.rs`, spawn de item. `TODO: world_state.rs:ProjectileLaunchEvent` |
| 28 | `ProjectileHitEvent` | — | Stub | `world_state.rs`, impacto de proyectil. `TODO: world_state.rs:ProjectileLaunchEvent` |
| 29 | `ProjectileHitBlockEvent` | — | Stub | `world_state.rs`, impacto en bloque. `TODO: world_state.rs:ProjectileLaunchEvent` |
| 30 | `ProjectileHitEntityEvent` | — | Stub | `world_state.rs`, impacto en entidad. `TODO: world_state.rs:ProjectileLaunchEvent` |
| 31 | `ItemDespawnEvent` | ✅ | Stub | `world_state.rs`, despawn de item. `TODO: world_state.rs:ProjectileLaunchEvent` |
| 32 | `ItemMergeEvent` | ✅ | Stub | `world_state.rs`, fusión de items. `TODO: world_state.rs:ProjectileLaunchEvent` |
| 33 | `ItemLaunchEvent` | ✅ | Stub | `world_state.rs`, lanzamiento de item / proyectil. `TODO: world_state.rs:ProjectileLaunchEvent` |

### 5.4 Inventory — 7 clases (`pocketmine\event\inventory`)

| # | Evento | Cancellable | Estado | Cuándo se dispara / trigger sugerido |
| --- | --- | --- | --- | --- |
| 1 | `InventoryEvent` | — | Base | Clase base de inventario; no se dispara. |
| 2 | `InventoryTransactionEvent` | ✅ | Stub | `session.rs`, transacción de inventario. `TODO: session.rs:InventoryTransactionEvent` |
| 3 | `InventoryOpenEvent` | ✅ | Stub | `session.rs`, apertura de inventario. `TODO: session.rs:InventoryOpenEvent` |
| 4 | `InventoryCloseEvent` | — | Stub | `session.rs`, cierre de inventario. `TODO: session.rs:InventoryCloseEvent` |
| 5 | `CraftItemEvent` | ✅ | Stub | `session.rs`, crafteo. `TODO: session.rs:CraftItemEvent` |
| 6 | `FurnaceBurnEvent` | ✅ | Stub | `world_state.rs`, horno quemando. `TODO: world_state.rs:FurnaceBurnEvent`. Divergencia: vive en namespace `inventory` (PM) pero con coords de bloque. |
| 7 | `FurnaceSmeltEvent` | ✅ | Stub | `world_state.rs`, horno fundiendo. `TODO: world_state.rs:FurnaceBurnEvent` |

### 5.5 World / Chunk — 14 clases (`pocketmine\event\world`)

Todos en `world_state.rs` según `EVENT_TRIGGERS.md`
(`TODO: world_state.rs:WorldLoadEvent`, `TODO: world_state.rs:ChunkLoadEvent`, etc.).
Cancelables: `ChunkUnloadEvent`, `WorldParticleEvent`, `WorldSoundEvent`,
`WorldUnloadEvent`.

| # | Evento | Cancellable | Estado | Cuándo se dispara / trigger sugerido |
| --- | --- | --- | --- | --- |
| 1 | `WorldEvent` | — | Base | Clase base de mundo; no se dispara. |
| 2 | `ChunkEvent` | — | Base | Clase base de chunk; no se dispara. |
| 3 | `WorldLoadEvent` | — | Stub | `world_state.rs`, carga de mundo. `TODO: world_state.rs:WorldLoadEvent` |
| 4 | `WorldInitEvent` | — | Stub | `world_state.rs`, init de mundo. `TODO: world_state.rs:WorldLoadEvent` |
| 5 | `WorldSaveEvent` | — | Stub | `world_state.rs`, guardado de mundo. `TODO: world_state.rs:WorldLoadEvent` |
| 6 | `WorldUnloadEvent` | ✅ | Stub | `world_state.rs`, descarga de mundo. `TODO: world_state.rs:WorldLoadEvent` |
| 7 | `ChunkLoadEvent` | — | Stub | `world_state.rs`, carga de chunk. `TODO: world_state.rs:ChunkLoadEvent` |
| 8 | `ChunkPopulateEvent` | — | Stub | `world_state.rs`, población de chunk. `TODO: world_state.rs:ChunkLoadEvent` |
| 9 | `ChunkUnloadEvent` | ✅ | Stub | `world_state.rs`, descarga de chunk. `TODO: world_state.rs:ChunkLoadEvent` |
| 10 | `SpawnChangeEvent` | — | Stub | `world_state.rs`, cambio de spawn. `TODO: world_state.rs:WorldLoadEvent` |
| 11 | `WorldDifficultyChangeEvent` | — | Stub | `world_state.rs`, cambio de dificultad. `TODO: world_state.rs:WorldLoadEvent` |
| 12 | `WorldDisplayNameChangeEvent` | — | Stub | `world_state.rs`, cambio de nombre visible del mundo. `TODO: world_state.rs:WorldLoadEvent` |
| 13 | `WorldParticleEvent` | ✅ | Stub | `world_state.rs`, partícula de mundo. `TODO: world_state.rs:WorldLoadEvent` |
| 14 | `WorldSoundEvent` | ✅ | Stub | `world_state.rs`, sonido de mundo. `TODO: world_state.rs:WorldLoadEvent` |

### 5.6 Server — 12 clases (`pocketmine\event\server`)

| # | Evento | Cancellable | Estado | Cuándo se dispara / trigger sugerido |
| --- | --- | --- | --- | --- |
| 1 | `ServerEvent` | — | Base | Clase base de servidor; no se dispara. |
| 2 | `DataPacketDecodeEvent` | ✅ | Stub | `session.rs`, decodificación de paquete. `TODO: session.rs:DataPacketReceiveEvent` |
| 3 | `DataPacketReceiveEvent` | ✅ | Stub | `session.rs`, recepción de paquete. `TODO: session.rs:DataPacketReceiveEvent` |
| 4 | `DataPacketSendEvent` | ✅ | Stub | `session.rs`, envío de paquete. `TODO: session.rs:DataPacketReceiveEvent` |
| 5 | `LowMemoryEvent` | — | Stub | `server-core`, memoria baja. `TODO: server-core:LowMemoryEvent` |
| 6 | `QueryRegenerateEvent` | — | Stub | `server-core`, regeneración de query. `TODO: server-core:LowMemoryEvent` |
| 7 | `UpdateNotifyEvent` | — | Stub | `server-core`, aviso de actualización. `TODO: server-core:LowMemoryEvent` |
| 8 | `NetworkInterfaceEvent` | — | Stub | `session.rs`, interfaz de red. `TODO: session.rs:NetworkInterfaceRegisterEvent` |
| 9 | `NetworkInterfaceRegisterEvent` | — | Stub | `session.rs`, registro de interfaz. `TODO: session.rs:NetworkInterfaceRegisterEvent`. `EVENT_TRIGGERS.md` no marca cancelable. |
| 10 | `NetworkInterfaceUnregisterEvent` | — | Stub | `session.rs`, desregistro de interfaz. `TODO: session.rs:NetworkInterfaceRegisterEvent`. `EVENT_TRIGGERS.md` no marca cancelable. |
| 11 | `CommandEvent` | ✅ | Parcial | `command.rs`, comando genérico. Clase PHP en `event.php` (`CommandEvent extends Event implements Cancellable`, `getSender()` / `getCommand()`); en Rust solo existe `ServerCommandEvent`. |
| 12 | `ServerCommandEvent` | ✅ | Cableado | `command.rs`, comando de consola. Rust: `sender` (defecto `CONSOLE`) + `command`, cancelable. PHP: `ServerCommandEvent extends CommandEvent`. |

### 5.7 Plugin — 3 clases (`pocketmine\event\plugin`)

Ver sección 4 para el detalle. Ninguno es cancelable y ninguno cruza el IPC.

| # | Evento | Cancellable | Estado | Cuándo se dispara / trigger sugerido |
| --- | --- | --- | --- | --- |
| 1 | `PluginEvent` | — | Base | Clase base abstracta (`php/plugin.php`, `getPlugin()`); no se dispara. `fromArray()` lanza `BadMethodCallException`. |
| 2 | `PluginEnableEvent` | — | Cableado (PHP) | `PluginManager::enablePlugin()` en `php/plugin.php`, después de la contabilidad de habilitado y antes del auto-registro de listeners. `TODO: plugin_host.rs:PluginEnableEvent` en Rust (sin trigger). |
| 3 | `PluginDisableEvent` | — | Cableado (PHP) | `PluginManager::disablePlugin()` en `php/plugin.php`, antes de `onDisable()`; el desregistro de listeners ocurre después. |

## 6. Divergencias bridge vs PM upstream

De `EVENT_TRIGGERS.md`, sección «Divergencias documentadas»:

- `BaseBlockChangeEvent` y los `Block*` nuevos extienden `Event` con coords
  en vez de `BlockEvent(Player|Block)`, para no requerir un `Player` ficticio
  en eventos de mundo. Getters `getWorld()` / `getX()` / `getY()` / `getZ()` /
  `getBlockId()` compatibles. `BlockBreak` / `BlockPlace` originales sí
  extienden `BlockEvent` (sin cambios).
- `PlayerDataSave` / `Creation` / `DuplicateLogin` / `IdentityVerify` /
  `ResourcePackOffer` / `PreLogin` extienden `Event` (no `PlayerEvent`)
  igual que PM; el resto de player extiende `PlayerEvent`.
- `PlayerExperienceChange` / `Exhaust` extienden `PlayerEvent` en el bridge
  (PM usa `EntityEvent<Human>`); se expone `getPlayer()` + payload genérico.
- `FurnaceBurn` / `SmeltEvent` viven en namespace `inventory` (PM) pero con
  coords de bloque.
- `EntityEvent` guarda `entity_id` / `type` / `name` + `world`/`x`/`y`/`z`
  como arrays (no hay clase `Entity` en el bridge). Getters genéricos vía
  `__call` + `extra`.
- `PlayerRespawnEvent` es `Cancellable` en el bridge (PM upstream no lo es):
  cancelar deja al jugador muerto (anexo de integración de
  `EVENT_TRIGGERS.md`). `PlayerDeathEvent` en el bridge lleva payload
  simplificado `player` + `death_message` (`"{name} died"`).
- `BlockPlaceEvent`: `EVENT_TRIGGERS.md` dice que requiere decodificar
  `InventoryTransactionPacket` y lo lista como pendiente, pero `session.rs`
  ya lo dispara en el flujo de colocación cuando hay listeners. Trátelo
  como parcial.
- `@ignoreCancelled` invertido respecto a PM (sección 1.3): en este bridge
  `true` significa «ejecútame también si está cancelado».

## 7. Qué queda por cablear

De `EVENT_TRIGGERS.md`, sección «Qué queda»:

1. Cablear triggers en `session.rs` / `command.rs` / `world_state.rs`
   (equipo de visibilidad), usando los `TODO: <fichero>:<Evento>` de las
   tablas de la sección 5. Pendientes no triviales citados expresamente:
   `PlayerKickEvent`, `PlayerGameModeChangeEvent`, `PlayerToggle*Event`,
   `PlayerDataSaveEvent` (ya parcial en PHP), `PlayerLoginEvent`,
   `BlockPlaceEvent` (decodificar `InventoryTransactionPacket`), entidades,
   inventario, mundo/chunk y eventos de red.
2. Enriquecer payloads Rust con tipos reales (`Entity`, `Block`,
   `Position`, `Item`) cuando el core los tenga; hoy son `String`/`Value`
   genéricos + `extra`.
3. Tests de plugins reales contra cada evento (roundtrip PHP ya verificado
   según `EVENT_TRIGGERS.md`).
4. `scheduler.php` / `plugin.php`: sin cambios (ya completos).

## 8. Verificación y comandos

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
php -l crates/php-plugin-bridge/php/event.php
php -l crates/php-plugin-bridge/php/plugin.php
php -l crates/php-plugin-bridge/php/chat.php
cargo check -p php-plugin-bridge
cargo test -p php-plugin-bridge
```

El test `test_event_dispatcher_priorities_and_cancellation` de
`crates/php-plugin-bridge/src/lib.rs` verifica el contrato de la sección
1.3 con `PlayerChatEvent`: listener `LOWEST` que cancela, listener
`NORMAL` con `ignore_cancelled: false` que **no** debe ejecutarse, y
listener `MONITOR` que **sí** debe ejecutarse con el evento cancelado.
Los tests `test_player_chat_apply_json` y
`placeholder_formatter_substitutes_player_and_message` verifican la
sección 3.

---

**Siguiente paso:** vuelve al [02 — Quickstart](02-quickstart.md) y añade
un listener `PlayerChatEvent` a tu plugin `Demo`, o sigue el hub en
[index.md](index.md).
