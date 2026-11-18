<?php

declare(strict_types=1);

namespace {
    // Pure PHP equivalent of `morton2d_encode` when the C extension is not loaded.
    if (!function_exists('morton2d_encode')) {
        function morton2d_encode(int $x, int $y): int
        {
            $res = 0;
            for ($i = 0; $i < 32; $i++) {
                $res |= (($x >> $i) & 1) << (2 * $i);
                $res |= (($y >> $i) & 1) << (2 * $i + 1);
            }
            return $res;
        }
    }
}

namespace pocketmine\data\runtime {

    use pocketmine\math\Facing;

    class RuntimeDataWriter implements RuntimeDataDescriber
    {
        private int $value = 0;
        private int $offset = 0;

        public function __construct(
            private int $maxBits
        ) {}

        public function getValue(): int
        {
            return $this->value;
        }

        public function writeInt(int $bits, int $value): void
        {
            if ($this->offset + $bits > $this->maxBits) {
                throw new \InvalidArgumentException("Bit buffer cannot be larger than $this->maxBits bits (already have $this->offset bits)");
            }
            if (($value & (~0 << $bits)) !== 0 && $bits < 64) {
                throw new \InvalidArgumentException("Value $value does not fit into $bits bits");
            }
            $this->value |= ($value << $this->offset);
            $this->offset += $bits;
        }

        public function int(int $bits, int &$value): void
        {
            $this->writeInt($bits, $value);
        }

        public function boundedInt(int $bits, int $min, int $max, int &$value): void
        {
            $this->writeBoundedIntAuto($min, $max, $value);
        }

        private function writeBoundedIntAuto(int $min, int $max, int $value): void
        {
            if ($value < $min || $value > $max) {
                throw new \InvalidArgumentException("Value $value is outside the range $min - $max");
            }
            $range = $max - $min;
            $bits = $range > 0 ? ((int)log($range, 2)) + 1 : 1;
            $this->writeInt($bits, $value - $min);
        }

        public function boundedIntAuto(int $min, int $max, int &$value): void
        {
            $this->writeBoundedIntAuto($min, $max, $value);
        }

        public function bool(bool &$value): void
        {
            $this->writeInt(1, $value ? 1 : 0);
        }

        public function horizontalFacing(int &$facing): void
        {
            $this->writeInt(2, match ($facing) {
                Facing::NORTH => 0,
                Facing::EAST => 1,
                Facing::SOUTH => 2,
                Facing::WEST => 3,
                default => 0
            });
        }

        public function facingFlags(array &$faces): void
        {
            $uniqueFaces = array_flip($faces);
            foreach (Facing::ALL as $facing) {
                $b = isset($uniqueFaces[$facing]);
                $this->bool($b);
            }
        }

        public function horizontalFacingFlags(array &$faces): void
        {
            $uniqueFaces = array_flip($faces);
            foreach (Facing::HORIZONTAL as $facing) {
                $b = isset($uniqueFaces[$facing]);
                $this->bool($b);
            }
        }

        public function facing(int &$facing): void
        {
            $this->writeInt(3, match ($facing) {
                Facing::DOWN => 0,
                Facing::UP => 1,
                Facing::NORTH => 2,
                Facing::SOUTH => 3,
                Facing::WEST => 4,
                Facing::EAST => 5,
                default => 0
            });
        }

        public function facingExcept(int &$facing, int $except): void
        {
            $this->facing($facing);
        }

        public function axis(int &$axis): void
        {
            $this->writeInt(2, $axis & 0x03);
        }

        public function horizontalAxis(int &$axis): void
        {
            $this->writeInt(1, $axis & 0x01);
        }

        public function wallConnections(array &$connections): void
        {
            foreach (Facing::HORIZONTAL as $f) {
                $val = isset($connections[$f]) ? 1 : 0;
                $this->writeInt(2, $val);
            }
        }

        public function brewingStandSlots(array &$slots): void
        {
            for ($i = 0; $i < 3; $i++) {
                $b = isset($slots[$i]);
                $this->bool($b);
            }
        }

        public function railShape(int &$shape): void
        {
            $this->writeInt(4, $shape & 0x0f);
        }

        public function straightOnlyRailShape(int &$shape): void
        {
            $this->writeInt(3, $shape & 0x07);
        }

        public function enum(mixed &$case): void
        {
            if ($case instanceof \UnitEnum) {
                $all = ($case::class)::cases();
                $idx = array_search($case, $all, true);
                if ($idx === false) $idx = 0;
                $count = count($all);
                $bits = $count > 1 ? ((int)log($count - 1, 2)) + 1 : 1;
                $this->writeInt($bits, (int)$idx);
            }
        }

        public function enumSet(array &$set, array $allCases): void
        {
            $lookup = array_flip(array_map(fn($c) => $c instanceof \UnitEnum ? $c->name : (string)$c, $set));
            foreach ($allCases as $c) {
                $name = $c instanceof \UnitEnum ? $c->name : (string)$c;
                $b = isset($lookup[$name]);
                $this->bool($b);
            }
        }

        public function int32(int $value): void { $this->writeInt(32, $value); }
        public function byte(int $value): void { $this->writeInt(8, $value); }
        public function float(float $value): void { $this->writeInt(32, (int)$value); }
        public function string(string $value): void { /* noop */ }
    }
}

