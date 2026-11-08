<?php

declare(strict_types=1);

namespace javabridge;

/**
 * Construye una columna de chunk de Java 1.12.2 (16x16x256) a partir de un
 * mapa de alturas.
 *
 * Formato de `map_chunk` (0x20) para el protocolo 340, ground-up continuous:
 * por cada sección presente en la máscara, en orden ascendente de Y:
 * `bits_per_block (u8)`, paleta (`varint` de longitud + estados), datos
 * (`varint` de longs + longs big-endian), luz de bloque (2048 bytes) y luz de
 * cielo (2048 bytes). Después de las secciones, 256 bytes de bioma.
 *
 * 1.12.2 usa empaquetado *no compacto*: un valor nunca cruza dos longs.
 *
 * Los bloques vienen del chunk real del servidor (`world.get_chunk`), así que
 * el terreno, el agua, la arena y los árboles coinciden con el mundo Bedrock.
 */
final class ChunkBuilder{

	private const MIN_BITS = 4;
	private const BLOCK_LIGHT_BYTES = 2048;
	private const SKY_LIGHT_BYTES = 2048;

	// Estados de bloque de Java 1.12.2: (id legacy << 4) | metadata.
	public const JAVA_AIR = 0;
	public const JAVA_STONE = 1 << 4;
	public const JAVA_GRASS = 2 << 4;
	public const JAVA_DIRT = 3 << 4;
	public const JAVA_BEDROCK = 7 << 4;

	/**
	 * Ids de red de Bedrock (hash FNV-1a) y su estado en Java 1.12.2.
	 *
	 * Los bloques cuyo id cambia entre versiones de Java (diamante, redstone,
	 * esmeralda) se dibujan como piedra.
	 */
	private const BEDROCK_STATES = [
		3690217760 => self::JAVA_AIR,
		2150698529 => self::JAVA_STONE,
		2186211206 => self::JAVA_DIRT,
		3727763636 => self::JAVA_GRASS,
		4121722107 => self::JAVA_BEDROCK,
		138639715 => 12 << 4,           // sand
		1529044762 => 13 << 4,          // gravel
		1211861802 => 9 << 4,           // water
		2754680827 => 24 << 4,          // sandstone		825916963 => 17 << 4,           // oak_log (eje Y)
		2110714365 => 18 << 4,          // oak_leaves
		507450469 => (17 << 4) | 1,     // spruce_log
		1404704691 => (18 << 4) | 1,    // spruce_leaves
		1423805714 => (17 << 4) | 2,    // birch_log
		1740500456 => (18 << 4) | 2,    // birch_leaves
		685383673 => 16 << 4,           // coal_ore
		2032622302 => 15 << 4,          // iron_ore
		2144082742 => 14 << 4,          // gold_ore
		2930261704 => 21 << 4,          // lapis_ore
		180213920 => 73 << 4,           // redstone_ore
		1419890941 => 56 << 4,          // diamond_ore
		170572189 => 129 << 4,          // emerald_ore
		3699616834 => 78 << 4,          // snow_layer
		2827903781 => (31 << 4) | 1,    // short_grass
		4175527249 => (31 << 4) | 1,    // tall_grass
		2383143704 => 37 << 4,          // dandelion
		3417975282 => 38 << 4,          // poppy
	];

	private static function legacyState(int $bedrockId) : int{
		return self::BEDROCK_STATES[$bedrockId] ?? self::JAVA_AIR;
	}

	/**
	 * Estado 1.12.2 equivalente a un id de red de Bedrock, y si existe la
	 * traducción (los ids desconocidos se dibujan como aire, así que un id sin
	 * traducir borra terreno).
	 */
	public static function javaState(int $bedrockId) : int{
		return self::legacyState($bedrockId);
	}

	public static function isMapped(int $bedrockId) : bool{
		return array_key_exists($bedrockId, self::BEDROCK_STATES);
	}

	/**
	 * Id legacy (Java 1.12 / PM, el que espera `Block::setBlock()`) de un
	 * bloque por nombre, o null si no hay traducción.
	 *
	 * Comparte el sistema numérico de `BlockFactory` del runtime PHP
	 * (`crates/php-plugin-bridge/php/world.php`): stone=1, grass_block=2,
	 * dirt=3, cobblestone=4, bedrock=7, water=9, sand=12, gravel=13...
	 */
	private const NAME_TO_LEGACY = [
		'air' => 0,
		'stone' => 1,
		'grass_block' => 2,
		'dirt' => 3,
		'cobblestone' => 4,
		'bedrock' => 7,
		'water' => 9,
		'sand' => 12,
		'gravel' => 13,
		'gold_ore' => 14,
		'iron_ore' => 15,
		'coal_ore' => 16,
		'oak_log' => 17,
		'oak_leaves' => 18,
		'lapis_ore' => 21,
		'sandstone' => 24,
		'short_grass' => 31,
		'tall_grass' => 31,
		'dandelion' => 37,
		'poppy' => 38,
		'diamond_ore' => 56,
		'redstone_ore' => 73,
		'snow' => 78,
		'emerald_ore' => 129,
	];

