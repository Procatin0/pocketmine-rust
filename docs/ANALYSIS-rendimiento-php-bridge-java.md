# Análisis de rendimiento — PHP plugin bridge y adaptador Java

Fecha: 2026-11-17
Alcance: `crates/php-plugin-bridge` (lado Rust + runtime PHP embebido en `php/`) y
`crates/java-bridge` junto con sus hooks reales en `crates/server-core/src/java_hooks.rs`.

Cada hallazgo indica evidencia (`archivo:línea`), el cambio propuesto y el riesgo.

## Estado de implementación (2026-11-17)

Implementado y verificado con `cargo test -p java-bridge` (43 unit + 4 de integración),
`cargo test -p php-plugin-bridge` (38) y `cargo check -p server-core --lib --tests`:

- **P-1** parseo único de respuestas IPC (`runtime/php.rs`), **P-4** buffer de escritura
  reutilizado y un solo `write_all`, **P-2** tick sin plugins PHP, **P-3** gate por
  `has_listeners` en `dispatch_event_sync` + refresco de la unión de listeners en cada
  `tick` (`ipc.php` devuelve `listener_events`).
- **J-1** `send`/`send_*_chunk` escriben el frame directo en `out_buf` (+`wire::varint_len`),
  **J-2** caché de chunks como `Arc` (`try_*_chunk_shared`), **J-3** dedup de snapshots
  una vez por tick (`dedup_snapshots`), **J-4** TAB normalizada una vez por tick
  (`sync_tab_normalized`), **J-5** `take_out_into` conserva la capacidad, **J-6**
  `BytesMut::advance` en `poll_bytes` (queda la copia del payload en `try_split_frame`),
  **J-7** un solo lock de mapa por tick y `playing` calculado una vez, **J-9** cabecera de
  chunk directa a `out_buf`, **J-10** log de paquetes a `debug`, pulso de flush compartido
  por servidor y estado de bloque por protocolo.

No implementado (deliberadamente, ver §6): **J-8** (locks por sesión), **P-5** (worker
fire-and-forget) y el `lower()` por target de `sync_entities` (micro-coste restante, ya sin
los clones O(P²) de snapshots).

---

## 0. Modelo de ejecución (por qué estos puntos son los caros)

**PHP bridge** — un único proceso PHP hijo, atendido por un solo hilo Rust
(`php-plugin-worker`) y **serializado**: una petición por vez, `write` a stdin →
`read_line` de stdout. Cualquier trabajo de plugin bloquea la siguiente petición, y
cada evento del servidor que cruza la frontera cuesta:

```
Rust: to_json(evento) → serde_json::to_string → write(stdin)      (1)
PHP : fgets → json_decode → hydrate evento → dispatch a listeners (2)
PHP : collectActions → json_encode(respuesta+evento) → write(stdout)
Rust: read_line → 2× parse JSON → apply_json(evento) → acciones
```

Conviven dos bucles en el tick del servidor: `PluginManager::tick_sync` (20/s) y un
`dispatch_event_sync` por evento disparado. Ambos son **round trips de proceso**.

**Java adapter** — en proceso, sin IPC, pero con un tick propio que copia/recalcula
estado por observador cada tick y con streaming de chunks como carga dominante.

Cargas dominantes medidas por el propio código (logs ya existentes):
`[lag-ipc]` (> 20 ms por evento, `manager.rs:554`), `Java bridge tick … total/sessions/tab/rest`
(`server.rs:390`) y `Tick del puente Java lento` (> 15 ms, `server.rs:405`).

---

## 1. Resumen ejecutivo (orden recomendado)

