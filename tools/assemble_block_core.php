<?php
declare(strict_types=1);

$regCode = file_get_contents(__DIR__ . '/generated_core_registry.php');

// Extract BlockTypeTags, BlockToolType, BlockTypeIds, VanillaBlocks from $regCode
$tagStart = strpos($regCode, 'final class BlockTypeTags');
$toolStart = strpos($regCode, 'final class BlockToolType');
$btiStart = strpos($regCode, 'final class BlockTypeIds');
$vbStart = strpos($regCode, 'final class VanillaBlocks');

// Let's build block.php
$out = <<<'HEADER'
<?php

declare(strict_types=1);

namespace pocketmine\data\runtime {

    /**
     * PM `InvalidSerializedRuntimeDataException`: a block's runtime state did
     * not fit the bits its `describeType()` declared.
     */
    class InvalidSerializedRuntimeDataException extends \InvalidArgumentException
    {
    }

    /**
     * PM `RuntimeDataDescriber`: what a block receives in `describeType()` to
     * declare the runtime properties that make up its Bedrock block state.
     */
    interface RuntimeDataDescriber
    {
        public function int(int $bits, int $value): void;

        public function int32(int $value): void;

        public function byte(int $value): void;

        public function bool(bool $value): void;

        public function float(float $value): void;

        public function string(string $value): void;
    }
}

namespace pocketmine\block {

    use pocketmine\block\utils\SupportType;
    use pocketmine\data\runtime\InvalidSerializedRuntimeDataException;
    use pocketmine\data\runtime\RuntimeDataDescriber;
    use pocketmine\entity\Entity;
    use pocketmine\entity\projectile\Projectile;
    use pocketmine\item\enchantment\VanillaEnchantments;
    use pocketmine\item\Item;
    use pocketmine\math\Axis;
    use pocketmine\math\AxisAlignedBB;
    use pocketmine\math\Facing;
    use pocketmine\math\Position;
    use pocketmine\math\RayTraceResult;
    use pocketmine\math\Vector3;
    use pocketmine\player\Player;
    use pocketmine\runtime\Bridge;
    use pocketmine\world\BlockFactory;
    use pocketmine\world\BlockTransaction;
    use pocketmine\world\World;

    /**
     * PM `BlockIdentifier`: the immutable identity of a block type (its type id
     * plus an optional tile class and variant).
     */
    class BlockIdentifier
    {
        public function __construct(
            private int $blockTypeId,
            private ?string $tileClass = null
        ) {
            if ($blockTypeId < 0) {
                throw new \InvalidArgumentException("Block type ID may not be negative");
            }
        }

        public function getBlockTypeId(): int
        {
            return $this->blockTypeId;
        }

        public function getTileClass(): ?string
        {
            return $this->tileClass;
        }

        /** Bridge backwards compatibility */
        public function getVariant(): ?int
        {
            return null;
        }
    }

    /**
     * PM `BlockBreakInfo`: hardness and tool requirements.
     */
    class BlockBreakInfo
    {
        public const COMPATIBLE_TOOL_MULTIPLIER = 1.5;
        public const INCOMPATIBLE_TOOL_MULTIPLIER = 5.0;

        private float $blastResistance;
        private bool $explosionHarvestable;

        public function __construct(
            private float $hardness,
            private int $toolType = BlockToolType::NONE,
            private int $toolHarvestLevel = 0,
            ?float $blastResistance = null,
            ?bool $explosionHarvestable = null,
        ) {
            $this->blastResistance = $blastResistance ?? $hardness * 5;
            $this->explosionHarvestable = $explosionHarvestable ?? (($this->toolType & BlockToolType::PICKAXE) !== 0);
        }

        public static function tier(float $hardness, int $toolType, mixed $toolTier, ?float $blastResistance = null): self
        {
            $harvestLevel = is_object($toolTier) && method_exists($toolTier, 'getHarvestLevel') ? $toolTier->getHarvestLevel() : (int)$toolTier;
            return new self($hardness, $toolType, $harvestLevel, $blastResistance);
        }

