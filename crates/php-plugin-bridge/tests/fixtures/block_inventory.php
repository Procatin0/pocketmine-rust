<?php

declare(strict_types=1);

/**
 * Test de fidelidad para block/inventory (Ola 6b):
 * - BlockInventory interface
 * - 19 clases e inventarios de bloque
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

use pocketmine\block\inventory\BlockInventory;
use pocketmine\block\inventory\ChestInventory;
use pocketmine\block\inventory\DoubleChestInventory;
use pocketmine\block\inventory\EnderChestInventory;
use pocketmine\block\inventory\FurnaceInventory;
use pocketmine\block\inventory\BarrelInventory;
use pocketmine\block\inventory\HopperInventory;
use pocketmine\block\inventory\BrewingStandInventory;
use pocketmine\block\inventory\ShulkerBoxInventory;
use pocketmine\block\inventory\CampfireInventory;
use pocketmine\block\inventory\AnvilInventory;
use pocketmine\block\inventory\EnchantInventory;
use pocketmine\block\inventory\LoomInventory;
use pocketmine\block\inventory\StonecutterInventory;
use pocketmine\block\inventory\CartographyTableInventory;
use pocketmine\block\inventory\CraftingTableInventory;
use pocketmine\block\inventory\SmithingTableInventory;
use pocketmine\crafting\FurnaceType;
use pocketmine\inventory\Inventory;
use pocketmine\world\Position;

// 1. Interfaces
$check('BlockInventory exists', true, interface_exists(BlockInventory::class));
$check('ChestInventory implements BlockInventory', true, is_subclass_of(ChestInventory::class, BlockInventory::class));
$check('DoubleChestInventory implements BlockInventory', true, is_subclass_of(DoubleChestInventory::class, BlockInventory::class));
$check('BarrelInventory implements BlockInventory', true, is_subclass_of(BarrelInventory::class, BlockInventory::class));
$check('FurnaceInventory implements BlockInventory', true, is_subclass_of(FurnaceInventory::class, BlockInventory::class));
$check('CraftingTableInventory implements BlockInventory', true, is_subclass_of(CraftingTableInventory::class, BlockInventory::class));
$check('CartographyTableInventory implements BlockInventory', true, is_subclass_of(CartographyTableInventory::class, BlockInventory::class));
$check('HopperInventory implements BlockInventory', true, is_subclass_of(HopperInventory::class, BlockInventory::class));
$check('BrewingStandInventory implements BlockInventory', true, is_subclass_of(BrewingStandInventory::class, BlockInventory::class));
$check('ShulkerBoxInventory implements BlockInventory', true, is_subclass_of(ShulkerBoxInventory::class, BlockInventory::class));
$check('CampfireInventory implements BlockInventory', true, is_subclass_of(CampfireInventory::class, BlockInventory::class));

// 2. Instanciación y tamaños
$pos = new Position(10, 64, 10, 'world');
$chestInv = new ChestInventory($pos);
$check('ChestInventory size', 27, $chestInv->getSize());
$check('ChestInventory holder is Position', true, $chestInv->getHolder() instanceof Position);

$furnaceInv = new FurnaceInventory($pos, FurnaceType::FURNACE);
$check('FurnaceInventory size', 3, $furnaceInv->getSize());
$check('FurnaceInventory furnaceType', FurnaceType::FURNACE, $furnaceInv->getFurnaceType());

$hopperInv = new HopperInventory($pos);
$check('HopperInventory size', 5, $hopperInv->getSize());

$shulkerInv = new ShulkerBoxInventory($pos);
$check('ShulkerBoxInventory size', 27, $shulkerInv->getSize());

$campfireInv = new CampfireInventory($pos);
$check('CampfireInventory size', 4, $campfireInv->getSize());

if ($failures > 0) {
    fwrite(STDERR, $failures . " failure(s)\n");
    exit(1);
}

echo "ALL OK\n";
