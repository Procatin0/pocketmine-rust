<?php

declare(strict_types=1);

namespace pocketmine\nbt {

    class NbtDataException extends \RuntimeException
    {
    }

    /**
     * PM `ReaderTracker`: enforces a maximum compound/list nesting depth while
     * reading untrusted NBT.
     */
    final class ReaderTracker
    {
        private int $depth = 0;

        public function __construct(private int $maxDepth)
        {
        }

        public function getMaxDepth(): int
        {
            return $this->maxDepth;
        }

        public function getDepth(): int
        {
            return $this->depth;
        }

        public function protectDepth(\Closure $closure): void
        {
            // PM callers pass 0 for "no limit" (EasyEdit reads whole schematics
            // with `new ReaderTracker(0)`), so only a positive depth is checked.
            if ($this->maxDepth > 0 && $this->depth >= $this->maxDepth) {
                throw new NbtDataException('NBT nesting level exceeds the configured maximum of ' . $this->maxDepth);
            }
            $this->depth++;
            try {
                $closure();
            } finally {
                $this->depth--;
            }
        }
    }

    /**
     * PM `NbtStreamReader`: the primitives a tag reader needs. Serializers
     * implement this so tag classes stay independent of endianness.
     */
    interface NbtStreamReader
    {
        public function readByte(): int;

        public function readSignedByte(): int;

        public function readShort(): int;

        public function readSignedShort(): int;

        public function readInt(): int;

        public function readLong(): int;

        public function readFloat(): float;

        public function readDouble(): float;

        public function readString(): string;

        public function readByteArray(): string;

        /** @return int[] */
        public function readIntArray(): array;

        /** @return int[] */
        public function readLongArray(): array;
    }

    /**
     * PM `NbtStreamWriter`: the primitives a tag writer needs.
     */
    interface NbtStreamWriter
    {
        public function writeByte(int $value): void;

        public function writeSignedByte(int $value): void;

        public function writeShort(int $value): void;

        public function writeSignedShort(int $value): void;

        public function writeInt(int $value): void;

        public function writeLong(int $value): void;

        public function writeFloat(float $value): void;

        public function writeDouble(float $value): void;

        public function writeString(string $value): void;

        public function writeByteArray(string $value): void;

        /** @param int[] $value */
        public function writeIntArray(array $value): void;

        /** @param int[] $value */
        public function writeLongArray(array $value): void;
    }

    /**
     * PM `NBT`: tag type ids and the type dispatcher used while reading.
     */
    class NBT
    {
        public const TAG_End = 0;
        public const TAG_Byte = 1;
        public const TAG_Short = 2;
        public const TAG_Int = 3;
        public const TAG_Long = 4;
        public const TAG_Float = 5;
        public const TAG_Double = 6;
        public const TAG_ByteArray = 7;
        public const TAG_String = 8;
        public const TAG_List = 9;
        public const TAG_Compound = 10;
        public const TAG_IntArray = 11;
        public const TAG_LongArray = 12;

        protected function __construct()
        {
        }

        /**
         * Creates an empty tag of the given type whose payload is read from
         * `$reader`. Mirrors PM's dispatcher, so plugin subclasses can
         * intercept specific types and fall back to the parent for the rest.
         */
        public static function createTag(int $type, NbtStreamReader $reader, ReaderTracker $tracker): \pocketmine\nbt\tag\Tag
        {
            return match ($type) {
                self::TAG_End => new \pocketmine\nbt\tag\EndTag(),
                self::TAG_Byte => \pocketmine\nbt\tag\ByteTag::read($reader),
                self::TAG_Short => \pocketmine\nbt\tag\ShortTag::read($reader),
                self::TAG_Int => \pocketmine\nbt\tag\IntTag::read($reader),
                self::TAG_Long => \pocketmine\nbt\tag\LongTag::read($reader),
                self::TAG_Float => \pocketmine\nbt\tag\FloatTag::read($reader),
                self::TAG_Double => \pocketmine\nbt\tag\DoubleTag::read($reader),
                self::TAG_ByteArray => \pocketmine\nbt\tag\ByteArrayTag::read($reader),
                self::TAG_String => \pocketmine\nbt\tag\StringTag::read($reader),
                self::TAG_List => \pocketmine\nbt\tag\ListTag::read($reader, $tracker),
                self::TAG_Compound => \pocketmine\nbt\tag\CompoundTag::read($reader, $tracker),
                self::TAG_IntArray => \pocketmine\nbt\tag\IntArrayTag::read($reader),
                self::TAG_LongArray => \pocketmine\nbt\tag\LongArrayTag::read($reader),
                default => throw new NbtDataException('Unknown NBT tag type ' . $type),
            };
        }
    }

