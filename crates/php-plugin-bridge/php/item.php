<?php

declare(strict_types=1);

namespace pocketmine\item {

    /**
     * Item mínimo del puente.
     *
     * Cobertura A3 (documentada):
     * - `customName` (NBT `display.Name`) ya existía; se mantiene.
     * - Encantamientos (`Enchantments`) y NBT compuesto (`CompoundTag`) son
     *   stubs: se guardan en arrays locales y se serializan en
     *   `toArray()`/`fromArray()`, pero nunca llegan a la red (no hay
     *   `ItemSerializer`/`EnchantmentManager` en el core).
     * - Durability (`Damage`/`MaxDurability`) es solo un entero local por
     *   id conocido; sin `Item` por tipo (espada, pico...) todos valen 0
     *   salvo `isDamaged()` derivado de `damage > 0`.
     */
    class Item
    {
        /** @var array<int,int> enchantment id => level (stub) */
        private array $enchantments = [];
        /** @var array<string,mixed> NBT stub (p. ej. can_place_on, lore) */
        private array $nbt = [];
        private int $damage = 0;

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

        // ---- NBT stub (CompoundTag sin serializador de red) ----

        public function getNBT(): array
        {
            return $this->nbt;
        }

        public function setNBT(array $tags): static
        {
            $this->nbt = $tags;
            return $this;
        }

        public function getCompoundTag(): ?array
        {
            return $this->nbt === [] ? null : $this->nbt;
        }

        public function setCompoundTag(?array $tag = null): void
        {
            $this->nbt = $tag ?? [];
        }

        public function getLore(): array
        {
            $lore = $this->nbt['display']['lore'] ?? [];
            return is_array($lore) ? array_values($lore) : [];
        }

        public function setLore(array $lore): void
        {
            if (!isset($this->nbt['display']) || !is_array($this->nbt['display'])) {
                $this->nbt['display'] = [];
            }
            $this->nbt['display']['lore'] = array_values($lore);
        }

        // ---- Encantamientos stub (id => level) ----

        /** @return array<int,int> */
        public function getEnchantments(): array
        {
            return $this->enchantments;
        }

        public function addEnchantment(int $enchantmentId, int $level = 1): void
        {
            $this->enchantments[$enchantmentId] = max(1, $level);
        }

        public function removeEnchantment(int $enchantmentId): void
        {
            unset($this->enchantments[$enchantmentId]);
        }

        public function clearEnchantments(): void
        {
            $this->enchantments = [];
        }

        public function hasEnchantments(): bool
        {
            return $this->enchantments !== [];
        }

        public function getEnchantmentLevel(int $enchantmentId): int
        {
            return $this->enchantments[$enchantmentId] ?? 0;
        }

        // ---- Durability stub ----

        public function getDamage(): int
        {
            return $this->damage;
        }

        public function setDamage(int $damage): void
        {
            $this->damage = max(0, $damage);
        }

        public function getMaxDurability(): int
        {
            // Sin clases de item por tipo: solo se devuelve un techo
            // genérico documentado. 0 = no dañable.
            return 0;
        }

        public function getDurability(): int
        {
            $max = $this->getMaxDurability();
            return $max === 0 ? 0 : max(0, $max - $this->damage);
        }

        public function isDamaged(): bool
        {
            return $this->damage > 0;
        }

        public function repair(): void
        {
            $this->damage = 0;
        }

        public static function fromArray(array $data): Item
        {
            $item = new Item(
                (int)($data['id'] ?? 0),
                (int)($data['meta'] ?? 0),
                (int)($data['count'] ?? 1),
                isset($data['custom_name']) ? (string)$data['custom_name'] : null
            );
            if (isset($data['nbt']) && is_array($data['nbt'])) {
                $item->nbt = $data['nbt'];
            }
            if (isset($data['enchantments']) && is_array($data['enchantments'])) {
                foreach ($data['enchantments'] as $id => $level) {
                    $item->enchantments[(int)$id] = (int)$level;
                }
            }
            if (isset($data['damage'])) {
                $item->damage = (int)$data['damage'];
            }
            return $item;
        }

        public function toArray(): array
        {
            $out = ['id' => $this->id, 'meta' => $this->meta, 'count' => $this->count];
            if ($this->customName !== null) {
                $out['custom_name'] = $this->customName;
            }
            if ($this->nbt !== []) {
                $out['nbt'] = $this->nbt;
            }
            if ($this->enchantments !== []) {
                $out['enchantments'] = $this->enchantments;
            }
            if ($this->damage !== 0) {
                $out['damage'] = $this->damage;
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

        /** Item creative inventory stub (uno por cada bloque conocido). */
        public static function getCreativeInventory(): array
        {
            static $inventory = null;
            if ($inventory === null) {
                $inventory = [];
                foreach (\pocketmine\world\BlockFactory::getBlockIdMap() as $name => $id) {
                    if ($id === 0) {
                        continue;
                    }
                    $inventory[] = self::get($id, 0, 1);
                }
            }
            return $inventory;
        }

        public static function clearCreativeItems(): void
        {
            // Stub: el inventario se deriva de BlockFactory, nada que limpiar.
        }

        public static function isRegistered(string $name): bool
        {
            return \pocketmine\world\BlockFactory::idFromName($name) !== 0;
        }

        /** Stub descriptivo: pondrá `isRegistered()` si se registra el item. */
        public static function getItemTypeId(string $name): string
        {
            return \pocketmine\world\BlockFactory::networkNameOf(
                \pocketmine\world\BlockFactory::idFromName($name)
            );
        }
    }
}
