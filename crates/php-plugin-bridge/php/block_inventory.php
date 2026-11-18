<?php

declare(strict_types=1);

namespace pocketmine\inventory {

    if (!interface_exists(Inventory::class, false)) {
        interface Inventory
        {
            public function getSize(): int;
            public function getMaxStackSize(): int;
            public function getItem(int $index): mixed;
            public function setItem(int $index, mixed $item): void;
            public function getContents(bool $includeEmpty = false): array;
        }
    }

    if (!interface_exists(InventoryHolder::class, false)) {
        interface InventoryHolder
        {
            public function getInventory(): Inventory;
        }
    }

    if (!interface_exists(TemporaryInventory::class, false)) {
        interface TemporaryInventory
        {
        }
    }

    if (!class_exists(BaseInventory::class, false)) {
        abstract class BaseInventory implements Inventory
        {
            protected array $slots = [];

            public function __construct(protected int $size = 0)
            {
            }

            public function getSize(): int
            {
                return $this->size;
            }

            public function getMaxStackSize(): int
            {
                return 64;
            }

            public function getItem(int $index): mixed
            {
                return $this->slots[$index] ?? null;
            }

            public function setItem(int $index, mixed $item): void
            {
                $this->slots[$index] = $item;
            }

            public function getContents(bool $includeEmpty = false): array
            {
                return $this->slots;
            }

            public function getViewers(): array
            {
                return [];
            }

            public function clearAll(): void
            {
                $this->slots = [];
            }
        }
    }

    if (!class_exists(SimpleInventory::class, false)) {
        class SimpleInventory extends BaseInventory
        {
        }
    }

    if (!class_exists(DelegateInventory::class, false)) {
        abstract class DelegateInventory implements Inventory
        {
            public function __construct(protected Inventory $delegate)
            {
            }

            public function getSize(): int
            {
                return $this->delegate->getSize();
            }

            public function getMaxStackSize(): int
            {
                return $this->delegate->getMaxStackSize();
            }

            public function getItem(int $index): mixed
            {
                return $this->delegate->getItem($index);
            }

            public function setItem(int $index, mixed $item): void
            {
                $this->delegate->setItem($index, $item);
            }

            public function getContents(bool $includeEmpty = false): array
            {
                return $this->delegate->getContents($includeEmpty);
            }

            public function getViewers(): array
            {
                return [];
            }
        }
    }

    if (!class_exists(PlayerEnderInventory::class, false)) {
        class PlayerEnderInventory extends SimpleInventory
        {
        }
    }
}

namespace pocketmine\block\inventory {

    use pocketmine\block\Barrel;
    use pocketmine\block\BlockTypeIds;
    use pocketmine\block\tile\EnderChest;
    use pocketmine\crafting\CraftingGrid;
    use pocketmine\crafting\FurnaceType;
    use pocketmine\event\player\PlayerEnchantingOptionsRequestEvent;
    use pocketmine\inventory\BaseInventory;
    use pocketmine\inventory\DelegateInventory;
    use pocketmine\inventory\Inventory;
    use pocketmine\inventory\InventoryHolder;
    use pocketmine\inventory\PlayerEnderInventory;
    use pocketmine\inventory\SimpleInventory;
    use pocketmine\inventory\TemporaryInventory;
    use pocketmine\item\enchantment\EnchantingHelper as Helper;
    use pocketmine\item\enchantment\EnchantingOption;
    use pocketmine\item\Item;
    use pocketmine\item\ItemTypeIds;
    use pocketmine\network\mcpe\protocol\BlockEventPacket;
    use pocketmine\network\mcpe\protocol\types\BlockPosition;
    use pocketmine\player\Player;
    use pocketmine\world\Position;
    use pocketmine\world\sound\BarrelCloseSound;
    use pocketmine\world\sound\BarrelOpenSound;
    use pocketmine\world\sound\ChestCloseSound;
    use pocketmine\world\sound\ChestOpenSound;
    use pocketmine\world\sound\EnderChestCloseSound;
    use pocketmine\world\sound\EnderChestOpenSound;
    use pocketmine\world\sound\ShulkerBoxCloseSound;
    use pocketmine\world\sound\ShulkerBoxOpenSound;
    use pocketmine\world\sound\Sound;
    use function array_values;
    use function count;

    interface BlockInventory
    {
        public function getHolder(): Position;
    }

    trait BlockInventoryTrait
    {
        protected Position $holder;

        public function getHolder(): Position
        {
            return $this->holder;
        }
    }

    trait AnimatedBlockInventoryTrait
    {
        use BlockInventoryTrait;

