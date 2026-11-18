<?php

/*
 *
 *  ____            _        _   __  __ _                  __  __ ____
 * |  _ \ ___   ___| | _____| |_|  \/  (_)_ __   ___      |  \/  |  _ \
 * | |_) / _ \ / __| |/ / _ \ __| |\/| | | '_ \ / _ \_____| |\/| | |_) |
 * |  __/ (_) | (__|   <  __/ |_| |  | | | | | |  __/_____| |  | |  __/
 * |_|   \___/ \___|_|\_\___|\__|_|  |_|_|_| |_|\___|     |_|  |_|_|
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Lesser General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * @author PocketMine Team
 * @link http://www.pocketmine.net/
 *
 *
 */

declare(strict_types=1);

namespace pocketmine\utils {

    if (!class_exists(Limits::class, false)) {
        final class Limits
        {
            public const INT16_MIN = -32768;
            public const INT16_MAX = 32767;
            public const UINT16_MAX = 65535;
            public const INT32_MIN = -2147483648;
            public const INT32_MAX = 2147483647;
            public const UINT32_MAX = 4294967295;
            public const INT64_MIN = -9223372036854775807 - 1;
            public const INT64_MAX = 9223372036854775807;

            private function __construct() {}
        }
    }

    if (!trait_exists(LegacyEnumShimTrait::class, false)) {
        trait LegacyEnumShimTrait
        {
            public static function __callStatic(string $name, array $arguments): self
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
                static $result = null;
                if ($result === null) {
                    $result = [];
                    foreach (self::cases() as $case) {
                        $result[strtoupper($case->name)] = $case;
                    }
                }
                return $result;
            }

            public function name(): string
            {
                return $this->name;
            }

            public function id(): int
            {
                return spl_object_id($this);
            }

            public function equals(self $other): bool
            {
                return $this === $other;
            }
        }
    }
}

namespace pocketmine\data {

    if (!class_exists(SavedDataLoadingException::class, false)) {
        class SavedDataLoadingException extends \RuntimeException {}
    }
}

namespace pocketmine\world\sound {

    use pocketmine\math\Vector3;

    if (!interface_exists(Sound::class, false)) {
        interface Sound
        {
            /** @return array<int, mixed> */
            public function encode(Vector3 $pos): array;
        }
    }

    if (!class_exists(SoundBase::class, false)) {
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
        }
    }

    if (!class_exists(FireworkExplosionSound::class, false)) {
        class FireworkExplosionSound extends SoundBase {}
    }

    if (!class_exists(FireworkLargeExplosionSound::class, false)) {
        class FireworkLargeExplosionSound extends SoundBase {}
    }
}

namespace pocketmine\entity\object {

    use pocketmine\entity\Entity;
    use pocketmine\item\FireworkRocketExplosion;
    use pocketmine\world\Location;

    if (!class_exists(FireworkRocket::class, false)) {
        class FireworkRocket
        {
            protected ?Location $location = null;
            protected ?\pocketmine\math\Vector3 $motion = null;
            public function getLocation(): ?Location { return $this->location; }
            public function spawnToAll(): void {}

            public const NETWORK_TYPE_ID = 'minecraft:fireworks_rocket';

            protected int $lifeTime = 0;

            /** @var FireworkRocketExplosion[] */
            protected array $explosions = [];

            /**
             * @param FireworkRocketExplosion[] $explosions
             */
            public function __construct(Location $location, int $lifeTime = 0, array $explosions = [])
            {
                $this->location = $location;
                $this->lifeTime = $lifeTime;
                $this->explosions = $explosions;
            }

            public function getLifeTime(): int
            {
                return $this->lifeTime;
            }

            public function setLifeTime(int $lifeTime): void
            {
                $this->lifeTime = $lifeTime;
            }

            /**
             * @return FireworkRocketExplosion[]
             */
            public function getExplosions(): array
            {
                return $this->explosions;
            }

            /**
             * @param FireworkRocketExplosion[] $explosions
             */
            public function setExplosions(array $explosions): void
            {
                $this->explosions = $explosions;
            }
        }
    }
}

namespace pocketmine\data\bedrock {

    use pocketmine\block\utils\DyeColor;
    use pocketmine\item\enchantment\Enchantment;
    use pocketmine\item\FireworkRocketType;
    use pocketmine\utils\SingletonTrait;
    use function array_key_exists;
    use function class_exists;
    use function enum_exists;
    use function spl_object_id;

    if (!trait_exists(IntSaveIdMapTrait::class, false)) {
        trait IntSaveIdMapTrait
        {
            private array $idToEnum = [];
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

            public function toId(object $enum): int
            {
                $runtimeId = $this->getRuntimeId($enum);
                if (!array_key_exists($runtimeId, $this->enumToId)) {
                    throw new \InvalidArgumentException("Object does not have a mapped save ID");
                }
                return $this->enumToId[$runtimeId];
            }
        }
    }