    /**
     * PM `TreeRoot`: a named root tag, i.e. one whole NBT document.
     */
    final class TreeRoot
    {
        public function __construct(private \pocketmine\nbt\tag\Tag $tag, private string $name = '')
        {
        }

        public function getTag(): \pocketmine\nbt\tag\Tag
        {
            return $this->tag;
        }

        public function getName(): string
        {
            return $this->name;
        }

        public function setName(string $name): void
        {
            $this->name = $name;
        }
    }
}

namespace pocketmine\nbt\tag {

    use pocketmine\nbt\NBT;
    use pocketmine\nbt\NbtDataException;
    use pocketmine\nbt\NbtStreamReader;
    use pocketmine\nbt\NbtStreamWriter;
    use pocketmine\nbt\ReaderTracker;

    /**
     * PM `Tag`: base class of every NBT value. Plugin subclasses (EasyEdit's
     * lazy list/byte-array tags) override the value accessors and reuse the
     * stringification hooks, so those stay protected and non-final.
     */
    abstract class Tag
    {
        abstract public function getValue();

        abstract public function getType(): int;

        abstract public function write(NbtStreamWriter $writer): void;

        abstract protected function getTypeName(): string;

        abstract protected function stringifyValue(int $indentation): string;

        abstract protected function makeCopy(): Tag;

        public function __toString(): string
        {
            return $this->toString();
        }

        public function toString(int $indentation = 0): string
        {
            return $this->getTypeName() . '(' . $this->stringifyValue($indentation) . ')';
        }

        public function equals(Tag $that): bool
        {
            return $that instanceof static && $this->getValue() === $that->getValue();
        }

        public function __clone()
        {
        }
    }

    final class EndTag extends Tag
    {
        public function getValue(): null
        {
            return null;
        }

        public function getType(): int
        {
            return NBT::TAG_End;
        }

        public function write(NbtStreamWriter $writer): void
        {
        }

        protected function getTypeName(): string
        {
            return 'End';
        }

        protected function stringifyValue(int $indentation): string
        {
            return '';
        }

        protected function makeCopy(): Tag
        {
            return clone $this;
        }
    }

    final class ByteTag extends Tag
    {
        public function __construct(private int $value)
        {
            $value = $value << 56 >> 56;
            $this->value = $value;
        }

        public static function read(NbtStreamReader $reader, ?ReaderTracker $tracker = null): self
        {
            return new self($reader->readSignedByte());
        }

        public function getValue(): int
        {
            return $this->value;
        }

        public function getType(): int
        {
            return NBT::TAG_Byte;
        }

        public function write(NbtStreamWriter $writer): void
        {
            $writer->writeSignedByte($this->value);
        }

        protected function getTypeName(): string
        {
            return 'Byte';
        }

        protected function stringifyValue(int $indentation): string
        {
            return (string)$this->value;
        }

        protected function makeCopy(): Tag
        {
            return clone $this;
        }
    }

    final class ShortTag extends Tag
    {
        public function __construct(private int $value)
        {
            $value = $value << 48 >> 48;
            $this->value = $value;
        }

        public static function read(NbtStreamReader $reader, ?ReaderTracker $tracker = null): self
        {
            return new self($reader->readSignedShort());
        }

        public function getValue(): int
        {
            return $this->value;
        }

        public function getType(): int
        {
            return NBT::TAG_Short;
        }

        public function write(NbtStreamWriter $writer): void
        {
            $writer->writeSignedShort($this->value);
        }

        protected function getTypeName(): string
        {
            return 'Short';
        }

