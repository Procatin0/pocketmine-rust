<?php

declare(strict_types=1);

/*
 * Prueba de red del flujo moderno (1.20.2+): login -> configuration -> play.
 *
 * Habla con el listener real por sockets y emula los paquetes que manda un
 * cliente 1.21.x, comprobando que las respuestas son consumibles campo a campo.
 */

namespace pocketmine\world {

	final class Location{
		public function __construct(private float $x, private float $y, private float $z){}
		public function getX() : float{ return $this->x; }
		public function getY() : float{ return $this->y; }
		public function getZ() : float{ return $this->z; }
	}

	/**
	 * Bloque con estado compartido (Fase 1/2): `x,y,z` => id legacy.
	 *
	 * Sin estado, `placeBlock()` no podría validar "celda vacía" ni el
	 * fallback por polling detectar cambios de Bedrock. El defecto es
	 * piedra en (0,3,0) para que el `block_dig` clásico siga rompiendo
	 * algo, y aire en el resto (el cielo sobre el terreno).
	 */
	final class Block{
		/** @var array<string, int> */
		public static array $state = [];

		public function __construct(private int $bx = 0, private int $by = 0, private int $bz = 0){}

		private function key() : string{ return $this->bx . ',' . $this->by . ',' . $this->bz; }

		public function getId() : int{
			return self::$state[$this->key()] ?? (($this->bx === 0 && $this->by === 3 && $this->bz === 0) ? 1 : 0);
		}