    if (!class_exists(DyeColorIdMap::class, false)) {
        final class DyeColorIdMap
        {
            private static ?self $instance = null;

            use IntSaveIdMapTrait {
                register as registerInt;
            }

            public static function getInstance(): self
            {
                return self::$instance ??= new self();
            }

            private function __construct()
            {
                $this->initIfEmpty();
            }

            private function initIfEmpty(): void
            {
                if (empty($this->idToEnum) && enum_exists(DyeColor::class)) {
                    foreach (DyeColor::cases() as $case) {
                        $colorId = match ($case) {
                            DyeColor::WHITE => 0,
                            DyeColor::ORANGE => 1,
                            DyeColor::MAGENTA => 2,
                            DyeColor::LIGHT_BLUE => 3,
                            DyeColor::YELLOW => 4,
                            DyeColor::LIME => 5,
                            DyeColor::PINK => 6,
                            DyeColor::GRAY => 7,
                            DyeColor::LIGHT_GRAY => 8,
                            DyeColor::CYAN => 9,
                            DyeColor::PURPLE => 10,
                            DyeColor::BLUE => 11,
                            DyeColor::BROWN => 12,
                            DyeColor::GREEN => 13,
                            DyeColor::RED => 14,
                            DyeColor::BLACK => 15,
                        };
                        $this->registerInt($colorId, $case);
                    }
                }
            }

            public function toInvertedId(DyeColor $color): int
            {
                $this->initIfEmpty();
                return ~$this->toId($color) & 0xf;
            }

            public function fromInvertedId(int $id): ?DyeColor
            {
                $this->initIfEmpty();
                /** @var DyeColor|null */
                return $this->fromId(~$id & 0xf);
            }
        }
    }

    if (!class_exists(EnchantmentIdMap::class, false)) {
        final class EnchantmentIdMap
        {
            use SingletonTrait;
            use IntSaveIdMapTrait;

            private function __construct() {}

            public function toId(Enchantment $enchantment): ?int
            {
                if (class_exists(\pocketmine\data\bedrock\enchantment\EnchantmentIdMap::class)) {
                    return \pocketmine\data\bedrock\enchantment\EnchantmentIdMap::toId($enchantment);
                }
                $runtimeId = spl_object_id($enchantment);
                return $this->enumToId[$runtimeId] ?? null;
            }

            public function fromId(int $id): ?Enchantment
            {
                if (class_exists(\pocketmine\data\bedrock\enchantment\EnchantmentIdMap::class)) {
                    return \pocketmine\data\bedrock\enchantment\EnchantmentIdMap::fromId($id);
                }
                return $this->idToEnum[$id] ?? null;
            }
        }
    }

    if (!class_exists(FireworkRocketTypeIds::class, false)) {
        final class FireworkRocketTypeIds
        {
            public const SMALL_BALL = 0;
            public const LARGE_BALL = 1;
            public const STAR = 2;
            public const CREEPER = 3;
            public const BURST = 4;

            // Legacy PM4 aliases
            public const SMALL_SPHERE = 0;
            public const HUGE_SPHERE = 1;
            public const CREEPER_HEAD = 3;
        }
    }

    if (!class_exists(FireworkRocketTypeIdMap::class, false)) {
        final class FireworkRocketTypeIdMap
        {
            use SingletonTrait;
            use IntSaveIdMapTrait;

            private function __construct()
            {
                $this->initIfEmpty();
            }

            private function initIfEmpty(): void
            {
                if (empty($this->idToEnum) && enum_exists(FireworkRocketType::class)) {
                    foreach (FireworkRocketType::cases() as $case) {
                        $id = match ($case) {
                            FireworkRocketType::SMALL_SPHERE => FireworkRocketTypeIds::SMALL_SPHERE,
                            FireworkRocketType::HUGE_SPHERE => FireworkRocketTypeIds::HUGE_SPHERE,
                            FireworkRocketType::STAR => FireworkRocketTypeIds::STAR,
                            FireworkRocketType::CREEPER_HEAD => FireworkRocketTypeIds::CREEPER_HEAD,
                            FireworkRocketType::BURST => FireworkRocketTypeIds::BURST,
                        };
                        $this->register($id, $case);
                    }
                }
            }

            public function fromId(int $id): ?FireworkRocketType
            {
                $this->initIfEmpty();
                /** @var FireworkRocketType|null */
                return $this->idToEnum[$id] ?? null;
            }

            public function toId(FireworkRocketType $type): int
            {
                $this->initIfEmpty();
                $runtimeId = spl_object_id($type);
                if (!array_key_exists($runtimeId, $this->enumToId)) {
                    throw new \InvalidArgumentException("Firework rocket type does not have a mapped save ID");
                }
                return $this->enumToId[$runtimeId];
            }
        }
    }
}

namespace pocketmine\item {

    use pocketmine\block\Block;
    use pocketmine\block\utils\DyeColor;
    use pocketmine\color\Color;
    use pocketmine\data\bedrock\DyeColorIdMap;
    use pocketmine\data\bedrock\EnchantmentIdMap;
    use pocketmine\data\bedrock\FireworkRocketTypeIdMap;
    use pocketmine\data\SavedDataLoadingException;
    use pocketmine\entity\Location;
    use pocketmine\entity\object\FireworkRocket as FireworkEntity;
    use pocketmine\item\enchantment\Enchantment;
    use pocketmine\item\enchantment\EnchantmentInstance;
    use pocketmine\math\Vector3;
    use pocketmine\nbt\NBT;
    use pocketmine\nbt\tag\ByteArrayTag;
    use pocketmine\nbt\tag\ByteTag;
    use pocketmine\nbt\tag\CompoundTag;
    use pocketmine\nbt\tag\IntTag;
    use pocketmine\nbt\tag\ListTag;
    use pocketmine\nbt\tag\ShortTag;
    use pocketmine\nbt\tag\StringTag;
    use pocketmine\player\Player;
    use pocketmine\utils\Binary;
    use pocketmine\utils\LegacyEnumShimTrait;
    use pocketmine\utils\Limits;
    use pocketmine\utils\Utils;
    use pocketmine\world\sound\FireworkExplosionSound;
    use pocketmine\world\sound\FireworkLargeExplosionSound;
    use pocketmine\world\sound\Sound;
    use function array_key_first;
    use function array_push;
    use function array_slice;
    use function array_values;
    use function chr;
    use function class_exists;
    use function count;
    use function function_exists;
    use function mb_scrub;
    use function method_exists;
    use function mt_rand;
    use function ord;
    use function spl_object_id;
    use function sprintf;
    use function strlen;
    use function strtoupper;

