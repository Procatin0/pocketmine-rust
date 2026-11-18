<?php

declare(strict_types=1);

/**
 * Test de fidelidad para el núcleo del módulo `pocketmine\block`:
 * - Block (70 métodos públicos)
 * - BlockIdentifier, BlockBreakInfo, BlockTypeInfo, BlockTypeTags, BlockToolType
 * - BlockTypeIds (constantes y registro dinámico)
 * - VanillaBlocks (__callStatic, alias legacy, getAll)
 */

$dir = $argv[1] ?? '';
if ($dir === '' || !is_file($dir . '/runtime_files.php')) {
    fwrite(STDERR, "missing PHP runtime directory argument\n");
    exit(2);
}
foreach (require $dir . '/runtime_files.php' as $file) {
    require $dir . '/' . $file;
}

$failures = 0;
$check = function (string $what, $expected, $actual) use (&$failures): void {
    if ($expected !== $actual) {
        $failures++;
        fwrite(STDERR, 'FAIL ' . $what . ': expected ' . var_export($expected, true) . ', got ' . var_export($actual, true) . "\n");
    }
};

use pocketmine\block\Block;
use pocketmine\block\BlockIdentifier;
use pocketmine\block\BlockBreakInfo;
use pocketmine\block\BlockTypeInfo;
use pocketmine\block\BlockTypeTags;
use pocketmine\block\BlockToolType;
use pocketmine\block\BlockTypeIds;
use pocketmine\block\VanillaBlocks;
use pocketmine\block\Air;
use pocketmine\block\Opaque;
use pocketmine\block\Transparent;
use pocketmine\block\Flowable;
use pocketmine\block\utils\SupportType;
use pocketmine\math\Vector3;
use pocketmine\math\Facing;
use pocketmine\math\AxisAlignedBB;

// 1. Verificación de los 70 métodos públicos en Block
$ref = new \ReflectionClass(Block::class);
$publicMethods = [];
foreach ($ref->getMethods(\ReflectionMethod::IS_PUBLIC) as $m) {
    if ($m->getDeclaringClass()->getName() === Block::class) {
        $publicMethods[] = $m->getName();
    }
}
$expectedCount = 70;
$check('Block public methods count >= 70', true, count($publicMethods) >= $expectedCount);

$mustHave = [
    'getIdInfo', 'getName', 'asItem', 'getBreakInfo', 'getDrops',
    'getDropsForCompatibleTool', 'getDropsForIncompatibleTool', 'getXpDropForTool',
    'isAffectedBySilkTouch', 'getPickedItem', 'getFlameEncouragement', 'getFlammability',
    'burnsForever', 'getLightLevel', 'getLightFilter', 'blocksDirectSkyLight',
    'getFrictionFactor', 'isTransparent', 'isSolid', 'canBeFlowedInto', 'canBeReplaced',
    'canClimb', 'hasEntityCollision', 'getSupportType', 'getCollisionBoxes',
    'hasSameTypeId', 'isSameState', 'getSide', 'getAllSides', 'getHorizontalSides',
    'onInteract', 'onNearbyBlockChange', 'onRandomTick', 'onScheduledUpdate',
    'onEntityLand', 'onProjectileHit', 'onAttack', 'onBreak', 'onEntityInside',
    'onIncinerate', 'onPostPlace', 'ticksRandomly', 'describeBlockItemState',
    'writeStateToWorld', 'readStateFromWorld', 'place', 'canBePlaced', 'canBePlacedAt',
    '__clone', 'getStateId', 'getTypeId', 'getTypeTags', 'hasTypeTag', 'isFullCube',
    'isFlammable', 'calculateIntercept', 'collidesWithBB', 'addVelocityToEntity'
];
foreach ($mustHave as $methodName) {
    $check("Block has method $methodName", true, $ref->hasMethod($methodName));
}

// 2. BlockIdentifier y BlockTypeInfo
$id = new BlockIdentifier(BlockTypeIds::STONE);
$breakInfo = BlockBreakInfo::tier(1.5, BlockToolType::PICKAXE, 1);
$typeInfo = new BlockTypeInfo($breakInfo, [BlockTypeTags::DIRT]);
$check('BlockTypeInfo getBreakInfo', $breakInfo, $typeInfo->getBreakInfo());
$check('BlockTypeInfo hasTypeTag', true, $typeInfo->hasTypeTag(BlockTypeTags::DIRT));
$check('BlockTypeInfo hasTypeTag false', false, $typeInfo->hasTypeTag(BlockTypeTags::MUD));

// 3. BlockTypeIds
$check('BlockTypeIds::AIR is int > 0', true, is_int(BlockTypeIds::AIR) && BlockTypeIds::AIR > 0);
$check('BlockTypeIds::STONE is int > 0', true, is_int(BlockTypeIds::STONE) && BlockTypeIds::STONE > 0);
$check('BlockTypeIds::DIRT is int > 0', true, is_int(BlockTypeIds::DIRT) && BlockTypeIds::DIRT > 0);
$check('BlockTypeIds::forName stone', BlockTypeIds::STONE, BlockTypeIds::forName('stone'));
$check('BlockTypeIds::forName minecraft:stone', BlockTypeIds::STONE, BlockTypeIds::forName('minecraft:stone'));
$newId = BlockTypeIds::newId();
$check('BlockTypeIds::newId is int > 0', true, is_int($newId) && $newId > 800);

// 4. VanillaBlocks
$stone = VanillaBlocks::STONE();
$check('VanillaBlocks::STONE is Block', true, $stone instanceof Block);
$check('stone is solid', true, $stone->isSolid());
$check('stone is not transparent', false, $stone->isTransparent());

// Alias legacy en VanillaBlocks
$grassBlock = VanillaBlocks::GRASS_BLOCK();
$grass = VanillaBlocks::GRASS();
$check('GRASS_BLOCK and GRASS return same type', $grassBlock->getTypeId(), $grass->getTypeId());

// getAll
$all = VanillaBlocks::getAll();
$check('VanillaBlocks::getAll returns many blocks', true, count($all) > 500);

// 5. Métodos espaciales de Block
$world = new \pocketmine\world\World('test_world', null, 0);
$b = clone $stone;
$b->position($world, 10, 64, 10);
$sideUp = $b->getSide(Facing::UP);
$check('getSide UP y coordinate', 65.0, (float)$sideUp->getPosition()->y);
$sideNorth = $b->getSide(Facing::NORTH);
$check('getSide NORTH z coordinate', 9.0, (float)$sideNorth->getPosition()->z);

if ($failures > 0) {
    fwrite(STDERR, $failures . " failure(s)\n");
    exit(1);
}

echo "ALL OK\n";
