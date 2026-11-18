<?php

declare(strict_types=1);

namespace pocketmine\math {

    class Vector3
    {
        public function __construct(
            public float $x = 0.0,
            public float $y = 0.0,
            public float $z = 0.0
        ) {
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

        public function getFloorX(): int
        {
            return (int)floor($this->x);
        }

        public function getFloorY(): int
        {
            return (int)floor($this->y);
        }

        public function getFloorZ(): int
        {
            return (int)floor($this->z);
        }

        public function getBlockX(): int
        {
            return $this->getFloorX();
        }

        public function getBlockY(): int
        {
            return $this->getFloorY();
        }

        public function getBlockZ(): int
        {
            return $this->getFloorZ();
        }

        public function asVector3(): Vector3
        {
            return $this;
        }

        public function setComponents(float $x, float $y = null, float $z = null): Vector3
        {
            $clone = clone $this;
            $clone->x = $x;
            if ($y !== null) {
                $clone->y = $y;
            }
            if ($z !== null) {
                $clone->z = $z;
            }
            return $clone;
        }

        public function add(float $x, float $y = null, float $z = null): Vector3
        {
            $x = (float)$x;
            $y = $y === null ? $x : (float)$y;
            $z = $z === null ? $x : (float)$z;
            return new Vector3($this->x + $x, $this->y + $y, $this->z + $z);
        }

        public function addVector(Vector3 $vector): Vector3
        {
            return new Vector3($this->x + $vector->x, $this->y + $vector->y, $this->z + $vector->z);
        }

        public function subtract(float $x, float $y = null, float $z = null): Vector3
        {
            $x = (float)$x;
            $y = $y === null ? $x : (float)$y;
            $z = $z === null ? $x : (float)$z;
            return new Vector3($this->x - $x, $this->y - $y, $this->z - $z);
        }

        public function subtractVector(Vector3 $vector): Vector3
        {
            return new Vector3($this->x - $vector->x, $this->y - $vector->y, $this->z - $vector->z);
        }

        public function multiply(float $number): Vector3
        {
            return new Vector3($this->x * $number, $this->y * $number, $this->z * $number);
        }

        public function divide(float $number): Vector3
        {
            if ($number === 0.0) {
                return new Vector3(0.0, 0.0, 0.0);
            }
            return new Vector3($this->x / $number, $this->y / $number, $this->z / $number);
        }

        public function ceil(): Vector3
        {
            return new Vector3(ceil($this->x), ceil($this->y), ceil($this->z));
        }

        public function floor(): Vector3
        {
            return new Vector3(floor($this->x), floor($this->y), floor($this->z));
        }

        public function round(int $precision = 0): Vector3
        {
            return new Vector3(
                round($this->x, $precision),
                round($this->y, $precision),
                round($this->z, $precision)
            );
        }

        public function abs(): Vector3
        {
            return new Vector3(abs($this->x), abs($this->y), abs($this->z));
        }

        public function distance(Vector3 $pos): float
        {
            return sqrt($this->distanceSquared($pos));
        }

        public function distanceSquared(Vector3 $pos): float
        {
            return ($this->x - $pos->x) ** 2 + ($this->y - $pos->y) ** 2 + ($this->z - $pos->z) ** 2;
        }

        public function maxPlainDistance($x, float $y = null, float $z = null): float
        {
            if ($x instanceof Vector3) {
                return max(abs($this->x - $x->x), abs($this->y - $x->y), abs($this->z - $x->z));
            }
            $y = $y === null ? (float)$x : $y;
            $z = $z === null ? (float)$x : $z;
            return max(abs($this->x - (float)$x), abs($this->y - $y), abs($this->z - $z));
        }

        public function length(): float
        {
            return sqrt($this->lengthSquared());
        }

        public function lengthSquared(): float
        {
            return $this->x ** 2 + $this->y ** 2 + $this->z ** 2;
        }

        public function normalize(): Vector3
        {
            $len = $this->length();
            if ($len == 0.0) {
                return new Vector3(0.0, 0.0, 0.0);
            }
            return $this->divide($len);
        }

        public function dot(Vector3 $v): float
        {
            return $this->x * $v->x + $this->y * $v->y + $this->z * $v->z;
        }

        public function cross(Vector3 $v): Vector3
        {
            return new Vector3(
                $this->y * $v->z - $this->z * $v->y,
                $this->z * $v->x - $this->x * $v->z,
                $this->x * $v->y - $this->y * $v->x
            );
        }

        public function equals(Vector3 $pos): bool
        {
            return $this->x == $pos->x && $this->y == $pos->y && $this->z == $pos->z;
        }

        /** @return Vector3[] */
        public function sides(): array
        {
            return [Vector3::SIDE_DOWN(), Vector3::SIDE_UP(), Vector3::SIDE_NORTH(), Vector3::SIDE_SOUTH(), Vector3::SIDE_WEST(), Vector3::SIDE_EAST()];
        }

        public static function SIDE_WEST(): Vector3
        {
            return new Vector3(-1, 0, 0);
        }

        public static function SIDE_EAST(): Vector3
        {
            return new Vector3(1, 0, 0);
        }

        public static function SIDE_NORTH(): Vector3
        {
            return new Vector3(0, 0, -1);
        }

        public static function SIDE_SOUTH(): Vector3
        {
            return new Vector3(0, 0, 1);
        }

        public static function SIDE_DOWN(): Vector3
        {
            return new Vector3(0, -1, 0);
        }

        public static function SIDE_UP(): Vector3
        {
            return new Vector3(0, 1, 0);
        }

        public function getSide(int $side, int $step = 1): Vector3
        {
            if ($step < 0) {
                $step = -$step;
                $side ^= 1;
            }
            return match ($side) {
                0 => new Vector3($this->x - $step, $this->y, $this->z),
                1 => new Vector3($this->x + $step, $this->y, $this->z),
                2 => new Vector3($this->x, $this->y, $this->z - $step),
                3 => new Vector3($this->x, $this->y, $this->z + $step),
                4 => new Vector3($this->x, $this->y - $step, $this->z),
                5 => new Vector3($this->x, $this->y + $step, $this->z),
                default => new Vector3($this->x, $this->y, $this->z),
            };
        }

        public static function fromArray(array $data): Vector3
        {
            return new Vector3((float)($data['x'] ?? 0.0), (float)($data['y'] ?? 0.0), (float)($data['z'] ?? 0.0));
        }

        public function toArray(): array
        {
            return ['x' => $this->x, 'y' => $this->y, 'z' => $this->z];
        }

        public function __toString(): string
        {
            return 'Vector3(x=' . $this->x . ',y=' . $this->y . ',z=' . $this->z . ')';
        }
    }

