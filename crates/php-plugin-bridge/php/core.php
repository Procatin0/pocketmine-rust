<?php

declare(strict_types=1);

namespace pocketmine {

    use pocketmine\command\CommandMap;
    use pocketmine\command\CommandSender;
    use pocketmine\command\ConsoleCommandSender;
    use pocketmine\permission\DefaultPermissions;
    use pocketmine\permission\PermissionManager;
    use pocketmine\player\Player;
    use pocketmine\plugin\Plugin;
    use pocketmine\plugin\PluginManager;
    use pocketmine\scheduler\Scheduler;
    use pocketmine\utils\Config;
    use pocketmine\utils\TextFormat;
    use pocketmine\world\World;
    use pocketmine\world\WorldManager;

    class Logger
    {
        public function __construct(private string $prefix = 'Server')
        {
        }

        private function emit(string $level, string $message): void
        {
            // PocketMine console layout: [18:26:43.210] [Server thread/INFO]: ...
            $scope = $this->prefix === 'Server' ? 'Server thread' : $this->prefix;
            $line = '[' . self::timestamp() . '] [' . $scope . '/' . $level . ']: ' . $message;
            fwrite(STDERR, self::toConsole($line) . PHP_EOL);
        }

        /**
         * Local wall-clock time with milliseconds. PHP runs with the UTC
         * default timezone, so the Rust host hands over its UTC offset.
         */
        private static function timestamp(): string
        {
            $now = microtime(true);
            $offset = (int)(getenv('PM_CONSOLE_UTC_OFFSET_SECS') ?: 0);
            $millis = (int)(($now - floor($now)) * 1000);
            return gmdate('H:i:s', (int)floor($now) + $offset) . sprintf('.%03d', $millis);
        }

        /**
         * Terminals cannot render the Bedrock colour marker (§), so messages are
         * translated to ANSI escapes (or stripped when stdout is redirected).
         * Like `MainLogger`, every console line starts and ends with a reset:
         * the leading one clears any colour inherited from a previous line and
         * the trailing one stops a message that leaves a colour open (e.g.
         * `§c...` without `§r`) from tinting the next log line.
         */
        private static function toConsole(string $line): string
        {
            $isTty = !function_exists('stream_isatty')
                || (defined('STDERR') && @stream_isatty(STDERR));
            return $isTty ? "\033[0m" . TextFormat::toANSI($line) . "\033[0m" : TextFormat::stripColors($line);
        }

        public function info(string $message): void
        {
            $this->emit('INFO', $message);
        }

        public function notice(string $message): void
        {
            $this->emit('NOTICE', $message);
        }

        public function warning(string $message): void
        {
            $this->emit('WARN', $message);
        }

        public function error(string $message): void
        {
            $this->emit('ERROR', $message);
        }

        public function critical(string $message): void
        {
            $this->emit('CRITICAL', $message);
        }

        public function alert(string $message): void
        {
            $this->emit('ALERT', $message);
        }

        public function emergency(string $message): void
        {
            $this->emit('EMERGENCY', $message);
        }

        public function debug(string $message): void
        {
            $this->emit('DEBUG', $message);
        }

        public function log(int $level, string $message): void
        {
            $this->emit((string)$level, $message);
        }

        public function logException(\Throwable $e, array $trace = []): void
        {
            $this->error($e::class . ': ' . $e->getMessage() . ' in ' . $e->getFile() . ':' . $e->getLine());
            foreach ($e->getTrace() as $frame) {
                $location = ($frame['file'] ?? '?') . ':' . ($frame['line'] ?? '?');
                $function = ($frame['class'] ?? '') . ($frame['type'] ?? '') . ($frame['function'] ?? '');
                fwrite(STDERR, '  at ' . $function . ' (' . $location . ')' . PHP_EOL);
            }
        }
    }

    class Server
    {
        /** PM `Server::BROADCAST_CHANNEL_ADMINISTRATIVE`. */
        public const BROADCAST_CHANNEL_ADMINISTRATIVE = 'pocketmine.broadcast.admin';
        /** PM `Server::BROADCAST_CHANNEL_USERS`. */
        public const BROADCAST_CHANNEL_USERS = 'pocketmine.broadcast.user';

        private static ?Server $instance = null;

        /** @var array<int, array{action:string, data:array}> */
        public array $actions = [];

        private Logger $logger;
        private PermissionManager $permissionManager;
        private ?PluginManager $pluginManager = null;
        private Scheduler $scheduler;
        private CommandMap $commandMap;
        private WorldManager $worldManager;
        private ?Config $config = null;

