<?php

declare(strict_types=1);

namespace javabridge;

/**
 * Una conexión TCP de Minecraft Java Edition.
 *
 * Versiones soportadas:
 *  - 340 (1.12.2): handshake -> status | login -> play.
 *  - 769/770/773/774/775 (1.21.4, 1.21.5, 1.21.9, 1.21.11, 26.1):
 *    handshake -> login -> configuration -> play.
 *
 * Todo el I/O es no bloqueante; `poll()` se llama desde el tick del scheduler
 * y `tick()` envía keep-alive, chunks pendientes y vacía el buffer de salida.
 */
final class JavaClient{

	private const STATE_HANDSHAKE = 'handshake';
	private const STATE_STATUS = 'status';
	private const STATE_LOGIN = 'login';
	private const STATE_CONFIG = 'config';
	private const STATE_PLAY = 'play';
	private const STATE_CLOSED = 'closed';

	private const KEEP_ALIVE_SECONDS = 10.0;
	private const TIMEOUT_SECONDS = 30.0;
	/** Presupuesto de tiempo por tick para no bloquear el hilo del servidor. */
	private const CHUNK_BUDGET_SECONDS = 0.012;
	/** Columnas por lote; vanilla usa lotes de este orden. */
	private const CHUNKS_PER_BATCH = 64;
	/** Si hay más de esto en el buffer de salida, se espera al siguiente tick. */
	private const CHUNK_BACKPRESSURE = 262144;

	private const WORLD_NAME = 'minecraft:overworld';
	private const SEA_LEVEL = 63;

	/**
	 * Bitflags u8 de `player_info_update` (moderno), en el orden de
	 * `bitflags.flags` de minecraft-data (protocol.json por versión en
	 * target/javamc-*, idéntico en 769, 770, 773, 774 y 775):
	 * add_player=0x01, initialize_chat=0x02, update_game_mode=0x04,
	 * update_listed=0x08, update_latency=0x10, update_display_name=0x20,
	 * update_hat=0x40, update_list_order=0x80.
	 *
	 * OJO: 0x02 es `initialize_chat`, NO `update_game_mode`: con 0x02 el
	 * cliente espera un `chat_session` (option) tras el UUID y aborta con
	 * `DecoderException: Failed to decode packet 'clientbound/minecraft:player_info_update'`.
	 */
	private const PLAYER_INFO_ADD_PLAYER = 0x01;
	private const PLAYER_INFO_UPDATE_GAME_MODE = 0x04;
	private const PLAYER_INFO_UPDATE_LISTED = 0x08;
	private const PLAYER_INFO_UPDATE_LATENCY = 0x10;

	/**
	 * Razón `change_game_mode` de `game_state_change`
	 * (`packet_game_state_change`: reason u8 mapper + gameMode f32,
	 * idéntico en 769-775). Es el único paquete que actualiza el
	 * gamemode del jugador local: `player_info_update` solo
	 * actualiza su entrada en la lista TAB.
	 */
	private const GAME_EVENT_CHANGE_GAMEMODE = 3;

	/** @var resource|null */
	private $stream;

	private string $state = self::STATE_HANDSHAKE;
	private string $in = '';
	private string $out = '';
	private int $outOffset = 0;
	private bool $closed = false;

	private ?string $username = null;
	private bool $playing = false;
	private int $protocol = 0;
	private ?ModernProfile $profile = null;

	private float $x = 0.5;
	private float $y = 4.0;
	private float $z = 0.5;
	private float $yaw = 0.0;
	private float $pitch = 0.0;
	/** Gamemode conocido por el cliente Java (join_game / último cambio). */
	private int $gamemode = 0;

	/** @var array<string, true> chunks ya enviados */
	private array $sentChunks = [];
	/** @var array<int, array{0:int,1:int}> chunks pendientes de enviar */
	private array $pendingChunks = [];
	/** @var array<string, int> chunkX:chunkZ => intentos; tras 20 se mueve al final */
	private array $pendingAttempts = [];
	private int $lastChunkX = PHP_INT_MIN;
	private int $lastChunkZ = PHP_INT_MIN;
	private bool $spawnChunksLogged = false;
	private float $spawnChunksAt = 0.0;
	/** Techo de chunks por tick pedido por el cliente (`chunk_batch_received`, f32). */
	private ?float $chunksPerTick = null;
	private bool $viewAnnounced = false;

	private int $teleportId = 1;
	private int $keepAliveId = 0;
	private float $lastKeepAlive = 0.0;
	private float $lastSeen = 0.0;

	/** Contador de ticks para el `player.update_position` throttled (~2 ticks). */
	private int $tickCount = 0;
	private int $lastPosUpdateTick = PHP_INT_MIN;
	private ?array $lastSentPos = null;
	private ?int $ownEntityId = null;

	/** True once `joinApi` succeeded for this TCP session (once per session). */
	public bool $apiJoined = false;

	/** @var array<string, array{entity:int,x:float,y:float,z:float,yaw:float,pitch:float}> entidades vistas por este cliente */
	private array $seenEntities = [];

	/**
	 * Jugadores ya anunciados en la lista TAB de este cliente
	 * (`lower => ['name'=>..., 'gamemode'=>..., 'ping'=>...]`).
	 *
	 * Estado independiente de `$seenEntities`: la TAB es global (todos
	 * los online, incluido el propio, sin puerta de chunks), mientras
	 * que las entidades solo existen cerca. No mezclarlos: despawnear
	 * una entidad al alejarse no debe sacarla de la TAB, y estar en
	 * la TAB no debe spawnear la entidad.
	 *
	 * @var array<string, array{name:string,gamemode:int,ping:int}>
	 */
	private array $tabListed = [];

	/** @param resource $stream */
	public function __construct(
		$stream,
		private string $peer
	){
		$this->stream = $stream;
		$this->lastSeen = microtime(true);
		$this->lastKeepAlive = microtime(true);
	}

	public function getPeer() : string{
		return $this->peer;
	}

	public function getUsername() : ?string{
		return $this->username;
	}

	public function getProtocol() : int{
		return $this->protocol;
	}

	public function isModern() : bool{
		return $this->profile !== null;
	}

	public function isPlaying() : bool{
		return $this->playing && !$this->closed;
	}

	public function isClosed() : bool{
		return $this->closed;
	}

	/** Posición canónica Bedrock (sin el +64 que sólo llevan los paquetes legacy). */
	public function getX() : float{ return $this->x; }
	public function getY() : float{ return $this->y; }
	public function getZ() : float{ return $this->z; }
	public function getYaw() : float{ return $this->yaw; }
	public function getPitch() : float{ return $this->pitch; }

	/** Gamemode que cree el cliente Java (0 survival, 1 creative, 2 adventure, 3 spectator). */
	public function getGamemode() : int{ return $this->gamemode; }

	/** Última posición compartida cliente<=>servidor `[x,y,z,yaw,pitch]`; null antes del primer push. */
	public function getLastSentPos() : ?array{ return $this->lastSentPos; }

	/**
	 * Olvida todo el streaming de chunks de esta sesión: el cliente entra al
	 * mundo sin columnas cargadas y hay que volver a enviárselas todas
	 * (incluido el chunk del jugador, que va primero por distancia).
	 */
	private function resetChunkStreaming() : void{
		$this->sentChunks = [];
		$this->pendingChunks = [];
		$this->pendingAttempts = [];
		$this->lastChunkX = PHP_INT_MIN;
		$this->lastChunkZ = PHP_INT_MIN;
		$this->viewAnnounced = false;
		$this->spawnChunksAt = microtime(true);
		$this->spawnChunksLogged = false;
	}

	/** Puerta `hasReceivedChunk` de `Entity::spawnTo`: el chunk ya está en el cliente. */
	public function hasSentChunk(int $chunkX, int $chunkZ) : bool{
		return isset($this->sentChunks[$chunkX . ':' . $chunkZ]);
	}

	/** @return array<string, array{entity:int,x:float,y:float,z:float,yaw:float,pitch:float}> */
	public function getSeenEntities() : array{
		return $this->seenEntities;
	}

	/** @return array<string, array{name:string,gamemode:int,ping:int}> jugadores en la TAB de este cliente */
	public function getTabListed() : array{
		return $this->tabListed;
	}

	public function setOwnEntityId(int $id) : void{
		$this->ownEntityId = $id;
	}

	/**
	 * Lee del socket y procesa los frames completos.
	 *
	 * @return bool false cuando la conexión debe cerrarse
	 */
	public function poll(JavaServer $server) : bool{
		if($this->closed || $this->stream === null){
			return false;
		}

		$data = @fread($this->stream, 65536);
		if($data === false || ($data === '' && @feof($this->stream))){
			return false;
		}
		if($data !== ''){
			$this->in .= $data;
			$this->lastSeen = microtime(true);
		}

		while(!$this->closed){
			$offset = 0;
			$length = Protocol::readVarInt($this->in, $offset);
			if($length === null){
				break;
			}
			if($length <= 0 || $length > Protocol::MAX_FRAME_LENGTH){
				$this->kick("Frame inválido");
				break;
			}
			if(strlen($this->in) - $offset < $length){
				break;
			}

			$body = substr($this->in, $offset, $length);
			$this->in = (string)substr($this->in, $offset + $length);

			$bodyOffset = 0;
			$packetId = Protocol::readVarInt($body, $bodyOffset);
			if($packetId === null){
				break;
			}
			$this->handlePacket($server, $packetId, substr($body, $bodyOffset));
		}

		$this->flush();
		return !$this->closed;
	}

