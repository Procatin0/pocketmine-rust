<?php

declare(strict_types=1);

namespace pocketmine\utils {

    /**
     * Thrown when a buffer is too short or a varint is malformed.
     */
    class BinaryDataException extends \RuntimeException
    {
    }

    /**
     * PM `Binary`: big/little-endian helpers and varint codecs.
     *
     * Ported from PocketMine-MP 5 `src/utils/Binary.php`; every method keeps the
     * PM signature `(string $str, int $offset, ?int &$len = null)` so plugin code
     * (`Binary::readInt($buffer, $offset)`, `Binary::signInt(...)`, ...) works
     * unchanged.
     */
    final class Binary
    {
        private function __construct()
        {
        }

        // ---- sign helpers -------------------------------------------------

        public static function signByte(int $value): int
        {
            return $value << 56 >> 56;
        }

        public static function unsignByte(int $value): int
        {
            return $value & 0xff;
        }

        public static function signShort(int $value): int
        {
            return $value << 48 >> 48;
        }

        public static function unsignShort(int $value): int
        {
            return $value & 0xffff;
        }

        public static function signInt(int $value): int
        {
            return $value << 32 >> 32;
        }

        public static function unsignInt(int $value): int
        {
            return $value & 0xffffffff;
        }

        public static function flipShortEndianness(int $value): int
        {
            return self::readLShort(self::writeShort($value));
        }

        public static function flipIntEndianness(int $value): int
        {
            return self::readLInt(self::writeInt($value));
        }

        public static function flipLongEndianness(int $value): int
        {
            return self::readLLong(self::writeLong($value));
        }

        private static function checkRead(string $str, int $offset, int $length): void
        {
            if ($offset < 0 || $length < 0 || strlen($str) < $offset + $length) {
                throw new BinaryDataException('Expected ' . $length . ' bytes at offset ' . $offset . ', got ' . max(0, strlen($str) - $offset));
            }
        }

        // ---- fixed-width reads -------------------------------------------

        public static function readByte(string $str, int $offset, ?int &$len = null): int
        {
            self::checkRead($str, $offset, 1);
            $len = 1;
            return ord($str[$offset]);
        }

        public static function readSignedByte(string $str, int $offset, ?int &$len = null): int
        {
            return self::signByte(self::readByte($str, $offset, $len));
        }

        public static function readShort(string $str, int $offset, ?int &$len = null): int
        {
            self::checkRead($str, $offset, 2);
            $len = 2;
            return unpack('n', $str, $offset)[1];
        }

        public static function readLShort(string $str, int $offset, ?int &$len = null): int
        {
            self::checkRead($str, $offset, 2);
            $len = 2;
            return unpack('v', $str, $offset)[1];
        }

        public static function readSignedShort(string $str, int $offset, ?int &$len = null): int
        {
            return self::signShort(self::readShort($str, $offset, $len));
        }

        public static function readSignedLShort(string $str, int $offset, ?int &$len = null): int
        {
            return self::signShort(self::readLShort($str, $offset, $len));
        }

        public static function readTriad(string $str, int $offset, ?int &$len = null): int
        {
            self::checkRead($str, $offset, 3);
            $len = 3;
            return unpack('N', "\x00" . substr($str, $offset, 3))[1];
        }

        public static function readLTriad(string $str, int $offset, ?int &$len = null): int
        {
            self::checkRead($str, $offset, 3);
            $len = 3;
            return unpack('V', substr($str, $offset, 3) . "\x00")[1];
        }

        public static function readInt(string $str, int $offset, ?int &$len = null): int
        {
            self::checkRead($str, $offset, 4);
            $len = 4;
            return unpack('N', $str, $offset)[1];
        }

        public static function readLInt(string $str, int $offset, ?int &$len = null): int
        {
            self::checkRead($str, $offset, 4);
            $len = 4;
            return unpack('V', $str, $offset)[1];
        }

        public static function readFloat(string $str, int $offset, ?int &$len = null): float
        {
            self::checkRead($str, $offset, 4);
            $len = 4;
            return unpack('G', $str, $offset)[1];
        }

        public static function readLFloat(string $str, int $offset, ?int &$len = null): float
        {
            self::checkRead($str, $offset, 4);
            $len = 4;
            return unpack('g', $str, $offset)[1];
        }

        public static function readDouble(string $str, int $offset, ?int &$len = null): float
        {
            self::checkRead($str, $offset, 8);
            $len = 8;
            return unpack('E', $str, $offset)[1];
        }

        public static function readLDouble(string $str, int $offset, ?int &$len = null): float
        {
            self::checkRead($str, $offset, 8);
            $len = 8;
            return unpack('e', $str, $offset)[1];
        }

