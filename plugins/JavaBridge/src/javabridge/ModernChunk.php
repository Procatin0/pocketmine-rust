<?php

declare(strict_types=1);

namespace javabridge;

/**
 * Columna de chunk para Java 1.18+ construida desde el chunk real del servidor.
 *
 * El puente pide la columna con `world.get_chunk` (formato de red de Bedrock:
 * subsecciones paletizadas) y la traduce a la columna de Java sin aproximar
 * nada: cada bloque que dibuja el cliente Java está en el mismo sitio que en el
 * mundo de Bedrock, incluidas las roturas y colocaciones de los jugadores.
 */
final class ModernChunk{

	public const MIN_Y = -64;
	public const HEIGHT = 384;
	public const SECTIONS = 24;

	// Estados globales (idénticos en 1.21.4 - 26.1).
	public const AIR = 0;
	public const STONE = 1;
	// Son los estados *por defecto*: 8 es `grass_block[snowy=true]` y 136 es el
	// tronco tumbado en X, así que el mundo salía nevado y con los árboles de
	// lado. Las variantes por versión están en ModernProfile.
	public const GRASS = 9;
	public const DIRT = 10;
	public const COBBLESTONE = 14;
	public const BEDROCK = 85;
	public const WATER = 86;
	public const SAND = 118;
	public const GRAVEL = 124;
	public const GOLD_ORE = 129;
	public const IRON_ORE = 131;
	public const COAL_ORE = 133;
	public const OAK_LOG = 137;
	public const SPRUCE_LOG = 140;
	public const BIRCH_LOG = 143;
	public const OAK_LOG_X = 136;
	public const BIRCH_LOG_Z = 144;
	public const OAK_LEAVES = 279;
	public const SPRUCE_LEAVES = 307;
	public const BIRCH_LEAVES = 335;
	public const LAPIS_ORE = 563;
	public const SANDSTONE = 578;

	/** Entradas de un contenedor de estados de bloque (16x16x16). */
	private const BLOCK_ENTRIES = 4096;

	/** Light sections incluye una sección por debajo y otra por encima. */
	private const LIGHT_SECTIONS = self::SECTIONS + 2;

	/** Los 55 KB de luz de cielo son idénticos en todas las columnas. */
	private static ?string $lightPayload = null;

	/**
	 * Ids de red de Bedrock (hash FNV-1a, ver
	 * `crates/bedrock-world/src/block_id.rs`) y el nombre del bloque en Java.
	 *
	 * El estado numérico sale de `ModernProfile::blockState()`, porque minas y
	 * plantas cambian de id entre 1.21.4 y 26.1.
	 */
	private const BEDROCK_BLOCKS = [
		3690217760 => 'air',
		2150698529 => 'stone',
		2186211206 => 'dirt',
		3727763636 => 'grass_block',
		4121722107 => 'bedrock',
		138639715 => 'sand',
		1529044762 => 'gravel',
		1211861802 => 'water',
		2754680827 => 'sandstone',
		825916963 => 'oak_log',
		2110714365 => 'oak_leaves',
		507450469 => 'spruce_log',
		1404704691 => 'spruce_leaves',
		1423805714 => 'birch_log',
		1740500456 => 'birch_leaves',
		685383673 => 'coal_ore',
		2032622302 => 'iron_ore',
		2144082742 => 'gold_ore',
		2930261704 => 'lapis_ore',
		1419890941 => 'diamond_ore',
		180213920 => 'redstone_ore',
		170572189 => 'emerald_ore',
		3699616834 => 'snow',             // snow_layer
		2827903781 => 'short_grass',
		4175527249 => 'tall_grass',
		2383143704 => 'dandelion',
		3417975282 => 'poppy',
	];