	/** Trabajo periódico: timeouts, keep-alive, chunks y vaciado del buffer. */
	public function tick(JavaServer $server, ?float $deadline = null) : void{
		if($this->closed){
			return;
		}
		$now = microtime(true);
		if($now - $this->lastSeen > self::TIMEOUT_SECONDS){
			$this->kick("Tiempo de espera agotado");
			return;
		}

		if($this->playing){
			$this->tickCount++;
			$this->keepAlive($now);
			$server->ensureRegistered($this);
			// La posición va ANTES que los chunks: `streamChunks()` puede
			// tardar decenas de ms en generar una columna (`world.get_chunk`
			// es una host call síncrona) y `maybeUpdatePosition()` publica
			// en Rust (`player.update_position`, otra host call) lo que los
			// Bedrock ven de este jugador. Si los chunks iban primero, el
			// push de posición quedaba detrás de la generación y los
			// Bedrock veían al Java congelado/saltando durante toda la
			// carga inicial de columnas. El coste es que un push en este
			// tick puede agotar la deadline compartida y dejar los chunks
			// para el siguiente tick (1 de cada 2 ticks mientras se mueve):
			// la posición manda sobre el terreno.
			$this->maybeUpdatePosition($server);
			// Chat, avisos y feedback de comandos que los plugins dejaron
			// para este jugador con la API del servidor. La host call
			// `player.take_messages` es síncrona (pipe hacia Rust) y no
			// vale 20 Hz: cada 5 ticks el chat llega con ≤250 ms de
			// retraso a cambio de no bloquear el hilo del servidor.
			// Gamemode/Teleport van cada 10 ticks y escalonados (2 y 7):
			// con N clientes las 3 host calls del mismo tick se
			// encadenaban en un solo tick (cada una ~15 ms en Windows);
			// escalonadas, cada tick paga como mucho una de ellas. La
			// copia PHP `apiPlayer` aún se comprueba en memoria cada
			// llamada, así que los cambios desde plugins siguen saliendo
			// al primer intento (el `player.get` al host solo se paga
			// cuando esa vía no detectó nada).
			if(($this->tickCount % 5) === 1){
				$server->deliverPluginMessages($this);
			}
			if(($this->tickCount % 10) === 2){
				// Gamemode: detecta cambios desde consola/Bedrock/plugins
				// (Rust actualiza el registry) y notifica al JavaClient.
				$server->syncGamemode($this);
			}
			if(($this->tickCount % 10) === 7){
				// Teleport: mismo patrón para la posición (consola `/tp`
				// sobre un jugador virtual o plugin): el registry/apiPlayer
				// cambió, se teleporta el cliente con el paquete position.
				// También antes de los chunks: el paquete `position` debe
				// salir del `out` antes que los datos de columnas.
				$server->syncTeleport($this);
			}
			$this->streamChunks($server, $deadline);
		}

		$this->flush();
	}

	/**
	 * `player.update_position` throttled: cada ~2 ticks o al cambiar de chunk.
	 *
	 * El movimiento llega en paquetes de movimiento; no se puede hacer una
	 * host call por paquete (20 Hz por jugador saturaría el IPC síncrono).
	 * Se guarda la última posición enviada y sólo se reporta si se movió y
	 * pasó el intervalo (~100 ms, invisible para la interpolación Bedrock)
	 * o cruzó de columna. Con 20 ticks (~1 s) los Bedrock veían al Java
	 * saltar una vez por segundo o congelado en tramos cortos.
	 */
	private function maybeUpdatePosition(JavaServer $server) : void{
		$current = [$this->x, $this->y, $this->z, $this->yaw, $this->pitch];
		$last = $this->lastSentPos;
		if($last === null){
			$server->updatePlayerPosition($this);
			$this->lastSentPos = $current;
			$this->lastPosUpdateTick = $this->tickCount;
			return;
		}
		if($last === $current){
			// Heartbeat when standing still: lets /list keep counting the
			// player in Rust and heals a deleted registry entry within ~5 s.
			if(($this->tickCount - $this->lastPosUpdateTick) >= 100){
				// Un teleport del servidor pendiente (registry/apiPlayer
				// ya apuntan al destino) no se sobrescribe con la posición
				// vieja: `syncTeleport` (cada 10 ticks) mueve al cliente.
				if($server->hasTeleportPending($this)){
					return;
				}
				$server->updatePlayerPosition($this);
				$this->lastPosUpdateTick = $this->tickCount;
			}
			return;
		}
		$chunkChanged = ((int)floor($last[0] / 16) !== (int)floor($current[0] / 16))
			|| ((int)floor($last[2] / 16) !== (int)floor($current[2] / 16));
		if($chunkChanged || ($this->tickCount - $this->lastPosUpdateTick) >= 2){
			if($server->hasTeleportPending($this)){
				return;
			}
			$server->updatePlayerPosition($this);
			$this->lastSentPos = $current;
			$this->lastPosUpdateTick = $this->tickCount;
		}
	}

	private function handlePacket(JavaServer $server, int $packetId, string $payload) : void{
		switch($this->state){
			case self::STATE_HANDSHAKE:
				$this->handleHandshake($server, $packetId, $payload);
				break;

			case self::STATE_STATUS:
				$this->handleStatus($server, $packetId, $payload);
				break;

			case self::STATE_LOGIN:
				$this->handleLogin($server, $packetId, $payload);
				break;

			case self::STATE_CONFIG:
				$this->handleConfiguration($server, $packetId);
				break;

			case self::STATE_PLAY:
				$this->handlePlay($server, $packetId, $payload);
				break;
		}
	}

	private function handleHandshake(JavaServer $server, int $packetId, string $payload) : void{
		if($packetId !== Protocol::C2S_HANDSHAKE){
			$this->kick("Se esperaba un handshake");
			return;
		}

		$offset = 0;
		$protocol = Protocol::readVarInt($payload, $offset);
		$address = Protocol::readString($payload, $offset);
		$port = Protocol::readUInt16($payload, $offset);
		$nextState = Protocol::readVarInt($payload, $offset);
		if($protocol === null || $address === null || $port === null || $nextState === null){
			$this->kick("Handshake inválido");
			return;
		}

		$this->protocol = $protocol;
		$server->log("Handshake de {$this->peer}: protocolo {$protocol}, destino {$address}:{$port}, estado {$nextState}");

		if($nextState === 1){
			$this->state = self::STATE_STATUS;
			return;
		}
		if($nextState !== 2){
			$this->kick("Estado de handshake desconocido");
			return;
		}

		$modern = ModernProfile::forProtocol($protocol);
		if($modern !== null){
			$this->profile = $modern;
			$this->state = self::STATE_LOGIN;
			return;
		}
		if($protocol === Protocol::PROTOCOL_VERSION){
			$this->state = self::STATE_LOGIN;
			return;
		}

		$this->send(Protocol::S2C_LOGIN_DISCONNECT, Protocol::string(Protocol::chatJson(
			"Versiones soportadas: Java " . Protocol::VERSION_NAME . " (protocolo " . Protocol::PROTOCOL_VERSION .
			") y " . implode(', ', ModernProfile::VERSION_NAMES) .
			". Tu cliente usa el protocolo " . $protocol . "."
		)));
		$this->flush();
		$this->closed = true;
	}

	private function handleStatus(JavaServer $server, int $packetId, string $payload) : void{
		if($packetId === Protocol::C2S_STATUS_REQUEST){
			$this->send(Protocol::S2C_STATUS_RESPONSE, Protocol::string($server->statusJson($this->protocol)));
			$this->flush();
			return;
		}

		if($packetId === Protocol::C2S_STATUS_PING){
			$offset = 0;
			$time = Protocol::readLong($payload, $offset);
			$this->send(Protocol::S2C_STATUS_PONG, Protocol::long($time ?? 0));
			$this->flush();
			$this->closed = true;
		}
	}

	private function handleLogin(JavaServer $server, int $packetId, string $payload) : void{
		if($this->profile !== null){
			$this->handleModernLogin($server, $packetId, $payload);
			return;
		}

		if($packetId !== Protocol::C2S_LOGIN_START){
			$this->kick("Se esperaba login_start");
			return;
		}

		$offset = 0;
		$requested = Protocol::readString($payload, $offset);
		$name = $requested === null ? '' : trim($requested);
		if(!$this->validName($name)){
			$this->send(Protocol::S2C_LOGIN_DISCONNECT, Protocol::string(Protocol::chatJson("Nombre de jugador inválido.")));
			$this->flush();
			$this->closed = true;
			return;
		}
		if(!$server->registerPlayer($this, $name)){
			$this->send(Protocol::S2C_LOGIN_DISCONNECT, Protocol::string(Protocol::chatJson("Ese nombre ya está conectado.")));
			$this->flush();
			$this->closed = true;
			return;
		}

		$this->username = $name;
		$uuid = Protocol::offlineUuid($name);
		$this->send(Protocol::S2C_LOGIN_SUCCESS, Protocol::string(Protocol::uuidString($uuid)) . Protocol::string($name));
		$this->enterPlay($server);
	}

	private function handleModernLogin(JavaServer $server, int $packetId, string $payload) : void{
		$profile = $this->profile;
		if($profile === null){
			return;
		}

		// El cliente confirma la entrada antes de pasar a `configuration`.
		if($packetId === $profile->id('login_acknowledged')){
			$this->beginConfiguration();
			return;
		}

		if($packetId !== Protocol::C2S_LOGIN_START){
			$this->kick("Se esperaba login_start");
			return;
		}

		$offset = 0;
		$requested = Protocol::readString($payload, $offset);
		$name = $requested === null ? '' : trim($requested);
		if(!$this->validName($name)){
			$this->send(Protocol::S2C_LOGIN_DISCONNECT, Protocol::string(Protocol::chatJson("Nombre de jugador inválido.")));
			$this->flush();
			$this->closed = true;
			return;
		}
		if(!$server->registerPlayer($this, $name)){
			$this->send(Protocol::S2C_LOGIN_DISCONNECT, Protocol::string(Protocol::chatJson("Ese nombre ya está conectado.")));
			$this->flush();
			$this->closed = true;
			return;
		}

		$this->username = $name;
		// 1.20.2+ : UUID binario (16 bytes) + nombre + propiedades.
		$this->send($profile->id('login_success'),
			Protocol::offlineUuid($name) . Protocol::string($name) . Protocol::varInt(0)
		);
		$this->flush();
	}

	/** Envía los registros y cierra la fase de configuración. */
	private function beginConfiguration() : void{
		$profile = $this->profile;
		if($profile === null){
			return;
		}

		$this->state = self::STATE_CONFIG;

		try{
			foreach($profile->registryPayloads() as $payload){
				$this->send($profile->id('registry_data'), $payload);
			}
		}catch(\Throwable $e){
			$this->kick("No se pudieron enviar los registros de Java: " . $e->getMessage());
			return;
		}

		$this->send($profile->id('feature_flags'), Protocol::varInt(1) . Protocol::string('minecraft:vanilla'));
		// El cliente resuelve tags con getOrThrow al construir los componentes de
		// los items; sin este paquete aborta con "Missing tag" en
		// finish_configuration.
		try{
			$this->send($profile->id('update_tags'), $profile->tagsPayload());
		}catch(\Throwable $e){
			$this->kick("No se pudieron enviar los tags de Java: " . $e->getMessage());
			return;
		}
		// Marca del servidor para F3 (`Server:`): custom_payload clientbound
		// de configuration en el canal `minecraft:brand`. Sin esto el cliente
		// muestra `null` aunque el login sea correcto.
		$this->send($profile->id('config_brand'), $profile->brandPayload());
		$this->send($profile->id('finish_configuration'), '');
		$this->flush();
	}

