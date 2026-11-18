<?php

declare(strict_types=1);

namespace pocketmine\entity {

    class Location extends \pocketmine\world\Location
    {
    }

    /**
     * `pocketmine\entity\EntitySizeInfo`: alto, ancho y altura de ojos.
     */
    final class EntitySizeInfo
    {
        private float $eyeHeight;

        public function __construct(
            private float $height,
            private float $width,
            ?float $eyeHeight = null
        ) {
            $this->eyeHeight = $eyeHeight ?? min($this->height / 2 + 0.1, $this->height);
        }

        public function getHeight(): float
        {
            return $this->height;
        }

        public function getWidth(): float
        {
            return $this->width;
        }

        public function getEyeHeight(): float
        {
            return $this->eyeHeight;
        }

        public function scale(float $newScale): self
        {
            return new self(
                $this->height * $newScale,
                $this->width * $newScale,
                $this->eyeHeight * $newScale
            );
        }
    }

    /** `pocketmine\entity\Ageable`: `WaterAnimal` y `Villager` lo implementan. */
    interface Ageable
    {
        public function isBaby(): bool;
    }

    abstract class Entity
    {
        public int|string $id = 0;
        protected ?\pocketmine\world\World $world = null;
        protected ?\pocketmine\world\Location $location = null;
        protected string $nameTag = '';
        protected bool $nameTagAlwaysVisible = false;

        /** `Entity::getId()` nativo; `null` mientras no esté en el mundo. */
        protected ?int $nativeUniqueId = null;
        /** Runtime id del actor Bedrock (el que usan los paquetes). */
        protected ?int $nativeRuntimeId = null;
        /** `getNetworkTypeId()` (`EntityIds`): lo fija la subclase o el proxy. */
        protected string $networkTypeId = '';
        protected ?\pocketmine\math\Vector3 $motion = null;
        protected ?EntitySizeInfo $size = null;
        protected bool $closed = false;

        /** `getNetworkTypeId()` de la clase (estático, como en PocketMine). */
        public const NETWORK_TYPE_ID = '';
        /** Último error del host (`tipo desconocido`, tope de entidades...). */
        protected ?string $lastError = null;

        public function getId(): int|string
        {
            return property_exists($this, 'runtimeId') ? $this->runtimeId : $this->id;
        }

        public static function getNetworkTypeId(): string
        {
            return static::NETWORK_TYPE_ID;
        }

        /**
         * Tipo de **esta instancia**: el estático de la clase, o el que fijó
         * `EntityProxy`/`applyNativeSnapshot()` para un tipo dinámico.
         */
        public function getInstanceTypeId(): string
        {
            return $this->networkTypeId !== '' ? $this->networkTypeId : static::getNetworkTypeId();
        }

        /**
         * `Entity::getName()`: por defecto el nombre corto de la clase, como
         * `(new ReflectionClass($this))->getShortName()` en PM.
         */
        public function getName(): string
        {
            return (new \ReflectionClass($this))->getShortName();
        }

        /** `Entity::getInitialSizeInfo()`; cada clase la sobreescribe. */
        protected function getInitialSizeInfo(): EntitySizeInfo
        {
            return new EntitySizeInfo(1.8, 0.6);
        }

        public function getSize(): EntitySizeInfo
        {
            return $this->size ??= $this->getInitialSizeInfo();
        }

        /** `Entity::getBoundingBox()`: centrada en x/z, desde los pies hacia arriba. */
        public function getBoundingBox(): \pocketmine\math\AxisAlignedBB
        {
            $location = $this->getLocation();
            $size = $this->getSize();
            $half = $size->getWidth() / 2;
            return new \pocketmine\math\AxisAlignedBB(
                $location->x - $half,
                $location->y,
                $location->z - $half,
                $location->x + $half,
                $location->y + $size->getHeight(),
                $location->z + $half
            );
        }

        /** `Entity::getDirectionVector()` (`-sin yaw`, `-sin pitch`, `cos yaw`). */
        public function getDirectionVector(): \pocketmine\math\Vector3
        {
            $location = $this->getLocation();
            $yaw = deg2rad($location->getYaw());
            $pitch = deg2rad($location->getPitch());
            return new \pocketmine\math\Vector3(
                -sin($yaw) * cos($pitch),
                -sin($pitch),
                cos($yaw) * cos($pitch)
            );
        }

        public function getMotion(): \pocketmine\math\Vector3
        {
            return $this->motion ??= new \pocketmine\math\Vector3(0.0, 0.0, 0.0);
        }

        /** `Entity::setMotion(Vector3 $motion): bool`. */
        public function setMotion(\pocketmine\math\Vector3 $motion): bool
        {
            $this->motion = $motion;
            if ($this->nativeUniqueId !== null
                && class_exists(\pocketmine\runtime\Bridge::class)
                && \pocketmine\runtime\Bridge::isActive()
            ) {
                $result = \pocketmine\runtime\Bridge::hostCall('entity.set_motion', [
                    'unique_id' => $this->nativeUniqueId,
                    'x' => $motion->x,
                    'y' => $motion->y,
                    'z' => $motion->z,
                ]);
                return is_array($result) && ($result['ok'] ?? false) === true;
            }
            return true;
        }

        /** `Living::knockBack($x, $z, $force, $verticalLimit)`. */
        public function knockBack(
            float $x,
            float $z,
            float $force = 0.4,
            ?float $verticalLimit = 0.4
        ): void {
            if ($this->nativeUniqueId === null
                || !class_exists(\pocketmine\runtime\Bridge::class)
                || !\pocketmine\runtime\Bridge::isActive()
            ) {
                return;
            }
            \pocketmine\runtime\Bridge::hostCall('entity.knock_back', [
                'unique_id' => $this->nativeUniqueId,
                'dx' => $x,
                'dz' => $z,
                'force' => $force,
            ]);
        }

        /** Id de PocketMine de la entidad nativa (`null` si no está spawneada). */
        public function getNativeId(): ?int
        {
            return $this->nativeUniqueId;
        }

        public function isNative(): bool
        {
            return $this->nativeUniqueId !== null;
        }

        public function getLastError(): ?string
        {
            return $this->lastError;
        }

        /** Hidrata la entidad desde un snapshot de `entity.get`/`entity.list`. */
        public function applyNativeSnapshot(array $data): void
        {
            if (isset($data['unique_id'])) {
                $this->nativeUniqueId = (int)$data['unique_id'];
                $this->id = $this->nativeUniqueId;
            }
            if (isset($data['runtime_id'])) {
                $this->nativeRuntimeId = (int)$data['runtime_id'];
                if (property_exists($this, 'runtimeId')) {
                    $this->runtimeId = $this->nativeRuntimeId;
                }
            }
            if (isset($data['type'])) {
                $this->networkTypeId = (string)$data['type'];
            }
            if (isset($data['x'], $data['y'], $data['z'])) {
                $this->location = new \pocketmine\world\Location(
                    (float)$data['x'],
                    (float)$data['y'],
                    (float)$data['z'],
                    (float)($data['yaw'] ?? 0.0),
                    (float)($data['pitch'] ?? 0.0),
                    $this->location?->getWorldName() ?? $this->getWorld()->getName()
                );
                foreach (['x' => 'x', 'y' => 'y', 'z' => 'z', 'yaw' => 'yaw', 'pitch' => 'pitch'] as $key => $property) {
                    if (property_exists($this, $property) && isset($data[$key])) {
                        $this->{$property} = (float)$data[$key];
                    }
                }
            }
            if (isset($data['motion_x'], $data['motion_y'], $data['motion_z'])) {
                $this->motion = new \pocketmine\math\Vector3(
                    (float)$data['motion_x'],
                    (float)$data['motion_y'],
                    (float)$data['motion_z']
                );
            }
            if (isset($data['height'], $data['width'])) {
                $this->size = new EntitySizeInfo(
                    (float)$data['height'],
                    (float)$data['width'],
                    isset($data['eye_height']) ? (float)$data['eye_height'] : null
                );
            }
            if (isset($data['name_tag'])) {
                $this->nameTag = (string)$data['name_tag'];
            }
            if (isset($data['health']) && property_exists($this, 'health')) {
                $this->health = (float)$data['health'];
            }
            if (isset($data['max_health']) && property_exists($this, 'maxHealth')) {
                $this->maxHealth = (float)$data['max_health'];
            }
            if (array_key_exists('alive', $data)) {
                $this->closed = !((bool)$data['alive']);
            }
        }

        public function getWorld(): \pocketmine\world\World
        {
            if ($this->world !== null) {
                return $this->world;
            }
            if (property_exists($this, 'worldName') && class_exists(\pocketmine\Server::class, false)) {
                $world = \pocketmine\Server::getInstance()->getWorldManager()->getWorldByName($this->worldName);
                if ($world !== null) {
                    return $world;
                }
            }
            if (class_exists(\pocketmine\Server::class, false)) {
                return \pocketmine\Server::getInstance()->getWorldManager()->getDefaultWorld();
            }
            return new \pocketmine\world\World('world');
        }

        public function getLocation(): \pocketmine\world\Location
        {
            if (property_exists($this, 'x') && property_exists($this, 'y') && property_exists($this, 'z')) {
                return new \pocketmine\world\Location(
                    $this->x,
                    $this->y,
                    $this->z,
                    $this->yaw ?? 0.0,
                    $this->pitch ?? 0.0,
                    property_exists($this, 'worldName') ? $this->worldName : 'world'
                );
            }
            return $this->location ??= new \pocketmine\world\Location(0, 0, 0, 0, 0, 'world');
        }

        public function getPosition(): \pocketmine\math\Position
        {
            return $this->getLocation();
        }

        public function getNameTag(): string
        {
            return $this->nameTag;
        }

        public function setNameTag(string $name): void
        {
            $this->nameTag = $name;
            if ($this->nativeUniqueId !== null
                && class_exists(\pocketmine\runtime\Bridge::class)
                && \pocketmine\runtime\Bridge::isActive()
            ) {
                \pocketmine\runtime\Bridge::hostCall('entity.set_name_tag', [
                    'unique_id' => $this->nativeUniqueId,
                    'name_tag' => $name,
                ]);
            }
        }

        public function isNameTagAlwaysVisible(): bool
        {
            return $this->nameTagAlwaysVisible;
        }

        public function setNameTagAlwaysVisible(bool $value = true): void
        {
            $this->nameTagAlwaysVisible = $value;
        }

        /**
         * `Entity::spawnToAll()`. La entidad se crea en el mundo nativo
         * (`entity.spawn`); la difusión a los clientes la hace el tick del
         * servidor por chunks (`Entity::spawnTo`).
         */
        public function spawnToAll(): void
        {
            $this->spawnTo(null);
        }