        public static function pickaxe(float $hardness, mixed $toolTier = null, ?float $blastResistance = null): self
        {
            $harvestLevel = is_object($toolTier) && method_exists($toolTier, 'getHarvestLevel') ? $toolTier->getHarvestLevel() : (int)($toolTier ?? 0);
            return new self($hardness, BlockToolType::PICKAXE, $harvestLevel, $blastResistance);
        }

        public static function shovel(float $hardness, mixed $toolTier = null, ?float $blastResistance = null): self
        {
            $harvestLevel = is_object($toolTier) && method_exists($toolTier, 'getHarvestLevel') ? $toolTier->getHarvestLevel() : (int)($toolTier ?? 0);
            return new self($hardness, BlockToolType::SHOVEL, $harvestLevel, $blastResistance);
        }

        public static function axe(float $hardness, mixed $toolTier = null, ?float $blastResistance = null): self
        {
            $harvestLevel = is_object($toolTier) && method_exists($toolTier, 'getHarvestLevel') ? $toolTier->getHarvestLevel() : (int)($toolTier ?? 0);
            return new self($hardness, BlockToolType::AXE, $harvestLevel, $blastResistance);
        }

        public static function instant(int $toolType = BlockToolType::NONE, int $toolHarvestLevel = 0): self
        {
            return new self(0.0, $toolType, $toolHarvestLevel, 0.0);
        }

        public static function indestructible(float $blastResistance = 18000003.75): self
        {
            return new self(-1.0, BlockToolType::NONE, 0, $blastResistance);
        }

        public static function byHand(float $hardness): self
        {
            return new self($hardness);
        }

        public static function byTool(mixed $toolType, mixed $toolTier, float $hardness): self
        {
            $tType = is_int($toolType) ? $toolType : BlockToolType::NONE;
            $tHarvest = is_object($toolTier) && method_exists($toolTier, 'getHarvestLevel') ? $toolTier->getHarvestLevel() : (int)($toolTier ?? 0);
            return new self($hardness, $tType, $tHarvest);
        }

        public function getHardness(): float
        {
            return $this->hardness;
        }

        public function isBreakable(): bool
        {
            return $this->hardness >= 0;
        }

        public function breaksInstantly(): bool
        {
            return $this->hardness === 0.0;
        }

        public function getBlastResistance(): float
        {
            return $this->blastResistance;
        }

        public function getToolType(): int
        {
            return $this->toolType;
        }

        public function getToolHarvestLevel(): int
        {
            return $this->toolHarvestLevel;
        }

        public function isToolCompatible(mixed $tool): bool
        {
            if ($this->hardness < 0) {
                return false;
            }
            if ($this->toolType === BlockToolType::NONE || $this->toolHarvestLevel === 0) {
                return true;
            }
            if (!is_object($tool) || !method_exists($tool, 'getBlockToolType')) {
                return false;
            }
            $toolType = $tool->getBlockToolType();
            $toolHarvestLevel = method_exists($tool, 'getBlockToolHarvestLevel') ? $tool->getBlockToolHarvestLevel() : 0;
            return ($this->toolType & $toolType) !== 0 && $toolHarvestLevel >= $this->toolHarvestLevel;
        }

        public function getBreakTime(mixed $item): float
        {
            $base = $this->hardness;
            if ($this->isToolCompatible($item)) {
                $base *= self::COMPATIBLE_TOOL_MULTIPLIER;
            } else {
                $base *= self::INCOMPATIBLE_TOOL_MULTIPLIER;
            }

            $efficiency = (is_object($item) && method_exists($item, 'getMiningEfficiency'))
                ? $item->getMiningEfficiency(($this->toolType & $item->getBlockToolType()) !== 0)
                : 1.0;
            if ($efficiency <= 0) {
                throw new \InvalidArgumentException(get_class($item) . " must have a positive mining efficiency, but got $efficiency");
            }

            return $base / $efficiency;
        }

        public function isExplosionHarvestable(): bool
        {
            return $this->explosionHarvestable;
        }

        public function isInstantBreak(): bool
        {
            return $this->breaksInstantly();
        }

        public function isAffectedByFortune(): bool
        {
            return false;
        }
    }

    /**
     * PM `BlockTypeInfo`: everything about a block type that is not its state.
     */
    final class BlockTypeInfo
    {
        /** @var array<string, true> */
        private array $typeTags;

