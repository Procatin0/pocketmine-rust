<?php

/*
 *
 *  ____            _        _   __  __ _                  __  __ ____
 * |  _ \ ___   ___| | _____| |_|  \/  (_)_ __   ___      |  \/  |  _ \
 * | |_) / _ \ / __| |/ / _ \ __| |\/| | | '_ \ / _ \_____| |\/| | |_) |
 * |  __/ (_) | (__|   <  __/ |_| |  | | | | | |  __/_____| |  | |  __/
 * |_|   \___/ \___|_|\_\___|\__|_|  |_|_|_| |_|\___|     |_|  |_|_|
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Lesser General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * @author PocketMine Team
 * @link http://www.pocketmine.net/
 *
 *
 */

declare(strict_types=1);

namespace pocketmine\utils {
    if (!trait_exists(LegacyEnumShimTrait::class, false)) {
        trait LegacyEnumShimTrait {
            public static function __callStatic(string $name, array $arguments): static {
                foreach (self::cases() as $case) {
                    if ($case->name === $name) {
                        return $case;
                    }
                }
                throw new \Error("Undefined constant " . static::class . "::$name");
            }
        }
    }
}



namespace pocketmine\world\sound {

    use pocketmine\math\Vector3;

    if (!interface_exists(Sound::class, false)) {
        interface Sound
        {
            /** @return array<int, mixed> */
            public function encode(Vector3 $pos): array;
        }
    }

    if (!class_exists(SoundBase::class, false)) {
        abstract class SoundBase implements Sound
        {
            public function encode(Vector3 $pos): array
            {
                return [];
            }

            public function getSoundName(): ?string
            {
                return null;
            }
        }
    }

    if (!class_exists(SoundId::class, false)) {
        class SoundId
        {
            public function __construct(private string $soundId)
            {
            }

            public function getSoundId(): string
            {
                return $this->soundId;
            }

            public function getName(): string
            {
                return $this->soundId;
            }

            public function __toString(): string
            {
                return $this->soundId;
            }
        }
    }

    if (!class_exists(PaintingPlaceSound::class, false)) {
        class PaintingPlaceSound extends SoundBase
        {
            public function getSoundName(): ?string
            {
                return 'painting.place';
            }
        }
    }
}

namespace pocketmine\entity\object {

    use pocketmine\entity\Entity;
    use pocketmine\math\AxisAlignedBB;
    use pocketmine\math\Facing;
    use pocketmine\math\Vector3;
    use pocketmine\nbt\tag\CompoundTag;
    use pocketmine\world\Location;
    use pocketmine\world\World;
    use function ceil;