        /** @var array<string, Player> lowercase name => player */
        private array $players = [];

        /** @var array<string, bool> lowercase name => op */
        private array $ops = [];

        /** @var array<string, bool> lowercase name => whitelisted */
        private array $whitelist = [];

        /** @var array<string, string> lowercase name => BanEntry string */
        private array $nameBans = [];

        /** @var array<string, string> lowercase ip => BanEntry string */
        private array $ipBans = [];

        /** @var array<string, array<int, CommandSender>> channelId => objectId => subscriber */
        private array $broadcastSubscribers = [];

        /** @var array<string, list<string>> alias => targets */
        private array $commandAliases = [];

        private int $tick = 0;
        private int $startTime;
        private bool $shuttingDown = false;

        private function __construct()
        {
            $this->logger = new Logger('Server');
            $this->permissionManager = PermissionManager::getInstance();
            DefaultPermissions::registerCorePermissions();
            $this->scheduler = Scheduler::getInstance();
            $this->commandMap = new CommandMap();
            $this->worldManager = new WorldManager();
            $this->startTime = time();
            $this->loadOps();
            $this->loadWhitelist();
            $this->loadBans();
        }

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
        }

        public function getLogger(): Logger
        {
            return $this->logger;
        }

        public function getPluginManager(): PluginManager
        {
            return $this->pluginManager ??= PluginManager::getInstance($this);
        }

        public function setPluginManager(PluginManager $manager): void
        {
            $this->pluginManager = $manager;
            $manager->setServer($this);
        }

        public function getScheduler(): Scheduler
        {
            return $this->scheduler;
        }

        public function getCommandMap(): CommandMap
        {
            return $this->commandMap;
        }

        public function getWorldManager(): WorldManager
        {
            return $this->worldManager;
        }

        public function getPermissionManager(): PermissionManager
        {
            return $this->permissionManager;
        }

        public function getConfig(): Config
        {
            if ($this->config === null) {
                $path = $this->getDataPath() . 'pocketmine.yml';
                $this->config = new Config($path, Config::YAML, []);
            }
            return $this->config;
        }

        public function getDataPath(): string
        {
            return rtrim(getcwd() ?: '.', '/\\') . DIRECTORY_SEPARATOR;
        }

        public function getPluginPath(): string
        {
            return $this->getDataPath() . 'plugins' . DIRECTORY_SEPARATOR;
        }

        public function getVersion(): string
        {
            return '5.51.2-dev';
        }

        public function getApiVersion(): string
        {
            return '5.0.0';
        }

        public function getMotd(): string
        {
            return (string)$this->getConfig()->get('motd', 'Axolotl-PM Server');
        }

        public function getIp(): string
        {
            return '0.0.0.0';
        }

        public function getPort(): int
        {
            return 19132;
        }

        public function getServerUniqueId(): string
        {
            return 'axolotl-pm';
        }

        public function getMaxPlayers(): int
        {
            return (int)$this->getConfig()->get('max-players', 1000);
        }

        // ----------------------------------------------------------------
        // Fase A1: paridad Server.php (getOnlineMode / gamemode / stubs /
        // UUID / config group / broadcast / aliases / query / bans).
        // ----------------------------------------------------------------

        /**
         * PM `Server::getOnlineMode()`: ¿se requiere autenticación Xbox?
         * Lee `online-mode` / `xbox-auth` de `pocketmine.yml` (default true).
         */
        public function getOnlineMode(): bool
        {
            $config = $this->getConfig();
            $value = $config->get('online-mode', $config->get('xbox-auth', true));
            if (is_string($value)) {
                return !in_array(strtolower($value), ['false', '0', 'no', 'off'], true);
            }
            return (bool)$value;
        }

        /** Alias de `getOnlineMode()` (PM). */
        public function requiresAuthentication(): bool
        {
            return $this->getOnlineMode();
        }

        /**
         * PM `Server::getGamemode(): GameMode` como int (0 survival,
         * 1 creative, 2 adventure, 3 spectator). Lee `gamemode` del config;
         * acepta string ("survival"/...) o int. Sin clase GameMode en bridge.
         */
        public function getGamemode(): int
        {
            $raw = $this->getConfig()->get('gamemode', 'survival');
            if (is_int($raw)) {
                return $raw;
            }
            return match (strtolower(trim((string)$raw))) {
                'creative', '1' => 1,
                'adventure', '2' => 2,
                'spectator', '3' => 3,
                default => 0,
            };
        }

