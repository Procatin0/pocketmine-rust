<?php

declare(strict_types=1);

namespace pocketmine\block\utils {

    use pocketmine\block\BaseBanner;
    use pocketmine\block\Block;
    use pocketmine\block\BlockIdentifier;
    use pocketmine\block\BlockTypeInfo;
    use pocketmine\block\Farmland;
    use pocketmine\block\VanillaBlocks;
    use pocketmine\block\inventory\BrewingStandInventory;
    use pocketmine\color\Color;
    use pocketmine\data\bedrock\block\BlockLegacyMetadata;
    use pocketmine\data\runtime\RuntimeDataDescriber;
    use pocketmine\entity\Location;
    use pocketmine\entity\object\FallingBlock;
    use pocketmine\entity\projectile\Projectile;
    use pocketmine\event\block\BlockDeathEvent;
    use pocketmine\event\block\BlockFormEvent;
    use pocketmine\event\block\BlockGrowEvent;
    use pocketmine\event\block\BlockMeltEvent;
    use pocketmine\event\block\BlockSpreadEvent;
    use pocketmine\item\Axe;
    use pocketmine\item\Durable;
    use pocketmine\item\Item;
    use pocketmine\item\ItemTypeIds;
    use pocketmine\item\VanillaItems;
    use pocketmine\item\enchantment\VanillaEnchantments;
    use pocketmine\lang\KnownTranslationFactory;
    use pocketmine\lang\Translatable;
    use pocketmine\math\Axis;
    use pocketmine\math\Facing;
    use pocketmine\math\RayTraceResult;
    use pocketmine\math\Vector3;
    use pocketmine\player\Player;
    use pocketmine\utils\AssumptionFailedError;
    use pocketmine\utils\LegacyEnumShimTrait;
    use pocketmine\utils\Utils;
    use pocketmine\world\BlockTransaction;
    use pocketmine\world\Position;
    use pocketmine\world\World;
    use pocketmine\world\generator\object\TreeType;
    use pocketmine\world\sound\AmethystBlockChimeSound;
    use pocketmine\world\sound\BlazeShootSound;
    use pocketmine\world\sound\BlockPunchSound;
    use pocketmine\world\sound\CopperWaxApplySound;
    use pocketmine\world\sound\CopperWaxRemoveSound;
    use pocketmine\world\sound\FireExtinguishSound;
    use pocketmine\world\sound\FlintSteelSound;
    use pocketmine\world\sound\ScrapeSound;
    use pocketmine\world\sound\Sound;
    use pocketmine\xoroshiro128pp\Xoroshiro128PP;

    use function array_fill;
    use function array_fill_keys;
    use function array_key_first;
    use function array_pad;
    use function array_slice;
    use function count;
    use function explode;
    use function floor;
    use function gmp_add;
    use function gmp_and;
    use function gmp_cmp;
    use function gmp_div_q;
    use function gmp_init;
    use function gmp_intval;
    use function gmp_mul;
    use function gmp_or;
    use function gmp_pow;
    use function gmp_sub;
    use function gmp_testbit;
    use function gmp_xor;
    use function intdiv;
    use function is_int;
    use function max;
    use function min;
    use function mt_getrandmax;
    use function mt_rand;
    use function spl_object_id;
    use function str_contains;

    // =========================================================================
    // Enums (22)
    // =========================================================================

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static BannerPatternType BORDER()
     * @method static BannerPatternType BRICKS()
     * @method static BannerPatternType CIRCLE()
     * @method static BannerPatternType CREEPER()
     * @method static BannerPatternType CROSS()
     * @method static BannerPatternType CURLY_BORDER()
     * @method static BannerPatternType DIAGONAL_LEFT()
     * @method static BannerPatternType DIAGONAL_RIGHT()
     * @method static BannerPatternType DIAGONAL_UP_LEFT()
     * @method static BannerPatternType DIAGONAL_UP_RIGHT()
     * @method static BannerPatternType FLOWER()
     * @method static BannerPatternType GRADIENT()
     * @method static BannerPatternType GRADIENT_UP()
     * @method static BannerPatternType HALF_HORIZONTAL()
     * @method static BannerPatternType HALF_HORIZONTAL_BOTTOM()
     * @method static BannerPatternType HALF_VERTICAL()
     * @method static BannerPatternType HALF_VERTICAL_RIGHT()
     * @method static BannerPatternType MOJANG()
     * @method static BannerPatternType RHOMBUS()
     * @method static BannerPatternType SKULL()
     * @method static BannerPatternType SMALL_STRIPES()
     * @method static BannerPatternType SQUARE_BOTTOM_LEFT()
     * @method static BannerPatternType SQUARE_BOTTOM_RIGHT()
     * @method static BannerPatternType SQUARE_TOP_LEFT()
     * @method static BannerPatternType SQUARE_TOP_RIGHT()
     * @method static BannerPatternType STRAIGHT_CROSS()
     * @method static BannerPatternType STRIPE_BOTTOM()
     * @method static BannerPatternType STRIPE_CENTER()
     * @method static BannerPatternType STRIPE_DOWNLEFT()
     * @method static BannerPatternType STRIPE_DOWNRIGHT()
     * @method static BannerPatternType STRIPE_LEFT()
     * @method static BannerPatternType STRIPE_MIDDLE()
     * @method static BannerPatternType STRIPE_RIGHT()
     * @method static BannerPatternType STRIPE_TOP()
     * @method static BannerPatternType TRIANGLES_BOTTOM()
     * @method static BannerPatternType TRIANGLES_TOP()
     * @method static BannerPatternType TRIANGLE_BOTTOM()
     * @method static BannerPatternType TRIANGLE_TOP()
     */
    enum BannerPatternType{
    	use LegacyEnumShimTrait;

    	case BORDER;
    	case BRICKS;
    	case CIRCLE;
    	case CREEPER;
    	case CROSS;
    	case CURLY_BORDER;
    	case DIAGONAL_LEFT;
    	case DIAGONAL_RIGHT;
    	case DIAGONAL_UP_LEFT;
    	case DIAGONAL_UP_RIGHT;
    	case FLOW;
    	case FLOWER;
    	case GLOBE;
    	case GRADIENT;
    	case GRADIENT_UP;
    	case GUSTER;
    	case HALF_HORIZONTAL;
    	case HALF_HORIZONTAL_BOTTOM;
    	case HALF_VERTICAL;
    	case HALF_VERTICAL_RIGHT;
    	case MOJANG;
    	case PIGLIN;
    	case RHOMBUS;
    	case SKULL;
    	case SMALL_STRIPES;
    	case SQUARE_BOTTOM_LEFT;
    	case SQUARE_BOTTOM_RIGHT;
    	case SQUARE_TOP_LEFT;
    	case SQUARE_TOP_RIGHT;
    	case STRAIGHT_CROSS;
    	case STRIPE_BOTTOM;
    	case STRIPE_CENTER;
    	case STRIPE_DOWNLEFT;
    	case STRIPE_DOWNRIGHT;
    	case STRIPE_LEFT;
    	case STRIPE_MIDDLE;
    	case STRIPE_RIGHT;
    	case STRIPE_TOP;
    	case TRIANGLE_BOTTOM;
    	case TRIANGLE_TOP;
    	case TRIANGLES_BOTTOM;
    	case TRIANGLES_TOP;
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static BellAttachmentType CEILING()
     * @method static BellAttachmentType FLOOR()
     * @method static BellAttachmentType ONE_WALL()
     * @method static BellAttachmentType TWO_WALLS()
     */
    enum BellAttachmentType{
    	use LegacyEnumShimTrait;

    	case CEILING;
    	case FLOOR;
    	case ONE_WALL;
    	case TWO_WALLS;
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static BrewingStandSlot EAST()
     * @method static BrewingStandSlot NORTHWEST()
     * @method static BrewingStandSlot SOUTHWEST()
     */
    enum BrewingStandSlot{
    	use LegacyEnumShimTrait;

    	case EAST;
    	case NORTHWEST;
    	case SOUTHWEST;

    	/**
    	 * Returns the brewing stand inventory slot number associated with this visual slot.
    	 */
    	public function getSlotNumber() : int{
    		return match($this){
    			self::EAST => BrewingStandInventory::SLOT_BOTTLE_LEFT,
    			self::NORTHWEST => BrewingStandInventory::SLOT_BOTTLE_MIDDLE,
    			self::SOUTHWEST => BrewingStandInventory::SLOT_BOTTLE_RIGHT
    		};
    	}
    }

    enum ChiseledBookshelfSlot : int{
    	case TOP_LEFT = 0;
    	case TOP_MIDDLE = 1;
    	case TOP_RIGHT = 2;
    	case BOTTOM_LEFT = 3;
    	case BOTTOM_MIDDLE = 4;
    	case BOTTOM_RIGHT = 5;

    	private const SLOTS_PER_SHELF = 3;

    	public static function fromBlockFaceCoordinates(float $x, float $y) : self{
    		if($x < 0 || $x > 1){
    			throw new \InvalidArgumentException("X must be between 0 and 1, got $x");
    		}
    		if($y < 0 || $y > 1){
    			throw new \InvalidArgumentException("Y must be between 0 and 1, got $y");
    		}

    		$slot = ($y < 0.5 ? self::SLOTS_PER_SHELF : 0) + match(true){
    			//we can't use simple maths here as the action is aligned to the 16x16 pixel grid :(
    			$x < 6 / 16 => 0,
    			$x < 11 / 16 => 1,
    			default => 2
    		};

    		return self::from($slot);
    	}
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static CopperOxidation EXPOSED()
     * @method static CopperOxidation NONE()
     * @method static CopperOxidation OXIDIZED()
     * @method static CopperOxidation WEATHERED()
     */
    enum CopperOxidation : int{
    	use LegacyEnumShimTrait;

