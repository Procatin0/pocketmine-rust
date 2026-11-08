<?php

declare(strict_types=1);

namespace javabridge;

/**
 * Primitivas binarias del protocolo de Minecraft: Java Edition 1.12.2
 * (protocolo 340): VarInt, cadenas, posiciones empaquetadas y frames.
 *
 * Los ids de paquete y el orden de campos salen de
 * PrismarineJS/minecraft-data (`data/pc/1.12.2/protocol.json`), el mismo
 * dataset que usa el paquete `minecraft-protocol`.
 */
final class Protocol{

	public const PROTOCOL_VERSION = 340;
	public const VERSION_NAME = "1.12.2";
	public const MAX_FRAME_LENGTH = 2097152;

	// Handshake / status / login
	public const C2S_HANDSHAKE = 0x00;
	public const C2S_STATUS_REQUEST = 0x00;
	public const C2S_STATUS_PING = 0x01;
	public const S2C_STATUS_RESPONSE = 0x00;
	public const S2C_STATUS_PONG = 0x01;
	public const C2S_LOGIN_START = 0x00;
	public const S2C_LOGIN_DISCONNECT = 0x00;
	public const S2C_LOGIN_SUCCESS = 0x02;

	// Play: servidor -> cliente
	public const S2C_BLOCK_CHANGE = 0x0B;
	public const S2C_CHAT = 0x0F;
	public const S2C_KICK = 0x1A;
	public const S2C_UNLOAD_CHUNK = 0x1D;
	public const S2C_KEEP_ALIVE = 0x1F;
	public const S2C_MAP_CHUNK = 0x20;
	public const S2C_JOIN_GAME = 0x23;
	public const S2C_ABILITIES = 0x2C;
	public const S2C_PLAYER_INFO = 0x2E;
	public const S2C_POSITION = 0x2F;
	public const S2C_UPDATE_HEALTH = 0x41;
	public const S2C_SPAWN_POSITION = 0x46;
	public const S2C_UPDATE_TIME = 0x47;
	public const S2C_DIFFICULTY = 0x0D;

	// Play: entidades de jugador (1.12.2, protocolo 340). Ids de
	// `data/pc/1.12.2/protocol.json` (minecraft-data): el spawn es el
	// paquete dedicado `named_entity_spawn` (0x05), no el genérico.
	public const S2C_NAMED_ENTITY_SPAWN = 0x05;
	public const S2C_ENTITY = 0x25;
	public const S2C_REL_ENTITY_MOVE = 0x26;
	public const S2C_ENTITY_MOVE_LOOK = 0x27;
	public const S2C_ENTITY_LOOK = 0x28;
	public const S2C_ENTITY_HEAD_ROTATION = 0x36;
	public const S2C_ENTITY_DESTROY = 0x32;
	public const S2C_ENTITY_METADATA = 0x3C;
	public const S2C_ENTITY_TELEPORT = 0x4C;

	// Play: cliente -> servidor
	public const C2S_TELEPORT_CONFIRM = 0x00;
	public const C2S_CHAT = 0x02;
	public const C2S_CLIENT_COMMAND = 0x03;
	public const C2S_SETTINGS = 0x04;
	public const C2S_KEEP_ALIVE = 0x0B;
	public const C2S_FLYING = 0x0C;
	public const C2S_POSITION = 0x0D;
	public const C2S_POSITION_LOOK = 0x0E;
	public const C2S_LOOK = 0x0F;
	public const C2S_BLOCK_DIG = 0x14;
	public const C2S_ARM_ANIMATION = 0x1D;
	public const C2S_BLOCK_PLACE = 0x1F;

	/**
	 * Lee un VarInt sin consumir nada si el buffer está incompleto.
	 *
	 * @param int $offset posición de lectura, actualizada sólo si hubo éxito
	 */
	public static function readVarInt(string $buffer, int &$offset) : ?int{
		$position = $offset;
		$value = 0;
		$shift = 0;
		$length = strlen($buffer);
		while($position < $length){
			$byte = ord($buffer[$position]);
			$position++;
			$value |= ($byte & 0x7F) << $shift;
			if(($byte & 0x80) === 0){
				$offset = $position;
				return $value;
			}
			$shift += 7;
			if($shift > 28){
				return null;
			}
		}
		return null;
	}