	private function handleConfiguration(JavaServer $server, int $packetId) : void{
		$profile = $this->profile;
		if($profile === null){
			return;
		}
		// settings, brand y select_known_packs no necesitan respuesta.
		if($packetId === $profile->id('config_finish')){
			$this->enterPlayModern($server);
		}
	}

	private function enterPlay(JavaServer $server) : void{
		$this->state = self::STATE_PLAY;
		$this->playing = true;

		$spawn = $server->spawn();
		// Posición canónica Bedrock; el +64 vive sólo en los paquetes legacy
		// (ver sendPosition). Si hay datos guardados se reaparece ahí.
		$loaded = $server->loadPlayerData((string)$this->username);
		$this->x = isset($loaded['x']) ? (float)$loaded['x'] : $spawn[0];
		$this->y = isset($loaded['y']) ? (float)$loaded['y'] : $spawn[1];
		$this->z = isset($loaded['z']) ? (float)$loaded['z'] : $spawn[2];
		// El save guarda el yaw/pitch Bedrock (canónico en Rust); el
		// cliente Java trabaja en yaw Java (`$this->yaw` es Java).
		if(isset($loaded['yaw'])){ $this->yaw = Protocol::bedrockYawToJava((float)$loaded['yaw']); }
		if(isset($loaded['pitch'])){ $this->pitch = Protocol::bedrockPitchToJava((float)$loaded['pitch']); }

		$gamemode = isset($loaded['gamemode']) ? (int)$loaded['gamemode'] : $server->gamemode();
		$this->gamemode = max(0, min(3, $gamemode));
		$health = isset($loaded['health']) ? (float)$loaded['health'] : 20.0;
		$difficulty = $server->difficulty();

		$this->send(Protocol::S2C_JOIN_GAME,
			pack('N', $this->entityId($server)) .
			Protocol::ubyte($gamemode) .
			pack('N', 0) . // dimensión: overworld
			Protocol::ubyte($difficulty) .
			Protocol::ubyte(max(1, min(255, $server->maxPlayers()))) .
			Protocol::string($server->levelType()) .
			Protocol::boolean(false)
		);
		$this->send(Protocol::S2C_DIFFICULTY, Protocol::ubyte($difficulty));
		$this->send(Protocol::S2C_SPAWN_POSITION, Protocol::position((int)floor($spawn[0]), (int)floor($spawn[1]) + 64, (int)floor($spawn[2])));
		$this->send(Protocol::S2C_ABILITIES, chr($this->abilityFlags($gamemode)) . Protocol::float(0.05) . Protocol::float(0.1));
		$this->send(Protocol::S2C_UPDATE_HEALTH, Protocol::float($health) . Protocol::varInt((int)round($health)) . Protocol::float(5.0));
		$this->sendTime($server->worldTime(), $server->worldTimeRunning());
		// En 1.12.2 el cliente no conoce su posición hasta este paquete.
		$this->sendPosition(true);
		// Alta en la API de plugins antes de la TAB y del conteo, para que
		// el jugador se cuente a sí mismo y los plugins ya lo vean.
		$server->joinApi($this);
		// TAB global: TODOS los online (Bedrock + Java, incluido él mismo),
		// no solo él. `syncTab()` deja el estado en `$tabListed` para que
		// el tick no lo reenvíe y para que el spawn de entidades no
		// duplique el `player_info` add.
		$this->syncTab($server->tabTargets());
		$this->sendChat("Conectado a PocketMine-Rust. " . $server->onlineCount() . " jugador(es) online.");

		// Entrar al mundo = el cliente no tiene ningún chunk cargado: hay que
		// forzar el reenvío de toda el área. Si `sentChunks` conservaba
		// columnas de una entrada anterior en la misma sesión TCP (doble
		// `config_finish`, re-join), `requestChunks()` las excluiría de
		// `pendingChunks` y el cliente nunca recibiría la columna nueva
		// (p. ej. con `fluidCount` real en 26.1: se hundiría en el agua).
		$this->resetChunkStreaming();
	}

	/**
	 * Entrada al mundo en versiones modernas: `login` (join game) con el
	 * SpawnInfo, posición absoluta y aviso de fin de carga de chunks.
	 */
	private function enterPlayModern(JavaServer $server) : void{
		$profile = $this->profile;
		if($profile === null){
			return;
		}

		$this->state = self::STATE_PLAY;
		$this->playing = true;

		// Mismo punto exacto que el cliente Bedrock (`player_position()`), para
		// que ambos aparezcan en el mismo sitio: ni medio bloque desplazado ni
		// 1,62 bloques más arriba. Con datos guardados se reaparece ahí.
		$spawn = $server->spawn();
		$loaded = $server->loadPlayerData((string)$this->username);
		$this->x = isset($loaded['x']) ? (float)$loaded['x'] : $spawn[0];
		$this->y = isset($loaded['y']) ? (float)$loaded['y'] : $spawn[1];
		$this->z = isset($loaded['z']) ? (float)$loaded['z'] : $spawn[2];
		// Save en yaw/pitch Bedrock, cliente en yaw/pitch Java (ver
		// `enterPlay()` legacy: misma conversión).
		if(isset($loaded['yaw'])){ $this->yaw = Protocol::bedrockYawToJava((float)$loaded['yaw']); }
		if(isset($loaded['pitch'])){ $this->pitch = Protocol::bedrockPitchToJava((float)$loaded['pitch']); }

		$gamemode = isset($loaded['gamemode']) ? (int)$loaded['gamemode'] : $server->gamemode();
		$this->gamemode = max(0, min(3, $gamemode));
		$viewDistance = max(2, min(32, $server->viewDistance()));
		$maxPlayers = max(1, min(255, $server->maxPlayers()));

		$this->send($profile->id('join_game'),
			pack('N', $this->entityId($server)) .
			Protocol::boolean(false) . // hardcore
			Protocol::varInt(1) . Protocol::string(self::WORLD_NAME) . // worldNames
			Protocol::varInt($maxPlayers) .
			Protocol::varInt($viewDistance) .
			Protocol::varInt($viewDistance) .
			Protocol::boolean(false) . // reduced debug info
			Protocol::boolean(true) . // enable respawn screen
			Protocol::boolean(false) . // limited crafting
			// SpawnInfo
			Protocol::varInt(0) . // dimension type (minecraft:overworld)
			Protocol::string(self::WORLD_NAME) .
			Protocol::long(0) . // hashed seed
			chr($gamemode & 0xFF) . // gamemode (i8)
			"\xFF" . // previous gamemode (-1)
			Protocol::boolean(false) . // isDebug
			Protocol::boolean(true) . // isFlat
			Protocol::boolean(false) . // has death location
			Protocol::varInt(0) . // portal cooldown
			Protocol::varInt(self::SEA_LEVEL) .
			Protocol::boolean(false) // enforces secure chat
		);
		// Hora del mundo compartido: sin esto el cliente moderno
		// entraba siempre a su hora por defecto (nunca se enviaba
		// `update_time` al entrar, a diferencia del legacy).
		$this->sendTime($server->worldTime(), $server->worldTimeRunning());
		$this->sendPosition(true);
		// 13 = level_chunks_load_start: quita la pantalla de carga.
		$this->send($profile->id('game_state_change'), chr(13) . Protocol::float(0.0));
		$server->joinApi($this);
		// TAB global (moderno): `player_info_update` add+listed con
		// gamemode y latency para TODOS los online, incluido él mismo.
		// Antes no se enviaba nada al entrar: la TAB solo se llenaba
		// con los que entraban en rango de chunks vía `syncEntities`.
		$this->syncTab($server->tabTargets());
		$this->sendChat("Conectado a PocketMine-Rust. " . $server->onlineCount() . " jugador(es) online.");

		// Entrar al mundo = el cliente no tiene ningún chunk cargado: hay que
		// forzar el reenvío de toda el área. Si `sentChunks` conservaba
		// columnas de una entrada anterior en la misma sesión TCP (doble
		// `config_finish`, re-join), `requestChunks()` las excluiría de
		// `pendingChunks` y el cliente nunca recibiría la columna nueva
		// (p. ej. con `fluidCount` real en 26.1: se hundiría en el agua).
		$this->resetChunkStreaming();
	}

	private function handlePlay(JavaServer $server, int $packetId, string $payload) : void{
		if($this->profile !== null){
			$this->handleModernPlay($server, $packetId, $payload);
			return;
		}

		switch($packetId){
			case Protocol::C2S_KEEP_ALIVE:
			case Protocol::C2S_TELEPORT_CONFIRM:
			case Protocol::C2S_FLYING:
			case Protocol::C2S_ARM_ANIMATION:
				return;

			case Protocol::C2S_CHAT:
				$offset = 0;
				$message = Protocol::readString($payload, $offset);
				if($message !== null && $message !== ''){
					$server->handleChat($this, $message);
				}
				return;

			case Protocol::C2S_POSITION:
				$offset = 0;
				$x = Protocol::readDouble($payload, $offset);
				$y = Protocol::readDouble($payload, $offset);
				$z = Protocol::readDouble($payload, $offset);
				if($x !== null && $y !== null && $z !== null){
					$this->x = $x;
					// El cliente legacy vive en Y+64; en memoria va canónico.
					$this->y = $y - 64.0;
					$this->z = $z;
					$this->requestChunks($server);
				}
				return;

			case Protocol::C2S_LOOK:
				$offset = 0;
				$yaw = Protocol::readFloat($payload, $offset);
				$pitch = Protocol::readFloat($payload, $offset);
				if($yaw !== null && $pitch !== null){
					$this->yaw = $yaw;
					$this->pitch = $pitch;
					$this->requestChunks($server);
				}
				return;

			case Protocol::C2S_POSITION_LOOK:
				$offset = 0;
				$x = Protocol::readDouble($payload, $offset);
				$y = Protocol::readDouble($payload, $offset);
				$z = Protocol::readDouble($payload, $offset);
				$yaw = Protocol::readFloat($payload, $offset);
				$pitch = Protocol::readFloat($payload, $offset);
				if($x !== null && $y !== null && $z !== null && $yaw !== null && $pitch !== null){
					$this->x = $x;
					$this->y = $y - 64.0;
					$this->z = $z;
					$this->yaw = $yaw;
					$this->pitch = $pitch;
					$this->requestChunks($server);
				}
				return;

			case Protocol::C2S_BLOCK_DIG:
				$this->handleBlockDig($server, $payload);
				return;

			case Protocol::C2S_SETTINGS:
				$offset = 0;
				$locale = Protocol::readString($payload, $offset);
				$server->log("Cliente Java {$this->peer} configurado (locale " . ($locale ?? '?') . ")");
				return;

			case Protocol::C2S_BLOCK_PLACE:
				$this->handleBlockPlace($server, $payload);
				return;

			case Protocol::C2S_CLIENT_COMMAND:
				$offset = 0;
				$action = Protocol::readVarInt($payload, $offset);
				if($action === 0){
					$this->teleport($server, $this->x, $this->y, $this->z);
				}
				return;
		}
	}

