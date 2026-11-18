<?php

declare(strict_types=1);

namespace pocketmine\block;

/**
 * PM `BlockTypeTags`: tags classifying blocks by behavior or type.
 */
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

/**
 * Types of tools that can be used to break blocks.
 * Blocks may allow multiple tool types by combining these bitflags.
 */
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

/**
 * Every block in VanillaBlocks has a corresponding constant in this class.
 * Complete PM5 block type IDs (833 constants).
 */
final class BlockTypeIds
{
    public const AIR = 10000;
    public const ACACIA_BUTTON = 10001;
    public const ACACIA_DOOR = 10002;
    public const ACACIA_FENCE = 10003;
    public const ACACIA_FENCE_GATE = 10004;
    public const ACACIA_LEAVES = 10005;
    public const ACACIA_LOG = 10006;
    public const ACACIA_PLANKS = 10007;
    public const ACACIA_PRESSURE_PLATE = 10008;
    public const ACACIA_SAPLING = 10009;
    public const ACACIA_SIGN = 10010;
    public const ACACIA_SLAB = 10011;
    public const ACACIA_STAIRS = 10012;
    public const ACACIA_TRAPDOOR = 10013;
    public const ACACIA_WALL_SIGN = 10014;
    public const ACACIA_WOOD = 10015;
    public const ACTIVATOR_RAIL = 10016;
    public const ALL_SIDED_MUSHROOM_STEM = 10017;
    public const ALLIUM = 10018;
    public const ANDESITE = 10019;
    public const ANDESITE_SLAB = 10020;
    public const ANDESITE_STAIRS = 10021;
    public const ANDESITE_WALL = 10022;
    public const ANVIL = 10023;
    public const AZURE_BLUET = 10024;
    public const BAMBOO = 10025;
    public const BAMBOO_SAPLING = 10026;
    public const BANNER = 10027;
    public const BARREL = 10028;
    public const BARRIER = 10029;
    public const BEACON = 10030;
    public const BED = 10031;
    public const BEDROCK = 10032;
    public const BEETROOTS = 10033;
    public const BELL = 10034;
    public const BIRCH_BUTTON = 10035;
    public const BIRCH_DOOR = 10036;
    public const BIRCH_FENCE = 10037;
    public const BIRCH_FENCE_GATE = 10038;
    public const BIRCH_LEAVES = 10039;
    public const BIRCH_LOG = 10040;
    public const BIRCH_PLANKS = 10041;
    public const BIRCH_PRESSURE_PLATE = 10042;
    public const BIRCH_SAPLING = 10043;
    public const BIRCH_SIGN = 10044;
    public const BIRCH_SLAB = 10045;
    public const BIRCH_STAIRS = 10046;
    public const BIRCH_TRAPDOOR = 10047;
    public const BIRCH_WALL_SIGN = 10048;
    public const BIRCH_WOOD = 10049;
    public const BLAST_FURNACE = 10051;
    public const BLUE_ICE = 10053;
    public const BLUE_ORCHID = 10054;
    public const BLUE_TORCH = 10055;
    public const BONE_BLOCK = 10056;
    public const BOOKSHELF = 10057;
    public const BREWING_STAND = 10058;
    public const BRICK_SLAB = 10059;
    public const BRICK_STAIRS = 10060;
    public const BRICK_WALL = 10061;
    public const BRICKS = 10062;
    public const BROWN_MUSHROOM = 10064;
    public const BROWN_MUSHROOM_BLOCK = 10065;
    public const CACTUS = 10066;
    public const CAKE = 10067;
    public const CARPET = 10068;
    public const CARROTS = 10069;
    public const CARVED_PUMPKIN = 10070;
    public const CHEMICAL_HEAT = 10071;
    public const CHEST = 10072;
    public const CHISELED_QUARTZ = 10073;
    public const CHISELED_RED_SANDSTONE = 10074;
    public const CHISELED_SANDSTONE = 10075;
    public const CHISELED_STONE_BRICKS = 10076;
    public const CLAY = 10077;
    public const COAL = 10078;
    public const COAL_ORE = 10079;
    public const COBBLESTONE = 10080;
    public const COBBLESTONE_SLAB = 10081;
    public const COBBLESTONE_STAIRS = 10082;
    public const COBBLESTONE_WALL = 10083;
    public const COBWEB = 10084;
    public const COCOA_POD = 10085;
    public const COMPOUND_CREATOR = 10086;
    public const CONCRETE = 10087;
    public const CONCRETE_POWDER = 10088;
    public const CORAL = 10089;
    public const CORAL_BLOCK = 10090;
    public const CORAL_FAN = 10091;
    public const CORNFLOWER = 10092;
    public const CRACKED_STONE_BRICKS = 10093;
    public const CRAFTING_TABLE = 10094;
    public const CUT_RED_SANDSTONE = 10095;
    public const CUT_RED_SANDSTONE_SLAB = 10096;
    public const CUT_SANDSTONE = 10097;
    public const CUT_SANDSTONE_SLAB = 10098;
    public const DANDELION = 10100;
    public const DARK_OAK_BUTTON = 10101;
    public const DARK_OAK_DOOR = 10102;
    public const DARK_OAK_FENCE = 10103;
    public const DARK_OAK_FENCE_GATE = 10104;
    public const DARK_OAK_LEAVES = 10105;
    public const DARK_OAK_LOG = 10106;
    public const DARK_OAK_PLANKS = 10107;
    public const DARK_OAK_PRESSURE_PLATE = 10108;
    public const DARK_OAK_SAPLING = 10109;
    public const DARK_OAK_SIGN = 10110;
    public const DARK_OAK_SLAB = 10111;
    public const DARK_OAK_STAIRS = 10112;
    public const DARK_OAK_TRAPDOOR = 10113;
    public const DARK_OAK_WALL_SIGN = 10114;
    public const DARK_OAK_WOOD = 10115;
    public const DARK_PRISMARINE = 10116;
    public const DARK_PRISMARINE_SLAB = 10117;
    public const DARK_PRISMARINE_STAIRS = 10118;
    public const DAYLIGHT_SENSOR = 10119;
    public const DEAD_BUSH = 10120;
    public const DETECTOR_RAIL = 10121;
    public const DIAMOND = 10122;
    public const DIAMOND_ORE = 10123;
    public const DIORITE = 10124;
    public const DIORITE_SLAB = 10125;
    public const DIORITE_STAIRS = 10126;
    public const DIORITE_WALL = 10127;
    public const DIRT = 10128;
    public const DOUBLE_TALLGRASS = 10129;
    public const DRAGON_EGG = 10130;
    public const DRIED_KELP = 10131;
    public const DYED_SHULKER_BOX = 10132;
    public const ELEMENT_ACTINIUM = 10133;
    public const ELEMENT_ALUMINUM = 10134;
    public const ELEMENT_AMERICIUM = 10135;
    public const ELEMENT_ANTIMONY = 10136;
    public const ELEMENT_ARGON = 10137;
    public const ELEMENT_ARSENIC = 10138;
    public const ELEMENT_ASTATINE = 10139;
    public const ELEMENT_BARIUM = 10140;
    public const ELEMENT_BERKELIUM = 10141;
    public const ELEMENT_BERYLLIUM = 10142;
    public const ELEMENT_BISMUTH = 10143;
    public const ELEMENT_BOHRIUM = 10144;
    public const ELEMENT_BORON = 10145;
    public const ELEMENT_BROMINE = 10146;
    public const ELEMENT_CADMIUM = 10147;
    public const ELEMENT_CALCIUM = 10148;
    public const ELEMENT_CALIFORNIUM = 10149;
    public const ELEMENT_CARBON = 10150;
    public const ELEMENT_CERIUM = 10151;
    public const ELEMENT_CESIUM = 10152;
    public const ELEMENT_CHLORINE = 10153;
    public const ELEMENT_CHROMIUM = 10154;
    public const ELEMENT_COBALT = 10155;
    public const ELEMENT_CONSTRUCTOR = 10156;
    public const ELEMENT_COPERNICIUM = 10157;
    public const ELEMENT_COPPER = 10158;
    public const ELEMENT_CURIUM = 10159;
    public const ELEMENT_DARMSTADTIUM = 10160;
    public const ELEMENT_DUBNIUM = 10161;
    public const ELEMENT_DYSPROSIUM = 10162;
    public const ELEMENT_EINSTEINIUM = 10163;
    public const ELEMENT_ERBIUM = 10164;
    public const ELEMENT_EUROPIUM = 10165;
    public const ELEMENT_FERMIUM = 10166;
    public const ELEMENT_FLEROVIUM = 10167;
    public const ELEMENT_FLUORINE = 10168;
    public const ELEMENT_FRANCIUM = 10169;
    public const ELEMENT_GADOLINIUM = 10170;
    public const ELEMENT_GALLIUM = 10171;
    public const ELEMENT_GERMANIUM = 10172;
    public const ELEMENT_GOLD = 10173;
    public const ELEMENT_HAFNIUM = 10174;
    public const ELEMENT_HASSIUM = 10175;
    public const ELEMENT_HELIUM = 10176;
    public const ELEMENT_HOLMIUM = 10177;
    public const ELEMENT_HYDROGEN = 10178;
    public const ELEMENT_INDIUM = 10179;
    public const ELEMENT_IODINE = 10180;
    public const ELEMENT_IRIDIUM = 10181;
    public const ELEMENT_IRON = 10182;
    public const ELEMENT_KRYPTON = 10183;
    public const ELEMENT_LANTHANUM = 10184;
    public const ELEMENT_LAWRENCIUM = 10185;
    public const ELEMENT_LEAD = 10186;
    public const ELEMENT_LITHIUM = 10187;
    public const ELEMENT_LIVERMORIUM = 10188;
    public const ELEMENT_LUTETIUM = 10189;
    public const ELEMENT_MAGNESIUM = 10190;
    public const ELEMENT_MANGANESE = 10191;
    public const ELEMENT_MEITNERIUM = 10192;
    public const ELEMENT_MENDELEVIUM = 10193;
    public const ELEMENT_MERCURY = 10194;
    public const ELEMENT_MOLYBDENUM = 10195;
    public const ELEMENT_MOSCOVIUM = 10196;
    public const ELEMENT_NEODYMIUM = 10197;
    public const ELEMENT_NEON = 10198;
    public const ELEMENT_NEPTUNIUM = 10199;
    public const ELEMENT_NICKEL = 10200;
    public const ELEMENT_NIHONIUM = 10201;
    public const ELEMENT_NIOBIUM = 10202;
    public const ELEMENT_NITROGEN = 10203;
    public const ELEMENT_NOBELIUM = 10204;
    public const ELEMENT_OGANESSON = 10205;
    public const ELEMENT_OSMIUM = 10206;
    public const ELEMENT_OXYGEN = 10207;
    public const ELEMENT_PALLADIUM = 10208;
    public const ELEMENT_PHOSPHORUS = 10209;
    public const ELEMENT_PLATINUM = 10210;
    public const ELEMENT_PLUTONIUM = 10211;
    public const ELEMENT_POLONIUM = 10212;
    public const ELEMENT_POTASSIUM = 10213;
    public const ELEMENT_PRASEODYMIUM = 10214;
    public const ELEMENT_PROMETHIUM = 10215;
    public const ELEMENT_PROTACTINIUM = 10216;
    public const ELEMENT_RADIUM = 10217;
    public const ELEMENT_RADON = 10218;
    public const ELEMENT_RHENIUM = 10219;
    public const ELEMENT_RHODIUM = 10220;
    public const ELEMENT_ROENTGENIUM = 10221;
    public const ELEMENT_RUBIDIUM = 10222;
    public const ELEMENT_RUTHENIUM = 10223;
    public const ELEMENT_RUTHERFORDIUM = 10224;
    public const ELEMENT_SAMARIUM = 10225;
    public const ELEMENT_SCANDIUM = 10226;
    public const ELEMENT_SEABORGIUM = 10227;
    public const ELEMENT_SELENIUM = 10228;
    public const ELEMENT_SILICON = 10229;
    public const ELEMENT_SILVER = 10230;
    public const ELEMENT_SODIUM = 10231;
    public const ELEMENT_STRONTIUM = 10232;
    public const ELEMENT_SULFUR = 10233;
    public const ELEMENT_TANTALUM = 10234;
    public const ELEMENT_TECHNETIUM = 10235;
    public const ELEMENT_TELLURIUM = 10236;
    public const ELEMENT_TENNESSINE = 10237;
    public const ELEMENT_TERBIUM = 10238;
    public const ELEMENT_THALLIUM = 10239;
    public const ELEMENT_THORIUM = 10240;
    public const ELEMENT_THULIUM = 10241;
    public const ELEMENT_TIN = 10242;
    public const ELEMENT_TITANIUM = 10243;
    public const ELEMENT_TUNGSTEN = 10244;
    public const ELEMENT_URANIUM = 10245;
    public const ELEMENT_VANADIUM = 10246;
    public const ELEMENT_XENON = 10247;
    public const ELEMENT_YTTERBIUM = 10248;
    public const ELEMENT_YTTRIUM = 10249;
    public const ELEMENT_ZERO = 10250;
    public const ELEMENT_ZINC = 10251;
    public const ELEMENT_ZIRCONIUM = 10252;
    public const EMERALD = 10253;
    public const EMERALD_ORE = 10254;
    public const ENCHANTING_TABLE = 10255;
    public const END_PORTAL_FRAME = 10256;
    public const END_ROD = 10257;
    public const END_STONE = 10258;
    public const END_STONE_BRICK_SLAB = 10259;
    public const END_STONE_BRICK_STAIRS = 10260;
    public const END_STONE_BRICK_WALL = 10261;
    public const END_STONE_BRICKS = 10262;
    public const ENDER_CHEST = 10263;
    public const FAKE_WOODEN_SLAB = 10264;
    public const FARMLAND = 10265;
    public const FERN = 10266;
    public const FIRE = 10267;
    public const FLETCHING_TABLE = 10268;
    public const FLOWER_POT = 10269;
    public const FROSTED_ICE = 10270;
    public const FURNACE = 10271;
    public const GLASS = 10272;
    public const GLASS_PANE = 10273;
    public const GLOWING_OBSIDIAN = 10274;
    public const GLOWSTONE = 10275;
    public const GOLD = 10276;
    public const GOLD_ORE = 10277;
    public const GRANITE = 10278;
    public const GRANITE_SLAB = 10279;
    public const GRANITE_STAIRS = 10280;
    public const GRANITE_WALL = 10281;
    public const GRASS = 10282;
    public const GRASS_PATH = 10283;
    public const GRAVEL = 10284;
    public const GREEN_TORCH = 10287;
    public const HARDENED_CLAY = 10288;
    public const HARDENED_GLASS = 10289;
    public const HARDENED_GLASS_PANE = 10290;
    public const HAY_BALE = 10291;
    public const HOPPER = 10292;
    public const ICE = 10293;
    public const INFESTED_CHISELED_STONE_BRICK = 10294;
    public const INFESTED_COBBLESTONE = 10295;
    public const INFESTED_CRACKED_STONE_BRICK = 10296;
    public const INFESTED_MOSSY_STONE_BRICK = 10297;
    public const INFESTED_STONE = 10298;
    public const INFESTED_STONE_BRICK = 10299;
    public const INFO_UPDATE = 10300;
    public const INFO_UPDATE2 = 10301;
    public const INVISIBLE_BEDROCK = 10302;
    public const IRON = 10303;
    public const IRON_BARS = 10304;
    public const IRON_DOOR = 10305;
    public const IRON_ORE = 10306;
    public const IRON_TRAPDOOR = 10307;
    public const ITEM_FRAME = 10308;
    public const JUKEBOX = 10309;
    public const JUNGLE_BUTTON = 10310;
    public const JUNGLE_DOOR = 10311;
    public const JUNGLE_FENCE = 10312;
    public const JUNGLE_FENCE_GATE = 10313;
    public const JUNGLE_LEAVES = 10314;
    public const JUNGLE_LOG = 10315;
    public const JUNGLE_PLANKS = 10316;
    public const JUNGLE_PRESSURE_PLATE = 10317;
    public const JUNGLE_SAPLING = 10318;
    public const JUNGLE_SIGN = 10319;
    public const JUNGLE_SLAB = 10320;
    public const JUNGLE_STAIRS = 10321;
    public const JUNGLE_TRAPDOOR = 10322;
    public const JUNGLE_WALL_SIGN = 10323;
    public const JUNGLE_WOOD = 10324;
    public const LAB_TABLE = 10325;
    public const LADDER = 10326;
    public const LANTERN = 10327;
    public const LAPIS_LAZULI = 10328;
    public const LAPIS_LAZULI_ORE = 10329;
    public const LARGE_FERN = 10330;
    public const LAVA = 10331;
    public const LECTERN = 10332;
    public const LEGACY_STONECUTTER = 10333;
    public const LEVER = 10334;
    public const LILAC = 10337;
    public const LILY_OF_THE_VALLEY = 10338;
    public const LILY_PAD = 10339;
    public const LIT_PUMPKIN = 10341;
    public const LOOM = 10342;
    public const MAGMA = 10344;
    public const MATERIAL_REDUCER = 10345;
    public const MELON = 10346;
    public const MELON_STEM = 10347;
    public const MOB_HEAD = 10348;
    public const MONSTER_SPAWNER = 10349;
    public const MOSSY_COBBLESTONE = 10350;
    public const MOSSY_COBBLESTONE_SLAB = 10351;
    public const MOSSY_COBBLESTONE_STAIRS = 10352;
    public const MOSSY_COBBLESTONE_WALL = 10353;
    public const MOSSY_STONE_BRICK_SLAB = 10354;
    public const MOSSY_STONE_BRICK_STAIRS = 10355;
    public const MOSSY_STONE_BRICK_WALL = 10356;
    public const MOSSY_STONE_BRICKS = 10357;
    public const MUSHROOM_STEM = 10358;
    public const MYCELIUM = 10359;
    public const NETHER_BRICK_FENCE = 10360;
    public const NETHER_BRICK_SLAB = 10361;
    public const NETHER_BRICK_STAIRS = 10362;
    public const NETHER_BRICK_WALL = 10363;
    public const NETHER_BRICKS = 10364;
    public const NETHER_PORTAL = 10365;
    public const NETHER_QUARTZ_ORE = 10366;
    public const NETHER_REACTOR_CORE = 10367;
    public const NETHER_WART = 10368;
    public const NETHER_WART_BLOCK = 10369;
    public const NETHERRACK = 10370;
    public const NOTE_BLOCK = 10371;
    public const OAK_BUTTON = 10372;
    public const OAK_DOOR = 10373;
    public const OAK_FENCE = 10374;
    public const OAK_FENCE_GATE = 10375;
    public const OAK_LEAVES = 10376;
    public const OAK_LOG = 10377;
    public const OAK_PLANKS = 10378;
    public const OAK_PRESSURE_PLATE = 10379;
    public const OAK_SAPLING = 10380;
    public const OAK_SIGN = 10381;
    public const OAK_SLAB = 10382;
    public const OAK_STAIRS = 10383;
    public const OAK_TRAPDOOR = 10384;
    public const OAK_WALL_SIGN = 10385;
    public const OAK_WOOD = 10386;
    public const OBSIDIAN = 10387;
    public const ORANGE_TULIP = 10389;
    public const OXEYE_DAISY = 10390;
    public const PACKED_ICE = 10391;
    public const PEONY = 10392;
    public const PINK_TULIP = 10394;
    public const PODZOL = 10395;
    public const POLISHED_ANDESITE = 10396;
    public const POLISHED_ANDESITE_SLAB = 10397;
    public const POLISHED_ANDESITE_STAIRS = 10398;
    public const POLISHED_DIORITE = 10399;
    public const POLISHED_DIORITE_SLAB = 10400;
    public const POLISHED_DIORITE_STAIRS = 10401;
    public const POLISHED_GRANITE = 10402;
    public const POLISHED_GRANITE_SLAB = 10403;
    public const POLISHED_GRANITE_STAIRS = 10404;
    public const POPPY = 10405;
    public const POTATOES = 10406;
    public const POWERED_RAIL = 10407;
    public const PRISMARINE = 10408;
    public const PRISMARINE_BRICKS = 10409;
    public const PRISMARINE_BRICKS_SLAB = 10410;
    public const PRISMARINE_BRICKS_STAIRS = 10411;
    public const PRISMARINE_SLAB = 10412;
    public const PRISMARINE_STAIRS = 10413;
    public const PRISMARINE_WALL = 10414;
    public const PUMPKIN = 10415;
    public const PUMPKIN_STEM = 10416;
    public const PURPLE_TORCH = 10418;
    public const PURPUR = 10419;
    public const PURPUR_PILLAR = 10420;
    public const PURPUR_SLAB = 10421;
    public const PURPUR_STAIRS = 10422;
    public const QUARTZ = 10423;
    public const QUARTZ_PILLAR = 10424;
    public const QUARTZ_SLAB = 10425;
    public const QUARTZ_STAIRS = 10426;
    public const RAIL = 10427;
    public const RED_MUSHROOM = 10429;
    public const RED_MUSHROOM_BLOCK = 10430;
    public const RED_NETHER_BRICK_SLAB = 10431;
    public const RED_NETHER_BRICK_STAIRS = 10432;
    public const RED_NETHER_BRICK_WALL = 10433;
    public const RED_NETHER_BRICKS = 10434;
    public const RED_SAND = 10435;
    public const RED_SANDSTONE = 10436;
    public const RED_SANDSTONE_SLAB = 10437;
    public const RED_SANDSTONE_STAIRS = 10438;
    public const RED_SANDSTONE_WALL = 10439;
    public const RED_TORCH = 10440;
    public const RED_TULIP = 10441;
    public const REDSTONE = 10442;
    public const REDSTONE_COMPARATOR = 10443;
    public const REDSTONE_LAMP = 10444;
    public const REDSTONE_ORE = 10445;
    public const REDSTONE_REPEATER = 10446;
    public const REDSTONE_TORCH = 10447;
    public const REDSTONE_WIRE = 10448;
    public const RESERVED6 = 10449;
    public const ROSE_BUSH = 10450;
    public const SAND = 10451;
    public const SANDSTONE = 10452;
    public const SANDSTONE_SLAB = 10453;
    public const SANDSTONE_STAIRS = 10454;
    public const SANDSTONE_WALL = 10455;
    public const SEA_LANTERN = 10456;
    public const SEA_PICKLE = 10457;
    public const SHULKER_BOX = 10458;
    public const SLIME = 10459;
    public const SMOKER = 10460;
    public const SMOOTH_QUARTZ = 10461;
    public const SMOOTH_QUARTZ_SLAB = 10462;
    public const SMOOTH_QUARTZ_STAIRS = 10463;
    public const SMOOTH_RED_SANDSTONE = 10464;
    public const SMOOTH_RED_SANDSTONE_SLAB = 10465;
    public const SMOOTH_RED_SANDSTONE_STAIRS = 10466;
    public const SMOOTH_SANDSTONE = 10467;
    public const SMOOTH_SANDSTONE_SLAB = 10468;
    public const SMOOTH_SANDSTONE_STAIRS = 10469;
    public const SMOOTH_STONE = 10470;
    public const SMOOTH_STONE_SLAB = 10471;
    public const SNOW = 10472;
    public const SNOW_LAYER = 10473;
    public const SOUL_SAND = 10474;
    public const SPONGE = 10475;
    public const SPRUCE_BUTTON = 10476;
    public const SPRUCE_DOOR = 10477;
    public const SPRUCE_FENCE = 10478;
    public const SPRUCE_FENCE_GATE = 10479;
    public const SPRUCE_LEAVES = 10480;
    public const SPRUCE_LOG = 10481;
    public const SPRUCE_PLANKS = 10482;
    public const SPRUCE_PRESSURE_PLATE = 10483;
    public const SPRUCE_SAPLING = 10484;
    public const SPRUCE_SIGN = 10485;
    public const SPRUCE_SLAB = 10486;
    public const SPRUCE_STAIRS = 10487;
    public const SPRUCE_TRAPDOOR = 10488;
    public const SPRUCE_WALL_SIGN = 10489;
    public const SPRUCE_WOOD = 10490;
    public const STAINED_CLAY = 10491;
    public const STAINED_GLASS = 10492;
    public const STAINED_GLASS_PANE = 10493;
    public const STAINED_HARDENED_GLASS = 10494;
    public const STAINED_HARDENED_GLASS_PANE = 10495;
    public const STONE = 10496;
    public const STONE_BRICK_SLAB = 10497;
    public const STONE_BRICK_STAIRS = 10498;
    public const STONE_BRICK_WALL = 10499;
    public const STONE_BRICKS = 10500;
    public const STONE_BUTTON = 10501;
    public const STONE_PRESSURE_PLATE = 10502;
    public const STONE_SLAB = 10503;
    public const STONE_STAIRS = 10504;
    public const STONECUTTER = 10505;
    public const SUGARCANE = 10518;
    public const SUNFLOWER = 10519;
    public const SWEET_BERRY_BUSH = 10520;
    public const TALL_GRASS = 10521;
    public const TNT = 10522;
    public const TORCH = 10523;
    public const TRAPPED_CHEST = 10524;
    public const TRIPWIRE = 10525;
    public const TRIPWIRE_HOOK = 10526;
    public const UNDERWATER_TORCH = 10527;
    public const VINES = 10528;
    public const WALL_BANNER = 10529;
    public const WALL_CORAL_FAN = 10530;
    public const WATER = 10531;
    public const WEIGHTED_PRESSURE_PLATE_HEAVY = 10532;
    public const WEIGHTED_PRESSURE_PLATE_LIGHT = 10533;
    public const WHEAT = 10534;
    public const BUDDING_AMETHYST = 10535;
    public const WHITE_TULIP = 10536;
    public const WOOL = 10537;
    public const AMETHYST_CLUSTER = 10538;
    public const GLAZED_TERRACOTTA = 10539;
    public const AMETHYST = 10540;
    public const ANCIENT_DEBRIS = 10541;
    public const BASALT = 10542;
    public const POLISHED_BASALT = 10543;
    public const SMOOTH_BASALT = 10544;
    public const BLACKSTONE = 10545;
    public const BLACKSTONE_SLAB = 10546;
    public const BLACKSTONE_STAIRS = 10547;
    public const BLACKSTONE_WALL = 10548;
    public const POLISHED_BLACKSTONE = 10549;
    public const POLISHED_BLACKSTONE_BUTTON = 10550;
    public const POLISHED_BLACKSTONE_PRESSURE_PLATE = 10551;
    public const POLISHED_BLACKSTONE_SLAB = 10552;
    public const POLISHED_BLACKSTONE_STAIRS = 10553;
    public const POLISHED_BLACKSTONE_WALL = 10554;
    public const CHISELED_POLISHED_BLACKSTONE = 10555;
    public const POLISHED_BLACKSTONE_BRICKS = 10556;
    public const POLISHED_BLACKSTONE_BRICK_SLAB = 10557;
    public const POLISHED_BLACKSTONE_BRICK_STAIRS = 10558;
    public const POLISHED_BLACKSTONE_BRICK_WALL = 10559;
    public const CRACKED_POLISHED_BLACKSTONE_BRICKS = 10560;
    public const LIGHT = 10561;
    public const RAW_COPPER = 10562;
    public const RAW_GOLD = 10563;
    public const RAW_IRON = 10564;
    public const CALCITE = 10565;
    public const DEEPSLATE = 10566;
    public const DEEPSLATE_BRICKS = 10567;
    public const DEEPSLATE_BRICK_SLAB = 10568;
    public const DEEPSLATE_BRICK_STAIRS = 10569;
    public const DEEPSLATE_BRICK_WALL = 10570;
    public const CRACKED_DEEPSLATE_BRICKS = 10571;
    public const DEEPSLATE_TILES = 10572;
    public const DEEPSLATE_TILE_SLAB = 10573;
    public const DEEPSLATE_TILE_STAIRS = 10574;
    public const DEEPSLATE_TILE_WALL = 10575;
    public const CRACKED_DEEPSLATE_TILES = 10576;
    public const COBBLED_DEEPSLATE = 10577;
    public const COBBLED_DEEPSLATE_SLAB = 10578;
    public const COBBLED_DEEPSLATE_STAIRS = 10579;
    public const COBBLED_DEEPSLATE_WALL = 10580;
    public const POLISHED_DEEPSLATE = 10581;
    public const POLISHED_DEEPSLATE_SLAB = 10582;
    public const POLISHED_DEEPSLATE_STAIRS = 10583;
    public const POLISHED_DEEPSLATE_WALL = 10584;
    public const QUARTZ_BRICKS = 10585;
    public const CHISELED_DEEPSLATE = 10586;
    public const CHISELED_NETHER_BRICKS = 10587;
    public const CRACKED_NETHER_BRICKS = 10588;
    public const TUFF = 10589;
    public const SOUL_TORCH = 10590;
    public const SOUL_LANTERN = 10591;
    public const SOUL_SOIL = 10592;
    public const SOUL_FIRE = 10593;
    public const SHROOMLIGHT = 10594;
    public const MANGROVE_PLANKS = 10595;
    public const CRIMSON_PLANKS = 10596;
    public const WARPED_PLANKS = 10597;
    public const MANGROVE_FENCE = 10598;
    public const CRIMSON_FENCE = 10599;
    public const WARPED_FENCE = 10600;
    public const MANGROVE_SLAB = 10601;
    public const CRIMSON_SLAB = 10602;
    public const WARPED_SLAB = 10603;
    public const MANGROVE_LOG = 10604;
    public const CRIMSON_STEM = 10605;
    public const WARPED_STEM = 10606;
    public const MANGROVE_WOOD = 10607;
    public const CRIMSON_HYPHAE = 10608;
    public const WARPED_HYPHAE = 10609;
    public const MANGROVE_TRAPDOOR = 10610;
    public const CRIMSON_TRAPDOOR = 10611;
    public const WARPED_TRAPDOOR = 10612;
    public const MANGROVE_BUTTON = 10613;
    public const CRIMSON_BUTTON = 10614;
    public const WARPED_BUTTON = 10615;
    public const MANGROVE_PRESSURE_PLATE = 10616;
    public const CRIMSON_PRESSURE_PLATE = 10617;
    public const WARPED_PRESSURE_PLATE = 10618;
    public const MANGROVE_DOOR = 10619;
    public const CRIMSON_DOOR = 10620;
    public const WARPED_DOOR = 10621;
    public const MANGROVE_FENCE_GATE = 10622;
    public const CRIMSON_FENCE_GATE = 10623;
    public const WARPED_FENCE_GATE = 10624;
    public const MANGROVE_STAIRS = 10625;
    public const CRIMSON_STAIRS = 10626;
    public const WARPED_STAIRS = 10627;
    public const MANGROVE_SIGN = 10628;
    public const CRIMSON_SIGN = 10629;
    public const WARPED_SIGN = 10630;
    public const MANGROVE_WALL_SIGN = 10631;
    public const CRIMSON_WALL_SIGN = 10632;
    public const WARPED_WALL_SIGN = 10633;
    public const TINTED_GLASS = 10634;
    public const HONEYCOMB = 10635;
    public const DEEPSLATE_COAL_ORE = 10636;
    public const DEEPSLATE_DIAMOND_ORE = 10637;
    public const DEEPSLATE_EMERALD_ORE = 10638;
    public const DEEPSLATE_LAPIS_LAZULI_ORE = 10639;
    public const DEEPSLATE_REDSTONE_ORE = 10640;
    public const DEEPSLATE_IRON_ORE = 10641;
    public const DEEPSLATE_GOLD_ORE = 10642;
    public const DEEPSLATE_COPPER_ORE = 10643;
    public const COPPER_ORE = 10644;
    public const NETHER_GOLD_ORE = 10645;
    public const MUD = 10646;
    public const MUD_BRICKS = 10647;
    public const MUD_BRICK_SLAB = 10648;
    public const MUD_BRICK_STAIRS = 10649;
    public const MUD_BRICK_WALL = 10650;
    public const PACKED_MUD = 10651;
    public const WARPED_WART_BLOCK = 10652;
    public const CRYING_OBSIDIAN = 10653;
    public const GILDED_BLACKSTONE = 10654;
    public const LIGHTNING_ROD = 10655;
    public const COPPER = 10656;
    public const CUT_COPPER = 10657;
    public const CUT_COPPER_SLAB = 10658;
    public const CUT_COPPER_STAIRS = 10659;
    public const CANDLE = 10660;
    public const DYED_CANDLE = 10661;
    public const CAKE_WITH_CANDLE = 10662;
    public const CAKE_WITH_DYED_CANDLE = 10663;
    public const WITHER_ROSE = 10664;
    public const HANGING_ROOTS = 10665;
    public const CARTOGRAPHY_TABLE = 10666;
    public const SMITHING_TABLE = 10667;
    public const NETHERITE = 10668;
    public const SPORE_BLOSSOM = 10669;
    public const CAULDRON = 10670;
    public const WATER_CAULDRON = 10671;
    public const LAVA_CAULDRON = 10672;
    public const POTION_CAULDRON = 10673;
    public const POWDER_SNOW_CAULDRON = 10674;
    public const CHORUS_FLOWER = 10675;
    public const CHORUS_PLANT = 10676;
    public const MANGROVE_ROOTS = 10677;
    public const MUDDY_MANGROVE_ROOTS = 10678;
    public const FROGLIGHT = 10679;
    public const TWISTING_VINES = 10680;
    public const WEEPING_VINES = 10681;
    public const CHAIN = 10682;
    public const SCULK = 10683;
    public const GLOWING_ITEM_FRAME = 10684;
    public const MANGROVE_LEAVES = 10685;
    public const AZALEA_LEAVES = 10686;
    public const FLOWERING_AZALEA_LEAVES = 10687;
    public const REINFORCED_DEEPSLATE = 10688;
    public const CAVE_VINES = 10689;
    public const GLOW_LICHEN = 10690;
    public const CHERRY_BUTTON = 10691;
    public const CHERRY_DOOR = 10692;
    public const CHERRY_FENCE = 10693;
    public const CHERRY_FENCE_GATE = 10694;
    public const CHERRY_LEAVES = 10695;
    public const CHERRY_LOG = 10696;
    public const CHERRY_PLANKS = 10697;
    public const CHERRY_PRESSURE_PLATE = 10698;
    public const CHERRY_SAPLING = 10699;
    public const CHERRY_SIGN = 10700;
    public const CHERRY_SLAB = 10701;
    public const CHERRY_STAIRS = 10702;
    public const CHERRY_TRAPDOOR = 10703;
    public const CHERRY_WALL_SIGN = 10704;
    public const CHERRY_WOOD = 10705;
    public const SMALL_DRIPLEAF = 10706;
    public const BIG_DRIPLEAF_HEAD = 10707;
    public const BIG_DRIPLEAF_STEM = 10708;
    public const PINK_PETALS = 10709;
    public const CRIMSON_ROOTS = 10710;
    public const WARPED_ROOTS = 10711;
    public const CHISELED_BOOKSHELF = 10712;
    public const TORCHFLOWER = 10713;
    public const TORCHFLOWER_CROP = 10714;
    public const PITCHER_PLANT = 10715;
    public const PITCHER_CROP = 10716;
    public const DOUBLE_PITCHER_CROP = 10717;
    public const CAMPFIRE = 10718;
    public const SOUL_CAMPFIRE = 10719;
    public const TUFF_SLAB = 10720;
    public const TUFF_STAIRS = 10721;
    public const TUFF_WALL = 10722;
    public const CHISELED_TUFF = 10723;
    public const TUFF_BRICKS = 10724;
    public const TUFF_BRICK_SLAB = 10725;
    public const TUFF_BRICK_STAIRS = 10726;
    public const TUFF_BRICK_WALL = 10727;
    public const CHISELED_TUFF_BRICKS = 10728;
    public const POLISHED_TUFF = 10729;
    public const POLISHED_TUFF_SLAB = 10730;
    public const POLISHED_TUFF_STAIRS = 10731;
    public const POLISHED_TUFF_WALL = 10732;
    public const COPPER_BULB = 10733;
    public const COPPER_DOOR = 10734;
    public const COPPER_TRAPDOOR = 10735;
    public const CHISELED_COPPER = 10736;
    public const COPPER_GRATE = 10737;
    public const PALE_OAK_BUTTON = 10738;
    public const PALE_OAK_DOOR = 10739;
    public const PALE_OAK_FENCE = 10740;
    public const PALE_OAK_FENCE_GATE = 10741;
    public const PALE_OAK_LEAVES = 10742;
    public const PALE_OAK_LOG = 10743;
    public const PALE_OAK_PLANKS = 10744;
    public const PALE_OAK_PRESSURE_PLATE = 10745;
    public const PALE_OAK_SIGN = 10746;
    public const PALE_OAK_SLAB = 10747;
    public const PALE_OAK_STAIRS = 10748;
    public const PALE_OAK_TRAPDOOR = 10749;
    public const PALE_OAK_WALL_SIGN = 10750;
    public const PALE_OAK_WOOD = 10751;
    public const RESIN = 10752;
    public const RESIN_BRICK_SLAB = 10753;
    public const RESIN_BRICK_STAIRS = 10754;
    public const RESIN_BRICK_WALL = 10755;
    public const RESIN_BRICKS = 10756;
    public const RESIN_CLUMP = 10757;
    public const CHISELED_RESIN_BRICKS = 10758;
    public const RESPAWN_ANCHOR = 10759;
    public const OMINOUS_BANNER = 10760;
    public const OMINOUS_WALL_BANNER = 10761;
    public const ACACIA_CEILING_CENTER_HANGING_SIGN = 10762;
    public const ACACIA_CEILING_EDGES_HANGING_SIGN = 10763;
    public const ACACIA_WALL_HANGING_SIGN = 10764;
    public const BIRCH_CEILING_CENTER_HANGING_SIGN = 10765;
    public const BIRCH_CEILING_EDGES_HANGING_SIGN = 10766;
    public const BIRCH_WALL_HANGING_SIGN = 10767;
    public const CHERRY_CEILING_CENTER_HANGING_SIGN = 10768;
    public const CHERRY_CEILING_EDGES_HANGING_SIGN = 10769;
    public const CHERRY_WALL_HANGING_SIGN = 10770;
    public const CRIMSON_CEILING_CENTER_HANGING_SIGN = 10771;
    public const CRIMSON_CEILING_EDGES_HANGING_SIGN = 10772;
    public const CRIMSON_WALL_HANGING_SIGN = 10773;
    public const DARK_OAK_CEILING_CENTER_HANGING_SIGN = 10774;
    public const DARK_OAK_CEILING_EDGES_HANGING_SIGN = 10775;
    public const DARK_OAK_WALL_HANGING_SIGN = 10776;
    public const JUNGLE_CEILING_CENTER_HANGING_SIGN = 10777;
    public const JUNGLE_CEILING_EDGES_HANGING_SIGN = 10778;
    public const JUNGLE_WALL_HANGING_SIGN = 10779;
    public const MANGROVE_CEILING_CENTER_HANGING_SIGN = 10780;
    public const MANGROVE_CEILING_EDGES_HANGING_SIGN = 10781;
    public const MANGROVE_WALL_HANGING_SIGN = 10782;
    public const OAK_CEILING_CENTER_HANGING_SIGN = 10783;
    public const OAK_CEILING_EDGES_HANGING_SIGN = 10784;
    public const OAK_WALL_HANGING_SIGN = 10785;
    public const PALE_OAK_CEILING_CENTER_HANGING_SIGN = 10786;
    public const PALE_OAK_CEILING_EDGES_HANGING_SIGN = 10787;
    public const PALE_OAK_WALL_HANGING_SIGN = 10788;
    public const SPRUCE_CEILING_CENTER_HANGING_SIGN = 10789;
    public const SPRUCE_CEILING_EDGES_HANGING_SIGN = 10790;
    public const SPRUCE_WALL_HANGING_SIGN = 10791;
    public const WARPED_CEILING_CENTER_HANGING_SIGN = 10792;
    public const WARPED_CEILING_EDGES_HANGING_SIGN = 10793;
    public const WARPED_WALL_HANGING_SIGN = 10794;
    public const COPPER_BARS = 10795;
    public const COPPER_CHAIN = 10796;
    public const COPPER_LANTERN = 10797;
    public const COPPER_TORCH = 10798;
    public const CACTUS_FLOWER = 10799;
    public const CRIMSON_FUNGUS = 10800;
    public const CRIMSON_NYLIUM = 10801;
    public const NETHER_SPROUTS = 10802;
    public const WARPED_FUNGUS = 10803;
    public const WARPED_NYLIUM = 10804;
    public const INFESTED_DEEPSLATE = 10805;
    public const STRUCTURE_VOID = 10806;
    public const BAMBOO_BLOCK = 10807;
    public const BAMBOO_BUTTON = 10808;
    public const BAMBOO_CEILING_CENTER_HANGING_SIGN = 10809;
    public const BAMBOO_CEILING_EDGES_HANGING_SIGN = 10810;
    public const BAMBOO_DOOR = 10811;
    public const BAMBOO_FENCE = 10812;
    public const BAMBOO_FENCE_GATE = 10813;
    public const BAMBOO_PLANKS = 10814;
    public const BAMBOO_PRESSURE_PLATE = 10815;
    public const BAMBOO_SIGN = 10816;
    public const BAMBOO_SLAB = 10817;
    public const BAMBOO_STAIRS = 10818;
    public const BAMBOO_TRAPDOOR = 10819;
    public const BAMBOO_WALL_HANGING_SIGN = 10820;
    public const BAMBOO_WALL_SIGN = 10821;
    public const BAMBOO_MOSAIC = 10822;
    public const BAMBOO_MOSAIC_SLAB = 10823;
    public const BAMBOO_MOSAIC_STAIRS = 10824;
    public const AZALEA = 10825;
    public const FLOWERING_AZALEA = 10826;
    public const HEAVY_CORE = 10827;
    public const SULFUR = 10828;
    public const SULFUR_SLAB = 10829;
    public const SULFUR_STAIRS = 10830;
    public const SULFUR_WALL = 10831;
    public const CHISELED_SULFUR = 10832;
    public const POLISHED_SULFUR = 10833;
    public const POLISHED_SULFUR_SLAB = 10834;
    public const POLISHED_SULFUR_STAIRS = 10835;
    public const POLISHED_SULFUR_WALL = 10836;
    public const SULFUR_BRICKS = 10837;
    public const SULFUR_BRICK_SLAB = 10838;
    public const SULFUR_BRICK_STAIRS = 10839;
    public const SULFUR_BRICK_WALL = 10840;
    public const POTENT_SULFUR = 10841;
    public const WET_POTENT_SULFUR = 10842;
    public const CONTINUOUS_POTENT_SULFUR = 10843;
    public const CYCLING_POTENT_SULFUR = 10844;
    public const CINNABAR = 10845;
    public const CINNABAR_SLAB = 10846;
    public const CINNABAR_STAIRS = 10847;
    public const CINNABAR_WALL = 10848;
    public const CHISELED_CINNABAR = 10849;
    public const POLISHED_CINNABAR = 10850;
    public const POLISHED_CINNABAR_SLAB = 10851;
    public const POLISHED_CINNABAR_STAIRS = 10852;
    public const POLISHED_CINNABAR_WALL = 10853;
    public const CINNABAR_BRICKS = 10854;
    public const CINNABAR_BRICK_SLAB = 10855;
    public const CINNABAR_BRICK_STAIRS = 10856;
    public const CINNABAR_BRICK_WALL = 10857;
    public const FIRST_UNUSED_BLOCK_ID = 10858;

