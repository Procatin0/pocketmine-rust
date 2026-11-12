<?php
declare(strict_types=1);

namespace pocketmine\event {
    use pocketmine\player\Player;

    interface Listener
    {
    }

    interface Cancellable
    {
    }

    abstract class Event
    {
        private bool $cancelled = false;

        public function isCancelled(): bool
        {
            return $this->cancelled;
        }

        public function cancel(): void
        {
            $this->cancelled = true;
        }

        public function uncancel(): void
        {
            $this->cancelled = false;
        }

        public function setCancelled(bool $cancelled = true): void
        {
            $this->cancelled = $cancelled;
        }

        public function isCancellable(): bool
        {
            return $this instanceof Cancellable;
        }

        public function getEventName(): string
        {
            $class = static::class;
            $pos = strrpos($class, '\\');
            return $pos === false ? $class : substr($class, $pos + 1);
        }

        /**
         * PM `Event::call(): void`: dispatches this event through the
         * bridge's `PluginManager` — the same machinery the host events
         * use (`PluginManager::dispatchEvent`, priorities/cancellation
         * included). Usado por eventos construidos en PHP (ScoreHud,
         * EconomyLite, etc.): `$ev = new FooEvent(); $ev->call();`.
         */
        public function call(): void
        {
            \pocketmine\plugin\PluginManager::getInstance()->dispatchEvent($this);
        }

        /**
         * Hydrates an event from the JSON payload sent by the Rust core.
         * Concrete events override this method.
         */
        public static function fromArray(array $data): Event
        {
            throw new \BadMethodCallException(static::class . ' does not implement fromArray()');
        }

        public function toArray(): array
        {
            return ['cancelled' => $this->cancelled];
        }

        protected function applyCancelled(array $data): void
        {
            if (array_key_exists('cancelled', $data)) {
                $this->cancelled = (bool)$data['cancelled'];
            }
        }
    }

    final class EventPriority
    {
        public const LOWEST = 0;
        public const LOW = 1;
        public const NORMAL = 2;
        public const HIGH = 3;
        public const HIGHEST = 4;
        public const MONITOR = 5;

        /** @var array<int, string> */
        private const NAMES = [
            self::LOWEST => 'LOWEST',
            self::LOW => 'LOW',
            self::NORMAL => 'NORMAL',
            self::HIGH => 'HIGH',
            self::HIGHEST => 'HIGHEST',
            self::MONITOR => 'MONITOR',
        ];

        public static function getPriority(int $priority): int
        {
            return max(self::LOWEST, min(self::MONITOR, $priority));
        }

        public static function getPriorityName(int $priority): string
        {
            return self::NAMES[self::getPriority($priority)] ?? 'NORMAL';
        }
    }

    class RegisteredListener
    {
        /** @var callable */
        private $callable;

        public function __construct(
            private string $pluginName,
            private string $eventClass,
            private int $priority,
            private bool $ignoreCancelled,
            callable $callable,
            private string $methodName = ''
        ) {
            $this->callable = $callable;
            $this->priority = EventPriority::getPriority($priority);
        }

        public function getPluginName(): string
        {
            return $this->pluginName;
        }

        public function getEventClass(): string
        {
            return $this->eventClass;
        }

        public function getPriority(): int
        {
            return $this->priority;
        }

        public function isIgnoringCancelled(): bool
        {
            return $this->ignoreCancelled;
        }

        public function getCallable(): callable
        {
            return $this->callable;
        }

        public function getMethodName(): string
        {
            return $this->methodName;
        }

        public function callEvent(Event $event): void
        {
            // Runtime contract: cancelled events are skipped unless the listener
            // opted into them (ignoreCancelled = true). MONITOR always runs.
            if ($event->isCancelled() && !$this->ignoreCancelled && $this->priority !== EventPriority::MONITOR) {
                return;
            }
            ($this->callable)($event);
        }
    }

    /**
     * Small serialization helpers shared by the event classes.
     */
    final class EventData
    {
        public static function player(mixed $raw): Player
        {
            if ($raw instanceof Player) {
                return $raw;
            }

            if (is_array($raw)) {
                try {
                    return new Player($raw);
                } catch (\Throwable) {
                    // Fall through to the string constructor / name only shape.
                }
                $name = (string)($raw['name'] ?? $raw['username'] ?? 'Steve');
                try {
                    return new Player($name);
                } catch (\Throwable) {
                    return new Player(['name' => $name]);
                }
            }

            if (is_string($raw) && $raw !== '') {
                try {
                    return new Player($raw);
                } catch (\Throwable) {
                    try {
                        return new Player(['name' => $raw]);
                    } catch (\Throwable) {
                        // Reported below.
                    }
                }
            }

            try {
                return new Player(['name' => 'Steve']);
            } catch (\Throwable $e) {
                throw new \RuntimeException('Unable to build Player: ' . $e->getMessage(), 0, $e);
            }
        }

