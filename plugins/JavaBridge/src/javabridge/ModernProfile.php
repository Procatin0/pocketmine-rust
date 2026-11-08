<?php

declare(strict_types=1);

namespace javabridge;

/**
 * Tabla de paquetes de las versiones modernas de Java Edition.
 *
 * Los ids salen de PrismarineJS/minecraft-data (`data/pc/<versión>/protocol.json`)
 * y cambian entre versiones, por eso cada protocolo tiene su propia tabla.
 *
 * Flujo moderno (1.20.2+): handshake -> login -> configuration -> play.
 */
final class ModernProfile{

	/** @var array<int, string> */
	public const VERSIONS = [
		769 => '1.21.4',
		770 => '1.21.5',
		773 => '1.21.9',
		774 => '1.21.11',
		775 => '26.1',
	];

	public const VERSION_NAMES = ['1.21.4', '1.21.5', '1.21.9', '1.21.11', '26.1'];

	/**
	 * Marca del servidor que ve el cliente en F3 (`Server:`).
	 *
	 * Viaja en configuration como `custom_payload` (id `config_brand`)
	 * en el canal `minecraft:brand` con el texto como String. Sin este
	 * paquete F3 muestra `null` como servidor. Es "PocketMine", el mismo
	 * nombre que ya usan el MOTD y el saludo del puente.
	 */
	public const BRAND = 'PocketMine';
	public const BRAND_CHANNEL = 'minecraft:brand';

