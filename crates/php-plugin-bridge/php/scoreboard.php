<?php

declare(strict_types=1);

namespace pocketmine\scoreboard {

    /**
     * PM `pocketmine\scoreboard\ScoreboardSlot`: where an objective is
     * displayed. Wire values are the Bedrock `DisplaySlot` bytes
     * (sidebar=1, below-name=2, list=3); Java uses list=0/sidebar=1/
     * below-name=2, mapped in Rust (`ScoreboardSlot::java_position`).
     */
    final class ScoreboardSlot
    {
        public const SIDEBAR = 1;
        public const BELOW_NAME = 2;
        public const LIST = 3;

        public static function fromInt(int $value): int
        {
            return in_array($value, [self::SIDEBAR, self::BELOW_NAME, self::LIST], true)
                ? $value
                : self::SIDEBAR;
        }

        public static function fromString(string $name): int
        {
            return match (strtolower($name)) {
                'below_name', 'belowname', 'below' => self::BELOW_NAME,
                'list' => self::LIST,
                default => self::SIDEBAR,
            };
        }

        public static function toString(int $slot): string
        {
            return match ($slot) {
                self::BELOW_NAME => 'below_name',
                self::LIST => 'list',
                default => 'sidebar',
            };
        }
    }

    /**
     * PM `pocketmine\scoreboard\ColorRange`: background tint of an
     * objective as RGB. Bedrock has no colour field on
     * `SetDisplayObjectivePacket`, so Rust encodes the nearest vanilla
     * `§` code as a display-name prefix; Java ignores it.
     */
    class ColorRange
    {
        public function __construct(
            public int $r = 255,
            public int $g = 85,
            public int $b = 85,
        ) {
        }

        public static function red(): self
        {
            return new self(255, 85, 85);
        }

        public static function black(): self
        {
            return new self(0, 0, 0);
        }

        public static function white(): self
        {
            return new self(255, 255, 255);
        }

        public static function fromArray(array $rgb): ?self
        {
            if (count($rgb) !== 3) {
                return null;
            }
            return new self((int)$rgb[0], (int)$rgb[1], (int)$rgb[2]);
        }

        /** @return array{0: int, 1: int, 2: int} */
        public function toArray(): array
        {
            return [$this->r, $this->g, $this->b];
        }
    }

    /**
     * PM `pocketmine\scoreboard\ScoreboardLine`: one text with a
     * numeric score (the sort key; higher scores render on top with
     * the default descending order).
     */
    class ScoreboardLine
    {
        public function __construct(
            public int $score = 0,
            public string $text = '',
        ) {
        }

        public function getScore(): int
        {
            return $this->score;
        }

        public function getText(): string
        {
            return $this->text;
        }

        public function setText(string $text): void
        {
            $this->text = $text;
        }

        /**
         * Fake-player id used on the wire (`SetScorePacket` /
         * `SetScoreboardIdentityPacket`); must match Rust
         * `ScoreboardLine::fake_player_id`.
         */
        public function getFakePlayerId(): string
        {
            return "\u{a7}r" . $this->score . "\u{a7}" . $this->text;
        }

        public function toArray(): array
        {
            return ['score' => $this->score, 'text' => $this->text];
        }
    }

    /**
     * PM `pocketmine\scoreboard\ScoreboardObjective`: a named
     * objective displayed in one slot, with lines keyed by score.
     */
    class ScoreboardObjective
    {
        /** @var array<int, ScoreboardLine> */
        private array $lines = [];
        private ?Scoreboard $scoreboard = null;

        public function __construct(
            private string $name,
            private string $displayName,
            private int $slot = ScoreboardSlot::SIDEBAR,
            private ?ColorRange $background = null,
            private int $sortOrder = 1,
        ) {
            $this->slot = ScoreboardSlot::fromInt($this->slot);
        }

        public function getName(): string
        {
            return $this->name;
        }