    if (!class_exists(PaintingMotive::class, false)) {
        class PaintingMotive
        {
            private static bool $initialized = false;

            /** @var PaintingMotive[] */
            protected static array $motives = [];

            public static function init(): void
            {
                self::$initialized = true;
                foreach ([
                    new PaintingMotive(1, 1, "Alban"),
                    new PaintingMotive(1, 1, "Aztec"),
                    new PaintingMotive(1, 1, "Aztec2"),
                    new PaintingMotive(1, 1, "Bomb"),
                    new PaintingMotive(1, 1, "Kebab"),
                    new PaintingMotive(1, 1, "meditative"),
                    new PaintingMotive(1, 1, "Plant"),
                    new PaintingMotive(1, 1, "Wasteland"),
                    new PaintingMotive(1, 2, "Graham"),
                    new PaintingMotive(1, 2, "prairie_ride"),
                    new PaintingMotive(1, 2, "Wanderer"),
                    new PaintingMotive(2, 1, "Courbet"),
                    new PaintingMotive(2, 1, "Creebet"),
                    new PaintingMotive(2, 1, "Pool"),
                    new PaintingMotive(2, 1, "Sea"),
                    new PaintingMotive(2, 1, "Sunset"),
                    new PaintingMotive(2, 2, "Bust"),
                    new PaintingMotive(2, 2, "baroque"),
                    new PaintingMotive(2, 2, "Earth"),
                    new PaintingMotive(2, 2, "Fire"),
                    new PaintingMotive(2, 2, "humble"),
                    new PaintingMotive(2, 2, "Match"),
                    new PaintingMotive(2, 2, "SkullAndRoses"),
                    new PaintingMotive(2, 2, "Stage"),
                    new PaintingMotive(2, 2, "Void"),
                    new PaintingMotive(2, 2, "Water"),
                    new PaintingMotive(2, 2, "Wind"),
                    new PaintingMotive(2, 2, "Wither"),
                    new PaintingMotive(3, 3, "bouquet"),
                    new PaintingMotive(3, 3, "cavebird"),
                    new PaintingMotive(3, 3, "cotan"),
                    new PaintingMotive(3, 3, "endboss"),
                    new PaintingMotive(3, 3, "fern"),
                    new PaintingMotive(3, 3, "owlemons"),
                    new PaintingMotive(3, 3, "sunflowers"),
                    new PaintingMotive(3, 3, "tides"),
                    new PaintingMotive(3, 4, "backyard"),
                    new PaintingMotive(3, 4, "pond"),
                    new PaintingMotive(4, 2, "changing"),
                    new PaintingMotive(4, 2, "Fighters"),
                    new PaintingMotive(4, 2, "finding"),
                    new PaintingMotive(4, 2, "lowmist"),
                    new PaintingMotive(4, 2, "passage"),
                    new PaintingMotive(4, 3, "DonkeyKong"),
                    new PaintingMotive(4, 3, "Skeleton"),
                    new PaintingMotive(4, 4, "BurningSkull"),
                    new PaintingMotive(4, 4, "orb"),
                    new PaintingMotive(4, 4, "Pigscene"),
                    new PaintingMotive(4, 4, "Pointer"),
                    new PaintingMotive(4, 4, "unpacked"),
                ] as $motive) {
                    self::registerMotive($motive);
                }
            }

            public static function registerMotive(PaintingMotive $motive): void
            {
                if (!self::$initialized) {
                    self::init();
                }
                self::$motives[$motive->getName()] = $motive;
            }

            public static function getMotiveByName(string $name): ?PaintingMotive
            {
                if (!self::$initialized) {
                    self::init();
                }
                return self::$motives[$name] ?? null;
            }

            /** @return PaintingMotive[] */
            public static function getAll(): array
            {
                if (!self::$initialized) {
                    self::init();
                }
                return self::$motives;
            }

            public function __construct(
                protected int $width,
                protected int $height,
                protected string $name
            ) {
            }

            public function getName(): string
            {
                return $this->name;
            }

            public function getWidth(): int
            {
                return $this->width;
            }

            public function getHeight(): int
            {
                return $this->height;
            }

            public function __toString(): string
            {
                return "PaintingMotive(name: " . $this->getName() . ", height: " . $this->getHeight() . ", width: " . $this->getWidth() . ")";
            }
        }
    }