	/**
	 * id por nombre lógico. `config_*` son paquetes del cliente en la fase de
	 * configuración.
	 *
	 * @var array<int, array<string, int>>
	 */
	private const IDS = [
		769 => [
			'login_success' => 2,
			'join_game' => 44,
			'position' => 66,
			'map_chunk' => 40,
			'game_state_change' => 35,
			'abilities' => 58,
			'system_chat' => 115,
			'block_change' => 9,
			'unload_chunk' => 34,
			'kick' => 29,
			'keep_alive' => 39,
			'chunk_batch_start' => 13,
			'chunk_batch_finished' => 12,
			'registry_data' => 7,
			'finish_configuration' => 3,
			'feature_flags' => 12,
			'tags' => 13,
			// Alias de `tags`: el paquete clientbound `minecraft:update_tags`
			// (configuration, 0x0d en 769-775 según protocol.json).
			'update_tags' => 13,
			// `custom_payload` clientbound de configuration (0x01 en 769-775,
			// canal + restBuffer). OJO: no es `config_custom_payload` (2), que
			// es el id serverbound de este mismo nombre en configuration.
			'config_brand' => 1,
			'login_acknowledged' => 3,
			'config_settings' => 0,
			'config_custom_payload' => 2,
			'config_select_known_packs' => 7,
			'config_finish' => 3,
			'chat_message' => 7,
			'chat_command' => 5,
			'sb_position' => 28,
			'sb_position_look' => 29,
			'sb_look' => 30,
			'client_command' => 10,
			'block_dig' => 39,
			'block_place' => 60,
			'teleport_confirm' => 0,
			'chunk_batch_received' => 9,
			'player_loaded' => 42,
			'spawn_entity' => 1,
			'rel_entity_move' => 47,
			'entity_move_look' => 48,
			'entity_look' => 50,
			'entity_head_rotation' => 77,
			'entity_destroy' => 71,
			'player_info' => 64,
			'player_remove' => 63,
			'entity_metadata' => 93,
			'entity_teleport' => 119,
			'sync_entity_position' => 32,
			'update_view_position' => 88,
			'update_view_distance' => 89,
			// `update_time`: age i64 + time i64 + tickDayTime bool
			// (minecraft-data `packet_update_time`, 769).
			'update_time' => 107,
		],
		770 => [
			'login_success' => 2,
			'join_game' => 43,
			'position' => 65,
			'map_chunk' => 39,
			'game_state_change' => 34,
			'abilities' => 57,
			'system_chat' => 114,
			'block_change' => 8,
			'unload_chunk' => 33,
			'kick' => 28,
			'keep_alive' => 38,
			'chunk_batch_start' => 12,
			'chunk_batch_finished' => 11,
			'registry_data' => 7,
			'finish_configuration' => 3,
			'feature_flags' => 12,
			'tags' => 13,
			// Alias de `tags`: el paquete clientbound `minecraft:update_tags`
			// (configuration, 0x0d en 769-775 según protocol.json).
			'update_tags' => 13,
			// `custom_payload` clientbound de configuration (0x01 en 769-775,
			// canal + restBuffer). OJO: no es `config_custom_payload` (2), que
			// es el id serverbound de este mismo nombre en configuration.
			'config_brand' => 1,
			'login_acknowledged' => 3,
			'config_settings' => 0,
			'config_custom_payload' => 2,
			'config_select_known_packs' => 7,
			'config_finish' => 3,
			'chat_message' => 7,
			'chat_command' => 5,
			'sb_position' => 28,
			'sb_position_look' => 29,
			'sb_look' => 30,
			'client_command' => 10,
			'block_dig' => 39,
			'block_place' => 62,
			'teleport_confirm' => 0,
			'chunk_batch_received' => 9,
			'player_loaded' => 42,
			'spawn_entity' => 1,
			'rel_entity_move' => 46,
			'entity_move_look' => 47,
			'entity_look' => 49,
			'entity_head_rotation' => 76,
			'entity_destroy' => 70,
			'player_info' => 63,
			'player_remove' => 62,
			'entity_metadata' => 92,
			'entity_teleport' => 118,
			'sync_entity_position' => 31,
			'update_view_position' => 87,
			'update_view_distance' => 88,
			// `update_time`: age i64 + time i64 + tickDayTime bool (770).
			'update_time' => 106,
		],
		773 => [
			'login_success' => 2,
			'join_game' => 48,
			'position' => 70,
			'map_chunk' => 44,
			'game_state_change' => 38,
			'abilities' => 62,
			'system_chat' => 119,
			'block_change' => 8,
			'unload_chunk' => 37,
			'kick' => 32,
			'keep_alive' => 43,
			'chunk_batch_start' => 12,
			'chunk_batch_finished' => 11,
			'registry_data' => 7,
			'finish_configuration' => 3,
			'feature_flags' => 12,
			'tags' => 13,
			// Alias de `tags`: el paquete clientbound `minecraft:update_tags`
			// (configuration, 0x0d en 769-775 según protocol.json).
			'update_tags' => 13,
			// `custom_payload` clientbound de configuration (0x01 en 769-775,
			// canal + restBuffer). OJO: no es `config_custom_payload` (2), que
			// es el id serverbound de este mismo nombre en configuration.
			'config_brand' => 1,
			'login_acknowledged' => 3,
			'config_settings' => 0,
			'config_custom_payload' => 2,
			'config_select_known_packs' => 7,
			'config_finish' => 3,
			'chat_message' => 8,
			'chat_command' => 6,
			'sb_position' => 29,
			'sb_position_look' => 30,
			'sb_look' => 31,
			'client_command' => 11,
			'block_dig' => 40,
			'block_place' => 63,
			'teleport_confirm' => 0,
			'chunk_batch_received' => 10,
			'player_loaded' => 43,
			'spawn_entity' => 1,
			'rel_entity_move' => 51,
			'entity_move_look' => 52,
			'entity_look' => 54,
			'entity_head_rotation' => 81,
			'entity_destroy' => 75,
			'player_info' => 68,
			'player_remove' => 67,
			'entity_metadata' => 97,
			'entity_teleport' => 123,
			'sync_entity_position' => 35,
			'update_view_position' => 92,
			'update_view_distance' => 93,
			// `update_time`: age i64 + time i64 + tickDayTime bool (773).
			'update_time' => 111,
		],
		774 => [
			'login_success' => 2,
			'join_game' => 48,
			'position' => 70,
			'map_chunk' => 44,
			'game_state_change' => 38,
			'abilities' => 62,
			'system_chat' => 119,
			'block_change' => 8,
			'unload_chunk' => 37,
			'kick' => 32,
			'keep_alive' => 43,
			'chunk_batch_start' => 12,
			'chunk_batch_finished' => 11,
			'registry_data' => 7,
			'finish_configuration' => 3,
			'feature_flags' => 12,
			'tags' => 13,
			// Alias de `tags`: el paquete clientbound `minecraft:update_tags`
			// (configuration, 0x0d en 769-775 según protocol.json).
			'update_tags' => 13,
			// `custom_payload` clientbound de configuration (0x01 en 769-775,
			// canal + restBuffer). OJO: no es `config_custom_payload` (2), que
			// es el id serverbound de este mismo nombre en configuration.
			'config_brand' => 1,
			'login_acknowledged' => 3,
			'config_settings' => 0,
			'config_custom_payload' => 2,
			'config_select_known_packs' => 7,
			'config_finish' => 3,
			'chat_message' => 8,
			'chat_command' => 6,
			'sb_position' => 29,
			'sb_position_look' => 30,
			'sb_look' => 31,
			'client_command' => 11,
			'block_dig' => 40,
			'block_place' => 63,
			'teleport_confirm' => 0,
			'chunk_batch_received' => 10,
			'player_loaded' => 43,
			'spawn_entity' => 1,
			'rel_entity_move' => 51,
			'entity_move_look' => 52,
			'entity_look' => 54,
			'entity_head_rotation' => 81,
			'entity_destroy' => 75,
			'player_info' => 68,
			'player_remove' => 67,
			'entity_metadata' => 97,
			'entity_teleport' => 123,
			'sync_entity_position' => 35,
			'update_view_position' => 92,
			'update_view_distance' => 93,
			// `update_time`: age i64 + time i64 + tickDayTime bool (774).
			'update_time' => 111,
		],
		775 => [
			'login_success' => 2,
			'join_game' => 49,
			'position' => 72,
			'map_chunk' => 45,
			'game_state_change' => 38,
			'abilities' => 64,
			'system_chat' => 121,
			'block_change' => 8,
			'unload_chunk' => 37,
			'kick' => 32,
			'keep_alive' => 44,
			'chunk_batch_start' => 12,
			'chunk_batch_finished' => 11,
			'registry_data' => 7,
			'finish_configuration' => 3,
			'feature_flags' => 12,
			'tags' => 13,
			// Alias de `tags`: el paquete clientbound `minecraft:update_tags`
			// (configuration, 0x0d en 769-775 según protocol.json).
			'update_tags' => 13,
			// `custom_payload` clientbound de configuration (0x01 en 769-775,
			// canal + restBuffer). OJO: no es `config_custom_payload` (2), que
			// es el id serverbound de este mismo nombre en configuration.
			'config_brand' => 1,
			'login_acknowledged' => 3,
			'config_settings' => 0,
			'config_custom_payload' => 2,
			'config_select_known_packs' => 7,
			'config_finish' => 3,
			'chat_message' => 9,
			'chat_command' => 7,
			'sb_position' => 30,
			'sb_position_look' => 31,
			'sb_look' => 32,
			'client_command' => 12,
			'block_dig' => 41,
			'block_place' => 66,
			'teleport_confirm' => 0,
			'chunk_batch_received' => 11,
			'player_loaded' => 44,
			'spawn_entity' => 1,
			'rel_entity_move' => 53,
			'entity_move_look' => 54,
			'entity_look' => 56,
			'entity_head_rotation' => 83,
			'entity_destroy' => 77,
			'player_info' => 70,
			'player_remove' => 69,
			'entity_metadata' => 99,
			'entity_teleport' => 125,
			'sync_entity_position' => 35,
			'update_view_position' => 94,
			'update_view_distance' => 95,
			// 26.1: `update_time` cambió a age i64 + clockUpdates[]
			// {id varint, totalTicks varlong, partialTick f32, rate f32}.
			'update_time' => 113,
		],
	];

