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

namespace pocketmine\world\sound {

    use pocketmine\item\GoatHornType;
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

            public function getEventId(): ?int
            {
                return null;
            }

            public function getExtraData(): int
            {
                return 0;
            }
        }
    }

    if (!class_exists(GoatHornSound::class, false)) {
        class GoatHornSound extends SoundBase
        {
            public function __construct(private GoatHornType $goatHornType)
            {
            }

            public function getHornType(): GoatHornType
            {
                return $this->goatHornType;
            }

            public function getSoundName(): ?string
            {
                return match ($this->goatHornType) {
                    GoatHornType::PONDER => 'horn.call.0',
                    GoatHornType::SING => 'horn.call.1',
                    GoatHornType::SEEK => 'horn.call.2',
                    GoatHornType::FEEL => 'horn.call.3',
                    GoatHornType::ADMIRE => 'horn.call.4',
                    GoatHornType::CALL => 'horn.call.5',
                    GoatHornType::YEARN => 'horn.call.6',
                    GoatHornType::DREAM => 'horn.call.7',
                };
            }
        }
    }
}

namespace pocketmine\item {

    use pocketmine\block\Block;
    use pocketmine\block\BlockTypeIds;
    use pocketmine\block\Lava;
    use pocketmine\block\Liquid;
    use pocketmine\block\utils\DyeColor;
    use pocketmine\block\VanillaBlocks;
    use pocketmine\data\runtime\RuntimeDataDescriber;
    use pocketmine\entity\Entity;
    use pocketmine\entity\Location;
    use pocketmine\entity\projectile\Arrow as ArrowEntity;
    use pocketmine\entity\projectile\Projectile;
    use pocketmine\event\entity\EntityShootBowEvent;
    use pocketmine\event\entity\ProjectileLaunchEvent;
    use pocketmine\event\player\PlayerBucketEmptyEvent;
    use pocketmine\event\player\PlayerBucketFillEvent;
    use pocketmine\item\enchantment\VanillaEnchantments;
    use pocketmine\math\Vector3;
    use pocketmine\player\Player;
    use pocketmine\utils\LegacyEnumShimTrait;
    use pocketmine\world\sound\BlazeShootSound;
    use pocketmine\world\sound\BowShootSound;
    use pocketmine\world\sound\GoatHornSound;
    use function class_exists;
    use function defined;
    use function enum_exists;
    use function intdiv;
    use function method_exists;
    use function min;
    use function strtolower;

    if (!class_exists(ItemCooldownTags::class, false)) {
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


    enum GoatHornType
    {
        use LegacyEnumShimTrait;

        case PONDER;
        case SING;
        case SEEK;
        case FEEL;
        case ADMIRE;
        case CALL;
        case YEARN;
        case DREAM;
    }

    final class Bamboo extends Item
    {
        public function getFuelTime(): int
        {
            return 50;
        }

        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::BAMBOO_SAPLING();
        }
    }

