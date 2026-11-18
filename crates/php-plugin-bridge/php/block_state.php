<?php

declare(strict_types=1);

namespace pocketmine\data\bedrock\block {

    use pocketmine\nbt\tag\CompoundTag;
    use pocketmine\nbt\tag\Tag;

    /**
     * PM `BlockStateSerializeException`: no serializer could turn a block into
     * a Bedrock state.
     */
    class BlockStateSerializeException extends \RuntimeException
    {
    }

    /**
     * PM `BlockStateDeserializeException`: a state string/id could not be
     * resolved into a block.
     */
    class BlockStateDeserializeException extends \RuntimeException
    {
    }

    /**
     * PM `BlockStateData`: a Bedrock block state, i.e. `minecraft:stone` plus
     * its NBT state tags and the palette version it was read from.
     */
    final class BlockStateData
    {
        /** Palette version shipped by the server (generated from the palette). */
        public const CURRENT_VERSION = \pocketmine\data\bedrock\block\BlockStateVersion::PALETTE_VERSION;

        public const TAG_NAME = 'name';
        public const TAG_STATES = 'states';
        public const TAG_VERSION = 'version';

        /** @param array<string, Tag> $states */
        public function __construct(
            private string $name,
            private array $states,
            private int $version = self::CURRENT_VERSION
        ) {
        }

        /**
         * @param array<string, Tag> $states
         */
        public static function current(string $name, array $states): self
        {
            return new self($name, $states, self::CURRENT_VERSION);
        }

        /**
         * Reads a block state from vanilla NBT (`{name, states, version}`).
         *
         * @throws BlockStateDeserializeException
         */
        public static function fromNbt(CompoundTag $nbt): self
        {
            if (!$nbt->hasTag(self::TAG_NAME) || !$nbt->hasTag(self::TAG_STATES)) {
                throw new BlockStateDeserializeException('Missing tag "' . self::TAG_STATES . '"');
            }
            $version = $nbt->hasTag(self::TAG_VERSION) ? $nbt->getInt(self::TAG_VERSION) : 0;
            return new self(
                $nbt->getString(self::TAG_NAME),
                $nbt->getCompoundTag(self::TAG_STATES)->getValue(),
                $version
            );
        }

        public function getName(): string
        {
            return $this->name;
        }

        /** @return array<string, Tag> */
        public function getStates(): array
        {
            return $this->states;
        }

        public function getState(string $name): ?Tag
        {
            return $this->states[$name] ?? null;
        }

        public function getVersion(): int
        {
            return $this->version;
        }

        /** Palette versions are packed as `major.minor.patch.revision`. */
        public function getVersionAsString(): string
        {
            $major = ($this->version >> 24) & 0xff;
            $minor = ($this->version >> 16) & 0xff;
            $patch = ($this->version >> 8) & 0xff;
            $revision = $this->version & 0xff;
            return "$major.$minor.$patch.$revision";
        }

        /** Encodes the blockstate as a vanilla Bedrock TAG_Compound. */
        public function toVanillaNbt(): CompoundTag
        {
            $states = CompoundTag::create();
            foreach ($this->states as $key => $tag) {
                $states->setTag((string)$key, $tag);
            }
            return CompoundTag::create()
                ->setString(self::TAG_NAME, $this->name)
                ->setInt(self::TAG_VERSION, $this->version)
                ->setTag(self::TAG_STATES, $states);
        }

        /**
         * PM adds its world-data version tag (`__version`) here; the bridge
         * stores no PM save data, so the vanilla encoding is the full payload.
         */
        public function toNbt(): CompoundTag
        {
            return $this->toVanillaNbt();
        }

        public function equals(self $that): bool
        {
            if ($this->name !== $that->name || count($this->states) !== count($that->states)) {
                return false;
            }
            foreach ($this->states as $key => $tag) {
                $other = $that->states[$key] ?? null;
                if (
                    $other === null
                    || $other->getType() !== $tag->getType()
                    || $other->getValue() !== $tag->getValue()
                ) {
                    return false;
                }
            }
            return true;
        }
    }
}

