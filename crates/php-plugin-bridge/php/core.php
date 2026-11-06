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
            $line = '[' . self::timestamp() . '] [' . $scope . '/' . $level . ']: '
                . self::toConsole($message);
            fwrite(STDERR, $line . PHP_EOL);
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
         */
        private static function toConsole(string $message): string
        {
            $isTty = !function_exists('stream_isatty')
                || (defined('STDERR') && @stream_isatty(STDERR));
            return $isTty ? TextFormat::toANSI($message) : TextFormat::stripColors($message);
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

        private int $tick = 0;
        private int $startTime;

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
            return '1.26.60-rust';
        }

        public function getApiVersion(): string
        {
            return '5.0.0';
        }

        public function getMotd(): string
        {
            return (string)$this->getConfig()->get('motd', 'PocketMine-Rust Server');
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
            return 'pocketmine-rust';
        }

        public function getMaxPlayers(): int
        {
            return (int)$this->getConfig()->get('max-players', 1000);
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

        public function addPlayer(Player $player): void
        {
            $this->players[$player->getLowerCaseName()] = $player;
            $this->worldManager->getWorldByName($player->getWorld()->getName())?->addPlayer($player);
        }

        public function removePlayer(Player $player): void
        {
            unset($this->players[$player->getLowerCaseName()]);
            $this->worldManager->getWorldByName($player->getWorld()->getName())?->removePlayer($player);
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

        public function broadcastMessage(string $message): void
        {
            $this->addAction('broadcast_message', ['message' => $message]);
        }

        public function broadcastTip(string $message): void
        {
            $this->addAction('broadcast_tip', ['message' => $message]);
        }

        public function broadcastPopup(string $message): void
        {
            $this->addAction('broadcast_popup', ['message' => $message]);
        }

        public function broadcastTitle(string $title, string $subtitle = '', int $fadeIn = 0, int $stay = 0, int $fadeOut = 0): void
        {
            $this->addAction('broadcast_title', [
                'title' => $title,
                'subtitle' => $subtitle,
                'fade_in' => $fadeIn,
                'stay' => $stay,
                'fade_out' => $fadeOut,
            ]);
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
        }

        public function removeOp(string $name): void
        {
            unset($this->ops[strtolower($name)]);
            $this->saveOps();
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

        public function getWhitelist(): array
        {
            return [];
        }

        public function hasWhitelist(): bool
        {
            return false;
        }

        public function shutdown(): void
        {
            $this->addAction('shutdown_server', []);
        }

        public function forceShutdown(): void
        {
            $this->shutdown();
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
            return 'PocketMine-Rust';
        }

        public function __toString(): string
        {
            return 'Server(' . $this->getMotd() . ')';
        }
    }
}
