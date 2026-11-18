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

namespace pocketmine\entity {

    use pocketmine\entity\effect\EffectInstance;

    if (!interface_exists(Consumable::class, false)) {
        /**
         * Interface implemented by objects that can be consumed by mobs.
         */
        interface Consumable
        {
            /**
             * @return EffectInstance[]
             */
            public function getAdditionalEffects(): array;

            /**
             * Called when this Consumable is consumed by mob, after standard resulting effects have been applied.
             */
            public function onConsume(Living $consumer): void;
        }
    }

    if (!interface_exists(FoodSource::class, false)) {
        /**
         * Interface implemented by objects that can be consumed by players, giving them food and saturation.
         */
        interface FoodSource extends Consumable
        {
            public function getFoodRestore(): int;

            public function getSaturationRestore(): float;

            /**
             * Returns whether a Human eating this FoodSource must have a non-full hunger bar.
             * This is ignored in creative mode and in peaceful difficulty.
             */
            public function requiresHunger(): bool;
        }
    }
}

namespace pocketmine\entity\projectile {

    if (!interface_exists(Throwable::class, false) && !class_exists(Throwable::class, false)) {
        interface Throwable
        {
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
        }
    }

    if (!class_exists(BottleEmptySound::class, false)) {
        class BottleEmptySound extends SoundBase
        {
            public function getSoundName(): ?string
            {
                return 'bottle.empty';
            }
        }
    }
}

namespace pocketmine\utils {

    if (!trait_exists(LegacyEnumShimTrait::class, false)) {
        trait LegacyEnumShimTrait
        {
            public static function __callStatic(string $name, array $arguments): self
            {
                if (count($arguments) > 0) {
                    throw new \ArgumentCountError("Expected exactly 0 arguments, " . count($arguments) . " passed");
                }
                $all = self::getAll();
                $key = strtoupper($name);
                if (!isset($all[$key])) {
                    throw new \InvalidArgumentException("No such case " . $name);
                }
                return $all[$key];
            }

            public static function getAll(): array
            {
                static $result = null;
                if ($result === null) {
                    $result = [];
                    foreach (self::cases() as $case) {
                        $result[strtoupper($case->name)] = $case;
                    }
                }
                return $result;
            }
        }
    }
}

namespace pocketmine\item {

    use pocketmine\block\Block;
    use pocketmine\block\Liquid;
    use pocketmine\block\VanillaBlocks;
    use pocketmine\data\runtime\RuntimeDataDescriber;
    use pocketmine\entity\Consumable;
    use pocketmine\entity\effect\Effect;
    use pocketmine\entity\effect\EffectInstance;
    use pocketmine\entity\effect\VanillaEffects;
    use pocketmine\entity\FoodSource;
    use pocketmine\entity\Living;
    use pocketmine\entity\Location;
    use pocketmine\entity\projectile\SplashPotion as SplashPotionEntity;
    use pocketmine\entity\projectile\Throwable;
    use pocketmine\event\entity\ProjectileLaunchEvent;
    use pocketmine\math\Vector3;
    use pocketmine\player\Player;
    use pocketmine\utils\LegacyEnumShimTrait;
    use pocketmine\utils\Utils;
    use pocketmine\world\sound\BottleEmptySound;
    use pocketmine\world\sound\EndermanTeleportSound;
    use pocketmine\world\sound\ThrowSound;
    use function class_exists;
    use function method_exists;
    use function min;
    use function mt_rand;
    use function spl_object_id;
    use function strtoupper;


    if (!class_exists(ItemCooldownTags::class, false)) {
        /**
         * Tags used by items to determine their cooldown group.
         */
        final class ItemCooldownTags
        {
            private function __construct()
            {
            }

            public const CHORUS_FRUIT = "chorus_fruit";
            public const ENDER_PEARL = "ender_pearl";
            public const SHIELD = "shield";
            public const GOAT_HORN = "goat_horn";
        }
    }

    /**
     * Interface implemented by objects that can be consumed by mobs.
     */
    interface ConsumableItem extends Consumable, Releasable
    {
        /**
         * Returns the leftover that this Consumable produces when it is consumed. For Items, this is usually air, but could
         * be an Item to add to a Player's inventory afterwards (such as a bowl).
         */
        public function getResidue(): Item;
    }

    interface FoodSourceItem extends ConsumableItem, FoodSource
    {
    }

    abstract class Food extends Item implements FoodSourceItem
    {
        public function requiresHunger(): bool
        {
            return true;
        }

        public function getResidue(): Item
        {
            return VanillaItems::AIR();
        }

