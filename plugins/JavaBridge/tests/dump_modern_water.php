<?php
// Genera la sección de agua de prueba (la misma que modern_test) y la vuelca a un fichero
// para parsearla con prismarine-chunk (Node) en vez de con nuestro parser PHP.
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

namespace {
	require dirname(__DIR__, 3) . '/crates/php-plugin-bridge/php/world.php';
	require dirname(__DIR__) . '/src/javabridge/Protocol.php';
	require dirname(__DIR__) . '/src/javabridge/ModernProfile.php';
	require dirname(__DIR__) . '/src/javabridge/ModernChunk.php';

	$profile = new \javabridge\ModernProfile(775);

	// Columna completa real (chunk 0,0 oceano) para probar hasFluid por sección.
	if($argv[1] === 'real'){
		$payload = file_get_contents('target/column_0_0.bin');
		\pocketmine\runtime\Bridge::$payload = $payload;
		\pocketmine\runtime\Bridge::$subchunks = 24;
		$chunk = new \pocketmine\world\Chunk($payload, 24);
		$data = \javabridge\ModernChunk::fromChunk($chunk, $profile);
		file_put_contents('target/modern_0_0_775.bin', $data);
		echo "wrote target/modern_0_0_775.bin (".strlen($data)." bytes)\n";
		return;
	}

	// Sección sintética mitad agua (como modern_test): sección 7, mitad inferior agua.
	$waterChunk = new class{
		public function getSectionPayload(int $baseY) : ?array{
			if($baseY !== 48){
				return null;
			}
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
	$data = \javabridge\ModernChunk::fromChunk($waterChunk, $profile);
	// Extraer solo la sección 7 (las 7 primeras son aire: 4+2 bytes cada una en 775? no, calcular parseando)
	// Simplificación: volcar la columna entera; el parser Node leerá las 24 secciones.
	file_put_contents('target/modern_water_775.bin', $data);
	echo "wrote target/modern_water_775.bin (".strlen($data)." bytes)\n";
}