namespace pocketmine\item {

    use pocketmine\block\Block;
    use pocketmine\block\BlockBreakInfo;
    use pocketmine\block\BlockToolType;
    use pocketmine\block\BlockTypeIds;
    use pocketmine\block\VanillaBlocks;
    use pocketmine\data\bedrock\EnchantmentIdMap;
    use pocketmine\data\bedrock\item\ItemTypeDeserializeException;
    use pocketmine\data\runtime\RuntimeDataDescriber;
    use pocketmine\data\runtime\RuntimeDataWriter;
    use pocketmine\data\SavedDataLoadingException;
    use pocketmine\entity\Entity;
    use pocketmine\entity\Living;
    use pocketmine\item\enchantment\Enchantment;
    use pocketmine\item\enchantment\EnchantmentInstance;
    use pocketmine\item\enchantment\VanillaEnchantments;
    use pocketmine\math\Vector3;
    use pocketmine\nbt\LittleEndianNbtSerializer;
    use pocketmine\nbt\NBT;
    use pocketmine\nbt\NbtException;
    use pocketmine\nbt\NetworkLittleEndianNbtSerializer;
    use pocketmine\nbt\tag\CompoundTag;
    use pocketmine\nbt\tag\ListTag;
    use pocketmine\nbt\tag\StringTag;
    use pocketmine\nbt\TreeRoot;
    use pocketmine\player\Player;
    use pocketmine\utils\LegacyEnumShimTrait;
    use pocketmine\utils\Utils;
    use pocketmine\world\BlockTransaction;
    use pocketmine\world\format\io\GlobalItemDataHandlers;

    interface Releasable
    {
        public function canStartUsingItem(Player $player): bool;
    }

    enum ItemUseResult
    {
        use LegacyEnumShimTrait;

        case NONE;
        case FAIL;
        case SUCCESS;
    }

    class ItemIdentifier
    {
        public function __construct(
            private int $typeId,
            private int $meta = 0
        ) {}

        public static function fromBlock(Block $block): self
        {
            return new self(ItemTypeIds::fromBlockTypeId($block->getTypeId()));
        }

        public function getTypeId(): int
        {
            return $this->typeId;
        }

        // Bridge compatibility extensions:
        public function getId(): int
        {
            return $this->typeId;
        }

        public function getMeta(): int
        {
            return $this->meta;
        }
    }

    trait ItemEnchantmentHandlingTrait
    {
        /** @var array<int, EnchantmentInstance> */
        protected array $enchantments = [];

        public function hasEnchantments(): bool
        {
            return count($this->enchantments) > 0;
        }

        public function hasEnchantment(Enchantment $enchantment, int $level = -1): bool
        {
            $id = spl_object_id($enchantment);
            return isset($this->enchantments[$id]) && ($level === -1 || $this->enchantments[$id]->getLevel() === $level);
        }

        public function getEnchantment(Enchantment $enchantment): ?EnchantmentInstance
        {
            return $this->enchantments[spl_object_id($enchantment)] ?? null;
        }

        public function removeEnchantment(Enchantment $enchantment, int $level = -1): self
        {
            $instance = $this->getEnchantment($enchantment);
            if ($instance !== null && ($level === -1 || $instance->getLevel() === $level)) {
                unset($this->enchantments[spl_object_id($enchantment)]);
            }
            return $this;
        }

        public function removeEnchantments(): self
        {
            $this->enchantments = [];
            return $this;
        }

        public function addEnchantment(EnchantmentInstance $enchantment): self
        {
            $this->enchantments[spl_object_id($enchantment->getType())] = $enchantment;
            return $this;
        }

        /** @return EnchantmentInstance[] */
        public function getEnchantments(): array
        {
            return array_values($this->enchantments);
        }

        public function getEnchantmentLevel(Enchantment $enchantment): int
        {
            return ($instance = $this->getEnchantment($enchantment)) !== null ? $instance->getLevel() : 0;
        }
    }

    class Item implements \JsonSerializable
    {
        use ItemEnchantmentHandlingTrait;

        public const TAG_ENCH = "ench";
        private const TAG_ENCH_ID = "id";
        private const TAG_ENCH_LVL = "lvl";

        public const TAG_DISPLAY = "display";
        public const TAG_BLOCK_ENTITY_TAG = "BlockEntityTag";

        public const TAG_DISPLAY_NAME = "Name";
        public const TAG_DISPLAY_LORE = "Lore";

        public const TAG_KEEP_ON_DEATH = "minecraft:keep_on_death";

        private const TAG_CAN_PLACE_ON = "CanPlaceOn";
        private const TAG_CAN_DESTROY = "CanDestroy";

        private CompoundTag $nbt;
        protected int $count = 1;
        protected string $customName = "";
        /** @var string[] */
        protected array $lore = [];
        protected ?CompoundTag $blockEntityTag = null;
        /** @var array<string, string> */
        protected array $canPlaceOn = [];
        /** @var array<string, string> */
        protected array $canDestroy = [];
        protected bool $keepOnDeath = false;

        // PM5 standard constructor properties
        private ItemIdentifier $identifier;
        protected string $name = "Unknown";
        /** @var string[] */
        private array $enchantmentTags = [];

