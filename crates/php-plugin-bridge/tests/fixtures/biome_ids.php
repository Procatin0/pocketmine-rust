<?php

declare(strict_types=1);

/**
 * Regression: `pocketmine\data\bedrock\BiomeIds` only declared a dozen
 * constants, so a plugin using `BiomeIds::MESA` (or any of the other 77 ids)
 * died with "undefined constant".
 *
 * Uso interno del test de integración `tests/biome_ids.rs`: recibe como
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

use pocketmine\data\bedrock\BiomeIds;
use pocketmine\world\biome\Biome;
use pocketmine\world\biome\BiomeRegistry;

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

// Constantes que antes no existían, con el valor de PM5.
$check('MESA', 37, BiomeIds::MESA);
$check('MESA_BRYCE', 165, BiomeIds::MESA_BRYCE);
$check('DEEP_DARK', 190, BiomeIds::DEEP_DARK);
$check('CHERRY_GROVE', 192, BiomeIds::CHERRY_GROVE);
$check('ICE_CAVES', 196, BiomeIds::ICE_CAVES);
$check('THE_END', 9, BiomeIds::THE_END);
$check('SUNFLOWER_PLAINS', 129, BiomeIds::SUNFLOWER_PLAINS);
$check('BAMBOO_JUNGLE', 48, BiomeIds::BAMBOO_JUNGLE);

// El catálogo completo: cada constante declarada resuelve a un Biome con su id.
$reflection = new \ReflectionClass(BiomeIds::class);
$constants = $reflection->getConstants();
$check('catalog size', 90, count($constants));
$registry = BiomeRegistry::getInstance();
$seen = [];
foreach ($constants as $name => $id) {
    if (isset($seen[$id])) {
        $failures++;
        fwrite(STDERR, "FAIL duplicate id $id ($name and {$seen[$id]})\n");
        continue;
    }
    $seen[$id] = $name;
    if ($id < 0 || $id >= Biome::MAX_BIOMES) {
        $failures++;
        fwrite(STDERR, "FAIL $name out of range: $id\n");
        continue;
    }
    $biome = $registry->getBiome($id);
    $check("registry[$name]", $id, $biome->getId());
}

// El registro asigna las clases portadas y `UnknownBiome` para el resto.
$check('plains class', 'Plains', $registry->getBiome(BiomeIds::PLAINS)->getName());
$check('desert class', 'Desert', $registry->getBiome(BiomeIds::DESERT)->getName());
$check('birch class', 'Forest', $registry->getBiome(BiomeIds::BIRCH_FOREST)->getName());
$check('mesa fallback', 'Unknown', $registry->getBiome(BiomeIds::MESA)->getName());
$check('cherry fallback', 'Unknown', $registry->getBiome(BiomeIds::CHERRY_GROVE)->getName());
$check('out-of-catalog fallback', 'Unknown', $registry->getBiome(250)->getName());

if ($failures > 0) {
    fwrite(STDERR, "FAILURES: $failures\n");
    exit(1);
}
echo "ALL OK\n";