    class Book extends Item
    {
        public function getEnchantability(): int
        {
            return 1;
        }
    }

    class WritableBookPage
    {
        public const PAGE_LENGTH_HARD_LIMIT_BYTES = Limits::INT16_MAX;
        public const PHOTO_NAME_LENGTH_HARD_LIMIT_BYTES = Limits::INT16_MAX;

        private string $text;
        private string $photoName;

        /**
         * @throws \InvalidArgumentException
         */
        private static function checkLength(string $string, string $name, int $maxLength): void
        {
            if (strlen($string) > $maxLength) {
                throw new \InvalidArgumentException(sprintf("%s must be at most %d bytes, but have %d bytes", $name, $maxLength, strlen($string)));
            }
        }

        public function __construct(string $text, string $photoName = "")
        {
            self::checkLength($text, "Text", self::PAGE_LENGTH_HARD_LIMIT_BYTES);
            self::checkLength($photoName, "Photo name", self::PHOTO_NAME_LENGTH_HARD_LIMIT_BYTES);
            if (class_exists(Utils::class) && method_exists(Utils::class, 'checkUTF8')) {
                Utils::checkUTF8($text);
            }
            $this->text = $text;
            $this->photoName = $photoName;
        }

        public function getText(): string
        {
            return $this->text;
        }

        public function getPhotoName(): string
        {
            return $this->photoName;
        }
    }

    abstract class WritableBookBase extends Item
    {
        public const TAG_PAGES = "pages"; //TAG_List<TAG_Compound>
        public const TAG_PAGE_TEXT = "text"; //TAG_String
        public const TAG_PAGE_PHOTONAME = "photoname"; //TAG_String

        /**
         * @var WritableBookPage[]
         * @phpstan-var list<WritableBookPage>
         */
        private array $pages = [];

        public function __construct(ItemIdentifier $identifier, string $name)
        {
            parent::__construct($identifier, $name);
        }

        /**
         * Returns whether the given page exists in this book.
         */
        public function pageExists(int $pageId): bool
        {
            return isset($this->pages[$pageId]);
        }

        /**
         * Returns a string containing the content of a page (which could be empty).
         *
         * @throws \OutOfRangeException if requesting a nonexisting page
         */
        public function getPageText(int $pageId): string
        {
            if (!isset($this->pages[$pageId])) {
                throw new \OutOfRangeException("Page \"$pageId\" does not exist");
            }
            return $this->pages[$pageId]->getText();
        }

        /**
         * Sets the text of a page in the book. Adds the page if the page does not yet exist.
         *
         * @return $this
         */
        public function setPageText(int $pageId, string $pageText): self
        {
            if (!$this->pageExists($pageId)) {
                $this->addPage($pageId);
            }

            $this->pages[$pageId] = new WritableBookPage($pageText);
            return $this;
        }

        /**
         * Returns the WritableBookPage object for the given page ID.
         *
         * @throws \OutOfRangeException if requesting a nonexisting page
         */
        public function getPage(int $pageId): WritableBookPage
        {
            if (!isset($this->pages[$pageId])) {
                throw new \OutOfRangeException("Page \"$pageId\" does not exist");
            }
            return $this->pages[$pageId];
        }

        /**
         * Sets a page in the book using a WritableBookPage object. Adds the page if it does not yet exist.
         *
         * @return $this
         */
        public function setPage(int $pageId, WritableBookPage $page): self
        {
            if (!$this->pageExists($pageId)) {
                $this->addPage($pageId);
            }
            $this->pages[$pageId] = $page;
            return $this;
        }

        /**
         * Adds a new page with the given page ID.
         * Creates a new page for every page between the given ID and existing pages that doesn't yet exist.
         *
         * @return $this
         */
        public function addPage(int $pageId): self
        {
            if ($pageId < 0) {
                throw new \InvalidArgumentException("Page number \"$pageId\" is out of range");
            }

            for ($current = count($this->pages); $current <= $pageId; $current++) {
                $this->pages[] = new WritableBookPage("");
            }
            return $this;
        }

        /**
         * Deletes an existing page with the given page ID.
         *
         * @return $this
         */
        public function deletePage(int $pageId): self
        {
            $newPages = $this->pages;
            unset($newPages[$pageId]);
            $this->pages = array_values($newPages);
            return $this;
        }

        /**
         * Inserts a new page with the given text and moves other pages upwards.
         *
         * @return $this
         */
        public function insertPage(int $pageId, string $pageText = ""): self
        {
            if ($pageId < 0 || $pageId > count($this->pages)) {
                throw new \InvalidArgumentException("Page ID must not be negative");
            }
            $newPages = array_slice($this->pages, 0, $pageId);
            $newPages[] = new WritableBookPage($pageText);
            array_push($newPages, ...array_slice($this->pages, $pageId));
            $this->pages = $newPages;
            return $this;
        }

        /**
         * Switches the text of two pages with each other.
         *
         * @return bool indicating success
         * @throws \OutOfRangeException if either of the pages does not exist
         */
        public function swapPages(int $pageId1, int $pageId2): bool
        {
            $pageContents1 = $this->getPageText($pageId1);
            $pageContents2 = $this->getPageText($pageId2);
            $this->setPageText($pageId1, $pageContents2);
            $this->setPageText($pageId2, $pageContents1);

            return true;
        }

        public function getMaxStackSize(): int
        {
            return 1;
        }

        /**
         * Returns an array containing all pages of this book.
         *
         * @return WritableBookPage[]
         */
        public function getPages(): array
        {
            return $this->pages;
        }

        /**
         * @param WritableBookPage[] $pages
         *
         * @return $this
         */
        public function setPages(array $pages): self
        {
            $this->pages = array_values($pages);
            return $this;
        }