        public static function readLong(string $str, int $offset, ?int &$len = null): int
        {
            self::checkRead($str, $offset, 8);
            $len = 8;
            return unpack('J', $str, $offset)[1];
        }

        public static function readLLong(string $str, int $offset, ?int &$len = null): int
        {
            self::checkRead($str, $offset, 8);
            $len = 8;
            return unpack('P', $str, $offset)[1];
        }

        // ---- varints ------------------------------------------------------

        public static function readUnsignedVarInt(string $buffer, int $offset, ?int &$len = null): int
        {
            $value = 0;
            $iterator = 0;
            do {
                if ($iterator > 9) {
                    throw new BinaryDataException('VarInt is too big');
                }
                if (!isset($buffer[$offset + $iterator])) {
                    throw new BinaryDataException('Unexpected end of buffer while reading VarInt');
                }
                $byte = ord($buffer[$offset + $iterator]);
                $value |= (($byte & 0x7f) << ($iterator * 7));
                $iterator++;
            } while (($byte & 0x80) !== 0);
            $len = $iterator;
            return $value;
        }

        public static function readVarInt(string $buffer, int $offset, ?int &$len = null): int
        {
            $value = self::readUnsignedVarInt($buffer, $offset, $len);
            return ($value >> 1) ^ (-($value & 1));
        }

        public static function readUnsignedVarLong(string $buffer, int $offset, ?int &$len = null): int
        {
            $value = 0;
            $iterator = 0;
            do {
                if ($iterator > 9) {
                    throw new BinaryDataException('VarLong is too big');
                }
                if (!isset($buffer[$offset + $iterator])) {
                    throw new BinaryDataException('Unexpected end of buffer while reading VarLong');
                }
                $byte = ord($buffer[$offset + $iterator]);
                $value |= (($byte & 0x7f) << ($iterator * 7));
                $iterator++;
            } while (($byte & 0x80) !== 0);
            $len = $iterator;
            return $value;
        }

        public static function readVarLong(string $buffer, int $offset, ?int &$len = null): int
        {
            $value = self::readUnsignedVarLong($buffer, $offset, $len);
            return ($value >> 1) ^ (-($value & 1));
        }

        // ---- fixed-width writes -------------------------------------------

        public static function writeByte(int $value): string
        {
            return chr($value);
        }

        public static function writeSignedByte(int $value): string
        {
            return chr($value);
        }

        public static function writeShort(int $value): string
        {
            return pack('n', $value);
        }

        public static function writeLShort(int $value): string
        {
            return pack('v', $value);
        }

        public static function writeTriad(int $value): string
        {
            return substr(pack('N', $value), 1, 3);
        }

        public static function writeLTriad(int $value): string
        {
            return substr(pack('V', $value), 0, 3);
        }

        public static function writeInt(int $value): string
        {
            return pack('N', $value);
        }

        public static function writeLInt(int $value): string
        {
            return pack('V', $value);
        }

        public static function writeFloat(float $value): string
        {
            return pack('G', $value);
        }

        public static function writeLFloat(float $value): string
        {
            return pack('g', $value);
        }

        public static function writeDouble(float $value): string
        {
            return pack('E', $value);
        }

        public static function writeLDouble(float $value): string
        {
            return pack('e', $value);
        }

        public static function writeLong(int $value): string
        {
            return pack('J', $value);
        }

        public static function writeLLong(int $value): string
        {
            return pack('P', $value);
        }

        // ---- varint writes ------------------------------------------------

        public static function writeUnsignedVarInt(int $value): string
        {
            $buffer = '';
            for ($i = 0; $i < 10; ++$i) {
                if (($value >> 7) !== 0) {
                    $buffer .= chr($value | 0x80);
                } else {
                    $buffer .= chr($value);
                    return $buffer;
                }
                $value = (($value >> 7) & (PHP_INT_MAX >> 6));
            }
            throw new \InvalidArgumentException('Value is too large to be encoded as a VarInt');
        }

        public static function writeVarInt(int $value): string
        {
            return self::writeUnsignedVarInt(($value << 1) ^ ($value >> 63));
        }

        public static function writeUnsignedVarLong(int $value): string
        {
            $buffer = '';
            for ($i = 0; $i < 10; ++$i) {
                if (($value >> 7) !== 0) {
                    $buffer .= chr($value | 0x80);
                } else {
                    $buffer .= chr($value);
                    return $buffer;
                }
                $value = (($value >> 7) & (PHP_INT_MAX >> 6));
            }
            throw new \InvalidArgumentException('Value is too large to be encoded as a VarLong');
        }

        public static function writeVarLong(int $value): string
        {
            return self::writeUnsignedVarLong(($value << 1) ^ ($value >> 63));
        }

        // ---- strings ------------------------------------------------------

