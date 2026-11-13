<?php
declare(strict_types=1);

namespace pocketmine\runtime {
    use pocketmine\command\ConsoleCommandSender;
    use pocketmine\event\Event;
    use pocketmine\event\EventFactory;
    use pocketmine\player\Player;
    use pocketmine\plugin\Plugin;
    use pocketmine\plugin\PluginBase;
    use pocketmine\plugin\PluginDescription;
    use pocketmine\plugin\PluginManager;

    final class Runtime
    {
        private static bool $initialized = false;
        private static bool $shutdown = false;
        private static mixed $server = null;
        private static ?PluginManager $pluginManager = null;
        private static int $tick = 0;
        /**
         * STDOUT (pipe IPC hacia Rust) está roto: Rust cerró el worker
         * (shutdown / restart / kill por timeout). A partir de ahí ningún
         * fwrite a STDOUT debe intentarse: en Windows produce
         * `fwrite(): Write of N bytes failed with errno=22 Invalid argument`
         * en cada respuesta/host_call y los notices se acumulan al apagar.
         */
        private static bool $ipcBroken = false;

        /** @var array<string, PluginBase> lowercase name => plugin */
        private static array $plugins = [];

        public static function main(): void
        {
            @ini_set('display_errors', 'stderr');
            self::initialize();
            fwrite(STDERR, '[PHP/RUNTIME] Runtime ready' . PHP_EOL);

            while (!self::$shutdown && !self::$ipcBroken) {
                $line = Bridge::$pendingResponse;
                if ($line !== null) {
                    Bridge::$pendingResponse = null;
                } elseif (defined('STDIN') && is_resource(STDIN)) {
                    $line = @fgets(STDIN);
                } else {
                    break;
                }

                if ($line === false) {
                    break;
                }
                $line = trim($line);
                if ($line === '') {
                    continue;
                }
                self::handleLine($line);
            }
        }

        public static function initialize(): void
        {
            if (self::$initialized) {
                return;
            }
            self::$initialized = true;

            register_shutdown_function(static function (): void {
                $error = error_get_last();
                if ($error !== null && in_array($error['type'], [E_ERROR, E_PARSE, E_CORE_ERROR, E_COMPILE_ERROR, E_USER_ERROR], true)) {
                    $prefix = match ($error['type']) {
                        E_PARSE => 'Parse error',
                        E_COMPILE_ERROR => 'Compile error',
                        E_CORE_ERROR => 'Core error',
                        default => 'Fatal error',
                    };
                    $msg = sprintf(
                        '%s: %s in %s on line %d',
                        $prefix,
                        $error['message'],
                        $error['file'],
                        $error['line']
                    );
                    if (!self::$ipcBroken && defined('STDOUT') && is_resource(STDOUT)) {
                        @fwrite(STDOUT, json_encode([
                            'status' => 'fatal_error',
                            'message' => $msg,
                        ]) . "\n");
                        @fflush(STDOUT);
                    }
                    fwrite(STDERR, '[PHP/FATAL] ' . $msg . PHP_EOL);
                }
            });

            PluginAutoloader::register();
            self::$server = self::server();

            $server = self::$server;
            $manager = PluginManager::getInstance($server instanceof \pocketmine\Server ? $server : null);

            if (is_object($server)) {
                try {
                    $existing = method_exists($server, 'getPluginManager')
                        ? $server->getPluginManager()
                        : null;
                    if ($existing instanceof PluginManager) {
                        if ($existing !== $manager) {
                            PluginManager::useInstance(
                                $existing,
                                $server instanceof \pocketmine\Server ? $server : null
                            );
                            $manager = $existing;
                        }
                    } elseif (method_exists($server, 'setPluginManager')) {
                        $server->setPluginManager($manager);
                    } else {
                        self::injectPluginManager($server, $manager);
                    }
                } catch (\Throwable $e) {
                    self::log('Unable to attach PluginManager to Server: ' . $e->getMessage());
                }
            }

            self::$pluginManager = $manager;
        }

        public static function getServer(): mixed
        {
            return self::$server;
        }

        public static function getPluginManager(): PluginManager
        {
            if (self::$pluginManager === null) {
                self::initialize();
            }
            return self::$pluginManager;
        }

        /** @return array<string, PluginBase> */
        public static function getPlugins(): array
        {
            return self::$plugins;
        }

