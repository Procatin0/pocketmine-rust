<?php

declare(strict_types=1);

/*
 * `pocketmine\world\biome` — port de `target/axolotl-pm/src/world/biome`.
 *
 * Igual que PM5: `Biome` es la clase base de datos (id, nombre, temperatura,
 * lluvia, elevación y ground cover), `BiomeRegistry` es un singleton que asigna
 * los ids de `BiomeIds` y `UnknownBiome` hace de polyfill para ids sin clase.
 *
 * Divergencias documentadas (docs/plugin-compatibility.md §4.17):
 * - No hay `Populator`/`TallGrass` en el puente: `addPopulator()` guarda el
 *   objeto pero `populateChunk()` es no-op (el generador nativo no ejecuta
 *   populadores PHP).
 * - `setGroundCover()` recibe `Block[]` como en PM; las clases base los crean
 *   con `BlockFactory::get()` sobre una posición dummy, porque en este puente
 *   un `Block` siempre lleva posición.
 */

namespace pocketmine\world\biome {

    use pocketmine\data\bedrock\BiomeIds;
    use pocketmine\math\Position;
    use pocketmine\world\BlockFactory;

    abstract class Biome
    {
        public const MAX_BIOMES = 256;

        private int $id = 0;
        private bool $registered = false;

        /** @var array<int, mixed> */
        private array $populators = [];

        private int $minElevation = 0;
        private int $maxElevation = 0;

        /** @var \pocketmine\block\Block[] */
        private array $groundCover = [];

        protected float $rainfall = 0.5;
        protected float $temperature = 0.5;

        public function clearPopulators(): void
        {
            $this->populators = [];
        }

        public function addPopulator(mixed $populator): void
        {
            $this->populators[] = $populator;
        }

        /** No-op: el generador nativo no ejecuta populadores PHP. */
        public function populateChunk(mixed $world, int $chunkX, int $chunkZ, mixed $random): void
        {
        }

        /** @return array<int, mixed> */
        public function getPopulators(): array
        {
            return $this->populators;
        }

        public function setId(int $id): void
        {
            if (!$this->registered) {
                $this->registered = true;
                $this->id = $id;
            }
        }

        public function getId(): int
        {
            return $this->id;
        }

        abstract public function getName(): string;

        public function getMinElevation(): int
        {
            return $this->minElevation;
        }

        public function getMaxElevation(): int
        {
            return $this->maxElevation;
        }

        public function setElevation(int $min, int $max): void
        {
            $this->minElevation = $min;
            $this->maxElevation = $max;
        }

        /**
         * `VanillaBlocks::X()` no existe en este runtime: el ground cover se
         * construye por nombre con `BlockFactory::get()` (posición dummy).
         *
         * @param string[] $covers
         */
        protected function setGroundCoverByName(array $covers): void
        {
            $position = new Position(0.0, 0.0, 0.0, '');
            $blocks = [];
            foreach ($covers as $name) {
                $blocks[] = BlockFactory::get($name, $position);
            }
            $this->groundCover = $blocks;
        }

        /** @return \pocketmine\block\Block[] */
        public function getGroundCover(): array
        {
            return $this->groundCover;
        }

        /** @param \pocketmine\block\Block[] $covers */
        public function setGroundCover(array $covers): void
        {
            $this->groundCover = $covers;
        }

        public function getTemperature(): float
        {
            return $this->temperature;
        }

        public function getRainfall(): float
        {
            return $this->rainfall;
        }
    }

    abstract class GrassyBiome extends Biome
    {
        public function __construct()
        {
            $this->setGroundCoverByName([
                'grass_block',
                'dirt',
                'dirt',
                'dirt',
                'dirt',
            ]);
        }
    }

    abstract class SandyBiome extends Biome
    {
        public function __construct()
        {
            $this->setGroundCoverByName([
                'sand',
                'sand',
                'sandstone',
                'sandstone',
                'sandstone',
            ]);
        }
    }

    abstract class SnowyBiome extends Biome
    {
        public function __construct()
        {
            $this->setGroundCoverByName([
                'snow_layer',
                'grass_block',
                'dirt',
                'dirt',
                'dirt',
            ]);
        }
    }

    class OceanBiome extends Biome
    {
        public function __construct()
        {
            $this->setGroundCoverByName(['gravel', 'gravel', 'gravel', 'gravel', 'gravel']);
            $this->setElevation(46, 58);
            $this->temperature = 0.5;
            $this->rainfall = 0.5;
        }

        public function getName(): string
        {
            return 'Ocean';
        }
    }

    class PlainBiome extends GrassyBiome
    {
        public function __construct()
        {
            parent::__construct();
            $this->setElevation(63, 68);
            $this->temperature = 0.8;
            $this->rainfall = 0.4;
        }

        public function getName(): string
        {
            return 'Plains';
        }
    }

    class DesertBiome extends SandyBiome
    {
        public function __construct()
        {
            parent::__construct();
            $this->setElevation(63, 74);
            $this->temperature = 2.0;
            $this->rainfall = 0.0;
        }

