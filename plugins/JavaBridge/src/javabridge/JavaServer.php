<?php

declare(strict_types=1);

namespace javabridge;

/**
 * Listener TCP del puente Java -> Bedrock.
 *
 * Todo el trabajo ocurre en `tick()`, que el plugin llama desde una tarea
 * repetitiva del scheduler (una vez por tick del servidor). El socket no
 * bloquea: `stream_socket_accept(..., 0)` y `fread()` sobre streams en modo
 * no bloqueante.
 */
final class JavaServer{

	/** @var resource|null */
	private $socket = null;

	/** @var JavaClient[] */
	private array $clients = [];

	/** @var array<string, array{0:int,1:string}> chunkX:chunkZ => [máscara, datos] */
	private array $chunkCache = [];

	/** @var array<int, true> protocolos que ya registraron chunk con agua (log una vez) */
	private array $loggedWaterChunk = [];

	/** @var array<string, array{data:string, heightmaps:string, light:string}> chunks modernos por protocolo */
	private array $modernChunkCache = [];

	/** @var array<string, object> chunkX:chunkZ => columna de Bedrock ya leída */
	private array $chunkDataCache = [];

	private bool $chunkApiMissing = false;

	/**
	 * Último estado conocido por los clientes Java, por coordenada.
	 *
	 * `x,y,z` => id de red de Bedrock (`getNetworkId()`), o null si no se
	 * pudo resolver. Sirve para dos cosas: no reenviar a los Java un cambio
	 * que ya les mandamos (p. ej. el eco de nuestro propio `placeBlock`), y
	 * como caché del fallback por polling cuando el binario Rust en
	 * ejecución aún no expone la cola de cambios (ver `tickBlockChanges()`).
	 *
	 * @var array<string, ?int>
	 */
	private array $lastKnownBlocks = [];

	/**
	 * Si el binario Rust en ejecución aún no expone la cola de cambios, no
	 * se pregunta al host cada tick (sería una host call inútil por tick):
	 * se reintenta cada `BLOCK_CHANGES_PROBE_INTERVAL` ticks para detectar
	 * el aterrizaje tras recompilar/reiniciar sin reiniciar este plugin.
	 * La cola SÍ está implementada en `crates/server-core/src/plugin_host.rs`
	 * (`world.take_block_changes` / `world.drain_changes`); si PHP la ve
	 * ausente, casi siempre es un `server-core.exe` sin recompilar o un
	 * worker PHP viejo (ver mensaje de `tickBlockChanges()`).
	 */
	private bool $blockChangesApiMissing = false;
	private int $ticksSinceBlockChangesProbe = PHP_INT_MAX;
	private int $fallbackPollCursor = 0;

	private float $lastSlowTickLog = 0.0;

	/** Ticks desde el último `update_time` a los Java en play. */
	private int $ticksSinceTimeSync = 0;
	/** Última hora enviada a los Java en play; null antes del primero. */
	private ?int $lastBroadcastTime = null;
	/** Último `time_running` enviado; null antes del primero. */
	private ?bool $lastBroadcastTimeRunning = null;
	/**
	 * Cada cuántos ticks se reenvía la hora aunque avance normal
	 * (20 ticks = 1 s, igual que el `SetTime` de Bedrock en Rust).
	 * Los clientes avanzan el sol localmente entre paquetes; 1 Hz
	 * los mantiene en sync sin un paquete por tick. Un salto mayor
	 * (`/time set/add`, plugin `setTime`) se envía en el siguiente
	 * tick, sin esperar al intervalo.
	 */
	private const TIME_SYNC_INTERVAL_TICKS = 20;

	// Las cachés deben cubrir el área completa de `view-distance: 6`
	// (13x13 = 169 columnas): con 64, el streaming inicial expulsaba las
	// primeras columnas antes de terminar y cualquier re-petición (segundo
	// cliente, volver a una zona) regeneraba el chunk en el host (~20 ms
	// en el hilo del tick) en vez de reutilizarlo.
	private const CHUNK_CACHE_LIMIT = 256;
	/**
	 * La caché moderna es por protocolo (`protocol:chunkX:chunkZ`) porque
	 * los ids de estado difieren entre versiones (769..775): no se puede
	 * compartir la traducción entre protocolos sin romper el render.
	 * Por eso el límite cubre dos protocolos completos (2×169 view-distance
	 * 6 + margen): con 256 compartidos, un 769 y un 775 conectados a la
	 * vez se expulsaban mutuamente (thrash y regeneración ~20 ms).
	 */
	private const MODERN_CHUNK_CACHE_LIMIT = 512;
	private const CHUNK_DATA_CACHE_LIMIT = 256;
	/**
	 * Nombre de la cola de cambios Bedrock -> Java en el host.
	 *
	 * Implementado en `crates/server-core/src/plugin_host.rs` como
	 * `world.take_block_changes` (alias `world.drain_changes`), con
	 * respuesta `{"ok": true, "changes": [{x, y, z, block_id}, ...]}`.
	 * PHP la consume con estos nombres en `tickBlockChanges()` /
	 * `probeBlockChangesHost()`. Si PHP sigue viendo null tras
	 * recompilar, revisar: (1) que el `server-core.exe` en ejecución sea
	 * el recién compilado (su fecha debe ser posterior a `plugin_host.rs`),
	 * (2) reinicio completo del servidor (el worker PHP hijo se crea con
	 * el servidor; no hay reload en caliente de PHP ni de host), y
	 * (3) el STDERR `[PHP/BRIDGE] Host call world.take_block_changes
	 * failed: ...` para el error exacto del host. Mientras falte, se usa
	 * el fallback por polling de `tickBlockChanges()`.
	 */
	/** @var list<string> */
	private const BLOCK_CHANGES_METHODS = ['world.take_block_changes', 'world.drain_changes'];
	/** Cada cuántos ticks se reintenta la cola si aún no existe. */
	private const BLOCK_CHANGES_PROBE_INTERVAL = 100;
	/**
	 * Coordenadas conocidas a sondear por tick en el fallback. Cada
	 * sondeo es un `world.get_block` (host call síncrona), así que se
	 * acota para no romper el presupuesto de 10 ms del tick. El
	 * fallback solo corre mientras el binario no expone la cola; con
	 * la cola activa nunca se llega aquí.
	 */
	private const FALLBACK_POLL_PER_TICK = 8;
	/** Tope de la caché de bloques conocidos (no crecer sin límite). */
	private const LAST_KNOWN_BLOCKS_LIMIT = 8192;

	/**
	 * Presupuesto TOTAL del tick del puente (todos los clientes). Generar un
	 * chunk nuevo cuesta decenas de ms en el host (NormalGenerator ~18 ms),
	 * así que cada cliente necesita su propio techo para que tres clientes
	 * no sumen 60 ms y bloqueen el resto del servidor.
	 */
	private const TICK_BUDGET_SECONDS = 0.010;

	/** Umbral e intervalo del aviso de tick lento: en uso normal no debe salir. */
	private const SLOW_TICK_LOG_MS = 15.0;
	private const SLOW_TICK_LOG_INTERVAL_SECONDS = 10.0;

	/**
	 * Cada cuántos ticks se sincronizan las entidades (`tickEntities()`,
	 * host call `server.get_players_full`): cada host call es un round-trip
	 * síncrono por el pipe IPC (~15 ms medidos en Windows) y no hace falta
	 * a 20 Hz para mover entidades que el cliente interpola. A 5 Hz el
	 * movimiento Bedrock->Java sigue viéndose fluido y el coste medio por
	 * tick baja a ~1/4.
	 */
	private const ENTITY_SYNC_INTERVAL_TICKS = 4;
	/**
	 * Cada cuántos ticks se drenan los cambios de bloques
	 * (`tickBlockChanges()`, host call `world.take_block_changes`): 10 Hz
	 * es imperceptible para romper/colocar bloques y recorta la mitad de
	 * las host calls del camino feliz.
	 */
	private const BLOCK_CHANGES_INTERVAL_TICKS = 2;

	private bool $running = false;

	/** Contador de ticks del puente (para los intervalos de arriba). */
	private int $tickCount = 0;

	/**
	 * Memo de `player.get` dentro de un único `tick()`: `syncGamemode`,
	 * `syncTeleport` y `hasTeleportPending` (vía `maybeUpdatePosition`)
	 * leían el mismo snapshot hasta 3 veces por tick y cliente, pagando
	 * una host call por lectura. Solo se usa durante `tick()` (las
	 * llamadas directas de los tests, fuera de `tick()`, van siempre al
	 * host: el stub de tests muta el registry entre llamadas).
	 *
	 * @var array<string, ?array>
	 */
	private array $playerGetMemo = [];
	private bool $inServerTick = false;

	/** @var array<string, \pocketmine\player\Player> jugadores Java dados de alta en la API (nombre => jugador) */
	private array $apiPlayers = [];

	/** Registro central de entityIds: empieza en 1_000_000 y no se reutiliza. */
	private int $nextEntityId = 1_000_000;

	/** @var array<string, int> nombre lower => entityId */
	private array $entityIds = [];

	public function __construct(
		private Main $plugin,
		private string $host,
		private int $port,
		private int $viewDistance
	){}

	public function start() : void{
		if($this->running){
			return;
		}

		$errno = 0;
		$errstr = '';
		$context = stream_context_create([
			'socket' => [
				'backlog' => 32,
				'so_reuseaddr' => true,
			],
		]);
		$socket = @stream_socket_server(
			'tcp://' . $this->host . ':' . $this->port,
			$errno,
			$errstr,
			STREAM_SERVER_BIND | STREAM_SERVER_LISTEN,
			$context
		);
		if($socket === false){
			throw new \RuntimeException(
				"No se pudo escuchar en tcp://{$this->host}:{$this->port}: {$errstr} ({$errno})"
			);
		}

		stream_set_blocking($socket, false);
		$this->socket = $socket;
		$this->running = true;
		$this->log("Escuchando en tcp://{$this->host}:{$this->port} (Java " . Protocol::VERSION_NAME .
			", protocolo " . Protocol::PROTOCOL_VERSION . ", view-distance {$this->viewDistance}).");
	}

	public function stop() : void{
		foreach($this->clients as $client){
			$this->leaveApi($client);
			$client->kick("El puente Java se está cerrando.");
			$client->close();
		}
		$this->clients = [];
		if($this->socket !== null){
			@fclose($this->socket);
			$this->socket = null;
		}
		$this->running = false;
		$this->log("Listener Java detenido.");
	}

	public function isRunning() : bool{
		return $this->running;
	}

	public function getHost() : string{
		return $this->host;
	}

	public function getPort() : int{
		return $this->port;
	}

	public function viewDistance() : int{
		return $this->viewDistance;
	}

	/** @return JavaClient[] */
	public function getClients() : array{
		return $this->clients;
	}

	/** Id de entidad central para un jugador (desde 1_000_000, sin reutilizar). */
	public function entityIdFor(string $name) : int{
		$key = strtolower($name);
		if(!isset($this->entityIds[$key])){
			$this->entityIds[$key] = $this->nextEntityId++;
		}
		return $this->entityIds[$key];
	}

	/** @return list<string> nombres de todos los jugadores online (Bedrock + Java). */
	public function getOnlinePlayerNames() : array{
		$names = [];
		foreach($this->plugin->getServer()->getOnlinePlayers() as $player){
			$names[] = $player->getName();
		}
		// getOnlinePlayers ya incluye a los Java (addPlayer), pero si el
		// runtime no expone addPlayer se añaden los clientes playing.
		$known = array_map('strtolower', $names);
		foreach($this->clients as $client){
			if($client->isPlaying() && $client->getUsername() !== null){
				if(!in_array(strtolower($client->getUsername()), $known, true)){
					$names[] = $client->getUsername();
				}
			}
		}
		return $names;
	}

	/**
	 * Datos guardados de un jugador (`player.load`), o null si nunca jugó.
	 *
	 * @return ?array{x:float,y:float,z:float,yaw:float,pitch:float,gamemode:int,health:float}
	 */
	public function loadPlayerData(string $name) : ?array{
		$result = self::host('player.load', ['name' => $name]);
		if(!is_array($result)){
			return null;
		}
		if(!($result['has_data'] ?? $result['exists'] ?? false)){
			return null;
		}
		$data = isset($result['data']) && is_array($result['data']) ? $result['data'] : $result;
		if(!isset($data['x'], $data['y'], $data['z'])){
			return null;
		}
		return [
			'x' => (float)$data['x'], 'y' => (float)$data['y'], 'z' => (float)$data['z'],
			'yaw' => isset($data['yaw']) ? (float)$data['yaw'] : 0.0,
			'pitch' => isset($data['pitch']) ? (float)$data['pitch'] : 0.0,
			'gamemode' => isset($data['gamemode']) ? (int)$data['gamemode'] : $this->gamemode(),
			'health' => isset($data['health']) ? (float)$data['health'] : 20.0,
		];
	}

	/** `player.update_position` throttled desde el cliente Java. */
	public function updatePlayerPosition(JavaClient $client) : void{
		$name = $client->getUsername();
		if($name === null){
			return;
		}
		// El cliente Java trabaja en yaw Java; el registry de Rust, la
		// copia `apiPlayer` y los saves son canónicos Bedrock: se publica
		// el yaw convertido (+180) para que los Bedrock vean al Java
		// mirando donde mira (y no al revés).
		$bedrockYaw = Protocol::javaYawToBedrock($client->getYaw());
		// Mantiene la copia de la API en sync sin host call.
		if(isset($this->apiPlayers[$name])){
			$player = $this->apiPlayers[$name];
			if(property_exists($player, 'x')){ $player->x = $client->getX(); }
			if(property_exists($player, 'y')){ $player->y = $client->getY(); }
			if(property_exists($player, 'z')){ $player->z = $client->getZ(); }
			if(property_exists($player, 'yaw')){ $player->yaw = $bedrockYaw; }
			if(property_exists($player, 'pitch')){ $player->pitch = $client->getPitch(); }
		}
		$result = self::host('player.update_position', [
			'name' => $name,
			'x' => $client->getX(), 'y' => $client->getY(), 'z' => $client->getZ(),
			'yaw' => $bedrockYaw, 'pitch' => $client->getPitch(),
		]);
		// Si Rust olvidó la entrada (borrada de golpe o `player.register`
		// perdida), se cura sin re-anunciar el join: solo re-registra el
		// snapshot para que /list vuelva a contar al jugador.
		if(is_array($result) && !($result['found'] ?? true)){
			$this->healRegistry($client);
		}
	}

