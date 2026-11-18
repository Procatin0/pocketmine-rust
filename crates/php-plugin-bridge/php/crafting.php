<?php

declare(strict_types=1);

namespace pocketmine\crafting {
    use pocketmine\item\Item;
    use pocketmine\item\ItemFactory;
    use pocketmine\runtime\Bridge;

    /** `pocketmine\crafting\RecipeIngredient`. */
    interface RecipeIngredient
    {
        public function accepts(Item $item): bool;

        public function __toString(): string;
    }

    /** `pocketmine\crafting\ExactRecipeIngredient`: one id + meta. */
    class ExactRecipeIngredient implements RecipeIngredient
    {
        public function __construct(private Item $item) {}

        public function getItem(): Item
        {
            return $this->item;
        }

        public function accepts(Item $item): bool
        {
            return $item->getId() === $this->item->getId() && $item->getMeta() === $this->item->getMeta();
        }

        public function __toString(): string
        {
            return $this->item->__toString();
        }
    }

    /** `pocketmine\crafting\MetaWildcardRecipeIngredient`: one id, any meta. */
    class MetaWildcardRecipeIngredient implements RecipeIngredient
    {
        public function __construct(private int $id) {}

        public function getId(): int
        {
            return $this->id;
        }

        public function accepts(Item $item): bool
        {
            return $item->getId() === $this->id;
        }

        public function __toString(): string
        {
            return 'MetaWildcard(' . $this->id . ')';
        }
    }

    /** `pocketmine\crafting\CraftingRecipe`. */
    interface CraftingRecipe
    {
        /** @return Item[] */
        public function getResults(): array;

        /** @return Item[] */
        public function getResultsFor(CraftingGrid $grid): array;
    }

    /**
     * `pocketmine\crafting\CraftingGrid`: the player's 2x2 or the crafting
     * table's 3x3 input grid.
     *
     * Upstream it extends `SimpleInventory`; the bridge has no inventory
     * hierarchy for grids, so the slots are kept here with the same accessors
     * the recipes use (`getItem()`, `isSlotEmpty()`, `getContents()`).
     */
    class CraftingGrid
    {
        public const SIZE_SMALL = 2;
        public const SIZE_BIG = 3;

        /** @var Item[] */
        private array $slots;

        public function __construct(private int $gridWidth)
        {
            $this->slots = array_fill(0, $gridWidth ** 2, ItemFactory::get(0, 0, 0));
        }

        public static function small(): self
        {
            return new self(self::SIZE_SMALL);
        }

        public static function big(): self
        {
            return new self(self::SIZE_BIG);
        }

        public function getGridWidth(): int
        {
            return $this->gridWidth;
        }

        public function getSize(): int
        {
            return $this->gridWidth ** 2;
        }

        public function setItem(int $index, Item $item): void
        {
            if ($index < 0 || $index >= $this->getSize()) {
                return;
            }
            $this->slots[$index] = $item;
        }

        public function getItem(int $index): Item
        {
            return $this->slots[$index] ?? ItemFactory::get(0, 0, 0);
        }

        public function isSlotEmpty(int $index): bool
        {
            return $this->getItem($index)->isNull();
        }

        /** @return Item[] */
        public function getContents(): array
        {
            return $this->slots;
        }

        public function clearAll(): void
        {
            $this->slots = array_fill(0, $this->getSize(), ItemFactory::get(0, 0, 0));
        }
    }

    /** `pocketmine\crafting\ShapedRecipe`. */
    class ShapedRecipe implements CraftingRecipe
    {
        /** @var string[] */
        private array $shape;
        /** @var array<string, RecipeIngredient> */
        private array $ingredientList = [];
        /** @var Item[] */
        private array $results;
        private int $height;
        private int $width;