	/** @var array<int, list<string>> payloads de `registry_data` ya serializados */
	private static array $registryCache = [];

	/** @var array<int, list<string>> ids de los registros enviados, por protocolo */
	private static array $registryIds = [];

	public readonly int $protocol;
	public readonly string $name;

	public function __construct(int $protocol){
		$name = self::VERSIONS[$protocol] ?? null;
		if($name === null){
			throw new \InvalidArgumentException("Protocolo moderno no soportado: {$protocol}");
		}
		$this->protocol = $protocol;
		$this->name = $name;
	}

	public static function forProtocol(int $protocol) : ?self{
		return isset(self::VERSIONS[$protocol]) ? new self($protocol) : null;
	}

	public static function supports(int $protocol) : bool{
		return isset(self::VERSIONS[$protocol]);
	}

	/**
	 * Estados globales de bloque con el mismo id en 1.21.4 y en 26.1.
	 *
	 * Se usa el estado por defecto de cada bloque, no `minStateId`: el primer
	 * estado de `grass_block` es `snowy=true` (el mundo salía nevado) y el de
	 * `oak_log` es el tronco tumbado en X, no en vertical.
	 */
	private const BLOCKS_STABLE = [
		'air' => 0,
		'stone' => 1,
		'granite' => 2,
		'diorite' => 4,
		'andesite' => 6,
		'grass_block' => 9,
		'dirt' => 10,
		'cobblestone' => 14,
		'bedrock' => 85,
		'water' => 86,
		'sand' => 118,
		'gravel' => 124,
		'gold_ore' => 129,
		'iron_ore' => 131,
		'coal_ore' => 133,
		'oak_log' => 137,
		'spruce_log' => 140,
		'birch_log' => 143,
		'oak_leaves' => 279,
		'spruce_leaves' => 307,
		'birch_leaves' => 335,
		'lapis_ore' => 563,
		'sandstone' => 578,
	];

