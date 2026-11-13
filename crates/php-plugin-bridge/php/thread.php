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

    if (class_exists(ThreadSafe::class)) {
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
            public function register(bool $prepend = false): bool
            {
                return true;
            }
        }
    } else {
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
            public function register(bool $prepend = false): bool
            {
                return true;
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
        }

        public function registerClassLoaders(): void
        {
            if ($this->composerAutoloaderPath !== null && is_file($this->composerAutoloaderPath)) {
                require_once $this->composerAutoloaderPath;
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
            $this->crashInfo = ThreadCrashInfo::fromThrowable($e, $this->getThreadName());
            fwrite(STDERR, '[Thread/' . $this->getThreadName() . '] Uncaught exception: ' . $e->getMessage() . PHP_EOL);
        }

        abstract protected function onRun(): void;

        public function getThreadName(): string
        {
            return (new \ReflectionClass($this))->getShortName();
        }
    }

    if (class_exists(NativeThread::class)) {
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

    if (class_exists(NativeWorker::class)) {
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