        /**
         * @return EffectInstance[]
         */
        public function getAdditionalEffects(): array
        {
            return [];
        }

        public function onConsume(Living $consumer): void
        {
        }

        public function canStartUsingItem(Player $player): bool
        {
            return !$this->requiresHunger() || $player->canEat();
        }
    }

    class Apple extends Food
    {
        public function getFoodRestore(): int
        {
            return 4;
        }

        public function getSaturationRestore(): float
        {
            return 2.4;
        }
    }

    class GoldenApple extends Food
    {
        public function requiresHunger(): bool
        {
            return false;
        }

        public function getFoodRestore(): int
        {
            return 4;
        }

        public function getSaturationRestore(): float
        {
            return 9.6;
        }

        public function getAdditionalEffects(): array
        {
            return [
                new EffectInstance(VanillaEffects::REGENERATION(), 100, 1),
                new EffectInstance(VanillaEffects::ABSORPTION(), 2400)
            ];
        }
    }

    class GoldenAppleEnchanted extends GoldenApple
    {
        public function getAdditionalEffects(): array
        {
            return [
                new EffectInstance(VanillaEffects::REGENERATION(), 600, 1),
                new EffectInstance(VanillaEffects::ABSORPTION(), 2400, 3),
                new EffectInstance(VanillaEffects::RESISTANCE(), 6000),
                new EffectInstance(VanillaEffects::FIRE_RESISTANCE(), 6000)
            ];
        }
    }

    class BakedPotato extends Food
    {
        public function getFoodRestore(): int
        {
            return 5;
        }

        public function getSaturationRestore(): float
        {
            return 7.2;
        }
    }

    class RawBeef extends Food
    {
        public function getFoodRestore(): int
        {
            return 3;
        }

        public function getSaturationRestore(): float
        {
            return 1.8;
        }
    }

    class Beef extends RawBeef
    {
    }

    class Bread extends Food
    {
        public function getFoodRestore(): int
        {
            return 5;
        }

        public function getSaturationRestore(): float
        {
            return 6.0;
        }
    }

    class Carrot extends Food
    {
        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::CARROTS();
        }

        public function getFoodRestore(): int
        {
            return 3;
        }

