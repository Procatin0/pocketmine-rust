<?php
declare(strict_types=1);

namespace pocketmine\plugin {
    use pocketmine\event\Event;
    use pocketmine\event\EventFactory;
    use pocketmine\event\EventPriority;
    use pocketmine\event\Listener;
    use pocketmine\event\RegisteredListener;

    interface Plugin
    {
        public function getName(): string;

        public function getDescription(): PluginDescription;

        public function getDataFolder(): string;

        public function getFile(): string;

        public function isEnabled(): bool;

        public function isDisabled(): bool;

        public function getServer();

        public function getLogger();

        public function onLoad(): void;

        public function onEnable(): void;

        public function onDisable(): void;

        public function getPluginManager(): PluginManager;

        public function getScheduler();

        public function getCommand(string $name);

        /** @return array<string, object> */
        public function getCommands(): array;

        public function saveResource(string $filename, bool $replace = false): void;

        public function getResource(string $filename);

        public function getConfig();

        public function saveConfig(): void;

        public function reloadConfig(): void;

        public function saveDefaultConfig(): void;

        public function registerEvents(Listener $listener, ?Plugin $plugin = null): void;
    }

    class PluginDescription
    {
        private string $name = '';
        private string $version = '';
        private string $main = '';
        private string $description = '';
        /** @var string[] */
        private array $authors = [];
        /** @var string[] */
        private array $api = [];
        private ?string $website = null;
        /** @var string[] */
        private array $depend = [];
        /** @var string[] */
        private array $softDepend = [];
        /** @var string[] */
        private array $loadBefore = [];
        private array $commands = [];
        private array $permissions = [];

        public static function fromArray(array $data): PluginDescription
        {
            $description = new self();
            $description->name = self::stringValue($data['name'] ?? '');
            $description->version = self::stringValue($data['version'] ?? '');
            $description->main = self::stringValue($data['main'] ?? '');
            $description->description = self::stringValue($data['description'] ?? '');
            $description->authors = self::stringList($data['authors'] ?? $data['author'] ?? []);
            $description->api = self::stringList($data['api'] ?? []);
            $website = $data['website'] ?? null;
            $description->website = $website === null || $website === '' ? null : self::stringValue($website);
            $description->depend = self::stringList($data['depend'] ?? []);
            $description->softDepend = self::stringList($data['softdepend'] ?? $data['softDepend'] ?? []);
            $description->loadBefore = self::stringList(
                $data['load-before'] ?? $data['loadbefore'] ?? $data['loadBefore'] ?? []
            );
            $description->commands = is_array($data['commands'] ?? null) ? $data['commands'] : [];
            $description->permissions = is_array($data['permissions'] ?? null) ? $data['permissions'] : [];
            return $description;
        }

        public function getName(): string
        {
            return $this->name;
        }

        public function getVersion(): string
        {
            return $this->version;
        }

        public function getMain(): string
        {
            return $this->main;
        }

        public function getDescription(): string
        {
            return $this->description;
        }

        /** @return string[] */
        public function getAuthors(): array
        {
            return $this->authors;
        }

        public function getAuthor(): ?string
        {
            return $this->authors[0] ?? null;
        }

        /** @return string[] */
        public function getApi(): array
        {
            return $this->api;
        }

        public function getWebsite(): ?string
        {
            return $this->website;
        }

        /** @return string[] */
        public function getDepend(): array
        {
            return $this->depend;
        }

        /** @return string[] */
        public function getSoftDepend(): array
        {
            return $this->softDepend;
        }

        /** @return string[] */
        public function getLoadBefore(): array
        {
            return $this->loadBefore;
        }

        public function getCommands(): array
        {
            return $this->commands;
        }

        public function getPermissions(): array
        {
            return $this->permissions;
        }

        public function getFullName(): string
        {
            return $this->name . ($this->version !== '' ? ' v' . $this->version : '');
        }

        public function __toString(): string
        {
            return $this->getFullName();
        }

        /** @return string[] */
        private static function stringList(mixed $value): array
        {
            if ($value === null || $value === '') {
                return [];
            }
            if (!is_array($value)) {
                $value = [$value];
            }
            $out = [];
            foreach ($value as $item) {
                if (is_scalar($item)) {
                    $out[] = (string)$item;
                }
            }
            return $out;
        }