        public static function collectActions(): array
        {
            try {
                $server = self::server();
                if (!is_object($server)) {
                    return [];
                }
                if (method_exists($server, 'takeActions')) {
                    $actions = $server->takeActions();
                    return is_array($actions) ? array_values($actions) : [];
                }
                if (property_exists($server, 'actions') && is_array($server->actions)) {
                    $actions = $server->actions;
                    $server->actions = [];
                    return array_values($actions);
                }
            } catch (\Throwable $e) {
                self::log('Unable to collect actions: ' . $e->getMessage());
            }
            return [];
        }

        private static function handleLine(string $line): void
        {
            $request = json_decode($line, true);
            if (!is_array($request)) {
                self::write(['status' => 'error', 'message' => 'Invalid JSON request', 'actions' => []]);
                return;
            }

            $type = (string)($request['type'] ?? '');
            $action = (string)($request['action'] ?? '');

            // Late host results must never be mistaken for a new request.
            if ($action === 'call_result' || $type === 'host_result') {
                self::log('Ignoring unexpected host result line');
                return;
            }

            try {
                Bridge::setActive(true);
                $payload = self::handleAction($action, $request);
            } catch (\Throwable $e) {
                self::log('Unhandled error for action ' . ($action !== '' ? $action : '(none)') . ': ' . $e->getMessage());
                $payload = ['status' => 'error', 'message' => $e->getMessage()];
            } finally {
                Bridge::setActive(false);
            }

            $payload['status'] = (string)($payload['status'] ?? 'ok');
            if (!array_key_exists('message', $payload)) {
                $payload['message'] = null;
            }
            $payload['actions'] = self::collectActions();

            self::write($payload);

            if ($action === 'shutdown') {
                self::$shutdown = true;
            }
        }

        private static function handleAction(string $action, array $request): array
        {
            return match ($action) {
                'ping' => self::actionPing($request),
                'discover_plugins' => self::actionDiscoverPlugins($request),
                'load_plugin' => self::actionLoadPlugin($request),
                'enable_plugin' => self::actionEnablePlugin($request),
                'disable_plugin' => self::actionDisablePlugin($request),
                'dispatch_event' => self::actionDispatchEvent($request),
                'execute_command' => self::actionExecuteCommand($request),
                'tick' => self::actionTick($request),
                'shutdown' => self::actionShutdown($request),
                default => ['status' => 'error', 'message' => 'Unknown action: ' . $action],
            };
        }

        private static function actionPing(array $request): array
        {
            return ['status' => 'ok', 'data' => 'pong'];
        }

        private static function actionDiscoverPlugins(array $request): array
        {
            $directory = (string)($request['dir'] ?? $request['directory'] ?? 'plugins');
            $plugins = [];

            if (!is_dir($directory)) {
                return ['status' => 'ok', 'plugins' => $plugins];
            }

            $entries = @scandir($directory);
            if ($entries === false) {
                return ['status' => 'error', 'message' => 'Unable to read directory: ' . $directory, 'plugins' => []];
            }

            foreach ($entries as $entry) {
                if ($entry === '.' || $entry === '..') {
                    continue;
                }
                $path = rtrim($directory, '/\\') . DIRECTORY_SEPARATOR . $entry;

                try {
                    if (is_dir($path)) {
                        $manifestPath = rtrim($path, '/\\') . DIRECTORY_SEPARATOR . 'plugin.yml';
                        if (!is_file($manifestPath)) {
                            continue;
                        }
                        $manifest = @file_get_contents($manifestPath);
                        if ($manifest === false) {
                            continue;
                        }
                        $plugins[] = [
                            'kind' => 'dir',
                            'path' => self::normalizeFsPath(realpath($path) ?: $path),
                            'manifest' => $manifest,
                        ];
                        continue;
                    }

                    if (!is_file($path) || strtolower(pathinfo($entry, PATHINFO_EXTENSION)) !== 'phar') {
                        continue;
                    }

                    $real = realpath($path) ?: $path;
                    $manifest = self::readPharEntry($real, 'plugin.yml');
                    if ($manifest === null) {
                        if (!self::pharAvailable()) {
                            // Not fatal: the server keeps running without
                            // phar support; the operator just cannot use .phar
                            // plugins until extension=phar is enabled.
                            self::log(
                                'Skipping ' . $path . ': the PHP phar extension appears to be disabled; '
                                . 'enable extension=phar and set phar.readonly=Off in php.ini to load .phar plugins'
                            );
                        } else {
                            self::log('Skipping phar without plugin.yml: ' . $path);
                        }
                        continue;
                    }
                    $plugins[] = [
                        'kind' => 'phar',
                        'path' => self::normalizeFsPath($real),
                        'manifest' => $manifest,
                    ];
                } catch (\Throwable $e) {
                    self::log('Failed to inspect ' . $path . ': ' . $e->getMessage());
                }
            }

            return ['status' => 'ok', 'plugins' => $plugins];
        }