        // Bridge compatibility fields
        protected int $id = 0;
        protected int $meta = 0;
        private ?string $rawNbt = null;

        public function __construct(
            mixed $identifier = 0,
            mixed $name = "Unknown",
            mixed $enchantmentTagsOrCount = [],
            ?string $customName = null
        ) {
            $this->nbt = new CompoundTag();

            if ($identifier instanceof ItemIdentifier) {
                $this->identifier = $identifier;
                $this->name = is_string($name) ? $name : "Unknown";
                $this->enchantmentTags = is_array($enchantmentTagsOrCount) ? $enchantmentTagsOrCount : [];
                $this->id = $identifier->getId();
                $this->meta = $identifier->getMeta();
                if ($customName !== null && $customName !== "") {
                    $this->customName = $customName;
                }
            } else {
                $this->id = (int)$identifier;
                $this->meta = is_int($name) ? $name : 0;
                $this->identifier = new ItemIdentifier($this->id, $this->meta);
                $this->count = is_int($enchantmentTagsOrCount) ? $enchantmentTagsOrCount : 1;
                $this->name = is_string($name) ? $name : "Unknown";
                if ($customName !== null && $customName !== "") {
                    $this->customName = $customName;
                }
            }
        }

        // ---- 62 Métodos públicos normativos de PM5 ----

        public function hasCustomBlockData(): bool
        {
            return $this->blockEntityTag !== null;
        }

        public function clearCustomBlockData(): static
        {
            $this->blockEntityTag = null;
            return $this;
        }

        public function setCustomBlockData(CompoundTag $compound): static
        {
            $this->blockEntityTag = clone $compound;
            return $this;
        }

        public function getCustomBlockData(): ?CompoundTag
        {
            return $this->blockEntityTag !== null ? clone $this->blockEntityTag : null;
        }

        public function hasCustomName(): bool
        {
            return $this->customName !== "";
        }

        public function getCustomName(): string
        {
            return $this->customName;
        }

        public function setCustomName(string $name): static
        {
            Utils::checkUTF8($name);
            $this->customName = $name;
            return $this;
        }

        public function clearCustomName(): static
        {
            return $this->setCustomName("");
        }

        /** @return string[] */
        public function getLore(): array
        {
            return $this->lore;
        }

        /**
         * @param string[] $lines
         * @return $this
         */
        public function setLore(array $lines): static
        {
            foreach ($lines as $line) {
                if (!is_string($line)) {
                    throw new \TypeError("Expected string[], but found " . gettype($line) . " in given array");
                }
                Utils::checkUTF8($line);
            }
            $this->lore = array_values($lines);
            return $this;
        }

        /** @return string[] */
        public function getCanPlaceOn(): array
        {
            return array_values($this->canPlaceOn);
        }

        /** @param string[] $canPlaceOn */
        public function setCanPlaceOn(array $canPlaceOn): void
        {
            $this->canPlaceOn = [];
            foreach ($canPlaceOn as $value) {
                $this->canPlaceOn[$value] = $value;
            }
        }

        /** @return string[] */
        public function getCanDestroy(): array
        {
            return array_values($this->canDestroy);
        }

        /** @param string[] $canDestroy */
        public function setCanDestroy(array $canDestroy): void
        {
            $this->canDestroy = [];
            foreach ($canDestroy as $value) {
                $this->canDestroy[$value] = $value;
            }
        }

        public function keepOnDeath(): bool
        {
            return $this->keepOnDeath;
        }

        public function setKeepOnDeath(bool $keepOnDeath): void
        {
            $this->keepOnDeath = $keepOnDeath;
        }

        public function hasNamedTag(): bool
        {
            return $this->getNamedTag()->count() > 0;
        }

        public function getNamedTag(): CompoundTag
        {
            $this->serializeCompoundTag($this->nbt);
            return $this->nbt;
        }

        public function setNamedTag(CompoundTag $tag): static
        {
            if ($tag->count() === 0) {
                return $this->clearNamedTag();
            }
            $this->nbt = clone $tag;
            $this->deserializeCompoundTag($this->nbt);
            return $this;
        }

        public function clearNamedTag(): static
        {
            $this->nbt = new CompoundTag();
            $this->deserializeCompoundTag($this->nbt);
            return $this;
        }

        public function getCount(): int
        {
            return $this->count;
        }

        public function setCount(int $count): static
        {
            $this->count = $count;
            return $this;
        }

        public function pop(int $count = 1): static
        {
            if ($count > $this->count) {
                throw new \InvalidArgumentException("Cannot pop $count items from a stack of $this->count");
            }
            $item = clone $this;
            $item->count = $count;
            $this->count -= $count;
            return $item;
        }

        public function isNull(): bool
        {
            return $this->count <= 0 || $this->getId() === 0;
        }

        final public function getName(): string
        {
            return $this->hasCustomName() ? $this->getCustomName() : $this->getVanillaName();
        }

        public function getVanillaName(): string
        {
            return $this->name;
        }

        /** @return string[] */
        public function getEnchantmentTags(): array
        {
            return $this->enchantmentTags;
        }

        public function getEnchantability(): int
        {
            return 1;
        }

        final public function canBePlaced(): bool
        {
            return $this->getBlock()->canBePlaced();
        }