        protected function stringifyValue(int $indentation): string
        {
            return (string)$this->value;
        }

        protected function makeCopy(): Tag
        {
            return clone $this;
        }
    }

    final class IntTag extends Tag
    {
        public function __construct(private int $value)
        {
            $value = $value << 32 >> 32;
            $this->value = $value;
        }

        public static function read(NbtStreamReader $reader, ?ReaderTracker $tracker = null): self
        {
            return new self($reader->readInt());
        }

        public function getValue(): int
        {
            return $this->value;
        }

        public function getType(): int
        {
            return NBT::TAG_Int;
        }

        public function write(NbtStreamWriter $writer): void
        {
            $writer->writeInt($this->value);
        }

        protected function getTypeName(): string
        {
            return 'Int';
        }

        protected function stringifyValue(int $indentation): string
        {
            return (string)$this->value;
        }

        protected function makeCopy(): Tag
        {
            return clone $this;
        }
    }

    final class LongTag extends Tag
    {
        public function __construct(private int $value)
        {
        }

        public static function read(NbtStreamReader $reader, ?ReaderTracker $tracker = null): self
        {
            return new self($reader->readLong());
        }

        public function getValue(): int
        {
            return $this->value;
        }

        public function getType(): int
        {
            return NBT::TAG_Long;
        }

        public function write(NbtStreamWriter $writer): void
        {
            $writer->writeLong($this->value);
        }

        protected function getTypeName(): string
        {
            return 'Long';
        }

        protected function stringifyValue(int $indentation): string
        {
            return (string)$this->value;
        }

        protected function makeCopy(): Tag
        {
            return clone $this;
        }
    }

    final class FloatTag extends Tag
    {
        public function __construct(private float $value)
        {
        }

        public static function read(NbtStreamReader $reader, ?ReaderTracker $tracker = null): self
        {
            return new self($reader->readFloat());
        }

        public function getValue(): float
        {
            return $this->value;
        }

        public function getType(): int
        {
            return NBT::TAG_Float;
        }

        public function write(NbtStreamWriter $writer): void
        {
            $writer->writeFloat($this->value);
        }

        protected function getTypeName(): string
        {
            return 'Float';
        }

        protected function stringifyValue(int $indentation): string
        {
            return (string)$this->value;
        }

        protected function makeCopy(): Tag
        {
            return clone $this;
        }
    }

    final class DoubleTag extends Tag
    {
        public function __construct(private float $value)
        {
        }

        public static function read(NbtStreamReader $reader, ?ReaderTracker $tracker = null): self
        {
            return new self($reader->readDouble());
        }

        public function getValue(): float
        {
            return $this->value;
        }

        public function getType(): int
        {
            return NBT::TAG_Double;
        }

        public function write(NbtStreamWriter $writer): void
        {
            $writer->writeDouble($this->value);
        }

        protected function getTypeName(): string
        {
            return 'Double';
        }

        protected function stringifyValue(int $indentation): string
        {
            return (string)$this->value;
        }

        protected function makeCopy(): Tag
        {
            return clone $this;
        }
    }

    final class StringTag extends Tag
    {
        public function __construct(private string $value)
        {
        }

        public static function read(NbtStreamReader $reader, ?ReaderTracker $tracker = null): self
        {
            return new self($reader->readString());
        }

        public function getValue(): string
        {
            return $this->value;
        }

        public function getType(): int
        {
            return NBT::TAG_String;
        }

        public function write(NbtStreamWriter $writer): void
        {
            $writer->writeString($this->value);
        }

        protected function getTypeName(): string
        {
            return 'String';
        }

        protected function stringifyValue(int $indentation): string
        {
            return $this->value;
        }

        protected function makeCopy(): Tag
        {
            return clone $this;
        }
    }

    final class ByteArrayTag extends Tag
    {
        public function __construct(private string $value)
        {
        }

        public static function read(NbtStreamReader $reader, ?ReaderTracker $tracker = null): self
        {
            return new self($reader->readByteArray());
        }

        public function getValue(): string
        {
            return $this->value;
        }

        public function getType(): int
        {
            return NBT::TAG_ByteArray;
        }