	public static function state(string $kind) : int{
		return match($kind){
			'air' => self::AIR,
			'stone' => self::STONE,
			'grass' => self::GRASS,
			'dirt' => self::DIRT,
			'cobblestone' => self::COBBLESTONE,
			'bedrock' => self::BEDROCK,
			'water' => self::WATER,
			'sand' => self::SAND,
			'gravel' => self::GRAVEL,
			'log' => self::OAK_LOG,
			'leaves' => self::OAK_LEAVES,
			'sandstone' => self::SANDSTONE,
			default => self::AIR,
		};
	}

	/** Nombre del bloque en Java, o null si el id no tiene traducción. */
	public static function blockName(int $id) : ?string{
		return self::BEDROCK_BLOCKS[$id] ?? null;
	}

	/** Estado de Java equivalente a un id de red de Bedrock, para esta versión. */
	public static function bedrockState(int $id, ModernProfile $profile) : int{
		return $profile->blockState(self::BEDROCK_BLOCKS[$id] ?? 'air');
	}

	/** Id de red de Bedrock equivalente a un nombre de bloque de Java, o null. */
	public static function bedrockNetworkIdForName(string $name) : ?int{
		foreach(self::BEDROCK_BLOCKS as $networkId => $blockName){
			if($blockName === $name){
				return (int)$networkId;
			}
		}
		return null;
	}

	/**
	 * Nombre de Java de un estado global para esta versión, o null.
	 *
	 * Inversa de `ModernProfile::blockState()`: se usa para traducir lo que
	 * un cliente moderno quiere colocar (sin inventario no se conoce el item
	 * exacto) y para resolver redes desconocidas a un nombre.
	 */
	public static function blockNameForJavaState(int $state, ModernProfile $profile) : ?string{
		foreach(self::BEDROCK_BLOCKS as $blockName){
			if($profile->blockState($blockName) === $state){
				return $blockName;
			}
		}
		return null;
	}

	/** Id de red de Bedrock equivalente a un estado global de Java, o null. */
	public static function bedrockNetworkIdForJavaState(int $state, ModernProfile $profile) : ?int{
		$name = self::blockNameForJavaState($state, $profile);
		return $name === null ? null : self::bedrockNetworkIdForName($name);
	}

