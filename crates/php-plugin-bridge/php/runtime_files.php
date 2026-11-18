<?php

declare(strict_types=1);

/**
 * Ordered list of the PHP runtime files.
 *
 * This is the single PHP-side source of truth, used by:
 * - `bootstrap.php`, which requires the runtime in the worker process.
 * - `thread.php` (`CommonThreadPartsTrait::run()`), which has to re-declare the
 *   runtime inside a child thread: pthreads threads start with a fresh
 *   interpreter, so the parent's classes, constants, globals, functions and
 *   autoloaders are not visible there.
 *
 * The Rust side (`PHP_RUNTIME_FILES` in `src/runtime/bootstrap.rs`) embeds the
 * same files and is kept in sync by a unit test.
 *
 * `bootstrap.php` and this file do not appear in the list: nothing requires
 * them.
 */
// pmmpthread child threads start with a fresh interpreter: PHP's CLI `STDERR`
// constant is not carried over, and the runtime loggers write to it. Define it
// before any runtime file can log (the main thread already has it).
if (!\defined('STDERR')) {
    \define('STDERR', \fopen('php://stderr', 'wb'));
}

return [
    'math.php',
    'binary.php',
    'internet.php',
    'registry.php',
    'nbt.php',
    'nbt_serializer.php',
    // Generadas por `build.rs` desde `data/canonical_block_states.nbt`.
    'block_state_version.php',
    'block_type_names.php',
    'block_state_names.php',
    'block_state_string_values.php',
    'block_state.php',
    'block_utils.php',
    'block.php',
    'block_shapes.php',
    'block_fluids.php',
    'block_plants.php',
    'world_sound.php',
    'text.php',
    'permission.php',
    'config.php',
    'thread.php',
    'scheduler.php',
    'command.php',
    'item_type_ids.php',
    'item.php',
    'item_tools.php',
    'item_projectiles.php',
    'item_food.php',
    'enchantment.php',
    'item_armor.php',
    'item_blocks.php',
    'item_books.php',
    'item_misc.php',
    'item_registry.php',
    'item_vanilla.php',
    'item_parser.php',
    'crafting.php',
    'world.php',
    // Módulo `world` por partes: `World` referencia estas clases en runtime.
    'world_format.php',
    'world_biome.php',
    'world_particle.php',
    'block_inventory.php',
    'block_tiles.php',
    'block_interactive.php',
    'block_concrete.php',
    'player.php',
    'effect.php',
    // `object.php`/`projectile.php` extend `Entity` from `player.php`.
    'object.php',
    'projectile.php',
    'form.php',
    'scoreboard.php',
    'bossbar.php',
    'chat.php',
    'core.php',
    'event.php',
    // After `event.php`/`player.php`: extends `Event` and uses `EventData`.
    'PlayerDataSaveEvent.php',
    'plugin.php',
    'autoload.php',
    'ipc.php',
];
