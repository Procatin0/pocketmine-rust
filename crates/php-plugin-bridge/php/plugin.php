<?php
declare(strict_types=1);

namespace pocketmine\plugin {
    use pocketmine\event\Event;
    use pocketmine\event\EventFactory;
    use pocketmine\event\EventPriority;
    use pocketmine\event\Listener;
    use pocketmine\event\RegisteredListener;

    /** @throws PluginException */
    class PluginException extends \Exception
    {
    }

    class PluginDescriptionParseException extends PluginException
    {
    }

    /**
     * @internal Thrown to suicide inside onEnable() (PM parity).
     */
    class DisablePluginException extends \Exception
    {
    }

    /**
     * PM `PluginEnableOrder`. `STARTUP` plugins enable before worlds,
     * `POSTWORLD` (default) plugins enable after them.
     */
    enum PluginEnableOrder
    {
        case STARTUP;
        case POSTWORLD;

        public static function fromString(string $name): ?self
        {
            return match (strtolower(trim($name))) {
                'startup' => self::STARTUP,
                'postworld' => self::POSTWORLD,
                default => null,
            };
        }

        /** @return string[] */
        public function getAliases(): array
        {
            return match ($this) {
                self::STARTUP => ['startup'],
                self::POSTWORLD => ['postworld'],
            };
        }

        public function name(): string
        {
            return $this->name;
        }
    }

    /**
     * PM `PluginDescriptionCommandEntry` (value object).
     * `PluginDescription::getCommands()` still returns the raw spec arrays the
     * bridge has always returned (see `getCommandEntries()` for the PM shape).
     *
     * @param string[] $aliases
     */
    final class PluginDescriptionCommandEntry
    {
        public function __construct(
            private ?string $description,
            private ?string $usageMessage,
            private array $aliases,
            private string $permission,
            private ?string $permissionDeniedMessage
        ) {
        }

        public function getDescription(): ?string
        {
            return $this->description;
        }

        public function getUsageMessage(): ?string
        {
            return $this->usageMessage;
        }

        /** @return string[] */
        public function getAliases(): array
        {
            return $this->aliases;
        }

        public function getPermission(): string
        {
            return $this->permission;
        }

        public function getPermissionDeniedMessage(): ?string
        {
            return $this->permissionDeniedMessage;
        }
    }

    /**
     * PM `PluginLoader` contract.
     */
    interface PluginLoader
    {
        public function canLoadPlugin(string $path): bool;

        public function loadPlugin(string $file): void;

        /**
         * @throws PluginDescriptionParseException
         */
        public function getPluginDescription(string $file): ?PluginDescription;

        public function getAccessProtocol(): string;
    }

    /**
     * Handles plugin directories (`plugin.yml` + `src/`).
     * Stub: `loadPlugin()` only registers the source with the autoloader (the
     * bridge's autoloader is per-plugin PSR-4, no ThreadSafeClassLoader needed).
     */
    class ScriptPluginLoader implements PluginLoader
    {
        public function canLoadPlugin(string $path): bool
        {
            return is_dir($path) && is_file(rtrim($path, '/\\') . DIRECTORY_SEPARATOR . 'plugin.yml');
        }

        public function loadPlugin(string $file): void
        {
            // Registration with the autoloader happens in PluginManager when the
            // description is parsed; nothing else loads at this stage.
        }

        public function getPluginDescription(string $file): ?PluginDescription
        {
            $manifest = rtrim($file, '/\\') . DIRECTORY_SEPARATOR . 'plugin.yml';
            if (!is_file($manifest)) {
                return null;
            }
            $yaml = @file_get_contents($manifest);
            if ($yaml === false || trim($yaml) === '') {
                throw new PluginDescriptionParseException('YAML parsing error in plugin manifest');
            }
            $map = function_exists('yaml_parse') ? @yaml_parse($yaml) : null;
            if (!is_array($map)) {
                throw new PluginDescriptionParseException('Invalid structure of plugin manifest, expected array');
            }
            return PluginDescription::fromArray($map);
        }

        public function getAccessProtocol(): string
        {
            return 'file://';
        }
    }

    /**
     * Handles `.phar` plugin archives (PM `PharPluginLoader.php` parity).
     *
     * PM's loader is constructed with a `ThreadSafeClassLoader` and calls
     * `$loader->addPath($description->getSrcNamespacePrefix(), "$file/src")`
     * from `loadPlugin()`. This bridge has no `ThreadSafeClassLoader`; its
     * per-plugin PSR-4 equivalent is `PluginAutoloader::registerPlugin()`,
     * which stores the same `prefix -> phar://<file>/src` mapping (the
     * autoloader itself is registered once via `spl_autoload_register`,
     * the `register_autoloader` equivalent of
     * `ThreadSafeClassLoader::register()`).
     *
     * `plugin.yml` is read from inside the archive, preferring the `Phar`
     * class (`new \Phar($file)`) and falling back to the `phar://` wrapper —
     * the `Phar::getMetadata()` metadata is archive metadata, not the
     * manifest, so it is not used here. No signature verification is done
     * (PM does not verify signatures either). When the Phar extension is
     * disabled every entry point logs a clear message and degrades
     * gracefully instead of throwing.
     */
    class PharPluginLoader implements PluginLoader
    {
        private bool $pharWarned = false;

        /**
         * Whether this loader can load the plugin in the given path:
         * a `.phar` file carrying a `plugin.yml` at its root.
         */
        public function canLoadPlugin(string $path): bool
        {
            if (!is_file($path)) {
                return false;
            }
            if (strtolower(pathinfo($path, PATHINFO_EXTENSION)) !== 'phar') {
                return false;
            }
            // With phar support we verify the manifest lives inside, so a
            // random archive is not mistaken for a plugin. Without phar
            // support we still claim the file so the caller reports the
            // missing-extension problem instead of silently skipping it.
            if (!$this->hasPharSupport()) {
                return true;
            }
            return $this->readManifest($path) !== null;
        }

        /**
         * Registers the archive's `src/` sources with the autoloader:
         * prefix = the manifest's `src-namespace-prefix` (default: the
         * namespace of the `main` class), directory = `phar://<file>/src`.
         */
        public function loadPlugin(string $file): void
        {
            try {
                $description = $this->getPluginDescription($file);
            } catch (\Throwable $e) {
                $this->warn('Could not load phar plugin ' . $file . ': ' . $e->getMessage());
                return;
            }
            if ($description === null) {
                $this->warn(
                    'Could not load phar plugin ' . $file
                    . ($this->hasPharSupport() ? ': missing plugin.yml' : $this->pharHint())
                );
                return;
            }

            $root = $this->pharRoot($file);
            $srcDir = $root . '/src';
            $explicit = trim(str_replace('/', '\\', $description->getSrcNamespacePrefix()), '\\');
            $main = ltrim(str_replace('/', '\\', $description->getMain()), '\\');
            $mainNs = '';
            if ($pos = strrpos($main, '\\')) {
                $mainNs = substr($main, 0, $pos);
            }
            $prefix = $explicit !== '' ? $explicit . '\\' : ($mainNs !== '' ? $mainNs . '\\' : '');

            if (class_exists(\pocketmine\runtime\PluginAutoloader::class)) {
                try {
                    // Isolation: the autoloader keys entries by description
                    // name + phar root, so two archives never collide.
                    \pocketmine\runtime\PluginAutoloader::registerPlugin(
                        $description->getName(),
                        $root,
                        $main,
                        $description->getSrcNamespacePrefix()
                    );
                } catch (\Throwable $e) {
                    $this->warn('Autoloader failed for phar plugin ' . $description->getName() . ': ' . $e->getMessage());
                }
                return;
            }

            // Fallback when the bridge autoloader file has not been loaded
            // yet: a plain SPL autoloader for this archive's src/ directory.
            spl_autoload_register(static function (string $class) use ($prefix, $srcDir): void {
                $class = ltrim($class, '\\');
                $candidates = [];
                if ($prefix !== '' && stripos($class, $prefix) === 0) {
                    $candidates[] = $srcDir . '/' . substr($class, strlen($prefix)) . '.php';
                }
                $candidates[] = $srcDir . '/' . $class . '.php';
                foreach ($candidates as $candidate) {
                    $candidate = str_replace('\\', '/', $candidate);
                    if (is_file($candidate)) {
                        require_once $candidate;
                        return;
                    }
                }
            }, true, true);
        }

