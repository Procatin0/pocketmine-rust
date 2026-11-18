<?php
declare(strict_types=1);

$inventory = json_decode(file_get_contents(__DIR__ . '/block_inventory.json'), true);
$declarations = $inventory['declarations'];

// Rules for family classification:
// 5: Tiles -> anything in block/tile/
// 6: Utils & Inventory -> anything in block/inventory/ or block/utils/ (unless assigned to geometry)
// Let's see: user prompt says:
// Family (1) núcleo/registro
// Family (2) formas y geometría
// Family (3) interactivos/contenedores/redstone
// Family (4) fluidos/fuego/plantas/crecimiento
// Family (5) tiles
// Family (6) utils/inventory

// Let's refine the assignment for each declaration:
function classify(array $d): int {
    $file = $d['file'];
    $name = $d['name'];
    $fqcn = $d['fqcn'];
    
    // Family 5: Tiles
    if (str_starts_with($file, 'block/tile/')) {
        return 5;
    }
    
    // Family 6: Inventory
    if (str_starts_with($file, 'block/inventory/')) {
        return 6;
    }
    
    // Family 1: Core / Registry
    $coreNames = [
        'Block', 'BlockIdentifier', 'BlockBreakInfo', 'BlockTypeInfo', 'BlockTypeIds',
        'VanillaBlocks', 'VanillaBlocksInputs', 'BlockTypeTags', 'BlockToolType',
        'RuntimeBlockStateRegistry', 'UnknownBlock'
    ];
    if (in_array($name, $coreNames, true) && dirname($file) === 'block') {
        return 1;
    }
    
    // Family 6: Utils (if in block/utils)
    if (str_starts_with($file, 'block/utils/')) {
        // Some utils traits/enums are referenced in geometry or interactive, but they are physically in utils/.
        // User says: "(6) utils/inventory" - Enums/traits restantes de utils/, plus inventory/.
        // But Ola 2 says: "Traits/enums de utils/: DyeColor, WoodType, WoodMaterial, Colored, AnyFacing, HorizontalFacing, PillarRotation, SlabType, StairShape, WallConnectionType, SupportType, Lightable, PoweredByRedstone, etc."
        // Let's keep utils in Family 6 or Family 2? Notice: "Congela el reparto de archivos antes de spawnear. block.php es el punto caliente: el líder lo parte en block.php (núcleo) + block_shapes.php + block_interactive.php + block_fluids.php + block_plants.php + block_tiles.php + block_utils.php, o asigna un único dueño por archivo."
        // If file is block_utils.php, then ALL of block/utils/ can go to block_utils.php (or shared by traits), or if a subagent owns block_utils.php!
        return 6;
    }
    
    // For root block classes (276 - core):
    // Let's classify into (2) formas/geometría, (3) interactivos/redstone/contenedores, (4) fluidos/plantas/fuego
    
    // Fluids, Fire, Plants, Growth
    $plantWords = [
        'Plant', 'Flower', 'Sapling', 'Leaves', 'Cactus', 'Bamboo', 'Crop', 'Stem', 'Vine', 'Mushroom',
        'Grass', 'Fern', 'Azalea', 'Bush', 'Wheat', 'Carrot', 'Potato', 'Beetroot', 'Melon', 'Pumpkin',
        'Cocoa', 'SugarCane', 'Sugarcane', 'NetherWart', 'SeaPickle', 'Kelp', 'Seagrass', 'Coral',
        'Spore', 'Dripleaf', 'Fungus', 'Roots', 'Nylium', 'Mycelium', 'Lichen', 'LilyPad', 'SweetBerry',
        'Chorus', 'Propagule', 'Mangrove', 'Pitcher', 'Torchflower', 'SnifferEgg', 'Frogspawn'
    ];
    $fluidFireWords = [
        'Liquid', 'Water', 'Lava', 'Fire', 'SoulFire', 'Flowable', 'Air', 'BaseFire', 'Ice', 'FrostedIce',
        'Snow', 'SnowLayer', 'Cobweb'
    ];
    
    foreach ($plantWords as $pw) {
        if (str_contains($name, $pw)) {
            return 4;
        }
    }
    foreach ($fluidFireWords as $fw) {
        if (str_contains($name, $fw)) {
            return 4;
        }
    }
    
    // Interactive, Containers, Redstone, Functional
    $interactiveWords = [
        'Chest', 'Furnace', 'Barrel', 'Hopper', 'Shulker', 'Brewing', 'Campfire', 'Cauldron',
        'Bed', 'Door', 'Trapdoor', 'Button', 'Lever', 'PressurePlate', 'Redstone', 'Repeater',
        'Comparator', 'Daylight', 'Sensor', 'Observer', 'Piston', 'Dispenser', 'Dropper',
        'Note', 'Jukebox', 'Lectern', 'Beacon', 'Anvil', 'Table', 'Bell', 'Frame', 'Head',
        'Skull', 'Stand', 'Pot', 'Respawn', 'Lodestone', 'Target', 'TNT', 'Tripwire',
        'Spawner', 'Conduit', 'Lightning', 'Sculk', 'Crafter', 'Vault', 'Trial', 'CopperBulb',
        'DetectorRail', 'PoweredRail', 'ActivatorRail', 'Chime', 'DecoratedPot'
    ];
    foreach ($interactiveWords as $iw) {
        if (str_contains($name, $iw)) {
            return 3;
        }
    }
    
    // Shapes and Geometry:
    // Stairs, Slabs, Walls, Fences, Gates, Thin, Pillars, Rail, Carpet, Ladder, Chain, Glass, Opaque, Transparent...
    $shapeWords = [
        'Stair', 'Slab', 'Wall', 'Fence', 'Gate', 'Thin', 'Pillar', 'Rail', 'Carpet',
        'Ladder', 'Chain', 'IronBars', 'GlassPane', 'Pane', 'Layer', 'EndRod', 'AmethystCluster',
        'PointedDripstone', 'Lantern', 'Torch', 'Rod'
    ];
    foreach ($shapeWords as $sw) {
        if (str_contains($name, $sw)) {
            return 2;
        }
    }
    
    // General base/opaque/transparent shapes
    $baseShapes = [
        'Opaque', 'Transparent', 'SimplePillar', 'Rotatable', 'HorizontalFacing', 'Planks', 'Wood',
        'Stone', 'Concrete', 'Terracotta', 'GlazedTerracotta', 'Wool', 'StainedGlass', 'Glass',
        'HardenedClay', 'Sandstone', 'Prismarine', 'Purpur', 'NetherBrick', 'EndStone', 'Blackstone',
        'Basalt', 'Deepslate', 'Tuff', 'Mud', 'Clay', 'Sand', 'Gravel', 'Dirt', 'DirtPath',
        'Farmland', 'Bedrock', 'Obsidian', 'CryingObsidian', 'AmethystBlock', 'Copper', 'Ore',
        'RawCopper', 'RawIron', 'RawGold', 'Gold', 'Iron', 'Diamond', 'Emerald', 'Lapis', 'Coal',
        'Netherite', 'Quartz', 'MonsterEggBlock', 'Infested', 'Sponge', 'Slime', 'HoneyBlock',
        'HayBale', 'BoneBlock', 'Target', 'SeaLantern', 'Glowstone', 'Magma', 'Shroomlight',
        'StructureVoid', 'ReinforcedDeepslate', 'Light'
    ];
    foreach ($baseShapes as $bw) {
        if (str_contains($name, $bw)) {
            return 2;
        }
    }

    // Default to Shapes & Geometry if it is a physical block, otherwise check
    return 2;
}

$counts = [1 => 0, 2 => 0, 3 => 0, 4 => 0, 5 => 0, 6 => 0];
$familyNames = [
    1 => '(1) Núcleo y registro',
    2 => '(2) Formas y geometría',
    3 => '(3) Interactivos, contenedores y redstone',
    4 => '(4) Fluidos, fuego, plantas y crecimiento',
    5 => '(5) Tiles',
    6 => '(6) Utils y inventory',
];

$classified = [];
foreach ($declarations as $d) {
    $fam = classify($d);
    $counts[$fam]++;
    $d['family'] = $fam;
    $classified[] = $d;
}

echo "Classification summary:\n";
foreach ($counts as $f => $cnt) {
    echo "Family $f ({$familyNames[$f]}): $cnt declarations\n";
}
echo "Total: " . array_sum($counts) . "\n";

file_put_contents(__DIR__ . '/block_classified.json', json_encode($classified, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES));
