<?php

declare(strict_types=1);

/**
 * Fixture de prueba para Wave 5: jerarquía completa de items y encantamientos.
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

use pocketmine\block\VanillaBlocks;
use pocketmine\entity\FoodSource;
use pocketmine\item\Apple;
use pocketmine\item\Armor;
use pocketmine\item\ArmorTypeInfo;
use pocketmine\item\Arrow;
use pocketmine\item\Axe;
use pocketmine\item\Boat;
use pocketmine\item\Durable;
use pocketmine\item\EnderPearl;
use pocketmine\item\Food;
use pocketmine\item\Item;
use pocketmine\item\ItemBlock;
use pocketmine\item\Pickaxe;
use pocketmine\item\ProjectileItem;
use pocketmine\item\Releasable;
use pocketmine\item\Snowball;
use pocketmine\item\StringToItemParser;
use pocketmine\item\Sword;
use pocketmine\item\TieredTool;
use pocketmine\item\Tool;
use pocketmine\item\ToolTier;
use pocketmine\item\TurtleHelmet;
use pocketmine\item\VanillaArmorMaterials;
use pocketmine\item\VanillaItems;
use pocketmine\item\WritableBook;
use pocketmine\item\WrittenBook;
use pocketmine\item\enchantment\EnchantmentInstance;
use pocketmine\item\enchantment\VanillaEnchantments;

// =========================================================================
// 1. HERRAMIENTAS (Tools & TieredTools)
// =========================================================================
$diamondSword = VanillaItems::DIAMOND_SWORD();
$check("DiamondSword instanceof Sword", true, $diamondSword instanceof Sword);
$check("DiamondSword instanceof TieredTool", true, $diamondSword instanceof TieredTool);
$check("DiamondSword instanceof Tool", true, $diamondSword instanceof Tool);
$check("DiamondSword instanceof Durable", true, $diamondSword instanceof Durable);
$check("DiamondSword instanceof Item", true, $diamondSword instanceof Item);
$check("DiamondSword tier is DIAMOND", ToolTier::DIAMOND, $diamondSword->getTier());
$check("DiamondSword attack points", 8, $diamondSword->getAttackPoints());
$check("DiamondSword max durability", 1562, $diamondSword->getMaxDurability());

$ironPick = VanillaItems::IRON_PICKAXE();
$check("IronPickaxe instanceof Pickaxe", true, $ironPick instanceof Pickaxe);
$check("IronPickaxe tier is IRON", ToolTier::IRON, $ironPick->getTier());
$check("IronPickaxe max durability", 251, $ironPick->getMaxDurability());

$goldenAxe = VanillaItems::GOLDEN_AXE();
$check("GoldenAxe instanceof Axe", true, $goldenAxe instanceof Axe);
$check("GoldenAxe tier is GOLD", ToolTier::GOLD, $goldenAxe->getTier());

$trident = VanillaItems::TRIDENT();
$check("Trident instanceof Releasable", true, $trident instanceof Releasable);
$check("Trident max durability", 251, $trident->getMaxDurability());

// =========================================================================
// 2. COMIDA (Food & Consumables)
// =========================================================================
$apple = VanillaItems::APPLE();
$check("Apple instanceof Food", true, $apple instanceof Food);
$check("Apple implements FoodSource", true, $apple instanceof FoodSource);
$check("Apple food restore", 4, $apple->getFoodRestore());
$check("Apple saturation restore", 2.4, round($apple->getSaturationRestore(), 1));
$check("Apple requires hunger", true, $apple->requiresHunger());

$goldenApple = VanillaItems::GOLDEN_APPLE();
$check("GoldenApple requires hunger", false, $goldenApple->requiresHunger());

$steak = VanillaItems::STEAK();
$check("Steak food restore", 8, $steak->getFoodRestore());
$check("Steak saturation restore", 12.8, round($steak->getSaturationRestore(), 1));

// =========================================================================
// 3. ARMADURAS (Armor & Materials)
// =========================================================================
$diamondChest = VanillaItems::DIAMOND_CHESTPLATE();
$check("DiamondChestplate instanceof Armor", true, $diamondChest instanceof Armor);
$check("DiamondChestplate slot CHEST", ArmorTypeInfo::SLOT_CHEST, $diamondChest->getArmorSlot());
$check("DiamondChestplate defense points", 8, $diamondChest->getDefensePoints());
$check("DiamondChestplate material", VanillaArmorMaterials::DIAMOND(), $diamondChest->getMaterial());
$check("DiamondChestplate max durability", 529, $diamondChest->getMaxDurability());

$turtleHelmet = VanillaItems::TURTLE_HELMET();
$check("TurtleHelmet instanceof TurtleHelmet", true, $turtleHelmet instanceof TurtleHelmet);
$check("TurtleHelmet instanceof Armor", true, $turtleHelmet instanceof Armor);
$check("TurtleHelmet slot HELMET", ArmorTypeInfo::SLOT_HEAD, $turtleHelmet->getArmorSlot());

// =========================================================================
// 4. PROYECTILES
// =========================================================================
$arrow = VanillaItems::ARROW();
$check("Arrow instanceof Item", true, $arrow instanceof Item);

$snowball = VanillaItems::SNOWBALL();
$check("Snowball instanceof ProjectileItem", true, $snowball instanceof ProjectileItem);
$check("Snowball throw force", 1.5, $snowball->getThrowForce());
$check("Snowball max stack", 16, $snowball->getMaxStackSize());

$pearl = VanillaItems::ENDER_PEARL();
$check("EnderPearl instanceof ProjectileItem", true, $pearl instanceof ProjectileItem);
$check("EnderPearl throw force", 1.5, $pearl->getThrowForce());
$check("EnderPearl max stack", 16, $pearl->getMaxStackSize());

// =========================================================================
// 5. ITEM BLOCK & PRESERVACION DE ESTADO
// =========================================================================
$stoneBlock = VanillaBlocks::STONE();
$stoneItem = $stoneBlock->asItem();
$check("STONE asItem instanceof ItemBlock", true, $stoneItem instanceof ItemBlock);
$check("STONE asItem getBlock hasSameTypeId STONE", true, $stoneItem->getBlock()->hasSameTypeId($stoneBlock));

$redWoolItem = VanillaBlocks::RED_WOOL()->asItem();
$whiteWoolItem = VanillaBlocks::WHITE_WOOL()->asItem();
$check("Wool items are ItemBlock", true, $redWoolItem instanceof ItemBlock && $whiteWoolItem instanceof ItemBlock);
$check("Different wool colors have different state IDs", true, $redWoolItem->getStateId() !== $whiteWoolItem->getStateId());

// =========================================================================
// 6. LIBROS Y OTROS
// =========================================================================
$writableBook = VanillaItems::WRITABLE_BOOK();
$check("WritableBook instanceof WritableBook", true, $writableBook instanceof WritableBook);
$check("WritableBook max stack", 1, $writableBook->getMaxStackSize());

$writtenBook = VanillaItems::WRITTEN_BOOK();
$check("WrittenBook instanceof WrittenBook", true, $writtenBook instanceof WrittenBook);

$acaciaBoat = VanillaItems::ACACIA_BOAT();
$check("AcaciaBoat instanceof Boat", true, $acaciaBoat instanceof Boat);

// =========================================================================
// 7. ENCANTAMIENTOS
// =========================================================================
$sharpness = VanillaEnchantments::SHARPNESS();
$protection = VanillaEnchantments::PROTECTION();
$silkTouch = VanillaEnchantments::SILK_TOUCH();
$efficiency = VanillaEnchantments::EFFICIENCY();
$unbreaking = VanillaEnchantments::UNBREAKING();

$check("Sharpness max level", 5, $sharpness->getMaxLevel());
$check("Protection max level", 4, $protection->getMaxLevel());
$check("SilkTouch max level", 1, $silkTouch->getMaxLevel());

$inst = new EnchantmentInstance($sharpness, 3);
$check("EnchantmentInstance level", 3, $inst->getLevel());
$check("EnchantmentInstance type", $sharpness, $inst->getType());

$swordWithEnchant = clone $diamondSword;
$swordWithEnchant->addEnchantment($inst);
$check("Item has enchantments", true, $swordWithEnchant->hasEnchantments());
$check("Item has sharpness enchantment", true, $swordWithEnchant->hasEnchantment($sharpness));
$check("Item enchantment level", 3, $swordWithEnchant->getEnchantmentLevel($sharpness));

// =========================================================================
// 8. PARSER (StringToItemParser)
// =========================================================================
$parser = StringToItemParser::getInstance();
$parsedSword = $parser->parse("diamond_sword");
$check("Parser resolves diamond_sword to Sword", true, $parsedSword instanceof Sword);
$parsedApple = $parser->parse("apple");
$check("Parser resolves apple to Apple", true, $parsedApple instanceof Apple);
$parsedStone = $parser->parse("stone");
$check("Parser resolves stone to ItemBlock", true, $parsedStone instanceof ItemBlock);

if ($failures === 0) {
    echo "ALL OK: item hierarchy and enchantments fidelity verified\n";
    exit(0);
}

fwrite(STDERR, "Encountered $failures failure(s) in item hierarchy tests\n");
exit(1);
