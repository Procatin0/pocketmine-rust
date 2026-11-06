<?php

declare(strict_types=1);

namespace pocketmine\world {

    use pocketmine\math\Position;
    use pocketmine\math\Vector3;
    use pocketmine\runtime\Bridge;

    /**
     * Maps the canonical Bedrock network ids used by the Rust core to the
     * legacy numeric ids and names that PocketMine plugins use.
     */
    final class BlockFactory
    {
        /** @var array<string, int> name => legacy id */
        private static array $nameToId = [
            'air' => 0,
            'stone' => 1,
            'grass_block' => 2,
            'grass' => 2,
            'dirt' => 3,
            'cobblestone' => 4,
            'planks' => 5,
            'oak_planks' => 5,
            'bedrock' => 7,
            'water' => 9,
            'sand' => 12,
            'gravel' => 13,
            'gold_ore' => 14,
            'iron_ore' => 15,
            'coal_ore' => 16,
            'oak_log' => 17,
            'log' => 17,
            'oak_leaves' => 18,
            'leaves' => 18,
            'glass' => 20,
            'lapis_ore' => 21,
            'sandstone' => 24,
            'spruce_log' => 17,
            'birch_log' => 17,
            'diamond_ore' => 56,
            'redstone_ore' => 73,
            'emerald_ore' => 129,
            'short_grass' => 31,
            'tall_grass' => 31,
            'snow_layer' => 78,
            'dandelion' => 37,
            'poppy' => 38,
        ];

        /** @var array<int, string> legacy id => name */
        private static array $idToName = [];

        /** @var array<int, int> legacy id => canonical network id (FNV-1a state hash) */
        private static array $legacyToNetwork = [
            0 => 3690217760,   // air
            1 => 2150698529,   // stone
            2 => 3727763636,   // grass_block
            3 => 2186211206,   // dirt
            4 => 3466092811,   // cobblestone (best effort)
            7 => 4121722107,   // bedrock
            9 => 1211861802,   // water
            12 => 138639715,   // sand
            13 => 1529044762,  // gravel
            16 => 685383673,   // coal_ore
            15 => 2032622302,  // iron_ore
            14 => 2144082742,  // gold_ore
            56 => 1419890941,  // diamond_ore
            73 => 180213920,   // redstone_ore
            129 => 170572189,  // emerald_ore
            17 => 825916963,   // oak_log
            18 => 2110714365,  // oak_leaves
            24 => 2755040846,  // sandstone (best effort)
            21 => 2930261704,  // lapis_ore
            31 => 4175527249,  // tall_grass
            78 => 3699616834,  // snow_layer
            37 => 2383143704,  // dandelion
            38 => 3417975282,  // poppy
        ];

        /** @var array<int, int> network id => legacy id */
        private static array $networkToLegacy = [];

        public static function legacyIdOf(float|int $networkId): int
        {
            $key = self::unsigned((int)$networkId);
            self::buildNetworkIndex();
            return self::$networkToLegacy[$key] ?? self::signedToLegacy($key);
        }

        public static function networkIdOf(int $legacyId): int
        {
            return self::$legacyToNetwork[$legacyId] ?? $legacyId;
        }

        private static function signedToLegacy(int $unsigned): int
        {
            // Unknown state: expose the state hash as a positive integer.
            return $unsigned > 0x7FFFFFFF ? $unsigned - 0x100000000 : $unsigned;
        }

        private static function unsigned(int $value): int
        {
            return $value < 0 ? $value + 0x100000000 : $value;
        }

        private static function buildNetworkIndex(): void
        {
            if (self::$networkToLegacy !== []) {
                return;
            }
            foreach (self::$legacyToNetwork as $legacy => $network) {
                self::$networkToLegacy[$network] = $legacy;
            }
            self::$networkToLegacy[3690217760] = 0;
        }

        public static function idFromName(string $name): int
        {
            $name = strtolower(str_replace(['minecraft:', ' '], ['', '_'], $name));
            return self::$nameToId[$name] ?? 0;
        }

        public static function nameOf(int $legacyId): string
        {
            if (self::$idToName === []) {
                foreach (self::$nameToId as $name => $id) {
                    if (!isset(self::$idToName[$id])) {
                        self::$idToName[$id] = $name;
                    }
                }
            }
            return self::$idToName[$legacyId] ?? ('unknown_' . $legacyId);
        }

        public static function networkNameOf(int $legacyId): string
        {
            return 'minecraft:' . self::nameOf($legacyId);
        }

        public static function friendlyNameOf(int $legacyId): string
        {
            return ucwords(str_replace('_', ' ', self::nameOf($legacyId)));
        }

        public static function get(string $name, Position $position): Block
        {
            $block = new Block($position->getWorldName(), $position->getFloorX(), $position->getFloorY(), $position->getFloorZ());
            $block->setTypeId($name);
            return $block;
        }

        public static function fromString(string $name, Position $position): Block
        {
            return self::get($name, $position);
        }

        public static function getBlockIdMap(): array
        {
            return self::$nameToId;
        }