	/**
	 * Datos del chunk: por cada sección, contador de bloques, paleta de estados
	 * y contenedor de biomas. Sin entidades de bloque.
	 *
	 * @param object $chunk columna de Bedrock con `getSectionPayload(int $baseY)`
	 * @param array<int, int>|null $fluidReport si se pasa, se rellena con
	 *                         section => fluido (para el log de depuración del
	 *                         puente: fluidCount debe ser > 0 donde hay agua y
	 *                         `hasFluid()` del cliente lo mira para narrar).
	 */
	public static function fromChunk(object $chunk, ModernProfile $profile, ?array &$fluidReport = null) : string{
		$data = '';
		$fluidCount = $profile->sectionsHaveFluidCount();
		$dataLength = $profile->paletteDataHasLength();
		$plains = $profile->plainsBiome();
		if($fluidReport !== null){
			$fluidReport = [];
		}

		for($section = 0; $section < self::SECTIONS; $section++){
			$baseY = self::MIN_Y + ($section << 4);
			$payload = method_exists($chunk, 'getSectionPayload') ? $chunk->getSectionPayload($baseY) : null;

			if($payload === null || ($payload['palette'] ?? []) === []){
				if($fluidReport !== null){ $fluidReport[$section] = 0; }
				$data .= pack('n', 0) . ($fluidCount ? pack('n', 0) : '')
					. self::single(self::AIR, $dataLength) . self::biomes($dataLength, $plains);
				continue;
			}

			$bits = (int)$payload['bits'];
			$words = $payload['words'];
			$palette = $payload['palette'];

			// Traduce la paleta de Bedrock a estados de Java.
			$states = [];
			$javaPalette = [];
			$slots = [];
			foreach($palette as $index => $id){
				$state = self::bedrockState((int)$id, $profile);
				$states[$index] = $state;
				$slot = $javaPalette[$state] ?? null;
				if($slot === null){
					$slot = count($javaPalette);
					$javaPalette[$state] = $slot;
				}
				$slots[$index] = $slot;
			}

			if(count($javaPalette) === 1){
				$only = array_key_first($javaPalette);
				$fluidSingle = self::isFluid((int)$only) ? self::BLOCK_ENTRIES : 0;
				if($fluidReport !== null){ $fluidReport[$section] = $fluidSingle; }
				$data .= pack('n', $only === self::AIR ? 0 : self::BLOCK_ENTRIES)
					. ($fluidCount ? pack('n', $fluidSingle) : '')
					. self::single((int)$only, $dataLength) . self::biomes($dataLength, $plains);
				continue;
			}

			$outBits = 4;
			while((1 << $outBits) < count($javaPalette)){
				$outBits++;
			}
			$inPerWord = max(1, intdiv(32, max(1, $bits)));
			$inMask = $bits === 0 ? 0 : (1 << $bits) - 1;
			$outPerLong = intdiv(64, $outBits);
			$longs = intdiv(self::BLOCK_ENTRIES + $outPerLong - 1, $outPerLong);
			$packed = array_fill(0, $longs, 0);
			$count = 0;
			$fluids = 0;

			// Bedrock indexa `x << 8 | z << 4 | y`; Java, `y << 8 | z << 4 | x`.
			// El recorrido es en orden de Java, así que cada entrada Java se lee
			// del índice de Bedrock con los ejes reordenados (si se leen del
			// mismo índice el mundo sale con X e Y intercambiadas).
			for($entry = 0; $entry < self::BLOCK_ENTRIES; $entry++){
				$source = (($entry & 0x0F) << 8) | ($entry & 0xF0) | (($entry >> 8) & 0x0F);
				$slotIndex = $bits === 0
					? 0
					: (($words[intdiv($source, $inPerWord)] >> (($source % $inPerWord) * $bits)) & $inMask);
				$state = $states[$slotIndex] ?? self::AIR;
				if($state !== self::AIR){
					$count++;
				}
				if(self::isFluid($state)){
					$fluids++;
				}
				$packed[intdiv($entry, $outPerLong)] |= $slots[$slotIndex] << (($entry % $outPerLong) * $outBits);
			}

			if($fluidReport !== null){ $fluidReport[$section] = $fluids; }
			$data .= pack('n', $count)
				. ($fluidCount ? pack('n', $fluids) : '')
				. chr($outBits) . Protocol::varInt(count($javaPalette));
			foreach($javaPalette as $state => $unused){
				$data .= Protocol::varInt((int)$state);
			}
			if($dataLength){
				$data .= Protocol::varInt($longs);
			}
			foreach($packed as $word){
				$data .= Protocol::long($word);
			}
			$data .= self::biomes($dataLength, $plains);
		}

		return $data;
	}

	/**
	 * Estado de bloque con fluido no vacío (agua 86-101 o lava 102-117 en
	 * todas las versiones, ver blocks.json de cada versión en target).
	 *
	 * En 26.1 el cliente decide si hay fluido en una sección con el
	 * `fluidCount` del paquete (`LevelChunkSection.hasFluid()`): con 0,
	 * `EntityFluidInteraction` ni siquiera examina los bloques y el jugador
	 * cae por el agua como si fuera aire, aunque la paleta sea correcta.
	 */
	private static function isFluid(int $state) : bool{
		return $state >= 86 && $state <= 117;
	}

	/**
	 * Alturas reales de las 256 columnas del chunk, para los heightmaps.
	 *
	 * @return int[] índice z*16+x, en Y de Bedrock
	 */
	public static function heightsFromChunk(object $chunk) : array{
		$heights = [];
		for($z = 0; $z < 16; $z++){
			for($x = 0; $x < 16; $x++){
				$heights[($z << 4) | $x] = (int)($chunk->getHighestBlockAt($x, $z) ?? self::MIN_Y);
			}
		}
		return $heights;
	}

	/** Paleta de valor único: sin longs y, desde 1.21.5, sin longitud. */
	private static function single(int $state, bool $dataLength) : string{
		return chr(0) . Protocol::varInt($state) . ($dataLength ? Protocol::varInt(0) : '');
	}