	/** Paquetes de la fase `play` en 1.20.2+ (ids por versión). */
	private function handleModernPlay(JavaServer $server, int $packetId, string $payload) : void{
		$profile = $this->profile;
		if($profile === null){
			return;
		}

		if($packetId === $profile->id('chat_message')){
			$offset = 0;
			$message = Protocol::readString($payload, $offset);
			if($message !== null && $message !== ''){
				$server->handleChat($this, $message);
			}
			return;
		}

		if($packetId === $profile->id('chat_command')){
			$offset = 0;
			$command = Protocol::readString($payload, $offset);
			if($command !== null && $command !== ''){
				$server->handleChat($this, '/' . $command);
			}
			return;
		}

		if($packetId === $profile->id('sb_position')){
			$offset = 0;
			$x = Protocol::readDouble($payload, $offset);
			$y = Protocol::readDouble($payload, $offset);
			$z = Protocol::readDouble($payload, $offset);
			if($x !== null && $y !== null && $z !== null){
				$this->x = $x;
				$this->y = $y;
				$this->z = $z;
				$this->requestChunks($server);
			}
			return;
		}

		if($packetId === $profile->id('sb_position_look')){
			$offset = 0;
			$x = Protocol::readDouble($payload, $offset);
			$y = Protocol::readDouble($payload, $offset);
			$z = Protocol::readDouble($payload, $offset);
			$yaw = Protocol::readFloat($payload, $offset);
			$pitch = Protocol::readFloat($payload, $offset);
			if($x !== null && $y !== null && $z !== null && $yaw !== null && $pitch !== null){
				$this->x = $x;
				$this->y = $y;
				$this->z = $z;
				$this->yaw = $yaw;
				$this->pitch = $pitch;
				$this->requestChunks($server);
			}
			return;
		}

		if($packetId === $profile->id('sb_look')){
			$offset = 0;
			$yaw = Protocol::readFloat($payload, $offset);
			$pitch = Protocol::readFloat($payload, $offset);
			if($yaw !== null && $pitch !== null){
				$this->yaw = $yaw;
				$this->pitch = $pitch;
				$this->requestChunks($server);
			}
			return;
		}

		if($packetId === $profile->id('chunk_batch_received')){
			$offset = 0;
			$rate = Protocol::readFloat($payload, $offset);
			if($rate !== null && $rate > 0.0){
				$this->chunksPerTick = $rate;
			}
			return;
		}

		if($packetId === $profile->id('client_command')){
			$offset = 0;
			$action = Protocol::readVarInt($payload, $offset);
			if($action === 0){
				$spawn = $server->spawn();
				$this->teleport($server, $spawn[0], $spawn[1], $spawn[2]);
			}
			return;
		}

		if($packetId === $profile->id('block_dig')){
			$this->handleBlockDig($server, $payload);
			return;
		}

		if($packetId === $profile->id('block_place')){
			$this->handleBlockPlace($server, $payload);
			return;
		}

		// keep_alive, teleport_confirm, chunk_batch_received, player_loaded,
		// settings... no necesitan respuesta.
	}

	private function handleBlockDig(JavaServer $server, string $payload) : void{
		$offset = 0;
		$status = Protocol::readVarInt($payload, $offset);
		$position = Protocol::readPosition($payload, $offset);
		if($status === null || $position === null){
			return;
		}
		// 0 = empezó a picar, 2 = terminó. Sólo el final modifica el mundo.
		if($status !== 2){
			return;
		}
		[$x, $y, $z] = $position;
		// El cliente legacy vive en Y+64: el mundo Bedrock es Y-64.
		$canonicalY = $this->profile === null ? $y - 64 : $y;
		if(abs($x + 0.5 - $this->x) > 6 || abs($canonicalY + 0.5 - $this->y) > 6 || abs($z + 0.5 - $this->z) > 6){
			return;
		}

		if($server->breakBlock($x, $canonicalY, $z)){
			$server->broadcastBlockChange($x, $canonicalY, $z, 'air');
		}else{
			// Bloque ya era aire o el mundo rechazó el cambio: se reenvía el
			// estado real para deshacer la predicción del cliente.
			$server->broadcastBlockChange($x, $canonicalY, $z, '');
		}
	}

	/**
	 * Fase 2 (Java -> Bedrock): colocar un bloque contra una cara.
	 *
	 * Moderno (`block_place`, `use_item_on`): mano (varint) + posición +
	 * cara (varint) + cursor (f32 x3) + dentro del bloque (bool) +
	 * secuencia (varint). El paquete NO trae el item en la mano y el
	 * puente no sincroniza inventario, así que en creativo se coloca
	 * piedra por defecto (TODO: inventario/hotbar del cliente).
	 *
	 * Legacy 1.12.2 (`C2S_BLOCK_PLACE`, 0x1F): posición + cara (u8) +
	 * held item (slot: id i16 + resto) + cursor (u8 x3). Aquí el id del
	 * slot sí es el bloque a colocar (legacy Java 1.12 = legacy PM).
	 *
	 * Por ahora solo creativo (sin inventario que descontar): en otro
	 * modo se avisa por chat. La celda (posición + desplazamiento de la
	 * cara) la valida `JavaServer::placeBlock()` (alcance y vacío) y,
	 * si se coloca, ella misma avisa a los Java; si no, se reenvía el
	 * estado real para deshacer la predicción del cliente.
	 */
	private function handleBlockPlace(JavaServer $server, string $payload) : void{
		if($this->gamemode !== 1){
			$this->sendChat("Colocar bloques desde Java solo está disponible en modo creativo por ahora (sin inventario). Usa /gamemode creative.");
			return;
		}

		$profile = $this->profile;
		$offset = 0;
		if($profile !== null){
			$hand = Protocol::readVarInt($payload, $offset);
			$position = Protocol::readPosition($payload, $offset);
			$face = Protocol::readVarInt($payload, $offset);
			if($hand === null || $position === null || $face === null){
				return;
			}
			// Sin item en el paquete: piedra en creativo (ver comentario).
			$blockId = ModernChunk::bedrockNetworkIdForJavaState(ModernChunk::STONE, $profile)
				?? ModernChunk::bedrockNetworkIdForName('stone')
				?? 2150698529;
		}else{
			$position = Protocol::readPosition($payload, $offset);
			if($position === null || strlen($payload) - $offset < 3){
				return;
			}
			$face = ord($payload[$offset]); // u8
			$offset++;
			if(strlen($payload) - $offset < 2){
				return;
			}
			// Held item: i16 con signo (-1 = mano vacía).
			$itemId = unpack('n', substr($payload, $offset, 2))[1];
			if($itemId >= 32768){
				$itemId -= 65536;
			}
			if($itemId <= 0){
				$this->sendChat("No tienes ningún bloque en la mano para colocar.");
				return;
			}
			$blockId = $itemId;
		}

		$target = self::placeTarget($position, $face);
		if($target === null){
			return;
		}
		[$tx, $ty, $tz] = $target;
		// El cliente legacy vive en Y+64: la celda sale en Y legacy y se
		// convierte a Y canónica Bedrock restando 64 una sola vez.
		if($profile === null){
			$ty -= 64;
		}

		if(!$server->placeBlock($this, $tx, $ty, $tz, $blockId)){
			// Fuera de alcance, celda ocupada o el mundo rechazó: estado
			// real para deshacer la predicción del cliente.
			$server->broadcastBlockChange($tx, $ty, $tz, '');
		}
	}