    private static int $nextDynamicId = self::FIRST_UNUSED_BLOCK_ID;

    /** @var array<string, int> Dynamic runtime cache */
    private static array $byName = [];

    /** @var array<string, int> Fast lookup for all known names and common aliases */
    private static array $nameToId = [
        'air' => self::AIR,
        'acacia_button' => self::ACACIA_BUTTON,
        'acacia_door' => self::ACACIA_DOOR,
        'acacia_fence' => self::ACACIA_FENCE,
        'acacia_fence_gate' => self::ACACIA_FENCE_GATE,
        'acacia_leaves' => self::ACACIA_LEAVES,
        'acacia_log' => self::ACACIA_LOG,
        'acacia_planks' => self::ACACIA_PLANKS,
        'acacia_pressure_plate' => self::ACACIA_PRESSURE_PLATE,
        'acacia_sapling' => self::ACACIA_SAPLING,
        'acacia_sign' => self::ACACIA_SIGN,
        'acacia_slab' => self::ACACIA_SLAB,
        'acacia_stairs' => self::ACACIA_STAIRS,
        'acacia_trapdoor' => self::ACACIA_TRAPDOOR,
        'acacia_wall_sign' => self::ACACIA_WALL_SIGN,
        'acacia_wood' => self::ACACIA_WOOD,
        'activator_rail' => self::ACTIVATOR_RAIL,
        'all_sided_mushroom_stem' => self::ALL_SIDED_MUSHROOM_STEM,
        'allium' => self::ALLIUM,
        'andesite' => self::ANDESITE,
        'andesite_slab' => self::ANDESITE_SLAB,
        'andesite_stairs' => self::ANDESITE_STAIRS,
        'andesite_wall' => self::ANDESITE_WALL,
        'anvil' => self::ANVIL,
        'azure_bluet' => self::AZURE_BLUET,
        'bamboo' => self::BAMBOO,
        'bamboo_sapling' => self::BAMBOO_SAPLING,
        'banner' => self::BANNER,
        'barrel' => self::BARREL,
        'barrier' => self::BARRIER,
        'beacon' => self::BEACON,
        'bed' => self::BED,
        'bedrock' => self::BEDROCK,
        'beetroots' => self::BEETROOTS,
        'bell' => self::BELL,
        'birch_button' => self::BIRCH_BUTTON,
        'birch_door' => self::BIRCH_DOOR,
        'birch_fence' => self::BIRCH_FENCE,
        'birch_fence_gate' => self::BIRCH_FENCE_GATE,
        'birch_leaves' => self::BIRCH_LEAVES,
        'birch_log' => self::BIRCH_LOG,
        'birch_planks' => self::BIRCH_PLANKS,
        'birch_pressure_plate' => self::BIRCH_PRESSURE_PLATE,
        'birch_sapling' => self::BIRCH_SAPLING,
        'birch_sign' => self::BIRCH_SIGN,
        'birch_slab' => self::BIRCH_SLAB,
        'birch_stairs' => self::BIRCH_STAIRS,
        'birch_trapdoor' => self::BIRCH_TRAPDOOR,
        'birch_wall_sign' => self::BIRCH_WALL_SIGN,
        'birch_wood' => self::BIRCH_WOOD,
        'blast_furnace' => self::BLAST_FURNACE,
        'blue_ice' => self::BLUE_ICE,
        'blue_orchid' => self::BLUE_ORCHID,
        'blue_torch' => self::BLUE_TORCH,
        'bone_block' => self::BONE_BLOCK,
        'bookshelf' => self::BOOKSHELF,
        'brewing_stand' => self::BREWING_STAND,
        'brick_slab' => self::BRICK_SLAB,
        'brick_stairs' => self::BRICK_STAIRS,
        'brick_wall' => self::BRICK_WALL,
        'bricks' => self::BRICKS,
        'brown_mushroom' => self::BROWN_MUSHROOM,
        'brown_mushroom_block' => self::BROWN_MUSHROOM_BLOCK,
        'cactus' => self::CACTUS,
        'cake' => self::CAKE,
        'carpet' => self::CARPET,
        'carrots' => self::CARROTS,
        'carved_pumpkin' => self::CARVED_PUMPKIN,
        'chemical_heat' => self::CHEMICAL_HEAT,
        'chest' => self::CHEST,
        'chiseled_quartz' => self::CHISELED_QUARTZ,
        'chiseled_red_sandstone' => self::CHISELED_RED_SANDSTONE,
        'chiseled_sandstone' => self::CHISELED_SANDSTONE,
        'chiseled_stone_bricks' => self::CHISELED_STONE_BRICKS,
        'clay' => self::CLAY,
        'coal' => self::COAL,
        'coal_ore' => self::COAL_ORE,
        'cobblestone' => self::COBBLESTONE,
        'cobblestone_slab' => self::COBBLESTONE_SLAB,
        'cobblestone_stairs' => self::COBBLESTONE_STAIRS,
        'cobblestone_wall' => self::COBBLESTONE_WALL,
        'cobweb' => self::COBWEB,
        'cocoa_pod' => self::COCOA_POD,
        'compound_creator' => self::COMPOUND_CREATOR,
        'concrete' => self::CONCRETE,
        'concrete_powder' => self::CONCRETE_POWDER,
        'coral' => self::CORAL,
        'coral_block' => self::CORAL_BLOCK,
        'coral_fan' => self::CORAL_FAN,
        'cornflower' => self::CORNFLOWER,
        'cracked_stone_bricks' => self::CRACKED_STONE_BRICKS,
        'crafting_table' => self::CRAFTING_TABLE,
        'cut_red_sandstone' => self::CUT_RED_SANDSTONE,
        'cut_red_sandstone_slab' => self::CUT_RED_SANDSTONE_SLAB,
        'cut_sandstone' => self::CUT_SANDSTONE,
        'cut_sandstone_slab' => self::CUT_SANDSTONE_SLAB,
        'dandelion' => self::DANDELION,
        'dark_oak_button' => self::DARK_OAK_BUTTON,
        'dark_oak_door' => self::DARK_OAK_DOOR,
        'dark_oak_fence' => self::DARK_OAK_FENCE,
        'dark_oak_fence_gate' => self::DARK_OAK_FENCE_GATE,
        'dark_oak_leaves' => self::DARK_OAK_LEAVES,
        'dark_oak_log' => self::DARK_OAK_LOG,
        'dark_oak_planks' => self::DARK_OAK_PLANKS,
        'dark_oak_pressure_plate' => self::DARK_OAK_PRESSURE_PLATE,
        'dark_oak_sapling' => self::DARK_OAK_SAPLING,
        'dark_oak_sign' => self::DARK_OAK_SIGN,
        'dark_oak_slab' => self::DARK_OAK_SLAB,
        'dark_oak_stairs' => self::DARK_OAK_STAIRS,
        'dark_oak_trapdoor' => self::DARK_OAK_TRAPDOOR,
        'dark_oak_wall_sign' => self::DARK_OAK_WALL_SIGN,
        'dark_oak_wood' => self::DARK_OAK_WOOD,
        'dark_prismarine' => self::DARK_PRISMARINE,
        'dark_prismarine_slab' => self::DARK_PRISMARINE_SLAB,
        'dark_prismarine_stairs' => self::DARK_PRISMARINE_STAIRS,
        'daylight_sensor' => self::DAYLIGHT_SENSOR,
        'dead_bush' => self::DEAD_BUSH,
        'detector_rail' => self::DETECTOR_RAIL,
        'diamond' => self::DIAMOND,
        'diamond_ore' => self::DIAMOND_ORE,
        'diorite' => self::DIORITE,
        'diorite_slab' => self::DIORITE_SLAB,
        'diorite_stairs' => self::DIORITE_STAIRS,
        'diorite_wall' => self::DIORITE_WALL,
        'dirt' => self::DIRT,
        'double_tallgrass' => self::DOUBLE_TALLGRASS,
        'dragon_egg' => self::DRAGON_EGG,
        'dried_kelp' => self::DRIED_KELP,
        'dyed_shulker_box' => self::DYED_SHULKER_BOX,
        'element_actinium' => self::ELEMENT_ACTINIUM,
        'element_aluminum' => self::ELEMENT_ALUMINUM,
        'element_americium' => self::ELEMENT_AMERICIUM,
        'element_antimony' => self::ELEMENT_ANTIMONY,
        'element_argon' => self::ELEMENT_ARGON,
        'element_arsenic' => self::ELEMENT_ARSENIC,
        'element_astatine' => self::ELEMENT_ASTATINE,
        'element_barium' => self::ELEMENT_BARIUM,
        'element_berkelium' => self::ELEMENT_BERKELIUM,
        'element_beryllium' => self::ELEMENT_BERYLLIUM,
        'element_bismuth' => self::ELEMENT_BISMUTH,
        'element_bohrium' => self::ELEMENT_BOHRIUM,
        'element_boron' => self::ELEMENT_BORON,
        'element_bromine' => self::ELEMENT_BROMINE,
        'element_cadmium' => self::ELEMENT_CADMIUM,
        'element_calcium' => self::ELEMENT_CALCIUM,
        'element_californium' => self::ELEMENT_CALIFORNIUM,
        'element_carbon' => self::ELEMENT_CARBON,
        'element_cerium' => self::ELEMENT_CERIUM,
        'element_cesium' => self::ELEMENT_CESIUM,
        'element_chlorine' => self::ELEMENT_CHLORINE,
        'element_chromium' => self::ELEMENT_CHROMIUM,
        'element_cobalt' => self::ELEMENT_COBALT,
        'element_constructor' => self::ELEMENT_CONSTRUCTOR,
        'element_copernicium' => self::ELEMENT_COPERNICIUM,
        'element_copper' => self::ELEMENT_COPPER,
        'element_curium' => self::ELEMENT_CURIUM,
        'element_darmstadtium' => self::ELEMENT_DARMSTADTIUM,
        'element_dubnium' => self::ELEMENT_DUBNIUM,
        'element_dysprosium' => self::ELEMENT_DYSPROSIUM,
        'element_einsteinium' => self::ELEMENT_EINSTEINIUM,
        'element_erbium' => self::ELEMENT_ERBIUM,
        'element_europium' => self::ELEMENT_EUROPIUM,
        'element_fermium' => self::ELEMENT_FERMIUM,
        'element_flerovium' => self::ELEMENT_FLEROVIUM,
        'element_fluorine' => self::ELEMENT_FLUORINE,
        'element_francium' => self::ELEMENT_FRANCIUM,
        'element_gadolinium' => self::ELEMENT_GADOLINIUM,
        'element_gallium' => self::ELEMENT_GALLIUM,
        'element_germanium' => self::ELEMENT_GERMANIUM,
        'element_gold' => self::ELEMENT_GOLD,
        'element_hafnium' => self::ELEMENT_HAFNIUM,
        'element_hassium' => self::ELEMENT_HASSIUM,
        'element_helium' => self::ELEMENT_HELIUM,
        'element_holmium' => self::ELEMENT_HOLMIUM,
        'element_hydrogen' => self::ELEMENT_HYDROGEN,
        'element_indium' => self::ELEMENT_INDIUM,
        'element_iodine' => self::ELEMENT_IODINE,
        'element_iridium' => self::ELEMENT_IRIDIUM,
        'element_iron' => self::ELEMENT_IRON,
        'element_krypton' => self::ELEMENT_KRYPTON,
        'element_lanthanum' => self::ELEMENT_LANTHANUM,
        'element_lawrencium' => self::ELEMENT_LAWRENCIUM,
        'element_lead' => self::ELEMENT_LEAD,
        'element_lithium' => self::ELEMENT_LITHIUM,
        'element_livermorium' => self::ELEMENT_LIVERMORIUM,
        'element_lutetium' => self::ELEMENT_LUTETIUM,
        'element_magnesium' => self::ELEMENT_MAGNESIUM,
        'element_manganese' => self::ELEMENT_MANGANESE,
        'element_meitnerium' => self::ELEMENT_MEITNERIUM,
        'element_mendelevium' => self::ELEMENT_MENDELEVIUM,
        'element_mercury' => self::ELEMENT_MERCURY,
        'element_molybdenum' => self::ELEMENT_MOLYBDENUM,
        'element_moscovium' => self::ELEMENT_MOSCOVIUM,
        'element_neodymium' => self::ELEMENT_NEODYMIUM,
        'element_neon' => self::ELEMENT_NEON,
        'element_neptunium' => self::ELEMENT_NEPTUNIUM,
        'element_nickel' => self::ELEMENT_NICKEL,
        'element_nihonium' => self::ELEMENT_NIHONIUM,
        'element_niobium' => self::ELEMENT_NIOBIUM,
        'element_nitrogen' => self::ELEMENT_NITROGEN,
        'element_nobelium' => self::ELEMENT_NOBELIUM,
        'element_oganesson' => self::ELEMENT_OGANESSON,
        'element_osmium' => self::ELEMENT_OSMIUM,
        'element_oxygen' => self::ELEMENT_OXYGEN,
        'element_palladium' => self::ELEMENT_PALLADIUM,
        'element_phosphorus' => self::ELEMENT_PHOSPHORUS,
        'element_platinum' => self::ELEMENT_PLATINUM,
        'element_plutonium' => self::ELEMENT_PLUTONIUM,
        'element_polonium' => self::ELEMENT_POLONIUM,
        'element_potassium' => self::ELEMENT_POTASSIUM,
        'element_praseodymium' => self::ELEMENT_PRASEODYMIUM,
        'element_promethium' => self::ELEMENT_PROMETHIUM,
        'element_protactinium' => self::ELEMENT_PROTACTINIUM,
        'element_radium' => self::ELEMENT_RADIUM,
        'element_radon' => self::ELEMENT_RADON,
        'element_rhenium' => self::ELEMENT_RHENIUM,
        'element_rhodium' => self::ELEMENT_RHODIUM,
        'element_roentgenium' => self::ELEMENT_ROENTGENIUM,
        'element_rubidium' => self::ELEMENT_RUBIDIUM,
        'element_ruthenium' => self::ELEMENT_RUTHENIUM,
        'element_rutherfordium' => self::ELEMENT_RUTHERFORDIUM,
        'element_samarium' => self::ELEMENT_SAMARIUM,
        'element_scandium' => self::ELEMENT_SCANDIUM,
        'element_seaborgium' => self::ELEMENT_SEABORGIUM,
        'element_selenium' => self::ELEMENT_SELENIUM,
        'element_silicon' => self::ELEMENT_SILICON,
        'element_silver' => self::ELEMENT_SILVER,
        'element_sodium' => self::ELEMENT_SODIUM,
        'element_strontium' => self::ELEMENT_STRONTIUM,
        'element_sulfur' => self::ELEMENT_SULFUR,
        'element_tantalum' => self::ELEMENT_TANTALUM,
        'element_technetium' => self::ELEMENT_TECHNETIUM,
        'element_tellurium' => self::ELEMENT_TELLURIUM,
        'element_tennessine' => self::ELEMENT_TENNESSINE,
        'element_terbium' => self::ELEMENT_TERBIUM,
        'element_thallium' => self::ELEMENT_THALLIUM,
        'element_thorium' => self::ELEMENT_THORIUM,
        'element_thulium' => self::ELEMENT_THULIUM,
        'element_tin' => self::ELEMENT_TIN,
        'element_titanium' => self::ELEMENT_TITANIUM,
        'element_tungsten' => self::ELEMENT_TUNGSTEN,
        'element_uranium' => self::ELEMENT_URANIUM,
        'element_vanadium' => self::ELEMENT_VANADIUM,
        'element_xenon' => self::ELEMENT_XENON,
        'element_ytterbium' => self::ELEMENT_YTTERBIUM,
        'element_yttrium' => self::ELEMENT_YTTRIUM,
        'element_zero' => self::ELEMENT_ZERO,
        'element_zinc' => self::ELEMENT_ZINC,
        'element_zirconium' => self::ELEMENT_ZIRCONIUM,
        'emerald' => self::EMERALD,
        'emerald_ore' => self::EMERALD_ORE,
        'enchanting_table' => self::ENCHANTING_TABLE,
        'end_portal_frame' => self::END_PORTAL_FRAME,
        'end_rod' => self::END_ROD,
        'end_stone' => self::END_STONE,
        'end_stone_brick_slab' => self::END_STONE_BRICK_SLAB,
        'end_stone_brick_stairs' => self::END_STONE_BRICK_STAIRS,
        'end_stone_brick_wall' => self::END_STONE_BRICK_WALL,
        'end_stone_bricks' => self::END_STONE_BRICKS,
        'ender_chest' => self::ENDER_CHEST,
        'fake_wooden_slab' => self::FAKE_WOODEN_SLAB,
        'farmland' => self::FARMLAND,
        'fern' => self::FERN,
        'fire' => self::FIRE,
        'fletching_table' => self::FLETCHING_TABLE,
        'flower_pot' => self::FLOWER_POT,
        'frosted_ice' => self::FROSTED_ICE,
        'furnace' => self::FURNACE,
        'glass' => self::GLASS,
        'glass_pane' => self::GLASS_PANE,
        'glowing_obsidian' => self::GLOWING_OBSIDIAN,
        'glowstone' => self::GLOWSTONE,
        'gold' => self::GOLD,
        'gold_ore' => self::GOLD_ORE,
        'granite' => self::GRANITE,
        'granite_slab' => self::GRANITE_SLAB,
        'granite_stairs' => self::GRANITE_STAIRS,
        'granite_wall' => self::GRANITE_WALL,
        'grass' => self::GRASS,
        'grass_path' => self::GRASS_PATH,
        'gravel' => self::GRAVEL,
        'green_torch' => self::GREEN_TORCH,
        'hardened_clay' => self::HARDENED_CLAY,
        'hardened_glass' => self::HARDENED_GLASS,
        'hardened_glass_pane' => self::HARDENED_GLASS_PANE,
        'hay_bale' => self::HAY_BALE,
        'hopper' => self::HOPPER,
        'ice' => self::ICE,
        'infested_chiseled_stone_brick' => self::INFESTED_CHISELED_STONE_BRICK,
        'infested_cobblestone' => self::INFESTED_COBBLESTONE,
        'infested_cracked_stone_brick' => self::INFESTED_CRACKED_STONE_BRICK,
        'infested_mossy_stone_brick' => self::INFESTED_MOSSY_STONE_BRICK,
        'infested_stone' => self::INFESTED_STONE,
        'infested_stone_brick' => self::INFESTED_STONE_BRICK,
        'info_update' => self::INFO_UPDATE,
        'info_update2' => self::INFO_UPDATE2,
        'invisible_bedrock' => self::INVISIBLE_BEDROCK,
        'iron' => self::IRON,
        'iron_bars' => self::IRON_BARS,
        'iron_door' => self::IRON_DOOR,
        'iron_ore' => self::IRON_ORE,
        'iron_trapdoor' => self::IRON_TRAPDOOR,
        'item_frame' => self::ITEM_FRAME,
        'jukebox' => self::JUKEBOX,
        'jungle_button' => self::JUNGLE_BUTTON,
        'jungle_door' => self::JUNGLE_DOOR,
        'jungle_fence' => self::JUNGLE_FENCE,
        'jungle_fence_gate' => self::JUNGLE_FENCE_GATE,
        'jungle_leaves' => self::JUNGLE_LEAVES,
        'jungle_log' => self::JUNGLE_LOG,
        'jungle_planks' => self::JUNGLE_PLANKS,
        'jungle_pressure_plate' => self::JUNGLE_PRESSURE_PLATE,
        'jungle_sapling' => self::JUNGLE_SAPLING,
        'jungle_sign' => self::JUNGLE_SIGN,
        'jungle_slab' => self::JUNGLE_SLAB,
        'jungle_stairs' => self::JUNGLE_STAIRS,
        'jungle_trapdoor' => self::JUNGLE_TRAPDOOR,
        'jungle_wall_sign' => self::JUNGLE_WALL_SIGN,
        'jungle_wood' => self::JUNGLE_WOOD,
        'lab_table' => self::LAB_TABLE,
        'ladder' => self::LADDER,
        'lantern' => self::LANTERN,
        'lapis_lazuli' => self::LAPIS_LAZULI,
        'lapis_lazuli_ore' => self::LAPIS_LAZULI_ORE,
        'large_fern' => self::LARGE_FERN,
        'lava' => self::LAVA,
        'lectern' => self::LECTERN,
        'legacy_stonecutter' => self::LEGACY_STONECUTTER,
        'lever' => self::LEVER,
        'lilac' => self::LILAC,
        'lily_of_the_valley' => self::LILY_OF_THE_VALLEY,
        'lily_pad' => self::LILY_PAD,
        'lit_pumpkin' => self::LIT_PUMPKIN,
        'loom' => self::LOOM,
        'magma' => self::MAGMA,
        'material_reducer' => self::MATERIAL_REDUCER,
        'melon' => self::MELON,
        'melon_stem' => self::MELON_STEM,
        'mob_head' => self::MOB_HEAD,
        'monster_spawner' => self::MONSTER_SPAWNER,
        'mossy_cobblestone' => self::MOSSY_COBBLESTONE,
        'mossy_cobblestone_slab' => self::MOSSY_COBBLESTONE_SLAB,
        'mossy_cobblestone_stairs' => self::MOSSY_COBBLESTONE_STAIRS,
        'mossy_cobblestone_wall' => self::MOSSY_COBBLESTONE_WALL,
        'mossy_stone_brick_slab' => self::MOSSY_STONE_BRICK_SLAB,
        'mossy_stone_brick_stairs' => self::MOSSY_STONE_BRICK_STAIRS,
        'mossy_stone_brick_wall' => self::MOSSY_STONE_BRICK_WALL,
        'mossy_stone_bricks' => self::MOSSY_STONE_BRICKS,
        'mushroom_stem' => self::MUSHROOM_STEM,
        'mycelium' => self::MYCELIUM,
        'nether_brick_fence' => self::NETHER_BRICK_FENCE,
        'nether_brick_slab' => self::NETHER_BRICK_SLAB,
        'nether_brick_stairs' => self::NETHER_BRICK_STAIRS,
        'nether_brick_wall' => self::NETHER_BRICK_WALL,
        'nether_bricks' => self::NETHER_BRICKS,
        'nether_portal' => self::NETHER_PORTAL,
        'nether_quartz_ore' => self::NETHER_QUARTZ_ORE,
        'nether_reactor_core' => self::NETHER_REACTOR_CORE,
        'nether_wart' => self::NETHER_WART,
        'nether_wart_block' => self::NETHER_WART_BLOCK,
        'netherrack' => self::NETHERRACK,
        'note_block' => self::NOTE_BLOCK,
        'oak_button' => self::OAK_BUTTON,
        'oak_door' => self::OAK_DOOR,
        'oak_fence' => self::OAK_FENCE,
        'oak_fence_gate' => self::OAK_FENCE_GATE,
        'oak_leaves' => self::OAK_LEAVES,
        'oak_log' => self::OAK_LOG,
        'oak_planks' => self::OAK_PLANKS,
        'oak_pressure_plate' => self::OAK_PRESSURE_PLATE,
        'oak_sapling' => self::OAK_SAPLING,
        'oak_sign' => self::OAK_SIGN,
        'oak_slab' => self::OAK_SLAB,
        'oak_stairs' => self::OAK_STAIRS,
        'oak_trapdoor' => self::OAK_TRAPDOOR,
        'oak_wall_sign' => self::OAK_WALL_SIGN,
        'oak_wood' => self::OAK_WOOD,
        'obsidian' => self::OBSIDIAN,
        'orange_tulip' => self::ORANGE_TULIP,
        'oxeye_daisy' => self::OXEYE_DAISY,
        'packed_ice' => self::PACKED_ICE,
        'peony' => self::PEONY,
        'pink_tulip' => self::PINK_TULIP,
        'podzol' => self::PODZOL,
        'polished_andesite' => self::POLISHED_ANDESITE,
        'polished_andesite_slab' => self::POLISHED_ANDESITE_SLAB,
        'polished_andesite_stairs' => self::POLISHED_ANDESITE_STAIRS,
        'polished_diorite' => self::POLISHED_DIORITE,
        'polished_diorite_slab' => self::POLISHED_DIORITE_SLAB,
        'polished_diorite_stairs' => self::POLISHED_DIORITE_STAIRS,
        'polished_granite' => self::POLISHED_GRANITE,
        'polished_granite_slab' => self::POLISHED_GRANITE_SLAB,
        'polished_granite_stairs' => self::POLISHED_GRANITE_STAIRS,
        'poppy' => self::POPPY,
        'potatoes' => self::POTATOES,
        'powered_rail' => self::POWERED_RAIL,
        'prismarine' => self::PRISMARINE,
        'prismarine_bricks' => self::PRISMARINE_BRICKS,
        'prismarine_bricks_slab' => self::PRISMARINE_BRICKS_SLAB,
        'prismarine_bricks_stairs' => self::PRISMARINE_BRICKS_STAIRS,
        'prismarine_slab' => self::PRISMARINE_SLAB,
        'prismarine_stairs' => self::PRISMARINE_STAIRS,
        'prismarine_wall' => self::PRISMARINE_WALL,
        'pumpkin' => self::PUMPKIN,
        'pumpkin_stem' => self::PUMPKIN_STEM,
        'purple_torch' => self::PURPLE_TORCH,
        'purpur' => self::PURPUR,
        'purpur_pillar' => self::PURPUR_PILLAR,
        'purpur_slab' => self::PURPUR_SLAB,
        'purpur_stairs' => self::PURPUR_STAIRS,
        'quartz' => self::QUARTZ,
        'quartz_pillar' => self::QUARTZ_PILLAR,
        'quartz_slab' => self::QUARTZ_SLAB,
        'quartz_stairs' => self::QUARTZ_STAIRS,
        'rail' => self::RAIL,
        'red_mushroom' => self::RED_MUSHROOM,
        'red_mushroom_block' => self::RED_MUSHROOM_BLOCK,
        'red_nether_brick_slab' => self::RED_NETHER_BRICK_SLAB,
        'red_nether_brick_stairs' => self::RED_NETHER_BRICK_STAIRS,
        'red_nether_brick_wall' => self::RED_NETHER_BRICK_WALL,
        'red_nether_bricks' => self::RED_NETHER_BRICKS,
        'red_sand' => self::RED_SAND,
        'red_sandstone' => self::RED_SANDSTONE,
        'red_sandstone_slab' => self::RED_SANDSTONE_SLAB,
        'red_sandstone_stairs' => self::RED_SANDSTONE_STAIRS,
        'red_sandstone_wall' => self::RED_SANDSTONE_WALL,
        'red_torch' => self::RED_TORCH,
        'red_tulip' => self::RED_TULIP,
        'redstone' => self::REDSTONE,
        'redstone_comparator' => self::REDSTONE_COMPARATOR,
        'redstone_lamp' => self::REDSTONE_LAMP,
        'redstone_ore' => self::REDSTONE_ORE,
        'redstone_repeater' => self::REDSTONE_REPEATER,
        'redstone_torch' => self::REDSTONE_TORCH,
        'redstone_wire' => self::REDSTONE_WIRE,
        'reserved6' => self::RESERVED6,
        'rose_bush' => self::ROSE_BUSH,
        'sand' => self::SAND,
        'sandstone' => self::SANDSTONE,
        'sandstone_slab' => self::SANDSTONE_SLAB,
        'sandstone_stairs' => self::SANDSTONE_STAIRS,
        'sandstone_wall' => self::SANDSTONE_WALL,
        'sea_lantern' => self::SEA_LANTERN,
        'sea_pickle' => self::SEA_PICKLE,
        'shulker_box' => self::SHULKER_BOX,
        'slime' => self::SLIME,
        'smoker' => self::SMOKER,
        'smooth_quartz' => self::SMOOTH_QUARTZ,
        'smooth_quartz_slab' => self::SMOOTH_QUARTZ_SLAB,
        'smooth_quartz_stairs' => self::SMOOTH_QUARTZ_STAIRS,
        'smooth_red_sandstone' => self::SMOOTH_RED_SANDSTONE,
        'smooth_red_sandstone_slab' => self::SMOOTH_RED_SANDSTONE_SLAB,
        'smooth_red_sandstone_stairs' => self::SMOOTH_RED_SANDSTONE_STAIRS,
        'smooth_sandstone' => self::SMOOTH_SANDSTONE,
        'smooth_sandstone_slab' => self::SMOOTH_SANDSTONE_SLAB,
        'smooth_sandstone_stairs' => self::SMOOTH_SANDSTONE_STAIRS,
        'smooth_stone' => self::SMOOTH_STONE,
        'smooth_stone_slab' => self::SMOOTH_STONE_SLAB,
        'snow' => self::SNOW,
        'snow_layer' => self::SNOW_LAYER,
        'soul_sand' => self::SOUL_SAND,
        'sponge' => self::SPONGE,
        'spruce_button' => self::SPRUCE_BUTTON,
        'spruce_door' => self::SPRUCE_DOOR,
        'spruce_fence' => self::SPRUCE_FENCE,
        'spruce_fence_gate' => self::SPRUCE_FENCE_GATE,
        'spruce_leaves' => self::SPRUCE_LEAVES,
        'spruce_log' => self::SPRUCE_LOG,
        'spruce_planks' => self::SPRUCE_PLANKS,
        'spruce_pressure_plate' => self::SPRUCE_PRESSURE_PLATE,
        'spruce_sapling' => self::SPRUCE_SAPLING,
        'spruce_sign' => self::SPRUCE_SIGN,
        'spruce_slab' => self::SPRUCE_SLAB,
        'spruce_stairs' => self::SPRUCE_STAIRS,
        'spruce_trapdoor' => self::SPRUCE_TRAPDOOR,
        'spruce_wall_sign' => self::SPRUCE_WALL_SIGN,
        'spruce_wood' => self::SPRUCE_WOOD,
        'stained_clay' => self::STAINED_CLAY,
        'stained_glass' => self::STAINED_GLASS,
        'stained_glass_pane' => self::STAINED_GLASS_PANE,
        'stained_hardened_glass' => self::STAINED_HARDENED_GLASS,
        'stained_hardened_glass_pane' => self::STAINED_HARDENED_GLASS_PANE,
        'stone' => self::STONE,
        'stone_brick_slab' => self::STONE_BRICK_SLAB,
        'stone_brick_stairs' => self::STONE_BRICK_STAIRS,
        'stone_brick_wall' => self::STONE_BRICK_WALL,
        'stone_bricks' => self::STONE_BRICKS,
        'stone_button' => self::STONE_BUTTON,
        'stone_pressure_plate' => self::STONE_PRESSURE_PLATE,
        'stone_slab' => self::STONE_SLAB,
        'stone_stairs' => self::STONE_STAIRS,
        'stonecutter' => self::STONECUTTER,
        'sugarcane' => self::SUGARCANE,
        'sunflower' => self::SUNFLOWER,
        'sweet_berry_bush' => self::SWEET_BERRY_BUSH,
        'tall_grass' => self::TALL_GRASS,
        'tnt' => self::TNT,
        'torch' => self::TORCH,
        'trapped_chest' => self::TRAPPED_CHEST,
        'tripwire' => self::TRIPWIRE,
        'tripwire_hook' => self::TRIPWIRE_HOOK,
        'underwater_torch' => self::UNDERWATER_TORCH,
        'vines' => self::VINES,
        'wall_banner' => self::WALL_BANNER,
        'wall_coral_fan' => self::WALL_CORAL_FAN,
        'water' => self::WATER,
        'weighted_pressure_plate_heavy' => self::WEIGHTED_PRESSURE_PLATE_HEAVY,
        'weighted_pressure_plate_light' => self::WEIGHTED_PRESSURE_PLATE_LIGHT,
        'wheat' => self::WHEAT,
        'budding_amethyst' => self::BUDDING_AMETHYST,
        'white_tulip' => self::WHITE_TULIP,
        'wool' => self::WOOL,
        'amethyst_cluster' => self::AMETHYST_CLUSTER,
        'glazed_terracotta' => self::GLAZED_TERRACOTTA,
        'amethyst' => self::AMETHYST,
        'ancient_debris' => self::ANCIENT_DEBRIS,
        'basalt' => self::BASALT,
        'polished_basalt' => self::POLISHED_BASALT,
        'smooth_basalt' => self::SMOOTH_BASALT,
        'blackstone' => self::BLACKSTONE,
        'blackstone_slab' => self::BLACKSTONE_SLAB,
        'blackstone_stairs' => self::BLACKSTONE_STAIRS,
        'blackstone_wall' => self::BLACKSTONE_WALL,
        'polished_blackstone' => self::POLISHED_BLACKSTONE,
        'polished_blackstone_button' => self::POLISHED_BLACKSTONE_BUTTON,
        'polished_blackstone_pressure_plate' => self::POLISHED_BLACKSTONE_PRESSURE_PLATE,
        'polished_blackstone_slab' => self::POLISHED_BLACKSTONE_SLAB,
        'polished_blackstone_stairs' => self::POLISHED_BLACKSTONE_STAIRS,
        'polished_blackstone_wall' => self::POLISHED_BLACKSTONE_WALL,
        'chiseled_polished_blackstone' => self::CHISELED_POLISHED_BLACKSTONE,
        'polished_blackstone_bricks' => self::POLISHED_BLACKSTONE_BRICKS,
        'polished_blackstone_brick_slab' => self::POLISHED_BLACKSTONE_BRICK_SLAB,
        'polished_blackstone_brick_stairs' => self::POLISHED_BLACKSTONE_BRICK_STAIRS,
        'polished_blackstone_brick_wall' => self::POLISHED_BLACKSTONE_BRICK_WALL,
        'cracked_polished_blackstone_bricks' => self::CRACKED_POLISHED_BLACKSTONE_BRICKS,
        'light' => self::LIGHT,
        'raw_copper' => self::RAW_COPPER,
        'raw_gold' => self::RAW_GOLD,
        'raw_iron' => self::RAW_IRON,
        'calcite' => self::CALCITE,
        'deepslate' => self::DEEPSLATE,
        'deepslate_bricks' => self::DEEPSLATE_BRICKS,
        'deepslate_brick_slab' => self::DEEPSLATE_BRICK_SLAB,
        'deepslate_brick_stairs' => self::DEEPSLATE_BRICK_STAIRS,
        'deepslate_brick_wall' => self::DEEPSLATE_BRICK_WALL,
        'cracked_deepslate_bricks' => self::CRACKED_DEEPSLATE_BRICKS,
        'deepslate_tiles' => self::DEEPSLATE_TILES,
        'deepslate_tile_slab' => self::DEEPSLATE_TILE_SLAB,
        'deepslate_tile_stairs' => self::DEEPSLATE_TILE_STAIRS,
        'deepslate_tile_wall' => self::DEEPSLATE_TILE_WALL,
        'cracked_deepslate_tiles' => self::CRACKED_DEEPSLATE_TILES,
        'cobbled_deepslate' => self::COBBLED_DEEPSLATE,
        'cobbled_deepslate_slab' => self::COBBLED_DEEPSLATE_SLAB,
        'cobbled_deepslate_stairs' => self::COBBLED_DEEPSLATE_STAIRS,
        'cobbled_deepslate_wall' => self::COBBLED_DEEPSLATE_WALL,
        'polished_deepslate' => self::POLISHED_DEEPSLATE,
        'polished_deepslate_slab' => self::POLISHED_DEEPSLATE_SLAB,
        'polished_deepslate_stairs' => self::POLISHED_DEEPSLATE_STAIRS,
        'polished_deepslate_wall' => self::POLISHED_DEEPSLATE_WALL,
        'quartz_bricks' => self::QUARTZ_BRICKS,
        'chiseled_deepslate' => self::CHISELED_DEEPSLATE,
        'chiseled_nether_bricks' => self::CHISELED_NETHER_BRICKS,
        'cracked_nether_bricks' => self::CRACKED_NETHER_BRICKS,
        'tuff' => self::TUFF,
        'soul_torch' => self::SOUL_TORCH,
        'soul_lantern' => self::SOUL_LANTERN,
        'soul_soil' => self::SOUL_SOIL,
        'soul_fire' => self::SOUL_FIRE,
        'shroomlight' => self::SHROOMLIGHT,
        'mangrove_planks' => self::MANGROVE_PLANKS,
        'crimson_planks' => self::CRIMSON_PLANKS,
        'warped_planks' => self::WARPED_PLANKS,
        'mangrove_fence' => self::MANGROVE_FENCE,
        'crimson_fence' => self::CRIMSON_FENCE,
        'warped_fence' => self::WARPED_FENCE,
        'mangrove_slab' => self::MANGROVE_SLAB,
        'crimson_slab' => self::CRIMSON_SLAB,
        'warped_slab' => self::WARPED_SLAB,
        'mangrove_log' => self::MANGROVE_LOG,
        'crimson_stem' => self::CRIMSON_STEM,
        'warped_stem' => self::WARPED_STEM,
        'mangrove_wood' => self::MANGROVE_WOOD,
        'crimson_hyphae' => self::CRIMSON_HYPHAE,
        'warped_hyphae' => self::WARPED_HYPHAE,
        'mangrove_trapdoor' => self::MANGROVE_TRAPDOOR,
        'crimson_trapdoor' => self::CRIMSON_TRAPDOOR,
        'warped_trapdoor' => self::WARPED_TRAPDOOR,
        'mangrove_button' => self::MANGROVE_BUTTON,
        'crimson_button' => self::CRIMSON_BUTTON,
        'warped_button' => self::WARPED_BUTTON,
        'mangrove_pressure_plate' => self::MANGROVE_PRESSURE_PLATE,
        'crimson_pressure_plate' => self::CRIMSON_PRESSURE_PLATE,
        'warped_pressure_plate' => self::WARPED_PRESSURE_PLATE,
        'mangrove_door' => self::MANGROVE_DOOR,
        'crimson_door' => self::CRIMSON_DOOR,
        'warped_door' => self::WARPED_DOOR,
        'mangrove_fence_gate' => self::MANGROVE_FENCE_GATE,
        'crimson_fence_gate' => self::CRIMSON_FENCE_GATE,
        'warped_fence_gate' => self::WARPED_FENCE_GATE,
        'mangrove_stairs' => self::MANGROVE_STAIRS,
        'crimson_stairs' => self::CRIMSON_STAIRS,
        'warped_stairs' => self::WARPED_STAIRS,
        'mangrove_sign' => self::MANGROVE_SIGN,
        'crimson_sign' => self::CRIMSON_SIGN,
        'warped_sign' => self::WARPED_SIGN,
        'mangrove_wall_sign' => self::MANGROVE_WALL_SIGN,
        'crimson_wall_sign' => self::CRIMSON_WALL_SIGN,
        'warped_wall_sign' => self::WARPED_WALL_SIGN,
        'tinted_glass' => self::TINTED_GLASS,
        'honeycomb' => self::HONEYCOMB,
        'deepslate_coal_ore' => self::DEEPSLATE_COAL_ORE,
        'deepslate_diamond_ore' => self::DEEPSLATE_DIAMOND_ORE,
        'deepslate_emerald_ore' => self::DEEPSLATE_EMERALD_ORE,
        'deepslate_lapis_lazuli_ore' => self::DEEPSLATE_LAPIS_LAZULI_ORE,
        'deepslate_redstone_ore' => self::DEEPSLATE_REDSTONE_ORE,
        'deepslate_iron_ore' => self::DEEPSLATE_IRON_ORE,
        'deepslate_gold_ore' => self::DEEPSLATE_GOLD_ORE,
        'deepslate_copper_ore' => self::DEEPSLATE_COPPER_ORE,
        'copper_ore' => self::COPPER_ORE,
        'nether_gold_ore' => self::NETHER_GOLD_ORE,
        'mud' => self::MUD,
        'mud_bricks' => self::MUD_BRICKS,
        'mud_brick_slab' => self::MUD_BRICK_SLAB,
        'mud_brick_stairs' => self::MUD_BRICK_STAIRS,
        'mud_brick_wall' => self::MUD_BRICK_WALL,
        'packed_mud' => self::PACKED_MUD,
        'warped_wart_block' => self::WARPED_WART_BLOCK,
        'crying_obsidian' => self::CRYING_OBSIDIAN,
        'gilded_blackstone' => self::GILDED_BLACKSTONE,
        'lightning_rod' => self::LIGHTNING_ROD,
        'copper' => self::COPPER,
        'cut_copper' => self::CUT_COPPER,
        'cut_copper_slab' => self::CUT_COPPER_SLAB,
        'cut_copper_stairs' => self::CUT_COPPER_STAIRS,
        'candle' => self::CANDLE,
        'dyed_candle' => self::DYED_CANDLE,
        'cake_with_candle' => self::CAKE_WITH_CANDLE,
        'cake_with_dyed_candle' => self::CAKE_WITH_DYED_CANDLE,
        'wither_rose' => self::WITHER_ROSE,
        'hanging_roots' => self::HANGING_ROOTS,
        'cartography_table' => self::CARTOGRAPHY_TABLE,
        'smithing_table' => self::SMITHING_TABLE,
        'netherite' => self::NETHERITE,
        'spore_blossom' => self::SPORE_BLOSSOM,
        'cauldron' => self::CAULDRON,
        'water_cauldron' => self::WATER_CAULDRON,
        'lava_cauldron' => self::LAVA_CAULDRON,
        'potion_cauldron' => self::POTION_CAULDRON,
        'powder_snow_cauldron' => self::POWDER_SNOW_CAULDRON,
        'chorus_flower' => self::CHORUS_FLOWER,
        'chorus_plant' => self::CHORUS_PLANT,
        'mangrove_roots' => self::MANGROVE_ROOTS,
        'muddy_mangrove_roots' => self::MUDDY_MANGROVE_ROOTS,
        'froglight' => self::FROGLIGHT,
        'twisting_vines' => self::TWISTING_VINES,
        'weeping_vines' => self::WEEPING_VINES,
        'chain' => self::CHAIN,
        'sculk' => self::SCULK,
        'glowing_item_frame' => self::GLOWING_ITEM_FRAME,
        'mangrove_leaves' => self::MANGROVE_LEAVES,
        'azalea_leaves' => self::AZALEA_LEAVES,
        'flowering_azalea_leaves' => self::FLOWERING_AZALEA_LEAVES,
        'reinforced_deepslate' => self::REINFORCED_DEEPSLATE,
        'cave_vines' => self::CAVE_VINES,
        'glow_lichen' => self::GLOW_LICHEN,
        'cherry_button' => self::CHERRY_BUTTON,
        'cherry_door' => self::CHERRY_DOOR,
        'cherry_fence' => self::CHERRY_FENCE,
        'cherry_fence_gate' => self::CHERRY_FENCE_GATE,
        'cherry_leaves' => self::CHERRY_LEAVES,
        'cherry_log' => self::CHERRY_LOG,
        'cherry_planks' => self::CHERRY_PLANKS,
        'cherry_pressure_plate' => self::CHERRY_PRESSURE_PLATE,
        'cherry_sapling' => self::CHERRY_SAPLING,
        'cherry_sign' => self::CHERRY_SIGN,
        'cherry_slab' => self::CHERRY_SLAB,
        'cherry_stairs' => self::CHERRY_STAIRS,
        'cherry_trapdoor' => self::CHERRY_TRAPDOOR,
        'cherry_wall_sign' => self::CHERRY_WALL_SIGN,
        'cherry_wood' => self::CHERRY_WOOD,
        'small_dripleaf' => self::SMALL_DRIPLEAF,
        'big_dripleaf_head' => self::BIG_DRIPLEAF_HEAD,
        'big_dripleaf_stem' => self::BIG_DRIPLEAF_STEM,
        'pink_petals' => self::PINK_PETALS,
        'crimson_roots' => self::CRIMSON_ROOTS,
        'warped_roots' => self::WARPED_ROOTS,
        'chiseled_bookshelf' => self::CHISELED_BOOKSHELF,
        'torchflower' => self::TORCHFLOWER,
        'torchflower_crop' => self::TORCHFLOWER_CROP,
        'pitcher_plant' => self::PITCHER_PLANT,
        'pitcher_crop' => self::PITCHER_CROP,
        'double_pitcher_crop' => self::DOUBLE_PITCHER_CROP,
        'campfire' => self::CAMPFIRE,
        'soul_campfire' => self::SOUL_CAMPFIRE,
        'tuff_slab' => self::TUFF_SLAB,
        'tuff_stairs' => self::TUFF_STAIRS,
        'tuff_wall' => self::TUFF_WALL,
        'chiseled_tuff' => self::CHISELED_TUFF,
        'tuff_bricks' => self::TUFF_BRICKS,
        'tuff_brick_slab' => self::TUFF_BRICK_SLAB,
        'tuff_brick_stairs' => self::TUFF_BRICK_STAIRS,
        'tuff_brick_wall' => self::TUFF_BRICK_WALL,
        'chiseled_tuff_bricks' => self::CHISELED_TUFF_BRICKS,
        'polished_tuff' => self::POLISHED_TUFF,
        'polished_tuff_slab' => self::POLISHED_TUFF_SLAB,
        'polished_tuff_stairs' => self::POLISHED_TUFF_STAIRS,
        'polished_tuff_wall' => self::POLISHED_TUFF_WALL,
        'copper_bulb' => self::COPPER_BULB,
        'copper_door' => self::COPPER_DOOR,
        'copper_trapdoor' => self::COPPER_TRAPDOOR,
        'chiseled_copper' => self::CHISELED_COPPER,
        'copper_grate' => self::COPPER_GRATE,
        'pale_oak_button' => self::PALE_OAK_BUTTON,
        'pale_oak_door' => self::PALE_OAK_DOOR,
        'pale_oak_fence' => self::PALE_OAK_FENCE,
        'pale_oak_fence_gate' => self::PALE_OAK_FENCE_GATE,
        'pale_oak_leaves' => self::PALE_OAK_LEAVES,
        'pale_oak_log' => self::PALE_OAK_LOG,
        'pale_oak_planks' => self::PALE_OAK_PLANKS,
        'pale_oak_pressure_plate' => self::PALE_OAK_PRESSURE_PLATE,
        'pale_oak_sign' => self::PALE_OAK_SIGN,
        'pale_oak_slab' => self::PALE_OAK_SLAB,
        'pale_oak_stairs' => self::PALE_OAK_STAIRS,
        'pale_oak_trapdoor' => self::PALE_OAK_TRAPDOOR,
        'pale_oak_wall_sign' => self::PALE_OAK_WALL_SIGN,
        'pale_oak_wood' => self::PALE_OAK_WOOD,
        'resin' => self::RESIN,
        'resin_brick_slab' => self::RESIN_BRICK_SLAB,
        'resin_brick_stairs' => self::RESIN_BRICK_STAIRS,
        'resin_brick_wall' => self::RESIN_BRICK_WALL,
        'resin_bricks' => self::RESIN_BRICKS,
        'resin_clump' => self::RESIN_CLUMP,
        'chiseled_resin_bricks' => self::CHISELED_RESIN_BRICKS,
        'respawn_anchor' => self::RESPAWN_ANCHOR,
        'ominous_banner' => self::OMINOUS_BANNER,
        'ominous_wall_banner' => self::OMINOUS_WALL_BANNER,
        'acacia_ceiling_center_hanging_sign' => self::ACACIA_CEILING_CENTER_HANGING_SIGN,
        'acacia_ceiling_edges_hanging_sign' => self::ACACIA_CEILING_EDGES_HANGING_SIGN,
        'acacia_wall_hanging_sign' => self::ACACIA_WALL_HANGING_SIGN,
        'birch_ceiling_center_hanging_sign' => self::BIRCH_CEILING_CENTER_HANGING_SIGN,
        'birch_ceiling_edges_hanging_sign' => self::BIRCH_CEILING_EDGES_HANGING_SIGN,
        'birch_wall_hanging_sign' => self::BIRCH_WALL_HANGING_SIGN,
        'cherry_ceiling_center_hanging_sign' => self::CHERRY_CEILING_CENTER_HANGING_SIGN,
        'cherry_ceiling_edges_hanging_sign' => self::CHERRY_CEILING_EDGES_HANGING_SIGN,
        'cherry_wall_hanging_sign' => self::CHERRY_WALL_HANGING_SIGN,
        'crimson_ceiling_center_hanging_sign' => self::CRIMSON_CEILING_CENTER_HANGING_SIGN,
        'crimson_ceiling_edges_hanging_sign' => self::CRIMSON_CEILING_EDGES_HANGING_SIGN,
        'crimson_wall_hanging_sign' => self::CRIMSON_WALL_HANGING_SIGN,
        'dark_oak_ceiling_center_hanging_sign' => self::DARK_OAK_CEILING_CENTER_HANGING_SIGN,
        'dark_oak_ceiling_edges_hanging_sign' => self::DARK_OAK_CEILING_EDGES_HANGING_SIGN,
        'dark_oak_wall_hanging_sign' => self::DARK_OAK_WALL_HANGING_SIGN,
        'jungle_ceiling_center_hanging_sign' => self::JUNGLE_CEILING_CENTER_HANGING_SIGN,
        'jungle_ceiling_edges_hanging_sign' => self::JUNGLE_CEILING_EDGES_HANGING_SIGN,
        'jungle_wall_hanging_sign' => self::JUNGLE_WALL_HANGING_SIGN,
        'mangrove_ceiling_center_hanging_sign' => self::MANGROVE_CEILING_CENTER_HANGING_SIGN,
        'mangrove_ceiling_edges_hanging_sign' => self::MANGROVE_CEILING_EDGES_HANGING_SIGN,
        'mangrove_wall_hanging_sign' => self::MANGROVE_WALL_HANGING_SIGN,
        'oak_ceiling_center_hanging_sign' => self::OAK_CEILING_CENTER_HANGING_SIGN,
        'oak_ceiling_edges_hanging_sign' => self::OAK_CEILING_EDGES_HANGING_SIGN,
        'oak_wall_hanging_sign' => self::OAK_WALL_HANGING_SIGN,
        'pale_oak_ceiling_center_hanging_sign' => self::PALE_OAK_CEILING_CENTER_HANGING_SIGN,
        'pale_oak_ceiling_edges_hanging_sign' => self::PALE_OAK_CEILING_EDGES_HANGING_SIGN,
        'pale_oak_wall_hanging_sign' => self::PALE_OAK_WALL_HANGING_SIGN,
        'spruce_ceiling_center_hanging_sign' => self::SPRUCE_CEILING_CENTER_HANGING_SIGN,
        'spruce_ceiling_edges_hanging_sign' => self::SPRUCE_CEILING_EDGES_HANGING_SIGN,
        'spruce_wall_hanging_sign' => self::SPRUCE_WALL_HANGING_SIGN,
        'warped_ceiling_center_hanging_sign' => self::WARPED_CEILING_CENTER_HANGING_SIGN,
        'warped_ceiling_edges_hanging_sign' => self::WARPED_CEILING_EDGES_HANGING_SIGN,
        'warped_wall_hanging_sign' => self::WARPED_WALL_HANGING_SIGN,
        'copper_bars' => self::COPPER_BARS,
        'copper_chain' => self::COPPER_CHAIN,
        'copper_lantern' => self::COPPER_LANTERN,
        'copper_torch' => self::COPPER_TORCH,
        'cactus_flower' => self::CACTUS_FLOWER,
        'crimson_fungus' => self::CRIMSON_FUNGUS,
        'crimson_nylium' => self::CRIMSON_NYLIUM,
        'nether_sprouts' => self::NETHER_SPROUTS,
        'warped_fungus' => self::WARPED_FUNGUS,
        'warped_nylium' => self::WARPED_NYLIUM,
        'infested_deepslate' => self::INFESTED_DEEPSLATE,
        'structure_void' => self::STRUCTURE_VOID,
        'bamboo_block' => self::BAMBOO_BLOCK,
        'bamboo_button' => self::BAMBOO_BUTTON,
        'bamboo_ceiling_center_hanging_sign' => self::BAMBOO_CEILING_CENTER_HANGING_SIGN,
        'bamboo_ceiling_edges_hanging_sign' => self::BAMBOO_CEILING_EDGES_HANGING_SIGN,
        'bamboo_door' => self::BAMBOO_DOOR,
        'bamboo_fence' => self::BAMBOO_FENCE,
        'bamboo_fence_gate' => self::BAMBOO_FENCE_GATE,
        'bamboo_planks' => self::BAMBOO_PLANKS,
        'bamboo_pressure_plate' => self::BAMBOO_PRESSURE_PLATE,
        'bamboo_sign' => self::BAMBOO_SIGN,
        'bamboo_slab' => self::BAMBOO_SLAB,
        'bamboo_stairs' => self::BAMBOO_STAIRS,
        'bamboo_trapdoor' => self::BAMBOO_TRAPDOOR,
        'bamboo_wall_hanging_sign' => self::BAMBOO_WALL_HANGING_SIGN,
        'bamboo_wall_sign' => self::BAMBOO_WALL_SIGN,
        'bamboo_mosaic' => self::BAMBOO_MOSAIC,
        'bamboo_mosaic_slab' => self::BAMBOO_MOSAIC_SLAB,
        'bamboo_mosaic_stairs' => self::BAMBOO_MOSAIC_STAIRS,
        'azalea' => self::AZALEA,
        'flowering_azalea' => self::FLOWERING_AZALEA,
        'heavy_core' => self::HEAVY_CORE,
        'sulfur' => self::SULFUR,
        'sulfur_slab' => self::SULFUR_SLAB,
        'sulfur_stairs' => self::SULFUR_STAIRS,
        'sulfur_wall' => self::SULFUR_WALL,
        'chiseled_sulfur' => self::CHISELED_SULFUR,
        'polished_sulfur' => self::POLISHED_SULFUR,
        'polished_sulfur_slab' => self::POLISHED_SULFUR_SLAB,
        'polished_sulfur_stairs' => self::POLISHED_SULFUR_STAIRS,
        'polished_sulfur_wall' => self::POLISHED_SULFUR_WALL,
        'sulfur_bricks' => self::SULFUR_BRICKS,
        'sulfur_brick_slab' => self::SULFUR_BRICK_SLAB,
        'sulfur_brick_stairs' => self::SULFUR_BRICK_STAIRS,
        'sulfur_brick_wall' => self::SULFUR_BRICK_WALL,
        'potent_sulfur' => self::POTENT_SULFUR,
        'wet_potent_sulfur' => self::WET_POTENT_SULFUR,
        'continuous_potent_sulfur' => self::CONTINUOUS_POTENT_SULFUR,
        'cycling_potent_sulfur' => self::CYCLING_POTENT_SULFUR,
        'cinnabar' => self::CINNABAR,
        'cinnabar_slab' => self::CINNABAR_SLAB,
        'cinnabar_stairs' => self::CINNABAR_STAIRS,
        'cinnabar_wall' => self::CINNABAR_WALL,
        'chiseled_cinnabar' => self::CHISELED_CINNABAR,
        'polished_cinnabar' => self::POLISHED_CINNABAR,
        'polished_cinnabar_slab' => self::POLISHED_CINNABAR_SLAB,
        'polished_cinnabar_stairs' => self::POLISHED_CINNABAR_STAIRS,
        'polished_cinnabar_wall' => self::POLISHED_CINNABAR_WALL,
        'cinnabar_bricks' => self::CINNABAR_BRICKS,
        'cinnabar_brick_slab' => self::CINNABAR_BRICK_SLAB,
        'cinnabar_brick_stairs' => self::CINNABAR_BRICK_STAIRS,
        'cinnabar_brick_wall' => self::CINNABAR_BRICK_WALL,
        'brick' => self::BRICKS,
        'brick_block' => self::BRICKS,
        'button' => self::OAK_BUTTON,
        'clay_block' => self::CLAY,
        'coal_block' => self::COAL,
        'cobble' => self::COBBLESTONE,
        'copper_block' => self::COPPER,
        'diamond_block' => self::DIAMOND,
        'dirt_path' => self::GRASS_PATH,
        'door' => self::OAK_DOOR,
        'double_slab' => self::STONE_SLAB,
        'emerald_block' => self::EMERALD,
        'end_stone_brick' => self::END_STONE_BRICKS,
        'fence' => self::OAK_FENCE,
        'fence_gate' => self::OAK_FENCE_GATE,
        'flowing_lava' => self::LAVA,
        'flowing_water' => self::WATER,
        'gold_block' => self::GOLD,
        'grass_block' => self::GRASS,
        'iron_block' => self::IRON,
        'jack_o_lantern' => self::LIT_PUMPKIN,
        'lapis' => self::LAPIS_LAZULI,
        'lapis_block' => self::LAPIS_LAZULI,
        'lapis_lazuli_block' => self::LAPIS_LAZULI,
        'leaves' => self::OAK_LEAVES,
        'log' => self::OAK_LOG,
        'melon_block' => self::MELON,
        'mossy_cobble' => self::MOSSY_COBBLESTONE,
        'netherite_block' => self::NETHERITE,
        'nether_brick' => self::NETHER_BRICKS,
        'nether_brick_block' => self::NETHER_BRICKS,
        'noteblock' => self::NOTE_BLOCK,
        'planks' => self::OAK_PLANKS,
        'pressure_plate' => self::OAK_PRESSURE_PLATE,
        'prismarine_brick' => self::PRISMARINE_BRICKS,
        'quartz_block' => self::QUARTZ,
        'redstone_block' => self::REDSTONE,
        'sapling' => self::OAK_SAPLING,
        'sign' => self::OAK_SIGN,
        'slab' => self::STONE_SLAB,
        'snow_block' => self::SNOW,
        'sponge_wet' => self::SPONGE,
        'stained_hardened_clay' => self::STAINED_CLAY,
        'standing_sign' => self::OAK_SIGN,
        'stonebrick' => self::STONE_BRICKS,
        'stone_brick' => self::STONE_BRICKS,
        'terracotta' => self::HARDENED_CLAY,
        'trapdoor' => self::OAK_TRAPDOOR,
        'wall_sign' => self::OAK_WALL_SIGN,
        'white_wool' => self::WOOL,
        'wooden_button' => self::OAK_BUTTON,
        'wooden_door' => self::OAK_DOOR,
        'wooden_pressure_plate' => self::OAK_PRESSURE_PLATE,
        'wooden_slab' => self::OAK_SLAB,
        'wooden_stairs' => self::OAK_STAIRS,
        'wooden_trapdoor' => self::OAK_TRAPDOOR,
        'workbench' => self::CRAFTING_TABLE,
    ];