	/** Re-registra el snapshot Rust sin repetir el alta PHP ni el evento. */
	private function healRegistry(JavaClient $client) : void{
		$name = $client->getUsername();
		if($name === null){
			return;
		}
		$entry = self::host('player.get', ['name' => $name]);
		if(is_array($entry) && ($entry['is_virtual'] ?? true) === false){
			// El nombre lo ocupa un Bedrock vivo: no robarlo.
			return;
		}
		$player = $this->apiPlayers[$name] ?? null;
		$world = 'world';
		if($player !== null){
			try{ $world = $player->getWorld()->getName(); }catch(\Throwable){}
		}
		self::host('player.register', [
			'name' => $name,
			'display_name' => $name,
			'address' => (string)$client->getPeer(),
			'gamemode' => $player?->getGamemode() ?? $this->gamemode(),
			'x' => $client->getX(), 'y' => $client->getY(), 'z' => $client->getZ(),
			'yaw' => Protocol::javaYawToBedrock($client->getYaw()), 'pitch' => $client->getPitch(),
			'world' => $world,
			'health' => $player?->getHealth() ?? 20.0,
			'max_health' => $player?->getMaxHealth() ?? 20.0,
			'is_op' => $player?->isOp() ?? false,
		]);
	}

	/**
	 * Garantiza el alta en la API mientras el cliente siga en play.
	 *
	 * Si el registro se borró (desconexión breve, `player.unregister`
	 * ajeno o `host()` sin Bridge en el tick del join), se vuelve a
	 * registrar sin esperar a una reconexión.
	 */
	public function ensureRegistered(JavaClient $client) : void{
		$name = $client->getUsername();
		if($name === null || !$client->isPlaying()){
			return;
		}
		if($client->apiJoined && isset($this->apiPlayers[$name])){
			return;
		}
		$this->joinApi($client);
	}

	/**
	 * Objetivos de entidades para un observador: todos los jugadores online
	 * salvo él mismo, filtrados por la puerta de chunks (sin chunk no hay spawn).
	 *
	 * Las posiciones Bedrock vienen de los snapshots del host
	 * (`server.get_players_full`, que Rust refresca en cada movimiento),
	 * no de la copia PHP creada en el join (congelada). Sin host (tests
	 * nativos) se usan los getters de la copia PHP como fallback.
	 *
	 * @param array<string, \pocketmine\player\Player> $players lista de un tick
	 * @param array<string, array> $snapshotsByName lower => snapshot del host
	 * @return array<string, array{name:string,x:float,y:float,z:float,yaw:float,pitch:float,gamemode:int}>
	 */
	private function entityTargetsFor(JavaClient $observer, ?array $players = null, ?array $snapshotsByName = null) : array{
		$targets = [];
		$self = $observer->getUsername();
		$players ??= $this->plugin->getServer()->getOnlinePlayers();
		$snapshotsByName ??= [];
		$known = [];
		foreach($players as $player){
			$name = $player->getName();
			$known[strtolower($name)] = true;
			if($self !== null && strcasecmp($name, $self) === 0){
				continue;
			}
			$x = method_exists($player, 'getX') ? (float)$player->getX() : 0.0;
			$y = method_exists($player, 'getY') ? (float)$player->getY() : 64.0;
			$z = method_exists($player, 'getZ') ? (float)$player->getZ() : 0.0;
			$yaw = method_exists($player, 'getYaw') ? (float)$player->getYaw() : 0.0;
			$pitch = method_exists($player, 'getPitch') ? (float)$player->getPitch() : 0.0;
			$gamemode = method_exists($player, 'getGamemode') ? (int)$player->getGamemode() : 0;
			$snapshot = $snapshotsByName[strtolower($name)] ?? null;
			if(is_array($snapshot) && ($snapshot['is_virtual'] ?? true) === false){
				// Snapshot Bedrock vivo: manda sobre la copia PHP.
				if(is_numeric($snapshot['x'] ?? null)){ $x = (float)$snapshot['x']; }
				if(is_numeric($snapshot['y'] ?? null)){ $y = (float)$snapshot['y']; }
				if(is_numeric($snapshot['z'] ?? null)){ $z = (float)$snapshot['z']; }
				if(is_numeric($snapshot['yaw'] ?? null)){ $yaw = (float)$snapshot['yaw']; }
				if(is_numeric($snapshot['pitch'] ?? null)){ $pitch = (float)$snapshot['pitch']; }
				if(is_numeric($snapshot['gamemode'] ?? null)){ $gamemode = (int)$snapshot['gamemode']; }
			}
			$cx = (int)floor($x / 16);
			$cz = (int)floor($z / 16);
			if(!$observer->hasSentChunk($cx, $cz)){
				continue;
			}
			// `$yaw`/`$pitch` vienen en sistema Bedrock (copia `apiPlayer`
			// o snapshot del host, ambos canónicos Bedrock): los
			// paquetes Java (`spawn_entity`, `entity_move_look`,
			// `entity_head_rotation`, ...) van en sistema Java. Sin
			// el `+180` el Java ve al Bedrock mirando hacia atrás.
			$targets[strtolower($name)] = [
				'name' => $name, 'x' => $x, 'y' => $y, 'z' => $z,
				'yaw' => Protocol::bedrockYawToJava($yaw), 'pitch' => Protocol::bedrockPitchToJava($pitch), 'gamemode' => $gamemode,
			];
		}
		// Bedrock vivos en el host pero sin copia PHP (join anterior al
		// runtime / evento perdido): se sintetizan como objetivos para que
		// los Java los vean igualmente.
		foreach($snapshotsByName as $lower => $snapshot){
			if(!is_array($snapshot) || isset($known[$lower])){
				continue;
			}
			if(($snapshot['is_virtual'] ?? true) === true){
				continue;
			}
			$name = (string)($snapshot['name'] ?? '');
			if($name === '' || ($self !== null && strcasecmp($name, $self) === 0)){
				continue;
			}
			if(!is_numeric($snapshot['x'] ?? null) || !is_numeric($snapshot['y'] ?? null) || !is_numeric($snapshot['z'] ?? null)){
				continue;
			}
			$x = (float)$snapshot['x'];
			$y = (float)$snapshot['y'];
			$z = (float)$snapshot['z'];
			$cx = (int)floor($x / 16);
			$cz = (int)floor($z / 16);
			if(!$observer->hasSentChunk($cx, $cz)){
				continue;
			}
			$targets[(string)$lower] = [
				'name' => $name, 'x' => $x, 'y' => $y, 'z' => $z,
				'yaw' => Protocol::bedrockYawToJava(is_numeric($snapshot['yaw'] ?? null) ? (float)$snapshot['yaw'] : 0.0),
				'pitch' => Protocol::bedrockPitchToJava(is_numeric($snapshot['pitch'] ?? null) ? (float)$snapshot['pitch'] : 0.0),
				'gamemode' => is_numeric($snapshot['gamemode'] ?? null) ? (int)$snapshot['gamemode'] : 0,
			];
		}
		return $targets;
	}

	/**
	 * Snapshots vivos de Rust (`server.get_players_full`): Bedrock y Java.
	 *
	 * @return list<array> lista de snapshots o [] sin host
	 */
	private function hostPlayerSnapshots() : array{
		$result = self::host('server.get_players_full', []);
		if(!is_array($result) || !array_is_list($result)){
			return [];
		}
		$out = [];
		foreach($result as $entry){
			if(is_array($entry) && isset($entry['name'], $entry['x'], $entry['y'], $entry['z'])){
				$out[] = $entry;
			}
		}
		return $out;
	}

	/**
	 * Refresca las copias PHP de los Bedrock con la posición viva del host.
	 *
	 * @param array<string, \pocketmine\player\Player> $players
	 * @param array<string, array> $snapshotsByName lower => snapshot
	 */
	private function refreshBedrockPositions(array $players, array $snapshotsByName) : void{
		foreach($players as $player){
			$snapshot = $snapshotsByName[strtolower($player->getName())] ?? null;
			if(!is_array($snapshot) || ($snapshot['is_virtual'] ?? true) === true){
				continue;
			}
			if(is_numeric($snapshot['x'] ?? null) && property_exists($player, 'x')){ $player->x = (float)$snapshot['x']; }
			if(is_numeric($snapshot['y'] ?? null) && property_exists($player, 'y')){ $player->y = (float)$snapshot['y']; }
			if(is_numeric($snapshot['z'] ?? null) && property_exists($player, 'z')){ $player->z = (float)$snapshot['z']; }
			if(is_numeric($snapshot['yaw'] ?? null) && property_exists($player, 'yaw')){ $player->yaw = (float)$snapshot['yaw']; }
			if(is_numeric($snapshot['pitch'] ?? null) && property_exists($player, 'pitch')){ $player->pitch = (float)$snapshot['pitch']; }
			if(is_numeric($snapshot['gamemode'] ?? null) && property_exists($player, 'gamemode')){ $player->gamemode = (int)$snapshot['gamemode']; }
		}
	}

	/**
	 * Sincroniza entidades para cada cliente Java en play. En `tick()`
	 * se llama cada `ENTITY_SYNC_INTERVAL_TICKS` ticks (no cada tick):
	 * la host call `server.get_players_full` es el coste dominante.
	 */
	private function tickEntities() : void{
		// Sin Java en play no hay observadores: no se paga la host call.
		$anyPlaying = false;
		foreach($this->clients as $observer){
			if($observer->isPlaying()){
				$anyPlaying = true;
				break;
			}
		}
		if(!$anyPlaying){
			return;
		}
		// Una sola lectura de los jugadores online por tick, no una por
		// observador: con N clientes `getOnlinePlayers()` se hacía N veces.
		$players = $this->plugin->getServer()->getOnlinePlayers();
		$snapshotsByName = [];
		foreach($this->hostPlayerSnapshots() as $snapshot){
			$snapshotsByName[strtolower((string)$snapshot['name'])] = $snapshot;
		}
		$this->refreshBedrockPositions($players, $snapshotsByName);
		foreach($this->clients as $observer){
			if(!$observer->isPlaying()){
				continue;
			}
			try{
				$observer->syncEntities($this, $this->entityTargetsFor($observer, $players, $snapshotsByName));
			}catch(\Throwable $e){
				// Un fallo en un observador no detiene a los demás.
				continue;
			}
		}
	}

	/**
	 * Objetivos de la TAB para cualquier observador: TODOS los
	 * jugadores online (Bedrock + Java, incluido el propio observador),
	 * sin puerta de chunks ni exclusión de sí mismo. A diferencia de
	 * `entityTargetsFor()`, estar lejos no saca a nadie de la lista.
	 *
	 * @param array<string, \pocketmine\player\Player>|null $players lista de un tick
	 * @return array<string, array{name:string,gamemode:int,ping:int}>
	 */
	public function tabTargets(?array $players = null) : array{
		$targets = [];
		$players ??= $this->plugin->getServer()->getOnlinePlayers();
		foreach($players as $player){
			$name = $player->getName();
			$gamemode = method_exists($player, 'getGamemode') ? (int)$player->getGamemode() : $this->gamemode();
			$ping = method_exists($player, 'getPing') ? (int)$player->getPing() : 0;
			$targets[strtolower($name)] = [
				'name' => $name,
				'gamemode' => max(0, min(3, $gamemode)),
				'ping' => max(0, $ping),
			];
		}
		// getOnlinePlayers ya incluye a los Java (addPlayer), pero si el
		// runtime no expone addPlayer se añaden los clientes playing
		// (mismo fallback que `getOnlinePlayerNames()`), para que el
		// propio jugador aparezca en su TAB aunque `joinApi` falle.
		foreach($this->clients as $client){
			if(!$client->isPlaying() || $client->getUsername() === null){
				continue;
			}
			$lower = strtolower($client->getUsername());
			if(!isset($targets[$lower])){
				$targets[$lower] = [
					'name' => $client->getUsername(),
					'gamemode' => max(0, min(3, $client->getGamemode())),
					'ping' => 0,
				];
			}
		}
		return $targets;
	}

	/**
	 * Sincroniza la TAB para cada cliente Java en play (una vez por tick).
	 *
	 * Detecta por diff contra `getOnlinePlayers()` a los Bedrock que
	 * entran/salen (el puente no recibe su evento) y a los Java que
	 * entran/salen por este mismo listener: el que acabe de entrar ya
	 * recibió su lista completa en `enterPlay*`, así que el tick solo
	 * avisa a los demás del nuevo; el que salió se retira a los
	 * restantes en el mismo tick de su desconexión.
	 */
	private function tickTab() : void{
		$players = $this->plugin->getServer()->getOnlinePlayers();
		$targets = $this->tabTargets($players);
		foreach($this->clients as $observer){
			if(!$observer->isPlaying()){
				continue;
			}
			try{
				$observer->syncTab($targets);
			}catch(\Throwable $e){
				continue;
			}
		}
	}

	public function log(string $message) : void{
		$this->plugin->getLogger()->info($message);
	}

	/**
	 * Presupuesto de altura disponible en este tick.
	 *
	 * El bucle de ticks del servidor espera la respuesta de PHP, así que todo
	 * lo que tarde este `tick()` se le resta al servidor: nunca se debe
	 * consultar más de unos pocos milisegundos de terreno por tick.
	 */