        protected final function tryPlacementTransaction(Block $blockPlace, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player): ?BlockTransaction
        {
            $position = $blockReplace->getPosition();
            $blockPlace->position($position->getWorld(), $position->getFloorX(), $position->getFloorY(), $position->getFloorZ());
            if (!$blockPlace->canBePlacedAt($blockReplace, $clickVector, $face, $blockReplace->getPosition()->equals($blockClicked->getPosition()))) {
                return null;
            }
            $transaction = new BlockTransaction($position->getWorld());
            return $blockPlace->place($transaction, $this, $blockReplace, $blockClicked, $face, $clickVector, $player) ? $transaction : null;
        }

        public function getPlacementTransaction(Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, ?Player $player = null): ?BlockTransaction
        {
            return $this->tryPlacementTransaction($this->getBlock($face), $blockReplace, $blockClicked, $face, $clickVector, $player);
        }

        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::AIR();
        }

        final public function getTypeId(): int
        {
            return $this->identifier->getTypeId();
        }

        final public function getStateId(): int
        {
            return morton2d_encode($this->identifier->getTypeId(), $this->computeStateData());
        }

        private function computeStateData(): int
        {
            $writer = new RuntimeDataWriter(16);
            $this->describeState($writer);
            return $writer->getValue();
        }

        protected function describeState(RuntimeDataDescriber $w): void
        {
            // NOOP by default
        }

        public function getMaxStackSize(): int
        {
            return 64;
        }

        public function getFuelTime(): int
        {
            return 0;
        }

        public function getFuelResidue(): Item
        {
            $item = clone $this;
            $item->pop();
            return $item;
        }

        public function isFireProof(): bool
        {
            return false;
        }

        public function getAttackPoints(): int
        {
            return 1;
        }

        public function getDefensePoints(): int
        {
            return 0;
        }

        public function getBlockToolType(): int
        {
            return BlockToolType::NONE;
        }

        public function getBlockToolHarvestLevel(): int
        {
            return 0;
        }

        public function getMiningEfficiency(bool $isCorrectTool): float
        {
            return 1.0;
        }

        public function onInteractBlock(Player $player, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, array &$returnedItems): ItemUseResult
        {
            return ItemUseResult::NONE;
        }

        public function onClickAir(Player $player, Vector3 $directionVector, array &$returnedItems): ItemUseResult
        {
            return ItemUseResult::NONE;
        }

        public function onReleaseUsing(Player $player, array &$returnedItems): ItemUseResult
        {
            return ItemUseResult::NONE;
        }

        public function onDestroyBlock(Block $block, array &$returnedItems): bool
        {
            return false;
        }

        public function onAttackEntity(Entity $victim, array &$returnedItems): bool
        {
            return false;
        }

        public function onTickWorn(Living $entity): bool
        {
            return false;
        }

        public function onInteractEntity(Player $player, Entity $entity, Vector3 $clickVector): bool
        {
            return false;
        }

        public function getCooldownTicks(): int
        {
            return 0;
        }

        public function getCooldownTag(): ?string
        {
            return null;
        }

        final public function equals(Item $item, bool $checkDamage = true, bool $checkCompound = true): bool
        {
            return $this->getStateId() === $item->getStateId() &&
                (!$checkCompound || $this->getNamedTag()->equals($item->getNamedTag()));
        }

        final public function canStackWith(Item $other): bool
        {
            return $this->equals($other, true, true);
        }

        final public function equalsExact(Item $other): bool
        {
            return $this->canStackWith($other) && $this->count === $other->count;
        }

        final public function __toString(): string
        {
            return 'Item(' . $this->getName() . ' x' . $this->count . ')';
        }

        public function jsonSerialize(): array
        {
            throw new \LogicException("json_encode()ing Item instances is no longer supported. Make your own method to convert the item to an array or stdClass.");
        }

        final public static function legacyJsonDeserialize(array $data): Item
        {
            $nbt = "";
            if (isset($data["nbt"])) {
                $nbt = (string)$data["nbt"];
            } elseif (isset($data["nbt_hex"])) {
                $nbt = (string)hex2bin($data["nbt_hex"]);
            } elseif (isset($data["nbt_b64"])) {
                $nbt = (string)base64_decode($data["nbt_b64"], true);
            }

            return ItemFactory::get(
                (int)($data["id"] ?? 0),
                (int)($data["damage"] ?? 0),
                (int)($data["count"] ?? 1)
            );
        }

        public function nbtSerialize(int $slot = -1): CompoundTag
        {
            $tag = clone $this->getNamedTag();
            $tag->setString("Name", \pocketmine\world\BlockFactory::networkNameOf($this->getId()));
            $tag->setByte("Count", $this->count);
            $tag->setShort("Damage", $this instanceof Durable ? $this->getDamage() : $this->meta);
            if ($slot !== -1) {
                $tag->setByte("Slot", $slot);
            }
            return $tag;
        }

