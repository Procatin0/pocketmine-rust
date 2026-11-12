<?php

declare(strict_types=1);

namespace pocketmine\network\mcpe\protocol\types {

    /**
     * PM `pocketmine\network\mcpe\protocol\types\BossBarColor`
     * (Bedrock `BossEventPacket` wire values).
     */
    class BossBarColor
    {
        public const PINK = 0;
        public const BLUE = 1;
        public const RED = 2;
        public const GREEN = 3;
        public const YELLOW = 4;
        public const PURPLE = 5;
        public const REBECCA_PURPLE = 6;
        public const WHITE = 7;
    }

    /**
     * Bedrock boss-bar overlays / dividers (Bedrock `BossEventPacket`
     * `overlay` values): progress, then 10/20/6/4/3 segments.
     */
    class BossBarOverlay
    {
        public const PROGRESS = 0;
        public const TEN = 1;
        public const TWENTY = 2;
        public const SIX = 3;
        public const FOUR = 4;
        public const THREE = 5;
    }
}

namespace xenialdan\apibossbar {

    use GlobalLogger;
    use InvalidArgumentException;
    use pocketmine\network\mcpe\protocol\types\BossBarColor;
    use pocketmine\network\mcpe\protocol\types\BossBarOverlay;
    use pocketmine\player\Player;
    use pocketmine\plugin\Plugin;
    use pocketmine\Server;

    /**
     * Rust port of `xenialdan\apibossbar` (apiBossBar virion), over the
     * `set_bossbar` / `remove_bossbar` bridge actions. Bedrock transport
     * is `BossEventPacket` (0x4A), Java is `boss_bar` — both owned by
     * `SessionManager` (`bossbar.rs`); this object only keeps the PHP
     * copy of the state and re-sends the WHOLE state on every mutation
     * (same full-state pattern as `set_scoreboard`), so the Rust side
     * always diffs a complete picture.
     *
     * `bossId` is the Bedrock actor unique id (`$actorId`; synthetic
     * `nextRuntimeId()` unless `setEntity()` binds a real entity).
     */
    class BossBar
    {
        /** @var Player[] keyed by lowercase name */
        protected array $players = [];
        protected string $title = "";
        protected string $subTitle = "";
        protected int $color = BossBarColor::PURPLE;
        protected int $overlay = BossBarOverlay::PROGRESS;
        /** Java-only flags (darken/music/fog bits). */
        protected int $flags = 0;
        public ?int $actorId = null;
        /** @var Player[] hidden via hideFrom (still subscribed) */
        protected array $hidden = [];
        private static ?int $nextRuntimeId = 1000000;

        public function __construct()
        {
            $this->actorId = self::$nextRuntimeId++;
        }

        /** `BossBar::createForEntity($entity)`: actor id = entity id. */
        public static function createForEntity(mixed $entity): self
        {
            $bar = new self();
            return $bar->setEntity($entity);
        }

        /** @return Player[] */
        public function getPlayers(): array
        {
            return array_values($this->players);
        }

        /** @param Player[] $players */
        public function addPlayers(array $players): static
        {
            foreach ($players as $player) {
                $this->addPlayer($player);
            }
            $this->sync();
            return $this;
        }

        public function addPlayer(Player $player): static
        {
            $this->players[strtolower($player->getName())] = $player;
            unset($this->hidden[strtolower($player->getName())]);
            $this->sync();
            return $this;
        }

        public function removePlayer(Player $player): static
        {
            $key = strtolower($player->getName());
            if (!isset($this->players[$key])) {
                \pocketmine\Server::getInstance()->getLogger()->debug("Removed player that was not added to the boss bar (" . $this . ")");
                return $this;
            }
            unset($this->players[$key], $this->hidden[$key]);
            $this->sync();
            return $this;
        }

        /** @param Player[] $players */
        public function removePlayers(array $players): static
        {
            foreach ($players as $player) {
                $this->removePlayer($player);
            }
            $this->sync();
            return $this;
        }