	/**
	 * Celda donde va el bloque: posición clicada + desplazamiento de la
	 * cara (0 abajo, 1 arriba, 2 norte, 3 sur, 4 oeste, 5 este).
	 *
	 * @param array{0:int,1:int,2:int} $position
	 * @return ?array{0:int,1:int,2:int}
	 */
	private static function placeTarget(array $position, int $face) : ?array{
		$offsets = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];
		$offset = $offsets[$face] ?? null;
		if($offset === null){
			return null;
		}
		return [$position[0] + $offset[0], $position[1] + $offset[1], $position[2] + $offset[2]];
	}

	private function validName(string $name) : bool{
		return $name !== '' && strlen($name) <= 16 && preg_match('/^[A-Za-z0-9_]+$/', $name) === 1;
	}

	private function entityId(JavaServer $server) : int{
		if($this->ownEntityId !== null){
			return $this->ownEntityId;
		}
		if($this->username !== null){
			$this->ownEntityId = $server->entityIdFor($this->username);
			return $this->ownEntityId;
		}
		return 1000 + (crc32((string)$this->peer) % 100000);
	}

	public function getEntityId(JavaServer $server) : int{
		return $this->entityId($server);
	}

	private function abilityFlags(int $gamemode) : int{
		return match($gamemode){
			1 => 0x01 | 0x04 | 0x08,
			3 => 0x01 | 0x04,
			default => 0
		};
	}

	/**
	 * Sincroniza la lista TAB (player list) con todos los online.
	 *
	 * Moderno (769-775): `player_info_update` con
	 * add_player(0x01)+update_game_mode(0x04)+update_listed(0x08)+
	 * update_latency(0x10) para los nuevos, `player_remove` para los
	 * que salieron y update_game_mode para los que cambiaron de modo.
	 * Legacy 1.12.2: `S2C_PLAYER_INFO` acción 0 (add, con gamemode y
	 * ping), acción 4 (remove) y acción 3 (update_gamemode).
	 *
	 * Se llama al entrar a play (lista completa, incluido él mismo) y
	 * cada tick desde `JavaServer::tickTab()` (Bedrock o Java que
	 * entra/sale, sin puerta de chunks). Sin cambios no se envía nada.
	 *
	 * @param array<string, array{name:string,gamemode:int,ping:int}> $targets lower => datos
	 */
	public function syncTab(array $targets) : void{
		if(!$this->playing || $this->closed){
			return;
		}

		// Normaliza las claves a lower-case por nombre: el llamador
		// promete `lower => datos`, pero si una clave viniera con
		// mayúsculas el diff de salidas/entradas no coincidiría y
		// el jugador se re-anunciaría en cada tick.
		$normalized = [];
		foreach($targets as $target){
			$name = (string)$target['name'];
			$normalized[strtolower($name)] = [
				'name' => $name,
				'gamemode' => max(0, min(3, (int)($target['gamemode'] ?? 0))),
				'ping' => max(0, (int)($target['ping'] ?? 0)),
			];
		}
		$targets = $normalized;

		// Salidas: ya no están en la lista deseada.
		$removed = [];
		foreach(array_keys($this->tabListed) as $lower){
			if(!array_key_exists($lower, $targets)){
				$removed[] = $this->tabListed[$lower];
				unset($this->tabListed[$lower]);
			}
		}
		if($removed !== []){
			$profile = $this->profile;
			if($profile !== null){
				$payload = Protocol::varInt(count($removed));
				foreach($removed as $entry){
					$payload .= Protocol::offlineUuid($entry['name']);
				}
				$this->send($profile->id('player_remove'), $payload);
			}else{
				$payload = Protocol::varInt(4) . Protocol::varInt(count($removed));
				foreach($removed as $entry){
					$payload .= Protocol::offlineUuid($entry['name']);
				}
				$this->send(Protocol::S2C_PLAYER_INFO, $payload);
			}
		}

		// Entradas y cambios de gamemode.
		$added = [];
		$gamemodeChanged = [];
		foreach($targets as $lower => $target){
			$name = (string)$target['name'];
			$gamemode = max(0, min(3, (int)($target['gamemode'] ?? 0)));
			$ping = max(0, (int)($target['ping'] ?? 0));
			if(!isset($this->tabListed[$lower])){
				$added[$lower] = ['name' => $name, 'gamemode' => $gamemode, 'ping' => $ping];
				continue;
			}
			if((int)$this->tabListed[$lower]['gamemode'] !== $gamemode){
				$gamemodeChanged[] = ['name' => $name, 'gamemode' => $gamemode, 'ping' => $ping];
			}
			$this->tabListed[$lower]['gamemode'] = $gamemode;
			$this->tabListed[$lower]['ping'] = $ping;
		}

		if($added !== []){
			$profile = $this->profile;
			if($profile !== null){
				$this->send($profile->id('player_info_update'), $this->modernPlayerInfoAddBatch(array_values($added)));
			}else{
				$this->send(Protocol::S2C_PLAYER_INFO, $this->legacyPlayerInfoAddBatch(array_values($added)));
			}
			foreach($added as $lower => $entry){
				$this->tabListed[$lower] = $entry;
			}
		}

		if($gamemodeChanged !== []){
			$profile = $this->profile;
			if($profile !== null){
				$this->send($profile->id('player_info_update'), $this->modernPlayerInfoGamemodeBatch($gamemodeChanged));
			}else{
				$this->send(Protocol::S2C_PLAYER_INFO, $this->legacyPlayerInfoGamemodeBatch($gamemodeChanged));
			}
		}
	}

	private function sendPosition(bool $force) : void{
		$profile = $this->profile;
		if($profile !== null){
			// 1.21.2+ : id de teleport, posición, delta, rotación y flags (u32).
			$this->teleportId++;
			$this->send($profile->id('position'),
				Protocol::varInt($this->teleportId) .
				Protocol::double($this->x) .
				Protocol::double($this->y) .
				Protocol::double($this->z) .
				Protocol::double(0.0) .
				Protocol::double(0.0) .
				Protocol::double(0.0) .
				Protocol::float($this->yaw) .
				Protocol::float($this->pitch) .
				pack('N', 0)
			);
			return;
		}

		$teleport = $force || $this->teleportId > 1;
		if($teleport){
			$this->teleportId++;
		}
		$this->send(Protocol::S2C_POSITION,
			Protocol::double($this->x) .
			Protocol::double($this->y + 64.0) .
			Protocol::double($this->z) .
			Protocol::float($this->yaw) .
			Protocol::float($this->pitch) .
			chr(0) .
			Protocol::varInt($teleport ? $this->teleportId : 0)
		);
	}

	/** Teletransporte absoluto (chat `/spawn`, `/tp` local o `syncTeleport`). */
	public function teleport(JavaServer $server, float $x, float $y, float $z, ?float $yaw = null, ?float $pitch = null) : void{
		$this->x = $x;
		// Coordenadas canónicas Bedrock; el +64 vive sólo en los paquetes
		// legacy (sendPosition).
		$this->y = $y;
		$this->z = $z;
		if($yaw !== null){
			$this->yaw = $yaw;
		}
		if($pitch !== null){
			$this->pitch = $pitch;
		}
		$this->lastChunkX = PHP_INT_MIN;
		$this->lastChunkZ = PHP_INT_MIN;
		$this->sendPosition(true);
		$this->requestChunks($server);
		// Marca el destino como última posición compartida y persíste
		// inmediato (sin throttle): si no, `syncTeleport` lo vería de nuevo
		// como divergencia y re-teleportaría cada 10 ticks, y el destino
		// tardaría hasta 20 ticks en llegar al registry de Rust.
		$this->lastSentPos = [$this->x, $this->y, $this->z, $this->yaw, $this->pitch];
		$this->lastPosUpdateTick = $this->tickCount;
		$server->updatePlayerPosition($this);
		$this->flush();
	}

	/**
	 * Envía la hora del mundo a este cliente (`update_time`).
	 *
	 * - Legacy 1.12.2: `S2C_UPDATE_TIME` (0x47) con age + time (i64+i64),
	 *   el formato que ya se mandaba al entrar.
	 * - Moderno 769-774: `update_time` (id por versión en ModernProfile)
	 *   con age i64 + time i64 + tickDayTime bool. `tickDayTime` es
	 *   `time_running` del servidor (`/time start`/`stop`): con false
	 *   el cliente congela el sol en vez de seguir avanzándolo hasta
	 *   la siguiente corrección periódica.
	 * - 26.1 (775): `update_time` cambió a age i64 + clockUpdates[]
	 *   {id varint, totalTicks varlong, partialTick f32, rate f32}.
	 *   Se manda un único reloj (id 0, el reloj del día) con
	 *   totalTicks = hora del día, partialTick 0 y rate 1/0 según
	 *   `time_running`.
	 *
	 * Antes este paquete solo salía en `enterPlay` (legacy) y nunca
	 * en `enterPlayModern`, así que un Java moderno entraba siempre
	 * a la hora por defecto del cliente y tras horas divergía de
	 * Bedrock. Ahora `JavaServer::tickTimeSync()` lo reenvía a todos
	 * los clientes en play cada segundo (y al instante si salta).
	 */
	public function sendTime(int $time, bool $tickDayTime = true) : void{
		if(!$this->playing || $this->closed){
			return;
		}
		$profile = $this->profile;
		if($profile !== null){
			if($profile->protocol >= 775){
				$payload = Protocol::long(0)
					. Protocol::varInt(1)
					. Protocol::varInt(0)
					. Protocol::varLong($time)
					. Protocol::float(0.0)
					. Protocol::float($tickDayTime ? 1.0 : 0.0);
			}else{
				$payload = Protocol::long(0)
					. Protocol::long($time)
					. Protocol::boolean($tickDayTime);
			}
			$this->send($profile->id('update_time'), $payload);
		}else{
			$this->send(Protocol::S2C_UPDATE_TIME, Protocol::long(0) . Protocol::long($time));
		}
		$this->flush();
	}

	/**
	 * Notifica al cliente Java de un cambio de gamemode y actualiza el
	 * estado conocido. Moderno (769-775): `player_info_update` con
	 * `update_game_mode` (bitflag 0x04) para el UUID propio (lista
	 * TAB) + `game_state_change` con razón `change_game_mode` (3, el
	 * que actualiza la UI del jugador local: corazones/hambre) +
	 * `abilities` (vuelo/invulnerabilidad); legacy 1.12.2:
	 * `S2C_PLAYER_INFO` acción 3 (update_gamemode) + `S2C_ABILITIES`.
	 * Sin `game_state_change` el cliente moderno sigue mostrando la
	 * UI de survival aunque Rust tenga el nuevo gamemode: solo
	 * `player_info_update` no basta para el jugador local.
	 */
	public function setGamemode(int $gamemode) : void{
		$gamemode = max(0, min(3, $gamemode));
		if($gamemode === $this->gamemode){
			return;
		}
		$this->gamemode = $gamemode;
		// La TAB ya lleva este gamemode: si no, el próximo syncTab lo
		// reenviaría como cambio duplicado.
		if($this->username !== null){
			$lower = strtolower($this->username);
			if(isset($this->tabListed[$lower])){
				$this->tabListed[$lower]['gamemode'] = $gamemode;
			}
		}
		if(!$this->playing){
			return;
		}
		$profile = $this->profile;
		if($profile !== null){
			$this->send($profile->id('player_info_update'), $this->modernPlayerInfoGamemode($gamemode));
			$this->send($profile->id('game_state_change'), chr(self::GAME_EVENT_CHANGE_GAMEMODE) . Protocol::float((float)$gamemode));
			$this->send($profile->id('abilities'), chr($this->abilityFlags($gamemode)) . Protocol::float(0.05) . Protocol::float(0.1));
		}else{
			$this->send(Protocol::S2C_PLAYER_INFO, $this->legacyPlayerInfoGamemode($gamemode));
			$this->send(Protocol::S2C_ABILITIES, chr($this->abilityFlags($gamemode)) . Protocol::float(0.05) . Protocol::float(0.1));
		}
		$this->flush();
	}

	/**
	 * `player_info_update` (moderno) con solo update_game_mode (propio u
	 * observado). Esquema verificado en `protocol.json` de 769-775
	 * (`packet_player_info`, id por versión en ModernProfile):
	 * bitflags u8 + count varInt + por entrada UUID + solo los campos
	 * activados por el bitset. Con 0x04 es únicamente gamemode varInt;
	 * no se envía add_player (name/properties) ni initialize_chat
	 * (chat_session).
	 */
	private function modernPlayerInfoGamemode(int $gamemode, ?string $name = null) : string{
		return chr(self::PLAYER_INFO_UPDATE_GAME_MODE)
			. Protocol::varInt(1)
			. Protocol::offlineUuid((string)($name ?? $this->username))
			. Protocol::varInt($gamemode);
	}

	/** `player_info` legacy (1.12.2) acción 3 (update_gamemode), 1 jugador. */
	private function legacyPlayerInfoGamemode(int $gamemode, ?string $name = null) : string{
		return Protocol::varInt(3)
			. Protocol::varInt(1)
			. Protocol::offlineUuid((string)($name ?? $this->username))
			. Protocol::varInt($gamemode);
	}

	/**
	 * `player_info_update` (moderno) en lote, solo update_game_mode.
	 *
	 * @param list<array{name:string,gamemode:int,ping:int}> $entries
	 */
	private function modernPlayerInfoGamemodeBatch(array $entries) : string{
		$payload = chr(self::PLAYER_INFO_UPDATE_GAME_MODE) . Protocol::varInt(count($entries));
		foreach($entries as $entry){
			$payload .= Protocol::offlineUuid($entry['name']) . Protocol::varInt((int)$entry['gamemode']);
		}
		return $payload;
	}

	/**
	 * `player_info` legacy en lote, acción 3 (update_gamemode).
	 *
	 * @param list<array{name:string,gamemode:int,ping:int}> $entries
	 */
	private function legacyPlayerInfoGamemodeBatch(array $entries) : string{
		$payload = Protocol::varInt(3) . Protocol::varInt(count($entries));
		foreach($entries as $entry){
			$payload .= Protocol::offlineUuid($entry['name']) . Protocol::varInt((int)$entry['gamemode']);
		}
		return $payload;
	}

	/**
	 * `player_info_update` (moderno) en lote para la TAB: add_player +
	 * update_game_mode + update_listed + update_latency (0x1D).
	 *
	 * Por entrada, en el orden de los bitflags: UUID, game_profile
	 * (nombre + 0 propiedades), gamemode, listed=1, latency=ping. Sin
	 * initialize_chat (0x02) ni display_name (0x20).
	 *
	 * @param list<array{name:string,gamemode:int,ping:int}> $entries
	 */
	private function modernPlayerInfoAddBatch(array $entries) : string{
		$flags = self::PLAYER_INFO_ADD_PLAYER | self::PLAYER_INFO_UPDATE_GAME_MODE
			| self::PLAYER_INFO_UPDATE_LISTED | self::PLAYER_INFO_UPDATE_LATENCY;
		$payload = chr($flags) . Protocol::varInt(count($entries));
		foreach($entries as $entry){
			$payload .= Protocol::offlineUuid($entry['name'])
				. Protocol::string($entry['name']) . Protocol::varInt(0)
				. Protocol::varInt(max(0, min(3, (int)$entry['gamemode'])))
				. Protocol::varInt(1) // listed
				. Protocol::varInt(max(0, (int)($entry['ping'] ?? 0)));
		}
		return $payload;
	}

	/**
	 * `player_info` legacy (1.12.2) en lote, acción 0 (add_player):
	 * nombre, propiedades, gamemode, ping y displayName.
	 *
	 * @param list<array{name:string,gamemode:int,ping:int}> $entries
	 */
	private function legacyPlayerInfoAddBatch(array $entries) : string{
		$payload = Protocol::varInt(0) . Protocol::varInt(count($entries));
		foreach($entries as $entry){
			$payload .= Protocol::offlineUuid($entry['name'])
				. Protocol::string($entry['name']) . Protocol::varInt(0)
				. Protocol::varInt(max(0, min(3, (int)$entry['gamemode'])))
				. Protocol::varInt(max(0, (int)($entry['ping'] ?? 0)))
				. Protocol::boolean(true) . Protocol::string($entry['name']);
		}
		return $payload;
	}

	private function keepAlive(float $now) : void{
		if($now - $this->lastKeepAlive < self::KEEP_ALIVE_SECONDS){
			return;
		}
		$this->lastKeepAlive = $now;
		$this->keepAliveId++;
		$id = $this->profile?->id('keep_alive') ?? Protocol::S2C_KEEP_ALIVE;
		$this->send($id, Protocol::long($this->keepAliveId));
	}

	/** Recalcula los chunks a enviar cuando el jugador cambia de columna. */
	private function requestChunks(JavaServer $server) : void{
		$chunkX = (int)floor($this->x / 16);
		$chunkZ = (int)floor($this->z / 16);
		if($chunkX === $this->lastChunkX && $chunkZ === $this->lastChunkZ){
			return;
		}
		$this->lastChunkX = $chunkX;
		$this->lastChunkZ = $chunkZ;

		// En moderno el cliente conoce el radio que anunciamos en `login` (2..32),
		// así que hay que enviar exactamente esa área y no el valor crudo del
		// config. La ruta 1.12.2 usa el valor tal cual.
		$radius = $this->profile !== null
			? max(2, min(32, $server->viewDistance()))
			: $server->viewDistance();
		$this->pendingChunks = [];
		$wanted = [];
		for($dx = -$radius; $dx <= $radius; $dx++){
			for($dz = -$radius; $dz <= $radius; $dz++){
				$key = ($chunkX + $dx) . ':' . ($chunkZ + $dz);
				$wanted[$key] = true;
				if(!isset($this->sentChunks[$key])){
					$this->pendingChunks[] = [$chunkX + $dx, $chunkZ + $dz];
				}
			}
		}

		// El chunk del jugador va primero: así el cliente cierra la pantalla de
		// carga con el terreno bajo los pies ya presente y no cae al vacío.
		// Distancia calculada una vez por columna (antes con `usort` cada
		// comparación recomputaba dos distancias con potencias).
		$decorated = [];
		foreach($this->pendingChunks as [$cx, $cz]){
			$decorated[] = [(($cx - $chunkX) ** 2) + (($cz - $chunkZ) ** 2), $cx, $cz];
		}
		sort($decorated);
		$this->pendingChunks = array_map(static fn(array $row) : array => [$row[1], $row[2]], $decorated);

		// Descargar los chunks que quedaron fuera de la vista (con margen).
		foreach(array_keys($this->sentChunks) as $key){
			if(isset($wanted[$key])){
				continue;
			}
			[$cx, $cz] = array_map('intval', explode(':', (string)$key));
			if(abs($cx - $chunkX) > $radius + 1 || abs($cz - $chunkZ) > $radius + 1){
				unset($this->sentChunks[$key]);
				$this->sendUnloadChunk((int)$cx, (int)$cz);
				// Si este chunk tenía entidades visibles, se limpian al
				// recargar; el sync de entidades los respawnea al volver.
				foreach($this->seenEntities as $lower => $seen){
					$ex = (int)floor($seen['x'] / 16);
					$ez = (int)floor($seen['z'] / 16);
					if($ex === (int)$cx && $ez === (int)$cz){
						unset($this->seenEntities[$lower]);
					}
				}
			}
		}

		if($this->profile !== null){
			$this->send($this->profile->id('update_view_position'),
				Protocol::varInt($chunkX) . Protocol::varInt($chunkZ));
			if(!$this->viewAnnounced){
				$this->send($this->profile->id('update_view_distance'),
					Protocol::varInt($radius));
				$this->viewAnnounced = true;
			}
		}
	}

	/**
	 * Envía columnas hasta agotar el presupuesto de tiempo del tick, en lotes
	 * de `CHUNKS_PER_BATCH`. El lote se abre sólo cuando hay algo que mandar, de
	 * modo que nunca queda un `chunk_batch_start` sin su `chunk_batch_finished`
	 * (el cliente espera ese cierre para aplicar los chunks).
	 *
	 * Anti-bloqueo: un chunk que no se puede construir tras 20 intentos en
	 * cabeza se mueve al final, para no bloquear el resto. El techo del
	 * cliente (`chunk_batch_received`, f32) limita los envíos por tick.
	 */
	private function streamChunks(JavaServer $server, ?float $deadline = null) : void{
		if($this->lastChunkX === PHP_INT_MIN){
			$this->requestChunks($server);
		}
		if($this->pendingChunks === []){
			return;
		}

		$profile = $this->profile;
		$ceiling = $this->chunksPerTick !== null ? max(1, (int)floor($this->chunksPerTick)) : self::CHUNKS_PER_BATCH;
		$perTick = min(self::CHUNKS_PER_BATCH, $ceiling);
		// `$deadline` lo fija el servidor para todos los clientes con una
		// sola vez; sin él (llamadas directas) se usa el presupuesto propio.
		$deadline ??= microtime(true) + self::CHUNK_BUDGET_SECONDS;
		$sent = 0;
		$batchOpen = false;
		while($this->pendingChunks !== []){
			if($this->pendingBytes() > self::CHUNK_BACKPRESSURE){
				break;
			}
			// También antes de la primera columna: si el presupuesto del
			// tick ya se gastó (p. ej. otro cliente del mismo tick), no se
			// arranca ninguna columna nueva en vez de sumar costes.
			if($sent >= $perTick || microtime(true) > $deadline){
				break;
			}

			[$chunkX, $chunkZ] = array_shift($this->pendingChunks);
			$key = $chunkX . ':' . $chunkZ;
			if(isset($this->sentChunks[$key])){
				unset($this->pendingAttempts[$key]);
				continue;
			}

			if($profile !== null){
				$chunk = $server->buildModernChunk($profile, $chunkX, $chunkZ);
				if($chunk === null){
					// Las alturas de este chunk aún no se han muestreado dentro del
					// presupuesto del tick: se reintenta en el siguiente sin abrir un
					// lote a medias, pero tras 20 intentos al final de la cola.
					$attempts = ($this->pendingAttempts[$key] ?? 0) + 1;
					$this->pendingAttempts[$key] = $attempts;
					if($attempts >= 20){
						$this->pendingChunks[] = [$chunkX, $chunkZ];
						$this->pendingAttempts[$key] = 0;
					}else{
						array_unshift($this->pendingChunks, [$chunkX, $chunkZ]);
					}
					break;
				}
				unset($this->pendingAttempts[$key]);
				if(!$batchOpen){
					$this->send($profile->id('chunk_batch_start'), '');
					$batchOpen = true;
				}
				$this->send($profile->id('map_chunk'),
					pack('N', $chunkX) .
					pack('N', $chunkZ) .
					$chunk['heightmaps'] .
					Protocol::varInt(strlen($chunk['data'])) . $chunk['data'] .
					Protocol::varInt(0) . // entidades de bloque
					$chunk['light']
				);
			}else{
				$chunk = $server->buildChunk($chunkX, $chunkZ);
				if($chunk === null){
					$attempts = ($this->pendingAttempts[$key] ?? 0) + 1;
					$this->pendingAttempts[$key] = $attempts;
					if($attempts >= 20){
						$this->pendingChunks[] = [$chunkX, $chunkZ];
						$this->pendingAttempts[$key] = 0;
					}else{
						array_unshift($this->pendingChunks, [$chunkX, $chunkZ]);
					}
					break;
				}
				unset($this->pendingAttempts[$key]);
				[$bitMap, $data] = $chunk;
				$this->send(Protocol::S2C_MAP_CHUNK,
					pack('N', $chunkX) .
					pack('N', $chunkZ) .
					Protocol::boolean(true) .
					Protocol::varInt($bitMap) .
					Protocol::varInt(strlen($data)) . $data .
					Protocol::varInt(0)
				);
			}

			$this->sentChunks[$key] = true;
			$sent++;
		}

		if($batchOpen && $profile !== null){
			$this->send($profile->id('chunk_batch_finished'), Protocol::varInt($sent));
		}

		if(!$this->spawnChunksLogged && $this->pendingChunks === []){
			$this->spawnChunksLogged = true;
			$server->log(sprintf(
				'Chunks iniciales de %s: %d columnas en %.0f ms (%d KB en cola)',
				(string)$this->username,
				count($this->sentChunks),
				(microtime(true) - $this->spawnChunksAt) * 1000.0,
				(int)($this->pendingBytes() / 1024)
			));
		}
	}

	private function sendUnloadChunk(int $chunkX, int $chunkZ) : void{
		$profile = $this->profile;
		if($profile !== null){
			// En 1.20.2+ el orden es Z, X.
			$this->send($profile->id('unload_chunk'), pack('N', $chunkZ) . pack('N', $chunkX));
			return;
		}
		$this->send(Protocol::S2C_UNLOAD_CHUNK, pack('N', $chunkX) . pack('N', $chunkZ));
	}

	public function sendChat(string $message) : void{
		$profile = $this->profile;
		if($profile !== null){
			$this->send($profile->id('system_chat'), Nbt::text($message) . Protocol::boolean(false));
			return;
		}
		$this->send(Protocol::S2C_CHAT, Protocol::string(Protocol::chatJson($message)) . chr(0));
	}

	/** Cambia un bloque para este cliente con el estado de su versión (real del mundo). */
	public function sendBlockChange(int $x, int $y, int $z, string $kind, ?int $networkId = null) : void{
		$profile = $this->profile;
		if($profile !== null){
			$state = $networkId !== null ? ModernChunk::bedrockState($networkId, $profile) : ModernChunk::state($kind === '' ? 'air' : $kind);
			$this->send($profile->id('block_change'),
				Protocol::position($x, $y, $z) . Protocol::varInt($state)
			);
			return;
		}
		$state = $networkId !== null ? ChunkBuilder::javaState($networkId) : ($kind === 'air' ? ChunkBuilder::JAVA_AIR : ChunkBuilder::JAVA_STONE);
		$this->send(Protocol::S2C_BLOCK_CHANGE, Protocol::position($x, $y + 64, $z) . Protocol::varInt($state));
	}

	/**
	 * Sincroniza las entidades visibles para este cliente.
	 *
	 * @param array<string, array{name:string,x:float,y:float,z:float,yaw:float,pitch:float,gamemode:int}> $targets lower => datos
	 */
	public function syncEntities(JavaServer $server, array $targets) : void{
		$profile = $this->profile;
		// Despawn: lo que ya no es visible.
		foreach(array_keys($this->seenEntities) as $lower){
			if(!array_key_exists($lower, $targets)){
				$this->despawnEntity($server, (string)$lower);
			}
		}
		if($profile === null){
			foreach($targets as $lower => $target){
				$this->syncLegacyEntity($server, (string)$lower, $target);
			}
		}else{
			foreach($targets as $lower => $target){
				$this->syncModernEntity($server, (string)$lower, $target);
			}
		}
	}

	/**
	 * @param array{name:string,x:float,y:float,z:float,yaw:float,pitch:float,gamemode:int} $target
	 */
	private function syncModernEntity(JavaServer $server, string $lower, array $target) : void{
		$profile = $this->profile;
		if($profile === null){
			return;
		}
		$entityId = $server->entityIdFor($target['name']);
		if(!isset($this->seenEntities[$lower])){
			// spawn_entity y nametag. El `player_info` add lo lleva la
			// TAB (`syncTab`), no el spawn: si el jugador ya está en la
			// TAB no se repite el add (el cliente rechazaría/ignoraría
			// el duplicado). Solo como fallback, si la TAB aún no lo
			// conoce (p. ej. cliente sintético sin tickTab), se anuncia
			// aquí y se registra en `$tabListed` para no duplicarlo.
			if(!isset($this->tabListed[$lower])){
				$this->send($profile->id('player_info'), $this->modernPlayerInfoAdd($target));
				$this->tabListed[$lower] = [
					'name' => $target['name'],
					'gamemode' => (int)$target['gamemode'],
					'ping' => (int)($target['ping'] ?? 0),
				];
			}
			$this->send($profile->id('spawn_entity'), $this->modernSpawnPayload($entityId, $target));
			$this->send($profile->id('entity_metadata'), $this->modernNametagPayload($entityId, $target['name']));
			$this->seenEntities[$lower] = [
				'name' => $target['name'],
				'entity' => $entityId,
				'x' => $target['x'], 'y' => $target['y'], 'z' => $target['z'],
				'yaw' => $target['yaw'], 'pitch' => $target['pitch'],
				'gamemode' => (int)$target['gamemode'],
			];
			return;
		}
		$prev = $this->seenEntities[$lower];
		$dist = abs($target['x'] - $prev['x']) + abs($target['y'] - $prev['y']) + abs($target['z'] - $prev['z']);
		// >8 bloques: teleport absoluto, no relativo.
		if($dist > 8.0){
			$this->send($profile->id('entity_teleport'), $this->modernTeleportPayload($entityId, $target));
		}elseif($dist > 0.0001){
			$dx = Protocol::relDelta($target['x'] - $prev['x']);
			$dy = Protocol::relDelta($target['y'] - $prev['y']);
			$dz = Protocol::relDelta($target['z'] - $prev['z']);
			$rotChanged = abs($target['yaw'] - $prev['yaw']) > 0.01 || abs($target['pitch'] - $prev['pitch']) > 0.01;
			if($rotChanged){
				$this->send($profile->id('entity_move_look'),
					Protocol::varInt($entityId) . Protocol::short($dx) . Protocol::short($dy) . Protocol::short($dz)
					. Protocol::byte(Protocol::angle($target['yaw'])) . Protocol::byte(Protocol::angle($target['pitch'])) . Protocol::boolean(false));
			}else{
				$this->send($profile->id('rel_entity_move'),
					Protocol::varInt($entityId) . Protocol::short($dx) . Protocol::short($dy) . Protocol::short($dz) . Protocol::boolean(false));
			}
		}else{
			$rotChanged = abs($target['yaw'] - $prev['yaw']) > 0.01 || abs($target['pitch'] - $prev['pitch']) > 0.01;
			if($rotChanged){
				$this->send($profile->id('entity_look'),
					Protocol::varInt($entityId) . Protocol::byte(Protocol::angle($target['yaw'])) . Protocol::byte(Protocol::angle($target['pitch'])) . Protocol::boolean(false));
			}
		}
		if(abs($target['yaw'] - $prev['yaw']) > 0.01){
			$this->send($profile->id('entity_head_rotation'),
				Protocol::varInt($entityId) . Protocol::byte(Protocol::angle($target['yaw'])));
		}
		// El cambio de gamemode en la TAB lo notifica `syncTab` (global,
		// también para jugadores lejos). Aquí solo se actualiza el
		// estado visto, sin paquete, para no duplicarlo.
		$this->seenEntities[$lower] = [
			'name' => $target['name'],
			'entity' => $entityId,
			'x' => $target['x'], 'y' => $target['y'], 'z' => $target['z'],
			'yaw' => $target['yaw'], 'pitch' => $target['pitch'],
			'gamemode' => (int)$target['gamemode'],
		];
	}

	/**
	 * @param array{name:string,x:float,y:float,z:float,yaw:float,pitch:float,gamemode:int} $target
	 */
	private function syncLegacyEntity(JavaServer $server, string $lower, array $target) : void{
		$entityId = $server->entityIdFor($target['name']);
		if(!isset($this->seenEntities[$lower])){
			// Ver `syncModernEntity`: el add lo lleva la TAB; aquí
			// solo como fallback si aún no está en `$tabListed`.
			if(!isset($this->tabListed[$lower])){
				$this->send(Protocol::S2C_PLAYER_INFO, $this->legacyPlayerInfoAdd($target));
				$this->tabListed[$lower] = [
					'name' => $target['name'],
					'gamemode' => (int)$target['gamemode'],
					'ping' => (int)($target['ping'] ?? 0),
				];
			}
			$this->send(Protocol::S2C_NAMED_ENTITY_SPAWN, $this->legacySpawnPayload($entityId, $target));
			$this->send(Protocol::S2C_ENTITY_METADATA, $this->legacyNametagPayload($entityId, $target['name']));
			$this->seenEntities[$lower] = [
				'name' => $target['name'],
				'entity' => $entityId,
				'x' => $target['x'], 'y' => $target['y'], 'z' => $target['z'],
				'yaw' => $target['yaw'], 'pitch' => $target['pitch'],
				'gamemode' => (int)$target['gamemode'],
			];
			return;
		}
		$prev = $this->seenEntities[$lower];
		$dist = abs($target['x'] - $prev['x']) + abs($target['y'] - $prev['y']) + abs($target['z'] - $prev['z']);
		if($dist > 8.0){
			$this->send(Protocol::S2C_ENTITY_TELEPORT,
				Protocol::varInt($entityId) . Protocol::double($target['x']) . Protocol::double($target['y'] + 64.0) . Protocol::double($target['z'])
				. Protocol::byte(Protocol::angle($target['yaw'])) . Protocol::byte(Protocol::angle($target['pitch'])) . Protocol::boolean(false));
		}elseif($dist > 0.0001){
			$dx = Protocol::relDelta($target['x'] - $prev['x']);
			$dy = Protocol::relDelta($target['y'] - $prev['y']);
			$dz = Protocol::relDelta($target['z'] - $prev['z']);
			$rotChanged = abs($target['yaw'] - $prev['yaw']) > 0.01 || abs($target['pitch'] - $prev['pitch']) > 0.01;
			if($rotChanged){
				$this->send(Protocol::S2C_ENTITY_MOVE_LOOK,
					Protocol::varInt($entityId) . Protocol::short($dx) . Protocol::short($dy) . Protocol::short($dz)
					. Protocol::byte(Protocol::angle($target['yaw'])) . Protocol::byte(Protocol::angle($target['pitch'])) . Protocol::boolean(false));
			}else{
				$this->send(Protocol::S2C_REL_ENTITY_MOVE,
					Protocol::varInt($entityId) . Protocol::short($dx) . Protocol::short($dy) . Protocol::short($dz) . Protocol::boolean(false));
			}
		}elseif(abs($target['yaw'] - $prev['yaw']) > 0.01 || abs($target['pitch'] - $prev['pitch']) > 0.01){
			$this->send(Protocol::S2C_ENTITY_LOOK,
				Protocol::varInt($entityId) . Protocol::byte(Protocol::angle($target['yaw'])) . Protocol::byte(Protocol::angle($target['pitch'])) . Protocol::boolean(false));
		}
		if(abs($target['yaw'] - $prev['yaw']) > 0.01){
			$this->send(Protocol::S2C_ENTITY_HEAD_ROTATION,
				Protocol::varInt($entityId) . Protocol::byte(Protocol::angle($target['yaw'])));
		}
		// Gamemode en TAB: lo notifica `syncTab` (ver moderno).
		$this->seenEntities[$lower] = [
			'name' => $target['name'],
			'entity' => $entityId,
			'x' => $target['x'], 'y' => $target['y'], 'z' => $target['z'],
			'yaw' => $target['yaw'], 'pitch' => $target['pitch'],
			'gamemode' => (int)$target['gamemode'],
		];
	}

	private function despawnEntity(JavaServer $server, string $lower) : void{
		$seen = $this->seenEntities[$lower] ?? null;
		unset($this->seenEntities[$lower]);
		if($seen === null){
			return;
		}
		// entity_destroy. La salida de la TAB (`player_remove` moderno /
		// acción 4 legacy) NO va aquí: alejarse no es salir del
		// servidor. Solo si la entidad se anunció por el fallback de
		// spawn (no está en `$tabListed`, la TAB nunca la tuvo) se
		// limpia también su `player_info`; si está en la TAB, la
		// retirada la hace `syncTab` cuando salga de verdad.
		$name = $seen['name'] ?? null;
		$inTab = isset($this->tabListed[$lower]);
		if($name === null){
			foreach($server->getClients() as $client){
				if($client->getUsername() !== null && strtolower($client->getUsername()) === $lower){
					$name = $client->getUsername();
					break;
				}
			}
		}
		$uuid = $name !== null ? Protocol::offlineUuid($name) : null;
		$profile = $this->profile;
		if($profile !== null){
			$this->send($profile->id('entity_destroy'),
				Protocol::varInt(1) . Protocol::varInt((int)$seen['entity']));
			if($inTab){
				return;
			}
			// uuid del jugador: si no está entre clientes, se reconstruye por
			// nombre desde el lower (offlineUuid es por nombre exacto, no lower).
			// Para Bedrock se usa el mismo offlineUuid del nombre guardado; aquí
			// se busca en getOnlinePlayers del servidor.
			$resolved = $uuid;
			if($resolved === null){
				foreach($server->getOnlinePlayerNames() as $candidate){
					if(strtolower($candidate) === $lower){ $resolved = Protocol::offlineUuid($candidate); break; }
				}
			}
			if($resolved !== null){
				$this->send($profile->id('player_remove'),
					Protocol::varInt(1) . $resolved);
			}
		}else{
			$this->send(Protocol::S2C_ENTITY_DESTROY,
				Protocol::varInt(1) . Protocol::varInt((int)$seen['entity']));
			if($inTab){
				return;
			}
			$resolved = $uuid;
			if($resolved === null){
				foreach($server->getOnlinePlayerNames() as $candidate){
					if(strtolower($candidate) === $lower){ $resolved = Protocol::offlineUuid($candidate); break; }
				}
			}
			if($resolved !== null){
				// player_info remove_player (acción 4) legacy.
				$this->send(Protocol::S2C_PLAYER_INFO,
					Protocol::varInt(4) . Protocol::varInt(1) . $resolved);
			}
		}
	}

	/** @param array{name:string,x:float,y:float,z:float,yaw:float,pitch:float,gamemode:int,ping?:int} $target */
	private function modernPlayerInfoAdd(array $target) : string{
		$profile = $this->profile;
		if($profile === null){
			return '';
		}
		// Mismo formato que la TAB (`modernPlayerInfoAddBatch`):
		// add_player | update_game_mode | update_listed | update_latency.
		return $this->modernPlayerInfoAddBatch([[
			'name' => $target['name'],
			'gamemode' => (int)$target['gamemode'],
			'ping' => (int)($target['ping'] ?? 0),
		]]);
	}

	/** @param array{name:string,x:float,y:float,z:float,yaw:float,pitch:float,gamemode:int} $target */
	private function modernSpawnPayload(int $entityId, array $target) : string{
		$profile = $this->profile;
		if($profile === null){
			return '';
		}
		// Esquema `packet_spawn_entity` de `protocol.json`:
		// 769/770: entityId, UUID, type, x/y/z, pitch, yaw, headPitch,
		//   objectData, velocity vec3i16 (al final).
		// 773/774/775: entityId, UUID, type, x/y/z, velocity lpVec3,
		//   pitch, yaw, headPitch, objectData. Con el formato antiguo en
		//   1.21.9+ el byte de pitch se lee como cabecera lpVec3 y el
		//   cliente aborta decodificando `add_entity` (26.1 en concreto).
		// Velocidad cero: vec3i16 = 0,0,0 y lpVec3 = 0x00.
		$angles = Protocol::byte(Protocol::angle($target['pitch']))
			. Protocol::byte(Protocol::angle($target['yaw']))
			. Protocol::byte(Protocol::angle($target['yaw']));
		$head = Protocol::varInt($entityId)
			. Protocol::offlineUuid($target['name'])
			. Protocol::varInt($profile->playerEntityType())
			. Protocol::double($target['x']) . Protocol::double($target['y']) . Protocol::double($target['z']);
		if($profile->spawnVelocityIsLpVec3()){
			return $head . Protocol::lpVec3(0.0, 0.0, 0.0) . $angles . Protocol::varInt(0);
		}
		return $head . $angles . Protocol::varInt(0) . Protocol::short(0) . Protocol::short(0) . Protocol::short(0);
	}

	/** @param array{name:string,x:float,y:float,z:float,yaw:float,pitch:float,gamemode:int} $target */
	private function modernTeleportPayload(int $entityId, array $target) : string{
		return Protocol::varInt($entityId)
			. Protocol::double($target['x']) . Protocol::double($target['y']) . Protocol::double($target['z'])
			. Protocol::double(0.0) . Protocol::double(0.0) . Protocol::double(0.0)
			. Protocol::float($target['yaw']) . Protocol::float($target['pitch'])
			. pack('N', 0) . Protocol::boolean(false);
	}

	private function modernNametagPayload(int $entityId, string $name) : string{
		// custom_name (índice 2, serializer optional_component=6) +
		// custom_name_visible (3, boolean=8). El campo 2 en vanilla es
		// `DATA_CUSTOM_NAME: Optional<Component>` (Entity.<clinit>,
		// + OptionalComponent en EntityDataSerializers): mandar el
		// tipo 5 (component directo) hace que SynchedEntityData
		// intente asignar un MutableComponent a un Optional y el
		// cliente 26.1 aborta con IllegalStateException. Como
		// `optional_component` es `option(anonymousNbt)`, el valor
		// lleva el byte de presencia 0x01 antes del NBT.
		// El id 6 y el 8 coinciden en protocol.json de 769-775
		// (entityMetadataEntry: 5=component, 6=optional_component,
		// 8=boolean), así que el fix aplica a todas las modernas.
		$profile = $this->profile;
		if($profile === null){
			return '';
		}
		$metadata = chr(2) . Protocol::varInt(6) . Protocol::boolean(true) . Nbt::text($name)
			. chr(3) . Protocol::varInt(8) . Protocol::boolean(true)
			. chr(0xFF);
		return Protocol::varInt($entityId) . $metadata;
	}

	/** @param array{name:string,x:float,y:float,z:float,yaw:float,pitch:float,gamemode:int,ping?:int} $target */
	private function legacyPlayerInfoAdd(array $target) : string{
		// add_player (0): nombre, propiedades, gamemode, ping, displayName opcional.
		return $this->legacyPlayerInfoAddBatch([[
			'name' => $target['name'],
			'gamemode' => (int)$target['gamemode'],
			'ping' => (int)($target['ping'] ?? 0),
		]]);
	}

	/** @param array{name:string,x:float,y:float,z:float,yaw:float,pitch:float,gamemode:int} $target */
	private function legacySpawnPayload(int $entityId, array $target) : string{
		// named_entity_spawn: uuid, posición (Y+64), ángulos y metadata vacía (0xFF).
		return Protocol::varInt($entityId) . Protocol::offlineUuid($target['name'])
			. Protocol::double($target['x']) . Protocol::double($target['y'] + 64.0) . Protocol::double($target['z'])
			. Protocol::byte(Protocol::angle($target['yaw'])) . Protocol::byte(Protocol::angle($target['pitch']))
			. chr(0xFF);
	}

	private function legacyNametagPayload(int $entityId, string $name) : string{
		// custom_name (2, string=3) + custom_name_visible (3, boolean=6) legacy.
		$metadata = chr(2) . chr(3) . Protocol::string($name)
			. chr(3) . chr(6) . Protocol::boolean(true)
			. chr(0xFF);
		return Protocol::varInt($entityId) . $metadata;
	}

	/** Cierra la conexión con un mensaje de desconexión. */
	public function kick(string $reason) : void{
		if($this->closed){
			return;
		}
		$profile = $this->profile;
		if($profile !== null && $this->playing){
			$this->send($profile->id('kick'), Nbt::text($reason));
		}elseif($this->state === self::STATE_LOGIN){
			$this->send(Protocol::S2C_LOGIN_DISCONNECT, Protocol::string(Protocol::chatJson($reason)));
		}elseif($this->playing){
			$this->send(Protocol::S2C_KICK, Protocol::string(Protocol::chatJson($reason)));
		}
		$this->flush();
		$this->close();
	}

	public function close() : void{
		if($this->stream !== null){
			@fclose($this->stream);
			$this->stream = null;
		}
		$this->out = '';
		$this->outOffset = 0;
		$this->closed = true;
		$this->playing = false;
	}

	private function send(int $packetId, string $payload) : void{
		if($this->closed){
			return;
		}
		$this->out .= Protocol::frame($packetId, $payload);
	}

	/** Bytes pendientes de escribir en el socket. */
	private function pendingBytes() : int{
		return strlen($this->out) - $this->outOffset;
	}

	/**
	 * Vacía el buffer de salida en el socket sin bloquear.
	 *
	 * Se escribe en trozos y se avanza un desplazamiento en vez de recortar la
	 * cadena en cada escritura parcial: con 1,5 MB de chunks en cola, un
	 * `substr` por escritura copiaba el buffer entero y el tick se iba en copias.
	 */
	private function flush() : void{
		if($this->stream === null || $this->out === ''){
			return;
		}
		$total = strlen($this->out);
		while($this->outOffset < $total){
			$written = @fwrite($this->stream, substr($this->out, $this->outOffset, 65536));
			if($written === false || $written === 0){
				break;
			}
			$this->outOffset += $written;
		}
		if($this->outOffset >= $total){
			$this->out = '';
			$this->outOffset = 0;
		}elseif($this->outOffset > 0){
			// Compacta para que el buffer no crezca indefinidamente.
			$this->out = substr($this->out, $this->outOffset);
			$this->outOffset = 0;
		}
	}
}
