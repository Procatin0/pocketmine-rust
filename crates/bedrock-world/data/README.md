# Datos de generación de mundo (Java 1.21.5)

Copias de los ficheros de datos de vanilla que describen la forma del terreno.
Son **datos del juego** (parámetros de ruido, splines y el grafo de funciones de
densidad), no código: la implementación que los consume está en
`src/generator/`.

Origen: <https://github.com/misode/mcmeta>, rama `1.21.5-data`, rutas
`data/minecraft/worldgen/{noise,noise_settings,density_function}`.

## Contenido

| Ruta | Qué es |
|---|---|
| `noise_settings_overworld.json` | Ajustes completos del Overworld: `noise_router` (qué función de densidad alimenta cada salida: `continents`, `erosion`, `ridges`, `depth`, `final_density`, ...), `sea_level`, acuíferos y reglas de superficie. |
| `density_function/overworld/*.json` | Grafo de funciones de densidad: `base_3d_noise`, `offset`, `factor`, `jaggedness`, `depth`, `continents`, `erosion`, `ridges`, `ridges_folded` y `sloped_cheese` (la densidad final del terreno). Las splines de continentes/erosión van **inline** dentro de `offset.json`, `factor.json` y `jaggedness.json`. |
| `density_function/overworld/caves/*.json` | Carvadores de cuevas: `spaghetti_2d`, `noodle`, `entrances`, `pillars`. |
| `noise/*.json` | Parámetros de cada ruido con nombre: `firstOctave` y `amplitudes` (p. ej. `continentalness`, `jagged`, `surface`, `cave_cheese`). |

## Cómo se usan

1. `noise/<nombre>.json` da la configuración de octavas de cada ruido; la sal se
   deriva de `md5("minecraft:<nombre>")`, igual que en el muestreador de clima
   ya portado (`java_biomes.rs`).
2. `density_function/...` es un árbol de operaciones: `add`, `mul`, `min`, `max`,
   `abs`, `sub`, `clamp`, `y_clamped_grad`, `noise`, `spline`, `quarter_negative`,
   `half_negative`, `old_blended_noise`, `interpolated`.
3. El objetivo de la fase 2 es evaluar `final_density` por columna y comparar la
   altura de superficie resultante con el `heightmap` que el BDS escribe en el
   registro `Data3D` (ver `docs/bds-biome-parity.md`).
