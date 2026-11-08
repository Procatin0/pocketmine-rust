<?php

declare(strict_types=1);

/*
 * Comprueba que la columna que dibuja el cliente Java es el mundo real.
 *
 * Se parte de una columna generada por el servidor Rust de verdad
 * (`cargo run -p bedrock-world --example dump_column -- <cx> <cz> <prefijo>`),
 * que escribe `<prefijo>.bin` (payload de `world.get_chunk`) y `<prefijo>.txt`
 * (verdad: todas las coordenadas con bloque, en ids de red de Bedrock).
 *
 * Después se comprueba dos cosas:
 *  1. que `pocketmine\world\Chunk` (el decodificador del runtime del puente)
 *     lea esos bloques en su coordenada, y
 *  2. que la columna de `ModernChunk::fromChunk()` tenga el mismo bloque en la
 *     misma coordenada, leyéndola con el índice de Java (`y << 8 | z << 4 | x`).
 *
 * Uso: php tests/real_column_test.php <prefijo> [protocolo]
 */

namespace pocketmine\runtime {

	final class Bridge{
		public static string $payload = '';
		public static int $subchunks = 24;

		public static function isActive() : bool{ return true; }

		public static function hostCall(string $method, array $params = []) : mixed{
			if($method === 'world.get_chunk'){
				return ['subchunks' => self::$subchunks, 'data' => bin2hex(self::$payload)];
			}
			return null;
		}
	}
}

namespace pocketmine\math {

	class Vector3{
		public function __construct(protected float $x = 0.0, protected float $y = 0.0, protected float $z = 0.0){}
		public function getX() : float{ return $this->x; }
		public function getY() : float{ return $this->y; }
		public function getZ() : float{ return $this->z; }
	}

	class Position extends Vector3{}
}

namespace {

	require dirname(__DIR__, 3) . '/crates/php-plugin-bridge/php/world.php';
	require dirname(__DIR__) . '/src/javabridge/Protocol.php';
	require dirname(__DIR__) . '/src/javabridge/ModernProfile.php';
	require dirname(__DIR__) . '/src/javabridge/ModernChunk.php';

	use javabridge\ModernChunk;
	use javabridge\ModernProfile;
	use pocketmine\world\Chunk;

	$failures = 0;
	function check(string $name, bool $ok, string $detail = '') : void{
		global $failures;
		if(!$ok){
			$failures++;
			fwrite(STDERR, "FAIL: {$name}" . ($detail === '' ? '' : " ({$detail})") . "\n");
		}else{
			fwrite(STDOUT, "ok: {$name}\n");
		}
	}

	function readVarInt(string $data, int &$offset) : int{
		$value = 0;
		$shift = 0;
		$length = strlen($data);
		while($offset < $length){
			$byte = ord($data[$offset++]);
			$value |= ($byte & 0x7F) << $shift;
			if(($byte & 0x80) === 0){
				break;
			}
			$shift += 7;
		}
		return $value;
	}

