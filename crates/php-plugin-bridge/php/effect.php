<?php

declare(strict_types=1);

namespace pocketmine\color {
    /**
     * `pocketmine\color\Color`: RGB(A) value used by the potion effects.
     */
    class Color
    {
        public function __construct(
            private int $r,
            private int $g,
            private int $b,
            private int $a = 255
        ) {}

        public function getR(): int { return $this->r; }
        public function getG(): int { return $this->g; }
        public function getB(): int { return $this->b; }
        public function getA(): int { return $this->a; }
        public function getRed(): int { return $this->r; }
        public function getGreen(): int { return $this->g; }
        public function getBlue(): int { return $this->b; }
        public function getAlpha(): int { return $this->a; }

        public function toRGBA(): int
        {
            return (($this->r & 0xff) << 24) | (($this->g & 0xff) << 16) | (($this->b & 0xff) << 8) | ($this->a & 0xff);
        }

        /** PM `Color::toARGB()`: el formato que usa `LevelEventPacket` (partículas). */
        public function toARGB(): int
        {
            return (($this->a & 0xff) << 24) | (($this->r & 0xff) << 16) | (($this->g & 0xff) << 8) | ($this->b & 0xff);
        }

        public function __toString(): string
        {
            return sprintf('Color(%d, %d, %d, %d)', $this->r, $this->g, $this->b, $this->a);
        }
    }
}

namespace pocketmine\entity\effect {
    use pocketmine\color\Color;
    use pocketmine\data\bedrock\effect\EffectIdMap;
    use pocketmine\entity\Entity;
    use pocketmine\entity\Living;

    /**
     * `pocketmine\entity\effect\Effect`: type data of one status effect.
     *
     * Ported fields and behaviour from `src/entity/effect/Effect.php`:
     * `getName()`, `getColor()`, `isBad()`, `getDefaultDuration()`,
     * `hasBubbles()`, `canTick()`, `getApplyInterval()`, `applyEffect()`,
     * `add()` and `remove()`.
     */
    class Effect
    {
        private string $name;

        public function __construct(
            mixed $name,
            protected Color $color,
            protected bool $bad = false,
            private int $defaultDuration = 600,
            protected bool $hasBubbles = true
        ) {
            $this->name = is_string($name)
                ? $name
                : ($name instanceof \Stringable ? (string)$name : get_debug_type($name));
        }

        /** Translation key/name of the effect. */
        public function getName(): string
        {
            return $this->name;
        }

        public function getColor(): Color
        {
            return $this->color;
        }

        public function isBad(): bool
        {
            return $this->bad;
        }

        public function getDefaultDuration(): int
        {
            return $this->defaultDuration;
        }

        public function hasBubbles(): bool
        {
            return $this->hasBubbles;
        }

        /** Whether `applyEffect()` runs on this tick (`getApplyInterval()`). */
        public function canTick(EffectInstance $instance): bool
        {
            $interval = $this->getApplyInterval($instance);
            return $interval > 0 && $instance->getDuration() % $interval === 0;
        }

        /** 0 = never ticks, 1 = every tick. */
        public function getApplyInterval(EffectInstance $instance): int
        {
            return 0;
        }

        public function applyEffect(Living $entity, EffectInstance $instance, float $potency = 1.0, ?Entity $source = null): void
        {
        }

        public function add(Living $entity, EffectInstance $instance): void
        {
        }

        public function remove(Living $entity, EffectInstance $instance): void
        {
        }

        public function __toString(): string
        {
            return $this->name;
        }
    }

    /** `InstantEffect`: applied exactly once, when the effect is added. */
    class InstantEffect extends Effect
    {
        public function getApplyInterval(EffectInstance $instance): int
        {
            return 1;
        }
    }

    /** `InstantDamageEffect`: 6 half-hearts per level (PM `6 << amplifier`). */
    class InstantDamageEffect extends InstantEffect
    {
        public function applyEffect(Living $entity, EffectInstance $instance, float $potency = 1.0, ?Entity $source = null): void
        {
            $entity->setHealth($entity->getHealth() - (6 * (2 ** $instance->getAmplifier())) * $potency);
        }
    }

