<?php

declare(strict_types=1);

namespace pocketmine\nbt {

    use pmmp\encoding\ByteBufferReader;
    use pmmp\encoding\VarInt;

    /**
     * `NbtStreamWriter` over a plain string buffer.
     *
     * The native `pmmp\encoding\ByteBufferWriter` extension writes varint
     * length prefixes for `writeByteArray()`, which is not what NBT needs, so
     * the NBT writer keeps its own buffer and packs values directly.
     *
     * The endianness flag keeps every primitive in one place: big-endian NBT is
     * the vanilla disk format, little-endian is what Bedrock structures
     * (.mcstructure / .schem) use.
     */
    class NbtBufferWriter implements NbtStreamWriter
    {
        private string $data = '';

        public function __construct(protected bool $littleEndian = false)
        {
        }

        public function getData(): string
        {
            return $this->data;
        }

        protected function put(string $bytes): void
        {
            $this->data .= $bytes;
        }

        public function writeByte(int $value): void
        {
            $this->data .= chr($value);
        }

        public function writeSignedByte(int $value): void
        {
            $this->data .= chr($value);
        }

        public function writeShort(int $value): void
        {
            $this->data .= pack($this->littleEndian ? 'v' : 'n', $value);
        }

        public function writeSignedShort(int $value): void
        {
            $this->writeShort($value);
        }

        public function writeInt(int $value): void
        {
            $this->data .= pack($this->littleEndian ? 'V' : 'N', $value);
        }

        public function writeLong(int $value): void
        {
            $this->data .= pack($this->littleEndian ? 'P' : 'J', $value);
        }

        public function writeFloat(float $value): void
        {
            $this->data .= pack($this->littleEndian ? 'g' : 'G', $value);
        }

        public function writeDouble(float $value): void
        {
            $this->data .= pack($this->littleEndian ? 'e' : 'E', $value);
        }

        public function writeString(string $value): void
        {
            $this->writeShort(strlen($value));
            $this->data .= $value;
        }

        public function writeByteArray(string $value): void
        {
            $this->writeInt(strlen($value));
            $this->data .= $value;
        }

        public function writeIntArray(array $value): void
        {
            $this->writeInt(count($value));
            foreach ($value as $v) {
                $this->writeInt($v);
            }
        }

        public function writeLongArray(array $value): void
        {
            $this->writeInt(count($value));
            foreach ($value as $v) {
                $this->writeLong($v);
            }
        }
    }

    /**
     * Writer for network little endian NBT: lengths are varints, values keep
     * the little-endian payload encoding.
     */
    class NbtNetworkBufferWriter extends NbtBufferWriter
    {
        private static function varInt(int $value): string
        {
            $buffer = '';
            for ($i = 0; $i < 10; $i++) {
                if (($value >> 7) !== 0) {
                    $buffer .= chr($value | 0x80);
                } else {
                    return $buffer . chr($value);
                }
                $value = (($value >> 7) & (PHP_INT_MAX >> 6));
            }
            throw new \InvalidArgumentException('Value is too large to be encoded as a VarInt');
        }

        public function writeString(string $value): void
        {
            $this->put(self::varInt(strlen($value)));
            $this->put($value);
        }

        public function writeByteArray(string $value): void
        {
            $this->put(self::varInt(strlen($value)));
            $this->put($value);
        }

        public function writeIntArray(array $value): void
        {
            $this->put(self::varInt(count($value)));
            foreach ($value as $v) {
                $this->put(self::varInt(($v << 1) ^ ($v >> 63)));
            }
        }

        public function writeLongArray(array $value): void
        {
            $this->put(self::varInt(count($value)));
            foreach ($value as $v) {
                $this->writeLong($v);
            }
        }
    }

    /**
     * PM `BaseNbtSerializer`: reads primitives through its own `$reader` (a
     * pmmp/encoding ByteBufferReader) and exposes the whole-document API.
     *
     * Plugin serializers extend the endian subclasses and swap `$reader` to
     * decode from an in-memory file chunk, so `$reader` must stay protected and
     * the primitives stay public.
     */
    abstract class BaseNbtSerializer implements NbtStreamReader
    {
        protected ByteBufferReader $reader;

        protected bool $littleEndian = false;

        public function __construct()
        {
            $this->reader = new ByteBufferReader('');
        }

        public function read(string $buffer): TreeRoot
        {
            $this->reader = new ByteBufferReader($buffer);
            return $this->readRoot();
        }

