<?php

declare(strict_types=1);

namespace pocketmine\permission {

    /**
     * Mirror of PM5 `DefaultPermissionNames` (61 nodes). Every constant
     * matches `target/axolotl-pm/src/permission/DefaultPermissionNames.php`.
     */
    final class DefaultPermissionNames
    {
        public const BROADCAST_ADMIN = 'pocketmine.broadcast.admin';
        public const BROADCAST_USER = 'pocketmine.broadcast.user';
        public const COMMAND_BAN_IP = 'pocketmine.command.ban.ip';
        public const COMMAND_BAN_LIST = 'pocketmine.command.ban.list';
        public const COMMAND_BAN_PLAYER = 'pocketmine.command.ban.player';
        public const COMMAND_CLEAR_OTHER = 'pocketmine.command.clear.other';
        public const COMMAND_CLEAR_SELF = 'pocketmine.command.clear.self';
        public const COMMAND_DEFAULTGAMEMODE = 'pocketmine.command.defaultgamemode';
        public const COMMAND_DIFFICULTY = 'pocketmine.command.difficulty';
        public const COMMAND_DUMPMEMORY = 'pocketmine.command.dumpmemory';
        public const COMMAND_EFFECT_OTHER = 'pocketmine.command.effect.other';
        public const COMMAND_EFFECT_SELF = 'pocketmine.command.effect.self';
        public const COMMAND_ENCHANT_OTHER = 'pocketmine.command.enchant.other';
        public const COMMAND_ENCHANT_SELF = 'pocketmine.command.enchant.self';
        public const COMMAND_GAMEMODE_OTHER = 'pocketmine.command.gamemode.other';
        public const COMMAND_GAMEMODE_SELF = 'pocketmine.command.gamemode.self';
        public const COMMAND_GC = 'pocketmine.command.gc';
        public const COMMAND_GIVE_OTHER = 'pocketmine.command.give.other';
        public const COMMAND_GIVE_SELF = 'pocketmine.command.give.self';
        public const COMMAND_HELP = 'pocketmine.command.help';
        public const COMMAND_KICK = 'pocketmine.command.kick';
        public const COMMAND_KILL_OTHER = 'pocketmine.command.kill.other';
        public const COMMAND_KILL_SELF = 'pocketmine.command.kill.self';
        public const COMMAND_LIST = 'pocketmine.command.list';
        public const COMMAND_ME = 'pocketmine.command.me';
        public const COMMAND_OP_GIVE = 'pocketmine.command.op.give';
        public const COMMAND_OP_TAKE = 'pocketmine.command.op.take';
        public const COMMAND_PARTICLE = 'pocketmine.command.particle';
        public const COMMAND_PLUGINS = 'pocketmine.command.plugins';
        public const COMMAND_SAVE_DISABLE = 'pocketmine.command.save.disable';
        public const COMMAND_SAVE_ENABLE = 'pocketmine.command.save.enable';
        public const COMMAND_SAVE_PERFORM = 'pocketmine.command.save.perform';
        public const COMMAND_SAY = 'pocketmine.command.say';
        public const COMMAND_SEED = 'pocketmine.command.seed';
        public const COMMAND_SETWORLDSPAWN = 'pocketmine.command.setworldspawn';
        public const COMMAND_SPAWNPOINT_OTHER = 'pocketmine.command.spawnpoint.other';
        public const COMMAND_SPAWNPOINT_SELF = 'pocketmine.command.spawnpoint.self';
        public const COMMAND_STATUS = 'pocketmine.command.status';
        public const COMMAND_STOP = 'pocketmine.command.stop';
        public const COMMAND_TELEPORT_OTHER = 'pocketmine.command.teleport.other';
        public const COMMAND_TELEPORT_SELF = 'pocketmine.command.teleport.self';
        public const COMMAND_TELL = 'pocketmine.command.tell';
        public const COMMAND_TIME_ADD = 'pocketmine.command.time.add';
        public const COMMAND_TIME_QUERY = 'pocketmine.command.time.query';
        public const COMMAND_TIME_SET = 'pocketmine.command.time.set';
        public const COMMAND_TIME_START = 'pocketmine.command.time.start';
        public const COMMAND_TIME_STOP = 'pocketmine.command.time.stop';
        public const COMMAND_TIMINGS = 'pocketmine.command.timings';
        public const COMMAND_TITLE_OTHER = 'pocketmine.command.title.other';
        public const COMMAND_TITLE_SELF = 'pocketmine.command.title.self';
        public const COMMAND_TRANSFERSERVER = 'pocketmine.command.transferserver';
        public const COMMAND_UNBAN_IP = 'pocketmine.command.unban.ip';
        public const COMMAND_UNBAN_PLAYER = 'pocketmine.command.unban.player';
        public const COMMAND_VERSION = 'pocketmine.command.version';
        public const COMMAND_WHITELIST_ADD = 'pocketmine.command.whitelist.add';
        public const COMMAND_WHITELIST_DISABLE = 'pocketmine.command.whitelist.disable';
        public const COMMAND_WHITELIST_ENABLE = 'pocketmine.command.whitelist.enable';
        public const COMMAND_WHITELIST_LIST = 'pocketmine.command.whitelist.list';
        public const COMMAND_WHITELIST_RELOAD = 'pocketmine.command.whitelist.reload';
        public const COMMAND_WHITELIST_REMOVE = 'pocketmine.command.whitelist.remove';
        public const COMMAND_XP_OTHER = 'pocketmine.command.xp.other';
        public const COMMAND_XP_SELF = 'pocketmine.command.xp.self';
        public const GROUP_CONSOLE = 'pocketmine.group.console';
        public const GROUP_OPERATOR = 'pocketmine.group.operator';
        public const GROUP_USER = 'pocketmine.group.user';
    }

