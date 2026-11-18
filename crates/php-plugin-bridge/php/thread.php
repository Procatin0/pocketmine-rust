<?php

declare(strict_types=1);

namespace {
    if (!interface_exists('Logger')) {
        interface Logger
        {
            public function emergency($message);
            public function alert($message);
            public function critical($message);
            public function error($message);
            public function warning($message);
            public function notice($message);
            public function info($message);
            public function debug($message);
            public function log($level, $message);
        }
    }
}

namespace pocketmine\thread\log {
    use pmmp\thread\ThreadSafe;
    use pmmp\thread\ThreadSafeArray;

    if (class_exists(ThreadSafe::class)) {
        abstract class ThreadSafeLogger extends ThreadSafe implements \Logger
        {
        }

        abstract class ThreadSafeLoggerAttachment extends ThreadSafe
        {
            abstract public function log(mixed $level, string $message): void;
        }

        abstract class AttachableThreadSafeLogger extends ThreadSafeLogger
        {
            /** @var ThreadSafeArray<int, ThreadSafeLoggerAttachment> */
            protected ThreadSafeArray $attachments;

            public function __construct()
            {
                $this->attachments = new ThreadSafeArray();
            }

            public function addAttachment(ThreadSafeLoggerAttachment $attachment): void
            {
                $this->attachments[] = $attachment;
            }

            public function removeAttachment(ThreadSafeLoggerAttachment $attachment): void
            {
                foreach ($this->attachments as $i => $a) {
                    if ($attachment === $a) {
                        unset($this->attachments[$i]);
                    }
                }
            }

            public function removeAttachments(): void
            {
                foreach ($this->attachments as $i => $a) {
                    unset($this->attachments[$i]);
                }
            }

            /** @return ThreadSafeLoggerAttachment[] */
            public function getAttachments(): array
            {
                $res = [];
                foreach ($this->attachments as $a) {
                    $res[] = $a;
                }
                return $res;
            }
        }
    } else {
        abstract class ThreadSafeLogger implements \Logger
        {
        }

        abstract class ThreadSafeLoggerAttachment
        {
            abstract public function log(mixed $level, string $message): void;
        }

        abstract class AttachableThreadSafeLogger extends ThreadSafeLogger
        {
            /** @var ThreadSafeLoggerAttachment[] */
            protected array $attachments = [];

            public function addAttachment(ThreadSafeLoggerAttachment $attachment): void
            {
                $this->attachments[] = $attachment;
            }

            public function removeAttachment(ThreadSafeLoggerAttachment $attachment): void
            {
                $idx = array_search($attachment, $this->attachments, true);
                if ($idx !== false) {
                    unset($this->attachments[$idx]);
                }
            }

            public function removeAttachments(): void
            {
                $this->attachments = [];
            }

            /** @return ThreadSafeLoggerAttachment[] */
            public function getAttachments(): array
            {
                return array_values($this->attachments);
            }
        }
    }
}

namespace pocketmine\thread {
    use pmmp\thread\Thread as NativeThread;
    use pmmp\thread\ThreadSafe;
    use pmmp\thread\ThreadSafeArray;
    use pmmp\thread\Worker as NativeWorker;

