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

// Shared with `thread.php`, which has to re-declare the runtime inside child
// threads (see `CommonThreadPartsTrait::loadRuntimeForThread()`).
$runtimeFiles = require $runtimeDirectory . DIRECTORY_SEPARATOR . 'runtime_files.php';

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
