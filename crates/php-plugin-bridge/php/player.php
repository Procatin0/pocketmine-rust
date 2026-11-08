<?php

declare(strict_types=1);

namespace pocketmine\player {

    use pocketmine\command\CommandSender;
    use pocketmine\inventory\PlayerInventory;
    use pocketmine\math\Position;
    use pocketmine\math\Vector3;
    use pocketmine\permission\Permission;
    use pocketmine\permission\PermissionManager;
    use pocketmine\utils\TextFormat;
    use pocketmine\world\Location;
    use pocketmine\world\World;

    class Player implements CommandSender
    {
        public string $name;
        public string $displayName;
        public string $address;
        public int $port;
        public int|string $runtimeId;
        public int|string $uniqueId;
        public int $ping;
        public int $gamemode;
        public string $worldName;
        public float $x;
        public float $y;
        public float $z;
        public float $yaw;
        public float $pitch;
        public float $health;
        public float $maxHealth;
        public bool $flying;
        public float $xp;
        public int $xpLevel;

        private PlayerInventory $inventory;
        private bool $online = true;
        private array $permissions = [];
        private ?bool $opOverride = null;

        public function __construct(array|string $data = ['name' => 'Steve'])
        {
            if (is_string($data)) {
                $data = ['name' => $data];
            }

            $this->name = (string)($data['name'] ?? $data['username'] ?? 'Steve');
            $this->displayName = (string)($data['display_name'] ?? $data['displayName'] ?? $this->name);
            $this->address = (string)($data['address'] ?? $data['ip'] ?? '127.0.0.1');
            $this->port = (int)($data['port'] ?? 19132);
            $this->runtimeId = (int)($data['runtime_id'] ?? $data['runtimeId'] ?? 1);
            $uniqueId = $data['unique_id'] ?? $data['uniqueId'] ?? $this->name;
            $this->uniqueId = is_numeric($uniqueId) ? (int)$uniqueId : (string)$uniqueId;
            $this->ping = (int)($data['ping'] ?? 0);
            $this->gamemode = (int)($data['gamemode'] ?? 0);
            $this->worldName = (string)($data['world'] ?? 'world');
            $this->x = (float)($data['x'] ?? 0.0);
            $this->y = (float)($data['y'] ?? 64.0);
            $this->z = (float)($data['z'] ?? 0.0);
            $this->yaw = (float)($data['yaw'] ?? 0.0);
            $this->pitch = (float)($data['pitch'] ?? 0.0);
            $this->health = (float)($data['health'] ?? 20.0);
            $this->maxHealth = (float)($data['max_health'] ?? 20.0);
            $this->flying = (bool)($data['flying'] ?? false);
            $this->xp = (float)($data['xp'] ?? 0.0);
            $this->xpLevel = (int)($data['xp_level'] ?? 0);
            $this->online = (bool)($data['online'] ?? true);

            if (isset($data['permissions']) && is_array($data['permissions'])) {
                $this->permissions = $data['permissions'];
            }
            if (array_key_exists('is_op', $data) || array_key_exists('op', $data)) {
                $this->opOverride = (bool)($data['is_op'] ?? $data['op']);
            }

            $this->inventory = new PlayerInventory($this->name);
        }

        public function getName(): string
        {
            return $this->name;
        }

        public function getLowerCaseName(): string
        {
            return strtolower($this->name);
        }

        public function getDisplayName(): string
        {
            return $this->displayName;
        }

        public function setDisplayName(string $name): void
        {
            $this->displayName = $name;
        }

        public function getRawDisplayName(): string
        {
            return TextFormat::clean($this->displayName);
        }

        public function getAddress(): string
        {
            return $this->address;
        }

        public function getPort(): int
        {
            return $this->port;
        }

        public function getPing(): int
        {
            return $this->ping;
        }

        public function getPingTicks(): int
        {
            return (int)round($this->ping / 50);
        }

        public function getGamemode(): int
        {
            return $this->gamemode;
        }

        public function setGamemode(int $gamemode): void
        {
            $this->gamemode = $gamemode;
            $this->addAction('set_gamemode', ['target_player' => $this->name, 'gamemode' => $gamemode]);
        }

        public function getX(): float
        {
            return $this->x;
        }

        public function getY(): float
        {
            return $this->y;
        }

        public function getZ(): float
        {
            return $this->z;
        }

        public function getYaw(): float
        {
            return $this->yaw;
        }

        public function getPitch(): float
        {
            return $this->pitch;
        }

        public function getPosition(): Position
        {
            return new Position($this->x, $this->y, $this->z, $this->worldName);
        }

        public function getLocation(): Location
        {
            return new Location($this->x, $this->y, $this->z, $this->yaw, $this->pitch, $this->worldName);
        }

        public function getWorld(): World
        {
            return \pocketmine\Server::getInstance()->getWorldManager()->getWorldByName($this->worldName);
        }

        public function getHealth(): float
        {
            return $this->health;
        }

        public function setHealth(float $health): void
        {
            $this->health = max(0.0, min($this->maxHealth, $health));
        }