	/**
	 * Bloques cuyo id global cambia entre versiones (minas y plantas).
	 *
	 * Obtenidos de `target/javamc-<versión>/blocks.json`; se usa el estado por
	 * defecto de cada bloque.
	 */
	private const BLOCKS_BY_PROTOCOL = [
		769 => [
			'redstone_ore' => 5904, 'diamond_ore' => 4329, 'emerald_ore' => 8285,
			'short_grass' => 2048, 'tall_grass' => 11635, 'snow' => 5941,
			'dandelion' => 2118, 'poppy' => 2120,
		],
		770 => [
			'redstone_ore' => 5913, 'diamond_ore' => 4338, 'emerald_ore' => 8295,
			'short_grass' => 2048, 'tall_grass' => 11645, 'snow' => 5950,
			'dandelion' => 2121, 'poppy' => 2123,
		],
		773 => [
			'redstone_ore' => 6681, 'diamond_ore' => 5106, 'emerald_ore' => 9372,
			'short_grass' => 2048, 'tall_grass' => 12722, 'snow' => 6718,
			'dandelion' => 2121, 'poppy' => 2123,
		],
		774 => [
			'redstone_ore' => 6681, 'diamond_ore' => 5106, 'emerald_ore' => 9372,
			'short_grass' => 2048, 'tall_grass' => 12722, 'snow' => 6718,
			'dandelion' => 2121, 'poppy' => 2123,
		],
		775 => [
			'redstone_ore' => 6882, 'diamond_ore' => 5307, 'emerald_ore' => 9573,
			'short_grass' => 2248, 'tall_grass' => 12924, 'snow' => 6919,
			'dandelion' => 2321, 'poppy' => 2324,
		],
	];

	/** Estado global de Java de un bloque por nombre, para esta versión. */
	public function blockState(string $name) : int{
		return self::BLOCKS_BY_PROTOCOL[$this->protocol][$name]
			?? self::BLOCKS_STABLE[$name]
			?? 0;
	}

	public function id(string $key) : int{
		$aliases = [
			'position_look' => 'sb_position_look',
			'look' => 'sb_look',
			'position' => 'position',
			'player_info_update' => 'player_info',
			'rel_move' => 'rel_entity_move',
			'entity_teleport' => 'entity_teleport',
			'entity_destroy' => 'entity_destroy',
			'entity_metadata' => 'entity_metadata',
			'brand' => 'config_brand',
		];
		$lookup = $aliases[$key] ?? $key;
		$id = self::IDS[$this->protocol][$lookup] ?? self::IDS[$this->protocol][$key] ?? null;
		if($id === null){
			throw new \InvalidArgumentException("Paquete desconocido para {$this->name}: {$key}");
		}
		return $id;
	}

	/**
	 * Id de tipo de entidad `minecraft:player` para esta versión.
	 *
	 * Valor de `spawn_entity.type` (índice del registro `entity_type`):
	 * 769=147, 770=148, 773=151, 774/775=155 (minecraft-data entities.json).
	 * No está en los registros de login: el cliente lo conoce de serie.
	 */
	public function playerEntityType() : int{
		return match($this->protocol){
			769 => 147,
			770 => 148,
			773 => 151,
			774, 775 => 155,
			default => 147,
		};
	}

	/** 1.21.4 manda los heightmaps como NBT; 1.21.5+ como lista tipada. */
	public function heightmapsAsNbt() : bool{
		return $this->protocol === 769;
	}

