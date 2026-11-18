<?php

declare(strict_types=1);

namespace pocketmine\item;

use pocketmine\block\utils\RecordType;
use pocketmine\block\VanillaBlocks as Blocks;
use pocketmine\entity\Entity;
use pocketmine\entity\Location;
use pocketmine\entity\Squid;
use pocketmine\entity\Villager;
use pocketmine\entity\Zombie;
use pocketmine\inventory\ArmorInventory;
use pocketmine\item\enchantment\ItemEnchantmentTags as EnchantmentTags;
use pocketmine\item\ItemIdentifier as IID;
use pocketmine\item\VanillaArmorMaterials as ArmorMaterials;
use pocketmine\math\Vector3;
use pocketmine\utils\RegistrySource;
use pocketmine\world\World;

/**
 * Fuente de registro para items vanilla de PM5.
 */
final class VanillaItemsInputs extends RegistrySource
{

	public function getTargetClassName() : string{
		return "VanillaItems";
	}

	public function getTargetClassDocComment() : array{
		return [
			"Allows getting a new instance of any item implemented by PocketMine-MP",
			"Every item here also has a constant of the same name in {@link ItemTypeIds} to enable items to be identified"
		];
	}

	public function cloneResults() : bool{ return true; }

	private static function makeIID(string $name) : IID{
		//this sketchy hack allows us to avoid manually writing the constants inline
		//since type IDs are generated from this class anyway, I'm OK with this hack
		//nonetheless, we should try to get rid of it in a future major version (e.g by using string type IDs)
		$reflect = new \ReflectionClass(ItemTypeIds::class);
		$typeId = $reflect->getConstant(mb_strtoupper($name));
		if(!is_int($typeId)){
			//this allows registering new stuff without adding new type ID constants
			//this reduces the number of mandatory steps to test new features in local development
			\GlobalLogger::get()->error(self::class . ": No constant type ID found for $name, generating a new one");
			$typeId = ItemTypeIds::newId();
		}

		return new IID($typeId);
	}

	/**
	 * @phpstan-template TItem of Item
	 * @phpstan-param \Closure(IID) : TItem $createItem
	 * @phpstan-return TItem
	 */
	protected function register(string $name, \Closure $createItem) : Item{
		$item = $createItem(self::makeIID($name));
		self::registerValue($name, $item);
		return $item;
	}