    if (!class_exists(Painting::class, false)) {
        class Painting
        {
            protected ?Location $location = null;
            protected ?Vector3 $motion = null;
            public function getLocation(): ?Location { return $this->location; }
            public function spawnToAll(): void {}

            public const NETWORK_TYPE_ID = 'minecraft:painting';

            public const TAG_TILE_X = "TileX";
            public const TAG_TILE_Y = "TileY";
            public const TAG_TILE_Z = "TileZ";
            public const TAG_FACING_JE = "Facing";
            public const TAG_DIRECTION_BE = "Direction";
            public const TAG_MOTIVE = "Motive";

            public const DATA_TO_FACING = [
                0 => Facing::SOUTH,
                1 => Facing::WEST,
                2 => Facing::NORTH,
                3 => Facing::EAST,
            ];
            private const FACING_TO_DATA = [
                Facing::SOUTH => 0,
                Facing::WEST => 1,
                Facing::NORTH => 2,
                Facing::EAST => 3,
            ];

            protected Vector3 $blockIn;
            protected int $facing;
            protected PaintingMotive $motive;

            public function __construct(Location $location, Vector3 $blockIn, int $facing, PaintingMotive $motive, ?CompoundTag $nbt = null)
            {
                $this->location = $location;
                $this->blockIn = $blockIn->asVector3();
                $this->facing = $facing;
                $this->motive = $motive;
                $this->motion = new Vector3(0.0, 0.0, 0.0);
            }

            public function getMotive(): PaintingMotive
            {
                return $this->motive;
            }

            public function getFacing(): int
            {
                return $this->facing;
            }

            private static function getPaintingBB(int $facing, PaintingMotive $motive): AxisAlignedBB
            {
                $width = $motive->getWidth();
                $height = $motive->getHeight();

                $horizontalStart = (int) (ceil($width / 2) - 1);
                $verticalStart = (int) (ceil($height / 2) - 1);

                return AxisAlignedBB::one()
                    ->trim($facing, 15 / 16)
                    ->extend(Facing::rotateY($facing, true), $horizontalStart)
                    ->extend(Facing::rotateY($facing, false), -$horizontalStart + $width - 1)
                    ->extend(Facing::DOWN, $verticalStart)
                    ->extend(Facing::UP, -$verticalStart + $height - 1);
            }

            public static function canFit(World $world, Vector3 $blockIn, int $facing, bool $checkOverlap, PaintingMotive $motive): bool
            {
                $width = $motive->getWidth();
                $height = $motive->getHeight();

                $horizontalStart = (int) (ceil($width / 2) - 1);
                $verticalStart = (int) (ceil($height / 2) - 1);

                $rotatedFace = Facing::rotateY($facing, false);
                $oppositeSide = Facing::opposite($facing);

                $startPos = $blockIn->asVector3()
                    ->getSide(Facing::opposite($rotatedFace), $horizontalStart)
                    ->getSide(Facing::DOWN, $verticalStart);

                for ($w = 0; $w < $width; ++$w) {
                    for ($h = 0; $h < $height; ++$h) {
                        $pos = $startPos->getSide($rotatedFace, $w)->getSide(Facing::UP, $h);

                        $block = $world->getBlockAt((int)$pos->x, (int)$pos->y, (int)$pos->z);
                        if ($block->isSolid() || !$block->getSide($oppositeSide)->isSolid()) {
                            return false;
                        }
                    }
                }

                if ($checkOverlap) {
                    $bb = self::getPaintingBB($facing, $motive)->offset($blockIn->x, $blockIn->y, $blockIn->z);
                    foreach ($world->getNearbyEntities($bb) as $entity) {
                        if ($entity instanceof self) {
                            return false;
                        }
                    }
                }

                return true;
            }

            protected function spawnHostCall(): ?array
            {
                $location = $this->getLocation();
                return ['entity.spawn_painting', [
                    'x' => $location->x,
                    'y' => $location->y,
                    'z' => $location->z,
                    'facing' => $this->facing,
                    'motive' => $this->motive->getName(),
                ]];
            }
        }
    }

    if (!class_exists(EndCrystal::class, false)) {
        class EndCrystal
        {
            protected ?Location $location = null;
            protected ?Vector3 $motion = null;
            public function getLocation(): ?Location { return $this->location; }
            public function spawnToAll(): void {}

            public const NETWORK_TYPE_ID = 'minecraft:ender_crystal';

            protected bool $showBase = false;
            protected ?Vector3 $beamTarget = null;

            public function __construct(Location $location, ?CompoundTag $nbt = null)
            {
                $this->location = $location;
                $this->motion = new Vector3(0.0, 0.0, 0.0);
            }

            public function isFireProof(): bool
            {
                return true;
            }

            public function showBase(): bool
            {
                return $this->showBase;
            }

            public function setShowBase(bool $showBase): void
            {
                $this->showBase = $showBase;
            }

            public function getBeamTarget(): ?Vector3
            {
                return $this->beamTarget;
            }

            public function setBeamTarget(?Vector3 $beamTarget): void
            {
                $this->beamTarget = $beamTarget;
            }

            protected function spawnHostCall(): ?array
            {
                $location = $this->getLocation();
                return ['entity.spawn_crystal', [
                    'x' => $location->x,
                    'y' => $location->y,
                    'z' => $location->z,
                ]];
            }
        }
    }
}