        public function getMaxHealth(): float
        {
            return $this->maxHealth;
        }

        public function setMaxHealth(float $health): void
        {
            $this->maxHealth = max(1.0, $health);
            if ($this->health > $this->maxHealth) {
                $this->health = $this->maxHealth;
            }
        }

        public function isFlying(): bool
        {
            return $this->flying;
        }

        public function setFlying(bool $flying): void
        {
            $this->flying = $flying;
        }

        public function getXp(): float
        {
            return $this->xp;
        }

        public function setXp(float $xp): void
        {
            $this->xp = max(0.0, $xp);
        }

        public function getXpLevel(): int
        {
            return $this->xpLevel;
        }

        public function setXpLevel(int $level): void
        {
            $this->xpLevel = max(0, $level);
        }

        public function getInventory(): PlayerInventory
        {
            return $this->inventory;
        }

        public function isOnline(): bool
        {
            return $this->online;
        }

        public function isAlive(): bool
        {
            return $this->health > 0.0;
        }

        public function isClosed(): bool
        {
            return !$this->online;
        }

        public function isConnected(): bool
        {
            return $this->online;
        }

        public function isOp(): bool
        {
            if ($this->opOverride !== null) {
                return $this->opOverride;
            }
            return \pocketmine\Server::getInstance()->isOp($this->name);
        }

        public function isPlayer(): bool
        {
            return true;
        }

        public function hasPermission($permission): bool
        {
            if (is_string($permission)) {
                if (array_key_exists($permission, $this->permissions)) {
                    return (bool)$this->permissions[$permission];
                }
                $manager = PermissionManager::getInstance();
                return $manager->hasPermission($manager->getPermission($permission), $this->isOp());
            }
            if ($permission instanceof Permission) {
                return PermissionManager::getInstance()->hasPermission($permission, $this->isOp());
            }
            return false;
        }

        public function sendMessage(string $message): void
        {
            $this->addAction('send_message', ['target_player' => $this->name, 'message' => $message]);
        }

        public function sendTip(string $message): void
        {
            $this->addAction('send_tip', ['target_player' => $this->name, 'message' => $message]);
        }

        public function sendPopup(string $message): void
        {
            $this->addAction('send_popup', ['target_player' => $this->name, 'message' => $message]);
        }

        public function sendTitle(string $title, string $subtitle = '', int $fadeIn = 0, int $stay = 0, int $fadeOut = 0): void
        {
            $this->addAction('send_title', [
                'target_player' => $this->name,
                'title' => $title,
                'subtitle' => $subtitle,
                'fade_in' => $fadeIn,
                'stay' => $stay,
                'fade_out' => $fadeOut,
            ]);
        }

        public function sendActionBarMessage(string $message): void
        {
            $this->sendPopup($message);
        }

        public function teleport(mixed $position): bool
        {
            if ($position instanceof Vector3) {
                $this->x = $position->getX();
                $this->y = $position->getY();
                $this->z = $position->getZ();
                $world = method_exists($position, 'getWorldName') ? $position->getWorldName() : null;
                if (is_string($world) && $world !== '') {
                    $this->worldName = $world;
                }
                if (property_exists($position, 'yaw')) {
                    $this->yaw = (float)$position->yaw;
                }
                if (property_exists($position, 'pitch')) {
                    $this->pitch = (float)$position->pitch;
                }
            }
            $action = [
                'target_player' => $this->name,
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
                'world' => $this->worldName,
            ];
            // Yaw/pitch Bedrock-canónico (los que la copia API guarda):
            // sin ellos el teleport de un plugin pierde la rotación en
            // Rust y el Java ve el giro viejo tras el salto (ver
            // `syncTeleport` + `tp_sync_test.php`).
            if ($position instanceof Vector3 && (property_exists($position, 'yaw') || property_exists($position, 'pitch'))) {
                $action['yaw'] = $this->yaw;
                $action['pitch'] = $this->pitch;
            }
            $this->addAction('teleport', $action);
            return true;
        }

        public function kick(string $reason = '', mixed $quitMessage = null): void
        {
            $this->online = false;
            $this->addAction('kick_player', ['target_player' => $this->name, 'reason' => $reason]);
        }

        public function getServer(): mixed
        {
            return \pocketmine\Server::getInstance();
        }

        public function getUniqueId(): int|string
        {
            return $this->uniqueId;
        }

        public function getXuid(): string
        {
            return (string)$this->uniqueId;
        }

        public function getRuntimeId(): int
        {
            return (int)$this->runtimeId;
        }

        /** @return array<int, string> */
        public function getUsedChunks(): array
        {
            return [];
        }

        public function toArray(): array
        {
            return [
                'name' => $this->name,
                'display_name' => $this->displayName,
                'address' => $this->address,
                'port' => $this->port,
                'runtime_id' => $this->runtimeId,
                'unique_id' => $this->uniqueId,
                'ping' => $this->ping,
                'gamemode' => $this->gamemode,
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
                'yaw' => $this->yaw,
                'pitch' => $this->pitch,
                'world' => $this->worldName,
                'health' => $this->health,
                'max_health' => $this->maxHealth,
                'flying' => $this->flying,
                'online' => $this->online,
                'is_op' => $this->isOp(),
            ];
        }

