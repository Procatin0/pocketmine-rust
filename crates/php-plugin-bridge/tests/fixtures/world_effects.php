<?php

declare(strict_types=1);

/**
 * `WorldSoundEvent` / `WorldParticleEvent`: el PHP los reconstruye desde el
 * payload que envía `SessionManager` (sonido/partícula "de cable"), exponen los
 * getters de PM y devuelven los cambios en `toArray()`.
 *
 * Uso interno del test de integración `tests/world_effects.rs`: recibe como
 * argv[1] el directorio donde se escribió el runtime PHP.
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
use pocketmine\event\EventFactory;
use pocketmine\event\world\WorldParticleEvent;
use pocketmine\event\world\WorldSoundEvent;
use pocketmine\world\particle\BlockBreakParticle;
use pocketmine\world\particle\WireParticle;
use pocketmine\world\sound\ClickSound;
use pocketmine\world\sound\WireSound;

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

$sound = EventFactory::create('WorldSoundEvent', [
    'world' => 'world',
    'x' => 1.5,
    'y' => 64.25,
    'z' => -2.5,
    'sound' => 'place',
    'event_id' => null,
    'extra_data' => 42,
    'recipients' => ['steve'],
]);
$check('sound event class', true, $sound instanceof WorldSoundEvent);
$check('sound world', 'world', $sound->getWorld()->getName());
$check('sound cancellable', true, $sound->isCancellable());
$check('sound wire class', true, $sound->getSound() instanceof WireSound);
$check('sound name', 'place', $sound->getSound()->getSoundName());
$check('sound extra data', 42, $sound->getSound()->getExtraData());
$check('sound position', [1.5, 64.25, -2.5], [
    $sound->getPosition()->getX(),
    $sound->getPosition()->getY(),
    $sound->getPosition()->getZ(),
]);

$data = $sound->toArray();
$check('sound toArray x', 1.5, $data['x']);
$check('sound toArray name', 'place', $data['sound']);
$check('sound toArray recipients', ['steve'], $data['recipients']);
$check('sound toArray cancelled', false, $data['cancelled']);

// Un listener puede cancelar y cambiar el sonido por uno de LevelEvent.
$sound->cancel();
$sound->setSound(new ClickSound(0.5));
$sound->setRecipients(['Alex', 'steve']);
$data = $sound->toArray();
$check('sound cancelled', true, $data['cancelled']);
$check('sound switched to level event', 1000, $data['event_id']);
$check('sound pitch data', 500, $data['extra_data']);
$check('sound null name', null, $data['sound']);
$check('sound recipients lowercased', ['alex', 'steve'], $data['recipients']);
// Sin servidor inicializado no hay `Player[]` que resolver.
$check('sound recipients players', [], $sound->getRecipients());

$particle = EventFactory::create('WorldParticleEvent', [
    'world' => 'world',
    'x' => 3.25,
    'y' => 65.5,
    'z' => 4.0,
    'event_id' => 0x4000 | 8,
    'event_data' => 0,
    'recipients' => [],
]);
$check('particle event class', true, $particle instanceof WorldParticleEvent);
$check('particle cancellable', true, $particle->isCancellable());
$check('particle wire class', true, $particle->getParticle() instanceof WireParticle);
$check('particle event id', 0x4000 | 8, $particle->getParticle()->getPacketEvent());
$check('particle position', 3.25, $particle->getPosition()->getX());

$data = $particle->toArray();
$check('particle toArray id', 0x4000 | 8, $data['event_id']);
$check('particle toArray recipients', [], $data['recipients']);

$stone = Block::fromName('stone', 'world', 0, 64, 0);
$particle->setParticle(new BlockBreakParticle($stone));
$particle->cancel();
$data = $particle->toArray();
$check('particle switched to destroy', 2001, $data['event_id']);
$check('particle switched data', $stone->getStateId(), $data['event_data']);
$check('particle cancelled', true, $data['cancelled']);

if ($failures > 0) {
    fwrite(STDERR, "FAILURES: $failures\n");
    exit(1);
}
echo "ALL OK\n";