        /**
         * @param string[] $shape     1-3 rows of at most 3 characters; spaces are air
         * @param array<string, RecipeIngredient> $ingredients char => ingredient
         * @param Item[]   $results
         */
        public function __construct(array $shape, array $ingredients, array $results)
        {
            $this->height = count($shape);
            if ($this->height > 3 || $this->height <= 0) {
                throw new \InvalidArgumentException("Shaped recipes may only have 1, 2 or 3 rows, not $this->height");
            }
            $shape = array_values($shape);
            $this->width = strlen($shape[0]);
            if ($this->width > 3 || $this->width <= 0) {
                throw new \InvalidArgumentException("Shaped recipes may only have 1, 2 or 3 columns, not $this->width");
            }
            foreach ($shape as $y => $row) {
                if (strlen($row) !== $this->width) {
                    throw new \InvalidArgumentException("Shaped recipe rows must all have the same length (expected $this->width, got " . strlen($row) . ')');
                }
                foreach (str_split($row) as $char) {
                    if ($char !== ' ' && !isset($ingredients[$char])) {
                        throw new \InvalidArgumentException("Missing ingredient for character '$char'");
                    }
                }
            }
            $this->shape = $shape;
            foreach ($ingredients as $char => $ingredient) {
                if (!is_string($char) || strlen($char) !== 1) {
                    throw new \InvalidArgumentException("Recipe ingredient keys must be single characters");
                }
                if (!is_bool($ingredient) && !($ingredient instanceof RecipeIngredient)) {
                    throw new \InvalidArgumentException('Recipe ingredients must implement RecipeIngredient');
                }
                $this->ingredientList[$char] = $ingredient;
            }
            $this->results = array_map(static fn(Item $item): Item => clone $item, array_values($results));
            if ($this->results === []) {
                throw new \InvalidArgumentException('A recipe needs at least one result item');
            }
        }

        /** @return string[] */
        public function getShape(): array
        {
            return $this->shape;
        }

        public function getHeight(): int
        {
            return $this->height;
        }

        public function getWidth(): int
        {
            return $this->width;
        }

        /** @return array<string, RecipeIngredient> */
        public function getIngredientList(): array
        {
            return $this->ingredientList;
        }

        public function getResults(): array
        {
            return array_map(static fn(Item $item): Item => clone $item, $this->results);
        }

        public function getResultsFor(CraftingGrid $grid): array
        {
            return $this->matches($grid) ? $this->getResults() : [];
        }

        public function matches(CraftingGrid $grid): bool
        {
            if ($this->width > $grid->getGridWidth() || $this->height > $grid->getGridWidth()) {
                return false;
            }
            for ($offsetY = 0; $offsetY <= $grid->getGridWidth() - $this->height; ++$offsetY) {
                for ($offsetX = 0; $offsetX <= $grid->getGridWidth() - $this->width; ++$offsetX) {
                    if ($this->matchesAt($grid, $offsetX, $offsetY)) {
                        return true;
                    }
                }
            }
            return false;
        }

        private function matchesAt(CraftingGrid $grid, int $offsetX, int $offsetY): bool
        {
            for ($y = 0; $y < $grid->getGridWidth(); ++$y) {
                for ($x = 0; $x < $grid->getGridWidth(); ++$x) {
                    $item = $grid->getItem($y * $grid->getGridWidth() + $x);
                    $char = ' ';
                    if ($x >= $offsetX && $x < $offsetX + $this->width
                        && $y >= $offsetY && $y < $offsetY + $this->height) {
                        $char = $this->shape[$y - $offsetY][$x - $offsetX];
                    }
                    $ingredient = $this->ingredientList[$char] ?? null;
                    if ($ingredient === null) {
                        if (!$item->isNull()) {
                            return false;
                        }
                    } elseif (!$ingredient->accepts($item)) {
                        return false;
                    }
                }
            }
            return true;
        }

        public function __toString(): string
        {
            return 'ShapedRecipe(' . implode('|', $this->shape) . ')';
        }
    }

    /** `pocketmine\crafting\ShapelessRecipe`. */
    class ShapelessRecipe implements CraftingRecipe
    {
        /** @var RecipeIngredient[] */
        private array $ingredients;
        /** @var Item[] */
        private array $results;

        /**
         * @param RecipeIngredient[] $ingredients at most 9
         * @param Item[]             $results
         */
        public function __construct(array $ingredients, array $results)
        {
            if (count($ingredients) > 9) {
                throw new \InvalidArgumentException('Shapeless recipes cannot have more than 9 ingredients');
            }
            if ($ingredients === []) {
                throw new \InvalidArgumentException('A shapeless recipe needs at least one ingredient');
            }
            $this->ingredients = array_values($ingredients);
            $this->results = array_map(static fn(Item $item): Item => clone $item, array_values($results));
            if ($this->results === []) {
                throw new \InvalidArgumentException('A recipe needs at least one result item');
            }
        }