        public static function playerToArray(Player $player): array
        {
            if (method_exists($player, 'toArray')) {
                try {
                    $data = $player->toArray();
                    if (is_array($data)) {
                        return $data;
                    }
                } catch (\Throwable) {
                    // Fall back to getters.
                }
            }

            $data = ['name' => self::stringify(method_exists($player, 'getName') ? $player->getName() : '')];
            $map = [
                'display_name' => 'getDisplayName',
                'address' => 'getAddress',
                'runtime_id' => 'getRuntimeId',
                'unique_id' => 'getUniqueId',
                'ping' => 'getPing',
                'gamemode' => 'getGamemode',
                'x' => 'getX',
                'y' => 'getY',
                'z' => 'getZ',
                'yaw' => 'getYaw',
                'pitch' => 'getPitch',
            ];
            foreach ($map as $key => $method) {
                if (method_exists($player, $method)) {
                    try {
                        $value = $player->{$method}();
                        if ($value !== null) {
                            $data[$key] = is_object($value) && method_exists($value, 'toString')
                                ? (string)$value->toString()
                                : $value;
                        }
                    } catch (\Throwable) {
                        // Ignore individual getters.
                    }
                }
            }
            if (method_exists($player, 'getWorld')) {
                try {
                    $data['world'] = self::worldName($player->getWorld());
                } catch (\Throwable) {
                    // Ignore.
                }
            }
            return $data;
        }

        public static function worldName(mixed $world): string
        {
            if (is_string($world)) {
                return $world;
            }
            if (is_object($world)) {
                foreach (['getName', 'getFolderName'] as $method) {
                    if (method_exists($world, $method)) {
                        try {
                            return (string)$world->{$method}();
                        } catch (\Throwable) {
                            // Try the next accessor.
                        }
                    }
                }
                if (property_exists($world, 'name')) {
                    return (string)$world->name;
                }
                if (property_exists($world, 'folderName')) {
                    return (string)$world->folderName;
                }
            }
            return '';
        }

        /**
         * @return array{x:float, y:float, z:float, world?:string}
         */
        public static function vectorToArray(mixed $value, mixed $world = null): array
        {
            if (is_array($value)) {
                if (array_key_exists('x', $value) || array_key_exists('y', $value) || array_key_exists('z', $value)) {
                    $out = [
                        'x' => (float)($value['x'] ?? 0.0),
                        'y' => (float)($value['y'] ?? 0.0),
                        'z' => (float)($value['z'] ?? 0.0),
                    ];
                    $w = $value['world'] ?? $world;
                    if ($w !== null && $w !== '') {
                        $out['world'] = self::worldName($w);
                    }
                    return $out;
                }
                if (array_is_list($value) && count($value) >= 3) {
                    return [
                        'x' => (float)$value[0],
                        'y' => (float)$value[1],
                        'z' => (float)$value[2],
                    ];
                }
            }

            if (is_object($value)) {
                $out = ['x' => 0.0, 'y' => 0.0, 'z' => 0.0];
                foreach (['getX' => 'x', 'getY' => 'y', 'getZ' => 'z'] as $method => $key) {
                    if (method_exists($value, $method)) {
                        try {
                            $out[$key] = (float)$value->{$method}();
                        } catch (\Throwable) {
                            // Keep the default.
                        }
                    }
                }
                if (method_exists($value, 'getWorld')) {
                    try {
                        $w = $value->getWorld();
                        if ($w !== null) {
                            $out['world'] = self::worldName($w);
                        }
                    } catch (\Throwable) {
                        // Ignore.
                    }
                }
                return $out;
            }

            $out = ['x' => 0.0, 'y' => 0.0, 'z' => 0.0];
            $w = $world;
            if ($w !== null && $w !== '') {
                $out['world'] = self::worldName($w);
            }
            return $out;
        }

        public static function toPosition(mixed $value, mixed $world = null): \pocketmine\math\Position
        {
            if ($value instanceof \pocketmine\math\Position) {
                return $value;
            }

            $vec = self::vectorToArray($value, $world);
            $worldValue = $vec['world'] ?? $world;
            $resolvedWorld = self::resolveWorld($worldValue);

            if (class_exists(\pocketmine\math\Position::class)) {
                try {
                    return new \pocketmine\math\Position($vec['x'], $vec['y'], $vec['z'], $resolvedWorld);
                } catch (\Throwable) {
                    // Try the shorter constructor shape used by some builds.
                }
                try {
                    return new \pocketmine\math\Position($vec['x'], $vec['y'], $vec['z']);
                } catch (\Throwable) {
                    // Reported below.
                }
            }

            throw new \RuntimeException('Position class is not available');
        }