    private function __construct()
    {
    }

    /**
     * Returns a new runtime block type ID, e.g. for use by a custom block.
     */
    public static function newId(): int
    {
        return self::$nextDynamicId++;
    }

    /**
     * Resolves a block type ID by name (case-insensitive, optional minecraft: prefix).
     * If unknown, registers and returns a dynamic ID.
     */
    public static function forName(string $name): int
    {
        $key = strtolower(str_replace(['minecraft:', ' '], ['', '_'], $name));
        if (isset(self::$byName[$key])) {
            return self::$byName[$key];
        }
        if (isset(self::$nameToId[$key])) {
            return self::$byName[$key] = self::$nameToId[$key];
        }
        return self::$byName[$key] = self::newId();
    }
}

/**
 * Allows getting a fresh instance of any block implemented by PocketMine-MP.
 * Every block here also has a constant of the same name in BlockTypeIds.
 */
final class VanillaBlocks
{
    /** @var array<string, Block> Cached block prototypes */
    private static array $blocks = [];

    /** @var array<string, string> Legacy and alternative aliases (uppercase => canonical uppercase) */
    private const ALIASES = [
        'BRICK' => 'BRICKS',
        'BRICK_BLOCK' => 'BRICKS',
        'BUTTON' => 'OAK_BUTTON',
        'CLAY_BLOCK' => 'CLAY',
        'COAL_BLOCK' => 'COAL',
        'COBBLE' => 'COBBLESTONE',
        'COPPER_BLOCK' => 'COPPER',
        'DIAMOND_BLOCK' => 'DIAMOND',
        'DIRT_PATH' => 'GRASS_PATH',
        'DOOR' => 'OAK_DOOR',
        'DOUBLE_SLAB' => 'STONE_SLAB',
        'EMERALD_BLOCK' => 'EMERALD',
        'END_STONE_BRICK' => 'END_STONE_BRICKS',
        'FENCE' => 'OAK_FENCE',
        'FENCE_GATE' => 'OAK_FENCE_GATE',
        'FLOWING_LAVA' => 'LAVA',
        'FLOWING_WATER' => 'WATER',
        'GOLD_BLOCK' => 'GOLD',
        'GRASS_BLOCK' => 'GRASS',
        'IRON_BLOCK' => 'IRON',
        'JACK_O_LANTERN' => 'LIT_PUMPKIN',
        'LAPIS' => 'LAPIS_LAZULI',
        'LAPIS_BLOCK' => 'LAPIS_LAZULI',
        'LAPIS_LAZULI_BLOCK' => 'LAPIS_LAZULI',
        'LEAVES' => 'OAK_LEAVES',
        'LOG' => 'OAK_LOG',
        'MELON_BLOCK' => 'MELON',
        'MOSSY_COBBLE' => 'MOSSY_COBBLESTONE',
        'NETHERITE_BLOCK' => 'NETHERITE',
        'NETHER_BRICK' => 'NETHER_BRICKS',
        'NETHER_BRICK_BLOCK' => 'NETHER_BRICKS',
        'NOTEBLOCK' => 'NOTE_BLOCK',
        'PLANKS' => 'OAK_PLANKS',
        'PRESSURE_PLATE' => 'OAK_PRESSURE_PLATE',
        'PRISMARINE_BRICK' => 'PRISMARINE_BRICKS',
        'QUARTZ_BLOCK' => 'QUARTZ',
        'REDSTONE_BLOCK' => 'REDSTONE',
        'SAPLING' => 'OAK_SAPLING',
        'SIGN' => 'OAK_SIGN',
        'SLAB' => 'STONE_SLAB',
        'SNOW_BLOCK' => 'SNOW',
        'SPONGE_WET' => 'SPONGE',
        'STAINED_HARDENED_CLAY' => 'STAINED_CLAY',
        'STANDING_SIGN' => 'OAK_SIGN',
        'STONEBRICK' => 'STONE_BRICKS',
        'STONE_BRICK' => 'STONE_BRICKS',
        'TERRACOTTA' => 'HARDENED_CLAY',
        'TRAPDOOR' => 'OAK_TRAPDOOR',
        'WALL_SIGN' => 'OAK_WALL_SIGN',
        'WHITE_WOOL' => 'WOOL',
        'WOODEN_BUTTON' => 'OAK_BUTTON',
        'WOODEN_DOOR' => 'OAK_DOOR',
        'WOODEN_PRESSURE_PLATE' => 'OAK_PRESSURE_PLATE',
        'WOODEN_SLAB' => 'OAK_SLAB',
        'WOODEN_STAIRS' => 'OAK_STAIRS',
        'WOODEN_TRAPDOOR' => 'OAK_TRAPDOOR',
        'WORKBENCH' => 'CRAFTING_TABLE',
    ];

