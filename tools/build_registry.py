import re

# 1. Read BlockTypeIds constants
with open('target/axolotl-pm/src/block/BlockTypeIds.php', 'r', encoding='utf-8') as f:
    bti_content = f.read()

bti_consts = re.findall(r'public const ([A-Z0-9_]+) = ([0-9]+);', bti_content)
print(f"Extracted {len(bti_consts)} BlockTypeIds constants")
assert len(bti_consts) == 833, f"Expected 833 constants, got {len(bti_consts)}"

# 2. Read VanillaBlocks assigners (830 blocks)
with open('target/axolotl-pm/generated/block/VanillaBlocks.php', 'r', encoding='utf-8') as f:
    vb_content = f.read()

assigners = re.findall(r'"([a-z0-9_]+)"\s*=>\s*fn\(([A-Za-z0-9_]+)\s*\$v\)\s*=>\s*self::\$_m([A-Z0-9_]+)', vb_content)
print(f"Extracted {len(assigners)} VanillaBlocks entries")
assert len(assigners) == 830, f"Expected 830 entries, got {len(assigners)}"

# 3. Read Bedrock BlockTypeNames
with open('target/axolotl-pm/generated/data/bedrock/block/BlockTypeNames.php', 'r', encoding='utf-8') as f:
    btn_text = f.read()
btn = dict(re.findall(r'public const ([A-Z0-9_]+) = "([^"]+)";', btn_text))

# 4. Read VanillaBlockMappings
with open('target/axolotl-pm/src/data/bedrock/block/convert/VanillaBlockMappings.php', 'r', encoding='utf-8') as f:
    vbm_text = f.read()
vbm = dict(re.findall(r'Blocks::([A-Z0-9_]+)\(\)[^;]*?Ids::([A-Z0-9_]+)', vbm_text))

special_mappings = {
    'ACACIA_CEILING_EDGES_HANGING_SIGN': 'minecraft:acacia_hanging_sign',
    'ACACIA_WALL_HANGING_SIGN': 'minecraft:acacia_hanging_sign',
    'BAMBOO_CEILING_EDGES_HANGING_SIGN': 'minecraft:bamboo_hanging_sign',
    'BAMBOO_WALL_HANGING_SIGN': 'minecraft:bamboo_hanging_sign',
    'BIRCH_CEILING_EDGES_HANGING_SIGN': 'minecraft:birch_hanging_sign',
    'BIRCH_WALL_HANGING_SIGN': 'minecraft:birch_hanging_sign',
    'CHERRY_CEILING_EDGES_HANGING_SIGN': 'minecraft:cherry_hanging_sign',
    'CHERRY_WALL_HANGING_SIGN': 'minecraft:cherry_hanging_sign',
    'CRIMSON_CEILING_EDGES_HANGING_SIGN': 'minecraft:crimson_hanging_sign',
    'CRIMSON_WALL_HANGING_SIGN': 'minecraft:crimson_hanging_sign',
    'DARK_OAK_CEILING_EDGES_HANGING_SIGN': 'minecraft:dark_oak_hanging_sign',
    'DARK_OAK_WALL_HANGING_SIGN': 'minecraft:dark_oak_hanging_sign',
    'JUNGLE_CEILING_EDGES_HANGING_SIGN': 'minecraft:jungle_hanging_sign',
    'JUNGLE_WALL_HANGING_SIGN': 'minecraft:jungle_hanging_sign',
    'MANGROVE_CEILING_EDGES_HANGING_SIGN': 'minecraft:mangrove_hanging_sign',
    'MANGROVE_WALL_HANGING_SIGN': 'minecraft:mangrove_hanging_sign',
    'OAK_CEILING_EDGES_HANGING_SIGN': 'minecraft:oak_hanging_sign',
    'OAK_WALL_HANGING_SIGN': 'minecraft:oak_hanging_sign',
    'PALE_OAK_CEILING_EDGES_HANGING_SIGN': 'minecraft:pale_oak_hanging_sign',
    'PALE_OAK_WALL_HANGING_SIGN': 'minecraft:pale_oak_hanging_sign',
    'SPRUCE_CEILING_EDGES_HANGING_SIGN': 'minecraft:spruce_hanging_sign',
    'SPRUCE_WALL_HANGING_SIGN': 'minecraft:spruce_hanging_sign',
    'WARPED_CEILING_CENTER_HANGING_SIGN': 'minecraft:warped_hanging_sign',
    'WARPED_CEILING_EDGES_HANGING_SIGN': 'minecraft:warped_hanging_sign',
    'WARPED_WALL_HANGING_SIGN': 'minecraft:warped_hanging_sign',
    'CAKE_WITH_DYED_CANDLE': 'minecraft:candle_cake',
    'CARPET': 'minecraft:carpet',
    'CONCRETE': 'minecraft:concrete',
    'CONCRETE_POWDER': 'minecraft:concrete_powder',
    'COPPER': 'minecraft:copper_block',
    'CORAL': 'minecraft:coral',
    'CORAL_BLOCK': 'minecraft:coral_block',
    'CORAL_FAN': 'minecraft:coral_fan',
    'WALL_CORAL_FAN': 'minecraft:coral_fan_hang',
    'DAYLIGHT_SENSOR': 'minecraft:daylight_detector',
    'DYED_CANDLE': 'minecraft:candle',
    'DYED_SHULKER_BOX': 'minecraft:shulker_box',
    'FAKE_WOODEN_SLAB': 'minecraft:wooden_slab',
    'FROGLIGHT': 'minecraft:ochre_froglight',
    'GLAZED_TERRACOTTA': 'minecraft:white_glazed_terracotta',
    'LIGHT': 'minecraft:light_block',
    'MOB_HEAD': 'minecraft:skull',
    'PRISMARINE_BRICKS_SLAB': 'minecraft:prismarine_brick_slab',
    'REDSTONE_COMPARATOR': 'minecraft:unpowered_comparator',
    'REDSTONE_REPEATER': 'minecraft:unpowered_repeater',
    'STAINED_CLAY': 'minecraft:stained_hardened_clay',
    'STAINED_GLASS': 'minecraft:stained_glass',
    'STAINED_GLASS_PANE': 'minecraft:stained_glass_pane',
    'STAINED_HARDENED_GLASS': 'minecraft:hard_stained_glass',
    'STAINED_HARDENED_GLASS_PANE': 'minecraft:hard_stained_glass_pane',
    'STONE_SLAB': 'minecraft:stone_slab',
    'WOOL': 'minecraft:wool',
}