    class Vector2
    {
        public function __construct(public float $x = 0.0, public float $y = 0.0)
        {
        }

        public function getX(): float
        {
            return $this->x;
        }

        public function getY(): float
        {
            return $this->y;
        }

        public function add(float $x, float $y = null): Vector2
        {
            $y = $y === null ? $x : $y;
            return new Vector2($this->x + $x, $this->y + $y);
        }

        public function subtract(float $x, float $y = null): Vector2
        {
            $y = $y === null ? $x : $y;
            return new Vector2($this->x - $x, $this->y - $y);
        }

        public function multiply(float $number): Vector2
        {
            return new Vector2($this->x * $number, $this->y * $number);
        }

        public function divide(float $number): Vector2
        {
            return $number == 0.0 ? new Vector2(0.0, 0.0) : new Vector2($this->x / $number, $this->y / $number);
        }

        public function ceil(): Vector2
        {
            return new Vector2(ceil($this->x), ceil($this->y));
        }

        public function floor(): Vector2
        {
            return new Vector2(floor($this->x), floor($this->y));
        }

        public function distance(Vector2 $pos): float
        {
            return sqrt($this->distanceSquared($pos));
        }

        public function distanceSquared(Vector2 $pos): float
        {
            return ($this->x - $pos->x) ** 2 + ($this->y - $pos->y) ** 2;
        }

