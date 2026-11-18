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

namespace pocketmine\inventory {

    /**
     * Note: Full ArmorInventory is NOT currently ported to the bridge.
     * Slot constants (SLOT_HEAD = 0, SLOT_CHEST = 1, SLOT_LEGS = 2, SLOT_FEET = 3)
     * are provided for compatibility with PocketMine-MP 5.
     */
    if (!class_exists(ArmorInventory::class, false)) {
        class ArmorInventory
        {
            public const SLOT_HEAD = 0;
            public const SLOT_CHEST = 1;
            public const SLOT_LEGS = 2;
            public const SLOT_FEET = 3;
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

    if (!class_exists(ArmorEquipGenericSound::class, false)) {
        class ArmorEquipGenericSound extends SoundBase {}
    }
    if (!class_exists(ArmorEquipChainSound::class, false)) {
        class ArmorEquipChainSound extends SoundBase {}
    }
    if (!class_exists(ArmorEquipCopperSound::class, false)) {
        class ArmorEquipCopperSound extends SoundBase {}
    }
    if (!class_exists(ArmorEquipDiamondSound::class, false)) {
        class ArmorEquipDiamondSound extends SoundBase {}
    }
    if (!class_exists(ArmorEquipGoldSound::class, false)) {
        class ArmorEquipGoldSound extends SoundBase {}
    }
    if (!class_exists(ArmorEquipIronSound::class, false)) {
        class ArmorEquipIronSound extends SoundBase {}
    }
    if (!class_exists(ArmorEquipLeatherSound::class, false)) {
        class ArmorEquipLeatherSound extends SoundBase {}
    }
    if (!class_exists(ArmorEquipNetheriteSound::class, false)) {
        class ArmorEquipNetheriteSound extends SoundBase {}
    }
}

namespace pocketmine\item\enchantment {

    use pocketmine\event\entity\EntityDamageEvent;
    use pocketmine\lang\Translatable;
    use function array_fill_keys;
    use function floor;

    if (!class_exists(ProtectionEnchantment::class, false)) {
        class ProtectionEnchantment extends Enchantment
        {
            protected float $typeModifier;
            /** @var array<int, true>|null */
            protected ?array $applicableDamageTypes = null;

            public function __construct(
                Translatable|string $name,
                int $rarity,
                int $primaryItemFlags,
                int $secondaryItemFlags,
                int $maxLevel,
                float $typeModifier,
                ?array $applicableDamageTypes,
                ?\Closure $minEnchantingPower = null,
                int $enchantingPowerRange = 50
            ) {
                parent::__construct($name, $rarity, $primaryItemFlags, $secondaryItemFlags, $maxLevel, $minEnchantingPower, $enchantingPowerRange);
                $this->typeModifier = $typeModifier;
                if ($applicableDamageTypes !== null) {
                    $this->applicableDamageTypes = array_fill_keys($applicableDamageTypes, true);
                }
            }

            public function getTypeModifier(): float
            {
                return $this->typeModifier;
            }

            public function getProtectionFactor(int $level): int
            {
                return (int) floor((6 + $level ** 2) * $this->typeModifier / 3);
            }

            public function isApplicable(EntityDamageEvent $event): bool
            {
                return $this->applicableDamageTypes === null || isset($this->applicableDamageTypes[$event->getCause()]);
            }
        }
    }
}

namespace pocketmine\item {

    use pocketmine\color\Color;
    use pocketmine\entity\effect\EffectInstance;
    use pocketmine\entity\effect\VanillaEffects;
    use pocketmine\entity\Human;
    use pocketmine\entity\Living;
    use pocketmine\event\entity\EntityDamageEvent;
    use pocketmine\inventory\ArmorInventory;
    use pocketmine\item\enchantment\ProtectionEnchantment;
    use pocketmine\item\enchantment\VanillaEnchantments;
    use pocketmine\math\Vector3;
    use pocketmine\nbt\tag\CompoundTag;
    use pocketmine\nbt\tag\IntTag;
    use pocketmine\player\Player;
    use pocketmine\utils\Binary;
    use pocketmine\utils\RegistrySource;
    use pocketmine\utils\Utils;
    use pocketmine\world\sound\ArmorEquipChainSound;
    use pocketmine\world\sound\ArmorEquipCopperSound;
    use pocketmine\world\sound\ArmorEquipDiamondSound;
    use pocketmine\world\sound\ArmorEquipGenericSound;
    use pocketmine\world\sound\ArmorEquipGoldSound;
    use pocketmine\world\sound\ArmorEquipIronSound;
    use pocketmine\world\sound\ArmorEquipLeatherSound;
    use pocketmine\world\sound\ArmorEquipNetheriteSound;
    use pocketmine\world\sound\Sound;
    use function mt_rand;
    use function mt_getrandmax;

    class ArmorMaterial
    {
        public function __construct(
            private readonly int $enchantability,
            private readonly ?Sound $equipSound = null
        ) {}

        /**
         * Returns the value that defines how enchantable the item is.
         *
         * The higher an item's enchantability is, the more likely it will be to gain high-level enchantments
         * or multiple enchantments upon being enchanted in an enchanting table.
         */
        public function getEnchantability(): int
        {
            return $this->enchantability;
        }

        /**
         * Returns the sound that plays when equipping the armor
         */
        public function getEquipSound(): ?Sound
        {
            return $this->equipSound;
        }
    }

    /**
     * Allows getting any vanilla armor material implemented by PocketMine-MP.
     *
     * @method static ArmorMaterial LEATHER()
     * @method static ArmorMaterial CHAINMAIL()
     * @method static ArmorMaterial COPPER()
     * @method static ArmorMaterial IRON()
     * @method static ArmorMaterial TURTLE()
     * @method static ArmorMaterial GOLD()
     * @method static ArmorMaterial DIAMOND()
     * @method static ArmorMaterial NETHERITE()
     */
    final class VanillaArmorMaterials
    {
        use \pocketmine\utils\RegistryTrait;

        protected static function setup(): void
        {
            self::_registryRegister("leather", new ArmorMaterial(15, new ArmorEquipLeatherSound()));
            self::_registryRegister("chainmail", new ArmorMaterial(12, new ArmorEquipChainSound()));
            self::_registryRegister("copper", new ArmorMaterial(8, new ArmorEquipCopperSound()));
            self::_registryRegister("iron", new ArmorMaterial(9, new ArmorEquipIronSound()));
            self::_registryRegister("turtle", new ArmorMaterial(9, new ArmorEquipGenericSound()));
            self::_registryRegister("gold", new ArmorMaterial(25, new ArmorEquipGoldSound()));
            self::_registryRegister("diamond", new ArmorMaterial(10, new ArmorEquipDiamondSound()));
            self::_registryRegister("netherite", new ArmorMaterial(15, new ArmorEquipNetheriteSound()));
        }

        /**
         * @return array<string, ArmorMaterial>
         */
        public static function getAll(): array
        {
            return self::_registryGetAll();
        }
    }

    /**
     * @internal
     * @phpstan-extends RegistrySource<ArmorMaterial>
     */
    final class VanillaArmorMaterialsInputs extends RegistrySource
    {
        public function getTargetClassName(): string
        {
            return "VanillaArmorMaterials";
        }

        /**
         * @return string[]
         */
        public function getTargetClassDocComment(): array
        {
            return ["Allows getting any vanilla armor material implemented by PocketMine-MP"];
        }

        protected function register(string $name, ArmorMaterial $armorMaterial): void
        {
            if (method_exists(parent::class, 'registerValue')) {
                parent::registerValue($name, $armorMaterial);
            }
        }

        protected function setup(): void
        {
            $this->register("leather", new ArmorMaterial(15, new ArmorEquipLeatherSound()));
            $this->register("chainmail", new ArmorMaterial(12, new ArmorEquipChainSound()));
            $this->register("copper", new ArmorMaterial(8, new ArmorEquipCopperSound()));
            $this->register("iron", new ArmorMaterial(9, new ArmorEquipIronSound()));
            $this->register("turtle", new ArmorMaterial(9, new ArmorEquipGenericSound()));
            $this->register("gold", new ArmorMaterial(25, new ArmorEquipGoldSound()));
            $this->register("diamond", new ArmorMaterial(10, new ArmorEquipDiamondSound()));
            $this->register("netherite", new ArmorMaterial(15, new ArmorEquipNetheriteSound()));
        }
    }

    class ArmorTypeInfo
    {
        public const SLOT_HEAD = 0;
        public const SLOT_CHEST = 1;
        public const SLOT_LEGS = 2;
        public const SLOT_FEET = 3;

        private ArmorMaterial $material;

        public function __construct(
            private int $defensePoints,
            private int $maxDurability,
            private int $armorSlot,
            private int $toughness = 0,
            private bool $fireProof = false,
            ?ArmorMaterial $material = null
        ) {
            $this->material = $material ?? VanillaArmorMaterials::LEATHER();
        }

        public function getDefensePoints(): int
        {
            return $this->defensePoints;
        }

        public function getMaxDurability(): int
        {
            return $this->maxDurability;
        }

        /**
         * Returns the armor slot where this armor can be equipped (0-3).
         * Note: ArmorInventory is not ported to the bridge; numeric constants
         * SLOT_HEAD (0), SLOT_CHEST (1), SLOT_LEGS (2), SLOT_FEET (3) are used.
         *
         * @see ArmorInventory
         */
        public function getArmorSlot(): int
        {
            return $this->armorSlot;
        }

        public function getToughness(): int
        {
            return $this->toughness;
        }

        public function isFireProof(): bool
        {
            return $this->fireProof;
        }

        public function getMaterial(): ArmorMaterial
        {
            return $this->material;
        }
    }

    class Armor extends Durable
    {
        public const TAG_CUSTOM_COLOR = "customColor"; //TAG_Int

        public const SLOT_HEAD = 0;
        public const SLOT_CHEST = 1;
        public const SLOT_LEGS = 2;
        public const SLOT_FEET = 3;

        private ArmorTypeInfo $armorInfo;

        protected ?Color $customColor = null;

        /**
         * @param string[] $enchantmentTags
         */
        public function __construct(ItemIdentifier $identifier, string $name, ArmorTypeInfo $info, array $enchantmentTags = [])
        {
            parent::__construct($identifier, $name, $enchantmentTags);
            $this->armorInfo = $info;
        }

        public function getMaxDurability(): int
        {
            return $this->armorInfo->getMaxDurability();
        }

        public function getDefensePoints(): int
        {
            return $this->armorInfo->getDefensePoints();
        }

        /**
         * Returns the armor slot where this armor can be equipped (0-3).
         * Note: ArmorInventory is not ported to the bridge; numeric constants
         * SLOT_HEAD (0), SLOT_CHEST (1), SLOT_LEGS (2), SLOT_FEET (3) are used.
         *
         * @see ArmorInventory
         */
        public function getArmorSlot(): int
        {
            return $this->armorInfo->getArmorSlot();
        }

        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function isFireProof(): bool
        {
            return $this->armorInfo->isFireProof();
        }

        public function getMaterial(): ArmorMaterial
        {
            return $this->armorInfo->getMaterial();
        }

        public function getEnchantability(): int
        {
            return $this->armorInfo->getMaterial()->getEnchantability();
        }

        /**
         * Returns the dyed colour of this armour piece. This generally only applies to leather armour.
         */
        public function getCustomColor(): ?Color
        {
            return $this->customColor;
        }

        /**
         * Sets the dyed colour of this armour piece. This generally only applies to leather armour.
         *
         * @return $this
         */
        public function setCustomColor(Color $color): self
        {
            $this->customColor = $color;
            return $this;
        }

        /** @return $this */
        public function clearCustomColor(): self
        {
            $this->customColor = null;
            return $this;
        }

        /**
         * Returns the total enchantment protection factor this armour piece offers from all applicable protection
         * enchantments on the item.
         */
        public function getEnchantmentProtectionFactor(EntityDamageEvent $event): int
        {
            $epf = 0;

            foreach ($this->getEnchantments() as $enchantment) {
                $type = $enchantment->getType();
                if ($type instanceof ProtectionEnchantment && $type->isApplicable($event)) {
                    $epf += $type->getProtectionFactor($enchantment->getLevel());
                }
            }

            return $epf;
        }

        protected function getUnbreakingDamageReduction(int $amount): int
        {
            if (($unbreakingLevel = $this->getEnchantmentLevel(VanillaEnchantments::UNBREAKING())) > 0) {
                $negated = 0;

                $chance = 1 / ($unbreakingLevel + 1);
                for ($i = 0; $i < $amount; ++$i) {
                    $rand = method_exists(Utils::class, 'getRandomFloat') ? Utils::getRandomFloat() : (mt_rand() / mt_getrandmax());
                    if (mt_rand(1, 100) > 60 && $rand > $chance) { // unbreaking only applies to armor 40% of the time at best
                        $negated++;
                    }
                }

                return $negated;
            }

            return 0;
        }

        public function onClickAir(Player $player, Vector3 $directionVector, array &$returnedItems): ItemUseResult
        {
            if (method_exists($player, 'getArmorInventory')) {
                $armorInv = $player->getArmorInventory();
                if ($armorInv !== null && method_exists($armorInv, 'getItem') && method_exists($armorInv, 'setItem')) {
                    $existing = $armorInv->getItem($this->getArmorSlot());
                    $thisCopy = clone $this;
                    $new = $thisCopy->pop();
                    $armorInv->setItem($this->getArmorSlot(), $new);
                    if (method_exists($player, 'getInventory')) {
                        $player->getInventory()->setItemInHand($existing);
                    }
                    $sound = $new->getMaterial()->getEquipSound();
                    if ($sound !== null && method_exists($player, 'broadcastSound')) {
                        $player->broadcastSound($sound);
                    }
                    if (!$thisCopy->isNull()) {
                        // if the stack size was bigger than 1 (usually won't happen, but might be caused by plugins)
                        $returnedItems[] = $thisCopy;
                    }
                    return ItemUseResult::SUCCESS;
                }
            }

            // Fallback when ArmorInventory is not present on Player in the bridge
            return ItemUseResult::FAIL;
        }

        protected function deserializeCompoundTag(CompoundTag $tag): void
        {
            $hasKeepOnDeath = $tag->getTag(self::TAG_KEEP_ON_DEATH) !== null;
            if (!$hasKeepOnDeath) {
                $tag->setByte(self::TAG_KEEP_ON_DEATH, 0);
            }
            $hasUnbreakable = $tag->getTag("Unbreakable") !== null;
            if (!$hasUnbreakable) {
                $tag->setByte("Unbreakable", 0);
            }
            $hasDamage = $tag->getTag("Damage") !== null;
            if (!$hasDamage) {
                $tag->setInt("Damage", 0);
            }

            try {
                parent::deserializeCompoundTag($tag);
            } finally {
                if (!$hasKeepOnDeath) {
                    $tag->removeTag(self::TAG_KEEP_ON_DEATH);
                }
                if (!$hasUnbreakable) {
                    $tag->removeTag("Unbreakable");
                }
                if (!$hasDamage) {
                    $tag->removeTag("Damage");
                }
            }

            if (($colorTag = $tag->getTag(self::TAG_CUSTOM_COLOR)) instanceof IntTag) {
                $val = Binary::unsignInt($colorTag->getValue());
                if (method_exists(Color::class, 'fromARGB')) {
                    $this->customColor = Color::fromARGB($val);
                } else {
                    $a = ($val >> 24) & 0xff;
                    $r = ($val >> 16) & 0xff;
                    $g = ($val >> 8) & 0xff;
                    $b = $val & 0xff;
                    $this->customColor = new Color($r, $g, $b, $a);
                }
            } else {
                $this->customColor = null;
            }
        }

        protected function serializeCompoundTag(CompoundTag $tag): void
        {
            parent::serializeCompoundTag($tag);
            if ($this->customColor !== null) {
                $argb = method_exists($this->customColor, 'toARGB')
                    ? $this->customColor->toARGB()
                    : ((($this->customColor->getA() & 0xff) << 24) | (($this->customColor->getR() & 0xff) << 16) | (($this->customColor->getG() & 0xff) << 8) | ($this->customColor->getB() & 0xff));
                $tag->setInt(self::TAG_CUSTOM_COLOR, Binary::signInt($argb));
            } else {
                $tag->removeTag(self::TAG_CUSTOM_COLOR);
            }
        }
    }

    class TurtleHelmet extends Armor
    {
        public function onTickWorn(Living $entity): bool
        {
            if ($entity instanceof Human && (!method_exists($entity, 'isUnderwater') || !$entity->isUnderwater())) {
                if (method_exists($entity, 'getEffects')) {
                    $entity->getEffects()->add(new EffectInstance(VanillaEffects::WATER_BREATHING(), 200, 0, false));
                    return true;
                }
            }

            return false;
        }
    }
}