        /**
         * Normalises a position-like value into the PM `Location` object
         * (`pocketmine\world\Location` in the bridge, the class
         * `Player::getLocation()` returns): feet x/y/z + yaw/pitch + world.
         * Accepts a Location/Position/Vector3 instance, an
         * `{"x","y","z","yaw","pitch","world"}` object array, or a legacy
         * `[x, y, z]` list.
         */
        public static function toLocation(mixed $value, mixed $world = null, ?float $yaw = null, ?float $pitch = null): \pocketmine\world\Location
        {
            if ($value instanceof \pocketmine\world\Location) {
                return $value;
            }

            $vec = self::vectorToArray($value, $world);
            $worldName = self::worldName($vec['world'] ?? $world);

            $resolvedYaw = $yaw;
            $resolvedPitch = $pitch;
            if (is_array($value)) {
                $resolvedYaw ??= isset($value['yaw']) ? (float)$value['yaw'] : null;
                $resolvedPitch ??= isset($value['pitch']) ? (float)$value['pitch'] : null;
            } elseif (is_object($value)) {
                if (method_exists($value, 'getYaw')) {
                    try {
                        $resolvedYaw ??= (float)$value->getYaw();
                    } catch (\Throwable) {
                        // Keep the default.
                    }
                }
                if (method_exists($value, 'getPitch')) {
                    try {
                        $resolvedPitch ??= (float)$value->getPitch();
                    } catch (\Throwable) {
                        // Keep the default.
                    }
                }
            }

            return new \pocketmine\world\Location(
                $vec['x'],
                $vec['y'],
                $vec['z'],
                $resolvedYaw ?? 0.0,
                $resolvedPitch ?? 0.0,
                $worldName
            );
        }

        /**
         * @return array{x:float, y:float, z:float, yaw:float, pitch:float, world:string}
         */
        public static function locationToArray(\pocketmine\world\Location $location): array
        {
            return [
                'x' => $location->getX(),
                'y' => $location->getY(),
                'z' => $location->getZ(),
                'yaw' => $location->getYaw(),
                'pitch' => $location->getPitch(),
                'world' => $location->getWorldName(),
            ];
        }

        public static function resolveWorld(mixed $world): mixed
        {
            if (!is_string($world) || $world === '') {
                return $world;
            }

            if (class_exists(\pocketmine\world\WorldManager::class)) {
                try {
                    $manager = \pocketmine\world\WorldManager::getInstance();
                    foreach (['getWorldByName', 'getWorld', 'getWorldByFolderName'] as $method) {
                        if (method_exists($manager, $method)) {
                            $found = $manager->{$method}($world);
                            if ($found !== null) {
                                return $found;
                            }
                        }
                    }
                } catch (\Throwable) {
                    // Fall back to the string.
                }
            }

            if (class_exists(\pocketmine\Server::class)) {
                try {
                    $server = \pocketmine\Server::getInstance();
                    if (method_exists($server, 'getWorldManager')) {
                        $manager = $server->getWorldManager();
                        foreach (['getWorldByName', 'getWorld'] as $method) {
                            if (is_object($manager) && method_exists($manager, $method)) {
                                $found = $manager->{$method}($world);
                                if ($found !== null) {
                                    return $found;
                                }
                            }
                        }
                    }
                } catch (\Throwable) {
                    // Fall back to the string.
                }
            }

            return $world;
        }