    	case NONE = 0;
    	case EXPOSED = 1;
    	case WEATHERED = 2;
    	case OXIDIZED = 3;

    	public function getPrevious() : ?self{
    		return self::tryFrom($this->value - 1);
    	}

    	public function getNext() : ?self{
    		return self::tryFrom($this->value + 1);
    	}
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static CoralType BRAIN()
     * @method static CoralType BUBBLE()
     * @method static CoralType FIRE()
     * @method static CoralType HORN()
     * @method static CoralType TUBE()
     */
    enum CoralType{
    	use LegacyEnumShimTrait;

    	case TUBE;
    	case BRAIN;
    	case BUBBLE;
    	case FIRE;
    	case HORN;

    	public function getDisplayName() : string{
    		return match($this){
    			self::TUBE => "Tube",
    			self::BRAIN => "Brain",
    			self::BUBBLE => "Bubble",
    			self::FIRE => "Fire",
    			self::HORN => "Horn",
    		};
    	}
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static DirtType COARSE()
     * @method static DirtType NORMAL()
     * @method static DirtType ROOTED()
     */
    enum DirtType{
    	use LegacyEnumShimTrait;

    	case NORMAL;
    	case COARSE;
    	case ROOTED;
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static DripleafState FULL_TILT()
     * @method static DripleafState PARTIAL_TILT()
     * @method static DripleafState STABLE()
     * @method static DripleafState UNSTABLE()
     */
    enum DripleafState{
    	use LegacyEnumShimTrait;

    	case STABLE;
    	case UNSTABLE;
    	case PARTIAL_TILT;
    	case FULL_TILT;

    	public function getScheduledUpdateDelayTicks() : ?int{
    		return match($this){
    			self::STABLE => null,
    			self::UNSTABLE, self::PARTIAL_TILT => 10,
    			self::FULL_TILT => 100,
    		};
    	}
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static DyeColor BLACK()
     * @method static DyeColor BLUE()
     * @method static DyeColor BROWN()
     * @method static DyeColor CYAN()
     * @method static DyeColor GRAY()
     * @method static DyeColor GREEN()
     * @method static DyeColor LIGHT_BLUE()
     * @method static DyeColor LIGHT_GRAY()
     * @method static DyeColor LIME()
     * @method static DyeColor MAGENTA()
     * @method static DyeColor ORANGE()
     * @method static DyeColor PINK()
     * @method static DyeColor PURPLE()
     * @method static DyeColor RED()
     * @method static DyeColor WHITE()
     * @method static DyeColor YELLOW()
     *
     * @phpstan-type TMetadata array{0: string, 1: Color}
     */
    enum DyeColor{
    	use LegacyEnumShimTrait;

    	case WHITE;
    	case ORANGE;
    	case MAGENTA;
    	case LIGHT_BLUE;
    	case YELLOW;
    	case LIME;
    	case PINK;
    	case GRAY;
    	case LIGHT_GRAY;
    	case CYAN;
    	case PURPLE;
    	case BLUE;
    	case BROWN;
    	case GREEN;
    	case RED;
    	case BLACK;

    	/**
    	 * This function exists only to permit the use of named arguments and to make the code easier to read in PhpStorm.
    	 *
    	 * @phpstan-return TMetadata
    	 */
    	private static function meta(string $displayName, Color $rgbValue) : array{
    		return [$displayName, $rgbValue];
    	}

    	/**
    	 * @phpstan-return TMetadata
    	 */
    	private function getMetadata() : array{
    		/** @phpstan-var array<int, TMetadata> $cache */
    		static $cache = [];

    		return $cache[spl_object_id($this)] ??= match($this){
    			self::WHITE => self::meta("White", new Color(0xf0, 0xf0, 0xf0)),
    			self::ORANGE => self::meta("Orange", new Color(0xf9, 0x80, 0x1d)),
    			self::MAGENTA => self::meta("Magenta", new Color(0xc7, 0x4e, 0xbd)),
    			self::LIGHT_BLUE => self::meta("Light Blue", new Color(0x3a, 0xb3, 0xda)),
    			self::YELLOW => self::meta("Yellow", new Color(0xfe, 0xd8, 0x3d)),
    			self::LIME => self::meta("Lime", new Color(0x80, 0xc7, 0x1f)),
    			self::PINK => self::meta("Pink", new Color(0xf3, 0x8b, 0xaa)),
    			self::GRAY => self::meta("Gray", new Color(0x47, 0x4f, 0x52)),
    			self::LIGHT_GRAY => self::meta("Light Gray", new Color(0x9d, 0x9d, 0x97)),
    			self::CYAN => self::meta("Cyan", new Color(0x16, 0x9c, 0x9c)),
    			self::PURPLE => self::meta("Purple", new Color(0x89, 0x32, 0xb8)),
    			self::BLUE => self::meta("Blue", new Color(0x3c, 0x44, 0xaa)),
    			self::BROWN => self::meta("Brown", new Color(0x83, 0x54, 0x32)),
    			self::GREEN => self::meta("Green", new Color(0x5e, 0x7c, 0x16)),
    			self::RED => self::meta("Red", new Color(0xb0, 0x2e, 0x26)),
    			self::BLACK => self::meta("Black", new Color(0x1d, 0x1d, 0x21)),
    		};
    	}

    	public function getDisplayName() : string{
    		return $this->getMetadata()[0];
    	}

    	public function getRgbValue() : Color{
    		return $this->getMetadata()[1];
    	}
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static FroglightType OCHRE()
     * @method static FroglightType PEARLESCENT()
     * @method static FroglightType VERDANT()
     */
    enum FroglightType{
    	use LegacyEnumShimTrait;

    	case OCHRE;
    	case PEARLESCENT;
    	case VERDANT;
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static LeavesType ACACIA()
     * @method static LeavesType AZALEA()
     * @method static LeavesType BIRCH()
     * @method static LeavesType CHERRY()
     * @method static LeavesType DARK_OAK()
     * @method static LeavesType FLOWERING_AZALEA()
     * @method static LeavesType JUNGLE()
     * @method static LeavesType MANGROVE()
     * @method static LeavesType OAK()
     * @method static LeavesType SPRUCE()
     */
    enum LeavesType{
    	use LegacyEnumShimTrait;

    	case OAK;
    	case SPRUCE;
    	case BIRCH;
    	case JUNGLE;
    	case ACACIA;
    	case DARK_OAK;
    	case MANGROVE;
    	case AZALEA;
    	case FLOWERING_AZALEA;
    	case CHERRY;
    	case PALE_OAK;

    	public function getDisplayName() : string{
    		return match($this){
    			self::OAK => "Oak",
    			self::SPRUCE => "Spruce",
    			self::BIRCH => "Birch",
    			self::JUNGLE => "Jungle",
    			self::ACACIA => "Acacia",
    			self::DARK_OAK => "Dark Oak",
    			self::MANGROVE => "Mangrove",
    			self::AZALEA => "Azalea",
    			self::FLOWERING_AZALEA => "Flowering Azalea",
    			self::CHERRY => "Cherry",
    			self::PALE_OAK => "Pale Oak",
    		};
    	}
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static LeverFacing DOWN_AXIS_X()
     * @method static LeverFacing DOWN_AXIS_Z()
     * @method static LeverFacing EAST()
     * @method static LeverFacing NORTH()
     * @method static LeverFacing SOUTH()
     * @method static LeverFacing UP_AXIS_X()
     * @method static LeverFacing UP_AXIS_Z()
     * @method static LeverFacing WEST()
     */
    enum LeverFacing{
    	use LegacyEnumShimTrait;

    	case UP_AXIS_X;
    	case UP_AXIS_Z;
    	case DOWN_AXIS_X;
    	case DOWN_AXIS_Z;
    	case NORTH;
    	case EAST;
    	case SOUTH;
    	case WEST;

    	public function getFacing() : int{
    		return match($this){
    			self::UP_AXIS_X, self::UP_AXIS_Z => Facing::UP,
    			self::DOWN_AXIS_X, self::DOWN_AXIS_Z => Facing::DOWN,
    			self::NORTH => Facing::NORTH,
    			self::EAST => Facing::EAST,
    			self::SOUTH => Facing::SOUTH,
    			self::WEST => Facing::WEST,
    		};
    	}
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static MobHeadType CREEPER()
     * @method static MobHeadType DRAGON()
     * @method static MobHeadType PIGLIN()
     * @method static MobHeadType PLAYER()
     * @method static MobHeadType SKELETON()
     * @method static MobHeadType WITHER_SKELETON()
     * @method static MobHeadType ZOMBIE()
     */
    enum MobHeadType{
    	use LegacyEnumShimTrait;

    	case SKELETON;
    	case WITHER_SKELETON;
    	case ZOMBIE;
    	case PLAYER;
    	case CREEPER;
    	case DRAGON;
    	case PIGLIN;