        private static function stringValue(mixed $value): string
        {
            return is_scalar($value) ? (string)$value : '';
        }
    }
}

namespace pocketmine\plugin {
    use pocketmine\event\Listener;

    abstract class PluginBase implements Plugin
    {
        public string $name = '';
        public mixed $description = null;
        public string $dataFolder = '';
        public string $file = '';
        public bool $enabled = false;
        public mixed $server = null;
        /** @var array<string, object> */
        public array $commands = [];
        public mixed $config = null;

        protected string $root = '';
        protected mixed $logger = null;

        public function init(
            PluginDescription $description,
            string $dataFolder,
            string $file,
            string $root,
            \pocketmine\Server $server
        ): void {
            $this->description = $description;
            if ($description->getName() !== '') {
                $this->name = $description->getName();
            }
            $this->dataFolder = rtrim($dataFolder, '/\\') . DIRECTORY_SEPARATOR;
            if (!is_dir($this->dataFolder)) {
                @mkdir($this->dataFolder, 0777, true);
            }
            $this->file = $file;
            $this->root = rtrim($root, '/\\');
            $this->server = $server;
        }

        public function getName(): string
        {
            if ($this->name !== '') {
                return $this->name;
            }
            if ($this->description instanceof PluginDescription && $this->description->getName() !== '') {
                return $this->description->getName();
            }
            $class = static::class;
            $pos = strrpos($class, '\\');
            return $pos === false ? $class : substr($class, $pos + 1);
        }

        public function getDescription(): PluginDescription
        {
            if ($this->description instanceof PluginDescription) {
                return $this->description;
            }
            $this->description = PluginDescription::fromArray(['name' => $this->getName()]);
            return $this->description;
        }

        public function getFullName(): string
        {
            return $this->getDescription()->getFullName();
        }

        public function getDataFolder(): string
        {
            if ($this->dataFolder === '') {
                $this->dataFolder = rtrim(getcwd() . DIRECTORY_SEPARATOR . 'plugin_data' . DIRECTORY_SEPARATOR . $this->getName(), '/\\') . DIRECTORY_SEPARATOR;
            }
            $folder = rtrim($this->dataFolder, '/\\') . DIRECTORY_SEPARATOR;
            if (!is_dir($folder)) {
                @mkdir($folder, 0777, true);
            }
            $this->dataFolder = $folder;
            return $folder;
        }

        public function getFile(): string
        {
            return $this->file;
        }

        public function getResource(string $filename)
        {
            $relative = $this->sanitizeResourceName($filename);
            if ($relative === null) {
                return false;
            }

            $root = $this->root;
            if ($root === '') {
                $root = dirname($this->file);
            }

            foreach ([$root . '/' . $relative, $root . '/resources/' . $relative] as $candidate) {
                $resolved = $this->resolvePluginFile($candidate);
                if ($resolved !== null) {
                    $handle = @fopen($resolved, 'rb');
                    if ($handle !== false) {
                        return $handle;
                    }
                }
            }
            return false;
        }

        public function saveResource(string $filename, bool $replace = false): void
        {
            $relative = $this->sanitizeResourceName($filename);
            if ($relative === null) {
                $this->logWarning('Invalid resource path: ' . $filename);
                return;
            }

            $target = $this->getDataFolder() . str_replace('/', DIRECTORY_SEPARATOR, $relative);
            if (is_file($target) && !$replace) {
                return;
            }

            $resource = $this->getResource($relative);
            if ($resource === false) {
                $this->logWarning('Resource not found: ' . $filename);
                return;
            }

            $contents = stream_get_contents($resource);
            fclose($resource);
            if ($contents === false) {
                $this->logWarning('Unable to read resource: ' . $filename);
                return;
            }

            $directory = dirname($target);
            if (!is_dir($directory)) {
                @mkdir($directory, 0777, true);
            }
            if (@file_put_contents($target, $contents) === false) {
                $this->logWarning('Unable to write resource: ' . $target);
            }
        }