	/**
	 * 1.21.5 quitó el prefijo de longitud del array de longs de cada paleta:
	 * el cliente lo deduce de los bits por entrada y del número de entradas.
	 */
	public function paletteDataHasLength() : bool{
		return $this->protocol === 769;
	}

	/** 26.1 añadió un contador de fluidos delante de cada sección. */
	public function sectionsHaveFluidCount() : bool{
		return $this->protocol >= 775;
	}

	/**
	 * En `spawn_entity` la velocidad va delante de los ángulos como `lpVec3`
	 * desde 1.21.9 (`target/javamc-<versión>/protocol.json`,
	 * `packet_spawn_entity`): 769/770 la llevan al final como `vec3i16`
	 * (3× i16), 773/774/775 en el medio como `lpVec3` (`native`). El id del
	 * paquete (`0x01`, `spawn_entity`) y el resto de campos no cambian.
	 */
	public function spawnVelocityIsLpVec3() : bool{
		return $this->protocol >= 773;
	}

	/**
	 * Índice de `minecraft:plains` en el registro `minecraft:worldgen/biome`
	 * de esta versión (39 en 769-773, 40 en 774-775). El 0 es `badlands`.
	 */
	public function plainsBiome() : int{
		static $cache = [];
		if(isset($cache[$this->protocol])){
			return $cache[$this->protocol];
		}
		$file = dirname(__DIR__, 2) . '/resources/registry/' . $this->protocol . '.json';
		$raw = @file_get_contents($file);
		$decoded = $raw === false ? null : json_decode($raw, true);
		$index = $this->protocol >= 774 ? 40 : 39;
		if(is_array($decoded)){
			foreach($decoded['registries'] ?? [] as $registry){
				if(($registry['id'] ?? '') !== 'minecraft:worldgen/biome'){
					continue;
				}
				foreach($registry['entries'] ?? [] as $i => $entry){
					if(($entry['key'] ?? '') === 'minecraft:plains'){
						$index = (int)$i;
						break 2;
					}
				}
			}
		}
		return $cache[$this->protocol] = $index;
	}

	/**
	 * Payloads del paquete `registry_data`: uno por registro.
	 *
	 * Se envían los registros del login packet vanilla de minecraft-data
	 * (`target/gen-registry.js`) salvo los que referencian tags dinámicos
	 * (`enchantment`, `dialog`) y sin el campo opcional `timelines` del
	 * `dimension_type`: el cliente aborta en "Registry Loading" con *Unbound
	 * tags* si un registro enviado referencia un tag que nunca llega en
	 * `update_tags`.
	 *
	 * @return list<string>
	 */
	public function registryPayloads() : array{
		if(isset(self::$registryCache[$this->protocol])){
			return self::$registryCache[$this->protocol];
		}

		$file = dirname(__DIR__, 2) . '/resources/registry/' . $this->protocol . '.json';
		$raw = @file_get_contents($file);
		$decoded = $raw === false ? null : json_decode($raw, true);
		if(!is_array($decoded) || !isset($decoded['registries'])){
			throw new \RuntimeException("Faltan los registros de Java para {$this->name}: {$file}");
		}

		self::$registryIds[$this->protocol] = array_map(
			static fn(array $registry) : string => (string)$registry['id'],
			$decoded['registries']
		);

		$payloads = [];
		foreach($decoded['registries'] as $registry){
			$payload = Protocol::string((string)$registry['id']) . Protocol::varInt(count($registry['entries']));
			foreach($registry['entries'] as $entry){
				$payload .= Protocol::string((string)$entry['key'])
					. Protocol::boolean(true)
					. Nbt::anonymous($entry['value']);
			}
			$payloads[] = $payload;
		}

		return self::$registryCache[$this->protocol] = $payloads;
	}

	/** @var array<int, string> payload de `tags` ya serializado */
	private static array $tagsCache = [];

	/** @var array<int, array<string, array<string, list<int>>>> miembros de tags por protocolo y registro */
	private static array $tagMembersCache = [];