        public function write(NbtStreamWriter $writer): void
        {
            $writer->writeByteArray($this->value);
        }

        protected function getTypeName(): string
        {
            return 'ByteArray';
        }

        protected function stringifyValue(int $indentation): string
        {
            return 'len=' . strlen($this->value);
        }

        protected function makeCopy(): Tag
        {
            return clone $this;
        }
    }

    final class IntArrayTag extends Tag
    {
        /** @param int[] $value */
        public function __construct(private array $value)
        {
        }

        public static function read(NbtStreamReader $reader, ?ReaderTracker $tracker = null): self
        {
            return new self($reader->readIntArray());
        }

        /** @return int[] */
        public function getValue(): array
        {
            return $this->value;
        }

        public function getType(): int
        {
            return NBT::TAG_IntArray;
        }

        public function write(NbtStreamWriter $writer): void
        {
            $writer->writeIntArray($this->value);
        }

        protected function getTypeName(): string
        {
            return 'IntArray';
        }

        protected function stringifyValue(int $indentation): string
        {
            return 'len=' . count($this->value);
        }

        protected function makeCopy(): Tag
        {
            return clone $this;
        }
    }

    final class LongArrayTag extends Tag
    {
        /** @param int[] $value */
        public function __construct(private array $value)
        {
        }

        public static function read(NbtStreamReader $reader, ?ReaderTracker $tracker = null): self
        {
            return new self($reader->readLongArray());
        }

        /** @return int[] */
        public function getValue(): array
        {
            return $this->value;
        }

        public function getType(): int
        {
            return NBT::TAG_LongArray;
        }

        public function write(NbtStreamWriter $writer): void
        {
            $writer->writeLongArray($this->value);
        }

        protected function getTypeName(): string
        {
            return 'LongArray';
        }

        protected function stringifyValue(int $indentation): string
        {
            return 'len=' . count($this->value);
        }

        protected function makeCopy(): Tag
        {
            return clone $this;
        }
    }

    /**
     * PM `ListTag`: homogeneous list of tags. The stored type is taken from the
     * first element when not given explicitly.
     */
    final class ListTag extends Tag implements \Countable, \IteratorAggregate
    {
        /** @var Tag[] */
        private array $value = [];
        private int $type;

        /** @param Tag[] $value */
        public function __construct(array $value = [], int $type = NBT::TAG_End)
        {
            $this->type = $type;
            foreach ($value as $tag) {
                $this->push($tag);
            }
        }

        public static function read(NbtStreamReader $reader, ?ReaderTracker $tracker = null): self
        {
            $tracker ??= new ReaderTracker(0);
            $type = $reader->readByte();
            $count = $reader->readInt();
            if ($count < 0) {
                throw new NbtDataException('Negative list length ' . $count);
            }
            $result = new self([], $type);
            $tracker->protectDepth(static function () use ($reader, $tracker, $result, $type, $count): void {
                for ($i = 0; $i < $count; $i++) {
                    $result->value[] = NBT::createTag($type, $reader, $tracker);
                }
            });
            return $result;
        }

        /** @return Tag[] */
        public function getValue(): array
        {
            return $this->value;
        }

        /** @return Tag[] */
        public function getAllValues(): array
        {
            return $this->value;
        }

        public function getType(): int
        {
            return NBT::TAG_List;
        }

        /** Type of the contained elements (PM `getTagType()`). */
        public function getTagType(): int
        {
            return $this->type;
        }

        public function getCount(): int
        {
            return count($this->value);
        }

        public function count(): int
        {
            return count($this->value);
        }

        public function get(int $index): Tag
        {
            if (!isset($this->value[$index])) {
                throw new \OutOfBoundsException('No tag at index ' . $index);
            }
            return $this->value[$index];
        }

        public function has(int $index): bool
        {
            return isset($this->value[$index]);
        }

        public function set(int $index, Tag $tag): void
        {
            if (!isset($this->value[$index])) {
                throw new \OutOfBoundsException('No tag at index ' . $index);
            }
            $this->checkType($tag);
            $this->value[$index] = $tag;
        }

