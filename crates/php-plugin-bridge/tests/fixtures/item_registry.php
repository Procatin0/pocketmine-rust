<?php

declare(strict_types=1);

/**
 * Fixture de prueba para Wave 5: VanillaItems (325 getters) e ItemFactory dispatch.
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

use pocketmine\item\Apple;
use pocketmine\item\Armor;
use pocketmine\item\Durable;
use pocketmine\item\Food;
use pocketmine\item\Item;
use pocketmine\item\ItemBlock;
use pocketmine\item\ItemFactory;
use pocketmine\item\ItemTypeIds;
use pocketmine\item\Sword;
use pocketmine\item\TieredTool;
use pocketmine\item\Tool;
use pocketmine\item\VanillaItems;

// =========================================================================
// 1. VERIFICACION DE LOS 325 GETTERS DE VanillaItems
// =========================================================================
$ref = new \ReflectionClass(VanillaItems::class);
$methods = $ref->getMethods(\ReflectionMethod::IS_STATIC | \ReflectionMethod::IS_PUBLIC);
$getterCount = 0;
$allClonedFresh = true;
$allValidNetworkId = true;
$airIsNull = false;

foreach ($methods as $method) {
    if (!$method->isPublic() || !$method->isStatic()) {
        continue;
    }
    $name = $method->getName();
    if ($name === 'getAll' || str_starts_with($name, '__')) {
        continue;
    }
    $getterCount++;
    /** @var Item $item1 */
    $item1 = VanillaItems::$name();
    /** @var Item $item2 */
    $item2 = VanillaItems::$name();

    if (!$item1 instanceof Item) {
        $failures++;
        fwrite(STDERR, "FAIL: VanillaItems::$name() did not return Item\n");
    }

    if ($item1 === $item2) {
        $allClonedFresh = false;
    }

    if ($name === 'AIR') {
        $airIsNull = $item1->isNull();
    } else {
        if ($item1->getId() <= 0) {
            $allValidNetworkId = false;
            fwrite(STDERR, "FAIL: VanillaItems::$name() has invalid network ID: " . $item1->getId() . "\n");
        }
    }
}

$check("VanillaItems getters count", 324, $getterCount);
$check("VanillaItems::getAll count", 324, count(VanillaItems::getAll()));
$check("All VanillaItems getters return cloned fresh instances", true, $allClonedFresh);
$check("VanillaItems::AIR isNull", true, $airIsNull);
$check("All non-air VanillaItems resolve valid Bedrock network ID", true, $allValidNetworkId);

// =========================================================================
// 2. DISPATCH EN ItemFactory::get()
// =========================================================================
$apple = ItemFactory::get("apple");
$check("ItemFactory::get('apple') returns Apple", true, $apple instanceof Apple);
$check("Apple typeId is APPLE", ItemTypeIds::APPLE, $apple->getTypeId());

$sword = ItemFactory::get("diamond_sword");
$check("ItemFactory::get('diamond_sword') returns Sword", true, $sword instanceof Sword);
$check("Sword instanceof TieredTool", true, $sword instanceof TieredTool);

$stone = ItemFactory::get("stone");
$check("ItemFactory::get('stone') returns ItemBlock", true, $stone instanceof ItemBlock);
$check("Stone network ID is 1", 1, $stone->getId());

$fromNetworkApple = ItemFactory::get(285);
$check("ItemFactory::get(285) returns Apple", true, $fromNetworkApple instanceof Apple);

$fromNetworkStone = ItemFactory::get(1);
$check("ItemFactory::get(1) returns ItemBlock", true, $fromNetworkStone instanceof ItemBlock);

// =========================================================================
// 3. RESOLUCION DE NETWORK ID
// =========================================================================
$appleNetId = ItemFactory::resolveNetworkId(ItemTypeIds::APPLE);
$check("resolveNetworkId(APPLE)", 285, $appleNetId);

$swordNetId = ItemFactory::resolveNetworkId(ItemTypeIds::DIAMOND_SWORD);
$check("resolveNetworkId(DIAMOND_SWORD)", 347, $swordNetId);

if ($failures === 0) {
    echo "ALL OK: item registry and factory dispatch verified\n";
    exit(0);
}

fwrite(STDERR, "Encountered $failures failure(s) in item registry tests\n");
exit(1);
