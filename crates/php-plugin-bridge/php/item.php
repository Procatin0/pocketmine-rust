<?php

declare(strict_types=1);

namespace pocketmine\item {

    class Item
    {
        public function __construct(
            private int $id = 0,
            private int $meta = 0,
            private int $count = 1,
            private ?string $customName = null
        ) {
        }

        public function getId(): int
        {
            return $this->id;
        }

        public function getMeta(): int
        {
            return $this->meta;
        }

        public function getCount(): int
        {
            return $this->count;
        }

        public function setCount(int $count): void
        {
            $this->count = $count;
        }

        public function getCustomName(): string
        {
            return $this->customName ?? $this->getName();
        }

        public function setCustomName(?string $name): void
        {
            $this->customName = $name;
        }

        public function hasCustomName(): bool
        {
            return $this->customName !== null && $this->customName !== '';
        }

        public function clearCustomName(): void
        {
            $this->customName = null;
        }

        public function isNull(): bool
        {
            return $this->id === 0 || $this->count <= 0;
        }

        public function getTypeId(): string
        {
            return \pocketmine\world\BlockFactory::networkNameOf($this->id);
        }

        public function getName(): string
        {
            return \pocketmine\world\BlockFactory::friendlyNameOf($this->id);
        }

        public function getVanillaName(): string
        {
            return $this->getName();
        }

        public function equals(mixed $item, bool $checkDamage = true, bool $checkCompound = true): bool
        {
            if (!$item instanceof Item) {
                return false;
            }
            return $this->id === $item->id
                && (!$checkDamage || $this->meta === $item->meta)
                && $this->count === $item->count;
        }

        public function setCustomBlockData(mixed $data): Item
        {
            return $this;
        }

        public function getCustomBlockData(): mixed
        {
            return null;
        }

        public static function fromArray(array $data): Item
        {
            return new Item(
                (int)($data['id'] ?? 0),
                (int)($data['meta'] ?? 0),
                (int)($data['count'] ?? 1),
                isset($data['custom_name']) ? (string)$data['custom_name'] : null
            );
        }

        public function toArray(): array
        {
            $out = ['id' => $this->id, 'meta' => $this->meta, 'count' => $this->count];
            if ($this->customName !== null) {
                $out['custom_name'] = $this->customName;
            }
            return $out;
        }

        public function __toString(): string
        {
            return 'Item(' . $this->getName() . ' x' . $this->count . ')';
        }
    }

    class ItemFactory
    {
        private static ?ItemFactory $instance = null;

        public static function getInstance(): self
        {
            return self::$instance ??= new self();
        }

        /** @param string|int $name */
        public static function get($name, int $meta = 0, int $count = 1): Item
        {
            if (is_string($name)) {
                $id = \pocketmine\world\BlockFactory::idFromName($name);
                return new Item($id, $meta, $count);
            }
            return new Item((int)$name, $meta, $count);
        }

        public static function getFromString(string $itemString): Item
        {
            $count = 1;
            if (preg_match('/^(.+?)x(\d+)$/i', $itemString, $matches)) {
                $itemString = $matches[1];
                $count = (int)$matches[2];
            }
            $meta = 0;
            if (str_contains($itemString, ':')) {
                [$itemString, $metaPart] = explode(':', $itemString, 2);
                $meta = (int)$metaPart;
            }
            return self::get($itemString, $meta, $count);
        }

        public function registerItem(Item $item, bool $override = false): void
        {
            // Items share the block name registry in this bridge.
        }
    }
}
