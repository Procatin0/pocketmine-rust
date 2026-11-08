<?php

declare(strict_types=1);

namespace pocketmine\world {

    use pocketmine\math\Position;
    use pocketmine\math\Vector3;
    use pocketmine\runtime\Bridge;

    /**
     * Maps the canonical Bedrock network ids used by the Rust core to the
     * legacy numeric ids and names that PocketMine plugins use.
     *
     * FNV coverage (Fase A3):
     * - `legacyToNetwork` only contains hashes verified against
     *   `crates/bedrock-world/src/block_id.rs` (canonical BDS palette hashes,
     *   protocol 2225) plus the same values reused by
     *   `plugins/JavaBridge/.../ChunkBuilder.php`/`ModernChunk.php`
     *   (`sandstone`/`cobblestone`/`short_grass`).
     * - The Rust generator only emits the `block_id.rs` set (stone, dirt,
     *   grass_block, bedrock, sand, sandstone, gravel, water, ores, logs,
     *   leaves, foliage). For every other name (planks, glass, brick, wool,
     *   wool colours, stairs, slabs, ...) `networkIdOf()` falls back to the
     *   legacy id, exactly like the previous bridge: the block is recorded
     *   in `world.json` as an edit but the Bedrock client may render it
     *   incorrectly until a BDS palette dump provides the canonical hash.
     *   No FNV hashes are invented here on purpose.
     * - 110+ names are registered in `nameToId` with their Java-legacy
     *   numeric id so `idFromName()`/`nameOf()`/`getDrops()` work for
     *   plugins; collisiony wood variants (spruce/birch/jungle log) share
     *   legacy 17/18 as in PM's legacy layout (type by meta, not id).
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
            'oak_planks' => 5,
            'planks' => 5,
            'spruce_planks' => 5,
            'birch_planks' => 5,
            'sapling' => 6,
            'oak_sapling' => 6,
            'bedrock' => 7,
            'flowing_water' => 8,
            'water' => 9,
            'flowing_lava' => 10,
            'lava' => 11,
            'sand' => 12,
            'gravel' => 13,
            'gold_ore' => 14,
            'iron_ore' => 15,
            'coal_ore' => 16,
            'oak_log' => 17,
            'log' => 17,
            'spruce_log' => 17,
            'birch_log' => 17,
            'jungle_log' => 17,
            'oak_leaves' => 18,
            'leaves' => 18,
            'spruce_leaves' => 18,
            'birch_leaves' => 18,
            'jungle_leaves' => 18,
            'sponge' => 19,
            'glass' => 20,
            'lapis_ore' => 21,
            'lapis_block' => 22,
            'dispenser' => 23,
            'sandstone' => 24,
            'chiseled_sandstone' => 24,
            'smooth_sandstone' => 24,
            'noteblock' => 25,
            'bed' => 26,
            'sticky_piston' => 29,
            'piston' => 34,
            'piston_head' => 34,
            'white_wool' => 35,
            'wool' => 35,
            'orange_wool' => 35,
            'magenta_wool' => 35,
            'light_blue_wool' => 35,
            'yellow_wool' => 35,
            'lime_wool' => 35,
            'pink_wool' => 35,
            'dandelion' => 37,
            'poppy' => 38,
            'brown_mushroom' => 39,
            'red_mushroom' => 40,
            'gold_block' => 41,
            'iron_block' => 42,
            'double_slab' => 43,
            'stone_slab' => 44,
            'tall_grass' => 31,
            'short_grass' => 31,
            'dead_bush' => 32,
            'brick_block' => 45,
            'bricks' => 45,
            'tnt' => 46,
            'bookshelf' => 47,
            'mossy_cobblestone' => 48,
            'obsidian' => 49,
            'torch' => 50,
            'fire' => 51,
            'mob_spawner' => 52,
            'oak_stairs' => 53,
            'chest' => 54,
            'redstone_wire' => 55,
            'diamond_ore' => 56,
            'diamond_block' => 57,
            'crafting_table' => 58,
            'wheat' => 59,
            'farmland' => 60,
            'furnace' => 61,
            'standing_sign' => 63,
            'oak_door' => 64,
            'ladder' => 65,
            'rail' => 66,
            'cobblestone_stairs' => 67,
            'wall_sign' => 68,
            'lever' => 69,
            'pressure_plate' => 70,
            'iron_door' => 71,
            'stone_pressure_plate' => 72,
            'redstone_ore' => 73,
            'redstone_torch' => 76,
            'stone_button' => 77,
            'snow_layer' => 78,
            'snow_block' => 80,
            'cactus' => 81,
            'clay' => 82,
            'reeds' => 83,
            'jukebox' => 84,
            'fence' => 85,
            'pumpkin' => 86,
            'netherrack' => 87,
            'soul_sand' => 88,
            'glowstone' => 89,
            'portal' => 90,
            'lit_pumpkin' => 91,
            'cake' => 92,
            'unpowered_repeater' => 93,
            'powered_repeater' => 94,
            'white_stained_glass' => 95,
            'trapdoor' => 96,
            'stonebrick' => 98,
            'brown_mushroom_block' => 99,
            'red_mushroom_block' => 100,
            'iron_bars' => 101,
            'glass_pane' => 102,
            'melon_block' => 103,
            'pumpkin_stem' => 104,
            'melon_stem' => 105,
            'vine' => 106,
            'fence_gate' => 107,
            'brick_stairs' => 108,
            'stone_brick_stairs' => 109,
            'mycelium' => 110,
            'waterlily' => 111,
            'nether_brick' => 112,
            'nether_brick_fence' => 113,
            'nether_brick_stairs' => 114,
            'nether_wart' => 115,
            'enchanting_table' => 116,
            'brewing_stand' => 117,
            'lantern' => 118,
            'end_stone' => 121,
            'dragon_egg' => 122,
            'redstone_lamp' => 123,
            'oak_slab' => 126,
            'chest_double' => 54,
            'sandstone_stairs' => 128,
            'emerald_ore' => 129,
            'ender_chest' => 130,
            'tripwire_hook' => 131,
            'tripwire' => 132,
            'emerald_block' => 133,
            'spruce_stairs' => 134,
            'birch_stairs' => 135,
            'command_block' => 137,
            'beacon' => 138,
            'cobblestone_wall' => 139,
            'flower_pot' => 140,
            'carrots' => 141,
            'potatoes' => 142,
            'oak_fence' => 143,
            'skull' => 144,
            'anvil' => 145,
            'trapped_chest' => 146,
            'light_weighted_pressure_plate' => 147,
            'dark_oak_fence' => 163,
            'dark_oak_stairs' => 164,
            'dark_oak_slab' => 181,
            'white_carpet' => 171,
            'hay_block' => 170,
            'hardened_clay' => 159,
            'coal_block' => 173,
            'podzol' => 3,
            'coarse_dirt' => 3,
            'farmland_moisture' => 60,
            'grass_path' => 2,
            'terracotta' => 159,
            'smooth_stone' => 1,
            'granite' => 1,
            'polished_granite' => 1,
            'diorite' => 1,
            'polished_diorite' => 1,
            'andesite' => 1,
            'polished_andesite' => 1,
            'purpur_block' => 201,
            'purpur_pillar' => 202,
            'purpur_slab' => 203,
            'packed_ice' => 174,
            'ice' => 79,
            'end_bricks' => 215,
            'red_sandstone' => 179,
            'smooth_red_sandstone' => 180,
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

        /**
         * Variants sharing a legacy id (log by meta) with their own
         * verified network hash from `block_id.rs`.
         *
         * @var array<string, int> name => network id
         */
        private static array $nameToNetwork = [
            'spruce_log' => 507450469,      // SPRUCE_LOG
            'spruce_leaves' => 1404704691,  // SPRUCE_LEAVES
            'birch_log' => 1423805714,      // BIRCH_LOG
            'birch_leaves' => 1740500456,   // BIRCH_LEAVES
            'short_grass' => 2827903781,    // SHORT_GRASS (JavaBridge ChunkBuilder)
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

        /** Network id por nombre exacto (variantes con hash propio). */
        public static function networkIdOfName(string $name): int
        {
            $key = strtolower(str_replace(['minecraft:', ' '], ['', '_'], $name));
            if (isset(self::$nameToNetwork[$key])) {
                return self::$nameToNetwork[$key];
            }
            return self::networkIdOf(self::idFromName($key));
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
            foreach (self::$nameToNetwork as $name => $network) {
                // spruce_log/birch/short_grass comparten legacy con oak,
                // pero el inverso debe reconocer su hash como legacy 17/18/31.
                self::$networkToLegacy[$network] = self::idFromName($name);
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
            $this->networkId = BlockFactory::networkIdOfName($name);
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
            // Divergencia UpdateBlockPacket (Fase A3): el host aplica la
            // edición en `WorldState` (persistida en `world.json`) pero la
            // difusión a clientes Bedrock vive en `session.rs` (NO de esta
            // fase). Hoy los clientes reciben el bloque al pedir chunks
            // (`LevelChunk`), no un `UpdateBlockPacket` unitario. Cambiarlo
            // exige tocar `session.rs`/`PluginAction::SetBlock` (otra fase);
            // por eso aquí se documenta y no se reenvía el chunk entero.
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

    /**
     * Columna Bedrock decodificada (formato de red de Bedrock).
     *
     * El payload es el que devuelve `world.get_chunk` (hex decodificado):
     * `subchunks` subsecciones (versión 8/9, paletas en zigzag, palabras en
     * little-endian), 24 paletas de bioma y, opcionalmente, byte de borde.
     * Al estilo de `pocketmine\world\format\Chunk` de axolotl-pm: los ids
     * son los de red (hash FNV-1a de `bedrock-world/src/block_id.rs`),
     * contados como enteros sin signo (p. ej. `AIR = 3690217760`).
     */
    final class Chunk
    {
        public const AIR = 3690217760;
        public const MIN_Y = -64;
        public const MAX_Y = 319;

        /** @var array<int, array{bits:int, words:array<int,int>, palette:array<int,int>}|null> */
        private array $sections = [];

        /** @var array<int, int> 24 biomas (una paleta por subsección). */
        private array $biomes = [];

        private int $subchunks = 24;

        public function __construct(private string $payload = '', int $subchunks = 24)
        {
            $this->subchunks = max(0, $subchunks);
            $this->parse();
        }

        /** Cantidad de palabras de 32 bits para `$bits` bits por bloque. */
        private static function wordsCount(int $bits): int
        {
            if ($bits === 0) {
                return 0;
            }
            $perWord = max(1, intdiv(32, $bits));
            return intdiv(4096 + $perWord - 1, $perWord);
        }

        private function readByte(int &$offset): ?int
        {
            if ($offset < 0 || $offset >= strlen($this->payload)) {
                return null;
            }
            return ord($this->payload[$offset++]);
        }

        /**
         * VarInt de 32 bits en zigzag (como `Writer::write_var_i32` de Rust):
         * LEB128 sin signo + zigzag. Devuelve el entero con signo.
         */
        private function readVarIntSigned(int &$offset): ?int
        {
            $result = 0;
            $shift = 0;
            $length = strlen($this->payload);
            for ($i = 0; $i < 5; $i++) {
                if ($offset >= $length) {
                    return null;
                }
                $byte = ord($this->payload[$offset++]);
                $result |= ($byte & 0x7F) << $shift;
                if (($byte & 0x80) === 0) {
                    return ($result >> 1) ^ -($result & 1);
                }
                $shift += 7;
            }
            return null;
        }

        /** Id de red sin signo a partir del entero con signo del payload. */
        private static function toUnsigned(int $signed): int
        {
            return $signed < 0 ? $signed + 0x100000000 : $signed;
        }

        private function parse(): void
        {
            $offset = 0;
            $length = strlen($this->payload);

            for ($index = 0; $index < $this->subchunks; $index++) {
                $version = $this->readByte($offset);
                $layers = $this->readByte($offset);
                if ($version === null || $layers === null) {
                    $this->sections[$index] = null;
                    continue;
                }
                if ($version !== 8 && $version !== 9) {
                    $this->sections[$index] = null;
                    continue;
                }
                if ($layers === 0) {
                    $this->sections[$index] = null;
                    continue;
                }

                // Solo se guarda la primera capa (la de estado); la segunda
                // (agua) no la usa el generador.
                $section = null;
                for ($layer = 0; $layer < $layers; $layer++) {
                    $header = $this->readByte($offset);
                    if ($header === null) {
                        break;
                    }
                    $bits = $header >> 1;
                    $wordsCount = self::wordsCount($bits);
                    $words = [];
                    $readOk = true;
                    for ($word = 0; $word < $wordsCount; $word++) {
                        if ($offset + 4 > $length) {
                            $readOk = false;
                            break;
                        }
                        $words[] = unpack('V', substr($this->payload, $offset, 4))[1];
                        $offset += 4;
                    }
                    if (!$readOk) {
                        break;
                    }

                    if ($bits === 0) {
                        $paletteSize = 1;
                    } else {
                        $paletteSize = $this->readVarIntSigned($offset);
                        if ($paletteSize === null || $paletteSize <= 0 || $paletteSize > 4096) {
                            break;
                        }
                    }

                    $palette = [];
                    $paletteOk = true;
                    for ($entry = 0; $entry < $paletteSize; $entry++) {
                        $signed = $this->readVarIntSigned($offset);
                        if ($signed === null) {
                            $paletteOk = false;
                            break;
                        }
                        $palette[] = self::toUnsigned($signed);
                    }
                    if (!$paletteOk) {
                        break;
                    }

                    if ($layer === 0) {
                        $section = ['bits' => $bits, 'words' => $words, 'palette' => $palette];
                    }
                }
                $this->sections[$index] = $section;
            }

            // 24 paletas de bioma (byte + varint cada una).
            for ($index = 0; $index < 24; $index++) {
                $header = $this->readByte($offset);
                if ($header === null) {
                    $this->biomes[$index] = 0;
                    continue;
                }
                $biome = $this->readVarIntSigned($offset);
                $this->biomes[$index] = $biome ?? 0;
            }
        }

        private function sectionIndexForY(int $y): ?int
        {
            $index = intdiv($y - self::MIN_Y, 16);
            if ($index < 0 || $index >= $this->subchunks || $index >= 24) {
                return null;
            }
            return $index;
        }

        /**
         * Id de estado de red en coordenadas del chunk: `x`/`z` locales
         * (0..15) e `y` absoluto (`-64..319`).
         */
        public function getBlockStateId(int $x, int $y, int $z): int
        {
            $index = $this->sectionIndexForY($y);
            if ($x < 0 || $x > 15 || $z < 0 || $z > 15 || $index === null) {
                return self::AIR;
            }
            $section = $this->sections[$index] ?? null;
            if ($section === null) {
                return self::AIR;
            }
            $bits = (int)$section['bits'];
            if ($bits === 0) {
                return (int)($section['palette'][0] ?? self::AIR);
            }
            $localY = $y - (self::MIN_Y + ($index << 4));
            $entry = ($x << 8) | ($z << 4) | $localY;
            $words = $section['words'];
            $perWord = max(1, intdiv(32, $bits));
            $word = $words[intdiv($entry, $perWord)] ?? 0;
            $slot = ($word >> (($entry % $perWord) * $bits)) & ((1 << $bits) - 1);
            return (int)($section['palette'][$slot] ?? self::AIR);
        }

        /** Mayor `y` con bloque no aire de la columna local (`x`, `z`). */
        public function getHighestBlockAt(int $x, int $z): int
        {
            if ($x < 0 || $x > 15 || $z < 0 || $z > 15) {
                return self::MIN_Y;
            }
            for ($index = $this->subchunks - 1; $index >= 0; $index--) {
                $section = $this->sections[$index] ?? null;
                if ($section === null) {
                    continue;
                }
                $baseY = self::MIN_Y + ($index << 4);
                // Sección de un solo aire: nada que buscar.
                if ((int)$section['bits'] === 0 && (int)($section['palette'][0] ?? self::AIR) === self::AIR) {
                    continue;
                }
                $palette = $section['palette'];
                $allAir = true;
                foreach ($palette as $id) {
                    if ((int)$id !== self::AIR && (int)$id !== 0) {
                        $allAir = false;
                        break;
                    }
                }
                if ($allAir) {
                    continue;
                }
                for ($localY = 15; $localY >= 0; $localY--) {
                    $y = $baseY + $localY;
                    $id = $this->getBlockStateId($x, $y, $z);
                    if ($id !== self::AIR && $id !== 0) {
                        return $y;
                    }
                }
            }
            return self::MIN_Y;
        }

        /**
         * Bioma de la subsección que contiene `y` (las paletas de bioma son
         * una por subsección; `x`/`z` no seleccionan dentro de la paleta).
         */
        public function getBiomeId(int $x, int $y, int $z): int
        {
            $index = intdiv($y - self::MIN_Y, 16);
            if ($index < 0) {
                $index = 0;
            } elseif ($index > 23) {
                $index = 23;
            }
            return (int)($this->biomes[$index] ?? 0);
        }

        /**
         * Payload de la sección con base `baseY` (p. ej. `-48`): `bits`,
         * `words` y `palette`, o `null` si la sección es aire / no existe.
         *
         * @return array{bits:int, words:array<int,int>, palette:array<int,int>}|null
         */
        public function getSectionPayload(int $baseY): ?array
        {
            $index = intdiv($baseY - self::MIN_Y, 16);
            if ($index < 0 || $index >= $this->subchunks) {
                return null;
            }
            $section = $this->sections[$index] ?? null;
            if ($section === null) {
                return null;
            }
            return [
                'bits' => (int)$section['bits'],
                'words' => array_values($section['words']),
                'palette' => array_values($section['palette']),
            ];
        }
    }

    class World
    {
        /** @var array<string, \pocketmine\player\Player> */
        private array $players = [];

        private Position $spawn;
        private int $difficulty = 2;
        private int $seed = 12345678;
        private int $tickCount = 0;
        private bool $timeRunning = true;

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

        /**
         * PM `World::getTileAt()`: tiles (containers) are not simulated here.
         *
         * Stub: siempre `null`. Hay que implementar tiles cuando el core
         * exponga persistencia NBT por bloque (divergencia documentada).
         */
        public function getTileAt(int $x, int $y, int $z): ?object
        {
            return null;
        }

        /**
         * PM `World::getEntityAt()`: no hay entidades persistentes por bloque.
         *
         * Stub: siempre `null` (jugadores viven en el registry, no en el mundo).
         */
        public function getEntityAt(int $x, int $y, int $z): ?object
        {
            return null;
        }

        /**
         * PM `World::dropItem()`: no hay entidades de item en el core.
         *
         * Stub: retorna `null` (documentado; no se crea `ItemEntity` fantasma).
         */
        public function dropItem(mixed $position, mixed $item, mixed $motion = null, int $delay = 10): ?object
        {
            return null;
        }

        /**
         * PM `World::addParticle()`: stub. Sin partículas de red en el puente;
         * se acepta la llamada sin hacer nada (documentado).
         */
        public function addParticle(mixed $position, mixed $particle = null, ?array $players = null): void
        {
        }

        /**
         * PM `World::addSound()`: stub. Sin sonidos de red en el puente;
         * se acepta la llamada sin hacer nada (documentado).
         */
        public function addSound(mixed $position, mixed $sound = null, ?array $players = null): void
        {
        }

        /**
         * PM `World::doTick()` / tick del mundo: avanza el contador local y
         * mantiene el tiempo vía host (`world.tick` si existe, si no no-op).
         * No rompe `getTime`/`setTime` (ya implementados vía host).
         */
        public function doTick(int $currentTick): void
        {
            $this->tickCount = $currentTick;
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('world.tick', ['world' => $this->name, 'current_tick' => $currentTick]);
            }
        }

        /** Alias PM `World::tick()` (mismo contador que `doTick`). */
        public function tick(int $currentTick = 0): void
        {
            $this->doTick($currentTick);
        }

        public function getTick(): int
        {
            return $this->tickCount;
        }

        /**
         * PM `World::doChunkGarbageCollection()`: stub.
         *
         * El core genera chunks bajo demanda desde el generador + ediciones;
         * no hay caché de chunks descargables que recoger. No-op documentado.
         */
        public function doChunkGarbageCollection(): void
        {
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

        /**
         * Columna Bedrock del chunk (`world.get_chunk` en hex). Tras la
         * Fase A3 esta clase había perdido `getChunk`/`Chunk`, y el puente
         * Java nunca enviaba chunks (cliente en "cargando terreno").
         */
        public function getChunk(int $chunkX, int $chunkZ): ?Chunk
        {
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                return null;
            }
            $result = \pocketmine\runtime\Bridge::hostCall('world.get_chunk', [
                'world' => $this->name,
                'chunk_x' => $chunkX,
                'chunk_z' => $chunkZ,
            ]);
            if (!is_array($result)) {
                return null;
            }
            $hex = $result['data'] ?? $result['payload'] ?? null;
            if (!is_string($hex) || $hex === '') {
                return null;
            }
            $payload = hex2bin($hex);
            if ($payload === false) {
                // Tolerancia: si el host devuelve binario directo, usarlo tal cual.
                $payload = $hex;
            }
            $subchunks = (int)($result['subchunks'] ?? 24);
            if ($subchunks <= 0) {
                $subchunks = 24;
            }
            return new Chunk($payload, $subchunks);
        }

        /**
         * Punto de aparición del jugador (`player_position()` del generador:
         * centro del bloque y altura de los ojos), como `[x, y, z]`.
         */
        public function getPlayerSpawnLocation(): ?array
        {
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                return null;
            }
            $result = \pocketmine\runtime\Bridge::hostCall('world.get_spawn', ['world' => $this->name]);
            if (!is_array($result)) {
                return null;
            }
            if (isset($result['player']) && is_array($result['player']) && count($result['player']) === 3) {
                $player = array_values($result['player']);
                return [(float)$player[0], (float)$player[1], (float)$player[2]];
            }
            if (isset($result['x'], $result['y'], $result['z'])) {
                return [
                    (float)$result['x'] + 0.5,
                    (float)$result['y'] + 1.62,
                    (float)$result['z'] + 0.5,
                ];
            }
            return null;
        }

        public function getTime(): int
        {
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                $result = \pocketmine\runtime\Bridge::hostCall('world.get_time', ['world' => $this->name]);
                if (is_array($result) && isset($result['time'])) {
                    $this->time = (int)$result['time'];
                }
                if (is_array($result) && isset($result['running'])) {
                    $this->timeRunning = (bool)$result['running'];
                }
            }
            return $this->time;
        }

        /**
         * PM `World::isTimeRunning()` (`/time start`/`stop`): whether
         * the clock advances each tick. The Java bridge sends it as
         * `tickDayTime`/`rate` in `update_time` so `/time stop`
         * freezes the sun on Java clients too.
         */
        public function isTimeRunning(): bool
        {
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                $result = \pocketmine\runtime\Bridge::hostCall('world.get_time_running', ['world' => $this->name]);
                if (is_array($result) && isset($result['running'])) {
                    $this->timeRunning = (bool)$result['running'];
                }
            }
            return $this->timeRunning;
        }