	/**
	 * Decodifica la columna de Java (formato de red 1.18+) a estados por
	 * entrada, con el índice que usa el cliente: `y << 8 | z << 4 | x`.
	 *
	 * @return array<int, array<int, int>> sección => (entrada => estado)
	 */
	function decodeJavaSections(string $data, ModernProfile $profile) : array{
		global $fluidMismatch, $biomeMismatch;
		$fluid = $profile->sectionsHaveFluidCount();
		$dataLength = $profile->paletteDataHasLength();
		$offset = 0;
		$sections = [];
		for($section = 0; $section < 24; $section++){
			$count = unpack('n', substr($data, $offset, 2))[1];
			$offset += 2;
			$fluidCount = 0;
			if($fluid){
				$fluidCount = unpack('n', substr($data, $offset, 2))[1];
				$offset += 2;
			}
			$bits = ord($data[$offset++]);
			$entries = [];
			if($bits === 0){
				$state = readVarInt($data, $offset);
				if($dataLength){
					readVarInt($data, $offset);
				}
				$entries = array_fill(0, 4096, $state);
			}else{
				$paletteSize = readVarInt($data, $offset);
				$palette = [];
				for($i = 0; $i < $paletteSize; $i++){
					$palette[] = readVarInt($data, $offset);
				}
				$perLong = intdiv(64, $bits);
				$longs = $dataLength ? readVarInt($data, $offset) : intdiv(4096 + $perLong - 1, $perLong);
				$words = [];
				for($i = 0; $i < $longs; $i++){
					$words[] = unpack('J', substr($data, $offset, 8))[1];
					$offset += 8;
				}
				$mask = (1 << $bits) - 1;
				for($entry = 0; $entry < 4096; $entry++){
					$word = $words[intdiv($entry, $perLong)] ?? 0;
					$index = ($word >> (($entry % $perLong) * $bits)) & $mask;
					$entries[$entry] = $palette[$index] ?? 0;
				}
			}
			if($fluid){
				$expectedFluids = 0;
				foreach($entries as $entryState){
					if($entryState >= 86 && $entryState <= 117){
						$expectedFluids++;
					}
				}
				if($expectedFluids !== (int)$fluidCount){
					$fluidMismatch++;
					fwrite(STDERR, "sección {$section}: fluidCount {$fluidCount} != fluidos reales {$expectedFluids}\n");
				}
				$expectedBlocks = 0;
				foreach($entries as $entryState){
					if($entryState !== 0){
						$expectedBlocks++;
					}
				}
				if($expectedBlocks !== (int)$count){
					fwrite(STDERR, "sección {$section}: blockCount {$count} != bloques reales {$expectedBlocks}\n");
				}
			}
			// Contenedor de biomas de la sección (mismo formato: bits + paleta + datos).
			$biomeBits = ord($data[$offset++]);
			if($biomeBits === 0){
				$biomeValue = readVarInt($data, $offset);
				if($biomeValue !== $profile->plainsBiome()){
					$biomeMismatch++;
					fwrite(STDERR, "sección {$section}: bioma {$biomeValue} != plains {$profile->plainsBiome()}\n");
				}
				if($dataLength){
					readVarInt($data, $offset);
				}
			}else{
				$biomeSize = readVarInt($data, $offset);
				for($i = 0; $i < $biomeSize; $i++){
					readVarInt($data, $offset);
				}
				$biomePerLong = intdiv(64, $biomeBits);
				$biomeLongs = $dataLength ? readVarInt($data, $offset) : intdiv(4096 + $biomePerLong - 1, $biomePerLong);
				$offset += 8 * $biomeLongs;
			}

			$sections[$section] = $entries;
			if($count < 0){
				fwrite(STDERR, "sección {$section} con contador negativo\n");
			}
		}
		return $sections;
	}

	$prefix = $argv[1] ?? '/tmp/column_0_0';
	$protocol = (int)($argv[2] ?? 775);

	$payload = @file_get_contents("{$prefix}.bin");
	$truthText = @file_get_contents("{$prefix}.txt");
	if($payload === false || $truthText === false){
		fwrite(STDERR, "Faltan {$prefix}.bin / {$prefix}.txt (genera con dump_column)\n");
		exit(1);
	}

	$lines = explode("\n", trim($truthText));
	$subchunks = 24;
	$blocks = [];
	foreach($lines as $line){
		if($line === ''){
			continue;
		}
		if(str_starts_with($line, '#')){
			if(preg_match('/subchunks (\d+)/', $line, $match) === 1){
				$subchunks = (int)$match[1];
			}
			continue;
		}
		$blocks[] = array_map('intval', explode(' ', $line));
	}

	$profile = new ModernProfile($protocol);
	printf(
		"%s: %d bytes, %d subsecciones, %d bloques, protocolo %d (%s)\n",
		$prefix, strlen($payload), $subchunks, count($blocks), $protocol, $profile->name
	);