    	public function getDisplayName() : string{
    		return match($this){
    			self::SKELETON => "Skeleton Skull",
    			self::WITHER_SKELETON => "Wither Skeleton Skull",
    			self::ZOMBIE => "Zombie Head",
    			self::PLAYER => "Player Head",
    			self::CREEPER => "Creeper Head",
    			self::DRAGON => "Dragon Head",
    			self::PIGLIN => "Piglin Head"
    		};
    	}
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static MushroomBlockType ALL_CAP()
     * @method static MushroomBlockType CAP_EAST()
     * @method static MushroomBlockType CAP_MIDDLE()
     * @method static MushroomBlockType CAP_NORTH()
     * @method static MushroomBlockType CAP_NORTHEAST()
     * @method static MushroomBlockType CAP_NORTHWEST()
     * @method static MushroomBlockType CAP_SOUTH()
     * @method static MushroomBlockType CAP_SOUTHEAST()
     * @method static MushroomBlockType CAP_SOUTHWEST()
     * @method static MushroomBlockType CAP_WEST()
     * @method static MushroomBlockType PORES()
     */
    enum MushroomBlockType{
    	use LegacyEnumShimTrait;

    	case PORES;
    	case CAP_NORTHWEST;
    	case CAP_NORTH;
    	case CAP_NORTHEAST;
    	case CAP_WEST;
    	case CAP_MIDDLE;
    	case CAP_EAST;
    	case CAP_SOUTHWEST;
    	case CAP_SOUTH;
    	case CAP_SOUTHEAST;
    	case ALL_CAP;
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static RecordType DISK_11()
     * @method static RecordType DISK_13()
     * @method static RecordType DISK_5()
     * @method static RecordType DISK_BLOCKS()
     * @method static RecordType DISK_CAT()
     * @method static RecordType DISK_CHIRP()
     * @method static RecordType DISK_FAR()
     * @method static RecordType DISK_MALL()
     * @method static RecordType DISK_MELLOHI()
     * @method static RecordType DISK_OTHERSIDE()
     * @method static RecordType DISK_PIGSTEP()
     * @method static RecordType DISK_STAL()
     * @method static RecordType DISK_STRAD()
     * @method static RecordType DISK_WAIT()
     * @method static RecordType DISK_WARD()
     *
     * @phpstan-type TMetadata array{0: string, 1: Translatable}
     */
    enum RecordType{
    	use LegacyEnumShimTrait;

    	case DISK_13;
    	case DISK_5;
    	case DISK_CAT;
    	case DISK_BLOCKS;
    	case DISK_CHIRP;
    	case DISK_CREATOR;
    	case DISK_CREATOR_MUSIC_BOX;
    	case DISK_FAR;
    	case DISK_LAVA_CHICKEN;
    	case DISK_MALL;
    	case DISK_MELLOHI;
    	case DISK_OTHERSIDE;
    	case DISK_PIGSTEP;
    	case DISK_PRECIPICE;
    	case DISK_RELIC;
    	case DISK_STAL;
    	case DISK_STRAD;
    	case DISK_WARD;
    	case DISK_11;
    	case DISK_WAIT;

    	/**
    	 * @phpstan-return TMetadata
    	 */
    	private function getMetadata() : array{
    		/** @phpstan-var array<int, TMetadata> $cache */
    		static $cache = [];

    		return $cache[spl_object_id($this)] ??= match($this){
    			self::DISK_13 => ["C418 - 13", KnownTranslationFactory::item_record_13_desc()],
    			self::DISK_5 => ["Samuel Åberg - 5", KnownTranslationFactory::item_record_5_desc()],
    			self::DISK_CAT => ["C418 - cat", KnownTranslationFactory::item_record_cat_desc()],
    			self::DISK_BLOCKS => ["C418 - blocks", KnownTranslationFactory::item_record_blocks_desc()],
    			self::DISK_CHIRP => ["C418 - chirp", KnownTranslationFactory::item_record_chirp_desc()],
    			self::DISK_CREATOR => ["Lena Raine - Creator", KnownTranslationFactory::item_record_creator_desc()],
    			self::DISK_CREATOR_MUSIC_BOX => ["Lena Raine - Creator (Music Box)", KnownTranslationFactory::item_record_creator_music_box_desc()],
    			self::DISK_FAR => ["C418 - far", KnownTranslationFactory::item_record_far_desc()],
    			self::DISK_LAVA_CHICKEN => ["Hyper Potions - Lava Chicken", KnownTranslationFactory::item_record_lava_chicken_desc()],
    			self::DISK_MALL => ["C418 - mall", KnownTranslationFactory::item_record_mall_desc()],
    			self::DISK_MELLOHI => ["C418 - mellohi", KnownTranslationFactory::item_record_mellohi_desc()],
    			self::DISK_OTHERSIDE => ["Lena Raine - otherside", KnownTranslationFactory::item_record_otherside_desc()],
    			self::DISK_PIGSTEP => ["Lena Raine - Pigstep", KnownTranslationFactory::item_record_pigstep_desc()],
    			self::DISK_PRECIPICE => ["Aaron Cherof - Precipice", KnownTranslationFactory::item_record_precipice_desc()],
    			self::DISK_RELIC => ["Aaron Cherof - Relic", KnownTranslationFactory::item_record_relic_desc()],
    			self::DISK_STAL => ["C418 - stal", KnownTranslationFactory::item_record_stal_desc()],
    			self::DISK_STRAD => ["C418 - strad", KnownTranslationFactory::item_record_strad_desc()],
    			self::DISK_WARD => ["C418 - ward", KnownTranslationFactory::item_record_ward_desc()],
    			self::DISK_11 => ["C418 - 11", KnownTranslationFactory::item_record_11_desc()],
    			self::DISK_WAIT => ["C418 - wait", KnownTranslationFactory::item_record_wait_desc()]
    		};
    	}

    	public function getSoundName() : string{
    		return $this->getMetadata()[0];
    	}

    	/**
    	 * @deprecated
    	 */
    	public function getSoundId() : int{
    		return 0;
    	}

    	public function getTranslatableName() : Translatable{
    		return $this->getMetadata()[1];
    	}
    }

    enum RedstoneWireConnectionType{
    	case NONE;
    	case SIDE;
    	case UP;
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static SaplingType ACACIA()
     * @method static SaplingType BIRCH()
     * @method static SaplingType DARK_OAK()
     * @method static SaplingType JUNGLE()
     * @method static SaplingType OAK()
     * @method static SaplingType SPRUCE()
     */
    enum SaplingType{
    	use LegacyEnumShimTrait;

    	case OAK;
    	case SPRUCE;
    	case BIRCH;
    	case JUNGLE;
    	case ACACIA;
    	case DARK_OAK;
    	//TODO: cherry

    	public function getTreeType() : TreeType{
    		return match($this){
    			self::OAK => TreeType::OAK,
    			self::SPRUCE => TreeType::SPRUCE,
    			self::BIRCH => TreeType::BIRCH,
    			self::JUNGLE => TreeType::JUNGLE,
    			self::ACACIA => TreeType::ACACIA,
    			self::DARK_OAK => TreeType::DARK_OAK,
    		};
    	}

    	public function getDisplayName() : string{
    		return $this->getTreeType()->getDisplayName();
    	}
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static SlabType BOTTOM()
     * @method static SlabType DOUBLE()
     * @method static SlabType TOP()
     */
    enum SlabType{
    	use LegacyEnumShimTrait;

    	case BOTTOM;
    	case TOP;
    	case DOUBLE;
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static StairShape INNER_LEFT()
     * @method static StairShape INNER_RIGHT()
     * @method static StairShape OUTER_LEFT()
     * @method static StairShape OUTER_RIGHT()
     * @method static StairShape STRAIGHT()
     */
    enum StairShape{
    	use LegacyEnumShimTrait;

    	case STRAIGHT;
    	case INNER_LEFT;
    	case INNER_RIGHT;
    	case OUTER_LEFT;
    	case OUTER_RIGHT;
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static SupportType CENTER()
     * @method static SupportType EDGE()
     * @method static SupportType FULL()
     * @method static SupportType NONE()
     */
    enum SupportType{
    	use LegacyEnumShimTrait;

    	case FULL;
    	case CENTER;
    	case EDGE;
    	case NONE;

    	public function hasEdgeSupport() : bool{
    		return $this === self::EDGE || $this === self::FULL;
    	}

    	public function hasCenterSupport() : bool{
    		return $this === self::CENTER || $this === self::FULL;
    	}
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static WallConnectionType SHORT()
     * @method static WallConnectionType TALL()
     */
    enum WallConnectionType{
    	use LegacyEnumShimTrait;

    	case SHORT;
    	case TALL;
    }

    /**
     * TODO: These tags need to be removed once we get rid of LegacyEnumShimTrait (PM6)
     *  These are retained for backwards compatibility only.
     *
     * @method static WoodType ACACIA()
     * @method static WoodType BIRCH()
     * @method static WoodType CHERRY()
     * @method static WoodType CRIMSON()
     * @method static WoodType DARK_OAK()
     * @method static WoodType JUNGLE()
     * @method static WoodType MANGROVE()
     * @method static WoodType OAK()
     * @method static WoodType SPRUCE()
     * @method static WoodType WARPED()
     */
    enum WoodType{
    	use LegacyEnumShimTrait;

    	case OAK;
    	case SPRUCE;
    	case BIRCH;
    	case JUNGLE;
    	case ACACIA;
    	case DARK_OAK;
    	case MANGROVE;
    	case CRIMSON;
    	case WARPED;
    	case CHERRY;
    	case PALE_OAK;
    	case BAMBOO;

    	public function getDisplayName() : string{
    		return match($this){
    			self::OAK => "Oak",
    			self::SPRUCE => "Spruce",
    			self::BIRCH => "Birch",
    			self::JUNGLE => "Jungle",
    			self::ACACIA => "Acacia",
    			self::DARK_OAK => "Dark Oak",
    			self::MANGROVE => "Mangrove",
    			self::CRIMSON => "Crimson",
    			self::WARPED => "Warped",
    			self::CHERRY => "Cherry",
    			self::PALE_OAK => "Pale Oak",
    			self::BAMBOO => "Bamboo",
    		};
    	}