        /**
         * Host call que crea la entidad nativa. Las subclases con estado propio
         * (`ItemEntity`, `FallingBlock`, `PrimedTNT`, `ExperienceOrb` y los
         * proyectiles) lo sobrescriben para mandar el suyo.
         *
         * @return array{0: string, 1: array<string, mixed>}|null
         */
        protected function spawnHostCall(): ?array
        {
            $location = $this->getLocation();
            return ['entity.spawn', [
                'type' => $this->getInstanceTypeId(),
                'x' => $location->x,
                'y' => $location->y,
                'z' => $location->z,
                'yaw' => $location->getYaw(),
                'pitch' => $location->getPitch(),
                'name_tag' => $this->nameTag,
            ]];
        }

        /**
         * `Entity::spawnTo(Player $player)`. El visor se ignora: la entidad se
         * registra en el mundo nativo y el servidor la difunde a los jugadores
         * que tienen cargado su chunk (lo que en PM hace `spawnTo`).
         */
        public function spawnTo(?\pocketmine\player\Player $player = null): void
        {
            if ($this->nativeUniqueId !== null || $this->getInstanceTypeId() === '') {
                return;
            }
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                $this->lastError = 'bridge inactive';
                return;
            }
            $call = $this->spawnHostCall();
            if ($call === null) {
                $this->lastError = 'unsupported entity';
                return;
            }
            $result = \pocketmine\runtime\Bridge::hostCall($call[0], $call[1]);
            if (is_array($result) && isset($result['entity'])) {
                $this->applyNativeSnapshot($result['entity']);
                $this->lastError = null;
                return;
            }
            $this->lastError = is_array($result)
                ? (string)($result['error'] ?? 'entity.spawn failed')
                : 'entity.spawn failed';
        }

        /** `Entity::kill()`: elimina la entidad del mundo nativo. */
        public function kill(): void
        {
            if ($this->nativeUniqueId !== null
                && class_exists(\pocketmine\runtime\Bridge::class)
                && \pocketmine\runtime\Bridge::isActive()
            ) {
                \pocketmine\runtime\Bridge::hostCall('entity.despawn', ['unique_id' => $this->nativeUniqueId]);
                $this->nativeUniqueId = null;
            }
            $this->closed = true;
        }

        public function isClosed(): bool
        {
            return $this->closed;
        }

        public function isAlive(): bool
        {
            return !$this->closed;
        }

        /**
         * `Entity::teleport(Vector3 $pos, ?float $yaw, ?float $pitch)`.
         * Aplica el movimiento en el mundo nativo cuando la entidad está
         * registrada; si no, solo actualiza la posición local.
         */
        public function teleport(\pocketmine\math\Vector3 $pos, ?float $yaw = null, ?float $pitch = null): bool
        {
            $targetYaw = $yaw ?? ($this->location?->getYaw() ?? 0.0);
            $targetPitch = $pitch ?? ($this->location?->getPitch() ?? 0.0);
            if ($this->nativeUniqueId !== null
                && class_exists(\pocketmine\runtime\Bridge::class)
                && \pocketmine\runtime\Bridge::isActive()
            ) {
                $result = \pocketmine\runtime\Bridge::hostCall('entity.teleport', [
                    'unique_id' => $this->nativeUniqueId,
                    'x' => $pos->x,
                    'y' => $pos->y,
                    'z' => $pos->z,
                    'yaw' => $targetYaw,
                    'pitch' => $targetPitch,
                ]);
                if (!is_array($result) || ($result['ok'] ?? false) !== true) {
                    return false;
                }
            }
            foreach (['x' => $pos->x, 'y' => $pos->y, 'z' => $pos->z, 'yaw' => $targetYaw, 'pitch' => $targetPitch] as $property => $value) {
                if (property_exists($this, $property)) {
                    $this->{$property} = (float)$value;
                }
            }
            $this->location = new \pocketmine\world\Location(
                $pos->x,
                $pos->y,
                $pos->z,
                $targetYaw,
                $targetPitch,
                $this->location?->getWorldName() ?? $this->getWorld()->getName()
            );
            return true;
        }

