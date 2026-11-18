<?php

declare(strict_types=1);

/**
 * Test de fidelidad para block/utils (Ola 6a):
 * - 68 clases, enums y traits de utils
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

use pocketmine\block\utils\SupportType;
use pocketmine\block\utils\DyeColor;
use pocketmine\block\utils\WoodType;
use pocketmine\block\utils\SlabType;
use pocketmine\block\utils\StairShape;
use pocketmine\block\utils\WallConnectionType;
use pocketmine\block\utils\PillarRotation;
use pocketmine\block\utils\LeverFacing;
use pocketmine\block\utils\BellAttachmentType;
use pocketmine\block\utils\CopperOxidation;
use pocketmine\block\utils\Colored;
use pocketmine\block\utils\ColoredTrait;
use pocketmine\block\utils\AnyFacing;
use pocketmine\block\utils\AnyFacingTrait;
use pocketmine\block\utils\HorizontalFacing;
use pocketmine\block\utils\HorizontalFacingTrait;
use pocketmine\block\utils\Lightable;
use pocketmine\block\utils\LightableTrait;
use pocketmine\block\utils\PoweredByRedstone;
use pocketmine\block\utils\PoweredByRedstoneTrait;
use pocketmine\block\utils\CopperMaterial;
use pocketmine\block\utils\CopperTrait;

// 1. Enums
$check('SupportType cases count', 4, count(SupportType::cases()));
$check('DyeColor cases count', 16, count(DyeColor::cases()));
$check('DyeColor::WHITE displayName', 'White', DyeColor::WHITE->getDisplayName());
$check('DyeColor::RED rgb', [0xb0, 0x2e, 0x26], [DyeColor::RED->getRgbValue()->getR(), DyeColor::RED->getRgbValue()->getG(), DyeColor::RED->getRgbValue()->getB()]);

$check('WoodType cases count', 12, count(WoodType::cases()));
$check('WoodType::OAK displayName', 'Oak', WoodType::OAK->getDisplayName());

$check('SlabType cases count', 3, count(SlabType::cases()));
$check('StairShape cases count', 5, count(StairShape::cases()));
$check('WallConnectionType cases count', 2, count(WallConnectionType::cases()));
$check('CopperOxidation cases count', 4, count(CopperOxidation::cases()));

// 2. Interfaces
$check('Colored interface exists', true, interface_exists(Colored::class));
$check('AnyFacing interface exists', true, interface_exists(AnyFacing::class));
$check('HorizontalFacing interface exists', true, interface_exists(HorizontalFacing::class));
$check('Lightable interface exists', true, interface_exists(Lightable::class));
$check('PoweredByRedstone interface exists', true, interface_exists(PoweredByRedstone::class));
$check('CopperMaterial interface exists', true, interface_exists(CopperMaterial::class));
$check('PillarRotation interface exists', true, interface_exists(PillarRotation::class));

// 3. Traits existence
$check('ColoredTrait exists', true, trait_exists(ColoredTrait::class));
$check('AnyFacingTrait exists', true, trait_exists(AnyFacingTrait::class));
$check('HorizontalFacingTrait exists', true, trait_exists(HorizontalFacingTrait::class));
$check('LightableTrait exists', true, trait_exists(LightableTrait::class));
$check('PoweredByRedstoneTrait exists', true, trait_exists(PoweredByRedstoneTrait::class));
$check('CopperTrait exists', true, trait_exists(CopperTrait::class));

if ($failures > 0) {
    fwrite(STDERR, $failures . " failure(s)\n");
    exit(1);
}

echo "ALL OK\n";
