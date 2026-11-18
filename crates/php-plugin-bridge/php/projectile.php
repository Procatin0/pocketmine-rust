<?php

declare(strict_types=1);

/**
 * `pocketmine\entity\projectile\*`: los proyectiles del core de PM5.
 *
 * El motor nativo simula el vuelo, el impacto contra bloque/entidad y el daño
 * (`ProjectileKind`); estas clases guardan el tirador, la velocidad inicial y
 * crean el actor con `entity.spawn_projectile`.
 */
namespace pocketmine\entity\projectile {

    use pocketmine\entity\Entity;
    use pocketmine\entity\Living;
    use pocketmine\math\Vector3;
    use pocketmine\world\Location;

    /**
     * `pocketmine\entity\projectile\Projectile`.
     *
     * `motion` es la velocidad inicial: si hay tirador se usa su dirección de
     * mirada escalada por `getInitialSpeed()`; si no, el proyectil nace quieto
     * y el plugin debe llamar a `setMotion()`.
     */
    abstract class Projectile extends Entity
    {
        private ?string $shootingEntityName = null;
        private bool $critical = false;

        public function __construct(Location $location, ?Living $shootingEntity = null, bool $critical = false)
        {
            $this->location = $location;
            $this->shootingEntityName = $shootingEntity?->getName();
            $this->critical = $critical;
            $direction = $shootingEntity?->getDirectionVector() ?? new Vector3(0.0, 0.0, 0.0);
            $speed = $this->getInitialSpeed();
            $this->motion = new Vector3(
                $direction->x * $speed,
                $direction->y * $speed,
                $direction->z * $speed
            );
        }

        /** `Projectile::getResultDamage()`: daño del impacto directo. */
        public function getResultDamage(): float
        {
            return 0.0;
        }

        /** Velocidad inicial por clase (bloques/tick). */
        protected function getInitialSpeed(): float
        {
            return 1.5;
        }

        public function getOwningEntityName(): ?string
        {
            return $this->shootingEntityName;
        }

        public function isCritical(): bool
        {
            return $this->critical;
        }

        public function setCritical(bool $critical): void
        {
            $this->critical = $critical;
        }

        protected function spawnHostCall(): ?array
        {
            $location = $this->getLocation();
            $motion = $this->getMotion();
            return ['entity.spawn_projectile', [
                'type' => $this->getInstanceTypeId(),
                'x' => $location->x,
                'y' => $location->y,
                'z' => $location->z,
                'motion_x' => $motion->x,
                'motion_y' => $motion->y,
                'motion_z' => $motion->z,
                'owner' => $this->shootingEntityName ?? '',
                'owner_runtime_id' => 0,
                'damage' => $this->getResultDamage(),
                'critical' => $this->critical,
            ]];
        }
    }

    /** `pocketmine\entity\projectile\Arrow`. */
    class Arrow extends Projectile
    {
        public const NETWORK_TYPE_ID = 'minecraft:arrow';

        public function getResultDamage(): float
        {
            return 2.0;
        }

        protected function getInitialSpeed(): float
        {
            return 3.0;
        }
    }

    /** `pocketmine\entity\projectile\Snowball` (sin daño directo). */
    class Snowball extends Projectile
    {
        public const NETWORK_TYPE_ID = 'minecraft:snowball';
    }

    /** `pocketmine\entity\projectile\Egg`. */
    class Egg extends Projectile
    {
        public const NETWORK_TYPE_ID = 'minecraft:egg';
    }

    /** `pocketmine\entity\projectile\EnderPearl` (teletransporta al tirador). */
    class EnderPearl extends Projectile
    {
        public const NETWORK_TYPE_ID = 'minecraft:ender_pearl';
    }

    /** `pocketmine\entity\projectile\SplashPotion`. */
    class SplashPotion extends Projectile
    {
        public const NETWORK_TYPE_ID = 'minecraft:splash_potion';
    }

    /** `pocketmine\entity\projectile\ExperienceBottle`. */
    class ExperienceBottle extends Projectile
    {
        public const NETWORK_TYPE_ID = 'minecraft:xp_bottle';
    }

    /** `pocketmine\entity\projectile\ThrownTrident`. */
    class ThrownTrident extends Projectile
    {
        public const NETWORK_TYPE_ID = 'minecraft:thrown_trident';

        public function getResultDamage(): float
        {
            return 8.0;
        }
    }

    /** `pocketmine\entity\projectile\IceBomb`. */
    class IceBomb extends Projectile
    {
        public const NETWORK_TYPE_ID = 'minecraft:ice_bomb';
    }
}