        /**
         * `Entity::attack(EntityDamageEvent $source)`. Acepta también un número
         * (daño directo) y delega en el host (`entity.damage`).
         */
        public function attack(mixed $source): void
        {
            if (is_numeric($source)) {
                $amount = (float)$source;
            } elseif (is_object($source) && method_exists($source, 'getFinalDamage')) {
                $amount = (float)$source->getFinalDamage();
            } else {
                return;
            }
            if ($amount <= 0.0 || $this->nativeUniqueId === null) {
                return;
            }
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                return;
            }
            $result = \pocketmine\runtime\Bridge::hostCall('entity.damage', [
                'unique_id' => $this->nativeUniqueId,
                'damage' => $amount,
            ]);
            if (is_array($result) && isset($result['entity'])) {
                $this->applyNativeSnapshot($result['entity']);
            }
        }

        public function close(): void
        {
            if ($this->nativeUniqueId !== null) {
                $this->kill();
            }
        }
    }

    class Living extends Entity
    {
        public float $health = 20.0;
        public float $maxHealth = 20.0;

        public function getHealth(): float
        {
            return $this->health;
        }

        public function getMaxHealth(): float
        {
            return $this->maxHealth;
        }

        public function setHealth(float $amount): void
        {
            $this->health = max(0.0, min($this->maxHealth, $amount));
            if ($this->nativeUniqueId !== null
                && class_exists(\pocketmine\runtime\Bridge::class)
                && \pocketmine\runtime\Bridge::isActive()
            ) {
                $result = \pocketmine\runtime\Bridge::hostCall('entity.set_health', [
                    'unique_id' => $this->nativeUniqueId,
                    'health' => $this->health,
                ]);
                if (is_array($result) && isset($result['entity'])) {
                    $this->applyNativeSnapshot($result['entity']);
                }
            }
        }

        public function setMaxHealth(float $amount): void
        {
            $this->maxHealth = max(1.0, $amount);
        }

        // ---- Efectos (`pocketmine\entity\effect`) ----

        private ?\pocketmine\entity\effect\EffectCollection $effects = null;

        /**
         * `Living::getEffects()`: en una entidad nativa lee el estado del
         * motor (`entity.effects`, que decae en `tick_entities`); sin puente
         * devuelve la colección local.
         */
        public function getEffects(): \pocketmine\entity\effect\EffectCollection
        {
            if ($this->nativeUniqueId === null
                || !class_exists(\pocketmine\runtime\Bridge::class)
                || !\pocketmine\runtime\Bridge::isActive()
            ) {
                return $this->effects ??= new \pocketmine\entity\effect\EffectCollection();
            }
            $result = \pocketmine\runtime\Bridge::hostCall('entity.effects', [
                'unique_id' => $this->nativeUniqueId,
            ]);
            $collection = new \pocketmine\entity\effect\EffectCollection();
            foreach ((is_array($result) ? ($result['effects'] ?? []) : []) as $data) {
                if (!is_array($data)) {
                    continue;
                }
                $type = \pocketmine\data\bedrock\effect\EffectIdMap::fromId((int)($data['id'] ?? 0));
                if ($type === null) {
                    continue;
                }
                $collection->add(new \pocketmine\entity\effect\EffectInstance(
                    $type,
                    (int)($data['duration'] ?? 0),
                    (int)($data['amplifier'] ?? 0),
                    (bool)($data['visible'] ?? true),
                    (bool)($data['ambient'] ?? false),
                    null,
                    (bool)($data['infinite'] ?? false)
                ));
            }
            return $this->effects = $collection;
        }

        /**
         * `Living::addEffect(EffectInstance): bool`. Un efecto sin id Bedrock
         * (custom) no se puede guardar en el motor y devuelve `false`.
         */
        public function addEffect(\pocketmine\entity\effect\EffectInstance $effect): bool
        {
            $id = \pocketmine\data\bedrock\effect\EffectIdMap::toId($effect->getType());
            if ($id === null) {
                return false;
            }
            if ($this->nativeUniqueId !== null
                && class_exists(\pocketmine\runtime\Bridge::class)
                && \pocketmine\runtime\Bridge::isActive()
            ) {
                $result = \pocketmine\runtime\Bridge::hostCall('entity.add_effect', [
                    'unique_id' => $this->nativeUniqueId,
                    'id' => $id,
                    'duration' => $effect->getDuration(),
                    'amplifier' => $effect->getAmplifier(),
                    'visible' => $effect->isVisible(),
                    'ambient' => $effect->isAmbient(),
                    'infinite' => $effect->isInfinite(),
                ]);
                if (!is_array($result) || empty($result['ok'])) {
                    return false;
                }
            }
            $this->getEffects()->add($effect);
            // `Effect::add()`: los efectos instantáneos se aplican al añadirlos.
            $type = $effect->getType();
            if ($type instanceof \pocketmine\entity\effect\InstantEffect) {
                $type->applyEffect($this, $effect);
            }
            return true;
        }

        /** `Living::removeEffect(Effect): bool`. */
        public function removeEffect(\pocketmine\entity\effect\Effect $effect): bool
        {
            $id = \pocketmine\data\bedrock\effect\EffectIdMap::toId($effect);
            if ($id === null) {
                return false;
            }
            $removed = false;
            if ($this->nativeUniqueId !== null
                && class_exists(\pocketmine\runtime\Bridge::class)
                && \pocketmine\runtime\Bridge::isActive()
            ) {
                $result = \pocketmine\runtime\Bridge::hostCall('entity.remove_effect', [
                    'unique_id' => $this->nativeUniqueId,
                    'id' => $id,
                ]);
                $removed = is_array($result) && !empty($result['ok']);
            }
            if ($this->effects !== null && $this->effects->has($effect)) {
                $this->effects->remove($effect);
                $removed = true;
            }
            return $removed;
        }

        /** `Living::hasEffect(Effect): bool`. */
        public function hasEffect(\pocketmine\entity\effect\Effect $effect): bool
        {
            return $this->getEffect($effect) !== null;
        }

        /** `Living::getEffect(Effect): ?EffectInstance`. */
        public function getEffect(\pocketmine\entity\effect\Effect $effect): ?\pocketmine\entity\effect\EffectInstance
        {
            return $this->getEffects()->get($effect);
        }

        /** `Living::removeAllEffects(): bool`. */
        public function removeAllEffects(): bool
        {
            $removed = $this->effects !== null && $this->effects->count() > 0;
            if ($this->nativeUniqueId !== null
                && class_exists(\pocketmine\runtime\Bridge::class)
                && \pocketmine\runtime\Bridge::isActive()
            ) {
                $result = \pocketmine\runtime\Bridge::hostCall('entity.clear_effects', [
                    'unique_id' => $this->nativeUniqueId,
                ]);
                $removed = $removed || (is_array($result) && ((int)($result['removed'] ?? 0)) > 0);
            }
            $this->effects?->clear();
            return $removed;
        }

        /** `Living::getDrops()`: sin tabla propia, sin drops. */
        public function getDrops(): array
        {
            return [];
        }

        /** `Living::getXpDropAmount()`; `Zombie` devuelve 5. */
        public function getXpDropAmount(): int
        {
            return 0;
        }
    }

    class Human extends Living
    {
        public const NETWORK_TYPE_ID = 'minecraft:player';

        protected ?Skin $skin = null;

        public function getSkin(): object
        {
            return $this->skin ??= new Skin();
        }

        public function setSkin(Skin $skin): void
        {
            $this->skin = $skin;
        }
    }

    /**
     * `pocketmine\entity\WaterAnimal` (abstracta): animal acuático.
     * `canBreathe()`/`onAirExpired()` dependen del estado de agua de la
     * sesión nativa, que aún no se consulta; se conserva la firma.
     */
    abstract class WaterAnimal extends Living implements Ageable
    {
        protected bool $baby = false;

        public function isBaby(): bool
        {
            return $this->baby;
        }

        public function setBaby(bool $baby = true): void
        {
            $this->baby = $baby;
        }

        public function canBreathe(): bool
        {
            return false;
        }

        public function onAirExpired(): void
        {
        }
    }

    /** `pocketmine\entity\Zombie`: 20 PV, 1.9x0.6, drops de podredumbre. */
    class Zombie extends Living
    {
        public const NETWORK_TYPE_ID = 'minecraft:zombie';

        public function __construct(?\pocketmine\world\Location $location = null, mixed $nbt = null)
        {
            if ($location !== null) {
                $this->location = $location;
            }
        }

        protected function getInitialSizeInfo(): EntitySizeInfo
        {
            return new EntitySizeInfo(1.9, 0.6);
        }

        public function getName(): string
        {
            return 'Zombie';
        }

        public function getDrops(): array
        {
            $flesh = \pocketmine\item\ItemFactory::fromString('minecraft:rotten_flesh');
            $flesh->setCount(mt_rand(0, 2));
            $drops = [$flesh];
            if (mt_rand(0, 199) < 5) {
                $drops[] = match (mt_rand(0, 2)) {
                    0 => \pocketmine\item\ItemFactory::fromString('minecraft:iron_ingot'),
                    1 => \pocketmine\item\ItemFactory::fromString('minecraft:carrot'),
                    default => \pocketmine\item\ItemFactory::fromString('minecraft:potato'),
                };
            }
            return $drops;
        }

        public function getXpDropAmount(): int
        {
            return 5;
        }
    }

    /** `pocketmine\entity\Villager`: 20 PV, 1.9x0.6, cinco profesiones. */
    class Villager extends Living implements Ageable
    {
        public const NETWORK_TYPE_ID = 'minecraft:villager';

        public const PROFESSION_FARMER = 0;
        public const PROFESSION_LIBRARIAN = 1;
        public const PROFESSION_PRIEST = 2;
        public const PROFESSION_BLACKSMITH = 3;
        public const PROFESSION_BUTCHER = 4;

        private bool $baby = false;
        private int $profession = self::PROFESSION_FARMER;

        public function __construct(?\pocketmine\world\Location $location = null, mixed $nbt = null)
        {
            if ($location !== null) {
                $this->location = $location;
            }
        }

        protected function getInitialSizeInfo(): EntitySizeInfo
        {
            return new EntitySizeInfo(1.9, 0.6);
        }

        public function getName(): string
        {
            return 'Villager';
        }

        public function isBaby(): bool
        {
            return $this->baby;
        }

        public function setBaby(bool $baby = true): void
        {
            $this->baby = $baby;
        }

        public function getProfession(): int
        {
            return $this->profession;
        }

        /** `Villager::setProfession()`: fuera de 0..4 se usa `FARMER`. */
        public function setProfession(int $profession): void
        {
            $this->profession = ($profession > 4 || $profession < 0)
                ? self::PROFESSION_FARMER
                : $profession;
        }
    }

    /** `pocketmine\entity\Squid`: 10 PV, 0.8x0.6, saca de tinta. */
    class Squid extends WaterAnimal
    {
        public const NETWORK_TYPE_ID = 'minecraft:squid';

        public function __construct(?\pocketmine\world\Location $location = null, mixed $nbt = null)
        {
            if ($location !== null) {
                $this->location = $location;
            }
        }

        protected function getInitialSizeInfo(): EntitySizeInfo
        {
            return new EntitySizeInfo(0.8, 0.6);
        }

        public function getName(): string
        {
            return 'Squid';
        }

        public function getDrops(): array
        {
            $ink = \pocketmine\item\ItemFactory::fromString('minecraft:ink_sac');
            $ink->setCount(mt_rand(1, 3));
            return [$ink];
        }
    }

    /**
     * PM `pocketmine\entity\Skin` stub (Fase A1): skin Steve por defecto.
     * TODO(A1): textura 64x64/128x128 real + geometría + capa + `setSkin`.
     */
    class Skin
    {
        public function __construct(
            private string $skinId = 'standard',
            private string $skinData = '',
            private string $capeData = '',
            private string $geometryData = ''
        ) {
        }

        public function getSkinId(): string
        {
            return $this->skinId;
        }

        public function getSkinData(): string
        {
            return $this->skinData;
        }

        public function getCapeData(): string
        {
            return $this->capeData;
        }

        public function getGeometryData(): string
        {
            return $this->geometryData;
        }

        public function __toString(): string
        {
            return 'Skin(' . $this->skinId . ')';
        }
    }

    class EntityFactory
    {
        private static ?self $instance = null;
        /** @var array<string, array{class: string, creator: \Closure, saveNames: string[]}> */
        private array $creators = [];
        /** @var array<string, string> */
        private array $saveNames = [];

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
        }

        public function register(string $className, \Closure $creationFunc, array $saveNames = []): void
        {
            $this->creators[$className] = [
                'class' => $className,
                'creator' => $creationFunc,
                'saveNames' => $saveNames,
            ];
            foreach ($saveNames as $saveName) {
                $this->saveNames[strtolower($saveName)] = $className;
            }
        }

        public function canCreate(string $saveName): bool
        {
            return isset($this->saveNames[strtolower($saveName)]);
        }

        public function createFromData(\pocketmine\world\World $world, mixed $nbt): ?Entity
        {
            $id = '';
            if (is_object($nbt) && method_exists($nbt, 'hasTag') && $nbt->hasTag('id')) {
                $id = $nbt->getString('id');
            }
            $className = $this->saveNames[strtolower($id)] ?? null;
            if ($className !== null && isset($this->creators[$className])) {
                return ($this->creators[$className]['creator'])($world, $nbt);
            }
            return null;
        }

        public function getSaveNames(): array
        {
            return array_keys($this->saveNames);
        }

        /**
         * `EntityFactory::create()`: construye la instancia PHP de un tipo de
         * entidad. Hoy devuelve `EntityProxy`; cada tipo con clase propia
         * (`Zombie`, `Cow`, ...) se añade en la fase 2 y se registra aquí.
         */
        public function create(
            string $networkTypeId,
            \pocketmine\world\Location $location,
            ?string $nameTag = null
        ): ?Entity {
            if (!str_contains($networkTypeId, ':')) {
                $networkTypeId = 'minecraft:' . $networkTypeId;
            }
            if ($networkTypeId === 'minecraft:player') {
                return null;
            }
            $map = [
                'minecraft:zombie' => Zombie::class,
                'minecraft:villager' => Villager::class,
                'minecraft:squid' => Squid::class,
            ];
            $class = $map[strtolower($networkTypeId)] ?? null;
            if ($class !== null) {
                /** @var Entity $entity */
                $entity = new $class($location);
                if ($nameTag !== null && $nameTag !== '') {
                    $entity->setNameTag($nameTag);
                }
                return $entity;
            }
            return new EntityProxy($networkTypeId, $location, $nameTag ?? '');
        }
    }

    class EntityDataHelper
    {
        public static function parseLocation(mixed $nbt, \pocketmine\world\World $world): \pocketmine\world\Location
        {
            $pos = [0.0, 0.0, 0.0];
            $rot = [0.0, 0.0];
            if (is_object($nbt) && method_exists($nbt, 'hasTag') && method_exists($nbt, 'getListTag')) {
                $posTag = $nbt->hasTag('Pos') ? $nbt->getListTag('Pos') : null;
                if ($posTag !== null && method_exists($posTag, 'getValue')) {
                    $values = $posTag->getValue();
                    $pos[0] = isset($values[0]) && method_exists($values[0], 'getValue') ? (float)$values[0]->getValue() : 0.0;
                    $pos[1] = isset($values[1]) && method_exists($values[1], 'getValue') ? (float)$values[1]->getValue() : 0.0;
                    $pos[2] = isset($values[2]) && method_exists($values[2], 'getValue') ? (float)$values[2]->getValue() : 0.0;
                }
                $rotTag = $nbt->hasTag('Rotation') ? $nbt->getListTag('Rotation') : null;
                if ($rotTag !== null && method_exists($rotTag, 'getValue')) {
                    $values = $rotTag->getValue();
                    $rot[0] = isset($values[0]) && method_exists($values[0], 'getValue') ? (float)$values[0]->getValue() : 0.0;
                    $rot[1] = isset($values[1]) && method_exists($values[1], 'getValue') ? (float)$values[1]->getValue() : 0.0;
                }
            }
            return new \pocketmine\world\Location($pos[0], $pos[1], $pos[2], $rot[0], $rot[1], $world);
        }

        /**
         * ¿Hay un `Server` que `applyNativeSnapshot()` pueda usar?
         *
         * La resolución del mundo es el único paso del snapshot que puede
         * fallar por el entorno: `Player::getWorld()` llama a
         * `Server::getInstance()` sin guardas, así que hace falta la clase
         * cargada **y** una instancia inicializable. Sin eso se hidrata el
         * snapshot sin posición; cualquier otro error se propaga.
         */
        private static function serverAvailable(): bool
        {
            try {
                if (!class_exists(\pocketmine\Server::class)) {
                    return false;
                }
                \pocketmine\Server::getInstance();
                return true;
            } catch (\Throwable) {
                return false;
            }
        }

        /**
         * Construye una entidad desde el snapshot que envía Rust
         * (`entity_json()`: `entity.get`, `entity.list` y eventos).
         */
        public static function entityFromArray(array $data): Entity
        {
            $location = new \pocketmine\world\Location(
                (float)($data['x'] ?? 0.0),
                (float)($data['y'] ?? 0.0),
                (float)($data['z'] ?? 0.0),
                (float)($data['yaw'] ?? 0.0),
                (float)($data['pitch'] ?? 0.0),
                (string)($data['world'] ?? 'world')
            );
            $type = (string)($data['type'] ?? '');
            // `minecraft:player` no tiene entrada en `EntityFactory`; un evento
            // de daño sobre un jugador debe entregar un `Player`, como en PM.
            if ($type === 'minecraft:player') {
                $player = new \pocketmine\player\Player(
                    $data + ['name' => (string)($data['name_tag'] ?? 'Steve')]
                );
                if (self::serverAvailable()) {
                    $player->applyNativeSnapshot($data);
                } else {
                    // Sin `Server` inicializable `Player::getWorld()` no puede
                    // resolver el mundo; el constructor ya copió posición, vida
                    // e ids del mismo payload, así que se hidrata el resto del
                    // snapshot sin tocar la posición. Cualquier otro error del
                    // snapshot se propaga en vez de perderse aquí.
                    unset($data['x'], $data['y'], $data['z'], $data['yaw'], $data['pitch']);
                    $player->applyNativeSnapshot($data);
                }
                if (($data['name_tag'] ?? '') !== '') {
                    $player->setNameTag((string)$data['name_tag']);
                }
                return $player;
            }
            $entity = EntityFactory::getInstance()->create($type, $location);
            if ($entity === null) {
                $entity = new EntityProxy($type === '' ? null : $type, $location);
            }
            $entity->applyNativeSnapshot($data);
            return $entity;
        }
    }

    /**
     * Instancia concreta que devuelve `EntityFactory::create()` para un tipo de
     * entidad cuya clase PHP todavía no existe (los tipos con clase propia se
     * portan uno a uno; ver `docs/plugin-compatibility.md`).
     *
     * Es la contraparte PHP de una entidad nativa de `entity.rs`: la creación
     * real ocurre en `spawnToAll()`, que llama al host (`entity.spawn`).
     */
    class EntityProxy extends Entity
    {
        public function __construct(
            ?string $networkTypeId = null,
            ?\pocketmine\world\Location $location = null,
            string $nameTag = ''
        ) {
            if ($networkTypeId !== null) {
                $this->networkTypeId = $networkTypeId;
            }
            if ($location !== null) {
                $this->location = $location;
            }
            $this->nameTag = $nameTag;
        }
    }
}

