<?php

declare(strict_types=1);

namespace pocketmine\command {

    use pocketmine\plugin\Plugin;
    use pocketmine\permission\Permission;
    use pocketmine\permission\PermissionManager;

    /**
     * Minimal `Translatable` parity (PM `pocketmine\lang\Translatable`).
     * Wraps a translation key + parameters; `__toString` renders the key
     * for bridges without a language backend.
     */
    class Translatable
    {
        /** @var array<string, mixed>|list<mixed> */
        private array $params = [];

        public function __construct(
            private string $key,
            array $params = []
        ) {
            $this->params = $params;
        }

        public function getKey(): string
        {
            return $this->key;
        }

        /** @return array<string, mixed>|list<mixed> */
        public function getParams(): array
        {
            return $this->params;
        }

        public function __toString(): string
        {
            return $this->key;
        }
    }

    interface CommandSender
    {
        public function sendMessage(string|\pocketmine\lang\Translatable $message): void;

        public function getServer(): mixed;

        public function getName(): string;

        public function hasPermission($permission): bool;

        public function isPlayer(): bool;

        public function isOp(): bool;
    }

    class ConsoleCommandSender implements CommandSender
    {
        private static ?ConsoleCommandSender $instance = null;

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
        }

        public function sendMessage(string|\pocketmine\lang\Translatable $message): void
        {
            $text = (string)$message;
            try {
                \pocketmine\Server::getInstance()->addAction('console_message', ['message' => $text]);
            } catch (\Throwable) {
                fwrite(STDERR, '[CONSOLE] ' . $text . PHP_EOL);
            }
        }

        public function getServer(): mixed
        {
            return \pocketmine\Server::getInstance();
        }

        public function getName(): string
        {
            return 'CONSOLE';
        }

        public function hasPermission($permission): bool
        {
            return true;
        }

        public function isPlayer(): bool
        {
            return false;
        }

        public function isOp(): bool
        {
            return true;
        }