        /** @return RecipeIngredient[] */
        public function getIngredientList(): array
        {
            return $this->ingredients;
        }

        public function getResults(): array
        {
            return array_map(static fn(Item $item): Item => clone $item, $this->results);
        }

        public function getResultsFor(CraftingGrid $grid): array
        {
            $items = [];
            foreach ($grid->getContents() as $item) {
                if (!$item->isNull()) {
                    $items[] = $item;
                }
            }
            if (count($items) !== count($this->ingredients)) {
                return [];
            }
            $remaining = array_fill(0, count($items), false);
            foreach ($this->ingredients as $ingredient) {
                $matched = null;
                foreach ($items as $index => $candidate) {
                    if ($remaining[$index]) {
                        continue;
                    }
                    if ($ingredient->accepts($candidate)) {
                        $matched = $index;
                        break;
                    }
                }
                if ($matched === null) {
                    return [];
                }
                $remaining[$matched] = true;
            }
            return $this->getResults();
        }

        public function __toString(): string
        {
            return 'ShapelessRecipe(' . implode(', ', array_map('strval', $this->ingredients)) . ')';
        }
    }

    /** `pocketmine\crafting\FurnaceRecipe`. */
    class FurnaceRecipe
    {
        public function __construct(
            private RecipeIngredient $input,
            private Item $result
        ) {}

        public function getInput(): RecipeIngredient
        {
            return $this->input;
        }

        public function getResult(): Item
        {
            return clone $this->result;
        }

        public function matches(Item $item): bool
        {
            return $this->input->accepts($item);
        }

        public function __toString(): string
        {
            return 'FurnaceRecipe(' . $this->input . ')';
        }
    }

    /**
     * `pocketmine\crafting\FurnaceType`: PM5 enum for furnace types.
     */
    enum FurnaceType
    {
        use \pocketmine\utils\LegacyEnumShimTrait;

        case FURNACE;
        case BLAST_FURNACE;
        case SMOKER;
        case CAMPFIRE;
        case SOUL_CAMPFIRE;

        public function getCookDurationTicks(): int
        {
            return match ($this) {
                self::BLAST_FURNACE, self::SMOKER => 100,
                self::CAMPFIRE, self::SOUL_CAMPFIRE => 600,
                default => 200,
            };
        }

        public static function cookTicks(self|string $type): int
        {
            if (is_string($type)) {
                $type = self::fromString($type);
            }
            return $type->getCookDurationTicks();
        }
    }

    /** `pocketmine\crafting\FurnaceRecipeManager` (one per `FurnaceType`). */
    class FurnaceRecipeManager
    {
        public function __construct(private string $type) {}

        public function getType(): string
        {
            return $this->type;
        }

        /** Asks the native engine, so plugin-registered smelting matches too. */
        public function match(Item $input): ?FurnaceRecipe
        {
            if (!Bridge::isActive()) {
                return null;
            }
            $result = Bridge::hostCall('crafting.furnace', [
                'block' => $this->type,
                'item' => ['id' => $input->getId(), 'meta' => $input->getMeta(), 'count' => 1],
            ]);
            if (!is_array($result) || empty($result['matched'])) {
                return null;
            }
            $output = Item::fromArray($result['result'] ?? []);
            return $output->isNull() ? null : new FurnaceRecipe(
                new ExactRecipeIngredient($input),
                $output
            );
        }
    }

    /**
     * `pocketmine\crafting\CraftingManager`.
     *
     * The engine is native (`crates/server-core/src/crafting.rs`): this class
     * mirrors the PM API, caches the recipe index from `crafting.recipes` and
     * delegates matching to `crafting.match`, so `matchRecipe()` returns
     * exactly what `ItemStackRequest` will craft.
     */
    class CraftingManager
    {
        /** @var array<int, CraftingRecipe>|null */
        private ?array $index = null;
        /** @var array<string, FurnaceRecipeManager> */
        private array $furnaceManagers = [];
        /** @var \Closure[] */
        private array $recipeRegisteredCallbacks = [];