        public function getDisplayName(): string
        {
            return $this->displayName;
        }

        public function setDisplayName(string $displayName): void
        {
            $this->displayName = $displayName;
            $this->notifyChange();
        }

        public function getSlot(): int
        {
            return $this->slot;
        }

        public function setSlot(int $slot): void
        {
            $this->slot = ScoreboardSlot::fromInt($slot);
            $this->notifyChange();
        }

        public function getBackground(): ?ColorRange
        {
            return $this->background;
        }

        public function getSortOrder(): int
        {
            return $this->sortOrder;
        }

        /** Internal: parent board, set by `Scoreboard::addObjective`. */
        public function setScoreboard(?Scoreboard $scoreboard): void
        {
            $this->scoreboard = $scoreboard;
        }

        /**
         * Adds (or replaces) the line with this score. Returns true
         * when the line set changed.
         */
        public function addScore(int $score, string $text): bool
        {
            $existing = $this->lines[$score] ?? null;
            if ($existing !== null && $existing->text === $text) {
                return false;
            }
            $this->lines[$score] = new ScoreboardLine($score, $text);
            $this->notifyChange();
            return true;
        }

        /** Removes the line with this score. True when present. */
        public function removeScore(int $score): bool
        {
            if (!isset($this->lines[$score])) {
                return false;
            }
            unset($this->lines[$score]);
            $this->notifyChange();
            return true;
        }

        public function getScore(int $score): ?ScoreboardLine
        {
            return $this->lines[$score] ?? null;
        }

        /** @return array<int, ScoreboardLine> in display order */
        public function getLines(): array
        {
            $lines = array_values($this->lines);
            usort($lines, fn (ScoreboardLine $a, ScoreboardLine $b) => $this->sortOrder === 1
                ? $b->score <=> $a->score
                : $a->score <=> $b->score);
            return $lines;
        }

        public function clearLines(): void
        {
            if ($this->lines !== []) {
                $this->lines = [];
                $this->notifyChange();
            }
        }

        private function notifyChange(): void
        {
            $this->scoreboard?->notifyChange();
        }

        public function toArray(): array
        {
            return [
                'name' => $this->name,
                'display_name' => $this->displayName,
                'slot' => $this->slot,
                'sort_order' => $this->sortOrder,
                'background' => $this->background?->toArray() ?? [],
                'lines' => array_map(
                    fn (ScoreboardLine $line) => $line->toArray(),
                    array_values($this->lines),
                ),
            ];
        }
    }

    /**
     * PM `pocketmine\scoreboard\Scoreboard`: a player's whole
     * scoreboard (title + ordered objectives). The PHP side owns the
     * object; every mutation re-emits the full `set_scoreboard`
     * action and Rust diffs it into Bedrock/Java packets (same
     * full-state pattern as `set_health`).
     */
    class Scoreboard
    {
        /** @var array<string, ScoreboardObjective> */
        private array $objectives = [];
        /** @var null|callable(Scoreboard): void */
        private $onChange = null;

        public function __construct(private string $title = '')
        {
        }

        /** Internal: `Player::setScoreboard` installs the emitter. */
        public function setOnChange(?callable $onChange): void
        {
            $this->onChange = $onChange;
        }

        public function notifyChange(): void
        {
            if ($this->onChange !== null) {
                ($this->onChange)($this);
            }
        }

        public function getTitle(): string
        {
            return $this->title;
        }

        public function setTitle(string $title): void
        {
            $this->title = $title;
            $this->notifyChange();
        }

        public function isEmpty(): bool
        {
            return $this->objectives === [];
        }

        public function clear(): void
        {
            if ($this->objectives !== []) {
                $this->objectives = [];
                $this->notifyChange();
            }
        }

        /** Adds (or replaces, by name) an objective. */
        public function addObjective(ScoreboardObjective $objective): void
        {
            $objective->setScoreboard($this);
            $this->objectives[$objective->getName()] = $objective;
            $this->notifyChange();
        }

