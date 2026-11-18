<?php

declare(strict_types=1);

/**
 * `BlockTransaction`: lote con `fetchBlock`/`getBlocks`, validadores y
 * `apply()` sobre `World::getBlockAt()`/`setBlockAt()`.
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

use pocketmine\block\Block;
use pocketmine\math\Vector3;
use pocketmine\world\BlockTransaction;
use pocketmine\world\World;

/** Mundo en memoria: registra escrituras sin host calls. */
final class TransactionProbeWorld extends World
{
    /** @var array<string, Block> */
    public array $virtual = [];

    /** @var array<int, array{int, int, int, int}> */
    public array $writes = [];

    public function getBlockAt(int $x, int $y, int $z, bool $cached = true, bool $addToCache = true): Block
    {
        return $this->virtual[$x . ':' . $y . ':' . $z]
            ?? Block::fromName('air', $this->getName(), $x, $y, $z);
    }

    public function setBlockAt(int $x, int $y, int $z, Block $block, bool $direct = false): bool
    {
        $this->virtual[$x . ':' . $y . ':' . $z] = $block;
        $this->writes[] = [$x, $y, $z, $block->getStateId()];
        return true;
    }
}

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

$world = new TransactionProbeWorld('world');
$stone = Block::fromName('stone', 'world', 0, 0, 0);
$dirt = Block::fromName('dirt', 'world', 0, 0, 0);
$check('fixture blocks differ', false, $stone->getStateId() === $dirt->getStateId());

$tx = new BlockTransaction($world);
$check('empty fetch', $world->getBlockAt(0, 0, 0)->getStateId(), $tx->fetchBlockAt(0, 0, 0)->getStateId());

$tx->addBlockAt(1, 2, 3, $stone);
$check('fetch pending', $stone->getStateId(), $tx->fetchBlockAt(1, 2, 3)->getStateId());
$check('fetch parent', $world->getBlockAt(1, 2, 3)->getStateId(), $tx->fetchBlockAt(9, 9, 9)->getStateId());

$blocks = iterator_to_array($tx->getBlocks(), false);
$check('getBlocks count', 1, count($blocks));
$check('getBlocks coords', [1, 2, 3], [$blocks[0][0], $blocks[0][1], $blocks[0][2]]);
$check('getBlocks state', $stone->getStateId(), $blocks[0][3]->getStateId());

$check('apply', true, $tx->apply());
$check('apply writes', 1, count($world->writes));
$check('apply state', $stone->getStateId(), $world->getBlockAt(1, 2, 3)->getStateId());
$check('reapply no-op', false, $tx->apply());
$check('reapply writes', 1, count($world->writes));

$tx->addBlock(new Vector3(4.7, 5.2, -1.9), $dirt);
$blocks = iterator_to_array($tx->getBlocks(), false);
$check('addBlock floors', [4, 5, -2], [$blocks[1][0], $blocks[1][1], $blocks[1][2]]);

$tx->addValidator(fn(World $w, int $x, int $y, int $z): bool => $z >= 0);
$check('validator rejects', false, $tx->apply());
$check('validator atomic', 1, count($world->writes));

// Los validadores son acumulativos: para aceptar hace falta otra transacción.
$tx2 = new BlockTransaction($world);
$tx2->addBlockAt(-2, 5, 3, $dirt);
$tx2->addValidator(fn(World $w, int $x, int $y, int $z): bool => $z >= -10);
$check('validators accept', true, $tx2->apply());
$check('writes after validators', 2, count($world->writes));

$check('dummy min', true, $tx->dummyValidator($world, 0, World::Y_MIN, 0));
$check('dummy max', false, $tx->dummyValidator($world, 0, World::Y_MAX, 0));

if ($failures > 0) {
    fwrite(STDERR, "FAILURES: $failures\n");
    exit(1);
}
echo "ALL OK\n";