    	public function isFlammable() : bool{
    		return $this !== self::CRIMSON && $this !== self::WARPED;
    	}

    	public function getStandardLogSuffix() : ?string{
    		return match($this){
    			self::CRIMSON, self::WARPED => "Stem",
    			self::BAMBOO => "Block",
    			default => null,
    		};
    	}

    	public function getAllSidedLogSuffix() : ?string{
    		return $this === self::CRIMSON || $this === self::WARPED ? "Hyphae" : null;
    	}
    }

    // =========================================================================
    // Interfaces (16)
    // =========================================================================

    /**
     * Implemented by blocks that can emit a redstone signal.
     *
     * TODO: add methods (e.g. which sides wire connects to) in a major version; it's a BC break now because
     * AnalogRedstoneSignalEmitter extends this
     */
    interface RedstoneSignalEmitter{
    }

    interface Ageable{

    	public function getAge() : int;

    	public function getMaxAge() : int;

    	/**
    	 * Must be in range 0 - getMaxAge()
    	 * @return $this
    	 */
    	public function setAge(int $age) : self;
    }

    interface AnalogRedstoneSignalEmitter extends RedstoneSignalEmitter{

    	public function getOutputSignalStrength() : int;

    	/**
    	 * @return $this
    	 */
    	public function setOutputSignalStrength(int $signalStrength) : self;
    }

    interface AnyFacing{

    	/**
    	 * @see Facing
    	 */
    	public function getFacing() : int;

    	/**
    	 * @return $this
    	 *
    	 * @see Facing
    	 */
    	public function setFacing(int $facing) : self;
    }

    interface Colored{

    	public function getColor() : DyeColor;

    	/**
    	 * @return $this
    	 */
    	public function setColor(DyeColor $color) : self;
    }

    /**
     * Represents copper blocks that have oxidized and waxed variations.
     */
    interface CopperMaterial{

    	public function getOxidation() : CopperOxidation;

    	/**
    	 * @return $this
    	 */
    	public function setOxidation(CopperOxidation $oxidation) : CopperMaterial;

    	public function isWaxed() : bool;

    	/**
    	 * @return $this
    	 */
    	public function setWaxed(bool $waxed) : CopperMaterial;
    }

    interface CoralMaterial{

    	public function getCoralType() : CoralType;

    	/**
    	 * @return $this
    	 */
    	public function setCoralType(CoralType $coralType) : self;

    	public function isDead() : bool;

    	/**
    	 * @return $this
    	 */
    	public function setDead(bool $dead) : self;
    }

    interface Fallable{

    	/**
    	 * Called every tick by FallingBlock to update the falling state of this block. Used by concrete to check when it
    	 * hits water.
    	 * Return null if you don't want to change the usual behaviour.
    	 */
    	public function tickFalling() : ?Block;

    	/**
    	 * Called when FallingBlock hits the ground.
    	 * Returns whether the block should be placed.
    	 */
    	public function onHitGround(FallingBlock $blockEntity) : bool;

    	/**
    	 * Returns the damage caused per fallen block. This is multiplied by the fall distance (and capped according to
    	 * {@link Fallable::getMaxFallDamage()}) to calculate the damage dealt to any entities who intersect with the block
    	 * when it hits the ground.
    	 */
    	public function getFallDamagePerBlock() : float;

    	/**
    	 * Returns the maximum damage the block can deal to an entity when it hits the ground.
    	 */
    	public function getMaxFallDamage() : float;

    	/**
    	 * Returns the sound that will be played when FallingBlock hits the ground.
    	 */
    	public function getLandSound() : ?Sound;
    }

    /**
     * Implemented by blocks (fences, glass panes, bars, ...) that carry per-horizontal-facing connection state.
     */
    interface HorizontalConnectable{

    	/**
    	 * @param int $facing one of Facing::NORTH/EAST/SOUTH/WEST
    	 */
    	public function isConnectedAt(int $facing) : bool;

    	/**
    	 * @param int $facing one of Facing::NORTH/EAST/SOUTH/WEST
    	 */
    	public function setConnectedAt(int $facing, bool $connected) : void;
    }

    interface HorizontalFacing{

    	/**
    	 * @see Facing
    	 */
    	public function getFacing() : int;

    	/**
    	 * @return $this
    	 *
    	 * @see Facing
    	 */
    	public function setFacing(int $facing) : self;
    }

    interface Lightable{

    	public function isLit() : bool;

    	/**
    	 * @return $this
    	 */
    	public function setLit(bool $lit = true) : self;
    }

    interface MultiAnyFacing{

    	/**
    	 * @return int[]
    	 * @see Facing
    	 */
    	public function getFaces() : array;

    	public function hasFace(int $face) : bool;

    	/**
    	 * @return $this
    	 *
    	 * @see Facing
    	 */
    	public function setFace(int $face, bool $value) : self;

    	/**
    	 * @param int[] $faces
    	 *
    	 * @return $this
    	 *
    	 * @see Facing
    	 */
    	public function setFaces(array $faces) : self;

    }

    interface PillarRotation{

    	/** @see Axis */
    	public function getAxis() : int;

    	/**
    	 * @return $this
    	 *
    	 * @see Axis
    	 */
    	public function setAxis(int $axis) : self;
    }

    interface PoweredByRedstone{

    	public function isPowered() : bool;

    	/**
    	 * @return $this
    	 */
    	public function setPowered(bool $powered) : self;
    }

    interface SignLikeRotation{

    	/**
    	 * @return int between 0 and 15
    	 */
    	public function getRotation() : int;

    	/**
    	 * @param int $rotation between 0 and 15
    	 *
    	 * @return $this
    	 */
    	public function setRotation(int $rotation) : self;
    }

    interface WoodMaterial{

    	public function getWoodType() : WoodType;
    }

    // =========================================================================
    // Traits (22)
    // =========================================================================

    trait LightableTrait{
    	protected bool $lit = false;

    	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
    		$w->bool($this->lit);
    	}

    	public function isLit() : bool{
    		return $this->lit;
    	}

    	/**
    	 * @return $this
    	 */
    	public function setLit(bool $lit = true) : self{
    		$this->lit = $lit;
    		return $this;
    	}
    }

    trait HorizontalFacingTrait{
    	protected int $facing = Facing::NORTH;

    	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
    		$w->horizontalFacing($this->facing);
    	}

    	public function getFacing() : int{ return $this->facing; }

    	/** @return $this */
    	public function setFacing(int $facing) : self{
    		$axis = Facing::axis($facing);
    		if($axis !== Axis::X && $axis !== Axis::Z){
    			throw new \InvalidArgumentException("Facing must be horizontal");
    		}
    		$this->facing = $facing;
    		return $this;
    	}
    }

    /**
     * Used by blocks that can have multiple target faces in the area of one solid block, such as covering three sides of a corner.
     */
    trait MultiAnyFacingTrait{

    	/** @var int[] */
    	protected array $faces = [];

    	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
    		$w->facingFlags($this->faces);
    	}

    	/** @return int[] */
    	public function getFaces() : array{ return $this->faces; }

    	public function hasFace(int $face) : bool{
    		return isset($this->faces[$face]);
    	}

    	/**
    	 * @param int[] $faces
    	 * @return $this
    	 */
    	public function setFaces(array $faces) : self{
    		$uniqueFaces = [];
    		foreach($faces as $face){
    			Facing::validate($face);
    			$uniqueFaces[$face] = $face;
    		}
    		$this->faces = $uniqueFaces;
    		return $this;
    	}

    	/** @return $this */
    	public function setFace(int $face, bool $value) : self{
    		Facing::validate($face);
    		if($value){
    			$this->faces[$face] = $face;
    		}else{
    			unset($this->faces[$face]);
    		}
    		return $this;
    	}
    }

    trait PoweredByRedstoneTrait{
    	protected bool $powered = false;

    	public function isPowered() : bool{ return $this->powered; }

    	/** @return $this */
    	public function setPowered(bool $powered) : self{
    		$this->powered = $powered;
    		return $this;
    	}
    }

    /**
     * This trait is used for blocks that have an age property.
     * Need to add to the block the constant MAX_AGE.
     */
    trait AgeableTrait{
    	protected int $age = 0;

    	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
    		$w->boundedIntAuto(0, self::MAX_AGE, $this->age);
    	}

    	public function getAge() : int{ return $this->age; }

    	public function getMaxAge() : int{ return self::MAX_AGE; }

    	/**
    	 * @return $this
    	 */
    	public function setAge(int $age) : self{
    		if($age < 0 || $age > self::MAX_AGE){
    			throw new \InvalidArgumentException("Age must be in range 0 ... " . self::MAX_AGE);
    		}
    		$this->age = $age;
    		return $this;
    	}
    }

    trait AmethystTrait{
    	/**
    	 * @see Block::onProjectileHit()
    	 */
    	public function onProjectileHit(Projectile $projectile, RayTraceResult $hitResult) : void{
    		$this->position->getWorld()->addSound($this->position, new AmethystBlockChimeSound());
    		$this->position->getWorld()->addSound($this->position, new BlockPunchSound($this));
    	}
    }

    trait AnalogRedstoneSignalEmitterTrait{
    	protected int $signalStrength = 0;

    	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
    		$w->boundedIntAuto(0, 15, $this->signalStrength);
    	}

    	public function getOutputSignalStrength() : int{ return $this->signalStrength; }