	protected function setup() : void{
		self::registerArmorItems();
		self::registerSpawnEggs();
		self::registerTierToolItems();
		self::registerSmithingTemplates();

		//this doesn't use the regular register() because it doesn't have an item typeID
		//in the future we'll probably want to dissociate this from the air block and make a proper null item
		self::registerDelayed("air", fn() : Item => Blocks::AIR()->asItem()->setCount(0));

		self::registerDelayed("acacia_sign", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::ACACIA_SIGN(), Blocks::ACACIA_WALL_SIGN()));
		self::registerDelayed("acacia_hanging_sign", fn(string $name) : HangingSign => new HangingSign(self::makeIID($name), "Acacia Hanging Sign", Blocks::ACACIA_CEILING_CENTER_HANGING_SIGN(), Blocks::ACACIA_CEILING_EDGES_HANGING_SIGN(), Blocks::ACACIA_WALL_HANGING_SIGN()));
		self::register("amethyst_shard", fn(IID $id) => new Item($id, "Amethyst Shard"));
		self::register("apple", fn(IID $id) => new Apple($id, "Apple"));
		self::register("arrow", fn(IID $id) => new Arrow($id, "Arrow"));
		self::register("baked_potato", fn(IID $id) => new BakedPotato($id, "Baked Potato"));
		self::register("bamboo", fn(IID $id) => new Bamboo($id, "Bamboo"));
		self::registerDelayed("bamboo_sign", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::BAMBOO_SIGN(), Blocks::BAMBOO_WALL_SIGN()));
		self::registerDelayed("bamboo_hanging_sign", fn(string $name) : HangingSign => new HangingSign(self::makeIID($name), "Bamboo Hanging Sign", Blocks::BAMBOO_CEILING_CENTER_HANGING_SIGN(), Blocks::BAMBOO_CEILING_EDGES_HANGING_SIGN(), Blocks::BAMBOO_WALL_HANGING_SIGN()));
		self::registerDelayed("banner", fn(string $name) : Banner => new Banner(self::makeIID($name), Blocks::BANNER(), Blocks::WALL_BANNER()));
		self::register("beetroot", fn(IID $id) => new Beetroot($id, "Beetroot"));
		self::register("beetroot_seeds", fn(IID $id) => new BeetrootSeeds($id, "Beetroot Seeds"));
		self::register("beetroot_soup", fn(IID $id) => new BeetrootSoup($id, "Beetroot Soup"));
		self::registerDelayed("birch_sign", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::BIRCH_SIGN(), Blocks::BIRCH_WALL_SIGN()));
		self::registerDelayed("birch_hanging_sign", fn(string $name) : HangingSign => new HangingSign(self::makeIID($name), "Birch Hanging Sign", Blocks::BIRCH_CEILING_CENTER_HANGING_SIGN(), Blocks::BIRCH_CEILING_EDGES_HANGING_SIGN(), Blocks::BIRCH_WALL_HANGING_SIGN()));
		self::register("blaze_powder", fn(IID $id) => new Item($id, "Blaze Powder"));
		self::register("blaze_rod", fn(IID $id) => new BlazeRod($id, "Blaze Rod"));
		self::register("bleach", fn(IID $id) => new Item($id, "Bleach"));
		self::register("bone", fn(IID $id) => new Item($id, "Bone"));
		self::register("bone_meal", fn(IID $id) => new Fertilizer($id, "Bone Meal"));
		self::register("book", fn(IID $id) => new Book($id, "Book", [EnchantmentTags::ALL]));
		self::register("bow", fn(IID $id) => new Bow($id, "Bow", [EnchantmentTags::BOW]));
		self::register("bowl", fn(IID $id) => new Bowl($id, "Bowl"));
		self::register("bread", fn(IID $id) => new Bread($id, "Bread"));
		self::register("brick", fn(IID $id) => new Item($id, "Brick"));
		self::register("bucket", fn(IID $id) => new Bucket($id, "Bucket"));
		self::register("carrot", fn(IID $id) => new Carrot($id, "Carrot"));
		self::register("charcoal", fn(IID $id) => new Coal($id, "Charcoal"));
		self::registerDelayed("cherry_sign", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::CHERRY_SIGN(), Blocks::CHERRY_WALL_SIGN()));
		self::registerDelayed("cherry_hanging_sign", fn(string $name) : HangingSign => new HangingSign(self::makeIID($name), "Cherry Hanging Sign", Blocks::CHERRY_CEILING_CENTER_HANGING_SIGN(), Blocks::CHERRY_CEILING_EDGES_HANGING_SIGN(), Blocks::CHERRY_WALL_HANGING_SIGN()));
		self::register("chemical_aluminium_oxide", fn(IID $id) => new Item($id, "Aluminium Oxide"));
		self::register("chemical_ammonia", fn(IID $id) => new Item($id, "Ammonia"));
		self::register("chemical_barium_sulphate", fn(IID $id) => new Item($id, "Barium Sulphate"));
		self::register("chemical_benzene", fn(IID $id) => new Item($id, "Benzene"));
		self::register("chemical_boron_trioxide", fn(IID $id) => new Item($id, "Boron Trioxide"));
		self::register("chemical_calcium_bromide", fn(IID $id) => new Item($id, "Calcium Bromide"));
		self::register("chemical_calcium_chloride", fn(IID $id) => new Item($id, "Calcium Chloride"));
		self::register("chemical_cerium_chloride", fn(IID $id) => new Item($id, "Cerium Chloride"));
		self::register("chemical_charcoal", fn(IID $id) => new Item($id, "Charcoal"));
		self::register("chemical_crude_oil", fn(IID $id) => new Item($id, "Crude Oil"));
		self::register("chemical_glue", fn(IID $id) => new Item($id, "Glue"));
		self::register("chemical_hydrogen_peroxide", fn(IID $id) => new Item($id, "Hydrogen Peroxide"));
		self::register("chemical_hypochlorite", fn(IID $id) => new Item($id, "Hypochlorite"));
		self::register("chemical_ink", fn(IID $id) => new Item($id, "Ink"));
		self::register("chemical_iron_sulphide", fn(IID $id) => new Item($id, "Iron Sulphide"));
		self::register("chemical_latex", fn(IID $id) => new Item($id, "Latex"));
		self::register("chemical_lithium_hydride", fn(IID $id) => new Item($id, "Lithium Hydride"));
		self::register("chemical_luminol", fn(IID $id) => new Item($id, "Luminol"));
		self::register("chemical_magnesium_nitrate", fn(IID $id) => new Item($id, "Magnesium Nitrate"));
		self::register("chemical_magnesium_oxide", fn(IID $id) => new Item($id, "Magnesium Oxide"));
		self::register("chemical_magnesium_salts", fn(IID $id) => new Item($id, "Magnesium Salts"));
		self::register("chemical_mercuric_chloride", fn(IID $id) => new Item($id, "Mercuric Chloride"));
		self::register("chemical_polyethylene", fn(IID $id) => new Item($id, "Polyethylene"));
		self::register("chemical_potassium_chloride", fn(IID $id) => new Item($id, "Potassium Chloride"));
		self::register("chemical_potassium_iodide", fn(IID $id) => new Item($id, "Potassium Iodide"));
		self::register("chemical_rubbish", fn(IID $id) => new Item($id, "Rubbish"));
		self::register("chemical_salt", fn(IID $id) => new Item($id, "Salt"));
		self::register("chemical_soap", fn(IID $id) => new Item($id, "Soap"));
		self::register("chemical_sodium_acetate", fn(IID $id) => new Item($id, "Sodium Acetate"));
		self::register("chemical_sodium_fluoride", fn(IID $id) => new Item($id, "Sodium Fluoride"));
		self::register("chemical_sodium_hydride", fn(IID $id) => new Item($id, "Sodium Hydride"));
		self::register("chemical_sodium_hydroxide", fn(IID $id) => new Item($id, "Sodium Hydroxide"));
		self::register("chemical_sodium_hypochlorite", fn(IID $id) => new Item($id, "Sodium Hypochlorite"));
		self::register("chemical_sodium_oxide", fn(IID $id) => new Item($id, "Sodium Oxide"));
		self::register("chemical_sugar", fn(IID $id) => new Item($id, "Sugar"));
		self::register("chemical_sulphate", fn(IID $id) => new Item($id, "Sulphate"));
		self::register("chemical_tungsten_chloride", fn(IID $id) => new Item($id, "Tungsten Chloride"));
		self::register("chemical_water", fn(IID $id) => new Item($id, "Water"));
		self::register("chorus_fruit", fn(IID $id) => new ChorusFruit($id, "Chorus Fruit"));
		self::register("clay", fn(IID $id) => new Item($id, "Clay"));
		self::register("clock", fn(IID $id) => new Clock($id, "Clock"));
		self::register("clownfish", fn(IID $id) => new Clownfish($id, "Clownfish"));
		self::register("coal", fn(IID $id) => new Coal($id, "Coal"));
		self::register("cocoa_beans", fn(IID $id) => new CocoaBeans($id, "Cocoa Beans"));
		self::register("compass", fn(IID $id) => new Compass($id, "Compass", [EnchantmentTags::COMPASS]));
		self::register("cooked_chicken", fn(IID $id) => new CookedChicken($id, "Cooked Chicken"));
		self::register("cooked_fish", fn(IID $id) => new CookedFish($id, "Cooked Fish"));
		self::register("cooked_mutton", fn(IID $id) => new CookedMutton($id, "Cooked Mutton"));
		self::register("cooked_porkchop", fn(IID $id) => new CookedPorkchop($id, "Cooked Porkchop"));
		self::register("cooked_rabbit", fn(IID $id) => new CookedRabbit($id, "Cooked Rabbit"));
		self::register("cooked_salmon", fn(IID $id) => new CookedSalmon($id, "Cooked Salmon"));
		self::register("cookie", fn(IID $id) => new Cookie($id, "Cookie"));
		self::register("copper_ingot", fn(IID $id) => new Item($id, "Copper Ingot"));
		self::register("copper_nugget", fn(IID $id) => new Item($id, "Copper Nugget"));
		self::registerDelayed("coral_fan", fn(string $name) : CoralFan => new CoralFan(self::makeIID($name))); //uses VanillaBlocks in constructor :(
		self::registerDelayed("crimson_sign", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::CRIMSON_SIGN(), Blocks::CRIMSON_WALL_SIGN()));
		self::registerDelayed("crimson_hanging_sign", fn(string $name) : HangingSign => new HangingSign(self::makeIID($name), "Crimson Hanging Sign", Blocks::CRIMSON_CEILING_CENTER_HANGING_SIGN(), Blocks::CRIMSON_CEILING_EDGES_HANGING_SIGN(), Blocks::CRIMSON_WALL_HANGING_SIGN()));
		self::registerDelayed("dark_oak_sign", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::DARK_OAK_SIGN(), Blocks::DARK_OAK_WALL_SIGN()));
		self::registerDelayed("dark_oak_hanging_sign", fn(string $name) : HangingSign => new HangingSign(self::makeIID($name), "Dark Oak Hanging Sign", Blocks::DARK_OAK_CEILING_CENTER_HANGING_SIGN(), Blocks::DARK_OAK_CEILING_EDGES_HANGING_SIGN(), Blocks::DARK_OAK_WALL_HANGING_SIGN()));
		self::register("diamond", fn(IID $id) => new Item($id, "Diamond"));
		self::register("disc_fragment_5", fn(IID $id) => new Item($id, "Disc Fragment (5)"));
		self::register("dragon_breath", fn(IID $id) => new Item($id, "Dragon's Breath"));
		self::register("dried_kelp", fn(IID $id) => new DriedKelp($id, "Dried Kelp"));
		//TODO: add interface to dye-colour objects
		self::register("dye", fn(IID $id) => new Dye($id, "Dye"));
		self::register("echo_shard", fn(IID $id) => new Item($id, "Echo Shard"));
		self::register("egg", fn(IID $id) => new Egg($id, "Egg"));
		self::register("emerald", fn(IID $id) => new Item($id, "Emerald"));
		self::register("enchanted_book", fn(IID $id) => new EnchantedBook($id, "Enchanted Book", [EnchantmentTags::ALL]));
		self::register("enchanted_golden_apple", fn(IID $id) => new GoldenAppleEnchanted($id, "Enchanted Golden Apple"));
		self::register("end_crystal", fn(IID $id) => new EndCrystal($id, "End Crystal"));
		self::register("ender_pearl", fn(IID $id) => new EnderPearl($id, "Ender Pearl"));
		self::register("experience_bottle", fn(IID $id) => new ExperienceBottle($id, "Bottle o' Enchanting"));
		self::register("feather", fn(IID $id) => new Item($id, "Feather"));
		self::register("fermented_spider_eye", fn(IID $id) => new Item($id, "Fermented Spider Eye"));
		self::register("firework_rocket", fn(IID $id) => new FireworkRocket($id, "Firework Rocket"));
		self::register("firework_star", fn(IID $id) => new FireworkStar($id, "Firework Star"));
		self::register("fire_charge", fn(IID $id) => new FireCharge($id, "Fire Charge"));
		self::register("fishing_rod", fn(IID $id) => new FishingRod($id, "Fishing Rod", [EnchantmentTags::FISHING_ROD]));
		self::register("flint", fn(IID $id) => new Item($id, "Flint"));
		self::register("flint_and_steel", fn(IID $id) => new FlintSteel($id, "Flint and Steel", [EnchantmentTags::FLINT_AND_STEEL]));
		self::register("ghast_tear", fn(IID $id) => new Item($id, "Ghast Tear"));
		self::register("glass_bottle", fn(IID $id) => new GlassBottle($id, "Glass Bottle"));
		self::register("glistering_melon", fn(IID $id) => new Item($id, "Glistering Melon"));
		self::register("glow_berries", fn(IID $id) => new GlowBerries($id, "Glow Berries"));
		self::register("glow_ink_sac", fn(IID $id) => new Item($id, "Glow Ink Sac"));
		self::register("glowstone_dust", fn(IID $id) => new Item($id, "Glowstone Dust"));
		self::register("goat_horn", fn(IID $id) => new GoatHorn($id, "Goat Horn"));
		self::register("gold_ingot", fn(IID $id) => new Item($id, "Gold Ingot"));
		self::register("gold_nugget", fn(IID $id) => new Item($id, "Gold Nugget"));
		self::register("golden_apple", fn(IID $id) => new GoldenApple($id, "Golden Apple"));
		self::register("golden_carrot", fn(IID $id) => new GoldenCarrot($id, "Golden Carrot"));
		self::register("gunpowder", fn(IID $id) => new Item($id, "Gunpowder"));
		self::register("heart_of_the_sea", fn(IID $id) => new Item($id, "Heart of the Sea"));
		self::register("honey_bottle", fn(IID $id) => new HoneyBottle($id, "Honey Bottle"));
		self::register("honeycomb", fn(IID $id) => new Item($id, "Honeycomb"));
		self::register("ice_bomb", fn(IID $id) => new IceBomb($id, "Ice Bomb"));
		self::register("ink_sac", fn(IID $id) => new Item($id, "Ink Sac"));
		self::register("iron_ingot", fn(IID $id) => new Item($id, "Iron Ingot"));
		self::register("iron_nugget", fn(IID $id) => new Item($id, "Iron Nugget"));
		self::registerDelayed("jungle_sign", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::JUNGLE_SIGN(), Blocks::JUNGLE_WALL_SIGN()));
		self::registerDelayed("jungle_hanging_sign", fn(string $name) : HangingSign => new HangingSign(self::makeIID($name), "Jungle Hanging Sign", Blocks::JUNGLE_CEILING_CENTER_HANGING_SIGN(), Blocks::JUNGLE_CEILING_EDGES_HANGING_SIGN(), Blocks::JUNGLE_WALL_HANGING_SIGN()));
		self::register("lapis_lazuli", fn(IID $id) => new Item($id, "Lapis Lazuli"));
		self::registerDelayed("lava_bucket", fn(string $name) : LiquidBucket => new LiquidBucket(self::makeIID($name), "Lava Bucket", Blocks::LAVA()));
		self::register("leather", fn(IID $id) => new Item($id, "Leather"));
		self::register("lingering_potion", fn(IID $id) => new SplashPotion($id, "Lingering Potion", linger: true));
		self::register("magma_cream", fn(IID $id) => new Item($id, "Magma Cream"));
		self::registerDelayed("mangrove_sign", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::MANGROVE_SIGN(), Blocks::MANGROVE_WALL_SIGN()));
		self::registerDelayed("mangrove_hanging_sign", fn(string $name) : HangingSign => new HangingSign(self::makeIID($name), "Mangrove Hanging Sign", Blocks::MANGROVE_CEILING_CENTER_HANGING_SIGN(), Blocks::MANGROVE_CEILING_EDGES_HANGING_SIGN(), Blocks::MANGROVE_WALL_HANGING_SIGN()));
		self::register("medicine", fn(IID $id) => new Medicine($id, "Medicine"));
		self::register("melon", fn(IID $id) => new Melon($id, "Melon"));
		self::register("melon_seeds", fn(IID $id) => new MelonSeeds($id, "Melon Seeds"));
		self::register("milk_bucket", fn(IID $id) => new MilkBucket($id, "Milk Bucket"));
		self::register("minecart", fn(IID $id) => new Minecart($id, "Minecart"));
		self::register("mushroom_stew", fn(IID $id) => new MushroomStew($id, "Mushroom Stew"));
		self::register("name_tag", fn(IID $id) => new NameTag($id, "Name Tag"));
		self::register("nautilus_shell", fn(IID $id) => new Item($id, "Nautilus Shell"));
		self::register("nether_brick", fn(IID $id) => new Item($id, "Nether Brick"));
		self::register("nether_quartz", fn(IID $id) => new Item($id, "Nether Quartz"));
		self::register("nether_star", fn(IID $id) => new Item($id, "Nether Star"));
		self::register("netherite_ingot", fn(IID $id) => new class($id, "Netherite Ingot") extends Item{
			public function isFireProof() : bool{ return true; }
		});
		self::register("netherite_scrap", fn(IID $id) => new class($id, "Netherite Scrap") extends Item{
			public function isFireProof() : bool{ return true; }
		});
		self::registerDelayed("oak_sign", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::OAK_SIGN(), Blocks::OAK_WALL_SIGN()));
		self::registerDelayed("oak_hanging_sign", fn(string $name) : HangingSign => new HangingSign(self::makeIID($name), "Oak Hanging Sign", Blocks::OAK_CEILING_CENTER_HANGING_SIGN(), Blocks::OAK_CEILING_EDGES_HANGING_SIGN(), Blocks::OAK_WALL_HANGING_SIGN()));
		self::registerDelayed("ominous_banner", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::OMINOUS_BANNER(), Blocks::OMINOUS_WALL_BANNER()));
		self::register("painting", fn(IID $id) => new PaintingItem($id, "Painting"));
		self::registerDelayed("pale_oak_sign", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::PALE_OAK_SIGN(), Blocks::PALE_OAK_WALL_SIGN()));
		self::registerDelayed("pale_oak_hanging_sign", fn(string $name) : HangingSign => new HangingSign(self::makeIID($name), "Pale Oak Hanging Sign", Blocks::PALE_OAK_CEILING_CENTER_HANGING_SIGN(), Blocks::PALE_OAK_CEILING_EDGES_HANGING_SIGN(), Blocks::PALE_OAK_WALL_HANGING_SIGN()));
		self::register("paper", fn(IID $id) => new Item($id, "Paper"));
		self::register("phantom_membrane", fn(IID $id) => new Item($id, "Phantom Membrane"));
		self::register("pitcher_pod", fn(IID $id) => new PitcherPod($id, "Pitcher Pod"));
		self::register("poisonous_potato", fn(IID $id) => new PoisonousPotato($id, "Poisonous Potato"));
		self::register("popped_chorus_fruit", fn(IID $id) => new Item($id, "Popped Chorus Fruit"));
		self::register("potato", fn(IID $id) => new Potato($id, "Potato"));
		self::register("potion", fn(IID $id) => new Potion($id, "Potion"));
		self::register("prismarine_crystals", fn(IID $id) => new Item($id, "Prismarine Crystals"));
		self::register("prismarine_shard", fn(IID $id) => new Item($id, "Prismarine Shard"));
		self::register("pufferfish", fn(IID $id) => new Pufferfish($id, "Pufferfish"));
		self::register("pumpkin_pie", fn(IID $id) => new PumpkinPie($id, "Pumpkin Pie"));
		self::register("pumpkin_seeds", fn(IID $id) => new PumpkinSeeds($id, "Pumpkin Seeds"));
		self::register("rabbit_foot", fn(IID $id) => new Item($id, "Rabbit's Foot"));
		self::register("rabbit_hide", fn(IID $id) => new Item($id, "Rabbit Hide"));
		self::register("rabbit_stew", fn(IID $id) => new RabbitStew($id, "Rabbit Stew"));
		self::register("raw_beef", fn(IID $id) => new RawBeef($id, "Raw Beef"));
		self::register("raw_chicken", fn(IID $id) => new RawChicken($id, "Raw Chicken"));
		self::register("raw_copper", fn(IID $id) => new Item($id, "Raw Copper"));
		self::register("raw_fish", fn(IID $id) => new RawFish($id, "Raw Fish"));
		self::register("raw_gold", fn(IID $id) => new Item($id, "Raw Gold"));
		self::register("raw_iron", fn(IID $id) => new Item($id, "Raw Iron"));
		self::register("raw_mutton", fn(IID $id) => new RawMutton($id, "Raw Mutton"));
		self::register("raw_porkchop", fn(IID $id) => new RawPorkchop($id, "Raw Porkchop"));
		self::register("raw_rabbit", fn(IID $id) => new RawRabbit($id, "Raw Rabbit"));
		self::register("raw_salmon", fn(IID $id) => new RawSalmon($id, "Raw Salmon"));
		self::register("record_11", fn(IID $id) => new Record($id, RecordType::DISK_11, "Record 11"));
		self::register("record_13", fn(IID $id) => new Record($id, RecordType::DISK_13, "Record 13"));
		self::register("record_5", fn(IID $id) => new Record($id, RecordType::DISK_5, "Record 5"));
		self::register("record_blocks", fn(IID $id) => new Record($id, RecordType::DISK_BLOCKS, "Record Blocks"));
		self::register("record_cat", fn(IID $id) => new Record($id, RecordType::DISK_CAT, "Record Cat"));
		self::register("record_chirp", fn(IID $id) => new Record($id, RecordType::DISK_CHIRP, "Record Chirp"));
		self::register("record_creator", fn(IID $id) => new Record($id, RecordType::DISK_CREATOR, "Record Creator"));
		self::register("record_creator_music_box", fn(IID $id) => new Record($id, RecordType::DISK_CREATOR_MUSIC_BOX, "Record Creator (Music Box)"));
		self::register("record_far", fn(IID $id) => new Record($id, RecordType::DISK_FAR, "Record Far"));
		self::register("record_lava_chicken", fn(IID $id) => new Record($id, RecordType::DISK_LAVA_CHICKEN, "Record Lava Chicken"));
		self::register("record_mall", fn(IID $id) => new Record($id, RecordType::DISK_MALL, "Record Mall"));
		self::register("record_mellohi", fn(IID $id) => new Record($id, RecordType::DISK_MELLOHI, "Record Mellohi"));
		self::register("record_otherside", fn(IID $id) => new Record($id, RecordType::DISK_OTHERSIDE, "Record Otherside"));
		self::register("record_pigstep", fn(IID $id) => new Record($id, RecordType::DISK_PIGSTEP, "Record Pigstep"));
		self::register("record_precipice", fn(IID $id) => new Record($id, RecordType::DISK_PRECIPICE, "Record Precipice"));
		self::register("record_relic", fn(IID $id) => new Record($id, RecordType::DISK_RELIC, "Record Relic"));
		self::register("record_stal", fn(IID $id) => new Record($id, RecordType::DISK_STAL, "Record Stal"));
		self::register("record_strad", fn(IID $id) => new Record($id, RecordType::DISK_STRAD, "Record Strad"));
		self::register("record_wait", fn(IID $id) => new Record($id, RecordType::DISK_WAIT, "Record Wait"));
		self::register("record_ward", fn(IID $id) => new Record($id, RecordType::DISK_WARD, "Record Ward"));
		self::register("recovery_compass", fn(IID $id) => new Item($id, "Recovery Compass"));
		self::register("redstone_dust", fn(IID $id) => new Redstone($id, "Redstone"));
		self::register("resin_brick", fn(IID $id) => new Item($id, "Resin Brick"));
		self::register("rotten_flesh", fn(IID $id) => new RottenFlesh($id, "Rotten Flesh"));
		self::register("scute", fn(IID $id) => new Item($id, "Scute"));
		self::register("shears", fn(IID $id) => new Shears($id, "Shears", [EnchantmentTags::SHEARS]));
		self::register("shulker_shell", fn(IID $id) => new Item($id, "Shulker Shell"));
		self::register("slimeball", fn(IID $id) => new Item($id, "Slimeball"));
		self::register("snowball", fn(IID $id) => new Snowball($id, "Snowball"));
		self::register("spider_eye", fn(IID $id) => new SpiderEye($id, "Spider Eye"));
		self::register("splash_potion", fn(IID $id) => new SplashPotion($id, "Splash Potion"));
		self::registerDelayed("spruce_sign", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::SPRUCE_SIGN(), Blocks::SPRUCE_WALL_SIGN()));
		self::registerDelayed("spruce_hanging_sign", fn(string $name) : HangingSign => new HangingSign(self::makeIID($name), "Spruce Hanging Sign", Blocks::SPRUCE_CEILING_CENTER_HANGING_SIGN(), Blocks::SPRUCE_CEILING_EDGES_HANGING_SIGN(), Blocks::SPRUCE_WALL_HANGING_SIGN()));
		self::register("spyglass", fn(IID $id) => new Spyglass($id, "Spyglass"));
		self::register("steak", fn(IID $id) => new Steak($id, "Steak"));
		self::register("stick", fn(IID $id) => new Stick($id, "Stick"));
		self::register("string", fn(IID $id) => new StringItem($id, "String"));
		self::register("sugar", fn(IID $id) => new Item($id, "Sugar"));
		self::register("suspicious_stew", fn(IID $id) => new SuspiciousStew($id, "Suspicious Stew"));
		self::register("sweet_berries", fn(IID $id) => new SweetBerries($id, "Sweet Berries"));
		self::register("torchflower_seeds", fn(IID $id) => new TorchflowerSeeds($id, "Torchflower Seeds"));
		self::register("totem", fn(IID $id) => new Totem($id, "Totem of Undying"));
		self::register("trident", fn(IID $id) => new Trident($id, "Trident"));
		self::registerDelayed("warped_sign", fn(string $name) : ItemBlockWallOrFloor => new ItemBlockWallOrFloor(self::makeIID($name), Blocks::WARPED_SIGN(), Blocks::WARPED_WALL_SIGN()));
		self::registerDelayed("warped_hanging_sign", fn(string $name) : HangingSign => new HangingSign(self::makeIID($name), "Warped Hanging Sign", Blocks::WARPED_CEILING_CENTER_HANGING_SIGN(), Blocks::WARPED_CEILING_EDGES_HANGING_SIGN(), Blocks::WARPED_WALL_HANGING_SIGN()));
		self::registerDelayed("water_bucket", fn(string $name) : LiquidBucket => new LiquidBucket(self::makeIID($name), "Water Bucket", Blocks::WATER()));
		self::register("wheat", fn(IID $id) => new Item($id, "Wheat"));
		self::register("wheat_seeds", fn(IID $id) => new WheatSeeds($id, "Wheat Seeds"));
		self::register("writable_book", fn(IID $id) => new WritableBook($id, "Book & Quill"));
		self::register("written_book", fn(IID $id) => new WrittenBook($id, "Written Book"));

		foreach(BoatType::cases() as $type){
			//boat type is static, because different types of wood may have different properties
			self::register(strtolower($type->name) . "_boat", fn(IID $id) => new Boat($id, $type->getDisplayName() . " Boat", $type));
		}
	}

	private function registerSpawnEggs() : void{
		self::register("zombie_spawn_egg", fn(IID $id) => new class($id, "Zombie Spawn Egg") extends SpawnEgg{
			protected function createEntity(World $world, Vector3 $pos, float $yaw, float $pitch) : Entity{
				return new Zombie(Location::fromObject($pos, $world, $yaw, $pitch));
			}
		});
		self::register("squid_spawn_egg", fn(IID $id) => new class($id, "Squid Spawn Egg") extends SpawnEgg{
			protected function createEntity(World $world, Vector3 $pos, float $yaw, float $pitch) : Entity{
				return new Squid(Location::fromObject($pos, $world, $yaw, $pitch));
			}
		});
		self::register("villager_spawn_egg", fn(IID $id) => new class($id, "Villager Spawn Egg") extends SpawnEgg{
			protected function createEntity(World $world, Vector3 $pos, float $yaw, float $pitch) : Entity{
				return new Villager(Location::fromObject($pos, $world, $yaw, $pitch));
			}
		});
	}

	private function registerTierToolItems() : void{
		foreach([
			[ToolTier::COPPER, "copper", "Copper"],
			[ToolTier::DIAMOND, "diamond", "Diamond"],
			[ToolTier::GOLD, "golden", "Golden"],
			[ToolTier::IRON, "iron", "Iron"],
			[ToolTier::NETHERITE, "netherite", "Netherite"],
			[ToolTier::STONE, "stone", "Stone"],
			[ToolTier::WOOD, "wooden", "Wooden"]
		] as [$tier, $idPrefix, $namePrefix]){
			self::register($idPrefix . "_axe", fn(IID $id) => new Axe($id, $namePrefix . " Axe", $tier, [EnchantmentTags::AXE]));
			self::register($idPrefix . "_hoe", fn(IID $id) => new Hoe($id, $namePrefix . " Hoe", $tier, [EnchantmentTags::HOE]));
			self::register($idPrefix . "_pickaxe", fn(IID $id) => new Pickaxe($id, $namePrefix . " Pickaxe", $tier, [EnchantmentTags::PICKAXE]));
			self::register($idPrefix . "_shovel", fn(IID $id) => new Shovel($id, $namePrefix . " Shovel", $tier, [EnchantmentTags::SHOVEL]));
			self::register($idPrefix . "_sword", fn(IID $id) => new Sword($id, $namePrefix . " Sword", $tier, [EnchantmentTags::SWORD]));
		}
	}

	private function registerArmorItems() : void{
		self::registerDelayed("chainmail_boots", fn($name) : Armor => new Armor(self::makeIID($name), "Chainmail Boots", new ArmorTypeInfo(1, 196, ArmorInventory::SLOT_FEET, material: ArmorMaterials::CHAINMAIL()), [EnchantmentTags::BOOTS]));
		self::registerDelayed("copper_boots", fn($name) : Armor => new Armor(self::makeIID($name), "Copper Boots", new ArmorTypeInfo(1, 144, ArmorInventory::SLOT_FEET, material: ArmorMaterials::COPPER()), [EnchantmentTags::BOOTS]));
		self::registerDelayed("diamond_boots", fn($name) : Armor => new Armor(self::makeIID($name), "Diamond Boots", new ArmorTypeInfo(3, 430, ArmorInventory::SLOT_FEET, 2, material: ArmorMaterials::DIAMOND()), [EnchantmentTags::BOOTS]));
		self::registerDelayed("golden_boots", fn($name) : Armor => new Armor(self::makeIID($name), "Golden Boots", new ArmorTypeInfo(1, 92, ArmorInventory::SLOT_FEET, material: ArmorMaterials::GOLD()), [EnchantmentTags::BOOTS]));
		self::registerDelayed("iron_boots", fn($name) : Armor => new Armor(self::makeIID($name), "Iron Boots", new ArmorTypeInfo(2, 196, ArmorInventory::SLOT_FEET, material: ArmorMaterials::IRON()), [EnchantmentTags::BOOTS]));
		self::registerDelayed("leather_boots", fn($name) : Armor => new Armor(self::makeIID($name), "Leather Boots", new ArmorTypeInfo(1, 66, ArmorInventory::SLOT_FEET, material: ArmorMaterials::LEATHER()), [EnchantmentTags::BOOTS]));
		self::registerDelayed("netherite_boots", fn($name) : Armor => new Armor(self::makeIID($name), "Netherite Boots", new ArmorTypeInfo(3, 482, ArmorInventory::SLOT_FEET, 3, true, material: ArmorMaterials::NETHERITE()), [EnchantmentTags::BOOTS]));

		self::registerDelayed("chainmail_chestplate", fn($name) : Armor => new Armor(self::makeIID($name), "Chainmail Chestplate", new ArmorTypeInfo(5, 241, ArmorInventory::SLOT_CHEST, material: ArmorMaterials::CHAINMAIL()), [EnchantmentTags::CHESTPLATE]));
		self::registerDelayed("copper_chestplate", fn($name) : Armor => new Armor(self::makeIID($name), "Copper Chestplate", new ArmorTypeInfo(4, 177, ArmorInventory::SLOT_CHEST, material: ArmorMaterials::COPPER()), [EnchantmentTags::CHESTPLATE]));
		self::registerDelayed("diamond_chestplate", fn($name) : Armor => new Armor(self::makeIID($name), "Diamond Chestplate", new ArmorTypeInfo(8, 529, ArmorInventory::SLOT_CHEST, 2, material: ArmorMaterials::DIAMOND()), [EnchantmentTags::CHESTPLATE]));
		self::registerDelayed("golden_chestplate", fn($name) : Armor => new Armor(self::makeIID($name), "Golden Chestplate", new ArmorTypeInfo(5, 113, ArmorInventory::SLOT_CHEST, material: ArmorMaterials::GOLD()), [EnchantmentTags::CHESTPLATE]));
		self::registerDelayed("iron_chestplate", fn($name) : Armor => new Armor(self::makeIID($name), "Iron Chestplate", new ArmorTypeInfo(6, 241, ArmorInventory::SLOT_CHEST, material: ArmorMaterials::IRON()), [EnchantmentTags::CHESTPLATE]));
		self::registerDelayed("leather_tunic", fn($name) : Armor => new Armor(self::makeIID($name), "Leather Tunic", new ArmorTypeInfo(3, 81, ArmorInventory::SLOT_CHEST, material: ArmorMaterials::LEATHER()), [EnchantmentTags::CHESTPLATE]));
		self::registerDelayed("netherite_chestplate", fn($name) : Armor => new Armor(self::makeIID($name), "Netherite Chestplate", new ArmorTypeInfo(8, 593, ArmorInventory::SLOT_CHEST, 3, true, material: ArmorMaterials::NETHERITE()), [EnchantmentTags::CHESTPLATE]));

		self::registerDelayed("chainmail_helmet", fn($name) : Armor => new Armor(self::makeIID($name), "Chainmail Helmet", new ArmorTypeInfo(2, 166, ArmorInventory::SLOT_HEAD, material: ArmorMaterials::CHAINMAIL()), [EnchantmentTags::HELMET]));
		self::registerDelayed("copper_helmet", fn($name) : Armor => new Armor(self::makeIID($name), "Copper Helmet", new ArmorTypeInfo(2, 122, ArmorInventory::SLOT_HEAD, material: ArmorMaterials::COPPER()), [EnchantmentTags::HELMET]));
		self::registerDelayed("diamond_helmet", fn($name) : Armor => new Armor(self::makeIID($name), "Diamond Helmet", new ArmorTypeInfo(3, 364, ArmorInventory::SLOT_HEAD, 2, material: ArmorMaterials::DIAMOND()), [EnchantmentTags::HELMET]));
		self::registerDelayed("golden_helmet", fn($name) : Armor => new Armor(self::makeIID($name), "Golden Helmet", new ArmorTypeInfo(2, 78, ArmorInventory::SLOT_HEAD, material: ArmorMaterials::GOLD()), [EnchantmentTags::HELMET]));
		self::registerDelayed("iron_helmet", fn($name) : Armor => new Armor(self::makeIID($name), "Iron Helmet", new ArmorTypeInfo(2, 166, ArmorInventory::SLOT_HEAD, material: ArmorMaterials::IRON()), [EnchantmentTags::HELMET]));
		self::registerDelayed("leather_cap", fn($name) : Armor => new Armor(self::makeIID($name), "Leather Cap", new ArmorTypeInfo(1, 56, ArmorInventory::SLOT_HEAD, material: ArmorMaterials::LEATHER()), [EnchantmentTags::HELMET]));
		self::registerDelayed("netherite_helmet", fn($name) : Armor => new Armor(self::makeIID($name), "Netherite Helmet", new ArmorTypeInfo(3, 408, ArmorInventory::SLOT_HEAD, 3, true, material: ArmorMaterials::NETHERITE()), [EnchantmentTags::HELMET]));
		self::registerDelayed("turtle_helmet", fn($name) : TurtleHelmet => new TurtleHelmet(self::makeIID($name), "Turtle Shell", new ArmorTypeInfo(2, 276, ArmorInventory::SLOT_HEAD, material: ArmorMaterials::TURTLE()), [EnchantmentTags::HELMET]));

		self::registerDelayed("chainmail_leggings", fn($name) : Armor => new Armor(self::makeIID($name), "Chainmail Leggings", new ArmorTypeInfo(4, 226, ArmorInventory::SLOT_LEGS, material: ArmorMaterials::CHAINMAIL()), [EnchantmentTags::LEGGINGS]));
		self::registerDelayed("copper_leggings", fn($name) : Armor => new Armor(self::makeIID($name), "Copper Leggings", new ArmorTypeInfo(3, 166, ArmorInventory::SLOT_LEGS, material: ArmorMaterials::COPPER()), [EnchantmentTags::LEGGINGS]));
		self::registerDelayed("diamond_leggings", fn($name) : Armor => new Armor(self::makeIID($name), "Diamond Leggings", new ArmorTypeInfo(6, 496, ArmorInventory::SLOT_LEGS, 2, material: ArmorMaterials::DIAMOND()), [EnchantmentTags::LEGGINGS]));
		self::registerDelayed("golden_leggings", fn($name) : Armor => new Armor(self::makeIID($name), "Golden Leggings", new ArmorTypeInfo(3, 106, ArmorInventory::SLOT_LEGS, material: ArmorMaterials::GOLD()), [EnchantmentTags::LEGGINGS]));
		self::registerDelayed("iron_leggings", fn($name) : Armor => new Armor(self::makeIID($name), "Iron Leggings", new ArmorTypeInfo(5, 226, ArmorInventory::SLOT_LEGS, material: ArmorMaterials::IRON()), [EnchantmentTags::LEGGINGS]));
		self::registerDelayed("leather_pants", fn($name) : Armor => new Armor(self::makeIID($name), "Leather Pants", new ArmorTypeInfo(2, 76, ArmorInventory::SLOT_LEGS, material: ArmorMaterials::LEATHER()), [EnchantmentTags::LEGGINGS]));
		self::registerDelayed("netherite_leggings", fn($name) : Armor => new Armor(self::makeIID($name), "Netherite Leggings", new ArmorTypeInfo(6, 556, ArmorInventory::SLOT_LEGS, 3, true, material: ArmorMaterials::NETHERITE()), [EnchantmentTags::LEGGINGS]));
	}

	private function registerSmithingTemplates() : void{
		self::register("netherite_upgrade_smithing_template", fn(IID $id) => new Item($id, "Netherite Upgrade Smithing Template"));
		self::register("coast_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Coast Armor Trim Smithing Template"));
		self::register("dune_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Dune Armor Trim Smithing Template"));
		self::register("eye_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Eye Armor Trim Smithing Template"));
		self::register("host_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Host Armor Trim Smithing Template"));
		self::register("raiser_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Raiser Armor Trim Smithing Template"));
		self::register("rib_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Rib Armor Trim Smithing Template"));
		self::register("sentry_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Sentry Armor Trim Smithing Template"));
		self::register("shaper_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Shaper Armor Trim Smithing Template"));
		self::register("silence_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Silence Armor Trim Smithing Template"));
		self::register("snout_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Snout Armor Trim Smithing Template"));
		self::register("spire_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Spire Armor Trim Smithing Template"));
		self::register("tide_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Tide Armor Trim Smithing Template"));
		self::register("vex_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Vex Armor Trim Smithing Template"));
		self::register("ward_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Ward Armor Trim Smithing Template"));
		self::register("wayfinder_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Wayfinder Armor Trim Smithing Template"));
		self::register("wild_armor_trim_smithing_template", fn(IID $id) => new Item($id, "Wild Armor Trim Smithing Template"));
	}


}