namespace pocketmine\entity\vehicle {

    use pocketmine\entity\Entity;

    if (!class_exists(Boat::class, false)) {
        class Boat
        {
            protected ?Location $location = null;
            protected ?Vector3 $motion = null;
            public function getLocation(): ?Location { return $this->location; }
            public function spawnToAll(): void {}

            public const NETWORK_TYPE_ID = 'minecraft:boat';

            protected function spawnHostCall(): ?array
            {
                $loc = $this->getLocation();
                return ['entity.spawn_boat', [
                    'x' => $loc->x,
                    'y' => $loc->y,
                    'z' => $loc->z,
                ]];
            }
        }
    }

    if (!class_exists(Minecart::class, false)) {
        class Minecart
        {
            protected ?Location $location = null;
            protected ?Vector3 $motion = null;
            public function getLocation(): ?Location { return $this->location; }
            public function spawnToAll(): void {}

            public const NETWORK_TYPE_ID = 'minecraft:minecart';

            protected function spawnHostCall(): ?array
            {
                $loc = $this->getLocation();
                return ['entity.spawn_minecart', [
                    'x' => $loc->x,
                    'y' => $loc->y,
                    'z' => $loc->z,
                ]];
            }
        }
    }
}

namespace pocketmine\item {

    use pocketmine\block\Block;
    use pocketmine\block\BlockTypeIds;
    use pocketmine\block\RuntimeBlockStateRegistry;
    use pocketmine\block\tile\Banner as TileBanner;
    use pocketmine\block\utils\BannerPatternLayer;
    use pocketmine\block\utils\CoralType;
    use pocketmine\block\utils\CoralTypeTrait;
    use pocketmine\block\utils\DyeColor;
    use pocketmine\block\utils\RecordType;
    use pocketmine\block\utils\WoodType;
    use pocketmine\block\VanillaBlocks;
    use pocketmine\data\bedrock\BannerPatternTypeIdMap;
    use pocketmine\data\bedrock\DyeColorIdMap;
    use pocketmine\data\runtime\RuntimeDataDescriber;
    use pocketmine\entity\Location;
    use pocketmine\entity\object\EndCrystal as EntityEndCrystal;
    use pocketmine\entity\object\Painting;
    use pocketmine\entity\object\PaintingMotive;
    use pocketmine\math\Axis;
    use pocketmine\math\AxisAlignedBB;
    use pocketmine\math\Facing;
    use pocketmine\math\Vector3;
    use pocketmine\nbt\tag\CompoundTag;
    use pocketmine\nbt\tag\ListTag;
    use pocketmine\player\Player;
    use pocketmine\utils\LegacyEnumShimTrait;
    use pocketmine\world\BlockTransaction;
    use pocketmine\world\sound\PaintingPlaceSound;
    use pocketmine\world\sound\SoundId;
    use function array_rand;
    use function class_alias;
    use function class_exists;
    use function count;
    use function is_string;
    use function method_exists;

    if (!class_exists(SoundId::class, false)) {
        class_alias(\pocketmine\world\sound\SoundId::class, SoundId::class);
    }

    /**
     * @method static BoatType ACACIA()
     * @method static BoatType BIRCH()
     * @method static BoatType DARK_OAK()
     * @method static BoatType JUNGLE()
     * @method static BoatType MANGROVE()
     * @method static BoatType OAK()
     * @method static BoatType SPRUCE()
     */
    enum BoatType
    {
        use LegacyEnumShimTrait;

        case OAK;
        case SPRUCE;
        case BIRCH;
        case JUNGLE;
        case ACACIA;
        case DARK_OAK;
        case MANGROVE;