        public function length(): float
        {
            return sqrt($this->lengthSquared());
        }

        public function lengthSquared(): float
        {
            return $this->x ** 2 + $this->y ** 2;
        }

        public function normalize(): Vector2
        {
            $len = $this->length();
            return $len == 0.0 ? new Vector2(0.0, 0.0) : $this->divide($len);
        }

        public function __toString(): string
        {
            return 'Vector2(x=' . $this->x . ',y=' . $this->y . ')';
        }
    }

    class AxisAlignedBB
    {
        public function __construct(
            public float $minX,
            public float $minY,
            public float $minZ,
            public float $maxX,
            public float $maxY,
            public float $maxZ
        ) {
        }

        public function getMinX(): float
        {
            return $this->minX;
        }

        public function getMinY(): float
        {
            return $this->minY;
        }

        public function getMinZ(): float
        {
            return $this->minZ;
        }

        public function getMaxX(): float
        {
            return $this->maxX;
        }

        public function getMaxY(): float
        {
            return $this->maxY;
        }

        public function getMaxZ(): float
        {
            return $this->maxZ;
        }

        public function isVectorInside(Vector3 $vector): bool
        {
            return $vector->x >= $this->minX && $vector->x <= $this->maxX
                && $vector->y >= $this->minY && $vector->y <= $this->maxY
                && $vector->z >= $this->minZ && $vector->z <= $this->maxZ;
        }

        /** PM `AxisAlignedBB::expandedCopy()`: copia crecida en cada eje. */
        public function expandedCopy(float $x, float $y, float $z): AxisAlignedBB
        {
            return new AxisAlignedBB(
                $this->minX - $x,
                $this->minY - $y,
                $this->minZ - $z,
                $this->maxX + $x,
                $this->maxY + $y,
                $this->maxZ + $z
            );
        }

        public function intersectsWith(AxisAlignedBB $bb): bool
        {
            return $this->maxX > $bb->minX && $this->minX < $bb->maxX
                && $this->maxY > $bb->minY && $this->minY < $bb->maxY
                && $this->maxZ > $bb->minZ && $this->minZ < $bb->maxZ;
        }

        public static function one(): self
        {
            return new self(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        }

        public function offset(float $x, float $y, float $z): self
        {
            $this->minX += $x;
            $this->minY += $y;
            $this->minZ += $z;
            $this->maxX += $x;
            $this->maxY += $y;
            $this->maxZ += $z;
            return $this;
        }

        public function contract(float $x, float $y, float $z): self
        {
            $this->minX += $x;
            $this->minY += $y;
            $this->minZ += $z;
            $this->maxX -= $x;
            $this->maxY -= $y;
            $this->maxZ -= $z;
            return $this;
        }

        public function extend(int $facing, float $dist): self
        {
            match ($facing) {
                Facing::DOWN => $this->minY -= $dist,
                Facing::UP => $this->maxY += $dist,
                Facing::NORTH => $this->minZ -= $dist,
                Facing::SOUTH => $this->maxZ += $dist,
                Facing::WEST => $this->minX -= $dist,
                Facing::EAST => $this->maxX += $dist,
                default => throw new \InvalidArgumentException("Invalid facing $facing"),
            };
            return $this;
        }

        public function trim(int $facing, float $dist): self
        {
            return $this->extend($facing, -$dist);
        }

        public function squash(int $axis, float $dist): self
        {
            match ($axis) {
                Axis::X => $this->contract($dist, 0.0, 0.0),
                Axis::Y => $this->contract(0.0, $dist, 0.0),
                Axis::Z => $this->contract(0.0, 0.0, $dist),
                default => throw new \InvalidArgumentException("Invalid axis $axis"),
            };
            return $this;
        }

        public function getAverageEdgeLength(): float
        {
            return (($this->maxX - $this->minX) + ($this->maxY - $this->minY) + ($this->maxZ - $this->minZ)) / 3.0;
        }

        public function isCube(): bool
        {
            $x = $this->maxX - $this->minX;
            $y = $this->maxY - $this->minY;
            $z = $this->maxZ - $this->minZ;
            return abs($x - $y) < 0.00001 && abs($y - $z) < 0.00001;
        }

        public function calculateIntercept(Vector3 $pos1, Vector3 $pos2): ?RayTraceResult
        {
            return null;
        }

        public function __toString(): string
        {
            return 'AxisAlignedBB(' . $this->minX . ',' . $this->minY . ',' . $this->minZ . ' - '
                . $this->maxX . ',' . $this->maxY . ',' . $this->maxZ . ')';
        }
    }