        public static function readString(string $str, int $offset, ?int &$len = null): string
        {
            $prefixLength = 0;
            $stringLength = self::readUnsignedVarInt($str, $offset, $prefixLength);
            self::checkRead($str, $offset + $prefixLength, $stringLength);
            $len = $prefixLength + $stringLength;
            return substr($str, $offset + $prefixLength, $stringLength);
        }

        public static function readShortString(string $str, int $offset, ?int &$len = null): string
        {
            $stringLength = self::readShort($str, $offset, $read);
            self::checkRead($str, $offset + $read, $stringLength);
            $len = $read + $stringLength;
            return substr($str, $offset + $read, $stringLength);
        }

        public static function readLongString(string $str, int $offset, ?int &$len = null): string
        {
            $stringLength = self::readInt($str, $offset, $read);
            self::checkRead($str, $offset + $read, $stringLength);
            $len = $read + $stringLength;
            return substr($str, $offset + $read, $stringLength);
        }

        public static function writeString(string $value): string
        {
            return self::writeUnsignedVarInt(strlen($value)) . $value;
        }

        public static function writeShortString(string $value): string
        {
            return self::writeShort(strlen($value)) . $value;
        }

        public static function writeLongString(string $value): string
        {
            return self::writeInt(strlen($value)) . $value;
        }

        // ---- misc ---------------------------------------------------------

        public static function readUUID(string $str, int $offset, ?int &$len = null): string
        {
            self::checkRead($str, $offset, 16);
            $len = 16;
            return substr($str, $offset, 16);
        }

        public static function writeUUID(string $uuidBytes): string
        {
            if (strlen($uuidBytes) !== 16) {
                throw new \InvalidArgumentException('UUID must be 16 bytes, got ' . strlen($uuidBytes));
            }
            return $uuidBytes;
        }

        public static function readRoundedFloat(string $str, int $offset, int $precision, ?int &$len = null): float
        {
            return round(self::readFloat($str, $offset, $len), $precision);
        }

        public static function readRoundedLFloat(string $str, int $offset, int $precision, ?int &$len = null): float
        {
            return round(self::readLFloat($str, $offset, $len), $precision);
        }

        public static function writeRoundedFloat(float $value, int $precision): string
        {
            return self::writeFloat(round($value, $precision));
        }

        public static function writeRoundedLFloat(float $value, int $precision): string
        {
            return self::writeLFloat(round($value, $precision));
        }
    }

    /**
     * PM `BinaryStream`: a growable buffer with the same read/write surface as
     * PocketMine-MP 5. `get()`/`put()` move the internal offset; `getBuffer()`
     * returns everything written so far (the pattern EasyEdit and NBT use).
     */
    class BinaryStream
    {
        protected int $offset = 0;
        protected string $buffer = '';

        public function __construct(string $buffer = '', int $offset = 0)
        {
            $this->setBuffer($buffer, $offset);
        }

        public function getOffset(): int
        {
            return $this->offset;
        }

        public function getBuffer(): string
        {
            return $this->buffer;
        }

        public function setOffset(int $offset): void
        {
            $this->offset = $offset;
        }

        public function setBuffer(string $buffer = '', int $offset = 0): void
        {
            $this->buffer = $buffer;
            $this->offset = $offset;
        }

        public function reset(): void
        {
            $this->buffer = '';
            $this->offset = 0;
        }

        public function getRemaining(): string
        {
            return substr($this->buffer, $this->offset);
        }

        public function feof(): bool
        {
            return !isset($this->buffer[$this->offset]);
        }

        public function skip(int $len): void
        {
            $this->offset += $len;
        }

        public function get(int $len): string
        {
            if ($len < 0) {
                throw new \InvalidArgumentException('Length must be positive, got ' . $len);
            }
            if ($len === 0) {
                return '';
            }
            $result = substr($this->buffer, $this->offset, $len);
            if (strlen($result) !== $len) {
                throw new BinaryDataException('Not enough bytes left in the buffer (wanted ' . $len . ', got ' . strlen($result) . ')');
            }
            $this->offset += $len;
            return $result;
        }

        public function put(string $str): void
        {
            $this->buffer .= $str;
        }

        public function getByte(): int
        {
            return ord($this->get(1));
        }

        public function getSignedByte(): int
        {
            return Binary::signByte($this->getByte());
        }

        public function putByte(int $value): void
        {
            $this->buffer .= chr($value);
        }

        public function putSignedByte(int $value): void
        {
            $this->buffer .= chr($value);
        }

        public function getBool(): bool
        {
            return $this->getByte() !== 0;
        }

        public function putBool(bool $value): void
        {
            $this->buffer .= chr($value ? 1 : 0);
        }

        public function getShort(): int
        {
            return Binary::readShort($this->get(2), 0);
        }