| # | Hallazgo | Dónde | Impacto | Esfuerzo | Riesgo |
|---|----------|-------|---------|----------|--------|
| J-1 | `send()` construye el frame con 2 `Vec` intermedios (3 copias del payload) | `java-bridge/src/session.rs:2409`, `wire.rs:336` | **Alto** (chunks) | Bajo | Nulo |
| J-2 | La caché de chunks devuelve una **copia completa** por columna y por jugador | `server-core/src/java_hooks.rs:84`, `:760` | **Alto** (carga inicial) | Medio | Bajo |
| J-3 | `entity_targets_for` deduplica y clona **todos** los snapshots por observador (O(P²)) | `java-bridge/src/server.rs:144,455` | Alto con muchos jugadores | Bajo | Bajo |
| J-4 | `sync_tab` normaliza y clona la lista TAB por sesión (O(P²)) | `java-bridge/src/session.rs:1471` | Alto con muchos jugadores | Medio | Bajo |
| P-1 | Respuesta IPC se parsea **dos veces** (a `Value` y luego a `IpcResponse`) | `php-plugin-bridge/src/runtime/php.rs:654` | Medio (×20/s + eventos) | Bajo | Nulo |
| P-2 | `tick_sync` hace round trip 20/s aunque no haya ningún plugin PHP cargado | `manager.rs:659` | Alto en servidores sin plugins PHP | Bajo | Nulo |
| P-3 | `dispatch_event_sync` no consulta `has_listeners` (chat/join/quit siempre cruzan) | `manager.rs:534`, `chat.rs:86`, `java_hooks.rs:424` | Alto (eventos de alta frecuencia) | Bajo | **Medio** (listeners dinámicos) |
| J-5 | `take_out()` tira la capacidad del buffer cada flush (reallocs durante streaming) | `session.rs:280` | Medio | Bajo | Nulo |
| J-6 | `poll_bytes`: `Vec::drain` + `to_vec` por paquete entrante | `session.rs:297`, `wire.rs:509` | Medio (PvP) | Medio | Bajo |
| P-4 | Codificación de petición + 2 `write` + `flush` por mensaje IPC | `php.rs:582,772` | Bajo-medio | Bajo | Nulo |
| J-7 | `tick_inner`: 4× `any_playing()` (lock+scan), `Vec` de ids por tick, N+1 locks | `server.rs:350-470` | Bajo-medio | Bajo | Bajo |
| J-8 | Contención: `poll_bytes` corre bajo el Mutex global de sesiones | `server.rs:660` | Alto bajo carga | Alto | Alto |
| P-5 | Un plugin lento bloquea **todo** el IPC (worker único) | `php.rs:582` | Estratégico | Alto | Alto |

---

## 2. PHP plugin bridge

### P-1. Doble deserialización de cada respuesta IPC — gratis de arreglar

`read_response` (`runtime/php.rs:654`) hace:

```rust
let value: Value = serde_json::from_str(trimmed)?;      // parse 1: árbol DOM completo
match value.get("type")…                                // discriminador host_call
let response: IpcResponse = serde_json::from_value(value)?; // parse 2: recorre el DOM
```

Se construye un `Value` (una `Map` por objeto, un `String`/`Number` por campo) y
después se recorre otra vez para materializar `IpcResponse` (con `Vec<PluginAction>`
anidado). Se paga en **cada** tick (20/s), en cada evento y en cada comando.

Propuesta: discriminar por prefijo y parsear una sola vez. El runtime PHP escribe las
respuestas con `status` como primera clave (`ipc.php:167-201`) y las líneas de control
con `type` como primera clave (`ipc.php:920`):

```rust
let trimmed = line.trim();
if trimmed.as_bytes().starts_with(b"{\"type\"") {
    let value: Value = serde_json::from_str(trimmed)?;   // solo host_call/host_result (raro)
    // … handle_host_call / ignore host_result …
    continue;
}
let response: IpcResponse = serde_json::from_str(trimmed)…?; // parse único
```

Ganancia: una pasada completa de JSON menos por mensaje (y toda la asignación del
DOM). Medible con `metrics::ipc()` (`server-core/src/metrics.rs:176`).
Riesgo: nulo (misma semántica; `type` ausente ya caía en `IpcResponse`).

### P-2. Tick IPC 20/s sin plugins PHP