        public function getConfig()
        {
            if (is_object($this->config)) {
                return $this->config;
            }

            $path = $this->getDataFolder() . 'config.yml';
            if (!is_file($path) && $this->hasResource('config.yml')) {
                $this->saveDefaultConfig();
            }

            $class = $this->findConfigClass();
            if ($class !== null) {
                try {
                    $this->config = $this->instantiateConfig($class, $path);
                    return $this->config;
                } catch (\Throwable $e) {
                    $this->logWarning('Unable to load config (' . $e->getMessage() . '); using fallback');
                }
            }

            $this->config = new FallbackConfig($path);
            return $this->config;
        }

        public function saveConfig(): void
        {
            $config = $this->config ?? $this->getConfig();
            if (is_object($config) && method_exists($config, 'save')) {
                try {
                    $config->save();
                } catch (\Throwable $e) {
                    $this->logWarning('Unable to save config: ' . $e->getMessage());
                }
            }
        }

        public function reloadConfig(): void
        {
            $config = $this->config;
            if (is_object($config) && method_exists($config, 'reload')) {
                try {
                    $config->reload();
                    return;
                } catch (\Throwable $e) {
                    $this->logWarning('Unable to reload config: ' . $e->getMessage());
                }
            }
            $this->config = null;
            $this->getConfig();
        }

        public function saveDefaultConfig(): void
        {
            if ($this->hasResource('config.yml')) {
                $this->saveResource('config.yml');
            }
        }

        public function getServer()
        {
            if ($this->server === null && class_exists(\pocketmine\Server::class)) {
                try {
                    $this->server = \pocketmine\Server::getInstance();
                } catch (\Throwable) {
                    // Leave null.
                }
            }
            return $this->server;
        }

        public function getLogger()
        {
            if ($this->logger !== null) {
                return $this->logger;
            }

            $server = $this->getServer();
            if (is_object($server) && method_exists($server, 'getLogger')) {
                try {
                    $logger = $server->getLogger();
                    if (is_object($logger)) {
                        return $this->logger = $logger;
                    }
                } catch (\Throwable) {
                    // Fall through to the local logger.
                }
            }

            return $this->logger = new PluginLoggerFallback($this->getFullName());
        }

        public function getPluginManager(): PluginManager
        {
            $server = $this->getServer();
            if (is_object($server) && method_exists($server, 'getPluginManager')) {
                try {
                    $manager = $server->getPluginManager();
                    if ($manager instanceof PluginManager) {
                        return $manager;
                    }
                } catch (\Throwable) {
                    // Fall through to the singleton.
                }
            }
            return PluginManager::getInstance($server instanceof \pocketmine\Server ? $server : null);
        }

        public function getScheduler()
        {
            $server = $this->getServer();
            if (is_object($server) && method_exists($server, 'getScheduler')) {
                try {
                    $scheduler = $server->getScheduler();
                    if (is_object($scheduler)) {
                        return $scheduler;
                    }
                } catch (\Throwable) {
                    // Fall through.
                }
            }

            foreach (['pocketmine\\scheduler\\Scheduler', 'pocketmine\\scheduler\\ServerScheduler', 'pocketmine\\Scheduler'] as $class) {
                if (!class_exists($class)) {
                    continue;
                }
                if (method_exists($class, 'getInstance')) {
                    try {
                        return $class::getInstance();
                    } catch (\Throwable) {
                        // Try a plain constructor next.
                    }
                }
                try {
                    return new $class();
                } catch (\Throwable) {
                    // Try the next class name.
                }
            }
            return null;
        }

        public function isEnabled(): bool
        {
            return $this->enabled;
        }

        public function isDisabled(): bool
        {
            return !$this->enabled;
        }

        public function setEnabled(bool $enabled): void
        {
            $this->enabled = $enabled;
        }

        public function getCommand(string $name)
        {
            $name = strtolower(ltrim(trim($name), '/'));
            return $this->commands[$name] ?? null;
        }

        /** @return array<string, object> */
        public function getCommands(): array
        {
            return $this->commands;
        }

