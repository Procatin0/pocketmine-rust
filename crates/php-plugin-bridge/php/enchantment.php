<?php

declare(strict_types=1);

namespace pocketmine\utils {
    if (!trait_exists(SingletonTrait::class, false)) {
        trait SingletonTrait
        {
            private static ?self $instance = null;

            public static function getInstance(): static
            {
                return self::$instance ??= new self();
            }

            public static function reset(): void
            {
                self::$instance = null;
            }
        }
    }

    if (!trait_exists(LegacyEnumShimTrait::class, false)) {
        trait LegacyEnumShimTrait
        {
            public static function __callStatic(string $name, array $arguments): mixed
            {
                if (count($arguments) > 0) {
                    throw new \ArgumentCountError("Expected exactly 0 arguments, " . count($arguments) . " passed");
                }
                $all = self::getAll();
                $key = strtoupper($name);
                if (!isset($all[$key])) {
                    throw new \InvalidArgumentException("No such case " . $name);
                }
                return $all[$key];
            }

            public static function getAll(): array
            {
                return [];
            }
        }
    }

    if (!trait_exists(NotCloneable::class, false)) {
        trait NotCloneable
        {
            private function __clone(): void
            {
            }
        }
    }

    if (!trait_exists(NotSerializable::class, false)) {
        trait NotSerializable
        {
            public function __serialize(): array
            {
                throw new \BadMethodCallException("Cannot serialize " . static::class);
            }

            public function __unserialize(array $data): void
            {
                throw new \BadMethodCallException("Cannot unserialize " . static::class);
            }
        }
    }

    if (!class_exists(Limits::class, false)) {
        final class Limits
        {
            public const INT32_MIN = -0x7fffffff - 1;
            public const INT32_MAX = 0x7fffffff;
        }
    }

    if (!class_exists(Random::class, false)) {
        class Random
        {
            public const X = 123456789;
            public const Y = 362436069;
            public const Z = 521288629;
            public const W = 88675123;

            private int $x;
            private int $y;
            private int $z;
            private int $w;
            protected int $seed;

            public function __construct(int $seed = -1)
            {
                $this->setSeed($seed === -1 ? time() : $seed);
            }

            public function setSeed(int $seed): void
            {
                $this->seed = $seed;
                $this->x = self::X ^ $seed;
                $this->y = self::Y ^ ($seed << 17) | (($seed >> 15) & 0x7fffffff) & 0xffffffff;
                $this->z = self::Z ^ ($seed << 31) | (($seed >> 1) & 0x7fffffff) & 0xffffffff;
                $this->w = self::W ^ ($seed << 18) | (($seed >> 14) & 0x7fffffff) & 0xffffffff;
            }

            public function getSeed(): int
            {
                return $this->seed;
            }

            public function nextInt(): int
            {
                return $this->nextSignedInt() & 0x7fffffff;
            }

            public function nextSignedInt(): int
            {
                $t = ($this->x ^ ($this->x << 11)) & 0xffffffff;

                $this->x = $this->y;
                $this->y = $this->z;
                $this->z = $this->w;
                $this->w = ($this->w ^ (($this->w >> 19) & 0x7fffffff) ^ ($t ^ (($t >> 8) & 0x7fffffff))) & 0xffffffff;

                return (int)($this->w >= 0x80000000 ? $this->w - 0x100000000 : $this->w);
            }

            public function nextFloat(): float
            {
                return $this->nextInt() / 0x7fffffff;
            }

            public function nextSignedFloat(): float
            {
                return $this->nextSignedInt() / 0x7fffffff;
            }

            public function nextBoolean(): bool
            {
                return ($this->nextSignedInt() & 0x01) === 0;
            }

            public function nextRange(int $start = 0, int $end = 0x7fffffff): int
            {
                return $start + ($this->nextInt() % ($end + 1 - $start));
            }

            public function nextBoundedInt(int $bound): int
            {
                return $this->nextInt() % $bound;
            }
        }
    }

    if (!class_exists(StringToTParser::class, false)) {
        abstract class StringToTParser
        {
            /** @var array<string, \Closure(string): mixed> */
            private array $callbackMap = [];

            public function register(string $alias, \Closure $callback): void
            {
                $key = $this->reprocess($alias);
                if (isset($this->callbackMap[$key])) {
                    throw new \InvalidArgumentException("Alias \"$key\" is already registered");
                }
                $this->callbackMap[$key] = $callback;
            }

            public function override(string $alias, \Closure $callback): void
            {
                $this->callbackMap[$this->reprocess($alias)] = $callback;
            }

            public function registerAlias(string $existing, string $alias): void
            {
                $existingKey = $this->reprocess($existing);
                if (!isset($this->callbackMap[$existingKey])) {
                    throw new \InvalidArgumentException("Cannot register new alias for unknown existing alias \"$existing\"");
                }
                $newKey = $this->reprocess($alias);
                if (isset($this->callbackMap[$newKey])) {
                    throw new \InvalidArgumentException("Alias \"$newKey\" is already registered");
                }
                $newCallback = $this->callbackMap[$existingKey];
                $this->callbackMap[$newKey] = $newCallback;
            }

            public function parse(string $input): mixed
            {
                $key = $this->reprocess($input);
                if (isset($this->callbackMap[$key])) {
                    return ($this->callbackMap[$key])($input);
                }
                return null;
            }

            protected function reprocess(string $input): string
            {
                return strtolower(str_replace([" ", "minecraft:"], ["_", ""], trim($input)));
            }

            public function getKnownAliases(): array
            {
                return array_keys($this->callbackMap);
            }
        }
    }

    if (!class_exists(RegistrySource::class, false)) {
        abstract class RegistrySource
        {
            abstract public function getTargetClassName(): string;
            abstract protected function setup(): void;
        }
    }
}

namespace pocketmine\lang {
    if (!interface_exists(Translatable::class, false) && !class_exists(Translatable::class, false)) {
        interface Translatable
        {
            public function getText(): string;
        }
    }

    if (!class_exists(KnownTranslationFactory::class, false)) {
        class KnownTranslationFactory
        {
            public static function __callStatic(string $name, array $args): \pocketmine\lang\Translatable|string
            {
                $text = 'enchantment.' . str_replace('_', '.', $name);
                if (class_exists(\pocketmine\lang\Translatable::class)) {
                    return new \pocketmine\lang\Translatable($text, $args);
                }
                return $text;
            }
        }
    }
}

namespace pocketmine\data\bedrock {
    use pocketmine\item\enchantment\Enchantment;
    use pocketmine\item\enchantment\VanillaEnchantments;
    use pocketmine\utils\SingletonTrait;

    if (!trait_exists(IntSaveIdMapTrait::class, false)) {
        trait IntSaveIdMapTrait
        {
            /** @var array<int, object> */
            private array $idToEnum = [];
            /** @var array<int, int> */
            private array $enumToId = [];

            protected function getRuntimeId(object $enum): int
            {
                return spl_object_id($enum);
            }

            public function register(int $saveId, object $enum): void
            {
                $this->idToEnum[$saveId] = $enum;
                $this->enumToId[$this->getRuntimeId($enum)] = $saveId;
            }

            public function fromId(int $id): ?object
            {
                return $this->idToEnum[$id] ?? null;
            }

            public function toId(object $enum): ?int
            {
                return $this->enumToId[$this->getRuntimeId($enum)] ?? null;
            }
        }
    }

    final class EnchantmentIds
    {
        private function __construct()
        {
        }

        public const PROTECTION = 0;
        public const FIRE_PROTECTION = 1;
        public const FEATHER_FALLING = 2;
        public const BLAST_PROTECTION = 3;
        public const PROJECTILE_PROTECTION = 4;
        public const THORNS = 5;
        public const RESPIRATION = 6;
        public const DEPTH_STRIDER = 7;
        public const AQUA_AFFINITY = 8;
        public const SHARPNESS = 9;
        public const SMITE = 10;
        public const BANE_OF_ARTHROPODS = 11;
        public const KNOCKBACK = 12;
        public const FIRE_ASPECT = 13;
        public const LOOTING = 14;
        public const EFFICIENCY = 15;
        public const SILK_TOUCH = 16;
        public const UNBREAKING = 17;
        public const FORTUNE = 18;
        public const POWER = 19;
        public const PUNCH = 20;
        public const FLAME = 21;
        public const INFINITY = 22;
        public const LUCK_OF_THE_SEA = 23;
        public const LURE = 24;
        public const FROST_WALKER = 25;
        public const MENDING = 26;
        public const BINDING = 27;
        public const VANISHING = 28;
        public const IMPALING = 29;
        public const RIPTIDE = 30;
        public const LOYALTY = 31;
        public const CHANNELING = 32;
        public const MULTISHOT = 33;
        public const PIERCING = 34;
        public const QUICK_CHARGE = 35;
        public const SOUL_SPEED = 36;
        public const SWIFT_SNEAK = 37;
    }

    final class EnchantmentIdMap
    {
        use SingletonTrait;
        use IntSaveIdMapTrait;

        /** @var array<int, Enchantment> */
        private static array $byId = [];
        /** @var array<int, int> */
        private static array $byObject = [];
        private static bool $setupDone = false;

        private function __construct()
        {
            self::setup();
        }

