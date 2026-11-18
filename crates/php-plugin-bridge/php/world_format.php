<?php

declare(strict_types=1);

/**
 * PM `pocketmine\world\format`: la vista de columna que ven los plugins.
 *
 * El parser de red vive en `pocketmine\world\Chunk` (compatibilidad con el
 * bridge Java); `format\Chunk` lo extiende con coordenadas de columna, mapa de
 * alturas, subsecciones y una capa de escritura local. `World::getChunk()`
 * devuelve esta clase para que `instanceof \pocketmine\world\format\Chunk`
 * funcione como en PM5.
 *
 * Límite: los cambios hechos con `setBlockStateId()`/`setSubChunk()` quedan en
 * el objeto PHP (es un snapshot); para cambiar el mundo hay que usar
 * `World::setBlockAt()`.
 */
namespace pocketmine\world\format {

    use pocketmine\world\Chunk as LegacyChunk;

    /** PM `ChunkException`: columna inválida o corrupta. */
    class ChunkException extends \RuntimeException
    {
    }

    /** PM `HeightArray`: 256 alturas en orden `ZZZZXXXX` (z << 4 | x). */
    final class HeightArray
    {
        /** @var \SplFixedArray<int> */
        private \SplFixedArray $array;

        /** @param int[] $values */
        public function __construct(array $values)
        {
            if (count($values) !== 256) {
                throw new \InvalidArgumentException('Expected exactly 256 values');
            }
            $this->array = \SplFixedArray::fromArray(array_values($values));
        }

        public static function fill(int $value): self
        {
            return new self(array_fill(0, 256, $value));
        }

        private static function idx(int $x, int $z): int
        {
            if ($x < 0 || $x >= 16 || $z < 0 || $z >= 16) {
                throw new \InvalidArgumentException('x and z must be in the range 0-15');
            }
            return ($z << 4) | $x;
        }

        public function get(int $x, int $z): int
        {
            return (int)$this->array[self::idx($x, $z)];
        }

        public function set(int $x, int $z, int $height): void
        {
            $this->array[self::idx($x, $z)] = $height;
        }

        /** @return int[] */
        public function getValues(): array
        {
            return $this->array->toArray();
        }

        public function __clone()
        {
            $this->array = clone $this->array;
        }
    }

    /**
     * PM `SubChunk`: una sección de 16×16×16 bloques.
     *
     * PM guarda `PalettedBlockArray`/`LightArray`; aquí la lectura delega en la
     * columna parseada y las escrituras van a su capa local. `getBlockLayers()`
     * devuelve `[]` (no hay modelo multi-capa) y la luz no se almacena: PM la
     * recalcula en el servidor, este core no tiene motor de luz.
     */
    final class SubChunk
    {
        public function __construct(
            private Chunk $chunk,
            private int $subChunkIndex
        ) {
        }

        private function baseY(): int
        {
            return $this->subChunkIndex << 4;
        }

        private static function check(int $x, int $y, int $z): void
        {
            if ($x < 0 || $x > 15 || $y < 0 || $y > 15 || $z < 0 || $z > 15) {
                throw new \InvalidArgumentException('x, y and z must be in the range 0-15');
            }
        }

        public function getSubChunkIndex(): int
        {
            return $this->subChunkIndex;
        }

        /** Id de estado de red en coordenadas locales de la sección. */
        public function getBlockStateId(int $x, int $y, int $z): int
        {
            self::check($x, $y, $z);
            return $this->chunk->getBlockStateId($x, $this->baseY() + $y, $z);
        }

        public function setBlockStateId(int $x, int $y, int $z, int $block): void
        {
            self::check($x, $y, $z);
            $this->chunk->setBlockStateId($x, $this->baseY() + $y, $z, $block);
        }

        public function isEmptyFast(): bool
        {
            if ($this->chunk->hasLocalBlocksInSection($this->subChunkIndex)) {
                return false;
            }
            $section = $this->chunk->getSectionPayload($this->baseY());
            if ($section === null) {
                return true;
            }
            foreach ($section['palette'] as $id) {
                if ((int)$id !== Chunk::AIR && (int)$id !== 0) {
                    return false;
                }
            }
            return true;
        }

        /** Sin capas de luz ni paletas 3D no hay una versión "autoritativa" distinta. */
        public function isEmptyAuthoritative(): bool
        {
            return $this->isEmptyFast();
        }

        public function getEmptyBlockId(): int
        {
            return Chunk::AIR;
        }