    if (class_exists(ThreadSafe::class)) {
        class ThreadCrashInfoFrame extends ThreadSafe
        {
            public function __construct(
                private string $printableFrame
            ) {
            }

            public function getPrintableFrame(): string
            {
                return $this->printableFrame;
            }
        }

        class ThreadCrashInfo extends ThreadSafe
        {
            /** @var ThreadSafeArray<int, ThreadCrashInfoFrame> */
            private ThreadSafeArray $trace;

            public function __construct(
                private string $type,
                private string $message,
                private string $file,
                private int $line,
                array $trace,
                private string $threadName
            ) {
                $this->trace = new ThreadSafeArray();
                foreach ($trace as $f) {
                    $this->trace[] = $f instanceof ThreadCrashInfoFrame ? $f : new ThreadCrashInfoFrame((string)$f);
                }
            }

            public static function fromThrowable(\Throwable $e, string $threadName): self
            {
                $trace = [];
                foreach ($e->getTrace() as $f) {
                    $location = ($f['file'] ?? '?') . ':' . ($f['line'] ?? '?');
                    $function = ($f['class'] ?? '') . ($f['type'] ?? '') . ($f['function'] ?? '');
                    $trace[] = new ThreadCrashInfoFrame($function . ' at ' . $location);
                }
                return new self($e::class, $e->getMessage(), $e->getFile(), $e->getLine(), $trace, $threadName);
            }

            public function getType(): string
            {
                return $this->type;
            }

            public function getMessage(): string
            {
                return $this->message;
            }

            public function getFile(): string
            {
                return $this->file;
            }

            public function getLine(): int
            {
                return $this->line;
            }

            /** @return ThreadCrashInfoFrame[] */
            public function getTrace(): array
            {
                $res = [];
                foreach ($this->trace as $f) {
                    $res[] = $f;
                }
                return $res;
            }

            public function getThreadName(): string
            {
                return $this->threadName;
            }

            public function makePrettyMessage(): string
            {
                return sprintf('%s: "%s" in %s on line %d', $this->type, $this->message, $this->file, $this->line);
            }
        }

        final class NonThreadSafeValue extends ThreadSafe
        {
            private string $serialized;

            public function __construct(mixed $value)
            {
                $this->serialized = function_exists('igbinary_serialize')
                    ? (string)igbinary_serialize($value)
                    : serialize($value);
            }

            public function deserialize(): mixed
            {
                return function_exists('igbinary_unserialize')
                    ? igbinary_unserialize($this->serialized)
                    : unserialize($this->serialized);
            }
        }

        class ThreadSafeClassLoader extends ThreadSafe
        {
            /** @var ThreadSafeArray<int, string> */
            protected ThreadSafeArray $paths;

            public function __construct()
            {
                $this->paths = new ThreadSafeArray();
            }

            public function addPath(string $prefix, string $path, bool $prepend = false): void
            {
                $cleanPrefix = trim(str_replace('/', '\\', $prefix), '\\');
                $cleanPath = rtrim(str_replace('\\', '/', $path), '/');
                $entry = $cleanPrefix . "\0" . $cleanPath;
                $this->paths[] = $entry;
                if (class_exists(\pocketmine\runtime\PluginAutoloader::class, false)) {
                    \pocketmine\runtime\PluginAutoloader::addPath($cleanPrefix, $cleanPath);
                }
            }

            public function removePath(string $prefix, string $path): void
            {
                $cleanPrefix = trim(str_replace('/', '\\', $prefix), '\\');
                $cleanPath = rtrim(str_replace('\\', '/', $path), '/');
                $target = $cleanPrefix . "\0" . $cleanPath;
                foreach ($this->paths as $i => $entry) {
                    if ($entry === $target) {
                        unset($this->paths[$i]);
                    }
                }
            }

            /** @return array<string, string[]> */
            public function getPrefixes(): array
            {
                $prefixes = [];
                foreach ($this->paths as $entry) {
                    [$prefix, $path] = explode("\0", (string)$entry, 2);
                    $prefixes[$prefix][] = $path;
                }
                return $prefixes;
            }

            public function loadClass(string $class): bool
            {
                $class = ltrim($class, '\\');
                if ($class === '') {
                    return false;
                }

                foreach ($this->paths as $entry) {
                    [$prefix, $path] = explode("\0", (string)$entry, 2);
                    if ($prefix === '' || stripos($class, $prefix . '\\') === 0 || stripos($class, $prefix) === 0) {
                        $relative = $prefix === '' ? $class : substr($class, strlen($prefix));
                        $candidate = $path . '/' . str_replace('\\', '/', ltrim($relative, '\\')) . '.php';
                        if (is_file($candidate)) {
                            require_once $candidate;
                            return true;
                        }
                    }
                }

                if (class_exists(\pocketmine\runtime\PluginAutoloader::class, false)) {
                    \pocketmine\runtime\PluginAutoloader::autoload($class);
                    if (class_exists($class, false) || interface_exists($class, false) || trait_exists($class, false)) {
                        return true;
                    }
                }

                return false;
            }

            public function register(bool $prepend = false): bool
            {
                return spl_autoload_register([$this, 'loadClass'], true, $prepend);
            }
        }
    } else {
        class ThreadCrashInfoFrame
        {
            public function __construct(
                private string $printableFrame
            ) {
            }

            public function getPrintableFrame(): string
            {
                return $this->printableFrame;
            }
        }

        class ThreadCrashInfo
        {
            public function __construct(
                private string $type,
                private string $message,
                private string $file,
                private int $line,
                private array $trace,
                private string $threadName
            ) {
            }

            public static function fromThrowable(\Throwable $e, string $threadName): self
            {
                return new self($e::class, $e->getMessage(), $e->getFile(), $e->getLine(), $e->getTrace(), $threadName);
            }

            public function getType(): string
            {
                return $this->type;
            }

            public function getMessage(): string
            {
                return $this->message;
            }

            public function getFile(): string
            {
                return $this->file;
            }

            public function getLine(): int
            {
                return $this->line;
            }

            public function getTrace(): array
            {
                return $this->trace;
            }

            public function getThreadName(): string
            {
                return $this->threadName;
            }

            public function makePrettyMessage(): string
            {
                return sprintf('%s: "%s" in %s on line %d', $this->type, $this->message, $this->file, $this->line);
            }
        }

        final class NonThreadSafeValue
        {
            private string $serialized;

            public function __construct(mixed $value)
            {
                $this->serialized = serialize($value);
            }

            public function deserialize(): mixed
            {
                return unserialize($this->serialized);
            }
        }

        class ThreadSafeClassLoader
        {
            /** @var string[] */
            protected array $paths = [];

            public function addPath(string $prefix, string $path, bool $prepend = false): void
            {
                $cleanPrefix = trim(str_replace('/', '\\', $prefix), '\\');
                $cleanPath = rtrim(str_replace('\\', '/', $path), '/');
                $entry = $cleanPrefix . "\0" . $cleanPath;
                if ($prepend) {
                    array_unshift($this->paths, $entry);
                } else {
                    $this->paths[] = $entry;
                }
                if (class_exists(\pocketmine\runtime\PluginAutoloader::class, false)) {
                    \pocketmine\runtime\PluginAutoloader::addPath($cleanPrefix, $cleanPath);
                }
            }

            public function removePath(string $prefix, string $path): void
            {
                $cleanPrefix = trim(str_replace('/', '\\', $prefix), '\\');
                $cleanPath = rtrim(str_replace('\\', '/', $path), '/');
                $target = $cleanPrefix . "\0" . $cleanPath;
                $idx = array_search($target, $this->paths, true);
                if ($idx !== false) {
                    unset($this->paths[$idx]);
                }
            }

            /** @return array<string, string[]> */
            public function getPrefixes(): array
            {
                $prefixes = [];
                foreach ($this->paths as $entry) {
                    [$prefix, $path] = explode("\0", $entry, 2);
                    $prefixes[$prefix][] = $path;
                }
                return $prefixes;
            }

            public function loadClass(string $class): bool
            {
                $class = ltrim($class, '\\');
                if ($class === '') {
                    return false;
                }

                foreach ($this->paths as $entry) {
                    [$prefix, $path] = explode("\0", $entry, 2);
                    if ($prefix === '' || stripos($class, $prefix . '\\') === 0 || stripos($class, $prefix) === 0) {
                        $relative = $prefix === '' ? $class : substr($class, strlen($prefix));
                        $candidate = $path . '/' . str_replace('\\', '/', ltrim($relative, '\\')) . '.php';
                        if (is_file($candidate)) {
                            require_once $candidate;
                            return true;
                        }
                    }
                }

                if (class_exists(\pocketmine\runtime\PluginAutoloader::class, false)) {
                    \pocketmine\runtime\PluginAutoloader::autoload($class);
                    if (class_exists($class, false) || interface_exists($class, false) || trait_exists($class, false)) {
                        return true;
                    }
                }

                return false;
            }

            public function register(bool $prepend = false): bool
            {
                return spl_autoload_register([$this, 'loadClass'], true, $prepend);
            }
        }
    }