	public static function varInt(int $value) : string{
		$value &= 0xFFFFFFFF;
		$out = '';
		do{
			$byte = $value & 0x7F;
			$value >>= 7;
			if($value !== 0){
				$byte |= 0x80;
			}
			$out .= chr($byte);
		}while($value !== 0);
		return $out;
	}

	/**
	 * VarLong de Java (LEB128 de 64 bits, sin el enmascarado a 32 bits
	 * de `varInt`). Para los valores no negativos que usa el puente
	 * (`totalTicks` de `update_time` en 26.1) la codificación coincide
	 * con `varInt`; se mantiene como helper propio para no confundir
	 * ambos tipos en el cable.
	 */
	public static function varLong(int $value) : string{
		$out = '';
		$remaining = $value;
		do{
			$byte = $remaining & 0x7F;
			$remaining >>= 7;
			if($remaining !== 0 && $remaining !== -1){
				$byte |= 0x80;
			}
			$out .= chr($byte);
		}while($remaining !== 0 && $remaining !== -1);
		return $out;
	}

	/** Cadena con longitud VarInt. Devuelve null si aún no está completa. */
	public static function readString(string $buffer, int &$offset) : ?string{
		$position = $offset;
		$length = self::readVarInt($buffer, $position);
		if($length === null || $length < 0 || $length > 32767 * 4){
			return null;
		}
		if(strlen($buffer) - $position < $length){
			return null;
		}
		$string = substr($buffer, $position, $length);
		$offset = $position + $length;
		return $string;
	}

	public static function string(string $value) : string{
		return self::varInt(strlen($value)) . $value;
	}

	/**
	 * Identificador con namespace (`minecraft:water`, `minecraft:brand`...).
	 * En el cable es una cadena con longitud VarInt, igual que `string()`;
	 * el alias existe para que el llamador no confunda un id de registro
	 * o de canal con texto libre.
	 */
	public static function identifier(string $value) : string{
		return self::string($value);
	}

	/** Array de VarInt con prefijo de longitud (p. ej. entradas de un tag). */
	public static function varIntArray(array $values) : string{
		$out = self::varInt(count($values));
		foreach($values as $value){
			$out .= self::varInt((int)$value);
		}
		return $out;
	}

	/**
	 * Lee un array de VarInt de `varIntArray()`.
	 *
	 * @return list<int>|null null si el buffer aún no está completo
	 */
	public static function readVarIntArray(string $buffer, int &$offset) : ?array{
		$position = $offset;
		$count = self::readVarInt($buffer, $position);
		if($count === null || $count < 0){
			return null;
		}
		$values = [];
		for($i = 0; $i < $count; $i++){
			$value = self::readVarInt($buffer, $position);
			if($value === null){
				return null;
			}
			$values[] = $value;
		}
		$offset = $position;
		return $values;
	}

	public static function readUInt16(string $buffer, int &$offset) : ?int{
		if(strlen($buffer) - $offset < 2){
			return null;
		}
		$value = unpack('n', substr($buffer, $offset, 2))[1];
		$offset += 2;
		return $value;
	}

	public static function readLong(string $buffer, int &$offset) : ?int{
		if(strlen($buffer) - $offset < 8){
			return null;
		}
		// Los bits se conservan aunque el valor desborde PHP_INT_MAX.
		$value = unpack('J', substr($buffer, $offset, 8))[1];
		$offset += 8;
		return is_int($value) ? $value : (int)$value;
	}

	public static function readDouble(string $buffer, int &$offset) : ?float{
		if(strlen($buffer) - $offset < 8){
			return null;
		}
		$value = unpack('E', substr($buffer, $offset, 8))[1];
		$offset += 8;
		return (float)$value;
	}

	public static function readFloat(string $buffer, int &$offset) : ?float{
		if(strlen($buffer) - $offset < 4){
			return null;
		}
		$value = unpack('G', substr($buffer, $offset, 4))[1];
		$offset += 4;
		return (float)$value;
	}