        public static function setup(): void
        {
            if (self::$setupDone) {
                return;
            }
            self::$setupDone = true;

            $map = [
                EnchantmentIds::PROTECTION => VanillaEnchantments::PROTECTION(),
                EnchantmentIds::FIRE_PROTECTION => VanillaEnchantments::FIRE_PROTECTION(),
                EnchantmentIds::FEATHER_FALLING => VanillaEnchantments::FEATHER_FALLING(),
                EnchantmentIds::BLAST_PROTECTION => VanillaEnchantments::BLAST_PROTECTION(),
                EnchantmentIds::PROJECTILE_PROTECTION => VanillaEnchantments::PROJECTILE_PROTECTION(),
                EnchantmentIds::THORNS => VanillaEnchantments::THORNS(),
                EnchantmentIds::RESPIRATION => VanillaEnchantments::RESPIRATION(),
                EnchantmentIds::DEPTH_STRIDER => VanillaEnchantments::DEPTH_STRIDER(),
                EnchantmentIds::AQUA_AFFINITY => VanillaEnchantments::AQUA_AFFINITY(),
                EnchantmentIds::SHARPNESS => VanillaEnchantments::SHARPNESS(),
                EnchantmentIds::SMITE => VanillaEnchantments::SMITE(),
                EnchantmentIds::BANE_OF_ARTHROPODS => VanillaEnchantments::BANE_OF_ARTHROPODS(),
                EnchantmentIds::KNOCKBACK => VanillaEnchantments::KNOCKBACK(),
                EnchantmentIds::FIRE_ASPECT => VanillaEnchantments::FIRE_ASPECT(),
                EnchantmentIds::LOOTING => VanillaEnchantments::LOOTING(),
                EnchantmentIds::EFFICIENCY => VanillaEnchantments::EFFICIENCY(),
                EnchantmentIds::SILK_TOUCH => VanillaEnchantments::SILK_TOUCH(),
                EnchantmentIds::UNBREAKING => VanillaEnchantments::UNBREAKING(),
                EnchantmentIds::FORTUNE => VanillaEnchantments::FORTUNE(),
                EnchantmentIds::POWER => VanillaEnchantments::POWER(),
                EnchantmentIds::PUNCH => VanillaEnchantments::PUNCH(),
                EnchantmentIds::FLAME => VanillaEnchantments::FLAME(),
                EnchantmentIds::INFINITY => VanillaEnchantments::INFINITY(),
                EnchantmentIds::LUCK_OF_THE_SEA => VanillaEnchantments::LUCK_OF_THE_SEA(),
                EnchantmentIds::LURE => VanillaEnchantments::LURE(),
                EnchantmentIds::FROST_WALKER => VanillaEnchantments::FROST_WALKER(),
                EnchantmentIds::MENDING => VanillaEnchantments::MENDING(),
                EnchantmentIds::BINDING => VanillaEnchantments::BINDING(),
                EnchantmentIds::VANISHING => VanillaEnchantments::VANISHING(),
                EnchantmentIds::IMPALING => VanillaEnchantments::IMPALING(),
                EnchantmentIds::RIPTIDE => VanillaEnchantments::RIPTIDE(),
                EnchantmentIds::LOYALTY => VanillaEnchantments::LOYALTY(),
                EnchantmentIds::CHANNELING => VanillaEnchantments::CHANNELING(),
                EnchantmentIds::MULTISHOT => VanillaEnchantments::MULTISHOT(),
                EnchantmentIds::PIERCING => VanillaEnchantments::PIERCING(),
                EnchantmentIds::QUICK_CHARGE => VanillaEnchantments::QUICK_CHARGE(),
                EnchantmentIds::SOUL_SPEED => VanillaEnchantments::SOUL_SPEED(),
                EnchantmentIds::SWIFT_SNEAK => VanillaEnchantments::SWIFT_SNEAK(),
            ];

            foreach ($map as $id => $enchantment) {
                self::register($id, $enchantment);
            }
        }

        public static function register(int $id, Enchantment $enchantment): void
        {
            self::$byId[$id] = $enchantment;
            self::$byObject[spl_object_id($enchantment)] = $id;
        }

        public static function toId(Enchantment $enchantment): ?int
        {
            self::setup();
            return self::$byObject[spl_object_id($enchantment)] ?? null;
        }

        public static function fromId(int $id): ?Enchantment
        {
            self::setup();
            return self::$byId[$id] ?? null;
        }
    }
}

namespace pocketmine\data\bedrock\enchantment {
    use pocketmine\data\bedrock\EnchantmentIdMap as BedrockEnchantmentIdMap;
    use pocketmine\item\enchantment\Enchantment;
    use pocketmine\utils\SingletonTrait;

    final class EnchantmentIdMap
    {
        use SingletonTrait;

        public static function register(int $id, Enchantment $enchantment): void
        {
            BedrockEnchantmentIdMap::register($id, $enchantment);
        }

        public static function toId(Enchantment $enchantment): ?int
        {
            return BedrockEnchantmentIdMap::toId($enchantment);
        }

        public static function fromId(int $id): ?Enchantment
        {
            return BedrockEnchantmentIdMap::fromId($id);
        }
    }
}

namespace pocketmine\item\enchantment {
    use pocketmine\block\BlockTypeIds;
    use pocketmine\data\bedrock\EnchantmentIdMap;
    use pocketmine\entity\Entity;
    use pocketmine\entity\Living;
    use pocketmine\event\entity\EntityDamageEvent;
    use pocketmine\item\enchantment\IncompatibleEnchantmentGroups as Groups;
    use pocketmine\item\enchantment\ItemEnchantmentTagRegistry as TagRegistry;
    use pocketmine\item\enchantment\ItemEnchantmentTags as Tags;
    use pocketmine\item\Item;
    use pocketmine\item\ItemTypeIds;
    use pocketmine\item\VanillaItems as Items;
    use pocketmine\lang\KnownTranslationFactory;
    use pocketmine\lang\Translatable;
    use pocketmine\utils\LegacyEnumShimTrait;
    use pocketmine\utils\Limits;
    use pocketmine\utils\NotCloneable;
    use pocketmine\utils\NotSerializable;
    use pocketmine\utils\Random;
    use pocketmine\utils\RegistrySource;
    use pocketmine\utils\SingletonTrait;
    use pocketmine\utils\StringToTParser;
    use pocketmine\world\Position;
    use function abs;
    use function array_diff;
    use function array_fill_keys;
    use function array_filter;
    use function array_intersect;
    use function array_intersect_key;
    use function array_merge;
    use function array_search;
    use function array_shift;
    use function array_unique;
    use function array_values;
    use function chr;
    use function count;
    use function floor;
    use function max;
    use function mb_strtoupper;
    use function min;
    use function mt_rand;
    use function ord;
    use function round;
    use function spl_object_id;
    use function strtolower;

    /**
     * `pocketmine\item\enchantment\Rarity`: Rarity constants used by `Enchantment::getRarity()`.
     */
    final class Rarity
    {
        use LegacyEnumShimTrait;

        private function __construct()
        {
            //NOOP
        }

        public const COMMON = 10;
        public const UNCOMMON = 5;
        public const RARE = 2;
        public const MYTHIC = 1;

        public static function COMMON(): int
        {
            return self::COMMON;
        }

        public static function UNCOMMON(): int
        {
            return self::UNCOMMON;
        }

        public static function RARE(): int
        {
            return self::RARE;
        }

        public static function MYTHIC(): int
        {
            return self::MYTHIC;
        }

        public static function getDisplayName(int $rarity): string
        {
            return match ($rarity) {
                self::COMMON => 'Common',
                self::UNCOMMON => 'Uncommon',
                self::RARE => 'Rare',
                self::MYTHIC => 'Mythic',
                default => 'Unknown',
            };
        }

        /** @return array<string, int> */
        public static function getAll(): array
        {
            return [
                'COMMON' => self::COMMON,
                'UNCOMMON' => self::UNCOMMON,
                'RARE' => self::RARE,
                'MYTHIC' => self::MYTHIC,
            ];
        }

        /** @return int[] */
        public static function cases(): array
        {
            return [
                self::COMMON,
                self::UNCOMMON,
                self::RARE,
                self::MYTHIC,
            ];
        }
    }

    /**
     * @deprecated
     */
    final class ItemFlags
    {
        private function __construct()
        {
            //NOOP
        }

        public const NONE = 0x0;
        public const ALL = 0xffff;
        public const ARMOR = self::HEAD | self::TORSO | self::LEGS | self::FEET;
        public const HEAD = 0x1;
        public const TORSO = 0x2;
        public const LEGS = 0x4;
        public const FEET = 0x8;
        public const SWORD = 0x10;
        public const BOW = 0x20;
        public const TOOL = self::HOE | self::SHEARS | self::FLINT_AND_STEEL;
        public const HOE = 0x40;
        public const SHEARS = 0x80;
        public const FLINT_AND_STEEL = 0x100;
        public const DIG = self::AXE | self::PICKAXE | self::SHOVEL;
        public const AXE = 0x200;
        public const PICKAXE = 0x400;
        public const SHOVEL = 0x800;
        public const FISHING_ROD = 0x1000;
        public const CARROT_STICK = 0x2000;
        public const ELYTRA = 0x4000;
        public const TRIDENT = 0x8000;
    }

