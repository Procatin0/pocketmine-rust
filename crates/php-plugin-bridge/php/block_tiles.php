<?php

declare(strict_types=1);

/**
 * PM5 Block Entity (Tile) contract and implementations.
 *
 * All 39 Tile classes from PocketMine-MP / Axolotl-PM ported to the PHP plugin bridge:
 * - Base & Interfaces: Tile, Spawnable, Container, ContainerTrait, Nameable, NameableTrait, TileFactory.
 * - Concrete Tiles: Banner, Barrel, Beacon, Bed, Bell, BlastFurnace, BrewingStand,
 *   Campfire, Cauldron, Chest, ChiseledBookshelf, Comparator, DaylightSensor,
 *   EnchantTable, EnderChest, FlowerPot, Furnace, GlowingItemFrame, HangingSign,
 *   Hopper, ItemFrame, Jukebox, Lectern, MobHead, MonsterSpawner, NormalFurnace,
 *   Note, PotentSulfur, ShulkerBox, Sign, Smoker, SporeBlossom, StructureBlock.
 *
 * Contract status: The native Rust world does not persist or materialize block
 * entities currently (World::getTileAt() returns null, §4.19). This file provides
 * the complete PM5 API contract, exact method signatures, NBT serialization /
 * deserialization, and runtime state management for plugin compatibility.
 */

namespace pocketmine {
    if (!class_exists(VersionInfo::class, false)) {
        final class VersionInfo
        {
            public const WORLD_DATA_VERSION = 1;
            public const TAG_WORLD_DATA_VERSION = "PMMPDataVersion";
        }
    }
}

namespace pocketmine\timings {
    if (!class_exists(TimingsHandler::class, false)) {
        class TimingsHandler
        {
            public function __construct(string $name, ?TimingsHandler $parent = null)
            {
            }

            public function startTiming(): void
            {
            }

            public function stopTiming(): void
            {
            }
        }
    }

    if (!class_exists(Timings::class, false)) {
        class Timings
        {
            public static function getTileEntityTimings(object $tile): TimingsHandler
            {
                return new TimingsHandler("Tile");
            }
        }
    }
}

namespace pocketmine\inventory {
    if (!interface_exists(InventoryHolder::class, false)) {
        interface InventoryHolder
        {
            public function getInventory(): Inventory;
        }
    }
}

namespace pocketmine\network\mcpe\protocol\types {
    use pocketmine\math\Vector3;

    if (!class_exists(BlockPosition::class, false)) {
        class BlockPosition
        {
            public function __construct(public int $x, public int $y, public int $z)
            {
            }

            public static function fromVector3(Vector3 $pos): self
            {
                return new self($pos->getFloorX(), $pos->getFloorY(), $pos->getFloorZ());
            }
        }
    }

    if (!class_exists(CacheableNbt::class, false)) {
        class CacheableNbt
        {
            public function __construct(private mixed $root)
            {
            }

            public function getRoot(): mixed
            {
                return $this->root;
            }
        }
    }
}

namespace pocketmine\network\mcpe\protocol {
    use pocketmine\network\mcpe\protocol\types\BlockPosition;
    use pocketmine\network\mcpe\protocol\types\CacheableNbt;

    if (!class_exists(BlockActorDataPacket::class, false)) {
        class BlockActorDataPacket
        {
            public static function create(BlockPosition $blockPosition, CacheableNbt $nbt): self
            {
                return new self();
            }
        }
    }

    if (!class_exists(ContainerSetDataPacket::class, false)) {
        class ContainerSetDataPacket
        {
            public const PROPERTY_FURNACE_SMELT_PROGRESS = 0;
            public const PROPERTY_FURNACE_REMAINING_FUEL_TIME = 1;
            public const PROPERTY_FURNACE_MAX_FUEL_TIME = 2;
            public const PROPERTY_BREWING_STAND_BREW_TIME = 0;
            public const PROPERTY_BREWING_STAND_FUEL_AMOUNT = 1;
            public const PROPERTY_BREWING_STAND_FUEL_TOTAL = 2;
        }
    }
}

namespace pocketmine\network\mcpe\convert {
    use pocketmine\item\Item;
    use pocketmine\nbt\tag\CompoundTag;

    if (!class_exists(TypeConverter::class, false)) {
        class TypeConverter
        {
            private static ?self $instance = null;

            public static function getInstance(): self
            {
                return self::$instance ??= new self();
            }

            public function getItemTranslator(): object
            {
                return new class {
                    public function toNetworkNbt(Item $item): CompoundTag
                    {
                        return $item->nbtSerialize();
                    }
                };
            }

            public function getBlockTranslator(): object
            {
                return new class {
                    public function internalIdToNetworkStateData(int $stateId): object
                    {
                        return new class {
                            public function toNbt(): CompoundTag
                            {
                                return CompoundTag::create();
                            }
                        };
                    }
                };
            }
        }
    }
}

namespace pocketmine\data\bedrock\item {
    if (!class_exists(SavedItemStackData::class, false)) {
        final class SavedItemStackData
        {
            public const TAG_COUNT = "Count";
            public const TAG_SLOT = "Slot";
            public const TAG_WAS_PICKED_UP = "WasPickedUp";
            public const TAG_CAN_PLACE_ON = "CanPlaceOn";
            public const TAG_CAN_DESTROY = "CanDestroy";
        }
    }

    if (!class_exists(SavedItemData::class, false)) {
        final class SavedItemData
        {
            public const TAG_NAME = "Name";
            public const TAG_DAMAGE = "Damage";
        }
    }
}

namespace pocketmine\data\bedrock {
    use pocketmine\block\utils\BannerPatternType;
    use pocketmine\block\utils\DyeColor;
    use pocketmine\block\utils\MobHeadType;
    use function array_key_exists;
    use function enum_exists;
    use function spl_object_id;
    use function strtolower;

    if (!trait_exists(IntSaveIdMapTrait::class, false)) {
        trait IntSaveIdMapTrait
        {
            private array $idToEnum = [];
            private array $enumToId = [];

            protected function getRuntimeId(object $enum): int
            {
                return spl_object_id($enum);
            }

            public function register(int $saveId, object $enum): void
            {
                $this->idToEnum[$saveId] = $enum;
                $this->enumToId[$this->getRuntimeId($enum)] = $saveId;
            }

            public function fromId(int $id): ?object
            {
                return $this->idToEnum[$id] ?? null;
            }

            public function toId(object $enum): int
            {
                $runtimeId = $this->getRuntimeId($enum);
                if (!array_key_exists($runtimeId, $this->enumToId)) {
                    throw new \InvalidArgumentException("Object does not have a mapped save ID");
                }
                return $this->enumToId[$runtimeId];
            }
        }
    }

    if (!class_exists(DyeColorIdMap::class, false)) {
        final class DyeColorIdMap
        {
            private static ?self $instance = null;

            use IntSaveIdMapTrait {
                register as registerInt;
            }

            private array $itemIdToEnum = [];
            private array $enumToItemId = [];

            public static function getInstance(): self
            {
                return self::$instance ??= new self();
            }

            private function __construct()
            {
                if (enum_exists(DyeColor::class)) {
                    foreach (DyeColor::cases() as $case) {
                        $colorId = match ($case) {
                            DyeColor::WHITE => 0,
                            DyeColor::ORANGE => 1,
                            DyeColor::MAGENTA => 2,
                            DyeColor::LIGHT_BLUE => 3,
                            DyeColor::YELLOW => 4,
                            DyeColor::LIME => 5,
                            DyeColor::PINK => 6,
                            DyeColor::GRAY => 7,
                            DyeColor::LIGHT_GRAY => 8,
                            DyeColor::CYAN => 9,
                            DyeColor::PURPLE => 10,
                            DyeColor::BLUE => 11,
                            DyeColor::BROWN => 12,
                            DyeColor::GREEN => 13,
                            DyeColor::RED => 14,
                            DyeColor::BLACK => 15,
                        };
                        $dyeItemId = "minecraft:" . strtolower($case->name) . "_dye";
                        $this->register($colorId, $dyeItemId, $case);
                    }
                }
            }

            private function register(int $id, string $itemId, DyeColor $color): void
            {
                $this->registerInt($id, $color);
                $this->itemIdToEnum[$itemId] = $color;
                $this->enumToItemId[spl_object_id($color)] = $itemId;
            }

            public function toInvertedId(DyeColor $color): int
            {
                return ~$this->toId($color) & 0xf;
            }

            public function toItemId(DyeColor $color): string
            {
                return $this->enumToItemId[spl_object_id($color)] ?? "";
            }

            public function fromInvertedId(int $id): ?DyeColor
            {
                return $this->fromId(~$id & 0xf);
            }

            public function fromItemId(string $itemId): ?DyeColor
            {
                return $this->itemIdToEnum[$itemId] ?? null;
            }
        }
    }

    if (!class_exists(BannerPatternTypeIdMap::class, false)) {
        final class BannerPatternTypeIdMap
        {
            private static ?self $instance = null;
            private array $idToEnum = [];
            private array $enumToId = [];

            public static function getInstance(): self
            {
                return self::$instance ??= new self();
            }

            public function __construct()
            {
                if (enum_exists(BannerPatternType::class)) {
                    foreach (BannerPatternType::cases() as $case) {
                        $id = match ($case) {
                            BannerPatternType::BORDER => "bo",
                            BannerPatternType::BRICKS => "bri",
                            BannerPatternType::CIRCLE => "mc",
                            BannerPatternType::CREEPER => "cre",
                            BannerPatternType::CROSS => "cr",
                            BannerPatternType::CURLY_BORDER => "cbo",
                            BannerPatternType::DIAGONAL_LEFT => "lud",
                            BannerPatternType::DIAGONAL_RIGHT => "rd",
                            BannerPatternType::DIAGONAL_UP_LEFT => "ld",
                            BannerPatternType::DIAGONAL_UP_RIGHT => "rud",
                            BannerPatternType::FLOWER => "flo",
                            BannerPatternType::GRADIENT => "gra",
                            BannerPatternType::GRADIENT_UP => "gru",
                            BannerPatternType::HALF_HORIZONTAL => "hh",
                            BannerPatternType::HALF_HORIZONTAL_BOTTOM => "hhb",
                            BannerPatternType::HALF_VERTICAL => "vh",
                            BannerPatternType::HALF_VERTICAL_RIGHT => "vhr",
                            BannerPatternType::MOJANG => "moj",
                            BannerPatternType::RHOMBUS => "mr",
                            BannerPatternType::SKULL => "sku",
                            BannerPatternType::SMALL_STRIPES => "ss",
                            BannerPatternType::SQUARE_BOTTOM_LEFT => "bl",
                            BannerPatternType::SQUARE_BOTTOM_RIGHT => "br",
                            BannerPatternType::SQUARE_TOP_LEFT => "tl",
                            BannerPatternType::SQUARE_TOP_RIGHT => "tr",
                            BannerPatternType::STRAIGHT_CROSS => "sc",
                            BannerPatternType::STRIPE_BOTTOM => "bs",
                            BannerPatternType::STRIPE_CENTER => "cs",
                            BannerPatternType::STRIPE_DOWNLEFT => "dls",
                            BannerPatternType::STRIPE_DOWNRIGHT => "drs",
                            BannerPatternType::STRIPE_LEFT => "ls",
                            BannerPatternType::STRIPE_MIDDLE => "ms",
                            BannerPatternType::STRIPE_RIGHT => "rs",
                            BannerPatternType::STRIPE_TOP => "ts",
                            BannerPatternType::TRIANGLE_BOTTOM => "bt",
                            BannerPatternType::TRIANGLE_TOP => "tt",
                            BannerPatternType::TRIANGLES_BOTTOM => "bts",
                            BannerPatternType::TRIANGLES_TOP => "tts",
                            default => strtolower($case->name)
                        };
                        $this->register($id, $case);
                    }
                }
            }

            public function register(string $stringId, BannerPatternType $type): void
            {
                $this->idToEnum[$stringId] = $type;
                $this->enumToId[spl_object_id($type)] = $stringId;
            }

            public function fromId(string $id): ?BannerPatternType
            {
                return $this->idToEnum[$id] ?? null;
            }

            public function toId(BannerPatternType $type): string
            {
                $k = spl_object_id($type);
                if (!array_key_exists($k, $this->enumToId)) {
                    throw new \InvalidArgumentException("Missing mapping for banner pattern type " . $type->name);
                }
                return $this->enumToId[$k];
            }
        }
    }

    if (!class_exists(MobHeadTypeIdMap::class, false)) {
        final class MobHeadTypeIdMap
        {
            private static ?self $instance = null;
            use IntSaveIdMapTrait;

            public static function getInstance(): self
            {
                return self::$instance ??= new self();
            }

            private function __construct()
            {
                if (enum_exists(MobHeadType::class)) {
                    foreach (MobHeadType::cases() as $case) {
                        $this->register(match ($case) {
                            MobHeadType::SKELETON => 0,
                            MobHeadType::WITHER_SKELETON => 1,
                            MobHeadType::ZOMBIE => 2,
                            MobHeadType::PLAYER => 3,
                            MobHeadType::CREEPER => 4,
                            MobHeadType::DRAGON => 5,
                            MobHeadType::PIGLIN => 6,
                        }, $case);
                    }
                }
            }
        }
    }

    if (!class_exists(PotionTypeIdMap::class, false)) {
        final class PotionTypeIdMap
        {
            private static ?self $instance = null;
            use IntSaveIdMapTrait;

            public static function getInstance(): self
            {
                return self::$instance ??= new self();
            }

            private function __construct()
            {
                if (enum_exists(\pocketmine\item\PotionType::class)) {
                    foreach (\pocketmine\item\PotionType::cases() as $case) {
                        $this->register($case->value ?? spl_object_id($case), $case);
                    }
                }
            }
        }
    }

