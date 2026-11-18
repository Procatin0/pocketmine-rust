<?php

declare(strict_types=1);

namespace pocketmine\utils {

    /**
     * PM `CloningRegistryTrait`: a static, lazily-populated name => prototype map
     * where every access returns a clone, so callers can mutate the result
     * without touching the registry (`Registry::OUTLINE_BLOCK()` in EasyEdit
     * relies on that).
     *
     * The using class registers entries with `_registryRegister()` from its
     * `setup()` override; `setup()` runs once, on the first static access.
     *
     * @internal
     */
    trait CloningRegistryTrait
    {
        /** @var object[] */
        private static array $registry = [];

        private static bool $setupDone = false;

        private static function _registryRegister(string $name, object $object): void
        {
            if (isset(self::$registry[$name])) {
                throw new \InvalidArgumentException('Trying to register already registered ' . $name);
            }
            self::$registry[$name] = $object;
        }

        /** @return object[] */
        private static function _registryGetAll(): array
        {
            self::setupOnce();
            return self::$registry;
        }

        protected static function setup(): void
        {
        }

        private static function setupOnce(): void
        {
            if (!self::$setupDone) {
                self::$setupDone = true;
                self::setup();
            }
        }

        final public static function __callStatic(string $name, array $args): object
        {
            self::setupOnce();
            $name = strtolower($name);
            if (!isset(self::$registry[$name])) {
                throw new \InvalidArgumentException('No such ' . static::class . ' "' . $name . '"');
            }
            return clone self::$registry[$name];
        }
    }

    /**
     * PM `RegistryTrait`: same registry, but returns the stored instance instead
     * of a clone (used for immutable value objects / enum-like registries).
     *
     * @internal
     */
    trait RegistryTrait
    {
        /** @var object[] */
        private static array $registry = [];

        private static bool $setupDone = false;

        private static function _registryRegister(string $name, object $object): void
        {
            if (isset(self::$registry[$name])) {
                throw new \InvalidArgumentException('Trying to register already registered ' . $name);
            }
            self::$registry[$name] = $object;
        }

        /** @return object[] */
        private static function _registryGetAll(): array
        {
            self::setupOnce();
            return self::$registry;
        }

        protected static function setup(): void
        {
        }

        private static function setupOnce(): void
        {
            if (!self::$setupDone) {
                self::$setupDone = true;
                self::setup();
            }
        }

        final public static function __callStatic(string $name, array $args): object
        {
            self::setupOnce();
            $name = strtolower($name);
            if (!isset(self::$registry[$name])) {
                throw new \InvalidArgumentException('No such ' . static::class . ' "' . $name . '"');
            }
            return self::$registry[$name];
        }
    }

    /**
     * Offers a shim to make a native enum behave similarly to a legacy EnumTrait enum.
     */
    trait LegacyEnumShimTrait
    {
        public static function __callStatic(string $name, array $arguments): self
        {
            if (count($arguments) > 0) {
                throw new \ArgumentCountError("Expected exactly 0 arguments, " . count($arguments) . " passed");
            }
            $all = self::getAll();
            $key = strtoupper($name);
            if (!isset($all[$key])) {
                throw new \InvalidArgumentException("No such case " . $name);
            }
            return $all[$key];
        }

        public static function getAll(): array
        {
            static $result = null;
            if ($result === null) {
                $result = [];
                foreach (self::cases() as $case) {
                    $result[strtoupper($case->name)] = $case;
                }
            }
            return $result;
        }
    }

    if (!class_exists(RegistrySource::class)) {
        /**
         * @internal
         * @template TMember of object
         */
        abstract class RegistrySource
        {
            private static ?string $inSetupClass = null;
            /** @var array<string, mixed> */
            protected array $simpleMembers = [];
            /** @var array<string, \Closure> */
            protected array $delayedMembers = [];
            private bool $setupDone = false;

            final public function __construct() {}

            abstract public function getTargetClassName(): string;

            public function getTargetClassDocComment(): array { return []; }

            public function cloneResults(): bool { return false; }

            private function setupWrapper(): void
            {
                if ($this->setupDone) {
                    return;
                }
                $this->setupDone = true;
                self::$inSetupClass = static::class;
                try {
                    $this->setup();
                } finally {
                    self::$inSetupClass = null;
                }
            }

            abstract protected function setup(): void;

            public static function preprocessMember(object $member): object
            {
                return $member;
            }

            protected function registerValue(string $name, mixed $value): mixed
            {
                if (isset($this->simpleMembers[$name]) || isset($this->delayedMembers[$name])) {
                    throw new \InvalidArgumentException("Cannot redeclare registry member \"$name\"");
                }
                $this->simpleMembers[$name] = $value;
                return $value;
            }

            protected function registerDelayed(string $name, \Closure $valueFactory): void
            {
                if (isset($this->simpleMembers[$name]) || isset($this->delayedMembers[$name])) {
                    throw new \InvalidArgumentException("Cannot redeclare registry member \"$name\"");
                }
                $this->delayedMembers[$name] = $valueFactory;
            }

            public function getAllValues(): iterable
            {
                $this->setupWrapper();
                yield from $this->simpleMembers;
                foreach ($this->delayedMembers as $name => $callback) {
                    yield $name => $callback($name);
                }
            }

            public function getAll(): array
            {
                $result = [];
                foreach ($this->getAllValues() as $name => $val) {
                    $result[$name] = $val;
                }
                return $result;
            }
        }
    }
}