        public static function sender(mixed $raw): \pocketmine\command\CommandSender
        {
            if ($raw instanceof \pocketmine\command\CommandSender) {
                return $raw;
            }

            $type = is_array($raw) ? strtolower((string)($raw['type'] ?? '')) : '';
            $looksPlayer = is_array($raw) && (
                $type === 'player'
                || isset($raw['runtime_id'])
                || isset($raw['unique_id'])
                || isset($raw['x'])
            );

            if (is_object($raw)) {
                if ($raw instanceof Player) {
                    return $raw;
                }
            }

            if ($looksPlayer || ($type === '' && is_array($raw) && isset($raw['name']) && !isset($raw['console']))) {
                try {
                    return self::player($raw);
                } catch (\Throwable) {
                    // Fall back to console below.
                }
            }

            if (is_string($raw) && $raw !== '' && strcasecmp($raw, 'console') !== 0) {
                try {
                    return self::player($raw);
                } catch (\Throwable) {
                    // Fall back to console below.
                }
            }

            $class = \pocketmine\command\ConsoleCommandSender::class;
            if (class_exists($class)) {
                try {
                    /** @var \pocketmine\command\CommandSender $sender */
                    $sender = new $class();
                    return $sender;
                } catch (\Throwable) {
                    try {
                        /** @var \pocketmine\command\CommandSender $sender */
                        $sender = new $class(class_exists(\pocketmine\Server::class) ? \pocketmine\Server::getInstance() : null);
                        return $sender;
                    } catch (\Throwable) {
                        // Reported below.
                    }
                }
            }

            throw new \RuntimeException('ConsoleCommandSender class is not available');
        }

        public static function senderToArray(mixed $sender): mixed
        {
            if ($sender instanceof Player) {
                return self::playerToArray($sender);
            }
            if (is_object($sender)) {
                if (method_exists($sender, 'toArray')) {
                    try {
                        $data = $sender->toArray();
                        if (is_array($data)) {
                            return $data;
                        }
                    } catch (\Throwable) {
                        // Fall back to name/type.
                    }
                }
                $out = [];
                if (method_exists($sender, 'getName')) {
                    try {
                        $out['name'] = (string)$sender->getName();
                    } catch (\Throwable) {
                        // Ignore.
                    }
                }
                $out['type'] = 'console';
                return $out;
            }
            if (is_string($sender)) {
                return ['name' => $sender, 'type' => 'console'];
            }
            return $sender;
        }

        private static function stringify(mixed $value): string
        {
            if (is_object($value) && method_exists($value, 'toString')) {
                return (string)$value->toString();
            }
            return is_scalar($value) ? (string)$value : '';
        }
    }

    final class EventFactory
    {
        /** @var array<string, class-string<Event>> */
        private const EVENTS = [
            'PlayerJoinEvent' => \pocketmine\event\player\PlayerJoinEvent::class,
            'PlayerQuitEvent' => \pocketmine\event\player\PlayerQuitEvent::class,
            'PlayerChatEvent' => \pocketmine\event\player\PlayerChatEvent::class,
            'PlayerCommandPreprocessEvent' => \pocketmine\event\player\PlayerCommandPreprocessEvent::class,
            'PlayerMoveEvent' => \pocketmine\event\player\PlayerMoveEvent::class,
            'PlayerDeathEvent' => \pocketmine\event\player\PlayerDeathEvent::class,
            'PlayerRespawnEvent' => \pocketmine\event\player\PlayerRespawnEvent::class,
            'PlayerInteractEvent' => \pocketmine\event\player\PlayerInteractEvent::class,
            'BlockBreakEvent' => \pocketmine\event\block\BlockBreakEvent::class,
            'BlockPlaceEvent' => \pocketmine\event\block\BlockPlaceEvent::class,
            'ServerCommandEvent' => \pocketmine\event\server\ServerCommandEvent::class,
            'CommandEvent' => \pocketmine\event\server\CommandEvent::class,
        ];

        public static function classOf(string $name): ?string
        {
            $name = ltrim($name, '\\');
            if (isset(self::EVENTS[$name])) {
                return self::EVENTS[$name];
            }

            $pos = strrpos($name, '\\');
            $short = $pos === false ? $name : substr($name, $pos + 1);
            if (isset(self::EVENTS[$short])) {
                return self::EVENTS[$short];
            }

            if (class_exists($name) && is_a($name, Event::class, true)) {
                return $name;
            }

            return null;
        }

        public static function create(string $name, array $data = []): ?Event
        {
            $class = self::classOf($name);
            if ($class === null) {
                return null;
            }

            try {
                $event = $class::fromArray($data);
                return $event instanceof Event ? $event : null;
            } catch (\Throwable $e) {
                fwrite(STDERR, '[PHP/RUNTIME] Failed to create event ' . $name . ': ' . $e->getMessage() . PHP_EOL);
                return null;
            }
        }

        public static function nameOf(Event $event): string
        {
            $class = $event::class;
            $pos = strrpos($class, '\\');
            return $pos === false ? $class : substr($class, $pos + 1);
        }
    }
}

namespace pocketmine\event\player {
    use pocketmine\event\Cancellable;
    use pocketmine\event\Event;
    use pocketmine\event\EventData;
    use pocketmine\player\Player;

    abstract class PlayerEvent extends Event
    {
        protected Player $player;

        public function __construct(Player $player)
        {
            $this->player = $player;
        }