        public function getPluginDescription(string $file): ?PluginDescription
        {
            if (!$this->hasPharSupport()) {
                $this->warn('Cannot read plugin.yml from phar plugin ' . $file . $this->pharHint());
                return null;
            }
            $yaml = $this->readManifest($file);
            if ($yaml === null) {
                return null;
            }
            $map = function_exists('yaml_parse') ? @yaml_parse($yaml) : null;
            if (!is_array($map)) {
                throw new PluginDescriptionParseException('Invalid structure of plugin manifest, expected array');
            }
            return PluginDescription::fromArray($map);
        }

        public function getAccessProtocol(): string
        {
            return 'phar://';
        }

        /** Whether PHP here can open Phar archives at all. */
        private function hasPharSupport(): bool
        {
            return class_exists(\Phar::class) || in_array('phar', stream_get_wrappers(), true);
        }

        /**
         * Reads the raw `plugin.yml` from inside the archive, or null when
         * the archive carries none (or cannot be opened). Never throws.
         */
        private function readManifest(string $file): ?string
        {
            $real = $this->pharPath($file);
            if ($real === '' || !is_file($real)) {
                return null;
            }
            if (class_exists(\Phar::class)) {
                try {
                    $phar = new \Phar($real);
                    if (isset($phar['plugin.yml'])) {
                        $content = $phar['plugin.yml']->getContent();
                        if (is_string($content) && trim($content) !== '') {
                            return $content;
                        }
                    }
                    return null;
                } catch (\Throwable) {
                    // Not a valid Phar (or unreadable): fall through to the
                    // stream-wrapper attempt below, which fails the same way.
                }
            }
            $yaml = @file_get_contents($this->pharRoot($file) . '/plugin.yml');
            if ($yaml === false || trim((string)$yaml) === '') {
                return null;
            }
            return (string)$yaml;
        }

        /** Filesystem path of the archive, without the `phar://` scheme. */
        private function pharPath(string $file): string
        {
            $path = str_replace('\\', '/', $file);
            if (stripos($path, 'phar://') === 0) {
                $path = substr($path, 7);
            }
            $real = @realpath($path);
            return $real !== false ? str_replace('\\', '/', $real) : $path;
        }

        /** `phar://` URI of the archive root, e.g. `phar://C:/.../Foo.phar`. */
        private function pharRoot(string $file): string
        {
            return 'phar://' . ltrim($this->pharPath($file), '/');
        }

        private function pharHint(): string
        {
            return ': the PHP phar extension appears to be disabled; enable '
                . 'extension=phar and set phar.readonly=Off in php.ini to load .phar plugins';
        }

        private function warn(string $message): void
        {
            if ($this->pharWarned && stripos($message, 'phar extension') !== false) {
                return;
            }
            $this->pharWarned = true;
            fwrite(STDERR, '[PHP/PLUGIN] ' . $message . PHP_EOL);
        }
    }

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

    /**
     * PM `pocketmine\plugin\PluginOwned`: commands/tasks/listeners owned
     * by a plugin (virions like ScoreHud's command use both this and the
     * trait below; without them the class fails to load with
     * `Trait "pocketmine\plugin\PluginOwnedTrait" not found`).
     */
    interface PluginOwned
    {
        public function getOwningPlugin(): Plugin;
    }

    /**
     * PM `pocketmine\plugin\PluginOwnedTrait`: `$owningPlugin` is assigned
     * by the using class (usually its constructor) and read back through
     * `getOwningPlugin()`.
     */
    trait PluginOwnedTrait
    {
        protected Plugin $owningPlugin;

        public function getOwningPlugin(): Plugin
        {
            return $this->owningPlugin;
        }
    }

    /**
     * Validated `plugin.yml` description.
     *
     * `fromArray()` throws `PluginDescriptionParseException` for invalid
     * manifests, following `PluginDescription::loadMap()` in PM:
     * - `name` must match `/^[A-Za-z0-9 _.-]+$/` (PM then replaces spaces with
     *   `_`; the bridge preserves the original name to keep the Rust/PHP name
     *   maps identical, and the spaced-name warning is emitted by the
     *   loaders instead).
     * - `version` / `main` must be non-empty when present in the manifest.
     *   (PM also rejects `main` starting with `pocketmine\`; the bridge skips
     *   that check so the bundled `pocketmine\example\Main` keeps loading.)
     * - `api` entries must be valid base versions (`X.Y.Z[-suffix]`).
     * - `libraries` must be a list of strings (PM5 has no `libraries` key;
     *   older loaders / Poggit embedded libraries use it; missing is fine).
     * - `commands`: keys must be strings, values arrays with a string
     *   `permission`, like PM.
     * - `permissions`: validated with `PermissionParser::loadPermissions()`
     *   (PM `PermissionParserException` is wrapped into the parse exception).
     * - `load` must be `startup` / `postworld` (see `getOrder()`).
     * - `extensions` accepts both the linear (`[ext, ...]`) and map
     *   (`{ext: ["..."], ...}`) forms like PM.
     *
     * `getCommands()` / `getPermissions()` keep returning the raw spec arrays
     * (the IPC layer and existing plugins read them); `getCommandEntries()`
     * exposes the PM value object shape.
     */
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
        /** @var array<string, PluginDescriptionCommandEntry> */
        private array $commandEntries = [];
        private array $permissions = [];
        private string $srcNamespacePrefix = '';
        /** @var int[] */
        private array $compatibleMcpeProtocols = [];
        /** @var string[] */
        private array $compatibleOperatingSystems = [];
        /** @var array<string, string[]> */
        private array $extensions = [];
        /** @var string[] */
        private array $libraries = [];
        private string $prefix = '';
        private string $load = 'postworld';
        /** @var array<string, mixed> */
        private array $map = [];

        /**
         * PM `PluginDescription::__construct(array|string $yamlString)`.
         */
        public function __construct(array|string $yamlString)
        {
            if (is_string($yamlString)) {
                $map = function_exists('yaml_parse') ? @yaml_parse($yamlString) : null;
                if ($map === false) {
                    throw new PluginDescriptionParseException('YAML parsing error in plugin manifest');
                }
                if (!is_array($map)) {
                    throw new PluginDescriptionParseException(
                        'Invalid structure of plugin manifest, expected array but have ' . get_debug_type($map)
                    );
                }
            } else {
                $map = $yamlString;
            }
            $validated = self::buildFromMap($map);
            foreach (get_object_vars($validated) as $key => $value) {
                $this->{$key} = $value;
            }
            $this->map = $map;
        }

        public static function fromArray(array $data): PluginDescription
        {
            return self::buildFromMap($data);
        }

