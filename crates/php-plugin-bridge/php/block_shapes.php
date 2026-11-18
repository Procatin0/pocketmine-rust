<?php

declare(strict_types=1);

namespace pocketmine\block {

    use pocketmine\block\utils\AnalogRedstoneSignalEmitter;
    use pocketmine\block\utils\AnalogRedstoneSignalEmitterTrait;
    use pocketmine\block\utils\AnyFacing;
    use pocketmine\block\utils\AnyFacingTrait;
    use pocketmine\block\utils\BlockEventHelper;
    use pocketmine\block\utils\Colored;
    use pocketmine\block\utils\ColoredTrait;
    use pocketmine\block\utils\CopperMaterial;
    use pocketmine\block\utils\CopperTrait;
    use pocketmine\block\utils\Fallable;
    use pocketmine\block\utils\FallableTrait;
    use pocketmine\block\utils\HorizontalConnectable;
    use pocketmine\block\utils\HorizontalConnectableTrait;
    use pocketmine\block\utils\HorizontalFacing;
    use pocketmine\block\utils\HorizontalFacingTrait;
    use pocketmine\block\utils\PillarRotation;
    use pocketmine\block\utils\PillarRotationTrait;
    use pocketmine\block\utils\PoweredByRedstone;
    use pocketmine\block\utils\RailConnectionInfo;
    use pocketmine\block\utils\RailPoweredByRedstoneTrait;
    use pocketmine\block\utils\RedstoneSignalEmitter;
    use pocketmine\block\utils\SlabType;
    use pocketmine\block\utils\StairShape;
    use pocketmine\block\utils\StaticSupportTrait;
    use pocketmine\block\utils\SupportType;
    use pocketmine\block\utils\WallConnectionType;
    use pocketmine\block\utils\WoodMaterial;
    use pocketmine\block\utils\WoodType;
    use pocketmine\block\utils\WoodTypeTrait;
    use pocketmine\data\runtime\RuntimeDataDescriber;
    use pocketmine\entity\Entity;
    use pocketmine\entity\Living;
    use pocketmine\item\Item;
    use pocketmine\math\Axis;
    use pocketmine\math\AxisAlignedBB;
    use pocketmine\math\Facing;
    use pocketmine\math\Vector3;
    use pocketmine\player\Player;
    use pocketmine\world\BlockTransaction;