    /**
     * PM5 `Permission` + `PermissionParser` defaults. Children are a
     * `child => grant` map like PM (the Rust port flattens to a list).
     */
    class Permission
    {
        public const DEFAULT_TRUE = 'true';
        public const DEFAULT_FALSE = 'false';
        public const DEFAULT_OP = 'op';
        public const DEFAULT_NOT_OP = 'notop';

        /** @var array<string, bool> */
        private array $children = [];

        /**
         * @param string|\pocketmine\lang\Translatable|null $description
         * @param array<string, bool> $children
         */
        public function __construct(
            private string $name,
            private mixed $description = null,
            private ?string $default = self::DEFAULT_OP,
            array $children = []
        ) {
            if ($this->name === '') {
                throw new \InvalidArgumentException('Permission name cannot be empty');
            }
            $this->children = $children;
        }

        public function getName(): string
        {
            return $this->name;
        }

        /** @return string|\pocketmine\lang\Translatable|null */
        public function getDescription(): mixed
        {
            return $this->description;
        }

        public function setDescription(mixed $description): void
        {
            $this->description = $description;
        }

        public function getDefault(): string
        {
            return $this->default ?? self::DEFAULT_OP;
        }

        public function getDefaultPermission(): string
        {
            return $this->getDefault();
        }

        /** @return array<string, bool> */
        public function getChildren(): array
        {
            return $this->children;
        }

        public function addChild(string $name, bool $value = true): void
        {
            $this->children[$name] = $value;
        }

        public function removeChild(string $name): void
        {
            unset($this->children[$name]);
        }

        public function recalculatePermissibles(): void
        {
            // Permissible objects recalculate themselves on demand in this
            // bridge (no subscription graph); kept for PM API parity.
        }

        /** @return array */
        public function getPermissibles(): array
        {
            return [];
        }

        public function __toString(): string
        {
            return 'Permission(name=' . $this->name . ',default=' . $this->getDefault() . ')';
        }
    }

    /**
     * PM `PermissionParser::defaultFromString`: YAML aliases for defaults.
     */
    final class PermissionParser
    {
        public const DEFAULT_STRING_MAP = [
            'op' => Permission::DEFAULT_OP,
            'isop' => Permission::DEFAULT_OP,
            'operator' => Permission::DEFAULT_OP,
            'isoperator' => Permission::DEFAULT_OP,
            'admin' => Permission::DEFAULT_OP,
            'isadmin' => Permission::DEFAULT_OP,
            '!op' => Permission::DEFAULT_NOT_OP,
            'notop' => Permission::DEFAULT_NOT_OP,
            '!operator' => Permission::DEFAULT_NOT_OP,
            'notoperator' => Permission::DEFAULT_NOT_OP,
            '!admin' => Permission::DEFAULT_NOT_OP,
            'notadmin' => Permission::DEFAULT_NOT_OP,
            'true' => Permission::DEFAULT_TRUE,
            'false' => Permission::DEFAULT_FALSE,
        ];