        public function getSignedShort(): int
        {
            return Binary::signShort($this->getShort());
        }

        public function getLShort(): int
        {
            return Binary::readLShort($this->get(2), 0);
        }

        public function getSignedLShort(): int
        {
            return Binary::signShort($this->getLShort());
        }

        public function putShort(int $value): void
        {
            $this->buffer .= Binary::writeShort($value);
        }

        public function putLShort(int $value): void
        {
            $this->buffer .= Binary::writeLShort($value);
        }

        public function getTriad(): int
        {
            return Binary::readTriad($this->get(3), 0);
        }

        public function getLTriad(): int
        {
            return Binary::readLTriad($this->get(3), 0);
        }

        public function putTriad(int $value): void
        {
            $this->buffer .= Binary::writeTriad($value);
        }

        public function putLTriad(int $value): void
        {
            $this->buffer .= Binary::writeLTriad($value);
        }

        public function getInt(): int
        {
            return Binary::readInt($this->get(4), 0);
        }

        public function getLInt(): int
        {
            return Binary::readLInt($this->get(4), 0);
        }

        public function putInt(int $value): void
        {
            $this->buffer .= Binary::writeInt($value);
        }

        public function putLInt(int $value): void
        {
            $this->buffer .= Binary::writeLInt($value);
        }

        public function getFloat(): float
        {
            return Binary::readFloat($this->get(4), 0);
        }

        public function getLFloat(): float
        {
            return Binary::readLFloat($this->get(4), 0);
        }

        public function putFloat(float $value): void
        {
            $this->buffer .= Binary::writeFloat($value);
        }

        public function putLFloat(float $value): void
        {
            $this->buffer .= Binary::writeLFloat($value);
        }

        public function getDouble(): float
        {
            return Binary::readDouble($this->get(8), 0);
        }

        public function getLDouble(): float
        {
            return Binary::readLDouble($this->get(8), 0);
        }

        public function putDouble(float $value): void
        {
            $this->buffer .= Binary::writeDouble($value);
        }

        public function putLDouble(float $value): void
        {
            $this->buffer .= Binary::writeLDouble($value);
        }

        public function getLong(): int
        {
            return Binary::readLong($this->get(8), 0);
        }

        public function getLLong(): int
        {
            return Binary::readLLong($this->get(8), 0);
        }

        public function putLong(int $value): void
        {
            $this->buffer .= Binary::writeLong($value);
        }

        public function putLLong(int $value): void
        {
            $this->buffer .= Binary::writeLLong($value);
        }

        public function getUnsignedVarInt(): int
        {
            $value = Binary::readUnsignedVarInt($this->buffer, $this->offset, $read);
            $this->offset += $read;
            return $value;
        }

        public function getVarInt(): int
        {
            $value = Binary::readVarInt($this->buffer, $this->offset, $read);
            $this->offset += $read;
            return $value;
        }

        public function putUnsignedVarInt(int $value): void
        {
            $this->buffer .= Binary::writeUnsignedVarInt($value);
        }

        public function putVarInt(int $value): void
        {
            $this->buffer .= Binary::writeVarInt($value);
        }

        public function getUnsignedVarLong(): int
        {
            $value = Binary::readUnsignedVarLong($this->buffer, $this->offset, $read);
            $this->offset += $read;
            return $value;
        }

        public function getVarLong(): int
        {
            $value = Binary::readVarLong($this->buffer, $this->offset, $read);
            $this->offset += $read;
            return $value;
        }

        public function putUnsignedVarLong(int $value): void
        {
            $this->buffer .= Binary::writeUnsignedVarLong($value);
        }

        public function putVarLong(int $value): void
        {
            $this->buffer .= Binary::writeVarLong($value);
        }

        public function getString(): string
        {
            $value = Binary::readString($this->buffer, $this->offset, $read);
            $this->offset += $read;
            return $value;
        }

        public function getShortString(): string
        {
            $value = Binary::readShortString($this->buffer, $this->offset, $read);
            $this->offset += $read;
            return $value;
        }

        public function getLongString(): string
        {
            $value = Binary::readLongString($this->buffer, $this->offset, $read);
            $this->offset += $read;
            return $value;
        }

        public function putString(string $value): void
        {
            $this->buffer .= Binary::writeString($value);
        }

        public function putShortString(string $value): void
        {
            $this->buffer .= Binary::writeShortString($value);
        }

        public function putLongString(string $value): void
        {
            $this->buffer .= Binary::writeLongString($value);
        }

        public function getUUID(): string
        {
            return $this->get(16);
        }

        public function putUUID(string $uuidBytes): void
        {
            $this->buffer .= Binary::writeUUID($uuidBytes);
        }
    }
}
