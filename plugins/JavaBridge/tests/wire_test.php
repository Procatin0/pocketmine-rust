<?php

declare(strict_types=1);

/*
 * Prueba de red del plugin: abre el listener real en loopback, habla el
 * protocolo con sockets reales y comprueba los paquetes que devuelve.
 *
 * Los stubs de pocketmine\* imitan el runtime PHP de PocketMine-Rust
 * (crates/php-plugin-bridge/php) sin necesitar el servidor Rust.
 */

namespace pocketmine\world {

	final class Location{
		public function __construct(private float $x, private float $y, private float $z){}
		public function getX() : float{ return $this->x; }
		public function getY() : float{ return $this->y; }
		public function getZ() : float{ return $this->z; }
	}

	/** Bloque con estado compartido (Fase 2): ver modern_test.php. */
	final class Block{
		/** @var array<string, int> */
		public static array $state = [];

		public function __construct(private int $bx = 0, private int $by = 0, private int $bz = 0){}

		private function key() : string{ return $this->bx . ',' . $this->by . ',' . $this->bz; }

		public function getId() : int{
			return self::$state[$this->key()] ?? (($this->bx === 0 && $this->by === 4 && $this->bz === 0) ? 1 : 0);
		}

		public function getNetworkId() : int{
			return $this->getId() === 0 ? 3690217760 : 2150698529;
		}

		public function setBlock(int $legacyId) : bool{
			self::$state[$this->key()] = $legacyId;
			return true;
		}
	}

	/** Columna de Bedrock de prueba: bedrock, piedra, tierra y hierba. */
	final class Chunk{
		/** @var array<int, array{bits:int, words:array<int,int>, palette:array<int,int>}> */
		private array $sections = [];

		public function __construct(){
			$this->sections[0] = ['bits' => 0, 'words' => [], 'palette' => [4121722107]]; // bedrock en Y=-64

			$bits = 2;
			$perWord = intdiv(32, $bits);
			$words = array_fill(0, 256, 0);
			for($x = 0; $x < 16; $x++){
				for($z = 0; $z < 16; $z++){
					for($y = 0; $y < 16; $y++){
						$entry = ($x << 8) | ($z << 4) | $y;
						$slot = $y < 10 ? 0 : ($y < 14 ? 1 : ($y === 14 ? 2 : 3));
						$words[intdiv($entry, $perWord)] |= $slot << (($entry % $perWord) * $bits);
					}
				}
			}
			// piedra, tierra, hierba y aire en la subsección Y=-48
			$this->sections[1] = ['bits' => $bits, 'words' => $words,
				'palette' => [2150698529, 2186211206, 3727763636, 3690217760]];
		}

		public function getSectionPayload(int $baseY) : ?array{
			return $this->sections[intdiv($baseY + 64, 16)] ?? null;
		}

		public function getHighestBlockAt(int $x, int $z) : ?int{ return -34; }
	}

	final class World{
		public function getName() : string{ return 'world'; }
		public function getSpawnLocation() : Location{ return new Location(0.0, 4.0, 0.0); }
		public function getPlayerSpawnLocation() : ?array{ return [0.5, 5.62, 0.5]; }
		public function getHighestBlockAt(int $x, int $z) : int{ return 3; }
		public function getBlockAt(int $x, int $y, int $z) : Block{ return new Block($x, $y, $z); }
		public function getTime() : int{ return 6000; }
		public function getChunk(int $chunkX, int $chunkZ) : Chunk{ return new Chunk(); }
	}
}

namespace pocketmine\plugin {

	class PluginBase{
		public function getServer() : \pocketmine\Server{ return \pocketmine\Server::instance(); }

		public function getLogger() : object{
			return new class{
				public function info(string $message) : void{ echo "[log] $message\n"; }
				public function error(string $message) : void{ echo "[error] $message\n"; }
			};
		}

		public function getConfig() : object{
			return new class{
				public function get(string $key, mixed $default = null) : mixed{ return $default; }
			};
		}