        public static function register(string $name, int $legacyId): void
        {
            $name = strtolower($name);
            self::$nameToId[$name] = $legacyId;
            self::$idToName[$legacyId] ??= $name;
        }
    }

    class Block
    {
        private int $legacyId = 0;
        private int $networkId = 3690217760;
        private string $worldName = 'world';
        private int $x = 0;
        private int $y = 0;
        private int $z = 0;

        public function __construct(mixed $worldOrX = 'world', int $x = 0, int $y = 0, int $z = 0)
        {
            if (is_object($worldOrX) && method_exists($worldOrX, 'getName')) {
                $this->worldName = (string)$worldOrX->getName();
                $this->x = $x;
                $this->y = $y;
                $this->z = $z;
            } elseif (is_string($worldOrX)) {
                $this->worldName = $worldOrX;
                $this->x = $x;
                $this->y = $y;
                $this->z = $z;
            } else {
                // new Block($x, $y, $z)
                $this->x = (int)$worldOrX;
                $this->y = $x;
                $this->z = $y;
            }

            $this->refresh();
        }

        private function refresh(): void
        {
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                return;
            }
            $result = Bridge::hostCall('world.get_block', [
                'world' => $this->worldName,
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
            ]);
            if (is_array($result) && isset($result['id'])) {
                $this->networkId = (int)$result['id'];
                $this->legacyId = BlockFactory::legacyIdOf($this->networkId);
            }
        }

        public function getId(): int
        {
            return $this->legacyId;
        }

        public function getMeta(): int
        {
            return 0;
        }

        public function getTypeId(): string
        {
            return BlockFactory::networkNameOf($this->legacyId);
        }

        public function getName(): string
        {
            return BlockFactory::friendlyNameOf($this->legacyId);
        }

        public function getNetworkId(): int
        {
            return $this->networkId;
        }

        public function getPosition(): Position
        {
            return new Position($this->x, $this->y, $this->z, $this->worldName);
        }

        public function getX(): int
        {
            return $this->x;
        }

        public function getY(): int
        {
            return $this->y;
        }

        public function getZ(): int
        {
            return $this->z;
        }

        public function getWorld(): World
        {
            return \pocketmine\Server::getInstance()->getWorldManager()->getWorldByName($this->worldName);
        }

        public function getDrops(mixed $item = null): array
        {
            if ($this->legacyId === 0) {
                return [];
            }
            return [\pocketmine\item\ItemFactory::get($this->legacyId, 0, 1)];
        }

        public function setTypeId(string $name): bool
        {
            $legacy = BlockFactory::idFromName($name);
            $this->legacyId = $legacy;
            $this->networkId = BlockFactory::networkIdOf($legacy);
            return $this->write();
        }

        public function setType(string $name): bool
        {
            return $this->setTypeId($name);
        }

        public function setBlock(int $legacyId, int $meta = 0): bool
        {
            $this->legacyId = $legacyId;
            $this->networkId = BlockFactory::networkIdOf($legacyId);
            return $this->write();
        }