`PluginManager::tick_sync` (`manager.rs:659`) solo comprueba `is_available()` y hace
`request_sync("tick", …)`. Con cero plugins cargados no hay scheduler PHP que avanzar
ni acciones que recoger, pero se pagan 20 round trips/s (serialización, syscalls,
despertar del proceso PHP y del hilo worker).

```rust
if self.plugins.read().is_empty() {
    return Ok(Vec::new());
}
```

Riesgo: nulo — `actionTick` (`ipc.php:608`) solo mueve el scheduler y `Server::setTick`;
sin plugins nadie los lee, y al cargar el primero el valor se re-ancla desde
`$request['tick']`. Es la mejora con mejor relación impacto/esfuerzo del bridge.

### P-3. `dispatch_event_sync` no respeta `has_listeners`

`dispatch_event_sync` (`manager.rs:534`) solo comprueba `is_shutting_down()` e
`is_available()`. Varios emisores despachan siempre, sin consultar el conjunto de
listeners: `PlayerChatEvent` (`server-core/src/chat.rs:86`), `PlayerJoinEvent` y
`PlayerQuitEvent` (`java_hooks.rs:424`, `:442`), `ServerCommandEvent` (`main.rs:718`).
Consecuencia: cada mensaje de chat y cada conexión pagan un round trip completo a PHP
aunque ningún plugin escuche ese evento.

Propuesta: gate central en `dispatch_event_sync`:

```rust
if !self.php_worker.is_shutting_down()
    && self.php_worker.is_available()
    && self.has_listeners(event.name())
```

El resto de emisores ya usan este patrón (~15 sitios en `server-core/src/session.rs`),
así que el modelo "listener declarado en `onEnable`" ya es el contrato del proyecto.
**Riesgo medio**: un plugin que registre listeners *después* de `onEnable` (desde una
tarea o comando) no aparecería en `listener_events` hasta el siguiente
`enable_plugin`/`disable_plugin`. Mitigaciones: (a) refrescar
`listener_events_by_plugin` también en cada `tick` (coste: campos ya presentes en la
respuesta `tick`), o (b) gatear solo los eventos de alta frecuencia (chat, move) y
dejar join/quit/command sin gate.

### P-4. Coste por mensaje en la capa de transporte

`run_worker` (`php.rs:582`) construye un `Map` nuevo, inserta `action` como
`Value::String` y serializa a `String`; `write_json_line` (`php.rs:772`) hace
`write_all(line)`, `write_all(b"\n")` y `flush` → **2 syscalls** por mensaje (el
`flush` de `ChildStdin` no bufferea, es un no-op). Con ~20 ticks/s + eventos + host
calls, es tráfico suficiente para notarlo en un `perf`/ETW.

Propuesta: buffer reutilizable por worker y una sola escritura:

```rust
fn write_json_line(stdin: &mut ChildStdin, line: &str) -> Result<(), PluginError> {
    let mut buf = Vec::with_capacity(line.len() + 1);
    buf.extend_from_slice(line.as_bytes());
    buf.push(b'\n');
    stdin.write_all(&buf)?;   // 1 syscall
    Ok(())
}
```

Y en `run_worker`, serializar con `serde_json::to_writer(&mut buf, &payload)` sobre un
`Vec<u8>` que se reutiliza entre iteraciones (`clear()`), evitando el `String` por
petición. Ganancia pequeña pero constante; riesgo nulo.

### P-5. Un plugin lento bloquea todo el bridge (estructural)

El worker es único y estrictamente serial (`receiver.recv()` → `read_response`). Un
`onEnable` costoso, un `hostCall` a un mundo cargado o un `sleep` de plugin retrasa
*todos* los eventos siguientes, y el `REQUEST_TIMEOUT` de 30 s (`php.rs:23`) provoca
`mark_unavailable()` (pierde el runtime entero). El `[lag-ipc]` de `manager.rs:554`
solo lo hace visible.