		public function saveDefaultConfig() : void{}
	}
}

namespace pocketmine\event {
	interface Listener{}
}

namespace pocketmine {

	final class Server{
		/** @var string[] */
		public array $broadcasts = [];

		private static ?Server $instance = null;

		public static function instance() : Server{ return self::$instance ??= new self(); }

		/** @var array<string, \pocketmine\player\Player> */
		public array $online = [];

		private static ?\pocketmine\command\CommandMap $commandMap = null;

		public function getOnlinePlayers() : array{ return $this->online; }

		public function addPlayer(\pocketmine\player\Player $player) : void{
			$this->online[strtolower($player->getName())] = $player;
		}

		public function removePlayer(\pocketmine\player\Player $player) : void{
			unset($this->online[strtolower($player->getName())]);
		}

		public function getPlayerExact(string $name) : ?object{ return $this->online[strtolower($name)] ?? null; }

		public function getPluginManager() : \pocketmine\plugin\PluginManager{ return \pocketmine\plugin\PluginManager::getInstance(); }

		public function getCommandMap() : \pocketmine\command\CommandMap{ return self::$commandMap ??= new \pocketmine\command\CommandMap(); }
		public function getDefaultWorld() : \pocketmine\world\World{ return new \pocketmine\world\World(); }
		public function getMaxPlayers() : int{ return 20; }

		public function broadcastMessage(string $message) : void{
			$this->broadcasts[] = $message;
			echo "[bedrock] $message\n";
		}
	}
}

namespace {

	require __DIR__ . '/parity_stubs.php';
	require __DIR__ . '/../src/javabridge/Protocol.php';
	require __DIR__ . '/../src/javabridge/Nbt.php';
	require __DIR__ . '/../src/javabridge/ChunkBuilder.php';
	require __DIR__ . '/../src/javabridge/ModernProfile.php';
	require __DIR__ . '/../src/javabridge/ModernChunk.php';
	require __DIR__ . '/../src/javabridge/JavaClient.php';
	require __DIR__ . '/../src/javabridge/JavaServer.php';
	require __DIR__ . '/../src/javabridge/Main.php';

	use javabridge\JavaServer;
	use javabridge\Main;
	use javabridge\Protocol;

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

	/** @return list<array{0:int,1:string}> */
	function takeFrames(string &$buffer) : array{
		$frames = [];
		while(true){
			$offset = 0;
			$length = Protocol::readVarInt($buffer, $offset);
			if($length === null || strlen($buffer) - $offset < $length){
				break;
			}
			$body = substr($buffer, $offset, $length);
			$buffer = (string)substr($buffer, $offset + $length);
			$bodyOffset = 0;
			$id = Protocol::readVarInt($body, $bodyOffset);
			if($id === null){
				break;
			}
			$frames[] = [$id, substr($body, $bodyOffset)];
		}
		return $frames;
	}

	/** @param array<string, resource> $streams @param array<string, string> $buffers */
	function pump(JavaServer $server, array $streams, array &$buffers, float $seconds = 0.25) : void{
		$deadline = microtime(true) + $seconds;
		do{
			$server->tick();
			foreach($streams as $key => $stream){
				if(!is_resource($stream)){
					continue;
				}
				$chunk = @fread($stream, 65536);
				if($chunk !== false && $chunk !== ''){
					$buffers[$key] = ($buffers[$key] ?? '') . $chunk;
				}
			}
			usleep(3000);
		}while(microtime(true) < $deadline);
	}

	/** @return resource */
	function connect(int $port){
		$stream = @stream_socket_client("tcp://127.0.0.1:{$port}", $errno, $errstr, 2.0);
		if($stream === false){
			fwrite(STDERR, "No se pudo conectar: {$errstr} ({$errno})\n");
			exit(1);
		}
		stream_set_blocking($stream, false);
		return $stream;
	}