    // Ensure BlockLegacyMetadata is available for RailConnectionInfo
    if (!class_exists('pocketmine\\data\\bedrock\\block\\BlockLegacyMetadata', false)) {
        eval('namespace pocketmine\\data\\bedrock\\block { final class BlockLegacyMetadata {
            public const RAIL_STRAIGHT_NORTH_SOUTH = 0;
            public const RAIL_STRAIGHT_EAST_WEST = 1;
            public const RAIL_ASCENDING_EAST = 2;
            public const RAIL_ASCENDING_WEST = 3;
            public const RAIL_ASCENDING_NORTH = 4;
            public const RAIL_ASCENDING_SOUTH = 5;
            public const RAIL_CURVE_SOUTHEAST = 6;
            public const RAIL_CURVE_SOUTHWEST = 7;
            public const RAIL_CURVE_NORTHWEST = 8;
            public const RAIL_CURVE_NORTHEAST = 9;
        } }');
    }

    // Ensure pocketmine\math\Position alias exists for FallableTrait compatibility
    if (!class_exists('pocketmine\\math\\Position', false) && class_exists('pocketmine\\world\\Position', false)) {
        class_alias('pocketmine\\world\\Position', 'pocketmine\\math\\Position');
    }

    /**
     * Trait providing compatibility marker for block shapes.
     */
    trait BlockShapeTrait
    {
    }

    /**
     * PM `Opaque`: a block with no transparency at all.
     */
    if (!class_exists(Opaque::class, false)) {
        class Opaque extends Block
        {
            use BlockShapeTrait;

            public function isTransparent(): bool
            {
                return false;
            }
        }
    }

    /**
     * PM `Transparent`: transparent blocks do not block light from propagating through them.
     */
    if (!class_exists(Transparent::class, false)) {
        class Transparent extends Block
        {
            use BlockShapeTrait;

            public function isTransparent(): bool
            {
                return true;
            }
        }
    }

    /**
     * PM `Flowable`: destroyed if water flows into the same space as the block.
     */
    abstract class Flowable extends Transparent
    {
        use BlockShapeTrait;

        public function canBeFlowedInto(): bool
        {
            return true;
        }

        public function isSolid(): bool
        {
            return false;
        }

        public function canBePlacedAt(Block $blockReplace, Vector3 $clickVector, int $face, bool $isClickedBlock): bool
        {
            return (!$this->canBeFlowedInto() || !($blockReplace instanceof \pocketmine\block\Liquid))
                && parent::canBePlacedAt($blockReplace, $clickVector, $face, $isClickedBlock);
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }
    }

    /**
     * PM `Thin`: thin blocks behave like glass panes / iron bars.
     */
    class Thin extends Transparent implements HorizontalConnectable
    {
        use BlockShapeTrait;
        use HorizontalConnectableTrait;

        protected function recalculateConnections(): bool
        {
            $changed = false;
            foreach (Facing::HORIZONTAL as $facing) {
                $side = $this->getSide($facing);
                $connected = $side instanceof Thin || $side instanceof Wall || $side->getSupportType(Facing::opposite($facing)) === SupportType::FULL;
                if ($connected !== $this->isConnectedAt($facing)) {
                    $this->setConnectedAt($facing, $connected);
                    $changed = true;
                }
            }
            return $changed;
        }

        protected function recalculateCollisionBoxes(): array
        {
            $inset = 7 / 16;
            $bbs = [];

            if (isset($this->connections[Facing::WEST]) || isset($this->connections[Facing::EAST])) {
                $bb = AxisAlignedBB::one()->squash(Axis::Z, $inset);
                if (!isset($this->connections[Facing::WEST])) {
                    $bb->trim(Facing::WEST, $inset);
                } elseif (!isset($this->connections[Facing::EAST])) {
                    $bb->trim(Facing::EAST, $inset);
                }
                $bbs[] = $bb;
            }

            if (isset($this->connections[Facing::NORTH]) || isset($this->connections[Facing::SOUTH])) {
                $bb = AxisAlignedBB::one()->squash(Axis::X, $inset);
                if (!isset($this->connections[Facing::NORTH])) {
                    $bb->trim(Facing::NORTH, $inset);
                } elseif (!isset($this->connections[Facing::SOUTH])) {
                    $bb->trim(Facing::SOUTH, $inset);
                }
                $bbs[] = $bb;
            }

            if (count($bbs) === 0) {
                return [
                    AxisAlignedBB::one()->contract($inset, 0, $inset)
                ];
            }

            return $bbs;
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function onNearbyBlockChange(): void
        {
            if ($this->recalculateConnections()) {
                $this->getWorld()->setBlock($this->getPosition(), $this);
            }
        }
    }

    /**
     * PM `Wall`: cobble/stone wall geometry.
     */
    class Wall extends Transparent
    {
        use BlockShapeTrait;

        /** @var WallConnectionType[] */
        protected array $connections = [];
        protected bool $post = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            if (method_exists($w, 'wallConnections')) {
                $w->wallConnections($this->connections);
            }
            if (method_exists($w, 'bool')) {
                $w->bool($this->post);
            }
        }

        /** @return WallConnectionType[] */
        public function getConnections(): array
        {
            return $this->connections;
        }

        public function getConnection(int $face): ?WallConnectionType
        {
            return $this->connections[$face] ?? null;
        }

        /** @param WallConnectionType[] $connections */
        public function setConnections(array $connections): self
        {
            $this->connections = $connections;
            return $this;
        }

        public function setConnection(int $face, ?WallConnectionType $type): self
        {
            if ($face !== Facing::NORTH && $face !== Facing::SOUTH && $face !== Facing::WEST && $face !== Facing::EAST) {
                throw new \InvalidArgumentException("Facing can only be north, east, south or west");
            }
            if ($type !== null) {
                $this->connections[$face] = $type;
            } else {
                unset($this->connections[$face]);
            }
            return $this;
        }

        public function isPost(): bool
        {
            return $this->post;
        }

        public function setPost(bool $post): self
        {
            $this->post = $post;
            return $this;
        }

        public function onNearbyBlockChange(): void
        {
            if ($this->recalculateConnections()) {
                $this->getWorld()->setBlock($this->getPosition(), $this);
            }
        }

        protected function recalculateConnections(): bool
        {
            $changed = 0;
            foreach (Facing::HORIZONTAL as $facing) {
                $block = $this->getSide($facing);
                if ($block instanceof static || $block instanceof FenceGate || $block instanceof Thin || $block->getSupportType(Facing::opposite($facing)) === SupportType::FULL) {
                    if (!isset($this->connections[$facing])) {
                        $this->connections[$facing] = WallConnectionType::SHORT;
                        $changed++;
                    }
                } elseif (isset($this->connections[$facing])) {
                    unset($this->connections[$facing]);
                    $changed++;
                }
            }

            $up = $this->getSide(Facing::UP)->getTypeId() !== BlockTypeIds::AIR;
            if ($up !== $this->post) {
                $this->post = $up;
                $changed++;
            }

            return $changed > 0;
        }

        protected function recalculateCollisionBoxes(): array
        {
            $north = isset($this->connections[Facing::NORTH]);
            $south = isset($this->connections[Facing::SOUTH]);
            $west = isset($this->connections[Facing::WEST]);
            $east = isset($this->connections[Facing::EAST]);

            $inset = 0.25;
            if (
                !$this->post &&
                (
                    ($north && $south && !$west && !$east) ||
                    (!$north && !$south && $west && $east)
                )
            ) {
                $inset = 0.3125;
            }

            return [
                AxisAlignedBB::one()
                    ->extend(Facing::UP, 0.5)
                    ->trim(Facing::NORTH, $north ? 0.0 : $inset)
                    ->trim(Facing::SOUTH, $south ? 0.0 : $inset)
                    ->trim(Facing::WEST, $west ? 0.0 : $inset)
                    ->trim(Facing::EAST, $east ? 0.0 : $inset)
            ];
        }

        public function getSupportType(int $facing): SupportType
        {
            return Facing::axis($facing) === Axis::Y ? SupportType::CENTER : SupportType::NONE;
        }
    }

    /**
     * PM `Stair`: stair block geometry with inner/outer corner calculations.
     */
    class Stair extends Transparent implements HorizontalFacing
    {
        use BlockShapeTrait;
        use HorizontalFacingTrait;

        protected bool $upsideDown = false;
        protected StairShape $shape = StairShape::STRAIGHT;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            if (method_exists($w, 'horizontalFacing')) {
                $w->horizontalFacing($this->facing);
            } else {
                $w->int(2, $this->facing);
            }
            if (method_exists($w, 'bool')) {
                $w->bool($this->upsideDown);
            }
            if (method_exists($w, 'enum')) {
                $w->enum($this->shape);
            }
        }

        private function recalculateShape(): StairShape
        {
            $clockwise = Facing::rotateY($this->facing, true);
            if (($backFacing = $this->getPossibleCornerFacing(false)) !== null) {
                return $backFacing === $clockwise ? StairShape::OUTER_RIGHT : StairShape::OUTER_LEFT;
            } elseif (($frontFacing = $this->getPossibleCornerFacing(true)) !== null) {
                return $frontFacing === $clockwise ? StairShape::INNER_RIGHT : StairShape::INNER_LEFT;
            }
            return StairShape::STRAIGHT;
        }

        public function onNearbyBlockChange(): void
        {
            $shape = $this->recalculateShape();
            if ($shape !== $this->shape) {
                $this->shape = $shape;
                $this->getWorld()->setBlock($this->getPosition(), $this);
            }
        }

        public function isUpsideDown(): bool
        {
            return $this->upsideDown;
        }

        public function setUpsideDown(bool $upsideDown): self
        {
            $this->upsideDown = $upsideDown;
            return $this;
        }

        public function getShape(): StairShape
        {
            return $this->shape;
        }

        public function setShape(StairShape $shape): self
        {
            $this->shape = $shape;
            return $this;
        }

        protected function recalculateCollisionBoxes(): array
        {
            $topStepFace = $this->upsideDown ? Facing::DOWN : Facing::UP;
            $bbs = [
                AxisAlignedBB::one()->trim($topStepFace, 0.5)
            ];

            $topStep = AxisAlignedBB::one()
                ->trim(Facing::opposite($topStepFace), 0.5)
                ->trim(Facing::opposite($this->facing), 0.5);

            if ($this->shape === StairShape::OUTER_LEFT || $this->shape === StairShape::OUTER_RIGHT) {
                $topStep->trim(Facing::rotateY($this->facing, $this->shape === StairShape::OUTER_LEFT), 0.5);
            } elseif ($this->shape === StairShape::INNER_LEFT || $this->shape === StairShape::INNER_RIGHT) {
                $bbs[] = AxisAlignedBB::one()
                    ->trim(Facing::opposite($topStepFace), 0.5)
                    ->trim($this->facing, 0.5)
                    ->trim(Facing::rotateY($this->facing, $this->shape === StairShape::INNER_LEFT), 0.5);
            }

            $bbs[] = $topStep;

            return $bbs;
        }

        public function getSupportType(int $facing): SupportType
        {
            if (
                $facing === Facing::UP && $this->upsideDown ||
                $facing === Facing::DOWN && !$this->upsideDown ||
                ($facing === $this->facing && $this->shape !== StairShape::OUTER_LEFT && $this->shape !== StairShape::OUTER_RIGHT) ||
                ($facing === Facing::rotateY($this->facing, false) && $this->shape === StairShape::INNER_LEFT) ||
                ($facing === Facing::rotateY($this->facing, true) && $this->shape === StairShape::INNER_RIGHT)
            ) {
                return SupportType::FULL;
            }
            return SupportType::NONE;
        }

        private function getPossibleCornerFacing(bool $oppositeFacing): ?int
        {
            $side = $this->getSide($oppositeFacing ? Facing::opposite($this->facing) : $this->facing);
            return (
                $side instanceof Stair &&
                $side->upsideDown === $this->upsideDown &&
                Facing::axis($side->facing) !== Facing::axis($this->facing)
            ) ? $side->facing : null;
        }

        public function place(mixed $tx, mixed $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, mixed $player = null): bool
        {
            if ($player !== null) {
                if (method_exists($player, 'getHorizontalFacing')) {
                    $this->facing = $player->getHorizontalFacing();
                } elseif (method_exists($player, 'getLocation')) {
                    $yaw = (float)$player->getLocation()->getYaw();
                    $rotation = ((int)floor(($yaw + 45.0) / 90.0)) & 3;
                    $this->facing = match ($rotation) {
                        0 => Facing::SOUTH,
                        1 => Facing::WEST,
                        2 => Facing::NORTH,
                        default => Facing::EAST,
                    };
                }
            }
            $this->upsideDown = (($clickVector->y > 0.5 && $face !== Facing::UP) || $face === Facing::DOWN);

            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }
    }

    /**
     * PM `Slab`: half-block and double slab geometry.
     */
    class Slab extends Transparent
    {
        use BlockShapeTrait;

        protected SlabType $slabType = SlabType::BOTTOM;

        public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo)
        {
            parent::__construct($idInfo, str_ends_with($name, " Slab") ? $name : ($name . " Slab"), $typeInfo);
        }

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            if (method_exists($w, 'enum')) {
                $w->enum($this->slabType);
            }
        }

        public function isTransparent(): bool
        {
            return $this->slabType !== SlabType::DOUBLE;
        }

        public function getSlabType(): SlabType
        {
            return $this->slabType;
        }

        public function setSlabType(SlabType $slabType): self
        {
            $this->slabType = $slabType;
            return $this;
        }

        public function canBePlacedAt(Block $blockReplace, Vector3 $clickVector, int $face, bool $isClickedBlock): bool
        {
            if (parent::canBePlacedAt($blockReplace, $clickVector, $face, $isClickedBlock)) {
                return true;
            }

            if ($blockReplace instanceof Slab && $blockReplace->slabType !== SlabType::DOUBLE && $blockReplace->hasSameTypeId($this)) {
                if ($blockReplace->slabType === SlabType::TOP) {
                    return $clickVector->y <= 0.5 || (!$isClickedBlock && $face === Facing::UP);
                } else {
                    return $clickVector->y >= 0.5 || (!$isClickedBlock && $face === Facing::DOWN);
                }
            }

            return false;
        }

        public function place(mixed $tx, mixed $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, mixed $player = null): bool
        {
            if ($blockReplace instanceof Slab && $blockReplace->slabType !== SlabType::DOUBLE && $blockReplace->hasSameTypeId($this) && (
                ($blockReplace->slabType === SlabType::TOP && ($clickVector->y <= 0.5 || $face === Facing::UP)) ||
                ($blockReplace->slabType === SlabType::BOTTOM && ($clickVector->y >= 0.5 || $face === Facing::DOWN))
            )) {
                $this->slabType = SlabType::DOUBLE;
            } else {
                $this->slabType = (($face !== Facing::UP && $clickVector->y > 0.5) || $face === Facing::DOWN) ? SlabType::TOP : SlabType::BOTTOM;
            }

            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        protected function recalculateCollisionBoxes(): array
        {
            if ($this->slabType === SlabType::DOUBLE) {
                return [AxisAlignedBB::one()];
            }
            return [AxisAlignedBB::one()->trim($this->slabType === SlabType::TOP ? Facing::DOWN : Facing::UP, 0.5)];
        }

        public function getSupportType(int $facing): SupportType
        {
            if ($this->slabType === SlabType::DOUBLE) {
                return SupportType::FULL;
            } elseif (($facing === Facing::UP && $this->slabType === SlabType::TOP) || ($facing === Facing::DOWN && $this->slabType === SlabType::BOTTOM)) {
                return SupportType::FULL;
            }
            return SupportType::NONE;
        }

        public function getDropsForCompatibleTool(Item $item): array
        {
            return [$this->asItem()->setCount($this->slabType === SlabType::DOUBLE ? 2 : 1)];
        }
    }

    /**
     * PM `Fence`: fence geometry with 1.5 collision height.
     */
    class Fence extends Transparent implements HorizontalConnectable
    {
        use BlockShapeTrait;
        use HorizontalConnectableTrait;

        public function getThickness(): float
        {
            return 0.25;
        }

        protected function recalculateConnections(): bool
        {
            $changed = false;
            foreach (Facing::HORIZONTAL as $facing) {
                $block = $this->getSide($facing);
                $connected = $block instanceof static || $block instanceof FenceGate || $block->getSupportType(Facing::opposite($facing)) === SupportType::FULL;
                if ($connected !== $this->isConnectedAt($facing)) {
                    $this->setConnectedAt($facing, $connected);
                    $changed = true;
                }
            }
            return $changed;
        }

        protected function recalculateCollisionBoxes(): array
        {
            $inset = 0.5 - $this->getThickness() / 2;
            $bbs = [];

            $connectWest = isset($this->connections[Facing::WEST]);
            $connectEast = isset($this->connections[Facing::EAST]);

            if ($connectWest || $connectEast) {
                $bbs[] = AxisAlignedBB::one()
                    ->squash(Axis::Z, $inset)
                    ->extend(Facing::UP, 0.5)
                    ->trim(Facing::WEST, $connectWest ? 0.0 : $inset)
                    ->trim(Facing::EAST, $connectEast ? 0.0 : $inset);
            }

            $connectNorth = isset($this->connections[Facing::NORTH]);
            $connectSouth = isset($this->connections[Facing::SOUTH]);

            if ($connectNorth || $connectSouth) {
                $bbs[] = AxisAlignedBB::one()
                    ->squash(Axis::X, $inset)
                    ->extend(Facing::UP, 0.5)
                    ->trim(Facing::NORTH, $connectNorth ? 0.0 : $inset)
                    ->trim(Facing::SOUTH, $connectSouth ? 0.0 : $inset);
            }

            if (count($bbs) === 0) {
                return [
                    AxisAlignedBB::one()
                        ->extend(Facing::UP, 0.5)
                        ->contract($inset, 0.0, $inset)
                ];
            }

            return $bbs;
        }

        public function getSupportType(int $facing): SupportType
        {
            return Facing::axis($facing) === Axis::Y ? SupportType::CENTER : SupportType::NONE;
        }

        public function onNearbyBlockChange(): void
        {
            if ($this->recalculateConnections()) {
                $this->getWorld()->setBlock($this->getPosition(), $this);
            }
        }
    }

    /**
     * PM `FenceGate`: fence gate geometry and interaction.
     */
    class FenceGate extends Transparent implements HorizontalFacing, WoodMaterial
    {
        use BlockShapeTrait;
        use WoodTypeTrait;
        use HorizontalFacingTrait;

        protected bool $open = false;
        protected bool $inWall = false;

        public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, ?WoodType $woodType = null)
        {
            $this->woodType = $woodType ?? WoodType::OAK;
            parent::__construct($idInfo, $name, $typeInfo);
        }

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            if (method_exists($w, 'horizontalFacing')) {
                $w->horizontalFacing($this->facing);
            } else {
                $w->int(2, $this->facing);
            }
            if (method_exists($w, 'bool')) {
                $w->bool($this->open);
                $w->bool($this->inWall);
            }
        }

        public function isOpen(): bool
        {
            return $this->open;
        }

        public function setOpen(bool $open): self
        {
            $this->open = $open;
            return $this;
        }

        public function isInWall(): bool
        {
            return $this->inWall;
        }

        public function setInWall(bool $inWall): self
        {
            $this->inWall = $inWall;
            return $this;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return $this->open ? [] : [
                AxisAlignedBB::one()->extend(Facing::UP, 0.5)->squash(Facing::axis($this->facing), 6 / 16)
            ];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        private function checkInWall(): bool
        {
            return (
                $this->getSide(Facing::rotateY($this->facing, false)) instanceof Wall ||
                $this->getSide(Facing::rotateY($this->facing, true)) instanceof Wall
            );
        }

        public function place(mixed $tx, mixed $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, mixed $player = null): bool
        {
            if ($player !== null) {
                if (method_exists($player, 'getHorizontalFacing')) {
                    $this->facing = $player->getHorizontalFacing();
                } elseif (method_exists($player, 'getLocation')) {
                    $yaw = (float)$player->getLocation()->getYaw();
                    $rotation = ((int)floor(($yaw + 45.0) / 90.0)) & 3;
                    $this->facing = match ($rotation) {
                        0 => Facing::SOUTH,
                        1 => Facing::WEST,
                        2 => Facing::NORTH,
                        default => Facing::EAST,
                    };
                }
            }

            $this->inWall = $this->checkInWall();

            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function onNearbyBlockChange(): void
        {
            $inWall = $this->checkInWall();
            if ($inWall !== $this->inWall) {
                $this->inWall = $inWall;
                $this->getWorld()->setBlock($this->getPosition(), $this);
            }
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            $this->open = !$this->open;
            if ($this->open && $player !== null && method_exists($player, 'getHorizontalFacing')) {
                $playerFacing = $player->getHorizontalFacing();
                if ($playerFacing === Facing::opposite($this->facing)) {
                    $this->facing = $playerFacing;
                }
            }

            $world = $this->getWorld();
            $world->setBlock($this->getPosition(), $this);
            if (class_exists(\pocketmine\world\sound\DoorSound::class)) {
                $world->addSound($this->getPosition(), new \pocketmine\world\sound\DoorSound());
            }
            return true;
        }

        public function getFuelTime(): int
        {
            return $this->woodType->isFlammable() ? 300 : 0;
        }

        public function getFlameEncouragement(): int
        {
            return $this->woodType->isFlammable() ? 5 : 0;
        }

        public function getFlammability(): int
        {
            return $this->woodType->isFlammable() ? 20 : 0;
        }
    }

    /**
     * PM `Carpet`: carpet thin block.
     */
    class Carpet extends Flowable implements Colored
    {
        use ColoredTrait;
        use StaticSupportTrait;

        public function isSolid(): bool
        {
            return true;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->trim(Facing::UP, 15 / 16)];
        }

        private function canBeSupportedAt(Block $block): bool
        {
            return $block->getSide(Facing::DOWN)->getTypeId() !== BlockTypeIds::AIR;
        }

        public function getFlameEncouragement(): int
        {
            return 30;
        }

        public function getFlammability(): int
        {
            return 20;
        }
    }

    /**
     * PM `SnowLayer`: layerable snow block.
     */
    class SnowLayer extends Flowable implements Fallable
    {
        use FallableTrait;

        public const MIN_LAYERS = 1;
        public const MAX_LAYERS = 8;

        protected int $layers = self::MIN_LAYERS;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            if (method_exists($w, 'boundedIntAuto')) {
                $w->boundedIntAuto(self::MIN_LAYERS, self::MAX_LAYERS, $this->layers);
            } else {
                $w->int(3, $this->layers);
            }
        }

        public function getLayers(): int
        {
            return $this->layers;
        }

        public function setLayers(int $layers): self
        {
            if ($layers < self::MIN_LAYERS || $layers > self::MAX_LAYERS) {
                throw new \InvalidArgumentException("Layers must be in range " . self::MIN_LAYERS . " ... " . self::MAX_LAYERS);
            }
            $this->layers = $layers;
            return $this;
        }

        public function canBeReplaced(): bool
        {
            return $this->layers < self::MAX_LAYERS;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->trim(Facing::UP, (self::MAX_LAYERS - $this->layers + 1) / 8)];
        }

        public function getSupportType(int $facing): SupportType
        {
            if (!$this->canBeReplaced()) {
                return SupportType::FULL;
            }
            return SupportType::NONE;
        }

        private function canBeSupportedAt(Block $block): bool
        {
            return $block->getAdjacentSupportType(Facing::DOWN) === SupportType::FULL;
        }

        public function place(mixed $tx, mixed $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, mixed $player = null): bool
        {
            if ($blockReplace instanceof SnowLayer) {
                if ($blockReplace->layers >= self::MAX_LAYERS) {
                    return false;
                }
                $this->layers = $blockReplace->layers + 1;
            }
            if ($this->canBeSupportedAt($blockReplace)) {
                return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
            }

            return false;
        }

        public function ticksRandomly(): bool
        {
            return true;
        }

        public function onRandomTick(): void
        {
            $world = $this->getWorld();
            if ($world !== null && method_exists($world, 'getBlockLightAt') && $world->getBlockLightAt($this->getX(), $this->getY(), $this->getZ()) >= 12) {
                if (class_exists(BlockEventHelper::class) && class_exists(VanillaBlocks::class)) {
                    BlockEventHelper::melt($this, VanillaBlocks::AIR());
                } else {
                    $world->setBlock($this->getPosition(), Block::fromName('air'));
                }
            }
        }

        public function getDropsForCompatibleTool(Item $item): array
        {
            if (class_exists(\pocketmine\item\VanillaItems::class)) {
                return [
                    \pocketmine\item\VanillaItems::SNOWBALL()->setCount(max(1, (int)floor($this->layers / 2)))
                ];
            }
            return [];
        }
    }

    /**
     * PM `Ladder`: climbable wall ladder.
     */
    class Ladder extends Transparent implements HorizontalFacing
    {
        use BlockShapeTrait;
        use HorizontalFacingTrait;

        public function hasEntityCollision(): bool
        {
            return true;
        }

        public function isSolid(): bool
        {
            return false;
        }

        public function canClimb(): bool
        {
            return true;
        }

        public function onEntityInside(Entity $entity): bool
        {
            if ($entity instanceof Living && $entity->getPosition()->floor()->distanceSquared($this->getPosition()) < 1) {
                if (method_exists($entity, 'resetFallDistance')) {
                    $entity->resetFallDistance();
                }
                $entity->onGround = true;
            }
            return true;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->trim($this->facing, 13 / 16)];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function place(mixed $tx, mixed $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, mixed $player = null): bool
        {
            if ($this->canBeSupportedAt($blockReplace, Facing::opposite($face)) && Facing::axis($face) !== Axis::Y) {
                $this->facing = $face;
                return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
            }

            return false;
        }

        public function onNearbyBlockChange(): void
        {
            if (!$this->canBeSupportedAt($this, Facing::opposite($this->facing))) {
                $world = $this->getWorld();
                if ($world !== null) {
                    if (method_exists($world, 'useBreakOn')) {
                        $world->useBreakOn($this->getPosition());
                    } else {
                        $world->setBlock($this->getPosition(), Block::fromName('air'));
                    }
                }
            }
        }

        private function canBeSupportedAt(Block $block, int $face): bool
        {
            return $block->getAdjacentSupportType($face) === SupportType::FULL;
        }
    }

    /**
     * PM `Chain`: hanging or horizontal iron chain.
     */
    class Chain extends Transparent implements PillarRotation
    {
        use BlockShapeTrait;
        use PillarRotationTrait;

        public function getSupportType(int $facing): SupportType
        {
            return $this->axis === Axis::Y && Facing::axis($facing) === Axis::Y ? SupportType::CENTER : SupportType::NONE;
        }

        protected function recalculateCollisionBoxes(): array
        {
            $bb = AxisAlignedBB::one();
            foreach ([Axis::Y, Axis::Z, Axis::X] as $axis) {
                if ($axis !== $this->axis) {
                    $bb->squash($axis, 13 / 32);
                }
            }
            return [$bb];
        }
    }

    /**
     * PM `SimplePillar`: pillar-like rotatable blocks.
     */
    class SimplePillar extends Opaque implements PillarRotation
    {
        use BlockShapeTrait;
        use PillarRotationTrait;

        public function place(mixed $tx, mixed $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, mixed $player = null): bool
        {
            $this->axis = Facing::axis($face);
            return true;
        }
    }

    /**
     * PM `BaseRail`: base class for minecart rails.
     */
    abstract class BaseRail extends Flowable
    {
        use BlockShapeTrait;

        public function place(mixed $tx, mixed $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, mixed $player = null): bool
        {
            if ($blockReplace->getAdjacentSupportType(Facing::DOWN)->hasEdgeSupport()) {
                return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
            }

            return false;
        }

        public function onPostPlace(): void
        {
            $this->tryReconnect();
        }

        /**
         * @param int[]   $connections
         * @param int[][] $lookup
         */
        protected static function searchState(array $connections, array $lookup): ?int
        {
            $shape = array_search($connections, $lookup, true);
            if ($shape === false) {
                $shape = array_search(array_reverse($connections), $lookup, true);
            }
            return $shape === false ? null : $shape;
        }

        abstract protected function setShapeFromConnections(array $connections): void;

        /**
         * @return int[]
         */
        abstract protected function getCurrentShapeConnections(): array;

        /**
         * @return int[]
         */
        private function getConnectedDirections(): array
        {
            $connections = [];
            foreach ($this->getCurrentShapeConnections() as $connection) {
                $other = $this->getSide($connection & ~RailConnectionInfo::FLAG_ASCEND);
                $otherConnection = Facing::opposite($connection & ~RailConnectionInfo::FLAG_ASCEND);

                if (($connection & RailConnectionInfo::FLAG_ASCEND) !== 0) {
                    $other = $other->getSide(Facing::UP);
                } elseif (!($other instanceof BaseRail)) {
                    $other = $other->getSide(Facing::DOWN);
                    $otherConnection |= RailConnectionInfo::FLAG_ASCEND;
                }

                if (
                    $other instanceof BaseRail &&
                    in_array($otherConnection, $other->getCurrentShapeConnections(), true)
                ) {
                    $connections[] = $connection;
                }
            }

            return $connections;
        }

        /**
         * @param int[] $constraints
         * @return true[]
         */
        private function getPossibleConnectionDirections(array $constraints): array
        {
            switch (count($constraints)) {
                case 0:
                    $possible = [
                        Facing::NORTH => true,
                        Facing::SOUTH => true,
                        Facing::WEST => true,
                        Facing::EAST => true
                    ];
                    foreach ($possible as $p => $_) {
                        $possible[$p | RailConnectionInfo::FLAG_ASCEND] = true;
                    }
                    return $possible;
                case 1:
                    return $this->getPossibleConnectionDirectionsOneConstraint(array_shift($constraints));
                case 2:
                    return [];
                default:
                    throw new \InvalidArgumentException("Expected at most 2 constraints, got " . count($constraints));
            }
        }

        /**
         * @return true[]
         */
        protected function getPossibleConnectionDirectionsOneConstraint(int $constraint): array
        {
            $opposite = Facing::opposite($constraint & ~RailConnectionInfo::FLAG_ASCEND);
            $possible = [$opposite => true];

            if (($constraint & RailConnectionInfo::FLAG_ASCEND) === 0) {
                $possible[$opposite | RailConnectionInfo::FLAG_ASCEND] = true;
            }

            return $possible;
        }

        private function tryReconnect(): void
        {
            $thisConnections = $this->getConnectedDirections();
            $changed = false;
            $world = $this->getWorld();

            do {
                $possible = $this->getPossibleConnectionDirections($thisConnections);
                $continue = false;

                foreach ($possible as $thisSide => $_) {
                    $otherSide = Facing::opposite($thisSide & ~RailConnectionInfo::FLAG_ASCEND);
                    $other = $this->getSide($thisSide & ~RailConnectionInfo::FLAG_ASCEND);

                    if (($thisSide & RailConnectionInfo::FLAG_ASCEND) !== 0) {
                        $other = $other->getSide(Facing::UP);
                    } elseif (!($other instanceof BaseRail)) {
                        $other = $other->getSide(Facing::DOWN);
                        $otherSide |= RailConnectionInfo::FLAG_ASCEND;
                    }

                    if (!($other instanceof BaseRail) || count($otherConnections = $other->getConnectedDirections()) >= 2) {
                        continue;
                    }

                    $otherPossible = $other->getPossibleConnectionDirections($otherConnections);

                    if (isset($otherPossible[$otherSide])) {
                        $otherConnections[] = $otherSide;
                        $other->setConnections($otherConnections);
                        if ($world !== null) {
                            $world->setBlock($other->getPosition(), $other);
                        }

                        $changed = true;
                        $thisConnections[] = $thisSide;
                        $continue = count($thisConnections) < 2;
                        break;
                    }
                }
            } while ($continue);

            if ($changed && $world !== null) {
                $this->setConnections($thisConnections);
                $world->setBlock($this->getPosition(), $this);
            }
        }

        /**
         * @param int[] $connections
         */
        private function setConnections(array $connections): void
        {
            if (count($connections) === 1) {
                $connections[] = Facing::opposite($connections[0] & ~RailConnectionInfo::FLAG_ASCEND);
            } elseif (count($connections) !== 2) {
                throw new \InvalidArgumentException("Expected exactly 2 connections, got " . count($connections));
            }

            $this->setShapeFromConnections($connections);
        }

        public function onNearbyBlockChange(): void
        {
            $world = $this->getWorld();
            if ($world === null) {
                return;
            }
            if (!$this->getAdjacentSupportType(Facing::DOWN)->hasEdgeSupport()) {
                if (method_exists($world, 'useBreakOn')) {
                    $world->useBreakOn($this->getPosition());
                } else {
                    $world->setBlock($this->getPosition(), Block::fromName('air'));
                }
            } else {
                foreach ($this->getCurrentShapeConnections() as $connection) {
                    if (($connection & RailConnectionInfo::FLAG_ASCEND) !== 0 && !$this->getSide($connection & ~RailConnectionInfo::FLAG_ASCEND)->getSupportType(Facing::UP)->hasEdgeSupport()) {
                        if (method_exists($world, 'useBreakOn')) {
                            $world->useBreakOn($this->getPosition());
                        } else {
                            $world->setBlock($this->getPosition(), Block::fromName('air'));
                        }
                        break;
                    }
                }
            }
        }
    }

    /**
     * PM `StraightOnlyRail`: non-curving rails (powered, activator, detector).
     */
    class StraightOnlyRail extends BaseRail
    {
        private int $railShape = 0;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            if (method_exists($w, 'straightOnlyRailShape')) {
                $w->straightOnlyRailShape($this->railShape);
            } else {
                $w->int(4, $this->railShape);
            }
        }

        protected function setShapeFromConnections(array $connections): void
        {
            $railShape = self::searchState($connections, RailConnectionInfo::CONNECTIONS);
            if ($railShape === null) {
                throw new \InvalidArgumentException("No rail shape matches these connections");
            }
            $this->railShape = $railShape;
        }

        protected function getCurrentShapeConnections(): array
        {
            return RailConnectionInfo::CONNECTIONS[$this->railShape];
        }

        public function getShape(): int
        {
            return $this->railShape;
        }

        public function setShape(int $shape): self
        {
            if (!isset(RailConnectionInfo::CONNECTIONS[$shape])) {
                throw new \InvalidArgumentException("Invalid rail shape, must be one of " . implode(", ", array_keys(RailConnectionInfo::CONNECTIONS)));
            }
            $this->railShape = $shape;
            return $this;
        }
    }

    /**
     * PM `Rail`: standard curving and ascending rail.
     */
    class Rail extends BaseRail
    {
        private int $railShape = 0;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            if (method_exists($w, 'railShape')) {
                $w->railShape($this->railShape);
            } else {
                $w->int(4, $this->railShape);
            }
        }

        protected function setShapeFromConnections(array $connections): void
        {
            $railShape = self::searchState($connections, RailConnectionInfo::CONNECTIONS) ?? self::searchState($connections, RailConnectionInfo::CURVE_CONNECTIONS);
            if ($railShape === null) {
                throw new \InvalidArgumentException("No rail shape matches these connections");
            }
            $this->railShape = $railShape;
        }

        protected function getCurrentShapeConnections(): array
        {
            return RailConnectionInfo::CURVE_CONNECTIONS[$this->railShape] ?? RailConnectionInfo::CONNECTIONS[$this->railShape];
        }

        protected function getPossibleConnectionDirectionsOneConstraint(int $constraint): array
        {
            $possible = parent::getPossibleConnectionDirectionsOneConstraint($constraint);

            if (($constraint & RailConnectionInfo::FLAG_ASCEND) === 0) {
                foreach ([
                    Facing::NORTH,
                    Facing::SOUTH,
                    Facing::WEST,
                    Facing::EAST
                ] as $d) {
                    if ($constraint !== $d) {
                        $possible[$d] = true;
                    }
                }
            }

            return $possible;
        }

        public function getShape(): int
        {
            return $this->railShape;
        }

        public function setShape(int $shape): self
        {
            if (!isset(RailConnectionInfo::CONNECTIONS[$shape]) && !isset(RailConnectionInfo::CURVE_CONNECTIONS[$shape])) {
                throw new \InvalidArgumentException("Invalid shape, must be one of " . implode(", ", [...array_keys(RailConnectionInfo::CONNECTIONS), ...array_keys(RailConnectionInfo::CURVE_CONNECTIONS)]));
            }
            $this->railShape = $shape;
            return $this;
        }
    }

    /**
     * PM `NormalRail`: alias / subclass of standard rail.
     */
    class NormalRail extends Rail
    {
    }

    /**
     * PM `DetectorRail`: detector rail emitting redstone signal.
     */
    class DetectorRail extends StraightOnlyRail implements RedstoneSignalEmitter
    {
        protected bool $activated = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            parent::describeBlockOnlyState($w);
            if (method_exists($w, 'bool')) {
                $w->bool($this->activated);
            }
        }

        public function isActivated(): bool
        {
            return $this->activated;
        }

        public function setActivated(bool $activated): self
        {
            $this->activated = $activated;
            return $this;
        }
    }

    /**
     * PM `PoweredRail`: golden powered booster rail.
     */
    class PoweredRail extends StraightOnlyRail implements PoweredByRedstone
    {
        use RailPoweredByRedstoneTrait;
    }

    /**
     * PM `ActivatorRail`: activator rail.
     */
    class ActivatorRail extends StraightOnlyRail implements PoweredByRedstone
    {
        use RailPoweredByRedstoneTrait;
    }

    /**
     * PM `IronBars`: thin metal bars.
     */
    class IronBars extends Thin
    {
    }

    /**
     * PM `GlassPane`: regular clear glass pane.
     */
    class GlassPane extends Thin
    {
        public function getDropsForCompatibleTool(Item $item): array
        {
            return [];
        }

        public function isAffectedBySilkTouch(): bool
        {
            return true;
        }
    }

    /**
     * PM `Glass`: regular clear glass full cube.
     */
    class Glass extends Transparent
    {
        use BlockShapeTrait;

        public function getDropsForCompatibleTool(Item $item): array
        {
            return [];
        }

        public function isAffectedBySilkTouch(): bool
        {
            return true;
        }
    }

    /**
     * PM `StainedGlass`: colored glass block.
     */
    final class StainedGlass extends Glass implements Colored
    {
        use ColoredTrait;
    }

    /**
     * PM `StainedGlassPane`: colored glass pane.
     */
    final class StainedGlassPane extends GlassPane implements Colored
    {
        use ColoredTrait;
    }

    /**
     * PM `HardenedGlass`: chemically hardened glass.
     */
    class HardenedGlass extends Transparent
    {
        use BlockShapeTrait;
    }

    /**
     * PM `HardenedGlassPane`: hardened glass pane.
     */
    class HardenedGlassPane extends Thin
    {
    }

    /**
     * PM `StainedHardenedGlass`: stained hardened glass block.
     */
    final class StainedHardenedGlass extends HardenedGlass implements Colored
    {
        use ColoredTrait;
    }

    /**
     * PM `StainedHardenedGlassPane`: stained hardened glass pane.
     */
    final class StainedHardenedGlassPane extends HardenedGlassPane implements Colored
    {
        use ColoredTrait;
    }

    /**
     * PM `TintedGlass`: light-blocking transparent glass.
     */
    final class TintedGlass extends Transparent
    {
        use BlockShapeTrait;

        public function getLightFilter(): int
        {
            return 15;
        }
    }

    /**
     * PM `Wood`: log/wood pillar with bark on all sides and stripping interaction.
     */
    class Wood extends Opaque implements PillarRotation, WoodMaterial
    {
        use BlockShapeTrait;
        use PillarRotationTrait;
        use WoodTypeTrait;

        private bool $stripped = false;

        public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, ?WoodType $woodType = null)
        {
            $this->woodType = $woodType ?? WoodType::OAK;
            parent::__construct($idInfo, $name, $typeInfo);
        }

        public function describeBlockItemState(RuntimeDataDescriber $w): void
        {
            if (method_exists($w, 'bool')) {
                $w->bool($this->stripped);
            }
        }

        public function isStripped(): bool
        {
            return $this->stripped;
        }

        public function setStripped(bool $stripped): self
        {
            $this->stripped = $stripped;
            return $this;
        }

        public function getFuelTime(): int
        {
            return $this->woodType->isFlammable() ? 300 : 0;
        }

        public function getFlameEncouragement(): int
        {
            return $this->woodType->isFlammable() ? 5 : 0;
        }

        public function getFlammability(): int
        {
            return $this->woodType->isFlammable() ? 5 : 0;
        }

        public function place(mixed $tx, mixed $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, mixed $player = null): bool
        {
            $this->axis = Facing::axis($face);
            return true;
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if (!$this->stripped && (str_contains(strtolower($item->getName()), 'axe') || $item instanceof \pocketmine\item\Axe)) {
                if (method_exists($item, 'applyDamage')) {
                    $item->applyDamage(1);
                }
                $this->stripped = true;
                $world = $this->getWorld();
                if ($world !== null) {
                    $world->setBlock($this->getPosition(), $this);
                    if (class_exists(\pocketmine\world\sound\ItemUseOnBlockSound::class)) {
                        $world->addSound($this->getPosition(), new \pocketmine\world\sound\ItemUseOnBlockSound($this));
                    }
                }
                return true;
            }
            return false;
        }
    }

    /**
     * PM `Planks`: wooden planks block.
     */
    class Planks extends Opaque implements WoodMaterial
    {
        use BlockShapeTrait;
        use WoodTypeTrait;

        public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, ?WoodType $woodType = null)
        {
            $this->woodType = $woodType ?? WoodType::OAK;
            parent::__construct($idInfo, $name, $typeInfo);
        }

        public function getFuelTime(): int
        {
            return $this->woodType->isFlammable() ? 300 : 0;
        }

        public function getFlameEncouragement(): int
        {
            return $this->woodType->isFlammable() ? 5 : 0;
        }

        public function getFlammability(): int
        {
            return $this->woodType->isFlammable() ? 20 : 0;
        }
    }

    /**
     * PM `WoodenSlab`: wooden slab.
     */
    class WoodenSlab extends Slab implements WoodMaterial
    {
        use WoodTypeTrait;

        public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, ?WoodType $woodType = null)
        {
            $this->woodType = $woodType ?? WoodType::OAK;
            parent::__construct($idInfo, $name, $typeInfo);
        }

        public function getFuelTime(): int
        {
            return $this->woodType->isFlammable() ? 300 : 0;
        }

        public function getFlameEncouragement(): int
        {
            return $this->woodType->isFlammable() ? 5 : 0;
        }

        public function getFlammability(): int
        {
            return $this->woodType->isFlammable() ? 20 : 0;
        }
    }

    /**
     * PM `WoodenStairs`: wooden stairs.
     */
    class WoodenStairs extends Stair implements WoodMaterial
    {
        use WoodTypeTrait;

        public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, ?WoodType $woodType = null)
        {
            $this->woodType = $woodType ?? WoodType::OAK;
            parent::__construct($idInfo, $name, $typeInfo);
        }

        public function getFuelTime(): int
        {
            return $this->woodType->isFlammable() ? 300 : 0;
        }

        public function getFlameEncouragement(): int
        {
            return 5;
        }

        public function getFlammability(): int
        {
            return 20;
        }
    }

    /**
     * PM `WoodenFence`: wooden fence.
     */
    class WoodenFence extends Fence implements WoodMaterial
    {
        use WoodTypeTrait;

        public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, ?WoodType $woodType = null)
        {
            $this->woodType = $woodType ?? WoodType::OAK;
            parent::__construct($idInfo, $name, $typeInfo);
        }

        public function getFuelTime(): int
        {
            return $this->woodType->isFlammable() ? 300 : 0;
        }

        public function getFlameEncouragement(): int
        {
            return $this->woodType->isFlammable() ? 5 : 0;
        }

        public function getFlammability(): int
        {
            return $this->woodType->isFlammable() ? 20 : 0;
        }
    }

    /**
     * PM `PressurePlate`: base class for pressure plates.
     */
    abstract class PressurePlate extends Transparent implements RedstoneSignalEmitter
    {
        use BlockShapeTrait;
        use StaticSupportTrait;

        private readonly int $deactivationDelayTicks;

        public function __construct(
            BlockIdentifier $idInfo,
            string $name,
            BlockTypeInfo $typeInfo,
            int $deactivationDelayTicks = 20
        ) {
            parent::__construct($idInfo, $name, $typeInfo);
            $this->deactivationDelayTicks = $deactivationDelayTicks;
        }

        public function isSolid(): bool
        {
            return false;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        private function canBeSupportedAt(Block $block): bool
        {
            return $block->getAdjacentSupportType(Facing::DOWN) !== SupportType::NONE;
        }

        public function hasEntityCollision(): bool
        {
            return true;
        }

        public function onEntityInside(Entity $entity): bool
        {
            if (!$this->hasOutputSignal()) {
                $world = $this->getWorld();
                if ($world !== null && method_exists($world, 'scheduleDelayedBlockUpdate')) {
                    $world->scheduleDelayedBlockUpdate($this->getPosition(), 0);
                }
            }
            return true;
        }

        protected function getActivationBox(): AxisAlignedBB
        {
            return AxisAlignedBB::one()
                ->squash(Axis::X, 1 / 8)
                ->squash(Axis::Z, 1 / 8)
                ->trim(Facing::UP, 3 / 4)
                ->offset((float)$this->getX(), (float)$this->getY(), (float)$this->getZ());
        }

        protected function hasOutputSignal(): bool
        {
            return false;
        }

        /**
         * @param Entity[] $entities
         * @return mixed[]
         */
        protected function calculatePlateState(array $entities): array
        {
            return [$this, null];
        }

        /**
         * @param Entity[] $entities
         * @return Entity[]
         */
        protected function filterIrrelevantEntities(array $entities): array
        {
            return $entities;
        }

        public function onScheduledUpdate(): void
        {
            $world = $this->getWorld();
            if ($world === null) {
                return;
            }

            $intersectionAABB = $this->getActivationBox();
            $nearby = method_exists($world, 'getNearbyEntities') ? $world->getNearbyEntities($intersectionAABB) : [];
            $activatingEntities = $this->filterIrrelevantEntities($nearby);

            if (count($activatingEntities) > 0 || $this->hasOutputSignal()) {
                [$newState, $pressedChange] = $this->calculatePlateState($activatingEntities);

                if (class_exists(\pocketmine\event\block\PressurePlateUpdateEvent::class) && \pocketmine\event\block\PressurePlateUpdateEvent::hasHandlers()) {
                    $ev = new \pocketmine\event\block\PressurePlateUpdateEvent($this, $newState, $activatingEntities);
                    $ev->call();
                    $newState = $ev->isCancelled() ? null : $ev->getNewState();
                }

                if ($newState !== null) {
                    $world->setBlock($this->getPosition(), $newState);
                    if ($pressedChange !== null) {
                        if ($pressedChange && class_exists(\pocketmine\world\sound\PressurePlateActivateSound::class)) {
                            $world->addSound($this->getPosition(), new \pocketmine\world\sound\PressurePlateActivateSound($this));
                        } elseif (!$pressedChange && class_exists(\pocketmine\world\sound\PressurePlateDeactivateSound::class)) {
                            $world->addSound($this->getPosition(), new \pocketmine\world\sound\PressurePlateDeactivateSound($this));
                        }
                    }
                }

                if (($pressedChange ?? $this->hasOutputSignal()) && method_exists($world, 'scheduleDelayedBlockUpdate')) {
                    $world->scheduleDelayedBlockUpdate($this->getPosition(), $this->deactivationDelayTicks);
                }
            }
        }
    }

    /**
     * PM `SimplePressurePlate`: binary pressure plate (pressed or unpressed).
     */
    abstract class SimplePressurePlate extends PressurePlate
    {
        protected bool $pressed = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            if (method_exists($w, 'bool')) {
                $w->bool($this->pressed);
            }
        }

        public function isPressed(): bool
        {
            return $this->pressed;
        }

        public function setPressed(bool $pressed): self
        {
            $this->pressed = $pressed;
            return $this;
        }

        protected function hasOutputSignal(): bool
        {
            return $this->pressed;
        }

        protected function calculatePlateState(array $entities): array
        {
            $newPressed = count($entities) > 0;
            if ($newPressed === $this->pressed) {
                return [$this, null];
            }
            return [
                (clone $this)->setPressed($newPressed),
                $newPressed
            ];
        }
    }

    /**
     * PM `StonePressurePlate`: stone pressure plate only triggered by living entities.
     */
    class StonePressurePlate extends SimplePressurePlate
    {
        protected function filterIrrelevantEntities(array $entities): array
        {
            return array_filter($entities, fn(Entity $e) => $e instanceof Living);
        }
    }

    /**
     * PM `WoodenPressurePlate`: wooden pressure plate triggered by all entities.
     */
    class WoodenPressurePlate extends SimplePressurePlate implements WoodMaterial
    {
        use WoodTypeTrait;

        public function __construct(
            BlockIdentifier $idInfo,
            string $name,
            BlockTypeInfo $typeInfo,
            ?WoodType $woodType = null,
            int $deactivationDelayTicks = 20
        ) {
            $this->woodType = $woodType ?? WoodType::OAK;
            parent::__construct($idInfo, $name, $typeInfo, $deactivationDelayTicks);
        }

        public function getFuelTime(): int
        {
            return 300;
        }
    }

    /**
     * PM `WeightedPressurePlate`: weighted analog pressure plate.
     */
    class WeightedPressurePlate extends PressurePlate implements AnalogRedstoneSignalEmitter
    {
        use AnalogRedstoneSignalEmitterTrait;

        private readonly float $signalStrengthFactor;

        public function __construct(
            BlockIdentifier $idInfo,
            string $name,
            BlockTypeInfo $typeInfo,
            int $deactivationDelayTicks = 20,
            float $signalStrengthFactor = 1.0
        ) {
            parent::__construct($idInfo, $name, $typeInfo, $deactivationDelayTicks);
            $this->signalStrengthFactor = $signalStrengthFactor;
        }

        protected function hasOutputSignal(): bool
        {
            return $this->signalStrength > 0;
        }

        protected function calculatePlateState(array $entities): array
        {
            $newSignalStrength = min(15, max(0, (int)ceil(count($entities) * $this->signalStrengthFactor)));
            if ($newSignalStrength === $this->signalStrength) {
                return [$this, null];
            }
            $wasActive = $this->signalStrength !== 0;
            $isActive = $newSignalStrength !== 0;
            return [
                (clone $this)->setOutputSignalStrength($newSignalStrength),
                $wasActive !== $isActive ? $isActive : null
            ];
        }
    }

    /**
     * PM `WeightedPressurePlateHeavy`: iron weighted pressure plate.
     */
    class WeightedPressurePlateHeavy extends WeightedPressurePlate
    {
        public function __construct(
            BlockIdentifier $idInfo,
            string $name,
            BlockTypeInfo $typeInfo,
            int $deactivationDelayTicks = 20,
            float $signalStrengthFactor = 0.1
        ) {
            parent::__construct($idInfo, $name, $typeInfo, $deactivationDelayTicks, $signalStrengthFactor);
        }
    }

    /**
     * PM `WeightedPressurePlateLight`: gold weighted pressure plate.
     */
    class WeightedPressurePlateLight extends WeightedPressurePlate
    {
        public function __construct(
            BlockIdentifier $idInfo,
            string $name,
            BlockTypeInfo $typeInfo,
            int $deactivationDelayTicks = 20,
            float $signalStrengthFactor = 1.0
        ) {
            parent::__construct($idInfo, $name, $typeInfo, $deactivationDelayTicks, $signalStrengthFactor);
        }
    }

    /**
     * PM `CopperSlab`: copper slab.
     */
    class CopperSlab extends Slab implements CopperMaterial
    {
        use CopperTrait;
    }

    /**
     * PM `CopperStairs`: copper stairs.
     */
    class CopperStairs extends Stair implements CopperMaterial
    {
        use CopperTrait;
    }

    /**
     * PM `CopperBars`: copper grate bars.
     */
    class CopperBars extends Thin implements CopperMaterial
    {
        use CopperTrait;
    }

    /**
     * PM `CopperChain`: copper chain.
     */
    class CopperChain extends Chain implements CopperMaterial
    {
        use CopperTrait;
    }

    /**
     * PM `CopperGrate`: copper grate block.
     */
    class CopperGrate extends Transparent implements CopperMaterial
    {
        use BlockShapeTrait;
        use CopperTrait;
    }

    /**
     * PM `EndRod`: end rod illumination and thin geometry.
     */
    class EndRod extends Flowable implements AnyFacing
    {
        use AnyFacingTrait;

        public function place(mixed $tx, mixed $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, mixed $player = null): bool
        {
            $this->facing = $face;
            if ($blockClicked instanceof EndRod && $blockClicked->facing === $this->facing) {
                $this->facing = Facing::opposite($face);
            }

            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function isSolid(): bool
        {
            return true;
        }

        public function getLightLevel(): int
        {
            return 14;
        }

        protected function recalculateCollisionBoxes(): array
        {
            $myAxis = Facing::axis($this->facing);
            $bb = AxisAlignedBB::one();
            foreach ([Axis::Y, Axis::Z, Axis::X] as $axis) {
                if ($axis === $myAxis) {
                    continue;
                }
                $bb->squash($axis, 6 / 16);
            }
            return [$bb];
        }
    }

    /**
     * PM `LightningRod`: lightning rod with redstone emission.
     */
    final class LightningRod extends Transparent implements AnyFacing, CopperMaterial, RedstoneSignalEmitter
    {
        use BlockShapeTrait;
        use CopperTrait;
        use AnyFacingTrait;

        protected function recalculateCollisionBoxes(): array
        {
            $myAxis = Facing::axis($this->facing);
            $result = AxisAlignedBB::one();
            foreach ([Axis::X, Axis::Y, Axis::Z] as $axis) {
                if ($axis !== $myAxis) {
                    $result->squash($axis, 6 / 16);
                }
            }

            return [$result];
        }

        public function place(mixed $tx, mixed $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, mixed $player = null): bool
        {
            $this->facing = $face;
            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }
    }

    /**
     * PM `HeavyCore`: heavy core block.
     */
    final class HeavyCore extends Transparent
    {
        use BlockShapeTrait;

        protected function recalculateCollisionBoxes(): array
        {
            return [
                AxisAlignedBB::one()
                    ->contract(0.25, 0.0, 0.25)
                    ->trim(Facing::UP, 0.5)
            ];
        }

        public function getDropsForIncompatibleTool(mixed $item): array
        {
            return [$this->asItem()];
        }
    }

    // =========================================================================
    // Vanilla-specific wall subclasses for compatibility
    // =========================================================================
    class CobblestoneWall extends Wall {}
    class AndesiteWall extends Wall {}
    class DioriteWall extends Wall {}
    class GraniteWall extends Wall {}
    class SandstoneWall extends Wall {}
    class RedSandstoneWall extends Wall {}
    class StoneBrickWall extends Wall {}
    class MossyStoneBrickWall extends Wall {}
    class BrickWall extends Wall {}
    class PrismarineWall extends Wall {}
    class NetherBrickWall extends Wall {}
    class RedNetherBrickWall extends Wall {}
    class EndStoneBrickWall extends Wall {}
    class BlackstoneWall extends Wall {}
    class PolishedBlackstoneWall extends Wall {}
    class PolishedBlackstoneBrickWall extends Wall {}
    class MudBrickWall extends Wall {}
    class CobbledDeepslateWall extends Wall {}
    class PolishedDeepslateWall extends Wall {}
    class DeepslateBrickWall extends Wall {}
    class DeepslateTileWall extends Wall {}

    // =========================================================================
    // Vanilla-specific stair subclasses for compatibility
    // =========================================================================
    class CobblestoneStairs extends Stair {}
    class StoneStairs extends Stair {}
    class SandstoneStairs extends Stair {}
    class BrickStairs extends Stair {}
    class NetherBrickStairs extends Stair {}
    class QuartzStairs extends Stair {}
    class PurpurStairs extends Stair {}
    class PrismarineStairs extends Stair {}
    class StoneBrickStairs extends Stair {}
    class AndesiteStairs extends Stair {}
    class DioriteStairs extends Stair {}
    class GraniteStairs extends Stair {}
    class PolishedAndesiteStairs extends Stair {}
    class PolishedDioriteStairs extends Stair {}
    class PolishedGraniteStairs extends Stair {}
    class MossyCobblestoneStairs extends Stair {}
    class MossyStoneBrickStairs extends Stair {}
    class EndStoneBrickStairs extends Stair {}
    class RedNetherBrickStairs extends Stair {}
    class DarkPrismarineStairs extends Stair {}
    class PrismarineBrickStairs extends Stair {}

    // =========================================================================
    // Vanilla-specific slab subclasses for compatibility
    // =========================================================================
    class StoneSlab extends Slab {}
    class CobblestoneSlab extends Slab {}
    class SandstoneSlab extends Slab {}
    class BrickSlab extends Slab {}
    class NetherBrickSlab extends Slab {}
    class QuartzSlab extends Slab {}
    class PurpurSlab extends Slab {}
    class PrismarineSlab extends Slab {}
    class StoneBrickSlab extends Slab {}
    class AndesiteSlab extends Slab {}
    class DioriteSlab extends Slab {}
    class GraniteSlab extends Slab {}
    class PolishedAndesiteSlab extends Slab {}
    class PolishedDioriteSlab extends Slab {}
    class PolishedGraniteSlab extends Slab {}
    class MossyCobblestoneSlab extends Slab {}
    class MossyStoneBrickSlab extends Slab {}
    class EndStoneBrickSlab extends Slab {}
    class SmoothStoneSlab extends Slab {}
    class RedSandstoneSlab extends Slab {}
    class CutSandstoneSlab extends Slab {}
    class CutRedSandstoneSlab extends Slab {}

    // =========================================================================
    // Vanilla-specific fence subclasses for compatibility
    // =========================================================================
    class NetherBrickFence extends Fence {}
}