        public function getPlayer(): Player
        {
            return $this->player;
        }
    }

    class PlayerJoinEvent extends PlayerEvent
    {
        private string $joinMessage;

        public function __construct(Player $player, string $joinMessage = '')
        {
            parent::__construct($player);
            $this->joinMessage = $joinMessage;
        }

        public function getJoinMessage(): string
        {
            return $this->joinMessage;
        }

        public function setJoinMessage(?string $joinMessage): void
        {
            $this->joinMessage = $joinMessage ?? '';
        }

        public static function fromArray(array $data): Event
        {
            $event = new self(
                EventData::player($data['player'] ?? null),
                (string)($data['join_message'] ?? $data['joinMessage'] ?? '')
            );
            $event->applyCancelled($data);
            return $event;
        }

        public function toArray(): array
        {
            return [
                'player' => EventData::playerToArray($this->player),
                'join_message' => $this->joinMessage,
            ];
        }
    }

    class PlayerQuitEvent extends PlayerEvent
    {
        private string $quitMessage;
        private string $reason;

        public function __construct(Player $player, string $quitMessage = '', string $reason = 'Disconnected')
        {
            parent::__construct($player);
            $this->quitMessage = $quitMessage;
            $this->reason = $reason;
        }

        public function getQuitMessage(): string
        {
            return $this->quitMessage;
        }

        public function setQuitMessage(?string $quitMessage): void
        {
            $this->quitMessage = $quitMessage ?? '';
        }

        public function getReason(): string
        {
            return $this->reason;
        }

        public function setReason(string $reason): void
        {
            $this->reason = $reason;
        }

        public static function fromArray(array $data): Event
        {
            $event = new self(
                EventData::player($data['player'] ?? null),
                (string)($data['quit_message'] ?? $data['quitMessage'] ?? ''),
                (string)($data['reason'] ?? 'Disconnected')
            );
            $event->applyCancelled($data);
            return $event;
        }

        public function toArray(): array
        {
            return [
                'player' => EventData::playerToArray($this->player),
                'quit_message' => $this->quitMessage,
                'reason' => $this->reason,
            ];
        }
    }

    class PlayerChatEvent extends PlayerEvent implements Cancellable
    {
        private string $message;
        private string $format;
        private ?\pocketmine\player\chat\ChatFormatter $formatter = null;

        /** @var Player[] */
        private array $recipients;

        /**
         * @param Player[] $recipients
         */
        public function __construct(
            Player $player,
            string $message,
            string $format = '<{PLAYER}> {MESSAGE}',
            array $recipients = []
        ) {
            parent::__construct($player);
            $this->message = $message;
            $this->format = $format;
            $this->recipients = $recipients;
        }

        public function getMessage(): string
        {
            return $this->message;
        }

        public function setMessage(string $message): void
        {
            $this->message = $message;
        }

        /** Rendered line: formatter takes precedence over the raw format string. */
        public function getFormat(): string
        {
            if ($this->formatter !== null) {
                return $this->formatter->format($this->player->getName(), $this->message);
            }
            return $this->format;
        }

        public function setFormat(string $format): void
        {
            $this->format = $format;
        }

        public function getFormatter(): ?\pocketmine\player\chat\ChatFormatter
        {
            return $this->formatter;
        }

        public function setFormatter(?\pocketmine\player\chat\ChatFormatter $formatter): void
        {
            $this->formatter = $formatter;
        }

        /** @return Player[] */
        public function getRecipients(): array
        {
            return $this->recipients;
        }

        /** @param Player[] $recipients */
        public function setRecipients(array $recipients): void
        {
            $this->recipients = array_values($recipients);
        }

        public static function fromArray(array $data): Event
        {
            $player = EventData::player($data['player'] ?? null);
            $message = (string)($data['message'] ?? '');
            $format = $data['format'] ?? ('<' . $player->getName() . '> ' . $message);
            $recipients = [];
            $rawRecipients = $data['recipients'] ?? [];
            if (is_array($rawRecipients)) {
                foreach ($rawRecipients as $raw) {
                    try {
                        $recipients[] = EventData::player($raw);
                    } catch (\Throwable) {
                        // Ignore malformed recipients.
                    }
                }
            }

            $event = new self($player, $message, (string)$format, $recipients);
            $event->applyCancelled($data);
            return $event;
        }

        public function toArray(): array
        {
            $recipients = [];
            foreach ($this->recipients as $recipient) {
                // The IPC contract transports recipient names, not player blobs.
                $recipients[] = $recipient->getName();
            }
            return [
                'player' => EventData::playerToArray($this->player),
                'message' => $this->message,
                'format' => $this->getFormat(),
                'recipients' => $recipients,
                'cancelled' => $this->isCancelled(),
            ];
        }
    }