        public function __construct()
        {
            foreach (FurnaceType::cases() as $type) {
                $this->furnaceManagers[$type->name] = new FurnaceRecipeManager($type->name);
                $this->furnaceManagers[strtolower($type->name)] = $this->furnaceManagers[$type->name];
            }
        }

        /** @return \Closure[] */
        public function getRecipeRegisteredCallbacks(): array
        {
            return $this->recipeRegisteredCallbacks;
        }

        public function registerRecipeRegisteredCallback(\Closure $callback): void
        {
            $this->recipeRegisteredCallbacks[] = $callback;
        }

        public function unregisterRecipeRegisteredCallback(\Closure $callback): void
        {
            foreach ($this->recipeRegisteredCallbacks as $index => $registered) {
                if ($registered === $callback) {
                    unset($this->recipeRegisteredCallbacks[$index]);
                }
            }
        }

        private function notifyRecipeRegistered(): void
        {
            foreach ($this->recipeRegisteredCallbacks as $callback) {
                $callback();
            }
        }

        /** @return array<int, CraftingRecipe> */
        public function getCraftingRecipeIndex(): array
        {
            if ($this->index === null) {
                $this->buildIndex();
            }
            return $this->index ?? [];
        }

        public function getCraftingRecipeFromIndex(int $index): ?CraftingRecipe
        {
            return $this->getCraftingRecipeIndex()[$index] ?? null;
        }

        /** @return ShapedRecipe[] */
        public function getShapedRecipes(): array
        {
            return array_values(array_filter(
                $this->getCraftingRecipeIndex(),
                static fn(CraftingRecipe $recipe): bool => $recipe instanceof ShapedRecipe
            ));
        }

        /** @return ShapelessRecipe[] */
        public function getShapelessRecipes(): array
        {
            return array_values(array_filter(
                $this->getCraftingRecipeIndex(),
                static fn(CraftingRecipe $recipe): bool => $recipe instanceof ShapelessRecipe
            ));
        }

        public function getFurnaceRecipeManager(FurnaceType|string $type): FurnaceRecipeManager
        {
            $key = $type instanceof FurnaceType ? $type->name : $type;
            return $this->furnaceManagers[$key] ??= new FurnaceRecipeManager($key);
        }

        /**
         * `CraftingManager::matchRecipe()`: asks the native engine for the
         * recipe that fits the grid, then returns the matching PHP recipe
         * object. `$outputs` is accepted for signature compatibility and is not
         * needed (the native engine owns the recipe table).
         *
         * @param Item[] $outputs
         */
        public function matchRecipe(CraftingGrid $grid, array $outputs = []): ?CraftingRecipe
        {
            if (!Bridge::isActive()) {
                return null;
            }
            $items = [];
            foreach ($grid->getContents() as $item) {
                $items[] = $item->isNull() ? null : [
                    'id' => $item->getId(),
                    'meta' => $item->getMeta(),
                    'count' => max(1, $item->getCount()),
                ];
            }
            $result = Bridge::hostCall('crafting.match', [
                'size' => $grid->getGridWidth(),
                'items' => $items,
            ]);
            if (!is_array($result) || empty($result['matched'])) {
                return null;
            }
            $matched = Item::fromArray($result['result'] ?? []);
            if ($matched->isNull()) {
                return null;
            }
            foreach ($this->getCraftingRecipeIndex() as $recipe) {
                foreach ($recipe->getResults() as $output) {
                    if ($output->getId() === $matched->getId() && $output->getMeta() === $matched->getMeta()) {
                        return $recipe;
                    }
                }
            }
            return null;
        }

        public function registerShapedRecipe(ShapedRecipe $recipe): void
        {
            if (!Bridge::isActive()) {
                return;
            }
            $ingredients = [];
            foreach ($recipe->getIngredientList() as $char => $ingredient) {
                $ingredients[$char] = self::ingredientToArray($ingredient);
            }
            Bridge::hostCall('crafting.register_shaped', [
                'shape' => $recipe->getShape(),
                'ingredients' => $ingredients,
                'results' => array_map([self::class, 'itemToArray'], $recipe->getResults()),
            ]);
            $this->index = null;
            $this->notifyRecipeRegistered();
        }

