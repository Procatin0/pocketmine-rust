<?php

declare(strict_types=1);

/**
 * Test de fidelidad para block entities / tiles (Ola 5):
 * - Clases base: Tile, Spawnable, Container, Nameable, TileFactory
 * - 39 clases de block/tile/
 * - NBT serialization / deserialization roundtrip
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

use pocketmine\block\tile\Tile;
use pocketmine\block\tile\Spawnable;
use pocketmine\block\tile\Container;
use pocketmine\block\tile\Nameable;
use pocketmine\block\tile\TileFactory;
use pocketmine\block\tile\Chest;
use pocketmine\block\tile\EnderChest;
use pocketmine\block\tile\Barrel;
use pocketmine\block\tile\Furnace;
use pocketmine\block\tile\BlastFurnace;
use pocketmine\block\tile\Smoker;
use pocketmine\block\tile\Hopper;
use pocketmine\block\tile\BrewingStand;
use pocketmine\block\tile\Campfire;
use pocketmine\block\tile\Bed;
use pocketmine\block\tile\Sign;
use pocketmine\block\tile\Banner;
use pocketmine\block\tile\Bell;
use pocketmine\block\tile\ShulkerBox;
use pocketmine\block\tile\Lectern;
use pocketmine\block\tile\DaylightSensor;
use pocketmine\block\tile\Note;
use pocketmine\block\tile\Jukebox;
use pocketmine\nbt\tag\CompoundTag;
use pocketmine\world\World;
use pocketmine\math\Vector3;

// 1. Jerarquías
$check('Spawnable is Tile', true, is_subclass_of(Spawnable::class, Tile::class));
$check('Chest is Spawnable', true, is_subclass_of(Chest::class, Spawnable::class));
$check('Chest implements Container', true, is_subclass_of(Chest::class, Container::class));
$check('Chest implements Nameable', true, is_subclass_of(Chest::class, Nameable::class));
$check('Barrel is Spawnable', true, is_subclass_of(Barrel::class, Spawnable::class));
$check('Barrel implements Container', true, is_subclass_of(Barrel::class, Container::class));
$check('Furnace is Spawnable', true, is_subclass_of(Furnace::class, Spawnable::class));
$check('BlastFurnace is Furnace', true, is_subclass_of(BlastFurnace::class, Furnace::class));
$check('Smoker is Furnace', true, is_subclass_of(Smoker::class, Furnace::class));
$check('Hopper is Spawnable', true, is_subclass_of(Hopper::class, Spawnable::class));
$check('BrewingStand is Spawnable', true, is_subclass_of(BrewingStand::class, Spawnable::class));
$check('Sign is Spawnable', true, is_subclass_of(Sign::class, Spawnable::class));
$check('Banner is Spawnable', true, is_subclass_of(Banner::class, Spawnable::class));
$check('Bed is Spawnable', true, is_subclass_of(Bed::class, Spawnable::class));
$check('ShulkerBox is Spawnable', true, is_subclass_of(ShulkerBox::class, Spawnable::class));

// 2. TileFactory
$factory = TileFactory::getInstance();
$check('TileFactory is TileFactory', true, $factory instanceof TileFactory);

// 3. Creación y NBT roundtrip
$world = new World('test_world', null, 0);
$nbt = CompoundTag::create()
    ->setString('id', 'Chest')
    ->setInt('x', 10)
    ->setInt('y', 64)
    ->setInt('z', -20)
    ->setString('CustomName', 'Test Chest');

$tile = $factory->createFromData($world, $nbt);
$check('Tile created is Chest', true, $tile instanceof Chest);
$check('Tile CustomName', 'Test Chest', $tile->getName());
$check('Tile pos x', 10, $tile->getPosition()->getFloorX());
$check('Tile pos y', 64, $tile->getPosition()->getFloorY());
$check('Tile pos z', -20, $tile->getPosition()->getFloorZ());

$saved = $tile->saveNBT();
$check('Saved NBT id', 'Chest', $saved->getString('id'));
$check('Saved NBT CustomName', 'Test Chest', $saved->getString('CustomName'));
$check('Saved NBT x', 10, $saved->getInt('x'));

if ($failures > 0) {
    fwrite(STDERR, $failures . " failure(s)\n");
    exit(1);
}

echo "ALL OK\n";