namespace pocketmine\data\bedrock\block\convert {

    use pocketmine\data\bedrock\block\BlockStateData;
    use pocketmine\nbt\tag\ByteTag;
    use pocketmine\nbt\tag\IntTag;
    use pocketmine\nbt\tag\StringTag;
    use pocketmine\nbt\tag\Tag;

    /**
     * PM `UnsupportedBlockStateException`: a block class produced a state the
     * translator cannot encode.
     */
    class UnsupportedBlockStateException extends \InvalidArgumentException
    {
    }

    /**
     * PM `BlockStateWriter`: fluent builder used by block `serialize()` hooks.
     */
    final class BlockStateWriter
    {
        /** @var array<string, Tag> */
        private array $states = [];

        private function __construct(private string $id)
        {
        }

        public static function create(string $id): self
        {
            return new self($id);
        }

        public function writeBool(string $name, bool $value): self
        {
            $this->states[$name] = new ByteTag($value ? 1 : 0);
            return $this;
        }

        public function writeInt(string $name, int $value): self
        {
            $this->states[$name] = new IntTag($value);
            return $this;
        }

        public function writeString(string $name, string $value): self
        {
            $this->states[$name] = new StringTag($value);
            return $this;
        }

        public function getBlockStateData(): BlockStateData
        {
            return BlockStateData::current($this->id, $this->states);
        }
    }
}

namespace pocketmine\data\bedrock\block\convert {

    use pocketmine\block\Block;
    use pocketmine\block\BlockBreakInfo;
    use pocketmine\block\BlockIdentifier;
    use pocketmine\block\BlockTypeIds;
    use pocketmine\block\BlockTypeInfo;
    use pocketmine\block\UnknownBlock;
    use pocketmine\data\bedrock\block\BlockStateData;
    use pocketmine\data\bedrock\block\BlockStateDeserializeException;
    use pocketmine\data\bedrock\block\BlockStateSerializeException;
    use pocketmine\nbt\tag\CompoundTag;
    use pocketmine\nbt\tag\IntTag;
    use pocketmine\nbt\tag\StringTag;
    use pocketmine\nbt\tag\Tag;
    use pocketmine\runtime\Bridge;

    /**
     * PM `BlockStateSerializer`: block -> Bedrock state.
     *
     * Plugin block classes register `map($prototype, fn(Block): BlockStateWriter)`;
     * everything else falls back to the Rust palette (`block.state`), so any
     * block the server knows can be serialized without a PHP class for it.
     */
    final class BlockStateSerializer
    {
        /** @var array<int, \Closure> block state id => writer */
        private array $mappings = [];

        public function map(Block $block, \Closure $writer): void
        {
            $this->mappings[$block->getStateId()] = $writer;
        }

        public function serialize(Block $block): BlockStateData
        {
            $mapping = $this->mappings[$block->getStateId()] ?? null;
            if ($mapping !== null) {
                $result = $mapping($block);
                if ($result instanceof BlockStateWriter) {
                    return $result->getBlockStateData();
                }
                if ($result instanceof BlockStateData) {
                    return $result;
                }
                throw new BlockStateSerializeException(
                    'Serializer for ' . $block->getName() . ' returned ' . get_debug_type($result)
                );
            }

            return self::stateFromPalette($block);
        }

        /** Alias used by some plugins/PM versions. */
        public function serializeBlock(Block $block): BlockStateData
        {
            return $this->serialize($block);
        }