        public function getViewerCount(): int
        {
            return count($this->getViewers());
        }

        /**
         * @return Player[]
         * @phpstan-return array<int, Player>
         */
        abstract public function getViewers(): array;

        abstract protected function getOpenSound(): Sound;

        abstract protected function getCloseSound(): Sound;

        public function onOpen(Player $who): void
        {
            parent::onOpen($who);

            if ($this->holder->isValid() && $this->getViewerCount() === 1) {
                //TODO: this crap really shouldn't be managed by the inventory
                $this->animateBlock(true);
                $this->holder->getWorld()->addSound($this->holder->add(0.5, 0.5, 0.5), $this->getOpenSound());
            }
        }

        abstract protected function animateBlock(bool $isOpen): void;

        public function onClose(Player $who): void
        {
            if ($this->holder->isValid() && $this->getViewerCount() === 1) {
                //TODO: this crap really shouldn't be managed by the inventory
                $this->animateBlock(false);
                $this->holder->getWorld()->addSound($this->holder->add(0.5, 0.5, 0.5), $this->getCloseSound());
            }
            parent::onClose($who);
        }
    }

    class AnvilInventory extends SimpleInventory implements BlockInventory, TemporaryInventory
    {
        use BlockInventoryTrait;

        public const SLOT_INPUT = 0;
        public const SLOT_MATERIAL = 1;

        public function __construct(Position $holder)
        {
            $this->holder = $holder;
            parent::__construct(2);
        }
    }

    class BarrelInventory extends SimpleInventory implements BlockInventory
    {
        use AnimatedBlockInventoryTrait;

        public function __construct(Position $holder)
        {
            $this->holder = $holder;
            parent::__construct(27);
        }

        protected function getOpenSound(): Sound
        {
            return new BarrelOpenSound();
        }

        protected function getCloseSound(): Sound
        {
            return new BarrelCloseSound();
        }

        protected function animateBlock(bool $isOpen): void
        {
            $holder = $this->getHolder();
            $world = $holder->getWorld();
            $block = $world->getBlock($holder);
            if ($block instanceof Barrel) {
                $world->setBlock($holder, $block->setOpen($isOpen));
            }
        }
    }

    class BrewingStandInventory extends SimpleInventory implements BlockInventory
    {
        use BlockInventoryTrait;

        public const SLOT_INGREDIENT = 0;
        public const SLOT_BOTTLE_LEFT = 1;
        public const SLOT_BOTTLE_MIDDLE = 2;
        public const SLOT_BOTTLE_RIGHT = 3;
        public const SLOT_FUEL = 4;

        public function __construct(Position $holder, int $size = 5)
        {
            $this->holder = $holder;
            parent::__construct($size);
        }
    }

    class CampfireInventory extends SimpleInventory implements BlockInventory
    {
        use BlockInventoryTrait;

        public function __construct(Position $holder)
        {
            $this->holder = $holder;
            parent::__construct(4);
        }

        public function getMaxStackSize(): int
        {
            return 1;
        }
    }

    final class CartographyTableInventory extends SimpleInventory implements BlockInventory, TemporaryInventory
    {
        use BlockInventoryTrait;

        public function __construct(Position $holder)
        {
            $this->holder = $holder;
            parent::__construct(2);
        }
    }

    class ChestInventory extends SimpleInventory implements BlockInventory
    {
        use AnimatedBlockInventoryTrait;

        public function __construct(Position $holder)
        {
            $this->holder = $holder;
            parent::__construct(27);
        }

        protected function getOpenSound(): Sound
        {
            return new ChestOpenSound();
        }

        protected function getCloseSound(): Sound
        {
            return new ChestCloseSound();
        }

        public function animateBlock(bool $isOpen): void
        {
            $holder = $this->getHolder();

            //event ID is always 1 for a chest
            $holder->getWorld()->broadcastPacketToViewers($holder, BlockEventPacket::create(BlockPosition::fromVector3($holder), 1, $isOpen ? 1 : 0));
        }
    }

    final class CraftingTableInventory extends CraftingGrid implements BlockInventory, TemporaryInventory
    {
        use BlockInventoryTrait;

        public function __construct(Position $holder)
        {
            $this->holder = $holder;
            parent::__construct(CraftingGrid::SIZE_BIG);
        }
    }

    class DoubleChestInventory extends BaseInventory implements BlockInventory, InventoryHolder
    {
        use AnimatedBlockInventoryTrait;

        public function __construct(
            private ChestInventory $left,
            private ChestInventory $right
        ) {
            $this->holder = $this->left->getHolder();
            parent::__construct();
        }