block_to_bedrock = {}
for name, cls, const in assigners:
    if const in special_mappings:
        block_to_bedrock[const] = special_mappings[const]
    elif const in vbm and vbm[const] in btn:
        block_to_bedrock[const] = btn[vbm[const]]
    elif const in btn:
        block_to_bedrock[const] = btn[const]
    else:
        block_to_bedrock[const] = 'minecraft:' + name

# Common legacy and alternative aliases
# Format: ALIAS => CANONICAL_UPPERCASE
aliases = {
    'GRASS_BLOCK': 'GRASS',
    'GOLD_BLOCK': 'GOLD',
    'IRON_BLOCK': 'IRON',
    'DIAMOND_BLOCK': 'DIAMOND',
    'EMERALD_BLOCK': 'EMERALD',
    'LAPIS': 'LAPIS_LAZULI',
    'LAPIS_BLOCK': 'LAPIS_LAZULI',
    'LAPIS_LAZULI_BLOCK': 'LAPIS_LAZULI',
    'COAL_BLOCK': 'COAL',
    'REDSTONE_BLOCK': 'REDSTONE',
    'COPPER_BLOCK': 'COPPER',
    'NETHERITE_BLOCK': 'NETHERITE',
    'NOTEBLOCK': 'NOTE_BLOCK',
    'LEAVES': 'OAK_LEAVES',
    'LOG': 'OAK_LOG',
    'PLANKS': 'OAK_PLANKS',
    'SAPLING': 'OAK_SAPLING',
    'WOODEN_DOOR': 'OAK_DOOR',
    'DOOR': 'OAK_DOOR',
    'WOODEN_TRAPDOOR': 'OAK_TRAPDOOR',
    'TRAPDOOR': 'OAK_TRAPDOOR',
    'WOODEN_PRESSURE_PLATE': 'OAK_PRESSURE_PLATE',
    'PRESSURE_PLATE': 'OAK_PRESSURE_PLATE',
    'WOODEN_BUTTON': 'OAK_BUTTON',
    'BUTTON': 'OAK_BUTTON',
    'SIGN': 'OAK_SIGN',
    'STANDING_SIGN': 'OAK_SIGN',
    'WALL_SIGN': 'OAK_WALL_SIGN',
    'FENCE': 'OAK_FENCE',
    'FENCE_GATE': 'OAK_FENCE_GATE',
    'WORKBENCH': 'CRAFTING_TABLE',
    'FLOWING_WATER': 'WATER',
    'FLOWING_LAVA': 'LAVA',
    'COBBLE': 'COBBLESTONE',
    'MOSSY_COBBLE': 'MOSSY_COBBLESTONE',
    'STONE_BRICK': 'STONE_BRICKS',
    'STONEBRICK': 'STONE_BRICKS',
    'BRICK': 'BRICKS',
    'BRICK_BLOCK': 'BRICKS',
    'NETHER_BRICK': 'NETHER_BRICKS',
    'NETHER_BRICK_BLOCK': 'NETHER_BRICKS',
    'END_STONE_BRICK': 'END_STONE_BRICKS',
    'PRISMARINE_BRICK': 'PRISMARINE_BRICKS',
    'QUARTZ_BLOCK': 'QUARTZ',
    'MELON_BLOCK': 'MELON',
    'JACK_O_LANTERN': 'LIT_PUMPKIN',
    'SNOW_BLOCK': 'SNOW',
    'CLAY_BLOCK': 'CLAY',
    'TERRACOTTA': 'HARDENED_CLAY',
    'STAINED_HARDENED_CLAY': 'STAINED_CLAY',
    'DIRT_PATH': 'GRASS_PATH',
    'SLAB': 'STONE_SLAB',
    'WOODEN_SLAB': 'OAK_SLAB',
    'WOODEN_STAIRS': 'OAK_STAIRS',
    'DOUBLE_SLAB': 'STONE_SLAB',
    'SPONGE_WET': 'SPONGE',
    'WHITE_WOOL': 'WOOL',
}