        /** PM `Server::getForceGamemode(): bool` (`force-gamemode`). */
        public function getForceGamemode(): bool
        {
            $value = $this->getConfig()->get('force-gamemode', false);
            if (is_string($value)) {
                return !in_array(strtolower($value), ['false', '0', '', 'no', 'off'], true);
            }
            return (bool)$value;
        }

        /**
         * TODO(A1): sin ClassLoader en bridge (PHP nativo usa el autoloader
         * de `autoload.php`). Null documentado; plugins que exijan el loader
         * deben degradar (p.ej. `if ($loader !== null)`).
         */
        public function getLoader(): ?object
        {
            return null;
        }

        /** TODO(A1): sin UpdateChecker en bridge. Null documentado. */
        public function getUpdater(): ?object
        {
            return null;
        }

        /** TODO(A1): sin CraftingManager en bridge. Null documentado. */
        public function getCraftingManager(): ?object
        {
            return null;
        }

        /** TODO(A1): sin ResourcePackManager en bridge. Null documentado. */
        public function getResourcePackManager(): ?object
        {
            return null;
        }

        /** TODO(A1): sin AsyncPool en bridge (compresión sync). Null documentado. */
        public function getAsyncPool(): ?object
        {
            return null;
        }

        /** TODO(A1): sin MemoryManager/view-distance en bridge. Null documentado. */
        public function getMemoryManager(): ?object
        {
            return null;
        }

        /**
         * TODO(A1): sin AuthKeyProvider (auth_keys.rs es de sesión). Null documentado.
         */
        public function getAuthKeyProvider(): ?object
        {
            return null;
        }

        /**
         * TODO(A1): sin Network/ServerSocket en bridge (RakNet/WebRTC viven
         * en Rust `main.rs`, no expuesto a PHP). Null documentado.
         */
        public function getNetwork(): ?object
        {
            return null;
        }