    	/** @return $this */
    	public function setOutputSignalStrength(int $signalStrength) : self{
    		if($signalStrength < 0 || $signalStrength > 15){
    			throw new \InvalidArgumentException("Signal strength must be in range 0-15");
    		}
    		$this->signalStrength = $signalStrength;
    		return $this;
    	}
    }

    trait AnyFacingTrait{
    	protected int $facing = Facing::DOWN;

    	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
    		$w->facing($this->facing);
    	}

    	public function getFacing() : int{ return $this->facing; }

    	/** @return $this */
    	public function setFacing(int $facing) : self{
    		Facing::validate($this->facing);
    		$this->facing = $facing;
    		return $this;
    	}
    }

    trait CandleTrait{
    	use LightableTrait;

    	public function getLightLevel() : int{
    		return $this->lit ? 3 : 0;
    	}

    	/**
    	 * @param Item[] &$returnedItems
    	 * @see Block::onInteract()
    	 */
    	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
    		if($item->getTypeId() === ItemTypeIds::FIRE_CHARGE || $item->getTypeId() === ItemTypeIds::FLINT_AND_STEEL || $item->hasEnchantment(VanillaEnchantments::FIRE_ASPECT())){
    			if($this->lit){
    				return true;
    			}
    			if($item instanceof Durable){
    				$item->applyDamage(1);
    			}elseif($item->getTypeId() === ItemTypeIds::FIRE_CHARGE){
    				$item->pop();
    				//TODO: not sure if this is intentional, but it's what Bedrock currently does as of 1.20.10
    				$this->position->getWorld()->addSound($this->position, new BlazeShootSound());
    			}
    			$this->position->getWorld()->addSound($this->position, new FlintSteelSound());
    			$this->position->getWorld()->setBlock($this->position, $this->setLit(true));

    			return true;
    		}
    		if($item->isNull()){ //candle can only be extinguished with an empty hand
    			if(!$this->lit){
    				return true;
    			}
    			$this->position->getWorld()->addSound($this->position, new FireExtinguishSound());
    			$this->position->getWorld()->setBlock($this->position, $this->setLit(false));

    			return true;
    		}

    		//yes, this is intentional! in vanilla, if the candle is not interacted with, a block is placed.
    		return false;
    	}

    	/** @see Block::onProjectileHit() */
    	public function onProjectileHit(Projectile $projectile, RayTraceResult $hitResult) : void{
    		if(!$this->lit && $projectile->isOnFire()){
    			$this->position->getWorld()->setBlock($this->position, $this->setLit(true));
    		}
    	}
    }

    trait ColoredTrait{
    	/** @var DyeColor */
    	private $color = DyeColor::WHITE;

    	/** @see Block::describeBlockItemState() */
    	public function describeBlockItemState(RuntimeDataDescriber $w) : void{
    		$w->enum($this->color);
    	}

    	public function getColor() : DyeColor{ return $this->color; }

    	/** @return $this */
    	public function setColor(DyeColor $color) : self{
    		$this->color = $color;
    		return $this;
    	}
    }

    trait CopperTrait{
    	private CopperOxidation $oxidation = CopperOxidation::NONE;
    	private bool $waxed = false;

    	public function describeBlockItemState(RuntimeDataDescriber $w) : void{
    		$w->enum($this->oxidation);
    		$w->bool($this->waxed);
    	}

    	public function getOxidation() : CopperOxidation{ return $this->oxidation; }

    	/** @return $this */
    	public function setOxidation(CopperOxidation $oxidation) : self{
    		$this->oxidation = $oxidation;
    		return $this;
    	}

    	public function isWaxed() : bool{ return $this->waxed; }

    	/** @return $this */
    	public function setWaxed(bool $waxed) : self{
    		$this->waxed = $waxed;
    		return $this;
    	}

    	/**
    	 * @param Item[] &$returnedItems
    	 * @see Block::onInteract()
    	 */
    	public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []) : bool{
    		if(!$this->waxed && $item->getTypeId() === ItemTypeIds::HONEYCOMB){
    			$this->waxed = true;
    			$this->position->getWorld()->setBlock($this->position, $this);
    			//TODO: orange particles are supposed to appear when applying wax
    			$this->position->getWorld()->addSound($this->position, new CopperWaxApplySound());
    			$item->pop();
    			return true;
    		}

    		if($item instanceof Axe){
    			if($this->waxed){
    				$this->waxed = false;
    				$this->position->getWorld()->setBlock($this->position, $this);
    				//TODO: white particles are supposed to appear when removing wax
    				$this->position->getWorld()->addSound($this->position, new CopperWaxRemoveSound());
    				$item->applyDamage(1);
    				return true;
    			}

    			$previousOxidation = $this->oxidation->getPrevious();
    			if($previousOxidation !== null){
    				$this->oxidation = $previousOxidation;
    				$this->position->getWorld()->setBlock($this->position, $this);
    				//TODO: turquoise particles are supposed to appear when removing oxidation
    				$this->position->getWorld()->addSound($this->position, new ScrapeSound());
    				$item->applyDamage(1);
    				return true;
    			}
    		}

    		return false;
    	}
    }

    trait CoralTypeTrait{
    	protected CoralType $coralType = CoralType::TUBE;
    	protected bool $dead = false;

    	/** @see Block::describeBlockItemState() */
    	public function describeBlockItemState(RuntimeDataDescriber $w) : void{
    		$w->enum($this->coralType);
    		$w->bool($this->dead);
    	}

    	public function getCoralType() : CoralType{ return $this->coralType; }

    	/** @return $this */
    	public function setCoralType(CoralType $coralType) : self{
    		$this->coralType = $coralType;
    		return $this;
    	}

    	public function isDead() : bool{ return $this->dead; }

    	/** @return $this */
    	public function setDead(bool $dead) : self{
    		$this->dead = $dead;
    		return $this;
    	}
    }

    trait FacesOppositePlacingPlayerTrait{
    	use HorizontalFacingTrait;

    	/**
    	 * @see Block::place()
    	 */
    	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
    		if($player !== null){
    			$this->facing = Facing::opposite($player->getHorizontalFacing());
    		}
    		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
    	}
    }

    /**
     * This trait handles falling behaviour for blocks that need them.
     * TODO: convert this into a dynamic component
     * @see Fallable
     */
    trait FallableTrait{

    	abstract protected function getPosition() : Position;

    	public function onNearbyBlockChange() : void{
    		$pos = $this->getPosition();
    		$world = $pos->getWorld();
    		$down = $world->getBlock($pos->getSide(Facing::DOWN));
    		if($down->canBeReplaced()){
    			$world->setBlock($pos, VanillaBlocks::AIR());

    			$block = $this;
    			if(!($block instanceof Block)) throw new AssumptionFailedError(__TRAIT__ . " should only be used by Blocks");

    			$fall = new FallingBlock(Location::fromObject($pos->add(0.5, 0, 0.5), $world), $block);
    			$fall->spawnToAll();
    		}
    	}

    	public function tickFalling() : ?Block{
    		return null;
    	}

    	public function onHitGround(FallingBlock $blockEntity) : bool{
    		return true;
    	}

    	public function getFallDamagePerBlock() : float{
    		return 0.0;
    	}

    	public function getMaxFallDamage() : float{
    		return 0.0;
    	}

    	public function getLandSound() : ?Sound{
    		return null;
    	}
    }

    trait HorizontalConnectableTrait{
    	/** @var int[] facing => facing */
    	protected array $connections = [];

    	/**
    	 * @see Block::describeBlockOnlyState()
    	 */
    	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
    		$w->horizontalFacingFlags($this->connections);
    	}

    	public function isConnectedAt(int $facing) : bool{
    		return isset($this->connections[$facing]);
    	}

    	public function setConnectedAt(int $facing, bool $connected) : void{
    		if($connected){
    			$this->connections[$facing] = $facing;
    		}else{
    			unset($this->connections[$facing]);
    		}
    	}

    	/**
    	 * @see Block::onNearbyBlockChange()
    	 */
    	public function onNearbyBlockChange() : void{
    		if($this->recalculateConnections()){
    			$this->position->getWorld()->setBlock($this->position, $this);
    		}
    	}

    	/**
    	 * Implement this to (re)compute connections from the block's neighbours. Must return whether anything changed.
    	 */
    	abstract protected function recalculateConnections() : bool;
    }

    /**
     * Used by blocks that have multiple support requirements in the area of one solid block, such as covering three sides of a corner.
     * Prevents placement if support isn't available and automatically destroys a block side if it's support is removed.
     */
    trait MultiAnySupportTrait{
    	use MultiAnyFacingTrait;

    	/**
    	 * Returns a list of faces that block should already have when placed.
    	 *
    	 * @return int[]
    	 */
    	abstract protected function getInitialPlaceFaces(Block $blockReplace) : array;

    	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
    		$this->faces = $this->getInitialPlaceFaces($blockReplace);
    		$availableFaces = $this->getAvailableFaces();

    		if(count($availableFaces) === 0){
    			return false;
    		}

    		$opposite = Facing::opposite($face);
    		$placedFace = isset($availableFaces[$opposite]) ? $opposite : array_key_first($availableFaces);
    		$this->faces[$placedFace] = $placedFace;

    		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
    	}

    	public function onNearbyBlockChange() : void{
    		$changed = false;

    		foreach($this->faces as $face){
    			if($this->getAdjacentSupportType($face) !== SupportType::FULL){
    				unset($this->faces[$face]);
    				$changed = true;
    			}
    		}

    		if($changed){
    			$world = $this->position->getWorld();
    			if(count($this->faces) === 0){
    				$world->useBreakOn($this->position);
    			}else{
    				$world->setBlock($this->position, $this);
    			}
    		}
    	}