namespace pocketmine\player {

    use pocketmine\command\CommandSender;
    use pocketmine\inventory\PlayerInventory;
    use pocketmine\math\Position;
    use pocketmine\math\Vector3;
    use pocketmine\permission\Permission;
    use pocketmine\permission\PermissionManager;
    use pocketmine\utils\TextFormat;
    use pocketmine\world\Location;
    use pocketmine\world\World;

    /**
     * PM 5 `pocketmine\player\GameMode`.
     *
     * PM 5 lo declara como `enum` con `LegacyEnumShimTrait` (por eso
     * `GameMode::CREATIVE()` funciona). El puente usa una clase final con las
     * mismas constantes y métodos estáticos, para no depender de enums ni del
     * trait: `GameMode::CREATIVE()` y `GameMode::CREATIVE` valen igual.
     */
    final class GameMode
    {
        public const SURVIVAL = 0;
        public const CREATIVE = 1;
        public const ADVENTURE = 2;
        public const SPECTATOR = 3;

        private function __construct(
            private int $id,
            private string $englishName,
            /** @var string[] */
            private array $aliases
        ) {
        }

        public static function SURVIVAL(): self
        {
            return new self(self::SURVIVAL, 'Survival', ['survival', 's', '0']);
        }

        public static function CREATIVE(): self
        {
            return new self(self::CREATIVE, 'Creative', ['creative', 'c', '1']);
        }

        public static function ADVENTURE(): self
        {
            return new self(self::ADVENTURE, 'Adventure', ['adventure', 'a', '2']);
        }

        public static function SPECTATOR(): self
        {
            return new self(self::SPECTATOR, 'Spectator', ['spectator', 'v', 'view', '3']);
        }

        /** PM `GameMode::fromString()`: acepta nombre, inicial o id. */
        public static function fromString(string $str): ?self
        {
            $needle = mb_strtolower(trim($str));
            foreach (self::all() as $mode) {
                if (in_array($needle, $mode->aliases, true)) {
                    return $mode;
                }
            }
            return null;
        }

        public static function fromId(int $id): ?self
        {
            return match ($id) {
                self::SURVIVAL => self::SURVIVAL(),
                self::CREATIVE => self::CREATIVE(),
                self::ADVENTURE => self::ADVENTURE(),
                self::SPECTATOR => self::SPECTATOR(),
                default => null,
            };
        }

        /** @return self[] */
        private static function all(): array
        {
            return [self::SURVIVAL(), self::CREATIVE(), self::ADVENTURE(), self::SPECTATOR()];
        }

        public function id(): int
        {
            return $this->id;
        }

        public function getEnglishName(): string
        {
            return $this->englishName;
        }

        /** @return string[] */
        public function getAliases(): array
        {
            return $this->aliases;
        }

        public function equals(self $other): bool
        {
            return $this->id === $other->id;
        }

        public function __toString(): string
        {
            return $this->englishName;
        }
    }

    class Player extends \pocketmine\entity\Human implements CommandSender
    {
        public string $name;
        public string $displayName;
        public string $address;
        public int $port;
        public int|string $runtimeId;
        public int|string $uniqueId;
        public int $ping;
        public int $gamemode;
        public string $worldName;
        public float $x;
        public float $y;
        public float $z;
        public float $yaw;
        public float $pitch;
        public float $health;
        public float $maxHealth;
        public bool $flying;
        public float $xp;
        public int $xpLevel;

        private PlayerInventory $inventory;
        private PlayerInventory $enderInventory;
        private PlayerInventory $cursorInventory;
        private PlayerInventory $craftingGrid;
        private PlayerInventory $creativeInventory;
        private bool $online = true;
        private array $permissions = [];
        private ?bool $opOverride = null;

        /** Fase A1: `hiddenPlayers` (canSee). Clave: lowercase name. */
        private array $hiddenPlayers = [];
        /** Fase A1: locale PM (`en_US` por defecto). */
        private string $locale = 'en_US';
        /** Fase A1: flags de movimiento (toggle* locales). */
        private bool $sprinting = false;
        private bool $sneaking = false;
        private bool $sneakPressed = false;
        private bool $gliding = false;
        private bool $swimming = false;
        /** Fase A1: posición de la cama si duerme (`isSleeping`). */
        private mixed $sleeping = null;
        /** Fase A1: hambre/aire para `canEat`/`canBreathe` (simplificado). */
        private float $food = 20.0;
        private float $saturation = 20.0;
        private int $air = 300;
        /** Scoreboard (Fase 7): `Player::getScoreboard()`, null until set. */
        private ?\pocketmine\scoreboard\Scoreboard $scoreboard = null;
        /** Cache de `getUniqueId()` (objeto Uuid sobre `uniqueId` crudo). */
        private ?\pocketmine\uuid\Uuid $uuid = null;

        /** `Player::getFirstPlayed()`/`getLastPlayed()` (microtiempo float). */
        private float $firstPlayed = 0.0;
        private float $lastPlayed = 0.0;
        /** ¿Existía `players/<name>.json` antes de esta sesión? */
        private ?bool $hadSavedData = null;
        private bool $playedLoaded = false;

        public function __construct(array|string $data = ['name' => 'Steve'])
        {
            if (is_string($data)) {
                $data = ['name' => $data];
            }

            $this->name = (string)($data['name'] ?? $data['username'] ?? 'Steve');
            $this->displayName = (string)($data['display_name'] ?? $data['displayName'] ?? $this->name);
            $this->address = (string)($data['address'] ?? $data['ip'] ?? '127.0.0.1');
            $this->port = (int)($data['port'] ?? 19132);
            $this->runtimeId = (int)($data['runtime_id'] ?? $data['runtimeId'] ?? 1);
            $uniqueId = $data['unique_id'] ?? $data['uniqueId'] ?? $data['xuid'] ?? $this->name;
            $this->uniqueId = is_numeric($uniqueId) ? (int)$uniqueId : (string)$uniqueId;
            $this->ping = (int)($data['ping'] ?? 0);
            $this->gamemode = (int)($data['gamemode'] ?? 0);
            $this->worldName = (string)($data['world'] ?? 'world');
            $this->x = (float)($data['x'] ?? 0.0);
            $this->y = (float)($data['y'] ?? 64.0);
            $this->z = (float)($data['z'] ?? 0.0);
            $this->yaw = (float)($data['yaw'] ?? 0.0);
            $this->pitch = (float)($data['pitch'] ?? 0.0);
            $this->health = (float)($data['health'] ?? 20.0);
            $this->maxHealth = (float)($data['max_health'] ?? 20.0);
            $this->flying = (bool)($data['flying'] ?? false);
            $this->xp = (float)($data['xp'] ?? 0.0);
            $this->xpLevel = (int)($data['xp_level'] ?? $data['xpLevel'] ?? 0);
            $this->online = (bool)($data['online'] ?? true);
            $this->locale = (string)($data['locale'] ?? 'en_US');
            $this->sprinting = (bool)($data['sprinting'] ?? false);
            $this->sneaking = (bool)($data['sneaking'] ?? false);
            $this->gliding = (bool)($data['gliding'] ?? false);
            $this->swimming = (bool)($data['swimming'] ?? false);
            $this->food = (float)($data['food'] ?? 20.0);
            $this->saturation = (float)($data['saturation'] ?? 20.0);
            $this->air = (int)($data['air'] ?? 300);
            $this->firstPlayed = (float)($data['first_played'] ?? $data['firstPlayed'] ?? 0.0);
            $this->lastPlayed = (float)($data['last_played'] ?? $data['lastPlayed'] ?? 0.0);
            $hasPlayedBefore = $data['has_played_before'] ?? $data['hasPlayedBefore'] ?? null;
            if (is_bool($hasPlayedBefore)) {
                $this->hadSavedData = $hasPlayedBefore;
                $this->playedLoaded = true;
            }
            if ($this->firstPlayed > 0.0 || $this->lastPlayed > 0.0) {
                $this->playedLoaded = true;
                if ($this->hadSavedData === null) {
                    $this->hadSavedData = $this->lastPlayed > $this->firstPlayed + 0.001;
                }
            }

            if (isset($data['permissions']) && is_array($data['permissions'])) {
                $this->permissions = $data['permissions'];
            }
            if (array_key_exists('is_op', $data) || array_key_exists('op', $data)) {
                $this->opOverride = (bool)($data['is_op'] ?? $data['op']);
            }
            if (isset($data['hidden_players']) && is_array($data['hidden_players'])) {
                foreach ($data['hidden_players'] as $name) {
                    $this->hiddenPlayers[strtolower((string)$name)] = true;
                }
            }

            $this->inventory = new PlayerInventory($this->name);
            // Fase A1: stub inventories (ver getters; sync TODO).
            $this->enderInventory = new PlayerInventory($this->name . '#ender');
            $this->cursorInventory = new PlayerInventory($this->name . '#cursor');
            $this->craftingGrid = new PlayerInventory($this->name . '#crafting');
            $this->creativeInventory = new PlayerInventory($this->name . '#creative');
        }

        public function getName(): string
        {
            return $this->name;
        }

        public function getLowerCaseName(): string
        {
            return strtolower($this->name);
        }

        public function getDisplayName(): string
        {
            return $this->displayName;
        }