        public static function nbtDeserialize(CompoundTag $tag): Item
        {
            $name = $tag->getString("Name", "");
            $count = $tag->getByte("Count", 1);
            $damage = $tag->getShort("Damage", 0);
            $item = ItemFactory::fromString($name);
            $item->setCount($count);
            if ($item instanceof Durable) {
                $item->setDamage($damage);
            }
            $cleanTag = clone $tag;
            $cleanTag->removeTag("Name");
            $cleanTag->removeTag("Count");
            $cleanTag->removeTag("Damage");
            $cleanTag->removeTag("Slot");
            if ($cleanTag->count() > 0) {
                $item->setNamedTag($cleanTag);
            }
            return $item;
        }

        public static function safeNbtDeserialize(CompoundTag $tag, string $errorLogContext, ?\Logger $logger = null): Item
        {
            try {
                return self::nbtDeserialize($tag);
            } catch (\Throwable $e) {
                return VanillaItems::AIR();
            }
        }

        public function __clone()
        {
            $this->nbt = clone $this->nbt;
            if ($this->blockEntityTag !== null) {
                $this->blockEntityTag = clone $this->blockEntityTag;
            }
        }

        // ---- Extensiones bridge-only y soporte NBT ----

        public function getId(): int
        {
            if ($this instanceof ItemBlock) {
                return \pocketmine\world\BlockFactory::idFromName($this->getBlock()->getName());
            }
            $netId = ItemFactory::resolveNetworkId($this->getTypeId());
            if ($netId !== null) {
                return $netId;
            }
            return $this->id !== 0 ? $this->id : $this->identifier->getId();
        }

        public function getMeta(): int
        {
            return $this->meta;
        }

        public function setNbtBytes(string $bytes): void
        {
            $this->rawNbt = $bytes === '' ? null : $bytes;
            if ($bytes !== '') {
                try {
                    $root = (new NetworkLittleEndianNbtSerializer())->read($bytes);
                    $tag = $root->getTag();
                    if ($tag instanceof CompoundTag) {
                        $this->setNamedTag($tag);
                    }
                } catch (\Throwable) {
                    // Raw bytes preserved
                }
            } else {
                $this->clearNamedTag();
            }
        }

        public function getNbtBytes(): string
        {
            $tag = $this->getNamedTag();
            if ($tag->count() === 0) {
                return $this->rawNbt ?? '';
            }
            try {
                return (new NetworkLittleEndianNbtSerializer())->write(new TreeRoot($tag));
            } catch (\Throwable) {
                return $this->rawNbt ?? '';
            }
        }

        public function getNBT(): array
        {
            return $this->getNamedTag()->getValue();
        }

        public function setNBT(array $tags): static
        {
            $tag = CompoundTag::create();
            foreach ($tags as $k => $v) {
                if ($v instanceof \pocketmine\nbt\tag\Tag) {
                    $tag->setTag($k, $v);
                }
            }
            $this->setNamedTag($tag);
            return $this;
        }

        public function getCompoundTag(): ?CompoundTag
        {
            $tag = $this->getNamedTag();
            return $tag->count() === 0 ? null : $tag;
        }

        public function setCompoundTag(?CompoundTag $tag = null): void
        {
            if ($tag === null) {
                $this->clearNamedTag();
            } else {
                $this->setNamedTag($tag);
            }
        }

        public function clearEnchantments(): void
        {
            $this->removeEnchantments();
        }

        protected function deserializeCompoundTag(CompoundTag $tag): void
        {
            $this->customName = "";
            $this->lore = [];

            $display = $tag->getTag(self::TAG_DISPLAY);
            if ($display instanceof CompoundTag) {
                if ($display->hasTag(self::TAG_DISPLAY_NAME)) {
                    $this->customName = $display->getString(self::TAG_DISPLAY_NAME, $this->customName);
                }
                $lore = $display->getTag(self::TAG_DISPLAY_LORE);
                if ($lore instanceof ListTag) {
                    foreach ($lore as $t) {
                        if ($t instanceof StringTag) {
                            $this->lore[] = $t->getValue();
                        }
                    }
                }
            }

            $this->removeEnchantments();
            $enchantments = $tag->getTag(self::TAG_ENCH);
            if ($enchantments instanceof ListTag) {
                VanillaEnchantments::getAll();
                foreach ($enchantments as $enchantment) {
                    if ($enchantment instanceof CompoundTag) {
                        $magicNumber = $enchantment->getShort(self::TAG_ENCH_ID, -1);
                        $level = $enchantment->getShort(self::TAG_ENCH_LVL, 0);
                        if ($level <= 0) {
                            continue;
                        }
                        $type = EnchantmentIdMap::getInstance()->fromId($magicNumber);
                        if ($type !== null) {
                            $this->addEnchantment(new EnchantmentInstance($type, $level));
                        }
                    }
                }
            }

            $bet = $tag->getTag(self::TAG_BLOCK_ENTITY_TAG);
            $this->blockEntityTag = $bet instanceof CompoundTag ? clone $bet : null;

            $this->canPlaceOn = [];
            $canPlaceOn = $tag->getTag(self::TAG_CAN_PLACE_ON);
            if ($canPlaceOn instanceof ListTag) {
                foreach ($canPlaceOn as $entry) {
                    if ($entry instanceof StringTag) {
                        $this->canPlaceOn[$entry->getValue()] = $entry->getValue();
                    }
                }
            }

            $this->canDestroy = [];
            $canDestroy = $tag->getTag(self::TAG_CAN_DESTROY);
            if ($canDestroy instanceof ListTag) {
                foreach ($canDestroy as $entry) {
                    if ($entry instanceof StringTag) {
                        $this->canDestroy[$entry->getValue()] = $entry->getValue();
                    }
                }
            }

            $this->keepOnDeath = $tag->getByte(self::TAG_KEEP_ON_DEATH, 0) !== 0;
        }

