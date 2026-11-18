<?php
declare(strict_types=1);

// 1. Read BlockTypeIds constants from target/axolotl-pm/src/block/BlockTypeIds.php
$btiSource = file_get_contents(__DIR__ . '/../target/axolotl-pm/src/block/BlockTypeIds.php');
preg_match_all('/public const ([A-Z0-9_]+) = ([0-9]+);/', $btiSource, $constMatches, PREG_SET_ORDER);

echo "Loaded " . count($constMatches) . " constants for BlockTypeIds.\n";

// 2. Read VanillaBlocks methods from target/axolotl-pm/generated/block/VanillaBlocks.php
$vbSource = file_get_contents(__DIR__ . '/../target/axolotl-pm/generated/block/VanillaBlocks.php');
preg_match_all('/public static function ([A-Z0-9_]+)\(\)\s*:\s*([A-Za-z0-9_]+)/', $vbSource, $vbMatches, PREG_SET_ORDER);

echo "Loaded " . count($vbMatches) . " methods for VanillaBlocks.\n";

// Generate code for BlockTypeTags, BlockToolType, BlockTypeIds, and VanillaBlocks
$code = "<?php\n\ndeclare(strict_types=1);\n\nnamespace pocketmine\\block {\n\n";

// BlockTypeTags
$code .= <<<'TAGS'
    final class BlockTypeTags
    {
        private const PREFIX = "pocketmine:";

        public const DIRT = self::PREFIX . "dirt";
        public const MUD = self::PREFIX . "mud";
        public const SAND = self::PREFIX . "sand";
        public const POTTABLE_PLANTS = self::PREFIX . "pottable";
        public const FIRE = self::PREFIX . "fire";
        public const HANGING_SIGN = self::PREFIX . "hanging_sign";
        public const NYLIUM = self::PREFIX . "nylium";
        public const HUGE_FUNGUS_REPLACEABLE = self::PREFIX . "huge_fungus_replaceable";
        public const BAMBOO_MOSAIC = self::PREFIX . "bamboo_mosaic";

        private function __construct()
        {
        }
    }

TAGS;

// BlockToolType
$code .= <<<'TOOL'
    final class BlockToolType
    {
        public const NONE = 0;
        public const SWORD = 1 << 0;
        public const SHOVEL = 1 << 1;
        public const PICKAXE = 1 << 2;
        public const AXE = 1 << 3;
        public const SHEARS = 1 << 4;
        public const HOE = 1 << 5;

        private function __construct()
        {
        }
    }

TOOL;

// BlockTypeIds
$code .= "    final class BlockTypeIds\n    {\n        private function __construct()\n        {\n        }\n\n";

foreach ($constMatches as $m) {
    $code .= "        public const " . $m[1] . " = " . $m[2] . ";\n";
}

$code .= <<<'BTI_METHODS'

        private static int $nextDynamicId = self::FIRST_UNUSED_BLOCK_ID;
        /** @var array<string, int> */
        private static array $byName = [];

        public static function newId(): int
        {
            return self::$nextDynamicId++;
        }

        public static function forName(string $name): int
        {
            $key = strtolower(str_replace(['minecraft:', ' '], ['', '_'], $name));
            if (isset(self::$byName[$key])) {
                return self::$byName[$key];
            }
            $upper = strtoupper($key);
            if (defined(self::class . '::' . $upper)) {
                return self::$byName[$key] = constant(self::class . '::' . $upper);
            }
            // Common aliases
            $alias = match ($key) {
                'grass_block' => self::GRASS,
                'oak_leaves' => self::OAK_LEAVES,
                'flowing_water' => self::WATER,
                'flowing_lava' => self::LAVA,
                default => null,
            };
            if ($alias !== null) {
                return self::$byName[$key] = $alias;
            }
            return self::$byName[$key] = self::newId();
        }
    }

BTI_METHODS;

// VanillaBlocks
$code .= <<<'VB_HEAD'
    /**
     * Allows getting a new instance of any block implemented by PocketMine-MP.
     * Every block here also has a constant of the same name in BlockTypeIds.
     */
    final class VanillaBlocks
    {
        /** @var array<string, Block> */
        private static array $blocks = [];
        private static array $aliases = [
            'GRASS' => 'grass_block',
            'GRASS_BLOCK' => 'grass_block',
            'NOTEBLOCK' => 'noteblock',
            'GOLD_BLOCK' => 'gold_block',
            'DIAMOND_BLOCK' => 'diamond_block',
            'IRON_BLOCK' => 'iron_block',
            'NETHERITE_BLOCK' => 'netherite_block',
            'EMERALD_BLOCK' => 'emerald_block',
            'LAPIS_BLOCK' => 'lapis_block',
            'REDSTONE_BLOCK' => 'redstone_block',
            'COAL_BLOCK' => 'coal_block',
        ];

        private function __construct()
        {
        }

        public static function __callStatic(string $name, array $args): Block
        {
            $key = strtoupper($name);
            if (!isset(self::$blocks[$key])) {
                $blockName = self::$aliases[$key] ?? ('minecraft:' . strtolower($key));
                self::$blocks[$key] = Block::fromName($blockName);
            }
            return clone self::$blocks[$key];
        }

        public static function getAll(): array
        {
            // Returns prototype clones of all registered vanilla blocks
            $all = [];
            $reflect = new \ReflectionClass(BlockTypeIds::class);
            foreach ($reflect->getConstants() as $cName => $cVal) {
                if ($cName === 'FIRST_UNUSED_BLOCK_ID') continue;
                $all[$cName] = self::__callStatic($cName, []);
            }
            return $all;
        }
VB_HEAD;

// Add static getter methods for the 830 vanilla blocks for direct static calls (VanillaBlocks::AIR(), etc.)
foreach ($vbMatches as $vm) {
    $mName = $vm[1];
    $mRet = $vm[2];
    $code .= "        public static function " . $mName . "(): Block { return self::__callStatic('" . $mName . "', []); }\n";
}

$code .= "    }\n}\n";

file_put_contents(__DIR__ . '/generated_core_registry.php', $code);
echo "Written generated_core_registry.php (" . strlen($code) . " bytes).\n";