        /** @return array<int, mixed> */
        public function getBlockLayers(): array
        {
            return [];
        }

        /** Mayor `y` absoluto no aire de la columna local, o `null`. */
        public function getHighestBlockAt(int $x, int $z): ?int
        {
            if ($x < 0 || $x > 15 || $z < 0 || $z > 15) {
                throw new \InvalidArgumentException('x and z must be in the range 0-15');
            }
            for ($y = 15; $y >= 0; $y--) {
                $id = $this->chunk->getBlockStateId($x, $this->baseY() + $y, $z);
                if ($id !== Chunk::AIR && $id !== 0) {
                    return $this->baseY() + $y;
                }
            }
            return null;
        }

        public function collectGarbage(): void
        {
        }
    }

    class Chunk extends LegacyChunk
    {
        public const COORD_BIT_SIZE = 4;
        public const COORD_MASK = 0xF;

        /** @var array<string, int> escrituras locales, clave `x:y:z` */
        private array $overrides = [];

        /** @var array<int, SubChunk> */
        private array $subChunkCache = [];

        private bool $populated = true;

        public function __construct(
            string $payload = '',
            int $subchunks = 24,
            private int $chunkX = 0,
            private int $chunkZ = 0
        ) {
            parent::__construct($payload, $subchunks);
        }

        public function getX(): int
        {
            return $this->chunkX;
        }

        public function getZ(): int
        {
            return $this->chunkZ;
        }

        /** PM `getHeight()`: altura total en bloques (384 en el overworld actual). */
        public function getHeight(): int
        {
            return self::MAX_Y - self::MIN_Y + 1;
        }

        public function getBlockStateId(int $x, int $y, int $z): int
        {
            return $this->overrides[$x . ':' . $y . ':' . $z] ?? parent::getBlockStateId($x, $y, $z);
        }

        /** Escritura local (snapshot): no llega al mundo nativo. */
        public function setBlockStateId(int $x, int $y, int $z, int $block): void
        {
            $this->overrides[$x . ':' . $y . ':' . $z] = $block;
        }

        /** ¿La capa local escribió algún bloque no aire en esta subsección? */
        public function hasLocalBlocksInSection(int $sectionIndex): bool
        {
            $baseY = $sectionIndex << 4;
            foreach ($this->overrides as $key => $id) {
                if ($id === self::AIR || $id === 0) {
                    continue;
                }
                $y = (int)explode(':', $key)[1];
                if ($y >= $baseY && $y < $baseY + 16) {
                    return true;
                }
            }
            return false;
        }

        public function getHighestBlockAt(int $x, int $z): int
        {
            $highest = parent::getHighestBlockAt($x, $z);
            foreach ($this->overrides as $key => $id) {
                if ($id === self::AIR || $id === 0) {
                    continue;
                }
                [$ox, $oy, $oz] = array_map('intval', explode(':', $key));
                if ($ox === $x && $oz === $z && $oy > $highest) {
                    $highest = $oy;
                }
            }
            return $highest;
        }

        /** PM `getHeightMap()`: mayor `y` no aire de la columna. */
        public function getHeightMap(int $x, int $z): int
        {
            return $this->getHighestBlockAt($x, $z);
        }

        /**
         * PM `getSubChunk()`: el argumento es el índice de subsección
         * (`-4..19`), no una coordenada de bloque.
         */
        public function getSubChunk(int $y): SubChunk
        {
            if ($y < self::MIN_SUBCHUNK_INDEX || $y > self::MAX_SUBCHUNK_INDEX) {
                throw new \InvalidArgumentException("Invalid subchunk Y coordinate $y");
            }
            return $this->subChunkCache[$y] ??= new SubChunk($this, $y);
        }

        /** @return array<int, SubChunk> */
        public function getSubChunks(): array
        {
            $result = [];
            for ($y = self::MIN_SUBCHUNK_INDEX; $y <= self::MAX_SUBCHUNK_INDEX; $y++) {
                $result[$y] = $this->getSubChunk($y);
            }
            return $result;
        }

        public function isPopulated(): bool
        {
            return $this->populated;
        }

        public function setPopulated(bool $value = true): void
        {
            $this->populated = $value;
        }

        /** @return array<int, mixed> */
        public function getTiles(): array
        {
            return [];
        }

        public function getTile(int $x, int $y, int $z): mixed
        {
            return null;
        }

        public function getChunkCoordHash(): int
        {
            return ($this->chunkX << 32) | ($this->chunkZ & 0xFFFFFFFF);
        }
    }
}