        public function push(Tag ...$tags): void
        {
            foreach ($tags as $tag) {
                $this->checkType($tag);
                $this->value[] = $tag;
            }
        }

        public function pop(): Tag
        {
            $tag = array_pop($this->value);
            if ($tag === null) {
                throw new \UnderflowException('List is empty');
            }
            return $tag;
        }

        public function remove(int $index): void
        {
            if (!isset($this->value[$index])) {
                throw new \OutOfBoundsException('No tag at index ' . $index);
            }
            array_splice($this->value, $index, 1);
            if (count($this->value) === 0) {
                $this->type = NBT::TAG_End;
            }
        }

        public function __clone()
        {
            foreach ($this->value as $index => $tag) {
                $this->value[$index] = clone $tag;
            }
        }

        private function checkType(Tag $tag): void
        {
            if ($this->type === NBT::TAG_End) {
                $this->type = $tag->getType();
                return;
            }
            if ($tag->getType() !== $this->type) {
                throw new \InvalidArgumentException('List tag type mismatch: expected ' . $this->type . ', got ' . $tag->getType());
            }
        }

        public function write(NbtStreamWriter $writer): void
        {
            $writer->writeByte($this->type);
            $writer->writeInt(count($this->value));
            foreach ($this->value as $tag) {
                $tag->write($writer);
            }
        }

        public function getIterator(): \ArrayIterator
        {
            return new \ArrayIterator($this->value);
        }

        protected function getTypeName(): string
        {
            return 'List';
        }

        protected function stringifyValue(int $indentation): string
        {
            $suffix = $indentation > 0 ? "\n" . str_repeat('  ', $indentation) : '';
            return 'type=' . $this->type . ', len=' . count($this->value) . $suffix;
        }

        protected function makeCopy(): Tag
        {
            $copy = new self([], $this->type);
            foreach ($this->value as $tag) {
                $copy->value[] = $tag->makeCopy();
            }
            return $copy;
        }
    }

    /**
     * PM `CompoundTag`: ordered name => tag map.
     */
    final class CompoundTag extends Tag implements \Countable, \IteratorAggregate
    {
        /** @var Tag[] */
        private array $value = [];

        public static function create(): self
        {
            return new self();
        }

        public function equals(Tag $that): bool
        {
            if (!$that instanceof CompoundTag || count($this->value) !== count($that->value)) {
                return false;
            }
            foreach ($this->value as $name => $tag) {
                $other = $that->getTag($name);
                if ($other === null || !$tag->equals($other)) {
                    return false;
                }
            }
            return true;
        }

        public static function read(NbtStreamReader $reader, ?ReaderTracker $tracker = null): self
        {
            $tracker ??= new ReaderTracker(0);
            $result = new self();
            $tracker->protectDepth(static function () use ($reader, $tracker, $result): void {
                while (($type = $reader->readByte()) !== NBT::TAG_End) {
                    $name = $reader->readString();
                    $result->value[$name] = NBT::createTag($type, $reader, $tracker);
                }
            });
            return $result;
        }

        /** @return Tag[] */
        public function getValue(): array
        {
            return $this->value;
        }

        /** @return Tag[] */
        public function getAllValues(): array
        {
            return $this->value;
        }

        public function getType(): int
        {
            return NBT::TAG_Compound;
        }

        public function count(): int
        {
            return count($this->value);
        }

        public function getCount(): int
        {
            return count($this->value);
        }

        public function getIterator(): \ArrayIterator
        {
            return new \ArrayIterator($this->value);
        }

        public function hasTag(string $name): bool
        {
            return isset($this->value[$name]);
        }

        public function getTag(string $name): ?Tag
        {
            return $this->value[$name] ?? null;
        }

        public function setTag(string $name, Tag $tag): self
        {
            $this->value[$name] = $tag;
            return $this;
        }

        public function removeTag(string $name): self
        {
            unset($this->value[$name]);
            return $this;
        }

        private function requireTag(string $name, int $type): Tag
        {
            $tag = $this->getTag($name);
            if ($tag === null) {
                throw new \InvalidArgumentException('Tag "' . $name . '" not found');
            }
            if ($tag->getType() !== $type) {
                throw new \InvalidArgumentException('Tag "' . $name . '" is not of type ' . $type);
            }
            return $tag;
        }

