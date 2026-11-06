<?php

declare(strict_types=1);

namespace pocketmine\command {

    use pocketmine\plugin\Plugin;
    use pocketmine\permission\Permission;
    use pocketmine\permission\PermissionManager;

    interface CommandSender
    {
        public function sendMessage(string $message): void;

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

        public function sendMessage(string $message): void
        {
            try {
                \pocketmine\Server::getInstance()->addAction('console_message', ['message' => $message]);
            } catch (\Throwable) {
                fwrite(STDERR, '[CONSOLE] ' . $message . PHP_EOL);
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
        private mixed $executor = null;
        private ?string $permissionMessage = null;

        public function __construct(
            private string $name,
            private ?string $description = null,
            private ?string $usageMessage = null,
            private array $aliases = [],
            private ?string $permission = null
        ) {
            if ($name === '') {
                throw new \InvalidArgumentException('Command name cannot be empty');
            }
            $this->label = $name;
        }

        public function getName(): string
        {
            return $this->name;
        }

        public function getDescription(): string
        {
            return $this->description ?? '';
        }

        public function setDescription(string $description): void
        {
            $this->description = $description;
        }

        public function getUsage(): string
        {
            return $this->usageMessage ?? '/' . $this->name;
        }

        public function setUsage(string $usage): void
        {
            $this->usageMessage = $usage;
        }

        public function getAliases(): array
        {
            return $this->aliases;
        }

        public function setAliases(array $aliases): void
        {
            $this->aliases = $aliases;
        }

        public function getPermission(): ?string
        {
            return $this->permission;
        }

        public function setPermission(?string $permission): void
        {
            $this->permission = $permission;
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

        public function setLabel(string $label): void
        {
            $this->label = $label;
        }

        public function setExecutor(mixed $executor): void
        {
            $this->executor = $executor;
        }

        public function getExecutor(): mixed
        {
            return $this->executor;
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

        public function testPermission(CommandSender $target): bool
        {
            if ($this->testPermissionSilent($target)) {
                return true;
            }
            $target->sendMessage($this->getPermissionMessage());
            return false;
        }

        public function testPermissionSilent(CommandSender $target): bool
        {
            $permission = $this->permission;
            if ($permission === null || $permission === '') {
                return true;
            }
            return $target->hasPermission($permission);
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

    class CommandMap
    {
        /** @var array<string, Command> */
        private array $commands = [];

        /** @var array<string, string> */
        private array $aliases = [];

        public function register(string $fallbackPrefix, Command $command): bool
        {
            $name = strtolower($command->getName());
            $this->commands[$name] = $command;

            foreach ($command->getAliases() as $alias) {
                $alias = strtolower($alias);
                if ($alias !== '' && !isset($this->aliases[$alias])) {
                    $this->aliases[$alias] = $name;
                }
            }

            $server = \pocketmine\Server::getInstance();
            if (method_exists($server, 'syncCommands')) {
                try {
                    $server->syncCommands();
                } catch (\Throwable) {
                    // Optional hook.
                }
            }
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

        public function unregister(string $name): bool
        {
            $name = strtolower($name);
            if (!isset($this->commands[$name])) {
                return false;
            }
            unset($this->commands[$name]);
            foreach ($this->aliases as $alias => $target) {
                if ($target === $name) {
                    unset($this->aliases[$alias]);
                }
            }
            return true;
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

            $parts = preg_split('/\s+/', $commandLine) ?: [];
            if ($parts === []) {
                return false;
            }

            $label = strtolower(array_shift($parts));
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
    }
}