    final class Axis
    {
        public const X = 0;
        public const Y = 1;
        public const Z = 2;

        private function __construct()
        {
        }
    }

    final class Facing
    {
        public const DOWN = 0;
        public const UP = 1;
        public const NORTH = 2;
        public const SOUTH = 3;
        public const WEST = 4;
        public const EAST = 5;

        public const ALL = [self::DOWN, self::UP, self::NORTH, self::SOUTH, self::WEST, self::EAST];
        public const HORIZONTAL = [self::NORTH, self::SOUTH, self::WEST, self::EAST];

        public const OFFSET = [
            self::DOWN => [0, -1, 0],
            self::UP => [0, 1, 0],
            self::NORTH => [0, 0, -1],
            self::SOUTH => [0, 0, 1],
            self::WEST => [-1, 0, 0],
            self::EAST => [1, 0, 0],
        ];

        public const OPPOSITE = [
            self::DOWN => self::UP,
            self::UP => self::DOWN,
            self::NORTH => self::SOUTH,
            self::SOUTH => self::NORTH,
            self::WEST => self::EAST,
            self::EAST => self::WEST,
        ];

        public const AXIS = [
            self::DOWN => Axis::Y,
            self::UP => Axis::Y,
            self::NORTH => Axis::Z,
            self::SOUTH => Axis::Z,
            self::WEST => Axis::X,
            self::EAST => Axis::X,
        ];

        private function __construct()
        {
        }

        public static function opposite(int $facing): int
        {
            return self::OPPOSITE[$facing] ?? throw new \InvalidArgumentException("Invalid facing $facing");
        }

        public static function axis(int $facing): int
        {
            return self::AXIS[$facing] ?? throw new \InvalidArgumentException("Invalid facing $facing");
        }

        public static function rotateY(int $facing, bool $clockwise): int
        {
            return match ($facing) {
                self::NORTH => $clockwise ? self::EAST : self::WEST,
                self::EAST => $clockwise ? self::SOUTH : self::NORTH,
                self::SOUTH => $clockwise ? self::WEST : self::EAST,
                self::WEST => $clockwise ? self::NORTH : self::SOUTH,
                default => throw new \InvalidArgumentException("Invalid horizontal facing $facing"),
            };
        }

        public static function rotateX(int $facing, bool $clockwise): int
        {
            return match ($facing) {
                self::UP => $clockwise ? self::NORTH : self::SOUTH,
                self::NORTH => $clockwise ? self::DOWN : self::UP,
                self::DOWN => $clockwise ? self::SOUTH : self::NORTH,
                self::SOUTH => $clockwise ? self::UP : self::DOWN,
                default => throw new \InvalidArgumentException("Invalid facing for X rotation $facing"),
            };
        }

        public static function rotateZ(int $facing, bool $clockwise): int
        {
            return match ($facing) {
                self::UP => $clockwise ? self::EAST : self::WEST,
                self::EAST => $clockwise ? self::DOWN : self::UP,
                self::DOWN => $clockwise ? self::WEST : self::EAST,
                self::WEST => $clockwise ? self::UP : self::DOWN,
                default => throw new \InvalidArgumentException("Invalid facing for Z rotation $facing"),
            };
        }

        public static function validate(int $facing): void
        {
            if (!isset(self::OPPOSITE[$facing])) {
                throw new \InvalidArgumentException("Invalid facing $facing");
            }
        }
    }

    class RayTraceResult
    {
        public function __construct(
            public AxisAlignedBB $boundingBox,
            public int $hitFace,
            public Vector3 $hitVector
        ) {
        }
    }
}

namespace pocketmine\world {