    class PlayerCommandPreprocessEvent extends PlayerEvent implements Cancellable
    {
        private string $message;

        public function __construct(Player $player, string $message)
        {
            parent::__construct($player);
            $this->message = $message;
        }

        public function getMessage(): string
        {
            return $this->message;
        }

        public function setMessage(string $message): void
        {
            $this->message = $message;
        }

        public static function fromArray(array $data): Event
        {
            $event = new self(
                EventData::player($data['player'] ?? null),
                (string)($data['message'] ?? '')
            );
            $event->applyCancelled($data);
            return $event;
        }

        public function toArray(): array
        {
            return [
                'player' => EventData::playerToArray($this->player),
                'message' => $this->message,
                'cancelled' => $this->isCancelled(),
            ];
        }
    }

    /**
     * PM `PlayerMoveEvent(Player $player, Location $from, Location $to)`:
     * `from` is the last location, `to` the new one (both feet-based
     * `Location` objects, like `Player::getLocation()`); plugins may
     * rewrite the destination with `setTo()` (the core then teleports
     * there), and `cancel()` reverts the movement.
     */
    class PlayerMoveEvent extends PlayerEvent implements Cancellable
    {
        private \pocketmine\world\Location $from;
        private \pocketmine\world\Location $to;

        public function __construct(Player $player, mixed $from = null, mixed $to = null)
        {
            parent::__construct($player);
            $this->from = EventData::toLocation($from ?? $player->getLocation());
            $this->to = EventData::toLocation($to ?? $player->getLocation());
        }

        public function getFrom(): \pocketmine\world\Location
        {
            return $this->from;
        }

        public function getTo(): \pocketmine\world\Location
        {
            return $this->to;
        }

        public function setTo(\pocketmine\world\Location $to): void
        {
            $this->to = $to;
        }

        public function hasChangedPosition(): bool
        {
            return $this->from->x !== $this->to->x
                || $this->from->y !== $this->to->y
                || $this->from->z !== $this->to->z;
        }

        public function hasChangedRotation(): bool
        {
            return $this->from->getYaw() !== $this->to->getYaw()
                || $this->from->getPitch() !== $this->to->getPitch();
        }

        public static function fromArray(array $data): Event
        {
            $player = EventData::player($data['player'] ?? null);
            $event = new self(
                $player,
                $data['from'] ?? $player->getLocation(),
                $data['to'] ?? $player->getLocation()
            );
            $event->applyCancelled($data);
            return $event;
        }

        public function toArray(): array
        {
            return [
                'player' => EventData::playerToArray($this->player),
                'from' => EventData::locationToArray($this->from),
                'to' => EventData::locationToArray($this->to),
                'cancelled' => $this->isCancelled(),
            ];
        }
    }

    class PlayerDeathEvent extends PlayerEvent
    {
        private string $deathMessage;

        public function __construct(Player $player, string $deathMessage = '')
        {
            parent::__construct($player);
            $this->deathMessage = $deathMessage;
        }

        public function getDeathMessage(): string
        {
            return $this->deathMessage;
        }

        public function setDeathMessage(?string $deathMessage): void
        {
            $this->deathMessage = $deathMessage ?? '';
        }

        public static function fromArray(array $data): Event
        {
            $event = new self(
                EventData::player($data['player'] ?? null),
                (string)($data['death_message'] ?? $data['deathMessage'] ?? '')
            );
            $event->applyCancelled($data);
            return $event;
        }

        public function toArray(): array
        {
            return [
                'player' => EventData::playerToArray($this->player),
                'death_message' => $this->deathMessage,
            ];
        }
    }

    class PlayerRespawnEvent extends PlayerEvent implements Cancellable
    {
        private mixed $position;
        private bool $respawnAnchor;

        public function __construct(Player $player, mixed $position = [], bool $isRespawnAnchor = false)
        {
            parent::__construct($player);
            $this->respawnAnchor = $isRespawnAnchor;
            if ($position instanceof \pocketmine\math\Position) {
                $this->position = $position;
            } else {
                try {
                    $this->position = EventData::toPosition($position);
                } catch (\Throwable) {
                    $this->position = $position;
                }
            }
        }

        public function getRespawnPosition(): \pocketmine\math\Position
        {
            if ($this->position instanceof \pocketmine\math\Position) {
                return $this->position;
            }
            $this->position = EventData::toPosition($this->position);
            return $this->position;
        }