        protected function serializeCompoundTag(CompoundTag $tag): void
        {
            $display = $tag->getTag(self::TAG_DISPLAY);
            if (!$display instanceof CompoundTag) {
                $display = null;
            }

            if ($this->customName !== "") {
                $display ??= new CompoundTag();
                $display->setString(self::TAG_DISPLAY_NAME, $this->customName);
            } else {
                $display?->removeTag(self::TAG_DISPLAY_NAME);
            }

            if (count($this->lore) > 0) {
                $loreTag = new ListTag([], NBT::TAG_String);
                foreach ($this->lore as $line) {
                    $loreTag->push(new StringTag($line));
                }
                $display ??= new CompoundTag();
                $display->setTag(self::TAG_DISPLAY_LORE, $loreTag);
            } else {
                $display?->removeTag(self::TAG_DISPLAY_LORE);
            }

            if ($display !== null && $display->count() > 0) {
                $tag->setTag(self::TAG_DISPLAY, $display);
            } else {
                $tag->removeTag(self::TAG_DISPLAY);
            }

            if (count($this->enchantments) > 0) {
                $ench = new ListTag([], NBT::TAG_Compound);
                $enchantmentIdMap = EnchantmentIdMap::getInstance();
                foreach ($this->enchantments as $enchantmentInstance) {
                    $id = $enchantmentIdMap->toId($enchantmentInstance->getType());
                    if ($id !== null) {
                        $ench->push(CompoundTag::create()
                            ->setShort(self::TAG_ENCH_ID, $id)
                            ->setShort(self::TAG_ENCH_LVL, $enchantmentInstance->getLevel())
                        );
                    }
                }
                if ($ench->count() > 0) {
                    $tag->setTag(self::TAG_ENCH, $ench);
                } else {
                    $tag->removeTag(self::TAG_ENCH);
                }
            } else {
                $tag->removeTag(self::TAG_ENCH);
            }

            if ($this->blockEntityTag !== null) {
                $tag->setTag(self::TAG_BLOCK_ENTITY_TAG, clone $this->blockEntityTag);
            } else {
                $tag->removeTag(self::TAG_BLOCK_ENTITY_TAG);
            }

            if (count($this->canPlaceOn) > 0) {
                $canPlaceOn = new ListTag([], NBT::TAG_String);
                foreach ($this->canPlaceOn as $item) {
                    $canPlaceOn->push(new StringTag($item));
                }
                $tag->setTag(self::TAG_CAN_PLACE_ON, $canPlaceOn);
            } else {
                $tag->removeTag(self::TAG_CAN_PLACE_ON);
            }

            if (count($this->canDestroy) > 0) {
                $canDestroy = new ListTag([], NBT::TAG_String);
                foreach ($this->canDestroy as $item) {
                    $canDestroy->push(new StringTag($item));
                }
                $tag->setTag(self::TAG_CAN_DESTROY, $canDestroy);
            } else {
                $tag->removeTag(self::TAG_CAN_DESTROY);
            }

            if ($this->keepOnDeath) {
                $tag->setByte(self::TAG_KEEP_ON_DEATH, 1);
            } else {
                $tag->removeTag(self::TAG_KEEP_ON_DEATH);
            }
        }

        public static function fromArray(array $data): Item
        {
            $id = $data['id'] ?? 0;
            $meta = (int)($data['meta'] ?? 0);
            $count = (int)($data['count'] ?? 1);

            $item = ItemFactory::get($id, $meta, $count);

            if (isset($data['custom_name']) && is_string($data['custom_name']) && $data['custom_name'] !== '') {
                $item->setCustomName($data['custom_name']);
            }
            if (isset($data['nbt']) && is_string($data['nbt']) && $data['nbt'] !== '') {
                $bytes = @hex2bin($data['nbt']);
                if ($bytes !== false) {
                    $item->setNbtBytes($bytes);
                }
            }
            if (isset($data['damage']) && $item instanceof Durable) {
                $item->setDamage((int)$data['damage']);
            }
            if (isset($data['enchantments']) && is_array($data['enchantments'])) {
                VanillaEnchantments::getAll();
                foreach ($data['enchantments'] as $enchId => $level) {
                    if (is_array($level)) {
                        $enchId = (int)($level['type'] ?? 0);
                        $level = (int)($level['level'] ?? 1);
                    }
                    $type = EnchantmentIdMap::getInstance()->fromId((int)$enchId);
                    if ($type !== null) {
                        $item->addEnchantment(new EnchantmentInstance($type, max(1, (int)$level)));
                    }
                }
            }
            return $item;
        }

