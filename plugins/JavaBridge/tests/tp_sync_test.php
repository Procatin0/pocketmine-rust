<?php

declare(strict_types=1);

/*
 * Verificación de /tp para jugador Java (protocolo legacy 1.12.2, posición
 * en el hilo sin servidor Rust): `syncTeleport` mueve al JavaClient cuando
 * el registry de Rust (consola `/tp`) o la copia apiPlayer (plugin) cambian,
 * sin confundir caminar con teleportear.
 */

namespace pocketmine\world {
	final class Location{
		public function __construct(private float $x, private float $y, private float $z){}
		public function getX() : float{ return $this->x; }
		public function getY() : float{ return $this->y; }
		public function getZ() : float{ return $this->z; }
	}
	final class Block{
		public function getId() : int{ return 1; }
		public function setBlock(int $legacyId) : bool{ return true; }
	}
	final class Chunk{
		public function getSectionPayload(int $baseY) : ?array{ return null; }
		public function getHighestBlockAt(int $x, int $z) : ?int{ return 3; }
	}
	final class World{
		public function getName() : string{ return 'world'; }
		public function getSpawnLocation() : Location{ return new Location(0.0, 4.0, 0.0); }
		public function getPlayerSpawnLocation() : ?array{ return [0.5, 5.62, 0.5]; }
		public function getBlockAt(int $x, int $y, int $z) : Block{ return new Block(); }
		public function getTime() : int{ return 6000; }
		public function getChunk(int $chunkX, int $chunkZ) : Chunk{ return new Chunk(); }
	}
}
namespace pocketmine\plugin {
	class PluginBase{
		public function getServer() : \pocketmine\Server{ return \pocketmine\Server::instance(); }
		public function getLogger() : object{
			return new class{
				public function info(string $message) : void{}
				public function error(string $message) : void{}
			};
		}
		public function getConfig() : object{
			return new class{ public function get(string $key, mixed $default = null) : mixed{ return $default; } };
		}
		public function getScheduler() : object{ return new class{ public function scheduleRepeatingTask(mixed $task, int $period) : void{} }; }
		public function saveDefaultConfig() : void{}
	}
	final class PluginManager{
		public static function getInstance() : PluginManager{ return new PluginManager(); }
		public function dispatchEvent(object $event) : void{}
	}
}
namespace pocketmine\scheduler {
	final class ClosureTask{ public function __construct(mixed $fn = null){} }
}
namespace pocketmine\event {
	interface Listener{}
}
namespace pocketmine\command {
	final class CommandMap{
		/** @return bool */
		public function dispatch(object $sender, string $command) : bool{ return false; }
		public function register(string $name, object $command) : void{}
	}
}
namespace pocketmine\player {
	final class Player{
		public int $gamemode = 0;
		public float $x = 0.0;
		public float $y = 64.0;
		public float $z = 0.0;
		public float $yaw = 0.0;
		public float $pitch = 0.0;
		public float $health = 20.0;
		public float $maxHealth = 20.0;
		private string $name = 'Steve';
		private string $displayName = 'Steve';
		private string $worldName = 'world';
		public function __construct(array|string $data = ['name' => 'Steve']){
			if(is_string($data)){ $data = ['name' => $data]; }
			$this->name = (string)($data['name'] ?? 'Steve');
			$this->displayName = (string)($data['display_name'] ?? $this->name);
			$this->worldName = (string)($data['world'] ?? 'world');
			$this->x = (float)($data['x'] ?? 0.0);
			$this->y = (float)($data['y'] ?? 64.0);
			$this->z = (float)($data['z'] ?? 0.0);
			$this->yaw = (float)($data['yaw'] ?? 0.0);
			$this->pitch = (float)($data['pitch'] ?? 0.0);
			$this->gamemode = (int)($data['gamemode'] ?? 0);
			$this->health = (float)($data['health'] ?? 20.0);
			$this->maxHealth = (float)($data['max_health'] ?? 20.0);
		}
		public function getName() : string{ return $this->name; }
		public function getDisplayName() : string{ return $this->displayName; }
		public function getGamemode() : int{ return $this->gamemode; }
		public function setGamemode(int $gamemode) : void{ $this->gamemode = $gamemode; }
		public function getX() : float{ return $this->x; }
		public function getY() : float{ return $this->y; }
		public function getZ() : float{ return $this->z; }
		public function getYaw() : float{ return $this->yaw; }
		public function getPitch() : float{ return $this->pitch; }
		public function getHealth() : float{ return $this->health; }
		public function getMaxHealth() : float{ return $this->maxHealth; }
		public function isOp() : bool{ return true; }
		public function save(bool $isQuit = true) : bool{ return true; }
		public function getWorld() : \pocketmine\world\World{ return new \pocketmine\world\World(); }
	}
}
namespace pocketmine\runtime {
	/** Registro virtual de Rust (`player.get`/`player.update_position`/`player.register`). */
	final class Bridge{
		public static bool $active = true;
		/** @var array<string, array> */
		public static array $registry = [];
		public static array $calls = [];
		public static function isActive() : bool{ return self::$active; }
		public static function setActive(bool $active = true) : void{ self::$active = $active; }
		public static function hostCall(string $method, array $params = []) : mixed{
			if(!self::$active){ return null; }
			self::$calls[] = [$method, $params];
			$name = strtolower((string)($params['name'] ?? ''));
			switch($method){
				case 'player.get':
					return self::$registry[$name] ?? null;
				case 'player.update_position':
					if(!isset(self::$registry[$name]) || (self::$registry[$name]['is_virtual'] ?? true) === false){
						return ['ok' => true, 'found' => false];
					}
					foreach(['x', 'y', 'z', 'yaw', 'pitch'] as $key){
						if(isset($params[$key])){ self::$registry[$name][$key] = (float)$params[$key]; }
					}
					return ['ok' => true, 'found' => true];
				case 'player.register':
					self::$registry[$name] = [
						'name' => (string)($params['name'] ?? ''), 'display_name' => (string)($params['display_name'] ?? $params['name'] ?? ''),
						'address' => (string)($params['address'] ?? ''), 'gamemode' => (int)($params['gamemode'] ?? 0),
						'x' => (float)($params['x'] ?? 0.0), 'y' => (float)($params['y'] ?? 64.0), 'z' => (float)($params['z'] ?? 0.0),
						'yaw' => (float)($params['yaw'] ?? 0.0), 'pitch' => (float)($params['pitch'] ?? 0.0),
						'world' => (string)($params['world'] ?? 'world'), 'health' => (float)($params['health'] ?? 20.0),
						'max_health' => (float)($params['max_health'] ?? 20.0), 'is_op' => (bool)($params['is_op'] ?? false),
						'is_virtual' => true,
					];
					return ['ok' => true];
				case 'player.unregister':
					unset(self::$registry[$name]);
					return ['ok' => true];
				case 'player.take_messages':
					return ['messages' => []];
				case 'player.load':
					return ['has_data' => false, 'exists' => false];
				default:
					return ['ok' => true];
			}
		}
		public static function reset() : void{ self::$registry = []; self::$calls = []; }
	}
}
namespace pocketmine {
	final class Server{
		public array $broadcasts = [];
		private static ?Server $instance = null;
		public static function instance() : Server{ return self::$instance ??= new self(); }
		/** @var array<string, \pocketmine\player\Player> */
		public array $online = [];
		public function getOnlinePlayers() : array{ return array_values($this->online); }
		public function addPlayer(\pocketmine\player\Player $player) : void{ $this->online[strtolower($player->getName())] = $player; }
		public function removePlayer(\pocketmine\player\Player $player) : void{ unset($this->online[strtolower($player->getName())]); }
		public function getPlayerExact(string $name) : ?object{ return $this->online[strtolower($name)] ?? null; }
		public function getPluginManager() : \pocketmine\plugin\PluginManager{ return \pocketmine\plugin\PluginManager::getInstance(); }
		public function getCommandMap() : \pocketmine\command\CommandMap{ return new \pocketmine\command\CommandMap(); }
		public function getDefaultWorld() : \pocketmine\world\World{ return new \pocketmine\world\World(); }
		public function getMaxPlayers() : int{ return 20; }
		public function broadcastMessage(string $message) : void{ $this->broadcasts[] = $message; }
	}
}
namespace {
	require __DIR__ . '/../src/javabridge/Protocol.php';
	require __DIR__ . '/../src/javabridge/Nbt.php';
	require __DIR__ . '/../src/javabridge/ChunkBuilder.php';
	require __DIR__ . '/../src/javabridge/ModernProfile.php';
	require __DIR__ . '/../src/javabridge/ModernChunk.php';
	require __DIR__ . '/../src/javabridge/JavaClient.php';
	require __DIR__ . '/../src/javabridge/JavaServer.php';
	require __DIR__ . '/../src/javabridge/Main.php';

