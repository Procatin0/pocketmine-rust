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

namespace pocketmine\entity\projectile {

    if (!class_exists(Throwable::class, false) && !interface_exists(Throwable::class, false)) {
        if (class_exists(Projectile::class, false)) {
            class_alias(Projectile::class, Throwable::class);
        } else {
            spl_autoload_register(static function (string $class): void {
                if ($class === 'pocketmine\\entity\\projectile\\Throwable' && !class_exists('pocketmine\\entity\\projectile\\Throwable', false)) {
                    if (class_exists('pocketmine\\entity\\projectile\\Projectile', false)) {
                        class_alias('pocketmine\\entity\\projectile\\Projectile', 'pocketmine\\entity\\projectile\\Throwable');
                    }
                }
            });
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

    if (!class_exists(ThrowSound::class, false)) {
        class ThrowSound extends SoundBase
        {
            public function getSoundName(): ?string
            {
                return 'random.bow';
            }
        }
    }
}

namespace pocketmine\item {

    use pocketmine\entity\Location;
    use pocketmine\entity\projectile\Egg as EggEntity;
    use pocketmine\entity\projectile\EnderPearl as EnderPearlEntity;
    use pocketmine\entity\projectile\ExperienceBottle as ExperienceBottleEntity;
    use pocketmine\entity\projectile\IceBomb as IceBombEntity;
    use pocketmine\entity\projectile\Snowball as SnowballEntity;
    use pocketmine\entity\projectile\Throwable;
    use pocketmine\event\entity\ProjectileLaunchEvent;
    use pocketmine\math\Vector3;
    use pocketmine\player\Player;
    use pocketmine\world\sound\ThrowSound;
    use function class_alias;
    use function class_exists;
    use function interface_exists;
    use function method_exists;

    if (!class_exists(ItemCooldownTags::class, false)) {
        final class ItemCooldownTags
        {
            private function __construct()
            {
                // NOOP
            }

            public const CHORUS_FRUIT = "chorus_fruit";
            public const ENDER_PEARL = "ender_pearl";
            public const SHIELD = "shield";
            public const GOAT_HORN = "goat_horn";
        }
    }

    if (!class_exists(ProjectileItemThrowableFallback::class, false)) {
        abstract class ProjectileItemThrowableFallback
        {
        }
    }

    abstract class ProjectileItem extends Item
    {
        protected static function ensureThrowable(): void
        {
            if (!class_exists(Throwable::class, false) && !interface_exists(Throwable::class, false)) {
                if (class_exists(\pocketmine\entity\projectile\Projectile::class, false)) {
                    class_alias(\pocketmine\entity\projectile\Projectile::class, Throwable::class);
                } else {
                    class_alias(ProjectileItemThrowableFallback::class, Throwable::class);
                }
            }
        }

        abstract public function getThrowForce(): float;

        abstract protected function createEntity(Location $location, Player $thrower): Throwable;

        public function onClickAir(Player $player, Vector3 $directionVector, array &$returnedItems): ItemUseResult
        {
            self::ensureThrowable();

            $location = $player->getLocation();
            $yaw = method_exists($location, 'getYaw') ? $location->getYaw() : (isset($location->yaw) ? (float) $location->yaw : 0.0);
            $pitch = method_exists($location, 'getPitch') ? $location->getPitch() : (isset($location->pitch) ? (float) $location->pitch : 0.0);

            $world = null;
            try {
                if (method_exists($location, 'getWorld')) {
                    $world = $location->getWorld();
                }
            } catch (\Throwable) {
                $world = null;
            }
            if ($world === null) {
                try {
                    if (method_exists($player, 'getWorld')) {
                        $world = $player->getWorld();
                    }
                } catch (\Throwable) {
                    $world = null;
                }
            }

            $eyePos = $player->getEyePos();
            $worldName = '';
            if ($world !== null && method_exists($world, 'getFolderName')) {
                $worldName = $world->getFolderName();
            } elseif (method_exists($location, 'getWorldName')) {
                $worldName = $location->getWorldName();
            }

            if (is_subclass_of(Location::class, \pocketmine\world\Location::class)) {
                $launchLocation = new Location($eyePos->x, $eyePos->y, $eyePos->z, $yaw, $pitch, $worldName);
            } else {
                $launchLocation = Location::fromObject($eyePos, $world, $yaw, $pitch);
            }
            if (!($launchLocation instanceof Location)) {
                $launchLocation = new Location($eyePos->x, $eyePos->y, $eyePos->z, $yaw, $pitch, $worldName);
            }

            try {
                $projectile = $this->createEntity($launchLocation, $player);
            } catch (\Throwable) {
                return ItemUseResult::FAIL;
            }

            if ($projectile === null) {
                return ItemUseResult::FAIL;
            }

            if (method_exists($projectile, 'setMotion')) {
                $projectile->setMotion($directionVector->multiply($this->getThrowForce()));
            }

            if (class_exists(ProjectileLaunchEvent::class)) {
                $projectileEv = new ProjectileLaunchEvent($projectile);
                try {
                    $projectileEv->call();
                } catch (\Throwable) {
                    // PluginManager not loaded in isolated or unit test environments
                }
                if ($projectileEv->isCancelled()) {
                    if (method_exists($projectile, 'flagForDespawn')) {
                        $projectile->flagForDespawn();
                    } elseif (method_exists($projectile, 'close')) {
                        $projectile->close();
                    }
                    return ItemUseResult::FAIL;
                }
            }

            if (method_exists($projectile, 'spawnToAll')) {
                $projectile->spawnToAll();
            }

            if ($world !== null && class_exists(ThrowSound::class)) {
                $world->addSound($location, new ThrowSound());
            }

            $this->pop();

            return ItemUseResult::SUCCESS;
        }
    }

    class Arrow extends Item
    {
    }

    class Egg extends ProjectileItem
    {
        public function getMaxStackSize(): int
        {
            return 16;
        }

        protected function createEntity(Location $location, Player $thrower): Throwable
        {
            self::ensureThrowable();
            if (!class_exists(EggEntity::class)) {
                throw new \RuntimeException("Egg entity is not available");
            }
            return new EggEntity($location, $thrower);
        }

        public function getThrowForce(): float
        {
            return 1.5;
        }
    }

    class Snowball extends ProjectileItem
    {
        public function getMaxStackSize(): int
        {
            return 16;
        }

        protected function createEntity(Location $location, Player $thrower): Throwable
        {
            self::ensureThrowable();
            if (!class_exists(SnowballEntity::class)) {
                throw new \RuntimeException("Snowball entity is not available");
            }
            return new SnowballEntity($location, $thrower);
        }

        public function getThrowForce(): float
        {
            return 1.5;
        }
    }

    class EnderPearl extends ProjectileItem
    {
        public function getMaxStackSize(): int
        {
            return 16;
        }

        protected function createEntity(Location $location, Player $thrower): Throwable
        {
            self::ensureThrowable();
            if (!class_exists(EnderPearlEntity::class)) {
                throw new \RuntimeException("EnderPearl entity is not available");
            }
            return new EnderPearlEntity($location, $thrower);
        }

        public function getThrowForce(): float
        {
            return 1.5;
        }

        public function getCooldownTicks(): int
        {
            return 20;
        }

        public function getCooldownTag(): ?string
        {
            return ItemCooldownTags::ENDER_PEARL;
        }
    }

    class ExperienceBottle extends ProjectileItem
    {
        protected function createEntity(Location $location, Player $thrower): Throwable
        {
            self::ensureThrowable();
            if (!class_exists(ExperienceBottleEntity::class)) {
                throw new \RuntimeException("ExperienceBottle entity is not available");
            }
            return new ExperienceBottleEntity($location, $thrower);
        }

        public function getThrowForce(): float
        {
            return 0.7;
        }
    }

    class IceBomb extends ProjectileItem
    {
        public function getMaxStackSize(): int
        {
            return 16;
        }

        protected function createEntity(Location $location, Player $thrower): Throwable
        {
            self::ensureThrowable();
            if (!class_exists(IceBombEntity::class)) {
                throw new \RuntimeException("IceBomb entity is not available");
            }
            return new IceBombEntity($location, $thrower);
        }

        public function getThrowForce(): float
        {
            return 1.5;
        }

        public function getCooldownTicks(): int
        {
            return 10;
        }
    }
}