        public function setTime(int $time): void
        {
            $this->time = $time % 24000;
            if ($this->time < 0) {
                $this->time += 24000;
            }
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('world.set_time', ['world' => $this->name, 'time' => $this->time]);
            }
        }

        public function startTime(): void
        {
            $this->timeRunning = true;
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('world.set_time_running', ['world' => $this->name, 'running' => true]);
            }
        }

        public function stopTime(): void
        {
            $this->timeRunning = false;
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('world.set_time_running', ['world' => $this->name, 'running' => false]);
            }
        }

        public function getDifficulty(): int
        {
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                $result = \pocketmine\runtime\Bridge::hostCall('world.get_difficulty', ['world' => $this->name]);
                if (is_array($result) && isset($result['difficulty'])) {
                    return (int)$result['difficulty'];
                }
                if (is_int($result)) {
                    return $result;
                }
            }
            // Fallback to the local mirror when the host has no such method yet.
            return $this->difficulty;
        }

        public function setDifficulty(int $difficulty): void
        {
            $clamped = $difficulty < 0 ? 0 : ($difficulty > 3 ? 3 : $difficulty);
            $this->difficulty = $clamped;
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('world.set_difficulty', ['world' => $this->name, 'difficulty' => $clamped]);
            }
        }

        public function getSeed(): int
        {
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                $result = \pocketmine\runtime\Bridge::hostCall('world.get_seed', ['world' => $this->name]);
                if (is_array($result) && isset($result['seed'])) {
                    return (int)$result['seed'];
                }
                if (is_int($result)) {
                    return $result;
                }
            }
            // Fallback to the local mirror (defaults to the generator seed).
            return $this->seed;
        }

        /**
         * PM `World::save()`: persists the world snapshot. Routes through
         * `world.save` when the host implements it, otherwise reports the
         * local write as done.
         */
        public function save(): bool
        {
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                $result = \pocketmine\runtime\Bridge::hostCall('world.save', ['world' => $this->name]);
                if (is_array($result) && isset($result['ok'])) {
                    return (bool)$result['ok'];
                }
                if (is_bool($result)) {
                    return $result;
                }
                // Host does not implement world.save yet: fall through to local.
            }
            return true;
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

        /**
         * Single-world divergence (documentada):
         * `WorldState` es único (ver `world_state.rs`); todos los nombres
         * comparten el mismo generador/ediciones. `getOrCreate()` NO se usa
         * en getters: `getWorldByName()` devuelve el mundo cargado o, en
         * modo single-world, el mundo por defecto (sin crear un `World`
         * fantasma nuevo). `isWorldLoaded()` dice la verdad.
         */
        public function getDefaultWorld(): World
        {
            return $this->getOrCreate($this->defaultWorldName);
        }

        public function getDefaultWorldName(): string
        {
            return $this->defaultWorldName;
        }

        /**
         * PM `WorldManager::getWorld(int $worldId)`: busca por
         * `World::getId()` (crc32 del nombre). Sin autocreación: `null`
         * si no existe. El string como parámetro crea TypeError en PM
         * upstream; aquí se acepta `int` para mantener paridad.
         */
        public function getWorld(int $worldId): ?World
        {
            foreach ($this->worlds as $world) {
                if ($world->getId() === $worldId) {
                    return $world;
                }
            }
            return null;
        }

        public function getWorldByName(string $name): ?World
        {
            $key = strtolower($name);
            if (isset($this->worlds[$key])) {
                return $this->worlds[$key];
            }
            // Single-world: nombre desconocido => mundo por defecto
            // (sin fantasma nuevo). Ver doc del bloque.
            return $this->getDefaultWorld();
        }

        /** @return array<string, World> */
        public function getWorlds(): array
        {
            return $this->worlds;
        }

        public function isWorldLoaded(string $name): bool
        {
            $key = strtolower($name);
            return isset($this->worlds[$key]) || $key === strtolower($this->defaultWorldName);
        }

        /** PM `WorldManager::isWorldGenerated()`: single-world, solo existe el default. */
        public function isWorldGenerated(string $name): bool
        {
            return $this->isWorldLoaded($name);
        }

        public function isWorldGenerating(string $name): bool
        {
            return false;
        }

        /**
         * PM `WorldManager::findEntity(int $entityId)`: stub.
         *
         * El core no tiene entidades persistentes propias (solo jugadores
         * en el registry); siempre `null` (documentado).
         */
        public function findEntity(int $entityId): ?object
        {
            return null;
        }

        /**
         * PM `WorldManager::tick(int $currentTick)`: tickea cada mundo
         * cargado y, cada `autoSaveInterval` ticks, dispara
         * `world.do_auto_save` vía host si está activo.
         */
        public function tick(int $currentTick = 0): void
        {
            foreach ($this->worlds as $world) {
                $world->doTick($currentTick);
            }
            if ($this->autoSave && ++$this->autoSaveTicker >= $this->autoSaveInterval) {
                $this->autoSaveTicker = 0;
                if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                    \pocketmine\runtime\Bridge::hostCall('world.do_auto_save', []);
                }
            }
        }

        /**
         * PM `WorldManager::generateWorld(string $name, WorldCreationOptions $options, bool $backgroundGeneration = true): bool`.
         *
         * $options acepta `WorldCreationOptions|array|null` por compatibilidad
         * con plugins que pasan un array de opciones. Single-world: si ya
         * está cargado devuelve `false` (como PM); si no, registra el nombre
         * (reusando el generador) y devuelve `true`.
         */
        public function generateWorld(string $name, mixed $options = null, bool $backgroundGeneration = true): bool
        {
            if (trim($name) === '' || $this->isWorldGenerated($name)) {
                return false;
            }
            $this->getOrCreate($name);
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('world.generate', ['world' => $name]);
            }
            return true;
        }

        public function getAutoSave(): bool
        {
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                $result = \pocketmine\runtime\Bridge::hostCall('world.get_auto_save', []);
                if (is_array($result) && isset($result['auto_save'])) {
                    return (bool)$result['auto_save'];
                }
                if (is_bool($result)) {
                    return $result;
                }
            }
            return $this->autoSave;
        }

        public function setAutoSave(bool $autoSave): void
        {
            $this->autoSave = $autoSave;
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('world.set_auto_save', ['auto_save' => $autoSave]);
            }
        }

        private bool $autoSave = true;
        /** PM `WorldManager::TICKS_PER_AUTOSAVE` (300s * 20 tps). */
        private int $autoSaveInterval = 6000;
        private int $autoSaveTicker = 0;

        /** PM `WorldManager::getAutoSaveInterval()`. */
        public function getAutoSaveInterval(): int
        {
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                $result = \pocketmine\runtime\Bridge::hostCall('world.get_auto_save_interval', []);
                if (is_array($result) && isset($result['interval'])) {
                    return (int)$result['interval'];
                }
                if (is_int($result)) {
                    return $result;
                }
            }
            return $this->autoSaveInterval;
        }

        /** PM `WorldManager::setAutoSaveInterval()` (lanza si <= 0). */
        public function setAutoSaveInterval(int $autoSaveTicks): void
        {
            if ($autoSaveTicks <= 0) {
                throw new \InvalidArgumentException('Autosave ticks must be positive');
            }
            $this->autoSaveInterval = $autoSaveTicks;
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('world.set_auto_save_interval', ['interval' => $autoSaveTicks]);
            }
        }

        /**
         * PM `WorldManager::loadWorld` overloads: accepts a `World` (legacy
         * bridge path) or a world name string (PM5 primary path).
         *
         * String path: devuelve `false` si el nombre está vacío o si la
         * carpeta no existe en modo multi-world; en single-world registra el
         * nombre (reusa el generador) y devuelve `true` (documentado en
         * `world_state.rs`).
         */
        public function loadWorld(World|string $world, bool $autoUpgrade = true): bool
        {
            if ($world instanceof World) {
                $this->worlds[strtolower($world->getName())] = $world;
                return true;
            }
            if (trim($world) === '') {
                throw new \InvalidArgumentException('Invalid empty world name');
            }
            if ($this->isWorldLoaded($world)) {
                return true;
            }
            $this->getOrCreate($world);
            return true;
        }

        public function unloadWorld(World $world, bool $forceUnload = false): bool
        {
            if ($world->getName() === $this->defaultWorldName && !$forceUnload) {
                throw new \InvalidArgumentException('The default world cannot be unloaded while running, please switch worlds.');
            }
            unset($this->worlds[strtolower($world->getName())]);
            return true;
        }

        public function setDefaultWorld(?World $world): bool
        {
            if ($world === null) {
                return false;
            }
            if ($this->isWorldLoaded($world->getFolderName()) && $world !== $this->getDefaultWorld()) {
                $this->defaultWorldName = $world->getName();
            }
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
