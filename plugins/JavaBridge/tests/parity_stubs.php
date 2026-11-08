<?php

declare(strict_types=1);

/*
 * Stubs del runtime para la paridad con la API de plugins.
 *
 * Imitan la parte de `pocketmine\*` que hace que un jugador Java sea un
 * jugador más: el registro del servidor, `PlayerChatEvent` /
 * `PlayerJoinEvent` / `PlayerQuitEvent` y la bandeja donde los plugins dejan
 * sus líneas. El `Bridge` apunta cada llamada al host y sirve las líneas
 * pendientes, así que las pruebas pueden comprobar el recorrido completo
 * (registro -> evento -> acción -> línea en el socket) sin el servidor Rust.
 *
 * El primer parámetro que se registra y el orden importan: `$calls` es la
 * traza real de lo que el puente pidió al servidor.
 */

namespace pocketmine\runtime {

	final class Bridge
	{
		// En el servidor el puente corre dentro de una llamada del host, así
		// que la API está disponible; las pruebas lo modelan igual.
		public static bool $active = true;

		/** @var list<array{0:string, 1:array}> */
		public static array $calls = [];

		/** @var array<string, list<string>> lower-case name => lines que esperan al cliente */
		public static array $messages = [];

		/**
		 * Cola (Fase 1) que `world.take_block_changes` / `world.drain_changes`
		 * devuelven y drenan, como haría Rust cuando la implemente.
		 *
		 * @var list<array>
		 */
		public static array $blockChanges = [];

		/** false = el host no implementa la cola (hostCall devuelve null). */
		public static bool $blockChangesAvailable = true;

		/**
		 * Snapshots (Fase 2, Bedrock visible para Java) que
		 * `server.get_players_full` devuelve y drena, como haría Rust.
		 *
		 * @var list<array>
		 */
		public static array $playerSnapshots = [];

		public static function isActive(): bool
		{
			return self::$active;
		}

		public static function setActive(bool $active): void
		{
			self::$active = $active;
		}

		public static function hostCall(string $method, array $params = []): mixed
		{
			if (!self::$active) {
				return null;
			}
			self::$calls[] = [$method, $params];

			if ($method === 'player.take_messages') {
				$key = strtolower((string)($params['name'] ?? ''));
				$lines = self::$messages[$key] ?? [];
				self::$messages[$key] = [];
				return ['messages' => $lines];
			}

			if ($method === 'world.take_block_changes' || $method === 'world.drain_changes') {
				if (!self::$blockChangesAvailable) {
					return null;
				}
				$changes = self::$blockChanges;
				self::$blockChanges = [];
				return ['changes' => $changes];
			}

			if ($method === 'server.get_players_full') {
				return self::$playerSnapshots;
			}

			return ['ok' => true];
		}

		/** @return list<array> */
		public static function callsTo(string $method): array
		{
			$out = [];
			foreach (self::$calls as [$called, $params]) {
				if ($called === $method) {
					$out[] = $params;
				}
			}
			return $out;
		}

		public static function reset(): void
		{
			self::$calls = [];
			self::$messages = [];
			self::$blockChanges = [];
			self::$blockChangesAvailable = true;
			self::$playerSnapshots = [];
		}
	}
}

namespace pocketmine\player {

	final class Player
	{
		/** @var list<string> */
		public array $received = [];

		private string $name;
		private string $world;

		/** Gamemode de la copia API (JavaServer lo lee/escribe como en el runtime real). */
		public int $gamemode = 0;

		/**
		 * Posición de la copia API, como en el runtime real
		 * (`Player::$x/$y/$z/$yaw/$pitch` públicos): `JavaServer`
		 * los actualiza con `updatePlayerPosition()` y `syncTeleport`
		 * los lee. Antes el stub devolvía siempre el spawn en
		 * `getX/Y/Z`, así que tras un `/tp` el sync veía el apiPlayer
		 * "distinto" de `lastSentPos` y re-teleportaba al spawn
		 * (test de `/tp` intermitente según el tick de sync).
		 */
		public float $x = 0.5;
		public float $y = 5.62;
		public float $z = 0.5;
		public float $yaw = 0.0;
		public float $pitch = 0.0;
		public int $ping = 0;