        public function getName(): string
        {
            return 'Desert';
        }
    }

    class MountainsBiome extends GrassyBiome
    {
        public function __construct()
        {
            parent::__construct();
            $this->setElevation(63, 127);
            $this->temperature = 0.4;
            $this->rainfall = 0.5;
        }

        public function getName(): string
        {
            return 'Mountains';
        }
    }

    class ForestBiome extends GrassyBiome
    {
        public function __construct(private ?int $type = null)
        {
            parent::__construct();
            $this->setElevation(63, 81);
            if ($type === self::TYPE_BIRCH) {
                $this->temperature = 0.7;
                $this->rainfall = 0.8;
            } else {
                $this->temperature = 0.6;
                $this->rainfall = 0.5;
            }
        }

        /** `TreeType::BIRCH` (el puente no tiene `TreeType`). */
        public const TYPE_BIRCH = 2;

        public function getType(): ?int
        {
            return $this->type;
        }

        public function getName(): string
        {
            return 'Forest';
        }
    }

    class TaigaBiome extends SnowyBiome
    {
        public function __construct()
        {
            parent::__construct();
            $this->setElevation(63, 81);
            $this->temperature = 0.05;
            $this->rainfall = 0.8;
        }

        public function getName(): string
        {
            return 'Taiga';
        }
    }

    class SwampBiome extends GrassyBiome
    {
        public function __construct()
        {
            parent::__construct();
            $this->setElevation(62, 63);
            $this->temperature = 0.8;
            $this->rainfall = 0.9;
        }

        public function getName(): string
        {
            return 'Swamp';
        }
    }

    class RiverBiome extends Biome
    {
        public function __construct()
        {
            $this->setGroundCoverByName(['dirt', 'dirt', 'dirt', 'dirt', 'dirt']);
            $this->setElevation(58, 62);
            $this->temperature = 0.5;
            $this->rainfall = 0.7;
        }

        public function getName(): string
        {
            return 'River';
        }
    }

    class HellBiome extends Biome
    {
        public function getName(): string
        {
            return 'Hell';
        }
    }

    class IcePlainsBiome extends SnowyBiome
    {
        public function __construct()
        {
            parent::__construct();
            $this->setElevation(63, 74);
            $this->temperature = 0.05;
            $this->rainfall = 0.8;
        }

        public function getName(): string
        {
            return 'Ice Plains';
        }
    }

    class SmallMountainsBiome extends MountainsBiome
    {
        public function __construct()
        {
            parent::__construct();
            $this->setElevation(63, 97);
        }

        public function getName(): string
        {
            return 'Small Mountains';
        }
    }

    /** Polyfill de PM para ids sin clase registrada (`UnknownBiome`). */
    class UnknownBiome extends Biome
    {
        public function getName(): string
        {
            return 'Unknown';
        }
    }

    final class BiomeRegistry
    {
        private static ?BiomeRegistry $instance = null;

        /** @var \SplFixedArray<Biome> */
        private \SplFixedArray $biomes;

        public function __construct()
        {
            $this->biomes = new \SplFixedArray(Biome::MAX_BIOMES);

            $this->register(BiomeIds::OCEAN, new OceanBiome());
            $this->register(BiomeIds::PLAINS, new PlainBiome());
            $this->register(BiomeIds::DESERT, new DesertBiome());
            $this->register(BiomeIds::EXTREME_HILLS, new MountainsBiome());
            $this->register(BiomeIds::FOREST, new ForestBiome());
            $this->register(BiomeIds::TAIGA, new TaigaBiome());
            $this->register(BiomeIds::SWAMPLAND, new SwampBiome());
            $this->register(BiomeIds::RIVER, new RiverBiome());
            $this->register(BiomeIds::HELL, new HellBiome());
            $this->register(BiomeIds::ICE_PLAINS, new IcePlainsBiome());
            $this->register(BiomeIds::EXTREME_HILLS_EDGE, new SmallMountainsBiome());
            $this->register(BiomeIds::BIRCH_FOREST, new ForestBiome(ForestBiome::TYPE_BIRCH));
        }

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
        }

        public static function reset(): void
        {
            self::$instance = null;
        }

        public function register(int $id, Biome $biome): void
        {
            if ($id < 0 || $id >= Biome::MAX_BIOMES) {
                throw new \InvalidArgumentException('Biome id must be in the range 0..255');
            }
            $this->biomes[$id] = $biome;
            $biome->setId($id);
        }

        public function getBiome(int $id): Biome
        {
            $biome = $this->biomes[$id] ?? null;
            if ($biome === null) {
                $biome = new UnknownBiome();
                $this->register($id, $biome);
            }
            return $biome;
        }

        /** @return array<int, Biome> */
        public function getBiomes(): array
        {
            $biomes = [];
            foreach ($this->biomes as $id => $biome) {
                if ($biome !== null) {
                    $biomes[$id] = $biome;
                }
            }
            return $biomes;
        }
    }

}

