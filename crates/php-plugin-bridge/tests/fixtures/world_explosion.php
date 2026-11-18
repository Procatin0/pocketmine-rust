<?php

declare(strict_types=1);

/**
 * `pocketmine\world\Explosion`: validación, `explodeA()` (preview de bloques),
 * `explodeB()` (encolado) y edición de `$affectedBlocks` entre ambos.
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
use pocketmine\world\Explosion;
use pocketmine\world\Position;
use pocketmine\world\World;

/** `Position` con mundo directo (sin `Server`). */
final class ExplosionProbePosition extends Position
{
    public function __construct(private World $level, float $x, float $y, float $z)
    {
        parent::__construct($x, $y, $z, $level->getName());
    }

    public function getWorld(): ?World
    {
        return $this->level;
    }
}

/** Inyecta las respuestas del host y registra las llamadas. */
final class ExplosionProbe extends Explosion
{
    /** @var array<int, array{0:string, 1:array}> */
    public array $calls = [];

    /** @var array<int, array{x:int, y:int, z:int, state_id:int}> */
    public array $blocksResponse = [];

    protected function hostCall(string $method, array $params): mixed
    {
        $this->calls[] = [$method, $params];
        return match ($method) {
            'world.explosion_blocks' => ['blocks' => $this->blocksResponse],
            'world.explode' => ['ok' => true, 'count' => count($params['blocks'] ?? [])],
            default => null,
        };
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
$throws = function (string $what, callable $fn) use (&$failures): void {
    try {
        $fn();
    } catch (\InvalidArgumentException) {
        return;
    }
    $failures++;
    fwrite(STDERR, 'FAIL ' . $what . ": no InvalidArgumentException\n");
};

$world = new World('world');
$source = new ExplosionProbePosition($world, 1.5, 64.0, 2.5);

$throws('radius zero', fn() => new Explosion($source, 0.0));
$throws('radius negative', fn() => new Explosion($source, -1.0));
$throws('fire chance high', fn() => new Explosion($source, 4.0, null, 1.5));
$throws('fire chance negative', fn() => new Explosion($source, 4.0, null, -0.1));

$explosion = new ExplosionProbe($source, 4.0);
$check('source', $source, $explosion->getSource());
$check('radius', 4.0, $explosion->getRadius());
$check('what', null, $explosion->getWhat());
$check('fire chance default', 0.0, $explosion->getFireChance());
$check('default rays', 16, $explosion->rays);
$check('default step', 0.3, $explosion->stepLen);

$explosion->setFireChance(0.5);
$check('fire chance set', 0.5, $explosion->getFireChance());
$throws('set fire chance range', fn() => $explosion->setFireChance(2.0));

// Radio < 0.1: PM no calcula ni destruye nada.
$tiny = new ExplosionProbe($source, 0.05);
$check('tiny explodeA', false, $tiny->explodeA());
$check('tiny no calls', 0, count($tiny->calls));
$tiny->explode();
$check('tiny explode no calls', 0, count($tiny->calls));

$stone = Block::fromName('stone', 'world', 0, 0, 0)->getStateId();
$dirt = Block::fromName('dirt', 'world', 0, 0, 0)->getStateId();
$explosion->blocksResponse = [
    ['x' => 1, 'y' => 64, 'z' => 2, 'state_id' => $stone],
    ['x' => 2, 'y' => 64, 'z' => 2, 'state_id' => $dirt],
];
$check('explodeA', true, $explosion->explodeA());
$check('preview calls', 1, count($explosion->calls));
$check('preview method', 'world.explosion_blocks', $explosion->calls[0][0]);
$check('preview radius', 4.0, $explosion->calls[0][1]['radius']);
$check('preview position', [1.5, 64.0, 2.5], [
    $explosion->calls[0][1]['x'],
    $explosion->calls[0][1]['y'],
    $explosion->calls[0][1]['z'],
]);
$check('affected count', 2, count($explosion->getAffectedBlocks()));
$check('affected stone', $stone, $explosion->affectedBlocks['1:64:2']->getStateId());
$check('affected dirt pos', 2, $explosion->affectedBlocks['2:64:2']->getX());

// El plugin puede editar la lista antes de explodeB().
unset($explosion->affectedBlocks['2:64:2']);
$explosion->explodeB();
$check('explodeB calls', 2, count($explosion->calls));
$check('explodeB method', 'world.explode', $explosion->calls[1][0]);
$check('explodeB blocks', 1, count($explosion->calls[1][1]['blocks']));
$check('explodeB block x', 1, $explosion->calls[1][1]['blocks'][0]['x']);
$check('explodeB block id', $stone, $explosion->calls[1][1]['blocks'][0]['state_id']);
$check('explodeB world', 'world', $explosion->calls[1][1]['world']);
$check('explodeB no entity', false, isset($explosion->calls[1][1]['entity_id']));

// explodeB() sin explodeA(): no destruye bloques (PM), pero sí encola daño.
$direct = new ExplosionProbe($source, 3.0);
$check('direct explodeB', true, $direct->explodeB());
$check('direct blocks empty', [], $direct->calls[0][1]['blocks']);

$withBlock = new ExplosionProbe($source, 3.0, Block::fromName('tnt', 'world', 0, 0, 0));
$check('block source', true, $withBlock->getWhat() instanceof Block);
$withBlock->explodeB();
$check('block source no entity', false, isset($withBlock->calls[0][1]['entity_id']));

// `explode()` encadena preview + encolado.
$full = new ExplosionProbe($source, 3.0);
$full->blocksResponse = [['x' => 0, 'y' => 64, 'z' => 0, 'state_id' => $stone]];
$full->explode();
$check('full calls', 2, count($full->calls));
$check('full first', 'world.explosion_blocks', $full->calls[0][0]);
$check('full second', 'world.explode', $full->calls[1][0]);

if ($failures > 0) {
    fwrite(STDERR, "FAILURES: $failures\n");
    exit(1);
}
echo "ALL OK\n";