        protected function deserializeCompoundTag(CompoundTag $tag): void
        {
            $hasKeepOnDeath = $tag->getTag(self::TAG_KEEP_ON_DEATH) !== null;
            if (!$hasKeepOnDeath) {
                $tag->setByte(self::TAG_KEEP_ON_DEATH, 0);
            }
            try {
                parent::deserializeCompoundTag($tag);
            } finally {
                if (!$hasKeepOnDeath) {
                    $tag->removeTag(self::TAG_KEEP_ON_DEATH);
                }
            }

            $this->pages = [];

            $pages = $tag->getTag(self::TAG_PAGES);
            if ($pages instanceof ListTag) {
                foreach ($pages as $page) {
                    if ($page instanceof CompoundTag) {
                        $textTag = $page->getTag(self::TAG_PAGE_TEXT);
                        $photoTag = $page->getTag(self::TAG_PAGE_PHOTONAME);
                        $text = $textTag instanceof StringTag ? $textTag->getValue() : "";
                        $photo = $photoTag instanceof StringTag ? $photoTag->getValue() : "";
                        $scrubbed = function_exists('mb_scrub') ? mb_scrub($text, 'UTF-8') : $text;
                        $this->pages[] = new WritableBookPage($scrubbed, $photo);
                    } elseif ($page instanceof StringTag) {
                        $text = $page->getValue();
                        $scrubbed = function_exists('mb_scrub') ? mb_scrub($text, 'UTF-8') : $text;
                        $this->pages[] = new WritableBookPage($scrubbed);
                    }
                }
            }
        }

        protected function serializeCompoundTag(CompoundTag $tag): void
        {
            parent::serializeCompoundTag($tag);
            if (count($this->pages) > 0) {
                $pages = new ListTag([], NBT::TAG_Compound);
                foreach ($this->pages as $page) {
                    $pageTag = CompoundTag::create()
                        ->setString(self::TAG_PAGE_TEXT, $page->getText())
                        ->setString(self::TAG_PAGE_PHOTONAME, $page->getPhotoName());
                    $pages->push($pageTag);
                }
                $tag->setTag(self::TAG_PAGES, $pages);
            } else {
                $tag->removeTag(self::TAG_PAGES);
            }
        }
    }

    class WritableBook extends WritableBookBase
    {
    }

    class WrittenBook extends WritableBookBase
    {
        public const GENERATION_ORIGINAL = 0;
        public const GENERATION_COPY = 1;
        public const GENERATION_COPY_OF_COPY = 2;
        public const GENERATION_TATTERED = 3;

        public const TAG_GENERATION = "generation"; //TAG_Int
        public const TAG_AUTHOR = "author"; //TAG_String
        public const TAG_TITLE = "title"; //TAG_String

        private int $generation = self::GENERATION_ORIGINAL;
        private string $author = "";
        private string $title = "";

        public function getMaxStackSize(): int
        {
            return 16;
        }

        /**
         * Returns the generation of the book.
         * Generations higher than 1 can not be copied.
         */
        public function getGeneration(): int
        {
            return $this->generation;
        }

        /**
         * Sets the generation of a book.
         *
         * @return $this
         */
        public function setGeneration(int $generation): self
        {
            if ($generation < 0 || $generation > 3) {
                throw new \InvalidArgumentException("Generation \"$generation\" is out of range");
            }

            $this->generation = $generation;
            return $this;
        }

        /**
         * Returns the author of this book.
         */
        public function getAuthor(): string
        {
            return $this->author;
        }

        /**
         * Sets the author of this book.
         *
         * @return $this
         */
        public function setAuthor(string $authorName): self
        {
            if (strlen($authorName) > Limits::INT16_MAX) {
                throw new \InvalidArgumentException(sprintf("Author must be at most %d bytes, but have %d bytes", Limits::INT16_MAX, strlen($authorName)));
            }
            if (class_exists(Utils::class) && method_exists(Utils::class, 'checkUTF8')) {
                Utils::checkUTF8($authorName);
            }
            $this->author = $authorName;
            return $this;
        }

        /**
         * Returns the title of this book.
         */
        public function getTitle(): string
        {
            return $this->title;
        }

        /**
         * Sets the title of this book.
         *
         * @return $this
         */
        public function setTitle(string $title): self
        {
            if (strlen($title) > Limits::INT16_MAX) {
                throw new \InvalidArgumentException(sprintf("Title must be at most %d bytes, but have %d bytes", Limits::INT16_MAX, strlen($title)));
            }
            if (class_exists(Utils::class) && method_exists(Utils::class, 'checkUTF8')) {
                Utils::checkUTF8($title);
            }
            $this->title = $title;
            return $this;
        }

        protected function deserializeCompoundTag(CompoundTag $tag): void
        {
            parent::deserializeCompoundTag($tag);

            $genTag = $tag->getTag(self::TAG_GENERATION);
            if ($genTag instanceof IntTag) {
                $this->generation = $genTag->getValue();
            }

            $authorTag = $tag->getTag(self::TAG_AUTHOR);
            if ($authorTag instanceof StringTag) {
                $this->author = $authorTag->getValue();
            }

            $titleTag = $tag->getTag(self::TAG_TITLE);
            if ($titleTag instanceof StringTag) {
                $this->title = $titleTag->getValue();
            }
        }

        protected function serializeCompoundTag(CompoundTag $tag): void
        {
            parent::serializeCompoundTag($tag);
            $tag->setInt(self::TAG_GENERATION, $this->generation);
            $tag->setString(self::TAG_AUTHOR, $this->author);
            $tag->setString(self::TAG_TITLE, $this->title);
        }
    }