# Generate PHP code
lines = []
lines.append("<?php")
lines.append("")
lines.append("declare(strict_types=1);")
lines.append("")
lines.append("namespace pocketmine\\block;")
lines.append("")
lines.append("/**")
lines.append(" * PM `BlockTypeTags`: tags classifying blocks by behavior or type.")
lines.append(" */")
lines.append("final class BlockTypeTags")
lines.append("{")
lines.append("    private const PREFIX = \"pocketmine:\";")
lines.append("")
lines.append("    public const DIRT = self::PREFIX . \"dirt\";")
lines.append("    public const MUD = self::PREFIX . \"mud\";")
lines.append("    public const SAND = self::PREFIX . \"sand\";")
lines.append("    public const POTTABLE_PLANTS = self::PREFIX . \"pottable\";")
lines.append("    public const FIRE = self::PREFIX . \"fire\";")
lines.append("    public const HANGING_SIGN = self::PREFIX . \"hanging_sign\";")
lines.append("    public const NYLIUM = self::PREFIX . \"nylium\";")
lines.append("    public const HUGE_FUNGUS_REPLACEABLE = self::PREFIX . \"huge_fungus_replaceable\";")
lines.append("    public const BAMBOO_MOSAIC = self::PREFIX . \"bamboo_mosaic\";")
lines.append("")
lines.append("    private function __construct()")
lines.append("    {")
lines.append("    }")
lines.append("}")
lines.append("")
lines.append("/**")
lines.append(" * Types of tools that can be used to break blocks.")
lines.append(" * Blocks may allow multiple tool types by combining these bitflags.")
lines.append(" */")
lines.append("final class BlockToolType")
lines.append("{")
lines.append("    public const NONE = 0;")
lines.append("    public const SWORD = 1 << 0;")
lines.append("    public const SHOVEL = 1 << 1;")
lines.append("    public const PICKAXE = 1 << 2;")
lines.append("    public const AXE = 1 << 3;")
lines.append("    public const SHEARS = 1 << 4;")
lines.append("    public const HOE = 1 << 5;")
lines.append("")
lines.append("    private function __construct()")
lines.append("    {")
lines.append("    }")
lines.append("}")
lines.append("")
lines.append("/**")
lines.append(" * Every block in VanillaBlocks has a corresponding constant in this class.")
lines.append(" * Complete PM5 block type IDs (833 constants).")
lines.append(" */")
lines.append("final class BlockTypeIds")
lines.append("{")

# Write all constants
for const_name, const_val in bti_consts:
    lines.append(f"    public const {const_name} = {const_val};")

lines.append("")
lines.append("    private static int $nextDynamicId = self::FIRST_UNUSED_BLOCK_ID;")
lines.append("")
lines.append("    /** @var array<string, int> Dynamic runtime cache */")
lines.append("    private static array $byName = [];")
lines.append("")
lines.append("    /** @var array<string, int> Fast lookup for all known names and common aliases */")
lines.append("    private static array $nameToId = [")

# Add all block names to nameToId
for const_name, const_val in bti_consts:
    if const_name == 'FIRST_UNUSED_BLOCK_ID':
        continue
    lines.append(f"        '{const_name.lower()}' => self::{const_name},")

# Add aliases to nameToId
for alias_name, canonical in sorted(aliases.items()):
    lines.append(f"        '{alias_name.lower()}' => self::{canonical},")