        /** @param array<string, object> $commands */
        public function setCommands(array $commands): void
        {
            $this->commands = [];
            foreach ($commands as $key => $command) {
                $label = is_object($command) && method_exists($command, 'getName')
                    ? (string)$command->getName()
                    : (string)$key;
                $label = strtolower(ltrim(trim($label), '/'));
                if ($label !== '') {
                    $this->commands[$label] = $command;
                }
            }
        }

        public function registerEvents(Listener $listener, ?Plugin $plugin = null): void
        {
            $this->getPluginManager()->registerEvents($listener, $plugin ?? $this);
        }

        public function onLoad(): void
        {
        }

        public function onEnable(): void
        {
        }

        public function onDisable(): void
        {
        }

        protected function sanitizeResourceName(string $filename): ?string
        {
            $filename = trim(str_replace('\\', '/', $filename), '/');
            if ($filename === '' || str_contains($filename, '..') || str_contains($filename, "\0")) {
                return null;
            }
            return $filename;
        }

        protected function resolvePluginFile(string $path): ?string
        {
            if (class_exists(\pocketmine\runtime\PluginAutoloader::class)) {
                return \pocketmine\runtime\PluginAutoloader::resolve($path);
            }
            $path = str_replace('\\', '/', $path);
            return is_file($path) ? $path : null;
        }

        protected function hasResource(string $filename): bool
        {
            $relative = $this->sanitizeResourceName($filename);
            if ($relative === null) {
                return false;
            }
            $root = $this->root !== '' ? $this->root : dirname($this->file);
            foreach ([$root . '/' . $relative, $root . '/resources/' . $relative] as $candidate) {
                if ($this->resolvePluginFile($candidate) !== null) {
                    return true;
                }
            }
            return false;
        }

        protected function findConfigClass(): ?string
        {
            foreach (['pocketmine\\utils\\Config', 'pocketmine\\config\\Config', 'pocketmine\\Config'] as $class) {
                if (class_exists($class)) {
                    return $class;
                }
            }
            return null;
        }

        /** @param class-string $class */
        protected function instantiateConfig(string $class, string $path): object
        {
            $reflection = new \ReflectionClass($class);
            $constructor = $reflection->getConstructor();
            if ($constructor === null) {
                $config = $reflection->newInstance();
                if (method_exists($config, 'load')) {
                    $config->load();
                }
                return $config;
            }

            $args = [];
            foreach ($constructor->getParameters() as $parameter) {
                $name = strtolower($parameter->getName());
                if (in_array($name, ['file', 'path', 'filename'], true)) {
                    $args[] = $path;
                } elseif (in_array($name, ['type', 'format'], true)) {
                    $yaml = defined($class . '::YAML') ? constant($class . '::YAML') : 2;
                    $args[] = $yaml;
                } elseif (in_array($name, ['default', 'defaults'], true)) {
                    $args[] = [];
                } elseif ($parameter->isDefaultValueAvailable()) {
                    $args[] = $parameter->getDefaultValue();
                } else {
                    $args[] = null;
                }
            }
            return $reflection->newInstanceArgs($args);
        }

        private function logWarning(string $message): void
        {
            try {
                $logger = $this->getLogger();
                if (is_object($logger) && method_exists($logger, 'warning')) {
                    $logger->warning($message);
                    return;
                }
            } catch (\Throwable) {
                // Use stderr below.
            }
            fwrite(STDERR, '[PHP/RUNTIME] ' . $message . PHP_EOL);
        }
    }
}

namespace pocketmine\plugin {
    /**
     * Minimal logger used when the core Logger class is unavailable or incomplete.
     */
    final class PluginLoggerFallback
    {
        public function __construct(private string $prefix = 'Plugin')
        {
        }

        public function info(string $message): void
        {
            $this->write('INFO', $message);
        }

        public function notice(string $message): void
        {
            $this->write('NOTICE', $message);
        }

        public function warning(string $message): void
        {
            $this->write('WARN', $message);
        }

        public function error(string $message): void
        {
            $this->write('ERROR', $message);
        }

        public function critical(string $message): void
        {
            $this->write('CRITICAL', $message);
        }

        public function debug(string $message): void
        {
            $this->write('DEBUG', $message);
        }

        public function logException(\Throwable $e, array $trace = []): void
        {
            $this->error($e::class . ': ' . $e->getMessage());
        }