    /** @var array<string, string> Canonical uppercase name => authoritative Bedrock identifier */
    private const BLOCK_NAMES = [
        'ACACIA_BUTTON' => 'minecraft:acacia_button',
        'ACACIA_CEILING_CENTER_HANGING_SIGN' => 'minecraft:bamboo_hanging_sign',
        'ACACIA_CEILING_EDGES_HANGING_SIGN' => 'minecraft:acacia_hanging_sign',
        'ACACIA_DOOR' => 'minecraft:acacia_door',
        'ACACIA_FENCE' => 'minecraft:acacia_fence',
        'ACACIA_FENCE_GATE' => 'minecraft:acacia_fence_gate',
        'ACACIA_LEAVES' => 'minecraft:azalea_leaves',
        'ACACIA_LOG' => 'minecraft:acacia_log',
        'ACACIA_PLANKS' => 'minecraft:acacia_planks',
        'ACACIA_PRESSURE_PLATE' => 'minecraft:acacia_pressure_plate',
        'ACACIA_SAPLING' => 'minecraft:birch_sapling',
        'ACACIA_SIGN' => 'minecraft:acacia_standing_sign',
        'ACACIA_SLAB' => 'minecraft:acacia_slab',
        'ACACIA_STAIRS' => 'minecraft:acacia_stairs',
        'ACACIA_TRAPDOOR' => 'minecraft:acacia_trapdoor',
        'ACACIA_WALL_HANGING_SIGN' => 'minecraft:acacia_hanging_sign',
        'ACACIA_WALL_SIGN' => 'minecraft:acacia_wall_sign',
        'ACACIA_WOOD' => 'minecraft:acacia_wood',
        'ACTIVATOR_RAIL' => 'minecraft:activator_rail',
        'AIR' => 'minecraft:air',
        'ALLIUM' => 'minecraft:allium',
        'ALL_SIDED_MUSHROOM_STEM' => 'minecraft:mushroom_stem',
        'AMETHYST' => 'minecraft:amethyst_block',
        'AMETHYST_CLUSTER' => 'minecraft:small_amethyst_bud',
        'ANCIENT_DEBRIS' => 'minecraft:ancient_debris',
        'ANDESITE' => 'minecraft:andesite',
        'ANDESITE_SLAB' => 'minecraft:andesite_slab',
        'ANDESITE_STAIRS' => 'minecraft:andesite_stairs',
        'ANDESITE_WALL' => 'minecraft:blackstone_wall',
        'ANVIL' => 'minecraft:anvil',
        'AZALEA' => 'minecraft:azalea',
        'AZALEA_LEAVES' => 'minecraft:azalea_leaves_flowered',
        'AZURE_BLUET' => 'minecraft:azure_bluet',
        'BAMBOO' => 'minecraft:bamboo',
        'BAMBOO_BLOCK' => 'minecraft:bamboo_block',
        'BAMBOO_BUTTON' => 'minecraft:bamboo_button',
        'BAMBOO_CEILING_CENTER_HANGING_SIGN' => 'minecraft:birch_hanging_sign',
        'BAMBOO_CEILING_EDGES_HANGING_SIGN' => 'minecraft:bamboo_hanging_sign',
        'BAMBOO_DOOR' => 'minecraft:bamboo_door',
        'BAMBOO_FENCE' => 'minecraft:bamboo_fence',
        'BAMBOO_FENCE_GATE' => 'minecraft:bamboo_fence_gate',
        'BAMBOO_MOSAIC' => 'minecraft:bamboo_mosaic',
        'BAMBOO_MOSAIC_SLAB' => 'minecraft:bamboo_mosaic_slab',
        'BAMBOO_MOSAIC_STAIRS' => 'minecraft:bamboo_mosaic_stairs',
        'BAMBOO_PLANKS' => 'minecraft:bamboo_planks',
        'BAMBOO_PRESSURE_PLATE' => 'minecraft:bamboo_pressure_plate',
        'BAMBOO_SAPLING' => 'minecraft:bamboo_sapling',
        'BAMBOO_SIGN' => 'minecraft:bamboo_standing_sign',
        'BAMBOO_SLAB' => 'minecraft:bamboo_slab',
        'BAMBOO_STAIRS' => 'minecraft:bamboo_stairs',
        'BAMBOO_TRAPDOOR' => 'minecraft:bamboo_trapdoor',
        'BAMBOO_WALL_HANGING_SIGN' => 'minecraft:bamboo_hanging_sign',
        'BAMBOO_WALL_SIGN' => 'minecraft:bamboo_wall_sign',
        'BANNER' => 'minecraft:standing_banner',
        'BARREL' => 'minecraft:barrel',
        'BARRIER' => 'minecraft:barrier',
        'BASALT' => 'minecraft:basalt',
        'BEACON' => 'minecraft:beacon',
        'BED' => 'minecraft:bed',
        'BEDROCK' => 'minecraft:bedrock',
        'BEETROOTS' => 'minecraft:beetroot',
        'BELL' => 'minecraft:bell',
        'BIG_DRIPLEAF_HEAD' => 'minecraft:big_dripleaf',
        'BIG_DRIPLEAF_STEM' => 'minecraft:big_dripleaf',
        'BIRCH_BUTTON' => 'minecraft:birch_button',
        'BIRCH_CEILING_CENTER_HANGING_SIGN' => 'minecraft:cherry_hanging_sign',
        'BIRCH_CEILING_EDGES_HANGING_SIGN' => 'minecraft:birch_hanging_sign',
        'BIRCH_DOOR' => 'minecraft:birch_door',
        'BIRCH_FENCE' => 'minecraft:birch_fence',
        'BIRCH_FENCE_GATE' => 'minecraft:birch_fence_gate',
        'BIRCH_LEAVES' => 'minecraft:cherry_leaves',
        'BIRCH_LOG' => 'minecraft:birch_log',
        'BIRCH_PLANKS' => 'minecraft:birch_planks',
        'BIRCH_PRESSURE_PLATE' => 'minecraft:birch_pressure_plate',
        'BIRCH_SAPLING' => 'minecraft:dark_oak_sapling',
        'BIRCH_SIGN' => 'minecraft:birch_standing_sign',
        'BIRCH_SLAB' => 'minecraft:birch_slab',
        'BIRCH_STAIRS' => 'minecraft:birch_stairs',
        'BIRCH_TRAPDOOR' => 'minecraft:birch_trapdoor',
        'BIRCH_WALL_HANGING_SIGN' => 'minecraft:birch_hanging_sign',
        'BIRCH_WALL_SIGN' => 'minecraft:birch_wall_sign',
        'BIRCH_WOOD' => 'minecraft:birch_wood',
        'BLACKSTONE' => 'minecraft:blackstone',
        'BLACKSTONE_SLAB' => 'minecraft:blackstone_slab',
        'BLACKSTONE_STAIRS' => 'minecraft:blackstone_stairs',
        'BLACKSTONE_WALL' => 'minecraft:brick_wall',
        'BLAST_FURNACE' => 'minecraft:blast_furnace',
        'BLUE_ICE' => 'minecraft:blue_ice',
        'BLUE_ORCHID' => 'minecraft:blue_orchid',
        'BLUE_TORCH' => 'minecraft:colored_torch_blue',
        'BONE_BLOCK' => 'minecraft:bone_block',
        'BOOKSHELF' => 'minecraft:bookshelf',
        'BREWING_STAND' => 'minecraft:brewing_stand',
        'BRICKS' => 'minecraft:brick_block',
        'BRICK_SLAB' => 'minecraft:brick_slab',
        'BRICK_STAIRS' => 'minecraft:brick_stairs',
        'BRICK_WALL' => 'minecraft:cinnabar_brick_wall',
        'BROWN_MUSHROOM' => 'minecraft:brown_mushroom',
        'BROWN_MUSHROOM_BLOCK' => 'minecraft:brown_mushroom_block',
        'BUDDING_AMETHYST' => 'minecraft:budding_amethyst',
        'CACTUS' => 'minecraft:cactus',
        'CACTUS_FLOWER' => 'minecraft:cactus_flower',
        'CAKE' => 'minecraft:cake',
        'CAKE_WITH_CANDLE' => 'minecraft:candle_cake',
        'CAKE_WITH_DYED_CANDLE' => 'minecraft:candle_cake',
        'CALCITE' => 'minecraft:calcite',
        'CAMPFIRE' => 'minecraft:campfire',
        'CANDLE' => 'minecraft:candle',
        'CARPET' => 'minecraft:carpet',
        'CARROTS' => 'minecraft:carrots',
        'CARTOGRAPHY_TABLE' => 'minecraft:cartography_table',
        'CARVED_PUMPKIN' => 'minecraft:carved_pumpkin',
        'CAULDRON' => 'minecraft:cauldron',
        'CAVE_VINES' => 'minecraft:cave_vines',
        'CHAIN' => 'minecraft:iron_chain',
        'CHEMICAL_HEAT' => 'minecraft:chemical_heat',
        'CHERRY_BUTTON' => 'minecraft:cherry_button',
        'CHERRY_CEILING_CENTER_HANGING_SIGN' => 'minecraft:crimson_hanging_sign',
        'CHERRY_CEILING_EDGES_HANGING_SIGN' => 'minecraft:cherry_hanging_sign',
        'CHERRY_DOOR' => 'minecraft:cherry_door',
        'CHERRY_FENCE' => 'minecraft:cherry_fence',
        'CHERRY_FENCE_GATE' => 'minecraft:cherry_fence_gate',
        'CHERRY_LEAVES' => 'minecraft:dark_oak_leaves',
        'CHERRY_LOG' => 'minecraft:cherry_log',
        'CHERRY_PLANKS' => 'minecraft:cherry_planks',
        'CHERRY_PRESSURE_PLATE' => 'minecraft:cherry_pressure_plate',
        'CHERRY_SIGN' => 'minecraft:cherry_standing_sign',
        'CHERRY_SLAB' => 'minecraft:cherry_slab',
        'CHERRY_STAIRS' => 'minecraft:cherry_stairs',
        'CHERRY_TRAPDOOR' => 'minecraft:cherry_trapdoor',
        'CHERRY_WALL_HANGING_SIGN' => 'minecraft:cherry_hanging_sign',
        'CHERRY_WALL_SIGN' => 'minecraft:cherry_wall_sign',
        'CHERRY_WOOD' => 'minecraft:cherry_wood',
        'CHEST' => 'minecraft:chest',
        'CHISELED_BOOKSHELF' => 'minecraft:chiseled_bookshelf',
        'CHISELED_CINNABAR' => 'minecraft:chiseled_cinnabar',
        'CHISELED_COPPER' => 'minecraft:chiseled_copper',
        'CHISELED_DEEPSLATE' => 'minecraft:chiseled_deepslate',
        'CHISELED_NETHER_BRICKS' => 'minecraft:chiseled_nether_bricks',
        'CHISELED_POLISHED_BLACKSTONE' => 'minecraft:chiseled_polished_blackstone',
        'CHISELED_QUARTZ' => 'minecraft:chiseled_quartz_block',
        'CHISELED_RED_SANDSTONE' => 'minecraft:chiseled_red_sandstone',
        'CHISELED_RESIN_BRICKS' => 'minecraft:chiseled_resin_bricks',
        'CHISELED_SANDSTONE' => 'minecraft:chiseled_sandstone',
        'CHISELED_STONE_BRICKS' => 'minecraft:chiseled_stone_bricks',
        'CHISELED_SULFUR' => 'minecraft:chiseled_sulfur',
        'CHISELED_TUFF' => 'minecraft:chiseled_tuff',
        'CHISELED_TUFF_BRICKS' => 'minecraft:chiseled_tuff_bricks',
        'CHORUS_FLOWER' => 'minecraft:chorus_flower',
        'CHORUS_PLANT' => 'minecraft:chorus_plant',
        'CINNABAR' => 'minecraft:cinnabar',
        'CINNABAR_BRICKS' => 'minecraft:cinnabar_bricks',
        'CINNABAR_BRICK_SLAB' => 'minecraft:cinnabar_brick_slab',
        'CINNABAR_BRICK_STAIRS' => 'minecraft:cinnabar_brick_stairs',
        'CINNABAR_BRICK_WALL' => 'minecraft:cinnabar_wall',
        'CINNABAR_SLAB' => 'minecraft:cinnabar_slab',
        'CINNABAR_STAIRS' => 'minecraft:cinnabar_stairs',
        'CINNABAR_WALL' => 'minecraft:cobbled_deepslate_wall',
        'CLAY' => 'minecraft:clay',
        'COAL' => 'minecraft:coal_block',
        'COAL_ORE' => 'minecraft:coal_ore',
        'COBBLED_DEEPSLATE' => 'minecraft:cobbled_deepslate',
        'COBBLED_DEEPSLATE_SLAB' => 'minecraft:cobbled_deepslate_slab',
        'COBBLED_DEEPSLATE_STAIRS' => 'minecraft:cobbled_deepslate_stairs',
        'COBBLED_DEEPSLATE_WALL' => 'minecraft:cobblestone_wall',
        'COBBLESTONE' => 'minecraft:cobblestone',
        'COBBLESTONE_SLAB' => 'minecraft:cobblestone_slab',
        'COBBLESTONE_STAIRS' => 'minecraft:stone_stairs',
        'COBBLESTONE_WALL' => 'minecraft:deepslate_brick_wall',
        'COBWEB' => 'minecraft:web',
        'COCOA_POD' => 'minecraft:cocoa',
        'COMPOUND_CREATOR' => 'minecraft:compound_creator',
        'CONCRETE' => 'minecraft:concrete',
        'CONCRETE_POWDER' => 'minecraft:concrete_powder',
        'CONTINUOUS_POTENT_SULFUR' => 'minecraft:potent_sulfur',
        'COPPER' => 'minecraft:copper_block',
        'COPPER_BARS' => 'minecraft:copper_bars',
        'COPPER_BULB' => 'minecraft:copper_bulb',
        'COPPER_CHAIN' => 'minecraft:copper_chain',
        'COPPER_DOOR' => 'minecraft:copper_door',
        'COPPER_GRATE' => 'minecraft:copper_grate',
        'COPPER_LANTERN' => 'minecraft:copper_lantern',
        'COPPER_ORE' => 'minecraft:copper_ore',
        'COPPER_TORCH' => 'minecraft:copper_torch',
        'COPPER_TRAPDOOR' => 'minecraft:copper_trapdoor',
        'CORAL' => 'minecraft:coral',
        'CORAL_BLOCK' => 'minecraft:coral_block',
        'CORAL_FAN' => 'minecraft:coral_fan',
        'CORNFLOWER' => 'minecraft:cornflower',
        'CRACKED_DEEPSLATE_BRICKS' => 'minecraft:cracked_deepslate_bricks',
        'CRACKED_DEEPSLATE_TILES' => 'minecraft:cracked_deepslate_tiles',
        'CRACKED_NETHER_BRICKS' => 'minecraft:cracked_nether_bricks',
        'CRACKED_POLISHED_BLACKSTONE_BRICKS' => 'minecraft:cracked_polished_blackstone_bricks',
        'CRACKED_STONE_BRICKS' => 'minecraft:cracked_stone_bricks',
        'CRAFTING_TABLE' => 'minecraft:crafting_table',
        'CRIMSON_BUTTON' => 'minecraft:crimson_button',
        'CRIMSON_CEILING_CENTER_HANGING_SIGN' => 'minecraft:dark_oak_hanging_sign',
        'CRIMSON_CEILING_EDGES_HANGING_SIGN' => 'minecraft:crimson_hanging_sign',
        'CRIMSON_DOOR' => 'minecraft:crimson_door',
        'CRIMSON_FENCE' => 'minecraft:crimson_fence',
        'CRIMSON_FENCE_GATE' => 'minecraft:crimson_fence_gate',
        'CRIMSON_FUNGUS' => 'minecraft:crimson_fungus',
        'CRIMSON_HYPHAE' => 'minecraft:crimson_hyphae',
        'CRIMSON_NYLIUM' => 'minecraft:crimson_nylium',
        'CRIMSON_PLANKS' => 'minecraft:crimson_planks',
        'CRIMSON_PRESSURE_PLATE' => 'minecraft:crimson_pressure_plate',
        'CRIMSON_ROOTS' => 'minecraft:crimson_roots',
        'CRIMSON_SIGN' => 'minecraft:crimson_standing_sign',
        'CRIMSON_SLAB' => 'minecraft:crimson_slab',
        'CRIMSON_STAIRS' => 'minecraft:crimson_stairs',
        'CRIMSON_STEM' => 'minecraft:crimson_stem',
        'CRIMSON_TRAPDOOR' => 'minecraft:crimson_trapdoor',
        'CRIMSON_WALL_HANGING_SIGN' => 'minecraft:crimson_hanging_sign',
        'CRIMSON_WALL_SIGN' => 'minecraft:crimson_wall_sign',
        'CRYING_OBSIDIAN' => 'minecraft:crying_obsidian',
        'CUT_COPPER' => 'minecraft:cut_copper',
        'CUT_COPPER_SLAB' => 'minecraft:cut_copper_slab',
        'CUT_COPPER_STAIRS' => 'minecraft:cut_copper_stairs',
        'CUT_RED_SANDSTONE' => 'minecraft:cut_red_sandstone',
        'CUT_RED_SANDSTONE_SLAB' => 'minecraft:cut_red_sandstone_slab',
        'CUT_SANDSTONE' => 'minecraft:cut_sandstone',
        'CUT_SANDSTONE_SLAB' => 'minecraft:cut_sandstone_slab',
        'CYCLING_POTENT_SULFUR' => 'minecraft:potent_sulfur',
        'DANDELION' => 'minecraft:dandelion',
        'DARK_OAK_BUTTON' => 'minecraft:dark_oak_button',
        'DARK_OAK_CEILING_CENTER_HANGING_SIGN' => 'minecraft:jungle_hanging_sign',
        'DARK_OAK_CEILING_EDGES_HANGING_SIGN' => 'minecraft:dark_oak_hanging_sign',
        'DARK_OAK_DOOR' => 'minecraft:dark_oak_door',
        'DARK_OAK_FENCE' => 'minecraft:dark_oak_fence',
        'DARK_OAK_FENCE_GATE' => 'minecraft:dark_oak_fence_gate',
        'DARK_OAK_LEAVES' => 'minecraft:jungle_leaves',
        'DARK_OAK_LOG' => 'minecraft:dark_oak_log',
        'DARK_OAK_PLANKS' => 'minecraft:dark_oak_planks',
        'DARK_OAK_PRESSURE_PLATE' => 'minecraft:dark_oak_pressure_plate',
        'DARK_OAK_SAPLING' => 'minecraft:jungle_sapling',
        'DARK_OAK_SIGN' => 'minecraft:darkoak_standing_sign',
        'DARK_OAK_SLAB' => 'minecraft:dark_oak_slab',
        'DARK_OAK_STAIRS' => 'minecraft:dark_oak_stairs',
        'DARK_OAK_TRAPDOOR' => 'minecraft:dark_oak_trapdoor',
        'DARK_OAK_WALL_HANGING_SIGN' => 'minecraft:dark_oak_hanging_sign',
        'DARK_OAK_WALL_SIGN' => 'minecraft:darkoak_wall_sign',
        'DARK_OAK_WOOD' => 'minecraft:dark_oak_wood',
        'DARK_PRISMARINE' => 'minecraft:dark_prismarine',
        'DARK_PRISMARINE_SLAB' => 'minecraft:dark_prismarine_slab',
        'DARK_PRISMARINE_STAIRS' => 'minecraft:dark_prismarine_stairs',
        'DAYLIGHT_SENSOR' => 'minecraft:daylight_detector',
        'DEAD_BUSH' => 'minecraft:deadbush',
        'DEEPSLATE' => 'minecraft:deepslate',
        'DEEPSLATE_BRICKS' => 'minecraft:deepslate_bricks',
        'DEEPSLATE_BRICK_SLAB' => 'minecraft:deepslate_brick_slab',
        'DEEPSLATE_BRICK_STAIRS' => 'minecraft:deepslate_brick_stairs',
        'DEEPSLATE_BRICK_WALL' => 'minecraft:deepslate_tile_wall',
        'DEEPSLATE_COAL_ORE' => 'minecraft:deepslate_coal_ore',
        'DEEPSLATE_COPPER_ORE' => 'minecraft:deepslate_copper_ore',
        'DEEPSLATE_DIAMOND_ORE' => 'minecraft:deepslate_diamond_ore',
        'DEEPSLATE_EMERALD_ORE' => 'minecraft:deepslate_emerald_ore',
        'DEEPSLATE_GOLD_ORE' => 'minecraft:deepslate_gold_ore',
        'DEEPSLATE_IRON_ORE' => 'minecraft:deepslate_iron_ore',
        'DEEPSLATE_LAPIS_LAZULI_ORE' => 'minecraft:deepslate_lapis_ore',
        'DEEPSLATE_REDSTONE_ORE' => 'minecraft:deepslate_redstone_ore',
        'DEEPSLATE_TILES' => 'minecraft:deepslate_tiles',
        'DEEPSLATE_TILE_SLAB' => 'minecraft:deepslate_tile_slab',
        'DEEPSLATE_TILE_STAIRS' => 'minecraft:deepslate_tile_stairs',
        'DEEPSLATE_TILE_WALL' => 'minecraft:diorite_wall',
        'DETECTOR_RAIL' => 'minecraft:detector_rail',
        'DIAMOND' => 'minecraft:diamond_block',
        'DIAMOND_ORE' => 'minecraft:diamond_ore',
        'DIORITE' => 'minecraft:diorite',
        'DIORITE_SLAB' => 'minecraft:diorite_slab',
        'DIORITE_STAIRS' => 'minecraft:diorite_stairs',
        'DIORITE_WALL' => 'minecraft:end_stone_brick_wall',
        'DIRT' => 'minecraft:dirt',
        'DOUBLE_PITCHER_CROP' => 'minecraft:pitcher_crop',
        'DOUBLE_TALLGRASS' => 'minecraft:tall_grass',
        'DRAGON_EGG' => 'minecraft:dragon_egg',
        'DRIED_KELP' => 'minecraft:dried_kelp_block',
        'DYED_CANDLE' => 'minecraft:candle',
        'DYED_SHULKER_BOX' => 'minecraft:shulker_box',
        'ELEMENT_ACTINIUM' => 'minecraft:element_89',
        'ELEMENT_ALUMINUM' => 'minecraft:element_13',
        'ELEMENT_AMERICIUM' => 'minecraft:element_95',
        'ELEMENT_ANTIMONY' => 'minecraft:element_51',
        'ELEMENT_ARGON' => 'minecraft:element_18',
        'ELEMENT_ARSENIC' => 'minecraft:element_33',
        'ELEMENT_ASTATINE' => 'minecraft:element_85',
        'ELEMENT_BARIUM' => 'minecraft:element_56',
        'ELEMENT_BERKELIUM' => 'minecraft:element_97',
        'ELEMENT_BERYLLIUM' => 'minecraft:element_4',
        'ELEMENT_BISMUTH' => 'minecraft:element_83',
        'ELEMENT_BOHRIUM' => 'minecraft:element_107',
        'ELEMENT_BORON' => 'minecraft:element_5',
        'ELEMENT_BROMINE' => 'minecraft:element_35',
        'ELEMENT_CADMIUM' => 'minecraft:element_48',
        'ELEMENT_CALCIUM' => 'minecraft:element_20',
        'ELEMENT_CALIFORNIUM' => 'minecraft:element_98',
        'ELEMENT_CARBON' => 'minecraft:element_6',
        'ELEMENT_CERIUM' => 'minecraft:element_58',
        'ELEMENT_CESIUM' => 'minecraft:element_55',
        'ELEMENT_CHLORINE' => 'minecraft:element_17',
        'ELEMENT_CHROMIUM' => 'minecraft:element_24',
        'ELEMENT_COBALT' => 'minecraft:element_27',
        'ELEMENT_CONSTRUCTOR' => 'minecraft:element_constructor',
        'ELEMENT_COPERNICIUM' => 'minecraft:element_112',
        'ELEMENT_COPPER' => 'minecraft:element_29',
        'ELEMENT_CURIUM' => 'minecraft:element_96',
        'ELEMENT_DARMSTADTIUM' => 'minecraft:element_110',
        'ELEMENT_DUBNIUM' => 'minecraft:element_105',
        'ELEMENT_DYSPROSIUM' => 'minecraft:element_66',
        'ELEMENT_EINSTEINIUM' => 'minecraft:element_99',
        'ELEMENT_ERBIUM' => 'minecraft:element_68',
        'ELEMENT_EUROPIUM' => 'minecraft:element_63',
        'ELEMENT_FERMIUM' => 'minecraft:element_100',
        'ELEMENT_FLEROVIUM' => 'minecraft:element_114',
        'ELEMENT_FLUORINE' => 'minecraft:element_9',
        'ELEMENT_FRANCIUM' => 'minecraft:element_87',
        'ELEMENT_GADOLINIUM' => 'minecraft:element_64',
        'ELEMENT_GALLIUM' => 'minecraft:element_31',
        'ELEMENT_GERMANIUM' => 'minecraft:element_32',
        'ELEMENT_GOLD' => 'minecraft:element_79',
        'ELEMENT_HAFNIUM' => 'minecraft:element_72',
        'ELEMENT_HASSIUM' => 'minecraft:element_108',
        'ELEMENT_HELIUM' => 'minecraft:element_2',
        'ELEMENT_HOLMIUM' => 'minecraft:element_67',
        'ELEMENT_HYDROGEN' => 'minecraft:element_1',
        'ELEMENT_INDIUM' => 'minecraft:element_49',
        'ELEMENT_IODINE' => 'minecraft:element_53',
        'ELEMENT_IRIDIUM' => 'minecraft:element_77',
        'ELEMENT_IRON' => 'minecraft:element_26',
        'ELEMENT_KRYPTON' => 'minecraft:element_36',
        'ELEMENT_LANTHANUM' => 'minecraft:element_57',
        'ELEMENT_LAWRENCIUM' => 'minecraft:element_103',
        'ELEMENT_LEAD' => 'minecraft:element_82',
        'ELEMENT_LITHIUM' => 'minecraft:element_3',
        'ELEMENT_LIVERMORIUM' => 'minecraft:element_116',
        'ELEMENT_LUTETIUM' => 'minecraft:element_71',
        'ELEMENT_MAGNESIUM' => 'minecraft:element_12',
        'ELEMENT_MANGANESE' => 'minecraft:element_25',
        'ELEMENT_MEITNERIUM' => 'minecraft:element_109',
        'ELEMENT_MENDELEVIUM' => 'minecraft:element_101',
        'ELEMENT_MERCURY' => 'minecraft:element_80',
        'ELEMENT_MOLYBDENUM' => 'minecraft:element_42',
        'ELEMENT_MOSCOVIUM' => 'minecraft:element_115',
        'ELEMENT_NEODYMIUM' => 'minecraft:element_60',
        'ELEMENT_NEON' => 'minecraft:element_10',
        'ELEMENT_NEPTUNIUM' => 'minecraft:element_93',
        'ELEMENT_NICKEL' => 'minecraft:element_28',
        'ELEMENT_NIHONIUM' => 'minecraft:element_113',
        'ELEMENT_NIOBIUM' => 'minecraft:element_41',
        'ELEMENT_NITROGEN' => 'minecraft:element_7',
        'ELEMENT_NOBELIUM' => 'minecraft:element_102',
        'ELEMENT_OGANESSON' => 'minecraft:element_118',
        'ELEMENT_OSMIUM' => 'minecraft:element_76',
        'ELEMENT_OXYGEN' => 'minecraft:element_8',
        'ELEMENT_PALLADIUM' => 'minecraft:element_46',
        'ELEMENT_PHOSPHORUS' => 'minecraft:element_15',
        'ELEMENT_PLATINUM' => 'minecraft:element_78',
        'ELEMENT_PLUTONIUM' => 'minecraft:element_94',
        'ELEMENT_POLONIUM' => 'minecraft:element_84',
        'ELEMENT_POTASSIUM' => 'minecraft:element_19',
        'ELEMENT_PRASEODYMIUM' => 'minecraft:element_59',
        'ELEMENT_PROMETHIUM' => 'minecraft:element_61',
        'ELEMENT_PROTACTINIUM' => 'minecraft:element_91',
        'ELEMENT_RADIUM' => 'minecraft:element_88',
        'ELEMENT_RADON' => 'minecraft:element_86',
        'ELEMENT_RHENIUM' => 'minecraft:element_75',
        'ELEMENT_RHODIUM' => 'minecraft:element_45',
        'ELEMENT_ROENTGENIUM' => 'minecraft:element_111',
        'ELEMENT_RUBIDIUM' => 'minecraft:element_37',
        'ELEMENT_RUTHENIUM' => 'minecraft:element_44',
        'ELEMENT_RUTHERFORDIUM' => 'minecraft:element_104',
        'ELEMENT_SAMARIUM' => 'minecraft:element_62',
        'ELEMENT_SCANDIUM' => 'minecraft:element_21',
        'ELEMENT_SEABORGIUM' => 'minecraft:element_106',
        'ELEMENT_SELENIUM' => 'minecraft:element_34',
        'ELEMENT_SILICON' => 'minecraft:element_14',
        'ELEMENT_SILVER' => 'minecraft:element_47',
        'ELEMENT_SODIUM' => 'minecraft:element_11',
        'ELEMENT_STRONTIUM' => 'minecraft:element_38',
        'ELEMENT_SULFUR' => 'minecraft:element_16',
        'ELEMENT_TANTALUM' => 'minecraft:element_73',
        'ELEMENT_TECHNETIUM' => 'minecraft:element_43',
        'ELEMENT_TELLURIUM' => 'minecraft:element_52',
        'ELEMENT_TENNESSINE' => 'minecraft:element_117',
        'ELEMENT_TERBIUM' => 'minecraft:element_65',
        'ELEMENT_THALLIUM' => 'minecraft:element_81',
        'ELEMENT_THORIUM' => 'minecraft:element_90',
        'ELEMENT_THULIUM' => 'minecraft:element_69',
        'ELEMENT_TIN' => 'minecraft:element_50',
        'ELEMENT_TITANIUM' => 'minecraft:element_22',
        'ELEMENT_TUNGSTEN' => 'minecraft:element_74',
        'ELEMENT_URANIUM' => 'minecraft:element_92',
        'ELEMENT_VANADIUM' => 'minecraft:element_23',
        'ELEMENT_XENON' => 'minecraft:element_54',
        'ELEMENT_YTTERBIUM' => 'minecraft:element_70',
        'ELEMENT_YTTRIUM' => 'minecraft:element_39',
        'ELEMENT_ZERO' => 'minecraft:element_0',
        'ELEMENT_ZINC' => 'minecraft:element_30',
        'ELEMENT_ZIRCONIUM' => 'minecraft:element_40',
        'EMERALD' => 'minecraft:emerald_block',
        'EMERALD_ORE' => 'minecraft:emerald_ore',
        'ENCHANTING_TABLE' => 'minecraft:enchanting_table',
        'ENDER_CHEST' => 'minecraft:ender_chest',
        'END_PORTAL_FRAME' => 'minecraft:end_portal_frame',
        'END_ROD' => 'minecraft:end_rod',
        'END_STONE' => 'minecraft:end_stone',
        'END_STONE_BRICKS' => 'minecraft:end_bricks',
        'END_STONE_BRICK_SLAB' => 'minecraft:end_stone_brick_slab',
        'END_STONE_BRICK_STAIRS' => 'minecraft:end_brick_stairs',
        'END_STONE_BRICK_WALL' => 'minecraft:granite_wall',
        'FAKE_WOODEN_SLAB' => 'minecraft:wooden_slab',
        'FARMLAND' => 'minecraft:farmland',
        'FERN' => 'minecraft:fern',
        'FIRE' => 'minecraft:fire',
        'FLETCHING_TABLE' => 'minecraft:fletching_table',
        'FLOWERING_AZALEA' => 'minecraft:flowering_azalea',
        'FLOWERING_AZALEA_LEAVES' => 'minecraft:birch_leaves',
        'FLOWER_POT' => 'minecraft:flower_pot',
        'FROGLIGHT' => 'minecraft:ochre_froglight',
        'FROSTED_ICE' => 'minecraft:frosted_ice',
        'FURNACE' => 'minecraft:furnace',
        'GILDED_BLACKSTONE' => 'minecraft:gilded_blackstone',
        'GLASS' => 'minecraft:glass',
        'GLASS_PANE' => 'minecraft:glass_pane',
        'GLAZED_TERRACOTTA' => 'minecraft:white_glazed_terracotta',
        'GLOWING_ITEM_FRAME' => 'minecraft:glow_frame',
        'GLOWING_OBSIDIAN' => 'minecraft:glowingobsidian',
        'GLOWSTONE' => 'minecraft:glowstone',
        'GLOW_LICHEN' => 'minecraft:glow_lichen',
        'GOLD' => 'minecraft:gold_block',
        'GOLD_ORE' => 'minecraft:gold_ore',
        'GRANITE' => 'minecraft:granite',
        'GRANITE_SLAB' => 'minecraft:granite_slab',
        'GRANITE_STAIRS' => 'minecraft:granite_stairs',
        'GRANITE_WALL' => 'minecraft:mossy_cobblestone_wall',
        'GRASS' => 'minecraft:grass_block',
        'GRASS_PATH' => 'minecraft:grass_path',
        'GRAVEL' => 'minecraft:gravel',
        'GREEN_TORCH' => 'minecraft:colored_torch_green',
        'HANGING_ROOTS' => 'minecraft:hanging_roots',
        'HARDENED_CLAY' => 'minecraft:hardened_clay',
        'HARDENED_GLASS' => 'minecraft:hard_glass',
        'HARDENED_GLASS_PANE' => 'minecraft:hard_glass_pane',
        'HAY_BALE' => 'minecraft:hay_block',
        'HEAVY_CORE' => 'minecraft:heavy_core',
        'HONEYCOMB' => 'minecraft:honeycomb_block',
        'HOPPER' => 'minecraft:hopper',
        'ICE' => 'minecraft:ice',
        'INFESTED_CHISELED_STONE_BRICK' => 'minecraft:infested_chiseled_stone_bricks',
        'INFESTED_COBBLESTONE' => 'minecraft:infested_cobblestone',
        'INFESTED_CRACKED_STONE_BRICK' => 'minecraft:infested_cracked_stone_bricks',
        'INFESTED_DEEPSLATE' => 'minecraft:infested_deepslate',
        'INFESTED_MOSSY_STONE_BRICK' => 'minecraft:infested_mossy_stone_bricks',
        'INFESTED_STONE' => 'minecraft:infested_stone',
        'INFESTED_STONE_BRICK' => 'minecraft:infested_stone_bricks',
        'INFO_UPDATE' => 'minecraft:info_update',
        'INFO_UPDATE2' => 'minecraft:info_update2',
        'INVISIBLE_BEDROCK' => 'minecraft:invisible_bedrock',
        'IRON' => 'minecraft:iron_block',
        'IRON_BARS' => 'minecraft:iron_bars',
        'IRON_DOOR' => 'minecraft:iron_door',
        'IRON_ORE' => 'minecraft:iron_ore',
        'IRON_TRAPDOOR' => 'minecraft:iron_trapdoor',
        'ITEM_FRAME' => 'minecraft:frame',
        'JUKEBOX' => 'minecraft:jukebox',
        'JUNGLE_BUTTON' => 'minecraft:jungle_button',
        'JUNGLE_CEILING_CENTER_HANGING_SIGN' => 'minecraft:mangrove_hanging_sign',
        'JUNGLE_CEILING_EDGES_HANGING_SIGN' => 'minecraft:jungle_hanging_sign',
        'JUNGLE_DOOR' => 'minecraft:jungle_door',
        'JUNGLE_FENCE' => 'minecraft:jungle_fence',
        'JUNGLE_FENCE_GATE' => 'minecraft:jungle_fence_gate',
        'JUNGLE_LEAVES' => 'minecraft:mangrove_leaves',
        'JUNGLE_LOG' => 'minecraft:jungle_log',
        'JUNGLE_PLANKS' => 'minecraft:jungle_planks',
        'JUNGLE_PRESSURE_PLATE' => 'minecraft:jungle_pressure_plate',
        'JUNGLE_SAPLING' => 'minecraft:oak_sapling',
        'JUNGLE_SIGN' => 'minecraft:jungle_standing_sign',
        'JUNGLE_SLAB' => 'minecraft:jungle_slab',
        'JUNGLE_STAIRS' => 'minecraft:jungle_stairs',
        'JUNGLE_TRAPDOOR' => 'minecraft:jungle_trapdoor',
        'JUNGLE_WALL_HANGING_SIGN' => 'minecraft:jungle_hanging_sign',
        'JUNGLE_WALL_SIGN' => 'minecraft:jungle_wall_sign',
        'JUNGLE_WOOD' => 'minecraft:jungle_wood',
        'LAB_TABLE' => 'minecraft:lab_table',
        'LADDER' => 'minecraft:ladder',
        'LANTERN' => 'minecraft:lantern',
        'LAPIS_LAZULI' => 'minecraft:lapis_block',
        'LAPIS_LAZULI_ORE' => 'minecraft:lapis_ore',
        'LARGE_FERN' => 'minecraft:large_fern',
        'LAVA' => 'minecraft:lava',
        'LAVA_CAULDRON' => 'minecraft:cauldron',
        'LECTERN' => 'minecraft:lectern',
        'LEGACY_STONECUTTER' => 'minecraft:stonecutter',
        'LEVER' => 'minecraft:lever',
        'LIGHT' => 'minecraft:light_block',
        'LIGHTNING_ROD' => 'minecraft:lightning_rod',
        'LILAC' => 'minecraft:lilac',
        'LILY_OF_THE_VALLEY' => 'minecraft:lily_of_the_valley',
        'LILY_PAD' => 'minecraft:waterlily',
        'LIT_PUMPKIN' => 'minecraft:lit_pumpkin',
        'LOOM' => 'minecraft:loom',
        'MAGMA' => 'minecraft:magma',
        'MANGROVE_BUTTON' => 'minecraft:mangrove_button',
        'MANGROVE_CEILING_CENTER_HANGING_SIGN' => 'minecraft:oak_hanging_sign',
        'MANGROVE_CEILING_EDGES_HANGING_SIGN' => 'minecraft:mangrove_hanging_sign',
        'MANGROVE_DOOR' => 'minecraft:mangrove_door',
        'MANGROVE_FENCE' => 'minecraft:mangrove_fence',
        'MANGROVE_FENCE_GATE' => 'minecraft:mangrove_fence_gate',
        'MANGROVE_LEAVES' => 'minecraft:oak_leaves',
        'MANGROVE_LOG' => 'minecraft:mangrove_log',
        'MANGROVE_PLANKS' => 'minecraft:mangrove_planks',
        'MANGROVE_PRESSURE_PLATE' => 'minecraft:mangrove_pressure_plate',
        'MANGROVE_ROOTS' => 'minecraft:mangrove_roots',
        'MANGROVE_SIGN' => 'minecraft:mangrove_standing_sign',
        'MANGROVE_SLAB' => 'minecraft:mangrove_slab',
        'MANGROVE_STAIRS' => 'minecraft:mangrove_stairs',
        'MANGROVE_TRAPDOOR' => 'minecraft:mangrove_trapdoor',
        'MANGROVE_WALL_HANGING_SIGN' => 'minecraft:mangrove_hanging_sign',
        'MANGROVE_WALL_SIGN' => 'minecraft:mangrove_wall_sign',
        'MANGROVE_WOOD' => 'minecraft:mangrove_wood',
        'MATERIAL_REDUCER' => 'minecraft:material_reducer',
        'MELON' => 'minecraft:melon_block',
        'MELON_STEM' => 'minecraft:melon_stem',
        'MOB_HEAD' => 'minecraft:skull',
        'MONSTER_SPAWNER' => 'minecraft:mob_spawner',
        'MOSSY_COBBLESTONE' => 'minecraft:mossy_cobblestone',
        'MOSSY_COBBLESTONE_SLAB' => 'minecraft:mossy_cobblestone_slab',
        'MOSSY_COBBLESTONE_STAIRS' => 'minecraft:mossy_cobblestone_stairs',
        'MOSSY_COBBLESTONE_WALL' => 'minecraft:mossy_stone_brick_wall',
        'MOSSY_STONE_BRICKS' => 'minecraft:mossy_stone_bricks',
        'MOSSY_STONE_BRICK_SLAB' => 'minecraft:mossy_stone_brick_slab',
        'MOSSY_STONE_BRICK_STAIRS' => 'minecraft:mossy_stone_brick_stairs',
        'MOSSY_STONE_BRICK_WALL' => 'minecraft:mud_brick_wall',
        'MUD' => 'minecraft:mud',
        'MUDDY_MANGROVE_ROOTS' => 'minecraft:muddy_mangrove_roots',
        'MUD_BRICKS' => 'minecraft:mud_bricks',
        'MUD_BRICK_SLAB' => 'minecraft:mud_brick_slab',
        'MUD_BRICK_STAIRS' => 'minecraft:mud_brick_stairs',
        'MUD_BRICK_WALL' => 'minecraft:nether_brick_wall',
        'MUSHROOM_STEM' => 'minecraft:mushroom_stem',
        'MYCELIUM' => 'minecraft:mycelium',
        'NETHERITE' => 'minecraft:netherite_block',
        'NETHERRACK' => 'minecraft:netherrack',
        'NETHER_BRICKS' => 'minecraft:nether_brick',
        'NETHER_BRICK_FENCE' => 'minecraft:nether_brick_fence',
        'NETHER_BRICK_SLAB' => 'minecraft:nether_brick_slab',
        'NETHER_BRICK_STAIRS' => 'minecraft:nether_brick_stairs',
        'NETHER_BRICK_WALL' => 'minecraft:polished_blackstone_brick_wall',
        'NETHER_GOLD_ORE' => 'minecraft:nether_gold_ore',
        'NETHER_PORTAL' => 'minecraft:portal',
        'NETHER_QUARTZ_ORE' => 'minecraft:quartz_ore',
        'NETHER_REACTOR_CORE' => 'minecraft:netherreactor',
        'NETHER_SPROUTS' => 'minecraft:nether_sprouts',
        'NETHER_WART' => 'minecraft:nether_wart',
        'NETHER_WART_BLOCK' => 'minecraft:nether_wart_block',
        'NOTE_BLOCK' => 'minecraft:noteblock',
        'OAK_BUTTON' => 'minecraft:wooden_button',
        'OAK_CEILING_CENTER_HANGING_SIGN' => 'minecraft:pale_oak_hanging_sign',
        'OAK_CEILING_EDGES_HANGING_SIGN' => 'minecraft:oak_hanging_sign',
        'OAK_DOOR' => 'minecraft:wooden_door',
        'OAK_FENCE' => 'minecraft:oak_fence',
        'OAK_FENCE_GATE' => 'minecraft:fence_gate',
        'OAK_LEAVES' => 'minecraft:pale_oak_leaves',
        'OAK_LOG' => 'minecraft:oak_log',
        'OAK_PLANKS' => 'minecraft:oak_planks',
        'OAK_PRESSURE_PLATE' => 'minecraft:wooden_pressure_plate',
        'OAK_SAPLING' => 'minecraft:spruce_sapling',
        'OAK_SIGN' => 'minecraft:standing_sign',
        'OAK_SLAB' => 'minecraft:oak_slab',
        'OAK_STAIRS' => 'minecraft:oak_stairs',
        'OAK_TRAPDOOR' => 'minecraft:trapdoor',
        'OAK_WALL_HANGING_SIGN' => 'minecraft:oak_hanging_sign',
        'OAK_WALL_SIGN' => 'minecraft:wall_sign',
        'OAK_WOOD' => 'minecraft:oak_wood',
        'OBSIDIAN' => 'minecraft:obsidian',
        'OMINOUS_BANNER' => 'minecraft:standing_banner',
        'OMINOUS_WALL_BANNER' => 'minecraft:wall_banner',
        'ORANGE_TULIP' => 'minecraft:orange_tulip',
        'OXEYE_DAISY' => 'minecraft:oxeye_daisy',
        'PACKED_ICE' => 'minecraft:packed_ice',
        'PACKED_MUD' => 'minecraft:packed_mud',
        'PALE_OAK_BUTTON' => 'minecraft:pale_oak_button',
        'PALE_OAK_CEILING_CENTER_HANGING_SIGN' => 'minecraft:spruce_hanging_sign',
        'PALE_OAK_CEILING_EDGES_HANGING_SIGN' => 'minecraft:pale_oak_hanging_sign',
        'PALE_OAK_DOOR' => 'minecraft:pale_oak_door',
        'PALE_OAK_FENCE' => 'minecraft:pale_oak_fence',
        'PALE_OAK_FENCE_GATE' => 'minecraft:pale_oak_fence_gate',
        'PALE_OAK_LEAVES' => 'minecraft:spruce_leaves',
        'PALE_OAK_LOG' => 'minecraft:pale_oak_log',
        'PALE_OAK_PLANKS' => 'minecraft:pale_oak_planks',
        'PALE_OAK_PRESSURE_PLATE' => 'minecraft:pale_oak_pressure_plate',
        'PALE_OAK_SIGN' => 'minecraft:pale_oak_standing_sign',
        'PALE_OAK_SLAB' => 'minecraft:pale_oak_slab',
        'PALE_OAK_STAIRS' => 'minecraft:pale_oak_stairs',
        'PALE_OAK_TRAPDOOR' => 'minecraft:pale_oak_trapdoor',
        'PALE_OAK_WALL_HANGING_SIGN' => 'minecraft:pale_oak_hanging_sign',
        'PALE_OAK_WALL_SIGN' => 'minecraft:pale_oak_wall_sign',
        'PALE_OAK_WOOD' => 'minecraft:pale_oak_wood',
        'PEONY' => 'minecraft:peony',
        'PINK_PETALS' => 'minecraft:pink_petals',
        'PINK_TULIP' => 'minecraft:pink_tulip',
        'PITCHER_CROP' => 'minecraft:pitcher_crop',
        'PITCHER_PLANT' => 'minecraft:pitcher_plant',
        'PODZOL' => 'minecraft:podzol',
        'POLISHED_ANDESITE' => 'minecraft:polished_andesite',
        'POLISHED_ANDESITE_SLAB' => 'minecraft:polished_andesite_slab',
        'POLISHED_ANDESITE_STAIRS' => 'minecraft:polished_andesite_stairs',
        'POLISHED_BASALT' => 'minecraft:polished_basalt',
        'POLISHED_BLACKSTONE' => 'minecraft:polished_blackstone',
        'POLISHED_BLACKSTONE_BRICKS' => 'minecraft:polished_blackstone_bricks',
        'POLISHED_BLACKSTONE_BRICK_SLAB' => 'minecraft:polished_blackstone_brick_slab',
        'POLISHED_BLACKSTONE_BRICK_STAIRS' => 'minecraft:polished_blackstone_brick_stairs',
        'POLISHED_BLACKSTONE_BRICK_WALL' => 'minecraft:polished_blackstone_wall',
        'POLISHED_BLACKSTONE_BUTTON' => 'minecraft:polished_blackstone_button',
        'POLISHED_BLACKSTONE_PRESSURE_PLATE' => 'minecraft:polished_blackstone_pressure_plate',
        'POLISHED_BLACKSTONE_SLAB' => 'minecraft:polished_blackstone_slab',
        'POLISHED_BLACKSTONE_STAIRS' => 'minecraft:polished_blackstone_stairs',
        'POLISHED_BLACKSTONE_WALL' => 'minecraft:polished_cinnabar_wall',
        'POLISHED_CINNABAR' => 'minecraft:polished_cinnabar',
        'POLISHED_CINNABAR_SLAB' => 'minecraft:polished_cinnabar_slab',
        'POLISHED_CINNABAR_STAIRS' => 'minecraft:polished_cinnabar_stairs',
        'POLISHED_CINNABAR_WALL' => 'minecraft:polished_deepslate_wall',
        'POLISHED_DEEPSLATE' => 'minecraft:polished_deepslate',
        'POLISHED_DEEPSLATE_SLAB' => 'minecraft:polished_deepslate_slab',
        'POLISHED_DEEPSLATE_STAIRS' => 'minecraft:polished_deepslate_stairs',
        'POLISHED_DEEPSLATE_WALL' => 'minecraft:polished_sulfur_wall',
        'POLISHED_DIORITE' => 'minecraft:polished_diorite',
        'POLISHED_DIORITE_SLAB' => 'minecraft:polished_diorite_slab',
        'POLISHED_DIORITE_STAIRS' => 'minecraft:polished_diorite_stairs',
        'POLISHED_GRANITE' => 'minecraft:polished_granite',
        'POLISHED_GRANITE_SLAB' => 'minecraft:polished_granite_slab',
        'POLISHED_GRANITE_STAIRS' => 'minecraft:polished_granite_stairs',
        'POLISHED_SULFUR' => 'minecraft:polished_sulfur',
        'POLISHED_SULFUR_SLAB' => 'minecraft:polished_sulfur_slab',
        'POLISHED_SULFUR_STAIRS' => 'minecraft:polished_sulfur_stairs',
        'POLISHED_SULFUR_WALL' => 'minecraft:polished_tuff_wall',
        'POLISHED_TUFF' => 'minecraft:polished_tuff',
        'POLISHED_TUFF_SLAB' => 'minecraft:polished_tuff_slab',
        'POLISHED_TUFF_STAIRS' => 'minecraft:polished_tuff_stairs',
        'POLISHED_TUFF_WALL' => 'minecraft:prismarine_wall',
        'POPPY' => 'minecraft:poppy',
        'POTATOES' => 'minecraft:potatoes',
        'POTENT_SULFUR' => 'minecraft:potent_sulfur',
        'POTION_CAULDRON' => 'minecraft:cauldron',
        'POWERED_RAIL' => 'minecraft:golden_rail',
        'PRISMARINE' => 'minecraft:prismarine',
        'PRISMARINE_BRICKS' => 'minecraft:prismarine_bricks',
        'PRISMARINE_BRICKS_SLAB' => 'minecraft:prismarine_brick_slab',
        'PRISMARINE_BRICKS_STAIRS' => 'minecraft:prismarine_bricks_stairs',
        'PRISMARINE_SLAB' => 'minecraft:prismarine_slab',
        'PRISMARINE_STAIRS' => 'minecraft:prismarine_stairs',
        'PRISMARINE_WALL' => 'minecraft:red_nether_brick_wall',
        'PUMPKIN' => 'minecraft:pumpkin',
        'PUMPKIN_STEM' => 'minecraft:pumpkin_stem',
        'PURPLE_TORCH' => 'minecraft:colored_torch_purple',
        'PURPUR' => 'minecraft:purpur_block',
        'PURPUR_PILLAR' => 'minecraft:purpur_pillar',
        'PURPUR_SLAB' => 'minecraft:purpur_slab',
        'PURPUR_STAIRS' => 'minecraft:purpur_stairs',
        'QUARTZ' => 'minecraft:quartz_block',
        'QUARTZ_BRICKS' => 'minecraft:quartz_bricks',
        'QUARTZ_PILLAR' => 'minecraft:quartz_pillar',
        'QUARTZ_SLAB' => 'minecraft:quartz_slab',
        'QUARTZ_STAIRS' => 'minecraft:quartz_stairs',
        'RAIL' => 'minecraft:rail',
        'RAW_COPPER' => 'minecraft:raw_copper_block',
        'RAW_GOLD' => 'minecraft:raw_gold_block',
        'RAW_IRON' => 'minecraft:raw_iron_block',
        'REDSTONE' => 'minecraft:redstone_block',
        'REDSTONE_COMPARATOR' => 'minecraft:unpowered_comparator',
        'REDSTONE_LAMP' => 'minecraft:redstone_lamp',
        'REDSTONE_ORE' => 'minecraft:redstone_ore',
        'REDSTONE_REPEATER' => 'minecraft:unpowered_repeater',
        'REDSTONE_TORCH' => 'minecraft:redstone_torch',
        'REDSTONE_WIRE' => 'minecraft:redstone_wire',
        'RED_MUSHROOM' => 'minecraft:red_mushroom',
        'RED_MUSHROOM_BLOCK' => 'minecraft:red_mushroom_block',
        'RED_NETHER_BRICKS' => 'minecraft:red_nether_brick',
        'RED_NETHER_BRICK_SLAB' => 'minecraft:red_nether_brick_slab',
        'RED_NETHER_BRICK_STAIRS' => 'minecraft:red_nether_brick_stairs',
        'RED_NETHER_BRICK_WALL' => 'minecraft:red_sandstone_wall',
        'RED_SAND' => 'minecraft:red_sand',
        'RED_SANDSTONE' => 'minecraft:red_sandstone',
        'RED_SANDSTONE_SLAB' => 'minecraft:red_sandstone_slab',
        'RED_SANDSTONE_STAIRS' => 'minecraft:red_sandstone_stairs',
        'RED_SANDSTONE_WALL' => 'minecraft:resin_brick_wall',
        'RED_TORCH' => 'minecraft:colored_torch_red',
        'RED_TULIP' => 'minecraft:red_tulip',
        'REINFORCED_DEEPSLATE' => 'minecraft:reinforced_deepslate',
        'RESERVED6' => 'minecraft:reserved6',
        'RESIN' => 'minecraft:resin_block',
        'RESIN_BRICKS' => 'minecraft:resin_bricks',
        'RESIN_BRICK_SLAB' => 'minecraft:resin_brick_slab',
        'RESIN_BRICK_STAIRS' => 'minecraft:resin_brick_stairs',
        'RESIN_BRICK_WALL' => 'minecraft:sandstone_wall',
        'RESIN_CLUMP' => 'minecraft:resin_clump',
        'RESPAWN_ANCHOR' => 'minecraft:respawn_anchor',
        'ROSE_BUSH' => 'minecraft:rose_bush',
        'SAND' => 'minecraft:sand',
        'SANDSTONE' => 'minecraft:sandstone',
        'SANDSTONE_SLAB' => 'minecraft:sandstone_slab',
        'SANDSTONE_STAIRS' => 'minecraft:sandstone_stairs',
        'SANDSTONE_WALL' => 'minecraft:stone_brick_wall',
        'SCULK' => 'minecraft:sculk',
        'SEA_LANTERN' => 'minecraft:sea_lantern',
        'SEA_PICKLE' => 'minecraft:sea_pickle',
        'SHROOMLIGHT' => 'minecraft:shroomlight',
        'SHULKER_BOX' => 'minecraft:undyed_shulker_box',
        'SLIME' => 'minecraft:slime',
        'SMALL_DRIPLEAF' => 'minecraft:small_dripleaf_block',
        'SMITHING_TABLE' => 'minecraft:smithing_table',
        'SMOKER' => 'minecraft:smoker',
        'SMOOTH_BASALT' => 'minecraft:smooth_basalt',
        'SMOOTH_QUARTZ' => 'minecraft:smooth_quartz',
        'SMOOTH_QUARTZ_SLAB' => 'minecraft:smooth_quartz_slab',
        'SMOOTH_QUARTZ_STAIRS' => 'minecraft:smooth_quartz_stairs',
        'SMOOTH_RED_SANDSTONE' => 'minecraft:smooth_red_sandstone',
        'SMOOTH_RED_SANDSTONE_SLAB' => 'minecraft:smooth_red_sandstone_slab',
        'SMOOTH_RED_SANDSTONE_STAIRS' => 'minecraft:smooth_red_sandstone_stairs',
        'SMOOTH_SANDSTONE' => 'minecraft:smooth_sandstone',
        'SMOOTH_SANDSTONE_SLAB' => 'minecraft:smooth_sandstone_slab',
        'SMOOTH_SANDSTONE_STAIRS' => 'minecraft:smooth_sandstone_stairs',
        'SMOOTH_STONE' => 'minecraft:smooth_stone',
        'SMOOTH_STONE_SLAB' => 'minecraft:smooth_stone_slab',
        'SNOW' => 'minecraft:snow',
        'SNOW_LAYER' => 'minecraft:snow_layer',
        'SOUL_CAMPFIRE' => 'minecraft:soul_campfire',
        'SOUL_FIRE' => 'minecraft:soul_fire',
        'SOUL_LANTERN' => 'minecraft:soul_lantern',
        'SOUL_SAND' => 'minecraft:soul_sand',
        'SOUL_SOIL' => 'minecraft:soul_soil',
        'SOUL_TORCH' => 'minecraft:soul_torch',
        'SPONGE' => 'minecraft:sponge',
        'SPORE_BLOSSOM' => 'minecraft:spore_blossom',
        'SPRUCE_BUTTON' => 'minecraft:spruce_button',
        'SPRUCE_CEILING_CENTER_HANGING_SIGN' => 'minecraft:warped_hanging_sign',
        'SPRUCE_CEILING_EDGES_HANGING_SIGN' => 'minecraft:spruce_hanging_sign',
        'SPRUCE_DOOR' => 'minecraft:spruce_door',
        'SPRUCE_FENCE' => 'minecraft:spruce_fence',
        'SPRUCE_FENCE_GATE' => 'minecraft:spruce_fence_gate',
        'SPRUCE_LEAVES' => 'minecraft:spruce_leaves',
        'SPRUCE_LOG' => 'minecraft:spruce_log',
        'SPRUCE_PLANKS' => 'minecraft:spruce_planks',
        'SPRUCE_PRESSURE_PLATE' => 'minecraft:spruce_pressure_plate',
        'SPRUCE_SAPLING' => 'minecraft:spruce_sapling',
        'SPRUCE_SIGN' => 'minecraft:spruce_standing_sign',
        'SPRUCE_SLAB' => 'minecraft:spruce_slab',
        'SPRUCE_STAIRS' => 'minecraft:spruce_stairs',
        'SPRUCE_TRAPDOOR' => 'minecraft:spruce_trapdoor',
        'SPRUCE_WALL_HANGING_SIGN' => 'minecraft:spruce_hanging_sign',
        'SPRUCE_WALL_SIGN' => 'minecraft:spruce_wall_sign',
        'SPRUCE_WOOD' => 'minecraft:spruce_wood',
        'STAINED_CLAY' => 'minecraft:stained_hardened_clay',
        'STAINED_GLASS' => 'minecraft:stained_glass',
        'STAINED_GLASS_PANE' => 'minecraft:stained_glass_pane',
        'STAINED_HARDENED_GLASS' => 'minecraft:hard_stained_glass',
        'STAINED_HARDENED_GLASS_PANE' => 'minecraft:hard_stained_glass_pane',
        'STONE' => 'minecraft:stone',
        'STONECUTTER' => 'minecraft:stonecutter_block',
        'STONE_BRICKS' => 'minecraft:stone_bricks',
        'STONE_BRICK_SLAB' => 'minecraft:stone_brick_slab',
        'STONE_BRICK_STAIRS' => 'minecraft:stone_brick_stairs',
        'STONE_BRICK_WALL' => 'minecraft:sulfur_brick_wall',
        'STONE_BUTTON' => 'minecraft:stone_button',
        'STONE_PRESSURE_PLATE' => 'minecraft:stone_pressure_plate',
        'STONE_SLAB' => 'minecraft:stone_slab',
        'STONE_STAIRS' => 'minecraft:normal_stone_stairs',
        'STRUCTURE_VOID' => 'minecraft:structure_void',
        'SUGARCANE' => 'minecraft:reeds',
        'SULFUR' => 'minecraft:sulfur',
        'SULFUR_BRICKS' => 'minecraft:sulfur_bricks',
        'SULFUR_BRICK_SLAB' => 'minecraft:sulfur_brick_slab',
        'SULFUR_BRICK_STAIRS' => 'minecraft:sulfur_brick_stairs',
        'SULFUR_BRICK_WALL' => 'minecraft:sulfur_wall',
        'SULFUR_SLAB' => 'minecraft:sulfur_slab',
        'SULFUR_STAIRS' => 'minecraft:sulfur_stairs',
        'SULFUR_WALL' => 'minecraft:tuff_brick_wall',
        'SUNFLOWER' => 'minecraft:sunflower',
        'SWEET_BERRY_BUSH' => 'minecraft:sweet_berry_bush',
        'TALL_GRASS' => 'minecraft:short_grass',
        'TINTED_GLASS' => 'minecraft:tinted_glass',
        'TNT' => 'minecraft:tnt',
        'TORCH' => 'minecraft:torch',
        'TORCHFLOWER' => 'minecraft:torchflower',
        'TORCHFLOWER_CROP' => 'minecraft:torchflower_crop',
        'TRAPPED_CHEST' => 'minecraft:trapped_chest',
        'TRIPWIRE' => 'minecraft:trip_wire',
        'TRIPWIRE_HOOK' => 'minecraft:tripwire_hook',
        'TUFF' => 'minecraft:tuff',
        'TUFF_BRICKS' => 'minecraft:tuff_bricks',
        'TUFF_BRICK_SLAB' => 'minecraft:tuff_brick_slab',
        'TUFF_BRICK_STAIRS' => 'minecraft:tuff_brick_stairs',
        'TUFF_BRICK_WALL' => 'minecraft:tuff_wall',
        'TUFF_SLAB' => 'minecraft:tuff_slab',
        'TUFF_STAIRS' => 'minecraft:tuff_stairs',
        'TUFF_WALL' => 'minecraft:tuff_wall',
        'TWISTING_VINES' => 'minecraft:twisting_vines',
        'UNDERWATER_TORCH' => 'minecraft:underwater_torch',
        'VINES' => 'minecraft:vine',
        'WALL_BANNER' => 'minecraft:wall_banner',
        'WALL_CORAL_FAN' => 'minecraft:coral_fan_hang',
        'WARPED_BUTTON' => 'minecraft:warped_button',
        'WARPED_CEILING_CENTER_HANGING_SIGN' => 'minecraft:warped_hanging_sign',
        'WARPED_CEILING_EDGES_HANGING_SIGN' => 'minecraft:warped_hanging_sign',
        'WARPED_DOOR' => 'minecraft:warped_door',
        'WARPED_FENCE' => 'minecraft:warped_fence',
        'WARPED_FENCE_GATE' => 'minecraft:warped_fence_gate',
        'WARPED_FUNGUS' => 'minecraft:warped_fungus',
        'WARPED_HYPHAE' => 'minecraft:warped_hyphae',
        'WARPED_NYLIUM' => 'minecraft:warped_nylium',
        'WARPED_PLANKS' => 'minecraft:warped_planks',
        'WARPED_PRESSURE_PLATE' => 'minecraft:warped_pressure_plate',
        'WARPED_ROOTS' => 'minecraft:warped_roots',
        'WARPED_SIGN' => 'minecraft:warped_standing_sign',
        'WARPED_SLAB' => 'minecraft:warped_slab',
        'WARPED_STAIRS' => 'minecraft:warped_stairs',
        'WARPED_STEM' => 'minecraft:warped_stem',
        'WARPED_TRAPDOOR' => 'minecraft:warped_trapdoor',
        'WARPED_WALL_HANGING_SIGN' => 'minecraft:warped_hanging_sign',
        'WARPED_WALL_SIGN' => 'minecraft:warped_wall_sign',
        'WARPED_WART_BLOCK' => 'minecraft:warped_wart_block',
        'WATER' => 'minecraft:water',
        'WATER_CAULDRON' => 'minecraft:cauldron',
        'WEEPING_VINES' => 'minecraft:weeping_vines',
        'WEIGHTED_PRESSURE_PLATE_HEAVY' => 'minecraft:heavy_weighted_pressure_plate',
        'WEIGHTED_PRESSURE_PLATE_LIGHT' => 'minecraft:light_weighted_pressure_plate',
        'WET_POTENT_SULFUR' => 'minecraft:potent_sulfur',
        'WHEAT' => 'minecraft:wheat',
        'WHITE_TULIP' => 'minecraft:white_tulip',
        'WITHER_ROSE' => 'minecraft:wither_rose',
        'WOOL' => 'minecraft:wool',
    ];

