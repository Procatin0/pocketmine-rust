<?php

declare(strict_types=1);

namespace pocketmine\scheduler {

    use pocketmine\plugin\Plugin;

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

        public function isCancelled(): bool
        {
            return $this->cancelled;
        }

        public function isRunning(): bool
        {
            return $this->running;
        }

        public function cancel(): void
        {
            if ($this->cancelled) {
                return;
            }
            $this->cancelled = true;
            try {
                $this->task->onCancel();
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/SCHEDULER] onCancel failed: ' . $e->getMessage() . PHP_EOL);
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
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/SCHEDULER] Task #' . $this->taskId . ' of ' . $this->pluginName
                    . ' threw: ' . $e->getMessage() . PHP_EOL);
            } finally {
                $this->running = false;
            }

            if (!$this->repeating) {
                return true;
            }

            $this->startTick = $currentTick;
            $this->delay = max(1, $this->period);
            return false;
        }
    }

    class Scheduler
    {
        private static ?Scheduler $instance = null;

        /** @var array<int, TaskHandler> */
        private array $tasks = [];

        private int $nextTaskId = 1;

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
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

        public function scheduleAsyncTask(mixed $pluginOrTask, mixed $taskOrNothing = null): TaskHandler
        {
            if ($pluginOrTask instanceof Plugin) {
                $task = $this->requireTask($taskOrNothing);
                $owner = $pluginOrTask->getName();
            } else {
                $task = $this->requireTask($pluginOrTask);
                $owner = $this->resolveTaskOwner($task);
            }

            // The PHP worker is single threaded; async tasks run on the next tick.
            $handler = $this->createHandler($owner, $task, 1, 0, false);
            fwrite(STDERR, '[PHP/SCHEDULER] Async task #' . $handler->getTaskId()
                . ' scheduled on the main worker thread' . PHP_EOL);
            return $handler;
        }

        private function requireTask(mixed $task): Task
        {
            if (!$task instanceof Task) {
                throw new \InvalidArgumentException('Scheduler requires a Task instance');
            }
            return $task;
        }

        private function createHandler(string $pluginName, Task $task, int $delay, int $period, bool $repeating): TaskHandler
        {
            $handler = new TaskHandler(
                $this->nextTaskId++,
                $task,
                $pluginName,
                max(0, $delay),
                max(0, $period),
                $repeating,
                $this->getCurrentTick()
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

        public function mainThreadHeartbeat(int $currentTick): void
        {
            foreach ($this->tasks as $id => $handler) {
                if ($handler->run($currentTick)) {
                    unset($this->tasks[$id]);
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
            try {
                $server = \pocketmine\Server::getInstance();
                if (method_exists($server, 'getTick')) {
                    return (int)$server->getTick();
                }
            } catch (\Throwable) {
                // No server yet.
            }
            return 0;
        }
    }
}