        private function write(string $level, string $message): void
        {
            fwrite(STDERR, '[PHP/PLUGIN] [' . $level . '] [' . $this->prefix . '] ' . $message . PHP_EOL);
        }
    }

    /**
     * Tiny YAML-backed config used only when the core Config class is missing.
     */
    final class FallbackConfig
    {
        private array $data = [];

        public function __construct(private string $path)
        {
            $this->reload();
        }

        public function get(string $key, mixed $default = null): mixed
        {
            $value = $this->data;
            foreach (explode('.', $key) as $part) {
                if (!is_array($value) || !array_key_exists($part, $value)) {
                    return $default;
                }
                $value = $value[$part];
            }
            return $value;
        }

        public function getAll(): array
        {
            return $this->data;
        }

        public function set(string $key, mixed $value): void
        {
            $parts = explode('.', $key);
            $target = &$this->data;
            foreach ($parts as $index => $part) {
                if ($index === count($parts) - 1) {
                    $target[$part] = $value;
                    return;
                }
                if (!isset($target[$part]) || !is_array($target[$part])) {
                    $target[$part] = [];
                }
                $target = &$target[$part];
            }
        }

        public function setAll(array $data): void
        {
            $this->data = $data;
        }

        public function remove(string $key): void
        {
            $parts = explode('.', $key);
            $target = &$this->data;
            foreach ($parts as $index => $part) {
                if (!is_array($target) || !array_key_exists($part, $target)) {
                    return;
                }
                if ($index === count($parts) - 1) {
                    unset($target[$part]);
                    return;
                }
                $target = &$target[$part];
            }
        }

        public function exists(string $key): bool
        {
            $sentinel = new \stdClass();
            return $this->get($key, $sentinel) !== $sentinel;
        }

        public function save(): void
        {
            $directory = dirname($this->path);
            if (!is_dir($directory)) {
                @mkdir($directory, 0777, true);
            }
            if (function_exists('yaml_emit')) {
                $encoded = yaml_emit($this->data, defined('YAML_UTF8_ENCODING') ? YAML_UTF8_ENCODING : 0);
                if (is_string($encoded)) {
                    @file_put_contents($this->path, $encoded);
                }
                return;
            }
            @file_put_contents($this->path, json_encode($this->data, JSON_PRETTY_PRINT));
        }

        public function reload(): void
        {
            if (!is_file($this->path)) {
                $this->data = [];
                return;
            }
            if (function_exists('yaml_parse_file')) {
                $parsed = @yaml_parse_file($this->path);
                $this->data = is_array($parsed) ? $parsed : [];
                return;
            }
            $contents = @file_get_contents($this->path);
            $parsed = is_string($contents) ? json_decode($contents, true) : null;
            $this->data = is_array($parsed) ? $parsed : [];
        }

        public function getPath(): string
        {
            return $this->path;
        }
    }
}

namespace pocketmine\plugin {
    use pocketmine\event\Event;
    use pocketmine\event\EventFactory;
    use pocketmine\event\EventPriority;
    use pocketmine\event\Listener;
    use pocketmine\event\RegisteredListener;

    class PluginManager
    {
        private static ?PluginManager $instance = null;

        /** @var array<string, Plugin> */
        private array $plugins = [];

        /** @var array<string, array<int, RegisteredListener>> */
        private array $listeners = [];

        /** @var array<string, array<string, bool>> */
        private array $registeredKeys = [];

        private mixed $server = null;

        public function __construct(?\pocketmine\Server $server = null)
        {
            $this->server = $server;
        }

        public static function getInstance(?\pocketmine\Server $server = null): self
        {
            if (self::$instance === null) {
                self::$instance = new self($server);
            } elseif ($server !== null && self::$instance->server === null) {
                self::$instance->server = $server;
            }
            return self::$instance;
        }

        public static function useInstance(self $manager, ?\pocketmine\Server $server = null): void
        {
            self::$instance = $manager;
            if ($server !== null) {
                $manager->server = $server;
            }
        }

        public function setServer(mixed $server): void
        {
            $this->server = $server;
        }

        public function getServer(): mixed
        {
            return $this->server;
        }