	function handshake(int $protocol, int $nextState, string $name = '') : string{
		$payload = Protocol::varInt($protocol) . Protocol::string('127.0.0.1') . pack('n', 25565) . Protocol::varInt($nextState);
		$data = Protocol::frame(0x00, $payload);
		return $nextState === 2 ? $data . Protocol::frame(0x00, Protocol::string($name)) : $data;
	}

	// Puerto libre efímero
	$probe = stream_socket_server('tcp://127.0.0.1:0', $probeErrno, $probeErrstr);
	if($probe === false){
		fwrite(STDERR, "Sin puerto libre: {$probeErrstr}\n");
		exit(1);
	}
	$probeName = (string)stream_socket_get_name($probe, false);
	$port = (int)substr($probeName, strrpos($probeName, ':') + 1);
	fclose($probe);

	$main = new Main();
	foreach(['viewDistance' => 1, 'javaGamemode' => 0, 'javaDifficulty' => 2, 'javaLevelType' => 'flat', 'javaMotd' => 'JavaBridge test'] as $property => $value){
		$ref = new ReflectionProperty(Main::class, $property);
		$ref->setAccessible(true);
		$ref->setValue($main, $value);
	}

	$server = new JavaServer($main, '127.0.0.1', $port, 1);
	$server->start();
	check('listener activo', $server->isRunning() && $server->getPort() === $port);
	check('spawn: mismo punto que el cliente Bedrock', $server->spawn() === [0.5, 5.62, 0.5], var_export($server->spawn(), true));

	// ---------------- status ----------------
	$statusClient = connect($port);
	fwrite($statusClient, handshake(340, 1) . Protocol::frame(0x00, ''));
	$buffers = ['status' => ''];
	pump($server, ['status' => $statusClient], $buffers, 0.2);
	$frames = takeFrames($buffers['status']);
	check('status: una respuesta', count($frames) === 1, 'frames=' . count($frames));
	$json = '{}';
	if($frames !== [] && $frames[0][0] === Protocol::S2C_STATUS_RESPONSE){
		$offset = 0;
		$json = (string)Protocol::readString($frames[0][1], $offset);
	}
	$decoded = json_decode($json, true);
	check('status: json válido', is_array($decoded));
	check('status: protocolo 340', ($decoded['version']['protocol'] ?? null) === 340);
	check('status: motd', ($decoded['description']['text'] ?? null) === 'JavaBridge test');
	check('status: max players', ($decoded['players']['max'] ?? null) === 20);

	fwrite($statusClient, Protocol::frame(Protocol::C2S_STATUS_PING, Protocol::long(123456789)));
	pump($server, ['status' => $statusClient], $buffers, 0.2);
	$frames = takeFrames($buffers['status']);
	$pong = null;
	foreach($frames as [$id, $payload]){
		if($id === Protocol::S2C_STATUS_PONG){
			$offset = 0;
			$pong = Protocol::readLong($payload, $offset);
		}
	}
	check('status: pong con el mismo id', $pong === 123456789, var_export($pong, true));
	fclose($statusClient);

	// ---------------- login ----------------
	$client = connect($port);
	fwrite($client, handshake(340, 2, 'Steve'));
	$buffers = ['player' => ''];
	pump($server, ['player' => $client], $buffers, 0.25);
	$frames = takeFrames($buffers['player']);
	$ids = array_map(static fn(array $frame) : int => $frame[0], $frames);
	$expected = [
		Protocol::S2C_LOGIN_SUCCESS,
		Protocol::S2C_JOIN_GAME,
		Protocol::S2C_DIFFICULTY,
		Protocol::S2C_SPAWN_POSITION,
		Protocol::S2C_ABILITIES,
		Protocol::S2C_UPDATE_HEALTH,
		Protocol::S2C_UPDATE_TIME,
		Protocol::S2C_POSITION,
		Protocol::S2C_PLAYER_INFO,
		Protocol::S2C_CHAT,
	];
	check('login: secuencia de paquetes', array_slice($ids, 0, count($expected)) === $expected, implode(',', $ids));