    if (!class_exists(LegacyEntityIdToStringIdMap::class, false)) {
        final class LegacyEntityIdToStringIdMap
        {
            private static ?self $instance = null;

            public static function getInstance(): self
            {
                return self::$instance ??= new self();
            }

            public function legacyToString(int $id): ?string
            {
                return null;
            }

            public function stringToLegacy(string $id): ?int
            {
                return null;
            }
        }
    }
}

namespace pocketmine\block\tile {

    use pocketmine\block\Air;
    use pocketmine\block\Block;
    use pocketmine\block\Campfire as BlockCampfire;
    use pocketmine\block\CyclingPotentSulfur;
    use pocketmine\block\FillableCauldron;
    use pocketmine\block\Furnace as BlockFurnace;
    use pocketmine\block\inventory\BarrelInventory;
    use pocketmine\block\inventory\BrewingStandInventory;
    use pocketmine\block\inventory\CampfireInventory;
    use pocketmine\block\inventory\ChestInventory;
    use pocketmine\block\inventory\DoubleChestInventory;
    use pocketmine\block\inventory\FurnaceInventory;
    use pocketmine\block\inventory\HopperInventory;
    use pocketmine\block\inventory\ShulkerBoxInventory;
    use pocketmine\block\Note as BlockNote;
    use pocketmine\block\RedstoneComparator;
    use pocketmine\block\RuntimeBlockStateRegistry;
    use pocketmine\block\utils\BannerPatternLayer;
    use pocketmine\block\utils\ChiseledBookshelfSlot;
    use pocketmine\block\utils\DyeColor;
    use pocketmine\block\utils\MobHeadType;
    use pocketmine\block\utils\SignText;
    use pocketmine\color\Color;
    use pocketmine\crafting\BrewingRecipe;
    use pocketmine\crafting\FurnaceRecipe;
    use pocketmine\crafting\FurnaceType;
    use pocketmine\data\bedrock\BannerPatternTypeIdMap;
    use pocketmine\data\bedrock\block\BlockStateDeserializeException;
    use pocketmine\data\bedrock\block\BlockStateNames;
    use pocketmine\data\bedrock\DyeColorIdMap;
    use pocketmine\data\bedrock\item\SavedItemData;
    use pocketmine\data\bedrock\item\SavedItemStackData;
    use pocketmine\data\bedrock\LegacyEntityIdToStringIdMap;
    use pocketmine\data\bedrock\MobHeadTypeIdMap;
    use pocketmine\data\bedrock\PotionTypeIdMap;
    use pocketmine\data\SavedDataLoadingException;
    use pocketmine\event\block\BrewingFuelUseEvent;
    use pocketmine\event\block\BrewItemEvent;
    use pocketmine\event\inventory\FurnaceBurnEvent;
    use pocketmine\event\inventory\FurnaceSmeltEvent;
    use pocketmine\inventory\CallbackInventoryListener;
    use pocketmine\inventory\Inventory;
    use pocketmine\inventory\InventoryHolder;
    use pocketmine\inventory\SimpleInventory;
    use pocketmine\item\Item;
    use pocketmine\item\ItemTypeIds;
    use pocketmine\item\Potion;
    use pocketmine\item\Record;
    use pocketmine\item\SplashPotion;
    use pocketmine\item\VanillaItems;
    use pocketmine\item\WritableBookBase;
    use pocketmine\math\Facing;
    use pocketmine\math\Vector3;
    use pocketmine\nbt\NBT;
    use pocketmine\nbt\NbtException;
    use pocketmine\nbt\tag\ByteTag;
    use pocketmine\nbt\tag\CompoundTag;
    use pocketmine\nbt\tag\FloatTag;
    use pocketmine\nbt\tag\IntTag;
    use pocketmine\nbt\tag\ListTag;
    use pocketmine\nbt\tag\ShortTag;
    use pocketmine\nbt\tag\StringTag;
    use pocketmine\nbt\UnexpectedTagTypeException;
    use pocketmine\network\mcpe\convert\TypeConverter;
    use pocketmine\network\mcpe\protocol\BlockActorDataPacket;
    use pocketmine\network\mcpe\protocol\ContainerSetDataPacket;
    use pocketmine\network\mcpe\protocol\types\BlockPosition;
    use pocketmine\network\mcpe\protocol\types\CacheableNbt;
    use pocketmine\player\Player;
    use pocketmine\timings\Timings;
    use pocketmine\timings\TimingsHandler;
    use pocketmine\utils\AssumptionFailedError;
    use pocketmine\utils\Binary;
    use pocketmine\VersionInfo;
    use pocketmine\world\format\Chunk;
    use pocketmine\world\format\io\GlobalBlockStateHandlers;
    use pocketmine\world\Position;
    use pocketmine\world\sound\PotionFinishBrewingSound;
    use pocketmine\world\sound\RecordStopSound;
    use pocketmine\world\World;
    use function abs;
    use function array_map;
    use function array_pad;
    use function array_slice;
    use function count;
    use function explode;
    use function floor;
    use function get_class;
    use function implode;
    use function in_array;
    use function is_a;
    use function max;
    use function mb_scrub;
    use function method_exists;
    use function reset;
    use function rtrim;
    use function sprintf;

    /**
     * Interface for tiles holding an inventory.
     */
    interface Container extends InventoryHolder
    {
        public const TAG_ITEMS = "Items";
        public const TAG_LOCK = "Lock";

        public function getRealInventory(): Inventory;

        /**
         * Returns whether this container can be opened by an item with the given custom name.
         */
        public function canOpenWith(string $key): bool;
    }

    /**
     * Interface for tiles with customizable names.
     */
    interface Nameable
    {
        public const TAG_CUSTOM_NAME = "CustomName";

        public function getDefaultName(): string;

        public function getName(): string;

        public function setName(string $str): void;

        public function hasName(): bool;
    }

    /**
     * This trait implements most methods in the {@link Container} interface. It should only be used by Tiles.
     */
    trait ContainerTrait
    {
        private ?string $lock = null;

        abstract public function getRealInventory(): Inventory;

        protected function loadItems(CompoundTag $tag): void
        {
            $inventoryTag = ($raw = $tag->getTag(Container::TAG_ITEMS)) instanceof ListTag ? $raw : null;
            if ($inventoryTag !== null) {
                $inventory = $this->getRealInventory();
                $listeners = method_exists($inventory, 'getListeners') ? $inventory->getListeners()->toArray() : [];
                if (method_exists($inventory, 'getListeners')) {
                    $inventory->getListeners()->remove(...$listeners);
                }

                $newContents = [];
                $errorLogContext = "Container (" . $this->getPosition() . ")";
                foreach ($inventoryTag as $itemNBT) {
                    $slotId = $itemNBT->getByte(SavedItemStackData::TAG_SLOT, 0);
                    $newContents[$slotId] = Item::safeNbtDeserialize($itemNBT, "$errorLogContext slot $slotId");
                }
                if (method_exists($inventory, 'setContents')) {
                    $inventory->setContents($newContents);
                } else {
                    if (method_exists($inventory, 'clearAll')) {
                        $inventory->clearAll();
                    }
                    foreach ($newContents as $slotId => $item) {
                        $inventory->setItem($slotId, $item);
                    }
                }

                if (method_exists($inventory, 'getListeners')) {
                    $inventory->getListeners()->add(...$listeners);
                }
            }

            if (($lockTag = $tag->getTag(Container::TAG_LOCK)) instanceof StringTag) {
                $this->lock = $lockTag->getValue();
            }
        }

        protected function saveItems(CompoundTag $tag): void
        {
            $items = [];
            foreach ($this->getRealInventory()->getContents() as $slot => $item) {
                if ($item !== null && (method_exists($item, 'isNull') ? !$item->isNull() : true)) {
                    $items[] = $item->nbtSerialize($slot);
                }
            }

            $tag->setTag(Container::TAG_ITEMS, new ListTag($items, NBT::TAG_Compound));

            if ($this->lock !== null) {
                $tag->setString(Container::TAG_LOCK, $this->lock);
            }
        }

        /**
         * @see Container::canOpenWith()
         */
        public function canOpenWith(string $key): bool
        {
            return $this->lock === null || $this->lock === $key;
        }

        /**
         * @see Position::asPosition()
         */
        abstract protected function getPosition(): Position;

        /**
         * @see Tile::onBlockDestroyedHook()
         */
        protected function onBlockDestroyedHook(): void
        {
            $inv = $this->getRealInventory();
            $pos = $this->getPosition();

            $world = $pos->getWorld();
            $dropPos = $pos->add(0.5, 0.5, 0.5);
            foreach ($inv->getContents() as $k => $item) {
                $world->dropItem($dropPos, $item);
            }
            $inv->clearAll();
        }
    }

    /**
     * This trait implements most methods in the {@link Nameable} interface. It should only be used by Tiles.
     */
    trait NameableTrait
    {
        private ?string $customName = null;

        abstract public function getDefaultName(): string;

        public function getName(): string
        {
            return $this->customName ?? $this->getDefaultName();
        }

        public function setName(string $name): void
        {
            if ($name === "") {
                $this->customName = null;
            } else {
                $this->customName = $name;
            }
        }

        public function hasName(): bool
        {
            return $this->customName !== null;
        }

        public function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            if ($this->customName !== null) {
                $nbt->setString(Nameable::TAG_CUSTOM_NAME, $this->customName);
            }
        }

        protected function loadName(CompoundTag $tag): void
        {
            if (($customNameTag = $tag->getTag(Nameable::TAG_CUSTOM_NAME)) instanceof StringTag) {
                $this->customName = $customNameTag->getValue();
            }
        }

        protected function saveName(CompoundTag $tag): void
        {
            if ($this->customName !== null) {
                $tag->setString(Nameable::TAG_CUSTOM_NAME, $this->customName);
            }
        }