        public function registerPlugin(Plugin $plugin): void
        {
            $key = strtolower($plugin->getName());
            if (isset($this->plugins[$key]) && $this->plugins[$key] !== $plugin) {
                throw new \RuntimeException('Plugin ' . $plugin->getName() . ' is already registered');
            }
            $this->plugins[$key] = $plugin;
        }

        public function unregisterPlugin(Plugin|string $plugin): void
        {
            $key = is_string($plugin) ? strtolower($plugin) : strtolower($plugin->getName());
            $target = $this->plugins[$key] ?? null;
            if ($target === null && is_string($plugin)) {
                foreach ($this->plugins as $candidateKey => $candidate) {
                    if (strcasecmp($candidate->getName(), $plugin) === 0) {
                        $target = $candidate;
                        $key = $candidateKey;
                        break;
                    }
                }
            }
            if ($target === null) {
                return;
            }
            $this->unregisterEvents($target);
            unset($this->plugins[$key]);
        }

        public function getPlugin(string $name): ?Plugin
        {
            $key = strtolower($name);
            if (isset($this->plugins[$key])) {
                return $this->plugins[$key];
            }
            foreach ($this->plugins as $plugin) {
                if (strcasecmp($plugin->getName(), $name) === 0) {
                    return $plugin;
                }
            }
            return null;
        }

        /** @return array<string, Plugin> */
        public function getPlugins(): array
        {
            return $this->plugins;
        }

        public function isPluginEnabled(Plugin|string $plugin): bool
        {
            if (is_string($plugin)) {
                $plugin = $this->getPlugin($plugin);
            }
            return $plugin !== null && $plugin->isEnabled();
        }

        public function enablePlugin(Plugin $plugin): void
        {
            if ($plugin->isEnabled()) {
                return;
            }

            $plugin->setEnabled(true);
            try {
                $plugin->onEnable();
            } catch (\Throwable $e) {
                $plugin->setEnabled(false);
                $this->log('onEnable failed for ' . $plugin->getName() . ': ' . $e->getMessage());
                throw $e;
            }

            if ($plugin instanceof Listener && $this->getListenerEvents($plugin) === []) {
                try {
                    $this->registerEvents($plugin, $plugin);
                } catch (\Throwable $e) {
                    // A Listener with no event methods is legal; just log at debug level.
                    $this->log('No event handlers found for ' . $plugin->getName());
                }
            }
        }

        public function disablePlugin(Plugin $plugin): void
        {
            if (!$plugin->isEnabled()) {
                $this->unregisterEvents($plugin);
                return;
            }

            try {
                $plugin->onDisable();
            } catch (\Throwable $e) {
                $this->log('onDisable failed for ' . $plugin->getName() . ': ' . $e->getMessage());
            } finally {
                $this->unregisterEvents($plugin);
                $plugin->setEnabled(false);
            }
        }

        public function registerEvents(Listener $listener, Plugin $plugin): void
        {
            $reflection = new \ReflectionClass($listener);
            $pluginKey = strtolower($plugin->getName());
            $validMethods = 0;

            foreach ($reflection->getMethods(\ReflectionMethod::IS_PUBLIC) as $method) {
                if ($method->isConstructor() || $method->isDestructor()) {
                    continue;
                }

                $eventClass = $this->resolveEventParameter($method);
                if ($eventClass === null) {
                    continue;
                }
                $validMethods++;

                [$priority, $ignoreCancelled] = $this->readHandlerAnnotations($method);
                $methodName = $method->getName();
                $dedupeKey = get_class($listener) . '::' . $methodName . '#' . spl_object_id($listener) . '@' . $eventClass;
                if (!empty($this->registeredKeys[$pluginKey][$dedupeKey])) {
                    continue;
                }
                $this->registeredKeys[$pluginKey][$dedupeKey] = true;

                $callable = $method->isStatic()
                    ? [$method->getDeclaringClass()->getName(), $methodName]
                    : [$listener, $methodName];

                $this->listeners[$eventClass][] = new RegisteredListener(
                    $plugin->getName(),
                    $eventClass,
                    $priority,
                    $ignoreCancelled,
                    $callable,
                    $methodName
                );
            }

            if ($validMethods === 0) {
                throw new \InvalidArgumentException(
                    'Listener ' . get_class($listener) . ' has no public methods accepting an Event parameter'
                );
            }
        }

