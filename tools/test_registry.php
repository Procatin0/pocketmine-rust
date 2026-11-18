<?php

declare(strict_types=1);

// Load the generated registry file
require_once __DIR__ . '/generated_core_registry.php';

use pocketmine\block\BlockTypeTags;
use pocketmine\block\BlockToolType;
use pocketmine\block\BlockTypeIds;
use pocketmine\block\VanillaBlocks;

echo "--- 1. Testing BlockTypeTags ---\n";
assert(BlockTypeTags::DIRT === "pocketmine:dirt");
assert(BlockTypeTags::MUD === "pocketmine:mud");
assert(BlockTypeTags::SAND === "pocketmine:sand");
assert(BlockTypeTags::POTTABLE_PLANTS === "pocketmine:pottable");
assert(BlockTypeTags::FIRE === "pocketmine:fire");
assert(BlockTypeTags::HANGING_SIGN === "pocketmine:hanging_sign");
assert(BlockTypeTags::NYLIUM === "pocketmine:nylium");
assert(BlockTypeTags::HUGE_FUNGUS_REPLACEABLE === "pocketmine:huge_fungus_replaceable");
assert(BlockTypeTags::BAMBOO_MOSAIC === "pocketmine:bamboo_mosaic");
echo "BlockTypeTags OK (all 9 tags verified)\n";

echo "--- 2. Testing BlockToolType ---\n";
assert(BlockToolType::NONE === 0);
assert(BlockToolType::SWORD === 1 << 0);
assert(BlockToolType::SHOVEL === 1 << 1);
assert(BlockToolType::PICKAXE === 1 << 2);
assert(BlockToolType::AXE === 1 << 3);
assert(BlockToolType::SHEARS === 1 << 4);
assert(BlockToolType::HOE === 1 << 5);
echo "BlockToolType OK (all 7 bitflags verified)\n";

echo "--- 3. Testing BlockTypeIds ---\n";
$ref = new ReflectionClass(BlockTypeIds::class);
$constants = $ref->getConstants();
echo "Total constants in BlockTypeIds: " . count($constants) . "\n";
assert(count($constants) === 833, "Expected 833 constants, got " . count($constants));
assert(BlockTypeIds::AIR === 10000, "AIR must be 10000");
assert(BlockTypeIds::ACACIA_BUTTON === 10001, "ACACIA_BUTTON must be 10001");
assert(BlockTypeIds::CINNABAR_BRICK_WALL === 10857, "CINNABAR_BRICK_WALL must be 10857");
assert(BlockTypeIds::FIRST_UNUSED_BLOCK_ID === 10858, "FIRST_UNUSED_BLOCK_ID must be 10858");

// Test newId()
$id1 = BlockTypeIds::newId();
$id2 = BlockTypeIds::newId();
assert($id1 === 10858, "newId first call must be 10858");
assert($id2 === 10859, "newId second call must be 10859");

// Test forName()
assert(BlockTypeIds::forName('air') === BlockTypeIds::AIR);
assert(BlockTypeIds::forName('minecraft:air') === BlockTypeIds::AIR);
assert(BlockTypeIds::forName('STONE') === BlockTypeIds::STONE);
assert(BlockTypeIds::forName('minecraft:stone') === BlockTypeIds::STONE);
assert(BlockTypeIds::forName('grass_block') === BlockTypeIds::GRASS);
assert(BlockTypeIds::forName('grass') === BlockTypeIds::GRASS);
assert(BlockTypeIds::forName('noteblock') === BlockTypeIds::NOTE_BLOCK);
assert(BlockTypeIds::forName('gold_block') === BlockTypeIds::GOLD);
assert(BlockTypeIds::forName('leaves') === BlockTypeIds::OAK_LEAVES);

$dynamicId = BlockTypeIds::forName('custom_mod:unknown_block');
assert($dynamicId >= 10860, "Dynamic ID must be >= 10860");
assert(BlockTypeIds::forName('custom_mod:unknown_block') === $dynamicId, "Cached dynamic ID must match");
echo "BlockTypeIds OK\n";

echo "--- 4. Testing VanillaBlocks structure ---\n";
$vbRef = new ReflectionClass(VanillaBlocks::class);
$methods = $vbRef->getMethods(ReflectionMethod::IS_STATIC | ReflectionMethod::IS_PUBLIC);
echo "Total public static methods in VanillaBlocks: " . count($methods) . "\n";
// 830 getters + get + __callStatic + getAll = 833 methods
assert(count($methods) >= 832, "Expected at least 832 public static methods");

// Test that key getter methods exist
assert($vbRef->hasMethod('AIR'), "Must have AIR()");
assert($vbRef->hasMethod('STONE'), "Must have STONE()");
assert($vbRef->hasMethod('GRASS'), "Must have GRASS()");
assert($vbRef->hasMethod('GOLD'), "Must have GOLD()");
assert($vbRef->hasMethod('NOTE_BLOCK'), "Must have NOTE_BLOCK()");
assert($vbRef->hasMethod('WHITE_TULIP'), "Must have WHITE_TULIP()");
assert($vbRef->hasMethod('CINNABAR_BRICK_WALL'), "Must have CINNABAR_BRICK_WALL()");
assert($vbRef->hasMethod('getAll'), "Must have getAll()");
assert($vbRef->hasMethod('get'), "Must have get()");
assert($vbRef->hasMethod('__callStatic'), "Must have __callStatic()");

echo "VanillaBlocks method reflection OK\n";
echo "\nALL TESTS PASSED SUCCESSFULLY!\n";
