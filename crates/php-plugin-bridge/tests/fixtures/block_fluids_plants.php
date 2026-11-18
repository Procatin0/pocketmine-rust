<?php

declare(strict_types=1);

/**
 * Test de fidelidad para fluidos, fuego y plantas (Ola 4):
 * - Liquid, Water, Lava, BaseFire, Fire, SoulFire, Cobweb
 * - Plant, Leaves, Crops, Wheat, Carrot, Potato, Beetroot, Stem, Sapling
 * - Bamboo, Cactus, Sugarcane, NetherWartPlant, CocoaBlock, ChorusFlower
 * - Flower, WitherRose, TallGrass, DoublePlant, WaterLily, Mushroom
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
use pocketmine\block\Flowable;
use pocketmine\block\Liquid;
use pocketmine\block\Water;
use pocketmine\block\Lava;
use pocketmine\block\BaseFire;
use pocketmine\block\Fire;
use pocketmine\block\SoulFire;
use pocketmine\block\Cobweb;
use pocketmine\block\Plant;
use pocketmine\block\Leaves;
use pocketmine\block\Crops;
use pocketmine\block\Wheat;
use pocketmine\block\Carrot;
use pocketmine\block\Potato;
use pocketmine\block\Beetroot;
use pocketmine\block\Stem;
use pocketmine\block\Sapling;
use pocketmine\block\Bamboo;
use pocketmine\block\Cactus;
use pocketmine\block\Sugarcane;
use pocketmine\block\NetherWartPlant;
use pocketmine\block\CocoaBlock;
use pocketmine\block\ChorusFlower;
use pocketmine\block\Flower;
use pocketmine\block\WitherRose;
use pocketmine\block\TallGrass;
use pocketmine\block\DoublePlant;
use pocketmine\block\WaterLily;
use pocketmine\block\Mushroom;

$id = new BlockIdentifier(1);
$info = new BlockTypeInfo(BlockBreakInfo::instant());

// 1. Fluidos y fuego
$check('Liquid is Flowable', true, is_subclass_of(Liquid::class, Flowable::class));
$check('Water is Liquid', true, is_subclass_of(Water::class, Liquid::class));
$check('Lava is Liquid', true, is_subclass_of(Lava::class, Liquid::class));
$check('BaseFire is Flowable', true, is_subclass_of(BaseFire::class, Flowable::class));
$check('Fire is BaseFire', true, is_subclass_of(Fire::class, BaseFire::class));
$check('SoulFire is BaseFire', true, is_subclass_of(SoulFire::class, BaseFire::class));
$check('Cobweb is Flowable', true, is_subclass_of(Cobweb::class, Flowable::class));

$water = new Water($id, 'Water', $info);
$check('Water canBeFlowedInto', true, $water->canBeFlowedInto());
$check('Water is not solid', false, $water->isSolid());

$fire = new Fire($id, 'Fire', $info);
$check('Fire burnsForever false default', false, $fire->burnsForever());

// 2. Plantas y vegetación
$check('Leaves is Transparent', true, is_subclass_of(Leaves::class, Transparent::class));
$check('Plant is Flowable', true, is_subclass_of(Plant::class, Flowable::class));
$check('Crops is Flowable', true, is_subclass_of(Crops::class, Flowable::class));
$check('Wheat is Crops', true, is_subclass_of(Wheat::class, Crops::class));
$check('Carrot is Crops', true, is_subclass_of(Carrot::class, Crops::class));
$check('Potato is Crops', true, is_subclass_of(Potato::class, Crops::class));
$check('Beetroot is Crops', true, is_subclass_of(Beetroot::class, Crops::class));
$check('Stem is Flowable', true, is_subclass_of(Stem::class, Flowable::class));
$check('Sapling is Flowable', true, is_subclass_of(Sapling::class, Flowable::class));
$check('Flower is Flowable', true, is_subclass_of(Flower::class, Flowable::class));
$check('WitherRose is Flowable', true, is_subclass_of(WitherRose::class, Flowable::class));
$check('TallGrass is Flowable', true, is_subclass_of(TallGrass::class, Flowable::class));
$check('DoublePlant is Flowable', true, is_subclass_of(DoublePlant::class, Flowable::class));
$check('WaterLily is Flowable', true, is_subclass_of(WaterLily::class, Flowable::class));
$check('RedMushroom is Flowable', true, is_subclass_of(\pocketmine\block\RedMushroom::class, Flowable::class));

// 3. Comportamiento de cultivos
$wheat = new Wheat($id, 'Wheat', $info);
$check('Wheat max age', 7, $wheat::MAX_AGE);
$check('Wheat default age', 0, $wheat->getAge());
$wheat->setAge(5);
$check('Wheat modified age', 5, $wheat->getAge());

$leaves = new Leaves($id, 'Oak Leaves', $info);
$check('Leaves is transparent', true, $leaves->isTransparent());

if ($failures > 0) {
    fwrite(STDERR, $failures . " failure(s)\n");
    exit(1);
}

echo "ALL OK\n";
