# 13 — Scheduler (relój 20 TPS, tareas y pool async)

Fuentes: `crates/server-core/src/main.rs` (reloj lógico 50 ms),
`crates/php-plugin-bridge/src/manager.rs` (`tick_sync`),
`crates/php-plugin-bridge/php/ipc.php` (`actionTick`),
`crates/php-plugin-bridge/php/plugin.php` (`tickSchedulers`,
`getScheduler`), `crates/php-plugin-bridge/php/scheduler.php` (todo lo
demás), `crates/server-core/src/metrics.rs` (`/tps`, `/status`).

El scheduler **no vive en Rust**: no hay `scheduler.rs` en
`server-core`. El Rust solo produce el número de tick cada 50 ms (20 TPS)
y lo empuja por IPC al sidecar PHP, donde corren `Task`/`AsyncTask` en el
hilo principal de un solo proceso.

## 1. El reloj central: 20 TPS (50 ms), un solo intervalo

`main.rs` (comentario «Single server tick») levanta **exactamente un**
`tokio::time::interval(Duration::from_millis(50))` con
`MissedTickBehavior::Skip` (nunca ráfaga de catch-up: un tick lento no
encola cientos de ticks atrasados). Cada vuelta:

1. `metrics().record_tick()` — TPS 1m/5m/15m capado a 20 (`metrics.rs`).
2. `session_manager.tick()` → Bedrock core (world clock, `SetTime`,
   forwarding, sync virtual, `PreSpawn` 1 Hz) — solo estado + envelopes,
   **nunca I/O**.
3. `java_server.tick_at(tick)` en `spawn_blocking` con **el mismo número**
   de tick (mundo compartido `WorldState::time`).
4. `tick_tx.send(tick)` → sidecar de plugins (canal `mpsc::unbounded`);
   si el sidecar murió, el reloj sigue (el `let _ =` es deliberado).
5. Autosave PM cada 6000 ticks (5 min = `tick % 6000 == 0`) en
   `spawn_blocking`, fire-and-forget.
6. Si el tick tarda >10 ms, `debug!` «Logical tick {tick} took … (must stay
   << 50 ms)».

**Sidecar PHP** (`manager.rs::tick_sync`): goroutine Tokio con
`spawn_blocking(manager.tick_sync(tick))` serializada y en orden; guarda
`current_tick` (`AtomicU64`) y pide `php_worker.request_sync("tick",
{ tick })`. Respuestas con acciones se aplican vía
`session_manager.apply_actions()`. Fallos: warning «Plugin tick … failed»
y se sigue; si ya empieza el shutdown (`is_shutting_down()`), ticks
tardíos son no-ops — el reloj también se detiene (`break`) al apagarse.
Una acción PHP atascada (timeout IPC 30 s) solo embalsa su cola, no el TPS
(antes se esperaba inline y debían desistir).

**Lado PHP** (`ipc.php::actionTick`): fija `Server::setTick(tick)`, llama
`Scheduler::mainThreadHeartbeat(tick)` (try/catch con log) y responde
`{ status, tick, pending_tasks }`. Ojo: `pending_tasks` es
`Scheduler::getPendingTaskCount()` = tareas propias del singleton **más**
todos los `TaskScheduler` de plugin **más** la cola async (§4).

## 2. `Task` y `TaskHandler`

```php
abstract class Task {
  abstract public function onRun(int $currentTick): void;
  public function onCancel(): void {}
  getHandler() / getTaskId() (-1 si no hay) / isCancelled() / cancel()
  getName(): short class name sin namespace
}
class ClosureTask extends Task { __construct(Closure $run, ?Closure $onCancel) }
```

- `TaskHandler { taskId, task, pluginName, delay, period, repeating,
  startTick }`: `getNextRun() = startTick + delay`,
  `setNextRun(t)` re-ancla `startTick`. `cancel()` marca y llama
  `onCancel()` (captura `CancelTaskException` en silencio, resto a stderr);
  `remove()` marca y desengancha (`setHandler(null)`).
