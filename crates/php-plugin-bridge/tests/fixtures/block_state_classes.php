<?php

declare(strict_types=1);

/**
 * Regresión: `Block::fromStateId()` construía siempre la clase base `Block`,
 * así que el aire leído del mundo (que pasa por `fromCoords()` ->
 * `fromStateId()`) respondía `isSolid() === true`.
 *
 * Uso interno del test de integración `tests/block_state_classes.rs`:
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

// --- Aire desde un state id (el camino de `World::getBlock()`) --------------
$air = \pocketmine\block\Block::fromStateId(\pocketmine\block\Block::AIR_STATE_ID);
$check('air class', \pocketmine\block\Air::class, get_class($air));
$check('air is not solid', false, $air->isSolid());
$check('air is transparent', true, $air->isTransparent());
$check('air has no drops', [], $air->getDrops());
$check('air type id', \pocketmine\block\BlockTypeIds::AIR, $air->getTypeId());

// --- Un bloque sólido sigue siendo sólido y opaco ---------------------------
$stone = \pocketmine\block\Block::fromStateId(\pocketmine\block\Block::resolveStateId('minecraft:stone'));
$check('stone class', \pocketmine\block\Block::class, get_class($stone));
$check('stone is solid', true, $stone->isSolid());
$check('stone is not transparent', false, $stone->isTransparent());

// --- Los caminos por nombre eligen la misma clase ---------------------------
$check('fromName air class', \pocketmine\block\Air::class, get_class(\pocketmine\block\Block::fromName('minecraft:air')));
$check('VanillaBlocks::AIR class', \pocketmine\block\Air::class, get_class(\pocketmine\block\VanillaBlocks::AIR()));

// --- Familias con clase propia ---------------------------------------------
$leaves = \pocketmine\block\Block::fromName('minecraft:oak_leaves');
$check('leaves class', \pocketmine\block\Leaves::class, get_class($leaves));
$check('leaves are transparent', true, $leaves->isTransparent());

// --- Capa 2: estados Bedrock -----------------------------------------------
$check('BlockTypeNames::STRUCTURE_BLOCK', 'minecraft:structure_block', \pocketmine\data\bedrock\block\BlockTypeNames::STRUCTURE_BLOCK);
$check('BlockStateNames::STRUCTURE_BLOCK_TYPE', 'structure_block_type', \pocketmine\data\bedrock\block\BlockStateNames::STRUCTURE_BLOCK_TYPE);
$check('BlockStateStringValues', 'data', \pocketmine\data\bedrock\block\BlockStateStringValues::STRUCTURE_BLOCK_TYPE_DATA);
$check('palette version', true, \pocketmine\data\bedrock\block\BlockStateData::CURRENT_VERSION > 0);
$check(
    'BlockStateVersion matches',
    \pocketmine\data\bedrock\block\BlockStateVersion::PALETTE_VERSION,
    \pocketmine\data\bedrock\block\BlockStateData::CURRENT_VERSION
);

$versionString = sprintf(
    '%d.%d.%d.%d',
    (\pocketmine\data\bedrock\block\BlockStateData::CURRENT_VERSION >> 24) & 0xff,
    (\pocketmine\data\bedrock\block\BlockStateData::CURRENT_VERSION >> 16) & 0xff,
    (\pocketmine\data\bedrock\block\BlockStateData::CURRENT_VERSION >> 8) & 0xff,
    \pocketmine\data\bedrock\block\BlockStateData::CURRENT_VERSION & 0xff
);
$check(
    'version string',
    $versionString,
    \pocketmine\data\bedrock\block\BlockStateData::current('minecraft:stone', [])->getVersionAsString()
);

// `BlockStateWriter` encadenado, como en el `serialize()` de un bloque.
$writerState = \pocketmine\data\bedrock\block\convert\BlockStateWriter::create(\pocketmine\data\bedrock\block\BlockTypeNames::STRUCTURE_BLOCK)
    ->writeString(\pocketmine\data\bedrock\block\BlockStateNames::STRUCTURE_BLOCK_TYPE, \pocketmine\data\bedrock\block\BlockStateStringValues::STRUCTURE_BLOCK_TYPE_DATA)
    ->writeBool('some_flag', true)
    ->writeInt('some_count', 3)
    ->getBlockStateData();
$check('writer name', 'minecraft:structure_block', $writerState->getName());
$check(
    'writer string tag',
    'data',
    $writerState->getState(\pocketmine\data\bedrock\block\BlockStateNames::STRUCTURE_BLOCK_TYPE) instanceof \pocketmine\nbt\tag\StringTag
        ? $writerState->getState(\pocketmine\data\bedrock\block\BlockStateNames::STRUCTURE_BLOCK_TYPE)->getValue()
        : null
);
$check('writer bool tag', true, $writerState->getState('some_flag') instanceof \pocketmine\nbt\tag\ByteTag);
$check('writer int tag', true, $writerState->getState('some_count') instanceof \pocketmine\nbt\tag\IntTag);

// Desde NBT de bloque (tile/item), como `BlockItemConvertor`.
$fromNbt = \pocketmine\data\bedrock\block\BlockStateData::fromNbt(\pocketmine\nbt\tag\CompoundTag::create()
    ->setString('name', 'minecraft:stone')
    ->setInt('version', 42)
    ->setCompoundTag('states', \pocketmine\nbt\tag\CompoundTag::create()->setString('stone_type', 'granite')));
$check('fromNbt name', 'minecraft:stone', $fromNbt->getName());
$check('fromNbt version', 42, $fromNbt->getVersion());
$check(
    'fromNbt state',
    'granite',
    $fromNbt->getState('stone_type') instanceof \pocketmine\nbt\tag\StringTag
        ? $fromNbt->getState('stone_type')->getValue()
        : null
);
$check('toNbt round-trip', true, $fromNbt->equals(\pocketmine\data\bedrock\block\BlockStateData::fromNbt($fromNbt->toNbt())));
$check('toVanillaNbt name', 'minecraft:stone', $fromNbt->toVanillaNbt()->getString('name'));

// Serializer/deserializer de ida y vuelta (sin puente: tabla legacy).
$stoneBlock = \pocketmine\block\Block::fromStateId(\pocketmine\block\Block::resolveStateId('minecraft:stone'));
$state = \pocketmine\world\format\io\GlobalBlockStateHandlers::getSerializer()->serialize($stoneBlock);
$check('serializer name', 'minecraft:stone', $state->getName());
$back = \pocketmine\world\format\io\GlobalBlockStateHandlers::getDeserializer()->deserialize($state);
$check('deserializer name', 'minecraft:stone', $back->getTypeName());

// Mapeo explícito del serializer (el patrón de `Registry::registerToNetwork`).
$helper = new \pocketmine\block\Opaque(
    new \pocketmine\block\BlockIdentifier(\pocketmine\block\BlockTypeIds::newId()),
    'EasyEdit Helper',
    new \pocketmine\block\BlockTypeInfo(\pocketmine\block\BlockBreakInfo::instant())
);
\pocketmine\world\format\io\GlobalBlockStateHandlers::getSerializer()->map($helper, function (\pocketmine\block\Block $block): \pocketmine\data\bedrock\block\convert\BlockStateWriter {
    return \pocketmine\data\bedrock\block\convert\BlockStateWriter::create(\pocketmine\data\bedrock\block\BlockTypeNames::STRUCTURE_BLOCK)
        ->writeString(\pocketmine\data\bedrock\block\BlockStateNames::STRUCTURE_BLOCK_TYPE, \pocketmine\data\bedrock\block\BlockStateStringValues::STRUCTURE_BLOCK_TYPE_DATA);
});
$check(
    'mapped serializer',
    'minecraft:structure_block',
    \pocketmine\world\format\io\GlobalBlockStateHandlers::getSerializer()->serialize($helper)->getName()
);

// Propiedad entera: `BlockStateData::fromNbt()` la deja como `IntTag` y el host
// indexa la paleta por strings, así que el deserializer la convierte.
$intState = \pocketmine\data\bedrock\block\BlockStateData::fromNbt(\pocketmine\nbt\tag\CompoundTag::create()
    ->setString('name', 'minecraft:stone')
    ->setCompoundTag('states', \pocketmine\nbt\tag\CompoundTag::create()->setInt('stone_type', 1)));
$check('fromNbt int tag', true, $intState->getState('stone_type') instanceof \pocketmine\nbt\tag\IntTag);
$check(
    'int property deserializes to the same block',
    'minecraft:stone',
    \pocketmine\world\format\io\GlobalBlockStateHandlers::getDeserializer()->deserialize($intState)->getTypeName()
);

// El payload que recibe `block.network_id` ya va en strings.
$encode = new \ReflectionMethod(\pocketmine\data\bedrock\block\convert\BlockStateDeserializer::class, 'encodeStates');
$encode->setAccessible(true);
$check(
    'int property encoded as string',
    ['stone_type' => '1', 'shape' => 'data', 'open' => '1'],
    $encode->invoke(null, [
        'stone_type' => new \pocketmine\nbt\tag\IntTag(1),
        'shape' => new \pocketmine\nbt\tag\StringTag('data'),
        'open' => new \pocketmine\nbt\tag\ByteTag(1),
    ])
);

// Estado desconocido -> UnknownBlock, como en PM.
$unknown = \pocketmine\world\format\io\GlobalBlockStateHandlers::getDeserializer()->deserialize(\pocketmine\world\format\io\GlobalBlockStateHandlers::getUnknownBlockStateData());
$check('unknown block class', \pocketmine\block\UnknownBlock::class, get_class($unknown));
$check('unknown is not solid', false, $unknown->isSolid());

// --- Internet (EasyEdit descarga su repositorio de estados) ----------------
$check('Internet class', true, class_exists(\pocketmine\utils\Internet::class));
$result = new \pocketmine\utils\InternetRequestResult([['content-type' => 'text/plain']], 'body', 200);
$check('InternetRequestResult code', 200, $result->getCode());
$check('InternetRequestResult body', 'body', $result->getBody());
$check('InternetRequestResult header', 'text/plain', $result->getHeaders()[0]['content-type'] ?? null);
$err = null;
$check(
    'Internet failure returns null',
    null,
    \pocketmine\utils\Internet::getURL('http://127.0.0.1:1/', 1, [], $err)
);
$check('Internet failure sets err', true, is_string($err) && $err !== '');

if ($failures > 0) {
    fwrite(STDERR, $failures . " failure(s)\n");
    exit(1);
}

echo "ALL OK\n";