namespace pocketmine\data\bedrock {

    /**
     * `pocketmine\data\bedrock\BiomeIds`, catálogo completo de PM5
     * (`target/axolotl-pm/generated/data/bedrock/BiomeIds.php`).
     *
     * Los ids son los de Bedrock; `BiomeRegistry` solo instancia las clases de
     * bioma portadas y `getBiome()` devuelve `UnknownBiome` para el resto.
     */
    final class BiomeIds
    {
        public const OCEAN = 0;
        public const PLAINS = 1;
        public const DESERT = 2;
        public const EXTREME_HILLS = 3;
        public const FOREST = 4;
        public const TAIGA = 5;
        public const SWAMPLAND = 6;
        public const RIVER = 7;
        public const HELL = 8;
        public const THE_END = 9;
        public const LEGACY_FROZEN_OCEAN = 10;
        public const FROZEN_RIVER = 11;
        public const ICE_PLAINS = 12;
        public const ICE_MOUNTAINS = 13;
        public const MUSHROOM_ISLAND = 14;
        public const MUSHROOM_ISLAND_SHORE = 15;
        public const BEACH = 16;
        public const DESERT_HILLS = 17;
        public const FOREST_HILLS = 18;
        public const TAIGA_HILLS = 19;
        public const EXTREME_HILLS_EDGE = 20;
        public const JUNGLE = 21;
        public const JUNGLE_HILLS = 22;
        public const JUNGLE_EDGE = 23;
        public const DEEP_OCEAN = 24;
        public const STONE_BEACH = 25;
        public const COLD_BEACH = 26;
        public const BIRCH_FOREST = 27;
        public const BIRCH_FOREST_HILLS = 28;
        public const ROOFED_FOREST = 29;
        public const COLD_TAIGA = 30;
        public const COLD_TAIGA_HILLS = 31;
        public const MEGA_TAIGA = 32;
        public const MEGA_TAIGA_HILLS = 33;
        public const EXTREME_HILLS_PLUS_TREES = 34;
        public const SAVANNA = 35;
        public const SAVANNA_PLATEAU = 36;
        public const MESA = 37;
        public const MESA_PLATEAU_STONE = 38;
        public const MESA_PLATEAU = 39;
        public const WARM_OCEAN = 40;
        public const DEEP_WARM_OCEAN = 41;
        public const LUKEWARM_OCEAN = 42;
        public const DEEP_LUKEWARM_OCEAN = 43;
        public const COLD_OCEAN = 44;
        public const DEEP_COLD_OCEAN = 45;
        public const FROZEN_OCEAN = 46;
        public const DEEP_FROZEN_OCEAN = 47;
        public const BAMBOO_JUNGLE = 48;
        public const BAMBOO_JUNGLE_HILLS = 49;

        public const SUNFLOWER_PLAINS = 129;
        public const DESERT_MUTATED = 130;
        public const EXTREME_HILLS_MUTATED = 131;
        public const FLOWER_FOREST = 132;
        public const TAIGA_MUTATED = 133;
        public const SWAMPLAND_MUTATED = 134;

        public const ICE_PLAINS_SPIKES = 140;

        public const JUNGLE_MUTATED = 149;

        public const JUNGLE_EDGE_MUTATED = 151;

        public const BIRCH_FOREST_MUTATED = 155;
        public const BIRCH_FOREST_HILLS_MUTATED = 156;
        public const ROOFED_FOREST_MUTATED = 157;
        public const COLD_TAIGA_MUTATED = 158;

        public const REDWOOD_TAIGA_MUTATED = 160;
        public const REDWOOD_TAIGA_HILLS_MUTATED = 161;
        public const EXTREME_HILLS_PLUS_TREES_MUTATED = 162;
        public const SAVANNA_MUTATED = 163;
        public const SAVANNA_PLATEAU_MUTATED = 164;
        public const MESA_BRYCE = 165;
        public const MESA_PLATEAU_STONE_MUTATED = 166;
        public const MESA_PLATEAU_MUTATED = 167;

        public const SOULSAND_VALLEY = 178;
        public const CRIMSON_FOREST = 179;
        public const WARPED_FOREST = 180;
        public const BASALT_DELTAS = 181;
        public const JAGGED_PEAKS = 182;
        public const FROZEN_PEAKS = 183;
        public const SNOWY_SLOPES = 184;
        public const GROVE = 185;
        public const MEADOW = 186;
        public const LUSH_CAVES = 187;
        public const DRIPSTONE_CAVES = 188;
        public const STONY_PEAKS = 189;
        public const DEEP_DARK = 190;
        public const MANGROVE_SWAMP = 191;
        public const CHERRY_GROVE = 192;
        public const PALE_GARDEN = 193;
        public const SULFUR_CAVES = 194;
        public const DAPPLED_FOREST = 195;
        public const ICE_CAVES = 196;
    }
}