    /** `InstantHealthEffect`: heals 4 half-hearts per level. */
    class InstantHealthEffect extends InstantEffect
    {
        public function applyEffect(Living $entity, EffectInstance $instance, float $potency = 1.0, ?Entity $source = null): void
        {
            $entity->setHealth($entity->getHealth() + (4 * (2 ** $instance->getAmplifier())) * $potency);
        }
    }

    /** `RegenerationEffect`: one half-heart every 50 >> amplifier ticks. */
    class RegenerationEffect extends Effect
    {
        public function getApplyInterval(EffectInstance $instance): int
        {
            return (int)(50 / (2 ** $instance->getAmplifier()));
        }

        public function applyEffect(Living $entity, EffectInstance $instance, float $potency = 1.0, ?Entity $source = null): void
        {
            if ($entity->getHealth() < $entity->getMaxHealth()) {
                $entity->setHealth($entity->getHealth() + $potency);
            }
        }
    }

    /** `PoisonEffect`: damage every 25 >> amplifier ticks, never lethal. */
    class PoisonEffect extends Effect
    {
        public function __construct(
            mixed $name,
            Color $color,
            bool $bad = true,
            int $defaultDuration = 600,
            bool $hasBubbles = true,
            private bool $fatal = false
        ) {
            parent::__construct($name, $color, $bad, $defaultDuration, $hasBubbles);
        }

        public function getApplyInterval(EffectInstance $instance): int
        {
            return (int)(25 / (2 ** $instance->getAmplifier()));
        }

        public function applyEffect(Living $entity, EffectInstance $instance, float $potency = 1.0, ?Entity $source = null): void
        {
            if (!$this->fatal && $entity->getHealth() <= $potency) {
                return;
            }
            $entity->setHealth($entity->getHealth() - $potency);
        }
    }

    /** `WitherEffect`: damages and drains health, like PM's `WitherEffect`. */
    class WitherEffect extends Effect
    {
        public function getApplyInterval(EffectInstance $instance): int
        {
            return 40 >> $instance->getAmplifier();
        }

        public function applyEffect(Living $entity, EffectInstance $instance, float $potency = 1.0, ?Entity $source = null): void
        {
            $entity->setHealth($entity->getHealth() - $potency);
        }
    }

    /** `HungerEffect`: drains food (the native session owns `food`). */
    class HungerEffect extends Effect
    {
        public function getApplyInterval(EffectInstance $instance): int
        {
            return 1;
        }
    }

    /** `SaturationEffect`: restores food (native session owns `food`). */
    class SaturationEffect extends Effect
    {
        public function getApplyInterval(EffectInstance $instance): int
        {
            return 1;
        }
    }

    /** `SpeedEffect`: +20% movement speed per level (native physics owns it). */
    class SpeedEffect extends Effect
    {
    }

    /** `SlownessEffect`: -15% movement speed per level. */
    class SlownessEffect extends Effect
    {
    }

    /** `LevitationEffect`: upward motion, PM keeps the movement math native. */
    class LevitationEffect extends Effect
    {
    }

    /** `InvisibilityEffect`: only the metadata bit changes, not the state. */
    class InvisibilityEffect extends Effect
    {
    }

    /** `HealthBoostEffect`: raises the max health while active. */
    class HealthBoostEffect extends Effect
    {
        public function add(Living $entity, EffectInstance $instance): void
        {
            $entity->setMaxHealth($entity->getMaxHealth() + 4 * $instance->getEffectLevel());
        }

        public function remove(Living $entity, EffectInstance $instance): void
        {
            $entity->setMaxHealth(max(1.0, $entity->getMaxHealth() - 4 * $instance->getEffectLevel()));
        }
    }

    /** `AbsorptionEffect`: adds absorption hearts (not modelled natively). */
    class AbsorptionEffect extends Effect
    {
    }

    /**
     * `pocketmine\entity\effect\EffectInstance`.
     *
     * Durations follow PM: 0 or less is expired, `infinite` wraps around
     * `Limits::INT32_MAX` instead of expiring.
     */
    class EffectInstance
    {
        public const INFINITE_DURATION = 2147483647;

        private int $duration;
        private int $amplifier;
        private Color $color;
        private bool $infinite;

        public function __construct(
            private Effect $effectType,
            ?int $duration = null,
            int $amplifier = 0,
            private bool $visible = true,
            private bool $ambient = false,
            ?Color $overrideColor = null,
            bool $infinite = false
        ) {
            $this->setDuration($duration ?? $effectType->getDefaultDuration());
            $this->setAmplifier($amplifier);
            $this->color = $overrideColor ?? $effectType->getColor();
            $this->infinite = $infinite;
        }