No es un bug, es una decisión de diseño con coste: conviene documentarlo y, si se
quiere escalar, mover los eventos **no cancelables y de solo lectura** (los que solo
producen acciones tipo `ConsoleMessage`) a una cola fire-and-forget con su propio
worker, dejando el worker síncrono solo para los cancelables. Alto esfuerzo; no tocar
sin necesidad.

### P-6. Detalles menores del lado PHP

- `Runtime::write` (`ipc.php:836`) usa `JSON_INVALID_UTF8_SUBSTITUTE`: obliga a
  validar UTF-8 de todo el payload en cada respuesta. Necesario para no perder
  respuestas con texto inválido; **no quitar** sin medir, pero es coste por respuesta.
- `handleLine` (`ipc.php:167`) llama `collectActions()` en **todas** las acciones,
  incluido `ping`/`tick`: un `Server::getInstance()` + `method_exists` por petición.
  Se puede cachear `method_exists($server,'takeActions')` una vez en `initialize()`.
- `Bridge::hostCall` (`ipc.php:920`) usa `microtime(true)` en cada iteración del bucle
  de lectura y un deadline fijo de 10 s. El `hostCall` es la operación más cara del
  lado PHP (2 JSON + 2 syscalls + bloqueo). La superficie real es pequeña (35 sitios:
  22 en `world.php`, 8 en `core.php`, 4 en `player.php`, 1 en `ipc.php`), así que el
  riesgo de un `getHealth()` por tick no existe; el punto a vigilar es `world.tick`
  (`world.php:993`) si algún plugin llama `World::tick()` en bucle.
- `PluginManager::dispatchEvent` (`plugin.php:2061`) construye `class_parents($event)`
  y un array de listeners por evento. Con eventos de alta frecuencia y muchos
  listeners, cachear `[$event::class => [listeners]]` por generación de registro
  ahorra reflexión por dispatch. Impacto medio solo si hay plugins con listeners
  frecuentes.

---

## 3. Adaptador de conexiones Java

### J-1. `send()`: 3 copias y 2 asignaciones por paquete

```rust
// session.rs:2409
fn send(&mut self, packet_id: i32, payload: &[u8]) {
    self.out_buf.extend_from_slice(&wire::frame(packet_id, payload));
}
// wire.rs:336
pub fn frame(id: i32, payload: &[u8]) -> Vec<u8> {
    let mut body = varint(id);              // alloc 1
    body.extend_from_slice(payload);        // copia 1 del payload
    let mut out = varint(body.len() as i32); // alloc 2
    out.extend_from_slice(&body);           // copia 2
    out
}
```

`out_buf.extend_from_slice(&frame)` añade la **copia 3**. Para un chunk (~50-150 KB)
son dos buffers temporales y tres recorridos de memoria por columna y por jugador.
Es el punto caliente real del streaming.

```rust
fn send(&mut self, packet_id: i32, payload: &[u8]) {
    if self.closed { return; }
    let body_len = wire::varint_len(packet_id) + payload.len();
    wire::write_varint(&mut self.out_buf, body_len as i32);
    wire::write_varint(&mut self.out_buf, packet_id);
    self.out_buf.extend_from_slice(payload);   // 1 sola copia
}
```

Requiere `pub fn varint_len(value: i32) -> usize` en `wire.rs` (1-5). `wire::frame`
se mantiene para tests. Los tests de `wire_test.rs`/`modern_test.rs` verifican bytes
exactos, así que la equivalencia queda cubierta sin tests nuevos.

### J-2. La caché de chunks clona la columna entera por jugador

`PayloadCache::get` (`java_hooks.rs:84`) devuelve `Option<V>` con `cloned()`, y
`try_modern_chunk`/`modern_chunk` (`java_hooks.rs:748`, `:760`) lo propagan. Después:

- `build_chunk_cached_only` (`session.rs:1776`) mueve `data`/`heightmaps`/`light` a
  `BuiltChunk` (sin coste, es `move`),