    trait CommonThreadPartsTrait
    {
        protected bool $isKilled = false;
        private mixed $classLoaders = null;
        protected ?string $composerAutoloaderPath = null;
        private ?ThreadCrashInfo $crashInfo = null;

        public function getClassLoaders(): ?array
        {
            if ($this->classLoaders === null) {
                return null;
            }
            if (is_array($this->classLoaders)) {
                return $this->classLoaders;
            }
            $res = [];
            foreach ($this->classLoaders as $l) {
                $res[] = $l;
            }
            return $res;
        }

        public function setClassLoaders(?array $autoloaders = null): void
        {
            if (class_exists(ThreadSafeArray::class)) {
                $this->classLoaders = new ThreadSafeArray();
            } else {
                $this->classLoaders = [];
            }
            if ($autoloaders !== null) {
                foreach ($autoloaders as $loader) {
                    $this->classLoaders[] = $loader;
                }
            } elseif (class_exists(\pocketmine\Server::class, false)) {
                $serverLoader = \pocketmine\Server::getInstance()->getLoader();
                if ($serverLoader !== null) {
                    $this->classLoaders[] = $serverLoader;
                }
            }
        }

        public function registerClassLoaders(): void
        {
            $loaders = $this->getClassLoaders();
            if ($loaders !== null) {
                foreach ($loaders as $loader) {
                    if (is_object($loader) && method_exists($loader, 'register')) {
                        $loader->register(true);
                    }
                }
            }
            if ($this->composerAutoloaderPath !== null && is_file($this->composerAutoloaderPath)) {
                require_once $this->composerAutoloaderPath;
            }
        }