		public function __construct(array|string $data = [])
		{
			if (is_string($data)) {
				$data = ['name' => $data];
			}
			$this->name = (string)($data['name'] ?? $data['username'] ?? 'Steve');
			$this->world = (string)($data['world'] ?? 'world');
			$this->gamemode = (int)($data['gamemode'] ?? 0);
			$this->x = (float)($data['x'] ?? 0.5);
			$this->y = (float)($data['y'] ?? 5.62);
			$this->z = (float)($data['z'] ?? 0.5);
			$this->yaw = (float)($data['yaw'] ?? 0.0);
			$this->pitch = (float)($data['pitch'] ?? 0.0);
			$this->ping = (int)($data['ping'] ?? 0);
		}

		public function getName(): string
		{
			return $this->name;
		}

		public function getLowerCaseName(): string
		{
			return strtolower($this->name);
		}

		public function getDisplayName(): string
		{
			return $this->name;
		}

		public function getGamemode(): int
		{
			return $this->gamemode;
		}

		public function setGamemode(int $gamemode): void
		{
			$this->gamemode = $gamemode;
		}

		public function getHealth(): float
		{
			return 20.0;
		}

		public function getMaxHealth(): float
		{
			return 20.0;
		}

		public function getPing(): int
		{
			return $this->ping;
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

		public function getYaw(): float
		{
			return $this->yaw;
		}

		public function getPitch(): float
		{
			return $this->pitch;
		}

		public function getWorld(): object
		{
			$name = $this->world;
			return new class($name) {
				public function __construct(private string $name)
				{
				}

				public function getName(): string
				{
					return $this->name;
				}
			};
		}

		public function isOp(): bool
		{
			return false;
		}

		public function sendMessage(string $message): void
		{
			$this->received[] = $message;
		}
	}
}

namespace pocketmine\plugin {

	final class PluginManager
	{
		private static ?PluginManager $instance = null;

		/** @var list<object> */
		public static array $events = [];

		public static function getInstance(): PluginManager
		{
			return self::$instance ??= new self();
		}

		public function dispatchEvent(object $event): void
		{
			self::$events[] = $event;
		}

		public static function reset(): void
		{
			self::$events = [];
		}

		public static function last(string $class): ?object
		{
			for ($i = count(self::$events) - 1; $i >= 0; $i--) {
				if (self::$events[$i] instanceof $class) {
					return self::$events[$i];
				}
			}
			return null;
		}
	}
}

namespace pocketmine\command {

	final class CommandMap
	{
		/** @var list<string> */
		public array $lines = [];

		public function dispatch(object $sender, string $line): bool
		{
			$this->lines[] = $line;
			return false;
		}
	}
}

namespace pocketmine\event\player {

	use pocketmine\player\Player;

	class PlayerChatEvent
	{
		private bool $cancelled = false;

		/** @param Player[] $recipients */
		public function __construct(
			private Player $player,
			private string $message,
			private string $format = '',
			private array $recipients = []
		) {
		}

		public function getPlayer(): Player
		{
			return $this->player;
		}

		public function getMessage(): string
		{
			return $this->message;
		}

		public function setMessage(string $message): void
		{
			$this->message = $message;
		}

		public function getFormat(): string
		{
			return $this->format;
		}

		public function setFormat(string $format): void
		{
			$this->format = $format;
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

		public function isCancelled(): bool
		{
			return $this->cancelled;
		}

		public function setCancelled(bool $cancelled = true): void
		{
			$this->cancelled = $cancelled;
		}
	}

	class PlayerJoinEvent
	{
		public function __construct(private Player $player, private string $joinMessage = '')
		{
		}

		public function getPlayer(): Player
		{
			return $this->player;
		}

		public function getJoinMessage(): string
		{
			return $this->joinMessage;
		}

		public function setJoinMessage(?string $joinMessage): void
		{
			$this->joinMessage = $joinMessage ?? '';
		}
	}

	class PlayerQuitEvent
	{
		public function __construct(
			private Player $player,
			private string $quitMessage = '',
			private string $reason = 'Disconnected'
		) {
		}

		public function getPlayer(): Player
		{
			return $this->player;
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
	}
}