	/**
	 * Biomas de la sección: paleta de un solo valor (plains), sin array de datos.
	 *
	 * Antes se enviaba `bits=1` con una paleta de dos entradas y un único long,
	 * pero con 1 bit por entrada el cliente espera 64 longs (4096 entradas): se
	 * comía 504 bytes de la sección siguiente y el mundo salía descolocado. Con
	 * 0 bits la paleta es un valor único y no hay datos que leer.
	 *
	 * El valor es el índice de `minecraft:plains` en el registro de biomas de
	 * cada versión (39 en 769-773, 40 en 774-775), no 0: el 0 es
	 * `minecraft:badlands`, no la llanura que dice el comentario.
	 */
	private static function biomes(bool $dataLength, int $plains) : string{
		// En 1.21.4 el array de datos de cualquier contenedor lleva delante su
		// longitud (0 longs aquí); a partir de 1.21.5 la longitud no viaja.
		return chr(0) . Protocol::varInt($plains) . ($dataLength ? Protocol::varInt(0) : '');
	}

	/**
	 * Heightmaps: MOTION_BLOCKING con la altura real de cada columna (9 bits por
	 * entrada, 7 por long, 37 longs para las 384 alturas del overworld).
	 *
	 * El cliente usa este mapa para recalcular la luz de cielo.
	 *
	 * @param array<int, int> $heights 256 alturas (índice z*16+x) en Y de Bedrock
	 */

	public static function heightmaps(array $heights, ModernProfile $profile) : string{
		$values = [];
		for($i = 0; $i < 256; $i++){
			// El valor es la altura sobre MIN_Y del primer bloque libre.
			$values[$i] = max(0, min(511, ($heights[$i] ?? self::MIN_Y) + 1 - self::MIN_Y));
		}
		$longs = self::packValues($values, 9);
		if($profile->heightmapsAsNbt()){
			return Nbt::longArrayCompound('MOTION_BLOCKING', $longs);
		}
		return Protocol::varInt(1) . Protocol::varInt(4) . Protocol::varInt(count($longs))
			. implode('', array_map(static fn(int $long) : string => Protocol::long($long), $longs));
	}

	/**
	 * Empaqueta valores en longs de 64 bits, `$bits` por entrada y little-endian
	 * dentro del long (mismo esquema que el array de la paleta).
	 *
	 * @param array<int, int> $values
	 * @return list<int>
	 */
	private static function packValues(array $values, int $bits) : array{
		$perLong = intdiv(64, $bits);
		$mask = (1 << $bits) - 1;
		$longs = [];
		$count = count($values);
		for($index = 0; $index < $count; $index += $perLong){
			$word = 0;
			for($entry = 0; $entry < $perLong && $index + $entry < $count; $entry++){
				$word |= ($values[$index + $entry] & $mask) << ($entry * $bits);
			}
			$longs[] = $word;
		}
		return $longs;
	}

	/**
	 * Campos de luz del paquete `map_chunk`: cielo a 15 en todas las secciones
	 * y luz de bloque vacía.
	 */
	public static function light() : string{
		if(self::$lightPayload !== null){
			return self::$lightPayload;
		}

		$mask = (1 << self::LIGHT_SECTIONS) - 1;
		$full = str_repeat("\xFF", 2048);

		$out = Protocol::varInt(1) . Protocol::long($mask); // sky light mask
		$out .= Protocol::varInt(0); // block light mask
		$out .= Protocol::varInt(0); // empty sky light mask
		$out .= Protocol::varInt(1) . Protocol::long($mask); // empty block light mask
		$out .= Protocol::varInt(self::LIGHT_SECTIONS);
		for($section = 0; $section < self::LIGHT_SECTIONS; $section++){
			$out .= Protocol::varInt(2048) . $full;
		}
		$out .= Protocol::varInt(0); // block light arrays

		return self::$lightPayload = $out;
	}
}