	/**
	 * Posición empaquetada: x (26 bits) | y (12 bits) | z (26 bits).
	 *
	 * Se extrae a partir de las dos mitades de 32 bits para no depender del
	 * desbordamiento de enteros con signo de PHP.
	 */
	public static function readPosition(string $buffer, int &$offset) : ?array{
		if(strlen($buffer) - $offset < 8){
			return null;
		}
		$parts = unpack('Nhigh/Nlow', substr($buffer, $offset, 8));
		$offset += 8;
		$high = (int)$parts['high'];
		$low = (int)$parts['low'];

		$x = $high >> 6; // 26 bits superiores
		$y = (($high & 0x3F) << 6) | ($low >> 26);
		$z = $low & 0x3FFFFFF;
		if($x >= 0x2000000){
			$x -= 0x4000000;
		}
		if($z >= 0x2000000){
			$z -= 0x4000000;
		}
		// Y es de 12 bits con signo: en 1.18+ el mundo empieza en Y=-64.
		$y &= 0xFFF;
		if($y >= 0x800){
			$y -= 0x1000;
		}
		return [$x, $y, $z];
	}

	public static function position(int $x, int $y, int $z) : string{
		$packed = (($x & 0x3FFFFFF) << 38) | (($y & 0xFFF) << 26) | ($z & 0x3FFFFFF);
		return self::long($packed);
	}

	public static function long(int $value) : string{
		$out = '';
		for($byte = 7; $byte >= 0; $byte--){
			$out .= chr(($value >> ($byte * 8)) & 0xFF);
		}
		return $out;
	}

	public static function double(float $value) : string{
		return pack('E', $value);
	}

	public static function float(float $value) : string{
		return pack('G', $value);
	}

	public static function ubyte(int $value) : string{
		return chr($value & 0xFF);
	}

	public static function boolean(bool $value) : string{
		return chr($value ? 1 : 0);
	}

	/** Byte con signo (i8): el ángulo se transporta como grado*256/360. */
	public static function byte(int $value) : string{
		return chr($value & 0xFF);
	}

	/** Short con signo (i16) big-endian: deltas relativos *4096. */
	public static function short(int $value) : string{
		return pack('n', $value & 0xFFFF);
	}

	/**
	 * Ángulo en grados (-180..180 yaw, -90..90 pitch) a byte con signo.
	 *
	 * `byte = floor((angle * 256 / 360) % 256)`; con signo para el cliente.
	 */
	public static function angle(float $degrees) : int{
		$scaled = (int)floor(fmod($degrees * 256.0 / 360.0 + 360.0, 256.0));
		return $scaled >= 128 ? $scaled - 256 : $scaled;
	}

	/**
	 * Yaw Bedrock (0 = norte, 90 = este, 180 = sur, 270 = oeste) a yaw Java
	 * (0 = sur, 90 = oeste, 180 = norte, -90 = este): mismo sentido de giro,
	 * cero opuesto, así que la conversión es `+180` módulo 360, normalizada
	 * al rango Java (-180, 180].
	 *
	 * Sin esta conversión el cliente Java ve a los jugadores Bedrock
	 * mirando hacia atrás (180° opuestos). El registro canónico de Rust y
	 * la copia `apiPlayer` guardan el yaw Bedrock; sólo los paquetes Java
	 * llevan el yaw Java.
	 */
	public static function bedrockYawToJava(float $yaw) : float{
		$y = fmod($yaw + 180.0, 360.0);
		if($y < 0.0){
			$y += 360.0;
		}
		if($y > 180.0){
			$y -= 360.0;
		}
		return $y;
	}

	/**
	 * Yaw Java (-180, 180] a yaw Bedrock [0, 360): la inversa de
	 * `bedrockYawToJava()` (también `+180` módulo 360, el ciclo es una
	 * involución). Es la que se publica en el registry de Rust y en la
	 * copia `apiPlayer` cuando el jugador Java se mueve.
	 */
	public static function javaYawToBedrock(float $yaw) : float{
		$y = fmod($yaw + 180.0, 360.0);
		if($y < 0.0){
			$y += 360.0;
		}
		return $y;
	}