        private function write(): bool
        {
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                return false;
            }
            $result = Bridge::hostCall('world.set_block', [
                'world' => $this->worldName,
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
                'block_id' => $this->networkId,
            ]);
            return is_array($result) && ($result['ok'] ?? false) === true;
        }

        public function isSolid(): bool
        {
            return $this->legacyId !== 0;
        }

        public function __toString(): string
        {
            return 'Block(' . $this->getName() . ' @ ' . $this->x . ',' . $this->y . ',' . $this->z . ')';
        }
    }

    class World
    {
        /** @var array<string, \pocketmine\player\Player> */
        private array $players = [];

        private Position $spawn;

        public function __construct(
            private string $name = 'world',
            ?Position $spawn = null,
            private int $time = 6000
        ) {
            $this->spawn = $spawn ?? new Position(0.0, 64.0, 0.0, $this->name);
        }

        public function getName(): string
        {
            return $this->name;
        }

        public function getFolderName(): string
        {
            return $this->name;
        }

        public function getDisplayName(): string
        {
            return $this->name;
        }

        public function setDisplayName(string $name): void
        {
            $this->name = $name;
        }

        public function isLoaded(): bool
        {
            return true;
        }

        public function getId(): int
        {
            return crc32($this->name);
        }

        public function getSpawnLocation(): Position
        {
            return $this->spawn;
        }

        public function getSafeSpawn(?Vector3 $spawn = null): Position
        {
            if ($spawn === null) {
                return $this->spawn;
            }
            return new Position($spawn->getX(), $spawn->getY(), $spawn->getZ(), $this->name);
        }

        public function setSpawnLocation(Vector3 $position): void
        {
            $this->spawn = new Position($position->getX(), $position->getY(), $position->getZ(), $this->name);
        }

        /** @return array<string, \pocketmine\player\Player> */
        public function getPlayers(): array
        {
            return $this->players;
        }

        public function getPlayerCount(): int
        {
            return count($this->players);
        }

        public function addPlayer(\pocketmine\player\Player $player): void
        {
            $this->players[strtolower($player->getName())] = $player;
        }

        public function removePlayer(\pocketmine\player\Player $player): void
        {
            unset($this->players[strtolower($player->getName())]);
        }

        public function getBlock(mixed $position): Block
        {
            if ($position instanceof Vector3) {
                return new Block(
                    $this->name,
                    $position->getFloorX(),
                    $position->getFloorY(),
                    $position->getFloorZ()
                );
            }
            if (is_array($position)) {
                return new Block(
                    $this->name,
                    (int)($position['x'] ?? 0),
                    (int)($position['y'] ?? 0),
                    (int)($position['z'] ?? 0)
                );
            }
            return new Block($this->name, 0, 0, 0);
        }

        public function getBlockAt(int $x, int $y, int $z): Block
        {
            return new Block($this->name, $x, $y, $z);
        }

        public function setBlock(mixed $position, Block $block, bool $direct = false): bool
        {
            $x = $position instanceof Vector3 ? $position->getFloorX() : (int)($position['x'] ?? 0);
            $y = $position instanceof Vector3 ? $position->getFloorY() : (int)($position['y'] ?? 0);
            $z = $position instanceof Vector3 ? $position->getFloorZ() : (int)($position['z'] ?? 0);
            return (new Block($this->name, $x, $y, $z))->setBlock($block->getId());
        }

        public function setBlockAt(int $x, int $y, int $z, Block $block, bool $direct = false): bool
        {
            return (new Block($this->name, $x, $y, $z))->setBlock($block->getId());
        }

        public function getHighestBlockAt(mixed $x, ?int $z = null): int
        {
            $bx = $x instanceof Vector3 ? $x->getFloorX() : (int)$x;
            $bz = $x instanceof Vector3 ? $x->getFloorZ() : (int)($z ?? 0);
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                return 0;
            }
            $result = Bridge::hostCall('world.get_highest_block', [
                'world' => $this->name,
                'x' => $bx,
                'z' => $bz,
            ]);
            return is_array($result) && isset($result['y']) ? (int)$result['y'] : 0;
        }

        public function getTime(): int
        {
            return $this->time;
        }

        public function setTime(int $time): void
        {
            $this->time = $time;
        }

        public function getDifficulty(): int
        {
            return 1;
        }

        public function setDifficulty(int $difficulty): void
        {
        }

        public function getSeed(): int
        {
            return 12345678;
        }

        public function __toString(): string
        {
            return 'World(name=' . $this->name . ',players=' . count($this->players) . ')';
        }
    }

    class WorldManager
    {
        /** @var array<string, World> */
        private array $worlds = [];

        private string $defaultWorldName = 'world';

        public function __construct()
        {
            $this->getOrCreate($this->defaultWorldName);
        }

        private function getOrCreate(string $name): World
        {
            $key = strtolower($name);
            if (!isset($this->worlds[$key])) {
                $spawn = ['x' => 0.0, 'y' => 64.0, 'z' => 0.0];
                // Host calls are only safe while serving a request; during
                // bootstrap the bridge is not listening yet.
                if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                    $result = \pocketmine\runtime\Bridge::hostCall('world.get_spawn', ['world' => $name]);
                    if (is_array($result)) {
                        $spawn = [
                            'x' => (float)($result['x'] ?? 0.0),
                            'y' => (float)($result['y'] ?? 64.0),
                            'z' => (float)($result['z'] ?? 0.0),
                        ];
                    }
                }
                $this->worlds[$key] = new World(
                    $name,
                    new Position($spawn['x'], $spawn['y'], $spawn['z'], $name)
                );
            }
            return $this->worlds[$key];
        }

        public function getDefaultWorld(): World
        {
            return $this->getOrCreate($this->defaultWorldName);
        }

        public function getDefaultWorldName(): string
        {
            return $this->defaultWorldName;
        }

        public function getWorld(string $name): ?World
        {
            return $this->getOrCreate($name);
        }

        public function getWorldByName(string $name): ?World
        {
            return $this->getOrCreate($name);
        }

        /** @return array<string, World> */
        public function getWorlds(): array
        {
            return $this->worlds;
        }

        public function isWorldLoaded(string $name): bool
        {
            return true;
        }

        public function isWorldGenerating(string $name): bool
        {
            return false;
        }

        public function loadWorld(World $world, bool $autoUpgrade = true): bool
        {
            $this->worlds[strtolower($world->getName())] = $world;
            return true;
        }

        public function unloadWorld(World $world, bool $forceUnload = false): bool
        {
            unset($this->worlds[strtolower($world->getName())]);
            return true;
        }

        public function setDefaultWorld(?World $world): bool
        {
            if ($world === null) {
                return false;
            }
            $this->defaultWorldName = $world->getName();
            $this->worlds[strtolower($world->getName())] = $world;
            return true;
        }
    }
}

namespace pocketmine\math {

    /**
     * Legacy alias. PocketMine-MP 5 moved Position into `pocketmine\world`,
     * but older code and this bridge still refer to `pocketmine\math\Position`.
     */
    if (!class_exists(Position::class, false)) {
        class_alias(\pocketmine\world\Position::class, 'pocketmine\\math\\Position');
    }
}