        /**
         * pthreads threads start with a fresh interpreter: classes, functions,
         * constants, globals and autoloaders declared by the parent are not
         * visible there (only the running thread class itself is copied). PM
         * gets away with this because every PM class lives in its own file and
         * is autoloaded on demand by the class loaders inherited by the thread;
         * this bridge ships its API as a handful of monolithic files that are
         * `require`d once, so the thread has to require them again.
         *
         * Without this, threaded plugins (EasyEdit, for example) lose the whole
         * API in their thread and even the crash reporter below fails with
         * `Class "pocketmine\thread\ThreadCrashInfo" not found`.
         */
        private function loadRuntimeForThread(): void
        {
            if (class_exists(ThreadCrashInfo::class, false) && class_exists(\pocketmine\Server::class, false)) {
                // Same interpreter as the parent (non-pthreads fallback): already loaded.
                return;
            }

            $manifest = __DIR__ . DIRECTORY_SEPARATOR . 'runtime_files.php';
            if (!is_file($manifest)) {
                return;
            }

            $files = require $manifest;
            foreach ($files as $file) {
                $path = __DIR__ . DIRECTORY_SEPARATOR . $file;
                if (is_file($path)) {
                    require $path;
                }
            }

            $this->registerPluginAutoloadPaths();
        }

        /**
         * The plugin class loaders are `ThreadSafe`, so the thread can still
         * read their registered paths, but the thread-local `PluginAutoloader`
         * registry is empty: mirror the paths into it so plugin classes (and
         * their `src-namespace-prefix` layouts) resolve inside the thread.
         */
        private function registerPluginAutoloadPaths(): void
        {
            if (!class_exists(\pocketmine\runtime\PluginAutoloader::class, false)) {
                return;
            }

            foreach ($this->getClassLoaders() ?? [] as $loader) {
                if (!is_object($loader) || !method_exists($loader, 'getPrefixes')) {
                    continue;
                }
                foreach ($loader->getPrefixes() as $prefix => $paths) {
                    foreach ($paths as $path) {
                        \pocketmine\runtime\PluginAutoloader::addPath((string)$prefix, (string)$path);
                    }
                }
            }
        }