        /**
         * @param string[] $typeTags
         * @param string[] $enchantmentTags
         */
        public function __construct(
            private BlockBreakInfo $breakInfo,
            array $typeTags = [],
            private array $enchantmentTags = []
        ) {
            $this->typeTags = array_fill_keys($typeTags, true);
        }

        public function getBreakInfo(): BlockBreakInfo
        {
            return $this->breakInfo;
        }

        /** @return string[] */
        public function getTypeTags(): array
        {
            return array_keys($this->typeTags);
        }

        public function hasTypeTag(string $tag): bool
        {
            return isset($this->typeTags[$tag]);
        }

        /** @return string[] */
        public function getEnchantmentTags(): array
        {
            return $this->enchantmentTags;
        }
    }

HEADER;

// Read BlockTypeTags, BlockToolType, BlockTypeIds, VanillaBlocks from generated_core_registry.php
$regParts = substr($regCode, strpos($regCode, 'final class BlockTypeTags'));

// Now build Block class with all 70 methods
$blockClass = <<<'BLOCK_CLASS'

    /**
     * PM `Block`, complete PM5 implementation with 70 public methods.
     */
    class Block
    {
        public const AIR_STATE_ID = 3690217760;
        public const INTERNAL_STATE_DATA_BITS = 32;

        protected BlockIdentifier $idInfo;
        protected string $fallbackName;
        protected BlockTypeInfo $typeInfo;
        protected Position $position;
        protected ?array $collisionBoxes = null;

        private int $x = 0;
        private int $y = 0;
        private int $z = 0;
        private string $worldName = 'world';
        private int $stateId;

        /** @var array<int, array<string, mixed>|null> */
        private static array $stateCache = [];
        /** @var array<int, array<string, mixed>|null> */
        private static array $breakInfoCache = [];

        public function __construct(
            BlockIdentifier $idInfo,
            string $name,
            BlockTypeInfo $typeInfo
        ) {
            $this->idInfo = $idInfo;
            $this->fallbackName = $name;
            $this->typeInfo = $typeInfo;
            $this->position = new Position(0, 0, 0, $this->worldName);
            $this->stateId = $idInfo->getBlockTypeId();
        }

        public function __clone()
        {
            $this->position = clone $this->position;
        }

        // --- 1. Identity & Naming ---

        public function getIdInfo(): BlockIdentifier
        {
            return $this->idInfo;
        }

        public function getName(): string
        {
            return $this->fallbackName;
        }

        public function getTypeId(): int
        {
            return $this->idInfo->getBlockTypeId();
        }

        public function getStateId(): int
        {
            return $this->stateId;
        }

        public function hasSameTypeId(Block $other): bool
        {
            return $this->getTypeId() === $other->getTypeId();
        }

        public function isSameState(Block $other): bool
        {
            return $this->getStateId() === $other->getStateId();
        }

        public function getTypeTags(): array
        {
            return $this->typeInfo->getTypeTags();
        }

        public function hasTypeTag(string $tag): bool
        {
            return $this->typeInfo->hasTypeTag($tag);
        }

        // --- 2. State & Permutations ---

        public function asItem(): Item
        {
            try {
                return \pocketmine\item\ItemFactory::fromString($this->getTypeName());
            } catch (\Throwable) {
                return \pocketmine\item\ItemFactory::get(0, 0, 1);
            }
        }

        public function describeBlockItemState(RuntimeDataDescriber $w): void
        {
        }

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
        }

        public function generateStatePermutations(): \Generator
        {
            yield clone $this;
        }

        public function readStateFromWorld(): Block
        {
            return $this;
        }

        public function writeStateToWorld(): void
        {
            $this->write();
        }

        // --- 3. Placement & Breaking ---

        public function canBePlaced(): bool
        {
            return true;
        }

        public function canBeReplaced(): bool
        {
            return false;
        }