    use pocketmine\math\Vector3;

    class Position extends Vector3
    {
        protected ?\pocketmine\world\World $world = null;
        private string $worldName;

        public function __construct(float $x = 0.0, float $y = 0.0, float $z = 0.0, mixed $world = null)
        {
            parent::__construct($x, $y, $z);
            if ($world instanceof \pocketmine\world\World) {
                $this->world = $world;
                $this->worldName = $world->getFolderName();
            } else {
                $this->worldName = (string)$world;
            }
        }

        public function getWorldName(): string
        {
            return $this->worldName;
        }

        public function isValid(): bool
        {
            return $this->world !== null || $this->worldName !== '';
        }

        public function isWorldLoaded(): bool
        {
            if ($this->world !== null) {
                return $this->world->isLoaded();
            }
            if (!class_exists(\pocketmine\Server::class)) {
                return false;
            }
            try {
                $server = \pocketmine\Server::getInstance();
                return $server->getWorldManager()->isWorldLoaded($this->worldName);
            } catch (\Throwable) {
                return false;
            }
        }

        public function getWorld()
        {
            if ($this->world !== null) {
                return $this->world;
            }
            if (!class_exists(\pocketmine\Server::class)) {
                return null;
            }
            try {
                return \pocketmine\Server::getInstance()->getWorldManager()->getWorldByName($this->worldName);
            } catch (\Throwable) {
                return null;
            }
        }

        public function getPosition(): Position
        {
            return new Position($this->x, $this->y, $this->z, $this->worldName);
        }

        public function asPosition(): Position
        {
            return $this->getPosition();
        }

        public function withWorld(string $worldName): Position
        {
            return new Position($this->x, $this->y, $this->z, $worldName);
        }

        public function add(float $x, float $y = null, float $z = null): Vector3
        {
            $y = $y === null ? $x : $y;
            $z = $z === null ? $x : $z;
            return new Position($this->x + $x, $this->y + $y, $this->z + $z, $this->worldName);
        }

        public function subtract(float $x, float $y = null, float $z = null): Vector3
        {
            $y = $y === null ? $x : $y;
            $z = $z === null ? $x : $z;
            return new Position($this->x - $x, $this->y - $y, $this->z - $z, $this->worldName);
        }

        public function __toString(): string
        {
            return 'Position(x=' . $this->x . ',y=' . $this->y . ',z=' . $this->z . ',world=' . $this->worldName . ')';
        }
    }

    class Location extends Position
    {
        public function __construct(
            float $x = 0.0,
            float $y = 0.0,
            float $z = 0.0,
            private float $yaw = 0.0,
            private float $pitch = 0.0,
            string $worldName = ''
        ) {
            parent::__construct($x, $y, $z, $worldName);
        }

        public function getYaw(): float
        {
            return $this->yaw;
        }

        public function getPitch(): float
        {
            return $this->pitch;
        }

        public function setYaw(float $yaw): void
        {
            $this->yaw = $yaw;
        }

        public function setPitch(float $pitch): void
        {
            $this->pitch = $pitch;
        }

        public function getPosition(): Position
        {
            return new Position($this->x, $this->y, $this->z, $this->getWorldName());
        }

        public static function fromObject(Vector3 $pos, mixed $world = null, float $yaw = 0.0, float $pitch = 0.0): Location
        {
            $worldName = '';
            if (is_object($world) && method_exists($world, 'getFolderName')) {
                $worldName = $world->getFolderName();
            } elseif (is_string($world)) {
                $worldName = $world;
            } elseif ($pos instanceof Position) {
                $worldName = $pos->getWorldName();
            }
            return new Location($pos->x, $pos->y, $pos->z, $yaw, $pitch, $worldName);
        }

        public function __toString(): string
        {
            return 'Location(x=' . $this->x . ',y=' . $this->y . ',z=' . $this->z
                . ',yaw=' . $this->yaw . ',pitch=' . $this->pitch . ',world=' . $this->getWorldName() . ')';
        }
    }
}
