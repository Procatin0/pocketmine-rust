<?php

declare(strict_types=1);

/*
 * `pocketmine\world\particle` — port de `target/axolotl-pm/src/world/particle`.
 *
 * PM devuelve paquetes desde `encode()`; el puente no expone
 * `pocketmine\network\mcpe\protocol\*`, así que `encode()` devuelve `[]` y la
 * información de red viaja en `getPacketEvent()` (id de `LevelEventPacket` ya
 * con `ADD_PARTICLE_MASK` cuando es una partícula estándar) y `encodeData()`.
 * `World::addParticle()` la manda con `world.add_particle` y el tick emite el
 * `LevelEventPacket` (docs/plugin-compatibility.md §4.17).
 */

namespace pocketmine\world\particle {

    use pocketmine\block\Block;
    use pocketmine\color\Color;
    use pocketmine\item\Item;
    use pocketmine\math\Vector3;
    use pocketmine\world\BlockFactory;

    interface Particle
    {
        /** @return array<int, mixed> */
        public function encode(Vector3 $pos): array;
    }

    /** Base con la representación de red que usa el puente. */
    abstract class ParticleBase implements Particle
    {
        /** `LevelEvent::ADD_PARTICLE_MASK`. */
        public const ADD_PARTICLE_MASK = 0x4000;

        public function encode(Vector3 $pos): array
        {
            return [];
        }

        abstract public function getPacketEvent(): int;

        public function encodeData(): int
        {
            return 0;
        }
    }

    /**
     * Partícula "de cable" que reconstruye `WorldParticleEvent::fromArray()`:
     * el objeto PHP original no puede cruzar el IPC, así que el evento lleva el
     * par (`LevelEventPacket` id, data). `event_id = 0` = no emitir.
     *
     * @internal
     */
    final class WireParticle extends ParticleBase
    {
        public function __construct(private int $eventId, private int $eventData = 0) {}

        public function getPacketEvent(): int
        {
            return $this->eventId;
        }

        public function encodeData(): int
        {
            return $this->eventData;
        }
    }

    // ---- Partículas estándar (`ParticleIds` + ADD_PARTICLE_MASK) ----