        public function getByte(string $name): int
        {
            return $this->requireTag($name, NBT::TAG_Byte)->getValue();
        }

        public function getShort(string $name): int
        {
            return $this->requireTag($name, NBT::TAG_Short)->getValue();
        }

        public function getInt(string $name): int
        {
            return $this->requireTag($name, NBT::TAG_Int)->getValue();
        }

        public function getLong(string $name): int
        {
            return $this->requireTag($name, NBT::TAG_Long)->getValue();
        }

        public function getFloat(string $name): float
        {
            return $this->requireTag($name, NBT::TAG_Float)->getValue();
        }

        public function getDouble(string $name): float
        {
            return $this->requireTag($name, NBT::TAG_Double)->getValue();
        }

        public function getString(string $name): string
        {
            return $this->requireTag($name, NBT::TAG_String)->getValue();
        }

        public function getByteArray(string $name): string
        {
            return $this->requireTag($name, NBT::TAG_ByteArray)->getValue();
        }

        /** @return int[] */
        public function getIntArray(string $name): array
        {
            return $this->requireTag($name, NBT::TAG_IntArray)->getValue();
        }

        /** @return int[] */
        public function getLongArray(string $name): array
        {
            return $this->requireTag($name, NBT::TAG_LongArray)->getValue();
        }

        public function getListTag(string $name): ListTag
        {
            /** @var ListTag $tag */
            $tag = $this->requireTag($name, NBT::TAG_List);
            return $tag;
        }

        public function getCompoundTag(string $name): CompoundTag
        {
            /** @var CompoundTag $tag */
            $tag = $this->requireTag($name, NBT::TAG_Compound);
            return $tag;
        }

        public function setByte(string $name, int $value): self
        {
            return $this->setTag($name, new ByteTag($value));
        }

        public function setShort(string $name, int $value): self
        {
            return $this->setTag($name, new ShortTag($value));
        }

        public function setInt(string $name, int $value): self
        {
            return $this->setTag($name, new IntTag($value));
        }

        public function setLong(string $name, int $value): self
        {
            return $this->setTag($name, new LongTag($value));
        }

        public function setFloat(string $name, float $value): self
        {
            return $this->setTag($name, new FloatTag($value));
        }

        public function setDouble(string $name, float $value): self
        {
            return $this->setTag($name, new DoubleTag($value));
        }

        public function setString(string $name, string $value): self
        {
            return $this->setTag($name, new StringTag($value));
        }

        public function setByteArray(string $name, string $value): self
        {
            return $this->setTag($name, new ByteArrayTag($value));
        }

        /** @param int[] $value */
        public function setIntArray(string $name, array $value): self
        {
            return $this->setTag($name, new IntArrayTag($value));
        }

        /** @param int[] $value */
        public function setLongArray(string $name, array $value): self
        {
            return $this->setTag($name, new LongArrayTag($value));
        }

        public function setCompoundTag(string $name, CompoundTag $tag): self
        {
            return $this->setTag($name, $tag);
        }

        public function write(NbtStreamWriter $writer): void
        {
            foreach ($this->value as $name => $tag) {
                $writer->writeByte($tag->getType());
                $writer->writeString((string)$name);
                $tag->write($writer);
            }
            $writer->writeByte(NBT::TAG_End);
        }

        public function __clone()
        {
            foreach ($this->value as $name => $tag) {
                $this->value[$name] = clone $tag;
            }
        }

        protected function getTypeName(): string
        {
            return 'Compound';
        }

        protected function stringifyValue(int $indentation): string
        {
            $result = '';
            foreach ($this->value as $name => $tag) {
                $result .= "\n" . str_repeat('  ', $indentation + 1) . $name . ': ' . $tag->toString($indentation + 1);
            }
            return $result;
        }

        protected function makeCopy(): Tag
        {
            $copy = new self();
            foreach ($this->value as $name => $tag) {
                $copy->value[$name] = $tag->makeCopy();
            }
            return $copy;
        }
    }
}