        public function setDisplayName(string $name): void
        {
            $this->displayName = $name;
        }

        public function getRawDisplayName(): string
        {
            return TextFormat::clean($this->displayName);
        }

        public function getAddress(): string
        {
            return $this->address;
        }

        public function getPort(): int
        {
            return $this->port;
        }

        public function getPing(): int
        {
            return $this->ping;
        }

        public function getPingTicks(): int
        {
            return (int)round($this->ping / 50);
        }

        /** PM 5: `Player::getGamemode(): GameMode`. */
        public function getGamemode(): GameMode
        {
            return GameMode::fromId($this->gamemode) ?? GameMode::SURVIVAL();
        }

        /**
         * PM 5 tipa `GameMode`; el puente acepta también el id numérico para
         * no romper llamadas existentes.
         */
        public function setGamemode(int|GameMode $gamemode): void
        {
            $id = $gamemode instanceof GameMode ? $gamemode->id() : $gamemode;
            $this->gamemode = $id;
            $this->addAction('set_gamemode', ['target_player' => $this->name, 'gamemode' => $id]);
        }

        public function getX(): float
        {
            return $this->x;
        }

        public function getY(): float
        {
            return $this->y;
        }

        public function getZ(): float
        {
            return $this->z;
        }

        public function getYaw(): float
        {
            return $this->yaw;
        }

        public function getPitch(): float
        {
            return $this->pitch;
        }

        public function getPosition(): Position
        {
            return new Position($this->x, $this->y, $this->z, $this->worldName);
        }

        public function getLocation(): Location
        {
            return new Location($this->x, $this->y, $this->z, $this->yaw, $this->pitch, $this->worldName);
        }

        /**
         * PM `Entity::getEyeHeight(): float` (default size info: 1.62, the
         * same `EYE_HEIGHT` the Rust core removes from the Bedrock eye
         * position on `PlayerAuthInput`).
         */
        public function getEyeHeight(): float
        {
            return 1.62;
        }

        /**
         * PM `Entity::getEyePos(): Vector3` — feet position + eye height.
         * `getPosition()`/`getLocation()` (and `PlayerMoveEvent`'s
         * `from`/`to`) are feet-based; this is the only eye-based accessor.
         */
        public function getEyePos(): Vector3
        {
            return new Vector3($this->x, $this->y + $this->getEyeHeight(), $this->z);
        }

        public function getWorld(): World
        {
            return \pocketmine\Server::getInstance()->getWorldManager()->getWorldByName($this->worldName);
        }

        public function getHealth(): float
        {
            return $this->health;
        }

        public function setHealth(float $health): void
        {
            $this->health = max(0.0, min($this->maxHealth, $health));
            // Fase A1: `PluginAction::SetHealth` (salud + max). También host
            // para virtuales (subset sin sesión Bedrock).
            $this->addAction('set_health', [
                'target_player' => $this->name,
                'health' => $this->health,
                'max_health' => $this->maxHealth,
            ]);
            $this->hostUpdateHealthFallback();
        }

        public function getMaxHealth(): float
        {
            return $this->maxHealth;
        }

        public function setMaxHealth(float $health): void
        {
            $this->maxHealth = max(1.0, $health);
            if ($this->health > $this->maxHealth) {
                $this->health = $this->maxHealth;
            }
            $this->addAction('set_health', [
                'target_player' => $this->name,
                'health' => $this->health,
                'max_health' => $this->maxHealth,
            ]);
            $this->hostUpdateHealthFallback();
        }

        public function isFlying(): bool
        {
            return $this->flying;
        }

        public function setFlying(bool $flying): void
        {
            $this->flying = $flying;
        }

        public function getXp(): float
        {
            return $this->xp;
        }

        public function setXp(float $xp): void
        {
            $this->xp = max(0.0, $xp);
            // Fase A1: `PluginAction::SetXp` (progress `xp` + `level`).
            // TODO: picks de PM (`Living::setXp` puntos -> progreso/nivel por
            // `ExperienceManager`) no existen; aquí `xp` es progreso 0..1 y
            // `level` el nivel. Conversión puntos->progreso pendiente.
            $this->addAction('set_xp', [
                'target_player' => $this->name,
                'xp' => $this->xp,
                'level' => $this->xpLevel,
            ]);
        }

        public function getXpLevel(): int
        {
            return $this->xpLevel;
        }

        public function setXpLevel(int $level): void
        {
            $this->xpLevel = max(0, $level);
            $this->addAction('set_xp', [
                'target_player' => $this->name,
                'xp' => $this->xp,
                'level' => $this->xpLevel,
            ]);
        }

        public function getInventory(): PlayerInventory
        {
            return $this->inventory;
        }

        public function isOnline(): bool
        {
            return $this->online;
        }

        public function isAlive(): bool
        {
            return $this->health > 0.0;
        }

        public function isClosed(): bool
        {
            return !$this->online;
        }

        public function isConnected(): bool
        {
            return $this->online;
        }

        public function isOp(): bool
        {
            if ($this->opOverride !== null) {
                return $this->opOverride;
            }
            return \pocketmine\Server::getInstance()->isOp($this->name);
        }

        /**
         * PM `Player::hasOpPermission(): bool`. BetterChat (`ChatListener`)
         * lo usa para el prefijo/sufijo de OP; es un alias de `isOp()`.
         */
        public function hasOpPermission(): bool
        {
            return $this->isOp();
        }

