<?php

declare(strict_types=1);

/*
 * Prueba el `Chunk` del runtime PHP del puente (`world.get_chunk`).
 *
 * Se carga el `world.php` real de `crates/php-plugin-bridge/php` con un
 * `Bridge` de mentira que devuelve una columna Bedrock sintética, así que se
 * comprueba el decodificador de subsecciones (versión 8, paletas en zigzag,
 * varints) sin arrancar el servidor Rust.
 */

namespace pocketmine\runtime {

	final class Bridge{
		public static string $payload = '';

		public static function isActive() : bool{ return true; }

		public static function hostCall(string $method, array $params = []) : mixed{
			if($method === 'world.get_chunk'){
				return ['subchunks' => 2, 'data' => bin2hex(self::$payload)];
			}
			if($method === 'world.get_spawn'){
				return ['x' => 26, 'y' => 64, 'z' => -16, 'player' => [26.5, 65.62, -15.5], 'world' => 'world'];
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

	use pocketmine\runtime\Bridge;
	use pocketmine\world\Chunk;
	use pocketmine\world\World;

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

	/** VarInt en zigzag, como `Writer::write_var_i32` de Rust. */
	function varintZigzag(int $value) : string{
		// Zigzag sobre enteros PHP de 64 bits: los ids de red caben de sobra.
		$zigzag = ($value << 1) ^ ($value >> 63);
		$out = '';
		while(true){
			$byte = $zigzag & 0x7F;
			$zigzag >>= 7;
			if($zigzag === 0){
				return $out . chr($byte);
			}
			$out .= chr($byte | 0x80);
		}
	}

	function signedId(int $unsignedId) : int{
		return $unsignedId >= 0x80000000 ? $unsignedId - 0x100000000 : $unsignedId;
	}

	// --- Columna sintética: bedrock en Y=-64 y piedra/tierra/hierba/aire en Y=-48
	$payload = chr(8) . chr(1) . chr((0 << 1) | 1) . varintZigzag(signedId(4121722107));

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

	$payload .= chr(8) . chr(1) . chr(($bits << 1) | 1);
	foreach($words as $word){
		$payload .= pack('V', $word);
	}
	$payload .= varintZigzag(4);
	foreach([2150698529, 2186211206, 3727763636, 3690217760] as $id){
		$payload .= varintZigzag(signedId($id));
	}
	for($section = 0; $section < 24; $section++){
		$payload .= chr(1) . varintZigzag(1); // plains
	}

	Bridge::$payload = $payload;

	$world = new World('world');
	$chunk = $world->getChunk(0, 0);
	check('world.get_chunk devuelve un Chunk', $chunk instanceof Chunk);

	$spawn = $world->getPlayerSpawnLocation();
	check(
		'world.get_spawn: punto de aparición del jugador (centro + 1,62)',
		$spawn === [26.5, 65.62, -15.5],
		var_export($spawn, true)
	);

	$chunk = new Chunk($payload, 2);
	check('bedrock en Y=-64', $chunk->getBlockStateId(0, -64, 0) === 4121722107);
	check('piedra en Y=-48', $chunk->getBlockStateId(3, -48, 5) === 2150698529);
	check('tierra en Y=-38', $chunk->getBlockStateId(3, -38, 5) === 2186211206, (string)$chunk->getBlockStateId(3, -38, 5));
	check('hierba en Y=-34', $chunk->getBlockStateId(3, -34, 5) === 3727763636, (string)$chunk->getBlockStateId(3, -34, 5));
	check('aire en Y=-33', $chunk->getBlockStateId(3, -33, 5) === 3690217760);
	check('aire sobre la columna', $chunk->getBlockStateId(3, 100, 5) === 3690217760);
	check('altura de la columna', $chunk->getHighestBlockAt(0, 0) === -34, (string)$chunk->getHighestBlockAt(0, 0));
	check('bioma de la subsección', $chunk->getBiomeId(0, 0, 0) === 1);

	$section = $chunk->getSectionPayload(-48);
	check('getSectionPayload trae la paleta', $section !== null && count($section['palette']) === 4);
	check('getSectionPayload ignora el aire', $chunk->getSectionPayload(-32) === null);

	fwrite(STDOUT, $failures === 0 ? "ALL OK\n" : "$failures FAILURE(S)\n");
	exit($failures === 0 ? 0 : 1);
}
