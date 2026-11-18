<?php

declare(strict_types=1);

/*
 * `pocketmine\world\sound` — port de `target/axolotl-pm/src/world/sound`.
 *
 * PM devuelve `LevelSoundEventPacket` o `LevelEventPacket` desde `encode()`;
 * el puente no expone paquetes al PHP, así que `encode()` devuelve `[]` y cada
 * sonido declara su representación de red:
 * - `getSoundName()`: nombre de `LevelSoundEvent` (0x7B), o `null`.
 * - `getEventId()`: id de `LevelEventPacket` (0x19), o `null`.
 * - `getExtraData()`: `extraData` del packet (id de bloque o `pitch * 1000`).
 *
 * `World::addSound()` las envía por `world.add_sound`. Documentado en
 * docs/plugin-compatibility.md §4.17.
 */

namespace pocketmine\world\sound {

    use pocketmine\block\Block;
    use pocketmine\math\Vector3;

    interface Sound
    {
        /** @return array<int, mixed> */
        public function encode(Vector3 $pos): array;
    }

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

    /**
     * Sonido "de cable" que reconstruye `WorldSoundEvent::fromArray()`: el
     * objeto PHP original no puede cruzar el IPC, así que el evento transporta
     * el nombre de `LevelSoundEvent` o el id de `LevelEvent`.
     *
     * @internal
     */
    final class WireSound extends SoundBase
    {
        public function __construct(
            private ?string $soundName,
            private ?int $eventId,
            private int $extraData
        ) {}

        public function getSoundName(): ?string
        {
            return $this->soundName;
        }

        public function getEventId(): ?int
        {
            return $this->eventId;
        }

        public function getExtraData(): int
        {
            return $this->extraData;
        }
    }

    // ---- Sonidos por bloque (`LevelSoundEvent`) ----

    class BlockBreakSound extends SoundBase
    {
        public function __construct(private Block $block) {}

        public function getSoundName(): ?string { return 'break'; }
        public function getExtraData(): int { return $this->block->getStateId(); }
    }

    class BlockPlaceSound extends SoundBase
    {
        public function __construct(private Block $block) {}

        public function getSoundName(): ?string { return 'place'; }
        public function getExtraData(): int { return $this->block->getStateId(); }
    }

    class BlockPunchSound extends SoundBase
    {
        public function __construct(private Block $block) {}

        public function getSoundName(): ?string { return 'hit'; }
        public function getExtraData(): int { return $this->block->getStateId(); }
    }

    // ---- Sonidos de entidad/objeto (`LevelSoundEvent`) ----

    class ChestOpenSound extends SoundBase
    {
        public function getSoundName(): ?string { return 'chest.open'; }
    }

    class ChestCloseSound extends SoundBase
    {
        public function getSoundName(): ?string { return 'chest.closed'; }
    }

    class ExplodeSound extends SoundBase
    {
        public function getSoundName(): ?string { return 'explode'; }
    }

    class BowShootSound extends SoundBase
    {
        public function getSoundName(): ?string { return 'bow'; }
    }

    class ArrowHitSound extends SoundBase
    {
        public function getSoundName(): ?string { return 'bow.hit'; }
    }

    class BurpSound extends SoundBase
    {
        public function getSoundName(): ?string { return 'burp'; }
    }

    // ---- Sonidos por `LevelEvent` ----

    class ClickSound extends SoundBase
    {
        public function __construct(private float $pitch = 0) {}

        public function getPitch(): float { return $this->pitch; }
        public function getEventId(): ?int { return 1000; }
        public function getExtraData(): int { return (int) ($this->pitch * 1000); }
    }

    class DoorSound extends SoundBase
    {
        public function __construct(private float $pitch = 0) {}

        public function getPitch(): float { return $this->pitch; }
        public function getEventId(): ?int { return 1003; }
        public function getExtraData(): int { return (int) ($this->pitch * 1000); }
    }

    class FizzSound extends SoundBase
    {
        public function __construct(private float $pitch = 0) {}

        public function getPitch(): float { return $this->pitch; }
        public function getEventId(): ?int { return 1004; }
        public function getExtraData(): int { return (int) ($this->pitch * 1000); }
    }

    class PopSound extends SoundBase
    {
        public function __construct(private float $pitch = 0) {}

        public function getPitch(): float { return $this->pitch; }
        public function getEventId(): ?int { return 1030; }
        public function getExtraData(): int { return (int) ($this->pitch * 1000); }
    }

    class AnvilBreakSound extends SoundBase
    {
        public function getEventId(): ?int { return 1020; }
    }

    class AnvilUseSound extends SoundBase
    {
        public function getEventId(): ?int { return 1021; }
    }

    class AnvilFallSound extends SoundBase
    {
        public function getEventId(): ?int { return 1022; }
    }

    class BlazeShootSound extends SoundBase
    {
        public function getEventId(): ?int { return 1009; }
    }

    class EndermanTeleportSound extends SoundBase
    {
        public function getEventId(): ?int { return 1018; }
    }

    class GhastShootSound extends SoundBase
    {
        public function getEventId(): ?int { return 1008; }
    }
}