    	/**
    	 * @return array<int, int> $faces
    	 */
    	private function getAvailableFaces() : array{
    		$faces = [];
    		foreach(Facing::ALL as $face){
    			if(!$this->hasFace($face) && $this->getAdjacentSupportType($face) === SupportType::FULL){
    				$faces[$face] = $face;
    			}
    		}
    		return $faces;
    	}
    }

    trait PillarRotationTrait{
    	protected int $axis = Axis::Y;

    	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
    		$w->axis($this->axis);
    	}

    	/** @see Axis */
    	public function getAxis() : int{ return $this->axis; }

    	/** @return $this */
    	public function setAxis(int $axis) : self{
    		$this->axis = $axis;
    		return $this;
    	}

    	/**
    	 * @see Block::place()
    	 */
    	public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null) : bool{
    		$this->axis = Facing::axis($face);
    		/** @see Block::place() */
    		return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
    	}
    }

    trait RailPoweredByRedstoneTrait{
    	use PoweredByRedstoneTrait;

    	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
    		parent::describeBlockOnlyState($w);
    		$w->bool($this->powered);
    	}
    }

    trait SignLikeRotationTrait{
    	/** @var int */
    	private $rotation = 0;

    	protected function describeBlockOnlyState(RuntimeDataDescriber $w) : void{
    		$w->boundedIntAuto(0, 15, $this->rotation);
    	}

    	public function getRotation() : int{ return $this->rotation; }

    	/** @return $this */
    	public function setRotation(int $rotation) : self{
    		if($rotation < 0 || $rotation > 15){
    			throw new \InvalidArgumentException("Rotation must be in range 0-15");
    		}
    		$this->rotation = $rotation;
    		return $this;
    	}

    	private static function getRotationFromYaw(float $yaw) : int{
    		return ((int) floor((($yaw + 180) * 16 / 360) + 0.5)) & 0xf;
    	}
    }

    /**
     * Used by blocks which always have the same support requirements no matter what state they are in.
     * Prevents placement if support isn't available, and automatically destroys itself if support is removed.
     */
    trait StaticSupportTrait{

    	/**
    	 * Implement this to define the block's support requirements.
    	 */
    	abstract private function canBeSupportedAt(Block $block) : bool;

    	/**
    	 * @see Block::canBePlacedAt()
    	 */
    	public function canBePlacedAt(Block $blockReplace, Vector3 $clickVector, int $face, bool $isClickedBlock) : bool{
    		return $this->canBeSupportedAt($blockReplace) && parent::canBePlacedAt($blockReplace, $clickVector, $face, $isClickedBlock);
    	}

    	/**
    	 * @see Block::onNearbyBlockChange()
    	 */
    	public function onNearbyBlockChange() : void{
    		if(!$this->canBeSupportedAt($this)){
    			$this->position->getWorld()->useBreakOn($this->position);
    		}else{
    			parent::onNearbyBlockChange();
    		}
    	}
    }

    /**
     * @internal
     */
    trait TallGrassTrait{
    	public function canBeReplaced() : bool{
    		return true;
    	}

    	public function getDropsForIncompatibleTool(Item $item) : array{
    		if(FortuneDropHelper::bonusChanceDivisor($item, 8, 2)){
    			return [
    				VanillaItems::WHEAT_SEEDS()
    			];
    		}

    		return [];
    	}

    	public function getFlameEncouragement() : int{
    		return 60;
    	}

    	public function getFlammability() : int{
    		return 100;
    	}
    }

    trait WoodTypeTrait{
    	private WoodType $woodType; //immutable for now

    	public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, WoodType $woodType){
    		$this->woodType = $woodType;
    		parent::__construct($idInfo, $name, $typeInfo);
    	}

    	public function getWoodType() : WoodType{
    		return $this->woodType;
    	}
    }

    // =========================================================================
    // Classes (8)
    // =========================================================================

    /**
     * Contains information about a pattern layer on a banner.
     * @see BaseBanner
     */
    class BannerPatternLayer{
    	public function __construct(
    		private BannerPatternType $type,
    		private DyeColor $color
    	){}

    	public function getType() : BannerPatternType{ return $this->type; }

    	public function getColor() : DyeColor{
    		return $this->color;
    	}
    }

    /**
     * Helper class to call block changing events and apply the results to the world.
     * TODO: try to further reduce the amount of code duplication here - while this is much better than before, it's still
     * very repetitive.
     */
    final class BlockEventHelper{

    	public static function grow(Block $oldState, Block $newState, ?Player $causingPlayer) : bool{
    		if(BlockGrowEvent::hasHandlers()){
    			$ev = new BlockGrowEvent($oldState, $newState, $causingPlayer);
    			$ev->call();
    			if($ev->isCancelled()){
    				return false;
    			}
    			$newState = $ev->getNewState();
    		}

    		$position = $oldState->getPosition();
    		$position->getWorld()->setBlock($position, $newState);
    		return true;
    	}

    	public static function spread(Block $oldState, Block $newState, Block $source) : bool{
    		if(BlockSpreadEvent::hasHandlers()){
    			$ev = new BlockSpreadEvent($oldState, $source, $newState);
    			$ev->call();
    			if($ev->isCancelled()){
    				return false;
    			}
    			$newState = $ev->getNewState();
    		}

    		$position = $oldState->getPosition();
    		$position->getWorld()->setBlock($position, $newState);
    		return true;
    	}

    	public static function form(Block $oldState, Block $newState, Block $cause) : bool{
    		if(BlockFormEvent::hasHandlers()){
    			$ev = new BlockFormEvent($oldState, $newState, $cause);
    			$ev->call();
    			if($ev->isCancelled()){
    				return false;
    			}
    			$newState = $ev->getNewState();
    		}

    		$position = $oldState->getPosition();
    		$position->getWorld()->setBlock($position, $newState);
    		return true;
    	}

    	public static function melt(Block $oldState, Block $newState) : bool{
    		if(BlockMeltEvent::hasHandlers()){
    			$ev = new BlockMeltEvent($oldState, $newState);
    			$ev->call();
    			if($ev->isCancelled()){
    				return false;
    			}
    			$newState = $ev->getNewState();
    		}

    		$position = $oldState->getPosition();
    		$position->getWorld()->setBlock($position, $newState);
    		return true;
    	}

    	public static function die(Block $oldState, Block $newState) : bool{
    		if(BlockDeathEvent::hasHandlers()){
    			$ev = new BlockDeathEvent($oldState, $newState);
    			$ev->call();
    			if($ev->isCancelled()){
    				return false;
    			}
    			$newState = $ev->getNewState();
    		}

    		$position = $oldState->getPosition();
    		$position->getWorld()->setBlock($position, $newState);
    		return true;
    	}
    }

    final class CropGrowthHelper{

    	private const ON_HYDRATED_FARMLAND_BONUS = 3;
    	private const ON_DRY_FARMLAND_BONUS = 1;
    	private const ADJACENT_HYDRATED_FARMLAND_BONUS = 3 / 4;
    	private const ADJACENT_DRY_FARMLAND_BONUS = 1 / 4;

    	private const IMPROPER_ARRANGEMENT_DIVISOR = 2;

    	private const MIN_LIGHT_LEVEL = 9;

    	private function __construct(){
    		//NOOP
    	}

    	/**
    	 * Returns the speed at which this crop will grow, depending on its surroundings.
    	 * The default is once every 26 random ticks.
    	 *
    	 * Things which influence this include nearby farmland (bonus for hydrated farmland) and the position of other
    	 * nearby crops of the same type (nearby crops of the same type will negatively influence growth speed unless
    	 * planted in rows and properly spaced apart).
    	 */
    	public static function calculateMultiplier(Block $block) : float{
    		$result = 1;

    		$position = $block->getPosition();

    		$world = $position->getWorld();
    		$baseX = $position->getFloorX();
    		$baseY = $position->getFloorY();
    		$baseZ = $position->getFloorZ();

    		$farmland = $world->getBlockAt($baseX, $baseY - 1, $baseZ);

    		if($farmland instanceof Farmland){
    			$result += $farmland->getWetness() > 0 ? self::ON_HYDRATED_FARMLAND_BONUS : self::ON_DRY_FARMLAND_BONUS;
    		}

    		$xRow = false;
    		$zRow = false;
    		$improperArrangement = false;

    		for($x = -1; $x <= 1; $x++){
    			for($z = -1; $z <= 1; $z++){
    				if($x === 0 && $z === 0){
    					continue;
    				}
    				$nextFarmland = $world->getBlockAt($baseX + $x, $baseY - 1, $baseZ + $z);

    				if(!$nextFarmland instanceof Farmland){
    					continue;
    				}

    				$result += $nextFarmland->getWetness() > 0 ? self::ADJACENT_HYDRATED_FARMLAND_BONUS : self::ADJACENT_DRY_FARMLAND_BONUS;

    				if(!$improperArrangement){
    					$nextCrop = $world->getBlockAt($baseX + $x, $baseY, $baseZ + $z);
    					if($nextCrop->hasSameTypeId($block)){
    						match(0){
    							$x => $zRow ? $improperArrangement = true : $xRow = true,
    							$z => $xRow ? $improperArrangement = true : $zRow = true,
    							default => $improperArrangement = true,
    						};
    					}
    				}
    			}
    		}

    		//crops can be arranged in rows, but the rows must not cross and must be spaced apart by at least one block
    		if($improperArrangement){
    			$result /= self::IMPROPER_ARRANGEMENT_DIVISOR;
    		}

    		return $result;
    	}