/**
 * Capa de dispatch y fábrica de items de PocketMine-Rust.
 */
class ItemFactory
{
    private static ?ItemFactory $instance = null;
    /** @var array<string, \Closure(): Item> */
    private static array $cloners = [];
    /** @var array<string, int> */
    private static array $canonicalToNetwork = array (
  'acacia_boat' => 411,
  'acacia_chest_boat' => 684,
  'acacia_door' => 595,
  'acacia_fence_gate' => 187,
  'acacia_leaves' => 161,
  'acacia_log' => 162,
  'acacia_sign' => 618,
  'acacia_stairs' => 163,
  'activator_rail' => 126,
  'agent_spawn_egg' => 521,
  'allay_spawn_egg' => 673,
  'allow' => 210,
  'amethyst_shard' => 666,
  'angler_pottery_sherd' => 699,
  'anvil' => 145,
  'apple' => 285,
  'archer_pottery_sherd' => 700,
  'armadillo_scute' => 746,
  'armadillo_spawn_egg' => 745,
  'armor_stand' => 591,
  'arms_up_pottery_sherd' => 701,
  'arrow' => 332,
  'axolotl_bucket' => 401,
  'axolotl_spawn_egg' => 536,
  'baked_potato' => 310,
  'balloon' => 641,
  'bamboo_chest_raft' => 696,
  'bamboo_raft' => 695,
  'bamboo_sign' => 694,
  'banner' => 606,
  'banner_pattern' => 842,
  'bat_spawn_egg' => 486,
  'beacon' => 138,
  'bed' => 450,
  'bedrock' => 7,
  'bee_spawn_egg' => 528,
  'beef' => 302,
  'beetroot' => 314,
  'beetroot_seeds' => 324,
  'beetroot_soup' => 315,
  'birch_boat' => 408,
  'birch_chest_boat' => 681,
  'birch_door' => 593,
  'birch_fence_gate' => 184,
  'birch_sign' => 616,
  'birch_stairs' => 135,
  'black_bundle' => 264,
  'black_dye' => 427,
  'black_glazed_terracotta' => 235,
  'black_harness' => 756,
  'blade_pottery_sherd' => 702,
  'blaze_powder' => 462,
  'blaze_rod' => 455,
  'blaze_spawn_egg' => 489,
  'bleach' => 639,
  'blue_bundle' => 265,
  'blue_dye' => 431,
  'blue_egg' => 753,
  'blue_glazed_terracotta' => 231,
  'blue_harness' => 757,
  'board' => 635,
  'boat' => 840,
  'bogged_spawn_egg' => 496,
  'bolt_armor_trim_smithing_template' => 741,
  'bone' => 447,
  'bone_block' => 216,
  'bone_meal' => 443,
  'book' => 419,
  'bookshelf' => 47,
  'border_block' => 212,
  'bordure_indented_banner_pattern' => 625,
  'bow' => 331,
  'bowl' => 353,
  'bread' => 290,
  'breeze_rod' => 281,
  'breeze_spawn_egg' => 535,
  'brewer_pottery_sherd' => 703,
  'brewing_stand' => 464,
  'brick' => 415,
  'brick_block' => 45,
  'brick_stairs' => 108,
  'brown_bundle' => 266,
  'brown_dye' => 430,
  'brown_egg' => 754,
  'brown_glazed_terracotta' => 232,
  'brown_harness' => 758,
  'brown_mushroom' => 39,
  'brown_mushroom_block' => 99,
  'brush' => 722,
  'bucket' => 392,
  'bundle' => 267,
  'burn_pottery_sherd' => 704,
  'cactus' => 81,
  'cake' => 449,
  'camel_husk_spawn_egg' => 698,
  'camel_spawn_egg' => 697,
  'camera' => 636,
  'campfire' => 630,
  'carpet' => 796,
  'carrot' => 308,
  'carrot_on_a_stick' => 556,
  'carrots' => 141,
  'cat_spawn_egg' => 522,
  'cauldron' => 465,
  'cave_spider_spawn_egg' => 490,
  'chain_command_block' => 189,
  'chainmail_boots' => 374,
  'chainmail_chestplate' => 372,
  'chainmail_helmet' => 371,
  'chainmail_leggings' => 373,
  'chalkboard' => 230,
  'charcoal' => 334,
  'chemical_heat' => 192,
  'chemistry_table' => 834,
  'cherry_boat' => 691,
  'cherry_chest_boat' => 692,
  'cherry_sign' => 693,
  'chest' => 54,
  'chest_boat' => 687,
  'chest_minecart' => 421,
  'chicken' => 304,
  'chicken_spawn_egg' => 468,
  'chorus_flower' => 200,
  'chorus_fruit' => 597,
  'chorus_plant' => 240,
  'clay' => 82,
  'clay_ball' => 416,
  'clock' => 425,
  'coal' => 333,
  'coal_block' => 173,
  'coal_ore' => 16,
  'coast_armor_trim_smithing_template' => 726,
  'cobblestone' => 4,
  'cobblestone_wall' => 139,
  'cocoa' => 127,
  'cocoa_beans' => 444,
  'cod' => 293,
  'cod_bucket' => 396,
  'cod_spawn_egg' => 514,
  'colored_torch_blue' => 204,
  'colored_torch_bp' => 838,
  'colored_torch_red' => 202,
  'colored_torch_rg' => 837,
  'command_block' => 137,
  'command_block_minecart' => 602,
  'comparator' => 561,
  'compass' => 423,
  'compound' => 637,
  'compound_creator' => 238,
  'concrete' => 822,
  'concrete_powder' => 823,
  'cooked_beef' => 303,
  'cooked_chicken' => 305,
  'cooked_cod' => 297,
  'cooked_mutton' => 590,
  'cooked_porkchop' => 292,
  'cooked_rabbit' => 318,
  'cooked_salmon' => 298,
  'cookie' => 300,
  'copper_axe' => 776,
  'copper_boots' => 781,
  'copper_chestplate' => 779,
  'copper_golem_spawn_egg' => 772,
  'copper_helmet' => 778,
  'copper_hoe' => 777,
  'copper_horse_armor' => 783,
  'copper_ingot' => 544,
  'copper_leggings' => 780,
  'copper_nautilus_armor' => 787,
  'copper_nugget' => 782,
  'copper_pickaxe' => 775,
  'copper_shovel' => 774,
  'copper_spear' => 257,
  'copper_sword' => 773,
  'coral' => 818,
  'coral_block' => 800,
  'coral_fan' => 809,
  'coral_fan_dead' => 810,
  'cow_spawn_egg' => 469,
  'crafting_table' => 58,
  'creaking_spawn_egg' => 751,
  'creeper_banner_pattern' => 621,
  'creeper_spawn_egg' => 474,
  'crimson_door' => 659,
  'crimson_sign' => 657,
  'crossbow' => 614,
  'cyan_bundle' => 268,
  'cyan_dye' => 433,
  'cyan_glazed_terracotta' => 229,
  'cyan_harness' => 759,
  'dandelion' => 37,
  'danger_pottery_sherd' => 705,
  'dark_oak_boat' => 412,
  'dark_oak_chest_boat' => 685,
  'dark_oak_door' => 596,
  'dark_oak_fence_gate' => 186,
  'dark_oak_sign' => 619,
  'dark_oak_stairs' => 164,
  'daylight_detector' => 151,
  'daylight_detector_inverted' => 178,
  'deadbush' => 32,
  'deny' => 211,
  'detector_rail' => 28,
  'diamond' => 335,
  'diamond_axe' => 350,
  'diamond_block' => 57,
  'diamond_boots' => 382,
  'diamond_chestplate' => 380,
  'diamond_helmet' => 379,
  'diamond_hoe' => 364,
  'diamond_horse_armor' => 572,
  'diamond_leggings' => 381,
  'diamond_nautilus_armor' => 790,
  'diamond_ore' => 56,
  'diamond_pickaxe' => 349,
  'diamond_shovel' => 348,
  'diamond_spear' => 258,
  'diamond_sword' => 347,
  'dirt' => 3,
  'disc_fragment_5' => 679,
  'dispenser' => 23,
  'dolphin_spawn_egg' => 518,
  'donkey_spawn_egg' => 499,
  'double_plant' => 816,
  'double_stone_block_slab' => 805,
  'double_stone_block_slab2' => 806,
  'double_stone_block_slab3' => 807,
  'double_stone_block_slab4' => 808,
  'dragon_breath' => 599,
  'dragon_egg' => 122,
  'dried_kelp' => 299,
  'dropper' => 125,
  'drowned_spawn_egg' => 517,
  'dune_armor_trim_smithing_template' => 725,
  'dye' => 841,
  'echo_shard' => 689,
  'egg' => 422,
  'elder_guardian_spawn_egg' => 505,
  'element_0' => 36,
  'elytra' => 603,
  'emerald' => 552,
  'emerald_block' => 133,
  'emerald_ore' => 129,
  'empty_map' => 555,
  'enchanted_book' => 560,
  'enchanted_golden_apple' => 288,
  'enchanting_table' => 116,
  'end_bricks' => 206,
  'end_crystal' => 844,
  'end_gateway' => 209,
  'end_portal' => 119,
  'end_portal_frame' => 120,
  'end_rod' => 208,
  'end_stone' => 121,
  'ender_chest' => 130,
  'ender_dragon_spawn_egg' => 541,
  'ender_eye' => 466,
  'ender_pearl' => 454,
  'enderman_spawn_egg' => 475,
  'endermite_spawn_egg' => 493,
  'evoker_spawn_egg' => 509,
  'experience_bottle' => 548,
  'explorer_pottery_sherd' => 706,
  'eye_armor_trim_smithing_template' => 729,
  'farmland' => 60,
  'feather' => 359,
  'fence' => 798,
  'fence_gate' => 107,
  'fermented_spider_eye' => 461,
  'field_masoned_banner_pattern' => 624,
  'filled_map' => 452,
  'fire' => 51,
  'fire_charge' => 549,
  'firework_rocket' => 558,
  'firework_star' => 559,
  'fishing_rod' => 424,
  'flint' => 388,
  'flint_and_steel' => 330,
  'flow_armor_trim_smithing_template' => 740,
  'flow_banner_pattern' => 628,
  'flow_pottery_sherd' => 707,
  'flower_banner_pattern' => 620,
  'flower_pot' => 554,
  'flowing_lava' => 10,
  'flowing_water' => 8,
  'fox_spawn_egg' => 524,
  'frame' => 553,
  'friend_pottery_sherd' => 708,
  'frog_spawn_egg' => 670,
  'frosted_ice' => 207,
  'furnace' => 61,
  'ghast_spawn_egg' => 487,
  'ghast_tear' => 457,
  'glass' => 20,
  'glass_bottle' => 460,
  'glass_pane' => 102,
  'glistering_melon_slice' => 467,
  'globe_banner_pattern' => 627,
  'glow_berries' => 845,
  'glow_frame' => 665,
  'glow_ink_sac' => 543,
  'glow_squid_spawn_egg' => 538,
  'glow_stick' => 644,
  'glowingobsidian' => 246,
  'glowstone' => 89,
  'glowstone_dust' => 426,
  'goat_horn' => 669,
  'goat_spawn_egg' => 537,
  'gold_block' => 41,
  'gold_ingot' => 337,
  'gold_nugget' => 458,
  'gold_ore' => 14,
  'golden_apple' => 287,
  'golden_axe' => 357,
  'golden_boots' => 386,
  'golden_carrot' => 312,
  'golden_chestplate' => 384,
  'golden_helmet' => 383,
  'golden_hoe' => 365,
  'golden_horse_armor' => 571,
  'golden_leggings' => 385,
  'golden_nautilus_armor' => 789,
  'golden_pickaxe' => 356,
  'golden_rail' => 27,
  'golden_shovel' => 355,
  'golden_spear' => 259,
  'golden_sword' => 354,
  'grass_block' => 2,
  'grass_path' => 198,
  'gravel' => 13,
  'gray_bundle' => 269,
  'gray_dye' => 435,
  'gray_glazed_terracotta' => 227,
  'gray_harness' => 760,
  'green_bundle' => 270,
  'green_dye' => 429,
  'green_glazed_terracotta' => 233,
  'green_harness' => 761,
  'guardian_spawn_egg' => 494,
  'gunpowder' => 360,
  'guster_banner_pattern' => 629,
  'guster_pottery_sherd' => 709,
  'happy_ghast_spawn_egg' => 755,
  'hard_glass' => 253,
  'hard_glass_pane' => 190,
  'hard_stained_glass' => 835,
  'hard_stained_glass_pane' => 836,
  'hard_white_stained_glass' => 254,
  'hard_white_stained_glass_pane' => 191,
  'hardened_clay' => 172,
  'hay_block' => 170,
  'heart_of_the_sea' => 610,
  'heart_pottery_sherd' => 710,
  'heartbreak_pottery_sherd' => 711,
  'heavy_weighted_pressure_plate' => 148,
  'hoglin_spawn_egg' => 530,
  'honey_bottle' => 633,
  'honeycomb' => 632,
  'hopper' => 566,
  'hopper_minecart' => 565,
  'horse_spawn_egg' => 491,
  'host_armor_trim_smithing_template' => 739,
  'howl_pottery_sherd' => 712,
  'husk_spawn_egg' => 497,
  'ice' => 79,
  'ice_bomb' => 638,
  'infested_stone' => 97,
  'info_update' => 248,
  'info_update2' => 249,
  'ink_sac' => 445,
  'invisible_bedrock' => 95,
  'iron_axe' => 329,
  'iron_bars' => 101,
  'iron_block' => 42,
  'iron_boots' => 378,
  'iron_chestplate' => 376,
  'iron_door' => 404,
  'iron_golem_spawn_egg' => 539,
  'iron_helmet' => 375,
  'iron_hoe' => 363,
  'iron_horse_armor' => 570,
  'iron_ingot' => 336,
  'iron_leggings' => 377,
  'iron_nautilus_armor' => 788,
  'iron_nugget' => 608,
  'iron_ore' => 15,
  'iron_pickaxe' => 328,
  'iron_shovel' => 327,
  'iron_spear' => 260,
  'iron_sword' => 338,
  'iron_trapdoor' => 167,
  'item.bed' => 26,
  'item.beetroot' => 244,
  'item.brewing_stand' => 117,
  'item.cake' => 92,
  'item.camera' => 242,
  'item.cauldron' => 118,
  'item.flower_pot' => 140,
  'item.frame' => 199,
  'item.hopper' => 154,
  'item.nether_wart' => 115,
  'item.reeds' => 83,
  'item.wheat' => 59,
  'jukebox' => 84,
  'jungle_boat' => 409,
  'jungle_chest_boat' => 682,
  'jungle_door' => 594,
  'jungle_fence_gate' => 185,
  'jungle_sign' => 617,
  'jungle_stairs' => 136,
  'kelp' => 414,
  'ladder' => 65,
  'lapis_block' => 22,
  'lapis_lazuli' => 446,
  'lapis_ore' => 21,
  'lava' => 11,
  'lava_bucket' => 395,
  'lead' => 586,
  'leather' => 413,
  'leather_boots' => 370,
  'leather_chestplate' => 368,
  'leather_helmet' => 367,
  'leather_horse_armor' => 569,
  'leather_leggings' => 369,
  'leaves' => 812,
  'leaves2' => 813,
  'lever' => 69,
  'light_block' => 839,
  'light_blue_bundle' => 271,
  'light_blue_dye' => 439,
  'light_blue_glazed_terracotta' => 223,
  'light_blue_harness' => 762,
  'light_gray_bundle' => 272,
  'light_gray_dye' => 434,
  'light_gray_harness' => 763,
  'light_weighted_pressure_plate' => 147,
  'lime_bundle' => 273,
  'lime_dye' => 437,
  'lime_glazed_terracotta' => 225,
  'lime_harness' => 764,
  'lingering_potion' => 601,
  'lit_furnace' => 62,
  'lit_pumpkin' => 91,
  'lit_redstone_lamp' => 124,
  'lit_redstone_ore' => 74,
  'llama_spawn_egg' => 507,
  'lodestone_compass' => 645,
  'log' => 797,
  'log2' => 820,
  'mace' => 351,
  'magenta_bundle' => 274,
  'magenta_dye' => 440,
  'magenta_glazed_terracotta' => 222,
  'magenta_harness' => 765,
  'magma' => 213,
  'magma_cream' => 463,
  'magma_cube_spawn_egg' => 488,
  'mangrove_boat' => 677,
  'mangrove_chest_boat' => 686,
  'mangrove_door' => 675,
  'mangrove_sign' => 676,
  'medicine' => 642,
  'melon_block' => 103,
  'melon_seeds' => 322,
  'melon_slice' => 301,
  'melon_stem' => 105,
  'milk_bucket' => 393,
  'minecart' => 402,
  'miner_pottery_sherd' => 713,
  'mob_spawner' => 52,
  'mojang_banner_pattern' => 623,
  'monster_egg' => 821,
  'mooshroom_spawn_egg' => 473,
  'mossy_cobblestone' => 48,
  'mourner_pottery_sherd' => 714,
  'moving_block' => 250,
  'mule_spawn_egg' => 500,
  'mushroom_stew' => 289,
  'music_disc_11' => 583,
  'music_disc_13' => 573,
  'music_disc_5' => 678,
  'music_disc_blocks' => 575,
  'music_disc_bounce' => 833,
  'music_disc_cat' => 574,
  'music_disc_chirp' => 576,
  'music_disc_creator' => 828,
  'music_disc_creator_music_box' => 829,
  'music_disc_far' => 577,
  'music_disc_lava_chicken' => 832,
  'music_disc_mall' => 578,
  'music_disc_mellohi' => 579,
  'music_disc_otherside' => 668,
  'music_disc_pigstep' => 662,
  'music_disc_precipice' => 830,
  'music_disc_relic' => 742,
  'music_disc_stal' => 580,
  'music_disc_strad' => 581,
  'music_disc_tears' => 831,
  'music_disc_wait' => 584,
  'music_disc_ward' => 582,
  'mutton' => 589,
  'mycelium' => 110,
  'name_tag' => 587,
  'nautilus_shell' => 609,
  'nautilus_spawn_egg' => 784,
  'nether_brick' => 112,
  'nether_brick_fence' => 113,
  'nether_brick_stairs' => 114,
  'nether_sprouts' => 663,
  'nether_star' => 557,
  'nether_wart' => 323,
  'nether_wart_block' => 214,
  'netherbrick' => 562,
  'netherite_axe' => 649,
  'netherite_boots' => 655,
  'netherite_chestplate' => 653,
  'netherite_helmet' => 652,
  'netherite_hoe' => 650,
  'netherite_horse_armor' => 792,
  'netherite_ingot' => 651,
  'netherite_leggings' => 654,
  'netherite_nautilus_armor' => 791,
  'netherite_pickaxe' => 648,
  'netherite_scrap' => 656,
  'netherite_shovel' => 647,
  'netherite_spear' => 261,
  'netherite_sword' => 646,
  'netherite_upgrade_smithing_template' => 723,
  'netherrack' => 87,
  'netherreactor' => 247,
  'noteblock' => 25,
  'npc_spawn_egg' => 504,
  'oak_boat' => 407,
  'oak_chest_boat' => 680,
  'oak_double_slab' => 157,
  'oak_fence' => 85,
  'oak_leaves' => 18,
  'oak_log' => 17,
  'oak_planks' => 5,
  'oak_sapling' => 6,
  'oak_sign' => 390,
  'oak_slab' => 158,
  'oak_stairs' => 53,
  'observer' => 251,
  'obsidian' => 49,
  'ocelot_spawn_egg' => 484,
  'ominous_bottle' => 634,
  'ominous_trial_key' => 282,
  'orange_bundle' => 275,
  'orange_dye' => 441,
  'orange_glazed_terracotta' => 221,
  'orange_harness' => 766,
  'packed_ice' => 174,
  'painting' => 389,
  'pale_oak_boat' => 748,
  'pale_oak_chest_boat' => 749,
  'pale_oak_sign' => 750,
  'panda_spawn_egg' => 523,
  'paper' => 418,
  'parched_spawn_egg' => 786,
  'parrot_spawn_egg' => 512,
  'phantom_membrane' => 613,
  'phantom_spawn_egg' => 520,
  'pig_spawn_egg' => 470,
  'piglin_banner_pattern' => 626,
  'piglin_brute_spawn_egg' => 533,
  'piglin_spawn_egg' => 531,
  'pillager_spawn_egg' => 525,
  'pink_bundle' => 276,
  'pink_dye' => 436,
  'pink_glazed_terracotta' => 226,
  'pink_harness' => 767,
  'piston' => 33,
  'piston_arm_collision' => 34,
  'pitcher_pod' => 326,
  'planks' => 817,
  'plenty_pottery_sherd' => 715,
  'podzol' => 243,
  'poisonous_potato' => 311,
  'polar_bear_spawn_egg' => 506,
  'popped_chorus_fruit' => 598,
  'poppy' => 38,
  'porkchop' => 291,
  'portal' => 90,
  'potato' => 309,
  'potatoes' => 142,
  'potion' => 459,
  'powder_snow_bucket' => 400,
  'powered_comparator' => 150,
  'powered_repeater' => 94,
  'prismarine' => 168,
  'prismarine_crystals' => 588,
  'prismarine_shard' => 604,
  'prize_pottery_sherd' => 716,
  'pufferfish' => 296,
  'pufferfish_bucket' => 399,
  'pufferfish_spawn_egg' => 515,
  'pumpkin' => 86,
  'pumpkin_pie' => 313,
  'pumpkin_seeds' => 321,
  'pumpkin_stem' => 104,
  'purple_bundle' => 277,
  'purple_dye' => 432,
  'purple_glazed_terracotta' => 219,
  'purple_harness' => 768,
  'purpur_block' => 201,
  'purpur_stairs' => 203,
  'quartz' => 563,
  'quartz_block' => 155,
  'quartz_ore' => 153,
  'quartz_stairs' => 156,
  'rabbit' => 317,
  'rabbit_foot' => 567,
  'rabbit_hide' => 568,
  'rabbit_spawn_egg' => 492,
  'rabbit_stew' => 319,
  'rail' => 66,
  'raiser_armor_trim_smithing_template' => 737,
  'rapid_fertilizer' => 640,
  'ravager_spawn_egg' => 527,
  'raw_copper' => 547,
  'raw_gold' => 546,
  'raw_iron' => 545,
  'recovery_compass' => 688,
  'red_bundle' => 278,
  'red_dye' => 428,
  'red_flower' => 815,
  'red_glazed_terracotta' => 234,
  'red_harness' => 769,
  'red_mushroom' => 40,
  'red_mushroom_block' => 100,
  'red_nether_brick' => 215,
  'red_sandstone' => 179,
  'red_sandstone_double_slab' => 181,
  'red_sandstone_slab' => 182,
  'red_sandstone_stairs' => 180,
  'redstone' => 405,
  'redstone_block' => 152,
  'redstone_lamp' => 123,
  'redstone_ore' => 73,
  'redstone_torch' => 76,
  'redstone_wire' => 55,
  'repeater' => 451,
  'repeating_command_block' => 188,
  'reserved6' => 255,
  'resin_brick' => 752,
  'rib_armor_trim_smithing_template' => 733,
  'rotten_flesh' => 306,
  'saddle' => 403,
  'salmon' => 294,
  'salmon_bucket' => 397,
  'salmon_spawn_egg' => 516,
  'sand' => 12,
  'sandstone' => 24,
  'sandstone_stairs' => 128,
  'sapling' => 811,
  'scrape_pottery_sherd' => 717,
  'sea_lantern' => 169,
  'sentry_armor_trim_smithing_template' => 724,
  'shaper_armor_trim_smithing_template' => 738,
  'sheaf_pottery_sherd' => 718,
  'shears' => 453,
  'sheep_spawn_egg' => 471,
  'shelter_pottery_sherd' => 719,
  'shield' => 387,
  'short_grass' => 31,
  'shulker_box' => 826,
  'shulker_shell' => 605,
  'shulker_spawn_egg' => 503,
  'silence_armor_trim_smithing_template' => 735,
  'silver_glazed_terracotta' => 228,
  'silverfish_spawn_egg' => 476,
  'skeleton_horse_spawn_egg' => 501,
  'skeleton_skull' => 144,
  'skeleton_spawn_egg' => 477,
  'skull' => 743,
  'skull_banner_pattern' => 622,
  'skull_pottery_sherd' => 720,
  'slime' => 165,
  'slime_ball' => 420,
  'slime_spawn_egg' => 478,
  'smooth_stone_double_slab' => 43,
  'smooth_stone_slab' => 44,
  'sniffer_spawn_egg' => 534,
  'snort_pottery_sherd' => 721,
  'snout_armor_trim_smithing_template' => 732,
  'snow' => 80,
  'snow_golem_spawn_egg' => 540,
  'snow_layer' => 78,
  'snowball' => 406,
  'soul_campfire' => 664,
  'soul_sand' => 88,
  'sparkler' => 643,
  'spawn_egg' => 843,
  'spider_eye' => 307,
  'spider_spawn_egg' => 479,
  'spire_armor_trim_smithing_template' => 734,
  'splash_potion' => 600,
  'sponge' => 19,
  'spruce_boat' => 410,
  'spruce_chest_boat' => 683,
  'spruce_door' => 592,
  'spruce_fence_gate' => 183,
  'spruce_sign' => 615,
  'spruce_stairs' => 134,
  'spyglass' => 667,
  'squid_spawn_egg' => 483,
  'stained_glass' => 824,
  'stained_glass_pane' => 825,
  'stained_hardened_clay' => 744,
  'standing_banner' => 176,
  'standing_sign' => 63,
  'stick' => 352,
  'sticky_piston' => 29,
  'stone' => 1,
  'stone_axe' => 346,
  'stone_block_slab' => 801,
  'stone_block_slab2' => 802,
  'stone_block_slab3' => 803,
  'stone_block_slab4' => 804,
  'stone_brick_stairs' => 109,
  'stone_bricks' => 98,
  'stone_button' => 77,
  'stone_hoe' => 362,
  'stone_pickaxe' => 345,
  'stone_pressure_plate' => 70,
  'stone_shovel' => 344,
  'stone_spear' => 262,
  'stone_stairs' => 67,
  'stone_sword' => 343,
  'stonebrick' => 799,
  'stonecutter' => 245,
  'stray_spawn_egg' => 495,
  'strider_spawn_egg' => 529,
  'string' => 358,
  'structure_block' => 252,
  'structure_void' => 217,
  'sugar' => 448,
  'sugar_cane' => 417,
  'sulfur_cube_bucket' => 794,
  'sulfur_cube_spawn_egg' => 793,
  'sunflower' => 175,
  'suspicious_stew' => 631,
  'sweet_berries' => 316,
  'tadpole_bucket' => 672,
  'tadpole_spawn_egg' => 671,
  'tallgrass' => 819,
  'tide_armor_trim_smithing_template' => 731,
  'tnt' => 46,
  'tnt_minecart' => 564,
  'torch' => 50,
  'torchflower_seeds' => 325,
  'totem_of_undying' => 607,
  'trader_llama_spawn_egg' => 690,
  'trapdoor' => 96,
  'trapped_chest' => 146,
  'trial_key' => 283,
  'trident' => 585,
  'trip_wire' => 132,
  'tripwire_hook' => 131,
  'tropical_fish' => 295,
  'tropical_fish_bucket' => 398,
  'tropical_fish_spawn_egg' => 513,
  'turtle_helmet' => 612,
  'turtle_scute' => 611,
  'turtle_spawn_egg' => 519,
  'underwater_torch' => 239,
  'undyed_shulker_box' => 205,
  'unlit_redstone_torch' => 75,
  'unpowered_comparator' => 149,
  'unpowered_repeater' => 93,
  'vex_armor_trim_smithing_template' => 730,
  'vex_spawn_egg' => 510,
  'villager_spawn_egg' => 482,
  'vindicator_spawn_egg' => 508,
  'vine' => 106,
  'wall_banner' => 177,
  'wall_sign' => 68,
  'wandering_trader_spawn_egg' => 526,
  'ward_armor_trim_smithing_template' => 728,
  'warden_spawn_egg' => 674,
  'warped_door' => 660,
  'warped_fungus_on_a_stick' => 661,
  'warped_sign' => 658,
  'water' => 9,
  'water_bucket' => 394,
  'waterlily' => 111,
  'wayfinder_armor_trim_smithing_template' => 736,
  'web' => 30,
  'wheat' => 366,
  'wheat_seeds' => 320,
  'white_bundle' => 279,
  'white_carpet' => 171,
  'white_concrete' => 236,
  'white_concrete_powder' => 237,
  'white_dye' => 442,
  'white_glazed_terracotta' => 220,
  'white_harness' => 770,
  'white_shulker_box' => 218,
  'white_stained_glass' => 241,
  'white_stained_glass_pane' => 160,
  'white_terracotta' => 159,
  'white_wool' => 35,
  'wild_armor_trim_smithing_template' => 727,
  'wind_charge' => 284,
  'witch_spawn_egg' => 485,
  'wither_skeleton_spawn_egg' => 498,
  'wither_spawn_egg' => 542,
  'wolf_armor' => 747,
  'wolf_spawn_egg' => 472,
  'wood' => 827,
  'wooden_axe' => 342,
  'wooden_button' => 143,
  'wooden_door' => 391,
  'wooden_hoe' => 361,
  'wooden_pickaxe' => 341,
  'wooden_pressure_plate' => 72,
  'wooden_shovel' => 340,
  'wooden_slab' => 814,
  'wooden_spear' => 263,
  'wooden_sword' => 339,
  'wool' => 795,
  'writable_book' => 550,
  'written_book' => 551,
  'yellow_bundle' => 280,
  'yellow_dye' => 438,
  'yellow_glazed_terracotta' => 224,
  'yellow_harness' => 771,
  'zoglin_spawn_egg' => 532,
  'zombie_horse_spawn_egg' => 502,
  'zombie_nautilus_spawn_egg' => 785,
  'zombie_pigman_spawn_egg' => 481,
  'zombie_spawn_egg' => 480,
  'zombie_villager_spawn_egg' => 511,
);
    /** @var array<int, string> */
    private static array $networkToCanonical = [];
    private static bool $initialized = false;

