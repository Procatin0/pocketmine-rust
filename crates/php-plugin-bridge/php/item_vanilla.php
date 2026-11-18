<?php

declare(strict_types=1);

namespace pocketmine\item;

/**
 * Registro vanilla de items de PM5.
 * Generado automaticamente desde target/axolotl-pm/generated/item/VanillaItems.php.
 */
final class VanillaItems
{
    /** @var array<string, Item> */
    private static ?array $members = null;

    private function __construct() {}

    private static function init(): void
    {
        if (self::$members !== null) return;
        self::$members = [];
        $inputs = new VanillaItemsInputs();
        foreach ($inputs->getAll() as $name => $item) {
            self::$members[mb_strtoupper($name)] = $item;
        }
    }

    public static function getAll(): array
    {
        self::init();
        $cloned = [];
        foreach (self::$members as $name => $item) {
            $cloned[$name] = clone $item;
        }
        return $cloned;
    }

    public static function ACACIA_BOAT(): Boat
    {
        self::init();
        if (!isset(self::$members['ACACIA_BOAT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item ACACIA_BOAT");
        }
        return clone self::$members['ACACIA_BOAT'];
    }

    public static function ACACIA_HANGING_SIGN(): HangingSign
    {
        self::init();
        if (!isset(self::$members['ACACIA_HANGING_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item ACACIA_HANGING_SIGN");
        }
        return clone self::$members['ACACIA_HANGING_SIGN'];
    }

    public static function ACACIA_SIGN(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['ACACIA_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item ACACIA_SIGN");
        }
        return clone self::$members['ACACIA_SIGN'];
    }

    public static function AIR(): Item
    {
        self::init();
        if (!isset(self::$members['AIR'])) {
            throw new \InvalidArgumentException("Unknown vanilla item AIR");
        }
        return clone self::$members['AIR'];
    }

    public static function AMETHYST_SHARD(): Item
    {
        self::init();
        if (!isset(self::$members['AMETHYST_SHARD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item AMETHYST_SHARD");
        }
        return clone self::$members['AMETHYST_SHARD'];
    }

    public static function APPLE(): Apple
    {
        self::init();
        if (!isset(self::$members['APPLE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item APPLE");
        }
        return clone self::$members['APPLE'];
    }

    public static function ARROW(): Arrow
    {
        self::init();
        if (!isset(self::$members['ARROW'])) {
            throw new \InvalidArgumentException("Unknown vanilla item ARROW");
        }
        return clone self::$members['ARROW'];
    }

    public static function BAKED_POTATO(): BakedPotato
    {
        self::init();
        if (!isset(self::$members['BAKED_POTATO'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BAKED_POTATO");
        }
        return clone self::$members['BAKED_POTATO'];
    }

    public static function BAMBOO(): Bamboo
    {
        self::init();
        if (!isset(self::$members['BAMBOO'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BAMBOO");
        }
        return clone self::$members['BAMBOO'];
    }

    public static function BAMBOO_HANGING_SIGN(): HangingSign
    {
        self::init();
        if (!isset(self::$members['BAMBOO_HANGING_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BAMBOO_HANGING_SIGN");
        }
        return clone self::$members['BAMBOO_HANGING_SIGN'];
    }

    public static function BAMBOO_SIGN(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['BAMBOO_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BAMBOO_SIGN");
        }
        return clone self::$members['BAMBOO_SIGN'];
    }

    public static function BANNER(): Banner
    {
        self::init();
        if (!isset(self::$members['BANNER'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BANNER");
        }
        return clone self::$members['BANNER'];
    }

    public static function BEETROOT(): Beetroot
    {
        self::init();
        if (!isset(self::$members['BEETROOT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BEETROOT");
        }
        return clone self::$members['BEETROOT'];
    }

    public static function BEETROOT_SEEDS(): BeetrootSeeds
    {
        self::init();
        if (!isset(self::$members['BEETROOT_SEEDS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BEETROOT_SEEDS");
        }
        return clone self::$members['BEETROOT_SEEDS'];
    }

    public static function BEETROOT_SOUP(): BeetrootSoup
    {
        self::init();
        if (!isset(self::$members['BEETROOT_SOUP'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BEETROOT_SOUP");
        }
        return clone self::$members['BEETROOT_SOUP'];
    }

    public static function BIRCH_BOAT(): Boat
    {
        self::init();
        if (!isset(self::$members['BIRCH_BOAT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BIRCH_BOAT");
        }
        return clone self::$members['BIRCH_BOAT'];
    }

    public static function BIRCH_HANGING_SIGN(): HangingSign
    {
        self::init();
        if (!isset(self::$members['BIRCH_HANGING_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BIRCH_HANGING_SIGN");
        }
        return clone self::$members['BIRCH_HANGING_SIGN'];
    }

    public static function BIRCH_SIGN(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['BIRCH_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BIRCH_SIGN");
        }
        return clone self::$members['BIRCH_SIGN'];
    }

    public static function BLAZE_POWDER(): Item
    {
        self::init();
        if (!isset(self::$members['BLAZE_POWDER'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BLAZE_POWDER");
        }
        return clone self::$members['BLAZE_POWDER'];
    }

    public static function BLAZE_ROD(): BlazeRod
    {
        self::init();
        if (!isset(self::$members['BLAZE_ROD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BLAZE_ROD");
        }
        return clone self::$members['BLAZE_ROD'];
    }

    public static function BLEACH(): Item
    {
        self::init();
        if (!isset(self::$members['BLEACH'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BLEACH");
        }
        return clone self::$members['BLEACH'];
    }

    public static function BONE(): Item
    {
        self::init();
        if (!isset(self::$members['BONE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BONE");
        }
        return clone self::$members['BONE'];
    }

    public static function BONE_MEAL(): Fertilizer
    {
        self::init();
        if (!isset(self::$members['BONE_MEAL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BONE_MEAL");
        }
        return clone self::$members['BONE_MEAL'];
    }

    public static function BOOK(): Book
    {
        self::init();
        if (!isset(self::$members['BOOK'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BOOK");
        }
        return clone self::$members['BOOK'];
    }

    public static function BOW(): Bow
    {
        self::init();
        if (!isset(self::$members['BOW'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BOW");
        }
        return clone self::$members['BOW'];
    }

    public static function BOWL(): Bowl
    {
        self::init();
        if (!isset(self::$members['BOWL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BOWL");
        }
        return clone self::$members['BOWL'];
    }

    public static function BREAD(): Bread
    {
        self::init();
        if (!isset(self::$members['BREAD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BREAD");
        }
        return clone self::$members['BREAD'];
    }

    public static function BRICK(): Item
    {
        self::init();
        if (!isset(self::$members['BRICK'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BRICK");
        }
        return clone self::$members['BRICK'];
    }

    public static function BUCKET(): Bucket
    {
        self::init();
        if (!isset(self::$members['BUCKET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item BUCKET");
        }
        return clone self::$members['BUCKET'];
    }

    public static function CARROT(): Carrot
    {
        self::init();
        if (!isset(self::$members['CARROT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CARROT");
        }
        return clone self::$members['CARROT'];
    }

    public static function CHAINMAIL_BOOTS(): Armor
    {
        self::init();
        if (!isset(self::$members['CHAINMAIL_BOOTS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHAINMAIL_BOOTS");
        }
        return clone self::$members['CHAINMAIL_BOOTS'];
    }

    public static function CHAINMAIL_CHESTPLATE(): Armor
    {
        self::init();
        if (!isset(self::$members['CHAINMAIL_CHESTPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHAINMAIL_CHESTPLATE");
        }
        return clone self::$members['CHAINMAIL_CHESTPLATE'];
    }

    public static function CHAINMAIL_HELMET(): Armor
    {
        self::init();
        if (!isset(self::$members['CHAINMAIL_HELMET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHAINMAIL_HELMET");
        }
        return clone self::$members['CHAINMAIL_HELMET'];
    }

    public static function CHAINMAIL_LEGGINGS(): Armor
    {
        self::init();
        if (!isset(self::$members['CHAINMAIL_LEGGINGS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHAINMAIL_LEGGINGS");
        }
        return clone self::$members['CHAINMAIL_LEGGINGS'];
    }

    public static function CHARCOAL(): Coal
    {
        self::init();
        if (!isset(self::$members['CHARCOAL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHARCOAL");
        }
        return clone self::$members['CHARCOAL'];
    }

    public static function CHEMICAL_ALUMINIUM_OXIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_ALUMINIUM_OXIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_ALUMINIUM_OXIDE");
        }
        return clone self::$members['CHEMICAL_ALUMINIUM_OXIDE'];
    }

    public static function CHEMICAL_AMMONIA(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_AMMONIA'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_AMMONIA");
        }
        return clone self::$members['CHEMICAL_AMMONIA'];
    }

    public static function CHEMICAL_BARIUM_SULPHATE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_BARIUM_SULPHATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_BARIUM_SULPHATE");
        }
        return clone self::$members['CHEMICAL_BARIUM_SULPHATE'];
    }

    public static function CHEMICAL_BENZENE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_BENZENE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_BENZENE");
        }
        return clone self::$members['CHEMICAL_BENZENE'];
    }

    public static function CHEMICAL_BORON_TRIOXIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_BORON_TRIOXIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_BORON_TRIOXIDE");
        }
        return clone self::$members['CHEMICAL_BORON_TRIOXIDE'];
    }

    public static function CHEMICAL_CALCIUM_BROMIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_CALCIUM_BROMIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_CALCIUM_BROMIDE");
        }
        return clone self::$members['CHEMICAL_CALCIUM_BROMIDE'];
    }

    public static function CHEMICAL_CALCIUM_CHLORIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_CALCIUM_CHLORIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_CALCIUM_CHLORIDE");
        }
        return clone self::$members['CHEMICAL_CALCIUM_CHLORIDE'];
    }

    public static function CHEMICAL_CERIUM_CHLORIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_CERIUM_CHLORIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_CERIUM_CHLORIDE");
        }
        return clone self::$members['CHEMICAL_CERIUM_CHLORIDE'];
    }

    public static function CHEMICAL_CHARCOAL(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_CHARCOAL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_CHARCOAL");
        }
        return clone self::$members['CHEMICAL_CHARCOAL'];
    }

    public static function CHEMICAL_CRUDE_OIL(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_CRUDE_OIL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_CRUDE_OIL");
        }
        return clone self::$members['CHEMICAL_CRUDE_OIL'];
    }

    public static function CHEMICAL_GLUE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_GLUE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_GLUE");
        }
        return clone self::$members['CHEMICAL_GLUE'];
    }

    public static function CHEMICAL_HYDROGEN_PEROXIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_HYDROGEN_PEROXIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_HYDROGEN_PEROXIDE");
        }
        return clone self::$members['CHEMICAL_HYDROGEN_PEROXIDE'];
    }

    public static function CHEMICAL_HYPOCHLORITE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_HYPOCHLORITE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_HYPOCHLORITE");
        }
        return clone self::$members['CHEMICAL_HYPOCHLORITE'];
    }

    public static function CHEMICAL_INK(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_INK'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_INK");
        }
        return clone self::$members['CHEMICAL_INK'];
    }

    public static function CHEMICAL_IRON_SULPHIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_IRON_SULPHIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_IRON_SULPHIDE");
        }
        return clone self::$members['CHEMICAL_IRON_SULPHIDE'];
    }

    public static function CHEMICAL_LATEX(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_LATEX'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_LATEX");
        }
        return clone self::$members['CHEMICAL_LATEX'];
    }

    public static function CHEMICAL_LITHIUM_HYDRIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_LITHIUM_HYDRIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_LITHIUM_HYDRIDE");
        }
        return clone self::$members['CHEMICAL_LITHIUM_HYDRIDE'];
    }

    public static function CHEMICAL_LUMINOL(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_LUMINOL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_LUMINOL");
        }
        return clone self::$members['CHEMICAL_LUMINOL'];
    }

    public static function CHEMICAL_MAGNESIUM_NITRATE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_MAGNESIUM_NITRATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_MAGNESIUM_NITRATE");
        }
        return clone self::$members['CHEMICAL_MAGNESIUM_NITRATE'];
    }

    public static function CHEMICAL_MAGNESIUM_OXIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_MAGNESIUM_OXIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_MAGNESIUM_OXIDE");
        }
        return clone self::$members['CHEMICAL_MAGNESIUM_OXIDE'];
    }

    public static function CHEMICAL_MAGNESIUM_SALTS(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_MAGNESIUM_SALTS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_MAGNESIUM_SALTS");
        }
        return clone self::$members['CHEMICAL_MAGNESIUM_SALTS'];
    }

    public static function CHEMICAL_MERCURIC_CHLORIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_MERCURIC_CHLORIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_MERCURIC_CHLORIDE");
        }
        return clone self::$members['CHEMICAL_MERCURIC_CHLORIDE'];
    }

    public static function CHEMICAL_POLYETHYLENE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_POLYETHYLENE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_POLYETHYLENE");
        }
        return clone self::$members['CHEMICAL_POLYETHYLENE'];
    }

    public static function CHEMICAL_POTASSIUM_CHLORIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_POTASSIUM_CHLORIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_POTASSIUM_CHLORIDE");
        }
        return clone self::$members['CHEMICAL_POTASSIUM_CHLORIDE'];
    }

    public static function CHEMICAL_POTASSIUM_IODIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_POTASSIUM_IODIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_POTASSIUM_IODIDE");
        }
        return clone self::$members['CHEMICAL_POTASSIUM_IODIDE'];
    }

    public static function CHEMICAL_RUBBISH(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_RUBBISH'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_RUBBISH");
        }
        return clone self::$members['CHEMICAL_RUBBISH'];
    }

    public static function CHEMICAL_SALT(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_SALT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_SALT");
        }
        return clone self::$members['CHEMICAL_SALT'];
    }

    public static function CHEMICAL_SOAP(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_SOAP'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_SOAP");
        }
        return clone self::$members['CHEMICAL_SOAP'];
    }

    public static function CHEMICAL_SODIUM_ACETATE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_SODIUM_ACETATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_SODIUM_ACETATE");
        }
        return clone self::$members['CHEMICAL_SODIUM_ACETATE'];
    }

    public static function CHEMICAL_SODIUM_FLUORIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_SODIUM_FLUORIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_SODIUM_FLUORIDE");
        }
        return clone self::$members['CHEMICAL_SODIUM_FLUORIDE'];
    }

    public static function CHEMICAL_SODIUM_HYDRIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_SODIUM_HYDRIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_SODIUM_HYDRIDE");
        }
        return clone self::$members['CHEMICAL_SODIUM_HYDRIDE'];
    }

    public static function CHEMICAL_SODIUM_HYDROXIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_SODIUM_HYDROXIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_SODIUM_HYDROXIDE");
        }
        return clone self::$members['CHEMICAL_SODIUM_HYDROXIDE'];
    }

    public static function CHEMICAL_SODIUM_HYPOCHLORITE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_SODIUM_HYPOCHLORITE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_SODIUM_HYPOCHLORITE");
        }
        return clone self::$members['CHEMICAL_SODIUM_HYPOCHLORITE'];
    }

    public static function CHEMICAL_SODIUM_OXIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_SODIUM_OXIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_SODIUM_OXIDE");
        }
        return clone self::$members['CHEMICAL_SODIUM_OXIDE'];
    }

    public static function CHEMICAL_SUGAR(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_SUGAR'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_SUGAR");
        }
        return clone self::$members['CHEMICAL_SUGAR'];
    }

    public static function CHEMICAL_SULPHATE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_SULPHATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_SULPHATE");
        }
        return clone self::$members['CHEMICAL_SULPHATE'];
    }

    public static function CHEMICAL_TUNGSTEN_CHLORIDE(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_TUNGSTEN_CHLORIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_TUNGSTEN_CHLORIDE");
        }
        return clone self::$members['CHEMICAL_TUNGSTEN_CHLORIDE'];
    }

    public static function CHEMICAL_WATER(): Item
    {
        self::init();
        if (!isset(self::$members['CHEMICAL_WATER'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHEMICAL_WATER");
        }
        return clone self::$members['CHEMICAL_WATER'];
    }

    public static function CHERRY_HANGING_SIGN(): HangingSign
    {
        self::init();
        if (!isset(self::$members['CHERRY_HANGING_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHERRY_HANGING_SIGN");
        }
        return clone self::$members['CHERRY_HANGING_SIGN'];
    }

    public static function CHERRY_SIGN(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['CHERRY_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHERRY_SIGN");
        }
        return clone self::$members['CHERRY_SIGN'];
    }

    public static function CHORUS_FRUIT(): ChorusFruit
    {
        self::init();
        if (!isset(self::$members['CHORUS_FRUIT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CHORUS_FRUIT");
        }
        return clone self::$members['CHORUS_FRUIT'];
    }

    public static function CLAY(): Item
    {
        self::init();
        if (!isset(self::$members['CLAY'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CLAY");
        }
        return clone self::$members['CLAY'];
    }

    public static function CLOCK(): Clock
    {
        self::init();
        if (!isset(self::$members['CLOCK'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CLOCK");
        }
        return clone self::$members['CLOCK'];
    }

    public static function CLOWNFISH(): Clownfish
    {
        self::init();
        if (!isset(self::$members['CLOWNFISH'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CLOWNFISH");
        }
        return clone self::$members['CLOWNFISH'];
    }

    public static function COAL(): Coal
    {
        self::init();
        if (!isset(self::$members['COAL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COAL");
        }
        return clone self::$members['COAL'];
    }

    public static function COAST_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['COAST_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COAST_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['COAST_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function COCOA_BEANS(): CocoaBeans
    {
        self::init();
        if (!isset(self::$members['COCOA_BEANS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COCOA_BEANS");
        }
        return clone self::$members['COCOA_BEANS'];
    }

    public static function COMPASS(): Compass
    {
        self::init();
        if (!isset(self::$members['COMPASS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COMPASS");
        }
        return clone self::$members['COMPASS'];
    }

    public static function COOKED_CHICKEN(): CookedChicken
    {
        self::init();
        if (!isset(self::$members['COOKED_CHICKEN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COOKED_CHICKEN");
        }
        return clone self::$members['COOKED_CHICKEN'];
    }

    public static function COOKED_FISH(): CookedFish
    {
        self::init();
        if (!isset(self::$members['COOKED_FISH'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COOKED_FISH");
        }
        return clone self::$members['COOKED_FISH'];
    }

    public static function COOKED_MUTTON(): CookedMutton
    {
        self::init();
        if (!isset(self::$members['COOKED_MUTTON'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COOKED_MUTTON");
        }
        return clone self::$members['COOKED_MUTTON'];
    }

    public static function COOKED_PORKCHOP(): CookedPorkchop
    {
        self::init();
        if (!isset(self::$members['COOKED_PORKCHOP'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COOKED_PORKCHOP");
        }
        return clone self::$members['COOKED_PORKCHOP'];
    }

    public static function COOKED_RABBIT(): CookedRabbit
    {
        self::init();
        if (!isset(self::$members['COOKED_RABBIT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COOKED_RABBIT");
        }
        return clone self::$members['COOKED_RABBIT'];
    }

    public static function COOKED_SALMON(): CookedSalmon
    {
        self::init();
        if (!isset(self::$members['COOKED_SALMON'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COOKED_SALMON");
        }
        return clone self::$members['COOKED_SALMON'];
    }

    public static function COOKIE(): Cookie
    {
        self::init();
        if (!isset(self::$members['COOKIE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COOKIE");
        }
        return clone self::$members['COOKIE'];
    }

    public static function COPPER_AXE(): Axe
    {
        self::init();
        if (!isset(self::$members['COPPER_AXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COPPER_AXE");
        }
        return clone self::$members['COPPER_AXE'];
    }

    public static function COPPER_BOOTS(): Armor
    {
        self::init();
        if (!isset(self::$members['COPPER_BOOTS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COPPER_BOOTS");
        }
        return clone self::$members['COPPER_BOOTS'];
    }

    public static function COPPER_CHESTPLATE(): Armor
    {
        self::init();
        if (!isset(self::$members['COPPER_CHESTPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COPPER_CHESTPLATE");
        }
        return clone self::$members['COPPER_CHESTPLATE'];
    }

    public static function COPPER_HELMET(): Armor
    {
        self::init();
        if (!isset(self::$members['COPPER_HELMET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COPPER_HELMET");
        }
        return clone self::$members['COPPER_HELMET'];
    }

    public static function COPPER_HOE(): Hoe
    {
        self::init();
        if (!isset(self::$members['COPPER_HOE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COPPER_HOE");
        }
        return clone self::$members['COPPER_HOE'];
    }

    public static function COPPER_INGOT(): Item
    {
        self::init();
        if (!isset(self::$members['COPPER_INGOT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COPPER_INGOT");
        }
        return clone self::$members['COPPER_INGOT'];
    }

    public static function COPPER_LEGGINGS(): Armor
    {
        self::init();
        if (!isset(self::$members['COPPER_LEGGINGS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COPPER_LEGGINGS");
        }
        return clone self::$members['COPPER_LEGGINGS'];
    }

    public static function COPPER_NUGGET(): Item
    {
        self::init();
        if (!isset(self::$members['COPPER_NUGGET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COPPER_NUGGET");
        }
        return clone self::$members['COPPER_NUGGET'];
    }

    public static function COPPER_PICKAXE(): Pickaxe
    {
        self::init();
        if (!isset(self::$members['COPPER_PICKAXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COPPER_PICKAXE");
        }
        return clone self::$members['COPPER_PICKAXE'];
    }

    public static function COPPER_SHOVEL(): Shovel
    {
        self::init();
        if (!isset(self::$members['COPPER_SHOVEL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COPPER_SHOVEL");
        }
        return clone self::$members['COPPER_SHOVEL'];
    }

    public static function COPPER_SWORD(): Sword
    {
        self::init();
        if (!isset(self::$members['COPPER_SWORD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item COPPER_SWORD");
        }
        return clone self::$members['COPPER_SWORD'];
    }

    public static function CORAL_FAN(): CoralFan
    {
        self::init();
        if (!isset(self::$members['CORAL_FAN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CORAL_FAN");
        }
        return clone self::$members['CORAL_FAN'];
    }

    public static function CRIMSON_HANGING_SIGN(): HangingSign
    {
        self::init();
        if (!isset(self::$members['CRIMSON_HANGING_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CRIMSON_HANGING_SIGN");
        }
        return clone self::$members['CRIMSON_HANGING_SIGN'];
    }

    public static function CRIMSON_SIGN(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['CRIMSON_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item CRIMSON_SIGN");
        }
        return clone self::$members['CRIMSON_SIGN'];
    }

    public static function DARK_OAK_BOAT(): Boat
    {
        self::init();
        if (!isset(self::$members['DARK_OAK_BOAT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DARK_OAK_BOAT");
        }
        return clone self::$members['DARK_OAK_BOAT'];
    }

    public static function DARK_OAK_HANGING_SIGN(): HangingSign
    {
        self::init();
        if (!isset(self::$members['DARK_OAK_HANGING_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DARK_OAK_HANGING_SIGN");
        }
        return clone self::$members['DARK_OAK_HANGING_SIGN'];
    }

    public static function DARK_OAK_SIGN(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['DARK_OAK_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DARK_OAK_SIGN");
        }
        return clone self::$members['DARK_OAK_SIGN'];
    }

    public static function DIAMOND(): Item
    {
        self::init();
        if (!isset(self::$members['DIAMOND'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DIAMOND");
        }
        return clone self::$members['DIAMOND'];
    }

    public static function DIAMOND_AXE(): Axe
    {
        self::init();
        if (!isset(self::$members['DIAMOND_AXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DIAMOND_AXE");
        }
        return clone self::$members['DIAMOND_AXE'];
    }

    public static function DIAMOND_BOOTS(): Armor
    {
        self::init();
        if (!isset(self::$members['DIAMOND_BOOTS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DIAMOND_BOOTS");
        }
        return clone self::$members['DIAMOND_BOOTS'];
    }

    public static function DIAMOND_CHESTPLATE(): Armor
    {
        self::init();
        if (!isset(self::$members['DIAMOND_CHESTPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DIAMOND_CHESTPLATE");
        }
        return clone self::$members['DIAMOND_CHESTPLATE'];
    }

    public static function DIAMOND_HELMET(): Armor
    {
        self::init();
        if (!isset(self::$members['DIAMOND_HELMET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DIAMOND_HELMET");
        }
        return clone self::$members['DIAMOND_HELMET'];
    }

    public static function DIAMOND_HOE(): Hoe
    {
        self::init();
        if (!isset(self::$members['DIAMOND_HOE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DIAMOND_HOE");
        }
        return clone self::$members['DIAMOND_HOE'];
    }

    public static function DIAMOND_LEGGINGS(): Armor
    {
        self::init();
        if (!isset(self::$members['DIAMOND_LEGGINGS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DIAMOND_LEGGINGS");
        }
        return clone self::$members['DIAMOND_LEGGINGS'];
    }

    public static function DIAMOND_PICKAXE(): Pickaxe
    {
        self::init();
        if (!isset(self::$members['DIAMOND_PICKAXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DIAMOND_PICKAXE");
        }
        return clone self::$members['DIAMOND_PICKAXE'];
    }

    public static function DIAMOND_SHOVEL(): Shovel
    {
        self::init();
        if (!isset(self::$members['DIAMOND_SHOVEL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DIAMOND_SHOVEL");
        }
        return clone self::$members['DIAMOND_SHOVEL'];
    }

    public static function DIAMOND_SWORD(): Sword
    {
        self::init();
        if (!isset(self::$members['DIAMOND_SWORD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DIAMOND_SWORD");
        }
        return clone self::$members['DIAMOND_SWORD'];
    }

    public static function DISC_FRAGMENT_5(): Item
    {
        self::init();
        if (!isset(self::$members['DISC_FRAGMENT_5'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DISC_FRAGMENT_5");
        }
        return clone self::$members['DISC_FRAGMENT_5'];
    }

    public static function DRAGON_BREATH(): Item
    {
        self::init();
        if (!isset(self::$members['DRAGON_BREATH'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DRAGON_BREATH");
        }
        return clone self::$members['DRAGON_BREATH'];
    }

    public static function DRIED_KELP(): DriedKelp
    {
        self::init();
        if (!isset(self::$members['DRIED_KELP'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DRIED_KELP");
        }
        return clone self::$members['DRIED_KELP'];
    }

    public static function DUNE_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['DUNE_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DUNE_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['DUNE_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function DYE(): Dye
    {
        self::init();
        if (!isset(self::$members['DYE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item DYE");
        }
        return clone self::$members['DYE'];
    }

    public static function ECHO_SHARD(): Item
    {
        self::init();
        if (!isset(self::$members['ECHO_SHARD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item ECHO_SHARD");
        }
        return clone self::$members['ECHO_SHARD'];
    }

    public static function EGG(): Egg
    {
        self::init();
        if (!isset(self::$members['EGG'])) {
            throw new \InvalidArgumentException("Unknown vanilla item EGG");
        }
        return clone self::$members['EGG'];
    }

    public static function EMERALD(): Item
    {
        self::init();
        if (!isset(self::$members['EMERALD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item EMERALD");
        }
        return clone self::$members['EMERALD'];
    }

    public static function ENCHANTED_BOOK(): EnchantedBook
    {
        self::init();
        if (!isset(self::$members['ENCHANTED_BOOK'])) {
            throw new \InvalidArgumentException("Unknown vanilla item ENCHANTED_BOOK");
        }
        return clone self::$members['ENCHANTED_BOOK'];
    }

    public static function ENCHANTED_GOLDEN_APPLE(): GoldenAppleEnchanted
    {
        self::init();
        if (!isset(self::$members['ENCHANTED_GOLDEN_APPLE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item ENCHANTED_GOLDEN_APPLE");
        }
        return clone self::$members['ENCHANTED_GOLDEN_APPLE'];
    }

    public static function ENDER_PEARL(): EnderPearl
    {
        self::init();
        if (!isset(self::$members['ENDER_PEARL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item ENDER_PEARL");
        }
        return clone self::$members['ENDER_PEARL'];
    }

    public static function END_CRYSTAL(): EndCrystal
    {
        self::init();
        if (!isset(self::$members['END_CRYSTAL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item END_CRYSTAL");
        }
        return clone self::$members['END_CRYSTAL'];
    }

    public static function EXPERIENCE_BOTTLE(): ExperienceBottle
    {
        self::init();
        if (!isset(self::$members['EXPERIENCE_BOTTLE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item EXPERIENCE_BOTTLE");
        }
        return clone self::$members['EXPERIENCE_BOTTLE'];
    }

    public static function EYE_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['EYE_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item EYE_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['EYE_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function FEATHER(): Item
    {
        self::init();
        if (!isset(self::$members['FEATHER'])) {
            throw new \InvalidArgumentException("Unknown vanilla item FEATHER");
        }
        return clone self::$members['FEATHER'];
    }

    public static function FERMENTED_SPIDER_EYE(): Item
    {
        self::init();
        if (!isset(self::$members['FERMENTED_SPIDER_EYE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item FERMENTED_SPIDER_EYE");
        }
        return clone self::$members['FERMENTED_SPIDER_EYE'];
    }

    public static function FIREWORK_ROCKET(): FireworkRocket
    {
        self::init();
        if (!isset(self::$members['FIREWORK_ROCKET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item FIREWORK_ROCKET");
        }
        return clone self::$members['FIREWORK_ROCKET'];
    }

    public static function FIREWORK_STAR(): FireworkStar
    {
        self::init();
        if (!isset(self::$members['FIREWORK_STAR'])) {
            throw new \InvalidArgumentException("Unknown vanilla item FIREWORK_STAR");
        }
        return clone self::$members['FIREWORK_STAR'];
    }

    public static function FIRE_CHARGE(): FireCharge
    {
        self::init();
        if (!isset(self::$members['FIRE_CHARGE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item FIRE_CHARGE");
        }
        return clone self::$members['FIRE_CHARGE'];
    }

    public static function FISHING_ROD(): FishingRod
    {
        self::init();
        if (!isset(self::$members['FISHING_ROD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item FISHING_ROD");
        }
        return clone self::$members['FISHING_ROD'];
    }

    public static function FLINT(): Item
    {
        self::init();
        if (!isset(self::$members['FLINT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item FLINT");
        }
        return clone self::$members['FLINT'];
    }

    public static function FLINT_AND_STEEL(): FlintSteel
    {
        self::init();
        if (!isset(self::$members['FLINT_AND_STEEL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item FLINT_AND_STEEL");
        }
        return clone self::$members['FLINT_AND_STEEL'];
    }

    public static function GHAST_TEAR(): Item
    {
        self::init();
        if (!isset(self::$members['GHAST_TEAR'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GHAST_TEAR");
        }
        return clone self::$members['GHAST_TEAR'];
    }

    public static function GLASS_BOTTLE(): GlassBottle
    {
        self::init();
        if (!isset(self::$members['GLASS_BOTTLE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GLASS_BOTTLE");
        }
        return clone self::$members['GLASS_BOTTLE'];
    }

    public static function GLISTERING_MELON(): Item
    {
        self::init();
        if (!isset(self::$members['GLISTERING_MELON'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GLISTERING_MELON");
        }
        return clone self::$members['GLISTERING_MELON'];
    }

    public static function GLOWSTONE_DUST(): Item
    {
        self::init();
        if (!isset(self::$members['GLOWSTONE_DUST'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GLOWSTONE_DUST");
        }
        return clone self::$members['GLOWSTONE_DUST'];
    }

    public static function GLOW_BERRIES(): GlowBerries
    {
        self::init();
        if (!isset(self::$members['GLOW_BERRIES'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GLOW_BERRIES");
        }
        return clone self::$members['GLOW_BERRIES'];
    }

    public static function GLOW_INK_SAC(): Item
    {
        self::init();
        if (!isset(self::$members['GLOW_INK_SAC'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GLOW_INK_SAC");
        }
        return clone self::$members['GLOW_INK_SAC'];
    }

    public static function GOAT_HORN(): GoatHorn
    {
        self::init();
        if (!isset(self::$members['GOAT_HORN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOAT_HORN");
        }
        return clone self::$members['GOAT_HORN'];
    }

    public static function GOLDEN_APPLE(): GoldenApple
    {
        self::init();
        if (!isset(self::$members['GOLDEN_APPLE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLDEN_APPLE");
        }
        return clone self::$members['GOLDEN_APPLE'];
    }

    public static function GOLDEN_AXE(): Axe
    {
        self::init();
        if (!isset(self::$members['GOLDEN_AXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLDEN_AXE");
        }
        return clone self::$members['GOLDEN_AXE'];
    }

    public static function GOLDEN_BOOTS(): Armor
    {
        self::init();
        if (!isset(self::$members['GOLDEN_BOOTS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLDEN_BOOTS");
        }
        return clone self::$members['GOLDEN_BOOTS'];
    }

    public static function GOLDEN_CARROT(): GoldenCarrot
    {
        self::init();
        if (!isset(self::$members['GOLDEN_CARROT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLDEN_CARROT");
        }
        return clone self::$members['GOLDEN_CARROT'];
    }

    public static function GOLDEN_CHESTPLATE(): Armor
    {
        self::init();
        if (!isset(self::$members['GOLDEN_CHESTPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLDEN_CHESTPLATE");
        }
        return clone self::$members['GOLDEN_CHESTPLATE'];
    }

    public static function GOLDEN_HELMET(): Armor
    {
        self::init();
        if (!isset(self::$members['GOLDEN_HELMET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLDEN_HELMET");
        }
        return clone self::$members['GOLDEN_HELMET'];
    }

    public static function GOLDEN_HOE(): Hoe
    {
        self::init();
        if (!isset(self::$members['GOLDEN_HOE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLDEN_HOE");
        }
        return clone self::$members['GOLDEN_HOE'];
    }

    public static function GOLDEN_LEGGINGS(): Armor
    {
        self::init();
        if (!isset(self::$members['GOLDEN_LEGGINGS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLDEN_LEGGINGS");
        }
        return clone self::$members['GOLDEN_LEGGINGS'];
    }

    public static function GOLDEN_PICKAXE(): Pickaxe
    {
        self::init();
        if (!isset(self::$members['GOLDEN_PICKAXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLDEN_PICKAXE");
        }
        return clone self::$members['GOLDEN_PICKAXE'];
    }

    public static function GOLDEN_SHOVEL(): Shovel
    {
        self::init();
        if (!isset(self::$members['GOLDEN_SHOVEL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLDEN_SHOVEL");
        }
        return clone self::$members['GOLDEN_SHOVEL'];
    }

    public static function GOLDEN_SWORD(): Sword
    {
        self::init();
        if (!isset(self::$members['GOLDEN_SWORD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLDEN_SWORD");
        }
        return clone self::$members['GOLDEN_SWORD'];
    }

    public static function GOLD_INGOT(): Item
    {
        self::init();
        if (!isset(self::$members['GOLD_INGOT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLD_INGOT");
        }
        return clone self::$members['GOLD_INGOT'];
    }

    public static function GOLD_NUGGET(): Item
    {
        self::init();
        if (!isset(self::$members['GOLD_NUGGET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GOLD_NUGGET");
        }
        return clone self::$members['GOLD_NUGGET'];
    }

    public static function GUNPOWDER(): Item
    {
        self::init();
        if (!isset(self::$members['GUNPOWDER'])) {
            throw new \InvalidArgumentException("Unknown vanilla item GUNPOWDER");
        }
        return clone self::$members['GUNPOWDER'];
    }

    public static function HEART_OF_THE_SEA(): Item
    {
        self::init();
        if (!isset(self::$members['HEART_OF_THE_SEA'])) {
            throw new \InvalidArgumentException("Unknown vanilla item HEART_OF_THE_SEA");
        }
        return clone self::$members['HEART_OF_THE_SEA'];
    }

    public static function HONEYCOMB(): Item
    {
        self::init();
        if (!isset(self::$members['HONEYCOMB'])) {
            throw new \InvalidArgumentException("Unknown vanilla item HONEYCOMB");
        }
        return clone self::$members['HONEYCOMB'];
    }

    public static function HONEY_BOTTLE(): HoneyBottle
    {
        self::init();
        if (!isset(self::$members['HONEY_BOTTLE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item HONEY_BOTTLE");
        }
        return clone self::$members['HONEY_BOTTLE'];
    }

    public static function HOST_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['HOST_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item HOST_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['HOST_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function ICE_BOMB(): IceBomb
    {
        self::init();
        if (!isset(self::$members['ICE_BOMB'])) {
            throw new \InvalidArgumentException("Unknown vanilla item ICE_BOMB");
        }
        return clone self::$members['ICE_BOMB'];
    }

    public static function INK_SAC(): Item
    {
        self::init();
        if (!isset(self::$members['INK_SAC'])) {
            throw new \InvalidArgumentException("Unknown vanilla item INK_SAC");
        }
        return clone self::$members['INK_SAC'];
    }

    public static function IRON_AXE(): Axe
    {
        self::init();
        if (!isset(self::$members['IRON_AXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item IRON_AXE");
        }
        return clone self::$members['IRON_AXE'];
    }

    public static function IRON_BOOTS(): Armor
    {
        self::init();
        if (!isset(self::$members['IRON_BOOTS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item IRON_BOOTS");
        }
        return clone self::$members['IRON_BOOTS'];
    }

    public static function IRON_CHESTPLATE(): Armor
    {
        self::init();
        if (!isset(self::$members['IRON_CHESTPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item IRON_CHESTPLATE");
        }
        return clone self::$members['IRON_CHESTPLATE'];
    }

    public static function IRON_HELMET(): Armor
    {
        self::init();
        if (!isset(self::$members['IRON_HELMET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item IRON_HELMET");
        }
        return clone self::$members['IRON_HELMET'];
    }

    public static function IRON_HOE(): Hoe
    {
        self::init();
        if (!isset(self::$members['IRON_HOE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item IRON_HOE");
        }
        return clone self::$members['IRON_HOE'];
    }

    public static function IRON_INGOT(): Item
    {
        self::init();
        if (!isset(self::$members['IRON_INGOT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item IRON_INGOT");
        }
        return clone self::$members['IRON_INGOT'];
    }

    public static function IRON_LEGGINGS(): Armor
    {
        self::init();
        if (!isset(self::$members['IRON_LEGGINGS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item IRON_LEGGINGS");
        }
        return clone self::$members['IRON_LEGGINGS'];
    }

    public static function IRON_NUGGET(): Item
    {
        self::init();
        if (!isset(self::$members['IRON_NUGGET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item IRON_NUGGET");
        }
        return clone self::$members['IRON_NUGGET'];
    }

    public static function IRON_PICKAXE(): Pickaxe
    {
        self::init();
        if (!isset(self::$members['IRON_PICKAXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item IRON_PICKAXE");
        }
        return clone self::$members['IRON_PICKAXE'];
    }

    public static function IRON_SHOVEL(): Shovel
    {
        self::init();
        if (!isset(self::$members['IRON_SHOVEL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item IRON_SHOVEL");
        }
        return clone self::$members['IRON_SHOVEL'];
    }

    public static function IRON_SWORD(): Sword
    {
        self::init();
        if (!isset(self::$members['IRON_SWORD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item IRON_SWORD");
        }
        return clone self::$members['IRON_SWORD'];
    }

    public static function JUNGLE_BOAT(): Boat
    {
        self::init();
        if (!isset(self::$members['JUNGLE_BOAT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item JUNGLE_BOAT");
        }
        return clone self::$members['JUNGLE_BOAT'];
    }

    public static function JUNGLE_HANGING_SIGN(): HangingSign
    {
        self::init();
        if (!isset(self::$members['JUNGLE_HANGING_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item JUNGLE_HANGING_SIGN");
        }
        return clone self::$members['JUNGLE_HANGING_SIGN'];
    }

    public static function JUNGLE_SIGN(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['JUNGLE_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item JUNGLE_SIGN");
        }
        return clone self::$members['JUNGLE_SIGN'];
    }

    public static function LAPIS_LAZULI(): Item
    {
        self::init();
        if (!isset(self::$members['LAPIS_LAZULI'])) {
            throw new \InvalidArgumentException("Unknown vanilla item LAPIS_LAZULI");
        }
        return clone self::$members['LAPIS_LAZULI'];
    }

    public static function LAVA_BUCKET(): LiquidBucket
    {
        self::init();
        if (!isset(self::$members['LAVA_BUCKET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item LAVA_BUCKET");
        }
        return clone self::$members['LAVA_BUCKET'];
    }

    public static function LEATHER(): Item
    {
        self::init();
        if (!isset(self::$members['LEATHER'])) {
            throw new \InvalidArgumentException("Unknown vanilla item LEATHER");
        }
        return clone self::$members['LEATHER'];
    }

    public static function LEATHER_BOOTS(): Armor
    {
        self::init();
        if (!isset(self::$members['LEATHER_BOOTS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item LEATHER_BOOTS");
        }
        return clone self::$members['LEATHER_BOOTS'];
    }

    public static function LEATHER_CAP(): Armor
    {
        self::init();
        if (!isset(self::$members['LEATHER_CAP'])) {
            throw new \InvalidArgumentException("Unknown vanilla item LEATHER_CAP");
        }
        return clone self::$members['LEATHER_CAP'];
    }

    public static function LEATHER_PANTS(): Armor
    {
        self::init();
        if (!isset(self::$members['LEATHER_PANTS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item LEATHER_PANTS");
        }
        return clone self::$members['LEATHER_PANTS'];
    }

    public static function LEATHER_TUNIC(): Armor
    {
        self::init();
        if (!isset(self::$members['LEATHER_TUNIC'])) {
            throw new \InvalidArgumentException("Unknown vanilla item LEATHER_TUNIC");
        }
        return clone self::$members['LEATHER_TUNIC'];
    }

    public static function LINGERING_POTION(): SplashPotion
    {
        self::init();
        if (!isset(self::$members['LINGERING_POTION'])) {
            throw new \InvalidArgumentException("Unknown vanilla item LINGERING_POTION");
        }
        return clone self::$members['LINGERING_POTION'];
    }

    public static function MAGMA_CREAM(): Item
    {
        self::init();
        if (!isset(self::$members['MAGMA_CREAM'])) {
            throw new \InvalidArgumentException("Unknown vanilla item MAGMA_CREAM");
        }
        return clone self::$members['MAGMA_CREAM'];
    }

    public static function MANGROVE_BOAT(): Boat
    {
        self::init();
        if (!isset(self::$members['MANGROVE_BOAT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item MANGROVE_BOAT");
        }
        return clone self::$members['MANGROVE_BOAT'];
    }

    public static function MANGROVE_HANGING_SIGN(): HangingSign
    {
        self::init();
        if (!isset(self::$members['MANGROVE_HANGING_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item MANGROVE_HANGING_SIGN");
        }
        return clone self::$members['MANGROVE_HANGING_SIGN'];
    }

    public static function MANGROVE_SIGN(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['MANGROVE_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item MANGROVE_SIGN");
        }
        return clone self::$members['MANGROVE_SIGN'];
    }

    public static function MEDICINE(): Medicine
    {
        self::init();
        if (!isset(self::$members['MEDICINE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item MEDICINE");
        }
        return clone self::$members['MEDICINE'];
    }

    public static function MELON(): Melon
    {
        self::init();
        if (!isset(self::$members['MELON'])) {
            throw new \InvalidArgumentException("Unknown vanilla item MELON");
        }
        return clone self::$members['MELON'];
    }

    public static function MELON_SEEDS(): MelonSeeds
    {
        self::init();
        if (!isset(self::$members['MELON_SEEDS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item MELON_SEEDS");
        }
        return clone self::$members['MELON_SEEDS'];
    }

    public static function MILK_BUCKET(): MilkBucket
    {
        self::init();
        if (!isset(self::$members['MILK_BUCKET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item MILK_BUCKET");
        }
        return clone self::$members['MILK_BUCKET'];
    }

    public static function MINECART(): Minecart
    {
        self::init();
        if (!isset(self::$members['MINECART'])) {
            throw new \InvalidArgumentException("Unknown vanilla item MINECART");
        }
        return clone self::$members['MINECART'];
    }

    public static function MUSHROOM_STEW(): MushroomStew
    {
        self::init();
        if (!isset(self::$members['MUSHROOM_STEW'])) {
            throw new \InvalidArgumentException("Unknown vanilla item MUSHROOM_STEW");
        }
        return clone self::$members['MUSHROOM_STEW'];
    }

    public static function NAME_TAG(): NameTag
    {
        self::init();
        if (!isset(self::$members['NAME_TAG'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NAME_TAG");
        }
        return clone self::$members['NAME_TAG'];
    }

    public static function NAUTILUS_SHELL(): Item
    {
        self::init();
        if (!isset(self::$members['NAUTILUS_SHELL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NAUTILUS_SHELL");
        }
        return clone self::$members['NAUTILUS_SHELL'];
    }

    public static function NETHERITE_AXE(): Axe
    {
        self::init();
        if (!isset(self::$members['NETHERITE_AXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHERITE_AXE");
        }
        return clone self::$members['NETHERITE_AXE'];
    }

    public static function NETHERITE_BOOTS(): Armor
    {
        self::init();
        if (!isset(self::$members['NETHERITE_BOOTS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHERITE_BOOTS");
        }
        return clone self::$members['NETHERITE_BOOTS'];
    }

    public static function NETHERITE_CHESTPLATE(): Armor
    {
        self::init();
        if (!isset(self::$members['NETHERITE_CHESTPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHERITE_CHESTPLATE");
        }
        return clone self::$members['NETHERITE_CHESTPLATE'];
    }

    public static function NETHERITE_HELMET(): Armor
    {
        self::init();
        if (!isset(self::$members['NETHERITE_HELMET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHERITE_HELMET");
        }
        return clone self::$members['NETHERITE_HELMET'];
    }

    public static function NETHERITE_HOE(): Hoe
    {
        self::init();
        if (!isset(self::$members['NETHERITE_HOE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHERITE_HOE");
        }
        return clone self::$members['NETHERITE_HOE'];
    }

    public static function NETHERITE_INGOT(): Item
    {
        self::init();
        if (!isset(self::$members['NETHERITE_INGOT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHERITE_INGOT");
        }
        return clone self::$members['NETHERITE_INGOT'];
    }

    public static function NETHERITE_LEGGINGS(): Armor
    {
        self::init();
        if (!isset(self::$members['NETHERITE_LEGGINGS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHERITE_LEGGINGS");
        }
        return clone self::$members['NETHERITE_LEGGINGS'];
    }

    public static function NETHERITE_PICKAXE(): Pickaxe
    {
        self::init();
        if (!isset(self::$members['NETHERITE_PICKAXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHERITE_PICKAXE");
        }
        return clone self::$members['NETHERITE_PICKAXE'];
    }

    public static function NETHERITE_SCRAP(): Item
    {
        self::init();
        if (!isset(self::$members['NETHERITE_SCRAP'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHERITE_SCRAP");
        }
        return clone self::$members['NETHERITE_SCRAP'];
    }

    public static function NETHERITE_SHOVEL(): Shovel
    {
        self::init();
        if (!isset(self::$members['NETHERITE_SHOVEL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHERITE_SHOVEL");
        }
        return clone self::$members['NETHERITE_SHOVEL'];
    }

    public static function NETHERITE_SWORD(): Sword
    {
        self::init();
        if (!isset(self::$members['NETHERITE_SWORD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHERITE_SWORD");
        }
        return clone self::$members['NETHERITE_SWORD'];
    }

    public static function NETHERITE_UPGRADE_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['NETHERITE_UPGRADE_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHERITE_UPGRADE_SMITHING_TEMPLATE");
        }
        return clone self::$members['NETHERITE_UPGRADE_SMITHING_TEMPLATE'];
    }

    public static function NETHER_BRICK(): Item
    {
        self::init();
        if (!isset(self::$members['NETHER_BRICK'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHER_BRICK");
        }
        return clone self::$members['NETHER_BRICK'];
    }

    public static function NETHER_QUARTZ(): Item
    {
        self::init();
        if (!isset(self::$members['NETHER_QUARTZ'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHER_QUARTZ");
        }
        return clone self::$members['NETHER_QUARTZ'];
    }

    public static function NETHER_STAR(): Item
    {
        self::init();
        if (!isset(self::$members['NETHER_STAR'])) {
            throw new \InvalidArgumentException("Unknown vanilla item NETHER_STAR");
        }
        return clone self::$members['NETHER_STAR'];
    }

    public static function OAK_BOAT(): Boat
    {
        self::init();
        if (!isset(self::$members['OAK_BOAT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item OAK_BOAT");
        }
        return clone self::$members['OAK_BOAT'];
    }

    public static function OAK_HANGING_SIGN(): HangingSign
    {
        self::init();
        if (!isset(self::$members['OAK_HANGING_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item OAK_HANGING_SIGN");
        }
        return clone self::$members['OAK_HANGING_SIGN'];
    }

    public static function OAK_SIGN(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['OAK_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item OAK_SIGN");
        }
        return clone self::$members['OAK_SIGN'];
    }

    public static function OMINOUS_BANNER(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['OMINOUS_BANNER'])) {
            throw new \InvalidArgumentException("Unknown vanilla item OMINOUS_BANNER");
        }
        return clone self::$members['OMINOUS_BANNER'];
    }

    public static function PAINTING(): PaintingItem
    {
        self::init();
        if (!isset(self::$members['PAINTING'])) {
            throw new \InvalidArgumentException("Unknown vanilla item PAINTING");
        }
        return clone self::$members['PAINTING'];
    }

    public static function PALE_OAK_HANGING_SIGN(): HangingSign
    {
        self::init();
        if (!isset(self::$members['PALE_OAK_HANGING_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item PALE_OAK_HANGING_SIGN");
        }
        return clone self::$members['PALE_OAK_HANGING_SIGN'];
    }

    public static function PALE_OAK_SIGN(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['PALE_OAK_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item PALE_OAK_SIGN");
        }
        return clone self::$members['PALE_OAK_SIGN'];
    }

    public static function PAPER(): Item
    {
        self::init();
        if (!isset(self::$members['PAPER'])) {
            throw new \InvalidArgumentException("Unknown vanilla item PAPER");
        }
        return clone self::$members['PAPER'];
    }

    public static function PHANTOM_MEMBRANE(): Item
    {
        self::init();
        if (!isset(self::$members['PHANTOM_MEMBRANE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item PHANTOM_MEMBRANE");
        }
        return clone self::$members['PHANTOM_MEMBRANE'];
    }

    public static function PITCHER_POD(): PitcherPod
    {
        self::init();
        if (!isset(self::$members['PITCHER_POD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item PITCHER_POD");
        }
        return clone self::$members['PITCHER_POD'];
    }

    public static function POISONOUS_POTATO(): PoisonousPotato
    {
        self::init();
        if (!isset(self::$members['POISONOUS_POTATO'])) {
            throw new \InvalidArgumentException("Unknown vanilla item POISONOUS_POTATO");
        }
        return clone self::$members['POISONOUS_POTATO'];
    }

    public static function POPPED_CHORUS_FRUIT(): Item
    {
        self::init();
        if (!isset(self::$members['POPPED_CHORUS_FRUIT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item POPPED_CHORUS_FRUIT");
        }
        return clone self::$members['POPPED_CHORUS_FRUIT'];
    }

    public static function POTATO(): Potato
    {
        self::init();
        if (!isset(self::$members['POTATO'])) {
            throw new \InvalidArgumentException("Unknown vanilla item POTATO");
        }
        return clone self::$members['POTATO'];
    }

    public static function POTION(): Potion
    {
        self::init();
        if (!isset(self::$members['POTION'])) {
            throw new \InvalidArgumentException("Unknown vanilla item POTION");
        }
        return clone self::$members['POTION'];
    }

    public static function PRISMARINE_CRYSTALS(): Item
    {
        self::init();
        if (!isset(self::$members['PRISMARINE_CRYSTALS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item PRISMARINE_CRYSTALS");
        }
        return clone self::$members['PRISMARINE_CRYSTALS'];
    }

    public static function PRISMARINE_SHARD(): Item
    {
        self::init();
        if (!isset(self::$members['PRISMARINE_SHARD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item PRISMARINE_SHARD");
        }
        return clone self::$members['PRISMARINE_SHARD'];
    }

    public static function PUFFERFISH(): Pufferfish
    {
        self::init();
        if (!isset(self::$members['PUFFERFISH'])) {
            throw new \InvalidArgumentException("Unknown vanilla item PUFFERFISH");
        }
        return clone self::$members['PUFFERFISH'];
    }

    public static function PUMPKIN_PIE(): PumpkinPie
    {
        self::init();
        if (!isset(self::$members['PUMPKIN_PIE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item PUMPKIN_PIE");
        }
        return clone self::$members['PUMPKIN_PIE'];
    }

    public static function PUMPKIN_SEEDS(): PumpkinSeeds
    {
        self::init();
        if (!isset(self::$members['PUMPKIN_SEEDS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item PUMPKIN_SEEDS");
        }
        return clone self::$members['PUMPKIN_SEEDS'];
    }

    public static function RABBIT_FOOT(): Item
    {
        self::init();
        if (!isset(self::$members['RABBIT_FOOT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RABBIT_FOOT");
        }
        return clone self::$members['RABBIT_FOOT'];
    }

    public static function RABBIT_HIDE(): Item
    {
        self::init();
        if (!isset(self::$members['RABBIT_HIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RABBIT_HIDE");
        }
        return clone self::$members['RABBIT_HIDE'];
    }

    public static function RABBIT_STEW(): RabbitStew
    {
        self::init();
        if (!isset(self::$members['RABBIT_STEW'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RABBIT_STEW");
        }
        return clone self::$members['RABBIT_STEW'];
    }

    public static function RAISER_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['RAISER_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RAISER_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['RAISER_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function RAW_BEEF(): RawBeef
    {
        self::init();
        if (!isset(self::$members['RAW_BEEF'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RAW_BEEF");
        }
        return clone self::$members['RAW_BEEF'];
    }

    public static function RAW_CHICKEN(): RawChicken
    {
        self::init();
        if (!isset(self::$members['RAW_CHICKEN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RAW_CHICKEN");
        }
        return clone self::$members['RAW_CHICKEN'];
    }

    public static function RAW_COPPER(): Item
    {
        self::init();
        if (!isset(self::$members['RAW_COPPER'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RAW_COPPER");
        }
        return clone self::$members['RAW_COPPER'];
    }

    public static function RAW_FISH(): RawFish
    {
        self::init();
        if (!isset(self::$members['RAW_FISH'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RAW_FISH");
        }
        return clone self::$members['RAW_FISH'];
    }

    public static function RAW_GOLD(): Item
    {
        self::init();
        if (!isset(self::$members['RAW_GOLD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RAW_GOLD");
        }
        return clone self::$members['RAW_GOLD'];
    }

    public static function RAW_IRON(): Item
    {
        self::init();
        if (!isset(self::$members['RAW_IRON'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RAW_IRON");
        }
        return clone self::$members['RAW_IRON'];
    }

    public static function RAW_MUTTON(): RawMutton
    {
        self::init();
        if (!isset(self::$members['RAW_MUTTON'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RAW_MUTTON");
        }
        return clone self::$members['RAW_MUTTON'];
    }

    public static function RAW_PORKCHOP(): RawPorkchop
    {
        self::init();
        if (!isset(self::$members['RAW_PORKCHOP'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RAW_PORKCHOP");
        }
        return clone self::$members['RAW_PORKCHOP'];
    }

    public static function RAW_RABBIT(): RawRabbit
    {
        self::init();
        if (!isset(self::$members['RAW_RABBIT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RAW_RABBIT");
        }
        return clone self::$members['RAW_RABBIT'];
    }

    public static function RAW_SALMON(): RawSalmon
    {
        self::init();
        if (!isset(self::$members['RAW_SALMON'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RAW_SALMON");
        }
        return clone self::$members['RAW_SALMON'];
    }

    public static function RECORD_11(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_11'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_11");
        }
        return clone self::$members['RECORD_11'];
    }

    public static function RECORD_13(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_13'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_13");
        }
        return clone self::$members['RECORD_13'];
    }

    public static function RECORD_5(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_5'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_5");
        }
        return clone self::$members['RECORD_5'];
    }

    public static function RECORD_BLOCKS(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_BLOCKS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_BLOCKS");
        }
        return clone self::$members['RECORD_BLOCKS'];
    }

    public static function RECORD_CAT(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_CAT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_CAT");
        }
        return clone self::$members['RECORD_CAT'];
    }

    public static function RECORD_CHIRP(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_CHIRP'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_CHIRP");
        }
        return clone self::$members['RECORD_CHIRP'];
    }

    public static function RECORD_CREATOR(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_CREATOR'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_CREATOR");
        }
        return clone self::$members['RECORD_CREATOR'];
    }

    public static function RECORD_CREATOR_MUSIC_BOX(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_CREATOR_MUSIC_BOX'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_CREATOR_MUSIC_BOX");
        }
        return clone self::$members['RECORD_CREATOR_MUSIC_BOX'];
    }

    public static function RECORD_FAR(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_FAR'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_FAR");
        }
        return clone self::$members['RECORD_FAR'];
    }

    public static function RECORD_LAVA_CHICKEN(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_LAVA_CHICKEN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_LAVA_CHICKEN");
        }
        return clone self::$members['RECORD_LAVA_CHICKEN'];
    }

    public static function RECORD_MALL(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_MALL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_MALL");
        }
        return clone self::$members['RECORD_MALL'];
    }

    public static function RECORD_MELLOHI(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_MELLOHI'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_MELLOHI");
        }
        return clone self::$members['RECORD_MELLOHI'];
    }

    public static function RECORD_OTHERSIDE(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_OTHERSIDE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_OTHERSIDE");
        }
        return clone self::$members['RECORD_OTHERSIDE'];
    }

    public static function RECORD_PIGSTEP(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_PIGSTEP'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_PIGSTEP");
        }
        return clone self::$members['RECORD_PIGSTEP'];
    }

    public static function RECORD_PRECIPICE(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_PRECIPICE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_PRECIPICE");
        }
        return clone self::$members['RECORD_PRECIPICE'];
    }

    public static function RECORD_RELIC(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_RELIC'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_RELIC");
        }
        return clone self::$members['RECORD_RELIC'];
    }

    public static function RECORD_STAL(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_STAL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_STAL");
        }
        return clone self::$members['RECORD_STAL'];
    }

    public static function RECORD_STRAD(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_STRAD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_STRAD");
        }
        return clone self::$members['RECORD_STRAD'];
    }

    public static function RECORD_WAIT(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_WAIT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_WAIT");
        }
        return clone self::$members['RECORD_WAIT'];
    }

    public static function RECORD_WARD(): Record
    {
        self::init();
        if (!isset(self::$members['RECORD_WARD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECORD_WARD");
        }
        return clone self::$members['RECORD_WARD'];
    }

    public static function RECOVERY_COMPASS(): Item
    {
        self::init();
        if (!isset(self::$members['RECOVERY_COMPASS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RECOVERY_COMPASS");
        }
        return clone self::$members['RECOVERY_COMPASS'];
    }

    public static function REDSTONE_DUST(): Redstone
    {
        self::init();
        if (!isset(self::$members['REDSTONE_DUST'])) {
            throw new \InvalidArgumentException("Unknown vanilla item REDSTONE_DUST");
        }
        return clone self::$members['REDSTONE_DUST'];
    }

    public static function RESIN_BRICK(): Item
    {
        self::init();
        if (!isset(self::$members['RESIN_BRICK'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RESIN_BRICK");
        }
        return clone self::$members['RESIN_BRICK'];
    }

    public static function RIB_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['RIB_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item RIB_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['RIB_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function ROTTEN_FLESH(): RottenFlesh
    {
        self::init();
        if (!isset(self::$members['ROTTEN_FLESH'])) {
            throw new \InvalidArgumentException("Unknown vanilla item ROTTEN_FLESH");
        }
        return clone self::$members['ROTTEN_FLESH'];
    }

    public static function SCUTE(): Item
    {
        self::init();
        if (!isset(self::$members['SCUTE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SCUTE");
        }
        return clone self::$members['SCUTE'];
    }

    public static function SENTRY_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['SENTRY_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SENTRY_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['SENTRY_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function SHAPER_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['SHAPER_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SHAPER_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['SHAPER_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function SHEARS(): Shears
    {
        self::init();
        if (!isset(self::$members['SHEARS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SHEARS");
        }
        return clone self::$members['SHEARS'];
    }

    public static function SHULKER_SHELL(): Item
    {
        self::init();
        if (!isset(self::$members['SHULKER_SHELL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SHULKER_SHELL");
        }
        return clone self::$members['SHULKER_SHELL'];
    }

    public static function SILENCE_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['SILENCE_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SILENCE_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['SILENCE_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function SLIMEBALL(): Item
    {
        self::init();
        if (!isset(self::$members['SLIMEBALL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SLIMEBALL");
        }
        return clone self::$members['SLIMEBALL'];
    }

    public static function SNOUT_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['SNOUT_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SNOUT_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['SNOUT_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function SNOWBALL(): Snowball
    {
        self::init();
        if (!isset(self::$members['SNOWBALL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SNOWBALL");
        }
        return clone self::$members['SNOWBALL'];
    }

    public static function SPIDER_EYE(): SpiderEye
    {
        self::init();
        if (!isset(self::$members['SPIDER_EYE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SPIDER_EYE");
        }
        return clone self::$members['SPIDER_EYE'];
    }

    public static function SPIRE_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['SPIRE_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SPIRE_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['SPIRE_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function SPLASH_POTION(): SplashPotion
    {
        self::init();
        if (!isset(self::$members['SPLASH_POTION'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SPLASH_POTION");
        }
        return clone self::$members['SPLASH_POTION'];
    }

    public static function SPRUCE_BOAT(): Boat
    {
        self::init();
        if (!isset(self::$members['SPRUCE_BOAT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SPRUCE_BOAT");
        }
        return clone self::$members['SPRUCE_BOAT'];
    }

    public static function SPRUCE_HANGING_SIGN(): HangingSign
    {
        self::init();
        if (!isset(self::$members['SPRUCE_HANGING_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SPRUCE_HANGING_SIGN");
        }
        return clone self::$members['SPRUCE_HANGING_SIGN'];
    }

    public static function SPRUCE_SIGN(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['SPRUCE_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SPRUCE_SIGN");
        }
        return clone self::$members['SPRUCE_SIGN'];
    }

    public static function SPYGLASS(): Spyglass
    {
        self::init();
        if (!isset(self::$members['SPYGLASS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SPYGLASS");
        }
        return clone self::$members['SPYGLASS'];
    }

    public static function SQUID_SPAWN_EGG(): SpawnEgg
    {
        self::init();
        if (!isset(self::$members['SQUID_SPAWN_EGG'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SQUID_SPAWN_EGG");
        }
        return clone self::$members['SQUID_SPAWN_EGG'];
    }

    public static function STEAK(): Steak
    {
        self::init();
        if (!isset(self::$members['STEAK'])) {
            throw new \InvalidArgumentException("Unknown vanilla item STEAK");
        }
        return clone self::$members['STEAK'];
    }

    public static function STICK(): Stick
    {
        self::init();
        if (!isset(self::$members['STICK'])) {
            throw new \InvalidArgumentException("Unknown vanilla item STICK");
        }
        return clone self::$members['STICK'];
    }

    public static function STONE_AXE(): Axe
    {
        self::init();
        if (!isset(self::$members['STONE_AXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item STONE_AXE");
        }
        return clone self::$members['STONE_AXE'];
    }

    public static function STONE_HOE(): Hoe
    {
        self::init();
        if (!isset(self::$members['STONE_HOE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item STONE_HOE");
        }
        return clone self::$members['STONE_HOE'];
    }

    public static function STONE_PICKAXE(): Pickaxe
    {
        self::init();
        if (!isset(self::$members['STONE_PICKAXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item STONE_PICKAXE");
        }
        return clone self::$members['STONE_PICKAXE'];
    }

    public static function STONE_SHOVEL(): Shovel
    {
        self::init();
        if (!isset(self::$members['STONE_SHOVEL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item STONE_SHOVEL");
        }
        return clone self::$members['STONE_SHOVEL'];
    }

    public static function STONE_SWORD(): Sword
    {
        self::init();
        if (!isset(self::$members['STONE_SWORD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item STONE_SWORD");
        }
        return clone self::$members['STONE_SWORD'];
    }

    public static function STRING(): StringItem
    {
        self::init();
        if (!isset(self::$members['STRING'])) {
            throw new \InvalidArgumentException("Unknown vanilla item STRING");
        }
        return clone self::$members['STRING'];
    }

    public static function SUGAR(): Item
    {
        self::init();
        if (!isset(self::$members['SUGAR'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SUGAR");
        }
        return clone self::$members['SUGAR'];
    }

    public static function SUSPICIOUS_STEW(): SuspiciousStew
    {
        self::init();
        if (!isset(self::$members['SUSPICIOUS_STEW'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SUSPICIOUS_STEW");
        }
        return clone self::$members['SUSPICIOUS_STEW'];
    }

    public static function SWEET_BERRIES(): SweetBerries
    {
        self::init();
        if (!isset(self::$members['SWEET_BERRIES'])) {
            throw new \InvalidArgumentException("Unknown vanilla item SWEET_BERRIES");
        }
        return clone self::$members['SWEET_BERRIES'];
    }

    public static function TIDE_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['TIDE_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item TIDE_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['TIDE_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function TORCHFLOWER_SEEDS(): TorchflowerSeeds
    {
        self::init();
        if (!isset(self::$members['TORCHFLOWER_SEEDS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item TORCHFLOWER_SEEDS");
        }
        return clone self::$members['TORCHFLOWER_SEEDS'];
    }

    public static function TOTEM(): Totem
    {
        self::init();
        if (!isset(self::$members['TOTEM'])) {
            throw new \InvalidArgumentException("Unknown vanilla item TOTEM");
        }
        return clone self::$members['TOTEM'];
    }

    public static function TRIDENT(): Trident
    {
        self::init();
        if (!isset(self::$members['TRIDENT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item TRIDENT");
        }
        return clone self::$members['TRIDENT'];
    }

    public static function TURTLE_HELMET(): TurtleHelmet
    {
        self::init();
        if (!isset(self::$members['TURTLE_HELMET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item TURTLE_HELMET");
        }
        return clone self::$members['TURTLE_HELMET'];
    }

    public static function VEX_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['VEX_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item VEX_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['VEX_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function VILLAGER_SPAWN_EGG(): SpawnEgg
    {
        self::init();
        if (!isset(self::$members['VILLAGER_SPAWN_EGG'])) {
            throw new \InvalidArgumentException("Unknown vanilla item VILLAGER_SPAWN_EGG");
        }
        return clone self::$members['VILLAGER_SPAWN_EGG'];
    }

    public static function WARD_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['WARD_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WARD_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['WARD_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function WARPED_HANGING_SIGN(): HangingSign
    {
        self::init();
        if (!isset(self::$members['WARPED_HANGING_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WARPED_HANGING_SIGN");
        }
        return clone self::$members['WARPED_HANGING_SIGN'];
    }

    public static function WARPED_SIGN(): ItemBlockWallOrFloor
    {
        self::init();
        if (!isset(self::$members['WARPED_SIGN'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WARPED_SIGN");
        }
        return clone self::$members['WARPED_SIGN'];
    }

    public static function WATER_BUCKET(): LiquidBucket
    {
        self::init();
        if (!isset(self::$members['WATER_BUCKET'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WATER_BUCKET");
        }
        return clone self::$members['WATER_BUCKET'];
    }

    public static function WAYFINDER_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['WAYFINDER_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WAYFINDER_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['WAYFINDER_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function WHEAT(): Item
    {
        self::init();
        if (!isset(self::$members['WHEAT'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WHEAT");
        }
        return clone self::$members['WHEAT'];
    }

    public static function WHEAT_SEEDS(): WheatSeeds
    {
        self::init();
        if (!isset(self::$members['WHEAT_SEEDS'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WHEAT_SEEDS");
        }
        return clone self::$members['WHEAT_SEEDS'];
    }

    public static function WILD_ARMOR_TRIM_SMITHING_TEMPLATE(): Item
    {
        self::init();
        if (!isset(self::$members['WILD_ARMOR_TRIM_SMITHING_TEMPLATE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WILD_ARMOR_TRIM_SMITHING_TEMPLATE");
        }
        return clone self::$members['WILD_ARMOR_TRIM_SMITHING_TEMPLATE'];
    }

    public static function WOODEN_AXE(): Axe
    {
        self::init();
        if (!isset(self::$members['WOODEN_AXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WOODEN_AXE");
        }
        return clone self::$members['WOODEN_AXE'];
    }

    public static function WOODEN_HOE(): Hoe
    {
        self::init();
        if (!isset(self::$members['WOODEN_HOE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WOODEN_HOE");
        }
        return clone self::$members['WOODEN_HOE'];
    }

    public static function WOODEN_PICKAXE(): Pickaxe
    {
        self::init();
        if (!isset(self::$members['WOODEN_PICKAXE'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WOODEN_PICKAXE");
        }
        return clone self::$members['WOODEN_PICKAXE'];
    }

    public static function WOODEN_SHOVEL(): Shovel
    {
        self::init();
        if (!isset(self::$members['WOODEN_SHOVEL'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WOODEN_SHOVEL");
        }
        return clone self::$members['WOODEN_SHOVEL'];
    }

    public static function WOODEN_SWORD(): Sword
    {
        self::init();
        if (!isset(self::$members['WOODEN_SWORD'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WOODEN_SWORD");
        }
        return clone self::$members['WOODEN_SWORD'];
    }

    public static function WRITABLE_BOOK(): WritableBook
    {
        self::init();
        if (!isset(self::$members['WRITABLE_BOOK'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WRITABLE_BOOK");
        }
        return clone self::$members['WRITABLE_BOOK'];
    }

    public static function WRITTEN_BOOK(): WrittenBook
    {
        self::init();
        if (!isset(self::$members['WRITTEN_BOOK'])) {
            throw new \InvalidArgumentException("Unknown vanilla item WRITTEN_BOOK");
        }
        return clone self::$members['WRITTEN_BOOK'];
    }

    public static function ZOMBIE_SPAWN_EGG(): SpawnEgg
    {
        self::init();
        if (!isset(self::$members['ZOMBIE_SPAWN_EGG'])) {
            throw new \InvalidArgumentException("Unknown vanilla item ZOMBIE_SPAWN_EGG");
        }
        return clone self::$members['ZOMBIE_SPAWN_EGG'];
    }

    public static function __callStatic(string $name, array $args): Item
    {
        self::init();
        $key = mb_strtoupper($name);
        if (isset(self::$members[$key])) {
            return clone self::$members[$key];
        }
        throw new \InvalidArgumentException("Unknown vanilla item " . $name);
    }
}