        /**
         * @see Tile::copyDataFromItem()
         */
        public function copyDataFromItem(Item $item): void
        {
            parent::copyDataFromItem($item);
            if ($item->hasCustomName()) {
                $this->setName($item->getCustomName());
            }
        }
    }

    /**
     * Base class for all block entities.
     */
    abstract class Tile
    {
        public const TAG_ID = "id";
        public const TAG_X = "x";
        public const TAG_Y = "y";
        public const TAG_Z = "z";

        protected Position $position;
        public bool $closed = false;
        protected ?TimingsHandler $timings = null;

        public function __construct(World $world, Vector3 $pos)
        {
            if (method_exists(Position::class, 'fromObject')) {
                $this->position = Position::fromObject($pos, $world);
            } else {
                $worldName = '';
                try {
                    if (method_exists($world, 'getFolderName')) {
                        $worldName = $world->getFolderName();
                    } elseif (method_exists($world, 'getName')) {
                        $worldName = $world->getName();
                    }
                } catch (\Throwable) {
                    $worldName = '';
                }
                $this->position = new Position($pos->x, $pos->y, $pos->z, $worldName);
            }
            if (class_exists(Timings::class)) {
                $this->timings = Timings::getTileEntityTimings($this);
            }
        }

        /**
         * @internal
         * Reads additional data from the CompoundTag on tile creation.
         */
        abstract public function readSaveData(CompoundTag $nbt): void;

        /**
         * Writes additional save data to a CompoundTag, not including generic things like ID and coordinates.
         */
        abstract protected function writeSaveData(CompoundTag $nbt): void;

        public function saveNBT(): CompoundTag
        {
            $nbt = CompoundTag::create()
                ->setString(self::TAG_ID, TileFactory::getInstance()->getSaveId(get_class($this)))
                ->setInt(self::TAG_X, $this->position->getFloorX())
                ->setInt(self::TAG_Y, $this->position->getFloorY())
                ->setInt(self::TAG_Z, $this->position->getFloorZ())
                ->setLong(VersionInfo::TAG_WORLD_DATA_VERSION, VersionInfo::WORLD_DATA_VERSION);
            $this->writeSaveData($nbt);

            return $nbt;
        }

        public function getCleanedNBT(): ?CompoundTag
        {
            $this->writeSaveData($tag = new CompoundTag());
            return $tag->getCount() > 0 ? $tag : null;
        }

        /**
         * @internal
         */
        public function copyDataFromItem(Item $item): void
        {
            if (($blockNbt = $item->getCustomBlockData()) !== null) {
                $this->readSaveData($blockNbt);
            }
        }

        public function getBlock(): Block
        {
            $world = $this->position->getWorld();
            if ($world !== null && method_exists($world, 'getBlock')) {
                return $world->getBlock($this->position);
            }
            return \pocketmine\block\VanillaBlocks::AIR();
        }

        public function getPosition(): Position
        {
            return $this->position;
        }

        public function isClosed(): bool
        {
            return $this->closed;
        }

        public function __destruct()
        {
            $this->close();
        }

        /**
         * Called when the tile's block is destroyed.
         */
        final public function onBlockDestroyed(): void
        {
            $this->onBlockDestroyedHook();
            $this->close();
        }

        /**
         * Override this method to do actions you need to do when this tile is destroyed due to block being broken.
         */
        protected function onBlockDestroyedHook(): void
        {
        }

        public static function colorFromARGB(int $argb): Color
        {
            if (method_exists(Color::class, 'fromARGB')) {
                return Color::fromARGB($argb);
            }
            return new Color(($argb >> 16) & 0xff, ($argb >> 8) & 0xff, $argb & 0xff, ($argb >> 24) & 0xff);
        }

        public static function colorToARGB(Color $color): int
        {
            if (method_exists($color, 'toARGB')) {
                return $color->toARGB();
            }
            return (($color->getA() & 0xff) << 24) | (($color->getR() & 0xff) << 16) | (($color->getG() & 0xff) << 8) | ($color->getB() & 0xff);
        }

        public function close(): void
        {
            if (!$this->closed) {
                $this->closed = true;

                $world = method_exists($this->position, 'isValid') && !$this->position->isValid() ? null : $this->position->getWorld();
                if ($world !== null && method_exists($world, 'removeTile')) {
                    $world->removeTile($this);
                }
            }
        }
    }

    /**
     * Base class for tiles that spawn packet data to clients.
     */
    abstract class Spawnable extends Tile
    {
        private ?CacheableNbt $spawnCompoundCache = null;

        /**
         * @deprecated
         */
        public function isDirty(): bool
        {
            return $this->spawnCompoundCache === null;
        }

        /**
         * @deprecated
         */
        public function setDirty(bool $dirty = true): void
        {
            $this->clearSpawnCompoundCache();
        }

        public function clearSpawnCompoundCache(): void
        {
            $this->spawnCompoundCache = null;
        }

        /**
         * @return array<string, IntTag|StringTag|ByteTag>
         */
        public function getRenderUpdateBugWorkaroundStateProperties(Block $block): array
        {
            return [];
        }

        /**
         * Returns encoded NBT used to spawn this tile to clients.
         */
        final public function getSerializedSpawnCompound(): CacheableNbt
        {
            if ($this->spawnCompoundCache === null) {
                $this->spawnCompoundCache = new CacheableNbt($this->getSpawnCompound());
            }

            return $this->spawnCompoundCache;
        }

        final public function getSpawnCompound(): CompoundTag
        {
            $nbt = CompoundTag::create()
                ->setString(self::TAG_ID, TileFactory::getInstance()->getSaveId(get_class($this)))
                ->setInt(self::TAG_X, $this->position->getFloorX())
                ->setInt(self::TAG_Y, $this->position->getFloorY())
                ->setInt(self::TAG_Z, $this->position->getFloorZ());
            $this->addAdditionalSpawnData($nbt);
            return $nbt;
        }

        /**
         * An extension to getSpawnCompound() for further modifying the generic tile NBT.
         */
        abstract protected function addAdditionalSpawnData(CompoundTag $nbt): void;
    }

    /**
     * @deprecated
     * @see \pocketmine\block\BaseBanner
     */
    class Banner extends Spawnable
    {
        public const TAG_BASE = "Base";
        public const TAG_PATTERNS = "Patterns";
        public const TAG_PATTERN_COLOR = "Color";
        public const TAG_PATTERN_NAME = "Pattern";
        public const TAG_TYPE = "Type";

        public const TYPE_NORMAL = 0;
        public const TYPE_OMINOUS = 1;

        private DyeColor $baseColor = DyeColor::BLACK;
        private array $patterns = [];
        private int $type = self::TYPE_NORMAL;

        public function readSaveData(CompoundTag $nbt): void
        {
            $colorIdMap = DyeColorIdMap::getInstance();
            if (
                ($baseColorTag = $nbt->getTag(self::TAG_BASE)) instanceof IntTag &&
                ($baseColor = $colorIdMap->fromInvertedId($baseColorTag->getValue())) !== null
            ) {
                $this->baseColor = $baseColor;
            } else {
                $this->baseColor = DyeColor::BLACK;
            }

            $patternTypeIdMap = BannerPatternTypeIdMap::getInstance();
            $patterns = ($rawPatterns = $nbt->getTag(self::TAG_PATTERNS)) instanceof ListTag ? $rawPatterns : null;
            if ($patterns !== null) {
                foreach ($patterns as $pattern) {
                    $patternColor = $colorIdMap->fromInvertedId($pattern->getInt(self::TAG_PATTERN_COLOR, 0)) ?? DyeColor::BLACK;
                    $patternType = $patternTypeIdMap->fromId($pattern->getString(self::TAG_PATTERN_NAME, ""));
                    if ($patternType === null) {
                        continue;
                    }
                    $this->patterns[] = new BannerPatternLayer($patternType, $patternColor);
                }
            }

            $this->type = $nbt->getInt(self::TAG_TYPE, self::TYPE_NORMAL);
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $colorIdMap = DyeColorIdMap::getInstance();
            $patternIdMap = BannerPatternTypeIdMap::getInstance();
            $nbt->setInt(self::TAG_BASE, $colorIdMap->toInvertedId($this->baseColor));
            $patterns = new ListTag();
            foreach ($this->patterns as $pattern) {
                $patterns->push(CompoundTag::create()
                    ->setString(self::TAG_PATTERN_NAME, $patternIdMap->toId($pattern->getType()))
                    ->setInt(self::TAG_PATTERN_COLOR, $colorIdMap->toInvertedId($pattern->getColor()))
                );
            }
            $nbt->setTag(self::TAG_PATTERNS, $patterns);
            $nbt->setInt(self::TAG_TYPE, $this->type);
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            $colorIdMap = DyeColorIdMap::getInstance();
            $patternIdMap = BannerPatternTypeIdMap::getInstance();
            $nbt->setInt(self::TAG_BASE, $colorIdMap->toInvertedId($this->baseColor));
            $patterns = new ListTag();
            foreach ($this->patterns as $pattern) {
                $patterns->push(CompoundTag::create()
                    ->setString(self::TAG_PATTERN_NAME, $patternIdMap->toId($pattern->getType()))
                    ->setInt(self::TAG_PATTERN_COLOR, $colorIdMap->toInvertedId($pattern->getColor()))
                );
            }
            $nbt->setTag(self::TAG_PATTERNS, $patterns);
            $nbt->setInt(self::TAG_TYPE, $this->type);
        }

        public function getBaseColor(): DyeColor
        {
            return $this->baseColor;
        }

        public function setBaseColor(DyeColor $color): void
        {
            $this->baseColor = $color;
        }

        /**
         * @return BannerPatternLayer[]
         */
        public function getPatterns(): array
        {
            return $this->patterns;
        }

        /**
         * @param BannerPatternLayer[] $patterns
         */
        public function setPatterns(array $patterns): void
        {
            $this->patterns = $patterns;
        }

        public function getType(): int
        {
            return $this->type;
        }

        public function setType(int $type): void
        {
            $this->type = $type;
        }

        public function getDefaultName(): string
        {
            return "Banner";
        }
    }

    class Barrel extends Spawnable implements Container, Nameable
    {
        use NameableTrait;
        use ContainerTrait;

        protected BarrelInventory $inventory;

        public function __construct(World $world, Vector3 $pos)
        {
            parent::__construct($world, $pos);
            $this->inventory = new BarrelInventory($this->position);
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            $this->loadName($nbt);
            $this->loadItems($nbt);
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $this->saveName($nbt);
            $this->saveItems($nbt);
        }

        public function close(): void
        {
            if (!$this->closed) {
                if (method_exists($this->inventory, 'removeAllViewers')) {
                    $this->inventory->removeAllViewers();
                }
                parent::close();
            }
        }

        public function getInventory(): BarrelInventory
        {
            return $this->inventory;
        }

        public function getRealInventory(): BarrelInventory
        {
            return $this->inventory;
        }

        public function getDefaultName(): string
        {
            return "Barrel";
        }
    }

    final class Beacon extends Spawnable
    {
        private const TAG_PRIMARY = "primary";
        private const TAG_SECONDARY = "secondary";

        private int $primaryEffect = 0;
        private int $secondaryEffect = 0;

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            $nbt->setInt(self::TAG_PRIMARY, $this->primaryEffect);
            $nbt->setInt(self::TAG_SECONDARY, $this->secondaryEffect);
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            $this->primaryEffect = $nbt->getInt(self::TAG_PRIMARY, 0);
            $this->secondaryEffect = $nbt->getInt(self::TAG_SECONDARY, 0);
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $nbt->setInt(self::TAG_PRIMARY, $this->primaryEffect);
            $nbt->setInt(self::TAG_SECONDARY, $this->secondaryEffect);
        }

        public function getPrimaryEffect(): int
        {
            return $this->primaryEffect;
        }

        public function setPrimaryEffect(int $primaryEffect): void
        {
            $this->primaryEffect = $primaryEffect;
        }

        public function getSecondaryEffect(): int
        {
            return $this->secondaryEffect;
        }

        public function setSecondaryEffect(int $secondaryEffect): void
        {
            $this->secondaryEffect = $secondaryEffect;
        }
    }

    class Bed extends Spawnable
    {
        public const TAG_COLOR = "color";

        private DyeColor $color = DyeColor::RED;

        public function getColor(): DyeColor
        {
            return $this->color;
        }

        public function setColor(DyeColor $color): void
        {
            $this->color = $color;
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            if (
                ($colorTag = $nbt->getTag(self::TAG_COLOR)) instanceof ByteTag &&
                ($color = DyeColorIdMap::getInstance()->fromId($colorTag->getValue())) !== null
            ) {
                $this->color = $color;
            } else {
                $this->color = DyeColor::RED;
            }
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $nbt->setByte(self::TAG_COLOR, DyeColorIdMap::getInstance()->toId($this->color));
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            $nbt->setByte(self::TAG_COLOR, DyeColorIdMap::getInstance()->toId($this->color));
        }
    }

    final class Bell extends Spawnable
    {
        public const TAG_DIRECTION = "Direction";
        public const TAG_RINGING = "Ringing";
        public const TAG_TICKS = "Ticks";

        private bool $ringing = false;
        private int $facing = Facing::NORTH;
        private int $ticks = 0;

        public function isRinging(): bool
        {
            return $this->ringing;
        }

        public function setRinging(bool $ringing): void
        {
            $this->ringing = $ringing;
        }

        public function getFacing(): int
        {
            return $this->facing;
        }

        public function setFacing(int $facing): void
        {
            $this->facing = $facing;
        }

        public function getTicks(): int
        {
            return $this->ticks;
        }

        public function setTicks(int $ticks): void
        {
            $this->ticks = $ticks;
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            $nbt->setByte(self::TAG_RINGING, $this->ringing ? 1 : 0);
            $nbt->setInt(self::TAG_DIRECTION, $this->facing);
            $nbt->setInt(self::TAG_TICKS, $this->ticks);
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            $this->ringing = $nbt->getByte(self::TAG_RINGING, 0) !== 0;
            $this->facing = $nbt->getInt(self::TAG_DIRECTION, Facing::NORTH);
            $this->ticks = $nbt->getInt(self::TAG_TICKS, 0);
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $nbt->setByte(self::TAG_RINGING, $this->ringing ? 1 : 0);
            $nbt->setInt(self::TAG_DIRECTION, $this->facing);
            $nbt->setInt(self::TAG_TICKS, $this->ticks);
        }

        public function createFakeUpdatePacket(int $bellHitFace): BlockActorDataPacket
        {
            $nbt = $this->getSpawnCompound();
            $nbt->setByte(self::TAG_RINGING, 1);
            $nbt->setInt(self::TAG_DIRECTION, match ($bellHitFace) {
                Facing::SOUTH => 0,
                Facing::WEST => 1,
                Facing::NORTH => 2,
                Facing::EAST => 3,
                default => throw new AssumptionFailedError("Unreachable")
            });
            $nbt->setInt(self::TAG_TICKS, 0);
            return BlockActorDataPacket::create(BlockPosition::fromVector3($this->position), new CacheableNbt($nbt));
        }
    }

    abstract class Furnace extends Spawnable implements Container, Nameable
    {
        use NameableTrait;
        use ContainerTrait;

        public const TAG_BURN_TIME = "BurnTime";
        public const TAG_COOK_TIME = "CookTime";
        public const TAG_MAX_TIME = "MaxTime";

        protected FurnaceInventory $inventory;
        private int $remainingFuelTime = 0;
        private int $cookTime = 0;
        private int $maxFuelTime = 0;

        public function __construct(World $world, Vector3 $pos)
        {
            parent::__construct($world, $pos);
            $furnaceType = $this->getFurnaceType();
            try {
                $this->inventory = new FurnaceInventory($this->position, $furnaceType);
            } catch (\TypeError|\ArgumentCountError) {
                try {
                    $this->inventory = new FurnaceInventory($this->position, new FurnaceType());
                } catch (\Throwable) {
                    $ref = new \ReflectionClass(FurnaceInventory::class);
                    $this->inventory = $ref->newInstanceWithoutConstructor();
                    if ($ref->hasProperty('holder')) {
                        $prop = $ref->getProperty('holder');
                        $prop->setAccessible(true);
                        $prop->setValue($this->inventory, $this->position);
                    }
                }
            }
            if (method_exists($this->inventory, 'getListeners')) {
                $this->inventory->getListeners()->add(CallbackInventoryListener::onAnyChange(
                    static function (Inventory $unused) use ($world, $pos): void {
                        $world->scheduleDelayedBlockUpdate($pos, 1);
                    })
                );
            }
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            $this->remainingFuelTime = max(0, $nbt->getShort(self::TAG_BURN_TIME, $this->remainingFuelTime));

            $this->cookTime = $nbt->getShort(self::TAG_COOK_TIME, $this->cookTime);
            if ($this->remainingFuelTime === 0) {
                $this->cookTime = 0;
            }

            $this->maxFuelTime = $nbt->getShort(self::TAG_MAX_TIME, $this->maxFuelTime);
            if ($this->maxFuelTime === 0) {
                $this->maxFuelTime = $this->remainingFuelTime;
            }

            $this->loadName($nbt);
            $this->loadItems($nbt);

            if ($this->remainingFuelTime > 0) {
                $this->position->getWorld()->scheduleDelayedBlockUpdate($this->position, 1);
            }
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $nbt->setShort(self::TAG_BURN_TIME, $this->remainingFuelTime);
            $nbt->setShort(self::TAG_COOK_TIME, $this->cookTime);
            $nbt->setShort(self::TAG_MAX_TIME, $this->maxFuelTime);
            $this->saveName($nbt);
            $this->saveItems($nbt);
        }

        public function getDefaultName(): string
        {
            return "Furnace";
        }

        public function close(): void
        {
            if (!$this->closed) {
                if (method_exists($this->inventory, 'removeAllViewers')) {
                    $this->inventory->removeAllViewers();
                }
                parent::close();
            }
        }

        public function getInventory(): FurnaceInventory
        {
            return $this->inventory;
        }

        public function getRealInventory(): FurnaceInventory
        {
            return $this->getInventory();
        }

        protected function checkFuel(Item $fuel): void
        {
            $ev = new FurnaceBurnEvent($this, $fuel, $fuel->getFuelTime());
            $ev->call();
            if ($ev->isCancelled()) {
                return;
            }

            $this->maxFuelTime = $this->remainingFuelTime = $ev->getBurnTime();
            $this->onStartSmelting();

            if ($this->remainingFuelTime > 0 && $ev->isBurning()) {
                $this->inventory->setFuel($fuel->getFuelResidue());
            }
        }

        protected function onStartSmelting(): void
        {
            $block = $this->getBlock();
            if ($block instanceof BlockFurnace && !$block->isLit()) {
                $block->setLit(true);
                $this->position->getWorld()->setBlock($block->getPosition(), $block);
            }
        }

        protected function onStopSmelting(): void
        {
            $block = $this->getBlock();
            if ($block instanceof BlockFurnace && $block->isLit()) {
                $block->setLit(false);
                $this->position->getWorld()->setBlock($block->getPosition(), $block);
            }
        }

        abstract public function getFurnaceType(): mixed;

        public function onUpdate(): bool
        {
            if ($this->closed) {
                return false;
            }

            $this->timings?->startTiming();

            $prevCookTime = $this->cookTime;
            $prevRemainingFuelTime = $this->remainingFuelTime;
            $prevMaxFuelTime = $this->maxFuelTime;

            $ret = false;

            $fuel = $this->inventory->getFuel();
            $raw = $this->inventory->getSmelting();
            $product = $this->inventory->getResult();

            $furnaceType = $this->getFurnaceType();
            $furnaceTypeKey = is_string($furnaceType) ? $furnaceType : (is_object($furnaceType) && property_exists($furnaceType, 'name') ? strtolower((string)$furnaceType->name) : 'furnace');
            $smelt = $this->position->getWorld()->getServer()->getCraftingManager()->getFurnaceRecipeManager($furnaceTypeKey)->match($raw);
            $canSmelt = ($smelt instanceof FurnaceRecipe && $raw->getCount() > 0 && (($smelt->getResult()->canStackWith($product) && $product->getCount() < $product->getMaxStackSize()) || $product->isNull()));

            if ($this->remainingFuelTime <= 0 && $canSmelt && $fuel->getFuelTime() > 0 && $fuel->getCount() > 0) {
                $this->checkFuel($fuel);
            }

            if ($this->remainingFuelTime > 0) {
                --$this->remainingFuelTime;

                if ($smelt instanceof FurnaceRecipe && $canSmelt) {
                    ++$this->cookTime;

                    $cookDuration = 200;
                    if (is_object($furnaceType) && method_exists($furnaceType, 'getCookDurationTicks')) {
                        $cookDuration = $furnaceType->getCookDurationTicks();
                    } elseif (is_string($furnaceType) && method_exists(FurnaceType::class, 'cookTicks')) {
                        $cookDuration = FurnaceType::cookTicks($furnaceType);
                    } elseif ($furnaceType === FurnaceType::BLAST_FURNACE || $furnaceType === FurnaceType::SMOKER) {
                        $cookDuration = 100;
                    }

                    if ($this->cookTime >= $cookDuration) {
                        $product = $smelt->getResult()->setCount($product->getCount() + 1);

                        $ev = new FurnaceSmeltEvent($this, $raw, $product);
                        $ev->call();

                        if (!$ev->isCancelled()) {
                            $this->inventory->setResult($ev->getResult());
                            $raw->pop();
                            $this->inventory->setSmelting($raw);
                        }

                        $this->cookTime -= $cookDuration;
                    }
                } elseif ($this->remainingFuelTime <= 0) {
                    $this->remainingFuelTime = $this->cookTime = $this->maxFuelTime = 0;
                } else {
                    $this->cookTime = 0;
                }
                $ret = true;
            } else {
                $this->onStopSmelting();
                $this->remainingFuelTime = $this->cookTime = $this->maxFuelTime = 0;
            }

            $viewers = array_map(fn (Player $p) => $p->getNetworkSession()->getInvManager(), $this->inventory->getViewers());
            foreach ($viewers as $v) {
                if ($v === null) {
                    continue;
                }
                if ($prevCookTime !== $this->cookTime) {
                    $v->syncData($this->inventory, ContainerSetDataPacket::PROPERTY_FURNACE_SMELT_PROGRESS, $this->cookTime);
                }
                if ($prevRemainingFuelTime !== $this->remainingFuelTime) {
                    $v->syncData($this->inventory, ContainerSetDataPacket::PROPERTY_FURNACE_REMAINING_FUEL_TIME, $this->remainingFuelTime);
                }
                if ($prevMaxFuelTime !== $this->maxFuelTime) {
                    $v->syncData($this->inventory, ContainerSetDataPacket::PROPERTY_FURNACE_MAX_FUEL_TIME, $this->maxFuelTime);
                }
            }

            $this->timings?->stopTiming();

            return $ret;
        }
    }

    class BlastFurnace extends Furnace
    {
        public function getFurnaceType(): mixed
        {
            return FurnaceType::BLAST_FURNACE;
        }
    }

    class NormalFurnace extends Furnace
    {
        public function getFurnaceType(): mixed
        {
            return FurnaceType::FURNACE;
        }
    }

    class Smoker extends Furnace
    {
        public function getFurnaceType(): mixed
        {
            return FurnaceType::SMOKER;
        }
    }

    class BrewingStand extends Spawnable implements Container, Nameable
    {
        use NameableTrait {
            addAdditionalSpawnData as addNameSpawnData;
        }
        use ContainerTrait;

        public const BREW_TIME_TICKS = 400;

        private const TAG_BREW_TIME = "BrewTime";
        private const TAG_BREW_TIME_PE = "CookTime";
        private const TAG_MAX_FUEL_TIME = "FuelTotal";
        private const TAG_REMAINING_FUEL_TIME = "Fuel";
        private const TAG_REMAINING_FUEL_TIME_PE = "FuelAmount";

        private BrewingStandInventory $inventory;
        private int $brewTime = 0;
        private int $maxFuelTime = 0;
        private int $remainingFuelTime = 0;

        public function __construct(World $world, Vector3 $pos)
        {
            parent::__construct($world, $pos);
            $this->inventory = new BrewingStandInventory($this->position);
            if (method_exists($this->inventory, 'getListeners')) {
                $this->inventory->getListeners()->add(CallbackInventoryListener::onAnyChange(static function (Inventory $unused) use ($world, $pos): void {
                    $world->scheduleDelayedBlockUpdate($pos, 1);
                }));
            }
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            $this->loadName($nbt);
            $this->loadItems($nbt);

            $brewTimeTag = $nbt->getTag(self::TAG_BREW_TIME) ?? $nbt->getTag(self::TAG_BREW_TIME_PE);
            $this->brewTime = $brewTimeTag instanceof ShortTag ? $brewTimeTag->getValue() : 0;

            $maxFuelTag = $nbt->getTag(self::TAG_MAX_FUEL_TIME);
            $this->maxFuelTime = $maxFuelTag instanceof ShortTag ? $maxFuelTag->getValue() : 0;

            $fuelTag = $nbt->getTag(self::TAG_REMAINING_FUEL_TIME) ?? $nbt->getTag(self::TAG_REMAINING_FUEL_TIME_PE);
            $this->remainingFuelTime = $fuelTag instanceof ByteTag || $fuelTag instanceof ShortTag ? $fuelTag->getValue() : 0;
            if ($this->maxFuelTime === 0) {
                $this->maxFuelTime = $this->remainingFuelTime;
            }
            if ($this->remainingFuelTime === 0) {
                $this->maxFuelTime = $this->remainingFuelTime = $this->brewTime = 0;
            }
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $this->saveName($nbt);
            $this->saveItems($nbt);

            $nbt->setShort(self::TAG_BREW_TIME_PE, $this->brewTime);
            $nbt->setShort(self::TAG_MAX_FUEL_TIME, $this->maxFuelTime);
            $nbt->setShort(self::TAG_REMAINING_FUEL_TIME_PE, $this->remainingFuelTime);
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            $this->addNameSpawnData($nbt);

            $nbt->setShort(self::TAG_BREW_TIME_PE, $this->brewTime);
            $nbt->setShort(self::TAG_MAX_FUEL_TIME, $this->maxFuelTime);
            $nbt->setShort(self::TAG_REMAINING_FUEL_TIME_PE, $this->remainingFuelTime);
        }

        public function getDefaultName(): string
        {
            return "Brewing Stand";
        }

        public function close(): void
        {
            if (!$this->closed) {
                if (method_exists($this->inventory, 'removeAllViewers')) {
                    $this->inventory->removeAllViewers();
                }
                parent::close();
            }
        }

        public function getInventory(): BrewingStandInventory
        {
            return $this->inventory;
        }

        public function getRealInventory(): BrewingStandInventory
        {
            return $this->inventory;
        }

        private function checkFuel(Item $item): void
        {
            $ev = new BrewingFuelUseEvent($this);
            if (!$item->equals(VanillaItems::BLAZE_POWDER(), true, false)) {
                $ev->cancel();
            }

            $ev->call();
            if ($ev->isCancelled()) {
                return;
            }

            $item->pop();
            $this->inventory->setItem(BrewingStandInventory::SLOT_FUEL, $item);

            $this->maxFuelTime = $this->remainingFuelTime = $ev->getFuelTime();
        }

        /**
         * @return array<int, BrewingRecipe>
         */
        private function getBrewableRecipes(): array
        {
            $ingredient = $this->inventory->getItem(BrewingStandInventory::SLOT_INGREDIENT);
            if ($ingredient->isNull()) {
                return [];
            }

            $recipes = [];
            $craftingManager = $this->position->getWorld()->getServer()->getCraftingManager();
            foreach ([BrewingStandInventory::SLOT_BOTTLE_LEFT, BrewingStandInventory::SLOT_BOTTLE_MIDDLE, BrewingStandInventory::SLOT_BOTTLE_RIGHT] as $slot) {
                $input = $this->inventory->getItem($slot);
                if ($input->isNull()) {
                    continue;
                }

                if (($recipe = $craftingManager->matchBrewingRecipe($input, $ingredient)) !== null) {
                    $recipes[$slot] = $recipe;
                }
            }

            return $recipes;
        }

        public function onUpdate(): bool
        {
            if ($this->closed) {
                return false;
            }

            $this->timings?->startTiming();

            $prevBrewTime = $this->brewTime;
            $prevRemainingFuelTime = $this->remainingFuelTime;
            $prevMaxFuelTime = $this->maxFuelTime;

            $ret = false;

            $fuel = $this->inventory->getItem(BrewingStandInventory::SLOT_FUEL);
            $ingredient = $this->inventory->getItem(BrewingStandInventory::SLOT_INGREDIENT);

            $recipes = $this->getBrewableRecipes();
            $canBrew = count($recipes) !== 0;

            if ($this->remainingFuelTime <= 0 && $canBrew) {
                $this->checkFuel($fuel);
            }

            if ($this->remainingFuelTime > 0) {
                if ($canBrew) {
                    if ($this->brewTime === 0) {
                        $this->brewTime = self::BREW_TIME_TICKS;
                        --$this->remainingFuelTime;
                    }

                    --$this->brewTime;

                    if ($this->brewTime <= 0) {
                        $anythingBrewed = false;
                        foreach ($recipes as $slot => $recipe) {
                            $input = $this->inventory->getItem($slot);
                            $output = $recipe->getResultFor($input);
                            if ($output === null) {
                                continue;
                            }

                            $ev = new BrewItemEvent($this, $slot, $input, $output, $recipe);
                            $ev->call();
                            if ($ev->isCancelled()) {
                                continue;
                            }

                            $this->inventory->setItem($slot, $ev->getResult());
                            $anythingBrewed = true;
                        }

                        if ($anythingBrewed) {
                            $this->position->getWorld()->addSound($this->position->add(0.5, 0.5, 0.5), new PotionFinishBrewingSound());
                        }

                        $ingredient->pop();
                        $this->inventory->setItem(BrewingStandInventory::SLOT_INGREDIENT, $ingredient);

                        $this->brewTime = 0;
                    } else {
                        $ret = true;
                    }
                } else {
                    $this->brewTime = 0;
                }
            } else {
                $this->brewTime = $this->remainingFuelTime = $this->maxFuelTime = 0;
            }

            $viewers = array_map(fn (Player $p) => $p->getNetworkSession()->getInvManager(), $this->inventory->getViewers());
            foreach ($viewers as $v) {
                if ($v === null) {
                    continue;
                }
                if ($prevBrewTime !== $this->brewTime) {
                    $v->syncData($this->inventory, ContainerSetDataPacket::PROPERTY_BREWING_STAND_BREW_TIME, $this->brewTime);
                }
                if ($prevRemainingFuelTime !== $this->remainingFuelTime) {
                    $v->syncData($this->inventory, ContainerSetDataPacket::PROPERTY_BREWING_STAND_FUEL_AMOUNT, $this->remainingFuelTime);
                }
                if ($prevMaxFuelTime !== $this->maxFuelTime) {
                    $v->syncData($this->inventory, ContainerSetDataPacket::PROPERTY_BREWING_STAND_FUEL_TOTAL, $this->maxFuelTime);
                }
            }

            $this->timings?->stopTiming();

            return $ret;
        }
    }

    class Campfire extends Spawnable implements Container
    {
        use ContainerTrait;

        private const TAG_FIRST_INPUT_ITEM = "Item1";
        private const TAG_SECOND_INPUT_ITEM = "Item2";
        private const TAG_THIRD_INPUT_ITEM = "Item3";
        private const TAG_FOURTH_INPUT_ITEM = "Item4";

        private const TAG_FIRST_COOKING_TIME = "ItemTime1";
        private const TAG_SECOND_COOKING_TIME = "ItemTime2";
        private const TAG_THIRD_COOKING_TIME = "ItemTime3";
        private const TAG_FOURTH_COOKING_TIME = "ItemTime4";

        protected CampfireInventory $inventory;
        private array $cookingTimes = [];

        public function __construct(World $world, Vector3 $pos)
        {
            parent::__construct($world, $pos);
            $this->inventory = new CampfireInventory($this->position);
            if (method_exists($this->inventory, 'getListeners')) {
                $this->inventory->getListeners()->add(CallbackInventoryListener::onAnyChange(
                    static function (Inventory $unused) use ($world, $pos): void {
                        $block = $world->getBlock($pos);
                        if ($block instanceof BlockCampfire) {
                            $world->setBlock($pos, $block);
                        }
                    })
                );
            }
        }

        public function getInventory(): CampfireInventory
        {
            return $this->inventory;
        }

        public function getRealInventory(): CampfireInventory
        {
            return $this->inventory;
        }

        /**
         * @return array<int, int>
         */
        public function getCookingTimes(): array
        {
            return $this->cookingTimes;
        }

        /**
         * @param array<int, int> $cookingTimes
         */
        public function setCookingTimes(array $cookingTimes): void
        {
            $this->cookingTimes = $cookingTimes;
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            $items = [];
            $listeners = method_exists($this->inventory, 'getListeners') ? $this->inventory->getListeners()->toArray() : [];
            if (method_exists($this->inventory, 'getListeners')) {
                $this->inventory->getListeners()->remove(...$listeners);
            }

            $baseErrorContext = "Campfire ($this->position)";
            foreach ([
                [0, self::TAG_FIRST_INPUT_ITEM, self::TAG_FIRST_COOKING_TIME],
                [1, self::TAG_SECOND_INPUT_ITEM, self::TAG_SECOND_COOKING_TIME],
                [2, self::TAG_THIRD_INPUT_ITEM, self::TAG_THIRD_COOKING_TIME],
                [3, self::TAG_FOURTH_INPUT_ITEM, self::TAG_FOURTH_COOKING_TIME],
            ] as [$slot, $itemTag, $cookingTimeTag]) {
                if (($tag = $nbt->getTag($itemTag)) instanceof CompoundTag) {
                    $items[$slot] = Item::safeNbtDeserialize($tag, "$baseErrorContext slot $slot");
                }
                if (($tag = $nbt->getTag($cookingTimeTag)) instanceof IntTag) {
                    $this->cookingTimes[$slot] = $tag->getValue();
                }
            }
            if (method_exists($this->inventory, 'setContents')) {
                $this->inventory->setContents($items);
            } else {
                if (method_exists($this->inventory, 'clearAll')) {
                    $this->inventory->clearAll();
                }
                foreach ($items as $slot => $item) {
                    $this->inventory->setItem($slot, $item);
                }
            }
            if (method_exists($this->inventory, 'getListeners')) {
                $this->inventory->getListeners()->add(...$listeners);
            }
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            foreach ([
                [0, self::TAG_FIRST_INPUT_ITEM, self::TAG_FIRST_COOKING_TIME],
                [1, self::TAG_SECOND_INPUT_ITEM, self::TAG_SECOND_COOKING_TIME],
                [2, self::TAG_THIRD_INPUT_ITEM, self::TAG_THIRD_COOKING_TIME],
                [3, self::TAG_FOURTH_INPUT_ITEM, self::TAG_FOURTH_COOKING_TIME],
            ] as [$slot, $itemTag, $cookingTimeTag]) {
                $item = $this->inventory->getItem($slot);
                if ($item !== null && !$item->isNull()) {
                    $nbt->setTag($itemTag, $item->nbtSerialize());
                    if (isset($this->cookingTimes[$slot])) {
                        $nbt->setInt($cookingTimeTag, $this->cookingTimes[$slot]);
                    }
                }
            }
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            foreach ([
                0 => self::TAG_FIRST_INPUT_ITEM,
                1 => self::TAG_SECOND_INPUT_ITEM,
                2 => self::TAG_THIRD_INPUT_ITEM,
                3 => self::TAG_FOURTH_INPUT_ITEM,
            ] as $slot => $tag) {
                $item = $this->inventory->getItem($slot);
                if ($item !== null && !$item->isNull()) {
                    $nbt->setTag($tag, TypeConverter::getInstance()->getItemTranslator()->toNetworkNbt($item));
                }
            }
        }
    }

    final class Cauldron extends Spawnable
    {
        private const POTION_CONTAINER_TYPE_NONE = -1;
        private const POTION_CONTAINER_TYPE_NORMAL = 0;
        private const POTION_CONTAINER_TYPE_SPLASH = 1;
        private const POTION_CONTAINER_TYPE_LINGERING = 2;

        private const POTION_ID_NONE = -1;

        private const TAG_POTION_ID = "PotionId";
        private const TAG_POTION_CONTAINER_TYPE = "PotionType";
        private const TAG_CUSTOM_COLOR = "CustomColor";

        private ?Item $potionItem = null;
        private ?Color $customWaterColor = null;

        public function getPotionItem(): ?Item
        {
            return $this->potionItem;
        }

        public function setPotionItem(?Item $potionItem): void
        {
            $this->potionItem = $potionItem;
        }

        public function getCustomWaterColor(): ?Color
        {
            return $this->customWaterColor;
        }

        public function setCustomWaterColor(?Color $customWaterColor): void
        {
            $this->customWaterColor = $customWaterColor;
        }

        private function getPotionContainerType(): int
        {
            if ($this->potionItem === null) {
                return self::POTION_CONTAINER_TYPE_NONE;
            }
            if ($this->potionItem instanceof SplashPotion) {
                return self::POTION_CONTAINER_TYPE_SPLASH;
            }
            if ($this->potionItem instanceof LingeringPotion) {
                return self::POTION_CONTAINER_TYPE_LINGERING;
            }
            if ($this->potionItem instanceof Potion) {
                return self::POTION_CONTAINER_TYPE_NORMAL;
            }
            $cname = get_class($this->potionItem);
            if (str_contains($cname, 'SplashPotion')) {
                return self::POTION_CONTAINER_TYPE_SPLASH;
            }
            if (str_contains($cname, 'LingeringPotion')) {
                return self::POTION_CONTAINER_TYPE_LINGERING;
            }
            return self::POTION_CONTAINER_TYPE_NORMAL;
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            $nbt->setShort(self::TAG_POTION_CONTAINER_TYPE, $this->getPotionContainerType());

            $type = $this->potionItem instanceof Potion || $this->potionItem instanceof SplashPotion ? $this->potionItem->getType() : null;
            $nbt->setShort(self::TAG_POTION_ID, $type === null ? self::POTION_ID_NONE : PotionTypeIdMap::getInstance()->toId($type));

            if ($this->customWaterColor !== null) {
                $nbt->setInt(self::TAG_CUSTOM_COLOR, Binary::signInt(self::colorToARGB($this->customWaterColor)));
            }
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            $containerType = $nbt->getShort(self::TAG_POTION_CONTAINER_TYPE, self::POTION_CONTAINER_TYPE_NONE);
            $potionId = $nbt->getShort(self::TAG_POTION_ID, self::POTION_ID_NONE);
            if ($containerType !== self::POTION_CONTAINER_TYPE_NONE && $potionId !== self::POTION_ID_NONE) {
                $potionType = PotionTypeIdMap::getInstance()->fromId($potionId);
                if ($potionType === null) {
                    throw new SavedDataLoadingException("Unknown potion type ID $potionId");
                }
                $this->potionItem = match ($containerType) {
                    self::POTION_CONTAINER_TYPE_NORMAL => VanillaItems::POTION()->setType($potionType),
                    self::POTION_CONTAINER_TYPE_SPLASH => VanillaItems::SPLASH_POTION()->setType($potionType),
                    self::POTION_CONTAINER_TYPE_LINGERING => VanillaItems::LINGERING_POTION()->setType($potionType),
                    default => throw new SavedDataLoadingException("Invalid potion container type ID $containerType")
                };
            } else {
                $this->potionItem = null;
            }

            $this->customWaterColor = ($customColorTag = $nbt->getTag(self::TAG_CUSTOM_COLOR)) instanceof IntTag ? self::colorFromARGB(Binary::unsignInt($customColorTag->getValue())) : null;
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $nbt->setShort(self::TAG_POTION_CONTAINER_TYPE, $this->getPotionContainerType());

            $type = $this->potionItem instanceof Potion || $this->potionItem instanceof SplashPotion ? $this->potionItem->getType() : null;
            $nbt->setShort(self::TAG_POTION_ID, $type === null ? self::POTION_ID_NONE : PotionTypeIdMap::getInstance()->toId($type));

            if ($this->customWaterColor !== null) {
                $nbt->setInt(self::TAG_CUSTOM_COLOR, Binary::signInt(self::colorToARGB($this->customWaterColor)));
            }
        }

        public function getRenderUpdateBugWorkaroundStateProperties(Block $block): array
        {
            if ($block instanceof FillableCauldron) {
                $realFillLevel = $block->getFillLevel();
                return [BlockStateNames::FILL_LEVEL => new IntTag($realFillLevel === FillableCauldron::MAX_FILL_LEVEL ? FillableCauldron::MIN_FILL_LEVEL : $realFillLevel + 1)];
            }

            return [];
        }
    }

    class Chest extends Spawnable implements Container, Nameable
    {
        use NameableTrait {
            addAdditionalSpawnData as addNameSpawnData;
        }
        use ContainerTrait {
            onBlockDestroyedHook as containerTraitBlockDestroyedHook;
        }

        public const TAG_PAIRX = "pairx";
        public const TAG_PAIRZ = "pairz";
        public const TAG_PAIR_LEAD = "pairlead";

        protected ChestInventory $inventory;
        protected ?DoubleChestInventory $doubleInventory = null;

        private ?int $pairX = null;
        private ?int $pairZ = null;

        public function __construct(World $world, Vector3 $pos)
        {
            parent::__construct($world, $pos);
            $this->inventory = new ChestInventory($this->position);
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            if (($pairXTag = $nbt->getTag(self::TAG_PAIRX)) instanceof IntTag && ($pairZTag = $nbt->getTag(self::TAG_PAIRZ)) instanceof IntTag) {
                $pairX = $pairXTag->getValue();
                $pairZ = $pairZTag->getValue();
                if (
                    ($this->position->x === $pairX && abs($this->position->z - $pairZ) === 1) ||
                    ($this->position->z === $pairZ && abs($this->position->x - $pairX) === 1)
                ) {
                    $this->pairX = $pairX;
                    $this->pairZ = $pairZ;
                } else {
                    $this->pairX = $this->pairZ = null;
                }
            }
            $this->loadName($nbt);
            $this->loadItems($nbt);
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            if ($this->isPaired()) {
                $nbt->setInt(self::TAG_PAIRX, $this->pairX);
                $nbt->setInt(self::TAG_PAIRZ, $this->pairZ);
            }
            $this->saveName($nbt);
            $this->saveItems($nbt);
        }

        public function getCleanedNBT(): ?CompoundTag
        {
            $tag = parent::getCleanedNBT();
            if ($tag !== null) {
                $tag->removeTag(self::TAG_PAIRX, self::TAG_PAIRZ);
            }
            return $tag;
        }

        public function close(): void
        {
            if (!$this->closed) {
                if (method_exists($this->inventory, 'removeAllViewers')) {
                    $this->inventory->removeAllViewers();
                }

                if ($this->doubleInventory !== null) {
                    if ($this->isPaired() && $this->position->getWorld()->isChunkLoaded($this->pairX >> Chunk::COORD_BIT_SIZE, $this->pairZ >> Chunk::COORD_BIT_SIZE)) {
                        if (method_exists($this->doubleInventory, 'removeAllViewers')) {
                            $this->doubleInventory->removeAllViewers();
                        }
                        if (($pair = $this->getPair()) !== null) {
                            $pair->doubleInventory = null;
                        }
                    }
                    $this->doubleInventory = null;
                }

                parent::close();
            }
        }

        protected function onBlockDestroyedHook(): void
        {
            $this->unpair();
            $this->containerTraitBlockDestroyedHook();
        }

        public function getInventory(): ChestInventory|DoubleChestInventory
        {
            if ($this->isPaired() && $this->doubleInventory === null) {
                $this->checkPairing();
            }
            return $this->doubleInventory instanceof DoubleChestInventory ? $this->doubleInventory : $this->inventory;
        }

        public function getRealInventory(): ChestInventory
        {
            return $this->inventory;
        }

        protected function checkPairing(): void
        {
            if ($this->isPaired() && !$this->position->getWorld()->isInLoadedTerrain(new Vector3($this->pairX, $this->position->y, $this->pairZ))) {
                $this->doubleInventory = null;
            } elseif (($pair = $this->getPair()) instanceof Chest) {
                if (!$pair->isPaired()) {
                    $pair->createPair($this);
                    $pair->checkPairing();
                }
                if ($this->doubleInventory === null) {
                    if ($pair->doubleInventory !== null) {
                        $this->doubleInventory = $pair->doubleInventory;
                    } else {
                        if (($pair->position->x + ($pair->position->z << 15)) > ($this->position->x + ($this->position->z << 15))) {
                            $this->doubleInventory = $pair->doubleInventory = new DoubleChestInventory($pair->inventory, $this->inventory);
                        } else {
                            $this->doubleInventory = $pair->doubleInventory = new DoubleChestInventory($this->inventory, $pair->inventory);
                        }
                    }
                }
            } else {
                $this->doubleInventory = null;
                $this->pairX = $this->pairZ = null;
            }
        }

        public function getDefaultName(): string
        {
            return "Chest";
        }

        public function isPaired(): bool
        {
            return $this->pairX !== null && $this->pairZ !== null;
        }

        public function getPair(): ?Chest
        {
            if ($this->isPaired()) {
                $tile = $this->position->getWorld()->getTileAt($this->pairX, $this->position->y, $this->pairZ);
                if ($tile instanceof Chest) {
                    return $tile;
                }
            }

            return null;
        }

        public function pairWith(Chest $tile): bool
        {
            if ($this->isPaired() || $tile->isPaired()) {
                return false;
            }

            $this->createPair($tile);

            $this->clearSpawnCompoundCache();
            $tile->clearSpawnCompoundCache();
            $this->checkPairing();

            return true;
        }

        private function createPair(Chest $tile): void
        {
            $this->pairX = $tile->getPosition()->x;
            $this->pairZ = $tile->getPosition()->z;

            $tile->pairX = $this->getPosition()->x;
            $tile->pairZ = $this->getPosition()->z;
        }

        public function unpair(): bool
        {
            if (!$this->isPaired()) {
                return false;
            }

            $tile = $this->getPair();
            $this->pairX = $this->pairZ = null;

            $this->clearSpawnCompoundCache();

            if ($tile instanceof Chest) {
                $tile->pairX = $tile->pairZ = null;
                $tile->checkPairing();
                $tile->clearSpawnCompoundCache();
            }
            $this->checkPairing();

            return true;
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            if ($this->isPaired()) {
                $nbt->setInt(self::TAG_PAIRX, $this->pairX);
                $nbt->setInt(self::TAG_PAIRZ, $this->pairZ);
            }

            $this->addNameSpawnData($nbt);
        }
    }

    class ChiseledBookshelf extends Tile implements Container
    {
        use ContainerTrait;

        private const TAG_LAST_INTERACTED_SLOT = "LastInteractedSlot";

        private SimpleInventory $inventory;
        private ?ChiseledBookshelfSlot $lastInteractedSlot = null;

        public function __construct(World $world, Vector3 $pos)
        {
            parent::__construct($world, $pos);
            $this->inventory = new SimpleInventory(count(ChiseledBookshelfSlot::cases()));
        }

        public function getInventory(): SimpleInventory
        {
            return $this->inventory;
        }

        public function getRealInventory(): SimpleInventory
        {
            return $this->inventory;
        }

        public function getLastInteractedSlot(): ?ChiseledBookshelfSlot
        {
            return $this->lastInteractedSlot;
        }

        public function setLastInteractedSlot(?ChiseledBookshelfSlot $lastInteractedSlot): void
        {
            $this->lastInteractedSlot = $lastInteractedSlot;
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            $this->loadItems($nbt);

            $lastInteractedSlot = $nbt->getInt(self::TAG_LAST_INTERACTED_SLOT, 0);
            if ($lastInteractedSlot !== 0) {
                $this->lastInteractedSlot = ChiseledBookshelfSlot::tryFrom($lastInteractedSlot - 1);
            }
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $this->saveItems($nbt);

            $nbt->setInt(
                self::TAG_LAST_INTERACTED_SLOT,
                $this->lastInteractedSlot !== null ? $this->lastInteractedSlot->value + 1 : 0
            );
        }

        protected function loadItems(CompoundTag $tag): void
        {
            $inventoryTag = ($raw = $tag->getTag(Container::TAG_ITEMS)) instanceof ListTag ? $raw : null;
            if ($inventoryTag !== null) {
                $inventory = $this->getRealInventory();
                $listeners = method_exists($inventory, 'getListeners') ? $inventory->getListeners()->toArray() : [];
                if (method_exists($inventory, 'getListeners')) {
                    $inventory->getListeners()->remove(...$listeners);
                }

                $newContents = [];
                $errorLogContext = "ChiseledBookshelf ($this->position)";
                foreach ($inventoryTag as $slot => $itemNBT) {
                    $count = $itemNBT->getByte(SavedItemStackData::TAG_COUNT, 0);
                    if ($count === 0) {
                        continue;
                    }
                    $newContents[$slot] = Item::safeNbtDeserialize($itemNBT, "$errorLogContext slot $slot");
                }
                if (method_exists($inventory, 'setContents')) {
                    $inventory->setContents($newContents);
                } else {
                    if (method_exists($inventory, 'clearAll')) {
                        $inventory->clearAll();
                    }
                    foreach ($newContents as $slot => $item) {
                        $inventory->setItem($slot, $item);
                    }
                }

                if (method_exists($inventory, 'getListeners')) {
                    $inventory->getListeners()->add(...$listeners);
                }
            }

            if (($lockTag = $tag->getTag(Container::TAG_LOCK)) instanceof StringTag) {
                $this->lock = $lockTag->getValue();
            }
        }

        protected function saveItems(CompoundTag $tag): void
        {
            $items = [];
            foreach ($this->getRealInventory()->getContents(true) as $slot => $item) {
                if ($item === null || (method_exists($item, 'isNull') && $item->isNull())) {
                    $items[$slot] = CompoundTag::create()
                        ->setByte(SavedItemStackData::TAG_COUNT, 0)
                        ->setShort(SavedItemData::TAG_DAMAGE, 0)
                        ->setString(SavedItemData::TAG_NAME, "")
                        ->setByte(SavedItemStackData::TAG_WAS_PICKED_UP, 0);
                } else {
                    $items[$slot] = $item->nbtSerialize();
                }
            }

            $tag->setTag(Container::TAG_ITEMS, new ListTag($items, NBT::TAG_Compound));

            if ($this->lock !== null) {
                $tag->setString(Container::TAG_LOCK, $this->lock);
            }
        }
    }

    /**
     * @deprecated
     * @see RedstoneComparator
     */
    class Comparator extends Tile
    {
        private const TAG_OUTPUT_SIGNAL = "OutputSignal";

        protected int $signalStrength = 0;

        public function getSignalStrength(): int
        {
            return $this->signalStrength;
        }

        public function setSignalStrength(int $signalStrength): void
        {
            $this->signalStrength = $signalStrength;
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            $this->signalStrength = $nbt->getInt(self::TAG_OUTPUT_SIGNAL, 0);
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $nbt->setInt(self::TAG_OUTPUT_SIGNAL, $this->signalStrength);
        }
    }

    /**
     * @deprecated
     */
    class DaylightSensor extends Tile
    {
        public function readSaveData(CompoundTag $nbt): void
        {
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
        }
    }

    class EnchantTable extends Spawnable implements Nameable
    {
        use NameableTrait {
            loadName as public readSaveData;
            saveName as writeSaveData;
        }

        public function getDefaultName(): string
        {
            return "Enchanting Table";
        }
    }

    class EnderChest extends Spawnable
    {
        protected int $viewerCount = 0;

        public function getViewerCount(): int
        {
            return $this->viewerCount;
        }

        public function setViewerCount(int $viewerCount): void
        {
            if ($viewerCount < 0) {
                throw new \InvalidArgumentException('Viewer count cannot be negative');
            }
            $this->viewerCount = $viewerCount;
        }

        public function readSaveData(CompoundTag $nbt): void
        {
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
        }
    }

    /**
     * @deprecated
     * @see \pocketmine\block\FlowerPot
     */
    class FlowerPot extends Spawnable
    {
        private const TAG_ITEM = "item";
        private const TAG_ITEM_DATA = "mData";
        private const TAG_PLANT_BLOCK = "PlantBlock";

        private ?Block $plant = null;

        public function readSaveData(CompoundTag $nbt): void
        {
            $blockStateData = null;

            if (method_exists(GlobalBlockStateHandlers::class, 'getUpgrader')) {
                $blockDataUpgrader = GlobalBlockStateHandlers::getUpgrader();
                if (($itemIdTag = $nbt->getTag(self::TAG_ITEM)) instanceof ShortTag && ($itemMetaTag = $nbt->getTag(self::TAG_ITEM_DATA)) instanceof IntTag) {
                    try {
                        $blockStateData = $blockDataUpgrader->upgradeIntIdMeta($itemIdTag->getValue(), $itemMetaTag->getValue());
                    } catch (BlockStateDeserializeException $e) {
                        throw new SavedDataLoadingException("Error loading legacy flower pot item data: " . $e->getMessage(), 0, $e);
                    }
                } elseif (($plantBlockTag = $nbt->getTag(self::TAG_PLANT_BLOCK)) instanceof CompoundTag) {
                    try {
                        $blockStateData = $blockDataUpgrader->upgradeBlockStateNbt($plantBlockTag);
                    } catch (BlockStateDeserializeException $e) {
                        throw new SavedDataLoadingException("Error loading " . self::TAG_PLANT_BLOCK . " tag for flower pot: " . $e->getMessage(), 0, $e);
                    }
                }
            }

            if ($blockStateData !== null) {
                try {
                    $blockStateId = GlobalBlockStateHandlers::getDeserializer()->deserialize($blockStateData);
                } catch (BlockStateDeserializeException $e) {
                    throw new SavedDataLoadingException("Error deserializing plant for flower pot: " . $e->getMessage(), 0, $e);
                }
                $this->setPlant(RuntimeBlockStateRegistry::getInstance()->fromStateId($blockStateId));
            }
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            if ($this->plant !== null) {
                $nbt->setTag(self::TAG_PLANT_BLOCK, GlobalBlockStateHandlers::getSerializer()->serialize($this->plant->getStateId())->toNbt());
            }
        }

        public function getPlant(): ?Block
        {
            return $this->plant !== null ? clone $this->plant : null;
        }

        public function setPlant(?Block $plant): void
        {
            if ($plant === null || $plant instanceof Air) {
                $this->plant = null;
            } else {
                $this->plant = clone $plant;
            }
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            if ($this->plant !== null) {
                $nbt->setTag(self::TAG_PLANT_BLOCK, TypeConverter::getInstance()->getBlockTranslator()->internalIdToNetworkStateData($this->plant->getStateId())->toNbt());
            }
        }

        public function getRenderUpdateBugWorkaroundStateProperties(Block $block): array
        {
            return [BlockStateNames::UPDATE_BIT => new ByteTag(1)];
        }
    }

    /**
     * @deprecated
     * @see \pocketmine\block\ItemFrame
     */
    class ItemFrame extends Spawnable
    {
        public const TAG_ITEM_ROTATION = "ItemRotation";
        public const TAG_ITEM_DROP_CHANCE = "ItemDropChance";
        public const TAG_ITEM = "Item";

        private Item $item;
        private int $itemRotation = 0;
        private float $itemDropChance = 1.0;

        public function __construct(World $world, Vector3 $pos)
        {
            $this->item = VanillaItems::AIR();
            parent::__construct($world, $pos);
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            if (($itemTag = $nbt->getTag(self::TAG_ITEM)) instanceof CompoundTag) {
                $this->item = Item::safeNbtDeserialize($itemTag, "ItemFrame ($this->position) framed item");
            }
            if ($nbt->getTag(self::TAG_ITEM_ROTATION) instanceof FloatTag) {
                $this->itemRotation = (int) ($nbt->getFloat(self::TAG_ITEM_ROTATION, $this->itemRotation * 45) / 45);
            } else {
                $this->itemRotation = $nbt->getByte(self::TAG_ITEM_ROTATION, $this->itemRotation);
            }
            $this->itemDropChance = $nbt->getFloat(self::TAG_ITEM_DROP_CHANCE, $this->itemDropChance);
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $nbt->setFloat(self::TAG_ITEM_DROP_CHANCE, $this->itemDropChance);
            $nbt->setFloat(self::TAG_ITEM_ROTATION, $this->itemRotation * 45);
            if (!$this->item->isNull()) {
                $nbt->setTag(self::TAG_ITEM, $this->item->nbtSerialize());
            }
        }

        public function hasItem(): bool
        {
            return !$this->item->isNull();
        }

        public function getItem(): Item
        {
            return clone $this->item;
        }

        public function setItem(?Item $item): void
        {
            if ($item !== null && !$item->isNull()) {
                $this->item = clone $item;
            } else {
                $this->item = VanillaItems::AIR();
            }
        }

        public function getItemRotation(): int
        {
            return $this->itemRotation;
        }

        public function setItemRotation(int $rotation): void
        {
            $this->itemRotation = $rotation;
        }

        public function getItemDropChance(): float
        {
            return $this->itemDropChance;
        }

        public function setItemDropChance(float $chance): void
        {
            $this->itemDropChance = $chance;
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            $nbt->setFloat(self::TAG_ITEM_DROP_CHANCE, $this->itemDropChance);
            $nbt->setFloat(self::TAG_ITEM_ROTATION, $this->itemRotation * 45);
            if (!$this->item->isNull()) {
                $nbt->setTag(self::TAG_ITEM, TypeConverter::getInstance()->getItemTranslator()->toNetworkNbt($this->item));
            }
        }
    }

    /**
     * @deprecated
     * @see \pocketmine\block\ItemFrame
     */
    final class GlowingItemFrame extends ItemFrame
    {
    }

    /**
     * @deprecated
     * @see \pocketmine\block\BaseSign
     */
    class Sign extends Spawnable
    {
        public const TAG_TEXT_BLOB = "Text";
        public const TAG_TEXT_LINE = "Text%d";
        public const TAG_TEXT_COLOR = "SignTextColor";
        public const TAG_GLOWING_TEXT = "IgnoreLighting";
        public const TAG_PERSIST_FORMATTING = "PersistFormatting";
        public const TAG_LEGACY_BUG_RESOLVE = "TextIgnoreLegacyBugResolved";

        public const TAG_FRONT_TEXT = "FrontText";
        public const TAG_BACK_TEXT = "BackText";
        public const TAG_WAXED = "IsWaxed";
        public const TAG_LOCKED_FOR_EDITING_BY = "LockedForEditingBy";

        /**
         * @return string[]
         * @deprecated
         */
        public static function fixTextBlob(string $blob): array
        {
            return array_slice(array_pad(explode("\n", $blob, limit: 5), 4, ""), 0, 4);
        }

        protected SignText $text;
        protected SignText $backText;
        private bool $waxed = false;

        protected ?int $editorEntityRuntimeId = null;

        public function __construct(World $world, Vector3 $pos)
        {
            $this->text = new SignText();
            $this->backText = new SignText();
            parent::__construct($world, $pos);
        }

        private function createSignText(string $blob, Color $baseColor, bool $glowingText): SignText
        {
            if (method_exists(Utils::class, 'checkUTF8')) {
                try {
                    return SignText::fromBlob(mb_scrub($blob, 'UTF-8'), $baseColor, $glowingText);
                } catch (\Throwable) {}
            }
            $text = new SignText(null, $baseColor, $glowingText);
            $lines = array_slice(array_pad(explode("\n", mb_scrub($blob, 'UTF-8'), limit: SignText::LINE_COUNT + 1), SignText::LINE_COUNT, ""), 0, SignText::LINE_COUNT);
            $ref = new \ReflectionClass(SignText::class);
            if ($ref->hasProperty('lines')) {
                $prop = $ref->getProperty('lines');
                $prop->setAccessible(true);
                $prop->setValue($text, $lines);
            }
            return $text;
        }

        private function readTextTag(CompoundTag $nbt, bool $lightingBugResolved): SignText
        {
            $baseColor = new Color(0, 0, 0);
            $glowingText = false;
            if (($baseColorTag = $nbt->getTag(self::TAG_TEXT_COLOR)) instanceof IntTag) {
                $baseColor = self::colorFromARGB(Binary::unsignInt($baseColorTag->getValue()));
            }
            if ($lightingBugResolved && ($glowingTextTag = $nbt->getTag(self::TAG_GLOWING_TEXT)) instanceof ByteTag) {
                $glowingText = $glowingTextTag->getValue() !== 0;
            }
            $textBlob = ($textBlobTag = $nbt->getTag(self::TAG_TEXT_BLOB)) instanceof StringTag ? $textBlobTag->getValue() : "";
            return $this->createSignText($textBlob, $baseColor, $glowingText);
        }

        private function writeTextTag(SignText $text): CompoundTag
        {
            return CompoundTag::create()
                ->setString(self::TAG_TEXT_BLOB, rtrim(implode("\n", $text->getLines()), "\n"))
                ->setInt(self::TAG_TEXT_COLOR, Binary::signInt(self::colorToARGB($text->getBaseColor())))
                ->setByte(self::TAG_GLOWING_TEXT, $text->isGlowing() ? 1 : 0)
                ->setByte(self::TAG_PERSIST_FORMATTING, 1);
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            $frontTextTag = $nbt->getTag(self::TAG_FRONT_TEXT);
            if ($frontTextTag instanceof CompoundTag) {
                $this->text = $this->readTextTag($frontTextTag, true);
            } elseif ($nbt->getTag(self::TAG_TEXT_BLOB) instanceof StringTag) {
                $lightingBugResolved = false;
                if (($lightingBugResolvedTag = $nbt->getTag(self::TAG_LEGACY_BUG_RESOLVE)) instanceof ByteTag) {
                    $lightingBugResolved = $lightingBugResolvedTag->getValue() !== 0;
                }
                $this->text = $this->readTextTag($nbt, $lightingBugResolved);
            } else {
                $text = [];
                for ($i = 0; $i < SignText::LINE_COUNT; ++$i) {
                    $textKey = sprintf(self::TAG_TEXT_LINE, $i + 1);
                    if (($lineTag = $nbt->getTag($textKey)) instanceof StringTag) {
                        $text[$i] = mb_scrub($lineTag->getValue(), 'UTF-8');
                    }
                }
                $this->text = new SignText($text);
            }
            $backTextTag = $nbt->getTag(self::TAG_BACK_TEXT);
            $this->backText = $backTextTag instanceof CompoundTag ? $this->readTextTag($backTextTag, true) : new SignText();
            $this->waxed = $nbt->getByte(self::TAG_WAXED, 0) !== 0;
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $nbt->setTag(self::TAG_FRONT_TEXT, $this->writeTextTag($this->text));
            $nbt->setTag(self::TAG_BACK_TEXT, $this->writeTextTag($this->backText));
            $nbt->setByte(self::TAG_WAXED, $this->waxed ? 1 : 0);
        }

        public function getText(): SignText
        {
            return $this->text;
        }

        public function setText(SignText $text): void
        {
            $this->text = $text;
        }

        public function getBackText(): SignText
        {
            return $this->backText;
        }

        public function setBackText(SignText $backText): void
        {
            $this->backText = $backText;
        }

        public function isWaxed(): bool
        {
            return $this->waxed;
        }

        public function setWaxed(bool $waxed): void
        {
            $this->waxed = $waxed;
        }

        public function getEditorEntityRuntimeId(): ?int
        {
            return $this->editorEntityRuntimeId;
        }

        public function setEditorEntityRuntimeId(?int $editorEntityRuntimeId): void
        {
            $this->editorEntityRuntimeId = $editorEntityRuntimeId;
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            $nbt->setTag(self::TAG_FRONT_TEXT, $this->writeTextTag($this->text));
            $nbt->setTag(self::TAG_BACK_TEXT, $this->writeTextTag($this->backText));
            $nbt->setByte(self::TAG_WAXED, $this->waxed ? 1 : 0);
            $nbt->setLong(self::TAG_LOCKED_FOR_EDITING_BY, $this->editorEntityRuntimeId ?? -1);
        }
    }

    /**
     * @deprecated
     */
    final class HangingSign extends Sign
    {
    }

    class Hopper extends Spawnable implements Container, Nameable
    {
        use ContainerTrait;
        use NameableTrait;

        private const TAG_TRANSFER_COOLDOWN = "TransferCooldown";

        private HopperInventory $inventory;
        private int $transferCooldown = 0;

        public function __construct(World $world, Vector3 $pos)
        {
            parent::__construct($world, $pos);
            $this->inventory = new HopperInventory($this->position);
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            $this->loadItems($nbt);
            $this->loadName($nbt);

            $this->transferCooldown = $nbt->getInt(self::TAG_TRANSFER_COOLDOWN, 0);
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $this->saveItems($nbt);
            $this->saveName($nbt);

            $nbt->setInt(self::TAG_TRANSFER_COOLDOWN, $this->transferCooldown);
        }

        public function close(): void
        {
            if (!$this->closed) {
                if (method_exists($this->inventory, 'removeAllViewers')) {
                    $this->inventory->removeAllViewers();
                }
                parent::close();
            }
        }

        public function getDefaultName(): string
        {
            return "Hopper";
        }

        public function getInventory(): HopperInventory
        {
            return $this->inventory;
        }

        public function getRealInventory(): HopperInventory
        {
            return $this->inventory;
        }
    }

    class Jukebox extends Spawnable
    {
        private const TAG_RECORD = "RecordItem";

        private ?Record $record = null;

        public function getRecord(): ?Record
        {
            return $this->record;
        }

        public function setRecord(?Record $record): void
        {
            $this->record = $record;
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            if (($tag = $nbt->getTag(self::TAG_RECORD)) instanceof CompoundTag) {
                $record = Item::safeNbtDeserialize($tag, "Jukebox ($this->position) record");
                if ($record instanceof Record) {
                    $this->record = $record;
                }
            }
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            if ($this->record !== null) {
                $nbt->setTag(self::TAG_RECORD, $this->record->nbtSerialize());
            }
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            if ($this->record !== null) {
                $nbt->setTag(self::TAG_RECORD, TypeConverter::getInstance()->getItemTranslator()->toNetworkNbt($this->record));
            }
        }

        protected function onBlockDestroyedHook(): void
        {
            $this->position->getWorld()->addSound($this->position, new RecordStopSound());
        }
    }

    /**
     * @deprecated
     * @see \pocketmine\block\Lectern
     */
    class Lectern extends Spawnable
    {
        public const TAG_HAS_BOOK = "hasBook";
        public const TAG_PAGE = "page";
        public const TAG_TOTAL_PAGES = "totalPages";
        public const TAG_BOOK = "book";

        private int $viewedPage = 0;
        private ?WritableBookBase $book = null;

        public function readSaveData(CompoundTag $nbt): void
        {
            $this->viewedPage = $nbt->getInt(self::TAG_PAGE, 0);
            if (($itemTag = $nbt->getTag(self::TAG_BOOK)) instanceof CompoundTag) {
                $book = Item::safeNbtDeserialize($itemTag, "Lectern ($this->position) book");
                if ($book instanceof WritableBookBase && !$book->isNull()) {
                    $this->book = $book;
                }
            }
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $nbt->setByte(self::TAG_HAS_BOOK, $this->book !== null ? 1 : 0);
            $nbt->setInt(self::TAG_PAGE, $this->viewedPage);
            if ($this->book !== null) {
                $nbt->setTag(self::TAG_BOOK, $this->book->nbtSerialize());
                $nbt->setInt(self::TAG_TOTAL_PAGES, count($this->book->getPages()));
            }
        }

        public function getViewedPage(): int
        {
            return $this->viewedPage;
        }

        public function setViewedPage(int $viewedPage): void
        {
            $this->viewedPage = $viewedPage;
        }

        public function getBook(): ?WritableBookBase
        {
            return $this->book !== null ? clone $this->book : null;
        }

        public function setBook(?WritableBookBase $book): void
        {
            $this->book = $book !== null && !$book->isNull() ? clone $book : null;
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            $nbt->setByte(self::TAG_HAS_BOOK, $this->book !== null ? 1 : 0);
            $nbt->setInt(self::TAG_PAGE, $this->viewedPage);
            if ($this->book !== null) {
                $nbt->setTag(self::TAG_BOOK, TypeConverter::getInstance()->getItemTranslator()->toNetworkNbt($this->book));
                $nbt->setInt(self::TAG_TOTAL_PAGES, count($this->book->getPages()));
            }
        }
    }

    /**
     * @deprecated
     * @see \pocketmine\block\MobHead
     */
    class MobHead extends Spawnable
    {
        private const TAG_SKULL_TYPE = "SkullType";
        private const TAG_ROTATION = "Rotation";
        private const TAG_ROT = "Rot";
        private const TAG_MOUTH_MOVING = "MouthMoving";
        private const TAG_MOUTH_TICK_COUNT = "MouthTickCount";

        private MobHeadType $mobHeadType = MobHeadType::SKELETON;
        private int $rotation = 0;

        public function readSaveData(CompoundTag $nbt): void
        {
            if (($skullTypeTag = $nbt->getTag(self::TAG_SKULL_TYPE)) instanceof ByteTag) {
                $mobHeadType = MobHeadTypeIdMap::getInstance()->fromId($skullTypeTag->getValue());
                if ($mobHeadType === null) {
                    throw new SavedDataLoadingException("Invalid skull type tag value " . $skullTypeTag->getValue());
                }
                $this->mobHeadType = $mobHeadType;
            }

            if ($nbt->getTag(self::TAG_ROTATION) instanceof FloatTag) {
                $yaw = $nbt->getFloat(self::TAG_ROTATION, 0.0);
                $this->rotation = ((int) floor(($yaw * 16 / 360) + 0.5)) & 0xf;
            } else {
                $rotation = $nbt->getByte(self::TAG_ROT, 0);
                if ($rotation >= 0 && $rotation <= 15) {
                    $this->rotation = $rotation;
                }
            }
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $nbt->setByte(self::TAG_SKULL_TYPE, MobHeadTypeIdMap::getInstance()->toId($this->mobHeadType));
            $nbt->setFloat(self::TAG_ROTATION, $this->rotation * 360.0 / 16.0);
        }

        public function setMobHeadType(MobHeadType $type): void
        {
            $this->mobHeadType = $type;
        }

        public function getMobHeadType(): MobHeadType
        {
            return $this->mobHeadType;
        }

        public function getRotation(): int
        {
            return $this->rotation;
        }

        public function setRotation(int $rotation): void
        {
            $this->rotation = $rotation;
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            $nbt->setByte(self::TAG_SKULL_TYPE, MobHeadTypeIdMap::getInstance()->toId($this->mobHeadType));
            $nbt->setFloat(self::TAG_ROTATION, $this->rotation * 360.0 / 16.0);
        }
    }

    /**
     * @deprecated
     */
    class MonsterSpawner extends Spawnable
    {
        private const TAG_LEGACY_ENTITY_TYPE_ID = "EntityId";
        private const TAG_ENTITY_TYPE_ID = "EntityIdentifier";
        private const TAG_SPAWN_DELAY = "Delay";
        private const TAG_SPAWN_POTENTIALS = "SpawnPotentials";
        private const TAG_SPAWN_DATA = "SpawnData";
        private const TAG_MIN_SPAWN_DELAY = "MinSpawnDelay";
        private const TAG_MAX_SPAWN_DELAY = "MaxSpawnDelay";
        private const TAG_SPAWN_PER_ATTEMPT = "SpawnCount";
        private const TAG_MAX_NEARBY_ENTITIES = "MaxNearbyEntities";
        private const TAG_REQUIRED_PLAYER_RANGE = "RequiredPlayerRange";
        private const TAG_SPAWN_RANGE = "SpawnRange";
        private const TAG_ENTITY_WIDTH = "DisplayEntityWidth";
        private const TAG_ENTITY_HEIGHT = "DisplayEntityHeight";
        private const TAG_ENTITY_SCALE = "DisplayEntityScale";

        public const DEFAULT_MIN_SPAWN_DELAY = 200;
        public const DEFAULT_MAX_SPAWN_DELAY = 800;

        public const DEFAULT_MAX_NEARBY_ENTITIES = 6;
        public const DEFAULT_SPAWN_RANGE = 4;
        public const DEFAULT_REQUIRED_PLAYER_RANGE = 16;

        private string $entityTypeId = ":";
        private ?ListTag $spawnPotentials = null;
        private ?CompoundTag $spawnData = null;

        private float $displayEntityWidth = 1.0;
        private float $displayEntityHeight = 1.0;
        private float $displayEntityScale = 1.0;

        private int $spawnDelay = self::DEFAULT_MIN_SPAWN_DELAY;
        private int $minSpawnDelay = self::DEFAULT_MIN_SPAWN_DELAY;
        private int $maxSpawnDelay = self::DEFAULT_MAX_SPAWN_DELAY;
        private int $spawnPerAttempt = 1;
        private int $maxNearbyEntities = self::DEFAULT_MAX_NEARBY_ENTITIES;
        private int $spawnRange = self::DEFAULT_SPAWN_RANGE;
        private int $requiredPlayerRange = self::DEFAULT_REQUIRED_PLAYER_RANGE;

        public function readSaveData(CompoundTag $nbt): void
        {
            if (($legacyIdTag = $nbt->getTag(self::TAG_LEGACY_ENTITY_TYPE_ID)) instanceof IntTag) {
                $this->entityTypeId = LegacyEntityIdToStringIdMap::getInstance()->legacyToString($legacyIdTag->getValue()) ?? ":";
            } elseif (($idTag = $nbt->getTag(self::TAG_ENTITY_TYPE_ID)) instanceof StringTag) {
                $this->entityTypeId = $idTag->getValue();
            } else {
                $this->entityTypeId = ":";
            }

            $this->spawnData = ($rawSpawnData = $nbt->getTag(self::TAG_SPAWN_DATA)) instanceof CompoundTag ? $rawSpawnData : null;
            $this->spawnPotentials = ($rawSpawnPotentials = $nbt->getTag(self::TAG_SPAWN_POTENTIALS)) instanceof ListTag ? $rawSpawnPotentials : null;

            $this->spawnDelay = $nbt->getShort(self::TAG_SPAWN_DELAY, self::DEFAULT_MIN_SPAWN_DELAY);
            $this->minSpawnDelay = $nbt->getShort(self::TAG_MIN_SPAWN_DELAY, self::DEFAULT_MIN_SPAWN_DELAY);
            $this->maxSpawnDelay = $nbt->getShort(self::TAG_MAX_SPAWN_DELAY, self::DEFAULT_MAX_SPAWN_DELAY);
            $this->spawnPerAttempt = $nbt->getShort(self::TAG_SPAWN_PER_ATTEMPT, 1);
            $this->maxNearbyEntities = $nbt->getShort(self::TAG_MAX_NEARBY_ENTITIES, self::DEFAULT_MAX_NEARBY_ENTITIES);
            $this->requiredPlayerRange = $nbt->getShort(self::TAG_REQUIRED_PLAYER_RANGE, self::DEFAULT_REQUIRED_PLAYER_RANGE);
            $this->spawnRange = $nbt->getShort(self::TAG_SPAWN_RANGE, self::DEFAULT_SPAWN_RANGE);

            $this->displayEntityWidth = $nbt->getFloat(self::TAG_ENTITY_WIDTH, 1.0);
            $this->displayEntityHeight = $nbt->getFloat(self::TAG_ENTITY_HEIGHT, 1.0);
            $this->displayEntityScale = $nbt->getFloat(self::TAG_ENTITY_SCALE, 1.0);
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $nbt->setString(self::TAG_ENTITY_TYPE_ID, $this->entityTypeId);
            if ($this->spawnData !== null) {
                $nbt->setTag(self::TAG_SPAWN_DATA, clone $this->spawnData);
            }
            if ($this->spawnPotentials !== null) {
                $nbt->setTag(self::TAG_SPAWN_POTENTIALS, clone $this->spawnPotentials);
            }

            $nbt->setShort(self::TAG_SPAWN_DELAY, $this->spawnDelay);
            $nbt->setShort(self::TAG_MIN_SPAWN_DELAY, $this->minSpawnDelay);
            $nbt->setShort(self::TAG_MAX_SPAWN_DELAY, $this->maxSpawnDelay);
            $nbt->setShort(self::TAG_SPAWN_PER_ATTEMPT, $this->spawnPerAttempt);
            $nbt->setShort(self::TAG_MAX_NEARBY_ENTITIES, $this->maxNearbyEntities);
            $nbt->setShort(self::TAG_REQUIRED_PLAYER_RANGE, $this->requiredPlayerRange);
            $nbt->setShort(self::TAG_SPAWN_RANGE, $this->spawnRange);

            $nbt->setFloat(self::TAG_ENTITY_WIDTH, $this->displayEntityWidth);
            $nbt->setFloat(self::TAG_ENTITY_HEIGHT, $this->displayEntityHeight);
            $nbt->setFloat(self::TAG_ENTITY_SCALE, $this->displayEntityScale);
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            $nbt->setString(self::TAG_ENTITY_TYPE_ID, $this->entityTypeId);
            $nbt->setFloat(self::TAG_ENTITY_SCALE, $this->displayEntityScale);
        }
    }

    /**
     * @deprecated
     */
    class Note extends Tile
    {
        private int $pitch = 0;

        public function readSaveData(CompoundTag $nbt): void
        {
            if (($pitch = $nbt->getByte("note", $this->pitch)) > BlockNote::MIN_PITCH && $pitch <= BlockNote::MAX_PITCH) {
                $this->pitch = $pitch;
            }
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $nbt->setByte("note", $this->pitch);
        }

        public function getPitch(): int
        {
            return $this->pitch;
        }

        public function setPitch(int $pitch): void
        {
            if ($pitch < BlockNote::MIN_PITCH || $pitch > BlockNote::MAX_PITCH) {
                throw new \InvalidArgumentException("Pitch must be in range " . BlockNote::MIN_PITCH . " - " . BlockNote::MAX_PITCH);
            }
            $this->pitch = $pitch;
        }
    }

    /**
     * Holds the phase-shift countdown at runtime only ({@see CyclingPotentSulfur});
     * unlike Java Edition, this is not persisted to disk on Bedrock.
     */
    final class PotentSulfur extends Tile
    {
        private int $heartbeatsUntilPhaseShift = 0;

        public function readSaveData(CompoundTag $nbt): void
        {
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
        }

        public function getHeartbeatsUntilPhaseShift(): int
        {
            return $this->heartbeatsUntilPhaseShift;
        }

        public function setHeartbeatsUntilPhaseShift(int $heartbeats): void
        {
            $this->heartbeatsUntilPhaseShift = $heartbeats;
        }
    }

    class ShulkerBox extends Spawnable implements Container, Nameable
    {
        use NameableTrait {
            addAdditionalSpawnData as addNameSpawnData;
        }
        use ContainerTrait;

        public const TAG_FACING = "facing";

        protected int $facing = Facing::NORTH;
        protected ShulkerBoxInventory $inventory;

        public function __construct(World $world, Vector3 $pos)
        {
            parent::__construct($world, $pos);
            $this->inventory = new ShulkerBoxInventory($this->position);
        }

        public function readSaveData(CompoundTag $nbt): void
        {
            $this->loadName($nbt);
            $this->loadItems($nbt);
            $this->facing = $nbt->getByte(self::TAG_FACING, $this->facing);
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
            $this->saveName($nbt);
            $this->saveItems($nbt);
            $nbt->setByte(self::TAG_FACING, $this->facing);
        }

        public function copyDataFromItem(Item $item): void
        {
            $this->readSaveData($item->getNamedTag());
            if ($item->hasCustomName()) {
                $this->setName($item->getCustomName());
            }
        }

        public function close(): void
        {
            if (!$this->closed) {
                if (method_exists($this->inventory, 'removeAllViewers')) {
                    $this->inventory->removeAllViewers();
                }
                parent::close();
            }
        }

        protected function onBlockDestroyedHook(): void
        {
        }

        public function getCleanedNBT(): ?CompoundTag
        {
            $nbt = parent::getCleanedNBT();
            if ($nbt !== null) {
                $nbt->removeTag(self::TAG_FACING);
            }
            return $nbt;
        }

        public function getFacing(): int
        {
            return $this->facing;
        }

        public function setFacing(int $facing): void
        {
            $this->facing = $facing;
        }

        public function getInventory(): ShulkerBoxInventory
        {
            return $this->inventory;
        }

        public function getRealInventory(): ShulkerBoxInventory
        {
            return $this->inventory;
        }

        public function getDefaultName(): string
        {
            return "Shulker Box";
        }

        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
            $nbt->setByte(self::TAG_FACING, $this->facing);
            $this->addNameSpawnData($nbt);
        }
    }

    /**
     * This exists to force the client to update the spore blossom every tick, which is necessary for it to generate
     * particles.
     */
    final class SporeBlossom extends Spawnable
    {
        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
        }

        public function readSaveData(CompoundTag $nbt): void
        {
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
        }
    }

    /**
     * Block entity representing a structure block.
     */
    class StructureBlock extends Spawnable
    {
        protected function addAdditionalSpawnData(CompoundTag $nbt): void
        {
        }

        public function readSaveData(CompoundTag $nbt): void
        {
        }

        protected function writeSaveData(CompoundTag $nbt): void
        {
        }
    }

    /**
     * TileFactory maps tile save IDs to PHP Tile classes and instantiates them from CompoundTags.
     */
    final class TileFactory
    {
        private static ?self $instance = null;

        /** @var array<string, class-string<Tile>> */
        private array $knownTiles = [];

        /** @var array<class-string<Tile>, string> */
        private array $saveNames = [];

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
        }

        public static function setInstance(?self $instance): void
        {
            self::$instance = $instance;
        }

        public static function reset(): void
        {
            self::$instance = null;
        }

        public function __construct()
        {
            $this->register(Barrel::class, ["Barrel", "minecraft:barrel"]);
            $this->register(Banner::class, ["Banner", "minecraft:banner"]);
            $this->register(Beacon::class, ["Beacon", "minecraft:beacon"]);
            $this->register(Bed::class, ["Bed", "minecraft:bed"]);
            $this->register(Bell::class, ["Bell", "minecraft:bell"]);
            $this->register(BlastFurnace::class, ["BlastFurnace", "minecraft:blast_furnace"]);
            $this->register(BrewingStand::class, ["BrewingStand", "minecraft:brewing_stand"]);
            $this->register(Campfire::class, ["Campfire", "minecraft:campfire"]);
            $this->register(Cauldron::class, ["Cauldron", "minecraft:cauldron"]);
            $this->register(Chest::class, ["Chest", "minecraft:chest"]);
            $this->register(ChiseledBookshelf::class, ["ChiseledBookshelf", "minecraft:chiseled_bookshelf"]);
            $this->register(Comparator::class, ["Comparator", "minecraft:comparator"]);
            $this->register(DaylightSensor::class, ["DaylightDetector", "minecraft:daylight_detector"]);
            $this->register(EnchantTable::class, ["EnchantTable", "minecraft:enchanting_table"]);
            $this->register(EnderChest::class, ["EnderChest", "minecraft:ender_chest"]);
            $this->register(FlowerPot::class, ["FlowerPot", "minecraft:flower_pot"]);
            $this->register(NormalFurnace::class, ["Furnace", "minecraft:furnace"]);
            $this->register(Hopper::class, ["Hopper", "minecraft:hopper"]);
            $this->register(ItemFrame::class, ["ItemFrame"]);
            $this->register(Jukebox::class, ["Jukebox", "RecordPlayer", "minecraft:jukebox"]);
            $this->register(Lectern::class, ["Lectern", "minecraft:lectern"]);
            $this->register(MonsterSpawner::class, ["MobSpawner", "minecraft:mob_spawner"]);
            $this->register(Note::class, ["Music", "minecraft:noteblock"]);
            $this->register(PotentSulfur::class, ["PotentSulfurBlock", "minecraft:potent_sulfur"]);
            $this->register(ShulkerBox::class, ["ShulkerBox", "minecraft:shulker_box"]);
            $this->register(Sign::class, ["Sign", "minecraft:sign"]);
            $this->register(Smoker::class, ["Smoker", "minecraft:smoker"]);
            $this->register(SporeBlossom::class, ["SporeBlossom", "minecraft:spore_blossom"]);
            $this->register(MobHead::class, ["Skull", "minecraft:skull"]);
            $this->register(GlowingItemFrame::class, ["GlowItemFrame"]);
            $this->register(HangingSign::class, ["HangingSign", "minecraft:hanging_sign"]);
            $this->register(StructureBlock::class, ["StructureBlock", "minecraft:structure_block"]);
        }

        /**
         * @param string[] $saveNames
         * @phpstan-param class-string<Tile> $className
         */
        public function register(string $className, array $saveNames = []): void
        {
            if (!is_a($className, Tile::class, true)) {
                throw new \InvalidArgumentException("Class $className does not extend " . Tile::class);
            }
            $ref = new \ReflectionClass($className);
            if (!$ref->isInstantiable()) {
                throw new \InvalidArgumentException("Class $className cannot be constructed");
            }

            $shortName = $ref->getShortName();
            if (!in_array($shortName, $saveNames, true)) {
                $saveNames[] = $shortName;
            }

            foreach ($saveNames as $name) {
                $this->knownTiles[$name] = $className;
            }

            $this->saveNames[$className] = reset($saveNames);
        }

        /**
         * @phpstan-param class-string<Tile> $class
         */
        public function isRegistered(string $class): bool
        {
            return isset($this->saveNames[$class]);
        }

        /**
         * @internal
         * @throws SavedDataLoadingException
         */
        public function createFromData(World $world, CompoundTag $nbt): ?Tile
        {
            try {
                $type = $nbt->getString(Tile::TAG_ID, "");
                if (!isset($this->knownTiles[$type])) {
                    return null;
                }
                $class = $this->knownTiles[$type];
                $tile = new $class($world, new Vector3(
                    $nbt->getInt(Tile::TAG_X, 0),
                    $nbt->getInt(Tile::TAG_Y, 0),
                    $nbt->getInt(Tile::TAG_Z, 0)
                ));
                $tile->readSaveData($nbt);
            } catch (NbtException $e) {
                throw new SavedDataLoadingException($e->getMessage(), 0, $e);
            } catch (\Throwable $e) {
                return null;
            }

            return $tile;
        }

        /**
         * @phpstan-param class-string<Tile> $class
         */
        public function getSaveId(string $class): string
        {
            if (isset($this->saveNames[$class])) {
                return $this->saveNames[$class];
            }
            try {
                return (new \ReflectionClass($class))->getShortName();
            } catch (\ReflectionException) {
                return "Tile";
            }
        }
    }
}