        public function getInventory(): self
        {
            return $this;
        }

        public function getSize(): int
        {
            return $this->left->getSize() + $this->right->getSize();
        }

        public function getItem(int $index): Item
        {
            return $index < $this->left->getSize() ? $this->left->getItem($index) : $this->right->getItem($index - $this->left->getSize());
        }

        protected function internalSetItem(int $index, Item $item): void
        {
            $index < $this->left->getSize() ? $this->left->setItem($index, $item) : $this->right->setItem($index - $this->left->getSize(), $item);
        }

        public function getContents(bool $includeEmpty = false): array
        {
            $result = $this->left->getContents($includeEmpty);
            $leftSize = $this->left->getSize();

            foreach ($this->right->getContents($includeEmpty) as $i => $item) {
                $result[$i + $leftSize] = $item;
            }

            return $result;
        }

        protected function internalSetContents(array $items): void
        {
            $leftSize = $this->left->getSize();

            $leftContents = [];
            $rightContents = [];

            foreach ($items as $i => $item) {
                if ($i < $this->left->getSize()) {
                    $leftContents[$i] = $item;
                } else {
                    $rightContents[$i - $leftSize] = $item;
                }
            }
            $this->left->setContents($leftContents);
            $this->right->setContents($rightContents);
        }

        protected function getMatchingItemCount(int $slot, Item $test, bool $checkTags): int
        {
            $leftSize = $this->left->getSize();
            return $slot < $leftSize ?
                $this->left->getMatchingItemCount($slot, $test, $checkTags) :
                $this->right->getMatchingItemCount($slot - $leftSize, $test, $checkTags);
        }

        public function isSlotEmpty(int $index): bool
        {
            $leftSize = $this->left->getSize();
            return $index < $leftSize ?
                $this->left->isSlotEmpty($index) :
                $this->right->isSlotEmpty($index - $leftSize);
        }

        protected function getOpenSound(): Sound
        {
            return new ChestOpenSound();
        }

        protected function getCloseSound(): Sound
        {
            return new ChestCloseSound();
        }

        protected function animateBlock(bool $isOpen): void
        {
            $this->left->animateBlock($isOpen);
            $this->right->animateBlock($isOpen);
        }

        public function getLeftSide(): ChestInventory
        {
            return $this->left;
        }

        public function getRightSide(): ChestInventory
        {
            return $this->right;
        }
    }

    class EnchantInventory extends SimpleInventory implements BlockInventory, TemporaryInventory
    {
        use BlockInventoryTrait;

        public const SLOT_INPUT = 0;
        public const SLOT_LAPIS = 1;

        /**
         * @var EnchantingOption[] $options
         * @phpstan-var list<EnchantingOption>
         */
        private array $options = [];

        public function __construct(Position $holder)
        {
            $this->holder = $holder;
            parent::__construct(2);
        }

        protected function onSlotChange(int $index, Item $before): void
        {
            if ($index === self::SLOT_INPUT) {
                foreach ($this->viewers as $viewer) {
                    $this->options = [];
                    $item = $this->getInput();
                    $options = Helper::generateOptions($this->holder, $item, $viewer->getEnchantmentSeed());

                    $event = new PlayerEnchantingOptionsRequestEvent($viewer, $this, $options);
                    $event->call();
                    if (!$event->isCancelled() && count($event->getOptions()) > 0) {
                        $this->options = array_values($event->getOptions());
                        $viewer->getNetworkSession()->getInvManager()?->syncEnchantingTableOptions($this->options);
                    }
                }
            }

            parent::onSlotChange($index, $before);
        }

        public function getInput(): Item
        {
            return $this->getItem(self::SLOT_INPUT);
        }

        public function getLapis(): Item
        {
            return $this->getItem(self::SLOT_LAPIS);
        }

        public function getOutput(int $optionId): ?Item
        {
            $option = $this->getOption($optionId);
            return $option === null ? null : Helper::enchantItem($this->getInput(), $option->getEnchantments());
        }

        public function getOption(int $optionId): ?EnchantingOption
        {
            return $this->options[$optionId] ?? null;
        }
    }

    /**
     * EnderChestInventory is not a real inventory; it's just a gateway to the player's ender inventory.
     */
    class EnderChestInventory extends DelegateInventory implements BlockInventory
    {
        use AnimatedBlockInventoryTrait {
            onClose as animatedBlockInventoryTrait_onClose;
        }

        public function __construct(
            Position $holder,
            private PlayerEnderInventory $inventory
        ) {
            parent::__construct($inventory);
            $this->holder = $holder;
        }

        public function getEnderInventory(): PlayerEnderInventory
        {
            return $this->inventory;
        }