        private static function actionLoadPlugin(array $request): array
        {
            $name = (string)($request['name'] ?? '');
            $main = (string)($request['main'] ?? $request['main_class'] ?? '');
            if ($name === '' || $main === '') {
                return ['status' => 'error', 'message' => 'load_plugin requires name and main'];
            }

            if (isset(self::$plugins[strtolower($name)]) || self::getPluginManager()->getPlugin($name) !== null) {
                return ['status' => 'error', 'message' => 'Plugin already loaded: ' . $name];
            }

            $pluginDir = (string)($request['plugin_dir'] ?? $request['pluginDir'] ?? '');
            $file = (string)($request['file'] ?? '');
            if ($pluginDir === '') {
                $pluginDir = $file !== '' ? dirname($file) : '';
            }
            $root = rtrim(str_replace('\\', '/', $pluginDir), '/');

            $dataFolder = (string)($request['data_folder'] ?? $request['dataFolder'] ?? '');
            if ($dataFolder === '') {
                $dataFolder = getcwd() . DIRECTORY_SEPARATOR . 'plugin_data' . DIRECTORY_SEPARATOR . $name;
            }
            if (!is_dir($dataFolder)) {
                @mkdir($dataFolder, 0777, true);
            }

            PluginAutoloader::registerPlugin($name, $root, $main, self::requestedSrcPrefix($request));

            $relativeMain = str_replace('\\', '/', ltrim($main, '\\')) . '.php';
            $mainFile = $file !== ''
                ? str_replace('\\', '/', $file)
                : $root . '/src/' . $relativeMain;
            $candidates = [$mainFile, $root . '/src/' . $relativeMain];
            // `src-namespace-prefix` layout (`BetterChat\Main` -> `src/Main.php`).
            $stripped = ltrim($main, '\\');
            $prefixNs = rtrim(str_replace('/', '\\', self::requestedSrcPrefix($request)), '\\');
            if ($prefixNs !== '' && stripos($stripped, $prefixNs . '\\') === 0) {
                $stripped = substr($stripped, strlen($prefixNs) + 1);
            }
            $candidates[] = $root . '/src/' . str_replace('\\', '/', $stripped) . '.php';
            $candidates[] = $root . '/' . $relativeMain;
            $resolvedMain = null;
            foreach ($candidates as $candidate) {
                $resolvedMain = PluginAutoloader::resolve($candidate);
                if ($resolvedMain !== null) {
                    break;
                }
            }

            if ($resolvedMain !== null) {
                try {
                    require_once $resolvedMain;
                    $mainFile = $resolvedMain;
                } catch (\Throwable $e) {
                    return ['status' => 'error', 'message' => 'Failed to include ' . $mainFile . ': ' . $e->getMessage()];
                }
            }

            if (!class_exists($main)) {
                return [
                    'status' => 'error',
                    'message' => 'Main class not found: ' . $main . ' (file: ' . $mainFile . ')',
                ];
            }

            try {
                $instance = new $main();
            } catch (\Throwable $e) {
                return ['status' => 'error', 'message' => 'Failed to instantiate ' . $main . ': ' . $e->getMessage()];
            }

            if (!$instance instanceof PluginBase) {
                return ['status' => 'error', 'message' => 'Main class ' . $main . ' must extend PluginBase'];
            }

            $rawDescription = $request['description'] ?? [];
            if (is_string($rawDescription)) {
                $parsed = function_exists('yaml_parse') ? @yaml_parse($rawDescription) : null;
                $rawDescription = is_array($parsed) ? $parsed : [];
            }
            if (!is_array($rawDescription)) {
                $rawDescription = [];
            }
            if (isset($request['commands']) && is_array($request['commands']) && !isset($rawDescription['commands'])) {
                $rawDescription['commands'] = $request['commands'];
            }
            $rawDescription['name'] = $name;
            $rawDescription['main'] = $main;
            if (!isset($rawDescription['version'])) {
                $rawDescription['version'] = (string)($request['version'] ?? '');
            }
            $description = PluginDescription::fromArray($rawDescription);

            self::registerPermissions($description);

            try {
                $instance->init($description, $dataFolder, $mainFile, $root, self::serverInstance());
            } catch (\Throwable $e) {
                return ['status' => 'error', 'message' => 'Plugin init failed: ' . $e->getMessage()];
            }

            $commands = self::createCommands($description, $instance);
            if (method_exists($instance, 'setCommands')) {
                $instance->setCommands($commands);
            }

            try {
                if (method_exists($instance, 'onPluginLoad')) {
                    $instance->onPluginLoad();
                } elseif (method_exists($instance, 'onLoad')) {
                    (\Closure::bind(function () { $this->onLoad(); }, $instance, PluginBase::class))();
                }
            } catch (\Throwable $e) {
                return ['status' => 'error', 'message' => 'onLoad failed for ' . $name . ': ' . $e->getMessage()];
            }

            try {
                self::getPluginManager()->registerPlugin($instance);
            } catch (\Throwable $e) {
                return ['status' => 'error', 'message' => $e->getMessage()];
            }

            $server = self::$server;
            if (is_object($server) && method_exists($server, 'registerPlugin')) {
                try {
                    $server->registerPlugin($instance);
                } catch (\Throwable $e) {
                    self::log('Server registerPlugin failed for ' . $name . ': ' . $e->getMessage());
                }
            }

            self::$plugins[strtolower($name)] = $instance;

            return [
                'status' => 'ok',
                'commands' => array_keys($instance->getCommands()),
                'message' => 'Plugin ' . $name . ' loaded',
            ];
        }

