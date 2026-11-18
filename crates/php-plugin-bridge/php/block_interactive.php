<?php

declare(strict_types=1);

namespace pocketmine\block {

    use pocketmine\block\inventory\AnvilInventory;
    use pocketmine\block\inventory\BarrelInventory;
    use pocketmine\block\inventory\BrewingStandInventory;
    use pocketmine\block\inventory\CampfireInventory;
    use pocketmine\block\inventory\ChestInventory;
    use pocketmine\block\inventory\DoubleChestInventory;
    use pocketmine\block\inventory\EnchantInventory;
    use pocketmine\block\inventory\EnderChestInventory;
    use pocketmine\block\inventory\FurnaceInventory;
    use pocketmine\block\inventory\HopperInventory;
    use pocketmine\block\inventory\ShulkerBoxInventory;
    use pocketmine\block\utils\AnalogRedstoneSignalEmitter;
    use pocketmine\block\utils\AnalogRedstoneSignalEmitterTrait;
    use pocketmine\block\utils\AnyFacing;
    use pocketmine\block\utils\AnyFacingTrait;
    use pocketmine\block\utils\BellAttachmentType;
    use pocketmine\block\utils\BrewingStandSlot;
    use pocketmine\block\utils\ChiseledBookshelfSlot;
    use pocketmine\block\utils\Colored;
    use pocketmine\block\utils\ColoredTrait;
    use pocketmine\block\utils\CopperMaterial;
    use pocketmine\block\utils\CopperTrait;
    use pocketmine\block\utils\FacesOppositePlacingPlayerTrait;
    use pocketmine\block\utils\Fallable;
    use pocketmine\block\utils\FallableTrait;
    use pocketmine\block\utils\HorizontalFacing;
    use pocketmine\block\utils\HorizontalFacingTrait;
    use pocketmine\block\utils\LeverFacing;
    use pocketmine\block\utils\Lightable;
    use pocketmine\block\utils\LightableTrait;
    use pocketmine\block\utils\MobHeadType;
    use pocketmine\block\utils\PoweredByRedstone;
    use pocketmine\block\utils\PoweredByRedstoneTrait;
    use pocketmine\block\utils\RedstoneSignalEmitter;
    use pocketmine\block\utils\RedstoneWireConnectionType;
    use pocketmine\block\utils\StaticSupportTrait;
    use pocketmine\block\utils\SupportType;
    use pocketmine\block\utils\WoodMaterial;
    use pocketmine\block\utils\WoodTypeTrait;
    use pocketmine\crafting\FurnaceType;
    use pocketmine\data\runtime\RuntimeDataDescriber;
    use pocketmine\entity\Entity;
    use pocketmine\entity\Location;
    use pocketmine\entity\object\FallingBlock;
    use pocketmine\entity\projectile\Projectile;
    use pocketmine\inventory\Inventory;
    use pocketmine\inventory\InventoryHolder;
    use pocketmine\inventory\SimpleInventory;
    use pocketmine\item\Durable;
    use pocketmine\item\enchantment\VanillaEnchantments;
    use pocketmine\item\Item;
    use pocketmine\item\ItemTypeIds;
    use pocketmine\item\VanillaItems;
    use pocketmine\math\Axis;
    use pocketmine\math\AxisAlignedBB;
    use pocketmine\math\Facing;
    use pocketmine\math\RayTraceResult;
    use pocketmine\math\Vector3;
    use pocketmine\player\Player;
    use pocketmine\world\BlockTransaction;
    use pocketmine\world\Position;
    use pocketmine\world\sound\AnvilFallSound;
    use pocketmine\world\sound\ClickSound;
    use pocketmine\world\sound\DoorSound;
    use pocketmine\world\World;

    use function abs;
    use function array_key_exists;
    use function array_values;
    use function floor;
    use function min;
    use function mt_rand;
    use function spl_object_id;

    // =========================================================================
    // Base helper classes: Flowable & FillableCauldron
    // =========================================================================