    	public static function hasEnoughLight(Block $block, int $minLevel = self::MIN_LIGHT_LEVEL) : bool{
    		$position = $block->getPosition();
    		$world = $position->getWorld();

    		//crop growth is not affected by time of day since 1.11 or so
    		return $world->getPotentialLightAt($position->x, $position->y, $position->z) >= $minLevel;
    	}

    	public static function canGrow(Block $block) : bool{
    		//while it may be tempting to use mt_rand(0, 25) < multiplier, this would make crops grow a bit faster than
    		//vanilla in most cases due to the remainder of 25 / multiplier not being discarded
    		return mt_rand(0, (int) (25 / self::calculateMultiplier($block))) === 0 && self::hasEnoughLight($block);
    	}
    }

    final class FortuneDropHelper{
    	/**
    	 * If a random number between 0-1 is greater than 2/(level+2), this multiplies the max drop amount by level+1, and
    	 * picks a random amount between the minimum and multiplied maximum. Each level of fortune increases the chance of
    	 * fortune activation, and also increases the maximum drop limit when activated.
    	 *
    	 * Otherwise, returns a random amount of the item between the minimum and original maximum.
    	 *
    	 * @param Item $usedItem The item used to break the block
    	 * @param int  $min      Minimum amount
    	 * @param int  $maxBase  Maximum amount, as if fortune level was 0
    	 *
    	 * @return int the number of items to drop
    	 */
    	public static function weighted(Item $usedItem, int $min, int $maxBase) : int{
    		if($maxBase < $min){
    			throw new \InvalidArgumentException("Maximum drop amount must be greater than or equal to minimum drop amount");
    		}

    		$fortuneLevel = $usedItem->getEnchantmentLevel(VanillaEnchantments::FORTUNE());

    		return mt_rand($min,
    			$fortuneLevel > 0 && mt_rand() / mt_getrandmax() > 2 / ($fortuneLevel + 2) ?
    				$maxBase * ($fortuneLevel + 1) :
    				$maxBase
    		);
    	}

    	/**
    	 * Increases the drop amount according to a binomial distribution. The function will roll maxBase+level times, and add 1
    	 * if a random number between 0-1 is less than the given probability. Each level of fortune adds one extra roll.
    	 *
    	 * As many as maxBase+level items can be dropped. This applies even if the fortune level is 0.
    	 *
    	 * @param float $chance   The chance of adding 1 to the amount for each roll, must be in the range 0-1
    	 * @param int   $min      Minimum amount
    	 * @param int   $minRolls Number of rolls if fortune level is 0, added to fortune level to calculate total rolls
    	 *
    	 * @return int the number of items to drop
    	 */
    	public static function binomial(Item $usedItem, int $min, int $minRolls = 3, float $chance = 4 / 7) : int{
    		$fortuneLevel = $usedItem->getEnchantmentLevel(VanillaEnchantments::FORTUNE());

    		$count = $min;
    		$rolls = $minRolls + $fortuneLevel;
    		for($i = 0; $i < $rolls; ++$i){
    			if(mt_rand() / mt_getrandmax() < $chance){
    				++$count;
    			}
    		}

    		return $count;
    	}

    	/**
    	 * Adds the fortune level to the base max and picks a random number between the minimim and adjusted maximum.
    	 * Each amount in the range has an equal chance of being picked.
    	 *
    	 * @param int $maxBase Maximum base amount, as if the fortune level was 0
    	 *
    	 * @return int the number of items to drop
    	 */
    	public static function discrete(Item $usedItem, int $min, int $maxBase) : int{
    		if($maxBase < $min){
    			throw new \InvalidArgumentException("Minimum base drop amount must be less than or equal to maximum base drop amount");
    		}

    		$max = $maxBase + $usedItem->getEnchantmentLevel(VanillaEnchantments::FORTUNE());
    		return mt_rand($min, $max);
    	}

    	/**
    	 * Calculates a chance of getting an extra bonus drop by reducing the chance divisor by a given amount per fortune
    	 * level.
    	 *
    	 * @param int $divisorBase             The number to divide 1 by to get the chance, as if the fortune level was 0
    	 * @param int $divisorSubtractPerLevel The amount to subtract from the divisor for each level of fortune
    	 *
    	 * @return bool whether the bonus drop should be added
    	 */
    	public static function bonusChanceDivisor(Item $usedItem, int $divisorBase, int $divisorSubtractPerLevel) : bool{
    		$fortuneLevel = $usedItem->getEnchantmentLevel(VanillaEnchantments::FORTUNE());
    		return mt_rand(1, max(1, $divisorBase - ($fortuneLevel * $divisorSubtractPerLevel))) === 1;
    	}

    	/**
    	 * Calculates a chance of getting an extra bonus drop by increasing the chance by a fixed amount per fortune level.
    	 *
    	 * @param float $chanceBase          The base chance of getting a bonus drop, as if the fortune level was 0
    	 * @param float $addedChancePerLevel The amount to add to the chance for each level of fortune
    	 */
    	public static function bonusChanceFixed(Item $usedItem, float $chanceBase, float $addedChancePerLevel) : bool{
    		$fortuneLevel = $usedItem->getEnchantmentLevel(VanillaEnchantments::FORTUNE());
    		$chance = min(1, $chanceBase + ($fortuneLevel * $addedChancePerLevel));
    		return mt_rand() / mt_getrandmax() < $chance;
    	}
    }

    /**
     * Calculates the path(s) of least resistance for liquid flow.
     */
    final class MinimumCostFlowCalculator{

    	private const CAN_FLOW_DOWN = 1;
    	private const CAN_FLOW = 0;
    	private const BLOCKED = -1;

    	/** @var int[] */
    	private array $flowCostVisited = [];

    	/**
    	 * @phpstan-param \Closure(Block) : bool $canFlowInto
    	 */
    	public function __construct(
    		private World $world,
    		private int $flowDecayPerBlock,
    		private \Closure $canFlowInto
    	){}

    	private function calculateFlowCost(int $blockX, int $blockY, int $blockZ, int $accumulatedCost, int $maxCost, int $originOpposite, int $lastOpposite) : int{
    		$cost = 1000;

    		foreach(Facing::HORIZONTAL as $j){
    			if($j === $originOpposite || $j === $lastOpposite){
    				continue;
    			}
    			[$dx, $dy, $dz] = Facing::OFFSET[$j];
    			$x = $blockX + $dx;
    			$y = $blockY + $dy;
    			$z = $blockZ + $dz;

    			if(!isset($this->flowCostVisited[$hash = World::blockHash($x, $y, $z)])){
    				if(!$this->world->isInWorld($x, $y, $z) || !$this->canFlowInto($this->world->getBlockAt($x, $y, $z))){
    					$this->flowCostVisited[$hash] = self::BLOCKED;
    				}elseif($this->world->getBlockAt($x, $y - 1, $z)->canBeFlowedInto()){
    					$this->flowCostVisited[$hash] = self::CAN_FLOW_DOWN;
    				}else{
    					$this->flowCostVisited[$hash] = self::CAN_FLOW;
    				}
    			}

    			$status = $this->flowCostVisited[$hash];

    			if($status === self::BLOCKED){
    				continue;
    			}elseif($status === self::CAN_FLOW_DOWN){
    				return $accumulatedCost;
    			}

    			if($accumulatedCost >= $maxCost){
    				continue;
    			}

    			$realCost = $this->calculateFlowCost($x, $y, $z, $accumulatedCost + 1, $maxCost, $originOpposite, Facing::opposite($j));

    			if($realCost < $cost){
    				$cost = $realCost;
    			}
    		}

    		return $cost;
    	}

    	/**
    	 * @return int[]
    	 */
    	public function getOptimalFlowDirections(int $originX, int $originY, int $originZ) : array{
    		$flowCost = array_fill_keys(Facing::HORIZONTAL, 1000);
    		$maxCost = intdiv(4, $this->flowDecayPerBlock);
    		foreach(Facing::HORIZONTAL as $j){
    			[$dx, $dy, $dz] = Facing::OFFSET[$j];
    			$x = $originX + $dx;
    			$y = $originY + $dy;
    			$z = $originZ + $dz;

    			if(!$this->world->isInWorld($x, $y, $z) || !$this->canFlowInto($this->world->getBlockAt($x, $y, $z))){
    				$this->flowCostVisited[World::blockHash($x, $y, $z)] = self::BLOCKED;
    			}elseif($this->world->getBlockAt($x, $y - 1, $z)->canBeFlowedInto()){
    				$this->flowCostVisited[World::blockHash($x, $y, $z)] = self::CAN_FLOW_DOWN;
    				$flowCost[$j] = $maxCost = 0;
    			}elseif($maxCost > 0){
    				$this->flowCostVisited[World::blockHash($x, $y, $z)] = self::CAN_FLOW;
    				$opposite = Facing::opposite($j);
    				$flowCost[$j] = $this->calculateFlowCost($x, $y, $z, 1, $maxCost, $opposite, $opposite);
    				$maxCost = min($maxCost, $flowCost[$j]);
    			}
    		}

    		$this->flowCostVisited = [];

    		$minCost = min($flowCost);

    		$isOptimalFlowDirection = [];

    		foreach($flowCost as $facing => $cost){
    			if($cost === $minCost){
    				$isOptimalFlowDirection[] = $facing;
    			}
    		}

    		return $isOptimalFlowDirection;
    	}

    	private function canFlowInto(Block $block) : bool{
    		return ($this->canFlowInto)($block);
    	}
    }

    final class RailConnectionInfo{

    	public const FLAG_ASCEND = 1 << 24; //used to indicate direction-up