    private function __construct()
    {
    }

    /**
     * Resolves a block by name, case-insensitively, supporting camelCase, snake_case, and aliases.
     */
    public static function get(string $name): Block
    {
        if (preg_match('/[a-z][A-Z]/', $name)) {
            $name = (string) preg_replace('/(?<!^)[A-Z]/', '_$0', $name);
        }
        $key = strtoupper(str_replace(['minecraft:', ' '], ['', '_'], $name));
        $canonical = self::ALIASES[$key] ?? $key;

        if (!isset(self::$blocks[$canonical])) {
            $blockName = self::BLOCK_NAMES[$canonical] ?? ('minecraft:' . strtolower($canonical));
            self::$blocks[$canonical] = Block::fromName($blockName);
        }
        return clone self::$blocks[$canonical];
    }

    /**
     * Dynamic fallback for any method call (supports aliases, snake_case, camelCase, lowercase).
     */
    public static function __callStatic(string $name, array $args): Block
    {
        return self::get($name);
    }

    /**
     * Returns an array of all registered vanilla blocks (indexed by uppercase canonical name).
     *
     * @return array<string, Block>
     */
    public static function getAll(): array
    {
        $all = [];
        foreach (self::BLOCK_NAMES as $key => $blockName) {
            $all[$key] = self::get($key);
        }
        return $all;
    }