        public function unregisterEvents(Plugin $plugin): void
        {
            $pluginKey = strtolower($plugin->getName());
            foreach ($this->listeners as $eventClass => $listeners) {
                $kept = [];
                foreach ($listeners as $listener) {
                    if (strcasecmp($listener->getPluginName(), $plugin->getName()) !== 0) {
                        $kept[] = $listener;
                    }
                }
                if ($kept === []) {
                    unset($this->listeners[$eventClass]);
                } else {
                    $this->listeners[$eventClass] = $kept;
                }
            }
            unset($this->registeredKeys[$pluginKey]);
        }

        public function dispatchEvent(Event $event): void
        {
            $classes = [$event::class];
            foreach (class_parents($event) as $parent) {
                $classes[] = $parent;
            }

            $listeners = [];
            foreach ($classes as $class) {
                if (isset($this->listeners[$class])) {
                    foreach ($this->listeners[$class] as $listener) {
                        $listeners[] = $listener;
                    }
                }
            }

            usort(
                $listeners,
                static fn(RegisteredListener $a, RegisteredListener $b): int => $a->getPriority() <=> $b->getPriority()
            );

            foreach ($listeners as $listener) {
                try {
                    $listener->callEvent($event);
                } catch (\Throwable $e) {
                    $this->log(
                        'Exception in ' . $listener->getPluginName() . '::' . $listener->getMethodName()
                        . ' for ' . EventFactory::nameOf($event) . ': ' . $e->getMessage()
                    );
                }
            }
        }

        public function getListenerCount(?Plugin $plugin = null): int
        {
            if ($plugin === null) {
                $count = 0;
                foreach ($this->listeners as $listeners) {
                    $count += count($listeners);
                }
                return $count;
            }

            $count = 0;
            foreach ($this->listeners as $listeners) {
                foreach ($listeners as $listener) {
                    if (strcasecmp($listener->getPluginName(), $plugin->getName()) === 0) {
                        $count++;
                    }
                }
            }
            return $count;
        }

        /** @return string[] short event names */
        public function getListenerEvents(?Plugin $plugin = null): array
        {
            $names = [];
            foreach ($this->listeners as $eventClass => $listeners) {
                foreach ($listeners as $listener) {
                    if ($plugin !== null && strcasecmp($listener->getPluginName(), $plugin->getName()) !== 0) {
                        continue;
                    }
                    $pos = strrpos($eventClass, '\\');
                    $names[$pos === false ? $eventClass : substr($eventClass, $pos + 1)] = true;
                }
            }
            ksort($names);
            return array_keys($names);
        }

        /** @return RegisteredListener[] */
        public function getEventListeners(string $eventClass): array
        {
            $name = ltrim($eventClass, '\\');
            if (isset($this->listeners[$name])) {
                return $this->listeners[$name];
            }
            if (class_exists($name)) {
                return $this->listeners[$name] ?? [];
            }
            $resolved = EventFactory::classOf($name);
            if ($resolved !== null) {
                return $this->listeners[$resolved] ?? [];
            }
            return [];
        }

        public function getScheduler()
        {
            if (is_object($this->server) && method_exists($this->server, 'getScheduler')) {
                try {
                    $scheduler = $this->server->getScheduler();
                    if (is_object($scheduler)) {
                        return $scheduler;
                    }
                } catch (\Throwable) {
                    // Fall through to the static singleton.
                }
            }

            foreach (['pocketmine\\scheduler\\Scheduler', 'pocketmine\\Scheduler'] as $class) {
                if (!class_exists($class)) {
                    continue;
                }
                if (method_exists($class, 'getInstance')) {
                    try {
                        return $class::getInstance();
                    } catch (\Throwable) {
                        // Try a plain constructor next.
                    }
                }
                try {
                    return new $class();
                } catch (\Throwable) {
                    // Try the next class.
                }
            }

            return \pocketmine\scheduler\Scheduler::getInstance();
        }