    	public const CONNECTIONS = [
    		//straights
    		BlockLegacyMetadata::RAIL_STRAIGHT_NORTH_SOUTH => [
    			Facing::NORTH,
    			Facing::SOUTH
    		],
    		BlockLegacyMetadata::RAIL_STRAIGHT_EAST_WEST => [
    			Facing::EAST,
    			Facing::WEST
    		],

    		//ascending
    		BlockLegacyMetadata::RAIL_ASCENDING_EAST => [
    			Facing::WEST,
    			Facing::EAST | self::FLAG_ASCEND
    		],
    		BlockLegacyMetadata::RAIL_ASCENDING_WEST => [
    			Facing::EAST,
    			Facing::WEST | self::FLAG_ASCEND
    		],
    		BlockLegacyMetadata::RAIL_ASCENDING_NORTH => [
    			Facing::SOUTH,
    			Facing::NORTH | self::FLAG_ASCEND
    		],
    		BlockLegacyMetadata::RAIL_ASCENDING_SOUTH => [
    			Facing::NORTH,
    			Facing::SOUTH | self::FLAG_ASCEND
    		]
    	];

    	/* extended meta values for regular rails, to allow curving */
    	public const CURVE_CONNECTIONS = [
    		BlockLegacyMetadata::RAIL_CURVE_SOUTHEAST => [
    			Facing::SOUTH,
    			Facing::EAST
    		],
    		BlockLegacyMetadata::RAIL_CURVE_SOUTHWEST => [
    			Facing::SOUTH,
    			Facing::WEST
    		],
    		BlockLegacyMetadata::RAIL_CURVE_NORTHWEST => [
    			Facing::NORTH,
    			Facing::WEST
    		],
    		BlockLegacyMetadata::RAIL_CURVE_NORTHEAST => [
    			Facing::NORTH,
    			Facing::EAST
    		]
    	];
    }

    /**
     * Computes the position-seeded random offset Bedrock applies to a block's visual and collision shape (its
     * minecraft:random_offset component). Used by bamboo, pointed dripstone, sulfur spike and similar blocks, where a
     * position hash seeds a Xoroshiro128++ generator and each axis draws one value quantised into the configured range
     * and step count.
     */
    final class RandomOffsetGenerator{

    	private const MASK64 = "0xffffffffffffffff";

    	private static function mask(\GMP $g) : \GMP{
    		return gmp_and($g, gmp_init(self::MASK64));
    	}

    	private static function shr(\GMP $g, int $n) : \GMP{
    		return gmp_div_q(self::mask($g), gmp_pow(2, $n));
    	}

    	private static function shl(\GMP $g, int $n) : \GMP{
    		return self::mask(gmp_mul($g, gmp_pow(2, $n)));
    	}

    	/** Arithmetic (sign-propagating) shift right. */
    	private static function ashr(\GMP $g, int $n) : \GMP{
    		$r = self::shr($g, $n);
    		if(gmp_testbit($g, 63)){
    			$r = gmp_or($r, self::shl(gmp_sub(gmp_pow(2, $n), 1), 64 - $n));
    		}
    		return $r;
    	}

    	private static function sext32(\GMP $g) : \GMP{
    		$low = gmp_and($g, gmp_init("0xffffffff"));
    		if(gmp_testbit($low, 31)){
    			$low = gmp_or($low, gmp_init("0xffffffff00000000"));
    		}
    		return $low;
    	}

    	private static function toSigned(\GMP $g) : int{
    		$masked = self::mask($g);
    		if(gmp_cmp($masked, gmp_init("0x7fffffffffffffff")) > 0){
    			$masked = gmp_sub($masked, gmp_pow(2, 64));
    		}
    		return gmp_intval($masked);
    	}

    	/**
    	 * Hashes a block position down to the 64-bit seed the generator is built from.
    	 *
    	 * X is read as unsigned and Z as signed, which is not symmetrical but is what the game does, so it has to be
    	 * reproduced rather than tidied up. The result is fed to Xoroshiro128PP::fromSeed(), which applies the
    	 * silver ratio mix that completes the hash.
    	 */
    	private static function positionHash(int $x, int $z) : int{
    		$xu = gmp_and(gmp_init($x), gmp_init("0xffffffff"));
    		$a = self::ashr(self::mask(gmp_mul(gmp_init("0x2fc20f00000001"), $xu)), 32);
    		$b = self::mask(gmp_mul(gmp_init($z), gmp_init("0x6ebfff5")));
    		$v1 = self::mask(gmp_xor($a, $b));

    		$c = self::mask(gmp_add(gmp_mul(gmp_init("0x285b825"), $v1), gmp_init(11)));
    		return self::toSigned(self::sext32(self::shr(self::mask(gmp_mul($v1, $c)), 16)));
    	}

    	/**
    	 * Snaps a random value into one of the configured steps across [$min, $max].
    	 *
    	 * Note the asymmetry: the index is scaled by $steps but spread across $steps - 1 intervals, so the endpoints are
    	 * both reachable. A step count of 0 means the axis is continuous, and 1 means it is pinned to the midpoint.
    	 */
    	private static function quantize(float $min, float $max, int $steps, float $random) : float{
    		if($min >= $max){
    			return $min;
    		}
    		if($steps === 1){
    			return ($min + $max) * 0.5;
    		}
    		if($steps > 1){
    			$index = (float) (int) ($steps * $random);
    			return $min + $index * (($max - $min) / ($steps - 1));
    		}
    		return $min + ($max - $min) * $random;
    	}

    	/**
    	 * Computes the horizontal (XZ) random offset for a block at the given position. The Y draw is consumed but
    	 * unused, exactly as Bedrock does in XZ mode.
    	 *
    	 * @return float[] [$offsetX, $offsetZ]
    	 * @phpstan-return array{float, float}
    	 */
    	public static function horizontal(int $x, int $z, float $min, float $max, int $steps) : array{
    		$random = Xoroshiro128PP::fromSeed(self::positionHash($x, $z));

    		$offsetX = self::quantize($min, $max, $steps, $random->nextFloat());
    		$random->nextFloat(); //the Y axis draw is consumed even though its range is empty in XZ mode
    		$offsetZ = self::quantize($min, $max, $steps, $random->nextFloat());

    		return [$offsetX, $offsetZ];
    	}
    }

    class SignText{
    	public const LINE_COUNT = 4;

    	/**
    	 * @var string[]
    	 * @phpstan-var array{0: string, 1: string, 2: string, 3: string}
    	 */
    	private array $lines;
    	private Color $baseColor;
    	private bool $glowing;

    	/**
    	 * @param string[]|null $lines index-sensitive; keys 0-3 will be used, regardless of array order
    	 * @phpstan-param array{0?: string, 1?: string, 2?: string, 3?: string}|null $lines
    	 *
    	 * @throws \InvalidArgumentException if the array size is greater than 4
    	 * @throws \InvalidArgumentException if invalid keys (out of bounds or string) are found in the array
    	 * @throws \InvalidArgumentException if any line is not valid UTF-8 or contains a newline
    	 */
    	public function __construct(?array $lines = null, ?Color $baseColor = null, bool $glowing = false){
    		$this->lines = array_fill(0, self::LINE_COUNT, "");
    		if($lines !== null){
    			if(count($lines) > self::LINE_COUNT){
    				throw new \InvalidArgumentException("Expected at most 4 lines, got " . count($lines));
    			}
    			foreach($lines as $k => $line){
    				$this->checkLineIndex($k);
    				Utils::checkUTF8($line);
    				if(str_contains($line, "\n")){
    					throw new \InvalidArgumentException("Line must not contain newlines");
    				}
    				//TODO: add length checks
    				$this->lines[$k] = $line;
    			}
    		}
    		$this->baseColor = $baseColor ?? new Color(0, 0, 0);
    		$this->glowing = $glowing;
    	}

    	/**
    	 * Parses sign lines from the given string blob.
    	 * TODO: add a strict mode for this
    	 *
    	 * @throws \InvalidArgumentException if the text is not valid UTF-8
    	 */
    	public static function fromBlob(string $blob, ?Color $baseColor = null, bool $glowing = false) : SignText{
    		return new self(array_slice(array_pad(explode("\n", $blob, limit: self::LINE_COUNT + 1), self::LINE_COUNT, ""), 0, self::LINE_COUNT), $baseColor, $glowing);
    	}

    	/**
    	 * Returns an array of lines currently on the sign.
    	 *
    	 * @return string[]
    	 * @phpstan-return array{0: string, 1: string, 2: string, 3: string}
    	 */
    	public function getLines() : array{
    		return $this->lines;
    	}

    	private function checkLineIndex(int|string $index) : void{
    		if(!is_int($index)){
    			throw new \InvalidArgumentException("Index must be an integer");
    		}
    		if($index < 0 || $index >= self::LINE_COUNT){
    			throw new \InvalidArgumentException("Line index is out of bounds");
    		}
    	}

    	/**
    	 * Returns the sign line at the given offset.
    	 *
    	 * @throws \InvalidArgumentException
    	 */
    	public function getLine(int $index) : string{
    		$this->checkLineIndex($index);
    		return $this->lines[$index];
    	}

    	/**
    	 * Returns the base text color of sign. Color codes using the § escape character will override this color when used.
    	 */
    	public function getBaseColor() : Color{
    		return $this->baseColor;
    	}

    	/**
    	 * Returns whether the sign text is glowing. When true, the text will have an outline (usually a darker tone of the
    	 * base color, or white for black text), and will glow in the dark, making it readable without any light sources.
    	 */
    	public function isGlowing() : bool{
    		return $this->glowing;
    	}
    }
}