        public function toArray(): array
        {
            $out = [
                'id' => $this->getId(),
                'meta' => $this->getMeta(),
                'count' => $this->getCount(),
            ];
            if ($this->hasCustomName()) {
                $out['custom_name'] = $this->getCustomName();
            }
            $nbt = $this->getNbtBytes();
            if ($nbt !== '') {
                $out['nbt'] = bin2hex($nbt);
            }
            if ($this instanceof Durable && $this->getDamage() > 0) {
                $out['damage'] = $this->getDamage();
            }
            if ($this->hasEnchantments()) {
                $out['enchantments'] = [];
                $enchantmentIdMap = EnchantmentIdMap::getInstance();
                foreach ($this->getEnchantments() as $inst) {
                    $typeId = $enchantmentIdMap->toId($inst->getType());
                    if ($typeId !== null) {
                        $out['enchantments'][$typeId] = $inst->getLevel();
                    }
                }
            }
            return $out;
        }
    }

    final class ItemBlock extends Item
    {
        public function __construct(
            private Block $block
        ) {
            parent::__construct(ItemIdentifier::fromBlock($block), $block->getName(), $block->getEnchantmentTags());
        }

        protected function describeState(RuntimeDataDescriber $w): void
        {
            $this->block->describeBlockItemState($w);
        }

        public function getBlock(?int $clickedFace = null): Block
        {
            return clone $this->block;
        }

        public function getFuelTime(): int
        {
            return $this->block->getFuelTime();
        }

        public function isFireProof(): bool
        {
            return $this->block->isFireProofAsItem();
        }

        public function getMaxStackSize(): int
        {
            return $this->block->getMaxStackSize();
        }

        public function isNull(): bool
        {
            return parent::isNull() || $this->block->getTypeId() === BlockTypeIds::AIR;
        }
    }

    abstract class Durable extends Item
    {
        protected int $damage = 0;
        private bool $unbreakable = false;

        public function isUnbreakable(): bool
        {
            return $this->unbreakable;
        }

        public function setUnbreakable(bool $value = true): static
        {
            $this->unbreakable = $value;
            return $this;
        }

        public function applyDamage(int $amount): bool
        {
            if ($this->isUnbreakable() || $this->isBroken()) {
                return false;
            }
            $amount -= $this->getUnbreakingDamageReduction($amount);
            $this->damage = min($this->damage + $amount, $this->getMaxDurability());
            if ($this->isBroken()) {
                $this->onBroken();
            }
            return true;
        }

        public function getDamage(): int
        {
            return $this->damage;
        }

        public function setDamage(int $damage): static
        {
            if ($damage < 0 || $damage > $this->getMaxDurability()) {
                throw new \InvalidArgumentException("Damage must be in range 0 - " . $this->getMaxDurability());
            }
            $this->damage = $damage;
            return $this;
        }

        protected function getUnbreakingDamageReduction(int $amount): int
        {
            $unbreakingLevel = $this->getEnchantmentLevel(VanillaEnchantments::UNBREAKING());
            if ($unbreakingLevel <= 0) {
                return 0;
            }
            $negated = 0;
            for ($i = 0; $i < $amount; $i++) {
                if (mt_rand(1, $unbreakingLevel + 1) > 1) {
                    $negated++;
                }
            }
            return $negated;
        }

        public function getMaxDurability(): int
        {
            return 0;
        }

        public function getDurability(): int
        {
            $max = $this->getMaxDurability();
            return $max === 0 ? 0 : max(0, $max - $this->damage);
        }

        public function isDamaged(): bool
        {
            return $this->damage > 0;
        }

        public function isBroken(): bool
        {
            return $this->damage >= $this->getMaxDurability();
        }

        public function onBroken(): void
        {
            $this->pop();
        }

        public function repair(): void
        {
            $this->damage = 0;
        }

        protected function deserializeCompoundTag(CompoundTag $tag): void
        {
            parent::deserializeCompoundTag($tag);
            $this->unbreakable = $tag->getByte("Unbreakable", 0) !== 0;
            $damage = $tag->getInt("Damage", $this->damage);
            if ($damage !== $this->damage && $damage >= 0 && $damage <= $this->getMaxDurability()) {
                $this->setDamage($damage);
            }
        }

        protected function serializeCompoundTag(CompoundTag $tag): void
        {
            parent::serializeCompoundTag($tag);
            $this->unbreakable ? $tag->setByte("Unbreakable", 1) : $tag->removeTag("Unbreakable");
            $this->damage !== 0 ? $tag->setInt("Damage", $this->damage) : $tag->removeTag("Damage");
        }
    }

    abstract class Tool extends Durable
    {
        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function getMiningEfficiency(bool $isCorrectTool): float
        {
            $efficiency = 1.0;
            if ($isCorrectTool) {
                $efficiency = $this->getBaseMiningEfficiency();
                if (($enchantmentLevel = $this->getEnchantmentLevel(VanillaEnchantments::EFFICIENCY())) > 0) {
                    $efficiency += ($enchantmentLevel ** 2 + 1);
                }
            }
            return (float)$efficiency;
        }

        protected function getBaseMiningEfficiency(): float
        {
            return 1.0;
        }
    }

    abstract class TieredTool extends Tool
    {
        protected ToolTier $tier;

        public function __construct(ItemIdentifier $identifier, string $name, ToolTier $tier, array $enchantmentTags = [])
        {
            parent::__construct($identifier, $name, $enchantmentTags);
            $this->tier = $tier;
        }

        public function getMaxDurability(): int
        {
            return $this->tier->getMaxDurability();
        }

        public function getTier(): ToolTier
        {
            return $this->tier;
        }

        protected function getBaseMiningEfficiency(): float
        {
            return (float)$this->tier->getBaseEfficiency();
        }

        public function getEnchantability(): int
        {
            return $this->tier->getEnchantability();
        }

        public function getFuelTime(): int
        {
            if ($this->tier === ToolTier::WOOD) {
                return 200;
            }
            return 0;
        }

        public function isFireProof(): bool
        {
            return $this->tier === ToolTier::NETHERITE;
        }
    }

    enum ToolTier
    {
        use LegacyEnumShimTrait;

        case WOOD;
        case GOLD;
        case STONE;
        case COPPER;
        case IRON;
        case DIAMOND;
        case NETHERITE;

        private static function meta(int $harvestLevel, int $maxDurability, int $baseAttackPoints, int $baseEfficiency, int $enchantability): array
        {
            return [$harvestLevel, $maxDurability, $baseAttackPoints, $baseEfficiency, $enchantability];
        }

        private function getMetadata(): array
        {
            return match ($this) {
                self::WOOD => self::meta(1, 60, 5, 2, 15),
                self::GOLD => self::meta(2, 33, 5, 12, 22),
                self::STONE => self::meta(3, 132, 6, 4, 5),
                self::COPPER => self::meta(3, 191, 6, 5, 13),
                self::IRON => self::meta(4, 251, 7, 6, 14),
                self::DIAMOND => self::meta(5, 1562, 8, 8, 10),
                self::NETHERITE => self::meta(6, 2032, 9, 9, 15)
            };
        }

        public function getHarvestLevel(): int
        {
            return $this->getMetadata()[0];
        }

        public function getMaxDurability(): int
        {
            return $this->getMetadata()[1];
        }

        public function getBaseAttackPoints(): int
        {
            return $this->getMetadata()[2];
        }

        public function getBaseEfficiency(): int
        {
            return $this->getMetadata()[3];
        }

        public function getEnchantability(): int
        {
            return $this->getMetadata()[4];
        }
    }

    abstract class SpawnEgg extends Item
    {
        abstract protected function createEntity(\pocketmine\world\World $world, \pocketmine\math\Vector3 $pos, float $yaw, float $pitch): \pocketmine\entity\Entity;
    }
}