        private static function actionEnablePlugin(array $request): array
        {
            $name = (string)($request['name'] ?? '');
            $plugin = self::getPluginManager()->getPlugin($name);
            if ($plugin === null) {
                return ['status' => 'error', 'message' => 'Plugin not loaded: ' . $name];
            }

            try {
                self::getPluginManager()->enablePlugin($plugin);
            } catch (\Throwable $e) {
                return ['status' => 'error', 'message' => 'onEnable failed: ' . $e->getMessage()];
            }

            $commands = self::registerCommands($plugin);
            $server = self::$server;
            if (is_object($server) && method_exists($server, 'syncCommands')) {
                try {
                    $server->syncCommands();
                } catch (\Throwable) {
                    // Optional.
                }
            }

            return [
                'status' => 'ok',
                'commands' => $commands,
                'listeners' => self::getPluginManager()->getListenerCount($plugin),
                'listener_events' => self::getPluginManager()->getListenerEvents(),
                'message' => 'Plugin ' . $plugin->getName() . ' enabled',
            ];
        }

        /** `src-namespace-prefix` carried by the Rust-side description JSON. */
        private static function requestedSrcPrefix(array $request): string
        {
            $raw = $request['description'] ?? [];
            if (!is_array($raw)) {
                return '';
            }
            $prefix = (string)($raw['src-namespace-prefix'] ?? $raw['src_namespace_prefix'] ?? '');
            return trim(str_replace('/', '\\', $prefix), '\\');
        }

        private static function actionDisablePlugin(array $request): array
        {
            $name = (string)($request['name'] ?? '');
            $plugin = self::getPluginManager()->getPlugin($name);
            if ($plugin === null) {
                // Still drop any stale autoloader entry so a later load of
                // the same name starts clean (load/unload cycles).
                PluginAutoloader::unregisterPlugin($name);
                unset(self::$plugins[strtolower($name)]);
                return ['status' => 'ok', 'message' => 'Plugin not loaded: ' . $name];
            }

            try {
                self::getPluginManager()->disablePlugin($plugin);
            } catch (\Throwable $e) {
                self::log('onDisable failed for ' . $name . ': ' . $e->getMessage());
            }

            $commandMap = self::$server instanceof \pocketmine\Server ? self::$server->getCommandMap() : null;
            foreach ($plugin->getCommands() as $command) {
                $commandMap?->unregister($command);
            }

            // Full unload: free the isolated autoloader entry (name + root)
            // and the runtime registry, so the plugin can be loaded again
            // and sibling phar archives are unaffected.
            PluginAutoloader::unregisterPlugin($plugin->getName());
            PluginAutoloader::unregisterPlugin($name);
            unset(self::$plugins[strtolower($plugin->getName())]);

            return [
                'status' => 'ok',
                'commands' => [],
                'listeners' => self::getPluginManager()->getListenerCount(),
                'listener_events' => self::getPluginManager()->getListenerEvents(),
                'message' => 'Plugin ' . $name . ' disabled',
            ];
        }

