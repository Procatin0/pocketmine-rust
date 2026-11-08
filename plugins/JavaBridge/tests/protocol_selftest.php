<?php

declare(strict_types=1);

// Prueba aislada de Protocol + ChunkBuilder (no dependen del runtime de PM).
require __DIR__ . '/../src/javabridge/Protocol.php';
require __DIR__ . '/../src/javabridge/ChunkBuilder.php';

use javabridge\ChunkBuilder;
use javabridge\Protocol;

$failures = 0;
function check(string $name, bool $ok) : void{
	global $failures;
	if(!$ok){
		$failures++;
		fwrite(STDERR, "FAIL: $name\n");
	}else{
		fwrite(STDOUT, "ok: $name\n");
	}
}

// VarInt roundtrip
foreach([0, 1, 127, 128, 300, 25565, -1, 2147483647] as $value){
	$encoded = Protocol::varInt($value);
	$offset = 0;
	$decoded = Protocol::readVarInt($encoded, $offset);
	check("varint $value", $decoded === ($value & 0xFFFFFFFF) || $decoded === $value);
}

// Incomplete VarInt leaves the offset untouched
$buffer = "\x80";
$offset = 0;
check('varint incompleto', Protocol::readVarInt($buffer, $offset) === null && $offset === 0);

// Strings
$encoded = Protocol::string('Steve');
$offset = 0;
check('string', Protocol::readString($encoded, $offset) === 'Steve');

// Identifier is a length-prefixed string, same wire format as string()
$encoded = Protocol::identifier('minecraft:water');
$offset = 0;
check('identifier', Protocol::readString($encoded, $offset) === 'minecraft:water' && $offset === strlen($encoded));

// VarInt arrays (tag entries): roundtrip, empty and incomplete
foreach([[], [1], [1, 2], [0, 40, 482, 1167]] as $values){
	$encoded = Protocol::varIntArray($values);
	$offset = 0;
	check('varint array ' . json_encode($values), Protocol::readVarIntArray($encoded, $offset) === $values && $offset === strlen($encoded));
}
$buffer = "\x02\x01";
$offset = 0;
check('varint array incompleto', Protocol::readVarIntArray($buffer, $offset) === null && $offset === 0);

// Frame carries length + id + payload
$frame = Protocol::frame(0x00, Protocol::string('hello'));
$offset = 0;
$length = Protocol::readVarInt($frame, $offset);
check('frame length', $length === strlen($frame) - $offset);
$body = substr($frame, $offset, $length);
$bodyOffset = 0;
check('frame id', Protocol::readVarInt($body, $bodyOffset) === 0x00);
check('frame payload', substr($body, $bodyOffset) === Protocol::string('hello'));

// Position pack/unpack
foreach([[0, 4, 0], [-1, 64, 200], [1000000, 255, -1000000]] as $position){
	$encoded = Protocol::position($position[0], $position[1], $position[2]);
	$offset = 0;
	check('position ' . implode(',', $position), Protocol::readPosition($encoded, $offset) === $position);
}

// Offline UUID
$uuid = Protocol::offlineUuid('Steve');
check('uuid bytes', strlen($uuid) === 16);
check('uuid determinista', $uuid === Protocol::offlineUuid('Steve'));
check('uuid distinto', $uuid !== Protocol::offlineUuid('Alex'));
check('uuid formato', strlen(Protocol::uuidString($uuid)) === 36);
check('uuid version 3', (ord($uuid[6]) & 0xF0) === 0x30);

// Chat JSON
check('chat json', Protocol::chatJson('a"b') === '{"text":"a\\"b"}');

// Chunk: columna de Bedrock (subsecciones paletizadas) a 1.12.2
$column = static function(array $sections) : object{
	return new class($sections){
		public function __construct(private array $sections){}
		public function getSectionPayload(int $baseY) : ?array{
			return $this->sections[intdiv($baseY + 64, 16)] ?? null;
		}
	};
};

// Una subsección de bedrock (Y=-64) cae en la sección 0 de Java (Y=0)
[$bitMap, $data] = ChunkBuilder::fromChunk($column([
	0 => ['bits' => 0, 'words' => [], 'palette' => [4121722107]],
]));
check('chunk bitmask', $bitMap === 1);
check('chunk biomas', substr($data, -256) === str_repeat("\x01", 256));
check('chunk sky light', substr($data, -256 - 2048, 2048) === str_repeat("\xFF", 2048));
check('chunk bits per block', ord($data[0]) === 4);
check('chunk paleta bedrock', str_contains($data, Protocol::varInt(7 << 4)));

// Columna vacía: ni una sección
[$bitMap, $data] = ChunkBuilder::fromChunk($column([]));
check('chunk vacío', $bitMap === 0 && strlen($data) === 256);

// Tres subsecciones de Bedrock -> tres secciones de Java
[$bitMap, $data] = ChunkBuilder::fromChunk($column([
	0 => ['bits' => 0, 'words' => [], 'palette' => [4121722107]],
	1 => ['bits' => 0, 'words' => [], 'palette' => [2150698529]],
	2 => ['bits' => 0, 'words' => [], 'palette' => [2150698529]],
]));
check('chunk alto', $bitMap === 0b111 && strlen($data) > 8000);

fwrite(STDOUT, $failures === 0 ? "ALL OK\n" : "$failures FAILURE(S)\n");
exit($failures === 0 ? 0 : 1);