        private static function stateFromPalette(Block $block): BlockStateData
        {
            if (class_exists(Bridge::class) && Bridge::isActive()) {
                $result = Bridge::hostCall('block.state', ['id' => $block->getStateId()]);
                if (is_array($result) && isset($result['name'])) {
                    $states = [];
                    foreach (is_array($result['properties'] ?? null) ? $result['properties'] : [] as $key => $value) {
                        $states[(string)$key] = is_int($value) ? new IntTag($value) : new StringTag((string)$value);
                    }
                    return BlockStateData::current((string)$result['name'], $states);
                }
                throw new BlockStateSerializeException(
                    'Block state ' . $block->getStateId() . ' is not in the Bedrock palette'
                );
            }
            // Sin puente (tests): nombre de la tabla legacy, sin propiedades.
            return BlockStateData::current($block->getTypeName(), []);
        }
    }

    /**
     * PM `BlockStateDeserializer`: Bedrock state -> block.
     *
     * Unmapped names resolve through Rust, which keeps the palette in one
     * place; an unknown name becomes an `UnknownBlock` like on PM.
     */
    final class BlockStateDeserializer
    {
        /** @var array<string, \Closure> */
        private array $mappings = [];

        public function map(string $name, \Closure $deserializer): void
        {
            $this->mappings[$name] = $deserializer;
        }

        public function deserialize(BlockStateData $data): Block
        {
            $mapping = $this->mappings[$data->getName()] ?? null;
            if ($mapping !== null) {
                $block = $mapping($data);
                if (!$block instanceof Block) {
                    throw new BlockStateDeserializeException(
                        'Deserializer for ' . $data->getName() . ' returned ' . get_debug_type($block)
                    );
                }
                return $block;
            }

            if (class_exists(Bridge::class) && Bridge::isActive()) {
                $result = Bridge::hostCall('block.network_id', [
                    'name' => $data->getName(),
                    'states' => self::encodeStates($data->getStates()),
                ]);
                if (is_array($result) && isset($result['id'])) {
                    return Block::fromStateId((int)$result['id']);
                }
            } else {
                $id = Block::resolveStateId($data->getName());
                if ($id !== Block::AIR_STATE_ID || $data->getName() === 'minecraft:air') {
                    return Block::fromStateId($id);
                }
            }

            return new UnknownBlock(
                new BlockIdentifier(BlockTypeIds::forName($data->getName())),
                $data->getName(),
                new BlockTypeInfo(BlockBreakInfo::instant())
            );
        }

        /**
         * The palette indexes properties by their NBT/string form, but they
         * reach us as tags: `TAG_String` maps to its value and the numeric
         * tags (`TAG_Byte` from `writeBool()`, `TAG_Int`, ...) to the decimal
         * string the palette uses. Anything else (compounds, lists) is not a
         * block property and is dropped.
         *
         * @param array<string, Tag> $states
         * @return array<string, string>
         */
        private static function encodeStates(array $states): array
        {
            $encoded = [];
            foreach ($states as $key => $tag) {
                if ($tag instanceof CompoundTag) {
                    continue;
                }
                $value = $tag->getValue();
                if ($value === null || is_array($value)) {
                    continue;
                }
                $encoded[(string)$key] = is_bool($value) ? ($value ? 'true' : 'false') : (string)$value;
            }
            return $encoded;
        }
    }
}

namespace pocketmine\world\format\io {

    use pocketmine\data\bedrock\block\BlockStateData;
    use pocketmine\data\bedrock\block\convert\BlockStateDeserializer;
    use pocketmine\data\bedrock\block\convert\BlockStateSerializer;

    /**
     * PM `GlobalBlockStateHandlers`: the process-wide serializer/deserializer
     * pair block classes and plugins register against.
     */
    final class GlobalBlockStateHandlers
    {
        private static ?BlockStateSerializer $serializer = null;
        private static ?BlockStateDeserializer $deserializer = null;

        public static function getSerializer(): BlockStateSerializer
        {
            return self::$serializer ??= new BlockStateSerializer();
        }

        public static function getDeserializer(): BlockStateDeserializer
        {
            return self::$deserializer ??= new BlockStateDeserializer();
        }

        /** State PM uses as the stand-in for blocks outside the palette. */
        public static function getUnknownBlockStateData(): BlockStateData
        {
            return BlockStateData::current('minecraft:unknown', []);
        }
    }
}