	/**
	 * Miembros numéricos (IDs del registro estático) de un tag, según
	 * `resources/tag_members/<protocol>.json` (generado con
	 * `tools/gen-tag-members.py`). Los tags no listados ahí se envían
	 * vacíos: el nombre basta para `getOrThrow`, pero F3 no mostrará
	 * ese tag para ningún bloque/fluido.
	 *
	 * Ojo con `minecraft:fluid`: el registro estático es empty=0,
	 * flowing_water=1, water=2, flowing_lava=3, lava=4 (`Fluids.<clinit>`
	 * del jar vanilla), no empty/water/lava a secas — las variantes
	 * fluyentes sí son registros aparte. De ahí water=[1,2], lava=[3,4].
	 *
	 * @return list<int>
	 */
	public function tagMembers(string $registry, string $tag) : array{
		if(!isset(self::$tagMembersCache[$this->protocol])){
			$file = dirname(__DIR__, 2) . '/resources/tag_members/' . $this->protocol . '.json';
			$raw = @file_get_contents($file);
			$decoded = $raw === false ? null : json_decode($raw, true);
			self::$tagMembersCache[$this->protocol] = is_array($decoded) && isset($decoded['registries']) && is_array($decoded['registries'])
				? $decoded['registries']
				: [];
		}
		$members = self::$tagMembersCache[$this->protocol][$registry][$tag] ?? [];
		return is_array($members) ? array_values(array_map('intval', $members)) : [];
	}

	/**
	 * Payload de `config_brand`: canal `minecraft:brand` + String con la
	 * marca. Es el `custom_payload` clientbound de configuration (id 1
	 * en 769-775): `channel` (String) + `data` (restBuffer, aquí el
	 * String de la marca). Sin esto F3 pone `null` en `Server:`.
	 */
	public function brandPayload() : string{
		return Protocol::identifier(self::BRAND_CHANNEL) . Protocol::string(self::BRAND);
	}

	/**
	 * Payload del paquete `tags` (`minecraft:update_tags`) de configuration
	 * (id 13 desde 1.21.4; alias `update_tags` en IDS).
	 *
	 * Al acabar la configuración el cliente construye los componentes de los
	 * items y consulta tags con `getOrThrow`: si el tag no existe aborta con
	 * "Missing tag" (por ejemplo `minecraft:damage_type / minecraft:is_fire`).
	 * Se dan de alta todos los nombres de tags vanilla de cada registro de
	 * `resources/tags/<protocol>.json`; los que tienen miembros en
	 * `resources/tag_members/<protocol>.json` (fluidos, `mineable/*`,
	 * `climbable`, ...) se envían con sus IDs para que F3 los muestre
	 * (agua con `#minecraft:water`, grava con `#minecraft:mineable/shovel`)
	 * y el resto con la lista vacía, que basta para resolver la clave.
	 *
	 * Esquema (`packet_tags` en protocol.json): array de registros
	 * {tagType: String, tags: array de {tagName: String, entries: array
	 * de VarInt con IDs del registro}}.
	 */
	public function tagsPayload() : string{
		if(isset(self::$tagsCache[$this->protocol])){
			return self::$tagsCache[$this->protocol];
		}

		$file = dirname(__DIR__, 2) . '/resources/tags/' . $this->protocol . '.json';
		$raw = @file_get_contents($file);
		$decoded = $raw === false ? null : json_decode($raw, true);
		if(!is_array($decoded) || !isset($decoded['registries'])){
			throw new \RuntimeException("Faltan los tags de Java para {$this->name}: {$file}");
		}

		$this->registryPayloads();
		$present = array_flip(self::$registryIds[$this->protocol] ?? []);
		// Solo se pueden mandar tags de registros que el cliente tenga: los que
		// acabamos de enviar más los estáticos que ya trae de serie. Mandar tags
		// de un registro ausente aborta con "Missing registry".
		$static = ['minecraft:block', 'minecraft:item', 'minecraft:entity_type', 'minecraft:fluid'];
		$selected = [];
		foreach($decoded['registries'] as $registry => $tags){
			$id = 'minecraft:' . $registry;
			if(isset($present[$id]) || in_array($id, $static, true)){
				$selected[$registry] = [$id, $tags];
			}
		}

		$payload = Protocol::varInt(count($selected));
		foreach($selected as $registry => [$id, $tags]){
			$payload .= Protocol::identifier($id) . Protocol::varInt(count($tags));
			foreach($tags as $tag){
				$payload .= Protocol::identifier('minecraft:' . $tag)
					. Protocol::varIntArray($this->tagMembers((string)$registry, (string)$tag));
			}
		}

		return self::$tagsCache[$this->protocol] = $payload;
	}
}