        /** Removes an objective by name. True when present. */
        public function removeObjective(string $name): bool
        {
            if (!isset($this->objectives[$name])) {
                return false;
            }
            unset($this->objectives[$name]);
            $this->notifyChange();
            return true;
        }

        /** @return array<string, ScoreboardObjective> */
        public function getObjectives(): array
        {
            return $this->objectives;
        }

        public function getObjective(string $name): ?ScoreboardObjective
        {
            return $this->objectives[$name] ?? null;
        }

        /**
         * `Scoreboard::addScore(objective, score, text)`: adds a line
         * to an existing objective. False when the objective is
         * missing.
         */
        public function addScore(string $objectiveName, int $score, string $text): bool
        {
            $objective = $this->objectives[$objectiveName] ?? null;
            return $objective !== null && $objective->addScore($score, $text);
        }

        /** `Scoreboard::removeScore(objective, score)`. */
        public function removeScore(string $objectiveName, int $score): bool
        {
            $objective = $this->objectives[$objectiveName] ?? null;
            return $objective !== null && $objective->removeScore($score);
        }

        /** Wire form consumed by Rust `Scoreboard::from_data`. */
        public function toArray(): array
        {
            return [
                'title' => $this->title,
                'objectives' => array_map(
                    fn (ScoreboardObjective $objective) => $objective->toArray(),
                    array_values($this->objectives),
                ),
            ];
        }
    }
}

namespace Ifera\ScoreHud\libs\_8e069a2a13b3b21b\jackmd\scorefactory {

    use pocketmine\player\Player;
    use pocketmine\scoreboard\Scoreboard;
    use pocketmine\scoreboard\ScoreboardObjective;
    use pocketmine\scoreboard\ScoreboardSlot;

    /**
     * Compat: ScoreFactory (virion "shockwave-like") viene EMBEBIDO en el
     * phar de ScoreHud bajo este namespace con sufijo hash, y su render
     * es a nivel de paquetes (`SetDisplayObjectivePacket` +
     * `NetworkSession::sendDataPacket`) — superficie que el bridge no
     * tiene. Esta clase PRE-CARGADA (los ficheros del runtime se cargan
     * antes del autoload del phar, así que gana sobre la del phar)
     * redirige la API del virion al scoreboard NATIVO del bridge
     * (`pocketmine\scoreboard\Scoreboard`, que Rust difunde vía la
     * acción `set_scoreboard`): ScoreHud renderiza el board real sin
     * tocar el phar.
     *
     * Mapeo con el virion original:
     * - `setObjective(player, displayName)` -> board nativo nuevo con un
     *   objetivo `objective` en SIDEBAR y el displayName como título.
     * - line 1..15 -> `ScoreboardLine(score = line)` (objective en
     *   sort ASC, igual que el SetDisplayObjectivePacket del virion:
     *   score bajo arriba).
     * - `sendObjective`/`sendLine`/`sendLines` -> no-op: el emitter
     *   nativo reenvía el estado completo en cada mutación.
     * - `removeObjective` -> `Player::removeScoreboard()` (oculta el
     *   sidebar; Rust emite los paquetes de borrado).
     */
    final class ScoreFactory
    {
        private const OBJECTIVE_NAME = 'objective';
        private const MAX_LINES = 15;

        /** @var array<string, array{objective: string}> por `uuid bytes` */
        private static array $cache = [];

        public static function setObjective(
            Player $player,
            string $displayName,
            int $slotOrder = 0,
            string $displaySlot = 'sidebar',
            string $objectiveName = self::OBJECTIVE_NAME,
            string $criteriaName = 'dummy'
        ): void {
            self::assertConnected($player, 'objective');
            $objectiveName = $objectiveName !== '' ? $objectiveName : self::OBJECTIVE_NAME;

            $board = new Scoreboard($displayName);
            // sort 0 = ascendente en el shim nativo (score 1 mejor/top),
            // igual que el SetDisplayObjectivePacket::SORT_ORDER del virion.
            $board->addObjective(new ScoreboardObjective(
                $objectiveName,
                $displayName,
                ScoreboardSlot::SIDEBAR,
                null,
                0,
            ));
            $player->setScoreboard($board);
            self::$cache[self::key($player)] = ['objective' => $objectiveName];
        }

