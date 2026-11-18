<?php

declare(strict_types=1);

/**
 * Regression: commands registered from `onEnable()` were invisible to the
 * client's AvailableCommandsPacket, because only `plugin.yml`-declared
 * commands were tracked. EasyEdit registers `/set`, `/wand`, ... through
 * `CommandMap::register("easyedit", $command)` (players type `//set`), so
 * `Runtime::listCommands()` must report them.
 *
 * Uso interno del test de integración `tests/plugin_commands.rs`:
 * recibe como argv[1] el directorio donde se escribió el runtime PHP.
 */

$dir = $argv[1] ?? '';
if ($dir === '' || !is_file($dir . '/runtime_files.php')) {
    fwrite(STDERR, "missing PHP runtime directory argument\n");
    exit(2);
}
foreach (require $dir . '/runtime_files.php' as $file) {
    require $dir . '/' . $file;
}

$failures = 0;
$check = function (string $what, $expected, $actual) use (&$failures): void {
    if ($expected !== $actual) {
        $failures++;
        fwrite(STDERR, 'FAIL ' . $what . ': expected ' . var_export($expected, true) . ', got ' . var_export($actual, true) . "\n");
    }
};

$server = \pocketmine\Server::getInstance();
$commandMap = $server->getCommandMap();

// Same shape as EasyEdit's CommandManager: prefixed registration, label and
// aliases keep the leading slash.
$commandMap->register('easyedit', new \pocketmine\command\SimpleCommand(
    '/set',
    'Set blocks',
    '/set <block>',
    ['/s']
));
// Command without aliases and without the prefix colliding.
$commandMap->register('easyedit', new \pocketmine\command\SimpleCommand(
    '/wand',
    'Toggle the selection wand',
    '/wand'
));
// Bare label taken by a plugin command, like a vanilla command would.
$commandMap->register('server', new \pocketmine\command\SimpleCommand(
    'status',
    'Server status',
    '/status'
));

$listed = [];
$order = [];
foreach (\pocketmine\runtime\Runtime::listCommands() as $command) {
    $order[] = $command['name'];
    $listed[$command['name']] = $command;
}

$check('plugin command listed', true, isset($listed['set']));
$check('prefixed alias', true, isset($listed['wand']));
$check('plain command listed', true, isset($listed['status']));
$check('no prefixed duplicates', false, isset($listed['easyedit:set']) || isset($listed['easyedit:/set']));
$check('set description', 'Set blocks', $listed['set']['description'] ?? null);
$check('set usage', '/set <block>', $listed['set']['usage'] ?? null);
$check('set aliases', ['s'], $listed['set']['aliases'] ?? null);
$check('wand has no aliases', [], $listed['wand']['aliases'] ?? null);
$check('one entry per command', count($order), count(array_unique($order)));
$check('all commands reported', true, count($order) >= 3);

// Registration order is preserved (prefixed keys are skipped, not duplicated).
$check('registration order', ['set', 'wand', 'status'], $order);

if ($failures === 0) {
    echo "ALL OK\n";
}
exit($failures === 0 ? 0 : 1);