        private static function actionDispatchEvent(array $request): array
        {
            $eventName = (string)($request['event'] ?? '');
            $data = $request['data'] ?? [];
            if (!is_array($data)) {
                $data = [];
            }

            $event = EventFactory::create($eventName, $data);
            if ($event === null) {
                return ['status' => 'error', 'message' => 'Unknown event: ' . $eventName];
            }

            // Keep the PHP-side player registry in sync with the core before
            // listeners run, so plugins can resolve players during the event.
            $server = self::$server;
            if ($server instanceof \pocketmine\Server) {
                try {
                    if ($event instanceof \pocketmine\event\player\PlayerJoinEvent) {
                        $server->addPlayer($event->getPlayer());
                    } elseif ($event instanceof \pocketmine\event\player\PlayerQuitEvent) {
                        $server->removePlayer($event->getPlayer());
                    }
                } catch (\Throwable $e) {
                    self::log('Player registry sync failed: ' . $e->getMessage());
                }
            }

            self::getPluginManager()->dispatchEvent($event);

            if ($server instanceof \pocketmine\Server) {
                try {
                    if ($event instanceof \pocketmine\event\player\PlayerQuitEvent) {
                        $server->removePlayer($event->getPlayer());
                    }
                } catch (\Throwable) {
                    // Already removed.
                }
            }

            return [
                'status' => 'ok',
                'event' => $event->toArray(),
                'cancelled' => $event->isCancelled(),
            ];
        }

        private static function actionExecuteCommand(array $request): array
        {
            $senderData = $request['sender'] ?? ['type' => 'console'];
            $sender = self::buildSender(is_array($senderData) ? $senderData : ['type' => 'console']);

            $command = (string)($request['command'] ?? '');
            $args = $request['args'] ?? [];
            if (is_string($args)) {
                $args = preg_split('/\s+/', trim($args)) ?: [];
            }
            if (!is_array($args)) {
                $args = [];
            }
            $args = array_values(array_map('strval', $args));

            $line = trim($command);
            if ($line === '') {
                return ['status' => 'ok', 'handled' => false, 'message' => 'Empty command'];
            }
            if ($args !== []) {
                $line .= ' ' . implode(' ', $args);
            }

            $server = self::$server;
            $commandMap = $server instanceof \pocketmine\Server ? $server->getCommandMap() : null;
            if ($commandMap === null) {
                return ['status' => 'ok', 'handled' => false];
            }

            $handled = $commandMap->dispatch($sender, $line);
            return [
                'status' => 'ok',
                'handled' => $handled,
                'message' => $handled ? null : 'Unknown command: ' . $command,
            ];
        }

        private static function actionTick(array $request): array
        {
            self::$tick = (int)($request['tick'] ?? self::$tick + 1);
            $server = self::$server;
            if ($server instanceof \pocketmine\Server) {
                $server->setTick(self::$tick);
            }

            $scheduler = self::getPluginManager()->getScheduler();
            try {
                if (is_object($scheduler) && method_exists($scheduler, 'mainThreadHeartbeat')) {
                    $scheduler->mainThreadHeartbeat(self::$tick);
                }
            } catch (\Throwable $e) {
                self::log('Scheduler heartbeat failed: ' . $e->getMessage());
            }

            $pending = 0;
            if (is_object($scheduler) && method_exists($scheduler, 'getPendingTaskCount')) {
                $pending = (int)$scheduler->getPendingTaskCount();
            }

            return ['status' => 'ok', 'tick' => self::$tick, 'pending_tasks' => $pending];
        }