    /**
     * Tags used by items and enchantments to determine which enchantments can be applied to which items.
     * Some tags may contain other tags.
     * @see ItemEnchantmentTagRegistry
     */
    final class ItemEnchantmentTags
    {
        public const ALL = "all";
        public const ARMOR = "armor";
        public const HELMET = "helmet";
        public const CHESTPLATE = "chestplate";
        public const LEGGINGS = "leggings";
        public const BOOTS = "boots";
        public const SHIELD = "shield";
        public const SWORD = "sword";
        public const TRIDENT = "trident";
        public const BOW = "bow";
        public const CROSSBOW = "crossbow";
        public const SHEARS = "shears";
        public const FLINT_AND_STEEL = "flint_and_steel";
        public const BLOCK_TOOLS = "block_tools";
        public const AXE = "axe";
        public const PICKAXE = "pickaxe";
        public const SHOVEL = "shovel";
        public const HOE = "hoe";
        public const FISHING_ROD = "fishing_rod";
        public const CARROT_ON_STICK = "carrot_on_stick";
        public const COMPASS = "compass";
        public const MASK = "mask";
        public const ELYTRA = "elytra";
        public const BRUSH = "brush";
        public const WEAPONS = "weapons";
    }

    /**
     * Manages known item enchantment tags and the relations between them.
     * Used to determine which tags belong to which other tags, and to check if lists of tags intersect.
     */
    final class ItemEnchantmentTagRegistry
    {
        use SingletonTrait;

        /**
         * @phpstan-var array<string, list<string>>
         * @var string[][]
         */
        private array $tagMap = [];

        private function __construct()
        {
            $this->register(Tags::ARMOR, [Tags::HELMET, Tags::CHESTPLATE, Tags::LEGGINGS, Tags::BOOTS]);
            $this->register(Tags::SHIELD);
            $this->register(Tags::SWORD);
            $this->register(Tags::TRIDENT);
            $this->register(Tags::BOW);
            $this->register(Tags::CROSSBOW);
            $this->register(Tags::SHEARS);
            $this->register(Tags::FLINT_AND_STEEL);
            $this->register(Tags::BLOCK_TOOLS, [Tags::AXE, Tags::PICKAXE, Tags::SHOVEL, Tags::HOE]);
            $this->register(Tags::FISHING_ROD);
            $this->register(Tags::CARROT_ON_STICK);
            $this->register(Tags::COMPASS);
            $this->register(Tags::MASK);
            $this->register(Tags::ELYTRA);
            $this->register(Tags::BRUSH);
            $this->register(Tags::WEAPONS, [
                Tags::SWORD,
                Tags::TRIDENT,
                Tags::BOW,
                Tags::CROSSBOW,
                Tags::BLOCK_TOOLS,
            ]);
        }

        /**
         * Register tag and its nested tags.
         *
         * @param string[] $nestedTags
         */
        public function register(string $tag, array $nestedTags = []): void
        {
            $this->assertNotInternalTag($tag);

            foreach ($nestedTags as $nestedTag) {
                if (!isset($this->tagMap[$nestedTag])) {
                    $this->register($nestedTag);
                }
                $this->tagMap[$tag][] = $nestedTag;
            }

            if (!isset($this->tagMap[$tag])) {
                $this->tagMap[$tag] = [];
                $this->tagMap[Tags::ALL][] = $tag;
            }
        }

        public function unregister(string $tag): void
        {
            if (!isset($this->tagMap[$tag])) {
                return;
            }
            $this->assertNotInternalTag($tag);

            unset($this->tagMap[$tag]);

            foreach ($this->tagMap as $key => $nestedTags) {
                if (($nestedKey = array_search($tag, $nestedTags, true)) !== false) {
                    unset($nestedTags[$nestedKey]);
                    $this->tagMap[$key] = array_values($nestedTags);
                }
            }
        }

        /**
         * Remove specified nested tags.
         *
         * @param string[] $nestedTags
         */
        public function removeNested(string $tag, array $nestedTags): void
        {
            $this->assertNotInternalTag($tag);
            $this->tagMap[$tag] = array_values(array_diff($this->tagMap[$tag], $nestedTags));
        }

        /**
         * Returns nested tags of a particular tag.
         *
         * @return string[]
         */
        public function getNested(string $tag): array
        {
            return $this->tagMap[$tag] ?? [];
        }

        /**
         * @param string[] $firstTags
         * @param string[] $secondTags
         */
        public function isTagArrayIntersection(array $firstTags, array $secondTags): bool
        {
            if (count($firstTags) === 0 || count($secondTags) === 0) {
                return false;
            }

            $firstLeafTags = $this->getLeafTagsForArray($firstTags);
            $secondLeafTags = $this->getLeafTagsForArray($secondTags);

            return count(array_intersect($firstLeafTags, $secondLeafTags)) !== 0;
        }

        /**
         * Returns all tags that are recursively nested within each tag in the array and do not have any nested tags.
         *
         * @param string[] $tags
         * @return string[]
         */
        private function getLeafTagsForArray(array $tags): array
        {
            $leafTagArrays = [];
            foreach ($tags as $tag) {
                $leafTagArrays[] = $this->getLeafTags($tag);
            }
            return array_unique(array_merge(...$leafTagArrays));
        }

        /**
         * Returns all tags that are recursively nested within the given tag and do not have any nested tags.
         *
         * @return string[]
         */
        private function getLeafTags(string $tag): array
        {
            $result = [];
            $tagsToHandle = [$tag];

            while (count($tagsToHandle) !== 0) {
                $currentTag = array_shift($tagsToHandle);
                $nestedTags = $this->getNested($currentTag);

                if (count($nestedTags) === 0) {
                    $result[] = $currentTag;
                } else {
                    $tagsToHandle = array_merge($tagsToHandle, $nestedTags);
                }
            }

            return $result;
        }

        private function assertNotInternalTag(string $tag): void
        {
            if ($tag === Tags::ALL) {
                throw new \InvalidArgumentException(
                    "Cannot perform any operations on the internal item enchantment tag '$tag'"
                );
            }
        }
    }

    /**
     * Constants for groupings of incompatible enchantments.
     * Enchantments belonging to the same incompatibility group cannot be applied side-by-side on the same item.
     */
    final class IncompatibleEnchantmentGroups
    {
        public const PROTECTION = "protection";
        public const BOW_INFINITE = "bow_infinite";
        public const BLOCK_DROPS = "block_drops";
    }

    /**
     * Manages which enchantments are incompatible with each other.
     * Enchantments can be added to groups to make them incompatible with all other enchantments already in that group.
     */
    final class IncompatibleEnchantmentRegistry
    {
        use SingletonTrait;

        /**
         * @phpstan-var array<int, array<string, true>>
         * @var true[][]
         */
        private array $incompatibilityMap = [];

        private function __construct()
        {
            $this->register(Groups::PROTECTION, [
                VanillaEnchantments::PROTECTION(),
                VanillaEnchantments::FIRE_PROTECTION(),
                VanillaEnchantments::BLAST_PROTECTION(),
                VanillaEnchantments::PROJECTILE_PROTECTION(),
            ]);
            $this->register(Groups::BOW_INFINITE, [
                VanillaEnchantments::INFINITY(),
                VanillaEnchantments::MENDING(),
            ]);
            $this->register(Groups::BLOCK_DROPS, [
                VanillaEnchantments::FORTUNE(),
                VanillaEnchantments::SILK_TOUCH(),
            ]);
        }

        /**
         * Register incompatibility for an enchantment group.
         *
         * All enchantments belonging to the same group are incompatible with each other,
         * i.e. they cannot be added together on the same item.
         *
         * @param Enchantment[] $enchantments
         */
        public function register(string $tag, array $enchantments): void
        {
            foreach ($enchantments as $enchantment) {
                $this->incompatibilityMap[spl_object_id($enchantment)][$tag] = true;
            }
        }

        /**
         * Unregister incompatibility for some enchantments of a particular group.
         *
         * @param Enchantment[] $enchantments
         */
        public function unregister(string $tag, array $enchantments): void
        {
            foreach ($enchantments as $enchantment) {
                unset($this->incompatibilityMap[spl_object_id($enchantment)][$tag]);
            }
        }

        /**
         * Unregister incompatibility for all enchantments of a particular group.
         */
        public function unregisterAll(string $tag): void
        {
            foreach ($this->incompatibilityMap as $id => $tags) {
                unset($this->incompatibilityMap[$id][$tag]);
            }
        }

        /**
         * Returns whether two enchantments can be applied to the same item.
         */
        public function areCompatible(Enchantment $first, Enchantment $second): bool
        {
            $firstIncompatibilities = $this->incompatibilityMap[spl_object_id($first)] ?? [];
            $secondIncompatibilities = $this->incompatibilityMap[spl_object_id($second)] ?? [];
            return count(array_intersect_key($firstIncompatibilities, $secondIncompatibilities)) === 0;
        }
    }

    /**
     * Manages enchantment type data.
     */
    class Enchantment
    {
        use NotCloneable;
        use NotSerializable;

        private Translatable|string $name;
        private int $rarity;
        private int $primaryItemFlags;
        private int $secondaryItemFlags;
        private int $maxLevel;
        /** @var \Closure(int $level): int */
        private \Closure $minEnchantingPower;
        private int $enchantingPowerRange;