        public function getViewerCount(): int
        {
            $enderChest = $this->getHolder()->getWorld()->getTile($this->getHolder());
            if (!$enderChest instanceof EnderChest) {
                return 0;
            }
            return $enderChest->getViewerCount();
        }

        protected function getOpenSound(): Sound
        {
            return new EnderChestOpenSound();
        }

        protected function getCloseSound(): Sound
        {
            return new EnderChestCloseSound();
        }

        protected function animateBlock(bool $isOpen): void
        {
            $holder = $this->getHolder();

            //event ID is always 1 for a chest
            $holder->getWorld()->broadcastPacketToViewers($holder, BlockEventPacket::create(BlockPosition::fromVector3($holder), 1, $isOpen ? 1 : 0));
        }

        public function onClose(Player $who): void
        {
            $this->animatedBlockInventoryTrait_onClose($who);
            $enderChest = $this->getHolder()->getWorld()->getTile($this->getHolder());
            if ($enderChest instanceof EnderChest) {
                $enderChest->setViewerCount($enderChest->getViewerCount() - 1);
            }
        }
    }

    class FurnaceInventory extends SimpleInventory implements BlockInventory
    {
        use BlockInventoryTrait;

        public const SLOT_INPUT = 0;
        public const SLOT_FUEL = 1;
        public const SLOT_RESULT = 2;

        public function __construct(
            Position $holder,
            private FurnaceType $furnaceType
        ) {
            $this->holder = $holder;
            parent::__construct(3);
        }

        public function getFurnaceType(): FurnaceType
        {
            return $this->furnaceType;
        }

        public function getResult(): Item
        {
            return $this->getItem(self::SLOT_RESULT);
        }

        public function getFuel(): Item
        {
            return $this->getItem(self::SLOT_FUEL);
        }

        public function getSmelting(): Item
        {
            return $this->getItem(self::SLOT_INPUT);
        }

        public function setResult(Item $item): void
        {
            $this->setItem(self::SLOT_RESULT, $item);
        }

        public function setFuel(Item $item): void
        {
            $this->setItem(self::SLOT_FUEL, $item);
        }

        public function setSmelting(Item $item): void
        {
            $this->setItem(self::SLOT_INPUT, $item);
        }
    }

    class HopperInventory extends SimpleInventory implements BlockInventory
    {
        use BlockInventoryTrait;

        public function __construct(Position $holder, int $size = 5)
        {
            $this->holder = $holder;
            parent::__construct($size);
        }
    }

    final class LoomInventory extends SimpleInventory implements BlockInventory, TemporaryInventory
    {
        use BlockInventoryTrait;

        public const SLOT_BANNER = 0;
        public const SLOT_DYE = 1;
        public const SLOT_PATTERN = 2;

        public function __construct(Position $holder, int $size = 3)
        {
            $this->holder = $holder;
            parent::__construct($size);
        }
    }

    class ShulkerBoxInventory extends SimpleInventory implements BlockInventory
    {
        use AnimatedBlockInventoryTrait;

        public function __construct(Position $holder)
        {
            $this->holder = $holder;
            parent::__construct(27);
        }

        protected function getOpenSound(): Sound
        {
            return new ShulkerBoxOpenSound();
        }

        protected function getCloseSound(): Sound
        {
            return new ShulkerBoxCloseSound();
        }

        public function canAddItem(Item $item): bool
        {
            $blockTypeId = ItemTypeIds::toBlockTypeId($item->getTypeId());
            if ($blockTypeId === BlockTypeIds::SHULKER_BOX || $blockTypeId === BlockTypeIds::DYED_SHULKER_BOX) {
                return false;
            }
            return parent::canAddItem($item);
        }

        protected function animateBlock(bool $isOpen): void
        {
            $holder = $this->getHolder();

            //event ID is always 1 for a chest
            $holder->getWorld()->broadcastPacketToViewers($holder, BlockEventPacket::create(BlockPosition::fromVector3($holder), 1, $isOpen ? 1 : 0));
        }
    }

    final class SmithingTableInventory extends SimpleInventory implements BlockInventory, TemporaryInventory
    {
        use BlockInventoryTrait;

        public function __construct(Position $holder)
        {
            $this->holder = $holder;
            parent::__construct(3);
        }
    }

    class StonecutterInventory extends SimpleInventory implements BlockInventory, TemporaryInventory
    {
        use BlockInventoryTrait;

        public const SLOT_INPUT = 0;

        public function __construct(Position $holder)
        {
            $this->holder = $holder;
            parent::__construct(1);
        }
    }
}
