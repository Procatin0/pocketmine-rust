<?php

declare(strict_types=1);

namespace pocketmine\permission {

    class Permission
    {
        public const DEFAULT_TRUE = 'true';
        public const DEFAULT_FALSE = 'false';
        public const DEFAULT_OP = 'op';
        public const DEFAULT_NOT_OP = 'notop';

        public function __construct(
            private string $name,
            private ?string $description = null,
            private ?string $default = self::DEFAULT_OP
        ) {
            if ($this->name === '') {
                throw new \InvalidArgumentException('Permission name cannot be empty');
            }
        }

        public function getName(): string
        {
            return $this->name;
        }

        public function getDescription(): ?string
        {
            return $this->description;
        }

        public function getDefault(): string
        {
            return $this->default ?? self::DEFAULT_OP;
        }

        public function getDefaultPermission(): string
        {
            return $this->getDefault();
        }

        public function __toString(): string
        {
            return 'Permission(name=' . $this->name . ',default=' . $this->getDefault() . ')';
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

        public function isRegistered(string $name): bool
        {
            return isset($this->permissions[$name]);
        }
    }

    final class DefaultPermissions
    {
        public const ROOT = 'pocketmine';
        public const OPERATOR = 'pocketmine.operator';
        public const USER = 'pocketmine.user';

        public static function registerCorePermissions(): void
        {
            $manager = PermissionManager::getInstance();
            $manager->registerPermission(new Permission(self::ROOT, 'Root permission', Permission::DEFAULT_OP));
            $manager->registerPermission(new Permission(self::OPERATOR, 'Operator permission', Permission::DEFAULT_OP));
            $manager->registerPermission(new Permission(self::USER, 'Default user permission', Permission::DEFAULT_TRUE));
        }
    }
}