        public function setRespawnPosition(mixed $position): void
        {
            $this->position = $position instanceof \pocketmine\math\Position
                ? $position
                : EventData::toPosition($position);
        }

        public function isRespawnAnchor(): bool
        {
            return $this->respawnAnchor;
        }

        public static function fromArray(array $data): Event
        {
            $event = new self(
                EventData::player($data['player'] ?? null),
                $data['position'] ?? [],
                (bool)($data['respawn_anchor'] ?? $data['respawnAnchor'] ?? false)
            );
            $event->applyCancelled($data);
            return $event;
        }

        public function toArray(): array
        {
            return [
                'player' => EventData::playerToArray($this->player),
                'position' => EventData::vectorToArray($this->position),
                'cancelled' => $this->isCancelled(),
            ];
        }
    }

    class PlayerInteractEvent extends PlayerEvent implements Cancellable
    {
        public const ACTION_LEFT_CLICK = 0;
        public const ACTION_RIGHT_CLICK = 1;

        private mixed $world;
        private int $x;
        private int $y;
        private int $z;
        private int $action;
        private mixed $block = null;

        public function __construct(
            Player $player,
            mixed $world = '',
            int $x = 0,
            int $y = 0,
            int $z = 0,
            int $action = self::ACTION_RIGHT_CLICK
        ) {
            parent::__construct($player);

            if ($world instanceof \pocketmine\world\Block) {
                $this->block = $world;
                $this->world = method_exists($world, 'getWorld') ? $world->getWorld() : '';
                $this->x = method_exists($world, 'getX') ? (int)$world->getX() : 0;
                $this->y = method_exists($world, 'getY') ? (int)$world->getY() : 0;
                $this->z = method_exists($world, 'getZ') ? (int)$world->getZ() : 0;
                // In the PocketMine constructor the third argument is the action.
                $this->action = $x;
                return;
            }

            $this->world = $world;
            $this->x = $x;
            $this->y = $y;
            $this->z = $z;
            $this->action = $action;
        }

        public function getAction(): int
        {
            return $this->action;
        }

        public function getWorld(): mixed
        {
            return $this->world;
        }

        public function getBlock(): \pocketmine\world\Block
        {
            if ($this->block instanceof \pocketmine\world\Block) {
                return $this->block;
            }

            if (!class_exists(\pocketmine\world\Block::class)) {
                throw new \RuntimeException('Block class is not available');
            }

            $world = EventData::resolveWorld($this->world);
            foreach ([
                [$world, $this->x, $this->y, $this->z],
                [$this->x, $this->y, $this->z],
            ] as $args) {
                try {
                    /** @var \pocketmine\world\Block $block */
                    $block = new \pocketmine\world\Block(...$args);
                    $this->block = $block;
                    return $block;
                } catch (\Throwable) {
                    // Try the next constructor shape.
                }
            }

            throw new \RuntimeException('Unable to construct Block');
        }

        public function getPosition(): \pocketmine\math\Position
        {
            return EventData::toPosition(
                ['x' => $this->x, 'y' => $this->y, 'z' => $this->z, 'world' => EventData::worldName($this->world)],
                $this->world
            );
        }

        public function getFace(): int
        {
            return 0;
        }

        public function getItem(): mixed
        {
            return null;
        }

        public static function fromArray(array $data): Event
        {
            $event = new self(
                EventData::player($data['player'] ?? null),
                $data['world'] ?? '',
                (int)($data['x'] ?? 0),
                (int)($data['y'] ?? 0),
                (int)($data['z'] ?? 0),
                (int)($data['action'] ?? self::ACTION_RIGHT_CLICK)
            );
            $event->applyCancelled($data);
            return $event;
        }

        public function toArray(): array
        {
            return [
                'player' => EventData::playerToArray($this->player),
                'world' => EventData::worldName($this->world),
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
                'action' => $this->action,
                'cancelled' => $this->isCancelled(),
            ];
        }
    }
}

namespace pocketmine\event\block {
    use pocketmine\event\Cancellable;
    use pocketmine\event\Event;
    use pocketmine\event\EventData;
    use pocketmine\player\Player;

    abstract class BlockEvent extends Event
    {
        protected Player $player;
        protected mixed $world;
        protected int $x;
        protected int $y;
        protected int $z;
        protected int $blockId;
        protected mixed $block = null;