        private static function actionShutdown(array $request): array
        {
            foreach (self::getPluginManager()->getPlugins() as $plugin) {
                try {
                    self::getPluginManager()->disablePlugin($plugin);
                } catch (\Throwable $e) {
                    self::log('Failed to disable ' . $plugin->getName() . ': ' . $e->getMessage());
                }
            }
            return ['status' => 'ok', 'message' => 'Runtime shut down'];
        }

        private static function registerPermissions(PluginDescription $description): void
        {
            $manager = \pocketmine\permission\PermissionManager::getInstance();
            foreach ($description->getPermissions() as $name => $spec) {
                if (!is_string($name) || $name === '') {
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
                        $default = strtolower((string)$rawDefault);
                    }
                    $label = isset($spec['description']) ? (string)$spec['description'] : null;
                } elseif (is_bool($spec)) {
                    $default = $spec ? 'true' : 'false';
                } elseif (is_string($spec)) {
                    $default = strtolower($spec);
                }
                if (!in_array($default, ['true', 'false', 'op', 'notop'], true)) {
                    $default = 'op';
                }
                $manager->registerPermission(new \pocketmine\permission\Permission($name, $label, $default));
            }
        }

        private static function buildSender(array $data): \pocketmine\command\CommandSender
        {
            $type = strtolower((string)($data['type'] ?? 'console'));
            if ($type === 'player') {
                if (isset($data['player']) && is_array($data['player'])) {
                    $data = array_merge($data['player'], $data);
                }
                return new Player($data);
            }
            return ConsoleCommandSender::getInstance();
        }

        /**
         * @return array<string, \pocketmine\command\Command>
         */
        private static function createCommands(PluginDescription $description, PluginBase $plugin): array
        {
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

        /**
         * @return array<int, string>
         */
        private static function registerCommands(PluginBase $plugin): array
        {
            $server = self::$server;
            if (!$server instanceof \pocketmine\Server) {
                return [];
            }
            $commandMap = $server->getCommandMap();
            $registered = [];
            foreach ($plugin->getCommands() as $name => $command) {
                $commandMap->register($plugin->getName(), $command);
                $registered[] = (string)$name;
            }
            return $registered;
        }

        private static function serverInstance(): ?\pocketmine\Server
        {
            return self::$server instanceof \pocketmine\Server ? self::$server : null;
        }

        private static function server(): mixed
        {
            if (!class_exists(\pocketmine\Server::class)) {
                return null;
            }
            try {
                return \pocketmine\Server::getInstance();
            } catch (\Throwable $e) {
                self::log('Unable to obtain Server instance: ' . $e->getMessage());
                return null;
            }
        }

        private static function injectPluginManager(mixed $server, PluginManager $manager): void
        {
            if (!is_object($server)) {
                return;
            }
            try {
                $property = new \ReflectionProperty($server, 'pluginManager');
                $property->setAccessible(true);
                $property->setValue($server, $manager);
            } catch (\Throwable $e) {
                self::log('Unable to inject PluginManager: ' . $e->getMessage());
            }
        }

        private static function normalizeFsPath(string $path): string
        {
            return str_replace('\\', '/', $path);
        }

        /** Whether PHP here can read Phar archives (extension + wrapper). */
        private static function pharAvailable(): bool
        {
            return class_exists(\Phar::class) || in_array('phar', stream_get_wrappers(), true);
        }

        /**
         * Reads one entry from a `.phar`, preferring the `Phar` class and
         * falling back to the `phar://` stream wrapper. Returns null when
         * the entry is absent or the archive cannot be opened (including a
         * disabled Phar extension); callers log a specific message.
         */
        private static function readPharEntry(string $pharPath, string $entry): ?string
        {
            if (!self::pharAvailable()) {
                return null;
            }
            $file = self::normalizeFsPath($pharPath);
            $entry = ltrim(str_replace('\\', '/', $entry), '/');
            if (class_exists(\Phar::class)) {
                try {
                    $phar = new \Phar($file);
                    if (isset($phar[$entry])) {
                        $contents = $phar[$entry]->getContent();
                        if (is_string($contents) && $contents !== '') {
                            return $contents;
                        }
                    }
                    return null;
                } catch (\Throwable) {
                    // Not a valid Phar: fall through to the phar:// wrapper.
                }
            }
            $uri = 'phar://' . $file . '/' . $entry;
            $contents = @file_get_contents($uri);
            return ($contents === false || $contents === '') ? null : $contents;
        }

        private static function write(array $payload): void
        {
            if (self::$ipcBroken) {
                return;
            }
            if (!defined('STDOUT') || !is_resource(STDOUT)) {
                self::$ipcBroken = true;
                Bridge::markBroken();
                return;
            }
            $json = json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE);
            if ($json === false) {
                $json = json_encode([
                    'status' => 'error',
                    'message' => 'Failed to encode response: ' . json_last_error_msg(),
                    'actions' => [],
                ]);
            }
            $line = $json . PHP_EOL;
            // @: si Rust ya cerró el pipe (shutdown/restart) fwrite avisa
            // con errno=22 y devuelve false/short; no se reintenta ni se
            // repite el aviso: el bucle main() sale limpio en la siguiente
            // iteración y el proceso PHP termina sin Notices acumulados.
            $written = @fwrite(STDOUT, $line);
            if ($written === false || $written !== strlen($line)) {
                self::$ipcBroken = true;
                Bridge::markBroken();
                return;
            }
            if (!@fflush(STDOUT)) {
                self::$ipcBroken = true;
                Bridge::markBroken();
            }
        }