        public function removeAllPlayers(): static
        {
            $this->players = [];
            $this->hidden = [];
            $this->sync();
            return $this;
        }

        public function getTitle(): string
        {
            return $this->title;
        }

        public function setTitle(string $title = ""): static
        {
            $this->title = $title;
            $this->sync();
            return $this;
        }

        public function getSubTitle(): string
        {
            return $this->subTitle;
        }

        public function setSubTitle(string $subTitle = ""): static
        {
            $this->subTitle = $subTitle;
            $this->sync();
            return $this;
        }

        /** Combined title + subtitle (`"\n\n"` join, as upstream). */
        public function getFullTitle(): string
        {
            $text = $this->title;
            if (!empty($this->subTitle)) {
                $text .= "\n\n" . $this->subTitle;
            }
            return $text;
        }

        /** @param float $percentage 0-1 */
        public function setPercentage(float $percentage): static
        {
            $this->percentage = min(1.0, max(0.0, $percentage));
            $this->sync();
            return $this;
        }

        private float $percentage = 1.0;

        public function getPercentage(): float
        {
            return $this->percentage;
        }

        public function getColor(): int
        {
            return $this->color;
        }

        public function setColor(int $color): static
        {
            $this->color = $color;
            $this->sync();
            return $this;
        }

        public function getOverlay(): int
        {
            return $this->overlay;
        }

        /** Overlay / divider style (`BossBarOverlay::*`). */
        public function setOverlay(int $overlay): static
        {
            $this->overlay = $overlay;
            $this->sync();
            return $this;
        }

        /** `setStyle(color, overlay)` (Bedrock colour + overlay). */
        public function setStyle(int $color, int $overlay): static
        {
            $this->color = $color;
            $this->overlay = $overlay;
            $this->sync();
            return $this;
        }

        /** Java-only flags (darken/music/fog bits). */
        public function getFlags(): int
        {
            return $this->flags;
        }

        public function setFlags(int $flags): static
        {
            $this->flags = $flags;
            $this->sync();
            return $this;
        }

        /** @param Player[] $players */
        public function hideFrom(array $players): static
        {
            foreach ($players as $player) {
                $this->hidden[strtolower($player->getName())] = $player;
            }
            $this->sync();
            return $this;
        }

        public function hideFromAll(): static
        {
            $this->hidden = $this->players;
            $this->sync();
            return $this;
        }

        /** @param Player[] $players */
        public function showTo(array $players): static
        {
            foreach ($players as $player) {
                unset($this->hidden[strtolower($player->getName())]);
            }
            $this->sync();
            return $this;
        }

        public function showToAll(): static
        {
            $this->hidden = [];
            $this->sync();
            return $this;
        }

        /**
         * Bound entity (player name or other entity id string, as the
         * bridge has no entity lifecycle; `null` = synthetic actor).
         */
        public function getEntity(): mixed
        {
            return $this->entity;
        }

        private mixed $entity = null;

        public function setEntity(mixed $entity = null): static
        {
            if (is_object($entity) && method_exists($entity, 'isClosed') && ($entity->isClosed() || (method_exists($entity, 'isFlaggedForDespawn') && $entity->isFlaggedForDespawn()))) {
                throw new InvalidArgumentException("Entity " . get_class($entity) . " can not be used since its not valid anymore (closed or flagged for despawn)");
            }
            $this->entity = $entity;
            if ($entity instanceof Player) {
                // A real Bedrock session's unique id becomes the actor
                // id (upstream uses `$entity->getId()`).
                if (is_numeric($entity->getXuid()) || is_numeric($entity->uniqueId ?? null)) {
                    $this->actorId = (int)$entity->getXuid();
                }
            }
            $this->sync();
            return $this;
        }

        public function resetEntity(bool $removeEntity = false): static
        {
            $this->entity = null;
            $this->sync();
            return $this;
        }

