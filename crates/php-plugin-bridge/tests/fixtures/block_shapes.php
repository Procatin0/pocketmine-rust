<?php

declare(strict_types=1);

/**
 * Test de fidelidad para el módulo de formas y geometría (Ola 2):
 * - Jerarquías: Transparent, Opaque, Thin, Wall, Stair, Slab, Fence, FenceGate, Rail, etc.
 * - recalculateCollisionBoxes() y getCollisionBoxes()
 * - Soporte, tipos y orientaciones
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
use pocketmine\block\Transparent;
use pocketmine\block\Opaque;
use pocketmine\block\Thin;
use pocketmine\block\Wall;
use pocketmine\block\Stair;
use pocketmine\block\Slab;
use pocketmine\block\Fence;
use pocketmine\block\FenceGate;
use pocketmine\block\Carpet;
use pocketmine\block\SnowLayer;
use pocketmine\block\Ladder;
use pocketmine\block\Chain;
use pocketmine\block\SimplePillar;
use pocketmine\block\BaseRail;
use pocketmine\block\Rail;
use pocketmine\block\StraightOnlyRail;
use pocketmine\block\PressurePlate;
use pocketmine\block\SimplePressurePlate;
use pocketmine\block\StonePressurePlate;
use pocketmine\block\WoodenPressurePlate;
use pocketmine\block\WeightedPressurePlate;
use pocketmine\block\utils\SupportType;
use pocketmine\block\utils\StairShape;
use pocketmine\block\utils\SlabType;
use pocketmine\block\utils\WoodType;
use pocketmine\math\AxisAlignedBB;
use pocketmine\math\Facing;

$id = new BlockIdentifier(1);
$info = new BlockTypeInfo(BlockBreakInfo::instant());

// 1. Jerarquías
$check('Transparent is Block', true, is_subclass_of(Transparent::class, Block::class));
$check('Opaque is Block', true, is_subclass_of(Opaque::class, Block::class));
$check('Thin is Transparent', true, is_subclass_of(Thin::class, Transparent::class));
$check('Wall is Transparent', true, is_subclass_of(Wall::class, Transparent::class));
$check('Stair is Transparent', true, is_subclass_of(Stair::class, Transparent::class));
$check('Slab is Transparent', true, is_subclass_of(Slab::class, Transparent::class));
$check('Fence is Transparent', true, is_subclass_of(Fence::class, Transparent::class));
$check('FenceGate is Transparent', true, is_subclass_of(FenceGate::class, Transparent::class));
$check('Ladder is Transparent', true, is_subclass_of(Ladder::class, Transparent::class));
$check('Chain is Transparent', true, is_subclass_of(Chain::class, Transparent::class));
$check('SimplePillar is Opaque', true, is_subclass_of(SimplePillar::class, Opaque::class));
$check('Rail is BaseRail', true, is_subclass_of(Rail::class, BaseRail::class));
$check('SimplePressurePlate is PressurePlate', true, is_subclass_of(SimplePressurePlate::class, PressurePlate::class));

// 2. Comportamiento e instanciación
$wall = new Wall($id, 'Stone Wall', $info);
$check('Wall collision boxes', true, count($wall->getCollisionBoxes()) >= 1);
$check('Wall collision box is AxisAlignedBB', true, $wall->getCollisionBoxes()[0] instanceof AxisAlignedBB);

$stair = new Stair($id, 'Stone Stairs', $info);
$check('Stair collision boxes', true, count($stair->getCollisionBoxes()) >= 1);

$slab = new Slab($id, 'Stone Slab', $info);
$check('Slab collision boxes', true, count($slab->getCollisionBoxes()) >= 1);
$check('Slab getSlabType default', SlabType::BOTTOM, $slab->getSlabType());

$ladder = new Ladder($id, 'Ladder', $info);
$check('Ladder canClimb', true, $ladder->canClimb());
$check('Ladder is not solid', false, $ladder->isSolid());

$snow = new SnowLayer($id, 'Top Snow', $info);
$check('SnowLayer layers', 1, $snow->getLayers());

$gate = new FenceGate($id, 'Oak Fence Gate', $info);
$check('FenceGate isOpen default', false, $gate->isOpen());

// 3. Soporte y enums
$check('SupportType FULL', 'FULL', SupportType::FULL->name);
$check('SupportType NONE', 'NONE', SupportType::NONE->name);

if ($failures > 0) {
    fwrite(STDERR, $failures . " failure(s)\n");
    exit(1);
}

echo "ALL OK\n";