        public function getType(): Effect
        {
            return $this->effectType;
        }

        public function getDuration(): int
        {
            return $this->duration;
        }

        public function setDuration(int $duration): EffectInstance
        {
            if ($duration < 0 || $duration > self::INFINITE_DURATION) {
                throw new \InvalidArgumentException('Effect duration must be in range 0 - ' . self::INFINITE_DURATION . ", got $duration");
            }
            $this->duration = $duration;
            return $this;
        }

        public function decreaseDuration(int $ticks): EffectInstance
        {
            $newDuration = $this->duration - $ticks;
            if ($newDuration <= 0) {
                $newDuration = $this->infinite ? $newDuration + self::INFINITE_DURATION : 0;
            }
            $this->duration = $newDuration;
            return $this;
        }

        public function hasExpired(): bool
        {
            return $this->duration <= 0;
        }

        public function getAmplifier(): int
        {
            return $this->amplifier;
        }

        public function getEffectLevel(): int
        {
            return $this->amplifier + 1;
        }

        public function setAmplifier(int $amplifier): EffectInstance
        {
            if ($amplifier < 0 || $amplifier > 255) {
                throw new \InvalidArgumentException("Effect amplifier must be in range 0 - 255, got $amplifier");
            }
            $this->amplifier = $amplifier;
            return $this;
        }

        public function isVisible(): bool
        {
            return $this->visible;
        }

        public function setVisible(bool $visible): EffectInstance
        {
            $this->visible = $visible;
            return $this;
        }

        public function isAmbient(): bool
        {
            return $this->ambient;
        }

        public function setAmbient(bool $ambient): EffectInstance
        {
            $this->ambient = $ambient;
            return $this;
        }

        public function getColor(): Color
        {
            return $this->color;
        }

        public function setColor(Color $color): EffectInstance
        {
            $this->color = $color;
            return $this;
        }

        public function isInfinite(): bool
        {
            return $this->infinite;
        }

        public function setInfinite(bool $infinite): EffectInstance
        {
            $this->infinite = $infinite;
            return $this;
        }
    }

    /**
     * `pocketmine\entity\effect\EffectCollection`: the effects of one living
     * entity, keyed by effect type.
     */
    class EffectCollection implements \Countable, \IteratorAggregate
    {
        /** @var array<int, EffectInstance> spl_object_id(Effect) => instance */
        private array $effects = [];

        public function add(EffectInstance $effect): void
        {
            $this->effects[spl_object_id($effect->getType())] = $effect;
        }

        public function remove(Effect $effect): void
        {
            unset($this->effects[spl_object_id($effect)]);
        }

        public function has(Effect $effect): bool
        {
            return isset($this->effects[spl_object_id($effect)]);
        }

        public function get(Effect $effect): ?EffectInstance
        {
            return $this->effects[spl_object_id($effect)] ?? null;
        }

        /** @return EffectInstance[] */
        public function all(): array
        {
            return array_values($this->effects);
        }

        public function clear(): void
        {
            $this->effects = [];
        }

        /** `EffectCollection::tick()`: one tick off every finite effect. */
        public function tick(): void
        {
            foreach ($this->effects as $id => $effect) {
                $effect->decreaseDuration(1);
                if ($effect->hasExpired()) {
                    unset($this->effects[$id]);
                }
            }
        }

        public function count(): int
        {
            return count($this->effects);
        }

        public function getIterator(): \ArrayIterator
        {
            return new \ArrayIterator(array_values($this->effects));
        }
    }

    /**
     * `pocketmine\entity\effect\EffectManager`: name => `Effect` registry used
     * by the string parsers and by plugins registering custom effects.
     */
    final class EffectManager
    {
        /** @var array<string, Effect> */
        private static array $effects = [];

        public static function register(string $name, Effect $effect, bool $overwrite = false): void
        {
            $key = strtolower($name);
            if (!$overwrite && isset(self::$effects[$key])) {
                throw new \InvalidArgumentException("Effect $name is already registered");
            }
            self::$effects[$key] = $effect;
        }

        public static function getEffectByName(string $name): ?Effect
        {
            return self::$effects[strtolower($name)] ?? null;
        }

