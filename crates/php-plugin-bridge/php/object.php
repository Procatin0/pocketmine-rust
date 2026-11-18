<?php

declare(strict_types=1);

/**
 * `pocketmine\entity\object\*`: las entidades objeto del core de PM5.
 *
 * El motor nativo ya simula las cuatro (`spawn_item`,
 * `spawn_falling_block`, `spawn_primed_tnt`, `spawn_experience_orb`); estas
 * clases solo llevan el estado PHP y crean la entidad nativa en
 * `spawnToAll()` con el host call correcto (`Entity::spawnHostCall()`).
 */
namespace pocketmine\entity\object {

    use pocketmine\block\Block;
    use pocketmine\entity\Entity;
    use pocketmine\item\Item;
    use pocketmine\math\Vector3;
    use pocketmine\world\Location;

    /**
     * `pocketmine\entity\object\ItemEntity`: el stack tirado en el suelo.
     *
     * El motor se encarga del pickup delay, del despawn a los 6000 ticks y de
     * la fusión de stacks cercanos; PHP solo guarda el `Item` que se le dio.
     */
    class ItemEntity extends Entity
    {
        public const NETWORK_TYPE_ID = 'minecraft:item';

        public const DEFAULT_DESPAWN_DELAY = 6000;
        public const MERGE_CHECK_PERIOD = 20;
        public const DEFAULT_PICKUP_DELAY = 10;

        private Item $item;
        private int $pickupDelay;

        public function __construct(Location $location, Item $item, ?int $pickupDelay = null)
        {
            $this->location = $location;
            $this->item = clone $item;
            $this->pickupDelay = $pickupDelay ?? self::DEFAULT_PICKUP_DELAY;
            $this->motion = new Vector3(0.0, 0.0, 0.0);
        }

        public function getItem(): Item
        {
            return clone $this->item;
        }

        public function setItem(Item $item): void
        {
            $this->item = clone $item;
        }

        public function getPickupDelay(): int
        {
            return $this->pickupDelay;
        }

        public function setPickupDelay(int $pickupDelay): void
        {
            $this->pickupDelay = $pickupDelay;
        }

        protected function spawnHostCall(): ?array
        {
            $location = $this->getLocation();
            $motion = $this->getMotion();
            return ['entity.spawn_item', [
                'x' => $location->x,
                'y' => $location->y,
                'z' => $location->z,
                'owner' => '',
                'motion_x' => $motion->x,
                'motion_y' => $motion->y,
                'motion_z' => $motion->z,
                'item' => $this->item->toArray(),
            ]];
        }
    }

    /**
     * `pocketmine\entity\object\FallingBlock`: el bloque que cae (arena,
     * grava, yunque). El bloque viaja como id de estado en la entidad nativa.
     */
    class FallingBlock extends Entity
    {
        public const NETWORK_TYPE_ID = 'minecraft:falling_block';

        private Block $block;

        public function __construct(Location $location, Block $block)
        {
            $this->location = $location;
            $this->block = clone $block;
            $this->motion = new Vector3(0.0, 0.0, 0.0);
        }

        public function getBlock(): Block
        {
            return clone $this->block;
        }

        protected function spawnHostCall(): ?array
        {
            $location = $this->getLocation();
            return ['entity.spawn_falling_block', [
                'x' => (int)floor($location->x),
                'y' => (int)floor($location->y),
                'z' => (int)floor($location->z),
                'block_id' => $this->block->getStateId(),
            ]];
        }
    }

    /** `pocketmine\entity\object\PrimedTNT`: la TNT con mecha encendida. */
    class PrimedTNT extends Entity
    {
        public const NETWORK_TYPE_ID = 'minecraft:tnt';

        /** `PrimedTNT::DEFAULT_FUSE` (80 ticks). */
        public const DEFAULT_FUSE = 80;

        private int $fuse;

        public function __construct(Location $location, ?int $fuse = null)
        {
            $this->location = $location;
            $this->fuse = $fuse ?? self::DEFAULT_FUSE;
            $this->motion = new Vector3(0.0, 0.0, 0.0);
        }

        public function getFuse(): int
        {
            return $this->fuse;
        }

        public function setFuse(int $fuse): void
        {
            $this->fuse = max(0, $fuse);
        }

        protected function spawnHostCall(): ?array
        {
            $location = $this->getLocation();
            return ['entity.spawn_tnt', [
                'x' => $location->x,
                'y' => $location->y,
                'z' => $location->z,
                'fuse' => $this->fuse,
            ]];
        }
    }

    /** `pocketmine\entity\object\ExperienceOrb`: la esfera de XP. */
    class ExperienceOrb extends Entity
    {
        public const NETWORK_TYPE_ID = 'minecraft:xp_orb';

        private int $value;

        public function __construct(Location $location, int $value)
        {
            $this->location = $location;
            $this->value = max(0, $value);
            $this->motion = new Vector3(0.0, 0.0, 0.0);
        }

        public function getValue(): int
        {
            return $this->value;
        }

        public function setValue(int $value): void
        {
            $this->value = max(0, $value);
        }

        protected function spawnHostCall(): ?array
        {
            $location = $this->getLocation();
            return ['entity.spawn_xp_orb', [
                'x' => $location->x,
                'y' => $location->y,
                'z' => $location->z,
                'value' => $this->value,
            ]];
        }
    }
}