        /** Estado ya enviado por `setScoreboard`; envío no-op. */
        public static function sendObjective(Player $player): void
        {
            self::assertConnected($player, 'objective');
        }

        /**
         * Oculta el scoreboard del jugador. `removeCache`
         * (ScoreHud `PlayerSession::close`) no cambia el net effect:
         * el board nativo se limpia en ambos casos.
         */
        public static function removeObjective(Player $player, bool $removeCache = false): void
        {
            self::assertConnected($player, 'objective');
            unset(self::$cache[self::key($player)]);
            $player->removeScoreboard();
        }

        public static function hasObjective(Player $player): bool
        {
            return isset(self::$cache[self::key($player)])
                && $player->getScoreboard()?->getObjective(
                    self::$cache[self::key($player)]['objective']
                ) !== null;
        }

        /**
         * Configura la línea `line` (1..15, arriba->abajo) con `$message`
         * y la emite inmediatamente (state-based). Devuelve un "entry"
         * ligero para compat con `sendLine(...entry)`.
         */
        public static function setScoreLine(Player $player, int $line, string $message): ?object
        {
            self::assertConnected($player, 'score');
            self::assertHasObjective($player, 'score');
            if ($line < 0 || $line > self::MAX_LINES) {
                throw new ScoreFactoryException('Line: ' . $line . ' is out of range');
            }

            $entry = new \stdClass();
            $entry->objectiveName = self::$cache[self::key($player)]['objective'];
            $entry->score = $line;
            $entry->customName = $message;

            $objective = $player->getScoreboard()?->getObjective($entry->objectiveName);
            if ($objective !== null) {
                $objective->addScore($line, $message);
            }
            return $entry;
        }

        /** El set ya emitió el estado; va sin operación (payload ilusorio). */
        public static function sendLine(Player $player, int $line, ?object $entry = null): void
        {
            self::assertConnected($player, 'objective');
            self::assertHasObjective($player, 'objective');
        }

        public static function sendLines(Player $player): void
        {
            self::assertConnected($player, 'objective');
            self::assertHasObjective($player, 'objective');
        }

        /** Quita una línea del objective nativo. */
        public static function removeScoreLine(Player $player, int $line, bool $removeFromCache = true): void
        {
            self::assertConnected($player, 'score');
            self::assertHasObjective($player, 'score');
            $objectiveName = self::$cache[self::key($player)]['objective'];
            $player->getScoreboard()?->getObjective($objectiveName)?->removeScore($line);
        }

        /** @return Player[] (los con objective vivo conocidos por el shim) */
        public static function getActivePlayers(): array
        {
            return [];
        }

        private static function key(Player $player): string
        {
            try {
                $bytes = $player->getUniqueId()->getBytes();
            } catch (\Throwable) {
                $bytes = $player->getName();
            }
            return $bytes !== '' ? $bytes : strtolower($player->getName());
        }

        private static function assertConnected(Player $player, string $what): void
        {
            if (!$player->isConnected()) {
                throw new ScoreFactoryException('Player is not connected.');
            }
        }

        private static function assertHasObjective(Player $player, string $what): void
        {
            if (!self::hasObjective($player)) {
                throw new ScoreFactoryException(
                    'Cannot ' . $what . " to a player without a scoreboard. Please call ScoreFactory::setObjective() beforehand."
                );
            }
        }
    }

    /**
     * Excepción del virion, embebida igual que en el phar de ScoreHud.
     */
    final class ScoreFactoryException extends \RuntimeException
    {
    }
}