        /** @return array<string, Effect> */
        public static function getEffects(): array
        {
            return self::$effects;
        }
    }

    /**
     * Generated registry class in PM5: one static method per vanilla effect.
     *
     * Every effect is also registered in `EffectManager` and in `EffectIdMap`
     * under its Bedrock id.
     */
    final class VanillaEffects
    {
        /** @var array<string, Effect> */
        private static array $registry = [];

        private static function effect(string $name): Effect
        {
            return self::$registry[$name] ?? throw new \LogicException("Unknown effect $name");
        }

        private static function register(string $name, int $id, Effect $effect): void
        {
            self::$registry[$name] = $effect;
            EffectManager::register($name, $effect, true);
            EffectIdMap::register($id, $effect);
        }

        private static function setup(): void
        {
            if (self::$registry !== []) {
                return;
            }
            $c = static fn(int $r, int $g, int $b): Color => new Color($r, $g, $b);
            // Bedrock effect ids (`EffectIdMap` in PM5).
            self::register('absorption', 22, new AbsorptionEffect('potion.absorption', $c(0x25, 0x52, 0xa5)));
            self::register('blindness', 15, new Effect('potion.blindness', $c(0x1f, 0x1f, 0x23), true));
            self::register('conduit_power', 26, new Effect('potion.conduitPower', $c(0x1d, 0xc2, 0xd1)));
            self::register('darkness', 30, new Effect('effect.darkness', $c(0x29, 0x27, 0x21), true, 600, false));
            self::register('fatal_poison', 25, new PoisonEffect('potion.poison', $c(0x4e, 0x93, 0x31), true, 600, true, true));
            self::register('fire_resistance', 12, new Effect('potion.fireResistance', $c(0xe4, 0x9a, 0x3a)));
            self::register('haste', 3, new Effect('potion.digSpeed', $c(0xd9, 0xc0, 0x43)));
            self::register('health_boost', 21, new HealthBoostEffect('potion.healthBoost', $c(0xf8, 0x7d, 0x23)));
            self::register('hunger', 17, new HungerEffect('potion.hunger', $c(0x58, 0x76, 0x53), true));
            self::register('instant_damage', 7, new InstantDamageEffect('potion.harm', $c(0x43, 0x0a, 0x09), true, 1, false));
            self::register('instant_health', 6, new InstantHealthEffect('potion.heal', $c(0xf8, 0x24, 0x23), false, 1, false));
            self::register('invisibility', 14, new InvisibilityEffect('potion.invisibility', $c(0x7f, 0x83, 0x92)));
            self::register('jump_boost', 8, new Effect('potion.jump', $c(0x22, 0xff, 0x4c)));
            self::register('levitation', 24, new LevitationEffect('potion.levitation', $c(0xce, 0xff, 0xff)));
            self::register('mining_fatigue', 4, new Effect('potion.digSlowDown', $c(0x4a, 0x42, 0x17), true));
            self::register('nausea', 9, new Effect('potion.confusion', $c(0x55, 0x1d, 0x4a), true));
            self::register('night_vision', 16, new Effect('potion.nightVision', $c(0x1f, 0x1f, 0xa1)));
            self::register('poison', 19, new PoisonEffect('potion.poison', $c(0x4e, 0x93, 0x31), true));
            self::register('regeneration', 10, new RegenerationEffect('potion.regeneration', $c(0xcd, 0x5c, 0xab)));
            self::register('resistance', 11, new Effect('potion.resistance', $c(0x99, 0x45, 0x3a)));
            self::register('saturation', 23, new SaturationEffect('potion.saturation', $c(0xf8, 0x24, 0x23)));
            self::register('slowness', 2, new SlownessEffect('potion.moveSlowdown', $c(0x5a, 0x6c, 0x81), true));
            self::register('speed', 1, new SpeedEffect('potion.moveSpeed', $c(0x7c, 0xaf, 0xc6)));
            self::register('strength', 5, new Effect('potion.damageBoost', $c(0x93, 0x24, 0x23)));
            self::register('water_breathing', 13, new Effect('potion.waterBreathing', $c(0x2e, 0x52, 0x99)));
            self::register('weakness', 18, new Effect('potion.weakness', $c(0x48, 0x4d, 0x48), true));
            self::register('wither', 20, new WitherEffect('potion.wither', $c(0x35, 0x2a, 0x27), true));
        }

