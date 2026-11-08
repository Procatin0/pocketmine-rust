<?php

declare(strict_types=1);

/*
 * Comprueba que la columna 1.12.2 (`ChunkBuilder`) es el mundo real.
 *
 * Usa la misma verdad que `real_column_test.php`: `<prefijo>.bin` (payload de
 * `world.get_chunk`) y `<prefijo>.txt` (todas las coordenadas con bloque).
 *
 * Se comprueba:
 *  1. que cada bloque del mundo real está en la misma coordenada (Y + 64) en la
 *     columna 1.12.2, leída con el índice de Java (`y << 8 | z << 4 | x`),
 *  2. que no aparecen bloques de más, y
 *  3. que todos los ids del mundo real tienen traducción a 1.12.2 (un id sin
 *     traducir se dibuja como aire: terreno que desaparece).
 *
 * Uso: php tests/real_column_legacy_test.php <prefijo>
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
	require dirname(__DIR__) . '/src/javabridge/ModernChunk.php';
	require dirname(__DIR__) . '/src/javabridge/ChunkBuilder.php';

	use javabridge\ChunkBuilder;
	use javabridge\ModernChunk;
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
	 * Decodifica `map_chunk` (1.12.2) a estados por sección.
	 *
	 * @return array<int, array<int, int>> sección => (entrada => estado)
	 */
	function decodeLegacy(string $data, int $mask) : array{
		// La máscara viaja aparte en `map_chunk`; el bloque de datos que devuelve
		// `ChunkBuilder::fromChunk()` empieza directamente en la primera sección.
		$offset = 0;
		$sections = [];
		for($section = 0; $section < 16; $section++){
			if((($mask >> $section) & 1) === 0){
				continue;
			}
			$bits = ord($data[$offset++]);
			$paletteSize = readVarInt($data, $offset);
			$palette = [];
			for($i = 0; $i < $paletteSize; $i++){
				$palette[] = readVarInt($data, $offset);
			}
			$longs = readVarInt($data, $offset);
			$words = [];
			for($i = 0; $i < $longs; $i++){
				$words[] = unpack('J', substr($data, $offset, 8))[1];
				$offset += 8;
			}
			$offset += 2048 + 2048; // luz de bloque + luz de cielo

			$perLong = intdiv(64, $bits);
			$maskBits = (1 << $bits) - 1;
			$entries = [];
			for($entry = 0; $entry < 4096; $entry++){
				$word = $words[intdiv($entry, $perLong)] ?? 0;
				$index = ($word >> (($entry % $perLong) * $bits)) & $maskBits;
				$entries[$entry] = $palette[$index] ?? 0;
			}
			$sections[$section] = $entries;
		}
		return $sections;
	}

	$prefix = $argv[1] ?? '/tmp/column_0_0';

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

	$chunk = new Chunk($payload, $subchunks);
	[$mask, $encoded] = ChunkBuilder::fromChunk($chunk);
	$java = decodeLegacy($encoded, $mask);

	printf(
		"%s: %d bytes de columna 1.12.2, máscara 0x%04X, %d bloques, %d subsecciones\n",
		$prefix, strlen($encoded), $mask, count($blocks), $subchunks
	);

	// 1. Mismo bloque en la misma coordenada.
	$wrong = 0;
	$firstWrong = '';
	$unmapped = [];
	$unmappedBlocks = 0;
	foreach($blocks as [$x, $y, $z, $id]){
		$state = ChunkBuilder::javaState($id);
		if(!ChunkBuilder::isMapped($id)){
			$unmapped[$id] = ($unmapped[$id] ?? 0) + 1;
			$unmappedBlocks++;
		}
		$relative = $y + 64;
		if($relative < 0 || $relative > 255){
			continue;
		}
		$section = $relative >> 4;
		$index = (($relative & 15) << 8) | (($z & 15) << 4) | ($x & 15);
		$got = $java[$section][$index] ?? -1;
		if($got !== $state){
			$wrong++;
			if($firstWrong === ''){
				$firstWrong = "({$x},{$y},{$z}) id {$id} esperado {$state}, enviado {$got}";
			}
		}
	}
	check('columna 1.12.2: mismos bloques en las mismas coordenadas', $wrong === 0, $firstWrong);

	// 2. Ningún bloque inventado.
	$javaNonAir = 0;
	foreach($java as $entries){
		foreach($entries as $state){
			if($state !== ChunkBuilder::JAVA_AIR){
				$javaNonAir++;
			}
		}
	}
	$expected = 0;
	foreach($blocks as [$x, $y, $z, $id]){
		if(ChunkBuilder::javaState($id) !== ChunkBuilder::JAVA_AIR){
			$expected++;
		}
	}
	check('columna 1.12.2: sin bloques inventados', $javaNonAir === $expected, "java={$javaNonAir} esperado={$expected}");

	// 3. Todos los ids del mundo real tienen traducción.
	$list = [];
	foreach($unmapped as $id => $count){
		$name = ModernChunk::blockName($id) ?? 'desconocido';
		$list[] = "{$name} ({$id}) x{$count}";
	}
	check(
		'columna 1.12.2: todos los bloques del mundo tienen traducción',
		$unmappedBlocks === 0,
		implode(', ', array_slice($list, 0, 12))
	);

	// 4. Mapa de alturas lado a lado (Bedrock / 1.12.2 sin el desplazamiento).
	$bedrockSurface = [];
	$legacySurface = [];
	for($z = 0; $z < 16; $z++){
		for($x = 0; $x < 16; $x++){
			$bedrockSurface[$z][$x] = $chunk->getHighestBlockAt($x, $z) ?? -64;
			$height = -64;
			for($section = 15; $section >= 0; $section--){
				$found = false;
				for($y = 15; $y >= 0; $y--){
					$index = ($y << 8) | ($z << 4) | $x;
					if(($java[$section][$index] ?? 0) !== ChunkBuilder::JAVA_AIR){
						$height = ($section << 4) + $y - 64;
						$found = true;
						break;
					}
				}
				if($found){
					break;
				}
			}
			$legacySurface[$z][$x] = $height;
		}
	}
	$surfaceMismatch = 0;
	$firstSurface = '';
	for($z = 0; $z < 16; $z++){
		for($x = 0; $x < 16; $x++){
			if($bedrockSurface[$z][$x] !== $legacySurface[$z][$x]){
				$surfaceMismatch++;
				if($firstSurface === ''){
					$firstSurface = "({$x},{$z}) bedrock {$bedrockSurface[$z][$x]} java {$legacySurface[$z][$x]}";
				}
			}
		}
	}
	check('superficie idéntica columna a columna (1.12.2)', $surfaceMismatch === 0, $firstSurface);

	fwrite(STDOUT, "\nSuperficie Bedrock / 1.12.2:\n");
	for($z = 0; $z < 16; $z++){
		$row = '';
		for($x = 0; $x < 16; $x++){
			$row .= sprintf('%4d/%4d', $bedrockSurface[$z][$x], $legacySurface[$z][$x]);
		}
		fwrite(STDOUT, $row . "\n");
	}

	if($failures > 0){
		fwrite(STDERR, "\n{$failures} comprobación(es) fallida(s)\n");
		exit(1);
	}
	fwrite(STDOUT, "\nALL OK\n");
}