    // ------------------------------------------------------------------------
    // Explicit static getter methods for all 830 vanilla blocks
    // ------------------------------------------------------------------------
    public static function ACACIA_BUTTON(): Block { return self::get('ACACIA_BUTTON'); }
    public static function ACACIA_CEILING_CENTER_HANGING_SIGN(): Block { return self::get('ACACIA_CEILING_CENTER_HANGING_SIGN'); }
    public static function ACACIA_CEILING_EDGES_HANGING_SIGN(): Block { return self::get('ACACIA_CEILING_EDGES_HANGING_SIGN'); }
    public static function ACACIA_DOOR(): Block { return self::get('ACACIA_DOOR'); }
    public static function ACACIA_FENCE(): Block { return self::get('ACACIA_FENCE'); }
    public static function ACACIA_FENCE_GATE(): Block { return self::get('ACACIA_FENCE_GATE'); }
    public static function ACACIA_LEAVES(): Block { return self::get('ACACIA_LEAVES'); }
    public static function ACACIA_LOG(): Block { return self::get('ACACIA_LOG'); }
    public static function ACACIA_PLANKS(): Block { return self::get('ACACIA_PLANKS'); }
    public static function ACACIA_PRESSURE_PLATE(): Block { return self::get('ACACIA_PRESSURE_PLATE'); }
    public static function ACACIA_SAPLING(): Block { return self::get('ACACIA_SAPLING'); }
    public static function ACACIA_SIGN(): Block { return self::get('ACACIA_SIGN'); }
    public static function ACACIA_SLAB(): Block { return self::get('ACACIA_SLAB'); }
    public static function ACACIA_STAIRS(): Block { return self::get('ACACIA_STAIRS'); }
    public static function ACACIA_TRAPDOOR(): Block { return self::get('ACACIA_TRAPDOOR'); }
    public static function ACACIA_WALL_HANGING_SIGN(): Block { return self::get('ACACIA_WALL_HANGING_SIGN'); }
    public static function ACACIA_WALL_SIGN(): Block { return self::get('ACACIA_WALL_SIGN'); }
    public static function ACACIA_WOOD(): Block { return self::get('ACACIA_WOOD'); }
    public static function ACTIVATOR_RAIL(): Block { return self::get('ACTIVATOR_RAIL'); }
    public static function AIR(): Block { return self::get('AIR'); }
    public static function ALLIUM(): Block { return self::get('ALLIUM'); }
    public static function ALL_SIDED_MUSHROOM_STEM(): Block { return self::get('ALL_SIDED_MUSHROOM_STEM'); }
    public static function AMETHYST(): Block { return self::get('AMETHYST'); }
    public static function AMETHYST_CLUSTER(): Block { return self::get('AMETHYST_CLUSTER'); }
    public static function ANCIENT_DEBRIS(): Block { return self::get('ANCIENT_DEBRIS'); }
    public static function ANDESITE(): Block { return self::get('ANDESITE'); }
    public static function ANDESITE_SLAB(): Block { return self::get('ANDESITE_SLAB'); }
    public static function ANDESITE_STAIRS(): Block { return self::get('ANDESITE_STAIRS'); }
    public static function ANDESITE_WALL(): Block { return self::get('ANDESITE_WALL'); }
    public static function ANVIL(): Block { return self::get('ANVIL'); }
    public static function AZALEA(): Block { return self::get('AZALEA'); }
    public static function AZALEA_LEAVES(): Block { return self::get('AZALEA_LEAVES'); }
    public static function AZURE_BLUET(): Block { return self::get('AZURE_BLUET'); }
    public static function BAMBOO(): Block { return self::get('BAMBOO'); }
    public static function BAMBOO_BLOCK(): Block { return self::get('BAMBOO_BLOCK'); }
    public static function BAMBOO_BUTTON(): Block { return self::get('BAMBOO_BUTTON'); }
    public static function BAMBOO_CEILING_CENTER_HANGING_SIGN(): Block { return self::get('BAMBOO_CEILING_CENTER_HANGING_SIGN'); }
    public static function BAMBOO_CEILING_EDGES_HANGING_SIGN(): Block { return self::get('BAMBOO_CEILING_EDGES_HANGING_SIGN'); }
    public static function BAMBOO_DOOR(): Block { return self::get('BAMBOO_DOOR'); }
    public static function BAMBOO_FENCE(): Block { return self::get('BAMBOO_FENCE'); }
    public static function BAMBOO_FENCE_GATE(): Block { return self::get('BAMBOO_FENCE_GATE'); }
    public static function BAMBOO_MOSAIC(): Block { return self::get('BAMBOO_MOSAIC'); }
    public static function BAMBOO_MOSAIC_SLAB(): Block { return self::get('BAMBOO_MOSAIC_SLAB'); }
    public static function BAMBOO_MOSAIC_STAIRS(): Block { return self::get('BAMBOO_MOSAIC_STAIRS'); }
    public static function BAMBOO_PLANKS(): Block { return self::get('BAMBOO_PLANKS'); }
    public static function BAMBOO_PRESSURE_PLATE(): Block { return self::get('BAMBOO_PRESSURE_PLATE'); }
    public static function BAMBOO_SAPLING(): Block { return self::get('BAMBOO_SAPLING'); }
    public static function BAMBOO_SIGN(): Block { return self::get('BAMBOO_SIGN'); }
    public static function BAMBOO_SLAB(): Block { return self::get('BAMBOO_SLAB'); }
    public static function BAMBOO_STAIRS(): Block { return self::get('BAMBOO_STAIRS'); }
    public static function BAMBOO_TRAPDOOR(): Block { return self::get('BAMBOO_TRAPDOOR'); }
    public static function BAMBOO_WALL_HANGING_SIGN(): Block { return self::get('BAMBOO_WALL_HANGING_SIGN'); }
    public static function BAMBOO_WALL_SIGN(): Block { return self::get('BAMBOO_WALL_SIGN'); }
    public static function BANNER(): Block { return self::get('BANNER'); }
    public static function BARREL(): Block { return self::get('BARREL'); }
    public static function BARRIER(): Block { return self::get('BARRIER'); }
    public static function BASALT(): Block { return self::get('BASALT'); }
    public static function BEACON(): Block { return self::get('BEACON'); }
    public static function BED(): Block { return self::get('BED'); }
    public static function BEDROCK(): Block { return self::get('BEDROCK'); }
    public static function BEETROOTS(): Block { return self::get('BEETROOTS'); }
    public static function BELL(): Block { return self::get('BELL'); }
    public static function BIG_DRIPLEAF_HEAD(): Block { return self::get('BIG_DRIPLEAF_HEAD'); }
    public static function BIG_DRIPLEAF_STEM(): Block { return self::get('BIG_DRIPLEAF_STEM'); }
    public static function BIRCH_BUTTON(): Block { return self::get('BIRCH_BUTTON'); }
    public static function BIRCH_CEILING_CENTER_HANGING_SIGN(): Block { return self::get('BIRCH_CEILING_CENTER_HANGING_SIGN'); }
    public static function BIRCH_CEILING_EDGES_HANGING_SIGN(): Block { return self::get('BIRCH_CEILING_EDGES_HANGING_SIGN'); }
    public static function BIRCH_DOOR(): Block { return self::get('BIRCH_DOOR'); }
    public static function BIRCH_FENCE(): Block { return self::get('BIRCH_FENCE'); }
    public static function BIRCH_FENCE_GATE(): Block { return self::get('BIRCH_FENCE_GATE'); }
    public static function BIRCH_LEAVES(): Block { return self::get('BIRCH_LEAVES'); }
    public static function BIRCH_LOG(): Block { return self::get('BIRCH_LOG'); }
    public static function BIRCH_PLANKS(): Block { return self::get('BIRCH_PLANKS'); }
    public static function BIRCH_PRESSURE_PLATE(): Block { return self::get('BIRCH_PRESSURE_PLATE'); }
    public static function BIRCH_SAPLING(): Block { return self::get('BIRCH_SAPLING'); }
    public static function BIRCH_SIGN(): Block { return self::get('BIRCH_SIGN'); }
    public static function BIRCH_SLAB(): Block { return self::get('BIRCH_SLAB'); }
    public static function BIRCH_STAIRS(): Block { return self::get('BIRCH_STAIRS'); }
    public static function BIRCH_TRAPDOOR(): Block { return self::get('BIRCH_TRAPDOOR'); }
    public static function BIRCH_WALL_HANGING_SIGN(): Block { return self::get('BIRCH_WALL_HANGING_SIGN'); }
    public static function BIRCH_WALL_SIGN(): Block { return self::get('BIRCH_WALL_SIGN'); }
    public static function BIRCH_WOOD(): Block { return self::get('BIRCH_WOOD'); }
    public static function BLACKSTONE(): Block { return self::get('BLACKSTONE'); }
    public static function BLACKSTONE_SLAB(): Block { return self::get('BLACKSTONE_SLAB'); }
    public static function BLACKSTONE_STAIRS(): Block { return self::get('BLACKSTONE_STAIRS'); }
    public static function BLACKSTONE_WALL(): Block { return self::get('BLACKSTONE_WALL'); }
    public static function BLAST_FURNACE(): Block { return self::get('BLAST_FURNACE'); }
    public static function BLUE_ICE(): Block { return self::get('BLUE_ICE'); }
    public static function BLUE_ORCHID(): Block { return self::get('BLUE_ORCHID'); }
    public static function BLUE_TORCH(): Block { return self::get('BLUE_TORCH'); }
    public static function BONE_BLOCK(): Block { return self::get('BONE_BLOCK'); }
    public static function BOOKSHELF(): Block { return self::get('BOOKSHELF'); }
    public static function BREWING_STAND(): Block { return self::get('BREWING_STAND'); }
    public static function BRICKS(): Block { return self::get('BRICKS'); }
    public static function BRICK_SLAB(): Block { return self::get('BRICK_SLAB'); }
    public static function BRICK_STAIRS(): Block { return self::get('BRICK_STAIRS'); }
    public static function BRICK_WALL(): Block { return self::get('BRICK_WALL'); }
    public static function BROWN_MUSHROOM(): Block { return self::get('BROWN_MUSHROOM'); }
    public static function BROWN_MUSHROOM_BLOCK(): Block { return self::get('BROWN_MUSHROOM_BLOCK'); }
    public static function BUDDING_AMETHYST(): Block { return self::get('BUDDING_AMETHYST'); }
    public static function CACTUS(): Block { return self::get('CACTUS'); }
    public static function CACTUS_FLOWER(): Block { return self::get('CACTUS_FLOWER'); }
    public static function CAKE(): Block { return self::get('CAKE'); }
    public static function CAKE_WITH_CANDLE(): Block { return self::get('CAKE_WITH_CANDLE'); }
    public static function CAKE_WITH_DYED_CANDLE(): Block { return self::get('CAKE_WITH_DYED_CANDLE'); }
    public static function CALCITE(): Block { return self::get('CALCITE'); }
    public static function CAMPFIRE(): Block { return self::get('CAMPFIRE'); }
    public static function CANDLE(): Block { return self::get('CANDLE'); }
    public static function CARPET(): Block { return self::get('CARPET'); }
    public static function CARROTS(): Block { return self::get('CARROTS'); }
    public static function CARTOGRAPHY_TABLE(): Block { return self::get('CARTOGRAPHY_TABLE'); }
    public static function CARVED_PUMPKIN(): Block { return self::get('CARVED_PUMPKIN'); }
    public static function CAULDRON(): Block { return self::get('CAULDRON'); }
    public static function CAVE_VINES(): Block { return self::get('CAVE_VINES'); }
    public static function CHAIN(): Block { return self::get('CHAIN'); }
    public static function CHEMICAL_HEAT(): Block { return self::get('CHEMICAL_HEAT'); }
    public static function CHERRY_BUTTON(): Block { return self::get('CHERRY_BUTTON'); }
    public static function CHERRY_CEILING_CENTER_HANGING_SIGN(): Block { return self::get('CHERRY_CEILING_CENTER_HANGING_SIGN'); }
    public static function CHERRY_CEILING_EDGES_HANGING_SIGN(): Block { return self::get('CHERRY_CEILING_EDGES_HANGING_SIGN'); }
    public static function CHERRY_DOOR(): Block { return self::get('CHERRY_DOOR'); }
    public static function CHERRY_FENCE(): Block { return self::get('CHERRY_FENCE'); }
    public static function CHERRY_FENCE_GATE(): Block { return self::get('CHERRY_FENCE_GATE'); }
    public static function CHERRY_LEAVES(): Block { return self::get('CHERRY_LEAVES'); }
    public static function CHERRY_LOG(): Block { return self::get('CHERRY_LOG'); }
    public static function CHERRY_PLANKS(): Block { return self::get('CHERRY_PLANKS'); }
    public static function CHERRY_PRESSURE_PLATE(): Block { return self::get('CHERRY_PRESSURE_PLATE'); }
    public static function CHERRY_SIGN(): Block { return self::get('CHERRY_SIGN'); }
    public static function CHERRY_SLAB(): Block { return self::get('CHERRY_SLAB'); }
    public static function CHERRY_STAIRS(): Block { return self::get('CHERRY_STAIRS'); }
    public static function CHERRY_TRAPDOOR(): Block { return self::get('CHERRY_TRAPDOOR'); }
    public static function CHERRY_WALL_HANGING_SIGN(): Block { return self::get('CHERRY_WALL_HANGING_SIGN'); }
    public static function CHERRY_WALL_SIGN(): Block { return self::get('CHERRY_WALL_SIGN'); }
    public static function CHERRY_WOOD(): Block { return self::get('CHERRY_WOOD'); }
    public static function CHEST(): Block { return self::get('CHEST'); }
    public static function CHISELED_BOOKSHELF(): Block { return self::get('CHISELED_BOOKSHELF'); }
    public static function CHISELED_CINNABAR(): Block { return self::get('CHISELED_CINNABAR'); }
    public static function CHISELED_COPPER(): Block { return self::get('CHISELED_COPPER'); }
    public static function CHISELED_DEEPSLATE(): Block { return self::get('CHISELED_DEEPSLATE'); }
    public static function CHISELED_NETHER_BRICKS(): Block { return self::get('CHISELED_NETHER_BRICKS'); }
    public static function CHISELED_POLISHED_BLACKSTONE(): Block { return self::get('CHISELED_POLISHED_BLACKSTONE'); }
    public static function CHISELED_QUARTZ(): Block { return self::get('CHISELED_QUARTZ'); }
    public static function CHISELED_RED_SANDSTONE(): Block { return self::get('CHISELED_RED_SANDSTONE'); }
    public static function CHISELED_RESIN_BRICKS(): Block { return self::get('CHISELED_RESIN_BRICKS'); }
    public static function CHISELED_SANDSTONE(): Block { return self::get('CHISELED_SANDSTONE'); }
    public static function CHISELED_STONE_BRICKS(): Block { return self::get('CHISELED_STONE_BRICKS'); }
    public static function CHISELED_SULFUR(): Block { return self::get('CHISELED_SULFUR'); }
    public static function CHISELED_TUFF(): Block { return self::get('CHISELED_TUFF'); }
    public static function CHISELED_TUFF_BRICKS(): Block { return self::get('CHISELED_TUFF_BRICKS'); }
    public static function CHORUS_FLOWER(): Block { return self::get('CHORUS_FLOWER'); }
    public static function CHORUS_PLANT(): Block { return self::get('CHORUS_PLANT'); }
    public static function CINNABAR(): Block { return self::get('CINNABAR'); }
    public static function CINNABAR_BRICKS(): Block { return self::get('CINNABAR_BRICKS'); }
    public static function CINNABAR_BRICK_SLAB(): Block { return self::get('CINNABAR_BRICK_SLAB'); }
    public static function CINNABAR_BRICK_STAIRS(): Block { return self::get('CINNABAR_BRICK_STAIRS'); }
    public static function CINNABAR_BRICK_WALL(): Block { return self::get('CINNABAR_BRICK_WALL'); }
    public static function CINNABAR_SLAB(): Block { return self::get('CINNABAR_SLAB'); }
    public static function CINNABAR_STAIRS(): Block { return self::get('CINNABAR_STAIRS'); }
    public static function CINNABAR_WALL(): Block { return self::get('CINNABAR_WALL'); }
    public static function CLAY(): Block { return self::get('CLAY'); }
    public static function COAL(): Block { return self::get('COAL'); }
    public static function COAL_ORE(): Block { return self::get('COAL_ORE'); }
    public static function COBBLED_DEEPSLATE(): Block { return self::get('COBBLED_DEEPSLATE'); }
    public static function COBBLED_DEEPSLATE_SLAB(): Block { return self::get('COBBLED_DEEPSLATE_SLAB'); }
    public static function COBBLED_DEEPSLATE_STAIRS(): Block { return self::get('COBBLED_DEEPSLATE_STAIRS'); }
    public static function COBBLED_DEEPSLATE_WALL(): Block { return self::get('COBBLED_DEEPSLATE_WALL'); }
    public static function COBBLESTONE(): Block { return self::get('COBBLESTONE'); }
    public static function COBBLESTONE_SLAB(): Block { return self::get('COBBLESTONE_SLAB'); }
    public static function COBBLESTONE_STAIRS(): Block { return self::get('COBBLESTONE_STAIRS'); }
    public static function COBBLESTONE_WALL(): Block { return self::get('COBBLESTONE_WALL'); }
    public static function COBWEB(): Block { return self::get('COBWEB'); }
    public static function COCOA_POD(): Block { return self::get('COCOA_POD'); }
    public static function COMPOUND_CREATOR(): Block { return self::get('COMPOUND_CREATOR'); }
    public static function CONCRETE(): Block { return self::get('CONCRETE'); }
    public static function CONCRETE_POWDER(): Block { return self::get('CONCRETE_POWDER'); }
    public static function CONTINUOUS_POTENT_SULFUR(): Block { return self::get('CONTINUOUS_POTENT_SULFUR'); }
    public static function COPPER(): Block { return self::get('COPPER'); }
    public static function COPPER_BARS(): Block { return self::get('COPPER_BARS'); }
    public static function COPPER_BULB(): Block { return self::get('COPPER_BULB'); }
    public static function COPPER_CHAIN(): Block { return self::get('COPPER_CHAIN'); }
    public static function COPPER_DOOR(): Block { return self::get('COPPER_DOOR'); }
    public static function COPPER_GRATE(): Block { return self::get('COPPER_GRATE'); }
    public static function COPPER_LANTERN(): Block { return self::get('COPPER_LANTERN'); }
    public static function COPPER_ORE(): Block { return self::get('COPPER_ORE'); }
    public static function COPPER_TORCH(): Block { return self::get('COPPER_TORCH'); }
    public static function COPPER_TRAPDOOR(): Block { return self::get('COPPER_TRAPDOOR'); }
    public static function CORAL(): Block { return self::get('CORAL'); }
    public static function CORAL_BLOCK(): Block { return self::get('CORAL_BLOCK'); }
    public static function CORAL_FAN(): Block { return self::get('CORAL_FAN'); }
    public static function CORNFLOWER(): Block { return self::get('CORNFLOWER'); }
    public static function CRACKED_DEEPSLATE_BRICKS(): Block { return self::get('CRACKED_DEEPSLATE_BRICKS'); }
    public static function CRACKED_DEEPSLATE_TILES(): Block { return self::get('CRACKED_DEEPSLATE_TILES'); }
    public static function CRACKED_NETHER_BRICKS(): Block { return self::get('CRACKED_NETHER_BRICKS'); }
    public static function CRACKED_POLISHED_BLACKSTONE_BRICKS(): Block { return self::get('CRACKED_POLISHED_BLACKSTONE_BRICKS'); }
    public static function CRACKED_STONE_BRICKS(): Block { return self::get('CRACKED_STONE_BRICKS'); }
    public static function CRAFTING_TABLE(): Block { return self::get('CRAFTING_TABLE'); }
    public static function CRIMSON_BUTTON(): Block { return self::get('CRIMSON_BUTTON'); }
    public static function CRIMSON_CEILING_CENTER_HANGING_SIGN(): Block { return self::get('CRIMSON_CEILING_CENTER_HANGING_SIGN'); }
    public static function CRIMSON_CEILING_EDGES_HANGING_SIGN(): Block { return self::get('CRIMSON_CEILING_EDGES_HANGING_SIGN'); }
    public static function CRIMSON_DOOR(): Block { return self::get('CRIMSON_DOOR'); }
    public static function CRIMSON_FENCE(): Block { return self::get('CRIMSON_FENCE'); }
    public static function CRIMSON_FENCE_GATE(): Block { return self::get('CRIMSON_FENCE_GATE'); }
    public static function CRIMSON_FUNGUS(): Block { return self::get('CRIMSON_FUNGUS'); }
    public static function CRIMSON_HYPHAE(): Block { return self::get('CRIMSON_HYPHAE'); }
    public static function CRIMSON_NYLIUM(): Block { return self::get('CRIMSON_NYLIUM'); }
    public static function CRIMSON_PLANKS(): Block { return self::get('CRIMSON_PLANKS'); }
    public static function CRIMSON_PRESSURE_PLATE(): Block { return self::get('CRIMSON_PRESSURE_PLATE'); }
    public static function CRIMSON_ROOTS(): Block { return self::get('CRIMSON_ROOTS'); }
    public static function CRIMSON_SIGN(): Block { return self::get('CRIMSON_SIGN'); }
    public static function CRIMSON_SLAB(): Block { return self::get('CRIMSON_SLAB'); }
    public static function CRIMSON_STAIRS(): Block { return self::get('CRIMSON_STAIRS'); }
    public static function CRIMSON_STEM(): Block { return self::get('CRIMSON_STEM'); }
    public static function CRIMSON_TRAPDOOR(): Block { return self::get('CRIMSON_TRAPDOOR'); }
    public static function CRIMSON_WALL_HANGING_SIGN(): Block { return self::get('CRIMSON_WALL_HANGING_SIGN'); }
    public static function CRIMSON_WALL_SIGN(): Block { return self::get('CRIMSON_WALL_SIGN'); }
    public static function CRYING_OBSIDIAN(): Block { return self::get('CRYING_OBSIDIAN'); }
    public static function CUT_COPPER(): Block { return self::get('CUT_COPPER'); }
    public static function CUT_COPPER_SLAB(): Block { return self::get('CUT_COPPER_SLAB'); }
    public static function CUT_COPPER_STAIRS(): Block { return self::get('CUT_COPPER_STAIRS'); }
    public static function CUT_RED_SANDSTONE(): Block { return self::get('CUT_RED_SANDSTONE'); }
    public static function CUT_RED_SANDSTONE_SLAB(): Block { return self::get('CUT_RED_SANDSTONE_SLAB'); }
    public static function CUT_SANDSTONE(): Block { return self::get('CUT_SANDSTONE'); }
    public static function CUT_SANDSTONE_SLAB(): Block { return self::get('CUT_SANDSTONE_SLAB'); }
    public static function CYCLING_POTENT_SULFUR(): Block { return self::get('CYCLING_POTENT_SULFUR'); }
    public static function DANDELION(): Block { return self::get('DANDELION'); }
    public static function DARK_OAK_BUTTON(): Block { return self::get('DARK_OAK_BUTTON'); }
    public static function DARK_OAK_CEILING_CENTER_HANGING_SIGN(): Block { return self::get('DARK_OAK_CEILING_CENTER_HANGING_SIGN'); }
    public static function DARK_OAK_CEILING_EDGES_HANGING_SIGN(): Block { return self::get('DARK_OAK_CEILING_EDGES_HANGING_SIGN'); }
    public static function DARK_OAK_DOOR(): Block { return self::get('DARK_OAK_DOOR'); }
    public static function DARK_OAK_FENCE(): Block { return self::get('DARK_OAK_FENCE'); }
    public static function DARK_OAK_FENCE_GATE(): Block { return self::get('DARK_OAK_FENCE_GATE'); }
    public static function DARK_OAK_LEAVES(): Block { return self::get('DARK_OAK_LEAVES'); }
    public static function DARK_OAK_LOG(): Block { return self::get('DARK_OAK_LOG'); }
    public static function DARK_OAK_PLANKS(): Block { return self::get('DARK_OAK_PLANKS'); }
    public static function DARK_OAK_PRESSURE_PLATE(): Block { return self::get('DARK_OAK_PRESSURE_PLATE'); }
    public static function DARK_OAK_SAPLING(): Block { return self::get('DARK_OAK_SAPLING'); }
    public static function DARK_OAK_SIGN(): Block { return self::get('DARK_OAK_SIGN'); }
    public static function DARK_OAK_SLAB(): Block { return self::get('DARK_OAK_SLAB'); }
    public static function DARK_OAK_STAIRS(): Block { return self::get('DARK_OAK_STAIRS'); }
    public static function DARK_OAK_TRAPDOOR(): Block { return self::get('DARK_OAK_TRAPDOOR'); }
    public static function DARK_OAK_WALL_HANGING_SIGN(): Block { return self::get('DARK_OAK_WALL_HANGING_SIGN'); }
    public static function DARK_OAK_WALL_SIGN(): Block { return self::get('DARK_OAK_WALL_SIGN'); }
    public static function DARK_OAK_WOOD(): Block { return self::get('DARK_OAK_WOOD'); }
    public static function DARK_PRISMARINE(): Block { return self::get('DARK_PRISMARINE'); }
    public static function DARK_PRISMARINE_SLAB(): Block { return self::get('DARK_PRISMARINE_SLAB'); }
    public static function DARK_PRISMARINE_STAIRS(): Block { return self::get('DARK_PRISMARINE_STAIRS'); }
    public static function DAYLIGHT_SENSOR(): Block { return self::get('DAYLIGHT_SENSOR'); }
    public static function DEAD_BUSH(): Block { return self::get('DEAD_BUSH'); }
    public static function DEEPSLATE(): Block { return self::get('DEEPSLATE'); }
    public static function DEEPSLATE_BRICKS(): Block { return self::get('DEEPSLATE_BRICKS'); }
    public static function DEEPSLATE_BRICK_SLAB(): Block { return self::get('DEEPSLATE_BRICK_SLAB'); }
    public static function DEEPSLATE_BRICK_STAIRS(): Block { return self::get('DEEPSLATE_BRICK_STAIRS'); }
    public static function DEEPSLATE_BRICK_WALL(): Block { return self::get('DEEPSLATE_BRICK_WALL'); }
    public static function DEEPSLATE_COAL_ORE(): Block { return self::get('DEEPSLATE_COAL_ORE'); }
    public static function DEEPSLATE_COPPER_ORE(): Block { return self::get('DEEPSLATE_COPPER_ORE'); }
    public static function DEEPSLATE_DIAMOND_ORE(): Block { return self::get('DEEPSLATE_DIAMOND_ORE'); }
    public static function DEEPSLATE_EMERALD_ORE(): Block { return self::get('DEEPSLATE_EMERALD_ORE'); }
    public static function DEEPSLATE_GOLD_ORE(): Block { return self::get('DEEPSLATE_GOLD_ORE'); }
    public static function DEEPSLATE_IRON_ORE(): Block { return self::get('DEEPSLATE_IRON_ORE'); }
    public static function DEEPSLATE_LAPIS_LAZULI_ORE(): Block { return self::get('DEEPSLATE_LAPIS_LAZULI_ORE'); }
    public static function DEEPSLATE_REDSTONE_ORE(): Block { return self::get('DEEPSLATE_REDSTONE_ORE'); }
    public static function DEEPSLATE_TILES(): Block { return self::get('DEEPSLATE_TILES'); }
    public static function DEEPSLATE_TILE_SLAB(): Block { return self::get('DEEPSLATE_TILE_SLAB'); }
    public static function DEEPSLATE_TILE_STAIRS(): Block { return self::get('DEEPSLATE_TILE_STAIRS'); }
    public static function DEEPSLATE_TILE_WALL(): Block { return self::get('DEEPSLATE_TILE_WALL'); }
    public static function DETECTOR_RAIL(): Block { return self::get('DETECTOR_RAIL'); }
    public static function DIAMOND(): Block { return self::get('DIAMOND'); }
    public static function DIAMOND_ORE(): Block { return self::get('DIAMOND_ORE'); }
    public static function DIORITE(): Block { return self::get('DIORITE'); }
    public static function DIORITE_SLAB(): Block { return self::get('DIORITE_SLAB'); }
    public static function DIORITE_STAIRS(): Block { return self::get('DIORITE_STAIRS'); }
    public static function DIORITE_WALL(): Block { return self::get('DIORITE_WALL'); }
    public static function DIRT(): Block { return self::get('DIRT'); }
    public static function DOUBLE_PITCHER_CROP(): Block { return self::get('DOUBLE_PITCHER_CROP'); }
    public static function DOUBLE_TALLGRASS(): Block { return self::get('DOUBLE_TALLGRASS'); }
    public static function DRAGON_EGG(): Block { return self::get('DRAGON_EGG'); }
    public static function DRIED_KELP(): Block { return self::get('DRIED_KELP'); }
    public static function DYED_CANDLE(): Block { return self::get('DYED_CANDLE'); }
    public static function DYED_SHULKER_BOX(): Block { return self::get('DYED_SHULKER_BOX'); }
    public static function ELEMENT_ACTINIUM(): Block { return self::get('ELEMENT_ACTINIUM'); }
    public static function ELEMENT_ALUMINUM(): Block { return self::get('ELEMENT_ALUMINUM'); }
    public static function ELEMENT_AMERICIUM(): Block { return self::get('ELEMENT_AMERICIUM'); }
    public static function ELEMENT_ANTIMONY(): Block { return self::get('ELEMENT_ANTIMONY'); }
    public static function ELEMENT_ARGON(): Block { return self::get('ELEMENT_ARGON'); }
    public static function ELEMENT_ARSENIC(): Block { return self::get('ELEMENT_ARSENIC'); }
    public static function ELEMENT_ASTATINE(): Block { return self::get('ELEMENT_ASTATINE'); }
    public static function ELEMENT_BARIUM(): Block { return self::get('ELEMENT_BARIUM'); }
    public static function ELEMENT_BERKELIUM(): Block { return self::get('ELEMENT_BERKELIUM'); }
    public static function ELEMENT_BERYLLIUM(): Block { return self::get('ELEMENT_BERYLLIUM'); }
    public static function ELEMENT_BISMUTH(): Block { return self::get('ELEMENT_BISMUTH'); }
    public static function ELEMENT_BOHRIUM(): Block { return self::get('ELEMENT_BOHRIUM'); }
    public static function ELEMENT_BORON(): Block { return self::get('ELEMENT_BORON'); }
    public static function ELEMENT_BROMINE(): Block { return self::get('ELEMENT_BROMINE'); }
    public static function ELEMENT_CADMIUM(): Block { return self::get('ELEMENT_CADMIUM'); }
    public static function ELEMENT_CALCIUM(): Block { return self::get('ELEMENT_CALCIUM'); }
    public static function ELEMENT_CALIFORNIUM(): Block { return self::get('ELEMENT_CALIFORNIUM'); }
    public static function ELEMENT_CARBON(): Block { return self::get('ELEMENT_CARBON'); }
    public static function ELEMENT_CERIUM(): Block { return self::get('ELEMENT_CERIUM'); }
    public static function ELEMENT_CESIUM(): Block { return self::get('ELEMENT_CESIUM'); }
    public static function ELEMENT_CHLORINE(): Block { return self::get('ELEMENT_CHLORINE'); }
    public static function ELEMENT_CHROMIUM(): Block { return self::get('ELEMENT_CHROMIUM'); }
    public static function ELEMENT_COBALT(): Block { return self::get('ELEMENT_COBALT'); }
    public static function ELEMENT_CONSTRUCTOR(): Block { return self::get('ELEMENT_CONSTRUCTOR'); }
    public static function ELEMENT_COPERNICIUM(): Block { return self::get('ELEMENT_COPERNICIUM'); }
    public static function ELEMENT_COPPER(): Block { return self::get('ELEMENT_COPPER'); }
    public static function ELEMENT_CURIUM(): Block { return self::get('ELEMENT_CURIUM'); }
    public static function ELEMENT_DARMSTADTIUM(): Block { return self::get('ELEMENT_DARMSTADTIUM'); }
    public static function ELEMENT_DUBNIUM(): Block { return self::get('ELEMENT_DUBNIUM'); }
    public static function ELEMENT_DYSPROSIUM(): Block { return self::get('ELEMENT_DYSPROSIUM'); }
    public static function ELEMENT_EINSTEINIUM(): Block { return self::get('ELEMENT_EINSTEINIUM'); }
    public static function ELEMENT_ERBIUM(): Block { return self::get('ELEMENT_ERBIUM'); }
    public static function ELEMENT_EUROPIUM(): Block { return self::get('ELEMENT_EUROPIUM'); }
    public static function ELEMENT_FERMIUM(): Block { return self::get('ELEMENT_FERMIUM'); }
    public static function ELEMENT_FLEROVIUM(): Block { return self::get('ELEMENT_FLEROVIUM'); }
    public static function ELEMENT_FLUORINE(): Block { return self::get('ELEMENT_FLUORINE'); }
    public static function ELEMENT_FRANCIUM(): Block { return self::get('ELEMENT_FRANCIUM'); }
    public static function ELEMENT_GADOLINIUM(): Block { return self::get('ELEMENT_GADOLINIUM'); }
    public static function ELEMENT_GALLIUM(): Block { return self::get('ELEMENT_GALLIUM'); }
    public static function ELEMENT_GERMANIUM(): Block { return self::get('ELEMENT_GERMANIUM'); }
    public static function ELEMENT_GOLD(): Block { return self::get('ELEMENT_GOLD'); }
    public static function ELEMENT_HAFNIUM(): Block { return self::get('ELEMENT_HAFNIUM'); }
    public static function ELEMENT_HASSIUM(): Block { return self::get('ELEMENT_HASSIUM'); }
    public static function ELEMENT_HELIUM(): Block { return self::get('ELEMENT_HELIUM'); }
    public static function ELEMENT_HOLMIUM(): Block { return self::get('ELEMENT_HOLMIUM'); }
    public static function ELEMENT_HYDROGEN(): Block { return self::get('ELEMENT_HYDROGEN'); }
    public static function ELEMENT_INDIUM(): Block { return self::get('ELEMENT_INDIUM'); }
    public static function ELEMENT_IODINE(): Block { return self::get('ELEMENT_IODINE'); }
    public static function ELEMENT_IRIDIUM(): Block { return self::get('ELEMENT_IRIDIUM'); }
    public static function ELEMENT_IRON(): Block { return self::get('ELEMENT_IRON'); }
    public static function ELEMENT_KRYPTON(): Block { return self::get('ELEMENT_KRYPTON'); }
    public static function ELEMENT_LANTHANUM(): Block { return self::get('ELEMENT_LANTHANUM'); }
    public static function ELEMENT_LAWRENCIUM(): Block { return self::get('ELEMENT_LAWRENCIUM'); }
    public static function ELEMENT_LEAD(): Block { return self::get('ELEMENT_LEAD'); }
    public static function ELEMENT_LITHIUM(): Block { return self::get('ELEMENT_LITHIUM'); }
    public static function ELEMENT_LIVERMORIUM(): Block { return self::get('ELEMENT_LIVERMORIUM'); }
    public static function ELEMENT_LUTETIUM(): Block { return self::get('ELEMENT_LUTETIUM'); }
    public static function ELEMENT_MAGNESIUM(): Block { return self::get('ELEMENT_MAGNESIUM'); }
    public static function ELEMENT_MANGANESE(): Block { return self::get('ELEMENT_MANGANESE'); }
    public static function ELEMENT_MEITNERIUM(): Block { return self::get('ELEMENT_MEITNERIUM'); }
    public static function ELEMENT_MENDELEVIUM(): Block { return self::get('ELEMENT_MENDELEVIUM'); }
    public static function ELEMENT_MERCURY(): Block { return self::get('ELEMENT_MERCURY'); }
    public static function ELEMENT_MOLYBDENUM(): Block { return self::get('ELEMENT_MOLYBDENUM'); }
    public static function ELEMENT_MOSCOVIUM(): Block { return self::get('ELEMENT_MOSCOVIUM'); }
    public static function ELEMENT_NEODYMIUM(): Block { return self::get('ELEMENT_NEODYMIUM'); }
    public static function ELEMENT_NEON(): Block { return self::get('ELEMENT_NEON'); }
    public static function ELEMENT_NEPTUNIUM(): Block { return self::get('ELEMENT_NEPTUNIUM'); }
    public static function ELEMENT_NICKEL(): Block { return self::get('ELEMENT_NICKEL'); }
    public static function ELEMENT_NIHONIUM(): Block { return self::get('ELEMENT_NIHONIUM'); }
    public static function ELEMENT_NIOBIUM(): Block { return self::get('ELEMENT_NIOBIUM'); }
    public static function ELEMENT_NITROGEN(): Block { return self::get('ELEMENT_NITROGEN'); }
    public static function ELEMENT_NOBELIUM(): Block { return self::get('ELEMENT_NOBELIUM'); }
    public static function ELEMENT_OGANESSON(): Block { return self::get('ELEMENT_OGANESSON'); }
    public static function ELEMENT_OSMIUM(): Block { return self::get('ELEMENT_OSMIUM'); }
    public static function ELEMENT_OXYGEN(): Block { return self::get('ELEMENT_OXYGEN'); }
    public static function ELEMENT_PALLADIUM(): Block { return self::get('ELEMENT_PALLADIUM'); }
    public static function ELEMENT_PHOSPHORUS(): Block { return self::get('ELEMENT_PHOSPHORUS'); }
    public static function ELEMENT_PLATINUM(): Block { return self::get('ELEMENT_PLATINUM'); }
    public static function ELEMENT_PLUTONIUM(): Block { return self::get('ELEMENT_PLUTONIUM'); }
    public static function ELEMENT_POLONIUM(): Block { return self::get('ELEMENT_POLONIUM'); }
    public static function ELEMENT_POTASSIUM(): Block { return self::get('ELEMENT_POTASSIUM'); }
    public static function ELEMENT_PRASEODYMIUM(): Block { return self::get('ELEMENT_PRASEODYMIUM'); }
    public static function ELEMENT_PROMETHIUM(): Block { return self::get('ELEMENT_PROMETHIUM'); }
    public static function ELEMENT_PROTACTINIUM(): Block { return self::get('ELEMENT_PROTACTINIUM'); }
    public static function ELEMENT_RADIUM(): Block { return self::get('ELEMENT_RADIUM'); }
    public static function ELEMENT_RADON(): Block { return self::get('ELEMENT_RADON'); }
    public static function ELEMENT_RHENIUM(): Block { return self::get('ELEMENT_RHENIUM'); }
    public static function ELEMENT_RHODIUM(): Block { return self::get('ELEMENT_RHODIUM'); }
    public static function ELEMENT_ROENTGENIUM(): Block { return self::get('ELEMENT_ROENTGENIUM'); }
    public static function ELEMENT_RUBIDIUM(): Block { return self::get('ELEMENT_RUBIDIUM'); }
    public static function ELEMENT_RUTHENIUM(): Block { return self::get('ELEMENT_RUTHENIUM'); }
    public static function ELEMENT_RUTHERFORDIUM(): Block { return self::get('ELEMENT_RUTHERFORDIUM'); }
    public static function ELEMENT_SAMARIUM(): Block { return self::get('ELEMENT_SAMARIUM'); }
    public static function ELEMENT_SCANDIUM(): Block { return self::get('ELEMENT_SCANDIUM'); }
    public static function ELEMENT_SEABORGIUM(): Block { return self::get('ELEMENT_SEABORGIUM'); }
    public static function ELEMENT_SELENIUM(): Block { return self::get('ELEMENT_SELENIUM'); }
    public static function ELEMENT_SILICON(): Block { return self::get('ELEMENT_SILICON'); }
    public static function ELEMENT_SILVER(): Block { return self::get('ELEMENT_SILVER'); }
    public static function ELEMENT_SODIUM(): Block { return self::get('ELEMENT_SODIUM'); }
    public static function ELEMENT_STRONTIUM(): Block { return self::get('ELEMENT_STRONTIUM'); }
    public static function ELEMENT_SULFUR(): Block { return self::get('ELEMENT_SULFUR'); }
    public static function ELEMENT_TANTALUM(): Block { return self::get('ELEMENT_TANTALUM'); }
    public static function ELEMENT_TECHNETIUM(): Block { return self::get('ELEMENT_TECHNETIUM'); }
    public static function ELEMENT_TELLURIUM(): Block { return self::get('ELEMENT_TELLURIUM'); }
    public static function ELEMENT_TENNESSINE(): Block { return self::get('ELEMENT_TENNESSINE'); }
    public static function ELEMENT_TERBIUM(): Block { return self::get('ELEMENT_TERBIUM'); }
    public static function ELEMENT_THALLIUM(): Block { return self::get('ELEMENT_THALLIUM'); }
    public static function ELEMENT_THORIUM(): Block { return self::get('ELEMENT_THORIUM'); }
    public static function ELEMENT_THULIUM(): Block { return self::get('ELEMENT_THULIUM'); }
    public static function ELEMENT_TIN(): Block { return self::get('ELEMENT_TIN'); }
    public static function ELEMENT_TITANIUM(): Block { return self::get('ELEMENT_TITANIUM'); }
    public static function ELEMENT_TUNGSTEN(): Block { return self::get('ELEMENT_TUNGSTEN'); }
    public static function ELEMENT_URANIUM(): Block { return self::get('ELEMENT_URANIUM'); }
    public static function ELEMENT_VANADIUM(): Block { return self::get('ELEMENT_VANADIUM'); }
    public static function ELEMENT_XENON(): Block { return self::get('ELEMENT_XENON'); }
    public static function ELEMENT_YTTERBIUM(): Block { return self::get('ELEMENT_YTTERBIUM'); }
    public static function ELEMENT_YTTRIUM(): Block { return self::get('ELEMENT_YTTRIUM'); }
    public static function ELEMENT_ZERO(): Block { return self::get('ELEMENT_ZERO'); }
    public static function ELEMENT_ZINC(): Block { return self::get('ELEMENT_ZINC'); }
    public static function ELEMENT_ZIRCONIUM(): Block { return self::get('ELEMENT_ZIRCONIUM'); }
    public static function EMERALD(): Block { return self::get('EMERALD'); }
    public static function EMERALD_ORE(): Block { return self::get('EMERALD_ORE'); }
    public static function ENCHANTING_TABLE(): Block { return self::get('ENCHANTING_TABLE'); }
    public static function ENDER_CHEST(): Block { return self::get('ENDER_CHEST'); }
    public static function END_PORTAL_FRAME(): Block { return self::get('END_PORTAL_FRAME'); }
    public static function END_ROD(): Block { return self::get('END_ROD'); }
    public static function END_STONE(): Block { return self::get('END_STONE'); }
    public static function END_STONE_BRICKS(): Block { return self::get('END_STONE_BRICKS'); }
    public static function END_STONE_BRICK_SLAB(): Block { return self::get('END_STONE_BRICK_SLAB'); }
    public static function END_STONE_BRICK_STAIRS(): Block { return self::get('END_STONE_BRICK_STAIRS'); }
    public static function END_STONE_BRICK_WALL(): Block { return self::get('END_STONE_BRICK_WALL'); }
    public static function FAKE_WOODEN_SLAB(): Block { return self::get('FAKE_WOODEN_SLAB'); }
    public static function FARMLAND(): Block { return self::get('FARMLAND'); }
    public static function FERN(): Block { return self::get('FERN'); }
    public static function FIRE(): Block { return self::get('FIRE'); }
    public static function FLETCHING_TABLE(): Block { return self::get('FLETCHING_TABLE'); }
    public static function FLOWERING_AZALEA(): Block { return self::get('FLOWERING_AZALEA'); }
    public static function FLOWERING_AZALEA_LEAVES(): Block { return self::get('FLOWERING_AZALEA_LEAVES'); }
    public static function FLOWER_POT(): Block { return self::get('FLOWER_POT'); }
    public static function FROGLIGHT(): Block { return self::get('FROGLIGHT'); }
    public static function FROSTED_ICE(): Block { return self::get('FROSTED_ICE'); }
    public static function FURNACE(): Block { return self::get('FURNACE'); }
    public static function GILDED_BLACKSTONE(): Block { return self::get('GILDED_BLACKSTONE'); }
    public static function GLASS(): Block { return self::get('GLASS'); }
    public static function GLASS_PANE(): Block { return self::get('GLASS_PANE'); }
    public static function GLAZED_TERRACOTTA(): Block { return self::get('GLAZED_TERRACOTTA'); }
    public static function GLOWING_ITEM_FRAME(): Block { return self::get('GLOWING_ITEM_FRAME'); }
    public static function GLOWING_OBSIDIAN(): Block { return self::get('GLOWING_OBSIDIAN'); }
    public static function GLOWSTONE(): Block { return self::get('GLOWSTONE'); }
    public static function GLOW_LICHEN(): Block { return self::get('GLOW_LICHEN'); }
    public static function GOLD(): Block { return self::get('GOLD'); }
    public static function GOLD_ORE(): Block { return self::get('GOLD_ORE'); }
    public static function GRANITE(): Block { return self::get('GRANITE'); }
    public static function GRANITE_SLAB(): Block { return self::get('GRANITE_SLAB'); }
    public static function GRANITE_STAIRS(): Block { return self::get('GRANITE_STAIRS'); }
    public static function GRANITE_WALL(): Block { return self::get('GRANITE_WALL'); }
    public static function GRASS(): Block { return self::get('GRASS'); }
    public static function GRASS_PATH(): Block { return self::get('GRASS_PATH'); }
    public static function GRAVEL(): Block { return self::get('GRAVEL'); }
    public static function GREEN_TORCH(): Block { return self::get('GREEN_TORCH'); }
    public static function HANGING_ROOTS(): Block { return self::get('HANGING_ROOTS'); }
    public static function HARDENED_CLAY(): Block { return self::get('HARDENED_CLAY'); }
    public static function HARDENED_GLASS(): Block { return self::get('HARDENED_GLASS'); }
    public static function HARDENED_GLASS_PANE(): Block { return self::get('HARDENED_GLASS_PANE'); }
    public static function HAY_BALE(): Block { return self::get('HAY_BALE'); }
    public static function HEAVY_CORE(): Block { return self::get('HEAVY_CORE'); }
    public static function HONEYCOMB(): Block { return self::get('HONEYCOMB'); }
    public static function HOPPER(): Block { return self::get('HOPPER'); }
    public static function ICE(): Block { return self::get('ICE'); }
    public static function INFESTED_CHISELED_STONE_BRICK(): Block { return self::get('INFESTED_CHISELED_STONE_BRICK'); }
    public static function INFESTED_COBBLESTONE(): Block { return self::get('INFESTED_COBBLESTONE'); }
    public static function INFESTED_CRACKED_STONE_BRICK(): Block { return self::get('INFESTED_CRACKED_STONE_BRICK'); }
    public static function INFESTED_DEEPSLATE(): Block { return self::get('INFESTED_DEEPSLATE'); }
    public static function INFESTED_MOSSY_STONE_BRICK(): Block { return self::get('INFESTED_MOSSY_STONE_BRICK'); }
    public static function INFESTED_STONE(): Block { return self::get('INFESTED_STONE'); }
    public static function INFESTED_STONE_BRICK(): Block { return self::get('INFESTED_STONE_BRICK'); }
    public static function INFO_UPDATE(): Block { return self::get('INFO_UPDATE'); }
    public static function INFO_UPDATE2(): Block { return self::get('INFO_UPDATE2'); }
    public static function INVISIBLE_BEDROCK(): Block { return self::get('INVISIBLE_BEDROCK'); }
    public static function IRON(): Block { return self::get('IRON'); }
    public static function IRON_BARS(): Block { return self::get('IRON_BARS'); }
    public static function IRON_DOOR(): Block { return self::get('IRON_DOOR'); }
    public static function IRON_ORE(): Block { return self::get('IRON_ORE'); }
    public static function IRON_TRAPDOOR(): Block { return self::get('IRON_TRAPDOOR'); }
    public static function ITEM_FRAME(): Block { return self::get('ITEM_FRAME'); }
    public static function JUKEBOX(): Block { return self::get('JUKEBOX'); }
    public static function JUNGLE_BUTTON(): Block { return self::get('JUNGLE_BUTTON'); }
    public static function JUNGLE_CEILING_CENTER_HANGING_SIGN(): Block { return self::get('JUNGLE_CEILING_CENTER_HANGING_SIGN'); }
    public static function JUNGLE_CEILING_EDGES_HANGING_SIGN(): Block { return self::get('JUNGLE_CEILING_EDGES_HANGING_SIGN'); }
    public static function JUNGLE_DOOR(): Block { return self::get('JUNGLE_DOOR'); }
    public static function JUNGLE_FENCE(): Block { return self::get('JUNGLE_FENCE'); }
    public static function JUNGLE_FENCE_GATE(): Block { return self::get('JUNGLE_FENCE_GATE'); }
    public static function JUNGLE_LEAVES(): Block { return self::get('JUNGLE_LEAVES'); }
    public static function JUNGLE_LOG(): Block { return self::get('JUNGLE_LOG'); }
    public static function JUNGLE_PLANKS(): Block { return self::get('JUNGLE_PLANKS'); }
    public static function JUNGLE_PRESSURE_PLATE(): Block { return self::get('JUNGLE_PRESSURE_PLATE'); }
    public static function JUNGLE_SAPLING(): Block { return self::get('JUNGLE_SAPLING'); }
    public static function JUNGLE_SIGN(): Block { return self::get('JUNGLE_SIGN'); }
    public static function JUNGLE_SLAB(): Block { return self::get('JUNGLE_SLAB'); }
    public static function JUNGLE_STAIRS(): Block { return self::get('JUNGLE_STAIRS'); }
    public static function JUNGLE_TRAPDOOR(): Block { return self::get('JUNGLE_TRAPDOOR'); }
    public static function JUNGLE_WALL_HANGING_SIGN(): Block { return self::get('JUNGLE_WALL_HANGING_SIGN'); }
    public static function JUNGLE_WALL_SIGN(): Block { return self::get('JUNGLE_WALL_SIGN'); }
    public static function JUNGLE_WOOD(): Block { return self::get('JUNGLE_WOOD'); }
    public static function LAB_TABLE(): Block { return self::get('LAB_TABLE'); }
    public static function LADDER(): Block { return self::get('LADDER'); }
    public static function LANTERN(): Block { return self::get('LANTERN'); }
    public static function LAPIS_LAZULI(): Block { return self::get('LAPIS_LAZULI'); }
    public static function LAPIS_LAZULI_ORE(): Block { return self::get('LAPIS_LAZULI_ORE'); }
    public static function LARGE_FERN(): Block { return self::get('LARGE_FERN'); }
    public static function LAVA(): Block { return self::get('LAVA'); }
    public static function LAVA_CAULDRON(): Block { return self::get('LAVA_CAULDRON'); }
    public static function LECTERN(): Block { return self::get('LECTERN'); }
    public static function LEGACY_STONECUTTER(): Block { return self::get('LEGACY_STONECUTTER'); }
    public static function LEVER(): Block { return self::get('LEVER'); }
    public static function LIGHT(): Block { return self::get('LIGHT'); }
    public static function LIGHTNING_ROD(): Block { return self::get('LIGHTNING_ROD'); }
    public static function LILAC(): Block { return self::get('LILAC'); }
    public static function LILY_OF_THE_VALLEY(): Block { return self::get('LILY_OF_THE_VALLEY'); }
    public static function LILY_PAD(): Block { return self::get('LILY_PAD'); }
    public static function LIT_PUMPKIN(): Block { return self::get('LIT_PUMPKIN'); }
    public static function LOOM(): Block { return self::get('LOOM'); }
    public static function MAGMA(): Block { return self::get('MAGMA'); }
    public static function MANGROVE_BUTTON(): Block { return self::get('MANGROVE_BUTTON'); }
    public static function MANGROVE_CEILING_CENTER_HANGING_SIGN(): Block { return self::get('MANGROVE_CEILING_CENTER_HANGING_SIGN'); }
    public static function MANGROVE_CEILING_EDGES_HANGING_SIGN(): Block { return self::get('MANGROVE_CEILING_EDGES_HANGING_SIGN'); }
    public static function MANGROVE_DOOR(): Block { return self::get('MANGROVE_DOOR'); }
    public static function MANGROVE_FENCE(): Block { return self::get('MANGROVE_FENCE'); }
    public static function MANGROVE_FENCE_GATE(): Block { return self::get('MANGROVE_FENCE_GATE'); }
    public static function MANGROVE_LEAVES(): Block { return self::get('MANGROVE_LEAVES'); }
    public static function MANGROVE_LOG(): Block { return self::get('MANGROVE_LOG'); }
    public static function MANGROVE_PLANKS(): Block { return self::get('MANGROVE_PLANKS'); }
    public static function MANGROVE_PRESSURE_PLATE(): Block { return self::get('MANGROVE_PRESSURE_PLATE'); }
    public static function MANGROVE_ROOTS(): Block { return self::get('MANGROVE_ROOTS'); }
    public static function MANGROVE_SIGN(): Block { return self::get('MANGROVE_SIGN'); }
    public static function MANGROVE_SLAB(): Block { return self::get('MANGROVE_SLAB'); }
    public static function MANGROVE_STAIRS(): Block { return self::get('MANGROVE_STAIRS'); }
    public static function MANGROVE_TRAPDOOR(): Block { return self::get('MANGROVE_TRAPDOOR'); }
    public static function MANGROVE_WALL_HANGING_SIGN(): Block { return self::get('MANGROVE_WALL_HANGING_SIGN'); }
    public static function MANGROVE_WALL_SIGN(): Block { return self::get('MANGROVE_WALL_SIGN'); }
    public static function MANGROVE_WOOD(): Block { return self::get('MANGROVE_WOOD'); }
    public static function MATERIAL_REDUCER(): Block { return self::get('MATERIAL_REDUCER'); }
    public static function MELON(): Block { return self::get('MELON'); }
    public static function MELON_STEM(): Block { return self::get('MELON_STEM'); }
    public static function MOB_HEAD(): Block { return self::get('MOB_HEAD'); }
    public static function MONSTER_SPAWNER(): Block { return self::get('MONSTER_SPAWNER'); }
    public static function MOSSY_COBBLESTONE(): Block { return self::get('MOSSY_COBBLESTONE'); }
    public static function MOSSY_COBBLESTONE_SLAB(): Block { return self::get('MOSSY_COBBLESTONE_SLAB'); }
    public static function MOSSY_COBBLESTONE_STAIRS(): Block { return self::get('MOSSY_COBBLESTONE_STAIRS'); }
    public static function MOSSY_COBBLESTONE_WALL(): Block { return self::get('MOSSY_COBBLESTONE_WALL'); }
    public static function MOSSY_STONE_BRICKS(): Block { return self::get('MOSSY_STONE_BRICKS'); }
    public static function MOSSY_STONE_BRICK_SLAB(): Block { return self::get('MOSSY_STONE_BRICK_SLAB'); }
    public static function MOSSY_STONE_BRICK_STAIRS(): Block { return self::get('MOSSY_STONE_BRICK_STAIRS'); }
    public static function MOSSY_STONE_BRICK_WALL(): Block { return self::get('MOSSY_STONE_BRICK_WALL'); }
    public static function MUD(): Block { return self::get('MUD'); }
    public static function MUDDY_MANGROVE_ROOTS(): Block { return self::get('MUDDY_MANGROVE_ROOTS'); }
    public static function MUD_BRICKS(): Block { return self::get('MUD_BRICKS'); }
    public static function MUD_BRICK_SLAB(): Block { return self::get('MUD_BRICK_SLAB'); }
    public static function MUD_BRICK_STAIRS(): Block { return self::get('MUD_BRICK_STAIRS'); }
    public static function MUD_BRICK_WALL(): Block { return self::get('MUD_BRICK_WALL'); }
    public static function MUSHROOM_STEM(): Block { return self::get('MUSHROOM_STEM'); }
    public static function MYCELIUM(): Block { return self::get('MYCELIUM'); }
    public static function NETHERITE(): Block { return self::get('NETHERITE'); }
    public static function NETHERRACK(): Block { return self::get('NETHERRACK'); }
    public static function NETHER_BRICKS(): Block { return self::get('NETHER_BRICKS'); }
    public static function NETHER_BRICK_FENCE(): Block { return self::get('NETHER_BRICK_FENCE'); }
    public static function NETHER_BRICK_SLAB(): Block { return self::get('NETHER_BRICK_SLAB'); }
    public static function NETHER_BRICK_STAIRS(): Block { return self::get('NETHER_BRICK_STAIRS'); }
    public static function NETHER_BRICK_WALL(): Block { return self::get('NETHER_BRICK_WALL'); }
    public static function NETHER_GOLD_ORE(): Block { return self::get('NETHER_GOLD_ORE'); }
    public static function NETHER_PORTAL(): Block { return self::get('NETHER_PORTAL'); }
    public static function NETHER_QUARTZ_ORE(): Block { return self::get('NETHER_QUARTZ_ORE'); }
    public static function NETHER_REACTOR_CORE(): Block { return self::get('NETHER_REACTOR_CORE'); }
    public static function NETHER_SPROUTS(): Block { return self::get('NETHER_SPROUTS'); }
    public static function NETHER_WART(): Block { return self::get('NETHER_WART'); }
    public static function NETHER_WART_BLOCK(): Block { return self::get('NETHER_WART_BLOCK'); }
    public static function NOTE_BLOCK(): Block { return self::get('NOTE_BLOCK'); }
    public static function OAK_BUTTON(): Block { return self::get('OAK_BUTTON'); }
    public static function OAK_CEILING_CENTER_HANGING_SIGN(): Block { return self::get('OAK_CEILING_CENTER_HANGING_SIGN'); }
    public static function OAK_CEILING_EDGES_HANGING_SIGN(): Block { return self::get('OAK_CEILING_EDGES_HANGING_SIGN'); }
    public static function OAK_DOOR(): Block { return self::get('OAK_DOOR'); }
    public static function OAK_FENCE(): Block { return self::get('OAK_FENCE'); }
    public static function OAK_FENCE_GATE(): Block { return self::get('OAK_FENCE_GATE'); }
    public static function OAK_LEAVES(): Block { return self::get('OAK_LEAVES'); }
    public static function OAK_LOG(): Block { return self::get('OAK_LOG'); }
    public static function OAK_PLANKS(): Block { return self::get('OAK_PLANKS'); }
    public static function OAK_PRESSURE_PLATE(): Block { return self::get('OAK_PRESSURE_PLATE'); }
    public static function OAK_SAPLING(): Block { return self::get('OAK_SAPLING'); }
    public static function OAK_SIGN(): Block { return self::get('OAK_SIGN'); }
    public static function OAK_SLAB(): Block { return self::get('OAK_SLAB'); }
    public static function OAK_STAIRS(): Block { return self::get('OAK_STAIRS'); }
    public static function OAK_TRAPDOOR(): Block { return self::get('OAK_TRAPDOOR'); }
    public static function OAK_WALL_HANGING_SIGN(): Block { return self::get('OAK_WALL_HANGING_SIGN'); }
    public static function OAK_WALL_SIGN(): Block { return self::get('OAK_WALL_SIGN'); }
    public static function OAK_WOOD(): Block { return self::get('OAK_WOOD'); }
    public static function OBSIDIAN(): Block { return self::get('OBSIDIAN'); }
    public static function OMINOUS_BANNER(): Block { return self::get('OMINOUS_BANNER'); }
    public static function OMINOUS_WALL_BANNER(): Block { return self::get('OMINOUS_WALL_BANNER'); }
    public static function ORANGE_TULIP(): Block { return self::get('ORANGE_TULIP'); }
    public static function OXEYE_DAISY(): Block { return self::get('OXEYE_DAISY'); }
    public static function PACKED_ICE(): Block { return self::get('PACKED_ICE'); }
    public static function PACKED_MUD(): Block { return self::get('PACKED_MUD'); }
    public static function PALE_OAK_BUTTON(): Block { return self::get('PALE_OAK_BUTTON'); }
    public static function PALE_OAK_CEILING_CENTER_HANGING_SIGN(): Block { return self::get('PALE_OAK_CEILING_CENTER_HANGING_SIGN'); }
    public static function PALE_OAK_CEILING_EDGES_HANGING_SIGN(): Block { return self::get('PALE_OAK_CEILING_EDGES_HANGING_SIGN'); }
    public static function PALE_OAK_DOOR(): Block { return self::get('PALE_OAK_DOOR'); }
    public static function PALE_OAK_FENCE(): Block { return self::get('PALE_OAK_FENCE'); }
    public static function PALE_OAK_FENCE_GATE(): Block { return self::get('PALE_OAK_FENCE_GATE'); }
    public static function PALE_OAK_LEAVES(): Block { return self::get('PALE_OAK_LEAVES'); }
    public static function PALE_OAK_LOG(): Block { return self::get('PALE_OAK_LOG'); }
    public static function PALE_OAK_PLANKS(): Block { return self::get('PALE_OAK_PLANKS'); }
    public static function PALE_OAK_PRESSURE_PLATE(): Block { return self::get('PALE_OAK_PRESSURE_PLATE'); }
    public static function PALE_OAK_SIGN(): Block { return self::get('PALE_OAK_SIGN'); }
    public static function PALE_OAK_SLAB(): Block { return self::get('PALE_OAK_SLAB'); }
    public static function PALE_OAK_STAIRS(): Block { return self::get('PALE_OAK_STAIRS'); }
    public static function PALE_OAK_TRAPDOOR(): Block { return self::get('PALE_OAK_TRAPDOOR'); }
    public static function PALE_OAK_WALL_HANGING_SIGN(): Block { return self::get('PALE_OAK_WALL_HANGING_SIGN'); }
    public static function PALE_OAK_WALL_SIGN(): Block { return self::get('PALE_OAK_WALL_SIGN'); }
    public static function PALE_OAK_WOOD(): Block { return self::get('PALE_OAK_WOOD'); }
    public static function PEONY(): Block { return self::get('PEONY'); }
    public static function PINK_PETALS(): Block { return self::get('PINK_PETALS'); }
    public static function PINK_TULIP(): Block { return self::get('PINK_TULIP'); }
    public static function PITCHER_CROP(): Block { return self::get('PITCHER_CROP'); }
    public static function PITCHER_PLANT(): Block { return self::get('PITCHER_PLANT'); }
    public static function PODZOL(): Block { return self::get('PODZOL'); }
    public static function POLISHED_ANDESITE(): Block { return self::get('POLISHED_ANDESITE'); }
    public static function POLISHED_ANDESITE_SLAB(): Block { return self::get('POLISHED_ANDESITE_SLAB'); }
    public static function POLISHED_ANDESITE_STAIRS(): Block { return self::get('POLISHED_ANDESITE_STAIRS'); }
    public static function POLISHED_BASALT(): Block { return self::get('POLISHED_BASALT'); }
    public static function POLISHED_BLACKSTONE(): Block { return self::get('POLISHED_BLACKSTONE'); }
    public static function POLISHED_BLACKSTONE_BRICKS(): Block { return self::get('POLISHED_BLACKSTONE_BRICKS'); }
    public static function POLISHED_BLACKSTONE_BRICK_SLAB(): Block { return self::get('POLISHED_BLACKSTONE_BRICK_SLAB'); }
    public static function POLISHED_BLACKSTONE_BRICK_STAIRS(): Block { return self::get('POLISHED_BLACKSTONE_BRICK_STAIRS'); }
    public static function POLISHED_BLACKSTONE_BRICK_WALL(): Block { return self::get('POLISHED_BLACKSTONE_BRICK_WALL'); }
    public static function POLISHED_BLACKSTONE_BUTTON(): Block { return self::get('POLISHED_BLACKSTONE_BUTTON'); }
    public static function POLISHED_BLACKSTONE_PRESSURE_PLATE(): Block { return self::get('POLISHED_BLACKSTONE_PRESSURE_PLATE'); }
    public static function POLISHED_BLACKSTONE_SLAB(): Block { return self::get('POLISHED_BLACKSTONE_SLAB'); }
    public static function POLISHED_BLACKSTONE_STAIRS(): Block { return self::get('POLISHED_BLACKSTONE_STAIRS'); }
    public static function POLISHED_BLACKSTONE_WALL(): Block { return self::get('POLISHED_BLACKSTONE_WALL'); }
    public static function POLISHED_CINNABAR(): Block { return self::get('POLISHED_CINNABAR'); }
    public static function POLISHED_CINNABAR_SLAB(): Block { return self::get('POLISHED_CINNABAR_SLAB'); }
    public static function POLISHED_CINNABAR_STAIRS(): Block { return self::get('POLISHED_CINNABAR_STAIRS'); }
    public static function POLISHED_CINNABAR_WALL(): Block { return self::get('POLISHED_CINNABAR_WALL'); }
    public static function POLISHED_DEEPSLATE(): Block { return self::get('POLISHED_DEEPSLATE'); }
    public static function POLISHED_DEEPSLATE_SLAB(): Block { return self::get('POLISHED_DEEPSLATE_SLAB'); }
    public static function POLISHED_DEEPSLATE_STAIRS(): Block { return self::get('POLISHED_DEEPSLATE_STAIRS'); }
    public static function POLISHED_DEEPSLATE_WALL(): Block { return self::get('POLISHED_DEEPSLATE_WALL'); }
    public static function POLISHED_DIORITE(): Block { return self::get('POLISHED_DIORITE'); }
    public static function POLISHED_DIORITE_SLAB(): Block { return self::get('POLISHED_DIORITE_SLAB'); }
    public static function POLISHED_DIORITE_STAIRS(): Block { return self::get('POLISHED_DIORITE_STAIRS'); }
    public static function POLISHED_GRANITE(): Block { return self::get('POLISHED_GRANITE'); }
    public static function POLISHED_GRANITE_SLAB(): Block { return self::get('POLISHED_GRANITE_SLAB'); }
    public static function POLISHED_GRANITE_STAIRS(): Block { return self::get('POLISHED_GRANITE_STAIRS'); }
    public static function POLISHED_SULFUR(): Block { return self::get('POLISHED_SULFUR'); }
    public static function POLISHED_SULFUR_SLAB(): Block { return self::get('POLISHED_SULFUR_SLAB'); }
    public static function POLISHED_SULFUR_STAIRS(): Block { return self::get('POLISHED_SULFUR_STAIRS'); }
    public static function POLISHED_SULFUR_WALL(): Block { return self::get('POLISHED_SULFUR_WALL'); }
    public static function POLISHED_TUFF(): Block { return self::get('POLISHED_TUFF'); }
    public static function POLISHED_TUFF_SLAB(): Block { return self::get('POLISHED_TUFF_SLAB'); }
    public static function POLISHED_TUFF_STAIRS(): Block { return self::get('POLISHED_TUFF_STAIRS'); }
    public static function POLISHED_TUFF_WALL(): Block { return self::get('POLISHED_TUFF_WALL'); }
    public static function POPPY(): Block { return self::get('POPPY'); }
    public static function POTATOES(): Block { return self::get('POTATOES'); }
    public static function POTENT_SULFUR(): Block { return self::get('POTENT_SULFUR'); }
    public static function POTION_CAULDRON(): Block { return self::get('POTION_CAULDRON'); }
    public static function POWERED_RAIL(): Block { return self::get('POWERED_RAIL'); }
    public static function PRISMARINE(): Block { return self::get('PRISMARINE'); }
    public static function PRISMARINE_BRICKS(): Block { return self::get('PRISMARINE_BRICKS'); }
    public static function PRISMARINE_BRICKS_SLAB(): Block { return self::get('PRISMARINE_BRICKS_SLAB'); }
    public static function PRISMARINE_BRICKS_STAIRS(): Block { return self::get('PRISMARINE_BRICKS_STAIRS'); }
    public static function PRISMARINE_SLAB(): Block { return self::get('PRISMARINE_SLAB'); }
    public static function PRISMARINE_STAIRS(): Block { return self::get('PRISMARINE_STAIRS'); }
    public static function PRISMARINE_WALL(): Block { return self::get('PRISMARINE_WALL'); }
    public static function PUMPKIN(): Block { return self::get('PUMPKIN'); }
    public static function PUMPKIN_STEM(): Block { return self::get('PUMPKIN_STEM'); }
    public static function PURPLE_TORCH(): Block { return self::get('PURPLE_TORCH'); }
    public static function PURPUR(): Block { return self::get('PURPUR'); }
    public static function PURPUR_PILLAR(): Block { return self::get('PURPUR_PILLAR'); }
    public static function PURPUR_SLAB(): Block { return self::get('PURPUR_SLAB'); }
    public static function PURPUR_STAIRS(): Block { return self::get('PURPUR_STAIRS'); }
    public static function QUARTZ(): Block { return self::get('QUARTZ'); }
    public static function QUARTZ_BRICKS(): Block { return self::get('QUARTZ_BRICKS'); }
    public static function QUARTZ_PILLAR(): Block { return self::get('QUARTZ_PILLAR'); }
    public static function QUARTZ_SLAB(): Block { return self::get('QUARTZ_SLAB'); }
    public static function QUARTZ_STAIRS(): Block { return self::get('QUARTZ_STAIRS'); }
    public static function RAIL(): Block { return self::get('RAIL'); }
    public static function RAW_COPPER(): Block { return self::get('RAW_COPPER'); }
    public static function RAW_GOLD(): Block { return self::get('RAW_GOLD'); }
    public static function RAW_IRON(): Block { return self::get('RAW_IRON'); }
    public static function REDSTONE(): Block { return self::get('REDSTONE'); }
    public static function REDSTONE_COMPARATOR(): Block { return self::get('REDSTONE_COMPARATOR'); }
    public static function REDSTONE_LAMP(): Block { return self::get('REDSTONE_LAMP'); }
    public static function REDSTONE_ORE(): Block { return self::get('REDSTONE_ORE'); }
    public static function REDSTONE_REPEATER(): Block { return self::get('REDSTONE_REPEATER'); }
    public static function REDSTONE_TORCH(): Block { return self::get('REDSTONE_TORCH'); }
    public static function REDSTONE_WIRE(): Block { return self::get('REDSTONE_WIRE'); }
    public static function RED_MUSHROOM(): Block { return self::get('RED_MUSHROOM'); }
    public static function RED_MUSHROOM_BLOCK(): Block { return self::get('RED_MUSHROOM_BLOCK'); }
    public static function RED_NETHER_BRICKS(): Block { return self::get('RED_NETHER_BRICKS'); }
    public static function RED_NETHER_BRICK_SLAB(): Block { return self::get('RED_NETHER_BRICK_SLAB'); }
    public static function RED_NETHER_BRICK_STAIRS(): Block { return self::get('RED_NETHER_BRICK_STAIRS'); }
    public static function RED_NETHER_BRICK_WALL(): Block { return self::get('RED_NETHER_BRICK_WALL'); }
    public static function RED_SAND(): Block { return self::get('RED_SAND'); }
    public static function RED_SANDSTONE(): Block { return self::get('RED_SANDSTONE'); }
    public static function RED_SANDSTONE_SLAB(): Block { return self::get('RED_SANDSTONE_SLAB'); }
    public static function RED_SANDSTONE_STAIRS(): Block { return self::get('RED_SANDSTONE_STAIRS'); }
    public static function RED_SANDSTONE_WALL(): Block { return self::get('RED_SANDSTONE_WALL'); }
    public static function RED_TORCH(): Block { return self::get('RED_TORCH'); }
    public static function RED_TULIP(): Block { return self::get('RED_TULIP'); }
    public static function REINFORCED_DEEPSLATE(): Block { return self::get('REINFORCED_DEEPSLATE'); }
    public static function RESERVED6(): Block { return self::get('RESERVED6'); }
    public static function RESIN(): Block { return self::get('RESIN'); }
    public static function RESIN_BRICKS(): Block { return self::get('RESIN_BRICKS'); }
    public static function RESIN_BRICK_SLAB(): Block { return self::get('RESIN_BRICK_SLAB'); }
    public static function RESIN_BRICK_STAIRS(): Block { return self::get('RESIN_BRICK_STAIRS'); }
    public static function RESIN_BRICK_WALL(): Block { return self::get('RESIN_BRICK_WALL'); }
    public static function RESIN_CLUMP(): Block { return self::get('RESIN_CLUMP'); }
    public static function RESPAWN_ANCHOR(): Block { return self::get('RESPAWN_ANCHOR'); }
    public static function ROSE_BUSH(): Block { return self::get('ROSE_BUSH'); }
    public static function SAND(): Block { return self::get('SAND'); }
    public static function SANDSTONE(): Block { return self::get('SANDSTONE'); }
    public static function SANDSTONE_SLAB(): Block { return self::get('SANDSTONE_SLAB'); }
    public static function SANDSTONE_STAIRS(): Block { return self::get('SANDSTONE_STAIRS'); }
    public static function SANDSTONE_WALL(): Block { return self::get('SANDSTONE_WALL'); }
    public static function SCULK(): Block { return self::get('SCULK'); }
    public static function SEA_LANTERN(): Block { return self::get('SEA_LANTERN'); }
    public static function SEA_PICKLE(): Block { return self::get('SEA_PICKLE'); }
    public static function SHROOMLIGHT(): Block { return self::get('SHROOMLIGHT'); }
    public static function SHULKER_BOX(): Block { return self::get('SHULKER_BOX'); }
    public static function SLIME(): Block { return self::get('SLIME'); }
    public static function SMALL_DRIPLEAF(): Block { return self::get('SMALL_DRIPLEAF'); }
    public static function SMITHING_TABLE(): Block { return self::get('SMITHING_TABLE'); }
    public static function SMOKER(): Block { return self::get('SMOKER'); }
    public static function SMOOTH_BASALT(): Block { return self::get('SMOOTH_BASALT'); }
    public static function SMOOTH_QUARTZ(): Block { return self::get('SMOOTH_QUARTZ'); }
    public static function SMOOTH_QUARTZ_SLAB(): Block { return self::get('SMOOTH_QUARTZ_SLAB'); }
    public static function SMOOTH_QUARTZ_STAIRS(): Block { return self::get('SMOOTH_QUARTZ_STAIRS'); }
    public static function SMOOTH_RED_SANDSTONE(): Block { return self::get('SMOOTH_RED_SANDSTONE'); }
    public static function SMOOTH_RED_SANDSTONE_SLAB(): Block { return self::get('SMOOTH_RED_SANDSTONE_SLAB'); }
    public static function SMOOTH_RED_SANDSTONE_STAIRS(): Block { return self::get('SMOOTH_RED_SANDSTONE_STAIRS'); }
    public static function SMOOTH_SANDSTONE(): Block { return self::get('SMOOTH_SANDSTONE'); }
    public static function SMOOTH_SANDSTONE_SLAB(): Block { return self::get('SMOOTH_SANDSTONE_SLAB'); }
    public static function SMOOTH_SANDSTONE_STAIRS(): Block { return self::get('SMOOTH_SANDSTONE_STAIRS'); }
    public static function SMOOTH_STONE(): Block { return self::get('SMOOTH_STONE'); }
    public static function SMOOTH_STONE_SLAB(): Block { return self::get('SMOOTH_STONE_SLAB'); }
    public static function SNOW(): Block { return self::get('SNOW'); }
    public static function SNOW_LAYER(): Block { return self::get('SNOW_LAYER'); }
    public static function SOUL_CAMPFIRE(): Block { return self::get('SOUL_CAMPFIRE'); }
    public static function SOUL_FIRE(): Block { return self::get('SOUL_FIRE'); }
    public static function SOUL_LANTERN(): Block { return self::get('SOUL_LANTERN'); }
    public static function SOUL_SAND(): Block { return self::get('SOUL_SAND'); }
    public static function SOUL_SOIL(): Block { return self::get('SOUL_SOIL'); }
    public static function SOUL_TORCH(): Block { return self::get('SOUL_TORCH'); }
    public static function SPONGE(): Block { return self::get('SPONGE'); }
    public static function SPORE_BLOSSOM(): Block { return self::get('SPORE_BLOSSOM'); }
    public static function SPRUCE_BUTTON(): Block { return self::get('SPRUCE_BUTTON'); }
    public static function SPRUCE_CEILING_CENTER_HANGING_SIGN(): Block { return self::get('SPRUCE_CEILING_CENTER_HANGING_SIGN'); }
    public static function SPRUCE_CEILING_EDGES_HANGING_SIGN(): Block { return self::get('SPRUCE_CEILING_EDGES_HANGING_SIGN'); }
    public static function SPRUCE_DOOR(): Block { return self::get('SPRUCE_DOOR'); }
    public static function SPRUCE_FENCE(): Block { return self::get('SPRUCE_FENCE'); }
    public static function SPRUCE_FENCE_GATE(): Block { return self::get('SPRUCE_FENCE_GATE'); }
    public static function SPRUCE_LEAVES(): Block { return self::get('SPRUCE_LEAVES'); }
    public static function SPRUCE_LOG(): Block { return self::get('SPRUCE_LOG'); }
    public static function SPRUCE_PLANKS(): Block { return self::get('SPRUCE_PLANKS'); }
    public static function SPRUCE_PRESSURE_PLATE(): Block { return self::get('SPRUCE_PRESSURE_PLATE'); }
    public static function SPRUCE_SAPLING(): Block { return self::get('SPRUCE_SAPLING'); }
    public static function SPRUCE_SIGN(): Block { return self::get('SPRUCE_SIGN'); }
    public static function SPRUCE_SLAB(): Block { return self::get('SPRUCE_SLAB'); }
    public static function SPRUCE_STAIRS(): Block { return self::get('SPRUCE_STAIRS'); }
    public static function SPRUCE_TRAPDOOR(): Block { return self::get('SPRUCE_TRAPDOOR'); }
    public static function SPRUCE_WALL_HANGING_SIGN(): Block { return self::get('SPRUCE_WALL_HANGING_SIGN'); }
    public static function SPRUCE_WALL_SIGN(): Block { return self::get('SPRUCE_WALL_SIGN'); }
    public static function SPRUCE_WOOD(): Block { return self::get('SPRUCE_WOOD'); }
    public static function STAINED_CLAY(): Block { return self::get('STAINED_CLAY'); }
    public static function STAINED_GLASS(): Block { return self::get('STAINED_GLASS'); }
    public static function STAINED_GLASS_PANE(): Block { return self::get('STAINED_GLASS_PANE'); }
    public static function STAINED_HARDENED_GLASS(): Block { return self::get('STAINED_HARDENED_GLASS'); }
    public static function STAINED_HARDENED_GLASS_PANE(): Block { return self::get('STAINED_HARDENED_GLASS_PANE'); }
    public static function STONE(): Block { return self::get('STONE'); }
    public static function STONECUTTER(): Block { return self::get('STONECUTTER'); }
    public static function STONE_BRICKS(): Block { return self::get('STONE_BRICKS'); }
    public static function STONE_BRICK_SLAB(): Block { return self::get('STONE_BRICK_SLAB'); }
    public static function STONE_BRICK_STAIRS(): Block { return self::get('STONE_BRICK_STAIRS'); }
    public static function STONE_BRICK_WALL(): Block { return self::get('STONE_BRICK_WALL'); }
    public static function STONE_BUTTON(): Block { return self::get('STONE_BUTTON'); }
    public static function STONE_PRESSURE_PLATE(): Block { return self::get('STONE_PRESSURE_PLATE'); }
    public static function STONE_SLAB(): Block { return self::get('STONE_SLAB'); }
    public static function STONE_STAIRS(): Block { return self::get('STONE_STAIRS'); }
    public static function STRUCTURE_VOID(): Block { return self::get('STRUCTURE_VOID'); }
    public static function SUGARCANE(): Block { return self::get('SUGARCANE'); }
    public static function SULFUR(): Block { return self::get('SULFUR'); }
    public static function SULFUR_BRICKS(): Block { return self::get('SULFUR_BRICKS'); }
    public static function SULFUR_BRICK_SLAB(): Block { return self::get('SULFUR_BRICK_SLAB'); }
    public static function SULFUR_BRICK_STAIRS(): Block { return self::get('SULFUR_BRICK_STAIRS'); }
    public static function SULFUR_BRICK_WALL(): Block { return self::get('SULFUR_BRICK_WALL'); }
    public static function SULFUR_SLAB(): Block { return self::get('SULFUR_SLAB'); }
    public static function SULFUR_STAIRS(): Block { return self::get('SULFUR_STAIRS'); }
    public static function SULFUR_WALL(): Block { return self::get('SULFUR_WALL'); }
    public static function SUNFLOWER(): Block { return self::get('SUNFLOWER'); }
    public static function SWEET_BERRY_BUSH(): Block { return self::get('SWEET_BERRY_BUSH'); }
    public static function TALL_GRASS(): Block { return self::get('TALL_GRASS'); }
    public static function TINTED_GLASS(): Block { return self::get('TINTED_GLASS'); }
    public static function TNT(): Block { return self::get('TNT'); }
    public static function TORCH(): Block { return self::get('TORCH'); }
    public static function TORCHFLOWER(): Block { return self::get('TORCHFLOWER'); }
    public static function TORCHFLOWER_CROP(): Block { return self::get('TORCHFLOWER_CROP'); }
    public static function TRAPPED_CHEST(): Block { return self::get('TRAPPED_CHEST'); }
    public static function TRIPWIRE(): Block { return self::get('TRIPWIRE'); }
    public static function TRIPWIRE_HOOK(): Block { return self::get('TRIPWIRE_HOOK'); }
    public static function TUFF(): Block { return self::get('TUFF'); }
    public static function TUFF_BRICKS(): Block { return self::get('TUFF_BRICKS'); }
    public static function TUFF_BRICK_SLAB(): Block { return self::get('TUFF_BRICK_SLAB'); }
    public static function TUFF_BRICK_STAIRS(): Block { return self::get('TUFF_BRICK_STAIRS'); }
    public static function TUFF_BRICK_WALL(): Block { return self::get('TUFF_BRICK_WALL'); }
    public static function TUFF_SLAB(): Block { return self::get('TUFF_SLAB'); }
    public static function TUFF_STAIRS(): Block { return self::get('TUFF_STAIRS'); }
    public static function TUFF_WALL(): Block { return self::get('TUFF_WALL'); }
    public static function TWISTING_VINES(): Block { return self::get('TWISTING_VINES'); }
    public static function UNDERWATER_TORCH(): Block { return self::get('UNDERWATER_TORCH'); }
    public static function VINES(): Block { return self::get('VINES'); }
    public static function WALL_BANNER(): Block { return self::get('WALL_BANNER'); }
    public static function WALL_CORAL_FAN(): Block { return self::get('WALL_CORAL_FAN'); }
    public static function WARPED_BUTTON(): Block { return self::get('WARPED_BUTTON'); }
    public static function WARPED_CEILING_CENTER_HANGING_SIGN(): Block { return self::get('WARPED_CEILING_CENTER_HANGING_SIGN'); }
    public static function WARPED_CEILING_EDGES_HANGING_SIGN(): Block { return self::get('WARPED_CEILING_EDGES_HANGING_SIGN'); }
    public static function WARPED_DOOR(): Block { return self::get('WARPED_DOOR'); }
    public static function WARPED_FENCE(): Block { return self::get('WARPED_FENCE'); }
    public static function WARPED_FENCE_GATE(): Block { return self::get('WARPED_FENCE_GATE'); }
    public static function WARPED_FUNGUS(): Block { return self::get('WARPED_FUNGUS'); }
    public static function WARPED_HYPHAE(): Block { return self::get('WARPED_HYPHAE'); }
    public static function WARPED_NYLIUM(): Block { return self::get('WARPED_NYLIUM'); }
    public static function WARPED_PLANKS(): Block { return self::get('WARPED_PLANKS'); }
    public static function WARPED_PRESSURE_PLATE(): Block { return self::get('WARPED_PRESSURE_PLATE'); }
    public static function WARPED_ROOTS(): Block { return self::get('WARPED_ROOTS'); }
    public static function WARPED_SIGN(): Block { return self::get('WARPED_SIGN'); }
    public static function WARPED_SLAB(): Block { return self::get('WARPED_SLAB'); }
    public static function WARPED_STAIRS(): Block { return self::get('WARPED_STAIRS'); }
    public static function WARPED_STEM(): Block { return self::get('WARPED_STEM'); }
    public static function WARPED_TRAPDOOR(): Block { return self::get('WARPED_TRAPDOOR'); }
    public static function WARPED_WALL_HANGING_SIGN(): Block { return self::get('WARPED_WALL_HANGING_SIGN'); }
    public static function WARPED_WALL_SIGN(): Block { return self::get('WARPED_WALL_SIGN'); }
    public static function WARPED_WART_BLOCK(): Block { return self::get('WARPED_WART_BLOCK'); }
    public static function WATER(): Block { return self::get('WATER'); }
    public static function WATER_CAULDRON(): Block { return self::get('WATER_CAULDRON'); }
    public static function WEEPING_VINES(): Block { return self::get('WEEPING_VINES'); }
    public static function WEIGHTED_PRESSURE_PLATE_HEAVY(): Block { return self::get('WEIGHTED_PRESSURE_PLATE_HEAVY'); }
    public static function WEIGHTED_PRESSURE_PLATE_LIGHT(): Block { return self::get('WEIGHTED_PRESSURE_PLATE_LIGHT'); }
    public static function WET_POTENT_SULFUR(): Block { return self::get('WET_POTENT_SULFUR'); }
    public static function WHEAT(): Block { return self::get('WHEAT'); }
    public static function WHITE_TULIP(): Block { return self::get('WHITE_TULIP'); }
    public static function WITHER_ROSE(): Block { return self::get('WITHER_ROSE'); }
    public static function WOOL(): Block { return self::get('WOOL'); }
}