        /**
         * Carga perezosa de `first_played`/`last_played` y de la existencia
         * de datos guardados (`player.load` / `player.has_data`) vía host.
         * Sin host, los valores vienen del constructor (0.0 = nunca).
         */
        private function loadPlayedTimes(): void
        {
            if ($this->playedLoaded) {
                return;
            }
            $this->playedLoaded = true;
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                $this->hadSavedData ??= false;
                return;
            }
            try {
                $loaded = \pocketmine\runtime\Bridge::hostCall('player.load', ['name' => $this->name]);
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/PLAYER] player.load failed: ' . $e->getMessage() . PHP_EOL);
                $loaded = null;
            }
            if (!is_array($loaded)) {
                $this->hadSavedData ??= false;
                return;
            }
            $hasData = ($loaded['has_data'] ?? $loaded['exists'] ?? false) === true;
            $this->hadSavedData = $hasData || ($this->hadSavedData ?? false);
            if (!$hasData) {
                return;
            }
            $source = $loaded;
            if (isset($source['data']) && is_array($source['data'])) {
                $source = array_merge($source, $source['data']);
            }
            $first = (float)($source['first_played'] ?? $source['firstPlayed'] ?? 0.0);
            $last = (float)($source['last_played'] ?? $source['lastPlayed'] ?? 0.0);
            if ($first > 0.0) {
                $this->firstPlayed = $first;
            }
            if ($last > 0.0) {
                $this->lastPlayed = $last;
            }
        }

        /** PM `Player::getFirstPlayed(): float` (microtiempo, 0.0 = nunca). */
        public function getFirstPlayed(): float
        {
            $this->loadPlayedTimes();
            return $this->firstPlayed;
        }

        /** PM `Player::getLastPlayed(): float` (microtiempo, 0.0 = nunca). */
        public function getLastPlayed(): float
        {
            $this->loadPlayedTimes();
            return $this->lastPlayed;
        }

        /**
         * PM `Player::hasPlayedBefore(): bool` (umbral de 1ms como axolotl):
         * existían datos guardados antes de esta sesión, o `lastPlayed`
         * supera a `firstPlayed` en más de 1 milisegundo.
         */
        public function hasPlayedBefore(): bool
        {
            $this->loadPlayedTimes();
            return ($this->hadSavedData ?? false) || ($this->lastPlayed > $this->firstPlayed + 0.001);
        }

        public function isPlayer(): bool
        {
            return true;
        }

        public function hasPermission($permission): bool
        {
            if (is_string($permission)) {
                if (array_key_exists($permission, $this->permissions)) {
                    return (bool)$this->permissions[$permission];
                }
                $manager = PermissionManager::getInstance();
                return $manager->hasPermission($manager->getPermission($permission), $this->isOp());
            }
            if ($permission instanceof Permission) {
                return PermissionManager::getInstance()->hasPermission($permission, $this->isOp());
            }
            return false;
        }

        public function sendMessage(string|\pocketmine\lang\Translatable $message): void
        {
            $this->addAction('send_message', ['target_player' => $this->name, 'message' => (string)$message]);
        }

        public function sendTip(string $message): void
        {
            $this->addAction('send_tip', ['target_player' => $this->name, 'message' => $message]);
        }

        public function sendPopup(string $message): void
        {
            $this->addAction('send_popup', ['target_player' => $this->name, 'message' => $message]);
        }

        public function sendTitle(string $title, string $subtitle = '', int $fadeIn = 0, int $stay = 0, int $fadeOut = 0): void
        {
            $this->addAction('send_title', [
                'target_player' => $this->name,
                'title' => $title,
                'subtitle' => $subtitle,
                'fade_in' => $fadeIn,
                'stay' => $stay,
                'fade_out' => $fadeOut,
            ]);
        }

        /** PM `Player::sendSubTitle(string): void` (solo subtítulo). */
        public function sendSubTitle(string $subtitle): void
        {
            $this->addAction('send_subtitle', [
                'target_player' => $this->name,
                'subtitle' => $subtitle,
            ]);
        }

        /** PM `Player::sendActionBarMessage(string): void` (NO es popup). */
        public function sendActionBarMessage(string $message): void
        {
            $this->addAction('send_action_bar', [
                'target_player' => $this->name,
                'message' => $message,
            ]);
        }

        /**
         * PM `Player::getScoreboard(): ?Scoreboard` (null until set).
         * The PHP object is the source of truth; Rust mirrors it.
         */
        public function getScoreboard(): ?\pocketmine\scoreboard\Scoreboard
        {
            return $this->scoreboard;
        }

        /**
         * PM `Player::setScoreboard(Scoreboard): void`. Installs the
         * change emitter so every later objective/line mutation
         * re-sends the full board (`set_scoreboard` action).
         */
        public function setScoreboard(\pocketmine\scoreboard\Scoreboard $scoreboard): void
        {
            $this->scoreboard = $scoreboard;
            $name = $this->name;
            $scoreboard->setOnChange(function (\pocketmine\scoreboard\Scoreboard $board) use ($name): void {
                $this->addAction('set_scoreboard', [
                    'target_player' => $name,
                    'scoreboard' => $board->toArray(),
                ]);
            });
            $scoreboard->notifyChange();
        }

        /** PM `Player::removeScoreboard(): void`. */
        public function removeScoreboard(): void
        {
            $this->scoreboard = null;
            $this->addAction('remove_scoreboard', ['target_player' => $this->name]);
        }

        /**
         * PM `Player::sendToastNotification(string, string): void`.
         * Stub documentado: sin paquete Toast en bridge/protocolo.
         * TODO(A1): `ToastNotificationPacket` + queue (maestro Bridge).
         */
        public function sendToastNotification(string $title, string $body): void
        {
            // No-op stub: no se emite acción (variante inexistente rompería IPC).
        }

        /**
         * PM `Player::sendForm(Form): void`. Stub documentado: sin
         * `ModalForm`/`FormResponse` en bridge/protocol.
         * TODO(A1): `ModalFormDataPacket` + `Form` (equipo Forms).
         */
        public function sendForm(mixed $form): void
        {
            if (!$form instanceof \pocketmine\form\Form) {
                return;
            }
            $id = \pocketmine\runtime\FormRegistry::nextId();
            \pocketmine\runtime\FormRegistry::store($this->name, $id, $form);
            $json = json_encode($form, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
            if ($json === false) {
                return;
            }
            $this->addAction('send_form', [
                'target_player' => $this->name,
                'form_id' => $id,
                'data' => $json,
            ]);
        }

        public function teleport(mixed $position, ?float $yaw = null, ?float $pitch = null): bool
        {
            // `Entity::teleport()` aporta la rotación opcional; `Location`
            // expone `yaw`/`pitch` como privados, así que se leen por getter.
            $hasRotation = $yaw !== null || $pitch !== null;
            if ($position instanceof Vector3) {
                $this->x = $position->getX();
                $this->y = $position->getY();
                $this->z = $position->getZ();
                $world = method_exists($position, 'getWorldName') ? $position->getWorldName() : null;
                if (is_string($world) && $world !== '') {
                    $this->worldName = $world;
                }
                if ($yaw === null && method_exists($position, 'getYaw')) {
                    $this->yaw = (float)$position->getYaw();
                }
                if ($pitch === null && method_exists($position, 'getPitch')) {
                    $this->pitch = (float)$position->getPitch();
                }
                $hasRotation = $hasRotation
                    || method_exists($position, 'getYaw')
                    || method_exists($position, 'getPitch');
            }
            if ($yaw !== null) {
                $this->yaw = $yaw;
            }
            if ($pitch !== null) {
                $this->pitch = $pitch;
            }
            $action = [
                'target_player' => $this->name,
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
                'world' => $this->worldName,
            ];
            // Yaw/pitch Bedrock-canónico (los que la copia API guarda):
            // sin ellos el teleport de un plugin pierde la rotación en
            // Rust y el Java ve el giro viejo tras el salto (ver
            // `syncTeleport` + `tp_sync_test.php`).
            if ($hasRotation) {
                $action['yaw'] = $this->yaw;
                $action['pitch'] = $this->pitch;
            }
            $this->addAction('teleport', $action);
            return true;
        }

        public function kick(string $reason = '', mixed $quitMessage = null): void
        {
            $this->online = false;
            $this->addAction('kick_player', ['target_player' => $this->name, 'reason' => $reason]);
        }

        public function getServer(): mixed
        {
            return \pocketmine\Server::getInstance();
        }

        /**
         * PM `Player::getUniqueId(): UuidInterface`. Devuelve el shim
         * `pocketmine\uuid\Uuid` (ver clase): los plugins PM encadenan
         * `->toString()`/`->getBytes()` sobre él. La identidad canónica
         * es el valor crudo (`unique_id` int XUID o nombre).
         */
        public function getUniqueId(): \pocketmine\uuid\Uuid
        {
            return $this->uuid ??= \pocketmine\uuid\Uuid::fromMixed($this->uniqueId);
        }

        /** PM `Entity::getId(): int` — id de entidad de la sesión. */
        public function getId(): int
        {
            return (int)$this->runtimeId;
        }

        public function getXuid(): string
        {
            return (string)$this->uniqueId;
        }

        public function getRuntimeId(): int
        {
            return (int)$this->runtimeId;
        }

        /**
         * PM `Player::getNetworkSession(): NetworkSession`. Stub de solo
         * lectura en el bridge: expone dirección/ping (ScoreHud lee tags
         * `{scorehud.ip}`/`{scorehud.ping}`) y `sendDataPacket` no-op
         * (la capa de paquetes vive en Rust; sin IPC de paquetes PHP).
         */
        public function getNetworkSession(): object
        {
            return new class($this->address, $this->port, $this->ping) {
                public function __construct(
                    private string $ip,
                    private int $port,
                    private int $ping,
                ) {
                }

                public function getIp(): string
                {
                    return $this->ip;
                }

                public function getPort(): int
                {
                    return $this->port;
                }

                public function getPing(): int
                {
                    return $this->ping;
                }

                public function isConnected(): bool
                {
                    return true;
                }

                public function sendDataPacket(mixed $packet): void
                {
                    // No-op: el envío de paquetes PHP→Rust no existe (TODO).
                }
            };
        }

        /**
         * PM `Player::getXpManager(): XpManager` (solo lectura: los tags
         * XP de ScoreHud leen nivel/progreso). Base `xp`/`xp_level` del
         * payload; el resto se deriva de forma aproximada.
         */
        public function getXpManager(): XpManager
        {
            return new XpManager((int)$this->xpLevel, $this->xp);
        }

        /** @return array<int, string> */
        public function getUsedChunks(): array
        {
            return [];
        }

        // ----------------------------------------------------------------
        // Fase A1: paridad Player.php (visibilidad / skin / inventories /
        // gamemode helpers / toggles / sleep / eat / block-attack stubs).
        // ----------------------------------------------------------------

        /**
         * PM `Player::canSee(Player): bool` (`hiddenPlayers` por UUID).
         * Clave local por nombre (TODO: UUID bytes cuando exista `Uuid` real).
         */
        public function canSee(Player $player): bool
        {
            if ($player === $this) {
                return true;
            }
            return !isset($this->hiddenPlayers[$player->getLowerCaseName()]);
        }

        /**
         * PM `Player::hidePlayer(Player): void` -> `PluginAction::HidePlayer`
         * -> `SessionManager::hide_player` (despawn + flag). No-op si self.
         */
        public function hidePlayer(Player $player): void
        {
            if ($player === $this) {
                return;
            }
            $this->hiddenPlayers[$player->getLowerCaseName()] = true;
            $this->addAction('hide_player', ['viewer' => $this->name, 'target' => $player->getName()]);
        }

        /** PM `Player::showPlayer(Player): void` -> `PluginAction::ShowPlayer`. */
        public function showPlayer(Player $player): void
        {
            if ($player === $this) {
                return;
            }
            unset($this->hiddenPlayers[$player->getLowerCaseName()]);
            $this->addAction('show_player', ['viewer' => $this->name, 'target' => $player->getName()]);
        }

        /**
         * PM `Player::getSkin(): Skin`. Stub Steve (sin textura real).
         * TODO(A1): skin real (128x128 / geometría) + `changeSkin`/`sendSkin`.
         */
        public function getSkin(): object
        {
            if (class_exists(\pocketmine\entity\Skin::class)) {
                return new \pocketmine\entity\Skin();
            }
            $skin = new \stdClass();
            $skin->skinId = 'standard';
            $skin->skinData = '';
            return $skin;
        }

        /** PM `Player::getLocale(): string` (`en_US` por defecto). */
        public function getLocale(): string
        {
            return $this->locale;
        }

        /**
         * PM `Player::getEnderInventory(): EnderInventory`. Stub `PlayerInventory`.
         * TODO(A1): tipo `EnderInventory` + sync con sesión (inventario Rust).
         */
        public function getEnderInventory(): PlayerInventory
        {
            return $this->enderInventory;
        }

        /**
         * PM `Player::getCursorInventory(): PlayerCursorInventory`. Stub.
         * TODO(A1): inventario temporal cursor + sync de ventana.
         */
        public function getCursorInventory(): PlayerInventory
        {
            return $this->cursorInventory;
        }

        /**
         * PM `Player::getCraftingGrid(): CraftingGrid`. Stub.
         * TODO(A1): grid 2x2+output + recetas (CraftingManager).
         */
        public function getCraftingGrid(): PlayerInventory
        {
            return $this->craftingGrid;
        }

        /**
         * PM `Player::getCreativeInventory(): CreativeInventory`. Stub.
         * TODO(A1): inventario creativo compartido + `syncCreative`.
         */
        public function getCreativeInventory(): PlayerInventory
        {
            return $this->creativeInventory;
        }

        /** PM `Player::isSurvival(bool $literal = false)`. */
        public function isSurvival(bool $literal = false): bool
        {
            return $this->gamemode === GameMode::SURVIVAL || (!$literal && $this->gamemode === GameMode::ADVENTURE);
        }

        /** PM `Player::isCreative(bool $literal = false)`. */
        public function isCreative(bool $literal = false): bool
        {
            return $this->gamemode === GameMode::CREATIVE || (!$literal && $this->gamemode === GameMode::SPECTATOR);
        }

        /** PM `Player::isAdventure(bool $literal = false)`. */
        public function isAdventure(bool $literal = false): bool
        {
            return $this->gamemode === GameMode::ADVENTURE || (!$literal && $this->gamemode === GameMode::SPECTATOR);
        }

        /** PM `Player::isSpectator(): bool`. */
        public function isSpectator(): bool
        {
            return $this->gamemode === GameMode::SPECTATOR;
        }

        public function isSprinting(): bool
        {
            return $this->sprinting;
        }

        public function isSneaking(): bool
        {
            return $this->sneaking;
        }

        public function isGliding(): bool
        {
            return $this->gliding;
        }

        public function isSwimming(): bool
        {
            return $this->swimming;
        }

        /** PM `Player::toggleSprint(bool): bool`. Estado local; sync TODO. */
        public function toggleSprint(bool $sprint): bool
        {
            if ($sprint === $this->sprinting) {
                return true;
            }
            // TODO(A1): evento `PlayerToggleSprintEvent` + sync de flags
            // (`SetActorData` DATA_FLAGS) vía `PluginAction::SetFlags`.
            $this->sprinting = $sprint;
            return true;
        }

        /** PM `Player::toggleSneak(bool, bool $sneakPressed = true): bool`. */
        public function toggleSneak(bool $sneak, bool $sneakPressed = true): bool
        {
            $this->sneakPressed = $sneakPressed;
            if ($sneak === $this->sneaking) {
                return true;
            }
            // TODO(A1): evento `PlayerToggleSneakEvent` + sync DATA_FLAGS.
            $this->sneaking = $sneak;
            return true;
        }

        /** PM `Player::toggleFlight(bool): bool` (usa `flying`). */
        public function toggleFlight(bool $fly): bool
        {
            if ($fly === $this->flying) {
                return true;
            }
            // TODO(A1): evento `PlayerToggleFlightEvent` + chequeo
            // `allowFlight` (creative/spectator) + `UpdateAbilitiesPacket`.
            $this->flying = $fly;
            return true;
        }

        /** PM `Player::toggleGlide(bool): bool`. */
        public function toggleGlide(bool $glide): bool
        {
            if ($glide === $this->gliding) {
                return true;
            }
            // TODO(A1): evento + sync FLAG_GLIDING.
            $this->gliding = $glide;
            return true;
        }

        /** PM `Player::toggleSwim(bool): bool`. */
        public function toggleSwim(bool $swim): bool
        {
            if ($swim === $this->swimming) {
                return true;
            }
            // TODO(A1): evento + sync FLAG_SWIMMING.
            $this->swimming = $swim;
            return true;
        }

        /** PM `Player::isSleeping(): bool`. */
        public function isSleeping(): bool
        {
            return $this->sleeping !== null;
        }

        /**
         * PM `Player::sleepOn(Vector3): bool`. Stub: solo guarda posición
         * (floor). TODO(A1): bloque `Bed` + `PlayerBedEnterEvent` + spawn.
         */
        public function sleepOn(mixed $pos): bool
        {
            if ($this->sleeping !== null) {
                return false;
            }
            if ($pos instanceof Vector3) {
                $this->sleeping = $pos->floor();
                return true;
            }
            return false;
        }

        /** PM `Player::stopSleep(): void`. */
        public function stopSleep(): void
        {
            $this->sleeping = null;
        }

        /**
         * PM `Player::canEat(): bool` (`isCreative() || HungerManager`).
         * Simplificación: creative o vivo. TODO: hambre real (`food`).
         */
        public function canEat(): bool
        {
            return $this->isCreative() || $this->isAlive();
        }

        /**
         * PM `Player::canBreathe(): bool` (`isCreative() || air > 0`).
         * Simplificación: creative o vivo. TODO: aire real (`air`).
         */
        public function canBreathe(): bool
        {
            return $this->isCreative() || $this->isAlive();
        }

        /**
         * PM `Player::pickBlock(Vector3, bool): bool`.
         * TODO(A1): `World::getBlock` + `PlayerBlockPickEvent` + item pick.
         * Hoy stub devuelve `false` (sin mundo/bloque real en bridge).
         */
        public function pickBlock(mixed $pos, bool $addTileNBT = false): bool
        {
            return false;
        }

        /**
         * PM `Player::attackBlock(Vector3, int): bool` (clic izquierdo).
         * TODO(A1): `PlayerInteractEvent` + `World::useBreakOn` + handler.
         */
        public function attackBlock(mixed $pos, int $face = 0): bool
        {
            return false;
        }

        /**
         * PM `Player::interactBlock(Vector3, int, Vector3): bool`.
         * TODO(A1): `World::useItemOn` + alcance (`canInteract`).
         */
        public function interactBlock(mixed $pos, int $face = 0, mixed $clickOffset = null): bool
        {
            return false;
        }

        /**
         * PM `Player::attackEntity(Entity): bool`.
         * TODO(A1): `EntityDamageByEntityEvent` + `Entity::attack()` (sin
         * sistema de entidades en bridge).
         */
        public function attackEntity(mixed $entity): bool
        {
            return false;
        }

        /** Fallback virtual: sincroniza salud en registro host si hay Bridge. */
        private function hostUpdateHealthFallback(): void
        {
            if (class_exists(\pocketmine\runtime\Bridge::class) && \pocketmine\runtime\Bridge::isActive()) {
                \pocketmine\runtime\Bridge::hostCall('player.update_health', [
                    'name' => $this->name,
                    'health' => $this->health,
                    'max_health' => $this->maxHealth,
                ]);
            }
        }

        /**
         * Persiste el jugador (`players/<name>.json` vía host `player.save`).
         *
         * Dispara `PlayerDataSaveEvent` antes de escribir; si un plugin lo
         * cancela no se guarda.
         */
        public function save(bool $isQuit = true): bool
        {
            if (class_exists(\pocketmine\event\player\PlayerDataSaveEvent::class)) {
                try {
                    $manager = \pocketmine\Server::getInstance()->getPluginManager();
                    $event = new \pocketmine\event\player\PlayerDataSaveEvent($this);
                    $manager->dispatchEvent($event);
                    if (method_exists($event, 'isCancelled') && $event->isCancelled()) {
                        return false;
                    }
                } catch (\Throwable $e) {
                    fwrite(STDERR, '[PHP/PLAYER] PlayerDataSaveEvent failed: ' . $e->getMessage() . PHP_EOL);
                    return false;
                }
            }
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                return false;
            }
            // JSON v2 canónico: `y` sin +64 legacy; el host completa lo que
            // falte desde el registro/disco y sella `last_played=now`.
            $result = \pocketmine\runtime\Bridge::hostCall('player.save', [
                'name' => $this->name,
                'xuid' => (string)$this->uniqueId,
                'display_name' => $this->displayName,
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
                'yaw' => $this->yaw,
                'pitch' => $this->pitch,
                'gamemode' => $this->gamemode,
                'health' => $this->health,
                'max_health' => $this->maxHealth,
                'food' => $this->food,
                'saturation' => $this->saturation,
                'xp' => $this->xp,
                'xp_level' => $this->xpLevel,
                'air' => $this->air,
                'flying' => $this->flying,
                'sneaking' => $this->sneaking,
                'sprinting' => $this->sprinting,
                'swimming' => $this->swimming,
                'gliding' => $this->gliding,
                'world' => $this->worldName,
                'is_op' => $this->isOp(),
                'first_played' => $this->firstPlayed,
                'last_played' => $this->lastPlayed,
            ]);
            return is_array($result) && ($result['ok'] ?? false) === true;
        }

        /** Guarda datos genéricos del jugador (`player.save_data`). */
        public function saveData(array $data): bool
        {
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                return false;
            }
            $result = \pocketmine\runtime\Bridge::hostCall('player.save_data', [
                'name' => $this->name,
                'data' => $data,
            ]);
            return is_array($result) && ($result['ok'] ?? false) === true;
        }

        public function toArray(): array
        {
            return [
                'name' => $this->name,
                'display_name' => $this->displayName,
                'address' => $this->address,
                'port' => $this->port,
                'runtime_id' => $this->runtimeId,
                'unique_id' => $this->uniqueId,
                'ping' => $this->ping,
                'gamemode' => $this->gamemode,
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
                'yaw' => $this->yaw,
                'pitch' => $this->pitch,
                'world' => $this->worldName,
                'health' => $this->health,
                'max_health' => $this->maxHealth,
                'flying' => $this->flying,
                'online' => $this->online,
                'is_op' => $this->isOp(),
                'first_played' => $this->firstPlayed,
                'last_played' => $this->lastPlayed,
            ];
        }

        private function addAction(string $action, array $data = []): void
        {
            try {
                \pocketmine\Server::getInstance()->addAction($action, $data);
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/PLAYER] Unable to queue ' . $action . ': ' . $e->getMessage() . PHP_EOL);
            }
        }

        public function __toString(): string
        {
            return $this->name;
        }
    }

    class OfflinePlayer
    {
        public function __construct(private string $name, private ?array $data = null)
        {
        }

        public function getName(): string
        {
            return $this->name;
        }

        /** Datos guardados (`player.load`), o null si nunca jugó. */
        public function getData(): ?array
        {
            return $this->data;
        }

        public function getXuid(): string
        {
            return (string)($this->data['unique_id'] ?? $this->name);
        }

        public function isOnline(): bool
        {
            return \pocketmine\Server::getInstance()->getPlayerExact($this->name) !== null;
        }

        public function isOp(): bool
        {
            return \pocketmine\Server::getInstance()->isOp($this->name);
        }

        public function hasOpPermission(): bool
        {
            return $this->isOp();
        }

        public function hasPermission($permission): bool
        {
            return $this->isOp();
        }

        /** PM `OfflinePlayer::getFirstPlayed(): float` (0.0 = nunca). */
        public function getFirstPlayed(): float
        {
            $source = $this->data;
            if (is_array($source) && isset($source['data']) && is_array($source['data'])) {
                $source = array_merge($source, $source['data']);
            }
            return (float)($source['first_played'] ?? $source['firstPlayed'] ?? 0.0);
        }

        /** PM `OfflinePlayer::getLastPlayed(): float` (0.0 = nunca). */
        public function getLastPlayed(): float
        {
            $source = $this->data;
            if (is_array($source) && isset($source['data']) && is_array($source['data'])) {
                $source = array_merge($source, $source['data']);
            }
            return (float)($source['last_played'] ?? $source['lastPlayed'] ?? 0.0);
        }

        public function hasPlayedBefore(): bool
        {
            if ($this->data === null) {
                return false;
            }
            return ($this->getLastPlayed() > $this->getFirstPlayed() + 0.001)
                || ($this->data['has_data'] ?? $this->data['exists'] ?? true) === true;
        }

        public function __toString(): string
        {
            return $this->name;
        }
    }

    /**
     * PM `pocketmine\player\XpManager` (stub de solo lectura). ScoreHud
     * y plugins de XP consultan `getY()` vía `Player::getXpManager()`.
     * Nivel desde el payload (`xp_level`); progreso y total derivados
     * del float `xp` del bridge (mejor esfuerzo, tags informativos).
     */
    final class XpManager
    {
        public function __construct(
            private int $level = 0,
            private float $totalXp = 0.0,
        ) {
        }

        public function getXpLevel(): int
        {
            return $this->level;
        }

        public function getXpProgress(): float
        {
            return max(0.0, min(1.0, $this->totalXp - floor($this->totalXp)));
        }

        public function getRemainderXp(): int
        {
            return 0;
        }

        public function getCurrentTotalXp(): int
        {
            return (int)$this->totalXp;
        }
    }
}