	use javabridge\JavaClient;
	use javabridge\JavaServer;
	use javabridge\Main;
	use javabridge\Protocol;
	use pocketmine\runtime\Bridge;

	$failures = 0;
	function check(string $name, bool $ok, string $detail = '') : void{
		global $failures;
		if(!$ok){
			$failures++;
			fwrite(STDERR, "FAIL: $name" . ($detail === '' ? '' : " ($detail)") . "\n");
		}else{
			fwrite(STDOUT, "ok: $name\n");
		}
	}
	function setProp(object $obj, string $prop, mixed $value) : void{
		$ref = new ReflectionProperty($obj::class, $prop);
		$ref->setAccessible(true);
		$ref->setValue($obj, $value);
	}
	/** @return list<array{0:int,1:string}> */
	function framesFrom(string &$buffer) : array{
		$frames = [];
		while(true){
			$offset = 0;
			$length = Protocol::readVarInt($buffer, $offset);
			if($length === null || strlen($buffer) - $offset < $length){ break; }
			$body = substr($buffer, $offset, $length);
			$buffer = (string)substr($buffer, $offset + $length);
			$bodyOffset = 0;
			$id = Protocol::readVarInt($body, $bodyOffset);
			if($id === null){ break; }
			$frames[] = [$id, substr($body, $bodyOffset)];
		}
		return $frames;
	}
	function drain($clientSide) : string{
		usleep(20000);
		$bytes = @fread($clientSide, 65536);
		return $bytes === false ? '' : $bytes;
	}
	function positionFrames(string $buffer) : array{
		$pos = [];
		foreach(framesFrom($buffer) as [$id, $payload]){
			if($id !== Protocol::S2C_POSITION){ continue; }
			$off = 0;
			$x = Protocol::readDouble($payload, $off);
			$y = Protocol::readDouble($payload, $off);
			$z = Protocol::readDouble($payload, $off);
			if($x !== null && $y !== null && $z !== null){ $pos[] = [$x, $y, $z]; }
		}
		return $pos;
	}