        /**
         * @phpstan-param null|(\Closure(int $level) : int) $minEnchantingPower
         *
         * @param int $primaryItemFlags     @deprecated
         * @param int $secondaryItemFlags   @deprecated
         * @param int $enchantingPowerRange Value used to calculate the maximum enchanting power (minEnchantingPower + enchantingPowerRange)
         */
        public function __construct(
            Translatable|string $name,
            int|object $rarity,
            int $primaryItemFlags,
            int $secondaryItemFlags,
            int $maxLevel,
            ?\Closure $minEnchantingPower = null,
            int $enchantingPowerRange = 50
        ) {
            $this->name = $name;
            $this->rarity = is_int($rarity) ? $rarity : (isset($rarity->value) ? (int)$rarity->value : Rarity::COMMON);
            $this->primaryItemFlags = $primaryItemFlags;
            $this->secondaryItemFlags = $secondaryItemFlags;
            $this->maxLevel = $maxLevel;
            $this->minEnchantingPower = $minEnchantingPower ?? static fn(int $level): int => 1;
            $this->enchantingPowerRange = $enchantingPowerRange;
        }

        /**
         * Returns a translation key or object for this enchantment's name.
         */
        public function getName(): Translatable|string
        {
            return $this->name;
        }

        /**
         * Returns an int constant indicating how rare this enchantment type is.
         */
        public function getRarity(): int
        {
            return $this->rarity;
        }

        /**
         * Returns a bitset indicating what item types can have this item applied from an enchanting table.
         *
         * @deprecated
         * @see AvailableEnchantmentRegistry::getPrimaryItemTags()
         */
        public function getPrimaryItemFlags(): int
        {
            return $this->primaryItemFlags;
        }

        /**
         * Returns a bitset indicating what item types cannot have this item applied from an enchanting table, but can from an anvil.
         *
         * @deprecated
         * @see AvailableEnchantmentRegistry::getSecondaryItemTags()
         */
        public function getSecondaryItemFlags(): int
        {
            return $this->secondaryItemFlags;
        }

        /**
         * Returns whether this enchantment can apply to the item type from an enchanting table.
         *
         * @deprecated
         * @see AvailableEnchantmentRegistry
         */
        public function hasPrimaryItemType(int $flag): bool
        {
            return ($this->primaryItemFlags & $flag) !== 0;
        }

        /**
         * Returns whether this enchantment can apply to the item type from an anvil, if it is not a primary item.
         *
         * @deprecated
         * @see AvailableEnchantmentRegistry
         */
        public function hasSecondaryItemType(int $flag): bool
        {
            return ($this->secondaryItemFlags & $flag) !== 0;
        }

        /**
         * Returns the maximum level of this enchantment that can be found on an enchantment table.
         */
        public function getMaxLevel(): int
        {
            return $this->maxLevel;
        }

        /**
         * Returns whether this enchantment can be applied to the item along with the given enchantment.
         */
        public function isCompatibleWith(Enchantment $other): bool
        {
            return IncompatibleEnchantmentRegistry::getInstance()->areCompatible($this, $other);
        }

        /**
         * Returns the minimum enchanting power value required for the particular level of the enchantment
         * to be available in an enchanting table.
         */
        public function getMinEnchantingPower(int $level): int
        {
            return ($this->minEnchantingPower)($level);
        }

        /**
         * Returns the maximum enchanting power value allowed for the particular level of the enchantment
         * to be available in an enchanting table.
         */
        public function getMaxEnchantingPower(int $level): int
        {
            return $this->getMinEnchantingPower($level) + $this->enchantingPowerRange;
        }

        /** Bedrock id used by the `ench` NBT list, or `null` when custom. */
        public function getBedrockId(): ?int
        {
            return EnchantmentIdMap::toId($this);
        }

        public function __toString(): string
        {
            return $this->name instanceof Translatable ? $this->name->getText() : (string)$this->name;
        }
    }

    /**
     * Classes extending this class can be applied to weapons and activate when used by a mob to attack another mob in melee
     * combat.
     */
    abstract class MeleeWeaponEnchantment extends Enchantment
    {
        /**
         * Returns whether this melee enchantment has an effect on the target entity.
         */
        abstract public function isApplicableTo(Entity $victim): bool;

        /**
         * Returns the amount of additional damage caused by this enchantment to applicable targets.
         */
        abstract public function getDamageBonus(int $enchantmentLevel): float;

        /**
         * Called after damaging the entity to apply any post damage effects to the target.
         */
        public function onPostAttack(Entity $attacker, Entity $victim, int $enchantmentLevel): void
        {
        }
    }

    class SharpnessEnchantment extends MeleeWeaponEnchantment
    {
        public function isApplicableTo(Entity $victim): bool
        {
            return true;
        }

        public function getDamageBonus(int $enchantmentLevel): float
        {
            return 0.5 * ($enchantmentLevel + 1);
        }
    }

    class KnockbackEnchantment extends MeleeWeaponEnchantment
    {
        public function isApplicableTo(Entity $victim): bool
        {
            return $victim instanceof Living;
        }

        public function getDamageBonus(int $enchantmentLevel): float
        {
            return 0.0;
        }

        public function onPostAttack(Entity $attacker, Entity $victim, int $enchantmentLevel): void
        {
            if ($victim instanceof Living) {
                $diff = $victim->getPosition()->subtractVector($attacker->getPosition());
                $victim->knockBack($diff->x, $diff->z, $enchantmentLevel * 0.5);
            }
        }
    }

    class FireAspectEnchantment extends MeleeWeaponEnchantment
    {
        public function isApplicableTo(Entity $victim): bool
        {
            return true;
        }

        public function getDamageBonus(int $enchantmentLevel): float
        {
            return 0.0;
        }

        public function onPostAttack(Entity $attacker, Entity $victim, int $enchantmentLevel): void
        {
            $victim->setOnFire($enchantmentLevel * 4);
        }
    }

    class ProtectionEnchantment extends Enchantment
    {
        protected float $typeModifier;
        /**
         * @var true[]|null
         * @phpstan-var array<int, true>
         */
        protected ?array $applicableDamageTypes = null;

        /**
         * @phpstan-param null|(\Closure(int $level) : int) $minEnchantingPower
         * @phpstan-param list<int>|null $applicableDamageTypes
         *
         * @param int        $primaryItemFlags      @deprecated
         * @param int        $secondaryItemFlags    @deprecated
         * @param int[]|null $applicableDamageTypes EntityDamageEvent::CAUSE_* constants which this enchantment applies to, or null if all.
         * @param int        $enchantingPowerRange  Value used to calculate the maximum enchanting power (minEnchantingPower + enchantingPowerRange)
         */
        public function __construct(
            Translatable|string $name,
            int|object $rarity,
            int $primaryItemFlags,
            int $secondaryItemFlags,
            int $maxLevel,
            float $typeModifier,
            ?array $applicableDamageTypes,
            ?\Closure $minEnchantingPower = null,
            int $enchantingPowerRange = 50
        ) {
            parent::__construct($name, $rarity, $primaryItemFlags, $secondaryItemFlags, $maxLevel, $minEnchantingPower, $enchantingPowerRange);

            $this->typeModifier = $typeModifier;
            if ($applicableDamageTypes !== null) {
                $this->applicableDamageTypes = array_fill_keys($applicableDamageTypes, true);
            }
        }

        /**
         * Returns the multiplier by which this enchantment type's EPF increases with each enchantment level.
         */
        public function getTypeModifier(): float
        {
            return $this->typeModifier;
        }

        /**
         * Returns the base EPF this enchantment type offers for the given enchantment level.
         */
        public function getProtectionFactor(int $level): int
        {
            return (int)floor((6 + $level ** 2) * $this->typeModifier / 3);
        }

        /**
         * Returns whether this enchantment type offers protection from the specified damage source's cause.
         */
        public function isApplicable(EntityDamageEvent $event): bool
        {
            return $this->applicableDamageTypes === null || isset($this->applicableDamageTypes[$event->getCause()]);
        }
    }

    /**
     * Container for enchantment data applied to items.
     * Note: This class is assumed to be immutable.
     */
    final class EnchantmentInstance
    {
        public function __construct(
            private Enchantment $enchantment,
            private int $level = 1
        ) {
        }

        public function getType(): Enchantment
        {
            return $this->enchantment;
        }

        public function getLevel(): int
        {
            return $this->level;
        }
    }

    /**
     * Represents an option on the enchanting table menu.
     */
    class EnchantingOption
    {
        /**
         * @param EnchantmentInstance[] $enchantments
         */
        public function __construct(
            private int $requiredXpLevel,
            private string $displayName,
            private array $enchantments
        ) {
        }

        public function getRequiredXpLevel(): int
        {
            return $this->requiredXpLevel;
        }

        public function getDisplayName(): string
        {
            return $this->displayName;
        }

        /**
         * @return EnchantmentInstance[]
         */
        public function getEnchantments(): array
        {
            return $this->enchantments;
        }
    }

    /**
     * Helper methods used for enchanting using the enchanting table.
     */
    final class EnchantingHelper
    {
        private const MAX_BOOKSHELF_COUNT = 15;

        private function __construct()
        {
            //NOOP
        }

        /**
         * Generates a new random seed for enchant option randomization.
         */
        public static function generateSeed(): int
        {
            return mt_rand(Limits::INT32_MIN, Limits::INT32_MAX);
        }

        /**
         * @param EnchantmentInstance[] $enchantments
         */
        public static function enchantItem(Item $item, array $enchantments): Item
        {
            $bookId = defined(ItemTypeIds::class . '::BOOK') ? ItemTypeIds::BOOK : 20018;
            $resultItem = $item->getTypeId() === $bookId ? Items::ENCHANTED_BOOK() : clone $item;

            foreach ($enchantments as $enchantment) {
                $resultItem->addEnchantment($enchantment);
            }

            return $resultItem;
        }