namespace pocketmine\uuid {

    /**
     * Shim de `ramsey\uuid\UuidInterface` (lo que devolvía
     * `Player::getUniqueId()` en PocketMine-MP upstream). Los plugins de
     * PM (`ScoreHud`, etc.) hacen `$player->getUniqueId()->toString()` —
     * el valor crudo llega de Rust como `unique_id` JSON **int** (XUID),
     * de modo que devolver el escalar rompía la cadena:
     * `Call to a member function toString() on int`. Este objeto devuelve
     * la identidad del bridge con los métodos que el código PM espera.
     *
     * La representación canónica (y `__toString`) preserva el valor
     * crudo tal cual (decimal XUID, o nombre cuando no hay UUID), para
     * no romper la identidad que usan `getXuid()`, `toArray()` y el
     * matching de `getPlayerByRawUUID`.
     */
    final class Uuid implements \Stringable
    {
        private function __construct(private string $canonical)
        {
        }

        /** Factory. Acepta int|string (XUID decimal, hex UUID, nombre). */
        public static function fromMixed(mixed $raw): self
        {
            if (is_bool($raw)) {
                $raw = $raw ? '1' : '';
            }
            if (is_object($raw) || is_array($raw)) {
                $raw = '';
            }
            return new self(is_scalar($raw) ? (string)$raw : '');
        }

        /** `UuidInterface::toString()` — identidad canónica del jugador. */
        public function toString(): string
        {
            return $this->canonical;
        }