	// Par TCP real: un extremo para el JavaClient, el otro para leer lo enviado.
	$listen = stream_socket_server('tcp://127.0.0.1:0', $errno, $errstr);
	if($listen === false){ fwrite(STDERR, "listen: $errstr\n"); exit(1); }
	$listenName = stream_socket_get_name($listen, false);
	$clientSide = stream_socket_client('tcp://' . $listenName, $errno, $errstr, 2.0);
	if($clientSide === false){ fwrite(STDERR, "connect: $errstr\n"); exit(1); }
	stream_set_blocking($clientSide, false);
	$serverSide = stream_socket_accept($listen, 2.0);
	if($serverSide === false){ fwrite(STDERR, "accept: $errstr\n"); exit(1); }
	stream_set_blocking($serverSide, false);
	fclose($listen);

	$main = (new ReflectionClass(Main::class))->newInstanceWithoutConstructor();
	$server = new JavaServer($main, '127.0.0.1', 0, 1);
	$client = new JavaClient($serverSide, '127.0.0.1:1234');
	setProp($client, 'username', 'Procatin0');
	setProp($client, 'playing', true);
	setProp($client, 'x', 10.0);
	setProp($client, 'y', 64.0);
	setProp($client, 'z', 10.0);
	// Dos sistemas de yaw: el cliente y `lastSentPos` van en yaw Java,
	// el registry de Rust y la copia `apiPlayer` en yaw Bedrock (la
	// conversión es +180: Bedrock 0 = norte se ve como Java 180).
	setProp($client, 'yaw', 180.0);
	setProp($client, 'lastSentPos', [10.0, 64.0, 10.0, 180.0, 0.0]);