        public function getPermissionManager()
        {
            if (is_object($this->server) && method_exists($this->server, 'getPermissionManager')) {
                try {
                    $manager = $this->server->getPermissionManager();
                    if (is_object($manager)) {
                        return $manager;
                    }
                } catch (\Throwable) {
                    // Fall through to the static singleton.
                }
            }

            foreach (['pocketmine\\permission\\PermissionManager', 'pocketmine\\PermissionManager'] as $class) {
                if (!class_exists($class)) {
                    continue;
                }
                if (method_exists($class, 'getInstance')) {
                    try {
                        return $class::getInstance();
                    } catch (\Throwable) {
                        // Try plain construction.
                    }
                }
                try {
                    return new $class();
                } catch (\Throwable) {
                    // Try the next class name.
                }
            }
            return null;
        }

        private function resolveEventParameter(\ReflectionMethod $method): ?string
        {
            $parameters = $method->getParameters();
            if ($parameters === []) {
                return null;
            }

            $type = $parameters[0]->getType();
            if ($type === null) {
                return null;
            }

            foreach ($this->typeNames($type) as $name) {
                if ($name === '') {
                    continue;
                }
                $isBuiltin = $type instanceof \ReflectionNamedType && $type->isBuiltin();
                if ($isBuiltin) {
                    continue;
                }
                $name = ltrim($name, '\\');
                $lower = strtolower($name);
                if (in_array($lower, ['self', 'static', 'parent', 'mixed', 'object', 'iterable', 'callable', 'array', 'void', 'null', 'never'], true)) {
                    continue;
                }
                if (!class_exists($name) && !interface_exists($name)) {
                    continue;
                }
                if (is_a($name, Event::class, true)) {
                    return $name;
                }
            }
            return null;
        }

        /** @return string[] */
        private function typeNames(\ReflectionType $type): array
        {
            if ($type instanceof \ReflectionNamedType) {
                return [$type->getName()];
            }
            if ($type instanceof \ReflectionUnionType) {
                $names = [];
                foreach ($type->getTypes() as $inner) {
                    if ($inner instanceof \ReflectionNamedType) {
                        $names[] = $inner->getName();
                    }
                }
                return $names;
            }
            if ($type instanceof \ReflectionIntersectionType) {
                $names = [];
                foreach ($type->getTypes() as $inner) {
                    if ($inner instanceof \ReflectionNamedType) {
                        $names[] = $inner->getName();
                    }
                }
                return $names;
            }
            return [];
        }

        /** @return array{0:int, 1:bool} */
        private function readHandlerAnnotations(\ReflectionMethod $method): array
        {
            $priority = EventPriority::NORMAL;
            $ignoreCancelled = false;
            $doc = $method->getDocComment();
            if (!is_string($doc)) {
                return [$priority, $ignoreCancelled];
            }

            if (preg_match('/@priority[ \t]+([A-Za-z0-9_]+)/i', $doc, $match) === 1) {
                $name = strtoupper($match[1]);
                $map = [
                    'LOWEST' => EventPriority::LOWEST,
                    'LOW' => EventPriority::LOW,
                    'NORMAL' => EventPriority::NORMAL,
                    'HIGH' => EventPriority::HIGH,
                    'HIGHEST' => EventPriority::HIGHEST,
                    'MONITOR' => EventPriority::MONITOR,
                ];
                if (isset($map[$name])) {
                    $priority = $map[$name];
                } elseif (is_numeric($match[1])) {
                    $priority = EventPriority::getPriority((int)$match[1]);
                }
            }

            if (preg_match('/@ignoreCancelled[ \t]+(true|false|1|0)/i', $doc, $match) === 1) {
                $ignoreCancelled = in_array(strtolower($match[1]), ['true', '1'], true);
            } elseif (preg_match('/@handleCancelled[ \t]+(true|false|1|0)/i', $doc, $match) === 1) {
                $ignoreCancelled = in_array(strtolower($match[1]), ['true', '1'], true);
            }

            return [$priority, $ignoreCancelled];
        }

        private function log(string $message): void
        {
            fwrite(STDERR, '[PHP/RUNTIME] ' . $message . PHP_EOL);
        }
    }
}