	$joinGame = $frames[1][1] ?? '';
	check('login: join game tamaño mínimo', strlen($joinGame) >= 12, 'bytes=' . strlen($joinGame));
	$entityId = strlen($joinGame) >= 4 ? unpack('N', substr($joinGame, 0, 4))[1] : 0;
	check('login: entity id > 0', $entityId > 0, (string)$entityId);
	check('login: gamemode survival', strlen($joinGame) > 4 && ord($joinGame[4]) === 0);
	check('login: dimensión overworld', strlen($joinGame) > 8 && unpack('N', substr($joinGame, 5, 4))[1] === 0);
	$difficulty = strlen($joinGame) > 9 ? ord($joinGame[9]) : -1;
	$maxPlayers = strlen($joinGame) > 10 ? ord($joinGame[10]) : 0;
	$offset = 11;
	$levelType = Protocol::readString($joinGame, $offset);
	check('login: dificultad', $difficulty === 2, (string)$difficulty);
	check('login: max players en join game', $maxPlayers === 20, (string)$maxPlayers);
	check('login: level type', $levelType === 'flat', var_export($levelType, true));

	// Los chunks llegan repartidos: view-distance 1 = 9 columnas, 2 por tick.
	$chunks = 0;
	foreach($frames as [$id, $payload]){
		if($id !== Protocol::S2C_MAP_CHUNK){
			continue;
		}
		$chunks++;
		$chunkX = unpack('N', substr($payload, 0, 4))[1];
		$chunkZ = unpack('N', substr($payload, 4, 4))[1];
		$cursor = 9; // x, z, groundUp
		$bitMap = Protocol::readVarInt($payload, $cursor);
		$size = Protocol::readVarInt($payload, $cursor);
		$data = substr($payload, $cursor, (int)$size);
		$cursor += (int)$size;
		$blockEntities = Protocol::readVarInt($payload, $cursor);
		check(
			"chunk {$chunkX},{$chunkZ}",
			// Dos secciones con bloque (Y=0 bedrock y Y=16 piedra/tierra/hierba).
			$bitMap === 3 && $size > 1000 && strlen($data) === $size && $blockEntities === 0,
			"bitmap={$bitMap} size={$size} data=" . strlen($data) . " be={$blockEntities}"
		);
	}
	check('chunks: 9 columnas enviadas', $chunks === 9, (string)$chunks);
	check('chunks: sin residuos', $buffers['player'] === '', strlen($buffers['player']) . ' bytes');

	// ---------------- chat y comandos ----------------
	fwrite($client, Protocol::frame(Protocol::C2S_CHAT, Protocol::string('/list')));
	pump($server, ['player' => $client], $buffers, 0.15);
	$frames = takeFrames($buffers['player']);
	$listLine = null;
	foreach($frames as [$id, $payload]){
		if($id === Protocol::S2C_CHAT){
			$offset = 0;
			$listLine = Protocol::readString($payload, $offset);
		}
	}
	check('/list responde', is_string($listLine) && str_contains($listLine, 'Online (1)'), (string)$listLine);

	// ---------------- paridad con la API de plugins ----------------
	$registrations = \pocketmine\runtime\Bridge::callsTo('player.register');
	check(
		'API: el jugador Java entra en el registro del servidor',
		$registrations !== [] && ($registrations[0]['name'] ?? null) === 'Steve' && ($registrations[0]['world'] ?? null) === 'world',
		var_export($registrations, true)
	);
	check(
		'API: getOnlinePlayers lo lista como uno más',
		array_keys(\pocketmine\Server::instance()->getOnlinePlayers()) === ['steve'],
		var_export(array_keys(\pocketmine\Server::instance()->getOnlinePlayers()), true)
	);

	// Lo que un plugin le manda con la API sale por su socket.
	\pocketmine\runtime\Bridge::$messages['steve'] = ['plugin line here'];
	pump($server, ['player' => $client], $buffers, 0.15);
	$pluginLine = false;
	foreach(takeFrames($buffers['player']) as [$id, $payload]){
		if($id === Protocol::S2C_CHAT && str_contains($payload, 'plugin line here')){
			$pluginLine = true;
		}
	}
	check('API: las líneas de los plugins llegan al cliente Java', $pluginLine);