	/** Acepta conexiones nuevas y atiende a las existentes. */
	public function tick() : void{
		if(!$this->running || $this->socket === null){
			return;
		}

		$this->tickCount++;
		$this->playerGetMemo = [];
		$this->inServerTick = true;
		$tickStarted = microtime(true);
		// Deadline compartido entre clientes: el chunk más caro (generación
		// en el host) puede superar el presupuesto por sí solo, así que
		// tras el deadline ningún cliente arranca columnas nuevas en este
		// tick en vez de sumar presupuestos por cliente.
		$deadline = $tickStarted + self::TICK_BUDGET_SECONDS;

		while(($stream = @stream_socket_accept($this->socket, 0)) !== false){
			stream_set_blocking($stream, false);
			$peer = (string)@stream_socket_get_name($stream, true);
			if($peer === ''){
				$peer = 'desconocido';
			}
			$this->clients[] = new JavaClient($stream, $peer);
			$this->log("Conexión Java entrante desde {$peer}");
		}

		foreach(array_keys($this->clients) as $index){
			$client = $this->clients[$index] ?? null;
			if($client === null){
				continue;
			}

			$alive = true;
			try{
				$alive = $client->poll($this);
				if($alive){
					$client->tick($this, $deadline);
				}
			}catch(\Throwable $e){
				$this->log("Error atendiendo a " . $client->getPeer() . ": " . $e->getMessage());
				$alive = false;
			}

			if(!$alive){
				$this->onDisconnect($client);
				unset($this->clients[$index]);
			}
		}
		$this->clients = array_values($this->clients);
		// TAB antes que entidades: así el spawn ve a los jugadores ya
		// anunciados en la lista y no repite su `player_info` add. Es
		// solo memoria (sin host call), así que corre aunque la deadline
		// ya haya pasado.
		$this->tickTab();
		// Deadline real para las fases pesadas: una columna generada
		// puede soltar la deadline por sí sola (~18 ms); las fases
		// siguientes son host calls síncronas (~15 ms cada una en
		// Windows) y sin esta guarda sumaban 30 ms por tick aunque no
		// quedara terreno pendiente. Si ya pasó la deadline, se saltan
		// este tick y se recuperan en el siguiente (su gating por
		// intervalo las reprograma solo).
		if(microtime(true) <= $deadline){
			$this->tickTimeSync();
			// Entidades y cambios de bloques van en intervalos: cada una
			// es una host call síncrona por el pipe IPC y a 20 Hz sumaban
			// varias decenas de ms por tick con un Java conectado
			// (TPS ~17 parado). A 5/10 Hz el coste medio por tick cae
			// a unos pocos ms y el movimiento/entidades siguen
			// viéndose fluidos. Los tests llaman a `tickEntities()` /
			// `tickBlockChanges()` directamente y no pasan por este gating.
			if(($this->tickCount % self::ENTITY_SYNC_INTERVAL_TICKS) === 0){
				$this->tickEntities();
			}
			// Fase 1 (Bedrock -> Java): drena la cola del host en intervalo
			// (ver `tickBlockChanges()`); sin Java en play ni siquiera se
			// sondea, para no pagar la host call en vano cada tick.
			if(($this->tickCount % self::BLOCK_CHANGES_INTERVAL_TICKS) === 0){
				$this->tickBlockChanges();
			}
		}
		$this->inServerTick = false;
		$this->playerGetMemo = [];

		// Un tick largo retrasa el resto del servidor; se avisa para poder
		// diagnosticar sin capturar perfiles. En uso normal el tick del
		// puente queda por debajo de ~12 ms (sin columnas pendientes, los
		// getters y el sync de entidades son de memoria), así que el aviso
		// no aparece: sólo salta durante el streaming inicial de chunks.
		$elapsed = microtime(true) - $tickStarted;
		if($elapsed * 1000.0 > self::SLOW_TICK_LOG_MS
			&& microtime(true) - $this->lastSlowTickLog > self::SLOW_TICK_LOG_INTERVAL_SECONDS){
			$this->lastSlowTickLog = microtime(true);
			$this->log(sprintf(
				'Tick del puente Java lento: %.0f ms (%d chunks en caché)',
				$elapsed * 1000.0,
				count($this->chunkDataCache)
			));
		}
	}

	private function onDisconnect(JavaClient $client) : void{
		$name = $client->getUsername();
		// `close()`/`kick()` dejan de estar "playing", así que el estado se
		// captura antes: un kick en play también debe salir de la API.
		$wasPlaying = $client->isPlaying();
		$wasJoined = $client->apiJoined;
		$client->close();
		if($name === null || (!$wasPlaying && !$wasJoined)){
			return;
		}
		// Otro cliente Java con el mismo nombre sigue en play: el registro
		// es suyo, no se borra el alta ajena en una desconexión breve.
		foreach($this->clients as $other){
			if($other === $client){
				continue;
			}
			$otherName = $other->getUsername();
			if($otherName !== null && strcasecmp($otherName, $name) === 0 && $other->isPlaying()){
				return;
			}
		}
		// Sale de la API como un jugador Bedrock: `PlayerQuitEvent` y su mensaje.
		$this->leaveApi($client);
	}

	/** Nombres de los jugadores Java conectados. */
	public function javaPlayerNames() : array{
		$names = [];
		foreach($this->clients as $client){
			if($client->isPlaying() && $client->getUsername() !== null){
				$names[] = $client->getUsername();
			}
		}
		return $names;
	}

	public function onlineCount() : int{
		// Los jugadores Java ya están en el registro del servidor.
		return count($this->plugin->getServer()->getOnlinePlayers());
	}

	/**
	 * Reserva el nombre para un cliente Java.
	 *
	 * Los dos protocolos comparten espacio de nombres porque comparten chat,
	 * `/list` y (a futuro) entidades.
	 */
	public function registerPlayer(JavaClient $client, string $name) : bool{
		foreach($this->clients as $other){
			if($other === $client){
				continue;
			}
			$otherName = $other->getUsername();
			if($otherName !== null && strcasecmp($otherName, $name) === 0){
				return false;
			}
		}
		return $this->plugin->getServer()->getPlayerExact($name) === null;
	}