        /**
         * PM `Server::getPlayerByRawUUID(string $rawUUID): ?Player`.
         * `$rawUUID` es el UUID en bytes crudos o string comparable a
         * `Player::getUniqueId()` (incl. hex de 16 bytes). Consulta primero
         * el registro PHP local, luego cae a `player.get_by_raw_uuid` (host).
         */
        public function getPlayerByRawUUID(string $rawUUID): ?Player
        {
            if ($rawUUID !== '') {
                foreach ($this->players as $player) {
                    $id = (string)$player->getUniqueId();
                    if ($id === $rawUUID) {
                        return $player;
                    }
                    $hex = bin2hex((string)$id);
                    if (strtolower($rawUUID) === strtolower($hex)) {
                        return $player;
                    }
                }
            }
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                $found = \pocketmine\runtime\Bridge::hostCall('player.get_by_raw_uuid', ['raw_uuid' => $rawUUID]);
                if (is_array($found) && isset($found['name'])) {
                    return $this->getPlayerExact((string)$found['name']);
                }
            }
            return null;
        }

        /**
         * PM `Server::getPlayerByUUID(UuidInterface $uuid): ?Player`.
         * Acepta objeto UUID (`getBytes()`/`__toString()`) o string (raw).
         */
        public function getPlayerByUUID(mixed $uuid): ?Player
        {
            $raw = $uuid;
            if (is_object($uuid) && method_exists($uuid, 'getBytes')) {
                $raw = $uuid->getBytes();
            } elseif (is_object($uuid) && method_exists($uuid, '__toString')) {
                $raw = (string)$uuid;
            }
            if (!is_string($raw) && !is_int($raw)) {
                return null;
            }
            $found = $this->getPlayerByRawUUID((string)$raw);
            if ($found !== null) {
                return $found;
            }
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                $host = \pocketmine\runtime\Bridge::hostCall('player.get_by_uuid', ['uuid' => (string)$raw]);
                if (is_array($host) && isset($host['name'])) {
                    return $this->getPlayerExact((string)$host['name']);
                }
            }
            return null;
        }

        /**
         * PM `Server::getConfigGroup(): ServerConfigGroup`. En bridge se
         * devuelve `Config` de `pocketmine.yml` (`getConfig()`), funcional
         * para `getProperty*`/`getConfig*` sobre ese fichero.
         * TODO(A1): grupo real sobre `server.properties` + `pocketmine.yml`.
         */
        public function getConfigGroup(): mixed
        {
            return $this->getConfig();
        }

        /** PM `Server::subscribeToBroadcastChannel(string, CommandSender)`. */
        public function subscribeToBroadcastChannel(string $channelId, CommandSender $subscriber): void
        {
            $this->broadcastSubscribers[$channelId][spl_object_id($subscriber)] = $subscriber;
        }

        /** PM `Server::unsubscribeFromBroadcastChannel(string, CommandSender)`. */
        public function unsubscribeFromBroadcastChannel(string $channelId, CommandSender $subscriber): void
        {
            $id = spl_object_id($subscriber);
            if (!isset($this->broadcastSubscribers[$channelId][$id])) {
                return;
            }
            unset($this->broadcastSubscribers[$channelId][$id]);
            if ($this->broadcastSubscribers[$channelId] === []) {
                unset($this->broadcastSubscribers[$channelId]);
            }
        }

        /** PM `Server::unsubscribeFromAllBroadcastChannels(CommandSender)`. */
        public function unsubscribeFromAllBroadcastChannels(CommandSender $subscriber): void
        {
            foreach (array_keys($this->broadcastSubscribers) as $channelId) {
                $this->unsubscribeFromBroadcastChannel((string)$channelId, $subscriber);
            }
        }

        /** PM `Server::getBroadcastChannelSubscribers(string): array`. */
        public function getBroadcastChannelSubscribers(string $channelId): array
        {
            return array_values($this->broadcastSubscribers[$channelId] ?? []);
        }

        /**
         * @return array<string, list<string>> alias => targets, desde config
         * `aliases:` (+ registrados por `registerServerAliases()`).
         */
        public function getCommandAliases(): array
        {
            // Lee del config si aún no se registró ninguno.
            if ($this->commandAliases === []) {
                $section = $this->getConfig()->get('aliases', []);
                if (is_array($section)) {
                    $resolved = [];
                    foreach ($section as $key => $value) {
                        $targets = is_array($value) ? array_map(strval(...), $value) : [(string)$value];
                        $resolved[(string)$key] = $targets;
                    }
                    $this->commandAliases = $resolved;
                }
            }
            return $this->commandAliases;
        }

        /**
         * PM `SimpleCommandMap::registerServerAliases()`: registra cada alias
         * de `getCommandAliases()` como `FormattedCommandAlias` en el mapa.
         */
        public function registerServerAliases(): void
        {
            foreach ($this->getCommandAliases() as $alias => $targets) {
                $alias = (string)$alias;
                if ($alias === '' || $alias === 'help') {
                    continue;
                }
                if (!class_exists(\pocketmine\command\FormattedCommandAlias::class)) {
                    continue;
                }
                $command = new \pocketmine\command\FormattedCommandAlias($alias, array_values(array_map(strval(...), $targets)));
                // `help` es comando núcleo; no se sobrescribe.
                try {
                    $this->commandMap->register('server', $command);
                } catch (\Throwable) {
                    // Alias duplicado: se ignora (paridad PM).
                }
            }
        }

        /**
         * PM `Server::getQueryInformation(): QueryInfo` como array
         * (`hostname`, `numplayers`, `maxplayers`, `players`, `version`,
         * `server_engine`, `plugins`, `map`, `whitelist`, `hostip`,
         * `hostport`). Intenta `server.get_query_info` (host) para datos
         * vivos; si no hay Bridge, compone desde el registro PHP.
         * TODO(A1): objeto `QueryInfo` tipado (hoy array por compat).
         */
        public function getQueryInformation(): array
        {
            $live = null;
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                $result = \pocketmine\runtime\Bridge::hostCall('server.get_query_info', []);
                if (is_array($result)) {
                    $live = $result;
                }
            }
            $names = $live['players'] ?? array_values(array_map(static fn(Player $p) => $p->getName(), $this->getOnlinePlayers()));
            $count = isset($live['numplayers']) ? (int)$live['numplayers'] : count($names);
            $max = isset($live['maxplayers']) ? (int)$live['maxplayers'] : $this->getMaxPlayers();
            return [
                'hostname' => $live['hostname'] ?? $this->getMotd(),
                'gametype' => 'SMP',
                'game_id' => 'MINECRAFTPE',
                'version' => '1.26.60',
                'server_engine' => 'Axolotl-PM 5.51.2-dev',
                'plugins' => $this->getPluginManager()->getPlugins(),
                'map' => 'world',
                'numplayers' => $count,
                'maxplayers' => $max,
                'whitelist' => $this->hasWhitelist() ? 'on' : 'off',
                'hostip' => $this->getIp(),
                'hostport' => $this->getPort(),
                'players' => array_values($names),
            ];
        }

        public function getStartTime(): int
        {
            return $this->startTime;
        }

        public function getUptime(): int
        {
            return time() - $this->startTime;
        }

        public function getMemoryUsage(bool $advanced = false): int
        {
            return $advanced ? memory_get_usage(true) : memory_get_usage();
        }

        public function getTick(): int
        {
            return $this->tick;
        }

        public function setTick(int $tick): void
        {
            $this->tick = $tick;
        }

        public function getTicksPerSecond(): int
        {
            return 20;
        }

        public function getDefaultWorld(): World
        {
            return $this->worldManager->getDefaultWorld();
        }

        public function getWorld(string $name): ?World
        {
            return $this->worldManager->getWorldByName($name);
        }

        public function registerPlugin(Plugin $plugin): void
        {
            $this->players = $this->players;
        }

        public function getPlugins(): array
        {
            return $this->getPluginManager()->getPlugins();
        }

        public function getPlugin(string $name): ?Plugin
        {
            return $this->getPluginManager()->getPlugin($name);
        }

        public function addAction(string $action, array $data = []): void
        {
            $this->actions[] = ['action' => $action, 'data' => $data];
        }

        /** @return array<int, array{action:string, data:array}> */
        public function takeActions(): array
        {
            $actions = $this->actions;
            $this->actions = [];
            return $actions;
        }

        /**
         * PM `Server::addOnlinePlayer(Player): bool`. Valida duplicados por
         * nombre (case-insensitive) y por UUID (`unique_id`); devuelve false
         * sin añadir si ya está online. Luego registra en mundo.
         */
        public function addOnlinePlayer(Player $player): bool
        {
            $lower = $player->getLowerCaseName();
            if (isset($this->players[$lower])) {
                return false;
            }
            $uuid = (string)$player->getUniqueId();
            foreach ($this->players as $existing) {
                if ((string)$existing->getUniqueId() === $uuid && $uuid !== '') {
                    return false;
                }
            }
            $this->players[$lower] = $player;
            try {
                $world = $player->getWorld();
                $this->worldManager->getWorldByName($world->getName())?->addPlayer($player);
            } catch (\Throwable) {
                // Mundo no cargado: registro PHP basta (paridad tolerante).
            }
            return true;
        }

        /** PM `Server::removeOnlinePlayer(Player): void`. */
        public function removeOnlinePlayer(Player $player): void
        {
            $lower = $player->getLowerCaseName();
            if (!isset($this->players[$lower])) {
                return;
            }
            unset($this->players[$lower]);
            try {
                $world = $player->getWorld();
                $this->worldManager->getWorldByName($world->getName())?->removePlayer($player);
            } catch (\Throwable) {
                // Mundo no cargado: solo se limpia el registro PHP.
            }
        }

        /**
         * @deprecated Usar `addOnlinePlayer()` (PM). Wrapper compatible con
         * `ipc.php` (Runtime aún llama a `addPlayer`/`removePlayer`).
         */
        public function addPlayer(Player $player): void
        {
            $this->addOnlinePlayer($player);
        }

        /** @deprecated Usar `removeOnlinePlayer()` (PM). */
        public function removePlayer(Player $player): void
        {
            $this->removeOnlinePlayer($player);
        }

        public function getPlayerExact(string $name): ?Player
        {
            return $this->players[strtolower($name)] ?? null;
        }

        public function getPlayer(string $name): ?Player
        {
            $exact = $this->getPlayerExact($name);
            if ($exact !== null) {
                return $exact;
            }
            foreach ($this->players as $player) {
                if (stripos($player->getName(), $name) === 0) {
                    return $player;
                }
            }
            return null;
        }

        /**
         * PM `Server::getPlayerByPrefix()`: exact name wins, otherwise the
         * first case-insensitive prefix match. Alias kept for plugins that
         * use the vanilla name (`VanillaCommand::fetchPermittedPlayerTarget`).
         */
        public function getPlayerByPrefix(string $name): ?Player
        {
            return $this->getPlayer($name);
        }

        /** @return array<string, Player> */
        public function getOnlinePlayers(): array
        {
            return array_filter($this->players, static fn(Player $player) => $player->isOnline());
        }

        public function getPlayerCount(): int
        {
            return count($this->getOnlinePlayers());
        }

        public function getOnlinePlayerCount(): int
        {
            return $this->getPlayerCount();
        }

        public function isPlayerOnline(string $name): bool
        {
            $player = $this->getPlayerExact($name);
            return $player !== null && $player->isOnline();
        }

        /** Jugador offline (`player.has_data` / `player.load`): nunca null en PM. */
        public function getOfflinePlayer(string $name): \pocketmine\player\OfflinePlayer
        {
            $data = null;
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                $loaded = \pocketmine\runtime\Bridge::hostCall('player.load', ['name' => $name]);
                if (is_array($loaded) && ($loaded['has_data'] ?? $loaded['exists'] ?? false)) {
                    $data = $loaded;
                }
            }
            return new \pocketmine\player\OfflinePlayer($name, $data);
        }

        /** Alias case-insensitive de `getOfflinePlayer`. */
        public function getOfflinePlayerByName(string $name): \pocketmine\player\OfflinePlayer
        {
            return $this->getOfflinePlayer($name);
        }

        /** ¿Tiene datos guardados (`player.has_data`)? */
        public function hasOfflineData(string $name): bool
        {
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                return false;
            }
            $result = \pocketmine\runtime\Bridge::hostCall('player.has_data', ['name' => $name]);
            return is_array($result) && ($result['has_data'] ?? $result['exists'] ?? false) === true;
        }

        /** `Server::saveAll()`: mundo + jugadores vía `server.save_all`. */
        public function saveAll(): void
        {
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('server.save_all', []);
            }
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                foreach ($this->getOnlinePlayers() as $player) {
                    if (method_exists($player, 'save')) {
                        $player->save();
                    }
                }
            }
        }

        /**
         * PM `Server::broadcastMessage(Translatable|string, ?array $recipients): int`.
         * Sin `$recipients` envía a suscriptores de `BROADCAST_CHANNEL_USERS`
         * (fallback a online si no hay suscriptores, paridad bridge); con
         * `$recipients` entrega solo a ellos. Devuelve nº de destinatarios.
         */
        public function broadcastMessage(mixed $message, ?array $recipients = null): int
        {
            $text = (string)$message;
            if ($recipients !== null) {
                $count = 0;
                foreach ($recipients as $recipient) {
                    if ($recipient instanceof CommandSender) {
                        $recipient->sendMessage($text);
                        $count++;
                    }
                }
                return $count;
            }
            $subscribers = $this->getBroadcastChannelSubscribers(self::BROADCAST_CHANNEL_USERS);
            if ($subscribers === []) {
                $this->addAction('broadcast_message', ['message' => $text]);
                return count($this->getOnlinePlayers());
            }
            $count = 0;
            foreach ($subscribers as $recipient) {
                $recipient->sendMessage($text);
                $count++;
            }
            return $count;
        }

        /** PM `Server::broadcastTip(string, ?array $recipients): int` (solo Players). */
        public function broadcastTip(string $message, ?array $recipients = null): int
        {
            if ($recipients !== null) {
                $count = 0;
                foreach ($recipients as $recipient) {
                    if ($recipient instanceof Player) {
                        $recipient->sendTip($message);
                        $count++;
                    }
                }
                return $count;
            }
            $players = $this->getBroadcastChannelSubscribers(self::BROADCAST_CHANNEL_USERS);
            $players = array_filter($players, static fn($r) => $r instanceof Player);
            if ($players === []) {
                $this->addAction('broadcast_tip', ['message' => $message]);
                return count($this->getOnlinePlayers());
            }
            $count = 0;
            foreach ($players as $recipient) {
                $recipient->sendTip($message);
                $count++;
            }
            return $count;
        }

        /** PM `Server::broadcastPopup(string, ?array $recipients): int` (solo Players). */
        public function broadcastPopup(string $message, ?array $recipients = null): int
        {
            if ($recipients !== null) {
                $count = 0;
                foreach ($recipients as $recipient) {
                    if ($recipient instanceof Player) {
                        $recipient->sendPopup($message);
                        $count++;
                    }
                }
                return $count;
            }
            $players = $this->getBroadcastChannelSubscribers(self::BROADCAST_CHANNEL_USERS);
            $players = array_filter($players, static fn($r) => $r instanceof Player);
            if ($players === []) {
                $this->addAction('broadcast_popup', ['message' => $message]);
                return count($this->getOnlinePlayers());
            }
            $count = 0;
            foreach ($players as $recipient) {
                $recipient->sendPopup($message);
                $count++;
            }
            return $count;
        }

        /** PM `Server::broadcastTitle(string, string, int, int, int, ?array): int`. */
        public function broadcastTitle(string $title, string $subtitle = '', int $fadeIn = 0, int $stay = 0, int $fadeOut = 0, ?array $recipients = null): int
        {
            if ($recipients !== null) {
                $count = 0;
                foreach ($recipients as $recipient) {
                    if ($recipient instanceof Player) {
                        $recipient->sendTitle($title, $subtitle, $fadeIn, $stay, $fadeOut);
                        $count++;
                    }
                }
                return $count;
            }
            $players = $this->getBroadcastChannelSubscribers(self::BROADCAST_CHANNEL_USERS);
            $players = array_filter($players, static fn($r) => $r instanceof Player);
            if ($players === []) {
                $this->addAction('broadcast_title', [
                    'title' => $title,
                    'subtitle' => $subtitle,
                    'fade_in' => $fadeIn,
                    'stay' => $stay,
                    'fade_out' => $fadeOut,
                ]);
                return count($this->getOnlinePlayers());
            }
            $count = 0;
            foreach ($players as $recipient) {
                $recipient->sendTitle($title, $subtitle, $fadeIn, $stay, $fadeOut);
                $count++;
            }
            return $count;
        }

        public function dispatchCommand(CommandSender $sender, string $commandLine): bool
        {
            if ($this->commandMap->dispatch($sender, $commandLine)) {
                return true;
            }
            $this->addAction('dispatch_command', [
                'command' => $commandLine,
                'sender' => $sender->getName(),
            ]);
            return true;
        }

        public function getConsoleSender(): ConsoleCommandSender
        {
            return ConsoleCommandSender::getInstance();
        }

        public function isOp(string $name): bool
        {
            return $this->ops[strtolower($name)] ?? false;
        }

        public function addOp(string $name): void
        {
            $this->ops[strtolower($name)] = true;
            $this->saveOps();
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('server.set_op', ['name' => $name, 'op' => true]);
            }
        }

        public function removeOp(string $name): void
        {
            unset($this->ops[strtolower($name)]);
            $this->saveOps();
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('server.set_op', ['name' => $name, 'op' => false]);
            }
        }

        private function loadOps(): void
        {
            $path = $this->getDataPath() . 'ops.txt';
            if (!is_file($path)) {
                return;
            }
            $contents = @file_get_contents($path);
            if ($contents === false) {
                return;
            }
            foreach (preg_split('/\r\n|\r|\n/', $contents) ?: [] as $line) {
                $line = trim($line);
                if ($line === '' || $line[0] === '#') {
                    continue;
                }
                $this->ops[strtolower($line)] = true;
            }
        }

        private function saveOps(): void
        {
            $names = array_keys($this->ops);
            @file_put_contents($this->getDataPath() . 'ops.txt', implode(PHP_EOL, $names) . PHP_EOL);
        }

        private function loadWhitelist(): void
        {
            $path = $this->getDataPath() . 'white-list.txt';
            if (!is_file($path)) {
                return;
            }
            $contents = @file_get_contents($path);
            if ($contents === false) {
                return;
            }
            foreach (preg_split('/\r\n|\r|\n/', $contents) ?: [] as $line) {
                $line = trim($line);
                if ($line === '' || $line[0] === '#') {
                    continue;
                }
                $this->whitelist[strtolower($line)] = true;
            }
        }

        private function saveWhitelist(): void
        {
            $names = array_keys($this->whitelist);
            @file_put_contents($this->getDataPath() . 'white-list.txt', implode(PHP_EOL, $names) . PHP_EOL);
        }

        private function loadBans(): void
        {
            $this->nameBans = $this->readBanFile($this->getDataPath() . 'banned-players.txt');
            $this->ipBans = $this->readBanFile($this->getDataPath() . 'banned-ips.txt');
        }

        /** @return array<string, string> lowercase key => BanEntry line */
        private function readBanFile(string $path): array
        {
            if (!is_file($path)) {
                return [];
            }
            $contents = @file_get_contents($path);
            if ($contents === false) {
                return [];
            }
            $entries = [];
            foreach (preg_split('/\r\n|\r|\n/', $contents) ?: [] as $line) {
                $line = trim($line);
                if ($line === '' || $line[0] === '#') {
                    continue;
                }
                $name = strtolower(trim(explode('|', $line, 2)[0] ?? ''));
                if ($name !== '') {
                    $entries[$name] = $line;
                }
            }
            return $entries;
        }

        /**
         * Compat: nombre antiguo de `getWhitelisted()`. Devuelve los nombres
         * (lowercase) como lista para código que esperaba array.
         */
        public function getWhitelist(): array
        {
            return array_keys($this->whitelist);
        }

        /**
         * PM `Server::hasWhitelist(): bool` (`whitelist` del config).
         * Sustituye al stub `false` anterior.
         */
        public function hasWhitelist(): bool
        {
            $value = $this->getConfig()->get('whitelist', false);
            if (is_string($value)) {
                return !in_array(strtolower($value), ['false', '0', '', 'no', 'off'], true);
            }
            return (bool)$value;
        }

        /** PM `Server::addWhitelist(string): void` (`white-list.txt`). */
        public function addWhitelist(string $name): void
        {
            $this->whitelist[strtolower($name)] = true;
            $this->saveWhitelist();
        }

        /** PM `Server::removeWhitelist(string): void` (`white-list.txt`). */
        public function removeWhitelist(string $name): void
        {
            unset($this->whitelist[strtolower($name)]);
            $this->saveWhitelist();
        }

        /**
         * PM `Server::isWhitelisted(string): bool`:
         * `!hasWhitelist() || isOp() || whitelist[name]`.
         */
        public function isWhitelisted(string $name): bool
        {
            return !$this->hasWhitelist() || $this->isOp($name) || isset($this->whitelist[strtolower($name)]);
        }

        /**
         * PM `Server::getWhitelisted(): Config` (lista en `white-list.txt`).
         * Devuelve `Config` con los nombres (`getAll()` => valores).
         * TODO(A1): la persistencia de `Config::save()` sobre este fichero
         * usa formato `Config`, no líneas planas (diverge de Rust
         * `AccessList`; las escrituras deben hacerse vía
         * `addWhitelist`/`removeWhitelist`).
         */
        public function getWhitelisted(): Config
        {
            return new Config($this->getDataPath() . 'white-list.txt', Config::YAML, $this->whitelist);
        }

        /**
         * PM `Server::getOps(): Config` (lista en `ops.txt`).
         * Misma advertencia de formato que `getWhitelisted()`.
         */
        public function getOps(): Config
        {
            return new Config($this->getDataPath() . 'ops.txt', Config::YAML, $this->ops);
        }

        /**
         * PM `Server::getNameBans(): BanList` como `Config`
         * (`banned-players.txt`, formato `BanEntry`). `getAll()` => entradas.
         * TODO(A1): objeto `BanList` tipado; hoy `Config` por compat.
         */
        public function getNameBans(): Config
        {
            return new Config($this->getDataPath() . 'banned-players.txt', Config::YAML, $this->nameBans);
        }

        /** PM `Server::getIPBans(): BanList` como `Config` (`banned-ips.txt`). */
        public function getIPBans(): Config
        {
            return new Config($this->getDataPath() . 'banned-ips.txt', Config::YAML, $this->ipBans);
        }

        /**
         * PM `Server::prepareBatch(string, Compressor, ?bool, ?TimingsHandler)`.
         * Stub sync documentado: sin AsyncPool/Compressor en bridge, devuelve
         * el buffer sin comprimir (payload válido para batch sin compresión).
         * TODO(A1): usar `AsyncPool` + compresor real cuando existan.
         */
        public function prepareBatch(string $buffer, mixed $compressor = null, ?bool $sync = null, mixed $timings = null): string
        {
            // `chr(0)` (tipo NONE) + buffer es el batch "sin compresión" de PM;
            // aquí se devuelve el buffer tal cual para no romper lectores que
            // no esperan cabecera (paridad funcional mínima).
            return $buffer;
        }

        /**
         * PM `Server::shutdown(): void`: kick-all (`Server closed`) + `saveAll()`.
         * NOTA: no se emite `shutdown_server` (variante `PluginAction`
         * inexistente: `serde(tag=action)` estricto rompería toda la IPC).
         * La parada del bucle Rust requiere `main.rs` (fuera de ownership)
         * o una variante `ShutdownServer` del equipo Bridge.
         */
        public function shutdown(): void
        {
            if ($this->shuttingDown) {
                return;
            }
            $this->shuttingDown = true;
            foreach ($this->getOnlinePlayers() as $player) {
                $player->kick('Server closed');
            }
            $this->saveAll();
        }

        /** PM `Server::forceShutdown(): void`: kick-all inmediato + `saveAll()`. */
        public function forceShutdown(): void
        {
            if ($this->shuttingDown) {
                return;
            }
            $this->shuttingDown = true;
            foreach ($this->getOnlinePlayers() as $player) {
                $player->kick('Server stopped');
            }
            $this->saveAll();
        }

        public function getLanguage(): string
        {
            return 'eng';
        }

        public function getResourcePath(): string
        {
            return $this->getDataPath() . 'resources' . DIRECTORY_SEPARATOR;
        }

        public function getUpdatePath(): string
        {
            return $this->getDataPath();
        }

        public function getName(): string
        {
            return 'Axolotl-PM';
        }

        public function __toString(): string
        {
            return 'Server(' . $this->getMotd() . ')';
        }
    }
}