        /** Destroys the bar remotely (hide + unregister). */
        public function destroy(): void
        {
            $this->addAction('remove_bossbar', ['boss_id' => $this->actorId ?? 0]);
        }

        protected function sync(): void
        {
            if (!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()) {
                return;
            }
            try {
                $this->addAction('set_bossbar', ['bossbar' => $this->toData()]);
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/BOSSBAR] Unable to queue set_bossbar: ' . $e->getMessage() . PHP_EOL);
            }
        }

        /** Full-state payload for the `set_bossbar` action. */
        public function toData(): array
        {
            $names = static fn(array $players): array => array_values(array_map(
                static fn(Player $player): string => $player->getName(),
                $players
            ));
            return [
                'boss_id' => $this->actorId ?? 0,
                'diverse' => $this instanceof DiverseBossBar,
                'title' => $this->title,
                'subtitle' => $this->subTitle,
                'percentage' => $this->percentage,
                'color' => $this->color,
                'overlay' => $this->overlay,
                'flags' => $this->flags,
                'entity' => $this->entity instanceof Player ? $this->entity->getName()
                    : (is_string($this->entity) ? $this->entity : null),
                'players' => $names($this->getPlayers()),
                'hidden' => $names(array_values($this->hidden)),
                'overrides' => $this->overridesData(),
            ];
        }

        /** @return array<int, array> per-player overrides (empty here). */
        protected function overridesData(): array
        {
            return [];
        }