        public function getCrashInfo(): ?ThreadCrashInfo
        {
            if (method_exists($this, 'isTerminated') && method_exists($this, 'isJoined')) {
                if ($this->isTerminated() && !$this->isJoined()) {
                    $this->join();
                }
            }
            return $this->crashInfo;
        }

        public function start(int $options = 0): bool
        {
            $this->isKilled = false;
            ThreadManager::getInstance()->add($this);

            if ($this->getClassLoaders() === null) {
                $this->setClassLoaders();
            }

            if (is_subclass_of($this, NativeThread::class)) {
                return parent::start($options);
            }

            // Fallback for non-ZTS environments: run synchronously
            $this->run();
            return true;
        }

        final public function run(): void
        {
            error_reporting(-1);
            // Child threads get a fresh interpreter: without this, PHP warnings
            // go to stdout, which is the JSON IPC channel of the worker.
            @ini_set('display_errors', 'stderr');
            $this->loadRuntimeForThread();
            $this->registerClassLoaders();

            try {
                $this->onRun();
            } catch (\Throwable $e) {
                $this->onUncaughtException($e);
            } finally {
                $this->isKilled = true;
            }
        }

        public function quit(): void
        {
            $this->isKilled = true;
            if (method_exists($this, 'isJoined') && !$this->isJoined()) {
                if (method_exists($this, 'notify')) {
                    $this->notify();
                }
                if (method_exists($this, 'join')) {
                    $this->join();
                }
            }
            ThreadManager::getInstance()->remove($this);
        }

        protected function onUncaughtException(\Throwable $e): void
        {
            if (class_exists(ThreadCrashInfo::class, false)) {
                $this->crashInfo = ThreadCrashInfo::fromThrowable($e, $this->getThreadName());
            }
            // `STDERR` is defined by the CLI SAPI and is not inherited by
            // pthreads threads, so open the stream explicitly.
            $stderr = fopen('php://stderr', 'wb');
            if ($stderr !== false) {
                fwrite($stderr, '[Thread/' . $this->getThreadName() . '] Uncaught exception: ' . $e::class . ': ' . $e->getMessage() . ' in ' . $e->getFile() . ':' . $e->getLine() . PHP_EOL);
                fwrite($stderr, $e->getTraceAsString() . PHP_EOL);
                fclose($stderr);
            }
        }

        abstract protected function onRun(): void;

        public function getThreadName(): string
        {
            return (new \ReflectionClass($this))->getShortName();
        }
    }

    if (class_exists(Thread::class, false)) {
        // pthreads copies the running thread's class into the new thread, so
        // re-including this file inside a thread (see
        // CommonThreadPartsTrait::loadRuntimeForThread()) must not redeclare it.
    } elseif (class_exists(NativeThread::class)) {
        abstract class Thread extends NativeThread
        {
            use CommonThreadPartsTrait;
        }
    } else {
        abstract class Thread
        {
            use CommonThreadPartsTrait;

            public function synchronized(\Closure $closure): mixed
            {
                return $closure();
            }

            public function wait(): void
            {
            }

            public function notify(): void
            {
            }

            public function notifyOne(): void
            {
            }

            public function isJoined(): bool
            {
                return true;
            }

            public function isStarted(): bool
            {
                return true;
            }

            public function isRunning(): bool
            {
                return false;
            }

            public function isTerminated(): bool
            {
                return $this->isKilled;
            }

            public static function getCurrentThread(): ?self
            {
                return null;
            }

            public static function getCurrentThreadId(): int
            {
                return 0;
            }
        }
    }

    if (class_exists(Worker::class, false)) {
        // Already declared in this thread context (pthreads class copy).
    } elseif (class_exists(NativeWorker::class)) {
        abstract class Worker extends NativeWorker
        {
            use CommonThreadPartsTrait;
        }
    } else {
        abstract class Worker extends Thread
        {
        }
    }

    class ThreadManager
    {
        private static ?self $instance = null;
        /** @var array<int, object> */
        private array $threads = [];