    class BeetrootSeeds extends Item
    {
        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::BEETROOTS();
        }
    }

    class BlazeRod extends Item
    {
        public function getFuelTime(): int
        {
            return 2400;
        }
    }

    class Bow extends Tool implements Releasable
    {
        public function getFuelTime(): int
        {
            return 200;
        }

        public function getMaxDurability(): int
        {
            return 384;
        }

        public function onReleaseUsing(Player $player, array &$returnedItems): ItemUseResult
        {
            $arrow = VanillaItems::ARROW();
            $inventory = null;
            if (method_exists($player, 'getOffHandInventory') && $player->getOffHandInventory() !== null && $player->getOffHandInventory()->contains($arrow)) {
                $inventory = $player->getOffHandInventory();
            } elseif (method_exists($player, 'getInventory') && $player->getInventory() !== null && $player->getInventory()->contains($arrow)) {
                $inventory = $player->getInventory();
            }

            $hasFinite = method_exists($player, 'hasFiniteResources') ? $player->hasFiniteResources() : (method_exists($player, 'isCreative') ? !$player->isCreative() : true);

            if ($hasFinite && $inventory === null) {
                return ItemUseResult::FAIL;
            }

            $location = $player->getLocation();
            $yaw = method_exists($location, 'getYaw') ? $location->getYaw() : (isset($location->yaw) ? (float) $location->yaw : 0.0);
            $pitch = method_exists($location, 'getPitch') ? $location->getPitch() : (isset($location->pitch) ? (float) $location->pitch : 0.0);

            $diff = method_exists($player, 'getItemUseDuration') ? $player->getItemUseDuration() : 20;
            $p = $diff / 20;
            $baseForce = min((($p ** 2) + $p * 2) / 3, 1);

            $launchLocation = Location::fromObject(
                $player->getEyePos(),
                $player->getWorld(),
                ($yaw > 180 ? 360 : 0) - $yaw,
                -$pitch
            );

            $entity = null;
            if (class_exists(ArrowEntity::class)) {
                $entity = new ArrowEntity(
                    $launchLocation,
                    $player,
                    $baseForce >= 1
                );
            } elseif (class_exists(\pocketmine\entity\projectile\Arrow::class)) {
                $entity = new \pocketmine\entity\projectile\Arrow(
                    $launchLocation,
                    $player,
                    $baseForce >= 1
                );
            }

            if ($entity === null) {
                return ItemUseResult::FAIL;
            }

            $direction = $player->getDirectionVector();
            $entity->setMotion($direction);

            $infinity = false;
            if (class_exists(VanillaEnchantments::class) && method_exists(VanillaEnchantments::class, 'INFINITY')) {
                $infinity = $this->hasEnchantment(VanillaEnchantments::INFINITY());
            }
            if ($infinity && method_exists($entity, 'setPickupMode')) {
                $pickupMode = defined(ArrowEntity::class . '::PICKUP_CREATIVE') ? ArrowEntity::PICKUP_CREATIVE : 2;
                $entity->setPickupMode($pickupMode);
            }
            if (class_exists(VanillaEnchantments::class) && method_exists(VanillaEnchantments::class, 'PUNCH')) {
                if (($punchLevel = $this->getEnchantmentLevel(VanillaEnchantments::PUNCH())) > 0 && method_exists($entity, 'setPunchKnockback')) {
                    $entity->setPunchKnockback($punchLevel);
                }
            }
            if (class_exists(VanillaEnchantments::class) && method_exists(VanillaEnchantments::class, 'POWER')) {
                if (($powerLevel = $this->getEnchantmentLevel(VanillaEnchantments::POWER())) > 0 && method_exists($entity, 'getBaseDamage') && method_exists($entity, 'setBaseDamage')) {
                    $entity->setBaseDamage($entity->getBaseDamage() + (($powerLevel + 1) / 2));
                }
            }
            if (class_exists(VanillaEnchantments::class) && method_exists(VanillaEnchantments::class, 'FLAME')) {
                if ($this->hasEnchantment(VanillaEnchantments::FLAME()) && method_exists($entity, 'setOnFire')) {
                    $fireTicks = method_exists($entity, 'getFireTicks') ? $entity->getFireTicks() : 0;
                    $entity->setOnFire(intdiv($fireTicks, 20) + 100);
                }
            }

            $ev = null;
            if (class_exists(EntityShootBowEvent::class)) {
                $ev = new EntityShootBowEvent($player, $this, $entity, $baseForce * 3);
                if ($baseForce < 0.1 || $diff < 5 || (method_exists($player, 'isSpectator') && $player->isSpectator())) {
                    $ev->cancel();
                }
                $ev->call();
                $entity = $ev->getProjectile();

                if ($ev->isCancelled()) {
                    if (method_exists($entity, 'flagForDespawn')) {
                        $entity->flagForDespawn();
                    } elseif (method_exists($entity, 'close')) {
                        $entity->close();
                    }
                    return ItemUseResult::FAIL;
                }
            }

            $forceMultiplier = $ev !== null ? $ev->getForce() : ($baseForce * 3);
            $entity->setMotion($entity->getMotion()->multiply($forceMultiplier));

            if ($entity instanceof Projectile) {
                if (class_exists(ProjectileLaunchEvent::class)) {
                    $projectileEv = new ProjectileLaunchEvent($entity);
                    $projectileEv->call();
                    if ($projectileEv->isCancelled()) {
                        if (method_exists($entity, 'flagForDespawn')) {
                            $entity->flagForDespawn();
                        } elseif (method_exists($entity, 'close')) {
                            $entity->close();
                        }
                        return ItemUseResult::FAIL;
                    }
                }

                $entity->spawnToAll();
                if (class_exists(BowShootSound::class)) {
                    $location->getWorld()->addSound($location, new BowShootSound());
                }
            } else {
                $entity->spawnToAll();
            }

            if ($hasFinite) {
                if (!$infinity && $inventory !== null) {
                    $inventory->removeItem($arrow);
                }
                $this->applyDamage(1);
            }

            return ItemUseResult::SUCCESS;
        }

        public function canStartUsingItem(Player $player): bool
        {
            $hasFinite = method_exists($player, 'hasFiniteResources') ? $player->hasFiniteResources() : (method_exists($player, 'isCreative') ? !$player->isCreative() : true);
            if (!$hasFinite) {
                return true;
            }
            $arrow = VanillaItems::ARROW();
            return (method_exists($player, 'getOffHandInventory') && $player->getOffHandInventory() !== null && $player->getOffHandInventory()->contains($arrow)) ||
                (method_exists($player, 'getInventory') && $player->getInventory() !== null && $player->getInventory()->contains($arrow));
        }
    }

    class Bowl extends Item
    {
        public function getMaxStackSize(): int
        {
            return 64;
        }

        public function getFuelTime(): int
        {
            return 200;
        }
    }

    class Bucket extends Item
    {
        public function getMaxStackSize(): int
        {
            return 16;
        }

        public function onInteractBlock(Player $player, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, array &$returnedItems): ItemUseResult
        {
            if ($blockClicked instanceof Liquid && $blockClicked->isSource()) {
                $resultItem = match ($blockClicked->getTypeId()) {
                    BlockTypeIds::LAVA => VanillaItems::LAVA_BUCKET(),
                    BlockTypeIds::WATER => VanillaItems::WATER_BUCKET(),
                    default => null
                };
                if ($resultItem === null) {
                    return ItemUseResult::FAIL;
                }

                $ev = null;
                if (class_exists(PlayerBucketFillEvent::class)) {
                    $ev = new PlayerBucketFillEvent($player, $blockReplace, $face, $this, $resultItem);
                    $ev->call();
                    if ($ev->isCancelled()) {
                        return ItemUseResult::FAIL;
                    }
                }

                $player->getWorld()->setBlock($blockClicked->getPosition(), VanillaBlocks::AIR());
                if (method_exists($blockClicked, 'getBucketFillSound')) {
                    $player->getWorld()->addSound($blockClicked->getPosition()->add(0.5, 0.5, 0.5), $blockClicked->getBucketFillSound());
                }

                $this->pop();
                $returnedItems[] = $ev !== null ? $ev->getItem() : $resultItem;
                return ItemUseResult::SUCCESS;
            }

            return ItemUseResult::NONE;
        }
    }

    class LiquidBucket extends Item
    {
        private Block $liquid;

        public function __construct(ItemIdentifier $identifier, string $name, Block $liquid)
        {
            parent::__construct($identifier, $name);
            $this->liquid = $liquid;
        }

        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function getFuelTime(): int
        {
            if ($this->liquid instanceof Lava || $this->liquid->getTypeId() === BlockTypeIds::LAVA) {
                return 20000;
            }

            return 0;
        }

        public function getFuelResidue(): Item
        {
            return VanillaItems::BUCKET();
        }

        public function onInteractBlock(Player $player, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, array &$returnedItems): ItemUseResult
        {
            if (!$blockReplace->canBeReplaced()) {
                return ItemUseResult::NONE;
            }

            $resultBlock = clone $this->liquid;

            $ev = null;
            if (class_exists(PlayerBucketEmptyEvent::class)) {
                $ev = new PlayerBucketEmptyEvent($player, $blockReplace, $face, $this, VanillaItems::BUCKET());
                $ev->call();
                if ($ev->isCancelled()) {
                    return ItemUseResult::FAIL;
                }
            }

            $placedBlock = method_exists($resultBlock, 'getFlowingForm') ? $resultBlock->getFlowingForm() : $resultBlock;
            $player->getWorld()->setBlock($blockReplace->getPosition(), $placedBlock);
            if (method_exists($resultBlock, 'getBucketEmptySound')) {
                $player->getWorld()->addSound($blockReplace->getPosition()->add(0.5, 0.5, 0.5), $resultBlock->getBucketEmptySound());
            }

            $this->pop();
            $returnedItems[] = $ev !== null ? $ev->getItem() : VanillaItems::BUCKET();
            return ItemUseResult::SUCCESS;
        }

        public function getLiquid(): Block
        {
            return $this->liquid;
        }
    }

    class Clock extends Item
    {
    }

    class Compass extends Item
    {
    }

    class Coal extends Item
    {
        public function getFuelTime(): int
        {
            return 1600;
        }
    }

    class CocoaBeans extends Item
    {
        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::COCOA_POD();
        }
    }

    class Dye extends Item
    {
        private DyeColor $color = DyeColor::BLACK;

        protected function describeState(RuntimeDataDescriber $w): void
        {
            $w->enum($this->color);
        }

        public function getColor(): DyeColor
        {
            return $this->color;
        }

        public function setColor(DyeColor $color): self
        {
            $this->color = $color;
            return $this;
        }
    }

    class Fertilizer extends Item
    {
        public function onInteractBlock(Player $player, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, array &$returnedItems): ItemUseResult
        {
            if ($blockClicked->onInteract($this, $face, $clickVector, $player, $returnedItems)) {
                return ItemUseResult::SUCCESS;
            }

            return ItemUseResult::NONE;
        }
    }

    class FireCharge extends Item
    {
        public function onInteractBlock(Player $player, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, array &$returnedItems): ItemUseResult
        {
            if ($blockReplace->getTypeId() === BlockTypeIds::AIR) {
                $world = $player->getWorld();
                $world->setBlock($blockReplace->getPosition(), VanillaBlocks::FIRE());
                if (class_exists(BlazeShootSound::class)) {
                    $world->addSound($blockReplace->getPosition()->add(0.5, 0.5, 0.5), new BlazeShootSound());
                }

                $this->pop();

                return ItemUseResult::SUCCESS;
            }

            return ItemUseResult::NONE;
        }
    }

    class GlassBottle extends Item
    {
        public function getMaxStackSize(): int
        {
            return 64;
        }

        public function onInteractBlock(Player $player, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, array &$returnedItems): ItemUseResult
        {
            if ($blockClicked->getTypeId() === BlockTypeIds::WATER) {
                $this->pop();
                $potion = VanillaItems::POTION();
                if (method_exists($potion, 'setType')) {
                    $potionClass = '\\pocketmine\\item\\PotionType';
                    if (enum_exists($potionClass)) {
                        $waterCase = defined($potionClass . '::WATER') ? constant($potionClass . '::WATER') : null;
                        if ($waterCase !== null) {
                            $potion->setType($waterCase);
                        }
                    }
                }
                $returnedItems[] = $potion;

                return ItemUseResult::SUCCESS;
            }

            return ItemUseResult::NONE;
        }
    }

    class GoatHorn extends Item implements Releasable
    {
        private GoatHornType $goatHornType = GoatHornType::PONDER;

        protected function describeState(RuntimeDataDescriber $w): void
        {
            $w->enum($this->goatHornType);
        }

        public function getHornType(): GoatHornType
        {
            return $this->goatHornType;
        }

        public function setHornType(GoatHornType $type): self
        {
            $this->goatHornType = $type;
            return $this;
        }

        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function getCooldownTicks(): int
        {
            return 140;
        }

        public function getCooldownTag(): ?string
        {
            return ItemCooldownTags::GOAT_HORN;
        }

        public function canStartUsingItem(Player $player): bool
        {
            return true;
        }

        public function onClickAir(Player $player, Vector3 $directionVector, array &$returnedItems): ItemUseResult
        {
            $position = $player->getPosition();
            if (class_exists(GoatHornSound::class)) {
                $position->getWorld()->addSound($position, new GoatHornSound($this->goatHornType));
            }

            return ItemUseResult::SUCCESS;
        }
    }

    class MelonSeeds extends Item
    {
        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::MELON_STEM();
        }
    }

    class PumpkinSeeds extends Item
    {
        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::PUMPKIN_STEM();
        }
    }

    class WheatSeeds extends Item
    {
        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::WHEAT();
        }
    }

    final class TorchflowerSeeds extends Item
    {
        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::TORCHFLOWER_CROP();
        }
    }

    final class PitcherPod extends Item
    {
        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::PITCHER_CROP();
        }
    }

    class NameTag extends Item
    {
        public function onInteractEntity(Player $player, Entity $entity, Vector3 $clickVector): bool
        {
            $canRename = method_exists($entity, 'canBeRenamed') ? $entity->canBeRenamed() : !($entity instanceof Player);
            if ($canRename && $this->hasCustomName()) {
                $entity->setNameTag($this->getCustomName());
                $this->pop();
                return true;
            }
            return false;
        }
    }

    class Redstone extends Item
    {
        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::REDSTONE_WIRE();
        }
    }

    class Spyglass extends Item implements Releasable
    {
        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function canStartUsingItem(Player $player): bool
        {
            return true;
        }
    }

    class Stick extends Item
    {
        public function getFuelTime(): int
        {
            return 100;
        }
    }

    class StringItem extends Item
    {
        public function getBlock(?int $clickedFace = null): Block
        {
            return VanillaBlocks::TRIPWIRE();
        }
    }

    class Totem extends Item
    {
        public function getMaxStackSize(): int
        {
            return 1;
        }
    }
}