        public function getWoodType(): WoodType
        {
            return match ($this) {
                self::OAK => WoodType::OAK,
                self::SPRUCE => WoodType::SPRUCE,
                self::BIRCH => WoodType::BIRCH,
                self::JUNGLE => WoodType::JUNGLE,
                self::ACACIA => WoodType::ACACIA,
                self::DARK_OAK => WoodType::DARK_OAK,
                self::MANGROVE => WoodType::MANGROVE,
            };
        }

        public function getDisplayName(): string
        {
            return $this->getWoodType()->getDisplayName();
        }
    }

    class ItemBlockWallOrFloor extends Item
    {
        protected int $floorVariant;
        protected int $wallVariant;
        protected ?Block $floorVariantBlock = null;
        protected ?Block $wallVariantBlock = null;

        public function __construct(ItemIdentifier $identifier, Block $floorVariant, Block $wallVariant)
        {
            parent::__construct($identifier, $floorVariant->getName());
            $this->floorVariant = $floorVariant->getStateId();
            $this->wallVariant = $wallVariant->getStateId();
            $this->floorVariantBlock = clone $floorVariant;
            $this->wallVariantBlock = clone $wallVariant;
        }

        public function getBlock(?int $clickedFace = null): Block
        {
            if ($clickedFace !== null && Facing::axis($clickedFace) !== Axis::Y) {
                return RuntimeBlockStateRegistry::getInstance()->fromStateId($this->wallVariant) ?? ($this->wallVariantBlock !== null ? clone $this->wallVariantBlock : VanillaBlocks::AIR());
            }
            return RuntimeBlockStateRegistry::getInstance()->fromStateId($this->floorVariant) ?? ($this->floorVariantBlock !== null ? clone $this->floorVariantBlock : VanillaBlocks::AIR());
        }

        public function getFuelTime(): int
        {
            return $this->getBlock()->getFuelTime();
        }

        public function getMaxStackSize(): int
        {
            return $this->getBlock()->getMaxStackSize();
        }
    }

    class Banner extends ItemBlockWallOrFloor
    {
        public const TAG_PATTERNS = TileBanner::TAG_PATTERNS;
        public const TAG_PATTERN_COLOR = TileBanner::TAG_PATTERN_COLOR;
        public const TAG_PATTERN_NAME = TileBanner::TAG_PATTERN_NAME;

        private DyeColor $color = DyeColor::BLACK;

        /**
         * @var BannerPatternLayer[]
         * @phpstan-var list<BannerPatternLayer>
         */
        private array $patterns = [];

        public function __construct(ItemIdentifier $identifier, ?Block $floorVariant = null, ?Block $wallVariant = null)
        {
            $floor = $floorVariant ?? VanillaBlocks::BANNER();
            $wall = $wallVariant ?? VanillaBlocks::WALL_BANNER();
            parent::__construct($identifier, $floor, $wall);
        }

        public function getColor(): DyeColor
        {
            return $this->color;
        }

        /** @return $this */
        public function setColor(DyeColor $color): self
        {
            $this->color = $color;
            return $this;
        }

        protected function describeState(RuntimeDataDescriber $w): void
        {
            $w->enum($this->color);
        }

        /**
         * @return BannerPatternLayer[]
         * @phpstan-return list<BannerPatternLayer>
         */
        public function getPatterns(): array
        {
            return $this->patterns;
        }

        /**
         * @param BannerPatternLayer[] $patterns
         *
         * @phpstan-param list<BannerPatternLayer> $patterns
         *
         * @return $this
         */
        public function setPatterns(array $patterns): self
        {
            $this->patterns = $patterns;
            return $this;
        }

        public function getFuelTime(): int
        {
            return 300;
        }

        public function getBlock(?int $clickedFace = null): Block
        {
            $block = parent::getBlock($clickedFace);
            if (method_exists($block, 'setColor')) {
                $block->setColor($this->color);
            }
            return $block;
        }