    class EnchantedBook extends Item
    {
        public const TAG_STORED_ENCHANTMENTS = "StoredEnchantments";
        public const TAG_ENCH_ID = "id"; //TAG_Short
        public const TAG_ENCH_LVL = "lvl"; //TAG_Short

        /** @var array<int, EnchantmentInstance> */
        private array $customEnchantments = [];

        public function getMaxStackSize(): int
        {
            return 1;
        }

        public function addCustomEnchantment(EnchantmentInstance $enchantment): self
        {
            $this->customEnchantments[spl_object_id($enchantment->getType())] = $enchantment;
            return $this;
        }

        /**
         * @return EnchantmentInstance[]
         */
        public function getCustomEnchantments(): array
        {
            return array_values($this->customEnchantments);
        }

        public function hasCustomEnchantments(): bool
        {
            return count($this->customEnchantments) > 0;
        }

        public function hasCustomEnchantment(Enchantment $enchantment, int $level = -1): bool
        {
            $id = spl_object_id($enchantment);
            return isset($this->customEnchantments[$id]) && ($level === -1 || $this->customEnchantments[$id]->getLevel() === $level);
        }

        public function getCustomEnchantment(Enchantment $enchantment): ?EnchantmentInstance
        {
            return $this->customEnchantments[spl_object_id($enchantment)] ?? null;
        }

        public function removeCustomEnchantment(Enchantment $enchantment, int $level = -1): self
        {
            $instance = $this->getCustomEnchantment($enchantment);
            if ($instance !== null && ($level === -1 || $instance->getLevel() === $level)) {
                unset($this->customEnchantments[spl_object_id($enchantment)]);
            }
            return $this;
        }

        public function removeCustomEnchantments(): self
        {
            $this->customEnchantments = [];
            return $this;
        }

        // Stored enchantments alias methods
        public function addStoredEnchantment(EnchantmentInstance $enchantment): self
        {
            return $this->addCustomEnchantment($enchantment);
        }

        /**
         * @return EnchantmentInstance[]
         */
        public function getStoredEnchantments(): array
        {
            return $this->getCustomEnchantments();
        }

        public function hasStoredEnchantments(): bool
        {
            return $this->hasCustomEnchantments();
        }

        public function hasStoredEnchantment(Enchantment $enchantment, int $level = -1): bool
        {
            return $this->hasCustomEnchantment($enchantment, $level);
        }

        public function getStoredEnchantment(Enchantment $enchantment): ?EnchantmentInstance
        {
            return $this->getCustomEnchantment($enchantment);
        }

        public function removeStoredEnchantment(Enchantment $enchantment, int $level = -1): self
        {
            return $this->removeCustomEnchantment($enchantment, $level);
        }

        protected function deserializeCompoundTag(CompoundTag $tag): void
        {
            $hasKeepOnDeath = $tag->getTag(self::TAG_KEEP_ON_DEATH) !== null;
            if (!$hasKeepOnDeath) {
                $tag->setByte(self::TAG_KEEP_ON_DEATH, 0);
            }
            try {
                parent::deserializeCompoundTag($tag);
            } finally {
                if (!$hasKeepOnDeath) {
                    $tag->removeTag(self::TAG_KEEP_ON_DEATH);
                }
            }

            $this->customEnchantments = [];

            $stored = $tag->getTag(self::TAG_STORED_ENCHANTMENTS);
            if ($stored instanceof ListTag) {
                $enchantmentIdMap = EnchantmentIdMap::getInstance();
                foreach ($stored as $entry) {
                    if ($entry instanceof CompoundTag) {
                        $idTag = $entry->getTag(self::TAG_ENCH_ID);
                        $lvlTag = $entry->getTag(self::TAG_ENCH_LVL);
                        $id = ($idTag instanceof ShortTag || $idTag instanceof IntTag) ? $idTag->getValue() : -1;
                        $level = ($lvlTag instanceof ShortTag || $lvlTag instanceof IntTag) ? $lvlTag->getValue() : 0;
                        if ($level <= 0) {
                            continue;
                        }
                        $type = $enchantmentIdMap->fromId($id);
                        if ($type !== null) {
                            $this->addCustomEnchantment(new EnchantmentInstance($type, $level));
                        }
                    }
                }
            }
        }

        protected function serializeCompoundTag(CompoundTag $tag): void
        {
            parent::serializeCompoundTag($tag);
            if (count($this->customEnchantments) > 0) {
                $storedTag = new ListTag([], NBT::TAG_Compound);
                $enchantmentIdMap = EnchantmentIdMap::getInstance();
                foreach ($this->customEnchantments as $instance) {
                    $id = $enchantmentIdMap->toId($instance->getType());
                    if ($id !== null) {
                        $storedTag->push(CompoundTag::create()
                            ->setShort(self::TAG_ENCH_ID, $id)
                            ->setShort(self::TAG_ENCH_LVL, $instance->getLevel())
                        );
                    }
                }
                if ($storedTag->count() > 0) {
                    $tag->setTag(self::TAG_STORED_ENCHANTMENTS, $storedTag);
                } else {
                    $tag->removeTag(self::TAG_STORED_ENCHANTMENTS);
                }
            } else {
                $tag->removeTag(self::TAG_STORED_ENCHANTMENTS);
            }
        }
    }

    enum FireworkRocketType
    {
        use LegacyEnumShimTrait;

        case SMALL_SPHERE;
        case HUGE_SPHERE;
        case STAR;
        case CREEPER_HEAD;
        case BURST;

        public const SMALL_BALL = self::SMALL_SPHERE;
        public const LARGE_BALL = self::HUGE_SPHERE;
        public const CREEPER = self::CREEPER_HEAD;