        public function getLanguageCode(): string
        {
            return 'eng';
        }
    }

    interface CommandExecutor
    {
        /**
         * @param array<int, string> $args
         */
        public function onCommand(CommandSender $sender, Command $command, string $label, array $args): bool;
    }

    abstract class Command
    {
        private string $label;
        private string $nextLabel;
        private mixed $executor = null;
        /** @var string[] */
        private array $permissions = [];
        /** @var string[] */
        private array $aliases = [];
        /** @var string[] */
        private array $activeAliases = [];
        private ?string $permissionMessage = null;
        private ?CommandMap $commandMap = null;

        /**
         * @param string[] $aliases
         * @param string|\pocketmine\lang\Translatable|string|null $description
         * @param string|\pocketmine\lang\Translatable|string|null $usageMessage
         */
        public function __construct(
            private string $name,
            private mixed $description = null,
            private mixed $usageMessage = null,
            array $aliases = [],
            private ?string $permission = null
        ) {
            if ($name === '') {
                throw new \InvalidArgumentException('Command name cannot be empty');
            }
            $this->label = $name;
            $this->nextLabel = $name;
            $this->setAliases($aliases);
            if ($permission !== null && $permission !== '') {
                $this->setPermission($permission);
            }
        }

        public function getName(): string
        {
            return $this->name;
        }

        /** @return string|\pocketmine\lang\Translatable|string */
        public function getDescription(): mixed
        {
            return $this->description ?? '';
        }

        public function setDescription(mixed $description): void
        {
            $this->description = $description;
        }

        /** @return string|\pocketmine\lang\Translatable|string */
        public function getUsage(): mixed
        {
            return $this->usageMessage ?? '/' . $this->name;
        }

        public function setUsage(mixed $usage): void
        {
            $this->usageMessage = $usage;
        }

        /** @return string[] */
        public function getAliases(): array
        {
            return $this->activeAliases;
        }

        /**
         * @param string[] $aliases
         */
        public function setAliases(array $aliases): void
        {
            $aliases = array_values($aliases);
            $this->aliases = $aliases;
            if (!$this->isRegistered()) {
                $this->activeAliases = $aliases;
            }
        }

        /**
         * @return string[]
         */
        public function getPermissions(): array
        {
            return $this->permissions;
        }

        /**
         * PM `Command::setPermissions(array)`: every node must exist.
         *
         * @param string[] $permissions
         */
        public function setPermissions(array $permissions): void
        {
            foreach ($permissions as $perm) {
                if (PermissionManager::getInstance()->getPermission((string)$perm) === null) {
                    throw new \InvalidArgumentException('Cannot use non-existing permission "' . $perm . '"');
                }
            }
            $this->permissions = array_values(array_map(strval(...), $permissions));
        }

        public function setPermission(?string $permission): void
        {
            $this->setPermissions($permission === null || $permission === '' ? [] : explode(';', $permission));
        }

        /** Back-compat: first permission or null. */
        public function getPermission(): ?string
        {
            return $this->permissions[0] ?? null;
        }

        public function getPermissionMessage(): string
        {
            return $this->permissionMessage
                ?? (\pocketmine\utils\TextFormat::RED . 'You do not have permission to use this command.');
        }

        public function setPermissionMessage(?string $permissionMessage): void
        {
            $this->permissionMessage = $permissionMessage;
        }

        public function getLabel(): string
        {
            return $this->label;
        }

        public function setLabel(string $label): bool
        {
            $this->nextLabel = $label;
            if (!$this->isRegistered()) {
                $this->label = $label;
                return true;
            }
            return false;
        }

        public function setExecutor(mixed $executor): void
        {
            $this->executor = $executor;
        }

        public function getExecutor(): mixed
        {
            return $this->executor;
        }

        public function register(CommandMap $commandMap): bool
        {
            if ($this->commandMap === null || $this->commandMap === $commandMap) {
                $this->commandMap = $commandMap;
                return true;
            }
            return false;
        }

        public function unregister(CommandMap $commandMap): bool
        {
            if ($this->commandMap === null || $this->commandMap === $commandMap) {
                $this->commandMap = null;
                $this->activeAliases = $this->aliases;
                $this->label = $this->nextLabel;
                return true;
            }
            return false;
        }

        public function isRegistered(): bool
        {
            return $this->commandMap !== null;
        }

        public function getOwningPlugin(): ?Plugin
        {
            $server = \pocketmine\Server::getInstance();
            foreach ($server->getPlugins() as $plugin) {
                if ($plugin->getCommand($this->name) === $this) {
                    return $plugin;
                }
            }
            return null;
        }

        public function testPermission(CommandSender $target, ?string $permission = null): bool
        {
            if ($this->testPermissionSilent($target, $permission)) {
                return true;
            }
            $target->sendMessage($this->getPermissionMessage());
            return false;
        }

        public function testPermissionSilent(CommandSender $target, ?string $permission = null): bool
        {
            $list = $permission !== null ? [$permission] : $this->permissions;
            foreach ($list as $p) {
                if ($target->hasPermission($p)) {
                    return true;
                }
            }
            return false;
        }

        /**
         * PM `Command::broadcastCommandMessage`: the sender gets the plain
         * line, administrators get the grey italic `[source: message]`.
         * In this bridge the admin fan-out goes through `Server::addAction`.
         */
        public static function broadcastCommandMessage(CommandSender $source, string|\pocketmine\lang\Translatable $message, bool $sendToSource = true): void
        {
            $text = (string)$message;
            if ($sendToSource) {
                $source->sendMessage($text);
            }
            try {
                \pocketmine\Server::getInstance()->addAction('broadcast_admin', [
                    'source' => $source->getName(),
                    'message' => $text,
                ]);
            } catch (\Throwable) {
                // No server: nothing to broadcast to.
            }
        }

        public function __toString(): string
        {
            return $this->name;
        }

        /**
         * @param string $sender
         * @param array<int, string> $args
         */
        abstract public function execute(CommandSender $sender, string $commandLabel, array $args): mixed;
    }

    class SimpleCommand extends Command
    {
        private mixed $callable = null;

        /**
         * @param string[] $aliases
         */
        public function __construct(
            string $name,
            ?string $description = null,
            ?string $usageMessage = null,
            array $aliases = [],
            ?string $permission = null,
            ?callable $executor = null
        ) {
            parent::__construct($name, $description, $usageMessage, $aliases, $permission);
            $this->callable = $executor;
        }

        public function setExecutorCallable(?callable $executor): void
        {
            $this->callable = $executor;
        }

        public function getExecutorCallable(): mixed
        {
            return $this->callable;
        }

        public function execute(CommandSender $sender, string $commandLabel, array $args): mixed
        {
            if (!$this->testPermission($sender)) {
                return false;
            }

            if (is_callable($this->callable)) {
                return ($this->callable)($sender, $commandLabel, $args);
            }

            $executor = $this->getExecutor();
            if (is_callable($executor)) {
                return $executor($sender, $commandLabel, $args);
            }
            if (is_object($executor) && method_exists($executor, 'onCommand')) {
                return $executor->onCommand($sender, $this, $commandLabel, $args);
            }

            $usage = $this->getUsage();
            $sender->sendMessage(\pocketmine\utils\TextFormat::RED . 'Usage: ' . $usage);
            return false;
        }
    }

    /**
     * PM `ClosureCommand` parity: constructed from a closure + a permission
     * list. Deprecated upstream in favour of `SimpleCommand` + executor,
     * kept so plugin code using `new ClosureCommand(...)` keeps working.
     */
    final class ClosureCommand extends Command
    {
        /** @var \Closure */
        private \Closure $execute;

        /**
         * @param string[] $permissions
         */
        public function __construct(
            string $name,
            \Closure $execute,
            array $permissions,
            mixed $description = null,
            mixed $usageMessage = null,
            array $aliases = []
        ) {
            $this->execute = $execute;
            parent::__construct($name, $description, $usageMessage, $aliases, null);
            $this->setPermissions($permissions);
        }

        public function execute(CommandSender $sender, string $commandLabel, array $args): mixed
        {
            return ($this->execute)($sender, $this, $commandLabel, $args);
        }
    }

    /**
     * PM `PluginCommand` parity: a command owned by a plugin. The command
     * name is looked up from the plugin's manifest (`plugin.yml` commands).
     */
    class PluginCommand extends Command
    {
        public function __construct(
            private Plugin $plugin,
            string $name
        ) {
            parent::__construct($name);
        }

        public function getPlugin(): Plugin
        {
            return $this->plugin;
        }

        public function execute(CommandSender $sender, string $commandLabel, array $args): mixed
        {
            if (!$this->testPermission($sender)) {
                return false;
            }
            try {
                $result = $this->plugin->onCommand($sender, $this, $commandLabel, $args);
                return $result;
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/COMMAND] ' . $this->name . ' failed: ' . $e->getMessage() . PHP_EOL);
                return false;
            }
        }
    }

    class CommandMap
    {
        /** @var array<string, Command> */
        private array $commands = [];

        /** @var array<string, string> alias => canonical name */
        private array $aliases = [];

        /**
         * PM `SimpleCommandMap::register`: always stores `prefix:label`,
         * stores the bare `label` only if it is free (vanilla) or the label
         * is an alias slot, otherwise the command's label becomes
         * `prefix:label`.
         */
        public function register(string $fallbackPrefix, Command $command, ?string $label = null): bool
        {
            $fallbackPrefix = strtolower(trim($fallbackPrefix));
            $label = trim($label ?? $command->getLabel());

            $this->commands[$fallbackPrefix . ':' . $label] = $command;
            $registered = $this->registerLabel($command, false, $fallbackPrefix, $label);
            foreach ($command->getAliases() as $alias) {
                if (!$this->registerLabel($command, true, $fallbackPrefix, $alias)) {
                    // Drop aliases that could not be registered (PM behaviour).
                }
            }
            if (!$registered) {
                $command->setLabel($fallbackPrefix . ':' . $label);
            }
            $command->register($this);

            $server = \pocketmine\Server::getInstance();
            if (method_exists($server, 'syncCommands')) {
                try {
                    $server->syncCommands();
                } catch (\Throwable) {
                    // Optional hook.
                }
            }
            return $registered;
        }

        /**
         * @param Command[] $commands
         */
        public function registerAll(string $fallbackPrefix, array $commands): void
        {
            foreach ($commands as $command) {
                if ($command instanceof Command) {
                    $this->register($fallbackPrefix, $command);
                }
            }
        }

        private function registerLabel(Command $command, bool $isAlias, string $fallbackPrefix, string $alias): bool
        {
            $key = strtolower($alias);
            if ($key === '') {
                return false;
            }
            // Vanilla commands always win the bare label.
            if (!$isAlias && isset($this->commands[$key])) {
                return false;
            }
            if (isset($this->commands[$key]) && $this->commands[$key]->getLabel() === $alias) {
                return false;
            }
            if (!$isAlias) {
                $command->setLabel($alias);
            }
            $this->commands[$key] = $command;
            $this->aliases[$key] = strtolower($command->getName());
            return true;
        }

        public function registerAlias(string $name, string $alias): bool
        {
            $name = strtolower($name);
            if (!isset($this->commands[$name])) {
                return false;
            }
            $this->aliases[strtolower($alias)] = $name;
            return true;
        }

        public function unregister(Command $command): bool
        {
            $removed = false;
            foreach ($this->commands as $label => $registered) {
                if ($registered === $command) {
                    unset($this->commands[$label]);
                    $removed = true;
                }
            }
            $command->unregister($this);
            return $removed;
        }

        public function getCommand(string $name): ?Command
        {
            $name = strtolower($name);
            if (isset($this->commands[$name])) {
                return $this->commands[$name];
            }
            $resolved = $this->aliases[$name] ?? null;
            return $resolved === null ? null : ($this->commands[$resolved] ?? null);
        }

        /** @return array<string, Command> */
        public function getCommands(): array
        {
            return $this->commands;
        }

        public function clearCommands(): void
        {
            $this->commands = [];
            $this->aliases = [];
        }

        public function dispatch(CommandSender $sender, string $commandLine): bool
        {
            $commandLine = trim($commandLine);
            if ($commandLine === '') {
                return false;
            }
            if ($commandLine[0] === '/') {
                $commandLine = substr($commandLine, 1);
            }

            $order = preg_split('/\s+/', $commandLine) ?: [];
            if ($order === []) {
                return false;
            }

            $label = strtolower(array_shift($order));
            $parts = self::mergeSelectorTokens(array_values($order));
            $command = $this->getCommand($label);
            if ($command === null) {
                return false;
            }

            try {
                $command->execute($sender, $label, array_values($parts));
            } catch (\Throwable $e) {
                $sender->sendMessage(\pocketmine\utils\TextFormat::RED . 'An internal error occurred while executing this command.');
                fwrite(STDERR, '[PHP/COMMAND] ' . $label . ' failed: ' . $e->getMessage() . PHP_EOL);
            }
            return true;
        }

        /**
         * Merges whitespace-split tokens that belong to a single `@`
         * selector `[...]` (see `command.rs::merge_selector_tokens`).
         *
         * @param string[] $parts
         * @return string[]
         */
        private static function mergeSelectorTokens(array $parts): array
        {
            $merged = [];
            $buffer = null;
            $depth = 0;
            foreach ($parts as $token) {
                $token = (string)$token;
                $delta = substr_count($token, '[') - substr_count($token, ']');
                if ($buffer === null) {
                    $depth = $delta;
                    if ($depth <= 0) {
                        $merged[] = $token;
                    } else {
                        $buffer = $token;
                    }
                } else {
                    $buffer .= ' ' . $token;
                    $depth += $delta;
                    if ($depth <= 0) {
                        $merged[] = $buffer;
                        $buffer = null;
                    }
                }
            }
            if ($buffer !== null) {
                $merged[] = $buffer;
            }
            return $merged;
        }
    }

    /**
     * PM `FormattedCommandAlias` parity: an alias that expands to a fixed
     * command line (server aliases from `server.properties`).
     */
    final class FormattedCommandAlias extends Command
    {
        /**
         * @param string[] $targets
         */
        public function __construct(
            string $name,
            private array $targets
        ) {
            parent::__construct($name);
        }

        /** @return string[] */
        public function getTargets(): array
        {
            return $this->targets;
        }

        public function execute(CommandSender $sender, string $commandLabel, array $args): mixed
        {
            $map = $sender->getServer()->getCommandMap();
            foreach ($this->targets as $target) {
                $map->dispatch($sender, $target);
            }
            return true;
        }
    }

    /** PM `InvalidCommandSyntaxException` parity for `VanillaCommand`. */
    class InvalidCommandSyntaxException extends \InvalidArgumentException
    {
    }

    /** PM `PermissionCommandException` parity for `VanillaCommand`. */
    class PermissionCommandException extends \Exception
    {
    }

    /**
     * PM `VanillaCommand` parity: helpers used by vanilla core commands and
     * by plugins (`getInteger`, `getBoundedInt`, `fetchPermittedPlayerTarget`,
     * ...). Selector support mirrors `server-core/src/command.rs`:
     * `@s` (sender), `@p`/`@n` nearest, `@a`/`@c` all players, `@e` all
     * entities (online players only — the Bedrock runtime has no non-player
     * entity list, so `type` other than `player` matches nothing), `@r`
     * random, with `distance=`, `name=`, `type=`, `x/y/z`, `limit=` and
     * `sort=` arguments inside `[...]`.
     */
    abstract class VanillaCommand extends Command
    {
        protected function getInteger(CommandSender $sender, string $value, int $min = PHP_INT_MIN, int $max = PHP_INT_MAX): int
        {
            $int = (int)$value;
            if ($int < $min) {
                return $min;
            }
            if ($int > $max) {
                return $max;
            }
            return $int;
        }

        /**
         * PM `getBoundedInt`: non-numeric input is a syntax error, values
         * outside `[min, max]` are errors (Rust `bounded_int` returns the
         * too-big/too-small keys; here they are exception messages).
         */
        protected function getBoundedInt(CommandSender $sender, string $value, int $min, int $max): int
        {
            if (!is_numeric($value)) {
                throw new InvalidCommandSyntaxException('Invalid number: ' . $value);
            }
            $int = (int)$value;
            if ($int > $max) {
                throw new InvalidCommandSyntaxException("The number you have entered ($value) is too big, it must be at most $max");
            }
            if ($int < $min) {
                throw new InvalidCommandSyntaxException("The number you have entered ($value) is too small, it must be at least $min");
            }
            return $int;
        }

        protected function getDouble(CommandSender $sender, string $value): float
        {
            if (!is_numeric($value)) {
                throw new InvalidCommandSyntaxException('Invalid number: ' . $value);
            }
            return (float)$value;
        }

        protected function getBoolean(CommandSender $sender, string $value): bool
        {
            $lower = strtolower(trim($value));
            return $lower === 'on' || $lower === 'true' || $lower === 't' || $lower === '1';
        }

        /**
         * PM `fetchPermittedPlayerTarget`: `@s` / missing resolves to the
         * sender; selectors resolve via `resolveSelectorTargets()`. For
         * multi-target selectors the first result is returned (use
         * `fetchPermittedPlayerTargets()` for every match).
         */
        protected function fetchPermittedPlayerTarget(CommandSender $sender, ?string $target, string $selfPermission, string $otherPermission): \pocketmine\player\Player
        {
            $targets = $this->fetchPermittedPlayerTargets($sender, $target, $selfPermission, $otherPermission);
            return $targets[0];
        }

        /**
         * Selector-aware plural variant. Permission rule: every target
         * equal to the sender needs `$selfPermission`, any other target
         * needs `$otherPermission`.
         *
         * @return \pocketmine\player\Player[]
         */
        protected function fetchPermittedPlayerTargets(CommandSender $sender, ?string $target, string $selfPermission, string $otherPermission): array
        {
            $server = $sender->getServer();
            $targets = [];
            if ($target === null) {
                if (!$sender instanceof \pocketmine\player\Player) {
                    throw new InvalidCommandSyntaxException($this->getUsage());
                }
                $targets = [$sender];
            } elseif ($target !== '' && $target[0] === '@') {
                $players = $server->getOnlinePlayers();
                $targets = self::resolveSelectorTargets($sender, array_values($players), $target);
                if ($targets === []) {
                    throw new InvalidCommandSyntaxException('Cannot find a player matching "' . $target . '"');
                }
            } else {
                $player = null;
                if (method_exists($server, 'getPlayerByPrefix')) {
                    $player = $server->getPlayerByPrefix($target);
                }
                if ($player === null && method_exists($server, 'getPlayer')) {
                    $player = $server->getPlayer($target);
                }
                if ($player === null) {
                    throw new InvalidCommandSyntaxException('Cannot find a player matching "' . $target . '"');
                }
                $targets = [$player];
            }

            foreach ($targets as $player) {
                $isSelf = $player instanceof \pocketmine\player\Player
                    && strcasecmp($player->getName(), $sender->getName()) === 0;
                $node = $isSelf ? $selfPermission : $otherPermission;
                if (!$sender->hasPermission($node)) {
                    throw new PermissionCommandException("You don't have permission to use the command: " . $this->getName());
                }
            }
            return $targets;
        }

        /**
         * @param \pocketmine\player\Player[] $players
         * @return \pocketmine\player\Player[]
         */
        public static function resolveSelectorTargets(CommandSender $sender, array $players, string $selector): array
        {
            if ($selector === '' || $selector[0] !== '@' || strlen($selector) < 2) {
                return [];
            }
            $letter = strtolower($selector[1]);
            $kind = match ($letter) {
                's' => 'self',
                'p', 'n' => 'nearest',
                'a', 'c' => 'all',
                'e' => 'entities',
                'r' => 'random',
                default => null,
            };
            if ($kind === null) {
                return [];
            }
            $origin = self::senderOrigin($sender);
            $primaries = [0.0, 64.0, 0.0];
            // Parse bracket args.
            $rest = trim(substr($selector, 2));
            if ($rest !== '' && (!str_starts_with($rest, '[') || !str_ends_with($rest, ']'))) {
                return [];
            }
            $args = self::parseSelectorArgs($rest === '' ? '' : substr($rest, 1, -1), $origin);
            $origin = [
                $args['x'] ?? $origin[0],
                $args['y'] ?? $origin[1],
                $args['z'] ?? $origin[2],
            ];

            $pool = [];
            if ($kind === 'self') {
                foreach ($players as $player) {
                    if (strcasecmp($player->getName(), $sender->getName()) === 0) {
                        $pool[] = $player;
                        break;
                    }
                }
            } else {
                $pool = array_values($players);
            }

            // Filters.
            if (isset($args['name'])) {
                $pool = array_filter($pool, static fn($player) => strcasecmp($player->getName(), (string)$args['name']) === 0);
            }
            if (isset($args['type'])) {
                $type = strtolower((string)$args['type']);
                if ($type !== 'player' && $type !== 'players') {
                    return [];
                }
            }
            if (isset($args['gamemode'])) {
                $pool = array_filter($pool, static fn($player) => (int)$player->getGamemode() === (int)$args['gamemode']);
            }
            $min = $args['distance_min'] ?? null;
            $max = $args['distance_max'] ?? null;
            if ($min !== null || $max !== null) {
                $pool = array_filter($pool, static function ($player) use ($origin, $min, $max) {
                    $dx = (float)$player->getX() - $origin[0];
                    $dy = (float)$player->getY() - $origin[1];
                    $dz = (float)$player->getZ() - $origin[2];
                    $distance = sqrt($dx * $dx + $dy * $dy + $dz * $dz);
                    if ($min !== null && $distance < $min) {
                        return false;
                    }
                    if ($max !== null && $distance > $max) {
                        return false;
                    }
                    return true;
                });
            }
            $pool = array_values($pool);

            // Sort.
            $sort = $args['sort'] ?? match ($kind) {
                'nearest' => 'nearest',
                'random' => 'random',
                default => 'arbitrary',
            };
            if ($sort === 'nearest') {
                usort($pool, static function ($a, $b) use ($origin) {
                    $da = self::distanceTo($a, $origin);
                    $db = self::distanceTo($b, $origin);
                    return $da <=> $db;
                });
            } elseif ($sort === 'furthest') {
                usort($pool, static function ($a, $b) use ($origin) {
                    return self::distanceTo($b, $origin) <=> self::distanceTo($a, $origin);
                });
            } elseif ($sort === 'random') {
                shuffle($pool);
            }

            $defaultLimit = match ($kind) {
                'nearest', 'random', 'self' => 1,
                default => PHP_INT_MAX,
            };
            $limit = $args['limit'] ?? $defaultLimit;
            if ($limit <= 0) {
                return [];
            }
            if ($limit < count($pool)) {
                $pool = array_slice($pool, 0, $limit);
            }

            if ($kind === 'entities') {
                trigger_error('Selector @e: no non-player entities in the Bedrock runtime, returning ' . count($pool) . ' player(s) as entities (type filters other than player match nothing)', E_USER_NOTICE);
            }
            return $pool;
        }

        /** @param mixed $player @return array{0:float,1:float,2:float} */
        public static function senderOrigin(CommandSender $sender): array
        {
            if ($sender instanceof \pocketmine\player\Player) {
                return [(float)$sender->getX(), (float)$sender->getY(), (float)$sender->getZ()];
            }
            // Console: origin 0,0,0 like `command.rs`.
            return [0.0, 0.0, 0.0];
        }

        /** @param mixed $player @param array{0:float,1:float,2:float} $origin */
        private static function distanceTo($player, array $origin): float
        {
            $dx = (float)$player->getX() - $origin[0];
            $dy = (float)$player->getY() - $origin[1];
            $dz = (float)$player->getZ() - $origin[2];
            return sqrt($dx * $dx + $dy * $dy + $dz * $dz);
        }

        /** @param array{0:float,1:float,2:float} $origin @return array<string, mixed> */
        private static function parseSelectorArgs(string $inner, array $origin): array
        {
            $args = [];
            foreach (explode(',', $inner) as $part) {
                $part = trim($part);
                if ($part === '') {
                    continue;
                }
                $equals = strpos($part, '=');
                if ($equals === false) {
                    continue;
                }
                $key = strtolower(trim(substr($part, 0, $equals)));
                $value = trim(substr($part, $equals + 1), " \t\n\r\0\x0B\"'");
                $value = trim($value);
                match ($key) {
                    'x' => $args['x'] = self::parseCoordinate($value, $origin[0]),
                    'y' => $args['y'] = self::parseCoordinate($value, $origin[1]),
                    'z' => $args['z'] = self::parseCoordinate($value, $origin[2]),
                    'distance' => self::parseDistanceRange($value, $args),
                    'limit' => $args['limit'] = max(0, (int)$value),
                    'sort' => $args['sort'] = match (strtolower($value)) {
                        'nearest' => 'nearest',
                        'furthest' => 'furthest',
                        'random' => 'random',
                        default => 'arbitrary',
                    },
                    'name' => $args['name'] = $value,
                    'type' => $args['type'] = $value,
                    'gamemode' => $args['gamemode'] = self::parseGamemode($value),
                    default => null,
                };
            }
            // `parseCoordinate` may be null on invalid input: drop bad axes.
            foreach (['x', 'y', 'z', 'gamemode'] as $key) {
                if (array_key_exists($key, $args) && $args[$key] === null) {
                    unset($args[$key]);
                }
            }
            return $args;
        }

        private static function parseCoordinate(string $value, float $origin): ?float
        {
            $value = trim($value);
            if (str_starts_with($value, '~') || str_starts_with($value, '^')) {
                if ($value === '~' || $value === '^') {
                    return $origin;
                }
                $offset = substr($value, 1);
                if (!is_numeric($offset)) {
                    return null;
                }
                return $origin + (float)$offset;
            }
            return is_numeric($value) ? (float)$value : null;
        }

        /** @param array<string, mixed> $args */
        private static function parseDistanceRange(string $value, array &$args): void
        {
            $value = trim($value);
            $pos = strpos($value, '..');
            if ($pos !== false) {
                $min = trim(substr($value, 0, $pos));
                $max = trim(substr($value, $pos + 2));
                if ($min !== '' && is_numeric($min)) {
                    $args['distance_min'] = (float)$min;
                }
                if ($max !== '' && is_numeric($max)) {
                    $args['distance_max'] = (float)$max;
                }
            } elseif (is_numeric($value)) {
                $args['distance_max'] = (float)$value;
            }
        }

        private static function parseGamemode(string $value): ?int
        {
            return match (strtolower(trim($value))) {
                '0', 's', 'survival', 'supervivencia' => 0,
                '1', 'c', 'creative', 'creativo' => 1,
                '2', 'a', 'adventure', 'aventura' => 2,
                '3', 'sp', 'spectator', 'espectador' => 3,
                default => null,
            };
        }
    }
}