        /**
         * `UuidInterface::getBytes()` — bytes binarios deterministas.
         * XUID numérico -> pack de 8 bytes (padded a 16); hex de UUID 32
         * chars -> bytes crudos; otro valor -> md5 determinista.
         */
        public function getBytes(): string
        {
            $raw = $this->canonical;
            if (is_numeric($raw) && (string)(int)$raw === $raw) {
                return str_pad(pack('J', (int)$raw), 16, "\0");
            }
            $hex = strtolower(str_replace('-', '', $raw));
            if (preg_match('/^[0-9a-f]{32}$/', $hex) === 1) {
                return pack('H*', $hex);
            }
            return md5('pocketmine-rust:' . $raw, true);
        }

        /** `UuidInterface::equals()` — compara contra Uuid u escalar. */
        public function equals(mixed $other): bool
        {
            if ($other instanceof self) {
                return $other->canonical === $this->canonical;
            }
            return is_scalar($other) ? (string)$other === $this->canonical : false;
        }

        public function __toString(): string
        {
            return $this->canonical;
        }
    }
}

namespace pocketmine\inventory {

    use pocketmine\item\Item;
    use pocketmine\item\ItemFactory;

    class PlayerInventory
    {
        /** @var array<int, Item> */
        private array $items = [];
        private int $hotbarSize = 9;
        private int $size = 36;
        /** Tick cuyo snapshot está en caché; `-1` = sin reloj del servidor. */
        private int $loadedTick = -1;
        private bool $loaded = false;

        public function __construct(private string $ownerName = '')
        {
        }

        public function getOwnerName(): string
        {
            return $this->ownerName;
        }

        /**
         * Recarga el snapshot desde el inventario autoritativo del servidor
         * (`inventory.get`). Las lecturas ya lo refrescan una vez por tick.
         */
        public function refresh(): void
        {
            $this->items = [];
            $this->loadedTick = $this->serverTick() ?? -1;
            $this->loaded = true;
            if ($this->ownerName === '' || str_contains($this->ownerName, '#')) {
                // ender/cursor/crafting/creative: todavía sin host call.
                return;
            }
            $result = \pocketmine\runtime\Bridge::hostCall('inventory.get', ['player' => $this->ownerName]);
            if (!is_array($result) || empty($result['ok'])) {
                return;
            }
            foreach (($result['windows'] ?? []) as $window) {
                if ((int)($window['window'] ?? -1) !== 0) {
                    continue;
                }
                foreach (($window['slots'] ?? []) as $index => $slot) {
                    $item = self::itemFromArray($slot);
                    if ($item !== null) {
                        $this->items[(int)$index] = $item;
                    }
                }
            }
        }

        private function serverTick(): ?int
        {
            if (!class_exists(\pocketmine\Server::class)) {
                return null;
            }
            try {
                return \pocketmine\Server::getInstance()->getTick();
            } catch (\Throwable) {
                return null;
            }
        }

        private function ensureFresh(): void
        {
            if (!$this->loaded) {
                $this->refresh();
                return;
            }
            $tick = $this->serverTick();
            if ($tick === null || $tick === $this->loadedTick) {
                return;
            }
            $this->refresh();
        }

        private static function itemFromArray(mixed $data): ?Item
        {
            if (!is_array($data)) {
                return null;
            }
            $count = (int)($data['count'] ?? 0);
            if ($count <= 0) {
                return null;
            }
            return Item::fromArray($data);
        }

        /** Empuja el slot al inventario autoritativo (`inventory.set_slot`). */
        private function pushSlot(int $index): void
        {
            if ($this->ownerName === '' || str_contains($this->ownerName, '#')) {
                return;
            }
            $item = $this->items[$index] ?? null;
            $payload = null;
            if ($item !== null && !$item->isNull() && $item->getCount() > 0) {
                $payload = [
                    'id' => $item->getId(),
                    'meta' => $item->getMeta(),
                    'count' => $item->getCount(),
                ];
            }
            if (is_array($payload)) {
                // El NBT (`ench`, `display`, ...) viaja con el stack: sin él un
                // item encantado volvería sin encantamientos al leerlo.
                $nbt = $item?->getNbtBytes() ?? '';
                $payload['nbt'] = $nbt === '' ? null : bin2hex($nbt);
            }
            \pocketmine\runtime\Bridge::hostCall('inventory.set_slot', [
                'player' => $this->ownerName,
                'window' => 0,
                'slot' => $index,
                'item' => $payload,
            ]);
        }

        public function getItem(int $index): Item
        {
            $this->ensureFresh();
            return $this->items[$index] ?? ItemFactory::get(0, 0, 0);
        }

        public function setItem(int $index, Item $item): void
        {
            $this->ensureFresh();
            if ($index < 0 || $index >= $this->size) {
                return;
            }
            if ($item->isNull() || $item->getCount() <= 0) {
                unset($this->items[$index]);
            } else {
                $this->items[$index] = $item;
            }
            $this->pushSlot($index);
        }

        public function addItem(Item ...$items): array
        {
            $this->ensureFresh();
            $leftover = [];
            $touched = [];
            foreach ($items as $item) {
                if ($item->isNull() || $item->getCount() <= 0) {
                    continue;
                }
                $remaining = $item->getCount();
                // Primero apila en stacks existentes, como `addItem()` nativo.
                for ($i = 0; $i < $this->size && $remaining > 0; $i++) {
                    $existing = $this->items[$i] ?? null;
                    if ($existing === null
                        || $existing->getId() !== $item->getId()
                        || $existing->getMeta() !== $item->getMeta()) {
                        continue;
                    }
                    $space = max(0, 64 - $existing->getCount());
                    $take = min($space, $remaining);
                    if ($take <= 0) {
                        continue;
                    }
                    $existing->setCount($existing->getCount() + $take);
                    $remaining -= $take;
                    $touched[$i] = true;
                }
                // Luego los huecos libres.
                for ($i = 0; $i < $this->size && $remaining > 0; $i++) {
                    if (isset($this->items[$i])) {
                        continue;
                    }
                    $take = min(64, $remaining);
                    $this->items[$i] = ItemFactory::get($item->getId(), $item->getMeta(), $take);
                    $remaining -= $take;
                    $touched[$i] = true;
                }
                if ($remaining > 0) {
                    $leftover[] = ItemFactory::get($item->getId(), $item->getMeta(), $remaining);
                }
            }
            foreach (array_keys($touched) as $index) {
                $this->pushSlot((int)$index);
            }
            return $leftover;
        }

        public function removeItem(Item ...$items): array
        {
            $this->ensureFresh();
            $leftover = [];
            $touched = [];
            foreach ($items as $item) {
                $remaining = $item->getCount();
                for ($i = 0; $i < $this->size && $remaining > 0; $i++) {
                    $existing = $this->items[$i] ?? null;
                    if ($existing === null
                        || $existing->getId() !== $item->getId()
                        || $existing->getMeta() !== $item->getMeta()) {
                        continue;
                    }
                    $take = min($remaining, $existing->getCount());
                    $existing->setCount($existing->getCount() - $take);
                    $remaining -= $take;
                    if ($existing->getCount() <= 0) {
                        unset($this->items[$i]);
                    }
                    $touched[$i] = true;
                }
                if ($remaining > 0) {
                    $leftover[] = ItemFactory::get($item->getId(), $item->getMeta(), $remaining);
                }
            }
            foreach (array_keys($touched) as $index) {
                $this->pushSlot((int)$index);
            }
            return $leftover;
        }

        public function first(Item $item, bool $exact = false): int
        {
            $this->ensureFresh();
            foreach ($this->items as $index => $existing) {
                if ($existing->getId() === $item->getId()
                    && (!$exact || $existing->getMeta() === $item->getMeta())) {
                    return $index;
                }
            }
            return -1;
        }

        public function firstEmpty(): int
        {
            $this->ensureFresh();
            for ($i = 0; $i < $this->size; $i++) {
                if (!isset($this->items[$i])) {
                    return $i;
                }
            }
            return -1;
        }

        /** @return array<int, Item> */
        public function getContents(bool $includeEmpty = false): array
        {
            $this->ensureFresh();
            if ($includeEmpty) {
                $contents = [];
                for ($i = 0; $i < $this->size; $i++) {
                    $contents[$i] = $this->getItem($i);
                }
                return $contents;
            }
            return $this->items;
        }

        /** @param array<int, Item> $items */
        public function setContents(array $items): void
        {
            $this->clearAll();
            foreach ($items as $index => $item) {
                if (!($item instanceof Item) || $item->isNull() || $item->getCount() <= 0) {
                    continue;
                }
                $index = (int)$index;
                if ($index < 0 || $index >= $this->size) {
                    continue;
                }
                $this->items[$index] = $item;
                $this->pushSlot($index);
            }
        }

        public function clear(int $index): void
        {
            $this->ensureFresh();
            unset($this->items[$index]);
            $this->pushSlot($index);
        }

        public function clearAll(): void
        {
            $this->items = [];
            $this->loaded = true;
            if ($this->ownerName !== '' && !str_contains($this->ownerName, '#')) {
                \pocketmine\runtime\Bridge::hostCall('inventory.clear', ['player' => $this->ownerName]);
            }
        }

        public function getSize(): int
        {
            return $this->size;
        }

        public function getHotbarSize(): int
        {
            return $this->hotbarSize;
        }

        /** Slot 0 (la hotbar seleccionada aún no viaja al puente). */
        public function getItemInHand(): Item
        {
            return $this->getItem(0);
        }

        public function setItemInHand(Item $item): void
        {
            $this->setItem(0, $item);
        }
    }

    final class CreativeCategory
    {
        public const ALL = -1;
        public const CONSTRUCTION = 1;
        public const NATURE = 2;
        public const EQUIPMENT = 3;
        public const ITEMS = 4;
        public const COMMANDS = 5;
    }

    class CreativeInventory
    {
        private static ?self $instance = null;
        /** @var array<int, Item> */
        private array $items = [];

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
        }

        public static function reset(): void
        {
            self::$instance = null;
        }

        public function add(Item $item, ?int $category = null, ?string $group = null): void
        {
            $this->items[] = clone $item;
        }

        public function remove(Item $item): void
        {
            foreach ($this->items as $i => $it) {
                if (method_exists($it, 'equalsExact') ? $it->equalsExact($item) : ($it->getId() === $item->getId() && $it->getMeta() === $item->getMeta())) {
                    unset($this->items[$i]);
                }
            }
        }

        public function contains(Item $item): bool
        {
            foreach ($this->items as $it) {
                if (method_exists($it, 'equalsExact') ? $it->equalsExact($item) : ($it->getId() === $item->getId() && $it->getMeta() === $item->getMeta())) {
                    return true;
                }
            }
            return false;
        }

        /** @return array<int, Item> */
        public function getAll(): array
        {
            return array_values($this->items);
        }

        public function clear(): void
        {
            $this->items = [];
        }
    }
}