        public static function __callStatic(string $name, array $arguments): self
        {
            if (count($arguments) > 0) {
                throw new \ArgumentCountError("Expected exactly 0 arguments, " . count($arguments) . " passed");
            }
            $upper = strtoupper($name);
            return match ($upper) {
                'SMALL_BALL' => self::SMALL_SPHERE,
                'LARGE_BALL' => self::HUGE_SPHERE,
                'CREEPER' => self::CREEPER_HEAD,
                default => self::getAll()[$upper] ?? throw new \InvalidArgumentException("No such case " . $name),
            };
        }

        public function getExplosionSound(): Sound
        {
            return match ($this) {
                self::SMALL_SPHERE,
                self::STAR,
                self::CREEPER_HEAD,
                self::BURST => new FireworkExplosionSound(),
                self::HUGE_SPHERE => new FireworkLargeExplosionSound(),
            };
        }
    }

    class FireworkRocketExplosion
    {
        public const TAG_TYPE = "FireworkType"; //TAG_Byte
        public const TAG_COLORS = "FireworkColor"; //TAG_ByteArray
        public const TAG_FADE_COLORS = "FireworkFade"; //TAG_ByteArray
        public const TAG_TWINKLE = "FireworkFlicker"; //TAG_Byte
        public const TAG_TRAIL = "FireworkTrail"; //TAG_Byte

        /**
         * @param DyeColor[]|Color[] $colors
         * @param DyeColor[]|Color[] $fadeColors
         */
        public function __construct(
            protected FireworkRocketType $type,
            protected array $colors,
            protected array $fadeColors = [],
            protected bool $twinkle = false,
            protected bool $trail = false
        ) {
            if (count($colors) === 0) {
                throw new \InvalidArgumentException("Colors list cannot be empty");
            }
        }

        /**
         * @throws SavedDataLoadingException
         */
        public static function fromCompoundTag(CompoundTag $tag): self
        {
            $colorsTag = $tag->getTag(self::TAG_COLORS);
            $colorsBytes = $colorsTag instanceof ByteArrayTag ? $colorsTag->getValue() : "";
            $colors = self::decodeColors($colorsBytes);
            if (count($colors) === 0) {
                throw new SavedDataLoadingException("Colors list cannot be empty");
            }

            $typeTag = $tag->getTag(self::TAG_TYPE);
            $typeId = $typeTag instanceof ByteTag ? $typeTag->getValue() : 0;
            $type = FireworkRocketTypeIdMap::getInstance()->fromId($typeId);
            if ($type === null) {
                throw new SavedDataLoadingException("Invalid firework type: $typeId");
            }

            $fadeTag = $tag->getTag(self::TAG_FADE_COLORS);
            $fadeBytes = $fadeTag instanceof ByteArrayTag ? $fadeTag->getValue() : "";
            $fadeColors = $fadeBytes !== "" ? self::decodeColors($fadeBytes) : [];

            $twinkleTag = $tag->getTag(self::TAG_TWINKLE);
            $twinkle = $twinkleTag instanceof ByteTag ? ($twinkleTag->getValue() !== 0) : false;

            $trailTag = $tag->getTag(self::TAG_TRAIL);
            $trail = $trailTag instanceof ByteTag ? ($trailTag->getValue() !== 0) : false;

            return new self(
                $type,
                $colors,
                $fadeColors,
                $twinkle,
                $trail
            );
        }

        /**
         * @return DyeColor[]
         * @throws SavedDataLoadingException
         */
        protected static function decodeColors(string $colorsBytes): array
        {
            $colors = [];
            $dyeColorIdMap = DyeColorIdMap::getInstance();
            for ($i = 0, $len = strlen($colorsBytes); $i < $len; $i++) {
                $colorByte = ord($colorsBytes[$i]);
                $color = $dyeColorIdMap->fromInvertedId($colorByte);
                if ($color !== null) {
                    $colors[] = $color;
                } else {
                    throw new SavedDataLoadingException("Unknown color $colorByte");
                }
            }
            return $colors;
        }

        /**
         * @param DyeColor[]|Color[] $colors
         */
        protected static function encodeColors(array $colors): string
        {
            $colorsBytes = "";
            $dyeColorIdMap = DyeColorIdMap::getInstance();
            foreach ($colors as $color) {
                if ($color instanceof DyeColor) {
                    $colorsBytes .= chr($dyeColorIdMap->toInvertedId($color) & 0xff);
                }
            }
            return $colorsBytes;
        }

        public function getType(): FireworkRocketType
        {
            return $this->type;
        }

        /**
         * Returns the colors of the particles.
         *
         * @return DyeColor[]|Color[]
         */
        public function getColors(): array
        {
            return $this->colors;
        }

        /**
         * Returns the RGB Color objects of the particles.
         *
         * @return Color[]
         */
        public function getRGBColors(): array
        {
            $rgb = [];
            foreach ($this->colors as $color) {
                if ($color instanceof DyeColor) {
                    $rgb[] = $color->getRgbValue();
                } elseif ($color instanceof Color) {
                    $rgb[] = $color;
                }
            }
            return $rgb;
        }

        /**
         * Returns the flash color of the explosion.
         */
        public function getFlashColor(): DyeColor
        {
            $first = $this->colors[array_key_first($this->colors)];
            if ($first instanceof DyeColor) {
                return $first;
            }
            return DyeColor::WHITE;
        }