namespace pocketmine\world\format\io {
    class GlobalItemDataHandlers
    {
        private static ?ItemSerializer $serializer = null;
        private static ?ItemDeserializer $deserializer = null;

        public static function getSerializer(): ItemSerializer
        {
            return self::$serializer ??= new ItemSerializer();
        }

        public static function getDeserializer(): ItemDeserializer
        {
            return self::$deserializer ??= new ItemDeserializer();
        }
    }

    class ItemSerializer
    {
        public function map(\pocketmine\item\Item $item, \Closure $serializer): void {}

        public function serializeStack(\pocketmine\item\Item $item, ?int $slot = null): \pocketmine\data\bedrock\item\SavedItemStackData
        {
            return new \pocketmine\data\bedrock\item\SavedItemStackData(
                \pocketmine\world\BlockFactory::networkNameOf($item->getId()),
                $item instanceof \pocketmine\item\Durable ? $item->getDamage() : $item->getMeta(),
                $item->getCount(),
                $slot,
                $item->getNamedTag()
            );
        }
    }

    class ItemDeserializer
    {
        public function map(string $id, \Closure $deserializer): void {}

        public function deserializeStack(\pocketmine\data\bedrock\item\SavedItemStackData $data): \pocketmine\item\Item
        {
            $item = \pocketmine\item\ItemFactory::fromString($data->getName());
            $item->setCount($data->getCount());
            if ($item instanceof \pocketmine\item\Durable) {
                $item->setDamage($data->getMeta());
            }
            if ($data->getTag() instanceof \pocketmine\nbt\tag\CompoundTag) {
                $item->setNamedTag($data->getTag());
            }
            return $item;
        }
    }
}

namespace pocketmine\data\bedrock\item {
    class SavedItemData
    {
        public function __construct(
            private string $name,
            private int $meta = 0,
            private mixed $tag = null
        ) {}

        public function getName(): string { return $this->name; }
        public function getMeta(): int { return $this->meta; }
        public function getTag(): mixed { return $this->tag; }
    }

    class SavedItemStackData extends SavedItemData
    {
        public function __construct(
            string $name,
            int $meta = 0,
            private int $count = 1,
            private ?int $slot = null,
            mixed $tag = null
        ) {
            parent::__construct($name, $meta, $tag);
        }

        public function getCount(): int { return $this->count; }
        public function getSlot(): ?int { return $this->slot; }

        public function toNbt(): \pocketmine\nbt\tag\CompoundTag
        {
            $tag = \pocketmine\nbt\tag\CompoundTag::create()
                ->setString("Name", $this->getName())
                ->setByte("Count", $this->count)
                ->setShort("Damage", $this->getMeta());
            if ($this->slot !== null) {
                $tag->setByte("Slot", $this->slot);
            }
            if ($this->getTag() instanceof \pocketmine\nbt\tag\CompoundTag && $this->getTag()->count() > 0) {
                $tag->setTag("tag", clone $this->getTag());
            }
            return $tag;
        }
    }
}
