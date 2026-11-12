<?php

declare(strict_types=1);

/**
 * PocketMine-Rust PHP plugin runtime bootstrap.
 *
 * The Rust core writes these files to a temporary directory and starts
 * `php bootstrap.php` as a long-lived child process. Communication happens
 * over newline-delimited JSON on stdin/stdout; logs go to stderr.
 */

$runtimeDirectory = __DIR__;

$runtimeFiles = [
    'math.php',
    'text.php',
    'permission.php',
    'config.php',
    'scheduler.php',
    'command.php',
    'item.php',
    'world.php',
    'player.php',
    'scoreboard.php',
    'bossbar.php',
    'chat.php',
    'core.php',
    'event.php',
    'PlayerDataSaveEvent.php',
    'plugin.php',
    'autoload.php',
    'ipc.php',
];

foreach ($runtimeFiles as $runtimeFile) {
    $path = $runtimeDirectory . DIRECTORY_SEPARATOR . $runtimeFile;
    if (!is_file($path)) {
        fwrite(STDERR, '[PHP/BOOTSTRAP] Missing runtime file: ' . $path . PHP_EOL);
        exit(1);
    }
    require_once $path;
}

if (!class_exists(\pocketmine\runtime\Runtime::class)) {
    fwrite(STDERR, '[PHP/BOOTSTRAP] Runtime class was not loaded' . PHP_EOL);
    exit(1);
}

\pocketmine\runtime\Runtime::main();