        /**
         * @return EnchantingOption[]
         */
        public static function generateOptions(Position $tablePos, Item $input, int $seed): array
        {
            if ($input->isNull() || $input->hasEnchantments()) {
                return [];
            }

            $random = new Random($seed);

            $bookshelfCount = self::countBookshelves($tablePos);
            $baseRequiredLevel = $random->nextRange(1, 8) + ($bookshelfCount >> 1) + $random->nextRange(0, $bookshelfCount);
            $topRequiredLevel = (int)floor(max($baseRequiredLevel / 3, 1));
            $middleRequiredLevel = (int)floor($baseRequiredLevel * 2 / 3 + 1);
            $bottomRequiredLevel = max($baseRequiredLevel, $bookshelfCount * 2);

            return [
                self::createOption($random, $input, $topRequiredLevel),
                self::createOption($random, $input, $middleRequiredLevel),
                self::createOption($random, $input, $bottomRequiredLevel),
            ];
        }

        private static function countBookshelves(Position $tablePos): int
        {
            $bookshelfCount = 0;
            $world = $tablePos->getWorld();
            if ($world === null) {
                return 0;
            }
            $bookshelfTypeId = defined(BlockTypeIds::class . '::BOOKSHELF') ? BlockTypeIds::BOOKSHELF : 10057;

            for ($x = -2; $x <= 2; $x++) {
                for ($z = -2; $z <= 2; $z++) {
                    if (abs($x) !== 2 && abs($z) !== 2) {
                        continue;
                    }

                    for ($y = 0; $y <= 1; $y++) {
                        $spaceX = max(min($x, 1), -1);
                        $spaceZ = max(min($z, 1), -1);
                        $spaceBlock = $world->getBlock($tablePos->add($spaceX, $y, $spaceZ));
                        if ($spaceBlock->getTypeId() !== BlockTypeIds::AIR) {
                            continue 2;
                        }
                    }

                    for ($y = 0; $y <= 1; $y++) {
                        $block = $world->getBlock($tablePos->add($x, $y, $z));
                        if ($block->getTypeId() === $bookshelfTypeId) {
                            $bookshelfCount++;
                            if ($bookshelfCount === self::MAX_BOOKSHELF_COUNT) {
                                return $bookshelfCount;
                            }
                        }
                    }
                }
            }

            return $bookshelfCount;
        }

        private static function createOption(Random $random, Item $inputItem, int $requiredXpLevel): EnchantingOption
        {
            $enchantingPower = $requiredXpLevel;

            $enchantability = $inputItem->getEnchantability();
            $enchantingPower = $enchantingPower + $random->nextRange(0, $enchantability >> 2) + $random->nextRange(0, $enchantability >> 2) + 1;
            $bonus = 1 + ($random->nextFloat() + $random->nextFloat() - 1) * 0.15;
            $enchantingPower = (int)round($enchantingPower * $bonus);

            $resultEnchantments = [];
            $availableEnchantments = self::getAvailableEnchantments($enchantingPower, $inputItem);

            $lastEnchantment = self::getRandomWeightedEnchantment($random, $availableEnchantments);
            if ($lastEnchantment !== null) {
                $resultEnchantments[] = $lastEnchantment;

                while ($random->nextFloat() <= ($enchantingPower + 1) / 50) {
                    $availableEnchantments = array_filter(
                        $availableEnchantments,
                        static function (EnchantmentInstance $e) use ($lastEnchantment) {
                            return $e->getType() !== $lastEnchantment->getType() &&
                                $e->getType()->isCompatibleWith($lastEnchantment->getType());
                        }
                    );

                    $lastEnchantment = self::getRandomWeightedEnchantment($random, $availableEnchantments);
                    if ($lastEnchantment === null) {
                        break;
                    }

                    $resultEnchantments[] = $lastEnchantment;
                    $enchantingPower >>= 1;
                }
            }

            return new EnchantingOption($requiredXpLevel, self::getRandomOptionName($random), $resultEnchantments);
        }

        /**
         * @return EnchantmentInstance[]
         */
        private static function getAvailableEnchantments(int $enchantingPower, Item $item): array
        {
            $list = [];

            foreach (AvailableEnchantmentRegistry::getInstance()->getPrimaryEnchantmentsForItem($item) as $enchantment) {
                for ($lvl = $enchantment->getMaxLevel(); $lvl > 0; $lvl--) {
                    if ($enchantingPower >= $enchantment->getMinEnchantingPower($lvl) &&
                        $enchantingPower <= $enchantment->getMaxEnchantingPower($lvl)
                    ) {
                        $list[] = new EnchantmentInstance($enchantment, $lvl);
                        break;
                    }
                }
            }

            return $list;
        }

        /**
         * @param EnchantmentInstance[] $enchantments
         */
        private static function getRandomWeightedEnchantment(Random $random, array $enchantments): ?EnchantmentInstance
        {
            if (count($enchantments) === 0) {
                return null;
            }

            $totalWeight = 0;
            foreach ($enchantments as $enchantment) {
                $totalWeight += $enchantment->getType()->getRarity();
            }

            $randomWeight = $random->nextRange(1, $totalWeight);

            foreach ($enchantments as $enchantment) {
                $randomWeight -= $enchantment->getType()->getRarity();

                if ($randomWeight <= 0) {
                    return $enchantment;
                }
            }

            return null;
        }

        private static function getRandomOptionName(Random $random): string
        {
            $name = "";
            for ($i = $random->nextRange(5, 15); $i > 0; $i--) {
                $name .= chr($random->nextRange(ord("a"), ord("z")) & 0xff);
            }

            return $name;
        }
    }

    /**
     * Registry of enchantments that can be applied to items during in-game enchanting.
     */
    final class AvailableEnchantmentRegistry
    {
        use SingletonTrait;

        /** @var Enchantment[] */
        private array $enchantments = [];

        /** @var string[][] */
        private array $primaryItemTags = [];

        /** @var string[][] */
        private array $secondaryItemTags = [];

        private function __construct()
        {
            $this->register(VanillaEnchantments::PROTECTION(), [Tags::ARMOR], []);
            $this->register(VanillaEnchantments::FIRE_PROTECTION(), [Tags::ARMOR], []);
            $this->register(VanillaEnchantments::FEATHER_FALLING(), [Tags::BOOTS], []);
            $this->register(VanillaEnchantments::BLAST_PROTECTION(), [Tags::ARMOR], []);
            $this->register(VanillaEnchantments::PROJECTILE_PROTECTION(), [Tags::ARMOR], []);
            $this->register(VanillaEnchantments::THORNS(), [Tags::CHESTPLATE], [Tags::HELMET, Tags::LEGGINGS, Tags::BOOTS]);
            $this->register(VanillaEnchantments::RESPIRATION(), [Tags::HELMET], []);
            $this->register(VanillaEnchantments::AQUA_AFFINITY(), [Tags::HELMET], []);
            $this->register(VanillaEnchantments::FROST_WALKER(), [], [Tags::BOOTS]);
            $this->register(VanillaEnchantments::SHARPNESS(), [Tags::SWORD, Tags::AXE], []);
            $this->register(VanillaEnchantments::KNOCKBACK(), [Tags::SWORD], []);
            $this->register(VanillaEnchantments::FIRE_ASPECT(), [Tags::SWORD], []);
            $this->register(VanillaEnchantments::EFFICIENCY(), [Tags::BLOCK_TOOLS], [Tags::SHEARS]);
            $this->register(VanillaEnchantments::FORTUNE(), [Tags::BLOCK_TOOLS], []);
            $this->register(VanillaEnchantments::SILK_TOUCH(), [Tags::BLOCK_TOOLS], [Tags::SHEARS]);
            $this->register(
                VanillaEnchantments::UNBREAKING(),
                [Tags::ARMOR, Tags::WEAPONS, Tags::FISHING_ROD],
                [Tags::SHEARS, Tags::FLINT_AND_STEEL, Tags::SHIELD, Tags::CARROT_ON_STICK, Tags::ELYTRA, Tags::BRUSH]
            );
            $this->register(VanillaEnchantments::POWER(), [Tags::BOW], []);
            $this->register(VanillaEnchantments::PUNCH(), [Tags::BOW], []);
            $this->register(VanillaEnchantments::FLAME(), [Tags::BOW], []);
            $this->register(VanillaEnchantments::INFINITY(), [Tags::BOW], []);
            $this->register(
                VanillaEnchantments::MENDING(),
                [],
                [Tags::ARMOR, Tags::WEAPONS, Tags::FISHING_ROD,
                    Tags::SHEARS, Tags::FLINT_AND_STEEL, Tags::SHIELD, Tags::CARROT_ON_STICK, Tags::ELYTRA, Tags::BRUSH]
            );
            $this->register(VanillaEnchantments::VANISHING(), [], [Tags::ALL]);
            $this->register(VanillaEnchantments::SWIFT_SNEAK(), [], [Tags::LEGGINGS]);
        }

        /**
         * @param string[] $primaryItemTags
         * @param string[] $secondaryItemTags
         */
        public function register(Enchantment $enchantment, array $primaryItemTags, array $secondaryItemTags): void
        {
            $this->enchantments[spl_object_id($enchantment)] = $enchantment;
            $this->setPrimaryItemTags($enchantment, $primaryItemTags);
            $this->setSecondaryItemTags($enchantment, $secondaryItemTags);
        }

