<?php

declare(strict_types=1);

/**
 * Test de fidelidad para bloques interactivos, contenedores y redstone (Ola 3):
 * - Puertas y trampillas: Door, WoodenDoor, IronDoor, Trapdoor, WoodenTrapdoor, IronTrapdoor
 * - Mecanismos y botones: Button, WoodenButton, StoneButton, Lever, Tripwire, TripwireHook
 * - Contenedores y funcionales: Chest, EnderChest, Barrel, Furnace, BlastFurnace, Smoker,
 *   Hopper, ShulkerBox, BrewingStand, Campfire, Cauldron, Bed, Anvil, EnchantingTable, Bell
 * - Redstone: RedstoneWire, RedstoneRepeater, RedstoneComparator, DaylightSensor, Note, Jukebox, Lectern
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
use pocketmine\block\Door;
use pocketmine\block\WoodenDoor;
use pocketmine\block\IronDoor;
use pocketmine\block\Trapdoor;
use pocketmine\block\WoodenTrapdoor;
use pocketmine\block\IronTrapdoor;
use pocketmine\block\Button;
use pocketmine\block\WoodenButton;
use pocketmine\block\StoneButton;
use pocketmine\block\Lever;
use pocketmine\block\Tripwire;
use pocketmine\block\TripwireHook;
use pocketmine\block\Chest;
use pocketmine\block\EnderChest;
use pocketmine\block\Barrel;
use pocketmine\block\Furnace;
use pocketmine\block\BlastFurnace;
use pocketmine\block\Smoker;
use pocketmine\block\Hopper;
use pocketmine\block\ShulkerBox;
use pocketmine\block\BrewingStand;
use pocketmine\block\Campfire;
use pocketmine\block\Cauldron;
use pocketmine\block\Bed;
use pocketmine\block\RedstoneWire;
use pocketmine\block\RedstoneRepeater;
use pocketmine\block\RedstoneComparator;
use pocketmine\block\DaylightSensor;
use pocketmine\block\Note;
use pocketmine\block\Jukebox;
use pocketmine\block\Lectern;
use pocketmine\inventory\Inventory;

$id = new BlockIdentifier(1);
$info = new BlockTypeInfo(BlockBreakInfo::instant());

// 1. Jerarquías
$check('Door is Transparent', true, is_subclass_of(Door::class, Transparent::class));
$check('WoodenDoor is Door', true, is_subclass_of(WoodenDoor::class, Door::class));
$check('IronDoor is Door', true, is_subclass_of(IronDoor::class, Door::class));
$check('Trapdoor is Transparent', true, is_subclass_of(Trapdoor::class, Transparent::class));
$check('WoodenTrapdoor is Trapdoor', true, is_subclass_of(WoodenTrapdoor::class, Trapdoor::class));
$check('IronTrapdoor is Trapdoor', true, is_subclass_of(IronTrapdoor::class, Trapdoor::class));
$check('Button is Transparent', true, is_subclass_of(Button::class, Transparent::class));
$check('WoodenButton is Button', true, is_subclass_of(WoodenButton::class, Button::class));
$check('StoneButton is Button', true, is_subclass_of(StoneButton::class, Button::class));
$check('Lever is Transparent', true, is_subclass_of(Lever::class, Transparent::class));
$check('Chest is Transparent', true, is_subclass_of(Chest::class, Transparent::class));
$check('EnderChest is Transparent', true, is_subclass_of(EnderChest::class, Transparent::class));
$check('Hopper is Transparent', true, is_subclass_of(Hopper::class, Transparent::class));
$check('BrewingStand is Transparent', true, is_subclass_of(BrewingStand::class, Transparent::class));
$check('Bed is Transparent', true, is_subclass_of(Bed::class, Transparent::class));
$check('RedstoneWire is Transparent', true, is_subclass_of(RedstoneWire::class, Transparent::class));
$check('RedstoneRepeater is Transparent', true, is_subclass_of(RedstoneRepeater::class, Transparent::class));
$check('RedstoneComparator is Transparent', true, is_subclass_of(RedstoneComparator::class, Transparent::class));
$check('DaylightSensor is Transparent', true, is_subclass_of(DaylightSensor::class, Transparent::class));

// 2. Puertas y Trampillas
$door = new WoodenDoor($id, 'Oak Door', $info, \pocketmine\block\utils\WoodType::OAK);
$check('Door default closed', false, $door->isOpen());
$door->setOpen(true);
$check('Door modified open', true, $door->isOpen());

$trapdoor = new WoodenTrapdoor($id, 'Oak Trapdoor', $info, \pocketmine\block\utils\WoodType::OAK);
$check('Trapdoor default closed', false, $trapdoor->isOpen());
$trapdoor->setOpen(true);
$check('Trapdoor modified open', true, $trapdoor->isOpen());

// 3. Palancas y Botones
$lever = new Lever($id, 'Lever', $info);
$check('Lever default deactivated', false, $lever->isActivated());
$lever->setActivated(true);
$check('Lever modified activated', true, $lever->isActivated());

$button = new StoneButton($id, 'Stone Button', $info);
$check('Button default not pressed', false, $button->isPressed());

$woodButton = new WoodenButton($id, 'Oak Button', $info, \pocketmine\block\utils\WoodType::OAK);
$check('WoodenButton default not pressed', false, $woodButton->isPressed());

// 4. Contenedores e inventarios
$chest = new Chest($id, 'Chest', $info);
$inv = $chest->getInventory();
$check('Chest getInventory is Inventory', true, $inv instanceof Inventory);

$barrel = new Barrel($id, 'Barrel', $info);
$barrelInv = $barrel->getInventory();
$check('Barrel getInventory is Inventory', true, $barrelInv instanceof Inventory);

$daylight = new DaylightSensor($id, 'Daylight Sensor', $info);
$check('DaylightSensor default not inverted', false, $daylight->isInverted());
$daylight->setInverted(true);
$check('DaylightSensor modified inverted', true, $daylight->isInverted());

if ($failures > 0) {
    fwrite(STDERR, $failures . " failure(s)\n");
    exit(1);
}

echo "ALL OK\n";