        /**
         * @throws \InvalidArgumentException
         */
        public static function defaultFromString(bool|string $value): string
        {
            if (is_bool($value)) {
                return $value ? Permission::DEFAULT_TRUE : Permission::DEFAULT_FALSE;
            }
            $lower = strtolower($value);
            if (isset(self::DEFAULT_STRING_MAP[$lower])) {
                return self::DEFAULT_STRING_MAP[$lower];
            }
            throw new \InvalidArgumentException('Unknown permission default name "' . $value . '"');
        }

        /**
         * Mirrors `PermissionParser::loadPermissions`: `permissions:` subtree
         * from a `plugin.yml` manifest.
         *
         * @param array<string, array{description?: string, default?: bool|string, children?: array}> $data
         * @return Permission[]
         */
        public static function loadPermissions(array $data, string $default = Permission::DEFAULT_FALSE): array
        {
            $result = [];
            foreach ($data as $name => $entry) {
                $entryDefault = $default;
                if (isset($entry['default'])) {
                    $entryDefault = self::defaultFromString($entry['default']);
                }
                if (isset($entry['children'])) {
                    throw new \InvalidArgumentException('Nested permission declarations are no longer supported. Declare each permission separately.');
                }
                $desc = $entry['description'] ?? null;
                $result[] = new Permission((string)$name, $desc, $entryDefault);
            }
            return $result;
        }
    }

    /**
     * PM `Permissible` contract (simplified): a subject the server asks
     * about permissions. `Player` implements the full surface; this
     * interface keeps plugin code written against PM type-checking.
     */
    interface Permissible
    {
        public function hasPermission($permission): bool;
        public function isPermissionSet($permission): bool;
        public function isOp(): bool;
    }

    /**
     * Minimal `PermissibleInternal` parity: resolves a permission against
     * base grants + registered defaults, without the attachment graph.
     * Used by `Player::hasPermission` when no explicit map is set.
     */
    class PermissibleInternal implements Permissible
    {
        /** @var array<string, bool> */
        private array $rootPermissions = [];

        /** @var array<string, bool> */
        private array $effective = [];

        /**
         * @param array<string, bool> $basePermissions
         */
        public function __construct(array $basePermissions = [])
        {
            $this->rootPermissions = $basePermissions;
            $this->recalculatePermissions();
        }

        public function setBasePermission(Permission|string $name, bool $grant): void
        {
            $name = $name instanceof Permission ? $name->getName() : $name;
            $this->rootPermissions[$name] = $grant;
            $this->recalculatePermissions();
        }

        public function unsetBasePermission(Permission|string $name): void
        {
            $name = $name instanceof Permission ? $name->getName() : $name;
            unset($this->rootPermissions[$name]);
            $this->recalculatePermissions();
        }

        public function isPermissionSet($permission): bool
        {
            $name = $permission instanceof Permission ? $permission->getName() : (string)$permission;
            return isset($this->effective[$name]);
        }

        public function hasPermission($permission): bool
        {
            $name = $permission instanceof Permission ? $permission->getName() : (string)$permission;
            return $this->effective[$name] ?? false;
        }

        public function isOp(): bool
        {
            return false;
        }

        public function recalculatePermissions(): array
        {
            $old = $this->effective;
            $this->effective = [];
            $manager = PermissionManager::getInstance();
            foreach ($this->rootPermissions as $name => $granted) {
                $perm = $manager->getPermission($name);
                $this->effective[$name] = $granted;
                if ($perm !== null) {
                    $this->calculateChildPermissions($perm->getChildren(), !$granted);
                }
            }
            $diff = [];
            foreach ($this->effective as $name => $value) {
                if (($old[$name] ?? null) !== $value) {
                    $diff[$name] = $value;
                }
            }
            return $diff;
        }

        /**
         * @param array<string, bool> $children
         */
        private function calculateChildPermissions(array $children, bool $invert): void
        {
            $manager = PermissionManager::getInstance();
            foreach ($children as $name => $grant) {
                $value = ($grant xor $invert);
                $this->effective[$name] = $value;
                $perm = $manager->getPermission($name);
                if ($perm !== null) {
                    $this->calculateChildPermissions($perm->getChildren(), !$value);
                }
            }
        }

