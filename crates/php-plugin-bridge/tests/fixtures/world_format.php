<?php

declare(strict_types=1);

/**
 * `pocketmine\world\format`: `Chunk` (con coordenadas y capa local),
 * `SubChunk`, `HeightArray` y `ChunkException`.
 *
 * argv[1] = directorio del runtime PHP escrito por el test Rust.
 */

$dir = $argv[1] ?? '';
if ($dir === '' || !is_file($dir . '/runtime_files.php')) {
    fwrite(STDERR, "missing PHP runtime directory argument\n");
    exit(2);
}
foreach (require $dir . '/runtime_files.php' as $file) {
    require $dir . '/' . $file;
}

use pocketmine\world\format\Chunk;
use pocketmine\world\format\ChunkException;
use pocketmine\world\format\HeightArray;
use pocketmine\world\format\SubChunk;

$failures = 0;
$check = function (string $what, mixed $expected, mixed $actual) use (&$failures): void {
    if ($expected !== $actual) {
        $failures++;
        fwrite(
            STDERR,
            'FAIL ' . $what . ': expected ' . var_export($expected, true)
            . ', got ' . var_export($actual, true) . "\n"
        );
    }
};
$throws = function (string $what, callable $fn) use (&$failures): void {
    try {
        $fn();
    } catch (\InvalidArgumentException) {
        return;
    }
    $failures++;
    fwrite(STDERR, 'FAIL ' . $what . ": no InvalidArgumentException\n");
};

$heights = HeightArray::fill(70);
$check('height fill', 70, $heights->get(0, 0));
$check('height zzxxxx', 70, $heights->get(15, 15));
$heights->set(0, 0, -64);
$check('height set', -64, $heights->get(0, 0));
$check('height values', 256, count($heights->getValues()));
$throws('height x range', fn() => $heights->get(-1, 0));
$throws('height z range', fn() => $heights->set(0, 16, 1));
$throws('height constructor size', fn() => new HeightArray([1, 2, 3]));

$chunk = new Chunk('', 24, 3, -2);
$check('chunk x', 3, $chunk->getX());
$check('chunk z', -2, $chunk->getZ());
$check('chunk height', 384, $chunk->getHeight());
$check('chunk empty block', Chunk::AIR, $chunk->getBlockStateId(0, 0, 0));
$check('chunk empty biome', 0, $chunk->getBiomeId(0, 0, 0));
$check('chunk populated', true, $chunk->isPopulated());
$chunk->setPopulated(false);
$check('chunk unpopulated', false, $chunk->isPopulated());

// La escritura local se ve en getters/altura pero no toca el mundo nativo.
$chunk->setBlockStateId(1, 64, 2, 42);
$check('chunk override', 42, $chunk->getBlockStateId(1, 64, 2));
$check('chunk height map', 64, $chunk->getHeightMap(1, 2));
$check('chunk other column', Chunk::MIN_Y, $chunk->getHeightMap(2, 2));

$check('subchunk count', 24, count($chunk->getSubChunks()));
$empty = $chunk->getSubChunk(5);
$check('subchunk class', true, $empty instanceof SubChunk);
$check('subchunk index', 5, $empty->getSubChunkIndex());
$check('subchunk empty', true, $empty->isEmptyFast());
$check('subchunk empty authoritative', true, $empty->isEmptyAuthoritative());
$check('subchunk empty id', Chunk::AIR, $empty->getEmptyBlockId());
$check('subchunk layers', [], $empty->getBlockLayers());

$section = $chunk->getSubChunk(4); // baseY 64
$check('subchunk sees override', 42, $section->getBlockStateId(1, 0, 2));
$check('subchunk highest', 64, $section->getHighestBlockAt(1, 2));
$check('subchunk empty column', null, $section->getHighestBlockAt(0, 0));
$check('subchunk not empty', false, $section->isEmptyFast());
$section->setBlockStateId(2, 1, 3, 7);
$check('subchunk write', 7, $section->getBlockStateId(2, 1, 3));
$check('subchunk write absolute', 7, $chunk->getBlockStateId(2, 65, 3));
$throws('subchunk local y', fn() => $section->getBlockStateId(0, 16, 0));
$throws('subchunk range', fn() => $chunk->getSubChunk(20));

$check('chunk exception', true, new ChunkException('boom') instanceof \RuntimeException);
$check('chunk is format', true, $chunk instanceof \pocketmine\world\Chunk);
$check('chunk hash', (3 << 32) | 0xFFFFFFFE, $chunk->getChunkCoordHash());

if ($failures > 0) {
    fwrite(STDERR, "FAILURES: $failures\n");
    exit(1);
}
echo "ALL OK\n";