        public function unregister(Enchantment $enchantment): void
        {
            unset($this->enchantments[spl_object_id($enchantment)]);
            unset($this->primaryItemTags[spl_object_id($enchantment)]);
            unset($this->secondaryItemTags[spl_object_id($enchantment)]);
        }

        public function unregisterAll(): void
        {
            $this->enchantments = [];
            $this->primaryItemTags = [];
            $this->secondaryItemTags = [];
        }

        public function isRegistered(Enchantment $enchantment): bool
        {
            return isset($this->enchantments[spl_object_id($enchantment)]);
        }

        /**
         * @return string[]
         */
        public function getPrimaryItemTags(Enchantment $enchantment): array
        {
            return $this->primaryItemTags[spl_object_id($enchantment)] ?? [];
        }

        /**
         * @param string[] $tags
         */
        public function setPrimaryItemTags(Enchantment $enchantment, array $tags): void
        {
            if (!$this->isRegistered($enchantment)) {
                throw new \LogicException("Cannot set primary item tags for non-registered enchantment");
            }
            $this->primaryItemTags[spl_object_id($enchantment)] = array_values($tags);
        }

        /**
         * @return string[]
         */
        public function getSecondaryItemTags(Enchantment $enchantment): array
        {
            return $this->secondaryItemTags[spl_object_id($enchantment)] ?? [];
        }

        /**
         * @param string[] $tags
         */
        public function setSecondaryItemTags(Enchantment $enchantment, array $tags): void
        {
            if (!$this->isRegistered($enchantment)) {
                throw new \LogicException("Cannot set secondary item tags for non-registered enchantment");
            }
            $this->secondaryItemTags[spl_object_id($enchantment)] = array_values($tags);
        }

        /**
         * @return Enchantment[]
         */
        public function getPrimaryEnchantmentsForItem(Item $item): array
        {
            $itemTags = $item->getEnchantmentTags();
            if (count($itemTags) === 0 || $item->hasEnchantments()) {
                return [];
            }

            return array_filter(
                $this->enchantments,
                fn(Enchantment $e) => TagRegistry::getInstance()->isTagArrayIntersection($this->getPrimaryItemTags($e), $itemTags)
            );
        }

        /**
         * @return Enchantment[]
         */
        public function getAllEnchantmentsForItem(Item $item): array
        {
            if (count($item->getEnchantmentTags()) === 0) {
                return [];
            }

            return array_filter(
                $this->enchantments,
                fn(Enchantment $enchantment) => $this->isAvailableForItem($enchantment, $item)
            );
        }

        public function isAvailableForItem(Enchantment $enchantment, Item $item): bool
        {
            $itemTags = $item->getEnchantmentTags();
            $tagRegistry = TagRegistry::getInstance();

            return $tagRegistry->isTagArrayIntersection($this->getPrimaryItemTags($enchantment), $itemTags) ||
                $tagRegistry->isTagArrayIntersection($this->getSecondaryItemTags($enchantment), $itemTags);
        }

        /**
         * @return Enchantment[]
         */
        public function getAll(): array
        {
            return $this->enchantments;
        }
    }

    /**
     * Handles parsing enchantments from strings.
     *
     * @phpstan-extends StringToTParser<Enchantment>
     */
    final class StringToEnchantmentParser extends StringToTParser
    {
        use SingletonTrait;

        private static function make(): self
        {
            $result = new self();

            $result->register("blast_protection", fn() => VanillaEnchantments::BLAST_PROTECTION());
            $result->register("efficiency", fn() => VanillaEnchantments::EFFICIENCY());
            $result->register("feather_falling", fn() => VanillaEnchantments::FEATHER_FALLING());
            $result->register("fire_aspect", fn() => VanillaEnchantments::FIRE_ASPECT());
            $result->register("fire_protection", fn() => VanillaEnchantments::FIRE_PROTECTION());
            $result->register("flame", fn() => VanillaEnchantments::FLAME());
            $result->register("fortune", fn() => VanillaEnchantments::FORTUNE());
            $result->register("frost_walker", fn() => VanillaEnchantments::FROST_WALKER());
            $result->register("infinity", fn() => VanillaEnchantments::INFINITY());
            $result->register("knockback", fn() => VanillaEnchantments::KNOCKBACK());
            $result->register("mending", fn() => VanillaEnchantments::MENDING());
            $result->register("power", fn() => VanillaEnchantments::POWER());
            $result->register("projectile_protection", fn() => VanillaEnchantments::PROJECTILE_PROTECTION());
            $result->register("protection", fn() => VanillaEnchantments::PROTECTION());
            $result->register("punch", fn() => VanillaEnchantments::PUNCH());
            $result->register("respiration", fn() => VanillaEnchantments::RESPIRATION());
            $result->register("aqua_affinity", fn() => VanillaEnchantments::AQUA_AFFINITY());
            $result->register("sharpness", fn() => VanillaEnchantments::SHARPNESS());
            $result->register("silk_touch", fn() => VanillaEnchantments::SILK_TOUCH());
            $result->register("swift_sneak", fn() => VanillaEnchantments::SWIFT_SNEAK());
            $result->register("thorns", fn() => VanillaEnchantments::THORNS());
            $result->register("unbreaking", fn() => VanillaEnchantments::UNBREAKING());
            $result->register("vanishing", fn() => VanillaEnchantments::VANISHING());

            // Bedrock aliases
            $result->register("depth_strider", fn() => VanillaEnchantments::DEPTH_STRIDER());
            $result->register("smite", fn() => VanillaEnchantments::SMITE());
            $result->register("bane_of_arthropods", fn() => VanillaEnchantments::BANE_OF_ARTHROPODS());
            $result->register("looting", fn() => VanillaEnchantments::LOOTING());
            $result->register("luck_of_the_sea", fn() => VanillaEnchantments::LUCK_OF_THE_SEA());
            $result->register("lure", fn() => VanillaEnchantments::LURE());
            $result->register("binding", fn() => VanillaEnchantments::BINDING());
            $result->register("impaling", fn() => VanillaEnchantments::IMPALING());
            $result->register("riptide", fn() => VanillaEnchantments::RIPTIDE());
            $result->register("loyalty", fn() => VanillaEnchantments::LOYALTY());
            $result->register("channeling", fn() => VanillaEnchantments::CHANNELING());
            $result->register("multishot", fn() => VanillaEnchantments::MULTISHOT());
            $result->register("piercing", fn() => VanillaEnchantments::PIERCING());
            $result->register("quick_charge", fn() => VanillaEnchantments::QUICK_CHARGE());
            $result->register("soul_speed", fn() => VanillaEnchantments::SOUL_SPEED());

            return $result;
        }

        public function parse(string $input): ?Enchantment
        {
            $result = parent::parse($input);
            if ($result !== null) {
                return $result;
            }

            $key = strtolower(str_replace([" ", "minecraft:"], ["_", ""], trim($input)));
            if (str_contains($key, ':')) {
                $key = substr($key, (int)strrpos($key, ':') + 1);
            }

            foreach (VanillaEnchantments::getAll() as $method => $enchantment) {
                if (strtolower($method) === $key || strtolower((string)$enchantment->getName()) === $key) {
                    return $enchantment;
                }
            }

            return null;
        }
    }

    /**
     * @internal
     * @phpstan-extends RegistrySource<Enchantment>
     */
    final class VanillaEnchantmentsInputs extends RegistrySource
    {
        /** @var array<string, Enchantment> */
        private static array $members = [];

        public function getTargetClassName(): string
        {
            return "VanillaEnchantments";
        }

        public function getTargetClassDocComment(): array
        {
            return ["Allows getting any vanilla enchantment implemented by PocketMine-MP"];
        }