        public function readCompressed(string $buffer): TreeRoot
        {
            $data = @gzuncompress($buffer);
            if ($data === false) {
                throw new NbtDataException('Invalid or truncated compressed NBT data');
            }
            return $this->read($data);
        }

        protected function readRoot(): TreeRoot
        {
            $type = $this->readByte();
            if ($type === NBT::TAG_End) {
                throw new NbtDataException('Found TAG_End at the start of the buffer');
            }
            $name = $this->readString();
            $tag = NBT::createTag($type, $this, new ReaderTracker(0));
            return new TreeRoot($tag, $name);
        }

        public function write(TreeRoot $data): string
        {
            $writer = $this->createWriter();
            $tag = $data->getTag();
            $writer->writeByte($tag->getType());
            $writer->writeString($data->getName());
            $tag->write($writer);
            return $writer->getData();
        }

        public function writeCompressed(TreeRoot $data): string
        {
            return gzcompress($this->write($data), 7);
        }

        protected function createWriter(): NbtBufferWriter
        {
            return new NbtBufferWriter($this->littleEndian);
        }

        protected function raw(int $length): string
        {
            return $length === 0 ? '' : $this->reader->readByteArray($length);
        }

        // ---- NbtStreamReader ---------------------------------------------

        public function readByte(): int
        {
            return ord($this->raw(1));
        }

        public function readSignedByte(): int
        {
            $value = $this->readByte();
            return $value << 56 >> 56;
        }

        public function readShort(): int
        {
            return unpack($this->littleEndian ? 'v' : 'n', $this->raw(2))[1];
        }

        public function readSignedShort(): int
        {
            $value = $this->readShort();
            return $value << 48 >> 48;
        }

        public function readInt(): int
        {
            $value = unpack($this->littleEndian ? 'V' : 'N', $this->raw(4))[1];
            return $value << 32 >> 32;
        }

        public function readLong(): int
        {
            return unpack($this->littleEndian ? 'P' : 'J', $this->raw(8))[1];
        }

        public function readFloat(): float
        {
            return unpack($this->littleEndian ? 'g' : 'G', $this->raw(4))[1];
        }

        public function readDouble(): float
        {
            return unpack($this->littleEndian ? 'e' : 'E', $this->raw(8))[1];
        }

        public function readString(): string
        {
            $length = $this->readShort();
            if ($length < 0) {
                throw new NbtDataException('Negative string length ' . $length);
            }
            return $this->raw($length);
        }

        public function readByteArray(): string
        {
            $length = $this->readInt();
            if ($length < 0) {
                throw new NbtDataException('Negative byte array length ' . $length);
            }
            return $this->raw($length);
        }

        public function readIntArray(): array
        {
            $length = $this->readInt();
            if ($length < 0) {
                throw new NbtDataException('Negative int array length ' . $length);
            }
            $result = [];
            for ($i = 0; $i < $length; $i++) {
                $result[] = $this->readInt();
            }
            return $result;
        }

        public function readLongArray(): array
        {
            $length = $this->readInt();
            if ($length < 0) {
                throw new NbtDataException('Negative long array length ' . $length);
            }
            $result = [];
            for ($i = 0; $i < $length; $i++) {
                $result[] = $this->readLong();
            }
            return $result;
        }

        public function __clone(): void
        {
            $this->reader = clone $this->reader;
        }
    }

    class BigEndianNbtSerializer extends BaseNbtSerializer
    {
    }

    class LittleEndianNbtSerializer extends BaseNbtSerializer
    {
        protected bool $littleEndian = true;
    }

    /**
     * Network little endian uses varint lengths (Bedrock network NBT).
     */
    class NetworkLittleEndianNbtSerializer extends BaseNbtSerializer
    {
        protected bool $littleEndian = true;

        public function readString(): string
        {
            return $this->raw(VarInt::readUnsignedInt($this->reader));
        }

        public function readByteArray(): string
        {
            return $this->raw(VarInt::readUnsignedInt($this->reader));
        }

        public function readIntArray(): array
        {
            $length = VarInt::readUnsignedInt($this->reader);
            $result = [];
            for ($i = 0; $i < $length; $i++) {
                $result[] = VarInt::readSignedInt($this->reader);
            }
            return $result;
        }

        public function readLongArray(): array
        {
            $length = VarInt::readUnsignedInt($this->reader);
            $result = [];
            for ($i = 0; $i < $length; $i++) {
                $result[] = $this->readLong();
            }
            return $result;
        }

        protected function createWriter(): NbtBufferWriter
        {
            return new NbtNetworkBufferWriter(true);
        }
    }
}
