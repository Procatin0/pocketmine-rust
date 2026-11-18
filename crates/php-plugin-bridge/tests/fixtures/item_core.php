<?php

declare(strict_types=1);

/**
 * Fixture de prueba para Wave 0: pocketmine\item core.
 */

$dir = $argv[1] ?? '';
if ($dir === '' || !is_file($dir . '/runtime_files.php')) {
    fwrite(STDERR, "missing PHP runtime directory argument\n");
    exit(2);
}
foreach (require $dir . '/runtime_files.php' as $file) {
    require_once $dir . '/' . $file;
}

$failures = 0;
$check = function (string $what, $expected, $actual) use (&$failures): void {
    if ($expected !== $actual) {
        $failures++;
        fwrite(STDERR, 'FAIL ' . $what . ': expected ' . var_export($expected, true) . ', got ' . var_export($actual, true) . "\n");
    }
};

use pocketmine\block\BlockTypeIds;
use pocketmine\block\VanillaBlocks;
use pocketmine\item\Durable;
use pocketmine\item\Item;
use pocketmine\item\ItemBlock;
use pocketmine\item\ItemIdentifier;
use pocketmine\item\ItemTypeIds;
use pocketmine\item\ItemUseResult;
use pocketmine\item\Releasable;
use pocketmine\item\ToolTier;
use pocketmine\nbt\tag\CompoundTag;
use pocketmine\nbt\tag\StringTag;

// 1. Verificación de métodos públicos en Item
$ref = new \ReflectionClass(Item::class);
$publicMethods = [];
foreach ($ref->getMethods(\ReflectionMethod::IS_PUBLIC) as $m) {
    $publicMethods[] = $m->getName();
}
$check("Item has getTypeId", true, in_array('getTypeId', $publicMethods, true));
$check("Item has getStateId", true, in_array('getStateId', $publicMethods, true));
$check("Item has pop", true, in_array('pop', $publicMethods, true));
$check("Item has equalsExact", true, in_array('equalsExact', $publicMethods, true));
$check("Item has canStackWith", true, in_array('canStackWith', $publicMethods, true));

// 2. Identidad y constantes
$check("FIRST_UNUSED_ITEM_ID is 20326", 20326, ItemTypeIds::FIRST_UNUSED_ITEM_ID);
$newId = ItemTypeIds::newId();
$check("newId >= 20326", true, $newId >= 20326);
$check("fromBlockTypeId(10496) is -10496", -10496, ItemTypeIds::fromBlockTypeId(10496));
$check("toBlockTypeId(-10496) is 10496", 10496, ItemTypeIds::toBlockTypeId(-10496));
$check("toBlockTypeId(20002) is null", null, ItemTypeIds::toBlockTypeId(20002));

// 3. ItemIdentifier
$ident = new ItemIdentifier(20002);
$check("ItemIdentifier::getTypeId", 20002, $ident->getTypeId());
$check("ItemIdentifier::getId (bridge compat)", 20002, $ident->getId());

$stoneBlock = VanillaBlocks::STONE();
$blockIdent = ItemIdentifier::fromBlock($stoneBlock);
$check("ItemIdentifier::fromBlock getTypeId is negative", -BlockTypeIds::STONE, $blockIdent->getTypeId());

// 4. Item behavior: CustomName, Lore, Count, Pop
$item = new Item($ident, "Test Item");
$check("Default customName is empty", "", $item->getCustomName());
$check("Default hasCustomName is false", false, $item->hasCustomName());
$check("Default getName is vanilla name", "Test Item", $item->getName());

$item->setCustomName("Custom Diamond");
$check("CustomName set", "Custom Diamond", $item->getCustomName());
$check("hasCustomName true", true, $item->hasCustomName());
$check("getName returns custom", "Custom Diamond", $item->getName());

$item->setLore(["Line 1", "Line 2"]);
$check("Lore returns array", ["Line 1", "Line 2"], $item->getLore());

$item->setCount(10);
$check("Count is 10", 10, $item->getCount());
$popped = $item->pop(3);
$check("Popped count is 3", 3, $popped->getCount());
$check("Remaining count is 7", 7, $item->getCount());
$check("Popped preserves custom name", "Custom Diamond", $popped->getCustomName());