    public static function getInstance(): self
    {
        return self::$instance ??= new self();
    }

    private static function ensureInitialized(): void
    {
        if (self::$initialized) return;
        self::$initialized = true;

        foreach (self::$canonicalToNetwork as $name => $id) {
            self::$networkToCanonical[$id] = $name;
        }

        // Registrar cloners de todos los items vanilla
        foreach (VanillaItems::getAll() as $constName => $item) {
            $lower = strtolower($constName);
            self::$cloners[$lower] = fn() => clone $item;
        }
    }

    public static function registerCloner(string $name, \Closure $cloner): void
    {
        self::ensureInitialized();
        self::$cloners[strtolower($name)] = $cloner;
    }

    public static function resolveNetworkId(int $typeId): ?int
    {
        self::ensureInitialized();
        // Buscar el nombre constante en ItemTypeIds
        static $idToName = null;
        if ($idToName === null) {
            $idToName = [];
            $ref = new \ReflectionClass(ItemTypeIds::class);
            foreach ($ref->getConstants() as $cName => $cVal) {
                if (is_int($cVal)) {
                    $idToName[$cVal] = strtolower($cName);
                }
            }
        }
        if (isset($idToName[$typeId])) {
            $name = $idToName[$typeId];
            return self::$canonicalToNetwork[$name] ?? null;
        }
        return null;
    }