        protected function deserializeCompoundTag(CompoundTag $tag): void
        {
            parent::deserializeCompoundTag($tag);

            $this->patterns = [];

            $colorIdMap = DyeColorIdMap::getInstance();
            $patternIdMap = BannerPatternTypeIdMap::getInstance();
            $patterns = $tag->getListTag(self::TAG_PATTERNS, CompoundTag::class);
            if ($patterns !== null) {
                foreach ($patterns as $t) {
                    $patternColor = $colorIdMap->fromInvertedId($t->getInt(self::TAG_PATTERN_COLOR)) ?? DyeColor::BLACK;
                    $patternType = $patternIdMap->fromId($t->getString(self::TAG_PATTERN_NAME));
                    if ($patternType === null) {
                        continue;
                    }
                    $this->patterns[] = new BannerPatternLayer($patternType, $patternColor);
                }
            }
        }

        protected function serializeCompoundTag(CompoundTag $tag): void
        {
            parent::serializeCompoundTag($tag);

            if (count($this->patterns) > 0) {
                $patterns = new ListTag();
                $colorIdMap = DyeColorIdMap::getInstance();
                $patternIdMap = BannerPatternTypeIdMap::getInstance();
                foreach ($this->patterns as $pattern) {
                    $patterns->push(CompoundTag::create()
                        ->setString(self::TAG_PATTERN_NAME, $patternIdMap->toId($pattern->getType()))
                        ->setInt(self::TAG_PATTERN_COLOR, $colorIdMap->toInvertedId($pattern->getColor()))
                    );
                }

                $tag->setTag(self::TAG_PATTERNS, $patterns);
            } else {
                $tag->removeTag(self::TAG_PATTERNS);
            }
        }
    }

    class CoralFan extends ItemBlockWallOrFloor
    {
        use CoralTypeTrait {
            describeBlockItemState as encodeCoralType;
        }

        public function __construct(ItemIdentifier $identifier, ?Block $floorVariant = null, ?Block $wallVariant = null)
        {
            $floor = $floorVariant ?? VanillaBlocks::CORAL_FAN();
            $wall = $wallVariant ?? VanillaBlocks::WALL_CORAL_FAN();
            parent::__construct($identifier, $floor, $wall);
        }

        protected function describeState(RuntimeDataDescriber $w): void
        {
            $w->enum($this->coralType);
            $w->bool($this->dead);
        }

        public function getBlock(?int $clickedFace = null): Block
        {
            $block = parent::getBlock($clickedFace);
            if (method_exists($block, 'setCoralType')) {
                $block->setCoralType($this->coralType);
            }
            if (method_exists($block, 'setDead')) {
                $block->setDead($this->dead);
            }
            return $block;
        }

        public function getFuelTime(): int
        {
            return $this->getBlock()->getFuelTime();
        }

        public function getMaxStackSize(): int
        {
            return $this->getBlock()->getMaxStackSize();
        }
    }

    class HangingSign extends ItemBlockWallOrFloor
    {
        protected Block $centerPointCeilingVariant;
        protected Block $edgePointCeilingVariant;

        public function __construct(
            ItemIdentifier $identifier,
            string|Block $nameOrFloorVariant,
            ?Block $centerPointOrWallVariant = null,
            ?Block $edgePointCeilingVariant = null,
            ?Block $wallVariant = null
        ) {
            if ($nameOrFloorVariant instanceof Block) {
                $floor = $nameOrFloorVariant;
                $wall = $centerPointOrWallVariant ?? clone $floor;
                parent::__construct($identifier, $floor, $wall);
                $this->centerPointCeilingVariant = clone $floor;
                $this->edgePointCeilingVariant = clone $floor;
                $this->wallVariantBlock = clone $wall;
            } else {
                $name = (string)$nameOrFloorVariant;
                $center = $centerPointOrWallVariant ?? VanillaBlocks::AIR();
                $edge = $edgePointCeilingVariant ?? clone $center;
                $wall = $wallVariant ?? clone $center;
                parent::__construct($identifier, $center, $wall);
                $this->name = $name;
                $this->centerPointCeilingVariant = clone $center;
                $this->edgePointCeilingVariant = clone $edge;
                $this->wallVariantBlock = clone $wall;
            }
        }