    class AngryVillagerParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 40; }
    }

    class BubbleParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 1; }
    }

    class CriticalParticle extends ParticleBase
    {
        public function __construct(private int $scale = 2) {}

        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 3; }
        public function encodeData(): int { return $this->scale; }
    }

    class BlockForceFieldParticle extends ParticleBase
    {
        public function __construct(private int $data = 0) {}

        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 4; }
        public function encodeData(): int { return $this->data; }
    }

    class SmokeParticle extends ParticleBase
    {
        public function __construct(private int $scale = 0) {}

        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 5; }
        public function encodeData(): int { return $this->scale; }
    }

    class ExplodeParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 6; }
    }

    class FlameParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 8; }
    }

    class LavaParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 10; }
    }

    class RedstoneParticle extends ParticleBase
    {
        public function __construct(private int $lifetime = 1) {}

        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 12; }
        public function encodeData(): int { return $this->lifetime; }
    }

    class ItemBreakParticle extends ParticleBase
    {
        public function __construct(private Item $item) {}

        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 14; }

        public function encodeData(): int
        {
            // PM: `($id << 16) | $meta` con el id de red del item.
            return (BlockFactory::networkIdOf($this->item->getId()) << 16) | ($this->item->getMeta() & 0xffff);
        }
    }

    class SnowballPoofParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 15; }
    }

    class HugeExplodeParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 16; }
    }

    class HugeExplodeSeedParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 17; }
    }

    class EntityFlameParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 19; }
    }

    class HeartParticle extends ParticleBase
    {
        public function __construct(private int $scale = 0) {}

        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 20; }
        public function encodeData(): int { return $this->scale; }
    }

    class TerrainParticle extends ParticleBase
    {
        public function __construct(private Block $b) {}

        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 21; }
        public function encodeData(): int { return $this->b->getStateId(); }
    }

    class SporeParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 22; }
    }

    class PortalParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 23; }
    }

    class SplashParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 25; }
    }

    class WaterParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 27; }
    }

    class WaterDripParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 28; }
    }

    class LavaDripParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 29; }
    }

    class DustParticle extends ParticleBase
    {
        public function __construct(private Color $color) {}

        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 33; }
        public function encodeData(): int { return $this->color->toARGB(); }
    }

    class EnchantParticle extends ParticleBase
    {
        public function __construct(private Color $color) {}

        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 34; }
        public function encodeData(): int { return $this->color->toARGB(); }
    }

    class InstantEnchantParticle extends ParticleBase
    {
        public function __construct(private Color $color) {}

        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 36; }
        public function encodeData(): int { return $this->color->toARGB(); }
    }

    class InkParticle extends ParticleBase
    {
        public function __construct(private int $scale = 0) {}

        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 37; }
        public function encodeData(): int { return $this->scale; }
    }

    class RainSplashParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 39; }
    }

    class HappyVillagerParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 41; }
    }

    class EnchantmentTableParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 42; }
    }

    class SonicExplosionParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return self::ADD_PARTICLE_MASK | 85; }
    }

    // ---- Partículas con evento propio (`LevelEvent`) ----

    /** `LevelEvent::PARTICLE_DESTROY` (2001). */
    class BlockBreakParticle extends ParticleBase
    {
        public function __construct(private Block $b) {}

        public function getPacketEvent(): int { return 2001; }
        public function encodeData(): int { return $this->b->getStateId(); }
    }

    /** `LevelEvent::PARTICLE_PUNCH_BLOCK` (2014): estado del bloque | cara << 24. */
    class BlockPunchParticle extends ParticleBase
    {
        public function __construct(private Block $block, private int $face) {}

        public function getPacketEvent(): int { return 2014; }

        public function encodeData(): int
        {
            return $this->block->getStateId() | ($this->face << 24);
        }
    }

    /** `LevelEvent::PARTICLE_DRAGON_EGG_TELEPORT` (2010): diffs de -255..255. */
    class DragonEggTeleportParticle extends ParticleBase
    {
        private int $xDiff;
        private int $yDiff;
        private int $zDiff;

        public function __construct(int $xDiff, int $yDiff, int $zDiff)
        {
            $this->xDiff = self::boundOrThrow($xDiff);
            $this->yDiff = self::boundOrThrow($yDiff);
            $this->zDiff = self::boundOrThrow($zDiff);
        }

        private static function boundOrThrow(int $v): int
        {
            if ($v < -255 || $v > 255) {
                throw new \InvalidArgumentException('Value must be between -255 and 255');
            }
            return $v;
        }

        public function getPacketEvent(): int { return 2010; }

        public function encodeData(): int
        {
            return ($this->zDiff < 0 ? 1 << 26 : 0)
                | ($this->yDiff < 0 ? 1 << 25 : 0)
                | ($this->xDiff < 0 ? 1 << 24 : 0)
                | (abs($this->xDiff) << 16)
                | (abs($this->yDiff) << 8)
                | abs($this->zDiff);
        }
    }

    /** `LevelEvent::PARTICLE_ENDERMAN_TELEPORT` (2013). */
    class EndermanTeleportParticle extends ParticleBase
    {
        public function getPacketEvent(): int { return 2013; }
    }

    /** `LevelEvent::PARTICLE_SPAWN` (2004): ancho y alto del mob. */
    class MobSpawnParticle extends ParticleBase
    {
        public function __construct(protected int $width = 0, protected int $height = 0) {}

        public function getPacketEvent(): int { return 2004; }
        public function encodeData(): int { return ($this->width & 0xff) | (($this->height & 0xff) << 8); }
    }

    /** `LevelEvent::PARTICLE_SPLASH` (2002): color ARGB de la poción. */
    class PotionSplashParticle extends ParticleBase
    {
        public function __construct(private Color $color) {}

        public static function DEFAULT_COLOR(): Color
        {
            return new Color(0x38, 0x5d, 0xc6);
        }

        public function getColor(): Color
        {
            return $this->color;
        }

        public function getPacketEvent(): int { return 2002; }
        public function encodeData(): int { return $this->color->toARGB(); }
    }

    /**
     * PM la dibuja con `SpawnParticleEffectPacket` + entidades de texto;
     * el puente no tiene ninguno de los dos. `getPacketEvent()` devuelve 0
     * (señal de "no emitir") y `encode()` vacío, documentado en §4.17.
     */
    class FloatingTextParticle extends ParticleBase
    {
        protected bool $invisible = false;

        public function __construct(protected string $text, protected string $title = '') {}

        public function getText(): string
        {
            return $this->text;
        }

        public function setText(string $text): void
        {
            $this->text = $text;
        }

        public function getTitle(): string
        {
            return $this->title;
        }

        public function setTitle(string $title): void
        {
            $this->title = $title;
        }

        public function isInvisible(): bool
        {
            return $this->invisible;
        }

        public function setInvisible(bool $value = true): void
        {
            $this->invisible = $value;
        }

        public function getPacketEvent(): int { return 0; }
    }
}