        public function destroyCycles(): void
        {
            $this->effective = [];
        }
    }

    class PermissionManager
    {
        private static ?PermissionManager $instance = null;

        /** @var array<string, Permission> */
        private array $permissions = [];

        /** @var array<string, array<string, bool|null>> subject => permission => granted */
        private array $subscriptions = [];

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
        }

        public function registerPermission(Permission $permission): Permission
        {
            if (!isset($this->permissions[$permission->getName()])) {
                $this->permissions[$permission->getName()] = $permission;
            }
            return $this->permissions[$permission->getName()];
        }

        /**
         * PM `PermissionManager::addPermission` alias.
         */
        public function addPermission(Permission $permission): bool
        {
            if (!isset($this->permissions[$permission->getName()])) {
                $this->permissions[$permission->getName()] = $permission;
                return true;
            }
            return false;
        }

        public function removePermission(Permission|string $permission): void
        {
            $name = $permission instanceof Permission ? $permission->getName() : $permission;
            unset($this->permissions[$name]);
        }

        public function getPermission(string $name): ?Permission
        {
            return $this->permissions[$name] ?? null;
        }

        /** @return array<string, Permission> */
        public function getPermissions(): array
        {
            return $this->permissions;
        }

        public function getDefaultPermissions(bool $op): array
        {
            $result = [];
            foreach ($this->permissions as $name => $permission) {
                $default = $permission->getDefault();
                if ($default === Permission::DEFAULT_TRUE || ($op && $default === Permission::DEFAULT_OP)
                    || (!$op && $default === Permission::DEFAULT_NOT_OP)) {
                    $result[$name] = true;
                }
            }
            return $result;
        }

        public function recalculatePermissionDefaults(Permission $permission): void
        {
            $name = $permission->getName();
            foreach ($this->subscriptions as $subject => $permissions) {
                if (isset($permissions[$name])) {
                    $this->subscriptions[$subject][$name] = null;
                }
            }
        }

        public function subscribeToPermission(string $permission, string $subject): void
        {
            $this->subscriptions[$subject][$permission] = null;
        }

        public function unsubscribeFromPermission(string $permission, string $subject): void
        {
            unset($this->subscriptions[$subject][$permission]);
        }

        public function clearPermissions(): void
        {
            $this->permissions = [];
            $this->subscriptions = [];
        }

        /**
         * Mirrors PocketMine's resolution rules for a subject without an
         * explicit permission map: the registered default decides.
         */
        public function hasPermission(?Permission $permission, bool $isOp, ?bool $explicit = null): bool
        {
            if ($permission === null) {
                return false;
            }
            if ($explicit !== null) {
                return $explicit;
            }

            return match ($permission->getDefault()) {
                Permission::DEFAULT_TRUE => true,
                Permission::DEFAULT_FALSE => false,
                Permission::DEFAULT_OP => $isOp,
                Permission::DEFAULT_NOT_OP => !$isOp,
                default => false,
            };
        }

        /**
         * Resolves a node name for a subject (PM `Permissible::hasPermission`
         * equivalent without an object): attachment values win, then the
         * registered default. Unregistered nodes fall back to operator.
         *
         * @param array<string, bool>|null $explicitMap
         */
        public function hasNodePermission(string $name, bool $isOp, ?array $explicitMap = null): bool
        {
            if ($explicitMap !== null && array_key_exists($name, $explicitMap)) {
                return (bool)$explicitMap[$name];
            }
            $permission = $this->getPermission($name);
            if ($permission === null) {
                return $isOp;
            }
            return $this->hasPermission($permission, $isOp);
        }