        private static function log(string $message): void
        {
            if (defined('STDERR') && is_resource(STDERR)) {
                @fwrite(STDERR, '[PHP/RUNTIME] ' . $message . PHP_EOL);
            }
        }
    }

    /**
     * Synchronous host bridge. Only usable while the runtime is serving a
     * request, because it reads the response stream directly.
     */
    final class Bridge
    {
        public static int $nextId = 1;
        public static ?string $pendingResponse = null;
        private static bool $active = false;
        private static bool $broken = false;

        public static function isActive(): bool
        {
            return self::$active && !self::$broken && defined('STDIN') && is_resource(STDIN) && defined('STDOUT') && is_resource(STDOUT);
        }

        public static function setActive(bool $active): void
        {
            self::$active = $active;
        }

        /**
         * Memo de `player.get` por tick: lo marca `Runtime::write`/hostCall si
         * STDOUT se rompe a mitad de una host call (pipe cerrado por Rust).
         * A partir de ahí hostCall devuelve null sin escribir ni bloquear.
         */
        public static function markBroken(): void
        {
            self::$broken = true;
            self::$active = false;
        }

        public static function isBroken(): bool
        {
            return self::$broken;
        }

        /**
         * Calls a server-side method and returns its result, or null when the
         * host is unavailable or rejects the call.
         */
        public static function hostCall(string $method, array $params = []): mixed
        {
            if (!self::isActive()) {
                return null;
            }

            $id = self::$nextId++;
            $request = json_encode([
                'type' => 'host_call',
                'id' => $id,
                'method' => $method,
                'params' => $params,
            ], JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
            if ($request === false) {
                return null;
            }

            $requestLine = $request . PHP_EOL;
            $written = @fwrite(STDOUT, $requestLine);
            if ($written === false || $written !== strlen($requestLine) || !@fflush(STDOUT)) {
                // Pipe cerrado a mitad de host call (p. ej. shutdown durante
                // disable_plugin): no bloquear 10 s esperando un host_result
                // que nunca llegará; las gestiones superiores tratan el null
                // como "host no disponible" y el shutdown sigue limpio.
                self::markBroken();
                return null;
            }

            $deadline = microtime(true) + 10.0;
            while (microtime(true) < $deadline) {
                $line = @fgets(STDIN);
                if ($line === false) {
                    self::$pendingResponse = null;
                    return null;
                }
                $line = trim($line);
                if ($line === '') {
                    continue;
                }
                $decoded = json_decode($line, true);
                if (!is_array($decoded)) {
                    continue;
                }
                if (($decoded['type'] ?? '') === 'host_result' && (int)($decoded['id'] ?? -1) === $id) {
                    if (($decoded['ok'] ?? false) === true) {
                        return $decoded['result'] ?? null;
                    }
                    @fwrite(STDERR, '[PHP/BRIDGE] Host call ' . $method . ' failed: '
                        . (string)($decoded['error'] ?? 'unknown error') . PHP_EOL);
                    return null;
                }
                // Not our result: keep it for the main loop.
                self::$pendingResponse = $line;
                break;
            }

            return null;
        }
    }
}