// 5. Equals vs EqualsExact
$itemA = new Item(new ItemIdentifier(20002), "Apple");
$itemA->setCount(1);
$itemB = new Item(new ItemIdentifier(20002), "Apple");
$itemB->setCount(5);

$check("equals ignores count", true, $itemA->equals($itemB));
$check("canStackWith ignores count", true, $itemA->canStackWith($itemB));
$check("equalsExact considers count", false, $itemA->equalsExact($itemB));
$itemB->setCount(1);
$check("equalsExact true when counts match", true, $itemA->equalsExact($itemB));

// 6. JSON serialization throws LogicException
$threw = false;
try {
    $itemA->jsonSerialize();
} catch (\LogicException $e) {
    $threw = true;
}
$check("jsonSerialize throws LogicException", true, $threw);

// 7. Clone behavior
$orig = new Item(new ItemIdentifier(20002), "Apple");
$orig->getNamedTag()->setString("foo", "bar");
$cloned = clone $orig;
$cloned->getNamedTag()->setString("foo", "baz");
$check("Clone deep-copies NBT", "bar", $orig->getNamedTag()->getString("foo"));
$check("Cloned NBT modified", "baz", $cloned->getNamedTag()->getString("foo"));

// 8. ItemBlock
$stone = VanillaBlocks::STONE();
$itemBlock = new ItemBlock($stone);
$check("ItemBlock instanceof Item", true, $itemBlock instanceof Item);
$check("ItemBlock getTypeId is negative block id", -BlockTypeIds::STONE, $itemBlock->getTypeId());
$check("ItemBlock getBlock returns clone", BlockTypeIds::STONE, $itemBlock->getBlock()->getTypeId());
$check("ItemBlock isNull for Stone is false", false, $itemBlock->isNull());

$air = VanillaBlocks::AIR();
$itemAir = new ItemBlock($air);
$check("ItemBlock isNull for Air is true", true, $itemAir->isNull());

// Block::asItem() returns ItemBlock
$asItem = $stone->asItem();
$check("stone->asItem() instanceof ItemBlock", true, $asItem instanceof ItemBlock);
$check("stone->asItem()->getBlock()->getTypeId() matches stone", BlockTypeIds::STONE, $asItem->getBlock()->getTypeId());

// 9. Durable behavior
$mockDurable = new class(new ItemIdentifier(20086), "Diamond Axe") extends Durable {
    public function getMaxDurability(): int { return 1562; }
};
$check("Durable instanceof Item", true, $mockDurable instanceof Item);
$check("Durable initial damage 0", 0, $mockDurable->getDamage());
$check("Durable initial durability max", 1562, $mockDurable->getDurability());
$check("Durable isDamaged false", false, $mockDurable->isDamaged());

$mockDurable->applyDamage(50);
$check("Durable after applyDamage damage 50", 50, $mockDurable->getDamage());
$check("Durable after applyDamage durability 1512", 1512, $mockDurable->getDurability());
$check("Durable isDamaged true", true, $mockDurable->isDamaged());

$mockDurable->repair();
$check("Durable after repair damage 0", 0, $mockDurable->getDamage());

$mockDurable->setUnbreakable(true);
$mockDurable->applyDamage(100);
$check("Unbreakable ignores damage", 0, $mockDurable->getDamage());

// 10. ToolTier enum
$check("ToolTier::DIAMOND getMaxDurability", 1562, ToolTier::DIAMOND->getMaxDurability());
$check("ToolTier::NETHERITE getMaxDurability", 2032, ToolTier::NETHERITE->getMaxDurability());
$check("ToolTier::WOOD getMaxDurability", 60, ToolTier::WOOD->getMaxDurability());

// 11. State ID with Morton 2D
$stateId1 = $itemA->getStateId();
$stateId2 = $itemB->getStateId();
$check("Same item type produces same stateId", true, $stateId1 === $stateId2);
$check("Different item type produces different stateId", false, $stateId1 === $itemBlock->getStateId());

if ($failures === 0) {
    echo "ALL OK\n";
    exit(0);
} else {
    echo "FAILED with $failures error(s)\n";
    exit(1);
}