	Bridge::reset();
	Bridge::hostCall('player.register', [
		'name' => 'Procatin0', 'display_name' => 'Procatin0', 'address' => '127.0.0.1:1234',
		'gamemode' => 0, 'x' => 10.0, 'y' => 64.0, 'z' => 10.0, 'yaw' => 0.0, 'pitch' => 0.0,
		'world' => 'world', 'health' => 20.0, 'max_health' => 20.0, 'is_op' => false,
	]);
	$api = $server->apiPlayer($client);
	check('apiPlayer creado', $api !== null && $api->getX() === 10.0);

	// 1) Rust movió el registry (consola `/tp Procatin0 0 100 0`): sync mueve al Java.
	$reg = Bridge::$registry['procatin0'];
	$reg['x'] = 0.0;
	$reg['y'] = 100.0;
	$reg['z'] = 0.0;
	Bridge::$registry['procatin0'] = $reg;
	check('pending antes de sync (registry)', $server->hasTeleportPending($client));
	drain($clientSide);
	$server->syncTeleport($client);
	check('cliente movido por sync (registry)', $client->getX() === 0.0 && $client->getY() === 100.0 && $client->getZ() === 0.0,
		'pos=' . $client->getX() . ',' . $client->getY() . ',' . $client->getZ());
	$pos = positionFrames(drain($clientSide));
	check('paquete position 0,164,0 (legacy +64)', count($pos) === 1 && abs($pos[0][0]) < 0.001 && abs($pos[0][1] - 164.0) < 0.001 && abs($pos[0][2]) < 0.001,
		'frames=' . json_encode($pos));
	check('apiPlayer copiado del registry', $api->getX() === 0.0 && $api->getY() === 100.0 && $api->getZ() === 0.0);
	check('lastSentPos actualizado tras teleport', $client->getLastSentPos() === [0.0, 100.0, 0.0, 180.0, 0.0]);

	// 2) Caminar no dispara teleport.
	setProp($client, 'x', 0.5);
	setProp($client, 'z', 0.5);
	drain($clientSide);
	$server->syncTeleport($client);
	$extra = drain($clientSide);
	check('caminar no re-teleporta', $client->getX() === 0.5 && positionFrames($extra) === [] && !$server->hasTeleportPending($client));

	// 3) Plugin movió apiPlayer: sync mueve al Java y persiste el destino.
	setProp($client, 'x', 0.0);
	setProp($client, 'z', 0.0);
	$api->x = 5.0;
	$api->y = 70.0;
	$api->z = -3.0;
	drain($clientSide);
	$server->syncTeleport($client);
	check('cliente movido por sync (apiPlayer)', $client->getX() === 5.0 && $client->getY() === 70.0 && $client->getZ() === -3.0);
	$pos = positionFrames(drain($clientSide));
	check('paquete position 5,134,-3', count($pos) === 1 && abs($pos[0][0] - 5.0) < 0.001 && abs($pos[0][1] - 134.0) < 0.001 && abs($pos[0][2] + 3.0) < 0.001, 'frames=' . json_encode($pos));
	$reg = Bridge::$registry['procatin0'];
	check('registry persistido tras teleport apiPlayer', $reg['x'] === 5.0 && $reg['y'] === 70.0 && $reg['z'] === -3.0);