	public static function legacyIdForName(string $name) : ?int{
		return self::NAME_TO_LEGACY[$name] ?? null;
	}

	/** Nombre de Java 1.12 de un id legacy, o null. */
	public static function nameForLegacy(int $legacyId) : ?string{
		foreach(self::NAME_TO_LEGACY as $name => $id){
			if($id === $legacyId){
				return $name;
			}
		}
		return null;
	}

	/**
	 * Id legacy de un id de red de Bedrock (estado 1.12 >> 4), o null si
	 * el id no tiene traducción (se dibujaría como aire).
	 */
	public static function legacyIdForBedrock(int $bedrockId) : ?int{
		$state = self::BEDROCK_STATES[$bedrockId] ?? null;
		return $state === null ? null : ($state >> 4);
	}

	/** Id de red de Bedrock de un id legacy (+ meta), o null si no está mapeado. */
	public static function bedrockIdForLegacy(int $legacyId, int $meta = 0) : ?int{
		$wanted = ($legacyId << 4) | ($meta & 0x0F);
		foreach(self::BEDROCK_STATES as $bedrockId => $state){
			if($state === $wanted){
				return (int)$bedrockId;
			}
		}
		return null;
	}

	/**
	 * Construye la columna 1.12.2 desde el chunk real de Bedrock.
	 *
	 * @param object $chunk columna con `getSectionPayload(int $baseY)`
	 * @return array{0:int,1:string} máscara de secciones y datos
	 */
	public static function fromChunk(object $chunk) : array{
		$states = array_fill(0, 16 * 16 * 256, self::JAVA_AIR);

		// Bedrock: 24 subsecciones desde Y=-64. Java 1.12.2 sólo dibuja Y=0..255,
		// así que el mundo se desplaza 64 bloques hacia arriba: el lecho de
		// bedrock de Bedrock (-64) cae en Y=0.
		for($section = 0; $section < 24; $section++){
			$baseY = -64 + ($section << 4);
			if($baseY + 64 > 255){
				break;
			}
			$payload = method_exists($chunk, 'getSectionPayload') ? $chunk->getSectionPayload($baseY) : null;
			if($payload === null || ($payload['palette'] ?? []) === []){
				continue;
			}

			$bits = (int)$payload['bits'];
			$words = $payload['words'];
			$perWord = max(1, intdiv(32, max(1, $bits)));
			$mask = $bits === 0 ? 0 : (1 << $bits) - 1;

			$legacy = [];
			foreach($payload['palette'] as $index => $id){
				$legacy[$index] = self::legacyState((int)$id);
			}

			for($x = 0; $x < 16; $x++){
				for($z = 0; $z < 16; $z++){
					for($y = 0; $y < 16; $y++){
						$worldY = $baseY + $y + 64;
						if($worldY < 0 || $worldY > 255){
							continue;
						}
						$entry = ($x << 8) | ($z << 4) | $y;
						$index = $bits === 0
							? 0
							: (($words[intdiv($entry, $perWord)] >> (($entry % $perWord) * $bits)) & $mask);
						$states[($worldY << 8) | ($z << 4) | $x] = $legacy[$index] ?? self::JAVA_AIR;
					}
				}
			}
		}

		return self::encode($states);
	}

	/**
	 * @param int[] $states 65536 estados, índice (y << 8) | (z << 4) | x
	 * @return array{0:int,1:string}
	 */
	public static function encode(array $states) : array{
		$data = '';
		$bitMap = 0;

		for($section = 0; $section < 16; $section++){
			$base = $section * 4096;
			$slice = array_slice($states, $base, 4096);
			if(max($slice) === self::JAVA_AIR){
				continue;
			}
			$bitMap |= 1 << $section;

			$palette = array_values(array_unique($slice));
			$bits = self::MIN_BITS;
			while((1 << $bits) < count($palette)){
				$bits++;
			}
			if($bits > 8){
				$bits = 8;
				$palette = array_slice($palette, 0, 256);
			}

			$data .= chr($bits);
			$data .= Protocol::varInt(count($palette));
			foreach($palette as $state){
				$data .= Protocol::varInt((int)$state);
			}

			$perLong = intdiv(64, $bits);
			$longCount = intdiv(4096, $perLong);
			$data .= Protocol::varInt($longCount);

			$paletteIndex = array_flip($palette);
			$words = array_fill(0, $longCount, 0);
			foreach($slice as $index => $state){
				$words[intdiv($index, $perLong)] |= ($paletteIndex[$state] ?? 0) << (($index % $perLong) * $bits);
			}
			foreach($words as $word){
				$data .= Protocol::long((int)$word);
			}

			$data .= str_repeat("\x00", self::BLOCK_LIGHT_BYTES);
			$data .= str_repeat("\xFF", self::SKY_LIGHT_BYTES);
		}

		// Bioma: llanura (1) en todas las columnas.
		$data .= str_repeat("\x01", 256);

		return [$bitMap, $data];
	}
}