        protected function setup(): void
        {
            if (self::$members !== []) {
                return;
            }

            self::register("PROTECTION", new ProtectionEnchantment(
                KnownTranslationFactory::enchantment_protect_all(),
                Rarity::COMMON,
                0,
                0,
                4,
                0.75,
                null,
                fn(int $level): int => 11 * ($level - 1) + 1,
                20
            ));
            self::register("FIRE_PROTECTION", new ProtectionEnchantment(
                KnownTranslationFactory::enchantment_protect_fire(),
                Rarity::UNCOMMON,
                0,
                0,
                4,
                1.25,
                [
                    EntityDamageEvent::CAUSE_FIRE,
                    EntityDamageEvent::CAUSE_FIRE_TICK,
                    EntityDamageEvent::CAUSE_LAVA,
                ],
                fn(int $level): int => 8 * ($level - 1) + 10,
                12
            ));
            self::register("FEATHER_FALLING", new ProtectionEnchantment(
                KnownTranslationFactory::enchantment_protect_fall(),
                Rarity::UNCOMMON,
                0,
                0,
                4,
                2.5,
                [
                    EntityDamageEvent::CAUSE_FALL,
                ],
                fn(int $level): int => 6 * ($level - 1) + 5,
                10
            ));
            self::register("BLAST_PROTECTION", new ProtectionEnchantment(
                KnownTranslationFactory::enchantment_protect_explosion(),
                Rarity::RARE,
                0,
                0,
                4,
                1.5,
                [
                    EntityDamageEvent::CAUSE_BLOCK_EXPLOSION,
                    EntityDamageEvent::CAUSE_ENTITY_EXPLOSION,
                ],
                fn(int $level): int => 8 * ($level - 1) + 5,
                12
            ));
            self::register("PROJECTILE_PROTECTION", new ProtectionEnchantment(
                KnownTranslationFactory::enchantment_protect_projectile(),
                Rarity::UNCOMMON,
                0,
                0,
                4,
                1.5,
                [
                    EntityDamageEvent::CAUSE_PROJECTILE,
                ],
                fn(int $level): int => 6 * ($level - 1) + 3,
                15
            ));
            self::register("THORNS", new Enchantment(
                KnownTranslationFactory::enchantment_thorns(),
                Rarity::MYTHIC,
                0,
                0,
                3,
                fn(int $level): int => 20 * ($level - 1) + 10,
                50
            ));
            self::register("RESPIRATION", new Enchantment(
                KnownTranslationFactory::enchantment_oxygen(),
                Rarity::RARE,
                0,
                0,
                3,
                fn(int $level): int => 10 * $level,
                30
            ));
            self::register("FROST_WALKER", new Enchantment(
                KnownTranslationFactory::enchantment_frostwalker(),
                Rarity::RARE,
                0,
                0,
                2,
                fn(int $level): int => 10 * $level,
                15
            ));
            self::register("AQUA_AFFINITY", new Enchantment(
                KnownTranslationFactory::enchantment_waterWorker(),
                Rarity::RARE,
                0,
                0,
                1,
                null,
                40
            ));
            self::register("SHARPNESS", new SharpnessEnchantment(
                KnownTranslationFactory::enchantment_damage_all(),
                Rarity::COMMON,
                0,
                0,
                5,
                fn(int $level): int => 11 * ($level - 1) + 1,
                20
            ));
            self::register("KNOCKBACK", new KnockbackEnchantment(
                KnownTranslationFactory::enchantment_knockback(),
                Rarity::UNCOMMON,
                0,
                0,
                2,
                fn(int $level): int => 20 * ($level - 1) + 5,
                50
            ));
            self::register("FIRE_ASPECT", new FireAspectEnchantment(
                KnownTranslationFactory::enchantment_fire(),
                Rarity::RARE,
                0,
                0,
                2,
                fn(int $level): int => 20 * ($level - 1) + 10,
                50
            ));
            self::register("EFFICIENCY", new Enchantment(
                KnownTranslationFactory::enchantment_digging(),
                Rarity::COMMON,
                0,
                0,
                5,
                fn(int $level): int => 10 * ($level - 1) + 1,
                50
            ));
            self::register("FORTUNE", new Enchantment(
                KnownTranslationFactory::enchantment_lootBonusDigger(),
                Rarity::RARE,
                0,
                0,
                3,
                fn(int $level): int => 9 * ($level - 1) + 15,
                50
            ));
            self::register("SILK_TOUCH", new Enchantment(
                KnownTranslationFactory::enchantment_untouching(),
                Rarity::MYTHIC,
                0,
                0,
                1,
                fn(int $level): int => 15,
                50
            ));
            self::register("UNBREAKING", new Enchantment(
                KnownTranslationFactory::enchantment_durability(),
                Rarity::UNCOMMON,
                0,
                0,
                3,
                fn(int $level): int => 8 * ($level - 1) + 5,
                50
            ));
            self::register("POWER", new Enchantment(
                KnownTranslationFactory::enchantment_arrowDamage(),
                Rarity::COMMON,
                0,
                0,
                5,
                fn(int $level): int => 10 * ($level - 1) + 1,
                15
            ));
            self::register("PUNCH", new Enchantment(
                KnownTranslationFactory::enchantment_arrowKnockback(),
                Rarity::RARE,
                0,
                0,
                2,
                fn(int $level): int => 20 * ($level - 1) + 12,
                25
            ));
            self::register("FLAME", new Enchantment(
                KnownTranslationFactory::enchantment_arrowFire(),
                Rarity::RARE,
                0,
                0,
                1,
                fn(int $level): int => 20,
                30
            ));
            self::register("INFINITY", new Enchantment(
                KnownTranslationFactory::enchantment_arrowInfinite(),
                Rarity::MYTHIC,
                0,
                0,
                1,
                fn(int $level): int => 20,
                30
            ));
            self::register("MENDING", new Enchantment(
                KnownTranslationFactory::enchantment_mending(),
                Rarity::RARE,
                0,
                0,
                1,
                fn(int $level): int => 25,
                50
            ));
            self::register("VANISHING", new Enchantment(
                KnownTranslationFactory::enchantment_curse_vanishing(),
                Rarity::MYTHIC,
                0,
                0,
                1,
                fn(int $level): int => 25,
                25
            ));
            self::register("SWIFT_SNEAK", new Enchantment(
                KnownTranslationFactory::enchantment_swift_sneak(),
                Rarity::MYTHIC,
                0,
                0,
                3,
                fn(int $level): int => 10 * $level,
                5
            ));

            // Additional Bedrock enchantments for full parity and NBT compatibility
            self::register("DEPTH_STRIDER", new Enchantment(
                KnownTranslationFactory::enchantment_waterWalker(),
                Rarity::RARE,
                0,
                0,
                3,
                fn(int $level): int => 10 * $level,
                15
            ));
            self::register("SMITE", new SharpnessEnchantment(
                KnownTranslationFactory::enchantment_damage_undead(),
                Rarity::UNCOMMON,
                0,
                0,
                5,
                fn(int $level): int => 8 * ($level - 1) + 5,
                20
            ));
            self::register("BANE_OF_ARTHROPODS", new SharpnessEnchantment(
                KnownTranslationFactory::enchantment_damage_arthropods(),
                Rarity::UNCOMMON,
                0,
                0,
                5,
                fn(int $level): int => 8 * ($level - 1) + 5,
                20
            ));
            self::register("LOOTING", new Enchantment(
                KnownTranslationFactory::enchantment_lootBonus(),
                Rarity::RARE,
                0,
                0,
                3,
                fn(int $level): int => 9 * ($level - 1) + 15,
                50
            ));
            self::register("LUCK_OF_THE_SEA", new Enchantment(
                KnownTranslationFactory::enchantment_lootBonusFishing(),
                Rarity::RARE,
                0,
                0,
                3,
                fn(int $level): int => 9 * ($level - 1) + 15,
                50
            ));
            self::register("LURE", new Enchantment(
                KnownTranslationFactory::enchantment_fishingSpeed(),
                Rarity::RARE,
                0,
                0,
                3,
                fn(int $level): int => 9 * ($level - 1) + 15,
                50
            ));
            self::register("BINDING", new Enchantment(
                KnownTranslationFactory::enchantment_binding_curse(),
                Rarity::UNCOMMON,
                0,
                0,
                1,
                fn(int $level): int => 25,
                25
            ));
            self::register("IMPALING", new Enchantment(
                KnownTranslationFactory::enchantment_tridentImpaling(),
                Rarity::UNCOMMON,
                0,
                0,
                5,
                fn(int $level): int => 8 * ($level - 1) + 1,
                20
            ));
            self::register("RIPTIDE", new Enchantment(
                KnownTranslationFactory::enchantment_tridentRiptide(),
                Rarity::RARE,
                0,
                0,
                3,
                fn(int $level): int => 7 * ($level - 1) + 17,
                50
            ));
            self::register("LOYALTY", new Enchantment(
                KnownTranslationFactory::enchantment_tridentLoyalty(),
                Rarity::UNCOMMON,
                0,
                0,
                3,
                fn(int $level): int => 7 * ($level - 1) + 12,
                50
            ));
            self::register("CHANNELING", new Enchantment(
                KnownTranslationFactory::enchantment_tridentChanneling(),
                Rarity::MYTHIC,
                0,
                0,
                1,
                fn(int $level): int => 25,
                25
            ));
            self::register("MULTISHOT", new Enchantment(
                KnownTranslationFactory::enchantment_crossbowMultishot(),
                Rarity::RARE,
                0,
                0,
                1,
                fn(int $level): int => 20,
                30
            ));
            self::register("PIERCING", new Enchantment(
                KnownTranslationFactory::enchantment_crossbowPiercing(),
                Rarity::COMMON,
                0,
                0,
                4,
                fn(int $level): int => 10 * ($level - 1) + 1,
                50
            ));
            self::register("QUICK_CHARGE", new Enchantment(
                KnownTranslationFactory::enchantment_crossbowQuickCharge(),
                Rarity::UNCOMMON,
                0,
                0,
                3,
                fn(int $level): int => 12 * ($level - 1) + 12,
                50
            ));
            self::register("SOUL_SPEED", new Enchantment(
                KnownTranslationFactory::enchantment_soul_speed(),
                Rarity::RARE,
                0,
                0,
                3,
                fn(int $level): int => 10 * $level,
                15
            ));
        }

        protected function register(string $name, Enchantment $member): void
        {
            $this->registerValue($name, $member);
        }
    }

    /**
     * Allows getting any vanilla enchantment implemented by PocketMine-MP.
     *
     * Generated from VanillaEnchantmentsInputs.
     */
    final class VanillaEnchantments
    {
        private static Enchantment $_mAQUA_AFFINITY;
        private static ProtectionEnchantment $_mBLAST_PROTECTION;
        private static Enchantment $_mEFFICIENCY;
        private static ProtectionEnchantment $_mFEATHER_FALLING;
        private static FireAspectEnchantment $_mFIRE_ASPECT;
        private static ProtectionEnchantment $_mFIRE_PROTECTION;
        private static Enchantment $_mFLAME;
        private static Enchantment $_mFORTUNE;
        private static Enchantment $_mFROST_WALKER;
        private static Enchantment $_mINFINITY;
        private static KnockbackEnchantment $_mKNOCKBACK;
        private static Enchantment $_mMENDING;
        private static Enchantment $_mPOWER;
        private static ProtectionEnchantment $_mPROJECTILE_PROTECTION;
        private static ProtectionEnchantment $_mPROTECTION;
        private static Enchantment $_mPUNCH;
        private static Enchantment $_mRESPIRATION;
        private static SharpnessEnchantment $_mSHARPNESS;
        private static Enchantment $_mSILK_TOUCH;
        private static Enchantment $_mSWIFT_SNEAK;
        private static Enchantment $_mTHORNS;
        private static Enchantment $_mUNBREAKING;
        private static Enchantment $_mVANISHING;