        public static function ABSORPTION(): Effect { self::setup(); return self::effect('absorption'); }
        public static function BLINDNESS(): Effect { self::setup(); return self::effect('blindness'); }
        public static function CONDUIT_POWER(): Effect { self::setup(); return self::effect('conduit_power'); }
        public static function DARKNESS(): Effect { self::setup(); return self::effect('darkness'); }
        public static function FATAL_POISON(): Effect { self::setup(); return self::effect('fatal_poison'); }
        public static function FIRE_RESISTANCE(): Effect { self::setup(); return self::effect('fire_resistance'); }
        public static function HASTE(): Effect { self::setup(); return self::effect('haste'); }
        public static function HEALTH_BOOST(): Effect { self::setup(); return self::effect('health_boost'); }
        public static function HUNGER(): Effect { self::setup(); return self::effect('hunger'); }
        public static function INSTANT_DAMAGE(): Effect { self::setup(); return self::effect('instant_damage'); }
        public static function INSTANT_HEALTH(): Effect { self::setup(); return self::effect('instant_health'); }
        public static function INVISIBILITY(): Effect { self::setup(); return self::effect('invisibility'); }
        public static function JUMP_BOOST(): Effect { self::setup(); return self::effect('jump_boost'); }
        public static function LEVITATION(): Effect { self::setup(); return self::effect('levitation'); }
        public static function MINING_FATIGUE(): Effect { self::setup(); return self::effect('mining_fatigue'); }
        public static function NAUSEA(): Effect { self::setup(); return self::effect('nausea'); }
        public static function NIGHT_VISION(): Effect { self::setup(); return self::effect('night_vision'); }
        public static function POISON(): Effect { self::setup(); return self::effect('poison'); }
        public static function REGENERATION(): Effect { self::setup(); return self::effect('regeneration'); }
        public static function RESISTANCE(): Effect { self::setup(); return self::effect('resistance'); }
        public static function SATURATION(): Effect { self::setup(); return self::effect('saturation'); }
        public static function SLOWNESS(): Effect { self::setup(); return self::effect('slowness'); }
        public static function SPEED(): Effect { self::setup(); return self::effect('speed'); }
        public static function STRENGTH(): Effect { self::setup(); return self::effect('strength'); }
        public static function WATER_BREATHING(): Effect { self::setup(); return self::effect('water_breathing'); }
        public static function WEAKNESS(): Effect { self::setup(); return self::effect('weakness'); }
        public static function WITHER(): Effect { self::setup(); return self::effect('wither'); }
    }

    /** `pocketmine\entity\effect\StringToEffectParser`. */
    final class StringToEffectParser
    {
        private static ?self $instance = null;
        /** @var array<string, \Closure(): ?Effect> */
        private array $parsers = [];

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
        }

        public function register(string $string, \Closure $getEffectFunc): void
        {
            $this->parsers[strtolower($string)] = $getEffectFunc;
        }

        public function parse(string $string): ?Effect
        {
            $key = strtolower($string);
            if (isset($this->parsers[$key])) {
                return ($this->parsers[$key])();
            }
            return EffectManager::getEffectByName($key);
        }
    }
}

namespace pocketmine\data\bedrock\effect {
    use pocketmine\entity\effect\Effect;

    /**
     * `pocketmine\data\bedrock\effect\EffectIdMap`: Bedrock effect id <=>
     * `Effect`. PM5 generates it from the Bedrock data; this port registers the
     * vanilla entries from `VanillaEffects`.
     */
    final class EffectIdMap
    {
        /** @var array<int, Effect> */
        private static array $byId = [];
        /** @var array<int, int> spl_object_id(Effect) => id */
        private static array $byObject = [];

        public static function register(int $id, Effect $effect): void
        {
            self::$byId[$id] = $effect;
            self::$byObject[spl_object_id($effect)] = $id;
        }

        public static function toId(Effect $effect): ?int
        {
            return self::$byObject[spl_object_id($effect)] ?? null;
        }

        public static function fromId(int $id): ?Effect
        {
            return self::$byId[$id] ?? null;
        }

        /** @return array<int, Effect> */
        public static function getAll(): array
        {
            return self::$byId;
        }
    }
}