        public function getSaturationRestore(): float
        {
            return 4.8;
        }
    }

    class GoldenCarrot extends Food
    {
        public function getFoodRestore(): int
        {
            return 6;
        }

        public function getSaturationRestore(): float
        {
            return 14.4;
        }
    }

    class RawChicken extends Food
    {
        public function getFoodRestore(): int
        {
            return 2;
        }

        public function getSaturationRestore(): float
        {
            return 1.2;
        }

        public function getAdditionalEffects(): array
        {
            return mt_rand(0, 9) < 3 ? [new EffectInstance(VanillaEffects::HUNGER(), 600)] : [];
        }
    }

    class Chicken extends RawChicken
    {
    }

    class CookedChicken extends Food
    {
        public function getFoodRestore(): int
        {
            return 6;
        }

        public function getSaturationRestore(): float
        {
            return 7.2;
        }
    }

    class CookedFish extends Food
    {
        public function getFoodRestore(): int
        {
            return 5;
        }

        public function getSaturationRestore(): float
        {
            return 6.0;
        }
    }

    class CookedMutton extends Food
    {
        public function getFoodRestore(): int
        {
            return 6;
        }

        public function getSaturationRestore(): float
        {
            return 9.6;
        }
    }

    class CookedPorkchop extends Food
    {
        public function getFoodRestore(): int
        {
            return 8;
        }

        public function getSaturationRestore(): float
        {
            return 12.8;
        }
    }

    class RawPorkchop extends Food
    {
        public function getFoodRestore(): int
        {
            return 3;
        }

        public function getSaturationRestore(): float
        {
            return 0.6;
        }
    }

    class Porkchop extends RawPorkchop
    {
    }

    class CookedRabbit extends Food
    {
        public function getFoodRestore(): int
        {
            return 5;
        }

        public function getSaturationRestore(): float
        {
            return 6.0;
        }
    }

    class CookedSalmon extends Food
    {
        public function getFoodRestore(): int
        {
            return 6;
        }

        public function getSaturationRestore(): float
        {
            return 9.6;
        }
    }

    class Cookie extends Food
    {
        public function getFoodRestore(): int
        {
            return 2;
        }

        public function getSaturationRestore(): float
        {
            return 0.4;
        }
    }

    class DriedKelp extends Food
    {
        public function getFoodRestore(): int
        {
            return 1;
        }

        public function getSaturationRestore(): float
        {
            return 0.6;
        }
    }

    class RawFish extends Food
    {
        public function getFoodRestore(): int
        {
            return 2;
        }

        public function getSaturationRestore(): float
        {
            return 0.4;
        }
    }

    class Fish extends RawFish
    {
    }

    class Melon extends Food
    {
        public function getFoodRestore(): int
        {
            return 2;
        }

        public function getSaturationRestore(): float
        {
            return 1.2;
        }
    }

    class MushroomStew extends Food
    {
        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function getFoodRestore(): int
        {
            return 6;
        }

        public function getSaturationRestore(): float
        {
            return 7.2;
        }

        public function getResidue(): Item
        {
            return VanillaItems::BOWL();
        }
    }

    class RawMutton extends Food
    {
        public function getFoodRestore(): int
        {
            return 2;
        }

        public function getSaturationRestore(): float
        {
            return 1.2;
        }
    }

    class Mutton extends RawMutton
    {
    }

    class PoisonousPotato extends Food
    {
        public function getFoodRestore(): int
        {
            return 2;
        }

        public function getSaturationRestore(): float
        {
            return 1.2;
        }

        public function getAdditionalEffects(): array
        {
            if (mt_rand(0, 100) > 40) {
                return [
                    new EffectInstance(VanillaEffects::POISON(), 100)
                ];
            }
            return [];
        }
    }

    class Potato extends Food
    {
        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::POTATOES();
        }

        public function getFoodRestore(): int
        {
            return 1;
        }

        public function getSaturationRestore(): float
        {
            return 0.6;
        }
    }

    class Pufferfish extends Food
    {
        public function getFoodRestore(): int
        {
            return 1;
        }

        public function getSaturationRestore(): float
        {
            return 0.2;
        }

        public function getAdditionalEffects(): array
        {
            return [
                new EffectInstance(VanillaEffects::HUNGER(), 300, 2),
                new EffectInstance(VanillaEffects::POISON(), 1200, 3),
                new EffectInstance(VanillaEffects::NAUSEA(), 300, 1)
            ];
        }
    }

    class PumpkinPie extends Food
    {
        public function getFoodRestore(): int
        {
            return 8;
        }

        public function getSaturationRestore(): float
        {
            return 4.8;
        }
    }

    class RawRabbit extends Food
    {
        public function getFoodRestore(): int
        {
            return 3;
        }

        public function getSaturationRestore(): float
        {
            return 1.8;
        }
    }

    class Rabbit extends RawRabbit
    {
    }

    class RabbitStew extends Food
    {
        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function getFoodRestore(): int
        {
            return 10;
        }

        public function getSaturationRestore(): float
        {
            return 12.0;
        }

        public function getResidue(): Item
        {
            return VanillaItems::BOWL();
        }
    }

    class RawSalmon extends Food
    {
        public function getFoodRestore(): int
        {
            return 2;
        }

        public function getSaturationRestore(): float
        {
            return 0.2;
        }
    }

    class RottenFlesh extends Food
    {
        public function getFoodRestore(): int
        {
            return 4;
        }

        public function getSaturationRestore(): float
        {
            return 0.8;
        }

        public function getAdditionalEffects(): array
        {
            $rand = class_exists(Utils::class) && method_exists(Utils::class, 'getRandomFloat')
                ? Utils::getRandomFloat()
                : (mt_rand() / mt_getrandmax());
            if ($rand <= 0.8) {
                return [
                    new EffectInstance(VanillaEffects::HUNGER(), 600)
                ];
            }

            return [];
        }
    }

    class SpiderEye extends Food
    {
        public function getFoodRestore(): int
        {
            return 2;
        }

        public function getSaturationRestore(): float
        {
            return 3.2;
        }

        public function getAdditionalEffects(): array
        {
            return [new EffectInstance(VanillaEffects::POISON(), 80)];
        }
    }

    class Steak extends Food
    {
        public function getFoodRestore(): int
        {
            return 8;
        }

        public function getSaturationRestore(): float
        {
            return 12.8;
        }
    }

    class SweetBerries extends Food
    {
        public function getFoodRestore(): int
        {
            return 2;
        }

        public function getSaturationRestore(): float
        {
            return 1.2;
        }

        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::SWEET_BERRY_BUSH();
        }
    }

    class GlowBerries extends Food
    {
        public function getFoodRestore(): int
        {
            return 2;
        }

        public function getSaturationRestore(): float
        {
            return 0.4;
        }

        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::CAVE_VINES();
        }
    }

    class Clownfish extends Food
    {
        public function getFoodRestore(): int
        {
            return 1;
        }

        public function getSaturationRestore(): float
        {
            return 0.2;
        }
    }

    class Beetroot extends Food
    {
        public function getFoodRestore(): int
        {
            return 1;
        }

        public function getSaturationRestore(): float
        {
            return 1.2;
        }
    }

    class BeetrootSoup extends Food
    {
        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function getFoodRestore(): int
        {
            return 6;
        }

        public function getSaturationRestore(): float
        {
            return 7.2;
        }

        public function getResidue(): Item
        {
            return VanillaItems::BOWL();
        }
    }

    class ChorusFruit extends Food
    {
        public function getFoodRestore(): int
        {
            return 4;
        }

        public function getSaturationRestore(): float
        {
            return 2.4;
        }

        public function requiresHunger(): bool
        {
            return false;
        }

        public function onConsume(Living $consumer): void
        {
            $world = $consumer->getWorld();
            if ($world === null) {
                return;
            }

            $origin = $consumer->getPosition();
            $minX = (int) $origin->getFloorX() - 8;
            $worldMaxY = method_exists($world, 'getMaxY') ? $world->getMaxY() : 320;
            $minY = min((int) $origin->getFloorY(), $worldMaxY) - 8;
            $minZ = (int) $origin->getFloorZ() - 8;

            $maxX = $minX + 16;
            $maxY = $minY + 16;
            $maxZ = $minZ + 16;

            $worldMinY = method_exists($world, 'getMinY') ? $world->getMinY() : -64;

            for ($attempts = 0; $attempts < 16; ++$attempts) {
                $x = mt_rand($minX, $maxX);
                $y = mt_rand($minY, $maxY);
                $z = mt_rand($minZ, $maxZ);

                while ($y >= $worldMinY && method_exists($world, 'getBlockAt') && !$world->getBlockAt($x, $y, $z)->isSolid()) {
                    $y--;
                }
                if ($y < $worldMinY) {
                    continue;
                }

                if (method_exists($world, 'getBlockAt')) {
                    $blockUp = $world->getBlockAt($x, $y + 1, $z);
                    $blockUp2 = $world->getBlockAt($x, $y + 2, $z);
                    if ($blockUp->isSolid() || ($blockUp instanceof Liquid) || $blockUp2->isSolid() || ($blockUp2 instanceof Liquid)) {
                        continue;
                    }
                }

                // Sounds are broadcasted at both source and destination
                if (class_exists(EndermanTeleportSound::class) && method_exists($world, 'addSound')) {
                    $world->addSound($origin, new EndermanTeleportSound());
                }
                $target = new Vector3($x + 0.5, $y + 1, $z + 0.5);
                if (method_exists($consumer, 'teleport')) {
                    $consumer->teleport($target);
                }
                if (class_exists(EndermanTeleportSound::class) && method_exists($world, 'addSound')) {
                    $world->addSound($target, new EndermanTeleportSound());
                }

                break;
            }
        }

        public function getCooldownTicks(): int
        {
            return 20;
        }

        public function getCooldownTag(): ?string
        {
            return ItemCooldownTags::CHORUS_FRUIT;
        }
    }

    final class HoneyBottle extends Food
    {
        public function getMaxStackSize(): int
        {
            return 16;
        }

        public function requiresHunger(): bool
        {
            return false;
        }

        public function getFoodRestore(): int
        {
            return 6;
        }

        public function getSaturationRestore(): float
        {
            return 1.2;
        }

        public function getResidue(): Item
        {
            return VanillaItems::GLASS_BOTTLE();
        }

        public function onConsume(Living $consumer): void
        {
            $consumer->getEffects()->remove(VanillaEffects::POISON());
        }
    }

    class MilkBucket extends Item implements ConsumableItem
    {
        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function getResidue(): Item
        {
            return VanillaItems::BUCKET();
        }

        public function getAdditionalEffects(): array
        {
            return [];
        }

        public function onConsume(Living $consumer): void
        {
            $consumer->getEffects()->clear();
        }

        public function canStartUsingItem(Player $player): bool
        {
            return true;
        }
    }

    /**
     * @method static MedicineType ANTIDOTE()
     * @method static MedicineType ELIXIR()
     * @method static MedicineType EYE_DROPS()
     * @method static MedicineType TONIC()
     */
    enum MedicineType
    {
        use LegacyEnumShimTrait;

        case ANTIDOTE;
        case ELIXIR;
        case EYE_DROPS;
        case TONIC;

        /**
         * @return array{0: string, 1: Effect}
         */
        private function getMetadata(): array
        {
            return match ($this) {
                self::ANTIDOTE => ['Antidote', VanillaEffects::POISON()],
                self::ELIXIR => ['Elixir', VanillaEffects::WEAKNESS()],
                self::EYE_DROPS => ['Eye Drops', VanillaEffects::BLINDNESS()],
                self::TONIC => ['Tonic', VanillaEffects::NAUSEA()],
            };
        }

        public function getDisplayName(): string
        {
            return $this->getMetadata()[0];
        }

        public function getCuredEffect(): Effect
        {
            return $this->getMetadata()[1];
        }
    }

    class Medicine extends Item implements ConsumableItem
    {
        private MedicineType $medicineType = MedicineType::EYE_DROPS;

        protected function describeState(RuntimeDataDescriber $w): void
        {
            $w->enum($this->medicineType);
        }

        public function getType(): MedicineType
        {
            return $this->medicineType;
        }

        /**
         * @return $this
         */
        public function setType(MedicineType $type): self
        {
            $this->medicineType = $type;
            return $this;
        }

        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function onConsume(Living $consumer): void
        {
            $consumer->getEffects()->remove($this->getType()->getCuredEffect());
        }

        public function getAdditionalEffects(): array
        {
            return [];
        }

        public function getResidue(): Item
        {
            return VanillaItems::GLASS_BOTTLE();
        }

        public function canStartUsingItem(Player $player): bool
        {
            return $player->getEffects()->has($this->getType()->getCuredEffect());
        }
    }

    /**
     * @method static PotionType AWKWARD()
     * @method static PotionType FIRE_RESISTANCE()
     * @method static PotionType HARMING()
     * @method static PotionType HEALING()
     * @method static PotionType INVISIBILITY()
     * @method static PotionType LEAPING()
     * @method static PotionType LONG_FIRE_RESISTANCE()
     * @method static PotionType LONG_INVISIBILITY()
     * @method static PotionType LONG_LEAPING()
     * @method static PotionType LONG_MUNDANE()
     * @method static PotionType LONG_NIGHT_VISION()
     * @method static PotionType LONG_POISON()
     * @method static PotionType LONG_REGENERATION()
     * @method static PotionType LONG_SLOWNESS()
     * @method static PotionType LONG_SLOW_FALLING()
     * @method static PotionType LONG_STRENGTH()
     * @method static PotionType LONG_SWIFTNESS()
     * @method static PotionType LONG_TURTLE_MASTER()
     * @method static PotionType LONG_WATER_BREATHING()
     * @method static PotionType LONG_WEAKNESS()
     * @method static PotionType MUNDANE()
     * @method static PotionType NIGHT_VISION()
     * @method static PotionType POISON()
     * @method static PotionType REGENERATION()
     * @method static PotionType SLOWNESS()
     * @method static PotionType SLOW_FALLING()
     * @method static PotionType STRENGTH()
     * @method static PotionType STRONG_HARMING()
     * @method static PotionType STRONG_HEALING()
     * @method static PotionType STRONG_LEAPING()
     * @method static PotionType STRONG_POISON()
     * @method static PotionType STRONG_REGENERATION()
     * @method static PotionType STRONG_SLOWNESS()
     * @method static PotionType STRONG_STRENGTH()
     * @method static PotionType STRONG_SWIFTNESS()
     * @method static PotionType STRONG_TURTLE_MASTER()
     * @method static PotionType SWIFTNESS()
     * @method static PotionType THICK()
     * @method static PotionType TURTLE_MASTER()
     * @method static PotionType WATER()
     * @method static PotionType WATER_BREATHING()
     * @method static PotionType WEAKNESS()
     * @method static PotionType WITHER()
     */
    enum PotionType
    {
        use LegacyEnumShimTrait;

        case WATER;
        case MUNDANE;
        case LONG_MUNDANE;
        case THICK;
        case AWKWARD;
        case NIGHT_VISION;
        case LONG_NIGHT_VISION;
        case INVISIBILITY;
        case LONG_INVISIBILITY;
        case LEAPING;
        case LONG_LEAPING;
        case STRONG_LEAPING;
        case FIRE_RESISTANCE;
        case LONG_FIRE_RESISTANCE;
        case SWIFTNESS;
        case LONG_SWIFTNESS;
        case STRONG_SWIFTNESS;
        case SLOWNESS;
        case LONG_SLOWNESS;
        case WATER_BREATHING;
        case LONG_WATER_BREATHING;
        case HEALING;
        case STRONG_HEALING;
        case HARMING;
        case STRONG_HARMING;
        case POISON;
        case LONG_POISON;
        case STRONG_POISON;
        case REGENERATION;
        case LONG_REGENERATION;
        case STRONG_REGENERATION;
        case STRENGTH;
        case LONG_STRENGTH;
        case STRONG_STRENGTH;
        case WEAKNESS;
        case LONG_WEAKNESS;
        case WITHER;
        case TURTLE_MASTER;
        case LONG_TURTLE_MASTER;
        case STRONG_TURTLE_MASTER;
        case SLOW_FALLING;
        case LONG_SLOW_FALLING;
        case STRONG_SLOWNESS;

        /**
         * @return array{0: string, 1: \Closure(): list<EffectInstance>}
         */
        private function getMetadata(): array
        {
            /** @var array<int, array{0: string, 1: \Closure(): list<EffectInstance>}> $cache */
            static $cache = [];

            return $cache[spl_object_id($this)] ??= match ($this) {
                self::WATER => ["Water", fn() => []],
                self::MUNDANE => ["Mundane", fn() => []],
                self::LONG_MUNDANE => ["Long Mundane", fn() => []],
                self::THICK => ["Thick", fn() => []],
                self::AWKWARD => ["Awkward", fn() => []],
                self::NIGHT_VISION => ["Night Vision", fn() => [
                    new EffectInstance(VanillaEffects::NIGHT_VISION(), 3600)
                ]],
                self::LONG_NIGHT_VISION => ["Long Night Vision", fn() => [
                    new EffectInstance(VanillaEffects::NIGHT_VISION(), 9600)
                ]],
                self::INVISIBILITY => ["Invisibility", fn() => [
                    new EffectInstance(VanillaEffects::INVISIBILITY(), 3600)
                ]],
                self::LONG_INVISIBILITY => ["Long Invisibility", fn() => [
                    new EffectInstance(VanillaEffects::INVISIBILITY(), 9600)
                ]],
                self::LEAPING => ["Leaping", fn() => [
                    new EffectInstance(VanillaEffects::JUMP_BOOST(), 3600)
                ]],
                self::LONG_LEAPING => ["Long Leaping", fn() => [
                    new EffectInstance(VanillaEffects::JUMP_BOOST(), 9600)
                ]],
                self::STRONG_LEAPING => ["Strong Leaping", fn() => [
                    new EffectInstance(VanillaEffects::JUMP_BOOST(), 1800, 1)
                ]],
                self::FIRE_RESISTANCE => ["Fire Resistance", fn() => [
                    new EffectInstance(VanillaEffects::FIRE_RESISTANCE(), 3600)
                ]],
                self::LONG_FIRE_RESISTANCE => ["Long Fire Resistance", fn() => [
                    new EffectInstance(VanillaEffects::FIRE_RESISTANCE(), 9600)
                ]],
                self::SWIFTNESS => ["Swiftness", fn() => [
                    new EffectInstance(VanillaEffects::SPEED(), 3600)
                ]],
                self::LONG_SWIFTNESS => ["Long Swiftness", fn() => [
                    new EffectInstance(VanillaEffects::SPEED(), 9600)
                ]],
                self::STRONG_SWIFTNESS => ["Strong Swiftness", fn() => [
                    new EffectInstance(VanillaEffects::SPEED(), 1800, 1)
                ]],
                self::SLOWNESS => ["Slowness", fn() => [
                    new EffectInstance(VanillaEffects::SLOWNESS(), 1800)
                ]],
                self::LONG_SLOWNESS => ["Long Slowness", fn() => [
                    new EffectInstance(VanillaEffects::SLOWNESS(), 4800)
                ]],
                self::WATER_BREATHING => ["Water Breathing", fn() => [
                    new EffectInstance(VanillaEffects::WATER_BREATHING(), 3600)
                ]],
                self::LONG_WATER_BREATHING => ["Long Water Breathing", fn() => [
                    new EffectInstance(VanillaEffects::WATER_BREATHING(), 9600)
                ]],
                self::HEALING => ["Healing", fn() => [
                    new EffectInstance(VanillaEffects::INSTANT_HEALTH())
                ]],
                self::STRONG_HEALING => ["Strong Healing", fn() => [
                    new EffectInstance(VanillaEffects::INSTANT_HEALTH(), null, 1)
                ]],
                self::HARMING => ["Harming", fn() => [
                    new EffectInstance(VanillaEffects::INSTANT_DAMAGE())
                ]],
                self::STRONG_HARMING => ["Strong Harming", fn() => [
                    new EffectInstance(VanillaEffects::INSTANT_DAMAGE(), null, 1)
                ]],
                self::POISON => ["Poison", fn() => [
                    new EffectInstance(VanillaEffects::POISON(), 900)
                ]],
                self::LONG_POISON => ["Long Poison", fn() => [
                    new EffectInstance(VanillaEffects::POISON(), 2400)
                ]],
                self::STRONG_POISON => ["Strong Poison", fn() => [
                    new EffectInstance(VanillaEffects::POISON(), 440, 1)
                ]],
                self::REGENERATION => ["Regeneration", fn() => [
                    new EffectInstance(VanillaEffects::REGENERATION(), 900)
                ]],
                self::LONG_REGENERATION => ["Long Regeneration", fn() => [
                    new EffectInstance(VanillaEffects::REGENERATION(), 2400)
                ]],
                self::STRONG_REGENERATION => ["Strong Regeneration", fn() => [
                    new EffectInstance(VanillaEffects::REGENERATION(), 440, 1)
                ]],
                self::STRENGTH => ["Strength", fn() => [
                    new EffectInstance(VanillaEffects::STRENGTH(), 3600)
                ]],
                self::LONG_STRENGTH => ["Long Strength", fn() => [
                    new EffectInstance(VanillaEffects::STRENGTH(), 9600)
                ]],
                self::STRONG_STRENGTH => ["Strong Strength", fn() => [
                    new EffectInstance(VanillaEffects::STRENGTH(), 1800, 1)
                ]],
                self::WEAKNESS => ["Weakness", fn() => [
                    new EffectInstance(VanillaEffects::WEAKNESS(), 1800)
                ]],
                self::LONG_WEAKNESS => ["Long Weakness", fn() => [
                    new EffectInstance(VanillaEffects::WEAKNESS(), 4800)
                ]],
                self::WITHER => ["Wither", fn() => [
                    new EffectInstance(VanillaEffects::WITHER(), 800, 1)
                ]],
                self::TURTLE_MASTER => ["Turtle Master", fn() => [
                    new EffectInstance(VanillaEffects::SLOWNESS(), 20 * 20, 3),
                    new EffectInstance(VanillaEffects::RESISTANCE(), 20 * 20, 2),
                ]],
                self::LONG_TURTLE_MASTER => ["Long Turtle Master", fn() => [
                    new EffectInstance(VanillaEffects::SLOWNESS(), 40 * 20, 3),
                    new EffectInstance(VanillaEffects::RESISTANCE(), 40 * 20, 2),
                ]],
                self::STRONG_TURTLE_MASTER => ["Strong Turtle Master", fn() => [
                    new EffectInstance(VanillaEffects::SLOWNESS(), 20 * 20, 5),
                    new EffectInstance(VanillaEffects::RESISTANCE(), 20 * 20, 3),
                ]],
                self::SLOW_FALLING => ["Slow Falling", fn() => []],
                self::LONG_SLOW_FALLING => ["Long Slow Falling", fn() => []],
                self::STRONG_SLOWNESS => ["Strong Slowness", fn() => [
                    new EffectInstance(VanillaEffects::SLOWNESS(), 20 * 20, 3)
                ]],
            };
        }

        public function getDisplayName(): string
        {
            return $this->getMetadata()[0];
        }

        /**
         * @return EffectInstance[]
         */
        public function getEffects(): array
        {
            return ($this->getMetadata()[1])();
        }
    }

    class Potion extends Item implements ConsumableItem
    {
        private PotionType $potionType = PotionType::WATER;

        protected function describeState(RuntimeDataDescriber $w): void
        {
            $w->enum($this->potionType);
        }

        public function getType(): PotionType
        {
            return $this->potionType;
        }

        /**
         * @return $this
         */
        public function setType(PotionType $type): self
        {
            $this->potionType = $type;
            return $this;
        }

        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function onConsume(Living $consumer): void
        {
            if (method_exists($consumer, 'broadcastSound')) {
                $consumer->broadcastSound(new BottleEmptySound());
            } elseif (method_exists($consumer, 'getWorld')) {
                $world = $consumer->getWorld();
                if ($world !== null && method_exists($world, 'addSound')) {
                    $world->addSound($consumer->getPosition(), new BottleEmptySound());
                }
            }
        }

        public function getAdditionalEffects(): array
        {
            return $this->potionType->getEffects();
        }

        public function getResidue(): Item
        {
            return VanillaItems::GLASS_BOTTLE();
        }

        public function canStartUsingItem(Player $player): bool
        {
            return true;
        }
    }

    class SplashPotion extends ProjectileItem
    {
        private PotionType $potionType = PotionType::WATER;

        public function __construct(
            ItemIdentifier $identifier,
            string $name = "Splash Potion",
            array $enchantmentTags = [],
            private bool $linger = false
        ) {
            parent::__construct($identifier, $name, $enchantmentTags);
        }

        protected function describeState(RuntimeDataDescriber $w): void
        {
            $w->enum($this->potionType);
        }

        public function getType(): PotionType
        {
            return $this->potionType;
        }

        /**
         * @return $this
         */
        public function setType(PotionType $type): self
        {
            $this->potionType = $type;
            return $this;
        }

        public function getMaxStackSize(): int
        {
            return 1;
        }

        protected function createEntity(Location $location, Player $thrower): Throwable
        {
            $projectile = null;
            if (class_exists(SplashPotionEntity::class)) {
                try {
                    $projectile = new SplashPotionEntity($location, $thrower, $this->potionType);
                } catch (\TypeError) {
                    $projectile = new SplashPotionEntity($location, $thrower);
                }
            }
            if ($projectile !== null) {
                if (method_exists($projectile, 'setPotionType')) {
                    $projectile->setPotionType($this->potionType);
                }
                if (method_exists($projectile, 'setLinger')) {
                    $projectile->setLinger($this->linger);
                }
                return $projectile;
            }
            throw new \LogicException("SplashPotion entity class not found");
        }

        public function getThrowForce(): float
        {
            return 0.5;
        }

        /**
         * Returns whether this splash potion will create an area-effect cloud on impact.
         */
        public function willLinger(): bool
        {
            return $this->linger;
        }
    }

    /**
     * @method static SuspiciousStewType ALLIUM()
     * @method static SuspiciousStewType AZURE_BLUET()
     * @method static SuspiciousStewType BLUE_ORCHID()
     * @method static SuspiciousStewType CORNFLOWER()
     * @method static SuspiciousStewType DANDELION()
     * @method static SuspiciousStewType LILY_OF_THE_VALLEY()
     * @method static SuspiciousStewType OXEYE_DAISY()
     * @method static SuspiciousStewType POPPY()
     * @method static SuspiciousStewType TULIP()
     * @method static SuspiciousStewType WITHER_ROSE()
     */
    enum SuspiciousStewType
    {
        use LegacyEnumShimTrait;

        case POPPY;
        case CORNFLOWER;
        case TULIP;
        case AZURE_BLUET;
        case LILY_OF_THE_VALLEY;
        case DANDELION;
        case BLUE_ORCHID;
        case ALLIUM;
        case OXEYE_DAISY;
        case WITHER_ROSE;

        /**
         * @return EffectInstance[]
         */
        public function getEffects(): array
        {
            return match ($this) {
                self::POPPY => [new EffectInstance(VanillaEffects::NIGHT_VISION(), 80)],
                self::CORNFLOWER => [new EffectInstance(VanillaEffects::JUMP_BOOST(), 80)],
                self::TULIP => [new EffectInstance(VanillaEffects::WEAKNESS(), 140)],
                self::AZURE_BLUET => [new EffectInstance(VanillaEffects::BLINDNESS(), 120)],
                self::LILY_OF_THE_VALLEY => [new EffectInstance(VanillaEffects::POISON(), 200)],
                self::DANDELION,
                self::BLUE_ORCHID => [new EffectInstance(VanillaEffects::SATURATION(), 6)],
                self::ALLIUM => [new EffectInstance(VanillaEffects::FIRE_RESISTANCE(), 40)],
                self::OXEYE_DAISY => [new EffectInstance(VanillaEffects::REGENERATION(), 120)],
                self::WITHER_ROSE => [new EffectInstance(VanillaEffects::WITHER(), 120)],
            };
        }
    }

    class SuspiciousStew extends Food
    {
        private SuspiciousStewType $suspiciousStewType = SuspiciousStewType::POPPY;

        protected function describeState(RuntimeDataDescriber $w): void
        {
            $w->enum($this->suspiciousStewType);
        }

        public function getType(): SuspiciousStewType
        {
            return $this->suspiciousStewType;
        }

        /**
         * @return $this
         */
        public function setType(SuspiciousStewType $type): self
        {
            $this->suspiciousStewType = $type;
            return $this;
        }

        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function requiresHunger(): bool
        {
            return false;
        }

        public function getFoodRestore(): int
        {
            return 6;
        }

        public function getSaturationRestore(): float
        {
            return 7.2;
        }

        public function getAdditionalEffects(): array
        {
            return $this->suspiciousStewType->getEffects();
        }

        public function getResidue(): Item
        {
            return VanillaItems::BOWL();
        }
    }
}