- `run(currentTick)`: si cancelado → `true` (borrar); si aún no toca
  (`currentTick < startTick + delay`) → `false`; si toca: `onRun()`
  ( `CancelTaskException` → `cancel()` + `true`; otro `Throwable` → stderr
  «Task #id of plugin threw: …» y sigue); no-repeating → `true`;
  repeating → re-ancla `startTick = currentTick`, `delay = max(1, period)`
  y `false`. Llamar `cancel()` dentro de `onRun()` también se detecta al
  volver.

## 3. `TaskScheduler` (por plugin) y `Scheduler` (singleton)

`PluginBase::getScheduler()` crea **un `TaskScheduler` por plugin**
(`new TaskScheduler($plugin->getFullName())`, como PM) y se auto-registra en
el singleton (`Scheduler::registerTaskScheduler`, clave `spl_object_id`).
`PluginManager::tickSchedulers(tick)` los pulsa todos con heartbeat.

| API | `TaskScheduler` (por plugin) | `Scheduler` (singleton) |
|---|---|---|
| `scheduleTask(Task)` | delay 0 | delay 0 |
| `scheduleDelayedTask(Task, delay)` | una vez, tras `delay` | ídem |
| `scheduleRepeatingTask(Task, period)` | cada `period` (mín. 1) | ídem |
| `scheduleDelayedRepeatingTask(Task, delay, period)` | tras `delay`, cada `period` | ídem |
| `scheduleTask(Plugin, Task, delay, repeating, period)` | overload legacy del bridge | ídem (owner = `plugin->getName()`) |
| `scheduleAsyncTask(AsyncTask)` | — (va al pool) | `AsyncTaskHandler` (§4) |

Reglas compartidas:

- **Clamping** en `addTask`/`createHandler`: `delay <= 0` → `0`;
  `period <= -1` → `0`; `period < 1` y repeating → `1`. Scheduler
  deshabilitado → `LogicException('Tried to schedule task to disabled
  scheduler')`. Sin tick aún, `currentTick` se siembra desde
  `Server::getTick()` si hay servidor.
- **Heartbeat idempotente por tick**: `mainThreadHeartbeat(tick)` guarda
  `lastHeartbeatTick` y un segundo llamado con el mismo tick es no-op
  (pueden convivir `Scheduler::mainThreadHeartbeat()` y
  `PluginManager::tickSchedulers()` sin doble disparo). Un scheduler
  deshabilitado **ignora** el heartbeat (en PM lanzaría `LogicException`;
  aquí se degrada a no-op para no matar el tick del servidor).
- **Fan-out**: `Scheduler::mainThreadHeartbeat` corre sus tareas, luego
  pulsa **cada `TaskScheduler` registrado** y por último
  `AsyncPool::collectTasks()` — los errores de cada ramo van a stderr sin
  abortar el resto.
- `cancelTask(id)` / `cancelTasks(pluginName)` / `cancelAllTasks()`,
  `isQueued(handler)` (identidad, no solo id), `shutdown()` (deshabilita,
  cancela todo, desregistra; en el singleton además apaga el pool),
  `getTasks()` / `getCurrentTick()`.
- Type-safety cruzada: pasar un `AsyncTask` a `scheduleTask()` o un `Task`
  a `scheduleAsyncTask()` es `InvalidArgumentException` con mensaje que
  indica la alternativa correcta. Owner de un `ClosureTask` sin plugin: se
  infiere por reflection del closure enlazado (`getClosureThis()` es un
  `Plugin` → su nombre; si no, `'unknown'`).

## 4. `AsyncTask` / `AsyncPool`: stub con pool mínimo real

PM corre `AsyncTask::onRun()` en worker threads (`pmmp/thread`) sin acceso
a `Server`. **Este bridge no tiene threads** (ni `pmmp/thread` ni
`pcntl_fork` en el worker embebido): el «pool» es un stub con semántica
real de colas/handlers, pero la ejecución es **serial en el hilo
principal** durante `collectTasks()` (dentro del heartbeat). Correcto,
pero sin paralelismo ni aislamiento de memoria: trabajo CPU-bound
**bloquea el tick entero** (ver nota en `AsyncPool`).

**`AsyncTask`** (abstracta): `run()` limpia `result`, llama `onRun()` y
marca `finished`. `setResult/getResult/hasResult` en memoria (sin
`NonThreadSafeValue`: nunca se serializa). `publishProgress()` guarda
(payload JSON) y `checkProgressUpdates()` los entrega a
`onProgressUpdate()` antes de `onCompletion()` (también en principal, como
PM). `storeLocal/fetchLocal` = array por instancia
(`fetchLocal` sin clave → `InvalidArgumentException`).
`cancelRun()/hasCancelledRun()` deprecados = no-op/`false`, como PM.
Estado vía `isSubmitted/isFinished/isCrashed/isTerminated`, `workerId`.

**`AsyncPool`** (`size = 1` por defecto, mínimo 1; sin límite de memoria ni
class-loader):

- **Workers virtuales**: `ensureWorker()` crea `AsyncPoolWorkerEntry {
  tasks, sleeperNotifierId, lastUsed }` lazy al primer submit y dispara
  `addWorkerStartHook($workerId)` por worker.
- `submitTask(task): int` elige worker con **`selectWorker()`** (el idle
  primero; si no, el de menor backlog; si queda capacidad, arranca uno
  nuevo) y `submitTaskToWorker(task, worker)` valida rango
  (`InvalidArgumentException: Invalid worker …`) y **no re-enviar la misma
  instancia** (`Cannot submit the same AsyncTask instance…`).
  `submitTaskWithHandler(task, plugin)` = `submitTask` + `AsyncTaskHandler`
  (lo que usa `Scheduler::scheduleAsyncTask()`; el PM puro usa
  `submitTask()` y recibe el worker id).
- `collectTasks()` recorre workers, ejecuta cada `onRun()` en la cola FIFO,
  drena `checkProgressUpdates()`, llama `onCompletion()` — todo con
  try/catch a stderr — y marca el handler `pending → running →
  completed` (o `cancelled` si se canceló antes de correr). Devuelve `true`
  si queda trabajo (segundo loop de PM: colectar puede haber encolado más).
- `getTaskQueueSizes()` (worker → cola), `shutdownUnusedWorkers()` (sin uso
  en 300 s y cola vacía se tira), `shutdown()` (cancela handlers y tira
  workers y colas).
- `AsyncTaskHandler { STATUS_PENDING|RUNNING|COMPLETED|CANCELLED,
  taskId, task, pluginName, workerId }`: `isPending/isCancelled/isFinished`,
  `cancel()` (no-op si ya terminó), `getOwnerName()` = alias PM.
- `AsyncWorker` = stub sin thread (`start()` no-op, `quit()` marca
  terminado, `collect()` no-op, `isTerminated/getCrashInfo`).

**`BulkCurlTask`**: stub **cancelado por diseño** — acepta
`BulkCurlTaskOperation[]` (page/timeout/headers/opts) + closure de
completado, pero `onRun()` lanza
`LogicException('…no worker threads, no Internet helper…')` y
`onCompletion()` es no-op. Para HTTP: tarea síncrona (bloqueante) o fetch
desde Rust.

## 5. Métricas TPS y presupuestos

`metrics.rs` graba un `Instant` por tick y promedia TPS en ventanas
1m/5m/15m capadas a 20 (al arrancar no diluye: con <ventana transcurrida
cuenta solo lo medido, así TPS empieza ~20, no ~0). Eso alimenta
`CommandSource.tps` → `/tps` (formato Paper con `*` en el 20.0) y
`/status` (formato PM: TPS instantáneo + medio + `%` de carga). El
presupuesto real: «Logical tick … must stay << 50 ms», y todo lo
bloqueante (PHP IPC, chunks ~18 ms/columna, autosave) vive fuera del
reloj en `spawn_blocking`/sidecars.

## 6. Limitaciones

- Nada de `TaskScheduler` en Rust: `main.rs` duck-typea el lado PHP
  (`PluginManager::tick_sync`, `current_tick`, `pending_tasks`) vía IPC
  (`tick`, `pending_tasks`).
- Un `AsyncTask` pesada **congela el 20 TPS** mientras corre (mismo
  proceso, mismo hilo). Para I/O async usar `Task` síncrona corta o Rust.
- Fechas/horas y delays son en **ticks**, no wall-clock: si el servidor se
  pausa, los delays se pausan con él (paridad PM).