        private function addAction(string $action, array $data = []): void
        {
            try {
                \pocketmine\Server::getInstance()->addAction($action, $data);
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/BOSSBAR] Unable to queue ' . $action . ': ' . $e->getMessage() . PHP_EOL);
            }
        }

        public function __toString(): string
        {
            return __CLASS__ . " ID: " . $this->actorId . ", Players: " . count($this->players)
                . ", Title: \"" . $this->title . "\", Subtitle: \"" . $this->subTitle
                . "\", Percentage: \"" . $this->getPercentage() . "\", Color: \"" . $this->color . "\"";
        }
    }

    /**
     * `DiverseBossBar`: per-player title/subtitle/percentage/color
     * overrides over the shared defaults (upstream `*For` methods).
     */
    class DiverseBossBar extends BossBar
    {
        /** @var array<string, string> */
        private array $titles = [];
        /** @var array<string, string> */
        private array $subTitles = [];
        /** @var array<string, float> */
        private array $percentages = [];
        /** @var array<string, int> */
        private array $colors = [];

        public function addPlayer(Player $player): static
        {
            parent::addPlayer($player);
            return $this;
        }

        public function removePlayer(Player $player): static
        {
            $key = strtolower($player->getName());
            unset($this->titles[$key], $this->subTitles[$key], $this->percentages[$key], $this->colors[$key]);
            return parent::removePlayer($player);
        }

        public function removeAllPlayers(): static
        {
            $this->titles = [];
            $this->subTitles = [];
            $this->percentages = [];
            $this->colors = [];
            return parent::removeAllPlayers();
        }

        public function resetFor(Player $player): static
        {
            $key = strtolower($player->getName());
            unset($this->titles[$key], $this->subTitles[$key], $this->percentages[$key], $this->colors[$key]);
            $this->sync();
            return $this;
        }

        public function resetForAll(): static
        {
            $this->titles = [];
            $this->subTitles = [];
            $this->percentages = [];
            $this->colors = [];
            $this->sync();
            return $this;
        }

        public function getTitleFor(Player $player): string
        {
            return $this->titles[strtolower($player->getName())] ?? $this->getTitle();
        }

        /** @param Player[] $players */
        public function setTitleFor(array $players, string $title = ""): static
        {
            foreach ($players as $player) {
                $this->titles[strtolower($player->getName())] = $title;
            }
            $this->sync();
            return $this;
        }

        public function getSubTitleFor(Player $player): string
        {
            return $this->subTitles[strtolower($player->getName())] ?? $this->getSubTitle();
        }

        /** @param Player[] $players */
        public function setSubTitleFor(array $players, string $subTitle = ""): static
        {
            foreach ($players as $player) {
                $this->subTitles[strtolower($player->getName())] = $subTitle;
            }
            $this->sync();
            return $this;
        }

        /**
         * Upstream quirk kept bug-for-bug: only the override title /
         * subtitle count; an empty override falls back to the shared
         * full title.
         */
        public function getFullTitleFor(Player $player): string
        {
            $key = strtolower($player->getName());
            $text = $this->titles[$key] ?? "";
            if (!empty($this->subTitles[$key] ?? "")) {
                $text .= "\n\n" . ($this->subTitles[$key] ?? "");
            }
            if (empty($text)) {
                $text = $this->getFullTitle();
            }
            return $text;
        }

        /** @param Player[] $players */
        public function setPercentageFor(array $players, float $percentage): static
        {
            $percentage = min(1.0, max(0.0, $percentage));
            foreach ($players as $player) {
                $this->percentages[strtolower($player->getName())] = $percentage;
            }
            $this->sync();
            return $this;
        }

        public function getPercentageFor(Player $player): float
        {
            return $this->percentages[strtolower($player->getName())] ?? $this->getPercentage();
        }

        /** @param Player[] $players */
        public function setColorFor(array $players, int $color): static
        {
            foreach ($players as $player) {
                $this->colors[strtolower($player->getName())] = $color;
            }
            $this->sync();
            return $this;
        }

        public function getColorFor(Player $player): int
        {
            return $this->colors[strtolower($player->getName())] ?? $this->getColor();
        }

        protected function overridesData(): array
        {
            $out = [];
            $keys = array_unique(array_merge(
                array_keys($this->titles),
                array_keys($this->subTitles),
                array_keys($this->percentages),
                array_keys($this->colors)
            ));
            foreach ($keys as $key) {
                $name = $key;
                $player = $this->findPlayer($key);
                if ($player !== null) {
                    $name = $player->getName();
                }
                $out[] = [
                    'player' => $name,
                    'title' => $this->titles[$key] ?? null,
                    'subtitle' => $this->subTitles[$key] ?? null,
                    'percentage' => $this->percentages[$key] ?? null,
                    'color' => $this->colors[$key] ?? null,
                ];
            }
            return $out;
        }

        private function findPlayer(string $key): ?Player
        {
            foreach ($this->getPlayers() as $player) {
                if (strtolower($player->getName()) === $key) {
                    return $player;
                }
            }
            // Overrides for a player not currently subscribed are
            // still sent (lowercase key as name); Rust keys case-
            // insensitively.
            return null;
        }
    }

    /**
     * `API::load($plugin)`: registers the `PacketListener` for
     * client-sent `BossEventPacket`s. In the bridge the listener runs
     * in Rust (`SessionManager::handle_boss_event`); this is a no-op
     * kept for plugin-API parity (plugins call it from `onEnable`).
     */
    class API
    {
        public static function load(Plugin $plugin): void
        {
            // PacketListener::register($plugin); — handled in Rust.
        }
    }

    /**
     * Client-sent `BossEventPacket`s (register/unregister/query).
     * Rust decodes 0x4A and routes them through
     * `SessionManager::handle_boss_event` (register/unregister log,
     * query resends the bar); kept for API parity.
     */
    class PacketListener
    {
        private static ?Plugin $registrant = null;

        public static function isRegistered(): bool
        {
            return self::$registrant instanceof Plugin;
        }

        public static function getRegistrant(): ?Plugin
        {
            return self::$registrant;
        }

        public static function unregister(): void
        {
            self::$registrant = null;
        }

        public static function register(Plugin $plugin): void
        {
            if (self::isRegistered()) {
                return;
            }
            self::$registrant = $plugin;
        }
    }
}