        public static function init(): void
        {
            self::$instance = new self();
        }

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
        }

        public function add(object $thread): void
        {
            $this->threads[spl_object_id($thread)] = $thread;
        }

        public function remove(object $thread): void
        {
            unset($this->threads[spl_object_id($thread)]);
        }

        /** @return object[] */
        public function getAll(): array
        {
            return array_values($this->threads);
        }

        public function stopAll(): int
        {
            $count = 0;
            foreach ($this->threads as $thread) {
                if (method_exists($thread, 'quit')) {
                    try {
                        $thread->quit();
                        $count++;
                    } catch (\Throwable) {
                    }
                }
            }
            $this->threads = [];
            return $count;
        }
    }
}

namespace pocketmine\snooze {
    use pmmp\thread\ThreadSafe;
    use pmmp\thread\ThreadSafeArray;

    if (class_exists(ThreadSafe::class)) {
        class SleeperNotifier extends ThreadSafe
        {
            protected ?ThreadSafeArray $sharedQueue = null;
            protected int $notifierId = 0;

            public function setHandler(ThreadSafeArray $queue, int $id): void
            {
                $this->sharedQueue = $queue;
                $this->notifierId = $id;
            }

            public function wakeupSleeper(): void
            {
                if ($this->sharedQueue !== null) {
                    $this->sharedQueue[] = $this->notifierId;
                }
            }
        }
    } else {
        class SleeperNotifier
        {
            /** @var array<int, int>|null */
            protected ?array $sharedQueue = null;
            protected int $notifierId = 0;

            public function setHandler(array &$queue, int $id): void
            {
                $this->sharedQueue = &$queue;
                $this->notifierId = $id;
            }

            public function wakeupSleeper(): void
            {
                if ($this->sharedQueue !== null) {
                    $this->sharedQueue[] = $this->notifierId;
                }
            }
        }
    }

    class SleeperHandler
    {
        private mixed $queue;
        /** @var array<int, \Closure> */
        private array $notifiers = [];
        private int $nextId = 1;

        public function __construct()
        {
            if (class_exists(ThreadSafeArray::class)) {
                $this->queue = new ThreadSafeArray();
            } else {
                $this->queue = [];
            }
        }

        public function addNotifier(SleeperNotifier $notifier, \Closure $handler): int
        {
            $id = $this->nextId++;
            $this->notifiers[$id] = $handler;
            $this->attachNotifier($notifier, $id);
            return $id;
        }

        private function attachNotifier(SleeperNotifier $notifier, int $id): void
        {
            if ($this->queue instanceof ThreadSafeArray) {
                $notifier->setHandler($this->queue, $id);
            } else {
                $notifier->setHandler($this->queue, $id);
            }
        }

        public function removeNotifier(int|SleeperNotifier $notifier): void
        {
            if (is_int($notifier)) {
                unset($this->notifiers[$notifier]);
            }
        }

        public function processNotifications(): void
        {
            if (class_exists(ThreadSafeArray::class) && $this->queue instanceof ThreadSafeArray) {
                while (($id = $this->queue->shift()) !== null) {
                    $intId = (int)$id;
                    if (isset($this->notifiers[$intId])) {
                        try {
                            ($this->notifiers[$intId])();
                        } catch (\Throwable $e) {
                            fwrite(STDERR, '[PHP/SNOOZE] Sleeper notification callback failed: ' . $e->getMessage() . PHP_EOL);
                        }
                    }
                }
            } else {
                while (($id = array_shift($this->queue)) !== null) {
                    $intId = (int)$id;
                    if (isset($this->notifiers[$intId])) {
                        try {
                            ($this->notifiers[$intId])();
                        } catch (\Throwable $e) {
                            fwrite(STDERR, '[PHP/SNOOZE] Sleeper notification callback failed: ' . $e->getMessage() . PHP_EOL);
                        }
                    }
                }
            }
        }

        public function sleepUntilNotification(float $timeout): void
        {
            $this->processNotifications();
        }
    }
}