        public function registerShapelessRecipe(ShapelessRecipe $recipe): void
        {
            if (!Bridge::isActive()) {
                return;
            }
            Bridge::hostCall('crafting.register_shapeless', [
                'ingredients' => array_map([self::class, 'ingredientToArray'], $recipe->getIngredientList()),
                'results' => array_map([self::class, 'itemToArray'], $recipe->getResults()),
            ]);
            $this->index = null;
            $this->notifyRecipeRegistered();
        }

        /** @return array<string, mixed> */
        private static function ingredientToArray(RecipeIngredient $ingredient): array
        {
            if ($ingredient instanceof ExactRecipeIngredient) {
                $item = $ingredient->getItem();
                return ['type' => 'exact', 'id' => $item->getId(), 'meta' => $item->getMeta()];
            }
            if ($ingredient instanceof MetaWildcardRecipeIngredient) {
                return ['type' => 'meta', 'id' => $ingredient->getId()];
            }
            return ['type' => 'exact', 'id' => 0, 'meta' => 0];
        }

        /** @return array<string, int> */
        private static function itemToArray(Item $item): array
        {
            return ['id' => $item->getId(), 'meta' => $item->getMeta(), 'count' => max(1, $item->getCount())];
        }

        /** Rebuilds the recipe index from the native engine. */
        private function buildIndex(): void
        {
            $this->index = [];
            if (!Bridge::isActive()) {
                return;
            }
            $result = Bridge::hostCall('crafting.recipes', []);
            foreach (($result['recipes'] ?? []) as $entry) {
                $recipe = $this->recipeFromArray($entry);
                if ($recipe !== null) {
                    $this->index[(int)($entry['index'] ?? count($this->index))] = $recipe;
                }
            }
            ksort($this->index);
        }

        /** @param array<string, mixed> $data */
        private function recipeFromArray(array $data): ?CraftingRecipe
        {
            $result = Item::fromArray($data['result'] ?? []);
            if ($result->isNull()) {
                return null;
            }
            $kind = (string)($data['kind'] ?? '');
            if ($kind === 'shaped') {
                $cells = is_array($data['cells'] ?? null) ? $data['cells'] : [];
                $width = (int)($data['width'] ?? 0);
                $symbols = [];
                $shape = [];
                $nextChar = 0;
                for ($y = 0; $y < (int)($data['height'] ?? 0); ++$y) {
                    $row = '';
                    for ($x = 0; $x < $width; ++$x) {
                        $cell = $cells[$y * $width + $x] ?? null;
                        if (!is_array($cell)) {
                            $row .= ' ';
                            continue;
                        }
                        $key = json_encode($cell);
                        if (!isset($symbols[$key])) {
                            $symbols[$key] = ['char' => chr(65 + $nextChar++), 'ingredient' => self::ingredientFromArray($cell)];
                        }
                        $row .= $symbols[$key]['char'];
                    }
                    $shape[] = $row;
                }
                $ingredients = [];
                foreach ($symbols as $symbol) {
                    $ingredients[$symbol['char']] = $symbol['ingredient'];
                }
                try {
                    return new ShapedRecipe($shape, $ingredients, [$result]);
                } catch (\InvalidArgumentException) {
                    return null;
                }
            }
            if ($kind === 'shapeless' || $kind === 'furnace') {
                $ingredients = [];
                foreach (($data['ingredients'] ?? []) as $entry) {
                    if (is_array($entry)) {
                        $ingredients[] = self::ingredientFromArray($entry);
                    }
                }
                if ($ingredients === []) {
                    return null;
                }
                return new ShapelessRecipe($ingredients, [$result]);
            }
            return null;
        }

        /** @param array<string, mixed> $data */
        private static function ingredientFromArray(array $data): RecipeIngredient
        {
            if (($data['type'] ?? 'exact') === 'tag') {
                // Tag ingredients have no PM class in this bridge: match the
                // first known item of the tag would be wrong, so they fall back
                // to a wildcard on id 0 (never matches) and are documented.
                return new MetaWildcardRecipeIngredient(0);
            }
            if (($data['type'] ?? 'exact') === 'meta') {
                return new MetaWildcardRecipeIngredient((int)($data['id'] ?? 0));
            }
            return new ExactRecipeIngredient(ItemFactory::get(
                (int)($data['id'] ?? 0),
                (int)($data['meta'] ?? 0),
                1
            ));
        }
    }
}