        public function __construct(
            mixed $playerOrBlock,
            mixed $worldOrPlayer = null,
            ?int $x = null,
            ?int $y = null,
            ?int $z = null,
            ?int $blockId = null
        ) {
            if ($playerOrBlock instanceof Player) {
                $this->player = $playerOrBlock;
                $this->world = $worldOrPlayer ?? '';
                $this->x = $x ?? 0;
                $this->y = $y ?? 0;
                $this->z = $z ?? 0;
                $this->blockId = $blockId ?? 0;
                return;
            }

            if ($playerOrBlock instanceof \pocketmine\world\Block) {
                $block = $playerOrBlock;
                $this->block = $block;
                $this->player = $worldOrPlayer instanceof Player
                    ? $worldOrPlayer
                    : EventData::player($worldOrPlayer);
                $this->world = method_exists($block, 'getWorld') ? $block->getWorld() : '';
                $this->x = method_exists($block, 'getX') ? (int)$block->getX() : 0;
                $this->y = method_exists($block, 'getY') ? (int)$block->getY() : 0;
                $this->z = method_exists($block, 'getZ') ? (int)$block->getZ() : 0;
                $this->blockId = method_exists($block, 'getId') ? (int)$block->getId() : 0;
                return;
            }

            throw new \InvalidArgumentException('BlockEvent expects a Player or Block as first argument');
        }

        public function getPlayer(): Player
        {
            return $this->player;
        }

        public function getWorld(): mixed
        {
            return $this->world;
        }

        public function getBlockId(): int
        {
            return $this->blockId;
        }

        public function getBlock(): \pocketmine\world\Block
        {
            if ($this->block instanceof \pocketmine\world\Block) {
                return $this->block;
            }

            if (!class_exists(\pocketmine\world\Block::class)) {
                throw new \RuntimeException('Block class is not available');
            }

            $world = EventData::resolveWorld($this->world);
            $attempts = [
                [$world, $this->x, $this->y, $this->z, $this->blockId],
                [$world, $this->x, $this->y, $this->z],
                [$this->x, $this->y, $this->z, $this->blockId],
                [$this->x, $this->y, $this->z],
            ];
            foreach ($attempts as $args) {
                try {
                    /** @var \pocketmine\world\Block $block */
                    $block = new \pocketmine\world\Block(...$args);
                    $this->block = $block;
                    return $block;
                } catch (\Throwable) {
                    // Try the next constructor shape.
                }
            }

            throw new \RuntimeException('Unable to construct Block');
        }

        public function getPosition(): \pocketmine\math\Position
        {
            return EventData::toPosition(
                ['x' => $this->x, 'y' => $this->y, 'z' => $this->z, 'world' => EventData::worldName($this->world)],
                $this->world
            );
        }

        public static function fromArray(array $data): Event
        {
            $event = new static(
                EventData::player($data['player'] ?? null),
                $data['world'] ?? '',
                (int)($data['x'] ?? 0),
                (int)($data['y'] ?? 0),
                (int)($data['z'] ?? 0),
                (int)($data['block_id'] ?? $data['blockId'] ?? 0)
            );
            $event->applyCancelled($data);
            return $event;
        }

        public function toArray(): array
        {
            return [
                'player' => EventData::playerToArray($this->player),
                'world' => EventData::worldName($this->world),
                'x' => $this->x,
                'y' => $this->y,
                'z' => $this->z,
                'block_id' => $this->blockId,
                'cancelled' => $this->isCancelled(),
            ];
        }
    }

    class BlockBreakEvent extends BlockEvent implements Cancellable
    {
    }

    class BlockPlaceEvent extends BlockEvent implements Cancellable
    {
    }
}

namespace pocketmine\event\server {
    use pocketmine\event\Cancellable;
    use pocketmine\event\Event;
    use pocketmine\event\EventData;

    class CommandEvent extends Event implements Cancellable
    {
        protected \pocketmine\command\CommandSender $sender;
        protected string $command;

        public function __construct(\pocketmine\command\CommandSender $sender, string $command)
        {
            $this->sender = $sender;
            $this->command = $command;
        }

        public function getSender(): \pocketmine\command\CommandSender
        {
            return $this->sender;
        }

        public function getCommand(): string
        {
            return $this->command;
        }

        public function setCommand(string $command): void
        {
            $this->command = $command;
        }

        public static function fromArray(array $data): Event
        {
            $event = new static(
                EventData::sender($data['sender'] ?? null),
                (string)($data['command'] ?? '')
            );
            $event->applyCancelled($data);
            return $event;
        }

        public function toArray(): array
        {
            return [
                'sender' => EventData::senderToArray($this->sender),
                'command' => $this->command,
                'cancelled' => $this->isCancelled(),
            ];
        }
    }

    class ServerCommandEvent extends CommandEvent
    {
    }
}
