<?php

declare(strict_types=1);

namespace pocketmine\block {
    // Provide a minimal dummy Block class if block.php is not loaded,
    // so we can test VanillaBlocks execution standalone
    if (!class_exists('pocketmine\block\Block', false)) {
        class Block {
            public string $name;
            public static function fromName(string $name): self {
                $b = new self();
                $b->name = $name;
                return $b;
            }
        }
    }
}

namespace {
    require_once __DIR__ . '/generated_core_registry.php';

    use pocketmine\block\VanillaBlocks;
    use pocketmine\block\Block;

    echo "Testing VanillaBlocks execution...\n";

    // 1. Explicit getter methods
    $air = VanillaBlocks::AIR();
    assert($air instanceof Block);
    assert($air->name === 'minecraft:air');

    $stone = VanillaBlocks::STONE();
    assert($stone instanceof Block);
    assert($stone->name === 'minecraft:stone');

    $grass = VanillaBlocks::GRASS();
    assert($grass instanceof Block);
    assert($grass->name === 'minecraft:grass_block');

    // 2. Alias resolution via __callStatic
    $grassBlock = VanillaBlocks::GRASS_BLOCK();
    assert($grassBlock instanceof Block);
    assert($grassBlock->name === 'minecraft:grass_block');

    $noteblock = VanillaBlocks::NOTEBLOCK();
    assert($noteblock instanceof Block);
    assert($noteblock->name === 'minecraft:noteblock');

    $leaves = VanillaBlocks::LEAVES();
    assert($leaves instanceof Block);
    assert($leaves->name === 'minecraft:oak_leaves');

    // 3. Lowercase and snake_case resolution
    $grassLower = VanillaBlocks::grass();
    assert($grassLower instanceof Block);
    assert($grassLower->name === 'minecraft:grass_block');

    $grassSnake = VanillaBlocks::grass_block();
    assert($grassSnake instanceof Block);
    assert($grassSnake->name === 'minecraft:grass_block');

    $grassCamel = VanillaBlocks::grassBlock();
    assert($grassCamel instanceof Block);
    assert($grassCamel->name === 'minecraft:grass_block');

    // 4. getAll()
    $all = VanillaBlocks::getAll();
    echo "getAll() returned " . count($all) . " blocks\n";
    assert(count($all) === 830, "getAll() must return 830 blocks");
    assert(isset($all['AIR']), "Must contain AIR");
    assert(isset($all['STONE']), "Must contain STONE");
    assert(isset($all['GRASS']), "Must contain GRASS");
    assert(isset($all['CINNABAR_BRICK_WALL']), "Must contain CINNABAR_BRICK_WALL");
    foreach ($all as $k => $b) {
        assert($b instanceof Block, "Item $k must be instance of Block");
    }

    echo "VanillaBlocks runtime execution OK! All assertions passed.\n";
}