	// 4) Guard: push throttled no aplasta un destino nuevo del registry.
	$reg = Bridge::$registry['procatin0'];
	$reg['x'] = 0.0;
	$reg['y'] = 100.0;
	$reg['z'] = 0.0;
	Bridge::$registry['procatin0'] = $reg;
	setProp($client, 'x', 6.0); // paseo del cliente desde 5,70,-3
	setProp($client, 'tickCount', 30);
	setProp($client, 'lastPosUpdateTick', 0);
	$maybe = new ReflectionMethod(JavaClient::class, 'maybeUpdatePosition');
	$maybe->setAccessible(true);
	$maybe->invoke($client, $server);
	$reg = Bridge::$registry['procatin0'];
	check('push no aplasta /tp pendiente', $reg['x'] === 0.0 && $reg['y'] === 100.0 && $reg['z'] === 0.0 && $client->getLastSentPos() === [5.0, 70.0, -3.0, 180.0, 0.0]);
	drain($clientSide);
	$server->syncTeleport($client);
	check('sync aplica /tp tras paseo', $client->getX() === 0.0 && $client->getY() === 100.0 && $client->getZ() === 0.0);
	$pos = positionFrames(drain($clientSide));
	check('paquete position tras paseo 0,164,0', count($pos) === 1 && abs($pos[0][1] - 164.0) < 0.001, 'frames=' . json_encode($pos));

	// 5) Moderno (775 / 26.1): el mismo sync manda `position` sin +64 y
	//    pide los chunks nuevos (update_view_position).
	$profile = \javabridge\ModernProfile::forProtocol(775);
	check('perfil 775 disponible', $profile !== null);
	setProp($client, 'protocol', 775);
	setProp($client, 'profile', $profile);
	setProp($client, 'x', 5.0);
	setProp($client, 'y', 70.0);
	setProp($client, 'z', -3.0);
	setProp($client, 'lastSentPos', [5.0, 70.0, -3.0, 180.0, 0.0]);
	$api->x = 5.0;
	$api->y = 70.0;
	$api->z = -3.0;
	// Registry en el destino de consola; sync debe mover al moderno.
	$reg = Bridge::$registry['procatin0'];
	$reg['x'] = 0.0;
	$reg['y'] = 100.0;
	$reg['z'] = 0.0;
	Bridge::$registry['procatin0'] = $reg;
	drain($clientSide);
	$server->syncTeleport($client);
	check('moderno movido por sync', $client->getX() === 0.0 && $client->getY() === 100.0 && $client->getZ() === 0.0);
	$raw = drain($clientSide);
	$modernPos = [];
	$viewPos = [];
	foreach(framesFrom($raw) as [$id, $payload]){
		if($id === $profile->id('position')){
			$off = 0;
			Protocol::readVarInt($payload, $off);
			$mx = Protocol::readDouble($payload, $off);
			$my = Protocol::readDouble($payload, $off);
			$mz = Protocol::readDouble($payload, $off);
			if($mx !== null && $my !== null && $mz !== null){ $modernPos[] = [$mx, $my, $mz]; }
		}
		if($id === $profile->id('update_view_position')){ $viewPos[] = true; }
	}
	check('moderno position 0,100,0', count($modernPos) === 1 && abs($modernPos[0][0]) < 0.001 && abs($modernPos[0][1] - 100.0) < 0.001 && abs($modernPos[0][2]) < 0.001,
		'frames=' . json_encode($modernPos));
	check('moderno pide update_view_position (chunks nuevos)', $viewPos !== []);

	fclose($clientSide);
	fclose($serverSide);

	fwrite(STDOUT, $failures === 0 ? "ALL OK\n" : "$failures FAILURES\n");
	exit($failures === 0 ? 0 : 1);
}
