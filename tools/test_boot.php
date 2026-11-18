<?php
declare(strict_types=1);

$runtimeDir = __DIR__ . '/../crates/php-plugin-bridge/php';
$files = require $runtimeDir . '/runtime_files.php';

echo "Testing require of " . count($files) . " runtime files...\n";

foreach ($files as $file) {
    // block_state_version.php etc are in build out_dir, let's see if they exist or skip
    $path = $runtimeDir . '/' . $file;
    if (file_exists($path)) {
        require_once $path;
    } else {
        echo "Note: generated file $file not in php/, skipped in standalone test.\n";
    }
}

echo "All existing runtime files loaded successfully!\n";

// Test basic class checks
$checks = [
    'pocketmine\block\Block',
    'pocketmine\block\BlockBreakInfo',
    'pocketmine\block\BlockIdentifier',
    'pocketmine\block\BlockTypeInfo',
    'pocketmine\block\BlockTypeIds',
    'pocketmine\block\BlockTypeTags',
    'pocketmine\block\BlockToolType',
    'pocketmine\block\VanillaBlocks',
    'pocketmine\block\Opaque',
    'pocketmine\block\Transparent',
    'pocketmine\block\utils\SupportType',
    'pocketmine\block\utils\DyeColor',
    'pocketmine\block\utils\WoodType',
    'pocketmine\block\inventory\BlockInventory',
    'pocketmine\block\inventory\ChestInventory',
];

foreach ($checks as $cls) {
    if (class_exists($cls) || interface_exists($cls) || enum_exists($cls) || trait_exists($cls)) {
        echo "OK: $cls exists\n";
    } else {
        echo "FAIL: $cls does NOT exist!\n";
    }
}