	// El chat Java pasa por el mismo evento que el de Bedrock.
	\pocketmine\plugin\PluginManager::reset();
	\pocketmine\Server::instance()->broadcasts = [];
	fwrite($client, Protocol::frame(Protocol::C2S_CHAT, Protocol::string('hola mundo')));
	pump($server, ['player' => $client], $buffers, 0.15);
	$chatEvent = \pocketmine\plugin\PluginManager::last(\pocketmine\event\player\PlayerChatEvent::class);
	check(
		'API: el chat Java pasa por PlayerChatEvent',
		$chatEvent !== null && $chatEvent->getMessage() === 'hola mundo' && $chatEvent->getPlayer()->getName() === 'Steve',
		$chatEvent === null ? 'sin evento' : $chatEvent->getMessage()
	);
	check(
		'API: el chat se difunde con broadcastMessage',
		in_array('<Steve> hola mundo', \pocketmine\Server::instance()->broadcasts, true),
		implode(' | ', \pocketmine\Server::instance()->broadcasts)
	);

	fwrite($client, Protocol::frame(Protocol::C2S_CHAT, Protocol::string('/spawn')));
	pump($server, ['player' => $client], $buffers, 0.15);
	$frames = takeFrames($buffers['player']);
	$positionPackets = 0;
	foreach($frames as [$id, $payload]){
		if($id === Protocol::S2C_POSITION){
			$positionPackets++;
		}
	}
	check('/spawn envía posición', $positionPackets === 1, (string)$positionPackets);

	// El bloque se pica junto al jugador (a 1,12 bloques) porque el puente
	// ignora los picados que caen fuera de su alcance de 6 bloques.
	fwrite($client, Protocol::frame(Protocol::C2S_BLOCK_DIG, Protocol::varInt(2) . Protocol::position(0, 68, 0)));
	pump($server, ['player' => $client], $buffers, 0.15);
	$frames = takeFrames($buffers['player']);
	$blockChanges = 0;
	foreach($frames as [$id, $payload]){
		if($id === Protocol::S2C_BLOCK_CHANGE){
			$offset = 0;
			$position = Protocol::readPosition($payload, $offset);
			$state = Protocol::readVarInt($payload, $offset);
			if($position === [0, 68, 0] && $state === \javabridge\ChunkBuilder::JAVA_AIR){
				$blockChanges++;
			}
		}
	}
	check('block dig -> block change aire', $blockChanges === 1, (string)$blockChanges);

	// Colocar bloque legacy (Fase 2, 0x1F): contra (0,68,0) cara 1
	// (arriba) => celda legacy (0,69,0) = canónica (0,5,0), con piedra
	// (slot id 1) en la mano. Solo creativo.
	\pocketmine\world\Block::$state['0,5,0'] = 0;
	$legacyPlace = Protocol::position(0, 68, 0) . chr(1) . pack('n', 1) . chr(1) . pack('n', 0) . chr(8) . chr(8) . chr(8);
	fwrite($client, Protocol::frame(Protocol::C2S_BLOCK_PLACE, $legacyPlace));
	pump($server, ['player' => $client], $buffers, 0.15);
	$placeChat = '';
	foreach(takeFrames($buffers['player']) as [$id, $payload]){
		if($id === Protocol::S2C_CHAT){
			$placeChat .= $payload;
		}
	}
	check('block place survival: chat creativo sin colocar', str_contains($placeChat, 'modo creativo') && (\pocketmine\world\Block::$state['0,5,0'] ?? 0) === 0, $placeChat);
	fwrite($client, Protocol::frame(Protocol::C2S_CHAT, Protocol::string('/gamemode creative')));
	pump($server, ['player' => $client], $buffers, 0.15);
	takeFrames($buffers['player']);
	fwrite($client, Protocol::frame(Protocol::C2S_BLOCK_PLACE, $legacyPlace));
	pump($server, ['player' => $client], $buffers, 0.15);
	$placed = 0;
	foreach(takeFrames($buffers['player']) as [$id, $payload]){
		if($id === Protocol::S2C_BLOCK_CHANGE){
			$offset = 0;
			$position = Protocol::readPosition($payload, $offset);
			$state = Protocol::readVarInt($payload, $offset);
			if($position === [0, 69, 0] && $state === \javabridge\ChunkBuilder::JAVA_STONE){
				$placed++;
			}
		}
	}
	check('block place creative -> piedra legacy en (0,69,0)', $placed === 1 && (\pocketmine\world\Block::$state['0,5,0'] ?? 0) === 1, "placed={$placed} estado=" . var_export(\pocketmine\world\Block::$state['0,5,0'] ?? null, true));