        /**
         * Returns the mixture of colors from the explosion.
         */
        public function getColorMix(): Color
        {
            /** @var Color[] $rgbList */
            $rgbList = [];
            foreach ($this->colors as $c) {
                if ($c instanceof DyeColor) {
                    $rgbList[] = $c->getRgbValue();
                } elseif ($c instanceof Color) {
                    $rgbList[] = $c;
                }
            }
            if (count($rgbList) === 0) {
                return new Color(0, 0, 0);
            }
            if (method_exists(Color::class, 'mix')) {
                return Color::mix(...$rgbList);
            }
            $r = 0; $g = 0; $b = 0; $a = 0;
            foreach ($rgbList as $col) {
                $r += $col->getR();
                $g += $col->getG();
                $b += $col->getB();
                $a += $col->getA();
            }
            $n = count($rgbList);
            return new Color((int)($r / $n), (int)($g / $n), (int)($b / $n), (int)($a / $n));
        }

        /**
         * Returns the colors to which particles will fade.
         *
         * @return DyeColor[]|Color[]
         */
        public function getFadeColors(): array
        {
            return $this->fadeColors;
        }

        /**
         * Returns whether the explosion has a flickering effect.
         */
        public function willTwinkle(): bool
        {
            return $this->twinkle;
        }

        public function hasFlicker(): bool
        {
            return $this->twinkle;
        }

        public function getFlicker(): bool
        {
            return $this->twinkle;
        }

        /**
         * Returns whether the particles have a trail effect.
         */
        public function getTrail(): bool
        {
            return $this->trail;
        }

        public function hasTrail(): bool
        {
            return $this->trail;
        }

        public function toCompoundTag(): CompoundTag
        {
            return CompoundTag::create()
                ->setByte(self::TAG_TYPE, FireworkRocketTypeIdMap::getInstance()->toId($this->type))
                ->setByteArray(self::TAG_COLORS, self::encodeColors($this->colors))
                ->setByteArray(self::TAG_FADE_COLORS, self::encodeColors($this->fadeColors))
                ->setByte(self::TAG_TWINKLE, $this->twinkle ? 1 : 0)
                ->setByte(self::TAG_TRAIL, $this->trail ? 1 : 0);
        }
    }

    class FireworkRocket extends Item
    {
        public const TAG_FIREWORK_DATA = "Fireworks"; //TAG_Compound
        protected const TAG_FLIGHT_TIME_MULTIPLIER = "Flight"; //TAG_Byte
        public const TAG_EXPLOSIONS = "Explosions"; //TAG_List

        protected int $flightTimeMultiplier = 1;

        /** @var FireworkRocketExplosion[] */
        protected array $explosions = [];

        /**
         * Returns the value that will be used to calculate a randomized flight duration
         * for the firework (equals the amount of gunpowder used in crafting the rocket).
         */
        public function getFlightTimeMultiplier(): int
        {
            return $this->flightTimeMultiplier;
        }

        /**
         * Sets the value that will be used to calculate a randomized flight duration
         * for the firework.
         *
         * @return $this
         */
        public function setFlightTimeMultiplier(int $multiplier): self
        {
            if ($multiplier < 1 || $multiplier > 127) {
                throw new \InvalidArgumentException("Flight time multiplier must be in range 1-127");
            }
            $this->flightTimeMultiplier = $multiplier;

            return $this;
        }

        /**
         * Alias for getFlightTimeMultiplier().
         */
        public function getFlightDuration(): int
        {
            return $this->getFlightTimeMultiplier();
        }

        /**
         * Alias for setFlightTimeMultiplier().
         *
         * @return $this
         */
        public function setFlightDuration(int $duration): self
        {
            return $this->setFlightTimeMultiplier($duration);
        }

        /**
         * @return FireworkRocketExplosion[]
         */
        public function getExplosions(): array
        {
            return $this->explosions;
        }

        /**
         * @param FireworkRocketExplosion[] $explosions
         *
         * @return $this
         */
        public function setExplosions(array $explosions): self
        {
            foreach ($explosions as $explosion) {
                if (!$explosion instanceof FireworkRocketExplosion) {
                    throw new \TypeError("Array elements must be instances of " . FireworkRocketExplosion::class);
                }
            }
            $this->explosions = $explosions;

            return $this;
        }

        public function addExplosion(FireworkRocketExplosion $explosion): self
        {
            $this->explosions[] = $explosion;
            return $this;
        }

        public function clearExplosions(): self
        {
            $this->explosions = [];
            return $this;
        }

        public function onInteractBlock(Player $player, Block $blockReplace, Block $blockClicked, int $face, Vector3 $clickVector, array &$returnedItems): ItemUseResult
        {
            $zero = method_exists(Vector3::class, 'zero') ? Vector3::zero() : new Vector3(0.0, 0.0, 0.0);
            $position = $blockClicked->getPosition()->addVector($clickVector)->addVector($zero->getSide($face)->multiply(0.15));

            $randomDuration = (($this->flightTimeMultiplier + 1) * 10) + mt_rand(0, 12);

            $yaw = class_exists(Utils::class) && method_exists(Utils::class, 'getRandomFloat')
                ? Utils::getRandomFloat() * 360
                : (mt_rand() / mt_getrandmax()) * 360;

            $entity = new FireworkEntity(Location::fromObject($position, $player->getWorld(), $yaw, 90.0), $randomDuration, $this->explosions);
            if (method_exists($entity, 'setOwningEntity')) {
                $entity->setOwningEntity($player);
            }

            $rx = class_exists(Utils::class) && method_exists(Utils::class, 'getRandomFloat')
                ? (Utils::getRandomFloat() - Utils::getRandomFloat()) * 0.0023
                : ((mt_rand() - mt_rand()) / mt_getrandmax()) * 0.0023;
            $rz = class_exists(Utils::class) && method_exists(Utils::class, 'getRandomFloat')
                ? (Utils::getRandomFloat() - Utils::getRandomFloat()) * 0.0023
                : ((mt_rand() - mt_rand()) / mt_getrandmax()) * 0.0023;

            $entity->setMotion(new Vector3($rx, 0.05, $rz));
            $entity->spawnToAll();

            $this->pop();

            return ItemUseResult::SUCCESS;
        }