- `stream_chunks` (`session.rs:1667`) copia `built.data` al `payload` (J-9),
- `send` lo vuelve a copiar (J-1).

Total: **3 copias** de cada columna, y la primera (`cloned()` en la caché) es la que
se paga una vez **por jugador** durante la carga inicial (p. ej. 20 jugadores × ~500
columnas × ~80 KB ≈ 800 MB de memcpy). Además, cada `cloned()` es un pico de
asignación que presiona el allocator en el hilo del tick.

Propuesta: `Arc` en la caché (`PayloadCache<(…), Arc<ModernChunkData>>`, idem legacy) y
en `ServerHooks` (`Option<Arc<ModernChunkData>>`). `BuiltChunk` guarda el `Arc` y
`send` toma `&[u8]` por `Deref`; el fan-out a N jugadores pasa a ser solo la copia
final en `out_buf`. Si se quiere evitar tocar la trait pública, un paso intermedio es
añadir `try_modern_chunk_shared(protocol, cx, cz) -> Option<Arc<…>>` con default que
envuelva la versión por valor; `CoreHooks` implementa la versión `Arc`.

Riesgo: bajo (cambio de tipo en un trait con 1 implementación real + stubs de test).

### J-3. `entity_targets_for`: O(P²) clones de String por tick

`tick_entities` (`server.rs:444`) llama `entity_targets_for` **por sesión**, y dentro
(`server.rs:144`):

```rust
let mut by_name: HashMap<String, PlayerSnapshot> = HashMap::new();
for snapshot in snapshots { … by_name.insert(key, snapshot.clone()); }  // clona TODO
```

Es decir, para cada observador se reconstruye el mapa completo y se clonan nombre,
posición, flags… Con P jugadores: P² clones de `PlayerSnapshot` por tick (20 Hz). Con
40 jugadores son 1 600 clones/tick = 32 000/s; con 100, 200 000/s.

Propuesta: separar la deduplicación del filtro por observador:

```rust
pub fn dedup_snapshots(snapshots: &[PlayerSnapshot]) -> Vec<PlayerSnapshot> { … }
// una vez por tick en `tick_entities`
let snapshots = dedup_snapshots(&self.hooks.snapshots());
for id in ids { … entity_targets_for(&snapshots, session) … }
```

`entity_targets_for` deja de deduplicar (precondición documentada: nombres únicos) y
solo salta el propio jugador, aplica el chunk gate y construye `EntityTarget`. Los
tests existentes usan nombres únicos, así que el comportamiento observable no cambia.

Complemento (mismo tick): `sync_entities` (`session.rs:1827`) hace
`lower(&t.name)` por target y por sesión → otra P² de `String` en minúsculas. Con una
clave ya normalizada en `EntityTarget` (calculada una vez por tick) desaparece.

### J-4. `sync_tab`: O(P²) por tick

`tick_tab` construye `tab_targets` una vez por tick (bien), pero `sync_tab`
(`session.rs:1471`) vuelve a normalizar **por sesión**:

```rust
let mut normalized: HashMap<String, TabEntry> = HashMap::new();
for target in targets { normalized.insert(lower(&target.name), TabEntry { name: target.name.clone(), … }); }
```

P² `format!`/`to_lowercase` + clones por tick, más `removed`/`added`/`gamemode_changed`
que clonan de nuevo solo para comparar. Propuesta: precalcular en `tick_tab` una
`Vec<(String /*lower*/, TabEntry)>` compartida (o un `Arc<[NormalizedTab]>`) y que
`sync_tab` itere sin asignar salvo en las entradas que realmente cambian. Con 100
jugadores elimina ~10 000 asignaciones/tick.

### J-5. `take_out()` pierde la capacidad en cada flush

```rust
pub fn take_out(&mut self) -> Vec<u8> { std::mem::take(&mut self.out_buf) }
```

`mem::take` deja un `Vec` vacío con capacidad 0: durante el streaming, `out_buf`
vuelve a crecer desde cero (varias reallocaciones + copias) cada 50 ms, y el buffer
grande se libera en el hilo de la conexión. Propuesta: intercambio con un buffer
reciclado propiedad de `handle_stream`:

```rust
pub fn take_out_into(&mut self, dst: &mut Vec<u8>) {
    std::mem::swap(&mut self.out_buf, dst);
    self.out_buf.clear();   // conserva la capacidad anterior de `dst`
}
```

`handle_stream` mantiene un `Vec` de salida por conexión (ya reserva 64 KB para
lectura). Riesgo nulo, ganancia clara justo cuando el tick está más cargado.

### J-6. `poll_bytes`: memmove + copia por paquete entrante

`poll_bytes` (`session.rs:285`) hace `self.in_buf.drain(..consumed)` por frame
(desplaza el resto del buffer) y `try_split_frame` (`wire.rs:509`) devuelve el payload
como `Vec` nuevo (`body[br.pos..].to_vec()`). Con 20-40 paquetes/s por jugador (PvP,
movimiento) es un `alloc` + una copia + un `memmove` por paquete.

Propuesta: `bytes::BytesMut` (el crate **ya es dependencia** de `java-bridge`) con
`advance(consumed)` O(1) y `split_to` para entregar el payload sin copia
(`handle_packet` puede recibir `&[u8]` desde un `BytesMut` separado). Es el patrón
clásico de framing; riesgo bajo, esfuerzo medio.

### J-7. `tick_inner`: locks y asignaciones por tick

- `let ids: Vec<u64> = self.sessions.lock().keys().copied().collect();` (`server.rs:360`)
  asigna un `Vec` por tick (20/s) aunque no haya sesiones.
- Tras eso se vuelve a bloquear el map **una vez por sesión** (a propósito, para no
  retener el lock durante un tick lento — mantener).
- `any_playing()` (`server.rs:522`) bloquea y recorre todas las sesiones y se llama
  4 veces por tick (`tick_tab`, `tick_entities`, `tick_time_sync`, `tick_block_changes`).
- `tick_time_sync` toma `last_broadcast_time_running.lock()` dos veces en la misma
  condición (`server.rs:478`).

Propuesta: fast path `if self.sessions.lock().is_empty() { return; }` al inicio de
`tick_inner`; calcular `playing` una vez y pasarlo a las fases; resolver el doble lock
en una variable. Ganancia pequeña pero es trabajo por tick en el hilo del reloj
central, el más sensible a jitter.

### J-8. Contención: el Mutex global de sesiones en el camino de red

`handle_stream` (`server.rs:631`) toma `self.sessions.lock()`, llama
`session.poll_bytes(...)` (que puede ejecutar hooks del mundo: `break_block`,
`place_block`, chat, comandos) y recién después suelta el lock. Cualquier handler de
paquete lento bloquea **el tick entero** (que necesita el mismo lock para iterar
sesiones) y a todas las demás conexiones. Es la deuda estructural del adaptador.

Propuesta (alto esfuerzo, alto impacto): `HashMap<u64, Arc<Mutex<JavaSession>>>` con
el map bajo un `RwLock` (lectura para el tick, escritura solo en alta/baja de
sesión) y el estado por sesión en su propio lock. Elimina la serialización entre
jugadores y permite que el tick libere el map mientras hace chunk streaming.

### J-9. `stream_chunks`: asignaciones por columna

Por cada chunk se construyen 4-6 `Vec` pequeños (`(cx as u32).to_be_bytes().to_vec()`,
`wire::varint(...)`, `varint(0)`, …) y luego se copia `built.data` al `payload`
(`session.rs:1735-1749`). Con J-1 (`send` escribiendo directo a `out_buf`) se puede
escribir la cabecera y el cuerpo en el buffer de salida sin `payload` intermedio:
`out_buf.extend(cx/cz/heightmaps/len) + extend(&built.data)`. Ahorra ~5 allocaciones y
una copia de ~80 KB por columna.

### J-10. Detalles menores del adaptador