	// `/tp` usa coordenadas del mundo Bedrock; el cliente 1.12.2 las recibe con
	// el desplazamiento de +64 que tiene su copia del mundo.
	fwrite($client, Protocol::frame(Protocol::C2S_CHAT, Protocol::string('/tp 10 66 -30')));
	pump($server, ['player' => $client], $buffers, 0.15);
	$frames = takeFrames($buffers['player']);
	$teleport = null;
	foreach($frames as [$id, $payload]){
		if($id === Protocol::S2C_POSITION){
			$offset = 0;
			$teleport = [
				Protocol::readDouble($payload, $offset),
				Protocol::readDouble($payload, $offset),
				Protocol::readDouble($payload, $offset),
			];
		}
	}
	check(
		'/tp recoleta con el desplazamiento de 1.12.2',
		$teleport !== null && $teleport[0] === 10.0 && $teleport[1] === 130.0 && $teleport[2] === -30.0,
		var_export($teleport, true)
	);

	$statusJson = json_decode($server->statusJson(), true);
	check('status refleja 1 java online', ($statusJson['players']['online'] ?? null) === 1);
	check('lista de nombres java', $server->javaPlayerNames() === ['Steve']);

	// ---------------- nombre duplicado ----------------
	$dup = connect($port);
	fwrite($dup, handshake(340, 2, 'Steve'));
	$buffers['dup'] = '';
	pump($server, ['dup' => $dup], $buffers, 0.2);
	$frames = takeFrames($buffers['dup']);
	check('nombre duplicado rechazado', count($frames) === 1 && $frames[0][0] === Protocol::S2C_LOGIN_DISCONNECT, (string)count($frames));
	fclose($dup);

	// ---------------- protocolo incorrecto ----------------
	$old = connect($port);
	fwrite($old, handshake(47, 2, 'Alex'));
	$buffers['old'] = '';
	pump($server, ['old' => $old], $buffers, 0.2);
	$frames = takeFrames($buffers['old']);
	check('versión incorrecta rechazada', count($frames) === 1 && $frames[0][0] === Protocol::S2C_LOGIN_DISCONNECT, (string)count($frames));
	fclose($old);

	// ---------------- desconexión ----------------
	fclose($client);
	$server->tick();
	$server->tick();
	check(
		'API: la salida pasa por PlayerQuitEvent',
		\pocketmine\plugin\PluginManager::last(\pocketmine\event\player\PlayerQuitEvent::class) !== null
	);
	check(
		'desconexión anuncia salida',
		in_array('Steve left the game', \pocketmine\Server::instance()->broadcasts, true),
		implode(' | ', \pocketmine\Server::instance()->broadcasts)
	);
	check('API: el jugador sale del registro', \pocketmine\Server::instance()->getOnlinePlayers() === []);
	check('sin clientes tras cerrar', $server->getClients() === []);

	$server->stop();
	check('listener detenido', !$server->isRunning());

	fwrite(STDOUT, $failures === 0 ? "ALL OK\n" : "{$failures} FAILURE(S)\n");
	exit($failures === 0 ? 0 : 1);
}