        protected function deserializeCompoundTag(CompoundTag $tag): void
        {
            $hasKeepOnDeath = $tag->getTag(self::TAG_KEEP_ON_DEATH) !== null;
            if (!$hasKeepOnDeath) {
                $tag->setByte(self::TAG_KEEP_ON_DEATH, 0);
            }
            try {
                parent::deserializeCompoundTag($tag);
            } finally {
                if (!$hasKeepOnDeath) {
                    $tag->removeTag(self::TAG_KEEP_ON_DEATH);
                }
            }

            $fireworkData = $tag->getTag(self::TAG_FIREWORK_DATA);
            if (!$fireworkData instanceof CompoundTag) {
                throw new SavedDataLoadingException("Missing firework data");
            }

            $flightTag = $fireworkData->getTag(self::TAG_FLIGHT_TIME_MULTIPLIER);
            $flight = $flightTag instanceof ByteTag ? $flightTag->getValue() : 1;
            $this->setFlightTimeMultiplier($flight);

            $this->explosions = [];
            $explosionsTag = $fireworkData->getTag(self::TAG_EXPLOSIONS);
            if ($explosionsTag instanceof ListTag) {
                foreach ($explosionsTag as $explosion) {
                    if ($explosion instanceof CompoundTag) {
                        $this->explosions[] = FireworkRocketExplosion::fromCompoundTag($explosion);
                    }
                }
            }
        }

        protected function serializeCompoundTag(CompoundTag $tag): void
        {
            parent::serializeCompoundTag($tag);

            $fireworkData = CompoundTag::create();
            $fireworkData->setByte(self::TAG_FLIGHT_TIME_MULTIPLIER, $this->flightTimeMultiplier);
            $explosionsTag = new ListTag([], NBT::TAG_Compound);
            foreach ($this->explosions as $explosion) {
                $explosionsTag->push($explosion->toCompoundTag());
            }
            $fireworkData->setTag(self::TAG_EXPLOSIONS, $explosionsTag);

            $tag->setTag(self::TAG_FIREWORK_DATA, $fireworkData);
        }
    }

    class FireworkStar extends Item
    {
        public const TAG_EXPLOSION = "FireworksItem"; //TAG_Compound
        public const TAG_CUSTOM_COLOR = "customColor"; //TAG_Int

        protected FireworkRocketExplosion $explosion;

        protected ?Color $customColor = null;

        public function __construct(ItemIdentifier $identifier, string $name = "Firework Star")
        {
            parent::__construct($identifier, $name);

            $this->explosion = new FireworkRocketExplosion(
                FireworkRocketType::SMALL_SPHERE,
                [DyeColor::BLACK],
                [],
                false,
                false
            );
        }

        public function getExplosion(): FireworkRocketExplosion
        {
            return $this->explosion;
        }

        /** @return $this */
        public function setExplosion(FireworkRocketExplosion $explosion): self
        {
            $this->explosion = $explosion;
            return $this;
        }

        /**
         * Returns the displayed color of the item.
         * The mixture of explosion colors, or the custom color if it is set.
         */
        public function getColor(): Color
        {
            return $this->customColor ?? $this->explosion->getColorMix();
        }

        /**
         * Returns the displayed custom color of the item that overrides
         * the mixture of explosion colors, or null if it is not set.
         */
        public function getCustomColor(): ?Color
        {
            return $this->customColor;
        }

        /**
         * Sets the displayed custom color of the item that overrides
         * the mixture of explosion colors, or removes it if $color is null.
         *
         * @return $this
         */
        public function setCustomColor(?Color $color): self
        {
            $this->customColor = $color;
            return $this;
        }

        protected function deserializeCompoundTag(CompoundTag $tag): void
        {
            $hasKeepOnDeath = $tag->getTag(self::TAG_KEEP_ON_DEATH) !== null;
            if (!$hasKeepOnDeath) {
                $tag->setByte(self::TAG_KEEP_ON_DEATH, 0);
            }
            try {
                parent::deserializeCompoundTag($tag);
            } finally {
                if (!$hasKeepOnDeath) {
                    $tag->removeTag(self::TAG_KEEP_ON_DEATH);
                }
            }

            $explosionTag = $tag->getTag(self::TAG_EXPLOSION);
            if (!$explosionTag instanceof CompoundTag) {
                throw new SavedDataLoadingException("Missing explosion data");
            }
            $this->explosion = FireworkRocketExplosion::fromCompoundTag($explosionTag);

            $customColorTag = $tag->getTag(self::TAG_CUSTOM_COLOR);
            if ($customColorTag instanceof IntTag) {
                $val = Binary::unsignInt($customColorTag->getValue());
                if (method_exists(Color::class, 'fromARGB')) {
                    $customColor = Color::fromARGB($val);
                } else {
                    $a = ($val >> 24) & 0xff;
                    $r = ($val >> 16) & 0xff;
                    $g = ($val >> 8) & 0xff;
                    $b = $val & 0xff;
                    $customColor = new Color($r, $g, $b, $a);
                }
                $color = $this->explosion->getColorMix();
                $isEqual = method_exists($customColor, 'equals')
                    ? $customColor->equals($color)
                    : ($customColor->toARGB() === $color->toARGB());
                if (!$isEqual) {
                    $this->customColor = $customColor;
                }
            }
        }

        protected function serializeCompoundTag(CompoundTag $tag): void
        {
            parent::serializeCompoundTag($tag);

            $tag->setTag(self::TAG_EXPLOSION, $this->explosion->toCompoundTag());
            $tag->setInt(self::TAG_CUSTOM_COLOR, Binary::signInt($this->getColor()->toARGB()));
        }
    }
}