	// 1. El decodificador del runtime lee cada bloque en su sitio.
	$chunk = new Chunk($payload, $subchunks);
	$mismatch = 0;
	$firstBedrock = '';
	foreach($blocks as [$x, $y, $z, $id]){
		$got = $chunk->getBlockStateId($x & 15, $y, $z & 15);
		if($got !== $id){
			$mismatch++;
			if($firstBedrock === ''){
				$firstBedrock = "({$x},{$y},{$z}) esperado {$id}, leído {$got}";
			}
		}
	}
	check('world.get_chunk: cada bloque está en su coordenada', $mismatch === 0, $firstBedrock);

	// 2. La columna de Java tiene el mismo bloque en la misma coordenada.
	$fluidMismatch = 0;
	$biomeMismatch = 0;
	$java = decodeJavaSections(ModernChunk::fromChunk($chunk, $profile), $profile);
	check('columna de Java: contador de fluidos real (26.1)', $fluidMismatch === 0, (string)$fluidMismatch);
	check('columna de Java: bioma plains, no badlands', $biomeMismatch === 0, (string)$biomeMismatch);
	$wrong = 0;
	$firstJava = '';
	foreach($blocks as [$x, $y, $z, $id]){
		$state = ModernChunk::bedrockState($id, $profile);
		$relative = $y + 64;
		$section = $relative >> 4;
		$index = (($relative & 15) << 8) | (($z & 15) << 4) | ($x & 15);
		$got = $java[$section][$index] ?? -1;
		if($got !== $state){
			$wrong++;
			if($firstJava === ''){
				$firstJava = "({$x},{$y},{$z}) id {$id} esperado {$state}, enviado {$got}";
			}
		}
	}
	check('columna de Java: mismos bloques en las mismas coordenadas', $wrong === 0, $firstJava);

	// 3. Tampoco aparecen bloques de más (aire donde Bedrock tiene aire).
	$javaNonAir = 0;
	foreach($java as $entries){
		foreach($entries as $state){
			if($state !== ModernChunk::AIR){
				$javaNonAir++;
			}
		}
	}
	check(
		'columna de Java: sin bloques inventados',
		$javaNonAir === count($blocks),
		"java={$javaNonAir} bedrock=" . count($blocks)
	);

	// 4. Mapa de alturas para verlo de un vistazo: superficie de Bedrock y Java.
	$bedrockSurface = [];
	$javaSurface = [];
	for($z = 0; $z < 16; $z++){
		for($x = 0; $x < 16; $x++){
			$bedrockSurface[$z][$x] = $chunk->getHighestBlockAt($x, $z) ?? -64;
			$height = -64;
			for($section = 23; $section >= 0; $section--){
				$found = false;
				for($y = 15; $y >= 0; $y--){
					$index = ($y << 8) | ($z << 4) | $x;
					if(($java[$section][$index] ?? 0) !== ModernChunk::AIR){
						$height = ($section << 4) + $y - 64;
						$found = true;
						break;
					}
				}
				if($found){
					break;
				}
			}
			$javaSurface[$z][$x] = $height;
		}
	}
	$surfaceMismatch = 0;
	$firstSurface = '';
	for($z = 0; $z < 16; $z++){
		for($x = 0; $x < 16; $x++){
			if($bedrockSurface[$z][$x] !== $javaSurface[$z][$x]){
				$surfaceMismatch++;
				if($firstSurface === ''){
					$firstSurface = "({$x},{$z}) bedrock {$bedrockSurface[$z][$x]} java {$javaSurface[$z][$x]}";
				}
			}
		}
	}
	check('superficie idéntica columna a columna', $surfaceMismatch === 0, $firstSurface);

	fwrite(STDOUT, "\nSuperficie (Y del primer bloque libre) alrededor del chunk:\n");
	for($z = 0; $z < 16; $z++){
		$row = '';
		for($x = 0; $x < 16; $x++){
			$row .= sprintf('%4d', $javaSurface[$z][$x]);
		}
		fwrite(STDOUT, $row . "\n");
	}

	if($failures > 0){
		fwrite(STDERR, "\n{$failures} comprobación(es) fallida(s)\n");
		exit(1);
	}
	fwrite(STDOUT, "\nALL OK\n");
}