        /** @var array<string, Enchantment> */
        private static array $members;
        private static bool $initialized = false;

        private function __construct()
        {
            //NOOP
        }

        private static function init(): void
        {
            if (self::$initialized) {
                return;
            }
            self::$initialized = true;

            $source = new VanillaEnchantmentsInputs();
            foreach ($source->getAllValues() as $name => $value) {
                self::$members[mb_strtoupper($name)] = $value;
            }

            self::$_mAQUA_AFFINITY = self::$members['AQUA_AFFINITY'];
            self::$_mBLAST_PROTECTION = self::$members['BLAST_PROTECTION'];
            self::$_mEFFICIENCY = self::$members['EFFICIENCY'];
            self::$_mFEATHER_FALLING = self::$members['FEATHER_FALLING'];
            self::$_mFIRE_ASPECT = self::$members['FIRE_ASPECT'];
            self::$_mFIRE_PROTECTION = self::$members['FIRE_PROTECTION'];
            self::$_mFLAME = self::$members['FLAME'];
            self::$_mFORTUNE = self::$members['FORTUNE'];
            self::$_mFROST_WALKER = self::$members['FROST_WALKER'];
            self::$_mINFINITY = self::$members['INFINITY'];
            self::$_mKNOCKBACK = self::$members['KNOCKBACK'];
            self::$_mMENDING = self::$members['MENDING'];
            self::$_mPOWER = self::$members['POWER'];
            self::$_mPROJECTILE_PROTECTION = self::$members['PROJECTILE_PROTECTION'];
            self::$_mPROTECTION = self::$members['PROTECTION'];
            self::$_mPUNCH = self::$members['PUNCH'];
            self::$_mRESPIRATION = self::$members['RESPIRATION'];
            self::$_mSHARPNESS = self::$members['SHARPNESS'];
            self::$_mSILK_TOUCH = self::$members['SILK_TOUCH'];
            self::$_mSWIFT_SNEAK = self::$members['SWIFT_SNEAK'];
            self::$_mTHORNS = self::$members['THORNS'];
            self::$_mUNBREAKING = self::$members['UNBREAKING'];
            self::$_mVANISHING = self::$members['VANISHING'];
        }

        /**
         * @return array<string, Enchantment>
         */
        public static function getAll(): array
        {
            if (!isset(self::$members)) {
                self::init();
            }
            return self::$members;
        }

        public static function AQUA_AFFINITY(): Enchantment
        {
            if (!isset(self::$_mAQUA_AFFINITY)) {
                self::init();
            }
            return self::$_mAQUA_AFFINITY;
        }

        public static function BLAST_PROTECTION(): ProtectionEnchantment
        {
            if (!isset(self::$_mBLAST_PROTECTION)) {
                self::init();
            }
            return self::$_mBLAST_PROTECTION;
        }

        public static function EFFICIENCY(): Enchantment
        {
            if (!isset(self::$_mEFFICIENCY)) {
                self::init();
            }
            return self::$_mEFFICIENCY;
        }

        public static function FEATHER_FALLING(): ProtectionEnchantment
        {
            if (!isset(self::$_mFEATHER_FALLING)) {
                self::init();
            }
            return self::$_mFEATHER_FALLING;
        }

        public static function FIRE_ASPECT(): FireAspectEnchantment
        {
            if (!isset(self::$_mFIRE_ASPECT)) {
                self::init();
            }
            return self::$_mFIRE_ASPECT;
        }

        public static function FIRE_PROTECTION(): ProtectionEnchantment
        {
            if (!isset(self::$_mFIRE_PROTECTION)) {
                self::init();
            }
            return self::$_mFIRE_PROTECTION;
        }

        public static function FLAME(): Enchantment
        {
            if (!isset(self::$_mFLAME)) {
                self::init();
            }
            return self::$_mFLAME;
        }

        public static function FORTUNE(): Enchantment
        {
            if (!isset(self::$_mFORTUNE)) {
                self::init();
            }
            return self::$_mFORTUNE;
        }

        public static function FROST_WALKER(): Enchantment
        {
            if (!isset(self::$_mFROST_WALKER)) {
                self::init();
            }
            return self::$_mFROST_WALKER;
        }

        public static function INFINITY(): Enchantment
        {
            if (!isset(self::$_mINFINITY)) {
                self::init();
            }
            return self::$_mINFINITY;
        }

        public static function KNOCKBACK(): KnockbackEnchantment
        {
            if (!isset(self::$_mKNOCKBACK)) {
                self::init();
            }
            return self::$_mKNOCKBACK;
        }

        public static function MENDING(): Enchantment
        {
            if (!isset(self::$_mMENDING)) {
                self::init();
            }
            return self::$_mMENDING;
        }

        public static function POWER(): Enchantment
        {
            if (!isset(self::$_mPOWER)) {
                self::init();
            }
            return self::$_mPOWER;
        }

        public static function PROJECTILE_PROTECTION(): ProtectionEnchantment
        {
            if (!isset(self::$_mPROJECTILE_PROTECTION)) {
                self::init();
            }
            return self::$_mPROJECTILE_PROTECTION;
        }

        public static function PROTECTION(): ProtectionEnchantment
        {
            if (!isset(self::$_mPROTECTION)) {
                self::init();
            }
            return self::$_mPROTECTION;
        }

        public static function PUNCH(): Enchantment
        {
            if (!isset(self::$_mPUNCH)) {
                self::init();
            }
            return self::$_mPUNCH;
        }

        public static function RESPIRATION(): Enchantment
        {
            if (!isset(self::$_mRESPIRATION)) {
                self::init();
            }
            return self::$_mRESPIRATION;
        }

        public static function SHARPNESS(): SharpnessEnchantment
        {
            if (!isset(self::$_mSHARPNESS)) {
                self::init();
            }
            return self::$_mSHARPNESS;
        }

        public static function SILK_TOUCH(): Enchantment
        {
            if (!isset(self::$_mSILK_TOUCH)) {
                self::init();
            }
            return self::$_mSILK_TOUCH;
        }

        public static function SWIFT_SNEAK(): Enchantment
        {
            if (!isset(self::$_mSWIFT_SNEAK)) {
                self::init();
            }
            return self::$_mSWIFT_SNEAK;
        }

        public static function THORNS(): Enchantment
        {
            if (!isset(self::$_mTHORNS)) {
                self::init();
            }
            return self::$_mTHORNS;
        }

        public static function UNBREAKING(): Enchantment
        {
            if (!isset(self::$_mUNBREAKING)) {
                self::init();
            }
            return self::$_mUNBREAKING;
        }

        public static function VANISHING(): Enchantment
        {
            if (!isset(self::$_mVANISHING)) {
                self::init();
            }
            return self::$_mVANISHING;
        }

        // Additional Bedrock enchantments
        public static function DEPTH_STRIDER(): Enchantment
        {
            self::init();
            return self::$members['DEPTH_STRIDER'];
        }

        public static function SMITE(): Enchantment
        {
            self::init();
            return self::$members['SMITE'];
        }

        public static function BANE_OF_ARTHROPODS(): Enchantment
        {
            self::init();
            return self::$members['BANE_OF_ARTHROPODS'];
        }

        public static function LOOTING(): Enchantment
        {
            self::init();
            return self::$members['LOOTING'];
        }

        public static function LUCK_OF_THE_SEA(): Enchantment
        {
            self::init();
            return self::$members['LUCK_OF_THE_SEA'];
        }

        public static function LURE(): Enchantment
        {
            self::init();
            return self::$members['LURE'];
        }

        public static function BINDING(): Enchantment
        {
            self::init();
            return self::$members['BINDING'];
        }

        public static function IMPALING(): Enchantment
        {
            self::init();
            return self::$members['IMPALING'];
        }

        public static function RIPTIDE(): Enchantment
        {
            self::init();
            return self::$members['RIPTIDE'];
        }

        public static function LOYALTY(): Enchantment
        {
            self::init();
            return self::$members['LOYALTY'];
        }

        public static function CHANNELING(): Enchantment
        {
            self::init();
            return self::$members['CHANNELING'];
        }

        public static function MULTISHOT(): Enchantment
        {
            self::init();
            return self::$members['MULTISHOT'];
        }

        public static function PIERCING(): Enchantment
        {
            self::init();
            return self::$members['PIERCING'];
        }

        public static function QUICK_CHARGE(): Enchantment
        {
            self::init();
            return self::$members['QUICK_CHARGE'];
        }

        public static function SOUL_SPEED(): Enchantment
        {
            self::init();
            return self::$members['SOUL_SPEED'];
        }

        public static function __callStatic(string $name, array $args): Enchantment
        {
            self::init();
            $upper = mb_strtoupper($name);
            return self::$members[$upper] ?? throw new \InvalidArgumentException("No such enchantment $name");
        }
    }
}