    if (!class_exists(Flowable::class, false)) {
        /**
         * "Flowable" blocks are destroyed if water flows into their cell.
         */
        abstract class Flowable extends Transparent
        {
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
                return parent::canBePlacedAt($blockReplace, $clickVector, $face, $isClickedBlock);
            }

            protected function recalculateCollisionBoxes(): array
            {
                return [];
            }
        }
    }

    if (!class_exists(FillableCauldron::class, false)) {
        /**
         * Base class for cauldrons containing liquid.
         */
        abstract class FillableCauldron extends Transparent
        {
            public const MIN_FILL_LEVEL = 1;
            public const MAX_FILL_LEVEL = 6;

            protected int $fillLevel = self::MIN_FILL_LEVEL;

            protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
            {
                $w->boundedIntAuto(self::MIN_FILL_LEVEL, self::MAX_FILL_LEVEL, $this->fillLevel);
            }

            public function getFillLevel(): int
            {
                return $this->fillLevel;
            }

            public function setFillLevel(int $fillLevel): self
            {
                if ($fillLevel < self::MIN_FILL_LEVEL || $fillLevel > self::MAX_FILL_LEVEL) {
                    throw new \InvalidArgumentException("Fill level must be in range " . self::MIN_FILL_LEVEL . " ... " . self::MAX_FILL_LEVEL);
                }
                $this->fillLevel = $fillLevel;
                return $this;
            }

            protected function recalculateCollisionBoxes(): array
            {
                $result = [
                    AxisAlignedBB::one()->trim(Facing::UP, 11 / 16)
                ];
                foreach (Facing::HORIZONTAL as $f) {
                    $result[] = AxisAlignedBB::one()->trim($f, 14 / 16);
                }
                return $result;
            }

            public function getSupportType(int $facing): SupportType
            {
                return $facing === Facing::UP ? SupportType::EDGE : SupportType::NONE;
            }

            protected function withFillLevel(int $fillLevel): Block
            {
                return $fillLevel === 0 ? VanillaBlocks::CAULDRON() : $this->setFillLevel(min(self::MAX_FILL_LEVEL, $fillLevel));
            }

            /**
             * @param Item[] $returnedItems
             */
            protected function addFillLevels(int $amount, Item $usedItem, Item $returnedItem, array &$returnedItems): void
            {
                if ($this->fillLevel >= self::MAX_FILL_LEVEL) {
                    return;
                }
                $world = $this->position?->isValid() ? $this->position->getWorld() : null;
                if ($world !== null) {
                    $world->setBlock($this->position, $this->withFillLevel($this->fillLevel + $amount));
                }
                $usedItem->pop();
                $returnedItems[] = $returnedItem;
            }

            /**
             * @param Item[] $returnedItems
             */
            protected function removeFillLevels(int $amount, Item $usedItem, Item $returnedItem, array &$returnedItems): void
            {
                if ($this->fillLevel < $amount) {
                    return;
                }
                $world = $this->position?->isValid() ? $this->position->getWorld() : null;
                if ($world !== null) {
                    $world->setBlock($this->position, $this->withFillLevel($this->fillLevel - $amount));
                }
                $usedItem->pop();
                $returnedItems[] = $returnedItem;
            }

            public function asItem(): Item
            {
                return VanillaBlocks::CAULDRON()->asItem();
            }
        }
    }

    // =========================================================================
    // 1. Puertas y trampillas
    // =========================================================================

    /**
     * PM `Door`: Base class for double-height door blocks.
     */
    class Door extends Transparent implements HorizontalFacing
    {
        use HorizontalFacingTrait;

        protected bool $top = false;
        protected bool $hingeRight = false;
        protected bool $open = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->horizontalFacing($this->facing);
            $w->bool($this->top);
            $w->bool($this->hingeRight);
            $w->bool($this->open);
        }

        public function readStateFromWorld(): Block
        {
            parent::readStateFromWorld();

            $this->collisionBoxes = null;

            $other = $this->getSide($this->top ? Facing::DOWN : Facing::UP);
            if ($other instanceof Door && $other->hasSameTypeId($this)) {
                if ($this->top) {
                    $this->facing = $other->facing;
                    $this->open = $other->open;
                } else {
                    $this->hingeRight = $other->hingeRight;
                }
            }

            return $this;
        }

        public function isTop(): bool
        {
            return $this->top;
        }

        public function setTop(bool $top): self
        {
            $this->top = $top;
            return $this;
        }

        public function isHingeRight(): bool
        {
            return $this->hingeRight;
        }

        public function setHingeRight(bool $hingeRight): self
        {
            $this->hingeRight = $hingeRight;
            return $this;
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

        public function isSolid(): bool
        {
            return false;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->trim($this->open ? Facing::rotateY($this->facing, !$this->hingeRight) : $this->facing, 327 / 400)];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function onNearbyBlockChange(): void
        {
            if (!$this->canBeSupportedAt($this) && !$this->getSide(Facing::DOWN) instanceof Door) {
                $world = $this->position?->isValid() ? $this->position->getWorld() : null;
                if ($world !== null) {
                    $world->useBreakOn($this->position);
                }
            }
        }

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            if ($face === Facing::UP) {
                $blockUp = $this->getSide(Facing::UP);
                if (!$blockUp->canBeReplaced() || !$this->canBeSupportedAt($blockReplace)) {
                    return false;
                }

                if ($player !== null) {
                    $this->facing = $player->getHorizontalFacing();
                }

                $next = $this->getSide(Facing::rotateY($this->facing, false));
                $next2 = $this->getSide(Facing::rotateY($this->facing, true));

                if ($next->hasSameTypeId($this) || (!$next2->isTransparent() && $next->isTransparent())) {
                    $this->hingeRight = true;
                }

                $topHalf = clone $this;
                $topHalf->top = true;

                $tx->addBlock($blockReplace->getPosition(), $this)->addBlock($blockUp->getPosition(), $topHalf);
                return true;
            }

            return false;
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            $this->open = !$this->open;

            $other = $this->getSide($this->top ? Facing::DOWN : Facing::UP);
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            if ($world !== null) {
                if ($other instanceof Door && $other->hasSameTypeId($this)) {
                    $other->open = $this->open;
                    $world->setBlock($other->getPosition(), $other);
                }
                $world->setBlock($this->position, $this);
                $world->addSound($this->position, new DoorSound());
            }

            return true;
        }

        public function getDrops(mixed $item = null): array
        {
            if (!$this->top) {
                return parent::getDrops($item);
            }
            return [];
        }

        public function getAffectedBlocks(): array
        {
            $other = $this->getSide($this->top ? Facing::DOWN : Facing::UP);
            if ($other->hasSameTypeId($this)) {
                return [$this, $other];
            }
            return parent::getAffectedBlocks();
        }

        private function canBeSupportedAt(Block $block): bool
        {
            return $block->getAdjacentSupportType(Facing::DOWN)->hasEdgeSupport();
        }
    }

    /**
     * PM `WoodenDoor`: Wooden door variant with wood type.
     */
    class WoodenDoor extends Door implements WoodMaterial
    {
        use WoodTypeTrait;

        public function getFuelTime(): int
        {
            return $this->woodType->isFlammable() ? 200 : 0;
        }
    }

    /**
     * PM `IronDoor`: Iron door variant that cannot be opened by hand.
     */
    class IronDoor extends Door
    {
        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            return false;
        }
    }

    /**
     * PM `Trapdoor`: Horizontal or vertical single-block trapdoor.
     */
    class Trapdoor extends Transparent implements HorizontalFacing
    {
        use HorizontalFacingTrait;

        protected bool $open = false;
        protected bool $top = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->horizontalFacing($this->facing);
            $w->bool($this->top);
            $w->bool($this->open);
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

        public function isTop(): bool
        {
            return $this->top;
        }

        public function setTop(bool $top): self
        {
            $this->top = $top;
            return $this;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->trim($this->open ? $this->facing : ($this->top ? Facing::DOWN : Facing::UP), 1 - 0.1825)];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            if ($player !== null) {
                $this->facing = Facing::opposite($player->getHorizontalFacing());
            }
            if (($clickVector->y > 0.5 && $face !== Facing::UP) || $face === Facing::DOWN) {
                $this->top = true;
            }

            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            $this->open = !$this->open;
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            if ($world !== null) {
                $world->setBlock($this->position, $this);
                $world->addSound($this->position, new DoorSound());
            }
            return true;
        }
    }

    /**
     * PM `WoodenTrapdoor`: Wooden trapdoor variant with wood type.
     */
    class WoodenTrapdoor extends Trapdoor implements WoodMaterial
    {
        use WoodTypeTrait;

        public function getFuelTime(): int
        {
            return 300;
        }
    }

    /**
     * PM `IronTrapdoor`: Iron trapdoor variant that cannot be opened by hand.
     */
    class IronTrapdoor extends Trapdoor
    {
        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            return false;
        }
    }

    // =========================================================================
    // 2. Mecanismos de activación
    // =========================================================================

    /**
     * PM `Button`: Base button block.
     */
    abstract class Button extends Flowable implements AnyFacing, RedstoneSignalEmitter
    {
        use AnyFacingTrait;

        protected bool $pressed = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->facing($this->facing);
            $w->bool($this->pressed);
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

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            if ($this->canBeSupportedAt($blockReplace, $face)) {
                $this->facing = $face;
                return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
            }
            return false;
        }

        abstract protected function getActivationTime(): int;

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if (!$this->pressed) {
                $this->pressed = true;
                $world = $this->position?->isValid() ? $this->position->getWorld() : null;
                if ($world !== null) {
                    $world->setBlock($this->position, $this);
                    $world->scheduleDelayedBlockUpdate($this->position, $this->getActivationTime());
                    $world->addSound($this->position->add(0.5, 0.5, 0.5), new ClickSound());
                }
            }
            return true;
        }

        public function onScheduledUpdate(): void
        {
            if ($this->pressed) {
                $this->pressed = false;
                $world = $this->position?->isValid() ? $this->position->getWorld() : null;
                if ($world !== null) {
                    $world->setBlock($this->position, $this);
                    $world->addSound($this->position->add(0.5, 0.5, 0.5), new ClickSound());
                }
            }
        }

        public function onNearbyBlockChange(): void
        {
            if (!$this->canBeSupportedAt($this, $this->facing)) {
                $world = $this->position?->isValid() ? $this->position->getWorld() : null;
                if ($world !== null) {
                    $world->useBreakOn($this->position);
                }
            }
        }

        private function canBeSupportedAt(Block $block, int $face): bool
        {
            return $block->getAdjacentSupportType(Facing::opposite($face))->hasCenterSupport();
        }
    }

    /**
     * PM `WoodenButton`: Wooden button variant.
     */
    class WoodenButton extends Button implements WoodMaterial
    {
        use WoodTypeTrait;

        protected function getActivationTime(): int
        {
            return 30;
        }

        public function hasEntityCollision(): bool
        {
            return false;
        }

        public function getFuelTime(): int
        {
            return $this->woodType->isFlammable() ? 100 : 0;
        }
    }

    /**
     * PM `StoneButton`: Stone button variant.
     */
    class StoneButton extends Button
    {
        protected function getActivationTime(): int
        {
            return 20;
        }
    }

    /**
     * PM `Lever`: Toggleable lever switch.
     */
    class Lever extends Flowable implements RedstoneSignalEmitter
    {
        protected LeverFacing $facing = LeverFacing::UP_AXIS_X;
        protected bool $activated = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->enum($this->facing);
            $w->bool($this->activated);
        }

        public function getFacing(): LeverFacing
        {
            return $this->facing;
        }

        public function setFacing(LeverFacing $facing): self
        {
            $this->facing = $facing;
            return $this;
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

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            if (!$this->canBeSupportedAt($blockReplace, Facing::opposite($face))) {
                return false;
            }

            $selectUpDownPos = function (LeverFacing $x, LeverFacing $z) use ($player): LeverFacing {
                if ($player !== null) {
                    return Facing::axis($player->getHorizontalFacing()) === Axis::X ? $x : $z;
                }
                return $x;
            };

            $this->facing = match ($face) {
                Facing::DOWN => $selectUpDownPos(LeverFacing::DOWN_AXIS_X, LeverFacing::DOWN_AXIS_Z),
                Facing::UP => $selectUpDownPos(LeverFacing::UP_AXIS_X, LeverFacing::UP_AXIS_Z),
                Facing::NORTH => LeverFacing::NORTH,
                Facing::SOUTH => LeverFacing::SOUTH,
                Facing::WEST => LeverFacing::WEST,
                Facing::EAST => LeverFacing::EAST,
                default => LeverFacing::UP_AXIS_X,
            };

            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function onNearbyBlockChange(): void
        {
            if (!$this->canBeSupportedAt($this, Facing::opposite($this->facing->getFacing()))) {
                $world = $this->position?->isValid() ? $this->position->getWorld() : null;
                if ($world !== null) {
                    $world->useBreakOn($this->position);
                }
            }
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            $this->activated = !$this->activated;
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            if ($world !== null) {
                $world->setBlock($this->position, $this);
                $world->addSound($this->position->add(0.5, 0.5, 0.5), new ClickSound());
            }
            return true;
        }

        private function canBeSupportedAt(Block $block, int $face): bool
        {
            return $block->getAdjacentSupportType($face)->hasCenterSupport();
        }
    }

    /**
     * PM `Tripwire`: String tripwire line.
     */
    class Tripwire extends Flowable
    {
        protected bool $triggered = false;
        protected bool $suspended = false;
        protected bool $connected = false;
        protected bool $disarmed = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->bool($this->triggered);
            $w->bool($this->suspended);
            $w->bool($this->connected);
            $w->bool($this->disarmed);
        }

        public function isTriggered(): bool
        {
            return $this->triggered;
        }

        public function setTriggered(bool $triggered): self
        {
            $this->triggered = $triggered;
            return $this;
        }

        public function isSuspended(): bool
        {
            return $this->suspended;
        }

        public function setSuspended(bool $suspended): self
        {
            $this->suspended = $suspended;
            return $this;
        }

        public function isConnected(): bool
        {
            return $this->connected;
        }

        public function setConnected(bool $connected): self
        {
            $this->connected = $connected;
            return $this;
        }

        public function isDisarmed(): bool
        {
            return $this->disarmed;
        }

        public function setDisarmed(bool $disarmed): self
        {
            $this->disarmed = $disarmed;
            return $this;
        }

        public function asItem(): Item
        {
            return VanillaItems::STRING();
        }
    }

    /**
     * PM `TripwireHook`: Anchor hook for tripwire.
     */
    class TripwireHook extends Flowable implements HorizontalFacing, RedstoneSignalEmitter
    {
        use HorizontalFacingTrait;

        protected bool $connected = false;
        protected bool $powered = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->horizontalFacing($this->facing);
            $w->bool($this->connected);
            $w->bool($this->powered);
        }

        public function isConnected(): bool
        {
            return $this->connected;
        }

        public function setConnected(bool $connected): self
        {
            $this->connected = $connected;
            return $this;
        }

        public function isPowered(): bool
        {
            return $this->powered;
        }

        public function setPowered(bool $powered): self
        {
            $this->powered = $powered;
            return $this;
        }

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            if (Facing::axis($face) !== Axis::Y) {
                $this->facing = $face;
                return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
            }
            return false;
        }
    }

    /**
     * PM `LightningRod`: Copper lightning rod.
     */
    if (!class_exists(LightningRod::class, false)) {
        class LightningRod extends Transparent implements AnyFacing, CopperMaterial, RedstoneSignalEmitter
        {
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

            public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
            {
                $this->facing = $face;
                return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
            }
        }
    }

    // =========================================================================
    // 3. Contenedores y funcionales
    // =========================================================================

    /**
     * PM `Chest`: Single or double storage chest.
     */
    class Chest extends Transparent implements HorizontalFacing
    {
        use FacesOppositePlacingPlayerTrait;

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->contract(0.025, 0, 0.025)->trim(Facing::UP, 0.05)];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function getInventory(): Inventory
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            $tile = $world?->getTile($this->position);
            if ($tile instanceof InventoryHolder) {
                return $tile->getInventory();
            }
            return new ChestInventory($this->position);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($player !== null) {
                $inv = $this->getInventory();
                if (method_exists($player, 'setCurrentWindow')) {
                    $player->setCurrentWindow($inv);
                }
            }
            return true;
        }

        public function getFuelTime(): int
        {
            return 300;
        }
    }

    /**
     * PM `EnderChest`: Player-bound storage chest.
     */
    class EnderChest extends Transparent implements HorizontalFacing
    {
        use FacesOppositePlacingPlayerTrait;

        public function getLightLevel(): int
        {
            return 7;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->contract(0.025, 0, 0.025)->trim(Facing::UP, 0.05)];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function getInventory(?Player $player = null): ?Inventory
        {
            if ($player !== null && method_exists($player, 'getEnderInventory')) {
                return new EnderChestInventory($this->position, $player->getEnderInventory());
            }
            return null;
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($player !== null) {
                $inv = $this->getInventory($player);
                if ($inv !== null && method_exists($player, 'setCurrentWindow')) {
                    $player->setCurrentWindow($inv);
                }
            }
            return true;
        }

        public function getDropsForCompatibleTool(Item $item): array
        {
            return [
                VanillaBlocks::OBSIDIAN()->asItem()->setCount(8)
            ];
        }

        public function isAffectedBySilkTouch(): bool
        {
            return true;
        }
    }

    /**
     * PM `Barrel`: Full cube barrel container.
     */
    class Barrel extends Opaque implements AnyFacing
    {
        use AnyFacingTrait;

        protected bool $open = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->facing($this->facing);
            $w->bool($this->open);
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

        public function getInventory(): Inventory
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            $tile = $world?->getTile($this->position);
            if ($tile instanceof InventoryHolder) {
                return $tile->getInventory();
            }
            return new BarrelInventory($this->position);
        }

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            if ($player !== null) {
                if (abs($player->getPosition()->x - $this->position->x) < 2 && abs($player->getPosition()->z - $this->position->z) < 2) {
                    $y = $player->getEyePos()->y;
                    if ($y - $this->position->y > 2) {
                        $this->facing = Facing::UP;
                    } elseif ($this->position->y - $y > 0) {
                        $this->facing = Facing::DOWN;
                    } else {
                        $this->facing = Facing::opposite($player->getHorizontalFacing());
                    }
                } else {
                    $this->facing = Facing::opposite($player->getHorizontalFacing());
                }
            }
            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($player !== null) {
                $inv = $this->getInventory();
                if (method_exists($player, 'setCurrentWindow')) {
                    $player->setCurrentWindow($inv);
                }
            }
            return true;
        }

        public function getFuelTime(): int
        {
            return 300;
        }
    }

    /**
     * PM `Furnace`: Smelting furnace block.
     */
    class Furnace extends Opaque implements Lightable, HorizontalFacing
    {
        use FacesOppositePlacingPlayerTrait;
        use LightableTrait;

        protected mixed $furnaceType;

        public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, mixed $furnaceType = FurnaceType::FURNACE)
        {
            $this->furnaceType = $furnaceType;
            parent::__construct($idInfo, $name, $typeInfo);
        }

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->horizontalFacing($this->facing);
            $w->bool($this->lit);
        }

        public function getFurnaceType(): mixed
        {
            return $this->furnaceType;
        }

        public function getLightLevel(): int
        {
            return $this->lit ? 13 : 0;
        }

        public function getInventory(): Inventory
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            $tile = $world?->getTile($this->position);
            if ($tile instanceof InventoryHolder) {
                return $tile->getInventory();
            }
            return new FurnaceInventory($this->position);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($player !== null) {
                $inv = $this->getInventory();
                if (method_exists($player, 'setCurrentWindow')) {
                    $player->setCurrentWindow($inv);
                }
            }
            return true;
        }

        public function onScheduledUpdate(): void
        {
        }
    }

    /**
     * PM `BlastFurnace`: High-speed ore smelting furnace.
     */
    class BlastFurnace extends Furnace
    {
        public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, mixed $furnaceType = FurnaceType::BLAST_FURNACE)
        {
            parent::__construct($idInfo, $name, $typeInfo, $furnaceType);
        }
    }

    /**
     * PM `Smoker`: High-speed food smoking furnace.
     */
    class Smoker extends Furnace
    {
        public function __construct(BlockIdentifier $idInfo, string $name, BlockTypeInfo $typeInfo, mixed $furnaceType = FurnaceType::SMOKER)
        {
            parent::__construct($idInfo, $name, $typeInfo, $furnaceType);
        }
    }

    /**
     * PM `Hopper`: Funnel item transport container.
     */
    class Hopper extends Transparent implements PoweredByRedstone
    {
        use PoweredByRedstoneTrait;

        private int $facing = Facing::DOWN;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->facingExcept($this->facing, Facing::UP);
            $w->bool($this->powered);
        }

        public function getFacing(): int
        {
            return $this->facing;
        }

        public function setFacing(int $facing): self
        {
            if ($facing === Facing::UP) {
                throw new \InvalidArgumentException("Hopper may not face upward");
            }
            $this->facing = $facing;
            return $this;
        }

        protected function recalculateCollisionBoxes(): array
        {
            $result = [
                AxisAlignedBB::one()->trim(Facing::UP, 6 / 16)
            ];
            foreach (Facing::HORIZONTAL as $f) {
                $result[] = AxisAlignedBB::one()->trim($f, 14 / 16);
            }
            return $result;
        }

        public function getSupportType(int $facing): SupportType
        {
            return match ($facing) {
                Facing::UP => SupportType::FULL,
                Facing::DOWN => $this->facing === Facing::DOWN ? SupportType::CENTER : SupportType::NONE,
                default => SupportType::NONE,
            };
        }

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            $this->facing = $face === Facing::DOWN ? Facing::DOWN : Facing::opposite($face);
            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function getInventory(): Inventory
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            $tile = $world?->getTile($this->position);
            if ($tile instanceof InventoryHolder) {
                return $tile->getInventory();
            }
            return new HopperInventory($this->position);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($player !== null) {
                $inv = $this->getInventory();
                if (method_exists($player, 'setCurrentWindow')) {
                    $player->setCurrentWindow($inv);
                }
            }
            return true;
        }

        public function onScheduledUpdate(): void
        {
        }
    }

    /**
     * PM `ShulkerBox`: Portable container block.
     */
    class ShulkerBox extends Opaque implements AnyFacing
    {
        use AnyFacingTrait;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
        }

        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            $this->facing = $face;
            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function getInventory(): Inventory
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            $tile = $world?->getTile($this->position);
            if ($tile instanceof InventoryHolder) {
                return $tile->getInventory();
            }
            return new ShulkerBoxInventory($this->position);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($player !== null) {
                if ($this->getSide($this->facing)->isSolid()) {
                    return true;
                }
                $inv = $this->getInventory();
                if (method_exists($player, 'setCurrentWindow')) {
                    $player->setCurrentWindow($inv);
                }
            }
            return true;
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }
    }

    /**
     * PM `BrewingStand`: Potion brewing stand.
     */
    class BrewingStand extends Transparent
    {
        /** @var BrewingStandSlot[] */
        protected array $slots = [];

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->enumSet($this->slots, BrewingStandSlot::cases());
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [
                AxisAlignedBB::one()->trim(Facing::UP, 7 / 8),
                AxisAlignedBB::one()->squash(Axis::X, 7 / 16)->squash(Axis::Z, 7 / 16)->trim(Facing::UP, 1 / 8),
            ];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function hasSlot(BrewingStandSlot $slot): bool
        {
            return array_key_exists(spl_object_id($slot), $this->slots);
        }

        public function setSlot(BrewingStandSlot $slot, bool $occupied): self
        {
            if ($occupied) {
                $this->slots[spl_object_id($slot)] = $slot;
            } else {
                unset($this->slots[spl_object_id($slot)]);
            }
            return $this;
        }

        public function getSlots(): array
        {
            return $this->slots;
        }

        public function setSlots(array $slots): self
        {
            $this->slots = [];
            foreach ($slots as $slot) {
                $this->slots[spl_object_id($slot)] = $slot;
            }
            return $this;
        }

        public function getInventory(): Inventory
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            $tile = $world?->getTile($this->position);
            if ($tile instanceof InventoryHolder) {
                return $tile->getInventory();
            }
            return new BrewingStandInventory($this->position);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($player !== null) {
                $inv = $this->getInventory();
                if (method_exists($player, 'setCurrentWindow')) {
                    $player->setCurrentWindow($inv);
                }
            }
            return true;
        }

        public function onScheduledUpdate(): void
        {
        }
    }

    /**
     * PM `Campfire`: Campfire block for cooking and lighting.
     */
    class Campfire extends Transparent implements Lightable, HorizontalFacing
    {
        use HorizontalFacingTrait {
            HorizontalFacingTrait::describeBlockOnlyState as encodeFacingState;
        }
        use LightableTrait {
            LightableTrait::describeBlockOnlyState as encodeLitState;
        }

        protected ?CampfireInventory $inventory = null;
        /** @var array<int, int> */
        protected array $cookingTimes = [];

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $this->encodeFacingState($w);
            $this->encodeLitState($w);
        }

        public function readStateFromWorld(): Block
        {
            parent::readStateFromWorld();
            $tile = $this->position?->isValid() ? $this->position->getWorld()->getTile($this->position) : null;
            if ($tile instanceof InventoryHolder) {
                $inv = $tile->getInventory();
                if ($inv instanceof CampfireInventory) {
                    $this->inventory = $inv;
                }
            }
            $this->inventory ??= new CampfireInventory($this->position);
            return $this;
        }

        public function hasEntityCollision(): bool
        {
            return true;
        }

        public function getLightLevel(): int
        {
            return $this->lit ? 15 : 0;
        }

        public function isAffectedBySilkTouch(): bool
        {
            return true;
        }

        public function getDropsForCompatibleTool(Item $item): array
        {
            return [
                VanillaItems::CHARCOAL()->setCount(2)
            ];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->trim(Facing::UP, 9 / 16)];
        }

        public function getInventory(): CampfireInventory
        {
            return $this->inventory ??= new CampfireInventory($this->position);
        }

        public function getCookingTimes(): array
        {
            return $this->cookingTimes;
        }

        public function setCookingTimes(array $cookingTimes): self
        {
            $this->cookingTimes = $cookingTimes;
            return $this;
        }

        public function setCookingTime(int $slot, int $time): void
        {
            if ($slot < 0 || $slot > 3) {
                throw new \InvalidArgumentException("Slot must be in range 0-3");
            }
            $this->cookingTimes[$slot] = $time;
        }
    }

    /**
     * PM `SoulCampfire`: Soul campfire variant.
     */
    class SoulCampfire extends Campfire
    {
        public function getLightLevel(): int
        {
            return $this->lit ? 10 : 0;
        }

        public function getDropsForCompatibleTool(Item $item): array
        {
            return [
                VanillaBlocks::SOUL_SOIL()->asItem()
            ];
        }
    }

    /**
     * PM `Cauldron`: Empty metal cauldron.
     */
    class Cauldron extends Transparent
    {
        protected function recalculateCollisionBoxes(): array
        {
            $result = [
                AxisAlignedBB::one()->trim(Facing::UP, 11 / 16)
            ];
            foreach (Facing::HORIZONTAL as $f) {
                $result[] = AxisAlignedBB::one()->trim($f, 14 / 16);
            }
            return $result;
        }

        public function getSupportType(int $facing): SupportType
        {
            return $facing === Facing::UP ? SupportType::EDGE : SupportType::NONE;
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            return true;
        }

        public function onNearbyBlockChange(): void
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            if ($world !== null && $world->getBlock($this->position->up())->getTypeId() === BlockTypeIds::WATER) {
                $cauldron = VanillaBlocks::WATER_CAULDRON()->setFillLevel(FillableCauldron::MAX_FILL_LEVEL);
                $world->setBlock($this->position, $cauldron);
            }
        }
    }

    /**
     * PM `WaterCauldron`: Water-filled cauldron.
     */
    class WaterCauldron extends FillableCauldron
    {
        public const WATER_BOTTLE_FILL_AMOUNT = 2;

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            return true;
        }
    }

    /**
     * PM `LavaCauldron`: Lava-filled cauldron.
     */
    class LavaCauldron extends FillableCauldron
    {
        public function getLightLevel(): int
        {
            return 15;
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            return true;
        }
    }

    /**
     * PM `PotionCauldron`: Potion-filled cauldron.
     */
    class PotionCauldron extends FillableCauldron
    {
        public const POTION_FILL_AMOUNT = 2;

        protected ?Item $potionItem = null;

        public function getPotionItem(): ?Item
        {
            return $this->potionItem;
        }

        public function setPotionItem(?Item $potionItem): self
        {
            $this->potionItem = $potionItem;
            return $this;
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            return true;
        }
    }

    /**
     * PM `Bed`: Colored bed block.
     */
    class Bed extends Transparent implements Colored, HorizontalFacing
    {
        use ColoredTrait;
        use HorizontalFacingTrait;

        protected bool $occupied = false;
        protected bool $head = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->horizontalFacing($this->facing);
            $w->bool($this->occupied);
            $w->bool($this->head);
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->trim(Facing::UP, 7 / 16)];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function isHeadPart(): bool
        {
            return $this->head;
        }

        public function setHead(bool $head): self
        {
            $this->head = $head;
            return $this;
        }

        public function isOccupied(): bool
        {
            return $this->occupied;
        }

        public function setOccupied(bool $occupied): self
        {
            $this->occupied = $occupied;
            return $this;
        }

        public function getOtherHalf(): ?Bed
        {
            $other = $this->getSide($this->head ? Facing::opposite($this->facing) : $this->facing);
            return $other instanceof Bed && $other->hasSameTypeId($this) && $other->isHeadPart() !== $this->head ? $other : null;
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            return true;
        }
    }

    /**
     * PM `Anvil`: Gravity-affected anvil for repairing and naming items.
     */
    class Anvil extends Transparent implements Fallable, HorizontalFacing
    {
        use FallableTrait;
        use HorizontalFacingTrait;

        public const UNDAMAGED = 0;
        public const SLIGHTLY_DAMAGED = 1;
        public const VERY_DAMAGED = 2;

        private int $damage = self::UNDAMAGED;

        public function describeBlockItemState(RuntimeDataDescriber $w): void
        {
            $w->boundedIntAuto(self::UNDAMAGED, self::VERY_DAMAGED, $this->damage);
        }

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->horizontalFacing($this->facing);
        }

        public function getDamage(): int
        {
            return $this->damage;
        }

        public function setDamage(int $damage): self
        {
            if ($damage < self::UNDAMAGED || $damage > self::VERY_DAMAGED) {
                throw new \InvalidArgumentException("Damage must be in range " . self::UNDAMAGED . " ... " . self::VERY_DAMAGED);
            }
            $this->damage = $damage;
            return $this;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->squash(Facing::axis(Facing::rotateY($this->facing, false)), 1 / 8)];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function getInventory(): Inventory
        {
            return new AnvilInventory($this->position);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($player !== null) {
                $inv = $this->getInventory();
                if (method_exists($player, 'setCurrentWindow')) {
                    $player->setCurrentWindow($inv);
                }
            }
            return true;
        }

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            if ($player !== null) {
                $this->facing = Facing::rotateY($player->getHorizontalFacing(), false);
            }
            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function onHitGround(FallingBlock $blockEntity): bool
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            if ($world !== null) {
                $world->addSound($this->position, new AnvilFallSound());
            }
            return true;
        }
    }

    /**
     * PM `EnchantingTable`: Enchantment table block.
     */
    class EnchantingTable extends Transparent
    {
        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->trim(Facing::UP, 0.25)];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function getInventory(): Inventory
        {
            return new EnchantInventory($this->position);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($player !== null) {
                $inv = $this->getInventory();
                if (method_exists($player, 'setCurrentWindow')) {
                    $player->setCurrentWindow($inv);
                }
            }
            return true;
        }
    }

    /**
     * PM `Bell`: Wall or floor/ceiling mounted bell.
     */
    class Bell extends Transparent implements HorizontalFacing
    {
        use HorizontalFacingTrait;

        private BellAttachmentType $attachmentType = BellAttachmentType::FLOOR;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->enum($this->attachmentType);
            $w->horizontalFacing($this->facing);
        }

        protected function recalculateCollisionBoxes(): array
        {
            if ($this->attachmentType === BellAttachmentType::FLOOR) {
                return [AxisAlignedBB::one()->squash(Facing::axis($this->facing), 1 / 4)->trim(Facing::UP, 3 / 16)];
            }
            if ($this->attachmentType === BellAttachmentType::CEILING) {
                return [AxisAlignedBB::one()->contract(1 / 4, 0, 1 / 4)->trim(Facing::DOWN, 1 / 4)];
            }
            $box = AxisAlignedBB::one()
                ->squash(Facing::axis(Facing::rotateY($this->facing, true)), 1 / 4)
                ->trim(Facing::UP, 1 / 16)
                ->trim(Facing::DOWN, 1 / 4);
            return [
                $this->attachmentType === BellAttachmentType::ONE_WALL ? $box->trim($this->facing, 3 / 16) : $box
            ];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function getAttachmentType(): BellAttachmentType
        {
            return $this->attachmentType;
        }

        public function setAttachmentType(BellAttachmentType $attachmentType): self
        {
            $this->attachmentType = $attachmentType;
            return $this;
        }

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            if ($face === Facing::UP) {
                if ($player !== null) {
                    $this->setFacing(Facing::opposite($player->getHorizontalFacing()));
                }
                $this->setAttachmentType(BellAttachmentType::FLOOR);
            } elseif ($face === Facing::DOWN) {
                $this->setAttachmentType(BellAttachmentType::CEILING);
                if ($player !== null) {
                    $this->setFacing(Facing::opposite($player->getHorizontalFacing()));
                }
            } else {
                $this->setAttachmentType(BellAttachmentType::ONE_WALL);
                $this->setFacing($face);
            }
            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            return true;
        }
    }

    /**
     * PM `ItemFrame`: Wall/surface item showcase frame.
     */
    class ItemFrame extends Flowable implements AnyFacing
    {
        use AnyFacingTrait;

        public const ROTATIONS = 8;

        protected bool $hasMap = false;
        protected ?Item $framedItem = null;
        protected int $itemRotation = 0;
        protected float $itemDropChance = 1.0;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->facing($this->facing);
            $w->bool($this->hasMap);
        }

        public function getFramedItem(): ?Item
        {
            return $this->framedItem !== null ? clone $this->framedItem : null;
        }

        public function setFramedItem(?Item $item): self
        {
            if ($item === null || $item->isNull()) {
                $this->framedItem = null;
                $this->itemRotation = 0;
            } else {
                $this->framedItem = clone $item;
            }
            return $this;
        }

        public function getItemRotation(): int
        {
            return $this->itemRotation;
        }

        public function setItemRotation(int $rotation): self
        {
            $this->itemRotation = $rotation % self::ROTATIONS;
            return $this;
        }

        public function getItemDropChance(): float
        {
            return $this->itemDropChance;
        }

        public function setItemDropChance(float $chance): self
        {
            $this->itemDropChance = $chance;
            return $this;
        }

        public function hasMap(): bool
        {
            return $this->hasMap;
        }

        public function setHasMap(bool $hasMap): self
        {
            $this->hasMap = $hasMap;
            return $this;
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            if ($this->framedItem === null) {
                if (!$item->isNull()) {
                    $this->setFramedItem($item->pop());
                    if ($world !== null) {
                        $world->setBlock($this->position, $this);
                    }
                    return true;
                }
            } else {
                $this->itemRotation = ($this->itemRotation + 1) % self::ROTATIONS;
                if ($world !== null) {
                    $world->setBlock($this->position, $this);
                }
                return true;
            }
            return false;
        }
    }

    /**
     * PM `GlowingItemFrame`: Glow-in-the-dark item frame.
     */
    class GlowingItemFrame extends ItemFrame
    {
    }

    /**
     * PM `MobHead`: Decorative mob skull or head.
     */
    class MobHead extends Flowable
    {
        public const MIN_ROTATION = 0;
        public const MAX_ROTATION = 15;

        protected MobHeadType $mobHeadType = MobHeadType::SKELETON;
        protected int $facing = Facing::NORTH;
        protected int $rotation = self::MIN_ROTATION;

        public function describeBlockItemState(RuntimeDataDescriber $w): void
        {
            $w->enum($this->mobHeadType);
        }

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->facingExcept($this->facing, Facing::DOWN);
        }

        public function getMobHeadType(): MobHeadType
        {
            return $this->mobHeadType;
        }

        public function setMobHeadType(MobHeadType $mobHeadType): self
        {
            $this->mobHeadType = $mobHeadType;
            return $this;
        }

        public function getFacing(): int
        {
            return $this->facing;
        }

        public function setFacing(int $facing): self
        {
            if ($facing === Facing::DOWN) {
                throw new \InvalidArgumentException("Skull may not face DOWN");
            }
            $this->facing = $facing;
            return $this;
        }

        public function getRotation(): int
        {
            return $this->rotation;
        }

        public function setRotation(int $rotation): self
        {
            if ($rotation < self::MIN_ROTATION || $rotation > self::MAX_ROTATION) {
                throw new \InvalidArgumentException("Rotation must be in range " . self::MIN_ROTATION . " ... " . self::MAX_ROTATION);
            }
            $this->rotation = $rotation;
            return $this;
        }

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            if ($face === Facing::DOWN) {
                return false;
            }
            $this->facing = $face;
            if ($face === Facing::UP && $player !== null) {
                $this->rotation = (int) floor((($player->getLocation()->yaw + 180) * 16 / 360) + 0.5) & 0x0f;
            }
            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }
    }

    /**
     * PM `FlowerPot`: Decorative potted plant container.
     */
    class FlowerPot extends Flowable
    {
        use StaticSupportTrait;

        protected ?Block $plant = null;

        public function getPlant(): ?Block
        {
            return $this->plant;
        }

        public function setPlant(?Block $plant): self
        {
            if ($plant === null || $plant instanceof Air) {
                $this->plant = null;
            } else {
                $this->plant = clone $plant;
            }
            return $this;
        }

        public function canAddPlant(Block $block): bool
        {
            if ($this->plant !== null) {
                return false;
            }
            return $this->isValidPlant($block);
        }

        public function isValidPlant(Block $block): bool
        {
            return $block->hasTypeTag(BlockTypeTags::POTTABLE_PLANTS);
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->contract(3 / 16, 0, 3 / 16)->trim(Facing::UP, 5 / 8)];
        }

        private function canBeSupportedAt(Block $block): bool
        {
            return $block->getAdjacentSupportType(Facing::DOWN)->hasCenterSupport();
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            $block = $item->getBlock();
            if ($this->plant === null && $this->isValidPlant($block)) {
                $this->setPlant($block);
                $item->pop();
                $world = $this->position?->isValid() ? $this->position->getWorld() : null;
                if ($world !== null) {
                    $world->setBlock($this->position, $this);
                }
                return true;
            }
            return false;
        }
    }

    /**
     * PM `MonsterSpawner`: Monster spawner cage block.
     */
    class MonsterSpawner extends Transparent
    {
        public function getDropsForCompatibleTool(Item $item): array
        {
            return [];
        }

        protected function getXpDropAmount(): int
        {
            return mt_rand(15, 43);
        }

        public function onScheduledUpdate(): void
        {
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }
    }

    /**
     * PM `TNT`: Explosive dynamite block.
     */
    class TNT extends Opaque
    {
        protected bool $unstable = false;
        protected bool $worksUnderwater = false;

        public function describeBlockItemState(RuntimeDataDescriber $w): void
        {
            $w->bool($this->worksUnderwater);
        }

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->bool($this->unstable);
        }

        public function isUnstable(): bool
        {
            return $this->unstable;
        }

        public function setUnstable(bool $unstable): self
        {
            $this->unstable = $unstable;
            return $this;
        }

        public function worksUnderwater(): bool
        {
            return $this->worksUnderwater;
        }

        public function setWorksUnderwater(bool $worksUnderwater): self
        {
            $this->worksUnderwater = $worksUnderwater;
            return $this;
        }

        public function onBreak(Item $item, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($this->unstable) {
                $this->ignite();
                return true;
            }
            return parent::onBreak($item, $player, $returnedItems);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($item->getTypeId() === ItemTypeIds::FIRE_CHARGE || $item->hasEnchantment(VanillaEnchantments::FIRE_ASPECT())) {
                $item->pop();
                $this->ignite();
                return true;
            }
            return false;
        }

        public function ignite(int $fuse = 80): void
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            if ($world !== null) {
                $world->setBlock($this->position, VanillaBlocks::AIR());
            }
        }
    }

    /**
     * PM `ChiseledBookshelf`: Storage bookshelf holding up to 6 books.
     */
    class ChiseledBookshelf extends Opaque implements HorizontalFacing
    {
        use FacesOppositePlacingPlayerTrait;

        /** @var array<int, ChiseledBookshelfSlot> */
        private array $slots = [];
        private ?ChiseledBookshelfSlot $lastInteractedSlot = null;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->horizontalFacing($this->facing);
            $w->enumSet($this->slots, ChiseledBookshelfSlot::cases());
        }

        public function hasSlot(ChiseledBookshelfSlot $slot): bool
        {
            return isset($this->slots[spl_object_id($slot)]);
        }

        public function setSlot(ChiseledBookshelfSlot $slot, bool $occupied): self
        {
            if ($occupied) {
                $this->slots[spl_object_id($slot)] = $slot;
            } else {
                unset($this->slots[spl_object_id($slot)]);
            }
            return $this;
        }

        public function getSlots(): array
        {
            return array_values($this->slots);
        }

        public function setSlots(array $slots): self
        {
            $this->slots = [];
            foreach ($slots as $slot) {
                $this->slots[spl_object_id($slot)] = $slot;
            }
            return $this;
        }

        public function getLastInteractedSlot(): ?ChiseledBookshelfSlot
        {
            return $this->lastInteractedSlot;
        }

        public function setLastInteractedSlot(?ChiseledBookshelfSlot $slot): self
        {
            $this->lastInteractedSlot = $slot;
            return $this;
        }

        public function getInventory(): Inventory
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            $tile = $world?->getTile($this->position);
            if ($tile instanceof InventoryHolder) {
                return $tile->getInventory();
            }
            return new SimpleInventory(6);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            return true;
        }
    }

    /**
     * PM `Crafter`: Automated redstone crafting block.
     */
    class Crafter extends Opaque implements AnyFacing, PoweredByRedstone
    {
        use AnyFacingTrait;
        use PoweredByRedstoneTrait;

        protected bool $triggered = false;
        protected bool $crafting = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->facing($this->facing);
            $w->bool($this->triggered);
            $w->bool($this->crafting);
        }

        public function isTriggered(): bool
        {
            return $this->triggered;
        }

        public function setTriggered(bool $triggered): self
        {
            $this->triggered = $triggered;
            return $this;
        }

        public function isCrafting(): bool
        {
            return $this->crafting;
        }

        public function setCrafting(bool $crafting): self
        {
            $this->crafting = $crafting;
            return $this;
        }

        public function getInventory(): Inventory
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            $tile = $world?->getTile($this->position);
            if ($tile instanceof InventoryHolder) {
                return $tile->getInventory();
            }
            return new SimpleInventory(9);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($player !== null) {
                $inv = $this->getInventory();
                if (method_exists($player, 'setCurrentWindow')) {
                    $player->setCurrentWindow($inv);
                }
            }
            return true;
        }
    }

    // =========================================================================
    // 4. Redstone
    // =========================================================================

    /**
     * PM `RedstoneWire`: Dust transmission line.
     */
    class RedstoneWire extends Flowable implements AnalogRedstoneSignalEmitter
    {
        use AnalogRedstoneSignalEmitterTrait;
        use StaticSupportTrait;

        /** @var array<int, RedstoneWireConnectionType> */
        protected array $connections = [];

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->boundedIntAuto(0, 15, $this->signalStrength);
        }

        public function getConnection(int $facing): RedstoneWireConnectionType
        {
            return $this->connections[$facing] ?? RedstoneWireConnectionType::NONE;
        }

        public function setConnection(int $facing, RedstoneWireConnectionType $type): self
        {
            $axis = Facing::axis($facing);
            if ($axis !== Axis::X && $axis !== Axis::Z) {
                throw new \InvalidArgumentException("Facing must be horizontal");
            }
            $this->connections[$facing] = $type;
            return $this;
        }

        private function canBeSupportedAt(Block $block): bool
        {
            return $block->getAdjacentSupportType(Facing::DOWN)->hasCenterSupport();
        }

        public function onNearbyBlockChange(): void
        {
            if (!$this->canBeSupportedAt($this)) {
                $world = $this->position?->isValid() ? $this->position->getWorld() : null;
                if ($world !== null) {
                    $world->useBreakOn($this->position);
                }
            }
        }
    }

    /**
     * PM `RedstoneRepeater`: Diode/delay repeater block.
     */
    class RedstoneRepeater extends Flowable implements PoweredByRedstone, HorizontalFacing, RedstoneSignalEmitter
    {
        use HorizontalFacingTrait;
        use PoweredByRedstoneTrait;
        use StaticSupportTrait;

        public const MIN_DELAY = 1;
        public const MAX_DELAY = 4;

        protected int $delay = self::MIN_DELAY;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->horizontalFacing($this->facing);
            $w->boundedIntAuto(self::MIN_DELAY, self::MAX_DELAY, $this->delay);
            $w->bool($this->powered);
        }

        public function getDelay(): int
        {
            return $this->delay;
        }

        public function setDelay(int $delay): self
        {
            if ($delay < self::MIN_DELAY || $delay > self::MAX_DELAY) {
                throw new \InvalidArgumentException("Delay must be in range " . self::MIN_DELAY . " ... " . self::MAX_DELAY);
            }
            $this->delay = $delay;
            return $this;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->trim(Facing::UP, 7 / 8)];
        }

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            if ($player !== null) {
                $this->facing = Facing::opposite($player->getHorizontalFacing());
            }
            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if (++$this->delay > self::MAX_DELAY) {
                $this->delay = self::MIN_DELAY;
            }
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            if ($world !== null) {
                $world->setBlock($this->position, $this);
            }
            return true;
        }

        private function canBeSupportedAt(Block $block): bool
        {
            return $block->getAdjacentSupportType(Facing::DOWN) !== SupportType::NONE;
        }
    }

    /**
     * PM `RedstoneComparator`: Comparator logic gate block.
     */
    class RedstoneComparator extends Flowable implements AnalogRedstoneSignalEmitter, PoweredByRedstone, HorizontalFacing
    {
        use HorizontalFacingTrait;
        use AnalogRedstoneSignalEmitterTrait;
        use PoweredByRedstoneTrait;
        use StaticSupportTrait;

        protected bool $isSubtractMode = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->horizontalFacing($this->facing);
            $w->bool($this->isSubtractMode);
            $w->bool($this->powered);
        }

        public function isSubtractMode(): bool
        {
            return $this->isSubtractMode;
        }

        public function setSubtractMode(bool $isSubtractMode): self
        {
            $this->isSubtractMode = $isSubtractMode;
            return $this;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->trim(Facing::UP, 7 / 8)];
        }

        public function place(BlockTransaction $tx, Item $item, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): bool
        {
            if ($player !== null) {
                $this->facing = Facing::opposite($player->getHorizontalFacing());
            }
            return parent::place($tx, $item, $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            $this->isSubtractMode = !$this->isSubtractMode;
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            if ($world !== null) {
                $world->setBlock($this->position, $this);
            }
            return true;
        }

        private function canBeSupportedAt(Block $block): bool
        {
            return $block->getAdjacentSupportType(Facing::DOWN) !== SupportType::NONE;
        }
    }

    /**
     * PM `DaylightSensor`: Solar detector emitting redstone signal.
     */
    class DaylightSensor extends Transparent implements AnalogRedstoneSignalEmitter
    {
        use AnalogRedstoneSignalEmitterTrait;

        protected bool $inverted = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->boundedIntAuto(0, 15, $this->signalStrength);
            $w->bool($this->inverted);
        }

        public function isInverted(): bool
        {
            return $this->inverted;
        }

        public function setInverted(bool $inverted = true): self
        {
            $this->inverted = $inverted;
            return $this;
        }

        public function getFuelTime(): int
        {
            return 300;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->trim(Facing::UP, 10 / 16)];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            $this->inverted = !$this->inverted;
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            if ($world !== null) {
                $world->setBlock($this->position, $this);
            }
            return true;
        }

        public function onScheduledUpdate(): void
        {
        }
    }

    /**
     * PM `Note`: The noteblock.
     */
    class Note extends Opaque
    {
        public const MIN_PITCH = 0;
        public const MAX_PITCH = 24;

        private int $pitch = self::MIN_PITCH;

        public function getFuelTime(): int
        {
            return 300;
        }

        public function getPitch(): int
        {
            return $this->pitch;
        }

        public function setPitch(int $pitch): self
        {
            if ($pitch < self::MIN_PITCH || $pitch > self::MAX_PITCH) {
                throw new \InvalidArgumentException("Pitch must be in range " . self::MIN_PITCH . " - " . self::MAX_PITCH);
            }
            $this->pitch = $pitch;
            return $this;
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            $this->pitch = ($this->pitch + 1) % (self::MAX_PITCH + 1);
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            if ($world !== null) {
                $world->setBlock($this->position, $this);
            }
            return true;
        }
    }

    /**
     * PM `Jukebox`: Music disc player block.
     */
    class Jukebox extends Opaque
    {
        private ?Item $record = null;

        public function getFuelTime(): int
        {
            return 300;
        }

        public function getRecord(): ?Item
        {
            return $this->record;
        }

        public function insertRecord(Item $record): void
        {
            if ($this->record === null) {
                $this->record = clone $record;
            }
        }

        public function ejectRecord(): void
        {
            if ($this->record !== null) {
                $world = $this->position?->isValid() ? $this->position->getWorld() : null;
                if ($world !== null) {
                    $world->dropItem($this->position->add(0.5, 1, 0.5), $this->record);
                }
                $this->record = null;
            }
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($this->record !== null) {
                $this->ejectRecord();
                $world = $this->position?->isValid() ? $this->position->getWorld() : null;
                if ($world !== null) {
                    $world->setBlock($this->position, $this);
                }
                return true;
            }
            return false;
        }

        public function onBreak(Item $item, ?Player $player = null, array &$returnedItems = []): bool
        {
            $this->ejectRecord();
            return parent::onBreak($item, $player, $returnedItems);
        }
    }

    /**
     * PM `Lectern`: Reading lectern with redstone comparator output.
     */
    class Lectern extends Transparent implements HorizontalFacing, RedstoneSignalEmitter
    {
        use FacesOppositePlacingPlayerTrait;

        protected int $viewedPage = 0;
        protected ?Item $book = null;
        protected bool $producingSignal = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->horizontalFacing($this->facing);
            $w->bool($this->producingSignal);
        }

        public function getFlammability(): int
        {
            return 30;
        }

        public function getBook(): ?Item
        {
            return $this->book !== null ? clone $this->book : null;
        }

        public function setBook(?Item $book): self
        {
            $this->book = $book !== null ? clone $book : null;
            return $this;
        }

        public function getViewedPage(): int
        {
            return $this->viewedPage;
        }

        public function setViewedPage(int $page): self
        {
            $this->viewedPage = $page;
            return $this;
        }

        public function isProducingSignal(): bool
        {
            return $this->producingSignal;
        }

        public function setProducingSignal(bool $producingSignal): self
        {
            $this->producingSignal = $producingSignal;
            return $this;
        }

        public function getDrops(mixed $item = null): array
        {
            $drops = parent::getDrops($item);
            if ($this->book !== null) {
                $drops[] = clone $this->book;
            }
            return $drops;
        }

        protected function recalculateCollisionBoxes(): array
        {
            return [AxisAlignedBB::one()->trim(Facing::UP, 0.1)];
        }

        public function getSupportType(int $facing): SupportType
        {
            return SupportType::NONE;
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            return true;
        }
    }

    /**
     * PM `Beacon`: Beacon pyramid power block.
     */
    class Beacon extends Transparent
    {
        public function getLightLevel(): int
        {
            return 15;
        }
    }

    /**
     * PM `Dispenser`: 3x3 container dispensing projectiles/items.
     */
    class Dispenser extends Opaque implements AnyFacing, PoweredByRedstone
    {
        use AnyFacingTrait;
        use PoweredByRedstoneTrait;

        protected bool $triggered = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->facing($this->facing);
            $w->bool($this->triggered);
        }

        public function isTriggered(): bool
        {
            return $this->triggered;
        }

        public function setTriggered(bool $triggered): self
        {
            $this->triggered = $triggered;
            return $this;
        }

        public function getInventory(): Inventory
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            $tile = $world?->getTile($this->position);
            if ($tile instanceof InventoryHolder) {
                return $tile->getInventory();
            }
            return new SimpleInventory(9);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($player !== null) {
                $inv = $this->getInventory();
                if (method_exists($player, 'setCurrentWindow')) {
                    $player->setCurrentWindow($inv);
                }
            }
            return true;
        }
    }

    /**
     * PM `Dropper`: 3x3 container dropping raw items.
     */
    class Dropper extends Opaque implements AnyFacing, PoweredByRedstone
    {
        use AnyFacingTrait;
        use PoweredByRedstoneTrait;

        protected bool $triggered = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->facing($this->facing);
            $w->bool($this->triggered);
        }

        public function isTriggered(): bool
        {
            return $this->triggered;
        }

        public function setTriggered(bool $triggered): self
        {
            $this->triggered = $triggered;
            return $this;
        }

        public function getInventory(): Inventory
        {
            $world = $this->position?->isValid() ? $this->position->getWorld() : null;
            $tile = $world?->getTile($this->position);
            if ($tile instanceof InventoryHolder) {
                return $tile->getInventory();
            }
            return new SimpleInventory(9);
        }

        public function onInteract(Item $item, int $face, Vector3 $clickVector, ?Player $player = null, array &$returnedItems = []): bool
        {
            if ($player !== null) {
                $inv = $this->getInventory();
                if (method_exists($player, 'setCurrentWindow')) {
                    $player->setCurrentWindow($inv);
                }
            }
            return true;
        }
    }

    /**
     * PM `Piston`: Mechanical push piston block.
     */
    class Piston extends Transparent implements AnyFacing, PoweredByRedstone
    {
        use AnyFacingTrait;
        use PoweredByRedstoneTrait;

        protected bool $sticky = false;
        protected int $state = 0;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->facing($this->facing);
            $w->bool($this->sticky);
            $w->int(3, $this->state);
        }

        public function isSticky(): bool
        {
            return $this->sticky;
        }

        public function setSticky(bool $sticky): self
        {
            $this->sticky = $sticky;
            return $this;
        }

        public function getState(): int
        {
            return $this->state;
        }

        public function setState(int $state): self
        {
            $this->state = $state;
            return $this;
        }
    }

    /**
     * PM `PistonHead`: Extended piston arm and head block.
     */
    class PistonHead extends Transparent implements AnyFacing
    {
        use AnyFacingTrait;

        protected bool $sticky = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->facing($this->facing);
            $w->bool($this->sticky);
        }

        public function isSticky(): bool
        {
            return $this->sticky;
        }

        public function setSticky(bool $sticky): self
        {
            $this->sticky = $sticky;
            return $this;
        }
    }

    /**
     * PM `Observer`: Block-update sensing block.
     */
    class Observer extends Opaque implements AnyFacing, PoweredByRedstone, RedstoneSignalEmitter
    {
        use AnyFacingTrait;
        use PoweredByRedstoneTrait;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->facing($this->facing);
            $w->bool($this->powered);
        }
    }

    /**
     * PM `Target`: Projectile-sensitive archery target block.
     */
    class Target extends Opaque implements AnalogRedstoneSignalEmitter
    {
        use AnalogRedstoneSignalEmitterTrait;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->boundedIntAuto(0, 15, $this->signalStrength);
        }
    }

    /**
     * PM `SculkSensor`: Vibration-sensing sculk sensor.
     */
    class SculkSensor extends Transparent implements AnalogRedstoneSignalEmitter, PoweredByRedstone
    {
        use AnalogRedstoneSignalEmitterTrait;
        use PoweredByRedstoneTrait;

        public const PHASE_INACTIVE = 0;
        public const PHASE_ACTIVE = 1;
        public const PHASE_COOLDOWN = 2;

        protected int $phase = self::PHASE_INACTIVE;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->boundedIntAuto(0, 15, $this->signalStrength);
            $w->int(2, $this->phase);
            $w->bool($this->powered);
        }

        public function getPhase(): int
        {
            return $this->phase;
        }

        public function setPhase(int $phase): self
        {
            $this->phase = $phase;
            return $this;
        }

        public function getLightLevel(): int
        {
            return 1;
        }
    }

    /**
     * PM `SculkCatalyst`: Death-activated sculk spreading block.
     */
    class SculkCatalyst extends Opaque
    {
        protected bool $bloom = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->bool($this->bloom);
        }

        public function isBloom(): bool
        {
            return $this->bloom;
        }

        public function setBloom(bool $bloom): self
        {
            $this->bloom = $bloom;
            return $this;
        }

        public function getLightLevel(): int
        {
            return $this->bloom ? 6 : 0;
        }
    }

    /**
     * PM `SculkShrieker`: Warden-summoning sculk shrieker.
     */
    class SculkShrieker extends Transparent
    {
        protected bool $canSummon = false;
        protected bool $shrieking = false;

        protected function describeBlockOnlyState(RuntimeDataDescriber $w): void
        {
            $w->bool($this->canSummon);
            $w->bool($this->shrieking);
        }

        public function canSummon(): bool
        {
            return $this->canSummon;
        }

        public function setCanSummon(bool $canSummon): self
        {
            $this->canSummon = $canSummon;
            return $this;
        }

        public function isShrieking(): bool
        {
            return $this->shrieking;
        }

        public function setShrieking(bool $shrieking): self
        {
            $this->shrieking = $shrieking;
            return $this;
        }
    }
}