	/**
	 * Pitch Bedrock a pitch Java: mismo signo en ambas ediciones
	 * (-90 = mirar arriba, +90 = mirar abajo), pero Rust normaliza el
	 * pitch Bedrock a [0, 360) (`session.rs`), así que se pliega de
	 * vuelta a (-180, 180] para los paquetes Java (float e i8).
	 */
	public static function bedrockPitchToJava(float $pitch) : float{
		$p = fmod($pitch, 360.0);
		if($p < 0.0){
			$p += 360.0;
		}
		if($p > 180.0){
			$p -= 360.0;
		}
		return $p;
	}

	/** Desplazamiento relativo en bloques a i16 (*4096, con clamp). */
	public static function relDelta(float $delta) : int{
		$scaled = (int)round($delta * 4096.0);
		return max(-32768, min(32767, $scaled));
	}

	/**
	 * Vector de baja precisión `lpVec3` (1.21.9+, paquete `spawn_entity` y
	 * `entity_velocity` en `protocol.json`: `velocity: lpVec3`, tipo
	 * `native` implementado en `minecraft-protocol/src/datatypes/lpVec3.js`).
	 *
	 * El vector cero es un único byte `0x00`. En caso contrario van 6 bytes
	 * (tres componentes cuantizadas de 15 bits + escala en el primer byte)
	 * y, si la escala es mayor de 3, un VarInt adicional con `floor(scale/4)`.
	 * Es el formato que el cliente 26.1 (775) espera entre `z` y `pitch` en
	 * `spawn_entity`; mandar ahí el byte de `pitch` + `vec3i16` final (formato
	 * 769/770) hace que el cliente lo lea como cabecera `lpVec3` y aborte con
	 * `DecoderException: Failed to decode packet 'clientbound/minecraft:add_entity'`.
	 */
	public static function lpVec3(float $x, float $y, float $z) : string{
		$absMax = 1.7179869183e10;
		$absMin = 3.051944088384301e-5;
		$sanitize = static fn(float $value) : float => is_nan($value)
			? 0.0
			: max(-$absMax, min($value, $absMax));
		$x = $sanitize($x);
		$y = $sanitize($y);
		$z = $sanitize($z);
		$max = max(abs($x), abs($y), abs($z));
		if($max < $absMin){
			return "\x00";
		}
		$scale = (int)ceil($max);
		$pack = static fn(float $value) : int => (int)round(($value * 0.5 + 0.5) * 32766.0);
		$needsContinuation = $scale > 3;
		$markers = $needsContinuation ? (($scale % 4) | 4) : $scale;
		$packed = $markers
			+ $pack($x / $scale) * 0x8
			+ $pack($y / $scale) * 0x40000
			+ $pack($z / $scale) * 0x200000000;
		$bytes = chr($packed & 0xFF)
			. chr(($packed >> 8) & 0xFF)
			. pack('N', ($packed >> 16) & 0xFFFFFFFF);
		if($needsContinuation){
			$bytes .= self::varInt((int)floor($scale / 4));
		}
		return $bytes;
	}

	/** Frame completo: varint(longitud) + varint(id) + payload. */
	public static function frame(int $id, string $payload) : string{
		$body = self::varInt($id) . $payload;
		return self::varInt(strlen($body)) . $body;
	}

	/** Componente de chat JSON simple: {"text":"..."} */
	public static function chatJson(string $text) : string{
		$encoded = json_encode(['text' => $text], JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE);
		return $encoded === false ? '{"text":""}' : $encoded;
	}

	/**
	 * UUID offline-mode de Minecraft: MD5 de "OfflinePlayer:<nombre>" con los
	 * bits de versión 3 y variante RFC 4122.
	 *
	 * @return string 16 bytes crudos
	 */
	public static function offlineUuid(string $name) : string{
		$hash = md5('OfflinePlayer:' . $name, true);
		$hash[6] = chr((ord($hash[6]) & 0x0F) | 0x30);
		$hash[8] = chr((ord($hash[8]) & 0x3F) | 0x80);
		return $hash;
	}

	/** Formato 8-4-4-4-12 que espera `login_success`. */
	public static function uuidString(string $raw) : string{
		$hex = bin2hex($raw);
		return substr($hex, 0, 8) . '-' . substr($hex, 8, 4) . '-' . substr($hex, 12, 4)
			. '-' . substr($hex, 16, 4) . '-' . substr($hex, 20, 12);
	}
}