lines.append("    ];")
lines.append("")
lines.append("    private function __construct()")
lines.append("    {")
lines.append("    }")
lines.append("")
lines.append("    /**")
lines.append("     * Returns a new runtime block type ID, e.g. for use by a custom block.")
lines.append("     */")
lines.append("    public static function newId(): int")
lines.append("    {")
lines.append("        return self::$nextDynamicId++;")
lines.append("    }")
lines.append("")
lines.append("    /**")
lines.append("     * Resolves a block type ID by name (case-insensitive, optional minecraft: prefix).")
lines.append("     * If unknown, registers and returns a dynamic ID.")
lines.append("     */")
lines.append("    public static function forName(string $name): int")
lines.append("    {")
lines.append("        $key = strtolower(str_replace(['minecraft:', ' '], ['', '_'], $name));")
lines.append("        if (isset(self::$byName[$key])) {")
lines.append("            return self::$byName[$key];")
lines.append("        }")
lines.append("        if (isset(self::$nameToId[$key])) {")
lines.append("            return self::$byName[$key] = self::$nameToId[$key];")
lines.append("        }")
lines.append("        return self::$byName[$key] = self::newId();")
lines.append("    }")
lines.append("}")
lines.append("")
lines.append("/**")
lines.append(" * Allows getting a fresh instance of any block implemented by PocketMine-MP.")
lines.append(" * Every block here also has a constant of the same name in BlockTypeIds.")
lines.append(" */")
lines.append("final class VanillaBlocks")
lines.append("{")
lines.append("    /** @var array<string, Block> Cached block prototypes */")
lines.append("    private static array $blocks = [];")
lines.append("")
lines.append("    /** @var array<string, string> Legacy and alternative aliases (uppercase => canonical uppercase) */")
lines.append("    private const ALIASES = [")

for alias_name, canonical in sorted(aliases.items()):
    lines.append(f"        '{alias_name}' => '{canonical}',")

lines.append("    ];")
lines.append("")
lines.append("    /** @var array<string, string> Canonical uppercase name => authoritative Bedrock identifier */")
lines.append("    private const BLOCK_NAMES = [")

for const_name in sorted(block_to_bedrock.keys()):
    lines.append(f"        '{const_name}' => '{block_to_bedrock[const_name]}',")

lines.append("    ];")
lines.append("")
lines.append("    private function __construct()")
lines.append("    {")
lines.append("    }")
lines.append("")
lines.append("    /**")
lines.append("     * Resolves a block by name, case-insensitively, supporting camelCase, snake_case, and aliases.")
lines.append("     */")
lines.append("    public static function get(string $name): Block")
lines.append("    {")
lines.append("        if (preg_match('/[a-z][A-Z]/', $name)) {")
lines.append("            $name = (string) preg_replace('/(?<!^)[A-Z]/', '_$0', $name);")
lines.append("        }")
lines.append("        $key = strtoupper(str_replace(['minecraft:', ' '], ['', '_'], $name));")
lines.append("        $canonical = self::ALIASES[$key] ?? $key;")
lines.append("")
lines.append("        if (!isset(self::$blocks[$canonical])) {")
lines.append("            $blockName = self::BLOCK_NAMES[$canonical] ?? ('minecraft:' . strtolower($canonical));")
lines.append("            self::$blocks[$canonical] = Block::fromName($blockName);")
lines.append("        }")
lines.append("        return clone self::$blocks[$canonical];")
lines.append("    }")
lines.append("")
lines.append("    /**")
lines.append("     * Dynamic fallback for any method call (supports aliases, snake_case, camelCase, lowercase).")
lines.append("     */")
lines.append("    public static function __callStatic(string $name, array $args): Block")
lines.append("    {")
lines.append("        return self::get($name);")
lines.append("    }")
lines.append("")
lines.append("    /**")
lines.append("     * Returns an array of all registered vanilla blocks (indexed by uppercase canonical name).")
lines.append("     *")
lines.append("     * @return array<string, Block>")
lines.append("     */")
lines.append("    public static function getAll(): array")
lines.append("    {")
lines.append("        $all = [];")
lines.append("        foreach (self::BLOCK_NAMES as $key => $blockName) {")
lines.append("            $all[$key] = self::get($key);")
lines.append("        }")
lines.append("        return $all;")
lines.append("    }")
lines.append("")
lines.append("    // ------------------------------------------------------------------------")
lines.append("    // Explicit static getter methods for all 830 vanilla blocks")
lines.append("    // ------------------------------------------------------------------------")

for _, _, const_name in assigners:
    lines.append(f"    public static function {const_name}(): Block {{ return self::get('{const_name}'); }}")

lines.append("}")
lines.append("")

out_path = 'tools/generated_core_registry.php'
with open(out_path, 'w', encoding='utf-8') as f:
    f.write('\n'.join(lines))

print(f"Successfully generated {out_path} with {len(lines)} lines")