- `handle_packet` (`session.rs:344`) inserta en `unhandled_ids` **cada** paquete y, la
  primera vez por `(estado, id)`, emite un `tracing::info!`. Es diagnóstico de
  desarrollo en nivel `info`: pasar a `debug!` (o gatear por variable de entorno)
  quita ruido y una rama de logging por paquete.
- `handle_stream` usa   `[0u8; 65536]` en el stack de la tarea (64 KB × conexión, `server.rs:637`) y un
  `tokio::time::interval(20 ms)` **por conexión** (`server.rs:638`, hasta 50 despertares/s por jugador).
  Un único intervalo compartido que marque "flush" a todas las conexiones reduce
  timers de O(N×50/s) a 50/s.
- `broadcast_block_change_with_prev` (`server.rs:548`) recalcula
  `hooks.modern_block_state(prev_id, protocol)` por sesión para el mismo valor: cachear
  por protocolo dentro del bucle.
- `sync_entities`/`sync_tab` reciben `&[TabTarget]`/`&[EntityTarget]` y vuelven a
  clonar entradas en `tab_listed`/`seen_entities` en el camino de actualización; usar
  `entry()` y actualizar campos in situ evita clones por tick.

---

## 4. Lo que ya está bien (no tocar)

- **Prefetch de chunks en sidecar** (`prefetch_blocking` + `try_*_chunk` solo-caché):
  evita el build de ~18 ms dentro del tick; el fallback de liveness está acotado.
- **Deadline compartido por tick** (`TICK_BUDGET`, 10 ms) y el log de fases: buena
  instrumentación para validar cualquier cambio de este documento.
- **Tick por sesión con lock liberado entre sesiones** (`server.rs:360-364`, intencional).
- **Gate por `has_listeners`** en la mayoría de emisores de `server-core`: es el patrón
  a extender (P-3).
- **Batching de acciones** (`takeActions` en PHP, `apply_actions` en Rust): evita un
  round trip por acción.
- **`unhandled_ids`/once-logging** y el diagnóstico de paquetes: útiles, solo bajar de
  nivel (J-10).

---

## 5. Cómo medir antes/después

1. `metrics::ipc()` (`server-core/src/metrics.rs:176`) — tiempo por round trip de
   evento. Comparar p50/p99 antes y después de P-1/P-3.
2. Log `[lag-ipc]` (`manager.rs:554`) — cuenta de eventos > 20 ms.
3. `Java bridge tick …` (`server.rs:390`) — `total`/`sessions`/`tab`/`rest` con 0, 10,
   50 y 100 sesiones simuladas; es la métrica correcta para J-3/J-4/J-7.
4. `Tick del puente Java lento` (> 15 ms) — antes/después de J-1/J-5/J-9 con carga de
   streaming (un jugador volando con view-distance alto).
5. Presión de allocator: `perf stat`/ETW o simplemente `Instant` alrededor de
   `tick_entities` y de `stream_chunks` mientras se conectan N clientes.

Sin medición no debería afirmarse una mejora: los puntos J-1/J-2/J-3 son de manual
(copias y O(P²) evidentes), pero el resto conviene cuantificarlo con (3) y (4).

---

## 6. Plan de implementación sugerido

**Fase 1 — sin riesgo semántico (una sesión de trabajo):**
J-1 (`send` + `varint_len`), J-5 (`take_out_into`), J-7 (fast path y locks), P-1
(parse único), P-2 (tick sin plugins), P-4 (buffer de escritura).

**Fase 2 — requiere revisar tests de comportamiento:**
J-3 + J-4 (dedup/normalización por tick), J-9 (cabecera directa a `out_buf`),
J-2 (`Arc` en la caché de payloads), J-10 (nivel de log, timer compartido).

**Fase 3 — estructural, solo con medición que lo justifique:**
J-8 (sesiones con lock propio), J-6 (`BytesMut`), P-3 con refresco de
`listener_events` por tick, P-5 (worker fire-and-forget para eventos no cancelables).
