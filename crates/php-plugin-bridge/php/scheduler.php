<?php

declare(strict_types=1);

namespace pocketmine\scheduler {

    use pocketmine\plugin\Plugin;

    /**
     * Thrown from Task::onRun() to cancel the handler (PM parity).
     */
    class CancelTaskException extends \Exception
    {
    }

    abstract class Task
    {
        private ?TaskHandler $handler = null;

        public function __construct()
        {
        }

        abstract public function onRun(int $currentTick): void;

        public function onCancel(): void
        {
        }

        public function getHandler(): ?TaskHandler
        {
            return $this->handler;
        }

        /** @internal */
        public function setHandler(?TaskHandler $handler): void
        {
            $this->handler = $handler;
        }

        public function getTaskId(): int
        {
            return $this->handler?->getTaskId() ?? -1;
        }

        public function isCancelled(): bool
        {
            return $this->handler?->isCancelled() ?? false;
        }

        public function cancel(): void
        {
            $this->handler?->cancel();
        }

        /**
         * PM `Task::getName()`: short class name with namespace stripped.
         */
        public function getName(): string
        {
            $class = static::class;
            $pos = strrpos($class, '\\');
            return $pos === false ? $class : substr($class, $pos + 1);
        }
    }

    class ClosureTask extends Task
    {
        /** @var \Closure */
        private \Closure $closure;

        public function __construct(\Closure $closure, ?\Closure $onCancel = null)
        {
            parent::__construct();
            $this->closure = $closure;
            $this->onCancelClosure = $onCancel;
        }

        /** @var \Closure|null */
        private ?\Closure $onCancelClosure = null;

        public function getClosure(): \Closure
        {
            return $this->closure;
        }

        public function onRun(int $currentTick): void
        {
            ($this->closure)($currentTick);
        }

        public function onCancel(): void
        {
            if ($this->onCancelClosure !== null) {
                ($this->onCancelClosure)();
            }
        }
    }

    class TaskHandler
    {
        private bool $cancelled = false;
        private bool $running = false;

        public function __construct(
            private int $taskId,
            private Task $task,
            private string $pluginName,
            private int $delay,
            private int $period,
            private bool $repeating,
            private int $startTick
        ) {
            $task->setHandler($this);
        }

        public function getTaskId(): int
        {
            return $this->taskId;
        }

        public function getTask(): Task
        {
            return $this->task;
        }

        public function getPluginName(): string
        {
            return $this->pluginName;
        }

        /**
         * PM alias: the owning plugin / scheduler owner.
         */
        public function getOwnerName(): string
        {
            return $this->pluginName;
        }

        /**
         * PM alias: short class name of the task.
         */
        public function getTaskName(): string
        {
            return $this->task->getName();
        }

        public function getDelay(): int
        {
            return $this->delay;
        }

        public function getPeriod(): int
        {
            return $this->period;
        }

        public function isRepeating(): bool
        {
            return $this->repeating;
        }

        public function isDelayed(): bool
        {
            return $this->delay > 0;
        }

        public function isCancelled(): bool
        {
            return $this->cancelled;
        }

        public function isRunning(): bool
        {
            return $this->running;
        }

        /**
         * Tick on which `run()` next becomes due (PM `TaskHandler::getNextRun()`).
         */
        public function getNextRun(): int
        {
            return $this->startTick + $this->delay;
        }

        /** @internal */
        public function setNextRun(int $ticks): void
        {
            // Re-anchor startTick so `startTick + delay === $ticks`.
            $this->startTick = $ticks - $this->delay;
        }

        public function cancel(): void
        {
            if ($this->cancelled) {
                return;
            }
            $this->cancelled = true;
            try {
                $this->task->onCancel();
            } catch (CancelTaskException) {
                // PM swallows cancel exceptions the same way.
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/SCHEDULER] onCancel failed: ' . $e->getMessage() . PHP_EOL);
            }
        }

        /** @internal PM `TaskHandler::remove()`: mark cancelled and detach. */
        public function remove(): void
        {
            $this->cancelled = true;
            try {
                $this->task->setHandler(null);
            } catch (\Throwable) {
                // Ignore detach failures.
            }
        }