        private function addAction(string $action, array $data = []): void
        {
            try {
                \pocketmine\Server::getInstance()->addAction($action, $data);
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/PLAYER] Unable to queue ' . $action . ': ' . $e->getMessage() . PHP_EOL);
            }
        }

        public function __toString(): string
        {
            return $this->name;
        }
    }

    class OfflinePlayer
    {
        public function __construct(private string $name, private ?array $data = null)
        {
        }

        public function getName(): string
        {
            return $this->name;
        }

        public function getXuid(): string
        {
            return (string)($this->data['unique_id'] ?? $this->name);
        }

        public function isOnline(): bool
        {
            return \pocketmine\Server::getInstance()->getPlayerExact($this->name) !== null;
        }

        public function isOp(): bool
        {
            return \pocketmine\Server::getInstance()->isOp($this->name);
        }

        public function hasPermission($permission): bool
        {
            return $this->isOp();
        }

        public function getFirstPlayed(): int
        {
            return 0;
        }

        public function getLastPlayed(): int
        {
            return 0;
        }

        public function hasPlayedBefore(): bool
        {
            return true;
        }

        public function __toString(): string
        {
            return $this->name;
        }
    }
}

namespace pocketmine\inventory {

    use pocketmine\item\Item;
    use pocketmine\item\ItemFactory;

    class PlayerInventory
    {
        /** @var array<int, Item> */
        private array $items = [];

        private int $hotbarSize = 9;
        private int $size = 36;

        public function __construct(private string $ownerName = '')
        {
        }

        public function getItem(int $index): Item
        {
            return $this->items[$index] ?? ItemFactory::get(0, 0, 0);
        }

        public function setItem(int $index, Item $item): void
        {
            if ($item->isNull()) {
                unset($this->items[$index]);
                return;
            }
            $this->items[$index] = $item;
        }

        public function addItem(Item ...$items): array
        {
            $leftover = [];
            foreach ($items as $item) {
                if ($item->isNull()) {
                    continue;
                }
                $placed = false;
                foreach ($this->items as $index => $existing) {
                    if ($existing->getId() === $item->getId() && $existing->getMeta() === $item->getMeta()) {
                        $existing->setCount($existing->getCount() + $item->getCount());
                        $placed = true;
                        break;
                    }
                }
                if (!$placed) {
                    for ($i = 0; $i < $this->size; $i++) {
                        if (!isset($this->items[$i])) {
                            $this->items[$i] = $item;
                            $placed = true;
                            break;
                        }
                    }
                }
                if (!$placed) {
                    $leftover[] = $item;
                }
            }
            return $leftover;
        }

        public function removeItem(Item ...$items): array
        {
            $leftover = [];
            foreach ($items as $item) {
                $remaining = $item->getCount();
                foreach ($this->items as $index => $existing) {
                    if ($existing->getId() !== $item->getId() || $existing->getMeta() !== $item->getMeta()) {
                        continue;
                    }
                    $take = min($remaining, $existing->getCount());
                    $existing->setCount($existing->getCount() - $take);
                    $remaining -= $take;
                    if ($existing->getCount() <= 0) {
                        unset($this->items[$index]);
                    }
                    if ($remaining <= 0) {
                        break;
                    }
                }
                if ($remaining > 0) {
                    $leftover[] = ItemFactory::get($item->getId(), $item->getMeta(), $remaining);
                }
            }
            return $leftover;
        }

        public function first(Item $item, bool $exact = false): int
        {
            foreach ($this->items as $index => $existing) {
                if ($existing->getId() === $item->getId()
                    && (!$exact || $existing->getMeta() === $item->getMeta())) {
                    return $index;
                }
            }
            return -1;
        }

        public function firstEmpty(): int
        {
            for ($i = 0; $i < $this->size; $i++) {
                if (!isset($this->items[$i])) {
                    return $i;
                }
            }
            return -1;
        }

        /** @return array<int, Item> */
        public function getContents(bool $includeEmpty = false): array
        {
            if ($includeEmpty) {
                $contents = [];
                for ($i = 0; $i < $this->size; $i++) {
                    $contents[$i] = $this->getItem($i);
                }
                return $contents;
            }
            return $this->items;
        }

        /** @param array<int, Item> $items */
        public function setContents(array $items): void
        {
            $this->items = [];
            foreach ($items as $index => $item) {
                if ($item instanceof Item && !$item->isNull()) {
                    $this->items[$index] = $item;
                }
            }
        }

        public function clear(int $index): void
        {
            unset($this->items[$index]);
        }

        public function clearAll(): void
        {
            $this->items = [];
        }

        public function getSize(): int
        {
            return $this->size;
        }

        public function getHotbarSize(): int
        {
            return $this->hotbarSize;
        }

        public function getItemInHand(): Item
        {
            return $this->getItem(0);
        }

        public function setItemInHand(Item $item): void
        {
            $this->setItem(0, $item);
        }
    }
}