        public function isRegistered(string $name): bool
        {
            return isset($this->permissions[$name]);
        }
    }

    /**
     * Mirror of PM5 `DefaultPermissions::registerCorePermissions`:
     * console -> operator -> user tree with `children` edges.
     */
    final class DefaultPermissions
    {
        public const ROOT_CONSOLE = DefaultPermissionNames::GROUP_CONSOLE;
        public const ROOT_OPERATOR = DefaultPermissionNames::GROUP_OPERATOR;
        public const ROOT_USER = DefaultPermissionNames::GROUP_USER;
        // Legacy PM4 aliases still used by older plugins.
        public const ROOT = 'pocketmine';
        public const OPERATOR = 'pocketmine.operator';
        public const USER = 'pocketmine.user';

        /**
         * @param Permission[] $grantedBy
         * @param Permission[] $deniedBy
         */
        public static function registerPermission(Permission $candidate, array $grantedBy = [], array $deniedBy = []): Permission
        {
            foreach ($grantedBy as $permission) {
                $permission->addChild($candidate->getName(), true);
            }
            foreach ($deniedBy as $permission) {
                $permission->addChild($candidate->getName(), false);
            }
            PermissionManager::getInstance()->registerPermission($candidate);
            return PermissionManager::getInstance()->getPermission($candidate->getName());
        }

        public static function registerCorePermissions(): void
        {
            $names = new \ReflectionClass(DefaultPermissionNames::class);
            $manager = PermissionManager::getInstance();

            $consoleRoot = self::registerPermission(new Permission(self::ROOT_CONSOLE, null, Permission::DEFAULT_OP));
            $operatorRoot = self::registerPermission(new Permission(self::ROOT_OPERATOR, null, Permission::DEFAULT_OP), [$consoleRoot]);
            $everyoneRoot = self::registerPermission(new Permission(self::ROOT_USER, null, Permission::DEFAULT_OP), [$operatorRoot]);

            // Console-only (PM grants dumpmemory from the console root).
            self::registerPermission(new Permission(DefaultPermissionNames::COMMAND_DUMPMEMORY, null, Permission::DEFAULT_OP), [$consoleRoot]);

            $operatorPerms = [
                DefaultPermissionNames::BROADCAST_ADMIN,
                DefaultPermissionNames::COMMAND_BAN_IP,
                DefaultPermissionNames::COMMAND_BAN_LIST,
                DefaultPermissionNames::COMMAND_BAN_PLAYER,
                DefaultPermissionNames::COMMAND_CLEAR_OTHER,
                DefaultPermissionNames::COMMAND_DEFAULTGAMEMODE,
                DefaultPermissionNames::COMMAND_DIFFICULTY,
                DefaultPermissionNames::COMMAND_EFFECT_OTHER,
                DefaultPermissionNames::COMMAND_EFFECT_SELF,
                DefaultPermissionNames::COMMAND_ENCHANT_OTHER,
                DefaultPermissionNames::COMMAND_ENCHANT_SELF,
                DefaultPermissionNames::COMMAND_GAMEMODE_OTHER,
                DefaultPermissionNames::COMMAND_GAMEMODE_SELF,
                DefaultPermissionNames::COMMAND_GC,
                DefaultPermissionNames::COMMAND_GIVE_OTHER,
                DefaultPermissionNames::COMMAND_GIVE_SELF,
                DefaultPermissionNames::COMMAND_KICK,
                DefaultPermissionNames::COMMAND_KILL_OTHER,
                DefaultPermissionNames::COMMAND_LIST,
                DefaultPermissionNames::COMMAND_OP_GIVE,
                DefaultPermissionNames::COMMAND_OP_TAKE,
                DefaultPermissionNames::COMMAND_PARTICLE,
                DefaultPermissionNames::COMMAND_PLUGINS,
                DefaultPermissionNames::COMMAND_SAVE_DISABLE,
                DefaultPermissionNames::COMMAND_SAVE_ENABLE,
                DefaultPermissionNames::COMMAND_SAVE_PERFORM,
                DefaultPermissionNames::COMMAND_SAY,
                DefaultPermissionNames::COMMAND_SEED,
                DefaultPermissionNames::COMMAND_SETWORLDSPAWN,
                DefaultPermissionNames::COMMAND_SPAWNPOINT_OTHER,
                DefaultPermissionNames::COMMAND_SPAWNPOINT_SELF,
                DefaultPermissionNames::COMMAND_STATUS,
                DefaultPermissionNames::COMMAND_STOP,
                DefaultPermissionNames::COMMAND_TELEPORT_OTHER,
                DefaultPermissionNames::COMMAND_TELEPORT_SELF,
                DefaultPermissionNames::COMMAND_TIME_ADD,
                DefaultPermissionNames::COMMAND_TIME_QUERY,
                DefaultPermissionNames::COMMAND_TIME_SET,
                DefaultPermissionNames::COMMAND_TIME_START,
                DefaultPermissionNames::COMMAND_TIME_STOP,
                DefaultPermissionNames::COMMAND_TIMINGS,
                DefaultPermissionNames::COMMAND_TITLE_OTHER,
                DefaultPermissionNames::COMMAND_TITLE_SELF,
                DefaultPermissionNames::COMMAND_TRANSFERSERVER,
                DefaultPermissionNames::COMMAND_UNBAN_IP,
                DefaultPermissionNames::COMMAND_UNBAN_PLAYER,
                DefaultPermissionNames::COMMAND_WHITELIST_ADD,
                DefaultPermissionNames::COMMAND_WHITELIST_DISABLE,
                DefaultPermissionNames::COMMAND_WHITELIST_ENABLE,
                DefaultPermissionNames::COMMAND_WHITELIST_LIST,
                DefaultPermissionNames::COMMAND_WHITELIST_RELOAD,
                DefaultPermissionNames::COMMAND_WHITELIST_REMOVE,
                DefaultPermissionNames::COMMAND_XP_OTHER,
                DefaultPermissionNames::COMMAND_XP_SELF,
            ];
            foreach ($operatorPerms as $permission) {
                self::registerPermission(new Permission($permission, null, Permission::DEFAULT_OP), [$operatorRoot]);
            }

            foreach ([
                DefaultPermissionNames::COMMAND_KILL_SELF,
                DefaultPermissionNames::COMMAND_ME,
                DefaultPermissionNames::COMMAND_HELP,
                DefaultPermissionNames::BROADCAST_USER,
                DefaultPermissionNames::COMMAND_CLEAR_SELF,
                DefaultPermissionNames::COMMAND_TELL,
                DefaultPermissionNames::COMMAND_VERSION,
            ] as $permission) {
                self::registerPermission(new Permission($permission, null, Permission::DEFAULT_OP), [$everyoneRoot]);
            }

            // Legacy group aliases point at the PM5 roots.
            $manager->registerPermission(new Permission(self::ROOT, null, Permission::DEFAULT_OP));
            $manager->registerPermission(new Permission(self::OPERATOR, null, Permission::DEFAULT_OP));
            $manager->registerPermission(new Permission(self::USER, null, Permission::DEFAULT_TRUE));
            // Keep the reflection referenced so a missing constant fails loudly.
            unset($names);
        }
    }

    /**
     * Mirror of PM5 `BanEntry`: `name|created|source|expires|reason`.
     * Dates are unix timestamps formatted on demand; `fromString` accepts
     * both the PM format and the legacy `name|reason` lines.
     */
    class BanEntry
    {
        public static string $format = 'Y-m-d H:i:s';

        private string $name;
        private int $created;
        private string $source = '(Unknown)';
        private ?int $expires = null;
        private string $reason = 'Banned by an operator.';

        public function __construct(string $name)
        {
            $this->name = strtolower($name);
            $this->created = time();
        }

        public function getName(): string
        {
            return $this->name;
        }

        public function getCreated(): int
        {
            return $this->created;
        }

        public function setCreated(int $timestamp): void
        {
            $this->created = $timestamp;
        }

        public function getSource(): string
        {
            return $this->source;
        }

        public function setSource(string $source): void
        {
            $this->source = $source;
        }

        public function getExpires(): ?int
        {
            return $this->expires;
        }

        public function setExpires(?int $timestamp): void
        {
            $this->expires = $timestamp;
        }

        public function hasExpired(): bool
        {
            return $this->expires === null ? false : $this->expires < time();
        }

        public function getReason(): string
        {
            return $this->reason;
        }

        public function setReason(string $reason): void
        {
            $this->reason = $reason;
        }

        public function getString(): string
        {
            $expires = $this->expires === null ? 'Forever' : gmdate(self::$format, $this->expires);
            return implode('|', [
                $this->name,
                gmdate(self::$format, $this->created),
                $this->source,
                $expires,
                $this->reason,
            ]);
        }

        /**
         * @throws \RuntimeException on unparseable dates
         */
        public static function fromString(string $str): ?BanEntry
        {
            $str = trim($str);
            if (strlen($str) < 2) {
                return null;
            }
            $parts = explode('|', $str, 6);
            $entry = new BanEntry(trim((string)array_shift($parts)));
            // Legacy `name|reason`: the second field is not a date.
            if (count($parts) === 1) {
                $second = trim($parts[0]);
                $parsed = self::parseDate($second);
                if ($parsed === null) {
                    if ($second !== '') {
                        $entry->setReason($second);
                    }
                } else {
                    $entry->setCreated($parsed);
                }
                return $entry;
            }
            if (count($parts) > 0) {
                $parsed = self::parseDate(trim((string)array_shift($parts)));
                if ($parsed !== null) {
                    $entry->setCreated($parsed);
                }
            }
            if (count($parts) > 0) {
                $entry->setSource(trim((string)array_shift($parts)));
            }
            if (count($parts) > 0) {
                $expire = trim((string)array_shift($parts));
                if ($expire !== '' && strtolower($expire) !== 'forever') {
                    $parsed = self::parseDate($expire);
                    if ($parsed !== null) {
                        $entry->setExpires($parsed);
                    }
                }
            }
            if (count($parts) > 0) {
                $entry->setReason(trim((string)array_shift($parts)));
            }
            return $entry;
        }

        private static function parseDate(string $date): ?int
        {
            if ($date === '') {
                return null;
            }
            if (is_numeric($date)) {
                return (int)$date;
            }
            $timestamp = strtotime($date);
            return $timestamp === false ? null : $timestamp;
        }
    }

    /**
     * Mirror of PM5 `BanList` over a text file. Expiration is enforced on
     * every read (`isBanned`/`getEntry`/`getEntries`), like PM.
     */
    class BanList
    {
        /** @var array<string, BanEntry> */
        private array $list = [];

        private bool $enabled = true;

        public function __construct(
            private string $file
        ) {
        }

        public function isEnabled(): bool
        {
            return $this->enabled;
        }

        public function setEnabled(bool $flag): void
        {
            $this->enabled = $flag;
        }

        public function getEntry(string $name): ?BanEntry
        {
            $this->removeExpired();
            return $this->list[strtolower($name)] ?? null;
        }

        /** @return array<string, BanEntry> */
        public function getEntries(): array
        {
            $this->removeExpired();
            return $this->list;
        }

        public function isBanned(string $name): bool
        {
            $name = strtolower($name);
            if (!$this->enabled) {
                return false;
            }
            $this->removeExpired();
            return isset($this->list[$name]);
        }

        public function add(BanEntry $entry): void
        {
            $this->list[$entry->getName()] = $entry;
            $this->save();
        }

        public function addBan(string $target, ?string $reason = null, ?int $expires = null, ?string $source = null): BanEntry
        {
            $entry = new BanEntry($target);
            $entry->setSource($source ?? $entry->getSource());
            $entry->setExpires($expires);
            $entry->setReason($reason ?? $entry->getReason());
            $this->list[$entry->getName()] = $entry;
            $this->save();
            return $entry;
        }

        public function remove(string $name): void
        {
            $name = strtolower($name);
            if (isset($this->list[$name])) {
                unset($this->list[$name]);
                $this->save();
            }
        }

        public function removeExpired(): void
        {
            foreach ($this->list as $name => $entry) {
                if ($entry->hasExpired()) {
                    unset($this->list[$name]);
                }
            }
        }

        public function load(): void
        {
            $this->list = [];
            $fp = @fopen($this->file, 'r');
            if (is_resource($fp)) {
                while (($line = fgets($fp)) !== false) {
                    if ($line[0] !== '#') {
                        try {
                            $entry = BanEntry::fromString($line);
                            if ($entry !== null) {
                                $this->list[$entry->getName()] = $entry;
                            }
                        } catch (\Throwable $e) {
                            fwrite(STDERR, '[BAN] Failed to parse ban entry: ' . $e->getMessage() . PHP_EOL);
                        }
                    }
                }
                fclose($fp);
            }
            $this->removeExpired();
        }

        public function save(bool $writeHeader = true): void
        {
            $this->removeExpired();
            $fp = @fopen($this->file, 'w');
            if (is_resource($fp)) {
                if ($writeHeader) {
                    fwrite($fp, "# victim name | ban date | banned by | banned until | reason\n\n");
                }
                foreach ($this->list as $entry) {
                    fwrite($fp, $entry->getString() . "\n");
                }
                fclose($fp);
            }
        }
    }
}