        /** @internal returns true when the handler should be removed */
        public function run(int $currentTick): bool
        {
            if ($this->cancelled) {
                return true;
            }
            if ($currentTick < $this->startTick + $this->delay) {
                return false;
            }

            $this->running = true;
            try {
                $this->task->onRun($currentTick);
            } catch (CancelTaskException) {
                $this->cancel();
                return true;
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/SCHEDULER] Task #' . $this->taskId . ' of ' . $this->pluginName
                    . ' threw: ' . $e->getMessage() . PHP_EOL);
            } finally {
                $this->running = false;
            }

            if ($this->cancelled) {
                return true;
            }

            if (!$this->repeating) {
                return true;
            }

            $this->startTick = $currentTick;
            $this->delay = max(1, $this->period);
            return false;
        }
    }

    /**
     * PM `TaskScheduler`: per-plugin scheduler.
     *
     * One instance per plugin is created by `PluginBase::getScheduler()`
     * (`new TaskScheduler($plugin->getFullName())`, like PM). The server
     * singleton `Scheduler` ticks every registered `TaskScheduler` from its
     * `mainThreadHeartbeat()`, which the IPC loop calls on each `tick`
     * action, so per-plugin tasks run without the IPC layer having to know
     * about them.
     *
     * API differences vs PM (documented bridge extensions):
     * - PM signatures are `scheduleTask(Task)` / `scheduleDelayedTask(Task, int)`
     *   / `scheduleRepeatingTask(Task, int)` / `scheduleDelayedRepeatingTask(Task, int, int)`.
     *   This class also accepts the bridge's old overload
     *   `(Plugin $plugin, Task $task, ...)` and returns the same `TaskHandler`.
     * - `mainThreadHeartbeat()` is idempotent per tick (a second call with the
     *   same tick is a no-op) so both `Scheduler::mainThreadHeartbeat()` and
     *   `PluginManager::tickSchedulers()` may drive it safely within one tick.
     * - A disabled scheduler silently ignores heartbeats here; PM throws
     *   `LogicException`. The fan-out loop must not kill the server tick, so the
     *   throw is downgraded to a no-op (see `Scheduler::mainThreadHeartbeat()`).
     */
    class TaskScheduler
    {
        private bool $enabled = true;

        /** @var array<int, TaskHandler> */
        private array $tasks = [];

        private int $currentTick = 0;
        private int $lastHeartbeatTick = -1;
        private int $nextTaskId = 1;

        public function __construct(private ?string $owner = null)
        {
            Scheduler::registerTaskScheduler($this);
        }

        public function getOwner(): ?string
        {
            return $this->owner;
        }

        public function isEnabled(): bool
        {
            return $this->enabled;
        }

        public function setEnabled(bool $enabled): void
        {
            $this->enabled = $enabled;
        }

        /**
         * PM: `scheduleTask(Task $task)`. Bridge also accepts
         * `scheduleTask(Plugin $plugin, Task $task, int $delay = 0, ...)` for
         * code written against the old `Scheduler` overloads.
         */
        public function scheduleTask(mixed $pluginOrTask, mixed $taskOrDelay = 0, int $delay = 0, bool $repeating = false, int $period = 0): TaskHandler
        {
            if ($pluginOrTask instanceof Plugin) {
                return $this->addTask($this->requireTask($taskOrDelay), $delay, $period, $repeating);
            }
            $task = $this->requireTask($pluginOrTask);
            $d = (int)$taskOrDelay;
            return $this->addTask($task, $d, $period, $repeating);
        }

        public function scheduleDelayedTask(mixed $pluginOrTask, mixed $taskOrDelay, int $delayOrNothing = 0): TaskHandler
        {
            if ($pluginOrTask instanceof Plugin) {
                return $this->addTask($this->requireTask($taskOrDelay), $delayOrNothing, 0, false);
            }
            $task = $this->requireTask($pluginOrTask);
            return $this->addTask($task, (int)$taskOrDelay, 0, false);
        }

        public function scheduleRepeatingTask(mixed $pluginOrTask, mixed $taskOrPeriod, int $periodOrNothing = 1): TaskHandler
        {
            if ($pluginOrTask instanceof Plugin) {
                return $this->addTask($this->requireTask($taskOrPeriod), 0, $periodOrNothing, true);
            }
            $task = $this->requireTask($pluginOrTask);
            $period = (int)$taskOrPeriod;
            if ($period <= 0) {
                $period = $periodOrNothing;
            }
            return $this->addTask($task, 0, $period, true);
        }

        public function scheduleDelayedRepeatingTask(mixed $pluginOrTask, mixed $taskOrDelay, int $delayOrPeriod = 0, int $period = 0): TaskHandler
        {
            if ($pluginOrTask instanceof Plugin) {
                return $this->addTask($this->requireTask($taskOrDelay), $delayOrPeriod, $period, true);
            }
            $task = $this->requireTask($pluginOrTask);
            return $this->addTask($task, (int)$taskOrDelay, $delayOrPeriod, true);
        }

        public function cancelAllTasks(): void
        {
            foreach ($this->tasks as $handler) {
                $handler->cancel();
            }
            $this->tasks = [];
        }

        public function isQueued(TaskHandler $handler): bool
        {
            return isset($this->tasks[$handler->getTaskId()])
                && $this->tasks[$handler->getTaskId()] === $handler;
        }

        public function shutdown(): void
        {
            $this->enabled = false;
            $this->cancelAllTasks();
            Scheduler::unregisterTaskScheduler($this);
        }

        public function mainThreadHeartbeat(int $currentTick): void
        {
            if (!$this->enabled) {
                return;
            }
            if ($currentTick === $this->lastHeartbeatTick) {
                return;
            }
            $this->lastHeartbeatTick = $currentTick;
            $this->currentTick = $currentTick;
            foreach ($this->tasks as $id => $handler) {
                try {
                    if ($handler->run($currentTick)) {
                        unset($this->tasks[$id]);
                    }
                } catch (\Throwable $e) {
                    fwrite(STDERR, '[PHP/SCHEDULER] TaskScheduler heartbeat failed: ' . $e->getMessage() . PHP_EOL);
                }
            }
        }

        public function getPendingTaskCount(): int
        {
            return count($this->tasks);
        }

        /** @return array<int, TaskHandler> */
        public function getTasks(): array
        {
            return $this->tasks;
        }

        public function getCurrentTick(): int
        {
            return $this->currentTick;
        }

        private function requireTask(mixed $task): Task
        {
            if (!$task instanceof Task) {
                throw new \InvalidArgumentException('TaskScheduler requires a Task instance');
            }
            return $task;
        }

        private function addTask(Task $task, int $delay, int $period, bool $repeating): TaskHandler
        {
            if (!$this->enabled) {
                throw new \LogicException('Tried to schedule task to disabled scheduler');
            }
            if ($delay <= 0) {
                $delay = 0;
            }
            if ($period <= -1) {
                $period = 0;
            } elseif ($period < 1 && $repeating) {
                $period = 1;
            }
            // Seed currentTick from the server tick when available so a
            // scheduler created before the first heartbeat still schedules
            // relative to the server clock (PM keeps its own counter; the
            // IPC loop drives both, so they agree after the first tick).
            if ($this->currentTick === 0) {
                try {
                    $server = \pocketmine\Server::getInstance();
                    if (method_exists($server, 'getTick')) {
                        $this->currentTick = (int)$server->getTick();
                    }
                } catch (\Throwable) {
                    // No server yet; stay at 0.
                }
            }
            $handler = new TaskHandler(
                $this->nextTaskId++,
                $task,
                $this->owner ?? 'unknown',
                max(0, $delay),
                max(0, $period),
                $repeating,
                $this->currentTick
            );
            $this->tasks[$handler->getTaskId()] = $handler;
            return $handler;
        }
    }

    /**
     * PM `AsyncTask` (bridge stub).
     *
     * PM's `AsyncTask` runs in a worker thread (`pmmp/thread`) and cannot
     * access `Server`. This bridge has no worker threads: the PHP worker is a
     * single process, so the "pool" (`AsyncPool`) executes `onRun()` on the
     * main thread during the next `mainThreadHeartbeat()`, then calls
     * `onCompletion()` on the same tick. Code that relies on true parallelism
     * or thread isolation will run correctly but serially, on the main thread.
     *
     * Contract kept from PM where it fits the fallback:
     * - `run()` (called by the pool once) clears the result, calls `onRun()`,
     *   marks the task finished.
     * - `setResult()` / `getResult()` / `hasResult()` are in-memory (no
     *   `NonThreadSafeValue` wrapping needed; the fallback never serializes).
     * - `onCompletion()` is called on the main thread after `onRun()` completes.
     * - `publishProgress()` / `onProgressUpdate()` are accepted and delivered
     *   synchronously before `onCompletion()` (PM delivers them from the main
     *   thread too; the fallback queue is drained in `collectTasks()`).
     * - `storeLocal()` / `fetchLocal()` are plain per-instance arrays (PM
     *   restricts them to a single thread; the fallback has only one).
     * - `cancelRun()` / `hasCancelledRun()` are no-ops (deprecated in PM).
     */
    abstract class AsyncTask
    {
        private mixed $result = null;
        private bool $submitted = false;
        private bool $finished = false;
        private bool $terminated = false;
        /** @var array<int, string> serialized progress payloads (fallback: plain) */
        private array $progressUpdates = [];
        /** @var array<string, mixed> thread-local storage (fallback: per-instance) */
        private array $localStorage = [];
        private ?int $workerId = null;

        public function run(): void
        {
            $this->result = null;
            $this->finished = false;
            try {
                $this->onRun();
            } finally {
                $this->finished = true;
            }
        }

        /**
         * Actions to execute when run (main thread in the fallback pool).
         */
        abstract public function onRun(): void;

        /**
         * Actions to execute when completed (main thread).
         */
        public function onCompletion(): void
        {
        }

        public function publishProgress(mixed $progress): void
        {
            $this->progressUpdates[] = is_scalar($progress) || $progress === null
                ? (string)json_encode($progress)
                : (string)json_encode(['value' => $progress]);
        }

        /**
         * @internal Called by AsyncPool on the main thread.
         */
        public function checkProgressUpdates(): void
        {
            while ($this->progressUpdates !== []) {
                $raw = array_shift($this->progressUpdates);
                $decoded = json_decode((string)$raw, true);
                $this->onProgressUpdate($decoded);
            }
        }

        public function onProgressUpdate(mixed $progress): void
        {
        }

        public function onError(): void
        {
        }

        public function setResult(mixed $result): void
        {
            $this->result = $result;
        }

        public function getResult(): mixed
        {
            return $this->result;
        }

        public function hasResult(): bool
        {
            return $this->result !== null;
        }

        public function setSubmitted(): void
        {
            $this->submitted = true;
        }

        public function isSubmitted(): bool
        {
            return $this->submitted;
        }

        public function isFinished(): bool
        {
            return $this->finished || $this->terminated;
        }

        public function isCrashed(): bool
        {
            return $this->terminated;
        }

        public function isTerminated(): bool
        {
            return $this->terminated;
        }

        /** @internal */
        public function setTerminated(bool $terminated): void
        {
            $this->terminated = $terminated;
        }

        /** @deprecated no-op in PM. */
        public function cancelRun(): void
        {
        }

        /** @deprecated always false, like PM. */
        public function hasCancelledRun(): bool
        {
            return false;
        }

        public function setWorkerId(?int $workerId): void
        {
            $this->workerId = $workerId;
        }

        public function getWorkerId(): ?int
        {
            return $this->workerId;
        }

        protected function storeLocal(string $key, mixed $complexData): void
        {
            $this->localStorage[$key] = $complexData;
        }

        protected function fetchLocal(string $key): mixed
        {
            if (!array_key_exists($key, $this->localStorage)) {
                throw new \InvalidArgumentException('No matching thread-local data found on this thread');
            }
            return $this->localStorage[$key];
        }

        protected function reallyDestruct(): void
        {
        }
    }

    /**
     * Handler returned by `AsyncPool::submitTask()` / `Scheduler::scheduleAsyncTask()`.
     */
    class AsyncTaskHandler
    {
        public const STATUS_PENDING = 'pending';
        public const STATUS_RUNNING = 'running';
        public const STATUS_COMPLETED = 'completed';
        public const STATUS_CANCELLED = 'cancelled';

        private bool $cancelled = false;
        private string $status = self::STATUS_PENDING;

        public function __construct(
            private int $taskId,
            private AsyncTask $task,
            private string $pluginName,
            private int $workerId
        ) {
        }

        public function getTaskId(): int
        {
            return $this->taskId;
        }

        public function getTask(): AsyncTask
        {
            return $this->task;
        }

        public function getPluginName(): string
        {
            return $this->pluginName;
        }

        public function getOwnerName(): string
        {
            return $this->pluginName;
        }

        public function getWorkerId(): int
        {
            return $this->workerId;
        }

        public function getStatus(): string
        {
            return $this->status;
        }

        public function isPending(): bool
        {
            return $this->status === self::STATUS_PENDING;
        }

        public function isCancelled(): bool
        {
            return $this->cancelled || $this->status === self::STATUS_CANCELLED;
        }

        public function isFinished(): bool
        {
            return $this->status === self::STATUS_COMPLETED;
        }

        public function cancel(): void
        {
            if ($this->status === self::STATUS_COMPLETED || $this->status === self::STATUS_CANCELLED) {
                return;
            }
            $this->cancelled = true;
            $this->status = self::STATUS_CANCELLED;
        }

        /** @internal */
        public function setStatus(string $status): void
        {
            $this->status = $status;
        }
    }

    /**
     * PM `AsyncPoolWorkerEntry` stub (virtual worker).
     */
    class AsyncPoolWorkerEntry
    {
        public int $lastUsed = 0;

        public function __construct(
            public array $tasks = [],
            public int $sleeperNotifierId = 0
        ) {
        }
    }

    /**
     * PM `AsyncWorker` stub: no real thread, no class loader, no sleeper.
     */
    class AsyncWorker
    {
        private bool $terminated = false;
        private ?string $crashInfo = null;

        public function __construct(
            private int $workerId = 0
        ) {
        }

        public function getId(): int
        {
            return $this->workerId;
        }

        public function isTerminated(): bool
        {
            return $this->terminated;
        }

        public function getCrashInfo(): ?string
        {
            return $this->crashInfo;
        }

        public function start(int $options = 0): void
        {
            // No-op: virtual workers start on first submit.
        }

        public function quit(): void
        {
            $this->terminated = true;
        }

        public function collect(): void
        {
        }
    }

    /**
     * PM `AsyncPool` (bridge stub with a minimal real pool).
     *
     * Workers: this pool has no OS threads. `$size` is the maximum number of
     * virtual workers (default 1). Workers are started lazily on first submit
     * and are tracked in `$workers` exactly like PM, but `onRun()` executes on
     * the main thread during `collectTasks()` (called from
     * `Scheduler::mainThreadHeartbeat()` / the IPC `tick` action) instead of
     * on a `pmmp/thread` worker. There is no memory limit, class loader, or
     * sleeper notifier; `addWorkerStartHook()` still fires per virtual worker.
     *
     * A real threaded pool would require the `pmmp/thread` extension (or
     * `pcntl_fork`), neither of which is available in the embedded worker.
     * CPU-bound work submitted here will block the main thread for its whole
     * duration; I/O-bound work should use synchronous `Task` scheduling
     * instead (see `BulkCurlTask`).
     */
    class AsyncPool
    {
        /** @var array<int, AsyncPoolWorkerEntry> */
        private array $workers = [];

        /** @var array<int, AsyncTaskHandler> submitted handler by task id */
        private array $handlers = [];

        /** @var array<int, AsyncTask> FIFO queue per worker */
        private array $queues = [];

        /** @var array<int, \Closure> */
        private array $workerStartHooks = [];

        private int $nextTaskId = 1;

        public function __construct(
            private int $size = 1,
            private int $workerMemoryLimit = 0
        ) {
            if ($this->size < 1) {
                $this->size = 1;
            }
        }

        public function getSize(): int
        {
            return $this->size;
        }

        public function increaseSize(int $newSize): void
        {
            if ($newSize > $this->size) {
                $this->size = $newSize;
            }
        }

        /**
         * @param \Closure(int $workerId) : void $hook
         */
        public function addWorkerStartHook(\Closure $hook): void
        {
            $this->workerStartHooks[spl_object_id($hook)] = $hook;
            foreach ($this->workers as $id => $entry) {
                unset($entry);
                $hook($id);
            }
        }

        public function removeWorkerStartHook(\Closure $hook): void
        {
            unset($this->workerStartHooks[spl_object_id($hook)]);
        }

        /**
         * @return int[] IDs of currently running (virtual) workers.
         */
        public function getRunningWorkers(): array
        {
            return array_keys($this->workers);
        }

        public function getWorkerMemoryLimit(): int
        {
            return $this->workerMemoryLimit;
        }

        /**
         * PM `AsyncPool::submitTaskToWorker(AsyncTask, int)`.
         */
        public function submitTaskToWorker(AsyncTask $task, int $worker): void
        {
            if ($worker < 0 || $worker >= $this->size) {
                throw new \InvalidArgumentException('Invalid worker ' . $worker);
            }
            if ($task->isSubmitted()) {
                throw new \InvalidArgumentException('Cannot submit the same AsyncTask instance more than once');
            }
            $task->setSubmitted();
            $task->setWorkerId($worker);
            $this->ensureWorker($worker);
            $this->queues[$worker][] = $task;
        }

        /**
         * PM `AsyncPool::selectWorker()`: idle worker first, else smallest
         * backlog, starting a new virtual worker while the pool is not full.
         */
        public function selectWorker(): int
        {
            $worker = null;
            $minUsage = PHP_INT_MAX;
            foreach ($this->workers as $id => $entry) {
                unset($entry);
                $usage = count($this->queues[$id] ?? []);
                if ($usage < $minUsage) {
                    $worker = $id;
                    $minUsage = $usage;
                    if ($usage === 0) {
                        break;
                    }
                }
            }
            if ($worker === null || ($minUsage > 0 && count($this->workers) < $this->size)) {
                for ($i = 0; $i < $this->size; ++$i) {
                    if (!isset($this->workers[$i])) {
                        $worker = $i;
                        break;
                    }
                }
            }
            if ($worker === null) {
                $worker = 0;
            }
            return $worker;
        }

        /**
         * PM `AsyncPool::submitTask(AsyncTask): int`.
         */
        public function submitTask(AsyncTask $task): int
        {
            if ($task->isSubmitted()) {
                throw new \InvalidArgumentException('Cannot submit the same AsyncTask instance more than once');
            }
            $worker = $this->selectWorker();
            $this->submitTaskToWorker($task, $worker);
            return $worker;
        }

        /**
         * Fallback submit that also creates a trackable handler.
         * `Scheduler::scheduleAsyncTask()` uses this; PM code uses `submitTask()`.
         */
        public function submitTaskWithHandler(AsyncTask $task, string $pluginName): AsyncTaskHandler
        {
            $worker = $this->submitTask($task);
            $handler = new AsyncTaskHandler($this->nextTaskId++, $task, $pluginName, $worker);
            $this->handlers[$handler->getTaskId()] = $handler;
            return $handler;
        }

        /**
         * Executes pending `onRun()` work (virtual workers, main thread) and
         * delivers `onCompletion()` for finished tasks. Returns true when work
         * remains (PM meaning: tasks left to collect).
         */
        public function collectTasks(): bool
        {
            foreach (array_keys($this->workers) as $workerId) {
                $this->collectTasksFromWorker($workerId);
            }
            // Second loop mirrors PM: collection may have enqueued new work.
            foreach ($this->queues as $queue) {
                if ($queue !== []) {
                    return true;
                }
            }
            return false;
        }

        public function collectTasksFromWorker(int $worker): bool
        {
            if (!isset($this->workers[$worker])) {
                throw new \InvalidArgumentException('No such worker ' . $worker);
            }
            $more = false;
            while (!empty($this->queues[$worker])) {
                $task = $this->queues[$worker][0];
                // Virtual worker: run synchronously on the first collection.
                $handler = $this->findHandlerFor($task);
                if ($handler !== null && $handler->isCancelled()) {
                    array_shift($this->queues[$worker]);
                    continue;
                }
                if ($handler !== null && $handler->getStatus() === AsyncTaskHandler::STATUS_PENDING) {
                    $handler->setStatus(AsyncTaskHandler::STATUS_RUNNING);
                }
                try {
                    $task->run();
                } catch (\Throwable $e) {
                    fwrite(STDERR, '[PHP/SCHEDULER] Async task ' . $task::class . ' threw: ' . $e->getMessage() . PHP_EOL);
                }
                array_shift($this->queues[$worker]);
                $this->workers[$worker]->lastUsed = time();
                // Drain progress updates before completion, like PM.
                try {
                    $task->checkProgressUpdates();
                } catch (\Throwable $e) {
                    fwrite(STDERR, '[PHP/SCHEDULER] Async progress failed: ' . $e->getMessage() . PHP_EOL);
                }
                try {
                    $task->onCompletion();
                } catch (\Throwable $e) {
                    fwrite(STDERR, '[PHP/SCHEDULER] Async onCompletion failed: ' . $e->getMessage() . PHP_EOL);
                }
                if ($handler !== null && !$handler->isCancelled()) {
                    $handler->setStatus(AsyncTaskHandler::STATUS_COMPLETED);
                }
            }
            return $more;
        }

        /** @return array<int, int> worker id => queue size */
        public function getTaskQueueSizes(): array
        {
            $out = [];
            foreach ($this->workers as $id => $entry) {
                unset($entry);
                $out[$id] = count($this->queues[$id] ?? []);
            }
            return $out;
        }

        public function shutdownUnusedWorkers(): int
        {
            $ret = 0;
            $now = time();
            foreach ($this->workers as $id => $entry) {
                if ($entry->lastUsed + 300 < $now && empty($this->queues[$id])) {
                    unset($this->workers[$id], $this->queues[$id]);
                    $ret++;
                }
            }
            return $ret;
        }

        /**
         * Cancels all pending handlers and drops all workers.
         */
        public function shutdown(): void
        {
            foreach ($this->handlers as $handler) {
                $handler->cancel();
            }
            $this->handlers = [];
            $this->queues = [];
            $this->workers = [];
        }

        private function ensureWorker(int $workerId): void
        {
            if (!isset($this->workers[$workerId])) {
                $this->workers[$workerId] = new AsyncPoolWorkerEntry([], 0);
                $this->workers[$workerId]->lastUsed = time();
                $this->queues[$workerId] ??= [];
                foreach ($this->workerStartHooks as $hook) {
                    try {
                        $hook($workerId);
                    } catch (\Throwable $e) {
                        fwrite(STDERR, '[PHP/SCHEDULER] Worker start hook failed: ' . $e->getMessage() . PHP_EOL);
                    }
                }
            }
        }

        private function findHandlerFor(AsyncTask $task): ?AsyncTaskHandler
        {
            foreach ($this->handlers as $handler) {
                if ($handler->getTask() === $task) {
                    return $handler;
                }
            }
            return null;
        }
    }

    /**
     * PM `BulkCurlTaskOperation` stub.
     */
    final class BulkCurlTaskOperation
    {
        /**
         * @param array<string, mixed> $extraOpts
         */
        public function __construct(
            private string $page,
            private int $timeout = 10,
            private array $extraHeaders = [],
            private array $extraOpts = []
        ) {
        }

        public function getPage(): string
        {
            return $this->page;
        }

        public function getTimeout(): int
        {
            return $this->timeout;
        }

        /** @return array<string, mixed> */
        public function getExtraHeaders(): array
        {
            return $this->extraHeaders;
        }

        /** @return array<string, mixed> */
        public function getExtraOpts(): array
        {
            return $this->extraOpts;
        }
    }

    /**
     * PM `BulkCurlTask` kept as a cancelled stub.
     *
     * PM runs a list of cURL operations in a worker thread (`Internet::simpleCurl`)
     * and returns results through `onCompletion()`. In this bridge the async
     * pool is single-threaded and the `Internet` helper does not exist, so a
     * real HTTP fan-out is NOT implemented. The task is cancelled by design:
     * `onRun()` throws a `LogicException` with a clear message instead of
     * performing network I/O on the main thread, and `onCompletion()` never
     * fires. Callers should schedule a synchronous `Task` that performs the
     * HTTP fetch (blocking, on the main thread) or run the fetch from Rust.
     */
    class BulkCurlTask extends AsyncTask
    {
        /** @var array<int, BulkCurlTaskOperation> */
        private array $operations = [];

        /**
         * @param BulkCurlTaskOperation[] $operations
         * @param \Closure $onCompletion `function(array $results): void`
         */
        public function __construct(array $operations, \Closure $onCompletion)
        {
            $this->operations = array_values($operations);
            $this->storeLocal('completionCallback', $onCompletion);
        }

        public function onRun(): void
        {
            throw new \LogicException(
                'BulkCurlTask is not supported in this bridge (no worker threads, no Internet helper); '
                . 'schedule a synchronous Task for HTTP work instead'
            );
        }

        public function onCompletion(): void
        {
            // Intentionally empty: onRun() always throws, so no result exists.
        }
    }

    class Scheduler
    {
        private static ?Scheduler $instance = null;

        /** @var array<int, TaskHandler> */
        private array $tasks = [];

        /** @var array<int, TaskScheduler> */
        private static array $taskSchedulers = [];

        private ?AsyncPool $asyncPool = null;

        private int $nextTaskId = 1;
        private int $currentTick = 0;
        private int $lastHeartbeatTick = -1;
        private bool $enabled = true;

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
        }

        /** @internal */
        public static function registerTaskScheduler(TaskScheduler $scheduler): void
        {
            self::$taskSchedulers[spl_object_id($scheduler)] = $scheduler;
        }

        /** @internal */
        public static function unregisterTaskScheduler(TaskScheduler $scheduler): void
        {
            unset(self::$taskSchedulers[spl_object_id($scheduler)]);
        }

        /** @return array<int, TaskScheduler> */
        public static function getTaskSchedulers(): array
        {
            return self::$taskSchedulers;
        }

        public function getAsyncPool(): AsyncPool
        {
            return $this->asyncPool ??= new AsyncPool(1);
        }

        public function setAsyncPool(AsyncPool $pool): void
        {
            $this->asyncPool = $pool;
        }

        public function isEnabled(): bool
        {
            return $this->enabled;
        }

        public function setEnabled(bool $enabled): void
        {
            $this->enabled = $enabled;
        }

        public function scheduleTask(mixed $pluginOrTask, mixed $taskOrDelay = 0, int $delay = 0, bool $repeating = false, int $period = 0): TaskHandler
        {
            if ($pluginOrTask instanceof Plugin) {
                return $this->createHandler(
                    $pluginOrTask->getName(),
                    $this->requireTask($taskOrDelay),
                    $delay,
                    $period,
                    $repeating
                );
            }

            $task = $this->requireTask($pluginOrTask);
            return $this->createHandler($this->resolveTaskOwner($task), $task, (int)$taskOrDelay, 0, false);
        }

        public function scheduleDelayedTask(mixed $pluginOrTask, mixed $taskOrDelay, int $delayOrNothing = 0): TaskHandler
        {
            if ($pluginOrTask instanceof Plugin) {
                return $this->createHandler(
                    $pluginOrTask->getName(),
                    $this->requireTask($taskOrDelay),
                    $delayOrNothing,
                    0,
                    false
                );
            }

            $task = $this->requireTask($pluginOrTask);
            return $this->createHandler($this->resolveTaskOwner($task), $task, (int)$taskOrDelay, 0, false);
        }

        public function scheduleRepeatingTask(mixed $pluginOrTask, mixed $taskOrPeriod, int $periodOrNothing = 1): TaskHandler
        {
            if ($pluginOrTask instanceof Plugin) {
                return $this->createHandler(
                    $pluginOrTask->getName(),
                    $this->requireTask($taskOrPeriod),
                    0,
                    $periodOrNothing,
                    true
                );
            }

            $task = $this->requireTask($pluginOrTask);
            $period = (int)$taskOrPeriod;
            if ($period <= 0) {
                $period = $periodOrNothing;
            }
            return $this->createHandler($this->resolveTaskOwner($task), $task, 0, $period, true);
        }

        public function scheduleDelayedRepeatingTask(mixed $pluginOrTask, mixed $taskOrDelay, int $delayOrPeriod = 0, int $period = 0): TaskHandler
        {
            if ($pluginOrTask instanceof Plugin) {
                return $this->createHandler(
                    $pluginOrTask->getName(),
                    $this->requireTask($taskOrDelay),
                    $delayOrPeriod,
                    $period,
                    true
                );
            }

            $task = $this->requireTask($pluginOrTask);
            return $this->createHandler($this->resolveTaskOwner($task), $task, (int)$taskOrDelay, $delayOrPeriod, true);
        }

        /**
         * Minimal async scheduling through `AsyncPool`.
         *
         * In PM, async work goes through `AsyncPool::submitTask()` with worker
         * threads. This bridge has no threads, so the pool runs `onRun()` on
         * the main thread at the next `mainThreadHeartbeat()` (the IPC `tick`
         * action). The returned handler starts in `STATUS_PENDING` and moves to
         * `STATUS_RUNNING` / `STATUS_COMPLETED` as `collectTasks()` executes it.
         *
         * Passing a synchronous `Task` here is a clear error: use `scheduleTask()`
         * / `scheduleDelayedTask()` for `Task`, and this method only for `AsyncTask`.
         */
        public function scheduleAsyncTask(mixed $pluginOrTask, mixed $taskOrNothing = null): AsyncTaskHandler
        {
            if ($pluginOrTask instanceof Plugin) {
                $owner = $pluginOrTask->getName();
                $task = $this->requireAsyncTask($taskOrNothing);
            } else {
                $task = $this->requireAsyncTask($pluginOrTask);
                $owner = $this->resolveAsyncTaskOwner($task);
            }

            return $this->getAsyncPool()->submitTaskWithHandler($task, $owner);
        }

        private function requireTask(mixed $task): Task
        {
            if ($task instanceof AsyncTask) {
                throw new \InvalidArgumentException(
                    'scheduleTask requires a Task instance, got AsyncTask; use scheduleAsyncTask() for async work'
                );
            }
            if (!$task instanceof Task) {
                throw new \InvalidArgumentException('Scheduler requires a Task instance');
            }
            return $task;
        }

        private function requireAsyncTask(mixed $task): AsyncTask
        {
            if ($task instanceof Task && !$task instanceof AsyncTask) {
                throw new \InvalidArgumentException(
                    'scheduleAsyncTask requires an AsyncTask instance, got Task; use scheduleTask() for synchronous tasks'
                );
            }
            if (!$task instanceof AsyncTask) {
                throw new \InvalidArgumentException('scheduleAsyncTask requires an AsyncTask instance');
            }
            return $task;
        }

        private function createHandler(string $pluginName, Task $task, int $delay, int $period, bool $repeating): TaskHandler
        {
            if (!$this->enabled) {
                throw new \LogicException('Tried to schedule task to disabled scheduler');
            }
            $handler = new TaskHandler(
                $this->nextTaskId++,
                $task,
                $pluginName,
                max(0, $delay),
                max(0, $period),
                $repeating,
                $this->currentTick
            );
            $this->tasks[$handler->getTaskId()] = $handler;
            return $handler;
        }

        /**
         * Infers the owning plugin from the closure bound inside a ClosureTask,
         * which is how PocketMine plugins normally schedule work.
         */
        private function resolveTaskOwner(Task $task): string
        {
            if (!method_exists($task, 'getClosure')) {
                return 'unknown';
            }
            try {
                $closure = $task->getClosure();
                if (!$closure instanceof \Closure) {
                    return 'unknown';
                }
                $bound = (new \ReflectionFunction($closure))->getClosureThis();
                if ($bound instanceof Plugin) {
                    return $bound->getName();
                }
            } catch (\Throwable) {
                // Fall back below.
            }
            return 'unknown';
        }

        private function resolveAsyncTaskOwner(AsyncTask $task): string
        {
            return 'unknown';
        }

        public function cancelTask(int $taskId): void
        {
            if (!isset($this->tasks[$taskId])) {
                return;
            }
            $this->tasks[$taskId]->cancel();
            unset($this->tasks[$taskId]);
        }

        public function cancelTasks(string $pluginName): void
        {
            foreach ($this->tasks as $id => $handler) {
                if ($handler->getPluginName() === $pluginName) {
                    $handler->cancel();
                    unset($this->tasks[$id]);
                }
            }
        }

        public function cancelAllTasks(): void
        {
            foreach ($this->tasks as $handler) {
                $handler->cancel();
            }
            $this->tasks = [];
        }

        public function isQueued(TaskHandler $handler): bool
        {
            return isset($this->tasks[$handler->getTaskId()])
                && $this->tasks[$handler->getTaskId()] === $handler;
        }

        public function shutdown(): void
        {
            $this->enabled = false;
            $this->cancelAllTasks();
            $this->asyncPool?->shutdown();
        }

        public function mainThreadHeartbeat(int $currentTick): void
        {
            if ($currentTick === $this->lastHeartbeatTick) {
                return;
            }
            $this->lastHeartbeatTick = $currentTick;
            $this->currentTick = $currentTick;
            foreach ($this->tasks as $id => $handler) {
                try {
                    if ($handler->run($currentTick)) {
                        unset($this->tasks[$id]);
                    }
                } catch (\Throwable $e) {
                    fwrite(STDERR, '[PHP/SCHEDULER] Scheduler heartbeat failed: ' . $e->getMessage() . PHP_EOL);
                }
            }
            // Tick per-plugin schedulers (registered TaskSchedulers). The IPC
            // loop only calls this heartbeat (via PluginManager::getScheduler()),
            // so fan-out here is what makes PluginBase::getScheduler() work.
            foreach (self::$taskSchedulers as $scheduler) {
                try {
                    $scheduler->mainThreadHeartbeat($currentTick);
                } catch (\Throwable $e) {
                    fwrite(STDERR, '[PHP/SCHEDULER] TaskScheduler fan-out failed: ' . $e->getMessage() . PHP_EOL);
                }
            }
            try {
                $this->getAsyncPool()->collectTasks();
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/SCHEDULER] Async collect failed: ' . $e->getMessage() . PHP_EOL);
            }
        }

        public function getPendingTaskCount(): int
        {
            // Own tasks + every registered per-plugin scheduler + async queue,
            // so the IPC `pending_tasks` figure (which reads only this
            // singleton) reflects work scheduled via PluginBase::getScheduler().
            $count = count($this->tasks);
            foreach (self::$taskSchedulers as $scheduler) {
                $count += $scheduler->getPendingTaskCount();
            }
            if ($this->asyncPool !== null) {
                foreach ($this->asyncPool->getTaskQueueSizes() as $queued) {
                    $count += $queued;
                }
            }
            return $count;
        }

        /** @return array<int, TaskHandler> */
        public function getTasks(): array
        {
            return $this->tasks;
        }

        public function getCurrentTick(): int
        {
            try {
                $server = \pocketmine\Server::getInstance();
                if (method_exists($server, 'getTick')) {
                    return (int)$server->getTick();
                }
            } catch (\Throwable) {
                // No server yet.
            }
            return $this->currentTick;
        }
    }
}