        public function canBePlacedAt(Block $blockReplace, Vector3 $clickVector, int $face, bool $isClickedBlock): bool
        {
            return $blockReplace->canBeReplaced();
        }

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            $tx->addBlock($blockReplace->getPosition(), $this);
            return true;
        }

        public function onPostPlace(): void
        {
        }

        public function getBreakInfo(): BlockBreakInfo
        {
            return $this->typeInfo->getBreakInfo();
        }

        public function getEnchantmentTags(): array
        {
            return $this->typeInfo->getEnchantmentTags();
        }

        public function onBreak(Item $item, ?Player $player = null, array &$returnedItems = []): bool
        {
            $this->getWorld()->setBlock($this->getPosition(), VanillaBlocks::AIR());
            return true;
        }

        public function onNearbyBlockChange(): void
        {
        }

        public function ticksRandomly(): bool
        {
            return false;
        }

        public function onRandomTick(): void
        {
        }

        public function onScheduledUpdate(): void
        {
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            return false;
        }

        public function onAttack(Item $item, int $face, ?Player $player = null): bool
        {
            return false;
        }

        // --- 4. Physics & Properties ---

        public function getFrictionFactor(): float
        {
            return 0.6;
        }

        public function getLightLevel(): int
        {
            return 0;
        }

        public function getLightFilter(): int
        {
            return $this->isTransparent() ? 0 : 15;
        }

        public function blocksDirectSkyLight(): bool
        {
            return $this->getLightFilter() > 0;
        }

        public function isTransparent(): bool
        {
            return false;
        }

        public function isSolid(): bool
        {
            $info = self::stateInfo($this->stateId);
            if (is_array($info) && array_key_exists('solid', $info)) {
                return (bool)$info['solid'];
            }
            return true;
        }

        public function canBeFlowedInto(): bool
        {
            return false;
        }

        public function canClimb(): bool
        {
            return false;
        }

        final public function getPosition(): Position
        {
            return new Position($this->x, $this->y, $this->z, $this->worldName);
        }

        final public function position(World $world, int $x, int $y, int $z): void
        {
            $this->worldName = $world->getFolderName();
            $this->x = $x;
            $this->y = $y;
            $this->z = $z;
            $this->position = new Position($x, $y, $z, $this->worldName);
            $this->collisionBoxes = null;
        }

        // --- 5. Drops & Fuel ---

        public function getDrops(mixed $item = null): array
        {
            if ($item instanceof Item && $this->getBreakInfo()->isToolCompatible($item)) {
                if ($this->isAffectedBySilkTouch() && $item->hasEnchantment(VanillaEnchantments::SILK_TOUCH())) {
                    return $this->getSilkTouchDrops($item);
                }
                return $this->getDropsForCompatibleTool($item);
            }
            if ($item instanceof Item) {
                return $this->getDropsForIncompatibleTool($item);
            }
            $legacy = $this->getId();
            return $legacy === 0 ? [] : [\pocketmine\item\ItemFactory::get($legacy, 0, 1)];
        }

        public function getDropsForCompatibleTool(Item $item): array
        {
            return [$this->asItem()];
        }

        public function getDropsForIncompatibleTool(Item $item): array
        {
            return [];
        }

        public function getSilkTouchDrops(Item $item): array
        {
            return [$this->asItem()];
        }

        public function getXpDropForTool(Item $item): int
        {
            if ($item->hasEnchantment(VanillaEnchantments::SILK_TOUCH()) || !$this->getBreakInfo()->isToolCompatible($item)) {
                return 0;
            }
            return $this->getXpDropAmount();
        }

        protected function getXpDropAmount(): int
        {
            return 0;
        }

        public function isAffectedBySilkTouch(): bool
        {
            return false;
        }

        public function getPickedItem(bool $addUserData = false): Item
        {
            return $this->asItem();
        }

        public function getFuelTime(): int
        {
            return 0;
        }

        public function getMaxStackSize(): int
        {
            return 64;
        }

        public function isFireProofAsItem(): bool
        {
            return false;
        }

        // --- 6. Flammability ---

        public function getFlameEncouragement(): int
        {
            return 0;
        }

        public function getFlammability(): int
        {
            return 0;
        }

        public function burnsForever(): bool
        {
            return false;
        }

        public function isFlammable(): bool
        {
            return $this->getFlammability() > 0;
        }

        public function onIncinerate(): void
        {
        }

        // --- 7. Spatial & Neighbours ---

        public function getSide(int $side, int $step = 1)
        {
            $pos = $this->getPosition();
            if ($pos->isValid()) {
                [$dx, $dy, $dz] = Facing::OFFSET[$side] ?? [0, 0, 0];
                return $this->getWorld()->getBlockAt(
                    (int)($pos->x + ($dx * $step)),
                    (int)($pos->y + ($dy * $step)),
                    (int)($pos->z + ($dz * $step))
                );
            }
            throw new \LogicException("Block does not have a valid world");
        }

        public function getHorizontalSides(): \Generator
        {
            $world = $this->getWorld();
            $pos = $this->getPosition();
            foreach (Facing::HORIZONTAL as $facing) {
                [$dx, $dy, $dz] = Facing::OFFSET[$facing];
                yield $world->getBlockAt((int)($pos->x + $dx), (int)($pos->y + $dy), (int)($pos->z + $dz));
            }
        }

        public function getAllSides(): \Generator
        {
            $world = $this->getWorld();
            $pos = $this->getPosition();
            foreach (Facing::OFFSET as [$dx, $dy, $dz]) {
                yield $world->getBlockAt((int)($pos->x + $dx), (int)($pos->y + $dy), (int)($pos->z + $dz));
            }
        }

        public function getAffectedBlocks(): array
        {
            return [$this];
        }

        public function __toString(): string
        {
            return 'Block[' . $this->getName() . '] (' . $this->getTypeId() . ':' . $this->stateId . ')';
        }

        // --- 8. Collision & Intercept ---

        public function collidesWithBB(AxisAlignedBB $bb): bool
        {
            foreach ($this->getCollisionBoxes() as $bb2) {
                if ($bb->intersectsWith($bb2)) {
                    return true;
                }
            }
            return false;
        }

        public function hasEntityCollision(): bool
        {
            return false;
        }

        public function onEntityInside(Entity $entity): bool
        {
            return true;
        }

        public function addVelocityToEntity(Entity $entity): ?Vector3
        {
            return null;
        }

        public function onEntityLand(Entity $entity): ?float
        {
            return null;
        }

        public function onProjectileHit(Projectile $projectile, RayTraceResult $hitResult): void
        {
        }

        final public function getCollisionBoxes(): array
        {
            if ($this->collisionBoxes === null) {
                $this->collisionBoxes = $this->recalculateCollisionBoxes();
                $extraOffset = $this->getModelPositionOffset();
                $offset = $extraOffset !== null ? $this->getPosition()->addVector($extraOffset) : $this->getPosition();
                foreach ($this->collisionBoxes as $bb) {
                    $bb->offset($offset->x, $offset->y, $offset->z);
                }
            }
            return $this->collisionBoxes;
        }

        public function getModelPositionOffset(): ?Vector3
        {
            return null;
        }

        protected function recalculateCollisionBoxes(): array
        {
            if (!$this->isSolid()) {
                return [];
            }
            return [AxisAlignedBB::one()];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::FULL;
        }

        protected function getAdjacentSupportType(int $facing): SupportType
        {
            return $this->getSide($facing)->getSupportType(Facing::opposite($facing));
        }

        public function isFullCube(): bool
        {
            $bb = $this->getCollisionBoxes();
            return count($bb) === 1 && $bb[0]->getAverageEdgeLength() >= 1.0 && $bb[0]->isCube();
        }

        public function calculateIntercept(Vector3 $pos1, Vector3 $pos2): ?RayTraceResult
        {
            $bbs = $this->getCollisionBoxes();
            if (count($bbs) === 0) {
                return null;
            }
            $currentHit = null;
            $currentDistance = PHP_INT_MAX;
            foreach ($bbs as $bb) {
                $nextHit = $bb->calculateIntercept($pos1, $pos2);
                if ($nextHit === null) {
                    continue;
                }
                $nextDistance = $nextHit->hitVector->distanceSquared($pos1);
                if ($nextDistance < $currentDistance) {
                    $currentHit = $nextHit;
                    $currentDistance = $nextDistance;
                }
            }
            return $currentHit;
        }

        // --- Bridge helpers & compatibility ---

        public static function fromCoords(string $worldName, int $x, int $y, int $z): self
        {
            $stateId = self::AIR_STATE_ID;
            if (class_exists(Bridge::class) && Bridge::isActive()) {
                $result = Bridge::hostCall('world.get_block', [
                    'world' => $worldName,
                    'x' => $x,
                    'y' => $y,
                    'z' => $z,
                ]);
                if (is_array($result) && isset($result['id'])) {
                    $stateId = (int)$result['id'];
                }
            }
            return self::fromStateId($stateId, $worldName, $x, $y, $z);
        }

        public static function fromStateId(int $stateId, string $worldName = 'world', int $x = 0, int $y = 0, int $z = 0): self
        {
            $legacy = BlockFactory::legacyIdOf($stateId);
            $name = BlockFactory::nameOf($legacy);
            $class = self::classForName((string)$name);
            $block = new $class(
                new BlockIdentifier(BlockTypeIds::forName((string)$name)),
                BlockFactory::friendlyNameOf($legacy),
                new BlockTypeInfo(BlockBreakInfo::instant())
            );
            $block->worldName = $worldName;
            $block->x = $x;
            $block->y = $y;
            $block->z = $z;
            $block->stateId = $stateId;
            return $block;
        }

        public static function fromName(string $name, string $worldName = 'world', int $x = 0, int $y = 0, int $z = 0): self
        {
            $stateId = self::resolveStateId($name);
            return self::fromStateId($stateId, $worldName, $x, $y, $z);
        }

        public static function resolveStateId(string $name): int
        {
            $full = str_contains($name, ':') ? $name : 'minecraft:' . $name;
            if (class_exists(Bridge::class) && Bridge::isActive()) {
                $result = Bridge::hostCall('block.network_id', ['name' => $full]);
                if (is_array($result) && isset($result['id'])) {
                    return (int)$result['id'];
                }
            }
            $legacy = BlockFactory::idFromName($full);
            return $legacy === 0 ? self::AIR_STATE_ID : BlockFactory::networkIdOf($legacy);
        }

        private static function classForName(string $name): string
        {
            return self::class;
        }

        public function getIdentifier(): BlockIdentifier
        {
            return $this->idInfo;
        }

        public function getTypeInfo(): BlockTypeInfo
        {
            return $this->typeInfo;
        }

        private static function breakInfo(int $stateId): ?array
        {
            if (array_key_exists($stateId, self::$breakInfoCache)) {
                return self::$breakInfoCache[$stateId];
            }
            $info = null;
            if (class_exists(Bridge::class) && Bridge::isActive()) {
                $result = Bridge::hostCall('block.break_info', ['id' => $stateId]);
                if (is_array($result)) {
                    $info = $result;
                }
            }
            return self::$breakInfoCache[$stateId] = $info;
        }

        public static function stateInfo(int $stateId): ?array
        {
            if (array_key_exists($stateId, self::$stateCache)) {
                return self::$stateCache[$stateId];
            }
            $info = null;
            if (class_exists(Bridge::class) && Bridge::isActive()) {
                $result = Bridge::hostCall('block.state', ['id' => $stateId]);
                if (is_array($result) && isset($result['name'])) {
                    $info = $result;
                }
            }
            return self::$stateCache[$stateId] = $info;
        }

        public function getProperties(): array
        {
            $info = self::stateInfo($this->stateId);
            return is_array($info['properties'] ?? null) ? $info['properties'] : [];
        }

        public function hasProperty(string $name): bool
        {
            return array_key_exists($name, $this->getProperties());
        }

        public function getProperty(string $name): mixed
        {
            return $this->getProperties()[$name] ?? null;
        }

        public function getNetworkId(): int
        {
            return $this->stateId;
        }

        public function getId(): int
        {
            return BlockFactory::legacyIdOf($this->stateId);
        }

        public function getMeta(): int
        {
            $info = self::stateInfo($this->stateId);
            if (is_array($info) && isset($info['meta']) && is_int($info['meta'])) {
                return $info['meta'];
            }
            return 0;
        }

        public function getTypeName(): string
        {
            return BlockFactory::networkNameOf(BlockFactory::legacyIdOf($this->stateId));
        }

        public function getX(): int
        {
            return $this->x;
        }

        public function getY(): int
        {
            return $this->y;
        }

        public function getZ(): int
        {
            return $this->z;
        }

        public function getWorld(): World
        {
            return \pocketmine\Server::getInstance()->getWorldManager()->getWorldByName($this->worldName);
        }

        public function describeType(RuntimeDataDescriber $describer): void
        {
        }

        public function setTypeId(string $name): bool
        {
            $stateId = self::resolveStateId($name);
            $legacy = BlockFactory::legacyIdOf($stateId);
            $this->idInfo = new BlockIdentifier(BlockTypeIds::forName($name));
            $this->fallbackName = BlockFactory::friendlyNameOf($legacy);
            $this->stateId = $stateId;
            return $this->write();
        }

        public function setType(string $name): bool
        {
            return $this->setTypeId($name);
        }

        public function setBlock(int $legacyId, int $meta = 0): bool
        {
            $this->stateId = BlockFactory::networkIdOf($legacyId);
            $this->idInfo = new BlockIdentifier(BlockTypeIds::forName(BlockFactory::nameOf($legacyId)));
            $this->fallbackName = BlockFactory::friendlyNameOf($legacyId);
            return $this->write();
        }

        public function setStateId(int $stateId): bool
        {
            $info = self::stateInfo($stateId);
            if ($info === null && class_exists(Bridge::class) && Bridge::isActive()) {
                return false;
            }
            $legacy = BlockFactory::legacyIdOf($stateId);
            $this->stateId = $stateId;
            $this->fallbackName = BlockFactory::friendlyNameOf($legacy);
            $this->idInfo = new BlockIdentifier(BlockTypeIds::forName((string)($info['name'] ?? $this->fallbackName)));
            return $this->write();
        }

        private function write(): bool
        {
            if (!class_exists(Bridge::class) || !Bridge::isActive()) {
                return false;
            }
            $result = Bridge::hostCall('world.set_block', [
                'world' => $this->worldName,
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
                'block_id' => $this->stateId,
            ]);
            return is_array($result) && ($result['ok'] ?? false) === true;
        }
    }

    /** PM `Opaque`: a block with no transparency at all. */
    class Opaque extends Block
    {
        public function isTransparent(): bool
        {
            return false;
        }
    }

    /** PM `Transparent`: a block that allows light or sight through. */
    class Transparent extends Block
    {
        public function isTransparent(): bool
        {
            return true;
        }
    }

    /** PM `UnknownBlock`: sentinel block for unknown runtime states. */
    final class UnknownBlock extends Block
    {
        public function isSolid(): bool
        {
            return false;
        }

        public function isTransparent(): bool
        {
            return true;
        }
    }

    /** PM `RuntimeBlockStateRegistry` */
    final class RuntimeBlockStateRegistry
    {
        private static ?self $instance = null;

        /** @var array<int, Block> */
        private array $states = [];

        private function __construct()
        {
        }

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
        }

        public function register(Block $block): void
        {
            $stateId = $block->getStateId();
            if (isset($this->states[$stateId])) {
                throw new \InvalidArgumentException(
                    'State id ' . $stateId . ' is already registered by ' . $this->states[$stateId]->getName()
                );
            }
            $this->states[$stateId] = $block;
        }

        public function fromStateId(int $stateId): ?Block
        {
            $block = $this->states[$stateId] ?? null;
            return $block === null ? null : clone $block;
        }

        /** @return array<int, Block> */
        public function getAllStates(): array
        {
            return $this->states;
        }
    }

BLOCK_CLASS;

$tail = <<<'TAIL'
}

namespace pocketmine\world {

    if (!class_exists('pocketmine\\world\\Block', false)) {
        class_alias(\pocketmine\block\Block::class, 'pocketmine\\world\\Block');
    }
}

TAIL;

$full = $out . "\n" . $regParts . "\n" . $blockClass . "\n" . $tail;

file_put_contents(__DIR__ . '/../crates/php-plugin-bridge/php/block.php', $full);
echo "Written full block.php: " . strlen($full) . " bytes.\n";
