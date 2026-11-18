<?php
declare(strict_types=1);

$inventory = json_decode(file_get_contents(__DIR__ . '/block_classified.json'), true);
$artifactPath = 'C:/Users/matic/.gemini/antigravity/brain/86f9f545-779e-4b57-971b-25502fa31ca3/block_inventory.md';

$out = "# Inventario Completo y Triaje de `pocketmine\\block` (PM5 / axolotl)\n\n";
$out .= "Fecha de medición: " . date('Y-m-d H:i:s') . "\n";
$out .= "Total de declaraciones analizadas: **" . count($inventory) . "**\n\n";

$familyTitles = [
    1 => "Familia 1: Núcleo y Registro",
    2 => "Familia 2: Formas y Geometría",
    3 => "Familia 3: Interactivos, Contenedores y Redstone",
    4 => "Familia 4: Fluidos, Fuego, Plantas y Crecimiento",
    5 => "Familia 5: Block Tiles",
    6 => "Familia 6: Utils e Inventario",
];

$byFamily = [1 => [], 2 => [], 3 => [], 4 => [], 5 => [], 6 => []];
foreach ($inventory as $d) {
    $byFamily[$d['family']][] = $d;
}

$out .= "## Resumen por Familia\n\n";
$out .= "| ID | Familia | Declaraciones | Métodos Públicos | Instanciada en VBI | Con Tile |\n";
$out .= "|---|---|---:|---:|---:|---:|\n";

$totalMethods = 0;
$totalVbi = 0;
$totalTiles = 0;

foreach ($byFamily as $fId => $items) {
    $mCount = array_sum(array_column($items, 'public_methods_count'));
    $vCount = count(array_filter($items, fn($x) => $x['vbi_instantiated']));
    $tCount = count(array_filter($items, fn($x) => $x['has_tile']));
    $totalMethods += $mCount;
    $totalVbi += $vCount;
    $totalTiles += $tCount;
    $out .= sprintf("| %d | %s | %d | %d | %d | %d |\n", $fId, $familyTitles[$fId], count($items), $mCount, $vCount, $tCount);
}
$out .= sprintf("| **Total** | | **%d** | **%d** | **%d** | **%d** |\n\n", count($inventory), $totalMethods, $totalVbi, $totalTiles);

foreach ($byFamily as $fId => $items) {
    $out .= "## " . $familyTitles[$fId] . " (" . count($items) . " declaraciones)\n\n";
    $out .= "| FQCN | Archivo | Extends / Implements | Métodos | VBI | Tile | Deps (I, W, P, T, Inv) |\n";
    $out .= "|---|---|---|---:|:---:|:---:|---|\n";
    foreach ($items as $d) {
        $deps = [];
        if ($d['dep_item']) $deps[] = 'Item';
        if ($d['dep_world']) $deps[] = 'World';
        if ($d['dep_player']) $deps[] = 'Player';
        if ($d['dep_tile']) $deps[] = 'Tile';
        if ($d['dep_inventory']) $deps[] = 'Inv';
        $depStr = !empty($deps) ? implode(', ', $deps) : '-';
        
        $extImp = $d['extends'];
        if ($d['implements'] !== '-') {
            $extImp .= ' imp ' . $d['implements'];
        }
        
        $out .= sprintf(
            "| `%s` | `%s` | `%s` | %d | %s | %s | %s |\n",
            $d['fqcn'],
            $d['file'],
            $extImp,
            $d['public_methods_count'],
            $d['vbi_instantiated'] ? '✅' : '❌',
            $d['has_tile'] ? '✅' : '❌',
            $depStr
        );
    }
    $out .= "\n";
}

file_put_contents($artifactPath, $out);
echo "Written inventory artifact to $artifactPath with " . strlen($out) . " bytes.\n";
