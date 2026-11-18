<?php

declare(strict_types=1);

/**
 * `World`: colisiones (`getBlockCollisionBoxes`, `getCollisionBlocks`),
 * `scheduleDelayedBlockUpdate`, `notifyNeighbourBlockUpdate`, `isSpawnChunk`,
 * `getAdjacentChunks` y `chunkHash`.
 *
 * `ipc.php` no se carga: el fixture declara un `Bridge` falso para registrar
 * las host calls.
 */

namespace pocketmine\runtime {
    class Bridge
    {
        /** @var array<int, array{0:string, 1:array}> */
        public static array $calls = [];

        public static int $stoneState = 0;

        public static function isActive(): bool
        {
            return true;
        }

        public static function hostCall(string $method, array $params = []): mixed
        {
            self::$calls[] = [$method, $params];
            return match ($method) {
                'world.get_chunk' => null,
                'world.collision_boxes' => ['boxes' => [[
                    'min_x' => 2.0,
                    'min_y' => 5.0,
                    'min_z' => 2.0,
                    'max_x' => 3.0,
                    'max_y' => 6.0,
                    'max_z' => 3.0,
                ]]],
                'world.collision_blocks' => ['blocks' => [[
                    'x' => 2,
                    'y' => 5,
                    'z' => 2,
                    'state_id' => self::$stoneState,
                ]]],
                default => ['ok' => true],
            };
        }
    }
}

namespace {
    $dir = $argv[1] ?? '';
    if ($dir === '' || !is_file($dir . '/runtime_files.php')) {
        fwrite(STDERR, "missing PHP runtime directory argument\n");
        exit(2);
    }
    foreach (require $dir . '/runtime_files.php' as $file) {
        if ($file === 'ipc.php') {
            continue;
        }
        require $dir . '/' . $file;
    }

    use pocketmine\block\Block;
    use pocketmine\math\AxisAlignedBB;
    use pocketmine\math\Vector3;
    use pocketmine\runtime\Bridge;
    use pocketmine\world\World;

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

    // Resolver el id antes de medir las llamadas: `fromName()` usa el host.
    $stoneState = Block::fromName('stone', 'world')->getStateId();
    Bridge::$stoneState = $stoneState;
    Bridge::$calls = [];

    $world = new World('world');
    $bb = new AxisAlignedBB(1.5, 4.5, 1.5, 2.5, 5.5, 2.5);

    $boxes = $world->getBlockCollisionBoxes($bb);
    $check('boxes count', 1, count($boxes));
    $check('box class', true, $boxes[0] instanceof AxisAlignedBB);
    $check('box min', [2.0, 5.0, 2.0], [$boxes[0]->minX, $boxes[0]->minY, $boxes[0]->minZ]);
    $check('box max', [3.0, 6.0, 3.0], [$boxes[0]->maxX, $boxes[0]->maxY, $boxes[0]->maxZ]);
    $check(
        'box payload',
        ['min_x' => 1.5, 'min_y' => 4.5, 'min_z' => 1.5, 'max_x' => 2.5, 'max_y' => 5.5, 'max_z' => 2.5],
        Bridge::$calls[0][1]
    );

    $blocks = $world->getCollisionBlocks($bb);
    $check('blocks count', 1, count($blocks));
    $check('block position', [2, 5, 2], [$blocks[0]->getX(), $blocks[0]->getY(), $blocks[0]->getZ()]);
    $check('block state', $stoneState, $blocks[0]->getStateId());
    $check('blocks method', 'world.collision_blocks', Bridge::$calls[1][0]);

    $world->scheduleDelayedBlockUpdate(new Vector3(1.9, 64.2, -2.7), 5);
    $check('schedule method', 'world.schedule_block_update', Bridge::$calls[2][0]);
    $check('schedule pos', [1, 64, -3, 5], [
        Bridge::$calls[2][1]['x'],
        Bridge::$calls[2][1]['y'],
        Bridge::$calls[2][1]['z'],
        Bridge::$calls[2][1]['delay'],
    ]);

    $world->notifyNeighbourBlockUpdate(new Vector3(1.2, 64.8, 2.0));
    $check('notify method', 'world.notify_neighbour', Bridge::$calls[3][0]);
    $check('notify pos', [1, 64, 2], [
        Bridge::$calls[3][1]['x'],
        Bridge::$calls[3][1]['y'],
        Bridge::$calls[3][1]['z'],
    ]);

    $check('chunk hash', (1 << 32) | 0xFFFFFFFF, World::chunkHash(1, -1));
    $adjacent = $world->getAdjacentChunks(3, -2);
    $check('adjacent count', 8, count($adjacent));
    $check('adjacent center key absent', false, array_key_exists(World::chunkHash(0, 0), $adjacent));
    $check('adjacent null', true, $adjacent[World::chunkHash(-1, -1)] === null);

    $check('spawn chunk', true, $world->isSpawnChunk(0, 0));
    $check('spawn chunk edge', true, $world->isSpawnChunk(1, -1));
    $check('not spawn chunk', false, $world->isSpawnChunk(2, 0));

    $expanded = $bb->expandedCopy(0.25, 0.5, 1.0);
    $check('expanded', [1.25, 4.0, 0.5, 2.75, 6.0, 3.5], [
        $expanded->minX, $expanded->minY, $expanded->minZ,
        $expanded->maxX, $expanded->maxY, $expanded->maxZ,
    ]);

    if ($failures > 0) {
        fwrite(STDERR, "FAILURES: $failures\n");
        exit(1);
    }
    echo "ALL OK\n";
}