		public function getNetworkId() : int{
			return match($this->getId()){
				0 => 3690217760,   // air
				2 => 3727763636,   // grass_block
				3 => 2186211206,   // dirt
				7 => 4121722107,   // bedrock
				default => 2150698529, // stone (y fallback de los demás)
			};
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
						// Bloque suelto: si el puente confunde los ejes, se mueve de sitio.
						if($x === 1 && $z === 5 && $y === 15){
							$slot = 0;
						}
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
		public static int $time = 6000;
		public static bool $timeRunning = true;
		public function getName() : string{ return 'world'; }
		public function getSpawnLocation() : Location{ return new Location(0.0, 4.0, 0.0); }
		public function getPlayerSpawnLocation() : ?array{ return [0.5, 5.62, 0.5]; }
		public function getHighestBlockAt(int $x, int $z) : int{ return 3; }
		public function getBlockAt(int $x, int $y, int $z) : Block{ return new Block($x, $y, $z); }
		public function getTime() : int{ return self::$time; }
		public function isTimeRunning() : bool{ return self::$timeRunning; }
		public function setTime(int $time) : void{ self::$time = (($time % 24000) + 24000) % 24000; }
		public function startTime() : void{ self::$timeRunning = true; }
		public function stopTime() : void{ self::$timeRunning = false; }
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
	use javabridge\ModernProfile;
	use javabridge\Protocol;

	const NbtCompoundTag = 10;
	const NbtStringTag = 8;

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
	function pump(JavaServer $server, array $streams, array &$buffers, float $seconds = 0.3) : void{
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
			fwrite(STDERR, "No se pudo conectar: {$errstr}\n");
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

	/** Salta un NBT completo (tipo + nombre + payload). */
	function skipNbt(string $buffer, int &$offset) : void{
		$type = ord($buffer[$offset]);
		$offset++;
		if($type === 0){
			return;
		}
		// NBT de red: la raíz no lleva nombre.
		skipNbtPayload($buffer, $offset, $type);
	}

	function skipNbtPayload(string $buffer, int &$offset, int $type) : void{
		switch($type){
			case 1: $offset += 1; return;
			case 2: $offset += 2; return;
			case 3: case 5: $offset += 4; return;
			case 4: case 6: $offset += 8; return;
			case 7:
				$length = unpack('N', substr($buffer, $offset, 4))[1];
				$offset += 4 + $length;
				return;
			case 8:
				$length = unpack('n', substr($buffer, $offset, 2))[1];
				$offset += 2 + $length;
				return;
			case 9:
				$elementType = ord($buffer[$offset]);
				$offset++;
				$count = unpack('N', substr($buffer, $offset, 4))[1];
				$offset += 4;
				for($i = 0; $i < $count; $i++){
					skipNbtPayload($buffer, $offset, $elementType);
				}
				return;
			case 10:
				while(true){
					$childType = ord($buffer[$offset]);
					$offset++;
					if($childType === 0){
						return;
					}
					$nameLength = unpack('n', substr($buffer, $offset, 2))[1];
					$offset += 2 + $nameLength;
					skipNbtPayload($buffer, $offset, $childType);
				}
			case 11:
				$length = unpack('N', substr($buffer, $offset, 4))[1];
				$offset += 4 + 4 * $length;
				return;
			case 12:
				$length = unpack('N', substr($buffer, $offset, 4))[1];
				$offset += 4 + 8 * $length;
				return;
			default:
				throw new RuntimeException("Tipo NBT desconocido: {$type}");
		}
	}

	/** Extrae el texto de un componente NBT anónimo {"text": "..."}. */
	function parseTextComponent(string $payload) : string{
		if($payload === '' || ord($payload[0]) !== NbtCompoundTag){
			return '';
		}
		$offset = 1; // raíz sin nombre
		if(ord($payload[$offset]) !== NbtStringTag){
			return '';
		}
		$offset++;
		$nameLength = unpack('n', substr($payload, $offset, 2))[1];
		$offset += 2 + $nameLength;
		$length = unpack('n', substr($payload, $offset, 2))[1];
		$offset += 2;
		return substr($payload, $offset, $length);
	}

	/** @return array{bits:int, palette:list<int>} */
	function parseContainer(string $buffer, int &$offset, int $entries, bool $dataLength) : array{
		$bits = ord($buffer[$offset]);
		$offset++;
		if($bits === 0){
			$value = Protocol::readVarInt($buffer, $offset);
			if($dataLength){
				$length = Protocol::readVarInt($buffer, $offset);
				if($length !== 0){
					throw new RuntimeException("Contenedor de valor único con datos ({$length})");
				}
			}
			return ['bits' => 0, 'palette' => [(int)$value]];
		}

		$count = Protocol::readVarInt($buffer, $offset);
		$palette = [];
		for($i = 0; $i < $count; $i++){
			$palette[] = (int)Protocol::readVarInt($buffer, $offset);
		}
		// Desde 1.21.5 la longitud no viaja: se deduce de los bits y las entradas.
		$perLong = intdiv(64, $bits);
		$total = $dataLength
			? (int)Protocol::readVarInt($buffer, $offset)
			: intdiv($entries + $perLong - 1, $perLong);
		if(strlen($buffer) - $offset < $total * 8){
			throw new RuntimeException("Datos de paleta truncados");
		}
		$offset += $total * 8;
		return ['bits' => $bits, 'palette' => $palette, 'raw' => substr($buffer, $offset - $total * 8, $total * 8)];
	}

	/** Estado de una entrada concreta de un contenedor ya parseado. */
	function containerState(array $container, int $entry) : int{
		$bits = $container['bits'];
		if($bits === 0){
			return $container['palette'][0] ?? 0;
		}
		$perLong = intdiv(64, $bits);
		// Los longs de la paleta viajan en big-endian (Protocol::long).
		$long = unpack('J', substr($container['raw'], intdiv($entry, $perLong) * 8, 8))[1];
		$slot = ($long >> (($entry % $perLong) * $bits)) & ((1 << $bits) - 1);
		return $container['palette'][$slot] ?? 0;
	}

	/** @return list<array{bits:int, palette:list<int>, blockCount:int, fluidCount:int}> */
	function parseChunkData(string $data, ModernProfile $profile) : array{
		$offset = 0;
		$sections = [];
		$dataLength = $profile->paletteDataHasLength();
		while($offset < strlen($data)){
			$blockCount = unpack('n', substr($data, $offset, 2))[1];
			$offset += 2; // contador de bloques (i16)
			$fluidCount = 0;
			if($profile->sectionsHaveFluidCount()){
				$fluidCount = unpack('n', substr($data, $offset, 2))[1];
				$offset += 2; // contador de fluidos (26.1): sin esto hasFluid()=false
			}
			$blocks = parseContainer($data, $offset, 4096, $dataLength);
			$biomes = parseContainer($data, $offset, 64, $dataLength);
			$blocks['blockCount'] = (int)$blockCount;
			$blocks['fluidCount'] = (int)$fluidCount;
			$sections[] = $blocks;
			// Biomas: paleta de valor único (plains, no badlands=0) y sin datos.
			if($biomes['bits'] !== 0 || $biomes['palette'] !== [$profile->plainsBiome()]){
				throw new RuntimeException('Contenedor de biomas inesperado: ' . json_encode($biomes['palette']));
			}
		}
		return $sections;
	}

	/**
	 * Parsea un `map_chunk` completo y comprueba que no sobran bytes.
	 *
	 * @return array{x:int, z:int, sections:list<array{bits:int, palette:list<int>}>}
	 */
	function parseChunk(string $payload, ModernProfile $profile) : array{
		$offset = 0;
		$x = unpack('N', substr($payload, $offset, 4))[1];
		$offset += 4;
		$z = unpack('N', substr($payload, $offset, 4))[1];
		$offset += 4;

		if($profile->heightmapsAsNbt()){
			skipNbt($payload, $offset);
		}else{
			$count = Protocol::readVarInt($payload, $offset);
			for($i = 0; $i < $count; $i++){
				Protocol::readVarInt($payload, $offset); // tipo
				$longs = Protocol::readVarInt($payload, $offset);
				$offset += $longs * 8;
			}
		}

		$size = Protocol::readVarInt($payload, $offset);
		$data = substr($payload, $offset, (int)$size);
		$offset += (int)$size;
		$entities = Protocol::readVarInt($payload, $offset);
		if($entities !== 0){
			throw new RuntimeException('Entidades de bloque inesperadas');
		}

		$skyMaskCount = Protocol::readVarInt($payload, $offset);
		$offset += $skyMaskCount * 8;
		Protocol::readVarInt($payload, $offset); // block light mask
		Protocol::readVarInt($payload, $offset); // empty sky light mask
		$emptyBlockCount = Protocol::readVarInt($payload, $offset);
		$offset += $emptyBlockCount * 8;

		$skyLightCount = Protocol::readVarInt($payload, $offset);
		for($i = 0; $i < $skyLightCount; $i++){
			$length = Protocol::readVarInt($payload, $offset);
			$offset += $length;
		}
		$blockLightCount = Protocol::readVarInt($payload, $offset);
		for($i = 0; $i < $blockLightCount; $i++){
			$length = Protocol::readVarInt($payload, $offset);
			$offset += $length;
		}

		if($offset !== strlen($payload)){
			throw new RuntimeException('Sobran ' . (strlen($payload) - $offset) . ' bytes en map_chunk');
		}

		return ['x' => $x, 'z' => $z, 'sections' => parseChunkData($data, $profile)];
	}

	// ---------------- servidor ----------------
	$probe = stream_socket_server('tcp://127.0.0.1:0', $probeErrno, $probeErrstr);
	if($probe === false){
		fwrite(STDERR, "Sin puerto libre: {$probeErrstr}\n");
		exit(1);
	}
	$probeName = (string)stream_socket_get_name($probe, false);
	$port = (int)substr($probeName, strrpos($probeName, ':') + 1);
	fclose($probe);

	$main = new Main();
	foreach(['viewDistance' => 1, 'javaGamemode' => 0, 'javaDifficulty' => 2, 'javaLevelType' => 'flat', 'javaMotd' => 'JavaBridge modern'] as $property => $value){
		$ref = new ReflectionProperty(Main::class, $property);
		$ref->setAccessible(true);
		$ref->setValue($main, $value);
	}

	$server = new JavaServer($main, '127.0.0.1', $port, 1);
	$server->start();

	// Punto de aparición: el mismo que manda el servidor Bedrock en `start_game`
	// (centro del bloque y altura de los ojos del generador), no el bloque.
	check('spawn: mismo punto que el cliente Bedrock', $server->spawn() === [0.5, 5.62, 0.5], var_export($server->spawn(), true));

	$names = [769 => 'Steve', 770 => 'Bob', 773 => 'Carol', 774 => 'Alex', 775 => 'Diana'];

	foreach($names as $protocol => $playerName){
		$profile = ModernProfile::forProtocol($protocol);
		if($profile === null){
			check("perfil {$protocol}", false);
			continue;
		}
		$label = "{$profile->name} ({$protocol})";

		// ---- status ping ----
		$statusClient = connect($port);
		fwrite($statusClient, handshake($protocol, 1) . Protocol::frame(0x00, ''));
		$buffers = ['status' => ''];
		pump($server, ['status' => $statusClient], $buffers, 0.15);
		$frames = takeFrames($buffers['status']);
		$offset = 0;
		$json = count($frames) === 1 && $frames[0][0] === Protocol::S2C_STATUS_RESPONSE
			? (string)Protocol::readString($frames[0][1], $offset)
			: '{}';
		$decoded = json_decode($json, true);
		check("{$label} status: protocolo y nombre", ($decoded['version']['protocol'] ?? null) === $protocol && ($decoded['version']['name'] ?? null) === $profile->name, $json);
		fclose($statusClient);

		// ---- login ----
		$client = connect($port);
		fwrite($client, handshake($protocol, 2, $playerName));
		$buffers = ['player' => ''];
		pump($server, ['player' => $client], $buffers, 0.15);
		$frames = takeFrames($buffers['player']);
		check("{$label} login: login_success", count($frames) === 1 && $frames[0][0] === $profile->id('login_success'), (string)count($frames));
		$offset = 0;
		$uuid = substr($frames[0][1] ?? '', 0, 16);
		$offset = 16;
		$name = Protocol::readString($frames[0][1] ?? '', $offset);
		$properties = Protocol::readVarInt($frames[0][1] ?? '', $offset);
		check("{$label} login: uuid y nombre", strlen($uuid) === 16 && $name === $playerName && $properties === 0);
		check("{$label} login: sin tokens sobrantes", $offset === strlen($frames[0][1] ?? ''));

		// ---- configuration ----
		fwrite($client, Protocol::frame($profile->id('login_acknowledged'), ''));
		pump($server, ['player' => $client], $buffers, 1.0);
		$frames = takeFrames($buffers['player']);
		$registryIds = [];
		$finishSeen = false;
		$tagsPayload = null;
		$brandPayload = null;
		$brandBeforeFinish = false;
		foreach($frames as [$id, $payload]){
			if($id === $profile->id('registry_data')){
				$offset = 0;
				$registryIds[] = (string)Protocol::readString($payload, $offset);
			}
			if($id === $profile->id('update_tags')){
				$tagsPayload = $payload;
			}
			if($id === $profile->id('config_brand')){
				$brandPayload = $payload;
				$brandBeforeFinish = !$finishSeen;
			}
			if($id === $profile->id('finish_configuration')){
				$finishSeen = true;
			}
		}
		$registryFile = json_decode((string)file_get_contents(__DIR__ . '/../resources/registry/' . $protocol . '.json'), true);
		$expectedRegistries = array_column($registryFile['registries'] ?? [], 'id');
		check(
			"{$label} config: todos los registros",
			$registryIds === $expectedRegistries
				&& in_array('minecraft:dimension_type', $registryIds, true)
				&& in_array('minecraft:worldgen/biome', $registryIds, true)
				&& in_array('minecraft:chat_type', $registryIds, true),
			count($registryIds) . ' registros'
		);
		$tagsFile = json_decode((string)file_get_contents(__DIR__ . '/../resources/tags/' . $protocol . '.json'), true);
		$offset = 0;
		$tagsOk = is_string($tagsPayload);
		$tagRegistries = [];
		$tagTotal = 0;
		$memberTotal = 0;
		/** @var array<string, list<int>> "registro/tag" => IDs (p. ej. "minecraft:fluid/minecraft:water") */
		$parsedMembers = [];
		if(is_string($tagsPayload)){
			$registryCount = Protocol::readVarInt($tagsPayload, $offset);
			for($i = 0; $i < (int)$registryCount; $i++){
				$tagRegistry = (string)Protocol::readString($tagsPayload, $offset);
				$tagRegistries[] = $tagRegistry;
				$tagCount = (int)Protocol::readVarInt($tagsPayload, $offset);
				$tagTotal += $tagCount;
				for($j = 0; $j < $tagCount; $j++){
					$tagName = (string)Protocol::readString($tagsPayload, $offset);
					$members = Protocol::readVarIntArray($tagsPayload, $offset) ?? [];
					$parsedMembers[$tagRegistry . '/' . $tagName] = $members;
					$memberTotal += count($members);
				}
			}
			$tagsOk = $offset === strlen($tagsPayload);
		}
		$presentRegistries = array_flip($expectedRegistries);
		$staticTagRegistries = ['minecraft:block', 'minecraft:item', 'minecraft:entity_type', 'minecraft:fluid'];
		$expectedTagRegistries = [];
		foreach(array_keys($tagsFile['registries'] ?? []) as $tagRegistry){
			$tagRegistryId = 'minecraft:' . $tagRegistry;
			if(isset($presentRegistries[$tagRegistryId]) || in_array($tagRegistryId, $staticTagRegistries, true)){
				$expectedTagRegistries[] = $tagRegistryId;
			}
		}
		check(
			"{$label} config: update_tags completo",
			$tagsOk
				&& $tagRegistries === $expectedTagRegistries
				&& in_array('minecraft:damage_type', $tagRegistries, true)
				&& $tagTotal > 0,
			count($tagRegistries) . ' registros, ' . $tagTotal . ' tags'
		);
		check("{$label} config: finish_configuration", $finishSeen);

		// Miembros: sin ellos F3 no muestra ningún tag aunque el tag exista
		// vacío. Fluido (registro estático minecraft:fluid, orden de
		// `Fluids.<clinit>` verificado con javap sobre el jar vanilla
		// 26.1): empty=0, flowing_water=1, water=2, flowing_lava=3,
		// lava=4. Por eso water=[1,2] y lava=[3,4]: antes se mandaba
		// water=[1], lava=[2] y F3 mostraba el agua (2) con #minecraft:lava.
		// Bloque: stone=1 (mineable/pickaxe) y gravel=40 (mineable/shovel)
		// son los IDs de blocks.json en las 5 versiones.
		check(
			"{$label} config: update_tags con miembros (fluido agua/lava)",
			($parsedMembers['minecraft:fluid/minecraft:water'] ?? null) === [1, 2]
				&& ($parsedMembers['minecraft:fluid/minecraft:lava'] ?? null) === [3, 4]
				&& $memberTotal > 100,
			"agua=" . json_encode($parsedMembers['minecraft:fluid/minecraft:water'] ?? null)
				. " lava=" . json_encode($parsedMembers['minecraft:fluid/minecraft:lava'] ?? null)
				. " miembros={$memberTotal}"
		);
		// En 26.1 hay más tags de fluido: solo la fuente `water` (2) en
		// bubble/frogspawn/lily_pad, y `#minecraft:water` (1,2) en
		// supports_sugar_cane_adjacently (FluidTagsProvider vanilla).
		if($protocol === 775){
			check(
				"{$label} config: update_tags fluidos secundarios",
				($parsedMembers['minecraft:fluid/minecraft:bubble_column_can_occupy'] ?? null) === [2]
					&& ($parsedMembers['minecraft:fluid/minecraft:supports_frogspawn'] ?? null) === [2]
					&& ($parsedMembers['minecraft:fluid/minecraft:supports_lily_pad'] ?? null) === [2]
					&& ($parsedMembers['minecraft:fluid/minecraft:supports_sugar_cane_adjacently'] ?? null) === [1, 2],
				"burbuja=" . json_encode($parsedMembers['minecraft:fluid/minecraft:bubble_column_can_occupy'] ?? null)
					. " caña=" . json_encode($parsedMembers['minecraft:fluid/minecraft:supports_sugar_cane_adjacently'] ?? null)
			);
		}
		check(
			"{$label} config: update_tags mineable (piedra y grava)",
			in_array(1, $parsedMembers['minecraft:block/minecraft:mineable/pickaxe'] ?? [], true)
				&& in_array(40, $parsedMembers['minecraft:block/minecraft:mineable/shovel'] ?? [], true)
				&& ($parsedMembers['minecraft:block/minecraft:climbable'] ?? []) !== [],
			"pickaxe=" . count($parsedMembers['minecraft:block/minecraft:mineable/pickaxe'] ?? [])
				. " shovel=" . count($parsedMembers['minecraft:block/minecraft:mineable/shovel'] ?? [])
		);

		// Brand: custom_payload clientbound (id 1) en configuration, canal
		// minecraft:brand + String. Sin esto F3 pone `null` en `Server:`.
		$brandOffset = 0;
		$brandChannel = is_string($brandPayload) ? Protocol::readString($brandPayload, $brandOffset) : null;
		$brandText = is_string($brandPayload) ? Protocol::readString($brandPayload, $brandOffset) : null;
		check(
			"{$label} config: ids brand=1 y update_tags=13",
			$profile->id('config_brand') === 1 && $profile->id('update_tags') === 13 && $profile->id('brand') === 1
		);
		check(
			"{$label} config: brand PocketMine",
			is_string($brandPayload) && $brandChannel === 'minecraft:brand' && $brandText === 'PocketMine'
				&& $brandOffset === strlen($brandPayload) && $brandBeforeFinish,
			"canal=" . var_export($brandChannel, true) . " texto=" . var_export($brandText, true)
		);

		fwrite($client, Protocol::frame($profile->id('config_settings'),
			Protocol::string('en_US') . chr(10) . Protocol::varInt(0) . Protocol::boolean(true) . chr(0x7F) . Protocol::varInt(1)
		));
		fwrite($client, Protocol::frame($profile->id('config_finish'), ''));
		// 25 columnas de ~58 KB: hace falta margen para que el socket drene.
		pump($server, ['player' => $client], $buffers, 6.0);

		$frames = takeFrames($buffers['player']);
		$joinGame = null;
		$chunks = [];
		foreach($frames as [$id, $payload]){
			if($id === $profile->id('join_game')){
				$joinGame = $payload;
			}
			if($id === $profile->id('map_chunk')){
				$chunks[] = $payload;
			}
		}
		check("{$label} play: join game recibido", is_string($joinGame));

		if(is_string($joinGame)){
			$offset = 0;
			$entityId = unpack('N', substr($joinGame, $offset, 4))[1];
			$offset += 4;
			$hardcore = ord($joinGame[$offset]);
			$offset++;
			$worldCount = Protocol::readVarInt($joinGame, $offset);
			$worlds = [];
			for($i = 0; $i < $worldCount; $i++){
				$worlds[] = Protocol::readString($joinGame, $offset);
			}
			$maxPlayers = Protocol::readVarInt($joinGame, $offset);
			$viewDistanceJoin = Protocol::readVarInt($joinGame, $offset);
			$simulationDistance = Protocol::readVarInt($joinGame, $offset);
			$reduced = ord($joinGame[$offset]);
			$offset++;
			$respawnScreen = ord($joinGame[$offset]);
			$offset++;
			$limitedCrafting = ord($joinGame[$offset]);
			$offset++;
			$dimension = Protocol::readVarInt($joinGame, $offset);
			$dimensionName = Protocol::readString($joinGame, $offset);
			$seed = Protocol::readLong($joinGame, $offset);
			$gamemode = ord($joinGame[$offset]);
			$offset++;
			$previous = ord($joinGame[$offset]);
			$offset++;
			$debug = ord($joinGame[$offset]);
			$offset++;
			$flat = ord($joinGame[$offset]);
			$offset++;
			$death = ord($joinGame[$offset]);
			$offset++;
			$portalCooldown = Protocol::readVarInt($joinGame, $offset);
			$seaLevel = Protocol::readVarInt($joinGame, $offset);
			$secureChat = ord($joinGame[$offset]);
			$offset++;

			check("{$label} join: campos", $entityId > 0 && $hardcore === 0 && $worlds === ['minecraft:overworld'] && $maxPlayers === 20 && $viewDistanceJoin === 2 && $simulationDistance === 2 && $reduced === 0 && $respawnScreen === 1 && $limitedCrafting === 0, "worlds=" . implode(',', $worlds) . " max={$maxPlayers} vd={$viewDistanceJoin}");
			check("{$label} join: spawn info", $dimension === 0 && $dimensionName === 'minecraft:overworld' && $seed === 0 && $gamemode === 0 && $previous === 255 && $debug === 0 && $flat === 1 && $death === 0 && $portalCooldown === 0 && $seaLevel === 63 && $secureChat === 0, "dim={$dimension} gm={$gamemode} prev={$previous} flat={$flat} sea={$seaLevel}");
			check("{$label} join: sin bytes sobrantes", $offset === strlen($joinGame), (string)($offset - strlen($joinGame)));
		}

		// view_distance 2 => 5x5 columnas, y el jugador necesita la suya primero.
		$chunkCoords = [];
		foreach($chunks as $payload){
			$chunkCoords[] = unpack('l', substr($payload, 0, 4))[1] . ',' . unpack('l', substr($payload, 4, 4))[1];
		}
		check("{$label} chunks: 25 columnas", count($chunks) === 25, count($chunks) . ': ' . implode(' ', $chunkCoords));

		$bedrockSeen = false;
		$grassSeen = false;
		$dirtSeen = false;
		$stoneSeen = false;
		$snowyGrass = false;
		$fluidCountsOk = true;
		$sectionsOk = true;
		$markerOk = false;
		foreach($chunks as $payload){
			try{
				$parsed = parseChunk($payload, $profile);
			}catch(Throwable $e){
				$sectionsOk = false;
				fwrite(STDERR, "  chunk inválido: " . $e->getMessage() . "\n");
				continue;
			}
			if(count($parsed['sections']) !== \javabridge\ModernChunk::SECTIONS){
				$sectionsOk = false;
			}
			foreach($parsed['sections'] as $section){
				$bedrockSeen = $bedrockSeen || in_array(85, $section['palette'], true);
				$grassSeen = $grassSeen || in_array(9, $section['palette'], true);
				// 8 es `grass_block[snowy=true]`: el mundo saldría nevado.
				$snowyGrass = $snowyGrass || in_array(8, $section['palette'], true);
				$dirtSeen = $dirtSeen || in_array(10, $section['palette'], true);
				$stoneSeen = $stoneSeen || in_array(1, $section['palette'], true);
				// El chunk de prueba no tiene agua: fluido debe ser 0, no basura.
				$fluidCountsOk = $fluidCountsOk && ($section['fluidCount'] ?? -1) === 0;
			}
			// Bloque suelto en (x=1, y=15, z=5) de la subsección Y=-48: se mueve
			// de sitio si el puente confunde los ejes al reordenar los índices.
			if($parsed['x'] === 0 && $parsed['z'] === 0){
				$markerOk = containerState($parsed['sections'][1], (15 << 8) | (5 << 4) | 1) === 1
					&& containerState($parsed['sections'][1], (15 << 8) | (5 << 4) | 15) === 0;
			}
		}
		check("{$label} chunks: 24 secciones", $sectionsOk);
		check("{$label} chunks: bedrock, piedra, tierra y hierba", $bedrockSeen && $stoneSeen && $dirtSeen && $grassSeen, "bedrock=" . (int)$bedrockSeen . " stone=" . (int)$stoneSeen . " dirt=" . (int)$dirtSeen . " grass=" . (int)$grassSeen);
		check("{$label} chunks: bloque suelto en su coordenada", $markerOk);
		check("{$label} chunks: hierba sin nieve", !$snowyGrass);
		check("{$label} chunks: contador de fluidos coherente (0 sin agua)", $fluidCountsOk);

		// ---- chat y comandos ----
		fwrite($client, Protocol::frame($profile->id('chat_command'), Protocol::string('list')));
		pump($server, ['player' => $client], $buffers, 0.5);
		$frames = takeFrames($buffers['player']);
		$listLine = null;
		foreach($frames as [$id, $payload]){
			if($id === $profile->id('system_chat')){
				$listLine = parseTextComponent($payload);
			}
		}
		check("{$label} chat_command /list", is_string($listLine) && str_contains($listLine, 'Online (1)'), (string)$listLine);

		// ---- romper bloque ----
		fwrite($client, Protocol::frame($profile->id('block_dig'),
			Protocol::varInt(2) . Protocol::position(0, 3, 0) . chr(0) . Protocol::varInt(0)
		));
		pump($server, ['player' => $client], $buffers, 0.5);
		$frames = takeFrames($buffers['player']);
		$blockChanges = 0;
		foreach($frames as [$id, $payload]){
			if($id === $profile->id('block_change')){
				$offset = 0;
				$position = Protocol::readPosition($payload, $offset);
				$state = Protocol::readVarInt($payload, $offset);
				if($position === [0, 3, 0] && $state === 0){
					$blockChanges++;
				}
			}
		}
		check("{$label} block_dig -> aire", $blockChanges === 1, (string)$blockChanges);

		// ---- colocar bloque (Fase 2): solo creativo, sin inventario ----
		// Contra (0,3,0) cara 1 (arriba) => celda (0,4,0), en aire.
		\pocketmine\world\Block::$state['0,4,0'] = 0;
		$placePacket = Protocol::varInt(0) . Protocol::position(0, 3, 0) . Protocol::varInt(1)
			. Protocol::float(0.5) . Protocol::float(1.0) . Protocol::float(0.5)
			. Protocol::boolean(false) . Protocol::varInt(0);
		// En survival: chat explicando y sin colocar.
		fwrite($client, Protocol::frame($profile->id('block_place'), $placePacket));
		pump($server, ['player' => $client], $buffers, 0.5);
		$placeChat = null;
		$placedEarly = false;
		foreach(takeFrames($buffers['player']) as [$id, $payload]){
			if($id === $profile->id('system_chat')){
				$placeChat = parseTextComponent($payload);
			}
			if($id === $profile->id('block_change')){
				$offset = 0;
				$position = Protocol::readPosition($payload, $offset);
				$state = Protocol::readVarInt($payload, $offset);
				if($position === [0, 4, 0] && $state === 1){
					$placedEarly = true;
				}
			}
		}
		check("{$label} block_place survival: chat creativo sin colocar", is_string($placeChat) && str_contains($placeChat, 'modo creativo') && !$placedEarly && (\pocketmine\world\Block::$state['0,4,0'] ?? 0) === 0, (string)$placeChat);
		// En creativo: se coloca piedra (el paquete moderno no trae el item).
		fwrite($client, Protocol::frame($profile->id('chat_command'), Protocol::string('gamemode creative')));
		pump($server, ['player' => $client], $buffers, 0.5);
		takeFrames($buffers['player']); // vacía el "Tu modo..." previo
		fwrite($client, Protocol::frame($profile->id('block_place'), $placePacket));
		pump($server, ['player' => $client], $buffers, 0.5);
		$placed = 0;
		foreach(takeFrames($buffers['player']) as [$id, $payload]){
			if($id === $profile->id('block_change')){
				$offset = 0;
				$position = Protocol::readPosition($payload, $offset);
				$state = Protocol::readVarInt($payload, $offset);
				if($position === [0, 4, 0] && $state === 1){
					$placed++;
				}
			}
		}
		check("{$label} block_place creative -> piedra en (0,4,0)", $placed === 1 && (\pocketmine\world\Block::$state['0,4,0'] ?? 0) === 1, "placed={$placed} estado=" . var_export(\pocketmine\world\Block::$state['0,4,0'] ?? null, true));

		$statusJson = json_decode($server->statusJson($protocol), true);
		check("{$label} status online", ($statusJson['players']['online'] ?? null) === 1);

		// ---- paridad con la API de plugins ----
		$registrations = \pocketmine\runtime\Bridge::callsTo('player.register');
		$registeredNames = array_map(static fn(array $call) : string => (string)($call['name'] ?? ''), $registrations);
		check("{$label} API: jugador Java registrado", in_array($playerName, $registeredNames, true), implode(', ', $registeredNames));
		check(
			"{$label} API: aparece en getOnlinePlayers",
			array_key_exists(strtolower($playerName), \pocketmine\Server::instance()->getOnlinePlayers())
		);

		// Un plugin le habla con la API normal y la línea sale por su socket.
		\pocketmine\runtime\Bridge::$messages[strtolower($playerName)] = ['plugin line here'];
		pump($server, ['player' => $client], $buffers, 0.3);
		$pluginLine = false;
		foreach(takeFrames($buffers['player']) as [$id, $payload]){
			if($id === $profile->id('system_chat') && str_contains($payload, 'plugin line here')){
				$pluginLine = true;
			}
		}
		check("{$label} API: líneas de plugins llegan al cliente", $pluginLine);

		// ---- desconexión ----
		$broadcastsBefore = count(\pocketmine\Server::instance()->broadcasts);
		fclose($client);
		$server->tick();
		$server->tick();
		$broadcasts = \pocketmine\Server::instance()->broadcasts;
		check(
			"{$label} desconexión anunciada",
			count($broadcasts) > $broadcastsBefore && end($broadcasts) === "{$playerName} left the game",
			(string)end($broadcasts)
		);
		check(
			"{$label} API: salida por PlayerQuitEvent",
			\pocketmine\plugin\PluginManager::last(\pocketmine\event\player\PlayerQuitEvent::class) !== null
		);
		check("{$label} API: sale de getOnlinePlayers", !array_key_exists(strtolower($playerName), \pocketmine\Server::instance()->getOnlinePlayers()));
		check("{$label} sin clientes", $server->getClients() === []);
	}

	// ---- agua: estado 86 y contador de fluidos real en 26.1 ----
	// Sin esto `hasFluid()` es false en 26.1 y no se puede nadar.
	foreach([769, 775] as $waterProtocol){
		$waterProfile = ModernProfile::forProtocol($waterProtocol);
		if($waterProfile === null){
			continue;
		}
		$waterLabel = "{$waterProfile->name} ({$waterProtocol})";
		$waterChunk = new class{
			public function getSectionPayload(int $baseY) : ?array{
				if($baseY !== 48){
					return null; // sección 7 (Y 48-63): la del nivel del mar
				}
				// Paleta Bedrock: aire + agua, 1 bit por bloque. Mitad agua.
				$bits = 1;
				$perWord = 32;
				$words = array_fill(0, 128, 0);
				for($x = 0; $x < 16; $x++){
					for($z = 0; $z < 16; $z++){
						for($y = 0; $y < 16; $y++){
							if($y >= 8){
								continue;
							}
							$entry = ($x << 8) | ($z << 4) | $y;
							$words[intdiv($entry, $perWord)] |= 1 << (($entry % $perWord) * $bits);
						}
					}
				}
				return ['bits' => $bits, 'words' => $words, 'palette' => [3690217760, 1211861802]];
			}
		};
		$waterSections = parseChunkData(\javabridge\ModernChunk::fromChunk($waterChunk, $waterProfile), $waterProfile);
		$waterSection = $waterSections[7];
		check("{$waterLabel} agua: estado 86", in_array(86, $waterSection['palette'], true), json_encode($waterSection['palette']));
		check("{$waterLabel} agua: contador de bloques 2048", $waterSection['blockCount'] === 2048, (string)$waterSection['blockCount']);
		if($waterProfile->sectionsHaveFluidCount()){
			check("{$waterLabel} agua: contador de fluidos 2048", $waterSection['fluidCount'] === 2048, (string)$waterSection['fluidCount']);
		}
		// Entrada Java (y=7,z=0,x=0) es agua; (y=8,z=0,x=0) es aire.
		check("{$waterLabel} agua: bloque en su coordenada", containerState($waterSection, (7 << 8) | 0) === 86 && containerState($waterSection, (8 << 8) | 0) === 0);
	}

	// ---- player_info_update: cambio de gamemode (creativo) ----
	// `packet_player_info` en protocol.json de 769-775: bitflags u8
	// (add_player=0x01, initialize_chat=0x02, update_game_mode=0x04,
	// update_listed=0x08, ...) + count varInt + UUID + solo los campos
	// activados por el bitset. Mandar 0x02 hacía que el cliente leyera
	// `chat_session` (option) tras el UUID y abortara con DecoderException
	// al cambiar a creativo. Bytes exactos: 04 01 <uuid 16B> <gamemode
	// varInt> (19 bytes en gamemode 0-3), sin add_player. Ids contra el
	// mapper `packet` de cada protocol.json: 769=64, 770=63, 773=68,
	// 774=68, 775=70. `setGamemode` moderno manda además
	// `game_state_change` (razón 3 `change_game_mode` + gamemode f32,
	// que actualiza la UI del jugador local) y `abilities` (flags +
	// 2 f32): sin ellos la TAB cambia pero la UI sigue en survival.
	$playerInfoIds = [769 => 64, 770 => 63, 773 => 68, 774 => 68, 775 => 70];
	$gameStateIds = [769 => 35, 770 => 34, 773 => 38, 774 => 38, 775 => 38];
	$abilitiesIds = [769 => 58, 770 => 57, 773 => 62, 774 => 62, 775 => 64];
	foreach($playerInfoIds as $gmProtocol => $expectedPlayerInfoId){
		$gmProfile = ModernProfile::forProtocol($gmProtocol);
		if($gmProfile === null){
			check("{$gmProtocol} player_info: perfil", false);
			continue;
		}
		$gmLabel = "{$gmProfile->name} ({$gmProtocol})";
		check("{$gmLabel} player_info: id", $gmProfile->id('player_info_update') === $expectedPlayerInfoId, (string)$gmProfile->id('player_info_update'));
		check("{$gmLabel} game_state_change: id", $gmProfile->id('game_state_change') === $gameStateIds[$gmProtocol], (string)$gmProfile->id('game_state_change'));
		check("{$gmLabel} abilities: id", $gmProfile->id('abilities') === $abilitiesIds[$gmProtocol], (string)$gmProfile->id('abilities'));

		$gmStream = fopen('php://temp', 'r+b');
		if(!is_resource($gmStream)){
			check("{$gmLabel} player_info_update: stream", false);
			continue;
		}
		$gmClient = new \javabridge\JavaClient($gmStream, '127.0.0.1:1234');
		foreach(['profile' => $gmProfile, 'playing' => true, 'username' => 'Steve', 'protocol' => $gmProtocol] as $gmProp => $gmValue){
			$gmPropRef = new ReflectionProperty(\javabridge\JavaClient::class, $gmProp);
			$gmPropRef->setAccessible(true);
			$gmPropRef->setValue($gmClient, $gmValue);
		}
		$gmClient->setGamemode(1); // survival -> creative, como `/gamemode creative`
		rewind($gmStream);
		$gmBytes = (string)stream_get_contents($gmStream);
		$gmFrames = takeFrames($gmBytes);
		$gmPayload = ($gmFrames[0][1] ?? '');
		$gmExpected = chr(0x04) . Protocol::varInt(1) . Protocol::offlineUuid('Steve') . Protocol::varInt(1);
		check(
			"{$gmLabel} player_info_update: frame creative",
			count($gmFrames) === 3 && $gmFrames[0][0] === $expectedPlayerInfoId && $gmPayload === $gmExpected && $gmBytes === '',
			'frames=' . count($gmFrames) . ' id=' . ($gmFrames[0][0] ?? -1) . ' hex=' . bin2hex($gmPayload)
		);
		// `game_state_change` (razón 3 `change_game_mode` + f32) es el
		// que actualiza la UI local (corazones/hambre): bytes exactos
		// 03 <f32 gamemode>, id por versión.
		$gsPayload = ($gmFrames[1][1] ?? '');
		$gsExpected = chr(3) . Protocol::float(1.0);
		check(
			"{$gmLabel} game_state_change: change_game_mode a creativo",
			($gmFrames[1][0] ?? -1) === $gameStateIds[$gmProtocol] && $gsPayload === $gsExpected,
			'id=' . ($gmFrames[1][0] ?? -1) . ' hex=' . bin2hex($gsPayload) . ' exp=' . bin2hex($gsExpected)
		);
		// `abilities` (i8 flags + 2 f32): en creativo 0x0D
		// (invulnerable + allowFlying + instabuild) con 0.05/0.1.
		$abPayload = ($gmFrames[2][1] ?? '');
		$abExpected = chr(0x0D) . Protocol::float(0.05) . Protocol::float(0.1);
		check(
			"{$gmLabel} abilities: creativo con vuelo",
			($gmFrames[2][0] ?? -1) === $abilitiesIds[$gmProtocol] && $abPayload === $abExpected,
			'id=' . ($gmFrames[2][0] ?? -1) . ' hex=' . bin2hex($abPayload) . ' exp=' . bin2hex($abExpected)
		);
		// Decodificación campo a campo, como hace el cliente vanilla.
		$gmOffset = 0;
		$gmFlags = $gmPayload === '' ? -1 : ord($gmPayload[$gmOffset]);
		$gmOffset++;
		$gmCount = Protocol::readVarInt($gmPayload, $gmOffset);
		$gmUuid = substr($gmPayload, $gmOffset, 16);
		$gmOffset += 16;
		$gmMode = Protocol::readVarInt($gmPayload, $gmOffset);
		check(
			"{$gmLabel} player_info_update: bitset solo update_game_mode",
			$gmFlags === 0x04 && $gmCount === 1 && $gmUuid === Protocol::offlineUuid('Steve') && $gmMode === 1 && $gmOffset === strlen($gmPayload),
			"flags={$gmFlags} count=" . var_export($gmCount, true) . " gm=" . var_export($gmMode, true) . " len=" . strlen($gmPayload)
		);
		$gmMethod = new ReflectionMethod(\javabridge\JavaClient::class, 'modernPlayerInfoGamemode');
		$gmMethod->setAccessible(true);
		$gmPayload0 = (string)$gmMethod->invoke($gmClient, 0, 'Steve');
		check(
			"{$gmLabel} player_info_update: survival 19 bytes",
			$gmPayload0 === chr(0x04) . Protocol::varInt(1) . Protocol::offlineUuid('Steve') . Protocol::varInt(0) && strlen($gmPayload0) === 19,
			bin2hex($gmPayload0)
		);
		fclose($gmStream);
	}

	// Legacy 340 intacto: S2C_PLAYER_INFO acción 3 + S2C_ABILITIES.
	$legacyStream = fopen('php://temp', 'r+b');
	if(is_resource($legacyStream)){
		$legacyClient = new \javabridge\JavaClient($legacyStream, '127.0.0.1:1235');
		foreach(['playing' => true, 'username' => 'Steve'] as $legacyProp => $legacyValue){
			$legacyPropRef = new ReflectionProperty(\javabridge\JavaClient::class, $legacyProp);
			$legacyPropRef->setAccessible(true);
			$legacyPropRef->setValue($legacyClient, $legacyValue);
		}
		$legacyClient->setGamemode(1);
		rewind($legacyStream);
		$legacyBytes = (string)stream_get_contents($legacyStream);
		$legacyFrames = takeFrames($legacyBytes);
		$legacyExpected = Protocol::varInt(3) . Protocol::varInt(1) . Protocol::offlineUuid('Steve') . Protocol::varInt(1);
		check(
			'legacy player_info: acción 3 intacta',
			count($legacyFrames) === 2 && $legacyFrames[0][0] === Protocol::S2C_PLAYER_INFO
				&& ($legacyFrames[0][1] ?? '') === $legacyExpected && $legacyFrames[1][0] === Protocol::S2C_ABILITIES,
			'frames=' . count($legacyFrames) . ' hex=' . bin2hex($legacyFrames[0][1] ?? '')
		);
		fclose($legacyStream);
	}else{
		check('legacy player_info: stream', false);
	}

	// ---- Fase 1/2 (unit): cola, puerta de chunk, fallback y placeBlock ----
	// Un cliente moderno sintético sobre un stream temporal, sin login.
	$unitProfile = ModernProfile::forProtocol(775);
	$unitStream = fopen('php://temp', 'r+b');
	check('fase1/2 unit: stream y perfil', is_resource($unitStream) && $unitProfile !== null);
	$unitClient = new \javabridge\JavaClient($unitStream, '127.0.0.1:9999');
	foreach(['profile' => $unitProfile, 'playing' => true, 'username' => 'Unit', 'protocol' => 775, 'x' => 0.5, 'y' => 5.62, 'z' => 0.5, 'sentChunks' => ['0:0' => true]] as $unitProp => $unitValue){
		$unitPropRef = new ReflectionProperty(\javabridge\JavaClient::class, $unitProp);
		$unitPropRef->setAccessible(true);
		$unitPropRef->setValue($unitClient, $unitValue);
	}
	$unitClientsProp = new ReflectionProperty(JavaServer::class, 'clients');
	$unitClientsProp->setAccessible(true);
	$unitClients = $server->getClients();
	$unitClients[] = $unitClient;
	$unitClientsProp->setValue($server, $unitClients);
	$flushUnit = new ReflectionMethod(\javabridge\JavaClient::class, 'flush');
	$flushUnit->setAccessible(true);
	// `sendBlockChange()` solo encola en `$out`: en el flujo real lo vacía
	// el tick/poll del cliente, aquí se vacía a mano antes de leer.
	$drainUnit = function() use ($unitStream, $unitClient, $flushUnit) : array{
		$flushUnit->invoke($unitClient);
		rewind($unitStream);
		$bytes = (string)stream_get_contents($unitStream);
		// Sin truncar, la siguiente lectura repetiría los frames ya leídos.
		ftruncate($unitStream, 0);
		rewind($unitStream);
		return takeFrames($bytes);
	};
	$tickBlockChanges = new ReflectionMethod(JavaServer::class, 'tickBlockChanges');
	$tickBlockChanges->setAccessible(true);

	// Puerta de chunk: sin chunk enviado no hay block_change.
	$server->broadcastBlockChange(160, 64, 160, '', 2150698529);
	check('fase1: sin chunk no hay block_change', $drainUnit() === []);
	$server->broadcastBlockChange(0, 4, 0, '', 2150698529);
	$unitFrames = $drainUnit();
	check(
		'fase1: con chunk, piedra real (estado 1)',
		count($unitFrames) === 1 && $unitFrames[0][0] === $unitProfile->id('block_change'),
		'frames=' . count($unitFrames)
	);

	// Cola del host (Bedrock -> Java): se drena y se reenvía una vez.
	\pocketmine\world\Block::$state['0,5,0'] = 1;
	\pocketmine\runtime\Bridge::$blockChanges = [['x' => 0, 'y' => 5, 'z' => 0, 'block_id' => 2150698529]];
	$tickBlockChanges->invoke($server);
	$unitFrames = $drainUnit();
	check(
		'fase1: take_block_changes se consume y reenvía piedra',
		\pocketmine\runtime\Bridge::$blockChanges === [] && count($unitFrames) === 1,
		'frames=' . count($unitFrames)
	);
	// El mismo cambio otra vez (eco de un place/break propio): sin reenvío.
	\pocketmine\runtime\Bridge::$blockChanges = [['x' => 0, 'y' => 5, 'z' => 0, 'block_id' => 2150698529]];
	$tickBlockChanges->invoke($server);
	check('fase1: eco duplicado no se reenvía', $drainUnit() === []);

	// Fallback por polling (sin cola en Rust): Bedrock quita el bloque y
	// el polling sobre los conocidos lo detecta.
	\pocketmine\runtime\Bridge::$blockChangesAvailable = false;
	\pocketmine\world\Block::$state['0,5,0'] = 0;
	$tickBlockChanges->invoke($server);
	$unitFrames = $drainUnit();
	$airSeen = false;
	foreach($unitFrames as [$unitId, $unitPayload]){
		if($unitId !== $unitProfile->id('block_change')){
			continue;
		}
		$offset = 0;
		$airSeen = Protocol::readPosition($unitPayload, $offset) === [0, 5, 0] && Protocol::readVarInt($unitPayload, $offset) === 0;
	}
	check('fase1 fallback: polling detecta el aire', $airSeen, 'frames=' . count($unitFrames));
	\pocketmine\runtime\Bridge::$blockChangesAvailable = true;

	// placeBlock directo: alcance, vacío y escritura real.
	check('fase2 placeBlock: fuera de alcance rechazado', $server->placeBlock($unitClient, 100, 64, 100, 2150698529) === false);
	\pocketmine\world\Block::$state['0,3,0'] = 1;
	check('fase2 placeBlock: celda ocupada rechazada', $server->placeBlock($unitClient, 0, 3, 0, 2150698529) === false);
	\pocketmine\world\Block::$state['0,6,0'] = 0;
	check(
		'fase2 placeBlock: coloca piedra (red Bedrock) en vacío',
		$server->placeBlock($unitClient, 0, 6, 0, 2150698529) === true && (\pocketmine\world\Block::$state['0,6,0'] ?? 0) === 1
	);
	$unitFrames = $drainUnit();
	check('fase2 placeBlock: avisa a los Java', count($unitFrames) === 1 && $unitFrames[0][0] === $unitProfile->id('block_change'), 'frames=' . count($unitFrames));
	\pocketmine\world\Block::$state['1,6,0'] = 0;
	check('fase2 placeBlock: también acepta id legacy', $server->placeBlock($unitClient, 1, 6, 0, 1) === true && (\pocketmine\world\Block::$state['1,6,0'] ?? 0) === 1);
	$drainUnit();
	$unitClientsProp->setValue($server, []);
	fclose($unitStream);

	// ---- Bedrock visible para Java (refresh desde snapshots del host) ----
	// La copia PHP del jugador Bedrock se congela en el join; el tick debe
	// leer la posición viva de `server.get_players_full` para que el
	// movimiento Bedrock->Java llegue como `rel_entity_move`.
	$visStream = fopen('php://temp', 'r+b');
	check('bedrock-vis: stream y perfil', is_resource($visStream) && $unitProfile !== null);
	$visClient = new \javabridge\JavaClient($visStream, '127.0.0.1:9998');
	foreach(['profile' => $unitProfile, 'playing' => true, 'username' => 'Viewer', 'protocol' => 775, 'sentChunks' => ['0:0' => true]] as $visProp => $visValue){
		$visPropRef = new ReflectionProperty(\javabridge\JavaClient::class, $visProp);
		$visPropRef->setAccessible(true);
		$visPropRef->setValue($visClient, $visValue);
	}
	$flushVis = new ReflectionMethod(\javabridge\JavaClient::class, 'flush');
	$flushVis->setAccessible(true);
	$drainVis = function() use ($visStream, $visClient, $flushVis) : array{
		$flushVis->invoke($visClient);
		rewind($visStream);
		$bytes = (string)stream_get_contents($visStream);
		ftruncate($visStream, 0);
		rewind($visStream);
		return takeFrames($bytes);
	};
	$tickEntities = new ReflectionMethod(JavaServer::class, 'tickEntities');
	$tickEntities->setAccessible(true);
	$savedClients = $server->getClients();
	$unitClientsProp->setValue($server, [$visClient]);
	$bedrockStub = new \pocketmine\player\Player(['name' => 'BedrockGuy', 'world' => 'world']);
	\pocketmine\Server::instance()->addPlayer($bedrockStub);
	\pocketmine\runtime\Bridge::$playerSnapshots = [
		['name' => 'BedrockGuy', 'x' => 5.5, 'y' => 6.0, 'z' => 5.5, 'yaw' => 90.0, 'pitch' => 0.0, 'gamemode' => 0, 'is_virtual' => false],
	];
	$tickEntities->invoke($server);
	$visInfo = false;
	$visSpawn = false;
	foreach($drainVis() as [$visId, $visPayload]){
		if($visId === $unitProfile->id('player_info')){
			$visInfo = true;
		}
		if($visId === $unitProfile->id('spawn_entity')){
			$visSpawn = str_contains($visPayload, Protocol::double(5.5))
				&& str_contains($visPayload, Protocol::double(6.0));
		}
	}
	check('bedrock-vis: spawn con posición del snapshot (5.5,6.0)', $visInfo && $visSpawn, 'info=' . (int)$visInfo . ' spawn=' . (int)$visSpawn);
	// Yaw Bedrock 90 (este) -> yaw Java -90: sin el +180 el Java ve al
	// Bedrock mirando hacia atrás.
	$visSeen = $visClient->getSeenEntities();
	check('bedrock-vis: yaw Bedrock 90 -> Java -90', isset($visSeen['bedrockguy']) && abs($visSeen['bedrockguy']['yaw'] - (-90.0)) < 0.001, json_encode($visSeen));
	// Movimiento Bedrock: el mismo spawn, `rel_entity_move` al snapshot nuevo.
	\pocketmine\runtime\Bridge::$playerSnapshots[0]['x'] = 6.5;
	$tickEntities->invoke($server);
	$visMoved = false;
	foreach($drainVis() as [$visId2, $visPayload2]){
		if($visId2 === $unitProfile->id('rel_entity_move')){
			$visMoved = true;
		}
	}
	check('bedrock-vis: el movimiento Bedrock llega como rel_move', $visMoved);
	// Sin copia PHP: un Bedrock vivo solo en el host también se ve.
	\pocketmine\Server::instance()->removePlayer($bedrockStub);
	\pocketmine\runtime\Bridge::$playerSnapshots = [
		['name' => 'GhostBedrock', 'x' => 0.5, 'y' => 5.62, 'z' => 0.5, 'yaw' => 0.0, 'pitch' => 0.0, 'gamemode' => 0, 'is_virtual' => false],
	];
	$tickEntities->invoke($server);
	$ghostSpawn = false;
	foreach($drainVis() as [$visId3, $visPayload3]){
		if($visId3 === $unitProfile->id('spawn_entity') && str_contains($visPayload3, Protocol::offlineUuid('GhostBedrock'))){
			$ghostSpawn = true;
		}
	}
	check('bedrock-vis: Bedrock sin copia PHP también spawnea', $ghostSpawn);
	// Salida: sin snapshot ni copia -> entity_destroy.
	\pocketmine\runtime\Bridge::$playerSnapshots = [];
	$tickEntities->invoke($server);
	$ghostGone = false;
	foreach($drainVis() as [$visId4, $visPayload4]){
		if($visId4 === $unitProfile->id('entity_destroy')){
			$ghostGone = true;
		}
	}
	check('bedrock-vis: la salida despawnea', $ghostGone);
	$unitClientsProp->setValue($server, $savedClients);
	\pocketmine\runtime\Bridge::$playerSnapshots = [];
	fclose($visStream);

	// ---- spawn_entity: payload campo a campo por protocolo ----
	// `packet_spawn_entity` en `protocol.json` (target/javamc-<versión>):
	// 769/770: entityId varint, UUID, type varint, x/y/z f64, pitch/yaw/head
	// i8, objectData varint, velocity vec3i16 al final. 773/774/775: lo
	// mismo pero velocity lpVec3 (native, 0x00 para cero) delante de los
	// ángulos. Mandar el formato viejo en 1.21.9+ hacía que el cliente leyera
	// el byte de pitch como cabecera lpVec3 y abortara con DecoderException
	// en `clientbound/minecraft:add_entity` (26.1). Id del paquete 0x01 en
	// las cinco versiones; type player 147/148/151/155/155 (entities.json).
	foreach([769 => 147, 770 => 148, 773 => 151, 774 => 155, 775 => 155] as $spawnProtocol => $expectedType){
		$spawnProfile = ModernProfile::forProtocol($spawnProtocol);
		if($spawnProfile === null){
			check("spawn_entity {$spawnProtocol}: perfil", false);
			continue;
		}
		$spawnLabel = "{$spawnProfile->name} ({$spawnProtocol})";
		check("{$spawnLabel} spawn_entity: id 1 y type player {$expectedType}", $spawnProfile->id('spawn_entity') === 1 && $spawnProfile->playerEntityType() === $expectedType);
		check("{$spawnLabel} spawn_entity: lpVec3 igual a 773+", $spawnProfile->spawnVelocityIsLpVec3() === ($spawnProtocol >= 773));
		$spawnStream = fopen('php://temp', 'r+b');
		if(!is_resource($spawnStream)){
			check("{$spawnLabel} spawn_entity: stream", false);
			continue;
		}
		$spawnClient = new \javabridge\JavaClient($spawnStream, '127.0.0.1:9997');
		foreach(['profile' => $spawnProfile, 'playing' => true, 'username' => 'Viewer', 'protocol' => $spawnProtocol] as $spawnProp => $spawnValue){
			$spawnPropRef = new ReflectionProperty(\javabridge\JavaClient::class, $spawnProp);
			$spawnPropRef->setAccessible(true);
			$spawnPropRef->setValue($spawnClient, $spawnValue);
		}
		$spawnMethod = new ReflectionMethod(\javabridge\JavaClient::class, 'modernSpawnPayload');
		$spawnMethod->setAccessible(true);
		$spawnPayload = (string)$spawnMethod->invoke($spawnClient, 123, ['name' => 'Steve', 'x' => 5.5, 'y' => 6.0, 'z' => 5.5, 'yaw' => 90.0, 'pitch' => 10.0, 'gamemode' => 0]);
		$spawnOffset = 0;
		$spawnEntityId = Protocol::readVarInt($spawnPayload, $spawnOffset);
		$spawnUuid = substr($spawnPayload, $spawnOffset, 16);
		$spawnOffset += 16;
		$spawnType = Protocol::readVarInt($spawnPayload, $spawnOffset);
		$spawnX = Protocol::readDouble($spawnPayload, $spawnOffset);
		$spawnY = Protocol::readDouble($spawnPayload, $spawnOffset);
		$spawnZ = Protocol::readDouble($spawnPayload, $spawnOffset);
		$spawnLpOk = true;
		$spawnTrailOk = true;
		if($spawnProfile->spawnVelocityIsLpVec3()){
			$spawnLpOk = ($spawnPayload[$spawnOffset] ?? '') === "\x00";
			$spawnOffset += 1; // lpVec3 cero
			$spawnPitch = ord($spawnPayload[$spawnOffset]); $spawnOffset++;
			$spawnYaw = ord($spawnPayload[$spawnOffset]); $spawnOffset++;
			$spawnHead = ord($spawnPayload[$spawnOffset]); $spawnOffset++;
			$spawnData = Protocol::readVarInt($spawnPayload, $spawnOffset);
		}else{
			$spawnPitch = ord($spawnPayload[$spawnOffset]); $spawnOffset++;
			$spawnYaw = ord($spawnPayload[$spawnOffset]); $spawnOffset++;
			$spawnHead = ord($spawnPayload[$spawnOffset]); $spawnOffset++;
			$spawnData = Protocol::readVarInt($spawnPayload, $spawnOffset);
			$spawnTrailOk = substr($spawnPayload, $spawnOffset) === "\x00\x00\x00\x00\x00\x00";
			$spawnOffset += 6;
		}
		check(
			"{$spawnLabel} spawn_entity: campos sin sobrantes",
			$spawnEntityId === 123 && $spawnUuid === Protocol::offlineUuid('Steve') && $spawnType === $expectedType
				&& $spawnX === 5.5 && $spawnY === 6.0 && $spawnZ === 5.5
				&& $spawnPitch === (Protocol::angle(10.0) & 0xFF) && $spawnYaw === (Protocol::angle(90.0) & 0xFF) && $spawnHead === (Protocol::angle(90.0) & 0xFF)
				&& $spawnData === 0 && $spawnLpOk && $spawnTrailOk && $spawnOffset === strlen($spawnPayload),
			"len=" . strlen($spawnPayload) . " off={$spawnOffset} lp=" . (int)$spawnLpOk . " trail=" . (int)$spawnTrailOk . " hex=" . bin2hex($spawnPayload)
		);
		fclose($spawnStream);
	}
	// ---- entity_metadata / set_entity_data: custom name Optional ----
	// Campo 2 (DATA_CUSTOM_NAME) es Optional<Component> desde siempre en
	// moderno: el serializador es optional_component=6 (no component=5)
	// y el valor es option(anonymousNbt): byte 0x01 + NBT. Con el tipo 5
	// SynchedEntityData asigna un MutableComponent a un Optional y el
	// cliente 26.1 aborta con IllegalStateException al spawnear un
	// RemotePlayer (id 1000001). Campo 3 (visible) es boolean=8 en las
	// cinco versiones (entityMetadataEntry de protocol.json).
	// Bytes del campo 2 con "Steve": antes 02 05 0A..00 (directo), ahora
	// 02 06 01 0A..00 (Optional presente). Ids: 769=93, 770=92,
	// 773/774=97, 775=99.
	foreach([769 => 93, 770 => 92, 773 => 97, 774 => 97, 775 => 99] as $metaProtocol => $expectedMetaId){
		$metaProfile = ModernProfile::forProtocol($metaProtocol);
		if($metaProfile === null){
			check("entity_metadata {$metaProtocol}: perfil", false);
			continue;
		}
		$metaLabel = "{$metaProfile->name} ({$metaProtocol})";
		check("{$metaLabel} entity_metadata: id {$expectedMetaId}", $metaProfile->id('entity_metadata') === $expectedMetaId, (string)$metaProfile->id('entity_metadata'));
		$metaStream = fopen('php://temp', 'r+b');
		if(!is_resource($metaStream)){
			check("{$metaLabel} entity_metadata: stream", false);
			continue;
		}
		$metaClient = new \javabridge\JavaClient($metaStream, '127.0.0.1:9996');
		foreach(['profile' => $metaProfile, 'playing' => true, 'username' => 'Viewer', 'protocol' => $metaProtocol] as $metaProp => $metaValue){
			$metaPropRef = new ReflectionProperty(\javabridge\JavaClient::class, $metaProp);
			$metaPropRef->setAccessible(true);
			$metaPropRef->setValue($metaClient, $metaValue);
		}
		$metaMethod = new ReflectionMethod(\javabridge\JavaClient::class, 'modernNametagPayload');
		$metaMethod->setAccessible(true);
		$metaPayload = (string)$metaMethod->invoke($metaClient, 123, 'Steve');
		$metaOffset = 0;
		$metaEntityId = Protocol::readVarInt($metaPayload, $metaOffset);
		$metaKey2 = ord($metaPayload[$metaOffset]); $metaOffset++;
		$metaType2 = Protocol::readVarInt($metaPayload, $metaOffset);
		$metaPresent = $metaPayload[$metaOffset] ?? ''; $metaOffset++;
		$metaText = parseTextComponent(substr($metaPayload, $metaOffset));
		skipNbt($metaPayload, $metaOffset); // component Optional presente
		$metaKey3 = ord($metaPayload[$metaOffset]); $metaOffset++;
		$metaType3 = Protocol::readVarInt($metaPayload, $metaOffset);
		$metaVisible = $metaPayload[$metaOffset] ?? ''; $metaOffset++;
		$metaEnd = $metaPayload[$metaOffset] ?? ''; $metaOffset++;
		check(
			"{$metaLabel} entity_metadata: custom name Optional presente",
			$metaEntityId === 123 && $metaKey2 === 2 && $metaType2 === 6 && $metaPresent === "\x01" && $metaText === 'Steve'
				&& $metaKey3 === 3 && $metaType3 === 8 && $metaVisible === "\x01" && $metaEnd === "\xFF" && $metaOffset === strlen($metaPayload),
			"len=" . strlen($metaPayload) . " off={$metaOffset} type2={$metaType2} present=" . bin2hex($metaPresent) . " text={$metaText} hex=" . bin2hex($metaPayload)
		);
		fclose($metaStream);
	}
	// Legacy 340 intacto: custom name string=3 directo, visible boolean=6.
	$legacyMetaStream = fopen('php://temp', 'r+b');
	if(is_resource($legacyMetaStream)){
		$legacyMetaClient = new \javabridge\JavaClient($legacyMetaStream, '127.0.0.1:9995');
		$legacyMetaMethod = new ReflectionMethod(\javabridge\JavaClient::class, 'legacyNametagPayload');
		$legacyMetaMethod->setAccessible(true);
		$legacyMetaPayload = (string)$legacyMetaMethod->invoke($legacyMetaClient, 123, 'Steve');
		$legacyMetaExpected = Protocol::varInt(123) . chr(2) . chr(3) . Protocol::string('Steve') . chr(3) . chr(6) . Protocol::boolean(true) . chr(0xFF);
		check('legacy entity_metadata: string directo intacto', $legacyMetaPayload === $legacyMetaExpected, bin2hex($legacyMetaPayload));
		fclose($legacyMetaStream);
	}else{
		check('legacy entity_metadata: stream', false);
	}
	check('lpVec3: cero es 0x00', Protocol::lpVec3(0.0, 0.0, 0.0) === "\x00");

	// ---- yaw Bedrock <-> Java (+180, Bedrock 0 = norte, Java 0 = sur) ----
	check('yaw: Bedrock 0 (norte) -> Java 180', abs(Protocol::bedrockYawToJava(0.0) - 180.0) < 0.001);
	check('yaw: Bedrock 180 (sur) -> Java 0', abs(Protocol::bedrockYawToJava(180.0)) < 0.001);
	check('yaw: Bedrock 90 (este) -> Java -90', abs(Protocol::bedrockYawToJava(90.0) - (-90.0)) < 0.001);
	check('yaw: Bedrock 270 (oeste) -> Java 90', abs(Protocol::bedrockYawToJava(270.0) - 90.0) < 0.001);
	check('yaw: Java 0 (sur) -> Bedrock 180', abs(Protocol::javaYawToBedrock(0.0) - 180.0) < 0.001);
	check('yaw: Java -90 (este) -> Bedrock 90', abs(Protocol::javaYawToBedrock(-90.0) - 90.0) < 0.001);
	check('yaw: ida y vuelta Bedrock->Java->Bedrock', abs(Protocol::javaYawToBedrock(Protocol::bedrockYawToJava(123.0)) - 123.0) < 0.001);
	check('pitch: Bedrock 315 -> Java -45', abs(Protocol::bedrockPitchToJava(315.0) - (-45.0)) < 0.001);
	check('pitch: Bedrock -45 -> Java -45', abs(Protocol::bedrockPitchToJava(-45.0) - (-45.0)) < 0.001);

	// ---- update_time: payload por versión y sync periódica/salto ----
	// Ids de minecraft-data `packet_update_time`: 769=107, 770=106,
	// 773/774=111, 775=113. 769-774: age i64 + time i64 + tickDayTime
	// bool; 775: age i64 + clockUpdates[{id,totalTicks,partial,rate}].
	$timeIds = [769 => 107, 770 => 106, 773 => 111, 774 => 111, 775 => 113];
	foreach($timeIds as $timeProtocol => $expectedTimeId){
		$timeProfile = ModernProfile::forProtocol($timeProtocol);
		if($timeProfile === null){
			check("{$timeProtocol} update_time: perfil", false);
			continue;
		}
		$timeLabel = "{$timeProfile->name} ({$timeProtocol})";
		check("{$timeLabel} update_time: id {$expectedTimeId}", $timeProfile->id('update_time') === $expectedTimeId);
		$timeStream = fopen('php://temp', 'r+b');
		if(!is_resource($timeStream)){
			check("{$timeLabel} update_time: stream", false);
			continue;
		}
		$timeClient = new \javabridge\JavaClient($timeStream, '127.0.0.1:9994');
		foreach(['profile' => $timeProfile, 'playing' => true, 'username' => 'Steve', 'protocol' => $timeProtocol] as $timeProp => $timeValue){
			$timePropRef = new ReflectionProperty(\javabridge\JavaClient::class, $timeProp);
			$timePropRef->setAccessible(true);
			$timePropRef->setValue($timeClient, $timeValue);
		}
		$timeClient->sendTime(6000, true);
		rewind($timeStream);
		$timeBytes = (string)stream_get_contents($timeStream);
		$timeFrames = takeFrames($timeBytes);
		$timePayload = $timeFrames[0][1] ?? '';
		if($timeProtocol >= 775){
			$timeOffset = 0;
			$age = Protocol::readLong($timePayload, $timeOffset);
			$clockCount = Protocol::readVarInt($timePayload, $timeOffset);
			$clockId = Protocol::readVarInt($timePayload, $timeOffset);
			$totalTicks = Protocol::readVarInt($timePayload, $timeOffset); // varLong == varInt en positivo
			$partial = Protocol::readFloat($timePayload, $timeOffset);
			$rate = Protocol::readFloat($timePayload, $timeOffset);
			check(
				"{$timeLabel} update_time: clockUpdates reloj 0 a 6000 rate 1",
				count($timeFrames) === 1 && $timeFrames[0][0] === 113 && $age === 0 && $clockCount === 1 && $clockId === 0 && $totalTicks === 6000 && $partial === 0.0 && $rate === 1.0 && $timeOffset === strlen($timePayload),
				"age=" . var_export($age, true) . " count=" . var_export($clockCount, true) . " id=" . var_export($clockId, true) . " ticks=" . var_export($totalTicks, true) . " rate=" . var_export($rate, true) . " hex=" . bin2hex($timePayload)
			);
		}else{
			$expectedPayload = Protocol::long(0) . Protocol::long(6000) . Protocol::boolean(true);
			check(
				"{$timeLabel} update_time: age+time+tickDayTime",
				count($timeFrames) === 1 && $timeFrames[0][0] === $expectedTimeId && $timePayload === $expectedPayload,
				'hex=' . bin2hex($timePayload)
			);
		}
		fclose($timeStream);
	}
	// Congelado (`/time stop`): tickDayTime=false / rate=0.
	$freezeStream = fopen('php://temp', 'r+b');
	if(is_resource($freezeStream)){
		$freezeProfile = ModernProfile::forProtocol(769);
		$freezeClient = new \javabridge\JavaClient($freezeStream, '127.0.0.1:9993');
		foreach(['profile' => $freezeProfile, 'playing' => true, 'username' => 'Steve', 'protocol' => 769] as $freezeProp => $freezeValue){
			$freezePropRef = new ReflectionProperty(\javabridge\JavaClient::class, $freezeProp);
			$freezePropRef->setAccessible(true);
			$freezePropRef->setValue($freezeClient, $freezeValue);
		}
		$freezeClient->sendTime(6000, false);
		rewind($freezeStream);
		$freezeBytes = (string)stream_get_contents($freezeStream);
		$freezeFrames = takeFrames($freezeBytes);
		check(
			'update_time congelado: tickDayTime false',
			($freezeFrames[0][1] ?? '') === Protocol::long(0) . Protocol::long(6000) . Protocol::boolean(false)
		);
		fclose($freezeStream);
	}
	// `tickTimeSync`: primer tick broadcast, siguiente sin broadcast
	// (throttle 1 s), salto (`/time set 13000`) broadcast inmediato.
	$syncStream = fopen('php://temp', 'r+b');
	check('tickTimeSync: stream', is_resource($syncStream));
	if(is_resource($syncStream)){
		$syncProfile = ModernProfile::forProtocol(769);
		$syncClient = new \javabridge\JavaClient($syncStream, '127.0.0.1:9992');
		foreach(['profile' => $syncProfile, 'playing' => true, 'username' => 'Timer', 'protocol' => 769] as $syncProp => $syncValue){
			$syncPropRef = new ReflectionProperty(\javabridge\JavaClient::class, $syncProp);
			$syncPropRef->setAccessible(true);
			$syncPropRef->setValue($syncClient, $syncValue);
		}
		$savedSyncClients = $server->getClients();
		$unitClientsProp->setValue($server, [$syncClient]);
		$flushSync = new ReflectionMethod(\javabridge\JavaClient::class, 'flush');
		$flushSync->setAccessible(true);
		$drainSync = function() use ($syncStream, $syncClient, $flushSync) : array{
			$flushSync->invoke($syncClient);
			rewind($syncStream);
			$bytes = (string)stream_get_contents($syncStream);
			ftruncate($syncStream, 0);
			rewind($syncStream);
			return takeFrames($bytes);
		};
		$tickTimeSync = new ReflectionMethod(JavaServer::class, 'tickTimeSync');
		$tickTimeSync->setAccessible(true);
		// El servidor ya hizo broadcasts durante los pumps anteriores:
		// se reinicia el estado para probar el primer broadcast.
		$lastTimeProp = new ReflectionProperty(JavaServer::class, 'lastBroadcastTime');
		$lastTimeProp->setAccessible(true);
		$lastTimeProp->setValue($server, null);
		$ticksTimeProp = new ReflectionProperty(JavaServer::class, 'ticksSinceTimeSync');
		$ticksTimeProp->setAccessible(true);
		$ticksTimeProp->setValue($server, 0);
		\pocketmine\world\World::$time = 6000;
		\pocketmine\world\World::$timeRunning = true;
		$tickTimeSync->invoke($server);
		$syncFrames = $drainSync();
		check('tickTimeSync: primer broadcast con 6000', count($syncFrames) === 1 && $syncFrames[0][0] === 107 && ($syncFrames[0][1] ?? '') === Protocol::long(0) . Protocol::long(6000) . Protocol::boolean(true), 'frames=' . count($syncFrames));
		$tickTimeSync->invoke($server);
		check('tickTimeSync: throttle sin rebroadcast', $drainSync() === []);
		\pocketmine\world\World::$time = 13000; // `/time set night` en Bedrock
		// El salto se detecta en ≤5 ticks (lectura throttled cada 5
		// ticks, ver `tickTimeSync`): se invoca hasta que salga.
		$syncFrames = [];
		for($tickAttempt = 0; $tickAttempt < 5; $tickAttempt++){
			$tickTimeSync->invoke($server);
			$syncFrames = array_merge($syncFrames, $drainSync());
			if($syncFrames !== []){
				break;
			}
		}
		check('tickTimeSync: salto a 13000 broadcast en ≤5 ticks', count($syncFrames) === 1 && ($syncFrames[0][1] ?? '') === Protocol::long(0) . Protocol::long(13000) . Protocol::boolean(true), 'frames=' . count($syncFrames));
		\pocketmine\world\World::$time = 6000;
		$unitClientsProp->setValue($server, $savedSyncClients);
		fclose($syncStream);
	}

	// ---- protocolo no soportado ----
	$unknown = connect($port);
	fwrite($unknown, handshake(767, 2, 'Herobrine'));
	$buffers = ['unknown' => ''];
	pump($server, ['unknown' => $unknown], $buffers, 0.2);
	$frames = takeFrames($buffers['unknown']);
	$offset = 0;
	$message = count($frames) === 1 && $frames[0][0] === Protocol::S2C_LOGIN_DISCONNECT
		? (string)Protocol::readString($frames[0][1], $offset)
		: '';
	check('protocolo no soportado explica las versiones', str_contains($message, 'Versiones soportadas'), $message);
	fclose($unknown);

	$server->stop();
	check('listener detenido', !$server->isRunning());

	fwrite(STDOUT, $failures === 0 ? "ALL OK\n" : "{$failures} FAILURE(S)\n");
	exit($failures === 0 ? 0 : 1);
}