	public function statusJson(?int $protocol = null) : string{
		$profile = $protocol === null ? null : ModernProfile::forProtocol($protocol);
		$versionName = $profile?->name
			?? ($protocol === null || $protocol === Protocol::PROTOCOL_VERSION
				? Protocol::VERSION_NAME
				: Protocol::VERSION_NAME . ' / ' . implode(' / ', ModernProfile::VERSION_NAMES));
		$versionProtocol = $profile?->protocol ?? $protocol ?? Protocol::PROTOCOL_VERSION;

		// getOnlinePlayers ya incluye a los Java (addPlayer): no se duplican.
		$sample = [];
		foreach($this->plugin->getServer()->getOnlinePlayers() as $player){
			$sample[] = [
				'name' => $player->getName(),
				'id' => Protocol::uuidString(Protocol::offlineUuid($player->getName())),
			];
		}

		$payload = [
			'version' => [
				'name' => $versionName,
				'protocol' => $versionProtocol,
			],
			'players' => [
				'max' => $this->maxPlayers(),
				'online' => $this->onlineCount(),
				'sample' => array_slice($sample, 0, 10),
			],
			'description' => [
				'text' => $this->plugin->getJavaMotd(),
			],
		];

		$encoded = json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE);
		return $encoded === false ? '{}' : $encoded;
	}

	public function maxPlayers() : int{
		return (int)$this->plugin->getServer()->getMaxPlayers();
	}

	/**
	 * Punto de aparición, idéntico al de un jugador Bedrock.
	 *
	 * Se usa el `player_position()` del generador (centro del bloque y altura
	 * de los ojos), que es exactamente lo que el servidor Bedrock manda en
	 * `start_game`: así un jugador Java y uno Bedrock aparecen en el mismo
	 * punto, no uno encima del bloque y otro 1,62 bloques más arriba. Si el
	 * puente no expone ese dato se aproxima con el spawn del mundo.
	 *
	 * @return array{0:float,1:float,2:float}
	 */
	public function spawn() : array{
		$world = $this->world();
		if(method_exists($world, 'getPlayerSpawnLocation')){
			$player = $world->getPlayerSpawnLocation();
			if(is_array($player) && count($player) === 3){
				return [(float)$player[0], (float)$player[1], (float)$player[2]];
			}
		}

		$spawn = $world->getSpawnLocation();
		return [
			(float)$spawn->getX() + 0.5,
			(float)$spawn->getY() + 1.62,
			(float)$spawn->getZ() + 0.5,
		];
	}

	public function gamemode() : int{
		return $this->plugin->getJavaGamemode();
	}

	public function difficulty() : int{
		return $this->plugin->getJavaDifficulty();
	}

	public function levelType() : string{
		return $this->plugin->getJavaLevelType();
	}

	public function worldTime() : int{
		return (int)$this->world()->getTime();
	}

	/**
	 * ¿Avanza el reloj del mundo? (`/time start`/`stop`).
	 *
	 * Va como `tickDayTime` (769-774) / `rate` (775) en
	 * `update_time`: con false el cliente Java congela el sol en
	 * vez de seguir avanzándolo. Sin runtime con `isTimeRunning()`
	 * (stubs de tests) se asume que corre, como en Rust por defecto.
	 */
	public function worldTimeRunning() : bool{
		$world = $this->world();
		if(method_exists($world, 'isTimeRunning')){
			try{
				return (bool)$world->isTimeRunning();
			}catch(\Throwable $e){
				return true;
			}
		}
		return true;
	}

	/**
	 * Reenvía la hora del mundo a todos los Java en play.
	 *
	 * Antes `update_time` solo salía al entrar (y en moderno ni
	 * eso), así que tras horas Bedrock y Java divergían y
	 * `/time set` en Bedrock nunca llegaba a Java. Ahora, una vez
	 * por tick: se lee la hora compartida (`worldTime()`, la misma
	 * que avanza Rust en `tick_world_time`) y se broadcast cada
	 * `TIME_SYNC_INTERVAL_TICKS` ticks (1 s), o en el siguiente
	 * tick si la hora saltó más que el avance normal de un tick
	 * (`/time set/add` o `setTime()` de un plugin, que llegan por
	 * el host sin pasar por este broadcast).
	 */
	private function tickTimeSync() : void{
		$anyPlaying = false;
		foreach($this->clients as $client){
			if($client->isPlaying()){
				$anyPlaying = true;
				break;
			}
		}
		if(!$anyPlaying){
			return;
		}
		$this->ticksSinceTimeSync++;
		// `worldTime()` es una host call síncrona (~15 ms): leerla cada
		// tick solo para detectar saltos (`/time set`) es caro con un
		// Java en play. Entre broadcasts se lee cada 5 ticks (salto
		// detectado en ≤250 ms); el broadcast sigue cada 20 ticks o al
		// saltar, igual que antes.
		if($this->lastBroadcastTime !== null
			&& $this->ticksSinceTimeSync < self::TIME_SYNC_INTERVAL_TICKS
			&& ($this->ticksSinceTimeSync % 5) !== 0){
			return;
		}
		try{
			$time = $this->worldTime();
		}catch(\Throwable $e){
			return;
		}
		$jumped = $this->lastBroadcastTime !== null
			&& abs($time - $this->lastBroadcastTime) > self::TIME_SYNC_INTERVAL_TICKS;
		// `/time stop`/`start` no cambian la hora, solo `time_running`:
		// se comprueba en cada lectura (cada 5 ticks) para congelar /
		// reanudar el sol sin esperar al broadcast de 1 s.
		$running = $this->worldTimeRunning();
		$runningChanged = $this->lastBroadcastTimeRunning !== null
			&& $running !== $this->lastBroadcastTimeRunning;
		if($this->lastBroadcastTime !== null
			&& !$jumped
			&& !$runningChanged
			&& $this->ticksSinceTimeSync < self::TIME_SYNC_INTERVAL_TICKS){
			return;
		}
		foreach($this->clients as $client){
			if(!$client->isPlaying()){
				continue;
			}
			try{
				$client->sendTime($time, $running);
			}catch(\Throwable $e){
				continue;
			}
		}
		$this->lastBroadcastTime = $time;
		$this->lastBroadcastTimeRunning = $running;
		$this->ticksSinceTimeSync = 0;
	}

	/**
	 * Traduce la rotura de un bloque al mundo Bedrock compartido.
	 *
	 * `Block::setBlock(0)` escribe el id de red de aire mediante la host call
	 * `world.set_block`, así que el cambio es real para los clientes Bedrock.
	 */
	public function breakBlock(int $x, int $y, int $z) : bool{
		// Invalidar los cachés de chunk para que el cliente Java reciba
		// la edición aplicada al mundo Bedrock y no una copia obsoleta.
		$this->invalidateChunkCachesFor($x, $z);

		$block = $this->world()->getBlockAt($x, $y, $z);
		if($block->getId() === 0){
			return false;
		}
		return $block->setBlock(0);
	}

	/** Invalida los cachés de chunk (legacy, moderno y columna) de una coordenada. */
	private function invalidateChunkCachesFor(int $x, int $z) : void{
		$chunkX = (int)floor($x / 16);
		$chunkZ = (int)floor($z / 16);
		$key = $chunkX . ':' . $chunkZ;
		unset($this->chunkDataCache[$key]);
		unset($this->chunkCache[$key]);
		foreach(array_keys($this->modernChunkCache) as $cacheKey){
			if(str_ends_with($cacheKey, ':' . $key)){
				unset($this->modernChunkCache[$cacheKey]);
			}
		}
	}

	/**
	 * Fase 2 (Java -> Bedrock): coloca un bloque en el mundo compartido.
	 *
	 * `$blockId` admite dos representaciones (ambas las usa `JavaClient`):
	 *  - id legacy (Java 1.12 / PM, <= 1000), el que espera
	 *    `Block::setBlock()`: el held item del paquete legacy;
	 *  - id de red de Bedrock (> 1000, hash FNV-1a de `block_id.rs`): el
	 *    estado traducido del paquete moderno. Se convierte a legacy con
	 *    `ChunkBuilder::legacyIdForBedrock()` / `ModernChunk::blockName()`.
	 *
	 * Valida alcance (<= 6 bloques desde el jugador, como el picado) y
	 * que la celda esté vacía (aire). Escribe con el mismo `Block` que
	 * `breakBlock()` (así los plugins y Bedrock ven la edición), invalida
	 * los cachés de chunk y avisa a los Java con el chunk ya enviado
	 * (incluido el que coloca: su predicción coincide con el estado real).
	 * El llamador (`JavaClient`) reenvía el estado real si devuelve false
	 * para deshacer la predicción del cliente.
	 */
	public function placeBlock(JavaClient $client, int $x, int $y, int $z, int $blockId) : bool{
		if($blockId <= 0){
			return false;
		}
		if($y < -64 || $y > 319){
			return false;
		}
		// Alcance: mismo criterio que `handleBlockDig()`.
		if(abs($x + 0.5 - $client->getX()) > 6 || abs($y + 0.5 - $client->getY()) > 6 || abs($z + 0.5 - $client->getZ()) > 6){
			return false;
		}

		$legacyId = null;
		$networkId = null;
		if($blockId > 1000){
			$networkId = $blockId;
			$legacyId = ChunkBuilder::legacyIdForBedrock($networkId);
			if($legacyId === null){
				$name = ModernChunk::blockName($networkId);
				if($name !== null){
					$legacyId = ChunkBuilder::legacyIdForName($name);
				}
			}
		}else{
			$legacyId = $blockId;
			$networkId = ChunkBuilder::bedrockIdForLegacy($legacyId);
		}

		$block = $this->world()->getBlockAt($x, $y, $z);
		$currentNetwork = method_exists($block, 'getNetworkId') ? (int)$block->getNetworkId() : null;
		$currentLegacy = method_exists($block, 'getId') ? (int)$block->getId() : 0;
		// Vacío = aire de red (3690217760) o legacy 0. Ojo: en el runtime
		// real `getId()` de un hash desconocido puede no ser 0, por eso
		// manda el id de red cuando está disponible.
		$isEmpty = $currentNetwork !== null ? $currentNetwork === 3690217760 : $currentLegacy === 0;
		if(!$isEmpty){
			return false;
		}

		$this->invalidateChunkCachesFor($x, $z);

		if($legacyId === null || $legacyId <= 0){
			// Bloque sin traducción legacy conocida: se escribe el id de
			// red tal cual por host para que el edit quede en `world.json`
			// (Bedrock puede dibujarlo mal hasta tener su hash canónico,
			// pero no se pierde la colocación).
			if($networkId === null){
				return false;
			}
			$result = self::host('world.set_block', ['world' => $this->world()->getName(), 'x' => $x, 'y' => $y, 'z' => $z, 'block_id' => $networkId]);
			$ok = is_array($result) && ($result['ok'] ?? false) === true;
			if($ok){
				$this->broadcastBlockChange($x, $y, $z, '', $networkId);
			}
			return $ok;
		}

		$ok = $block->setBlock($legacyId);
		if($ok){
			// `networkId` ya es el estado real (o se re-lee en broadcast si
			// no se pudo derivar); `sendBlockChange()` lo mapea con
			// `ModernChunk::bedrockState()` / `ChunkBuilder::javaState()`.
			$this->broadcastBlockChange($x, $y, $z, '', $networkId);
		}
		return $ok;
	}

	public function buildChunk(int $chunkX, int $chunkZ) : ?array{
		$key = $chunkX . ':' . $chunkZ;
		if(isset($this->chunkCache[$key])){
			return $this->chunkCache[$key];
		}

		$source = $this->worldChunk($chunkX, $chunkZ);
		if($source === null){
			return null;
		}

		$chunk = ChunkBuilder::fromChunk($source);
		if(count($this->chunkCache) >= self::CHUNK_CACHE_LIMIT){
			array_shift($this->chunkCache);
		}
		$this->chunkCache[$key] = $chunk;
		return $chunk;
	}

	/**
	 * Columna de chunk 1.18+ para un cliente moderno.
	 *
	 * @return array{data:string, heightmaps:string, light:string}
	 */
	public function buildModernChunk(ModernProfile $profile, int $chunkX, int $chunkZ) : ?array{
		$key = $profile->protocol . ':' . $chunkX . ':' . $chunkZ;
		if(isset($this->modernChunkCache[$key])){
			return $this->modernChunkCache[$key];
		}

		$source = $this->worldChunk($chunkX, $chunkZ);
		if($source === null){
			return null;
		}

		// Ojo: `fromChunk()` solo rellena el informe si se le pasa un array
		// (con null nunca se llena y el log de agua de abajo no saldría).
		$fluidReport = [];
		$chunk = [
			'data' => ModernChunk::fromChunk($source, $profile, $fluidReport),
			'heightmaps' => ModernChunk::heightmaps(ModernChunk::heightsFromChunk($source), $profile),
			'light' => ModernChunk::light(),
		];
		// Log de depuración: una vez por protocolo, el primer chunk con fluido.
		// `fluidCount > 0` es lo que activa hasFluid() en el cliente 26.1 y le
		// permite narrar; con 0 el jugador cae por el agua como si fuera aire.
		// Verificable sin cliente: `php tests/real_column_test.php` por columna.
		$fluidSections = array_filter($fluidReport ?? [], static fn(int $n) : bool => $n > 0);
		if($fluidSections !== [] && !isset($this->loggedWaterChunk[$profile->protocol])){
			$this->loggedWaterChunk[$profile->protocol] = true;
			$this->log(sprintf(
				'Agua Java (%s): chunk %d,%d tiene fluido (nadar activo). fluidCount por seccion: %s',
				$profile->name, $chunkX, $chunkZ,
				implode(', ', array_map(static fn(int $sec, int $n) : string => "{$sec}={$n}", array_keys($fluidSections), $fluidSections))
			));
		}
		if(count($this->modernChunkCache) >= self::MODERN_CHUNK_CACHE_LIMIT){
			array_shift($this->modernChunkCache);
		}
		$this->modernChunkCache[$key] = $chunk;
		return $chunk;
	}

	/**
	 * Columna real del mundo Bedrock, tal y como la sirve la API del puente.
	 *
	 * `world.get_chunk` devuelve la columna ya generada (subsecciones
	 * paletizadas en formato de red), con las roturas y colocaciones de los
	 * jugadores aplicadas. Es una host call por chunk, comparada con las 256
	 * consultas de altura que hacía falta antes para adivinar el terreno.
	 */
	private function worldChunk(int $chunkX, int $chunkZ) : ?object{
		$key = $chunkX . ':' . $chunkZ;
		if(array_key_exists($key, $this->chunkDataCache)){
			return $this->chunkDataCache[$key];
		}

		$world = $this->world();
		if(!method_exists($world, 'getChunk')){
			if(!$this->chunkApiMissing){
				$this->chunkApiMissing = true;
				$this->log('La API del puente no expone World::getChunk(); no se pueden enviar chunks.');
			}
			return null;
		}

		try{
			$chunk = $world->getChunk($chunkX, $chunkZ);
		}catch(\Throwable $e){
			$this->log("Fallo leyendo el chunk {$key}: " . $e->getMessage());
			return null;
		}
		if($chunk === null){
			return null;
		}

		if(count($this->chunkDataCache) >= self::CHUNK_DATA_CACHE_LIMIT){
			array_shift($this->chunkDataCache);
		}
		$this->chunkDataCache[$key] = $chunk;
		return $chunk;
	}

	private function world() : \pocketmine\world\World{
		return $this->plugin->getServer()->getDefaultWorld();
	}

	// ------------------------------------------------------------------
	// Chat
	// ------------------------------------------------------------------

	/** Envía una línea a todos los clientes Java dentro del juego. */
	public function broadcastChat(string $line) : void{
		foreach($this->clients as $client){
			if($client->isPlaying()){
				$client->sendChat($line);
			}
		}
	}

	public function broadcastBlockChange(int $x, int $y, int $z, string $kind = '', ?int $networkId = null) : void{
		// Estado real del mundo (`world.get_block`): no aire/piedra de mentira.
		// Si el llamador ya trae el id de red (cola del host, `placeBlock`),
		// no se re-lee: es el estado que acaba de escribirse.
		if($networkId === null){
			try{
				$block = $this->world()->getBlockAt($x, $y, $z);
				if(method_exists($block, 'getNetworkId')){
					$networkId = (int)$block->getNetworkId();
				}elseif(method_exists($block, 'getId')){
					// Sin `getNetworkId()` (stubs de tests) el legacy sí
					// identifica el bloque: se deriva su red para el mapeo
					// real y para el fallback por polling.
					$networkId = ChunkBuilder::bedrockIdForLegacy((int)$block->getId());
				}
			}catch(\Throwable $e){
				// Sin runtime el `kind` heredado sigue valiendo como fallback.
			}
		}
		$this->noteKnownBlock($x, $y, $z, $networkId);

		// Puerta como las entidades (`entityTargetsFor()`): sin chunk
		// enviado no hay `block_change`; cuando el chunk se envíe ya
		// llevará el bloque nuevo porque se invalidaron los cachés.
		$chunkX = (int)floor($x / 16);
		$chunkZ = (int)floor($z / 16);
		foreach($this->clients as $client){
			if(!$client->isPlaying() || !$client->hasSentChunk($chunkX, $chunkZ)){
				continue;
			}
			try{
				// `sendBlockChange()` mapea el id real con
				// `ModernChunk::bedrockState()` (moderno) o
				// `ChunkBuilder::javaState()` (legacy); el `kind` solo es
				// fallback si no hay red.
				$client->sendBlockChange($x, $y, $z, $kind, $networkId);
			}catch(\Throwable $e){
				continue;
			}
		}
	}

	/** Actualiza la caché de bloques conocidos (con tope). */
	private function noteKnownBlock(int $x, int $y, int $z, ?int $networkId) : void{
		$this->lastKnownBlocks[$x . ',' . $y . ',' . $z] = $networkId;
		if(count($this->lastKnownBlocks) > self::LAST_KNOWN_BLOCKS_LIMIT){
			array_shift($this->lastKnownBlocks);
		}
	}

	// ------------------------------------------------------------------
	// Fase 1: Bedrock -> Java (cola del host + fallback por polling)
	// ------------------------------------------------------------------

	/**
	 * Consume los cambios de bloques de Bedrock una vez por tick.
	 *
	 * Camino feliz: `world.take_block_changes()` (alias
	 * `world.drain_changes`) devuelve la lista de ediciones desde el
	 * último drenaje y se reenvían a los Java con el chunk ya enviado.
	 * Es la única host call de este camino por drenaje (en `tick()` se
	 * llama cada `BLOCK_CHANGES_INTERVAL_TICKS` ticks, no cada tick).
	 *
	 * La cola está implementada en Rust (`plugin_host.rs`,
	 * `world.take_block_changes`). Si la sonda devuelve null, el binario
	 * en ejecución es anterior a esa implementación (o el IPC falló):
	 * se activa el fallback por polling sobre los bloques ya conocidos,
	 * reintentando la sonda cada 100 ticks. El fallback solo detecta
	 * cambios en coordenadas que ya se enviaron a los Java; con la cola
	 * activa se cubre todo el mundo. Ver comentario de
	 * `BLOCK_CHANGES_METHODS` para la lista de comprobación.
	 */
	private function tickBlockChanges() : void{
		// Sin Java en play no hay a quién reenviar los cambios: no se
		// paga la host call de drenaje (era 1 por tick siempre, también
		// sin ningún Java conectado, y ~15 ms de los ~15 ms del tick
		// del puente en vacío medidos en producción).
		$anyPlaying = false;
		foreach($this->clients as $client){
			if($client->isPlaying()){
				$anyPlaying = true;
				break;
			}
		}
		if(!$anyPlaying){
			return;
		}
		$changes = null;
		$shouldProbe = !$this->blockChangesApiMissing
			|| $this->ticksSinceBlockChangesProbe >= self::BLOCK_CHANGES_PROBE_INTERVAL;
		if($shouldProbe){
			$changes = $this->probeBlockChangesHost();
			$this->ticksSinceBlockChangesProbe = 0;
			if($changes === null){
				if(!$this->blockChangesApiMissing){
					$this->blockChangesApiMissing = true;
					$this->log('La API del puente aún no expone world.take_block_changes (ni world.drain_changes); cambios Bedrock->Java por polling de bloques conocidos. Si Rust ya la implementó, recompila y reinicia el servidor completo (el host se compila en server-core.exe y el worker PHP es hijo de ese proceso: no hay reload en caliente).');
				}
			}else{
				$this->blockChangesApiMissing = false;
			}
		}else{
			$this->ticksSinceBlockChangesProbe++;
		}

		if($changes !== null){
			foreach($changes as $change){
				$normalized = self::normalizeBlockChange($change);
				if($normalized === null){
					continue;
				}
				[$x, $y, $z, $networkId, $kind] = $normalized;
				$this->invalidateChunkCachesFor($x, $z);
				$key = $x . ',' . $y . ',' . $z;
				// Eco de nuestro propio `placeBlock`/`breakBlock`: ya se
				// anunció al colocar/romper en el mismo tick.
				if($networkId !== null && ($this->lastKnownBlocks[$key] ?? null) === $networkId){
					continue;
				}
				$this->broadcastBlockChange($x, $y, $z, $kind, $networkId);
			}
			return;
		}

		$this->pollKnownBlocks();
	}

	/**
	 * Prueba la cola del host con ambos nombres.
	 *
	 * @return ?list<array> lista de cambios (vacía si no hay ninguno) o
	 *                       null si el host no implementa ninguno
	 */
	private function probeBlockChangesHost() : ?array{
		foreach(self::BLOCK_CHANGES_METHODS as $method){
			$result = self::host($method, []);
			if(!is_array($result)){
				continue;
			}
			// Formas aceptadas: ['changes' => [...]], ['block_changes' => ...]
			// o la lista directamente. `['ok' => true]` sin cambios (stub
			// viejo) NO cuenta como éxito: es el "no implementado".
			$list = $result['changes'] ?? $result['block_changes'] ?? null;
			if(is_array($list)){
				return array_values($list);
			}
			if($result !== [] && array_is_list($result)){
				return array_values($result);
			}
		}
		return null;
	}

	/**
	 * Normaliza una edición de la cola a `[x, y, z, ?networkId, kind]`.
	 *
	 * Acepta las claves que Rust puede usar: `block_id`/`network_id`/
	 * `blockId`/`networkId`/`id`/`state` para el id de red (el mismo que
	 * devuelve `world.get_block` como `id`), y `kind`/`type` como nombre
	 * de fallback. Sin id de red, `broadcastBlockChange()` lo re-lee del
	 * mundo.
	 *
	 * @return ?array{0:int,1:int,2:int,3:?int,4:string}
	 */
	private static function normalizeBlockChange(mixed $change) : ?array{
		if(!is_array($change)){
			return null;
		}
		$x = $change['x'] ?? $change[0] ?? null;
		$y = $change['y'] ?? $change[1] ?? null;
		$z = $change['z'] ?? $change[2] ?? null;
		if(!is_numeric($x) || !is_numeric($y) || !is_numeric($z)){
			return null;
		}
		$networkId = $change['block_id'] ?? $change['network_id'] ?? $change['blockId']
			?? $change['networkId'] ?? $change['id'] ?? $change['state'] ?? null;
		$kind = $change['kind'] ?? $change['type'] ?? '';
		return [
			(int)$x, (int)$y, (int)$z,
			is_numeric($networkId) ? (int)$networkId : null,
			is_string($kind) ? $kind : '',
		];
	}

	/**
	 * Fallback por polling: compara `world.get_block` con la caché de
	 * bloques conocidos y reenvía lo que haya cambiado por fuera.
	 *
	 * Acotado a `FALLBACK_POLL_PER_TICK` coordenadas por tick (cada una es
	 * una host call síncrona) con cursor rotativo, para no romper el
	 * presupuesto de 10 ms del tick cuando hay muchos bloques conocidos.
	 */
	private function pollKnownBlocks() : void{
		if($this->lastKnownBlocks === []){
			return;
		}
		$playing = false;
		foreach($this->clients as $client){
			if($client->isPlaying()){
				$playing = true;
				break;
			}
		}
		if(!$playing){
			return;
		}

		$keys = array_keys($this->lastKnownBlocks);
		$count = count($keys);
		$budget = min(self::FALLBACK_POLL_PER_TICK, $count);
		for($i = 0; $i < $budget; $i++){
			$key = (string)$keys[($this->fallbackPollCursor + $i) % $count];
			$parts = explode(',', $key);
			if(count($parts) !== 3){
				continue;
			}
			[$x, $y, $z] = [(int)$parts[0], (int)$parts[1], (int)$parts[2]];
			try{
				$block = $this->world()->getBlockAt($x, $y, $z);
				$networkId = null;
				if(method_exists($block, 'getNetworkId')){
					$networkId = (int)$block->getNetworkId();
				}elseif(method_exists($block, 'getId')){
					$networkId = ChunkBuilder::bedrockIdForLegacy((int)$block->getId());
				}
			}catch(\Throwable $e){
				continue;
			}
			if($networkId === ($this->lastKnownBlocks[$key] ?? null)){
				continue;
			}
			$this->invalidateChunkCachesFor($x, $z);
			$this->broadcastBlockChange($x, $y, $z, '', $networkId);
		}
		$this->fallbackPollCursor = $count === 0 ? 0 : ($this->fallbackPollCursor + $budget) % $count;
	}

	/**
	 * Anuncia un mensaje a las dos plataformas.
	 *
	 * `broadcastMessage` es la API del servidor: llega a las sesiones Bedrock y
	 * a la bandeja de los jugadores Java, así que no se duplica nada aquí.
	 */
	public function announce(string $message) : void{
		$this->plugin->getServer()->broadcastMessage($message);
	}

	// ------------------------------------------------------------------
	// API de plugins: un jugador Java es un jugador más
	// ------------------------------------------------------------------

	/** Llamada al host del servidor, o null si el runtime no está disponible. */
	private static function host(string $method, array $params = []) : mixed{
		if(!class_exists(\pocketmine\runtime\Bridge::class) || !\pocketmine\runtime\Bridge::isActive()){
			return null;
		}
		try{
			return \pocketmine\runtime\Bridge::hostCall($method, $params);
		}catch(\Throwable $e){
			return null;
		}
	}

	/**
	 * Snapshot del registry para un jugador (`player.get`), con memo
	 * dentro de un único `tick()` del puente: `syncGamemode`,
	 * `syncTeleport` y `hasTeleportPending` pedían el mismo snapshot
	 * hasta 3 veces por tick y cliente (3 host calls ~15 ms cada una
	 * para el mismo dato). Fuera de `tick()` (tests) no hay memo.
	 *
	 * @return ?array<string, mixed>
	 */
	private function playerEntry(string $name) : ?array{
		$lower = strtolower($name);
		if($this->inServerTick && array_key_exists($lower, $this->playerGetMemo)){
			$memo = $this->playerGetMemo[$lower];
			return is_array($memo) ? $memo : null;
		}
		$entry = self::host('player.get', ['name' => $name]);
		$entry = is_array($entry) ? $entry : null;
		if($this->inServerTick){
			$this->playerGetMemo[$lower] = $entry;
		}
		return $entry;
	}

	/** El `Player` de la API para este cliente Java (se crea una vez). */
	public function apiPlayer(JavaClient $client) : ?\pocketmine\player\Player{
		$name = $client->getUsername();
		if($name === null){
			return null;
		}
		if(isset($this->apiPlayers[$name])){
			return $this->apiPlayers[$name];
		}

		try{
			$player = new \pocketmine\player\Player([
				'name' => $name,
				'display_name' => $name,
				'address' => (string)$client->getPeer(),
				'world' => $this->world()->getName(),
				'gamemode' => $this->gamemode(),
				'x' => $client->getX(),
				'y' => $client->getY(),
				'z' => $client->getZ(),
				// La copia API es canónica Bedrock; el cliente va en Java.
				'yaw' => Protocol::javaYawToBedrock($client->getYaw()),
				'pitch' => $client->getPitch(),
			]);
		}catch(\Throwable $e){
			$this->log("No se pudo crear el jugador de la API para {$name}: " . $e->getMessage());
			return null;
		}
		$this->apiPlayers[$name] = $player;
		return $player;
	}

	/**
	 * Da de alta al jugador Java al entrar al mundo.
	 *
	 * Desde aquí `getOnlinePlayers()`, `player.get`, los destinatarios de
	 * `PlayerChatEvent` y las acciones de los plugins (`send_message`,
	 * `broadcast_message`, feedback de comandos) lo tratan como a un jugador
	 * Bedrock; sus líneas llegan a `deliverPluginMessages()`.
	 */
	public function joinApi(JavaClient $client) : void{
		// Una vez por sesión TCP: `enterPlay` y `enterPlayModern` comparten
		// la ruta y un doble `config_finish` no debe re-anunciar el join.
		if($client->apiJoined){
			return;
		}
		$player = $this->apiPlayer($client);
		if($player === null){
			return;
		}

		// `player.load` antes de `player.register`: si hay datos guardados se
		// aplican al `Player` de la API para que el registro los incluya.
		$loaded = $this->loadPlayerData($player->getName());
		if($loaded !== null){
			if(property_exists($player, 'x')){ $player->x = $loaded['x']; }
			if(property_exists($player, 'y')){ $player->y = $loaded['y']; }
			if(property_exists($player, 'z')){ $player->z = $loaded['z']; }
			if(property_exists($player, 'yaw')){ $player->yaw = $loaded['yaw']; }
			if(property_exists($player, 'pitch')){ $player->pitch = $loaded['pitch']; }
			if(method_exists($player, 'setGamemode')){ $player->setGamemode($loaded['gamemode']); $player->gamemode = $loaded['gamemode']; }
			if(property_exists($player, 'health')){ $player->health = $loaded['health']; }
		}

		self::host('player.register', [
			'name' => $player->getName(),
			'display_name' => $player->getDisplayName(),
			'address' => (string)$client->getPeer(),
			'gamemode' => $player->getGamemode(),
			'x' => $player->getX(),
			'y' => $player->getY(),
			'z' => $player->getZ(),
			'yaw' => $player->getYaw(),
			'pitch' => $player->getPitch(),
			'world' => $player->getWorld()->getName(),
			'health' => $player->getHealth(),
			'max_health' => $player->getMaxHealth(),
			'is_op' => $player->isOp(),
		]);

		$server = $this->plugin->getServer();
		if(method_exists($server, 'addPlayer')){
			try{
				$server->addPlayer($player);
			}catch(\Throwable $e){
				$this->log('No se pudo registrar el jugador Java en el servidor: ' . $e->getMessage());
			}
		}

		$client->apiJoined = true;
		$this->announceApi($player, true);
	}

	/** Saca al jugador Java de la API al desconectarse (leaveApi/stop/onDisconnect). */
	public function leaveApi(JavaClient $client) : void{
		$name = $client->getUsername();
		if($name === null){
			return;
		}

		// Otro cliente Java con el mismo nombre sigue en play: conserva el
		// alta y el registro Rust/PHP intactos (reconexión, doble TCP).
		foreach($this->clients as $other){
			if($other === $client){
				continue;
			}
			$otherName = $other->getUsername();
			if($otherName !== null && strcasecmp($otherName, $name) === 0 && $other->isPlaying()){
				return;
			}
		}

		$player = $this->apiPlayers[$name] ?? null;
		unset($this->apiPlayers[$name]);
		$client->apiJoined = false;

		// 1) `player.update_position` final con la última posición conocida
		// (yaw Bedrock canónico, igual que `updatePlayerPosition()`).
		if($player !== null){
			if(property_exists($player, 'x')){ $player->x = $client->getX(); }
			if(property_exists($player, 'y')){ $player->y = $client->getY(); }
			if(property_exists($player, 'z')){ $player->z = $client->getZ(); }
			if(property_exists($player, 'yaw')){ $player->yaw = Protocol::javaYawToBedrock($client->getYaw()); }
			if(property_exists($player, 'pitch')){ $player->pitch = $client->getPitch(); }
		}
		if($client->isClosed() || true){
			self::host('player.update_position', [
				'name' => $name,
				'x' => $client->getX(), 'y' => $client->getY(), 'z' => $client->getZ(),
				'yaw' => Protocol::javaYawToBedrock($client->getYaw()), 'pitch' => $client->getPitch(),
			]);
		}

		// 2) `PlayerQuitEvent` y su mensaje.
		if($player !== null){
			$this->announceApi($player, false);
		}

		// 3) `$player->save()` -> host `player.save` (dispara PlayerDataSaveEvent).
		if($player !== null && method_exists($player, 'save')){
			try{
				$player->save();
			}catch(\Throwable $e){
				$this->log('No se pudo guardar el jugador Java ' . $name . ': ' . $e->getMessage());
			}
		}elseif($player !== null){
			// Fallback si el runtime no expone save(): guarda por host.
			self::host('player.save', [
				'name' => $name,
				'x' => $player->getX(), 'y' => $player->getY(), 'z' => $player->getZ(),
				'yaw' => $player->getYaw(), 'pitch' => $player->getPitch(),
				'gamemode' => $player->getGamemode(),
				'health' => $player->getHealth(), 'max_health' => $player->getMaxHealth(),
				'world' => $player->getWorld()->getName(),
			]);
		}

		// 4) Salida del registro.
		$server = $this->plugin->getServer();
		if($player !== null && method_exists($server, 'removePlayer')){
			try{
				$server->removePlayer($player);
			}catch(\Throwable $e){
				// Ya no estaba registrado.
			}
		}

		self::host('player.unregister', ['name' => $name]);
	}

	/**
	 * Entrada/salida con los mismos eventos que un jugador Bedrock.
	 *
	 * Los plugins ven al jugador Java en `PlayerJoinEvent`/`PlayerQuitEvent` y
	 * el mensaje final se difunde con la API normal, así que lo reciben las dos
	 * plataformas sin que el puente escriba sus propios avisos.
	 */
	private function announceApi(\pocketmine\player\Player $player, bool $join) : void{
		$server = $this->plugin->getServer();
		try{
			$manager = $server->getPluginManager();
		}catch(\Throwable $e){
			return;
		}

		if($join){
			$event = new \pocketmine\event\player\PlayerJoinEvent($player, $player->getName() . ' joined the game');
			$manager->dispatchEvent($event);
			$message = $event->getJoinMessage();
		}else{
			$event = new \pocketmine\event\player\PlayerQuitEvent($player, $player->getName() . ' left the game');
			$manager->dispatchEvent($event);
			$message = $event->getQuitMessage();
		}

		if($message !== ''){
			$server->broadcastMessage($message);
		}
	}

	/** Lo que los plugins le dejaron en la bandeja, escrito en su socket. */
	public function deliverPluginMessages(JavaClient $client) : void{
		$name = $client->getUsername();
		if($name === null){
			return;
		}

		$result = self::host('player.take_messages', ['name' => $name]);
		if(!is_array($result) || !isset($result['messages']) || !is_array($result['messages'])){
			return;
		}

		foreach($result['messages'] as $line){
			if(is_string($line) && $line !== ''){
				$client->sendChat($line);
			}
		}
	}

	/**
	 * Sincroniza el gamemode del cliente Java con el resto del servidor.
	 *
	 * Dos vías (`JavaClient::tick` llama a `syncGamemode` cada 10 ticks,
	 * escalonado de `deliverPluginMessages`/`syncTeleport`):
	 * 1) La copia PHP `apiPlayer` ya cambió (plugin llamó `setGamemode`): sólo
	 *    falta el paquete Java, sin host call. 2) El registry de Rust cambió
	 *    (consola `/gamemode` o Bedrock, `session.rs` actualiza el snapshot
	 *    virtual y `player.get` lo refleja): se actualiza la copia PHP sin
	 *    emitir `set_gamemode` (para no rebotar la acción) y se envía el
	 *    paquete (`JavaClient::setGamemode`).
	 */
	public function syncGamemode(JavaClient $client) : void{
		$name = $client->getUsername();
		if($name === null || !$client->isPlaying()){
			return;
		}
		$player = $this->apiPlayers[$name] ?? null;
		$playerMode = $player !== null && method_exists($player, 'getGamemode') ? (int)$player->getGamemode() : null;
		if($playerMode !== null && $playerMode !== $client->getGamemode()){
			$client->setGamemode($playerMode);
			return;
		}
		$entry = $this->playerEntry($name);
		if(!is_array($entry) || !isset($entry['gamemode'])){
			return;
		}
		$mode = max(0, min(3, (int)$entry['gamemode']));
		if($mode === $client->getGamemode()){
			return;
		}
		if($player !== null && property_exists($player, 'gamemode')){
			$player->gamemode = $mode;
		}
		$client->setGamemode($mode);
	}

	/**
	 * Sincroniza la posición del cliente Java con el resto del servidor
	 * (mismo patrón que `syncGamemode`, llamado desde `JavaClient::tick`
	 * cada 10 ticks, escalonado).
	 *
	 * Referencia: `lastSentPos` (última posición compartida cliente<=>
	 * servidor), no la posición actual del cliente: caminar cambia la
	 * posición del cliente, pero no `lastSentPos`, así que no se confunde
	 * un paseo con un teleport.
	 *
	 * Dos vías: 1) La copia PHP `apiPlayer` cambió (plugin llamó
	 *    `teleport()` o movió `x/y/z` directamente): se teleporta el cliente.
	 * 2) El registry de Rust cambió (consola `/tp` / `/teleport` / `/spawn`
	 *    o `PluginAction::teleport` para un jugador virtual, ambos
	 *    actualizan el snapshot y `player.get` lo refleja): se actualiza
	 *    la copia PHP sin emitir `teleport` (para no rebotar la acción) y se
	 *    envía el paquete (`JavaClient::teleport`).
	 */
	public function syncTeleport(JavaClient $client) : void{
		$name = $client->getUsername();
		if($name === null || !$client->isPlaying()){
			return;
		}
		$last = $client->getLastSentPos();
		if($last === null){
			return;
		}
		$player = $this->apiPlayers[$name] ?? null;
		if($player !== null && method_exists($player, 'getX') && method_exists($player, 'getY') && method_exists($player, 'getZ')){
			$px = (float)$player->getX();
			$py = (float)$player->getY();
			$pz = (float)$player->getZ();
			// La copia API va en yaw Bedrock y `lastSentPos` en yaw Java:
			// se convierte antes de comparar y de teleportar.
			$pyaw = method_exists($player, 'getYaw') ? Protocol::bedrockYawToJava((float)$player->getYaw()) : (float)$last[3];
			$ppitch = method_exists($player, 'getPitch') ? Protocol::bedrockPitchToJava((float)$player->getPitch()) : (float)$last[4];
			if(self::posDiffers($px, $py, $pz, $pyaw, $ppitch, $last)){
				$client->teleport($this, $px, $py, $pz, $pyaw, $ppitch);
				return;
			}
		}
		$entry = $this->playerEntry($name);
		if(!is_array($entry) || !isset($entry['x'], $entry['y'], $entry['z'])){
			return;
		}
		if(($entry['is_virtual'] ?? true) === false){
			return;
		}
		$rx = (float)$entry['x'];
		$ry = (float)$entry['y'];
		$rz = (float)$entry['z'];
		// El registry va en yaw Bedrock: la copia API lo guarda tal cual
		// y al cliente Java se le manda convertido.
		$bedrockYaw = isset($entry['yaw']) ? (float)$entry['yaw'] : Protocol::javaYawToBedrock((float)$last[3]);
		$bedrockPitch = isset($entry['pitch']) ? (float)$entry['pitch'] : (float)$last[4];
		$ryaw = Protocol::bedrockYawToJava($bedrockYaw);
		$rpitch = Protocol::bedrockPitchToJava($bedrockPitch);
		if(!self::posDiffers($rx, $ry, $rz, $ryaw, $rpitch, $last)){
			return;
		}
		if($player !== null){
			if(property_exists($player, 'x')){ $player->x = $rx; }
			if(property_exists($player, 'y')){ $player->y = $ry; }
			if(property_exists($player, 'z')){ $player->z = $rz; }
			if(property_exists($player, 'yaw')){ $player->yaw = $bedrockYaw; }
			if(property_exists($player, 'pitch')){ $player->pitch = $bedrockPitch; }
		}
		$client->teleport($this, $rx, $ry, $rz, $ryaw, $rpitch);
	}

	/**
	 * ¿Hay un teleport del servidor pendiente de aplicar a este cliente?
	 *
	 * Lo usa `maybeUpdatePosition`: si el registry/apiPlayer divergen de
	 * `lastSentPos`, el push del cliente (posición vieja) no debe
	 * sobrescribir el destino del teleport antes de que `syncTeleport`
	 * (cada 10 ticks, escalonado) lo mueva.
	 */
	public function hasTeleportPending(JavaClient $client) : bool{
		$name = $client->getUsername();
		if($name === null || !$client->isPlaying()){
			return false;
		}
		$last = $client->getLastSentPos();
		if($last === null){
			return false;
		}
		$player = $this->apiPlayers[$name] ?? null;
		if($player !== null && method_exists($player, 'getX') && method_exists($player, 'getY') && method_exists($player, 'getZ')){
			$pyaw = method_exists($player, 'getYaw') ? Protocol::bedrockYawToJava((float)$player->getYaw()) : (float)$last[3];
			$ppitch = method_exists($player, 'getPitch') ? Protocol::bedrockPitchToJava((float)$player->getPitch()) : (float)$last[4];
			if(self::posDiffers((float)$player->getX(), (float)$player->getY(), (float)$player->getZ(), $pyaw, $ppitch, $last)){
				return true;
			}
		}
		$entry = $this->playerEntry($name);
		if(!is_array($entry) || !isset($entry['x'], $entry['y'], $entry['z'])){
			return false;
		}
		if(($entry['is_virtual'] ?? true) === false){
			return false;
		}
		$ryaw = isset($entry['yaw']) ? Protocol::bedrockYawToJava((float)$entry['yaw']) : (float)$last[3];
		$rpitch = isset($entry['pitch']) ? Protocol::bedrockPitchToJava((float)$entry['pitch']) : (float)$last[4];
		return self::posDiffers((float)$entry['x'], (float)$entry['y'], (float)$entry['z'], $ryaw, $rpitch, $last);
	}

	/** ¿Difiere `(x,y,z,yaw,pitch)` de `lastSentPos` más que el ruido de andar? */
	private static function posDiffers(float $x, float $y, float $z, float $yaw, float $pitch, array $last) : bool{
		return abs($x - (float)$last[0]) > 0.001
			|| abs($y - (float)$last[1]) > 0.001
			|| abs($z - (float)$last[2]) > 0.001
			|| abs($yaw - (float)$last[3]) > 0.01
			|| abs($pitch - (float)$last[4]) > 0.01;
	}

	/** Chat o comando enviado por un jugador Java. */
	public function handleChat(JavaClient $client, string $message) : void{
		$name = $client->getUsername();
		if($name === null){
			return;
		}

		$message = trim($message);
		if($message === ''){
			return;
		}

		if($message[0] === '/'){
			$this->handleCommand($client, $name, $message);
			return;
		}

		$this->dispatchChat($client, $name, $message);
	}

	/**
	 * Chat de un jugador Java por `PlayerChatEvent`, el mismo evento que usa
	 * Bedrock: los plugins formatean o cancelan el mensaje una sola vez y el
	 * resultado se difunde a las dos plataformas. Sin `format` propio, el
	 * puente usa `<nombre> mensaje`, igual que el servidor.
	 */
	private function dispatchChat(JavaClient $client, string $name, string $message) : void{
		$server = $this->plugin->getServer();
		$player = $this->apiPlayer($client);
		if($player === null){
			$this->announce('<' . $name . '> ' . $message);
			return;
		}

		try{
			$manager = $server->getPluginManager();
			$recipients = array_values($server->getOnlinePlayers());
		}catch(\Throwable $e){
			$this->announce('<' . $name . '> ' . $message);
			return;
		}

		$event = new \pocketmine\event\player\PlayerChatEvent($player, $message, '', $recipients);
		$manager->dispatchEvent($event);
		if($event->isCancelled()){
			return;
		}

		$format = trim($event->getFormat());
		$line = $format === '' ? '<' . $name . '> ' . $event->getMessage() : $format;
		$server->broadcastMessage($line);
	}

	/**
	 * Comandos locales del puente.
	 *
	 * Ejecutar los comandos Bedrock desde Java requiere una forma de enviar el
	 * comando con un `CommandSender` real y de devolver la salida; el puente
	 * PHP todavía no la expone, así que se documenta como pendiente.
	 */
	private function handleCommand(JavaClient $client, string $name, string $line) : void{
		$parts = preg_split('/\s+/', ltrim($line, '/')) ?: [];
		$command = strtolower((string)array_shift($parts));

		switch($command){
			case 'help':
				$client->sendChat("/help, /list, /me <mensaje>, /gamemode <modo> [jugador], /time <set|add|start|stop|query>, /spawn y /tp <x> <y> <z> son locales del puente.");
				$client->sendChat("Los comandos de plugins se ejecutan con la misma API y su respuesta llega al chat; el resto de nativos todavía no.");
				break;

			case 'list':
			case 'players':
				// Misma lista que Bedrock: los jugadores Java están dados de alta
				// en el servidor, así que no hay que unirlos a mano.
				$names = [];
				foreach($this->plugin->getServer()->getOnlinePlayers() as $player){
					$names[] = $player->getName();
				}
				$client->sendChat("Online (" . count($names) . "): " . implode(', ', $names));
				break;

			case 'save-all':
			case 'save_all':
				$result = self::host('server.save_all', []);
				if($result === null || (is_array($result) && !($result['ok'] ?? true))){
					$client->sendChat("No se pudo guardar el servidor.");
				}else{
					$client->sendChat("Servidor guardado.");
				}
				break;

			case 'me':
				$text = implode(' ', $parts);
				if($text === ''){
					$client->sendChat("Uso: /me <mensaje>");
					break;
				}
				$this->announce('* ' . $name . ' ' . $text);
				break;

			case 'spawn':
				$spawn = $this->spawn();
				$client->sendChat("Teleportado al spawn del mundo Bedrock.");
				$this->announce($name . ' volvió al spawn');
				$client->sendChat("Usa /spawn de nuevo tras moverte si tu cliente no se ha recolocado.");
				$this->teleportClient($client, $spawn);
				break;

			case 'tp':
			case 'teleport':
				// Selector-aware `/tp`: `@s`/`@p`/`@a`/`@e`/`@r` (con
				// `distance/name/x/y/z/limit/sort`) resuelven contra
				// `getOnlinePlayers()`, que incluye a los virtuales Java
				// dados de alta en la API. Las coordenadas son las del
				// mundo Bedrock (`/tp` del cliente Bedrock, o las de un
				// jugador Bedrock); el cliente 1.12.2 aplica el
				// desplazamiento de su mundo al recibirlas.
				$this->handleTeleport($client, $name, $parts);
				break;

			case 'gamemode':
			case 'gm':
			case 'gmc':
			case 'gms':
			case 'gma':
			case 'gmsp':
				$this->handleGamemode($client, $name, $command, $parts);
				break;

			case 'time':
				$this->handleTime($client, $name, $parts);
				break;

			default:
				// Comandos de plugins con la API del servidor: mismo remitente,
				// mismos permisos y la respuesta sale por la bandeja del jugador.
				$player = $this->apiPlayer($client);
				$line = '/' . trim($command . ($parts === [] ? '' : ' ' . implode(' ', $parts)));
				$handled = false;
				if($player !== null){
					try{
						$handled = $this->plugin->getServer()->getCommandMap()->dispatch($player, $line);
					}catch(\Throwable $e){
						$handled = false;
					}
				}
				if(!$handled){
					$client->sendChat("Comando no disponible desde Java: /" . $command);
				}
				break;
		}
	}

	/** @param array{0:float,1:float,2:float} $spawn */
	private function teleportClient(JavaClient $client, array $spawn) : void{
		$client->teleport($this, $spawn[0], $spawn[1], $spawn[2]);
	}

	/**
	 * `/tp` / `/teleport` local del puente con selectores.
	 *
	 * Formas (mismo criterio de conteo que el core Rust `teleport()`):
	 * - `/tp <jugador>` (1): el remitente va a ese jugador (`@p`/`@r`/...).
	 * - `/tp <objetivo> <jugador>` (2): cada objetivo va al destino.
	 * - `/tp <x> <y> <z>` (3) y `/tp <x> <y> <z> <yaw> <pitch>` (5):
	 *   el remitente va a las coordenadas (`~` relativo al remitente).
	 * - `/tp <objetivo> <x> <y> <z>` (4) y con `<yaw> <pitch>` (6):
	 *   cada objetivo va a las coordenadas (`~` relativo a cada objetivo).
	 *
	 * Los objetivos `@a`/`@e`/`@p`/`@r`/`@n`/`@c`/`@s` (con `[...]`:
	 * `distance/name/type/x/y/z/limit/sort/gamemode`) resuelven contra
	 * `getOnlinePlayers()`, que ya incluye a los virtuales Java dados de
	 * alta en la API. Cada objetivo se teleporta y se avisa con un
	 * feedback por jugador.
	 *
	 * @param list<string> $parts
	 */
	private function handleTeleport(JavaClient $client, string $senderName, array $parts) : void{
		// Las partes llegaron separadas por espacios: si un selector trae
		// espacios dentro de `[...]`, se vuelven a unir.
		$parts = $this->mergeSelectorTokens($parts);
		$count = count($parts);
		$players = array_values($this->plugin->getServer()->getOnlinePlayers());
		if($count === 0){
			$client->sendChat("Uso: /tp <x> <y> <z> | /tp <jugador> | /tp <objetivo> <jugador|coords>");
			return;
		}

		if($count === 1){
			$destination = $this->resolveSinglePlayer($client, $players, (string)$parts[0]);
			if($destination === null){
				$client->sendChat("Jugador no encontrado: " . (string)$parts[0]);
				return;
			}
			$destPos = [
				method_exists($destination, 'getX') ? (float)$destination->getX() : 0.0,
				method_exists($destination, 'getY') ? (float)$destination->getY() : 64.0,
				method_exists($destination, 'getZ') ? (float)$destination->getZ() : 0.0,
			];
			$this->teleportClient($client, $destPos);
			// Actualiza también la copia apiPlayer (quien escucha
			// `syncTeleport` ya está sincronizado por `client->teleport()`,
			// que emite `player.update_position`).
			$api = $this->apiPlayer($client);
			if($api !== null){
				if(property_exists($api, 'x')){ $api->x = $destPos[0]; }
				if(property_exists($api, 'y')){ $api->y = $destPos[1]; }
				if(property_exists($api, 'z')){ $api->z = $destPos[2]; }
			}
			$client->sendChat(sprintf('Teleportado a %s.', $destination->getName()));
			return;
		}

		if($count === 2){
			$targets = $this->resolveTeleportTargets($client, $players, (string)$parts[0]);
			if($targets === []){
				$client->sendChat("Jugador no encontrado: " . (string)$parts[0]);
				return;
			}
			$destination = $this->resolveSinglePlayer($client, $players, (string)$parts[1]);
			if($destination === null){
				$client->sendChat("Jugador no encontrado: " . (string)$parts[1]);
				return;
			}
			$destPos = [
				method_exists($destination, 'getX') ? (float)$destination->getX() : 0.0,
				method_exists($destination, 'getY') ? (float)$destination->getY() : 64.0,
				method_exists($destination, 'getZ') ? (float)$destination->getZ() : 0.0,
			];
			foreach($targets as $target){
				$isSelf = strcasecmp($target->getName(), $senderName) === 0;
				$this->teleportPlayerObject($target, $destPos[0], $destPos[1], $destPos[2]);
				if($isSelf){
					$client->sendChat(sprintf('Teleportado a %s.', $destination->getName()));
				}else{
					$client->sendChat(sprintf('Teleportando a %s a %s.', $target->getName(), $destination->getName()));
				}
			}
			return;
		}

		if($count >= 3 && $count <= 6){
			$selfForm = ($count === 3 || $count === 5);
			$coordStart = $selfForm ? 0 : 1;
			if(!$selfForm){
				$targets = $this->resolveTeleportTargets($client, $players, (string)$parts[0]);
				if($targets === []){
					$client->sendChat("Jugador no encontrado: " . (string)$parts[0]);
					return;
				}
			}else{
				$targets = [$this->apiPlayer($client) ?? $this->virtualPlayerFor($client, $senderName)];
				$targets = array_values(array_filter($targets));
				if($targets === []){
					$client->sendChat("No se pudo resolver el remitente para /tp.");
					return;
				}
			}
			$coords = [$parts[$coordStart] ?? null, $parts[$coordStart + 1] ?? null, $parts[$coordStart + 2] ?? null];
			if($coords[0] === null || $coords[1] === null || $coords[2] === null){
				$client->sendChat("Uso: /tp <x> <y> <z> (coordenadas del mundo Bedrock)");
				return;
			}
			$yaw = ($count === 5 || $count === 6) && isset($parts[$coordStart + 3]) && is_numeric($parts[$coordStart + 3]) ? (float)$parts[$coordStart + 3] : null;
			$pitch = ($count === 5 || $count === 6) && isset($parts[$coordStart + 4]) && is_numeric($parts[$coordStart + 4]) ? (float)$parts[$coordStart + 4] : null;
			foreach($targets as $target){
				$origin = [
					method_exists($target, 'getX') ? (float)$target->getX() : 0.0,
					method_exists($target, 'getY') ? (float)$target->getY() : 64.0,
					method_exists($target, 'getZ') ? (float)$target->getZ() : 0.0,
				];
				$x = self::parseCoordinateTele((string)$coords[0], $origin[0]);
				$y = self::parseCoordinateTele((string)$coords[1], $origin[1]);
				$z = self::parseCoordinateTele((string)$coords[2], $origin[2]);
				if($x === null || $y === null || $z === null){
					$client->sendChat("Uso: /tp <x> <y> <z> (coordenadas del mundo Bedrock)");
					return;
				}
				$this->teleportPlayerObject($target, $x, $y, $z, $yaw, $pitch);
				$isSelf = strcasecmp($target->getName(), $senderName) === 0;
				if($isSelf){
					$client->sendChat(sprintf('Teleportado a %.2f, %.2f, %.2f.', $x, $y, $z));
				}else{
					$client->sendChat(sprintf('Teleportando a %s a %.2f, %.2f, %.2f.', $target->getName(), $x, $y, $z));
				}
			}
			return;
		}

		$client->sendChat("Uso: /tp <x> <y> <z> (coordenadas del mundo Bedrock)");
	}

	/** Jugador de la API para el remitente Java (o null si no está dado de alta). */
	private function virtualPlayerFor(JavaClient $client, string $senderName) : mixed{
		$api = $this->apiPlayer($client);
		if($api !== null){
			return $api;
		}
		foreach($this->plugin->getServer()->getOnlinePlayers() as $player){
			if(strcasecmp($player->getName(), $senderName) === 0){
				return $player;
			}
		}
		if(isset($this->apiPlayers[$senderName])){
			return $this->apiPlayers[$senderName];
		}
		return null;
	}

	/**
	 * Teleporta un `Player` de la API: si tiene cliente Java conectado se
	 * mueve por su socket (`JavaClient::teleport()`), si es Bedrock se
	 * emite `Player::teleport()` (acción `teleport` para Rust).
	 */
	private function teleportPlayerObject(mixed $player, float $x, float $y, float $z, ?float $yaw = null, ?float $pitch = null) : void{
		if($player === null){
			return;
		}
		$name = method_exists($player, 'getName') ? (string)$player->getName() : '';
		foreach($this->clients as $other){
			if($other->getUsername() !== null && $name !== '' && strcasecmp((string)$other->getUsername(), $name) === 0 && $other->isPlaying()){
				// `$yaw` llega en sistema Bedrock (comando `/tp` o copia
				// API): al socket Java va convertido, a la copia API
				// queda en Bedrock.
				$other->teleport($this, $x, $y, $z, $yaw !== null ? Protocol::bedrockYawToJava($yaw) : null, $pitch !== null ? Protocol::bedrockPitchToJava($pitch) : null);
				if(property_exists($player, 'x')){ $player->x = $x; }
				if(property_exists($player, 'y')){ $player->y = $y; }
				if(property_exists($player, 'z')){ $player->z = $z; }
				if($yaw !== null && property_exists($player, 'yaw')){ $player->yaw = $yaw; }
				if($pitch !== null && property_exists($player, 'pitch')){ $player->pitch = $pitch; }
				return;
			}
		}
		try{
			if(method_exists($player, 'teleport')){
				$position = $this->makeVector($x, $y, $z, $yaw, $pitch);
				$player->teleport($position);
				return;
			}
		}catch(\Throwable $e){
			$this->log("No se pudo teleportear a {$name}: " . $e->getMessage());
			return;
		}
		// Sin `teleport()` (stub de tests): escribe las props directamente y
		// emite la acción si la copia API lo soporta.
		if(property_exists($player, 'x')){ $player->x = $x; }
		if(property_exists($player, 'y')){ $player->y = $y; }
		if(property_exists($player, 'z')){ $player->z = $z; }
		if($yaw !== null && property_exists($player, 'yaw')){ $player->yaw = $yaw; }
		if($pitch !== null && property_exists($player, 'pitch')){ $player->pitch = $pitch; }
	}

	private function makeVector(float $x, float $y, float $z, ?float $yaw, ?float $pitch) : mixed{
		if(class_exists(\pocketmine\math\Vector3::class)){
			$vector = new \pocketmine\math\Vector3($x, $y, $z);
		}else{
			$vector = new \stdClass();
			$vector->x = $x;
			$vector->y = $y;
			$vector->z = $z;
		}
		if($yaw !== null){
			$vector->yaw = $yaw;
		}
		if($pitch !== null){
			$vector->pitch = $pitch;
		}
		return $vector;
	}

	private static function parseCoordinateTele(string $value, float $origin) : ?float{
		$value = trim($value);
		if(str_starts_with($value, '~') || str_starts_with($value, '^')){
			if($value === '~' || $value === '^'){
				return $origin;
			}
			$offset = substr($value, 1);
			return is_numeric($offset) ? $origin + (float)$offset : null;
		}
		return is_numeric($value) ? (float)$value : null;
	}

	/** @param list<string> $parts @return list<string> */
	private function mergeSelectorTokens(array $parts) : array{
		$merged = [];
		$buffer = null;
		$depth = 0;
		foreach($parts as $token){
			$token = (string)$token;
			$delta = substr_count($token, '[') - substr_count($token, ']');
			if($buffer === null){
				$depth = $delta;
				if($depth <= 0){
					$merged[] = $token;
				}else{
					$buffer = $token;
				}
			}else{
				$buffer .= ' ' . $token;
				$depth += $delta;
				if($depth <= 0){
					$merged[] = $buffer;
					$buffer = null;
				}
			}
		}
		if($buffer !== null){
			$merged[] = $buffer;
		}
		return $merged;
	}

	private function resolveSinglePlayer(JavaClient $client, array $players, string $query) : mixed{
		$list = $this->resolveTeleportTargets($client, $players, $query);
		return $list[0] ?? null;
	}

	/**
	 * @param array<int, \pocketmine\player\Player> $players
	 * @return list<\pocketmine\player\Player>
	 */
	private function resolveTeleportTargets(JavaClient $client, array $players, string $query) : array{
		$players = array_values($players);
		if($query !== '' && $query[0] === '@'){
			$parsed = $this->parseSelectorTeleport($client, $query);
			if($parsed === null){
				return [];
			}
			[$kind, $args] = $parsed;
			$origin = [
				$args['x'] ?? $client->getX(),
				$args['y'] ?? $client->getY(),
				$args['z'] ?? $client->getZ(),
			];
			$pool = [];
			if($kind === 'self'){
				foreach($players as $player){
					if(strcasecmp($player->getName(), (string)$client->getUsername()) === 0){
						$pool[] = $player;
						break;
					}
				}
				if($pool === []){
					$api = $this->apiPlayer($client);
					if($api !== null){
						$pool[] = $api;
					}
				}
			}else{
				$pool = $players;
				// `@a`/`@e` deben incluir también a los virtuales Java con
				// cliente en play aunque `getOnlinePlayers()` no los liste
				// (runtime sin `addPlayer`).
				$known = array_map(static fn($player) => strtolower($player->getName()), $pool);
				foreach($this->clients as $other){
					$username = $other->getUsername();
					if($username !== null && $other->isPlaying() && !in_array(strtolower((string)$username), $known, true)){
						$api = $this->apiPlayers[(string)$username] ?? $this->apiPlayer($other);
						if($api !== null){
							$pool[] = $api;
						}
					}
				}
			}
			if(isset($args['name'])){
				$pool = array_filter($pool, static fn($player) => strcasecmp($player->getName(), (string)$args['name']) === 0);
			}
			if(isset($args['type'])){
				$type = strtolower((string)$args['type']);
				if($type !== 'player' && $type !== 'players'){
					return [];
				}
			}
			if(isset($args['gamemode'])){
				$pool = array_filter($pool, static fn($player) => (int)$player->getGamemode() === (int)$args['gamemode']);
			}
			$min = $args['distance_min'] ?? null;
			$max = $args['distance_max'] ?? null;
			if($min !== null || $max !== null){
				$pool = array_filter($pool, function($player) use ($origin, $min, $max){
					$distance = $this->distanceToPlayer($player, $origin);
					if($min !== null && $distance < $min){
						return false;
					}
					if($max !== null && $distance > $max){
						return false;
					}
					return true;
				});
			}
			$pool = array_values($pool);
			$sort = $args['sort'] ?? match($kind){
				'nearest' => 'nearest',
				'random' => 'random',
				default => 'arbitrary',
			};
			if($sort === 'nearest'){
				usort($pool, fn($a, $b) => $this->distanceToPlayer($a, $origin) <=> $this->distanceToPlayer($b, $origin));
			}elseif($sort === 'furthest'){
				usort($pool, fn($a, $b) => $this->distanceToPlayer($b, $origin) <=> $this->distanceToPlayer($a, $origin));
			}elseif($sort === 'random'){
				shuffle($pool);
			}
			$defaultLimit = match($kind){
				'nearest', 'random', 'self' => 1,
				default => PHP_INT_MAX,
			};
			$limit = $args['limit'] ?? $defaultLimit;
			if($limit <= 0){
				return [];
			}
			if($limit < count($pool)){
				$pool = array_slice($pool, 0, $limit);
			}
			if($kind === 'entities'){
				$this->log('Selector @e: no hay entidades no-jugador en Bedrock, se devuelven ' . count($pool) . ' jugador(es) como entidades.');
			}
			return array_values($pool);
		}
		// Nombre con regla de prefijo único de `resolve_player` (Rust): exacto
		// gana; si no, sólo se acepta un único prefijo.
		$exact = null;
		foreach($players as $player){
			if(strcasecmp($player->getName(), $query) === 0){
				$exact = $player;
				break;
			}
		}
		if($exact !== null){
			return [$exact];
		}
		$matches = [];
		foreach($players as $player){
			if(stripos($player->getName(), $query) === 0){
				$matches[] = $player;
			}
		}
		// Fallback: virtuales Java con cliente pero sin entrada en
		// `getOnlinePlayers()` (runtime sin `addPlayer`).
		if($matches === []){
			foreach($this->clients as $other){
				$username = $other->getUsername();
				if($username !== null && $other->isPlaying() && stripos((string)$username, $query) === 0){
					$api = $this->apiPlayers[(string)$username] ?? $this->apiPlayer($other);
					if($api !== null){
						$matches[] = $api;
					}
				}
			}
		}
		return count($matches) === 1 ? $matches : [];
	}

	/** @param array{0:float,1:float,2:float} $origin */
	private function distanceToPlayer(mixed $player, array $origin) : float{
		$dx = (float)$player->getX() - $origin[0];
		$dy = (float)$player->getY() - $origin[1];
		$dz = (float)$player->getZ() - $origin[2];
		return sqrt($dx * $dx + $dy * $dy + $dz * $dz);
	}

	/** @return ?array{0:string,1:array<string, mixed>} */
	private function parseSelectorTeleport(JavaClient $client, string $selector) : ?array{
		if(strlen($selector) < 2){
			return null;
		}
		$letter = strtolower($selector[1]);
		$kind = match($letter){
			's' => 'self',
			'p', 'n' => 'nearest',
			'a', 'c' => 'all',
			'e' => 'entities',
			'r' => 'random',
			default => null,
		};
		if($kind === null){
			return null;
		}
		$rest = trim(substr($selector, 2));
		if($rest === ''){
			return [$kind, []];
		}
		if(!str_starts_with($rest, '[') || !str_ends_with($rest, ']')){
			return null;
		}
		$inner = substr($rest, 1, -1);
		$origin = [$client->getX(), $client->getY(), $client->getZ()];
		$args = [];
		foreach(explode(',', $inner) as $part){
			$part = trim($part);
			if($part === ''){
				continue;
			}
			$equals = strpos($part, '=');
			if($equals === false){
				continue;
			}
			$key = strtolower(trim(substr($part, 0, $equals)));
			$value = trim(substr($part, $equals + 1), " \t\n\r\0\x0B\"'");
			$value = trim($value);
			if($key === 'x'){
				$v = self::parseCoordinateTele($value, $origin[0]);
				if($v !== null){ $args['x'] = $v; }
			}elseif($key === 'y'){
				$v = self::parseCoordinateTele($value, $origin[1]);
				if($v !== null){ $args['y'] = $v; }
			}elseif($key === 'z'){
				$v = self::parseCoordinateTele($value, $origin[2]);
				if($v !== null){ $args['z'] = $v; }
			}elseif($key === 'distance'){
				$pos = strpos($value, '..');
				if($pos !== false){
					$min = trim(substr($value, 0, $pos));
					$max = trim(substr($value, $pos + 2));
					if($min !== '' && is_numeric($min)){ $args['distance_min'] = (float)$min; }
					if($max !== '' && is_numeric($max)){ $args['distance_max'] = (float)$max; }
				}elseif(is_numeric($value)){
					$args['distance_max'] = (float)$value;
				}
			}elseif($key === 'limit'){
				$args['limit'] = max(0, (int)$value);
			}elseif($key === 'sort'){
				$args['sort'] = match(strtolower($value)){
					'nearest' => 'nearest',
					'furthest' => 'furthest',
					'random' => 'random',
					default => 'arbitrary',
				};
			}elseif($key === 'name'){
				$args['name'] = $value;
			}elseif($key === 'type'){
				$args['type'] = $value;
			}elseif($key === 'gamemode'){
				$gm = self::parseGamemode($value);
				if($gm !== null){ $args['gamemode'] = $gm; }
			}
		}
		return [$kind, $args];
	}

	/**
	 * `/gamemode` local del puente (mismos modos que el core Rust:
	 * survival/s, creative/c, adventure/a, spectator/sp + 0-3 y alias
	 * gmc/gm...). Actualiza el `apiPlayer`, emite `set_gamemode` para que
	 * Rust actualice el registry y notifica al `JavaClient` con el paquete.
	 *
	 * @param list<string> $parts argumentos tras el nombre del comando
	 */
	private function handleGamemode(JavaClient $client, string $name, string $command, array $parts) : void{
		$parts = $this->mergeSelectorTokens($parts);
		$aliasMode = match($command){
			'gmc' => 1,
			'gms' => 0,
			'gma' => 2,
			'gmsp' => 3,
			default => null,
		};
		if($aliasMode !== null){
			$gamemode = $aliasMode;
			$targetArg = $parts[0] ?? null;
		}else{
			if(count($parts) === 0){
				$client->sendChat("Uso: /gamemode <survival|creative|adventure|spectator> [jugador]");
				return;
			}
			$gamemode = self::parseGamemode((string)$parts[0]);
			if($gamemode === null){
				$client->sendChat("Modo desconocido: " . (string)$parts[0]);
				return;
			}
			$targetArg = $parts[1] ?? null;
		}

		// Selector-aware: `@a`/`@p`/`@e`/`@r`/... resuelven contra
		// `getOnlinePlayers()` (mismo helper que `/tp`).
		if($targetArg !== null && (string)$targetArg !== '' && str_starts_with((string)$targetArg, '@')){
			$players = array_values($this->plugin->getServer()->getOnlinePlayers());
			$targets = $this->resolveTeleportTargets($client, $players, (string)$targetArg);
			if($targets === []){
				$client->sendChat("Jugador no encontrado: " . (string)$targetArg);
				return;
			}
			$player = $this->apiPlayer($client);
			$modeName = self::gamemodeName($gamemode);
			foreach($targets as $targetPlayer){
				$targetName = $targetPlayer->getName();
				$isSelf = strcasecmp($targetName, $name) === 0;
				if($player !== null && method_exists($player, 'hasPermission')){
					$node = $isSelf ? 'pocketmine.command.gamemode.self' : 'pocketmine.command.gamemode.other';
					try{
						if(!$player->hasPermission($node)){
							$client->sendChat("No tienes permiso para cambiar el gamemode.");
							continue;
						}
					}catch(\Throwable){}
				}
				if(method_exists($targetPlayer, 'getGamemode') && (int)$targetPlayer->getGamemode() === $gamemode){
					$client->sendChat("El cambio de modo de juego para " . $targetName . " falló (ya está en ese modo).");
					continue;
				}
				if(method_exists($targetPlayer, 'setGamemode')){
					$targetPlayer->setGamemode($gamemode);
				}else{
					try{
						$this->plugin->getServer()->addAction('set_gamemode', ['target_player' => $targetName, 'gamemode' => $gamemode]);
					}catch(\Throwable){}
				}
				foreach($this->clients as $other){
					if($other->getUsername() !== null && strcasecmp((string)$other->getUsername(), $targetName) === 0){
						$other->setGamemode($gamemode);
						break;
					}
				}
				if($isSelf){
					$client->sendChat("Tu modo de juego ha cambiado a " . $modeName);
				}else{
					$client->sendChat("Has cambiado el modo de juego de " . $targetName . " a " . $modeName);
				}
			}
			return;
		}

		$targetName = ($targetArg !== null && (string)$targetArg !== '') ? (string)$targetArg : $name;
		$player = $this->apiPlayer($client);
		$isSelf = strcasecmp($targetName, $name) === 0;

		// Permiso como el core: `.self` para sí mismo, `.other` para otro.
		if($player !== null && method_exists($player, 'hasPermission')){
			$node = $isSelf ? 'pocketmine.command.gamemode.self' : 'pocketmine.command.gamemode.other';
			try{
				if(!$player->hasPermission($node)){
					$client->sendChat("No tienes permiso para cambiar el gamemode.");
					return;
				}
			}catch(\Throwable){}
		}

		$targetPlayer = null;
		foreach($this->plugin->getServer()->getOnlinePlayers() as $online){
			if(strcasecmp($online->getName(), $targetName) === 0){
				$targetPlayer = $online;
				$targetName = $online->getName();
				break;
			}
		}
		$targetPlayer ??= $this->apiPlayers[$targetName] ?? null;
		if($isSelf){
			$targetPlayer = $player;
			$targetName = $name;
		}
		if($targetPlayer === null){
			$entry = self::host('player.get', ['name' => $targetName]);
			if(!is_array($entry)){
				$client->sendChat("Jugador no encontrado: " . $targetName);
				return;
			}
			$targetName = (string)($entry['name'] ?? $targetName);
		}elseif(method_exists($targetPlayer, 'getGamemode') && (int)$targetPlayer->getGamemode() === $gamemode){
			$client->sendChat("El cambio de modo de juego para " . $targetName . " falló (ya está en ese modo).");
			return;
		}

		// Actualiza la copia PHP y emite `set_gamemode` (Rust actualiza el
		// registry); el paquete Java sale ya vía `setGamemode` abajo.
		if($targetPlayer !== null && method_exists($targetPlayer, 'setGamemode')){
			$targetPlayer->setGamemode($gamemode);
		}else{
			try{
				$this->plugin->getServer()->addAction('set_gamemode', ['target_player' => $targetName, 'gamemode' => $gamemode]);
			}catch(\Throwable){}
		}
		foreach($this->clients as $other){
			if($other->getUsername() !== null && strcasecmp((string)$other->getUsername(), $targetName) === 0){
				$other->setGamemode($gamemode);
				break;
			}
		}
		if($isSelf){
			$client->setGamemode($gamemode);
		}
		$modeName = self::gamemodeName($gamemode);
		if($isSelf){
			$client->sendChat("Tu modo de juego ha cambiado a " . $modeName);
		}else{
			$client->sendChat("Has cambiado el modo de juego de " . $targetName . " a " . $modeName);
		}
	}

	/**
	 * `/time` local del puente (mismo contrato que el core Rust
	 * `command::time`): `set <ticks|day|noon|sunset|night|midnight|sunrise>`,
	 * `add <ticks>`, `start`, `stop`, `query`.
	 *
	 * Escribe en el mundo compartido (`World::setTime` /
	 * `startTime`/`stopTime` -> host `world.set_time*`): Bedrock
	 * recibe el salto por la detección de saltos de
	 * `SessionManager::tick_world_time` y los Java por
	 * `tickTimeSync()`, ambos en el siguiente tick, así un
	 * `/time set 6000` desde Java pone de noche a Bedrock y a los
	 * demás Java sin esperar al broadcast de 1 s.
	 *
	 * @param list<string> $parts argumentos tras `time`
	 */
	private function handleTime(JavaClient $client, string $name, array $parts) : void{
		$sub = strtolower((string)($parts[0] ?? 'query'));
		$player = $this->apiPlayer($client);
		$node = match($sub){
			'set' => 'pocketmine.command.time.set',
			'add' => 'pocketmine.command.time.add',
			'start' => 'pocketmine.command.time.start',
			'stop' => 'pocketmine.command.time.stop',
			default => 'pocketmine.command.time.query',
		};
		if($player !== null && method_exists($player, 'hasPermission')){
			try{
				if(!$player->hasPermission($node)){
					$client->sendChat("No tienes permiso para usar /time.");
					return;
				}
			}catch(\Throwable){}
		}
		$world = $this->world();
		switch($sub){
			case 'set':
				$ticks = isset($parts[1]) ? self::parseTimeValue((string)$parts[1]) : null;
				if($ticks === null){
					$client->sendChat("Uso: /time set <ticks|day|noon|sunset|night|midnight|sunrise>");
					return;
				}
				if(method_exists($world, 'setTime')){
					$world->setTime($ticks);
				}
				$client->sendChat("La hora del mundo se ha fijado en " . $this->worldTime());
				$this->broadcastTimeNow();
				break;
			case 'add':
				$delta = isset($parts[1]) && is_numeric($parts[1]) ? (int)$parts[1] : null;
				if($delta === null){
					$client->sendChat("Uso: /time add <ticks>");
					return;
				}
				if(method_exists($world, 'setTime')){
					$world->setTime($this->worldTime() + $delta);
				}
				$client->sendChat("Se han añadido " . $delta . " ticks a la hora del mundo (" . $this->worldTime() . ")");
				$this->broadcastTimeNow();
				break;
			case 'start':
				if(method_exists($world, 'startTime')){
					$world->startTime();
				}
				$client->sendChat("El ciclo día/noche se ha reiniciado.");
				$this->broadcastTimeNow();
				break;
			case 'stop':
				if(method_exists($world, 'stopTime')){
					$world->stopTime();
				}
				$client->sendChat("El ciclo día/noche se ha detenido.");
				$this->broadcastTimeNow();
				break;
			case 'query':
			default:
				if(!in_array($sub, ['query', ''], true)){
					$client->sendChat("Uso: /time <set|add|start|stop|query>");
					return;
				}
				$client->sendChat("La hora del mundo es " . $this->worldTime());
				break;
		}
	}

	/** Envía la hora actual a todos los Java en play ya (sin esperar al tick). */
	private function broadcastTimeNow() : void{
		try{
			$time = $this->worldTime();
		}catch(\Throwable $e){
			return;
		}
		$running = $this->worldTimeRunning();
		foreach($this->clients as $other){
			if(!$other->isPlaying()){
				continue;
			}
			try{
				$other->sendTime($time, $running);
			}catch(\Throwable $e){
				continue;
			}
		}
		$this->lastBroadcastTime = $time;
		$this->lastBroadcastTimeRunning = $running;
		$this->ticksSinceTimeSync = 0;
	}

	/** `command::parse_time_value` (Rust): mismos nombres y números. */
	private static function parseTimeValue(string $value) : ?int{
		$named = match(strtolower(trim($value))){
			'day', 'dia', 'día' => 1000,
			'noon', 'mediodia', 'mediodía' => 6000,
			'sunset', 'atardecer' => 12000,
			'night', 'noche' => 13000,
			'midnight', 'medianoche' => 18000,
			'sunrise', 'amanecer' => 23000,
			default => null,
		};
		if($named !== null){
			return $named;
		}
		return is_numeric($value) ? ((int)$value % 24000 + 24000) % 24000 : null;
	}

	/** `command::parse_gamemode` (Rust): mismos nombres y números. */
	private static function parseGamemode(string $value) : ?int{
		return match(strtolower(trim($value))){
			'0', 's', 'survival', 'supervivencia' => 0,
			'1', 'c', 'creative', 'creativo' => 1,
			'2', 'a', 'adventure', 'aventura' => 2,
			'3', 'sp', 'spectator', 'espectador' => 3,
			default => null,
		};
	}

	/** `command::gamemode_translatable` (Rust) en curso. */
	private static function gamemodeName(int $gamemode) : string{
		return match($gamemode){
			1 => 'Creative Mode',
			2 => 'Adventure Mode',
			3 => 'Spectator Mode',
			default => 'Survival Mode',
		};
	}
}