        private static function buildFromMap(array $data): PluginDescription
        {
            // Avoid invoking __construct (which would recurse into buildFromMap).
            $description = (new \ReflectionClass(self::class))->newInstanceWithoutConstructor();
            $description->map = $data;

            $name = self::stringValue($data['name'] ?? '');
            if ($name !== '') {
                if (preg_match('/^[A-Za-z0-9 _.-]+$/', $name) !== 1) {
                    throw new PluginDescriptionParseException('Invalid Plugin name');
                }
                // Intentional divergence from PM (which replaces spaces with "_"):
                // the Rust side keeps the raw name and warns about spaces.
            }
            $description->name = $name;
            $description->version = self::stringValue($data['version'] ?? '');
            $description->main = self::stringValue($data['main'] ?? '');
            // Fields required by PM but optional for the native fallback
            // `PluginDescription::fromArray(['name' => ...])` used when a plugin
            // is instantiated before init(); loadPlugins()/the IPC path always
            // provide them and the empty check below still guards against
            // a manifest that explicitly carries an empty value.
            if (array_key_exists('name', $data) && trim($description->name) === '') {
                throw new PluginDescriptionParseException('Invalid Plugin name');
            }
            if (array_key_exists('version', $data) && trim($description->version) === '') {
                throw new PluginDescriptionParseException('Invalid Plugin version');
            }
            if (array_key_exists('main', $data) && trim($description->main) === '') {
                throw new PluginDescriptionParseException('Invalid Plugin main');
            }
            $description->description = self::stringValue($data['description'] ?? '');
            // PM merges `author` + `authors`.
            $authors = [];
            if (isset($data['author'])) {
                if (is_array($data['author'])) {
                    foreach (self::stringList($data['author']) as $author) {
                        $authors[] = $author;
                    }
                } else {
                    $authors[] = self::stringValue($data['author']);
                }
            }
            if (isset($data['authors'])) {
                foreach (self::stringList($data['authors']) as $author) {
                    $authors[] = $author;
                }
            }
            $description->authors = $authors;

            // api: single-string form is coerced like PM's `(array)`.
            if (isset($data['api']) && !is_array($data['api']) && !is_string($data['api'])) {
                throw new PluginDescriptionParseException('Invalid Plugin "api"');
            }
            $description->api = self::stringList($data['api'] ?? []);
            foreach ($description->api as $api) {
                if (preg_match('/^\d+\.\d+\.\d+(?:-(.*))?$/', $api) !== 1) {
                    throw new PluginDescriptionParseException('Invalid Plugin api: "' . $api . '" (expected e.g. "5.0.0")');
                }
            }

            $description->srcNamespacePrefix = self::stringValue($data['src-namespace-prefix'] ?? '');
            $mcpe = $data['mcpe-protocol'] ?? [];
            $description->compatibleMcpeProtocols = array_map('intval', self::stringList($mcpe));
            $description->compatibleOperatingSystems = self::stringList($data['os'] ?? []);
            $description->extensions = self::parseExtensions($data['extensions'] ?? []);

            $website = $data['website'] ?? null;
            $description->website = $website === null || $website === '' ? null : self::stringValue($website);
            $description->depend = self::stringList($data['depend'] ?? []);
            $description->softDepend = self::stringList($data['softdepend'] ?? $data['softDepend'] ?? []);
            $description->loadBefore = self::stringList(
                $data['load-before'] ?? $data['loadbefore'] ?? $data['loadBefore'] ?? []
            );
            $description->prefix = self::stringValue($data['prefix'] ?? '');
            $description->libraries = self::parseLibraries($data['libraries'] ?? []);

            $load = $data['load'] ?? null;
            if ($load !== null && $load !== '') {
                $normalized = strtolower(trim(self::stringValue($load)));
                if ($normalized !== 'startup' && $normalized !== 'postworld') {
                    throw new PluginDescriptionParseException('Invalid Plugin "load"');
                }
                $description->load = $normalized;
            } else {
                $description->load = 'postworld';
            }

            $description->commands = [];
            $description->commandEntries = [];
            if (isset($data['commands']) && is_array($data['commands'])) {
                foreach ($data['commands'] as $commandName => $commandData) {
                    if (!is_string($commandName) || $commandName === '') {
                        throw new PluginDescriptionParseException('Invalid Plugin commands, key must be the name of the command');
                    }
                    if (!is_array($commandData)) {
                        throw new PluginDescriptionParseException("Command $commandName has invalid properties");
                    }
                    if (!isset($commandData['permission']) || !is_string($commandData['permission']) || $commandData['permission'] === '') {
                        throw new PluginDescriptionParseException("Command $commandName does not have a valid permission set");
                    }
                    $aliases = $commandData['aliases'] ?? [];
                    if (!is_array($aliases)) {
                        $aliases = [$aliases];
                    }
                    $aliases = array_map('strval', array_filter($aliases, 'is_scalar'));
                    $description->commandEntries[$commandName] = new PluginDescriptionCommandEntry(
                        isset($commandData['description']) ? self::nullableString($commandData['description']) : null,
                        isset($commandData['usage']) ? self::nullableString($commandData['usage']) : null,
                        array_values($aliases),
                        $commandData['permission'],
                        isset($commandData['permission-message']) ? self::nullableString($commandData['permission-message']) : null
                    );
                    $description->commands[$commandName] = $commandData;
                }
            } elseif (isset($data['commands']) && !is_array($data['commands'])) {
                throw new PluginDescriptionParseException('Invalid Plugin commands');
            }

            $description->permissions = is_array($data['permissions'] ?? null) ? $data['permissions'] : [];
            if (isset($data['permissions']) && !is_array($data['permissions'])) {
                throw new PluginDescriptionParseException('Invalid Plugin "permissions"');
            }
            if ($description->permissions !== []) {
                try {
                    // PM `PermissionParser::loadPermissions()`: rejects nested
                    // `children` and unknown defaults.
                    \pocketmine\permission\PermissionParser::loadPermissions($description->permissions);
                } catch (\Throwable $e) {
                    throw new PluginDescriptionParseException('Invalid Plugin "permissions": ' . $e->getMessage(), 0, $e);
                }
            }
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

        /** @return string[] PM alias. */
        public function getCompatibleApis(): array
        {
            return $this->api;
        }

        public function getSrcNamespacePrefix(): string
        {
            return $this->srcNamespacePrefix;
        }

        /** @return int[] */
        public function getCompatibleMcpeProtocols(): array
        {
            return $this->compatibleMcpeProtocols;
        }

        /** @return string[] */
        public function getCompatibleOperatingSystems(): array
        {
            return $this->compatibleOperatingSystems;
        }

        /** @return array<string, string[]> */
        public function getRequiredExtensions(): array
        {
            return $this->extensions;
        }

        /** @return string[] Embedded libraries (older loaders / Poggit). */
        public function getLibraries(): array
        {
            return $this->libraries;
        }

        public function getPrefix(): string
        {
            return $this->prefix;
        }

        public function getOrder(): PluginEnableOrder
        {
            return PluginEnableOrder::fromString($this->load) ?? PluginEnableOrder::POSTWORLD;
        }

        /** @return array<string, mixed> */
        public function getMap(): array
        {
            return $this->map;
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

        /** @return array<string, PluginDescriptionCommandEntry> PM shape. */
        public function getCommandEntries(): array
        {
            return $this->commandEntries;
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

        private static function nullableString(mixed $value): ?string
        {
            if ($value === null || $value === '') {
                return null;
            }
            return is_scalar($value) ? (string)$value : null;
        }

        /** @return array<string, string[]> */
        private static function parseExtensions(mixed $value): array
        {
            if (!is_array($value) || $value === []) {
                return [];
            }
            $extensions = $value;
            $isLinear = $extensions === array_values($extensions);
            $out = [];
            foreach ($extensions as $k => $v) {
                if ($isLinear) {
                    $k = $v;
                    $v = '*';
                }
                if (!is_scalar($k)) {
                    throw new PluginDescriptionParseException('Invalid Plugin extensions');
                }
                $versions = is_array($v) ? $v : [$v];
                $out[(string)$k] = array_values(array_map('strval', array_filter($versions, 'is_scalar')));
            }
            return $out;
        }

        /** @return string[] */
        private static function parseLibraries(mixed $value): array
        {
            if ($value === null || $value === '') {
                return [];
            }
            if (!is_array($value)) {
                throw new PluginDescriptionParseException('Invalid Plugin "libraries"');
            }
            $out = [];
            foreach ($value as $lib) {
                if (!is_scalar($lib) || (string)$lib === '') {
                    throw new PluginDescriptionParseException('Invalid Plugin "libraries"');
                }
                $out[] = (string)$lib;
            }
            return $out;
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
        private ?\pocketmine\scheduler\TaskScheduler $scheduler = null;
        private ?PluginLoader $loader = null;

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
            // PM `PluginBase::__construct`: one TaskScheduler per plugin.
            $this->scheduler = new \pocketmine\scheduler\TaskScheduler($description->getFullName());
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
            // Fallback for a plugin instantiated before init() (no manifest
            // yet): fabricate the minimal valid description from the class name.
            $fallbackName = $this->getName();
            $this->description = PluginDescription::fromArray([
                'name' => $fallbackName,
                'version' => '0.0.0',
                'main' => $fallbackName . '\\Main',
            ]);
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

        /**
         * Per-plugin scheduler (PM `PluginBase::getScheduler()` returns a
         * `TaskScheduler` owned by the plugin, never the server singleton).
         *
         * The instance is created in `init()` (lazily here for plugins created
         * without `init()`). Ticking is fanned out from
         * `Scheduler::mainThreadHeartbeat()` (the IPC `tick` action), so tasks
         * scheduled here run on the same clock as the server scheduler.
         */
        public function getScheduler(): \pocketmine\scheduler\TaskScheduler
        {
            if ($this->scheduler === null) {
                $owner = 'unknown';
                try {
                    $owner = $this->getFullName();
                } catch (\Throwable) {
                    // Use the class short name below.
                }
                if ($owner === '' || $owner === 'unknown') {
                    $owner = $this->getName();
                }
                $this->scheduler = new \pocketmine\scheduler\TaskScheduler($owner);
            }
            return $this->scheduler;
        }

        public function getPluginLoader(): ?PluginLoader
        {
            return $this->loader;
        }

        public function setPluginLoader(?PluginLoader $loader): void
        {
            $this->loader = $loader;
        }

        /**
         * PM `PluginBase::getResourceFolder()`: read-only resources directory.
         */
        public function getResourceFolder(): string
        {
            $root = $this->root !== '' ? $this->root : dirname($this->file);
            return rtrim($root, '/\\') . '/resources/';
        }

        /**
         * PM `PluginBase::onEnableStateChange()` (bridge: delegates to
         * `onEnable()` / `onDisable()` like PM).
         */
        public function onEnableStateChange(bool $enabled): void
        {
            if ($this->enabled === $enabled) {
                return;
            }
            $this->enabled = $enabled;
            if ($enabled) {
                $this->onEnable();
            } else {
                $this->onDisable();
            }
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

        /** @var array<string, Plugin> */
        private array $enabledPlugins = [];

        /** @var array<string, array<string, true>> dependency => dependents */
        private array $pluginDependents = [];

        /** @var array<string, array<int, RegisteredListener>> */
        private array $listeners = [];

        /** @var array<string, array<string, bool>> */
        private array $registeredKeys = [];

        /** @var array<string, PluginLoader> */
        protected array $fileAssociations = [];

        private bool $loadPluginsGuard = false;

        /** @var string[] planned load order (plugin names, dependencies first) */
        private array $loadOrder = [];

        private mixed $server = null;

        public function __construct(?\pocketmine\Server $server = null)
        {
            $this->server = $server;
            // PM `Server` registers the built-in loaders on boot.
            $this->registerInterface(new ScriptPluginLoader());
            $this->registerInterface(new PharPluginLoader());
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

        public function registerInterface(PluginLoader $loader): void
        {
            $this->fileAssociations[get_class($loader)] = $loader;
        }

        /** @return array<string, PluginLoader> */
        public function getLoaders(): array
        {
            return $this->fileAssociations;
        }

        /** @return string[] planned load order (dependencies first). */
        public function getLoadOrder(): array
        {
            return $this->loadOrder;
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

        public function enablePlugin(Plugin $plugin): bool
        {
            if ($plugin->isEnabled()) {
                return true;
            }

            // PM `PluginManager::enablePlugin()`: enable the per-plugin scheduler first.
            try {
                $scheduler = $plugin->getScheduler();
                if (is_object($scheduler) && method_exists($scheduler, 'setEnabled')) {
                    $scheduler->setEnabled(true);
                }
            } catch (\Throwable) {
                // Scheduler is best-effort here; onEnable() still runs.
            }

            try {
                // Bridge uses setEnabled()+onEnable() (PM uses onEnableStateChange()).
                // PluginBase provides onEnableStateChange() for PM-style plugins that
                // call it themselves; the manager does not double-invoke it.
                $plugin->setEnabled(true);
                $plugin->onEnable();
            } catch (DisablePluginException $e) {
                $this->disablePlugin($plugin);
                return false;
            } catch (\Throwable $e) {
                try {
                    $plugin->setEnabled(false);
                } catch (\Throwable) {
                    // Ignore.
                }
                $this->log('onEnable failed for ' . $plugin->getName() . ': ' . $e->getMessage());
                throw $e;
            }

            if (!$plugin->isEnabled()) {
                $this->log(
                    "Could not enable plugin '" . $plugin->getName() . "': Plugin disabled itself when attempting to enable it"
                );
                return false;
            }

            $this->enabledPlugins[$plugin->getName()] = $plugin;
            foreach ($plugin->getDescription()->getDepend() as $dependency) {
                $this->pluginDependents[$dependency][$plugin->getName()] = true;
            }
            foreach ($plugin->getDescription()->getSoftDepend() as $dependency) {
                if (isset($this->plugins[strtolower($dependency)])) {
                    $this->pluginDependents[$dependency][$plugin->getName()] = true;
                }
            }

            // PM fires PluginEnableEvent after the enabled bookkeeping.
            try {
                $this->dispatchEvent(new \pocketmine\event\plugin\PluginEnableEvent($plugin));
            } catch (\Throwable $e) {
                $this->log('PluginEnableEvent failed for ' . $plugin->getName() . ': ' . $e->getMessage());
            }

            if ($plugin instanceof Listener && $this->getListenerEvents($plugin) === []) {
                try {
                    $this->registerEvents($plugin, $plugin);
                } catch (\Throwable $e) {
                    // A Listener with no event methods is legal; just log at debug level.
                    $this->log('No event handlers found for ' . $plugin->getName());
                }
            }
            return true;
        }

        public function disablePlugin(Plugin $plugin): void
        {
            if (!$plugin->isEnabled()) {
                $this->unregisterEvents($plugin);
                return;
            }

            // PM fires PluginDisableEvent *before* onDisable() so listeners can
            // still observe it; their registrations are removed afterwards.
            try {
                $this->dispatchEvent(new \pocketmine\event\plugin\PluginDisableEvent($plugin));
            } catch (\Throwable $e) {
                $this->log('PluginDisableEvent failed for ' . $plugin->getName() . ': ' . $e->getMessage());
            }

            unset($this->enabledPlugins[$plugin->getName()]);
            foreach (array_keys($this->pluginDependents) as $dependency) {
                if (isset($this->pluginDependents[$dependency][$plugin->getName()])) {
                    if (count($this->pluginDependents[$dependency]) === 1) {
                        unset($this->pluginDependents[$dependency]);
                    } else {
                        unset($this->pluginDependents[$dependency][$plugin->getName()]);
                    }
                }
            }

            try {
                $plugin->onDisable();
            } catch (\Throwable $e) {
                $this->log('onDisable failed for ' . $plugin->getName() . ': ' . $e->getMessage());
            } finally {
                try {
                    $scheduler = $plugin->getScheduler();
                    if (is_object($scheduler) && method_exists($scheduler, 'shutdown')) {
                        $scheduler->shutdown();
                    }
                } catch (\Throwable) {
                    // Scheduler shutdown is best-effort.
                }
                $this->unregisterEvents($plugin);
                $plugin->setEnabled(false);
            }
        }

        public function disablePlugins(): void
        {
            // PM disables in reverse dependency order (dependents first).
            while ($this->enabledPlugins !== []) {
                foreach ($this->enabledPlugins as $plugin) {
                    if (!$plugin->isEnabled()) {
                        continue;
                    }
                    $name = $plugin->getDescription()->getName();
                    if (isset($this->pluginDependents[$name]) && count($this->pluginDependents[$name]) > 0) {
                        continue;
                    }
                    $this->disablePlugin($plugin);
                }
                // Safety: if dependents never clear (cycle), force-disable the rest.
                if ($this->enabledPlugins !== []) {
                    $progress = false;
                    foreach ($this->enabledPlugins as $plugin) {
                        if ($plugin->isEnabled()) {
                            $progress = true;
                            $this->disablePlugin($plugin);
                        }
                    }
                    if (!$progress) {
                        break;
                    }
                }
            }
        }

        public function clearPlugins(): void
        {
            $this->disablePlugins();
            $this->plugins = [];
            $this->enabledPlugins = [];
            $this->fileAssociations = [];
            $this->pluginDependents = [];
            $this->loadOrder = [];
        }

        /**
         * PM `Server::enablePlugins(PluginEnableOrder)`: enables loaded plugins
         * of one phase (`STARTUP` before worlds, `POSTWORLD` after), in
         * dependency order. The Rust host calls this directly (see
         * `PluginManager::enable_plugins_for`); this PHP helper exists for
         * parity / standalone use and mirrors the same filtering.
         *
         * @return bool true when every plugin in the phase enabled.
         */
        public function enablePluginsFor(PluginEnableOrder $order): bool
        {
            $allSuccess = true;
            foreach ($this->loadOrder as $name) {
                $plugin = $this->getPlugin($name);
                if ($plugin === null) {
                    continue;
                }
                if ($plugin->getDescription()->getOrder() !== $order) {
                    continue;
                }
                if ($plugin->isEnabled()) {
                    continue;
                }
                if (!$this->enablePlugin($plugin)) {
                    $allSuccess = false;
                }
            }
            // Native / late-registered plugins (not in loadOrder) enable last.
            foreach ($this->plugins as $plugin) {
                if (!in_array($plugin->getName(), $this->loadOrder, true)) {
                    if ($plugin->getDescription()->getOrder() === $order && !$plugin->isEnabled()) {
                        if (!$this->enablePlugin($plugin)) {
                            $allSuccess = false;
                        }
                    }
                }
            }
            return $allSuccess;
        }

        /**
         * PM `PluginManager::tickSchedulers()`: ticks each enabled plugin's
         * per-plugin scheduler. `Scheduler::mainThreadHeartbeat()` already
         * fans out to all registered `TaskScheduler`s (the IPC `tick` action
         * drives that path); heartbeat is idempotent per tick, so calling both
         * in the same tick is safe.
         */
        public function tickSchedulers(int $currentTick): void
        {
            foreach ($this->enabledPlugins as $pluginName => $plugin) {
                if (!isset($this->enabledPlugins[$pluginName])) {
                    continue;
                }
                try {
                    $plugin->getScheduler()->mainThreadHeartbeat($currentTick);
                } catch (\Throwable $e) {
                    $this->log('Scheduler heartbeat failed for ' . $pluginName . ': ' . $e->getMessage());
                }
            }
        }

        /**
         * PM `PluginManager::registerEvent()`.
         *
         * Registers a single `Closure` handler for `$event`; the handler runs
         * through `dispatchEvent()` (the bridge's `dispatch`, like PM's
         * `HandlerListManager`). Returns the created listener so callers can
         * detach it via `unregisterEvents()` / `clearPlugins()`.
         *
         * Bridge extension: `$priority` also accepts a string name
         * (`LOWEST|LOW|NORMAL|HIGH|HIGHEST|MONITOR`, case-insensitive) for
         * callers coming from docblock-parsing code; PM accepts only `int`.
         *
         * @param class-string<Event> $event
         * @param \Closure(Event) : void $handler
         * @throws PluginException
         */
        public function registerEvent(string $event, \Closure $handler, int|string $priority, Plugin $plugin, bool $handleCancelled = false): RegisteredListener
        {
            $eventClass = ltrim($event, '\\');
            if (!is_subclass_of($eventClass, Event::class)) {
                throw new PluginException($event . ' is not an Event');
            }
            $handlerName = self::niceClosureName($handler);
            $reflect = new \ReflectionFunction($handler);
            if ($reflect->isGenerator()) {
                throw new PluginException("Generator function $handlerName cannot be used as an event handler");
            }
            if (!$plugin->isEnabled()) {
                throw new PluginException(
                    'Plugin attempted to register event handler ' . $handlerName . '() to event ' . $event . ' while not enabled'
                );
            }
            if (is_string($priority)) {
                $normalized = strtoupper(trim($priority));
                $map = [
                    'LOWEST' => EventPriority::LOWEST,
                    'LOW' => EventPriority::LOW,
                    'NORMAL' => EventPriority::NORMAL,
                    'HIGH' => EventPriority::HIGH,
                    'HIGHEST' => EventPriority::HIGHEST,
                    'MONITOR' => EventPriority::MONITOR,
                ];
                if (!isset($map[$normalized])) {
                    throw new PluginException(
                        'Event handler ' . $handlerName . '() declares invalid/unknown priority "' . $priority . '"'
                    );
                }
                $priority = $map[$normalized];
            }
            $priorityInt = EventPriority::getPriority((int)$priority);
            if ($handleCancelled && !is_a($eventClass, \pocketmine\event\Cancellable::class, true)) {
                throw new PluginException(
                    "Event handler $handlerName() declares @handleCancelled for non-cancellable event of type $eventClass"
                );
            }
            $dedupeKey = $eventClass . '::' . $handlerName . '#' . spl_object_id($handler) . '@' . $plugin->getName();
            $pluginKey = strtolower($plugin->getName());
            if (!empty($this->registeredKeys[$pluginKey][$dedupeKey])) {
                // Already registered (e.g. registerEvents() + auto-register on
                // enable): return the existing listener instead of duplicating.
                foreach ($this->listeners[$eventClass] ?? [] as $existing) {
                    if (strcasecmp($existing->getPluginName(), $plugin->getName()) === 0
                        && $existing->getMethodName() === $handlerName
                        && $existing->getPriority() === $priorityInt) {
                        return $existing;
                    }
                }
            }
            $this->registeredKeys[$pluginKey][$dedupeKey] = true;
            // Legacy RegisteredListener shape: (pluginName, eventClass, priority, ignoreCancelled, callable, methodName).
            // `handleCancelled=true` means "run even when cancelled", i.e. the
            // inverse of PM's `ignoreCancelled` wording; the bridge preserves
            // the original `ignoreCancelled` semantics in the callable wrapper.
            $listener = new RegisteredListener(
                $plugin->getName(),
                $eventClass,
                $priorityInt,
                $handleCancelled,
                $handler,
                $handlerName
            );
            $this->listeners[$eventClass][] = $listener;
            return $listener;
        }

        public function registerEvents(Listener $listener, Plugin $plugin): void
        {
            if (!$plugin->isEnabled()) {
                throw new PluginException('Plugin attempted to register ' . get_class($listener) . ' while not enabled');
            }
            $reflection = new \ReflectionClass($listener);
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
                $pluginKey = strtolower($plugin->getName());
                if (!empty($this->registeredKeys[$pluginKey][$dedupeKey])) {
                    continue;
                }

                $callable = $method->isStatic()
                    ? [$method->getDeclaringClass()->getName(), $methodName]
                    : [$listener, $methodName];

                // Delegate to registerEvent() so validation, priority clamping
                // and dispatch bookkeeping live in one place. `registerEvent`
                // expects a Closure; wrap the method callable without changing
                // the stored callable shape for reflection handlers.
                $closure = \Closure::fromCallable($callable);
                // Preserve the method name for debugging (registerEvent derives
                // it from the closure; overwrite after creation).
                $registered = $this->registerEvent(
                    $eventClass,
                    $closure,
                    $priority,
                    $plugin,
                    $ignoreCancelled
                );
                try {
                    $ref = new \ReflectionProperty($registered, 'methodName');
                    $ref->setAccessible(true);
                    $ref->setValue($registered, $methodName);
                    // Restore the original array callable for stack traces.
                    $callableProp = new \ReflectionProperty($registered, 'callable');
                    $callableProp->setAccessible(true);
                    $callableProp->setValue($registered, $callable);
                } catch (\Throwable) {
                    // Best-effort introspection fix-up only.
                }
                $this->registeredKeys[$pluginKey][$dedupeKey] = true;
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

        /**
         * PM `PluginManager::loadPlugins(string $path, int &$loadErrorCount)`.
         *
         * Scans `$path` (directory or single file) through the registered
         * `PluginLoader`s, validates each `plugin.yml` (see
         * `PluginDescription::fromArray()`), checks loadability (restricted
         * names, `api`, `extensions`), and resolves `depend` / `softdepend` /
         * `loadbefore` in PM's iterative order: a plugin loads only when every
         * listed dependency (hard or soft) is loaded or is known to be absent
         * (missing soft deps are skipped; missing hard deps are errors).
         * Load order is the topological order returned in `getLoadOrder()`;
         * call `enablePluginsFor(STARTUP)` / `enablePluginsFor(POSTWORLD)` (or
         * `enablePlugin()` per plugin) to enable.
         *
         * Divergence from PM: instantiating a plugin uses the bridge's
         * `new Main()` + `init()` flow (PM constructs with
         * `new $main($loader, $server, $description, ...)`). Permissions,
         * commands and `onLoad()` are wired like the IPC `load_plugin` path.
         *
         * @return array<string, Plugin> loaded plugins by description name.
         */
        public function loadPlugins(string $path, int &$loadErrorCount = 0): array
        {
            if ($this->loadPluginsGuard) {
                throw new \LogicException(__CLASS__ . '::loadPlugins() cannot be called from within itself');
            }
            $this->loadPluginsGuard = true;
            try {
                $triage = ['plugins' => [], 'dependencies' => [], 'softDependencies' => []];
                $this->triagePlugins($path, $triage, $loadErrorCount);

                $loadedPlugins = [];
                while ($triage['plugins'] !== []) {
                    $loadedThisLoop = 0;
                    foreach (array_keys($triage['plugins']) as $name) {
                        if (!isset($triage['plugins'][$name])) {
                            continue;
                        }
                        $entry = $triage['plugins'][$name];
                        $this->checkDepsForTriage($name, 'hard', $triage['dependencies'], $loadedPlugins, $triage);
                        $this->checkDepsForTriage($name, 'soft', $triage['softDependencies'], $loadedPlugins, $triage);

                        if (!isset($triage['dependencies'][$name]) && !isset($triage['softDependencies'][$name])) {
                            unset($triage['plugins'][$name]);
                            $loadedThisLoop++;

                            $oldLoaders = $this->fileAssociations;
                            $plugin = $this->internalLoadPlugin($entry['file'], $entry['loader'], $entry['description']);
                            if ($plugin instanceof Plugin) {
                                $loadedPlugins[$plugin->getDescription()->getName()] = $plugin;
                                $diffLoaders = [];
                                foreach ($this->fileAssociations as $key => $loader) {
                                    if (!array_key_exists($key, $oldLoaders)) {
                                        $diffLoaders[] = $key;
                                    }
                                }
                                if ($diffLoaders !== []) {
                                    $beforeKeys = array_keys($triage['plugins']);
                                    $this->triagePlugins($path, $triage, $loadErrorCount, $diffLoaders);
                                    // Re-triage may have added entries; nothing else to do.
                                    unset($beforeKeys);
                                }
                            } else {
                                $loadErrorCount++;
                            }
                        }
                    }

                    if ($loadedThisLoop === 0) {
                        // No progress: first try skipping still-missing soft deps.
                        $retried = false;
                        foreach (array_keys($triage['plugins']) as $name) {
                            if (isset($triage['softDependencies'][$name]) && !isset($triage['dependencies'][$name])) {
                                foreach ($triage['softDependencies'][$name] as $key => $dependency) {
                                    if ($this->getPlugin($dependency) === null && !isset($triage['plugins'][$dependency])) {
                                        unset($triage['softDependencies'][$name][$key]);
                                    }
                                }
                                if (($triage['softDependencies'][$name] ?? []) === []) {
                                    unset($triage['softDependencies'][$name]);
                                    $retried = true;
                                }
                            }
                        }
                        if ($retried) {
                            continue;
                        }

                        foreach (array_keys($triage['plugins']) as $name) {
                            if (!isset($triage['dependencies'][$name])) {
                                continue;
                            }
                            $unknown = [];
                            foreach ($triage['dependencies'][$name] as $dependency) {
                                if ($this->getPlugin($dependency) === null && !isset($triage['plugins'][$dependency])) {
                                    $unknown[$dependency] = $dependency;
                                }
                            }
                            if ($unknown !== []) {
                                $this->log(
                                    "Could not load plugin '" . $name . "': Unknown dependency: " . implode(', ', $unknown)
                                );
                                unset($triage['plugins'][$name], $triage['dependencies'][$name], $triage['softDependencies'][$name]);
                                $loadErrorCount++;
                            }
                        }

                        foreach (array_keys($triage['plugins']) as $name) {
                            $this->log("Could not load plugin '" . $name . "': Circular dependency detected");
                            $loadErrorCount++;
                        }
                        $triage['plugins'] = [];
                        $triage['dependencies'] = [];
                        $triage['softDependencies'] = [];
                        break;
                    }
                }

                $this->loadOrder = array_keys($loadedPlugins);
                return $loadedPlugins;
            } finally {
                $this->loadPluginsGuard = false;
            }
        }

        /**
         * Scans `$path` and fills `$triage` (PM `triagePlugins()`).
         *
         * @param array{plugins: array, dependencies: array, softDependencies: array} $triage
         * @param string[]|null $newLoaders null = all loaders.
         */
        private function triagePlugins(string $path, array &$triage, int &$loadErrorCount, ?array $newLoaders = null): void
        {
            $loaders = [];
            if (is_array($newLoaders)) {
                foreach ($newLoaders as $key) {
                    if (isset($this->fileAssociations[$key])) {
                        $loaders[$key] = $this->fileAssociations[$key];
                    }
                }
            } else {
                $loaders = $this->fileAssociations;
            }
            if ($loaders === []) {
                return;
            }

            if (is_dir($path)) {
                $entries = @scandir($path);
                if ($entries === false) {
                    return;
                }
                $files = [];
                foreach ($entries as $entry) {
                    if ($entry === '.' || $entry === '..') {
                        continue;
                    }
                    $files[] = rtrim($path, '/\\') . DIRECTORY_SEPARATOR . $entry;
                }
                // PM shuffles; the bridge sorts for a deterministic (testable) order.
                usort($files, 'strcasecmp');
            } elseif (is_file($path)) {
                $real = realpath($path);
                $files = [$real !== false ? $real : $path];
            } else {
                return;
            }

            foreach ($loaders as $loader) {
                foreach ($files as $file) {
                    if (!$loader->canLoadPlugin($file)) {
                        continue;
                    }
                    try {
                        $description = $loader->getPluginDescription($file);
                    } catch (PluginDescriptionParseException $e) {
                        $this->log(
                            "Could not load plugin '" . $file . "': Invalid plugin manifest: " . $e->getMessage()
                        );
                        $loadErrorCount++;
                        continue;
                    } catch (\Throwable $e) {
                        $this->log("Could not load plugin '" . $file . "': " . $e->getMessage());
                        $loadErrorCount++;
                        continue;
                    }
                    if ($description === null) {
                        continue;
                    }
                    $name = $description->getName();
                    if (isset($triage['plugins'][$name]) || $this->getPlugin($name) instanceof Plugin) {
                        $this->log("Could not load plugin '" . $name . "': plugin exists");
                        $loadErrorCount++;
                        continue;
                    }
                    if (($loadabilityError = $this->checkLoadability($description)) !== null) {
                        $this->log("Could not load plugin '" . $name . "': " . $loadabilityError);
                        $loadErrorCount++;
                        continue;
                    }
                    if (str_contains($name, ' ')) {
                        $this->log("Plugin '" . $name . "' uses spaces in its name, this is discouraged");
                    }
                    $triage['plugins'][$name] = [
                        'file' => $file,
                        'loader' => $loader,
                        'description' => $description,
                    ];
                    $triage['softDependencies'][$name] = array_merge(
                        $triage['softDependencies'][$name] ?? [],
                        $description->getSoftDepend()
                    );
                    $triage['dependencies'][$name] = $description->getDepend();
                    foreach ($description->getLoadBefore() as $before) {
                        $triage['softDependencies'][$before][] = $name;
                    }
                }
            }
        }

        /**
         * Removes resolved deps for one plugin (PM `checkDepsForTriage()`).
         *
         * @param array<string, array> $dependencyLists
         * @param array<string, Plugin> $loadedPlugins
         */
        private function checkDepsForTriage(string $pluginName, string $dependencyType, array &$dependencyLists, array $loadedPlugins, array $triage): void
        {
            if (!isset($dependencyLists[$pluginName])) {
                return;
            }
            foreach ($dependencyLists[$pluginName] as $key => $dependency) {
                if (isset($loadedPlugins[$dependency]) || $this->getPlugin($dependency) instanceof Plugin) {
                    unset($dependencyLists[$pluginName][$key]);
                } elseif (isset($triage['plugins'][$dependency])) {
                    // Found but not loaded yet: defer.
                }
            }
            if ($dependencyLists[$pluginName] === []) {
                unset($dependencyLists[$pluginName]);
            }
        }

        /**
         * Loadability gate (PM `PluginLoadabilityChecker` subset: restricted
         * names, `api` validity + compatibility + ambiguity, `os`,
         * `extensions`). Returns an error string, or null when loadable.
         */
        private function checkLoadability(PluginDescription $description): ?string
        {
            $name = $description->getName();
            foreach (['pocketmine', 'minecraft', 'mojang'] as $restricted) {
                if (stripos($name, $restricted) !== false) {
                    return 'Plugin name "' . $name . '" is restricted';
                }
            }
            foreach ($description->getCompatibleApis() as $api) {
                if (preg_match('/^\d+\.\d+\.\d+(?:-(.*))?$/', $api) !== 1) {
                    return 'Invalid API "' . $api . '"';
                }
            }
            $serverApi = '5.0.0';
            try {
                if (class_exists(\pocketmine\Server::class)) {
                    $serverApi = \pocketmine\Server::getInstance()->getApiVersion();
                }
            } catch (\Throwable) {
                // Fall back to the bridge default.
            }
            if ($description->getCompatibleApis() !== [] && !$this->isApiCompatible($serverApi, $description->getCompatibleApis())) {
                return 'Incompatible API "' . implode(', ', $description->getCompatibleApis())
                    . '" (server is "' . $serverApi . '")';
            }
            $ambiguous = $this->checkAmbiguousVersions($description->getCompatibleApis());
            if ($ambiguous !== []) {
                return 'Ambiguous minimum API "' . implode(', ', $ambiguous) . '"';
            }
            if ($description->getCompatibleOperatingSystems() !== []) {
                $os = strtolower(PHP_OS_FAMILY ?: 'unknown');
                $compat = array_map('strtolower', $description->getCompatibleOperatingSystems());
                if (!in_array($os, $compat, true) && !in_array('any', $compat, true)) {
                    return 'Incompatible OS "' . implode(', ', $description->getCompatibleOperatingSystems()) . '"';
                }
            }
            foreach ($description->getRequiredExtensions() as $extensionName => $constraints) {
                if (!extension_loaded($extensionName)) {
                    return 'Extension "' . $extensionName . '" is not loaded';
                }
                $got = phpversion($extensionName);
                $got = $got === false ? '**UNKNOWN**' : (string)$got;
                foreach ($constraints as $index => $constraint) {
                    if ($constraint === '*') {
                        continue;
                    }
                    if ($constraint === '') {
                        return 'Extension "' . $extensionName . '" has empty version constraint #' . $index;
                    }
                    $matched = false;
                    foreach (['<=', 'le', '<>', '!=', 'ne', '<', 'lt', '==', '=', 'eq', '>=', 'ge', '>', 'gt'] as $comparator) {
                        if (str_starts_with($constraint, $comparator)) {
                            $version = substr($constraint, strlen($comparator));
                            $op = match ($comparator) {
                                'le' => '<=', 'ne' => '!=', 'lt' => '<', 'eq' => '==',
                                'ge' => '>=', 'gt' => '>', default => $comparator,
                            };
                            if (!version_compare($got, $version, $op)) {
                                return 'Incompatible extension version "' . $extensionName . ' ' . $got
                                    . '\" (requires "' . $constraint . '")';
                            }
                            $matched = true;
                            break;
                        }
                    }
                    if (!$matched) {
                        return 'Invalid extension version constraint "' . $constraint . '" for "' . $extensionName . '"';
                    }
                }
            }
            return null;
        }

        /** @param string[] $want */
        private function isApiCompatible(string $myVersion, array $want): bool
        {
            $my = $this->parseVersion($myVersion);
            if ($my === null) {
                return true;
            }
            foreach ($want as $wantVersion) {
                $version = $this->parseVersion($wantVersion);
                if ($version === null) {
                    continue;
                }
                if ($version['base'] === $my['base']) {
                    return true;
                }
                if ($version['major'] !== $my['major']) {
                    continue;
                }
                if ($version['minor'] > $my['minor']) {
                    continue;
                }
                if ($version['minor'] === $my['minor'] && $version['patch'] > $my['patch']) {
                    continue;
                }
                return true;
            }
            return false;
        }

        /** @param string[] $versions @return string[] */
        private function checkAmbiguousVersions(array $versions): array
        {
            $byMajor = [];
            foreach ($versions as $str) {
                $parsed = $this->parseVersion($str);
                if ($parsed === null || $parsed['suffix'] !== '') {
                    continue;
                }
                $byMajor[$parsed['major']][] = $str;
            }
            $out = [];
            foreach ($byMajor as $list) {
                if (count($list) > 1) {
                    $out = array_merge($out, $list);
                }
            }
            return array_values($out);
        }

        /** @return array{major:int, minor:int, patch:int, suffix:string, base:string}|null */
        private function parseVersion(string $version): ?array
        {
            if (preg_match('/^(\d+)\.(\d+)\.(\d+)(?:-(.*))?$/', $version, $m) !== 1) {
                return null;
            }
            return [
                'major' => (int)$m[1],
                'minor' => (int)$m[2],
                'patch' => (int)$m[3],
                'suffix' => $m[4] ?? '',
                'base' => $version,
            ];
        }

        private function internalLoadPlugin(string $path, PluginLoader $loader, PluginDescription $description): ?Plugin
        {
            $this->log('Loading ' . $description->getFullName());

            $dataFolder = $this->getDataDirectory($path, $description->getName());
            if (file_exists($dataFolder) && !is_dir($dataFolder)) {
                $this->log("Could not load plugin '" . $description->getName() . "': Data folder " . $dataFolder . ' is not a directory');
                return null;
            }
            if (!file_exists($dataFolder)) {
                @mkdir($dataFolder, 0777, true);
            }

            $isPhar = $loader instanceof PharPluginLoader
                || str_ends_with($path, '.phar')
                || stripos($path, 'phar://') === 0;
            $root = $isPhar
                ? 'phar://' . ltrim(str_replace('\\', '/', $path), '/')
                : $path;
            $root = rtrim(str_replace('\\', '/', $root), '/');

            $mainClass = $description->getMain();
            $srcPrefix = $description->getSrcNamespacePrefix();
            if (class_exists(\pocketmine\runtime\PluginAutoloader::class)) {
                try {
                    \pocketmine\runtime\PluginAutoloader::registerPlugin(
                        $description->getName(),
                        $root,
                        $mainClass,
                        $srcPrefix
                    );
                } catch (\Throwable $e) {
                    $this->log('Autoloader failed for ' . $description->getName() . ': ' . $e->getMessage());
                }
            }

            $relativeMain = str_replace('\\', '/', ltrim($mainClass, '\\')) . '.php';
            // `src/` under the manifest prefix (`BetterChat\Main` -> `src/Main.php`).
            $stripped = ltrim($mainClass, '\\');
            $explicitNs = rtrim(str_replace('/', '\\', $srcPrefix), '\\');
            if ($explicitNs !== '' && stripos($stripped, $explicitNs . '\\') === 0) {
                $stripped = substr($stripped, strlen($explicitNs) + 1);
            }
            $candidates = [
                $root . '/src/' . $relativeMain,
                $root . '/src/' . str_replace('\\', '/', $stripped) . '.php',
                $root . '/' . $relativeMain,
            ];
            $resolvedMain = null;
            foreach ($candidates as $candidate) {
                if (class_exists(\pocketmine\runtime\PluginAutoloader::class)) {
                    $resolvedMain = \pocketmine\runtime\PluginAutoloader::resolve($candidate);
                } else {
                    $resolvedMain = is_file($candidate) ? str_replace('\\', '/', $candidate) : null;
                }
                if ($resolvedMain !== null) {
                    break;
                }
            }
            $mainFile = $resolvedMain ?? ($root . '/src/' . $relativeMain);
            if ($resolvedMain !== null) {
                try {
                    require_once $resolvedMain;
                    $mainFile = $resolvedMain;
                } catch (\Throwable $e) {
                    $this->log("Could not load plugin '" . $description->getName() . "': Failed to include " . $mainFile . ': ' . $e->getMessage());
                    return null;
                }
            }

            if (!class_exists($mainClass, true)) {
                $this->log("Could not load plugin '" . $description->getName() . "': Main class not found: $mainClass");
                return null;
            }
            if (!is_a($mainClass, Plugin::class, true)) {
                $this->log("Could not load plugin '" . $description->getName() . "': Main class $mainClass does not implement Plugin");
                return null;
            }
            try {
                $reflect = new \ReflectionClass($mainClass);
                if (!$reflect->isInstantiable()) {
                    $this->log("Could not load plugin '" . $description->getName() . "': Main class $mainClass is abstract");
                    return null;
                }
            } catch (\Throwable) {
                // class_exists already passed; treat reflection failure as fatal.
            }

            // Permissions (PM internalLoadPlugin): duplicates are fatal.
            $permManager = \pocketmine\permission\PermissionManager::getInstance();
            $permSpecs = $description->getPermissions();
            $parsedPerms = [];
            foreach ($permSpecs as $permName => $spec) {
                if (!is_string($permName) || $permName === '') {
                    continue;
                }
                $default = 'op';
                $label = null;
                if (is_array($spec)) {
                    $rawDefault = $spec['default'] ?? 'op';
                    if (is_bool($rawDefault)) {
                        $default = $rawDefault ? 'true' : 'false';
                    } elseif ($rawDefault === null) {
                        $default = 'op';
                    } else {
                        try {
                            $default = \pocketmine\permission\PermissionParser::defaultFromString((string)$rawDefault);
                        } catch (\Throwable) {
                            $default = strtolower((string)$rawDefault);
                            if (!in_array($default, ['true', 'false', 'op', 'notop'], true)) {
                                $default = 'op';
                            }
                        }
                    }
                    $label = isset($spec['description']) ? (string)$spec['description'] : null;
                } elseif (is_bool($spec)) {
                    $default = $spec ? 'true' : 'false';
                } elseif (is_string($spec)) {
                    $default = strtolower($spec);
                    if (!in_array($default, ['true', 'false', 'op', 'notop'], true)) {
                        $default = 'op';
                    }
                }
                if ($permManager->getPermission($permName) !== null) {
                    $this->log("Could not load plugin '" . $description->getName() . "': Duplicate permission '" . $permName . "'");
                    return null;
                }
                $parsedPerms[] = new \pocketmine\permission\Permission($permName, $label, $default);
            }
            foreach ($parsedPerms as $perm) {
                $permManager->registerPermission($perm);
            }

            try {
                $plugin = new $mainClass();
            } catch (\Throwable $e) {
                $this->log("Could not load plugin '" . $description->getName() . "': Failed to instantiate $mainClass: " . $e->getMessage());
                return null;
            }
            if (!$plugin instanceof PluginBase) {
                $this->log("Could not load plugin '" . $description->getName() . "': Main class $mainClass must extend PluginBase");
                return null;
            }

            $server = $this->server;
            if (!$server instanceof \pocketmine\Server) {
                try {
                    $server = \pocketmine\Server::getInstance();
                } catch (\Throwable $e) {
                    $this->log("Could not load plugin '" . $description->getName() . "': No Server instance: " . $e->getMessage());
                    return null;
                }
            }
            try {
                $plugin->init($description, $dataFolder, $mainFile, $root, $server);
                if (method_exists($plugin, 'setPluginLoader')) {
                    $plugin->setPluginLoader($loader);
                }
            } catch (\Throwable $e) {
                $this->log("Could not load plugin '" . $description->getName() . "': Plugin init failed: " . $e->getMessage());
                return null;
            }

            // Commands declared in plugin.yml (mirrors the IPC load_plugin path).
            try {
                $commands = $this->createCommandsFromDescription($description);
                if (method_exists($plugin, 'setCommands')) {
                    $plugin->setCommands($commands);
                }
            } catch (\Throwable $e) {
                $this->log("Could not load plugin '" . $description->getName() . "': Failed to create commands: " . $e->getMessage());
            }

            try {
                $plugin->onLoad();
            } catch (\Throwable $e) {
                $this->log("Could not load plugin '" . $description->getName() . "': onLoad failed: " . $e->getMessage());
                return null;
            }

            try {
                $this->registerPlugin($plugin);
            } catch (\Throwable $e) {
                $this->log("Could not load plugin '" . $description->getName() . "': " . $e->getMessage());
                return null;
            }
            return $plugin;
        }

        /** @return array<string, object> */
        private function createCommandsFromDescription(PluginDescription $description): array
        {
            if (!class_exists(\pocketmine\command\SimpleCommand::class)) {
                return [];
            }
            $commands = [];
            foreach ($description->getCommands() as $name => $spec) {
                if (!is_array($spec)) {
                    $spec = [];
                }
                $commandName = is_string($name) ? $name : (string)($spec['name'] ?? '');
                if ($commandName === '') {
                    continue;
                }
                $aliases = $spec['aliases'] ?? [];
                if (!is_array($aliases)) {
                    $aliases = $aliases === null || $aliases === '' ? [] : [(string)$aliases];
                }
                $usage = $spec['usage'] ?? ('/' . $commandName);
                if (is_array($usage)) {
                    $usage = implode('|', array_map('strval', $usage));
                }
                $commands[strtolower($commandName)] = new \pocketmine\command\SimpleCommand(
                    $commandName,
                    isset($spec['description']) ? (string)$spec['description'] : null,
                    (string)$usage,
                    array_values(array_map('strval', $aliases)),
                    isset($spec['permission']) && $spec['permission'] !== '' ? (string)$spec['permission'] : null
                );
            }
            return $commands;
        }

        private function getDataDirectory(string $pluginPath, string $pluginName): string
        {
            $base = $this->server instanceof \pocketmine\Server
                ? $this->server->getDataPath()
                : rtrim(getcwd() ?: '.', '/\\') . DIRECTORY_SEPARATOR;
            return rtrim($base, '/\\') . DIRECTORY_SEPARATOR . 'plugin_data' . DIRECTORY_SEPARATOR . $pluginName;
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

        /** PM `Utils::getNiceClosureName()` subset. */
        private static function niceClosureName(\Closure $handler): string
        {
            try {
                $ref = new \ReflectionFunction($handler);
                $bound = $ref->getClosureThis();
                $boundClass = $bound !== null ? $bound::class : null;
                $scope = $ref->getClosureScopeClass()?->getName();
                $name = $scope ?? $boundClass;
                // Bound closures created via Closure::fromCallable([$obj, $method])
                // keep the method in the closure's scope; fall back to file/line.
                if ($name !== null) {
                    $method = $ref->getName();
                    // ReflectionFunction::getName() returns "{closure}"; try to
                    // recover the wrapped method name from the static variables.
                    return $name . '::' . $method;
                }
                return $ref->getFileName() . ':' . $ref->getStartLine();
            } catch (\Throwable) {
                return '{closure}';
            }
        }

        private function log(string $message): void
        {
            fwrite(STDERR, '[PHP/RUNTIME] ' . $message . PHP_EOL);
        }
    }
}

namespace pocketmine\event\plugin {
    use pocketmine\event\Event;
    use pocketmine\plugin\Plugin;

    /**
     * Base for plugin lifecycle events (PM `PluginEvent`).
     * Fired by `PluginManager::enablePlugin()` / `disablePlugin()` only; these
     * events never cross the IPC boundary (Rust has no lifecycle events).
     */
    abstract class PluginEvent extends Event
    {
        public function __construct(protected Plugin $plugin)
        {
        }

        public function getPlugin(): Plugin
        {
            return $this->plugin;
        }

        public static function fromArray(array $data): Event
        {
            throw new \BadMethodCallException(static::class . ' cannot be hydrated from IPC data');
        }

        public function toArray(): array
        {
            return [
                'plugin' => $this->plugin->getName(),
                'cancelled' => $this->isCancelled(),
            ];
        }
    }

    final class PluginEnableEvent extends PluginEvent
    {
    }

    final class PluginDisableEvent extends PluginEvent
    {
    }
}