        public function getPlacementTransaction(Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): ?BlockTransaction
        {
            if ($face !== Facing::DOWN) {
                return $this->tryPlacementTransaction(clone $this->wallVariantBlock, $blockReplace, $blockClicked, $face, $clickVector, $player);
            }
            // Ceiling edges sign has stricter placement conditions than ceiling center sign, so try that first
            $ceilingEdgeTx = $player === null || !$player->isSneaking() ?
                $this->tryPlacementTransaction(clone $this->edgePointCeilingVariant, $blockReplace, $blockClicked, $face, $clickVector, $player) :
                null;
            return $ceilingEdgeTx ?? $this->tryPlacementTransaction(clone $this->centerPointCeilingVariant, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function getBlock(?int $clickedFace = null): Block
        {
            return $clickedFace === Facing::DOWN ? clone $this->centerPointCeilingVariant : clone $this->wallVariantBlock;
        }

        public function getMaxStackSize(): int
        {
            return 16;
        }

        public function getFuelTime(): int
        {
            return 200;
        }
    }

    class Boat extends Item
    {
        private BoatType $boatType;

        public function __construct(ItemIdentifier $identifier, string $name, BoatType $boatType)
        {
            parent::__construct($identifier, $name);
            $this->boatType = $boatType;
        }

        public function getType(): BoatType
        {
            return $this->boatType;
        }

        public function getFuelTime(): int
        {
            return 1200; // 400 in PC
        }

        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function onInteractBlock(Player $player, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, array &$returnedItems): ItemUseResult
        {
            if (class_exists('pocketmine\\entity\\vehicle\\Boat')) {
                try {
                    $class = 'pocketmine\\entity\\vehicle\\Boat';
                    $pos = $blockReplace->getPosition();
                    $entity = new $class(Location::fromObject($pos->add(0.5, 0.0, 0.5), $pos->getWorld()));
                    if (method_exists($entity, 'setType')) {
                        $entity->setType($this->boatType);
                    }
                    $entity->spawnToAll();
                    $this->pop();
                    return ItemUseResult::SUCCESS;
                } catch (\Throwable) {
                    return ItemUseResult::NONE;
                }
            }
            return ItemUseResult::NONE;
        }
    }

    class Minecart extends Item
    {
        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function onInteractBlock(Player $player, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, array &$returnedItems): ItemUseResult
        {
            if (class_exists('pocketmine\\entity\\vehicle\\Minecart')) {
                try {
                    $class = 'pocketmine\\entity\\vehicle\\Minecart';
                    $pos = $blockReplace->getPosition();
                    $entity = new $class(Location::fromObject($pos->add(0.5, 0.0, 0.5), $pos->getWorld()));
                    $entity->spawnToAll();
                    $this->pop();
                    return ItemUseResult::SUCCESS;
                } catch (\Throwable) {
                    return ItemUseResult::NONE;
                }
            }
            return ItemUseResult::NONE;
        }
    }

    class PaintingItem extends Item
    {
        public function onInteractBlock(Player $player, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, array &$returnedItems): ItemUseResult
        {
            if (Facing::axis($face) === Axis::Y) {
                return ItemUseResult::NONE;
            }

            $motives = [];
            $totalDimension = 0;

            foreach (PaintingMotive::getAll() as $motive) {
                $currentTotalDimension = $motive->getHeight() + $motive->getWidth();
                if ($currentTotalDimension < $totalDimension) {
                    continue;
                }

                if (Painting::canFit($player->getWorld(), $blockReplace->getPosition(), $face, true, $motive)) {
                    if ($currentTotalDimension > $totalDimension) {
                        $totalDimension = $currentTotalDimension;
                        /*
                         * This drops all motive possibilities smaller than this
                         * We use the total of height + width to allow equal chance of horizontal/vertical paintings
                         * when there is an L-shape of space available.
                         */
                        $motives = [];
                    }

                    $motives[] = $motive;
                }
            }

            if (count($motives) === 0) { // No space available
                return ItemUseResult::NONE;
            }

            /** @var PaintingMotive $motive */
            $motive = $motives[array_rand($motives)];

            $replacePos = $blockReplace->getPosition();
            $clickedPos = $blockClicked->getPosition();

            try {
                $entity = new Painting(Location::fromObject($replacePos, $replacePos->getWorld()), $clickedPos, $face, $motive);
                $this->pop();
                $entity->spawnToAll();
            } catch (\Throwable) {
                $this->pop();
            }

            try {
                if (class_exists(PaintingPlaceSound::class)) {
                    $player->getWorld()->addSound($replacePos->add(0.5, 0.5, 0.5), new PaintingPlaceSound());
                }
            } catch (\Throwable) {
                // Ignore sound failure
            }

            return ItemUseResult::SUCCESS;
        }
    }

    class Record extends Item
    {
        private RecordType $recordType;

        public function __construct(ItemIdentifier $identifier, RecordType|string $recordTypeOrName, RecordType|string $nameOrRecordType = "")
        {
            if ($recordTypeOrName instanceof RecordType) {
                $this->recordType = $recordTypeOrName;
                $name = is_string($nameOrRecordType) && $nameOrRecordType !== "" ? $nameOrRecordType : $recordTypeOrName->getSoundName();
            } else {
                $name = $recordTypeOrName;
                $this->recordType = $nameOrRecordType instanceof RecordType ? $nameOrRecordType : RecordType::DISK_13;
            }
            parent::__construct($identifier, $name);
        }

        public function getRecordType(): RecordType
        {
            return $this->recordType;
        }

        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function getSoundId(): ?SoundId
        {
            return new SoundId($this->recordType->name);
        }

        public function getDuration(): int
        {
            return match ($this->recordType) {
                RecordType::DISK_13 => 178,
                RecordType::DISK_CAT => 185,
                RecordType::DISK_BLOCKS => 345,
                RecordType::DISK_CHIRP => 185,
                RecordType::DISK_FAR => 174,
                RecordType::DISK_MALL => 197,
                RecordType::DISK_MELLOHI => 96,
                RecordType::DISK_STAL => 150,
                RecordType::DISK_STRAD => 188,
                RecordType::DISK_WARD => 251,
                RecordType::DISK_11 => 71,
                RecordType::DISK_WAIT => 238,
                RecordType::DISK_5 => 178,
                RecordType::DISK_PIGSTEP => 149,
                RecordType::DISK_OTHERSIDE => 195,
                RecordType::DISK_RELIC => 218,
                RecordType::DISK_CREATOR => 176,
                RecordType::DISK_CREATOR_MUSIC_BOX => 73,
                RecordType::DISK_PRECIPICE => 299,
                RecordType::DISK_LAVA_CHICKEN => 135,
            };
        }
    }

    class EndCrystal extends Item
    {
        public function onInteractBlock(Player $player, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, array &$returnedItems): ItemUseResult
        {
            if ($blockClicked->getTypeId() === BlockTypeIds::OBSIDIAN || $blockClicked->getTypeId() === BlockTypeIds::BEDROCK) {
                $pos = $blockClicked->getPosition();
                $world = $pos->getWorld();
                $bb = AxisAlignedBB::one()
                    ->offset($pos->getX(), $pos->getY(), $pos->getZ())
                    ->extend(Facing::UP, 1);
                if (
                    count($world->getNearbyEntities($bb)) === 0 &&
                    $blockClicked->getSide(Facing::UP)->getTypeId() === BlockTypeIds::AIR &&
                    $blockClicked->getSide(Facing::UP, 2)->getTypeId() === BlockTypeIds::AIR
                ) {
                    try {
                        $crystal = new EntityEndCrystal(Location::fromObject($pos->add(0.5, 1.0, 0.5), $world));
                        $crystal->spawnToAll();
                    } catch (\Throwable) {
                        // Fallback gracefully without fatal errors
                    }

                    $this->pop();
                    return ItemUseResult::SUCCESS;
                }
            }
            return ItemUseResult::NONE;
        }
    }
}