    /**
     * @param string|int $name
     */
    public static function get($name, int $meta = 0, int $count = 1): Item
    {
        self::ensureInitialized();

        if (is_string($name)) {
            $clean = str_replace('minecraft:', '', strtolower($name));
            if (isset(self::$cloners[$clean])) {
                $item = (self::$cloners[$clean])();
                $item->setCount($count);
                return $item;
            }
            // Intentar con bloque
            $upper = strtoupper(str_ireplace('minecraft:', '', $clean));
            $block = Blocks::getAll()[$upper] ?? Blocks::getAll()[$clean] ?? null;
            if ($block !== null) {
                $ib = new ItemBlock($block);
                $ib->setCount($count);
                return $ib;
            }
            // Fallback por BlockFactory idFromName
            $blockId = \pocketmine\world\BlockFactory::idFromName($name);
            if ($blockId > 0) {
                $blockName = \pocketmine\world\BlockFactory::networkNameOf($blockId);
                $cleanB = strtoupper(str_ireplace('minecraft:', '', $blockName));
                $block = Blocks::getAll()[$cleanB] ?? null;
                if ($block !== null) {
                    $ib = new ItemBlock($block);
                    $ib->setCount($count);
                    return $ib;
                }
                return new Item($blockId, $meta, $count);
            }
            // Buscar por id nativo si está disponible
            if (isset(self::$canonicalToNetwork[$clean])) {
                return new Item(self::$canonicalToNetwork[$clean], $meta, $count);
            }
            return new Item(0, $meta, $count);
        }

        $numericId = (int)$name;
        if ($numericId === 0) {
            return new Item(0, 0, 0);
        }

        // Si es un id de red de un bloque
        $blockName = \pocketmine\world\BlockFactory::networkNameOf($numericId);
        if ($blockName !== 'minecraft:air') {
            $cleanBlock = strtoupper(str_ireplace('minecraft:', '', $blockName));
            $block = Blocks::getAll()[$cleanBlock] ?? Blocks::getAll()[strtolower($cleanBlock)] ?? null;
            if ($block !== null) {
                $ib = new ItemBlock($block);
                $ib->setCount($count);
                return $ib;
            }
        }

        // Si es un id de item de red
        if (isset(self::$networkToCanonical[$numericId])) {
            $cName = self::$networkToCanonical[$numericId];
            if (isset(self::$cloners[$cName])) {
                $item = (self::$cloners[$cName])();
                $item->setCount($count);
                return $item;
            }
        }

        return new Item($numericId, $meta, $count);
    }

    public static function getFromString(string $itemString): Item
    {
        $count = 1;
        if (preg_match('/^(.+?)x(\d+)$/i', $itemString, $matches)) {
            $itemString = $matches[1];
            $count = (int)$matches[2];
        }
        $meta = 0;
        if (!str_starts_with($itemString, 'minecraft:') && str_contains($itemString, ':')) {
            [$itemString, $metaPart] = explode(':', $itemString, 2);
            $meta = (int)$metaPart;
        }
        return self::get($itemString, $meta, $count);
    }

    public static function fromString(string $itemString, bool $multiple = false): Item
    {
        return self::getFromString($itemString);
    }

    public static function getCreativeInventory(): array
    {
        return VanillaItems::getAll();
    }

    public static function clearCreativeItems(): void {}

    public static function isRegistered(string $name): bool
    {
        self::ensureInitialized();
        $clean = str_replace('minecraft:', '', strtolower($name));
        return isset(self::$cloners[$clean]) || \pocketmine\world\BlockFactory::idFromName($name) !== 0;
    }

    public static function getItemTypeId(string $name): string
    {
        return \pocketmine\world\BlockFactory::networkNameOf(
            \pocketmine\world\BlockFactory::idFromName($name)
        );
    }
}
