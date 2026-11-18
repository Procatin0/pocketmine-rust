

   **Validacion cruzada del ruido (semilla 0, `examples/field_stats`)**: el
   campo `continents` del terreno y la continentalidad del clima (puerto del
   clima ya validado bit a bit contra cubiomes) coinciden en media, desviacion
   y extremos: -0,053 / 0,172 / [-0,645, +0,304] ambos. Igual para `erosion`
   (+0,049 / 0,093). Es la prueba de que la normalizacion de `NormalNoise` y
   la atenuacion de octavas estan bien.

   **Que hay realmente en `data/bds-reference/seed-*.json`**

   Cada chunk guarda `x`, `z`, `y`, `heightmap` (256 `i16`) y `runs`
   (`[[id, largo], ...]`, 256 columnas en total). Los `runs` **no son bloques**:
   son los ids de bioma del plano y=64 en el orden `x | (z << 4)`, tal como los
   genera `bds-probe export` con `biome_grid`. Sus valores son ids de bioma de
   Bedrock (0, 1, 4, 7, 16, 24, 25, 29, 187...), sin paleta ni estados de
   bloque detras.

   Por tanto **estos ficheros no permiten ninguna comprobacion de bloques**.
   La unica metrica real que admiten los `runs` es la coincidencia de bioma por
   columna en y=64, calculada directamente contra el JSON (sin LevelDB) con
   `examples/biome_bedrock`:

   | semilla | columnas | coincide | % |
   |---|---|---|---|
   | 0 | 6.912 | 6.407 | 92,69 % |
   | 1 | 4.864 | 4.228 | 86,92 % |
   | 42 | 9.216 | 8.226 | 89,26 % |
   | 12345 | 11.264 | 10.949 | 97,20 % |

   Son los mismos porcentajes que daba `bds-probe compare --impl java` contra
   la base LevelDB, ahora reproducibles solo con los ficheros versionados
   (la comparacion columna a columna vive en `examples/biome_bedrock`).

   Confusiones dominantes: (4->1) y (16->7) en la semilla 0, (0<->25) en la 1,
   (29->16) y (16->29) en la 42, (29->187) y (0->29) en la 12345: justo los
   pares que cambiaron entre 1.21 y 1.26 (bosques nuevos, cuevas de azufre y
   hielo, rios y costas).

   **Lo unico vertical: el `heightmap`**

   Permite comparar la altura de superficie predicha con la observada, no la
   identidad de bloque. Sus limites:

   * 1.692, 3.539, 2.916 y 8.809 columnas por semilla valen exactamente 127.
     Con la lectura `y = valor - 64` son y=63, el nivel del mar, y la
     ambiguedad es irreducible: oceano (agua en 63) y llanura costera (bloque
     solido en 63) dan el mismo 127.
   * Hay valores por debajo de 127 (333 en la semilla 0, 21 en la 42), que con
     `y = valor - 64` son superficie bajo el nivel del mar; encaja con
     depresiones secas y aquiferos, pero no se puede confirmar sin bloques.
   * Incluye agua, hierba y vegetacion, de modo que no es el limite del campo
     de densidad; por eso el sesgo medido en tierra es positivo (+5 a +11
     bloques) y no es error de densidad.

   Alturas medidas en columnas con valor > 127 (tierra emergida):

   | semilla | columnas | r | MAE |
   |---|---|---|---|
   | 0 | 4.887 | +0,589 | 5,8 |
   | 1 | 1.325 | +0,798 | 5,7 |
   | 42 | 6.279 | +0,504 | 9,5 |
   | 12345 | 2.455 | +0,933 | 12,2 |

   **Lo que no se puede medir con estos datos** (haria falta volver a capturar
   los sub-chunks `0x2F` del LevelDB): identidad de bloque a cualquier altura,
   techos de cuevas y aquiferos, reglas de superficie (hierba/tierra/arena),
   features y estructuras, profundidad del agua y biomas en alturas distintas
   de y=64.

   **Muestreo por celdas 4x8x4 (hecho)**: vanilla no evalua el campo en cada
   bloque, sino en las esquinas de una celda 4x8x4 y interpola. Con eso el MAE
   en tierra baja de 8,41 a **7,53** bloques (sesgos +4,9 / +3,9 / +8,5 / +9,9
   por semilla).

   **Grafo `final_density` completo, disponible pero aun peor**: se anadieron
   `squeeze`, `square`, `weird_scaled_sampler`, los datos de `final_density`,
   los seis ficheros de `caves/` y los **60 ruidos** de vanilla (`data/noise/`,
   cada uno con su sal `md5("minecraft:<nombre>")`). Se pide explicitamente con
   `TerrainDensity::with_root(seed, DensityRoot::FinalDensity)` (ejemplo:
   `surface_height <seed> final`); no hay ninguna variable de entorno.

   Mediciones con el MISMO extractor de altura (`surface_height` barre el
   bloque solido mas alto desde y=319, no biseca):

   | configuracion | MAE en tierra | sesgos por semilla |
   | --- | --- | --- |
   | `DensityRoot::SlopedCheese` (defecto) | **7,53** | +4,9 / +3,9 / +8,5 / +9,9 |
   | `DensityRoot::FinalDensity`, tal cual esta en el repositorio | **8,61** | +5,9 / +7,0 / +9,3 / +12,0 |

   La tabla anterior daba 16,60 para `FinalDensity`. Ese numero era en gran parte
   error del extractor: `surface_height` bisecaba el cruce de densidad asumiendo
   monotonia en y, y las cuevas la rompen, asi que aterrizaba en el suelo de una
   cueva. Con el barrido del bloque solido mas alto (que es lo que guarda el
   heightmap) la diferencia real entre los dos grafos es de **1,08 bloques**, no
   de 9. Por eso `SlopedCheese` sigue siendo el defecto.

   Medidas de diagnostico, todas con el extractor viejo y por tanto no
   comparables con la tabla de arriba:

   - `FinalDensity` con interpolacion 4x8x4 externa anadida: 16,60, identico al
     valor sin ella; la doble interpolacion no era la causa.
   - `FinalDensity` con `caves/entrances` forzado a la constante 1,0: 14,90. Ese
     parche (devolver `Density::Constant(1.0)` en la referencia) era de un solo
     uso y **no esta en el codigo**; no hay variable de entorno ni flag que lo
     active.

   Lo que queda descartado:

   - `slide` no es la causa: sus dos `y_clamped_gradient` y los `add` de
     `+0,1171875/-0,1171875/-0,078125/+0,078125` son identidad entre y=-40 y
     y=240, asi que sobre la superficie solo queda `sloped_cheese` tal cual.
   - El `min` raiz tampoco: en las columnas sondeadas la rama de cuevas vale
     +64,0 y siempre gana el terreno.
   - La doble interpolacion tampoco.

   Lo que queda abierto: el sesgo es positivo en los dos grafos (+5 a +12) y
   crece con `FinalDensity`, y `squeeze` conserva el signo, asi que ese bloque
   extra de error viene del tallado de cuevas (`range_choice` con
   `min(sloped_cheese, 5*entrances)` y el `min` raiz): nuestras cuevas recortan
   superficie que vanilla deja solida. Ahi siguen `weird_scaled_sampler`
   (mapeador de rareza lineal, aproximado) y la normalizacion de los ruidos de
   cueva como sospechosos.

   `TerrainDensity::new(seed)` sigue usando `DensityRoot::SlopedCheese` por
   defecto hasta cerrar eso, con MAE 7,53.

## Desvios deliberados del generador jugable (`BDSTerrain`)

El mundo que sirve el servidor (`level_type = bds`) se aparta a proposito de la
paridad exacta para que el terreno no se vea como un mapa de ruido. Todo vive en
`crates/bedrock-world/src/generator/bds_terrain.rs` como constantes:

- `CLIMATE_SCALE = 0.25`: los climas se muestrean a 0,25 de la coordenada, asi
  que biomas y continentes cubren ~4x mas bloques (medido: regiones de bioma de
  ~945 celdas, mediana 108).
- `WARP_BLOCKS = 60`: el punto de muestreo del clima se deforma con dos ruidos de
  baja frecuencia. Sin esto el campo es una funcion suave y los lagos y bordes de
  bioma salen como elipses casi perfectas; con el warp los contornos se doblan.
- `RELIEF_BLOCKS = 8` + `DETAIL_BLOCKS = 4`: ruidos de relieve (baja frecuencia) y
  detalle (alta frecuencia) sumados al offset de altura, que rompen los "anillos
  de nivel" que deja un campo puramente de baja frecuencia y hacen irregulares
  las orillas.
- `RIVER_HALF_WIDTH = 0.14` y `RIVER_DEPTH = 5`: un ruido propio cuyo cruce por
  cero se excava hasta 5 bloques bajo el nivel del mar, con la profundidad
  desvaneciendose hacia el borde de la banda. La banda mide ~8 bloques de ancho
  (una unidad de ruido son ~30 bloques en el cruce).
- `LAND_BIAS = 0.2`: se suma al offset de altura (una unidad = 32 bloques, asi
  que son ~6,4 bloques). El spline pone el offset 0 en y≈63, apenas por encima
  del mar, asi que la orilla caia donde el spline cruza cero y una tajada enorme
  del mapa quedaba bajo el agua. Subir el campo mete la orilla mar adentro: con
  esto el agua baja de 34-52 % a 20-31 % (768x768 bloques, semillas 0/12345/42)
  y la tierra deja de verse como islas.
- `SHORE_BAND = 10` + `SHORE_COMPRESS = 0.45`: dentro de 10 bloques sobre el
  nivel del mar la altura se comprime (factor que sube de 0,45 en la orilla a 1
  en el borde de la banda). El spline sube del mar a 2-3 bloques por bloque, que
  se ve como un acantilado con agua al pie; comprimido es una playa. Solo se
  comprime el lado seco: subir tambien el lado del mar rellenaria la plataforma.
- `DEPTH_NOISE_BLOCKS = 9` + `DEPTH_NOISE_VERTICAL = 1,5` y octavas de 16/8/4
  bloques: ruido **3D dentro de la funcion de densidad**
  (`density_at_with_offset`), no una decoracion de la altura. La superficie es el
  cruce por cero de esa funcion, asi que es la inversa de una funcion suave y no
  puede salir en escalones. El intento anterior (sumar un ruido 2D a una altura
  ya calculada) o dejaba terrazas o, a escala ~4 bloques con 4-6 bloques de
  amplitud, destrozaba la superficie con un pico por columna: fuera.
  Medido sobre 320x320 bloques (semilla 0): 50,7 % de parejas de columnas vecinas
  a la misma altura en tierra (antes 56 %), salto p95 de 4 bloques en tierra y 6
  en el fondo marino.
- `SEA_FLOOR_RAMP = 6` + `SEA_FLOOR_MAX_DEPTH = 22`: donde la columna ya esta
  bajo el mar, el fondo baja una cantidad proporcional a su profundidad
  (smoothstep) mas un ruido de relieve suave. Es una **resta**, no un `min` ni un
  multiplicador: en la orilla vale 0 y crece con el calado, asi que no hay muro
  donde empieza el agua. Un intento previo de estirar el offset negativo 6x
  convirtio el escalon propio del spline (continentalidad ≈ -0,47) en un
  acantilado de 20+ bloques, que es lo que se veia en captura.
- Voto mayoritario de bioma sobre las 3x3 celdas cuarto alrededor de cada celda,
  que disuelve los especks de una sola celda (biomas de cueva y empates del
  arbol de clima) que rompian las formas en confeti.
- El paso de difuminado Gaussiano de elevaciones por bioma se elimino: calculaba
  `min_heights`/`max_heights` que nadie leia. Ya no cuesta trabajo por chunk, y
  desaparece la maquinaria que redondeaba las siluetas de bioma.

La paridad sigue medida y verificada por `BDSTerrain::biome_at_bds`, que muestrea
el clima en las coordenadas crudas (sin warp ni escala). Es lo que usan el test
`niduy_parity_climate_noise_and_biomes_seed_12345` y `bds-probe compare`; el
`biome_at` normal es el que ve el jugador.

Medido con semilla 0 y 60 chunks por linea (cambios de bioma por 1000 bloques):

| | antes | despues |
| --- | --- | --- |
| cambios de bioma en chunk | 32,29 | 27,43 |
| pendiente media de altura (bloques/bloque) | 0,250 | 0,301 |
| 2a diferencia media de altura | 0,282 | 0,412 |

### Suelo submarino y detalle de altura

El `GroundCoverPopulator` solo aplicaba la capa seca del bioma, asi que cualquier
hondonada bajo el nivel del mar quedaba con la hierba/tierra de su bioma. Ahora,
cuando la columna tiene agua encima (`SEA_LEVEL = 62`), se aplican las reglas de
superficie de vanilla:

- hasta `SHALLOW_WATER = 3` bloques de agua: tierra, con parches de grava y arena;
- mas profundo: una capa de grava sobre piedra, con bancos de arena;
- oceanos templados y calidos: 4 capas de arena con arenisca debajo;
- pantanos: tierra con parches de arena.

Los parches salen de un value-noise estable entre chunks (hash + interpolacion
suave), no de numeros aleatorios por chunk, asi que no hay costuras.

### El agua que faltaba (rios secos y mares a medias)

El relleno de agua estaba condicionado al bioma (`is_water_biome(biome) ||
river > 0.05`), mientras que la excavacion de rios y el spline dejaban columnas
bajo el nivel del mar en cualquier bioma. El resultado, medido sobre 25x25 chunks
(semilla 0), era exactamente el sintoma reportado:

| | antes | despues |
| --- | --- | --- |
| columnas con agua en y=62 (320x320, semilla 0) | 8,7 % | 14,3 % |
| columnas con agua (768x768, semillas 0/12345/42) | 34-52 % | 19,8 / 31,0 / 25,4 % |
| columnas excavadas bajo el mar y secas | 31,6 % | **0 %** |
| columnas con agua pero huecos de aire dentro | 0 % | 0 % |
| agua de 11-20 bloques de profundidad | 0 % | 30 % del agua |

El escalon de altura entre columnas vecinas en la orilla mide p95 6 / p99 12
bloques (320x320, semilla 0). Con el estiramiento ×6 del offset habia muros de
20+ bloques, que es el acantilado que se veia en el juego.

La regla actual inunda **solo** los biomas de mar y rio (`is_water_biome`) y la
banda excavada por el ruido de rio. Ademas, una columna de bioma acuatico se
recorta a `sea - 2`, para que un bulto de ruido no levante una isla dentro del
mar.

Historial de reglas, porque cada una arregla algo y rompe otra cosa: inundar
solo los biomas de agua dejaba hondonadas excavadas y secas (el bug original);
inundar **toda** columna bajo el nivel del mar convierte el mapa en un mosaico de
charcos e islitas, porque con el ruido 3D la superficie cruza y=62 cada pocos
bloques; e inundar solo las hondonadas de mas de 4 bloques parte la tierra en
masas pequenas, porque esas hondonadas estan por todas partes.

Medido con semilla 0: 768x768 bloques dan 27,2 % de agua en 75 masas (la mayor de
155271 bloques) y 72,8 % de tierra en 98 componentes, de los que 66 son menores de
256 bloques y suman el 0,18 % del area. En 320x320: 14,8 % de agua, 12 masas.

Lo que queda son islas diminutas en mar abierto, y su origen no es el agua sino el
mapa de biomas: esas columnas estan clasificadas como bioma **de tierra** en medio
del mar por el arbol de clima, asi que no se inundan. Arreglarlo de verdad no es
otra regla de agua sino que la altura y el bioma salgan de la misma funcion; ver
`final_density` en `java_density.rs` (MAE 8,61 contra las capturas).

El bioma sigue decidiendo **de que** es el fondo (via `GroundCover`, que lee la
profundidad resultante) y **si hay** agua. Los rios pasaron de una linea de 1-2
bloques (se excavaba la banda entera y solo se inundaba su centro) a una banda de
~8 bloques con 5 de calado.

Medido con semilla 0 en 1024x1024 bloques (99248 columnas con agua, profundidad
media 6,5):

| fondo | global | con mas de 4 bloques de agua |
| --- | --- | --- |
| grava | 65,3 % | 80,2 % |
| arena | 18,9 % | 16,9 % |
| tierra | 13,7 % | 0,4 % |
| arcilla | 2,1 % | 2,5 % |

La arcilla va en los pantanos (capa principal y a veces la de debajo) y en
parches de agua somera y de fondo profundo. Su id no esta copiado a mano:
`block_id::CLAY = 666874214` es `block_states::state_hash("minecraft:clay", &[])`,
la misma funcion FNV-1a sobre el NBT canonico de estado que genera el resto de
constantes, y el test `block_id_constants_match_the_state_hash` comprueba que la
funcion reproduce `stone`, `dirt`, `sand`, `sandstone`, `gravel` y `clay`. Ese
test es lo que permite fiarse del valor derivado en vez de darlo por bueno a
ciegas: si la serializacion canonica cambiara, fallaria.

### Relieve: plains que se inclinan

La altura del terreno no depende del bioma; sale del spline de clima mas los
ruidos de detalle. El spline cambia a escala de cientos de bloques, asi que con
solo el era la unica forma del terreno: cada planicie salia como una meseta con
escalones en contorno, y el borde entre dos zonas se leia como un muro.

Ahora hay dos ruidos mas sumados al offset:

- `RELIEF_BLOCKS = 22`: relieve amplio (octavas de 256, 128 y 64 bloques), el que
  inclina las planicies y funde una zona con la siguiente;
- `DETAIL_BLOCKS = 4`: detalle de 32 y 16 bloques, el que quita los anillos de
  nivel y hace irregulares las orillas;

La tabla de abajo se midio con `RELIEF_BLOCKS = 14`; con 22 el relieve es mas
amplio, y el mapa de 768x768 de la seccion siguiente da el rango actual.

Medido con semilla 0 sobre 9000 columnas:

| | antes | despues |
| --- | --- | --- |
| columnas con la misma altura que la de al lado | 81,9 % | 73,4 % |
| pendiente media (bloques/bloque) | 0,207 | 0,295 |
| 2a diferencia media | 0,276 | 0,424 |

Sobre 512x512 muestras: alturas 38..117, media 64,0, 46 % bajo el agua.

Con esto las planicies ya no son planos horizontales perfectos, pero los
escalones fuertes siguen siendo posibles donde el spline cambia rapido: eso es
una pared de clima, no un bioma, y no se suaviza sin cambiar la forma del spline.

### La paridad de biomas ya no depende de la forma del terreno

El test `niduy_parity_...` comprueba los biomas contra capturas de BDS, y el arbol
de biomas recibe un parametro `depth` que sale del spline. Como ese test y el
terreno compartian la misma evaluacion del spline, **cualquier** experimento de
forma (probar interpolacion lineal, tocar el offset) lo rompia: paso dos veces y
hubo que revertir el cambio.

Ahora hay dos evaluaciones del mismo spline:

- `eval_spline`: la de siempre, Hermite con el `der` de cada punto. La usan
  `terrain_offset_raw` y el clasificador de biomas, o sea la ruta que tiene que
  seguir siendo comparable con las capturas.
- `eval_spline_straight`: segmentos rectos, para cuando `der == 0` (todos los
  puntos de este archivo). La usa solo `terrain_offset_from_climate`, que es la
  que da la *forma* del terreno.

Con eso se puede experimentar con el relieve sin tocar la paridad. El test sigue
pasando tal cual, sin suavizarlo.

### Bug de base: la altura analitica del spline

Buscando mas relieve aparecio un error viejo. La densidad es
`2.5 - y*4/128 - 83/160 + offset`, asi que el cruce por cero esta en
`(2.5 - 83/160 + offset) * 32`, pero `height_at_with_climate` arrancaba la busqueda
en `(1 - 83/160 + offset) * 32` y `spline_height` calculaba la altura con esa misma
formula. Las dos se quedaban 48 bloques cortas. Consecuencia practica:
`spline_height < sea` era **siempre** verdadero, asi que la regla "el mar es donde
el spline dice" nunca se aplico y el agua acababa decidiendose por la altura final.
Corregido con `DENSITY_BASE = 2.5 - 83/160` en los tres sitios (busqueda,
`spline_height` y la funcion de forma). Ahora el agua es simplemente "el suelo
queda por debajo del nivel del mar", sin predicado aparte.

### Clases de altitud: probadas y descartadas

Se probo estirar el terreno por encima del mar con una ganancia sacada de
`erosion` (region joven o gastada) y `peaks_and_valleys` (cresta o valle):

```
gain   = 1 + MOUNTAIN_GAIN * worn * (0.35 + 0.65 * ridge)
height = sea + (height - sea) * gain
```

Medido en 768x768, semilla 0, con `LAND_BIAS` reajustado en cada caso:

| `MOUNTAIN_GAIN` | p90 de altura | max | escalones de 2 (seco) |
| --- | --- | --- | --- |
| 0,0 (solo spline) | +19 | 53 | 2,85 % |
| 0,5 | +20 | 64 | 3,39 % |
| 0,8 | +23 | 73 | 3,88 % |
| 1,5 | +19 | 81 | 5,05 % |

Estirar el desnivel multiplica tambien los escalones: sube el techo del mundo un
20 % y los muros de dos bloques un 19 %. No compensa, asi que **se quito**. Para
montañas de verdad hace falta el pipeline de 1.18 (`slide` + `squash` + ruido de
jaggedness en 3D), que da picos sin convertir cada ladera en escalera; queda como
trabajo pendiente, no como una constante que se pueda subir.

### Escalas de clima por ruido

Antes los seis ruidos de clima se muestreaban con **una** escala compartida
(`CLIMATE_SCALE = 0.6`), asi que todos cerraban sobre los mismos contornos y los
biomas se estiraban en manchas enormes. En vanilla cada ruido tiene su frecuencia;
ahora hay `CLIMATE_SCALES: [f64; 6]`, en el orden temperatura, humedad,
continentalidad, erosion, picos, weirdness:

```
[1.3, 1.3, 0.6, 0.6, 0.6, 0.6]
```

Solo temperatura y humedad suben: son las que deciden *que* bioma recibe un trozo
de tierra, y con longitud de onda corta dejan de fundirse en un bosque infinito.
Los cuatro ruidos que alimentan el spline se quedaron en 0,6 a proposito: moverlos
cambia la forma del terreno (con 0,85 en picos y weirdness los escalones de dos
bloques subian de 2,83 % a 3,70 %), y el terreno ya esta en el punto que se queria.

El muestreo del mundo multiplica cada ruido por su escala; el muestreo de las
capturas de BDS (`biome_at_bds`) sigue a 1:1, que es lo que mantiene la paridad.

De paso aparecio un detalle del muestreo de `peaks` desplazado: la llamada
verificada usa `sample(x = z, y = x, z = 0)`, no `sample(x, 0, z)` como el resto.
Al refactorizar se perdio ese orden y el test de paridad empezo a fallar en un
punto concreto (`x = -1344`): ahora se documenta en el codigo.

Medido con `biome_histogram -- 24` (48x48 chunks, 3 km de lado, semilla 12345):

| | antes (`CLIMATE_SCALE` unico) | ahora |
| --- | --- | --- |
| biomas presentes | 31 | 32 |
| Oceano | 13,3 % | 7,5 % |
| Bosque | 19,4 % | 15,8 % |
| Llanura | 9,4 % | 6,6 % |
| Taiga | 3,7 % | 1,3 % |
| mancha de bioma: media / mediana / p90 | 80 / 52 / 200 bloques | 59 / 36 / 132 bloques |

La ventana de 192x192 que se usaba antes (`biome_histogram` sin argumentos) caia
casi entera dentro de un oceano y daba 79 % de oceano; con una ventana de 3 km la
distribucion ya era sana. Lo que cambia con las escalas no es el reparto sino el
tamaño de las manchas, y el terreno no se mueve: agua 25,4 %, p50 +6, p90 +19,
max 54, escalones secos de dos bloques 2,83 %.

### Minibiomas: el bioma y el terreno tienen que contar la misma historia

En el juego se veia una piramide de piedra desnuda en mitad de un bosque. No era
un parche pequeño: era la **costa pedregosa** de vanilla, que el arbol climatico
reparte mirando el clima y no la altura, y aqui el terreno es mucho mas suave de
lo que ese clima supone. Medido en 48x48 chunks (3 km) antes de tocar nada:

| bioma | altura del suelo | superficie |
| --- | --- | --- |
| costa pedregosa (25) | 61..96, p50 69 | 3,05 % del mapa, piedra |
| bosque de arboledas (185) | 62..125, p50 87 | 8,44 %, nieve |
| picos pedregosos (189) | desde 75 | 1,83 %, piedra |

Tres reglas nuevas, todas solo en el muestreo del mundo:

1. **Bandas de altura** para los biomas que existen por su altura:
   `STONE_SHORE` solo hasta 66 (la orilla), `GROVE` desde 88, `STONY_PEAKS`,
   `JAGGED_PEAKS` y `FROZEN_PEAKS` desde 92. Fuera de su banda caen a un bioma
   llano elegido por temperatura y humedad.
2. **El bioma sigue al agua.** La continentalidad decide "oceano" hasta 24
   bloques por encima del mar, y eso pintaba llanuras de grava (15 % de la
   superficie del mapa) donde el terreno era tierra firme. Ahora una columna mojada
   recibe el oceano que le toca por temperatura y profundidad (helado, frio,
   templado, tibio o calido, y la variante profunda a partir de 10 bloques) y una
   columna seca nunca se queda con un bioma de oceano.
3. **Sesgo de temperatura** de +0,15. Nuestro ruido de temperatura tiene mediana
   -0,15 (medido sobre 2 km) mientras que las bandas de vanilla (helado < -0,45,
   frio < -0,15, calido > 0,2) suponen un campo centrado en cero. Sin el sesgo,
   medio mundo salia taiga nevada.

Dos detalles que costaron encontrar:

- La comprobacion de altura tiene que usar la altura **real** de la columna, con
  el ruido de relieve (±26 bloques), no la del spline. Con el spline la puerta
  quedaba por debajo del terreno y un bosque de arboledas podia estar sobre suelo
  a cuarenta bloques bajo el mar.
- El muestreo de biomas es por celda de 4x4, asi que la altura que ve la regla es
  la de la esquina de la celda: dentro de una celda la comprobacion tiene unos
  ocho bloques de holgura. Es el mismo compromiso que hace vanilla con las celdas
  cuarto, y por eso el maximo medido de la costa pedregosa es 78 y no 66.

Antes y despues en la misma ventana de 768x768 (semilla 0):

| | antes | despues |
| --- | --- | --- |
| grava en la superficie del mapa | 15,1 % | 2,0 % |
| costa pedregosa (3 km) | 3,05 % | 0,36 % |
| biomas distintos (3 km) | 32 | 36 |
| parches de bioma < 64 columnas | 0,12 % del area | 0,15 % |
| agua en el mapa | 25,3 % | 25,4 % |
| p50 / p90 / max de altura | +6 / +19 / 54 | +6 / +19 / 54 |
| escalones secos de 2 bloques | 2,83 % | 2,83 % |

El terreno no se mueve: el bioma cambia, la forma no. La paridad de BDS tampoco:
todo esto vive en el muestreo del mundo, y `biome_at_bds` sigue devolviendo lo que
dice el arbol con el clima 1:1 (el test de paridad pasa sin tocarlo).

### Tamaño mínimo de bioma

El suavizado de biomas que habia votaba mayoria sobre 3 celdas de radio (12 bloques)
y, cuando ninguna mayoria ganaba, **se quedaba con el valor de la celda**. En campo
mezclado casi ninguna ventana tiene mayoria, asi que cada celda respondia con su
propio valor ruidoso: el filtro no limpiaba nada y las hilachas de arena, las
lenguas de tierras malas y las motas de grava seguian ahi. Eso es lo que se veia
como "minibiomas".

Ahora:

- La votacion es un **modo** (gana el mas repetido; empate al centro) sobre un radio
  de 8 celdas, o sea 32 bloques por lado. Un bioma que no aguante ~64 bloques de
  ancho pierde la votacion en todos sus bordes y desaparece.
- Dos pasadas de 3x3 sobre el resultado disuelven las muescas de una celda que deja
  la ventana grande al resolver cada celda por su cuenta.
- La ventana lee 23x23 celdas por chunk, asi que se anadio una cache de celdas de
  bioma (8192 entradas): sin ella el coste por chunk subia a ~30 ms; con ella el
  mapa de 2304 chunks tarda 10,4 s (**4,5 ms/chunk**, menos que los ~15 ms de antes
  de este cambio).

Lo que se ve en el mapa de 768x768: la tierras malas es ahora una sola mancha en
vez de cinco dedos, la arena de playa ya no deja cintas de dos bloques y
desaparecieron las motas grises dentro del bosque.

Aviso honesto sobre la metrica: `biome_histogram` cuenta *todos* los trozos
conectados de un mismo bioma, y eso incluye islas sueltas y muescas de 4 bloques en
los bordes, no solo hilachas. Con el filtro nuevo el recuento de trozos menores de
64 columnas sube (0,24 % del area frente a 0,12 %), porque el filtro redondea los
bordes a la celda de 4 y esas muescas se cuentan. La comprobacion que decide es la
imagen: antes habia cintas y dedos, ahora no.

### Superficies por bioma

El populator de `ground_cover` solo tenia cinco cubiertas (hierba, arena, grava,
rio y nieve) y las repartia por bioma sin mirar lo que hace vanilla, asi que la
taiga llevaba nieve encima, las tierras malas eran arena amarilla y las cimas
heladas hierba. Ahora siguen las reglas de superficie de vanilla:

| bioma | vanilla | cubierta |
| --- | --- | --- |
| tierras malas (6 variantes) | arena roja sobre bandas de terracota | arena roja sobre arenisca roja |
| taiga de arboles gigantes | podzol con tierra basta y hierba en manchas | podzol, con mezcla por ruido |
| montañas de grava | grava sobre piedra | grava sobre piedra |
| picos pedregosos, costa pedregosa | piedra desnuda | piedra |
| picos helados, picos escarpados | nieve sobre piedra | capa de nieve sobre piedra |
| playa nevada | nieve sobre arena | capa de nieve sobre arena |
| oceano helado, oceano helado profundo, rio helado | superficie congelada | hielo en el nivel del mar |
| taiga (normal) | hierba, sin nieve | hierba (antes: nieve) |

Los ids de los bloques nuevos (`PODZOL`, `COARSE_DIRT`, `RED_SAND`,
`RED_SANDSTONE`, `ICE`) no estan copiados: se derivan con el mismo `state_hash`
que el resto de constantes y `block_states::block_id_constants_match_the_state_hash`
los comprueba contra la funcion, asi que un id mal escrito rompe el test. Tambien
se borro la cubierta provisional de las tierras malas (era arena del desierto
porque "no habia arena roja en `block_id`"), y el test que la fijaba ahora exige
arena roja de verdad.

El podzol no es un manto uniforme: en la taiga de arboles gigantes el populator
mezcla podzol, tierra basta y hierba con ruido de 11 bloques, como la regla de
`podzol` de vanilla, y el hielo solo aparece donde habia agua (la columna queda a
62 con agua encima).

Composicion de superficie medida con `biome_histogram -- 24` (48x48 chunks, 3 km,
semilla 12345), los tres bloques mas frecuentes de cada bioma:

```
 12 Ice Plains         snow_layer 93 %, dirt 3 %, gravel 3 %
 30 Snowy Taiga        snow_layer 94 %, dirt 3 %, gravel 3 %
 32 Giant Tree Taiga   podzol 60 %, grass 35 %, coarse_dirt 3 %
 46 Frozen Ocean       gravel 78 %, dirt 17 %, sand 4 %
 11 Frozen River       dirt 100 %
183 Frozen Peaks       snow_layer 99 %, stone 1 %
184 Snowy Slopes       snow_layer 100 %
185 Grove              snow_layer 100 %
 37 Badlands           red_sand 95 %, stone 5 %
  2 Desert             sand 83 %, gravel 10 %, dirt 7 %
  5 Taiga              grass 95 %, gravel 4 %, dirt 2 %
```

En los oceanos templados el hielo no aparece (0 %) y el agua sigue por encima del
90 % en `Ocean`; en el oceano helado no queda una sola columna de agua abierta:
el 45 % de las columnas con agua salen con hielo y el resto del bioma son
columnas secas de grava.

Las tierras malas y el desierto se miden y se pintan aparte, en la isla del
noreste del mapa de 768: `map_render 0 384 docs/maps/world-seed0-badlands.ppm 64
160` da 22,7 % de arena roja, 16,8 % de arena y 40,4 % de agua, y
`map_render 0 512 docs/maps/world-seed0-frozen.ppm 640 -1024` mide el oceano
helado (25,0 % de la superficie es hielo). `map_render` acepta origen opcional
para poder mirar una region concreta.

El terreno no se toca: 25,4 % de agua, p50 +6, p90 +19, max 54 y 2,83 % de
escalones secos de dos bloques, exactamente las mismas cifras que antes de este
cambio.

### Los muros de dos bloques: banda de compresion y suelo duro### Los muros de dos bloques: banda de compresion y suelo duro

El jugador seguia encontrando "muros de 2 de altura" en mitad de una llanura. No
venian del ruido sino de dos reglas de la capa de columna:

1. **Banda de compresion de orilla.** Los 10 bloques por encima del mar se
   comprimian al 45 % para que la costa no fuera un acantilado: eso convertia diez
   bloques de desnivel en cuatro o cinco, o sea una repisa plana, y en su borde
   aparecia un escalon de dos bloques con llanura a los dos lados. Con el terreno
   ya suave esa compresion no hace falta: se borro (`SHORE_BAND` y
   `SHORE_COMPRESS` ya no existen).
2. **Suelo duro en el nivel del mar.** Las columnas secas que quedaban por debajo
   del mar se subian de golpe a `sea`, y el resultado era una meseta exactamente a
   y=63 cuya frontera era el muro. Ahora se levantan con una rama cuadratica
   (`SHORE_FLOOR_SOFT = 4`): una columna cuatro bloques por debajo acaba uno por
   debajo, asi que el borde es una cuesta.

Y los rios: sus bancos eran el resto. Ahora son mas anchos y mucho menos hondos
(`RIVER_HALF_WIDTH = 0.34`, `RIVER_DEPTH = 2`) y solo se excavan a menos de seis
bloques sobre el mar (`RIVER_MAX_HEIGHT = 6`, `RIVER_CAP_FADE = 8`), con lo que un
rio cruza la llanura pero no corta una loma.

Medido con `relief_render` en 384x384 (semilla 0), terreno seco solamente:

| | antes | despues |
| --- | --- | --- |
| escalones de 2 bloques | 2,79 % | 2,19 % |
| escalones de 3+ bloques | 0,73 % | 0,71 % |
| columnas en aguja | 10 | 2 |

Con los rios apagados ese mismo mapa da 1,53 % y 0,56 %: los bancos de rio siguen
siendo la mitad de lo que queda. En 768x768 con la configuracion final: 2,90 % de
escalones de 2 y 1,19 % de 3+, 28,1 % de agua.

Lo que no se toco, y conviene saber: el spline se evalua con Hermite y todos sus
puntos llevan `der = 0`, asi que cada tramo es plano en los nudos y hasta 1,5
veces mas empinado en medio. Probar a interpolarlo lineal quitaba parte de los
escalones pero rompia el test de paridad de biomas (`niduy_parity_...`), porque el
`depth` del clima depende de la altura; se dejo como estaba.

### La arena de la orilla la pone el bioma, no una regla de proximidad

Durante un tiempo la capa seca a dos bloques del nivel del mar se forzaba a arena
en cualquier columna que tocara agua. Se veia en el juego: cada charca y cada
ensenada salia con un anillo de arena, y en Minecraft no es asi. La arena de
costa la pone el bioma `BEACH` (y `SNOWY_BEACH`), que ya trae `SANDY_COVER` en
`biome.rs`, igual que en vanilla. Borrada la regla, la hierba llega al agua en los
biomas templados y la arena aparece solo donde el mapa de biomas dice playa.

### Agujas de un bloque, escalones y bancos de rio

El jugador lo describio como "demasiadas mini-montanas": columnas de un bloque de
ancho que sobresalen dos o tres bloques, y escalones duros en las laderas. Habia
dos causas, las dos medibles.

1. **Ruido 3D con demasiada amplitud para su longitud de onda.** `depth_noise`
   iba de `DEPTH_NOISE_BLOCKS = 9` bloques con octavas de 16, 8 y 4 bloques: la
   octava de 4 bloques movia la superficie +-9 bloques, o sea un gradiente de mas
   de dos bloques por bloque. Ahora son 4 bloques de amplitud en octavas de 32 y
   16 bloques.
2. **Cortes duros en las reglas.** El rio se activaba con un umbral
   (`river > 0.5` con el bioma de rio como llave) y se apagaba con otro
   (`height > sea + 14`). En el bloque donde cruzaba el umbral, el cauce pasaba de
   nada a ocho bloques de hondura: una pared de un bloque de ancho. Ahora las dos
   condiciones son rampas (`RIVER_CENTRE` con smoothstep y `RIVER_CAP_FADE = 20`
   bloques), y el rio es mas ancho y menos hondo (`RIVER_HALF_WIDTH = 0.26`,
   `RIVER_DEPTH = 4`), asi que sus bordes son bancos.

`--example relief_render` imprime ahora estas metricas, sobre 256x256 columnas
(semilla 0), con los troncos excluidos de la altura:

| configuracion | escalones 3+ | escalones de 2 |
| --- | --- | --- |
| ruido de 9 bloques en octavas de 16/8/4, cortes duros (medido con arboles dentro) | 4,72 % | 4,74 % |
| final (4 bloques en 32/16, rampas en el rio, rio ancho) | **2,20 %** | 3,83 % |
| final con los rios apagados | 1,73 % | 2,84 % |

El 1,73 % es el suelo del terreno: laderas con acantilados de bloques, que es lo
normal en un mundo de bloques y viene del relieve, no de una regla. Casi todo lo
que queda de 2,20 a 1,73 son las orillas de los rios.

Ojo con la primera cifra: la tabla de "antes" se midio antes de excluir los
troncos, y las 101 "agujas" que salieron entonces eran troncos de arbol, no
terreno. Con los troncos fuera, el terreno por si solo casi no tiene columnas
aisladas: 7 en 65 536. En el mapa de relieve los arboles no se ven, pero en el
juego una ladera con arboles sigue mostrando "picos" que son copas.

En 768x768 con la configuracion final: 61 columnas en aguja (0,01 %), 4,1 % de
escalones de 3+ y 26,3 % de agua.

### Costas organicas y escala de bioma

Tres cosas que el mapa cenital dejo a la vista:

1. **Orillas cuadradas.** El mar se decidia con el bioma (`is_water_biome`), y los
   biomas se asignan por celdas de 4x4 bloques con voto de mayoria: la frontera
   agua/tierra solo podia correr por bordes de celda, de ahi los angulos rectos y
   los escalones de cuatro bloques en la costa. Ahora el mar es donde el spline de
   clima pone el suelo por debajo del nivel del mar (`spline_height`), que es un
   campo suave: la orilla es una curva, sin cuadricula.
2. **Terraza de un bloque.** El ruido 3D de densidad se apagaba en una banda de
   `SHORE_SMOOTH = 12` bloques alrededor del mar, y con eso el terreno costero era
   un campo liso cuantizado a bloques enteros: mesetas anchas con escalones. El
   apagado se aplica ahora solo del lado del agua (el fondo marino sigue liso); en
   tierra el ruido va a plena fuerza.
   Las columnas secas que quedan por debajo del nivel del mar se suben a `sea`,
   para que el ruido no excave un hoyo seco en medio del agua.
3. **Escala del mundo.** `CLIMATE_SCALE` estaba en 0,25: los climas se muestreaban
   a un cuarto de la coordenada, o sea que cada bioma y cada masa de tierra salia
   cuatro veces mas grande que con el muestreo 1:1. Ahora vale 0,6, unas 2,4 veces
   mas pequeño que antes.

Mapa cenital de 768x768 bloques, semilla 0 (`cargo run --release -p
bedrock-world --example map_render -- 0 768 docs/maps/world-seed0-768.ppm`,
PNG en `docs/maps/world-seed0-768.png`):

| | |
| --- | --- |
| agua | 24,6 % en 267 masas (mayor 51 822 bloques) |
| masas de agua < 256 bloques | 236 (1,6 % del area) |
| tierra | 118 componentes (mayor 438 466 = 74 % del area) |
| alturas | p10 62, mediana 67, p90 81, max 117 (nivel del mar 63) |

El mismo mundo en mapa de relieve (`--example relief_render`, color hipsometrico
mas sombreado desde el noroeste, `docs/maps/world-relief-seed0-768.png`) da, sobre
768x768 columnas, alturas respecto del nivel del mar: minimo -38, p10 -17,
mediana **+5**, p90 +19, maximo +54, y 24,7 % de agua. La mediana de +5 dice lo
que la captura del jugador mostraba: la mayor parte del terreno se queda a menos
de diez bloques del agua, y el relieve grande son zonas aisladas.

Lo que sigue sin resolverse: quedan ~236 lagunas de pocos bloques por mapa de
768x768. Son hoyos del propio spline, no charcos del ruido: el mar se decide por
spline y esas hondonadas ya estan por debajo del nivel del mar. Quitarlas pide que
el spline no baje tanto en tierra adentro (`LAND_BIAS`), y eso tambien encoge el
mar.

### Rendimiento

`generate_chunk_internal` compone un chunk a partir de la ventana 3x3 de chunks
base, y hasta ahora generaba los nueve cada vez: al generar una region, cada
chunk base se recalculaba ocho veces. Ahora hay una cache FIFO de chunks base
(`BASE_CACHE_CAP = 128`) ademas de la de chunks completos, con una sola
`ChunkCache` que comparten las dos y sin mantener el lock durante la generacion.

`cargo run --release -p bedrock-world --example bench_chunks`, semilla 12345:

| | antes | despues |
| --- | --- | --- |
| chunk completo (`generate_chunk_internal`) | 62,34 ms | 14,4-16,5 ms |
| via `create_generator("bds")` | 30,65 ms | 14,2-15,0 ms |
| 100 llamadas `block_at` en chunks nuevos | 173,75 ms | 126,12 ms |

La generacion de un chunk base aislado no cambio de codigo; la primera medida
dio 7,40 ms y las repeticiones 3,65-4,14 ms, asi que ese numero es ruido de
medicion y no una mejora.

### Biomas grandes, sin lagos sueltos y con rios

Tres cosas estaban mal y las tres se arreglaron en `bds_terrain.rs`:

1. **El clasificador de bioma ignoraba la escala.** `biome_from_climate` volvia a
   muestrear los climas en las coordenadas crudas, asi que `CLIMATE_SCALE` solo
   afectaba al terreno. Por eso bajar la escala no cambiaba el tamaño de los
   biomas en las mediciones. Ahora clasifica en el punto de clima (deformado y
   escalado) cuando es el sampler del mundo, y en crudo solo para
   `biome_at_bds`.
2. **El `depth` del clima se tomaba en y=0**, o sea "bajo tierra" para todo el
   mapa: el arbol respondia biomas de cueva. Lush caves llegaba a ser el segundo
   bioma mas comun de la superficie (15 % de las celdas). Ahora se toma en la
   altura de la columna, y el sampler de capturas conserva el y=0 para seguir
   comparando con BDS.
3. **Se inundaba cualquier hondonada.** Con el ruido de relieve el terreno tiene
   muchos hoyos someros, y todos acababan como lagos. Ahora el mar sale del
   spline de clima (ver la seccion siguiente), no del bioma.

Ademas hay rios: un ruido propio de baja frecuencia cuyos cruces por cero se
excavan hasta cinco bloques por debajo del nivel del mar (`RIVER_DEPTH = 5`),
lo que da cauces serpenteantes de agua de unos ocho bloques de ancho
(`RIVER_HALF_WIDTH = 0.14`). Solo se excavan en biomas de rio y con el ruido en
el centro de su banda (`RIVER_CENTRE = 0.5`), y nunca por encima de
`sea + RIVER_MAX_HEIGHT`: sin esas tres condiciones el ruido dibujaba un canal
cada ~50 bloques en todo el mapa.

Medido en 768x768 bloques (48x48 chunks) con semilla 0, celdas cuarto:

| | antes | despues |
| --- | --- | --- |
| regiones de bioma | 331-349 | **39** |
| tamaño medio de region | ~105 celdas (~20x20 bloques) | **945 celdas (~100x100 bloques)** |
| mediana | 17-21 celdas | 108 celdas |
| agua en bioma no oceanico | 7,6 % de las columnas | 6,8 % (rios y pantanos) |

El test `niduy_parity_climate_noise_and_biomes_seed_12345` sigue comprobando los
65 valores BDS del sampler de capturas y ademas ahora verifica que el sampler del
mundo nunca devuelve un bioma de cueva y que las 16 columnas de una celda cuarto
comparten bioma.

### Reparto de biomas: el desierto que no existia

Con el terreno y los bordes ya arreglados quedaba el reparto. Medido con
`biome_histogram -- 24` (48x48 chunks, 3 km, semilla 12345) el mundo era un
bosque: Forest 16,0 %, Ice Plains 13,8 %, Plains 9,8 %, Snowy Taiga 6,0 %,
Wooded Badlands Plateau 6,8 %, y **Desert 0,00 %**. No habia desiertos, ni en la
medicion ni en el mapa que vio el jugador.

La causa no era el arbol de biomas sino los dos campos que lo alimentan:

| campo | p10 | mediana | p90 | maximo |
| --- | --- | --- | --- | --- |
| temperatura | -0,58 | **-0,15** | 0,22 | 0,53 |
| humedad | -0,29 | 0,02 | 0,31 | 0,42 |

El arbol pide `temperature > 0.55` para el desierto y nuestro campo no pasaba de
0,53: la rama del desierto era inalcanzable por construccion. Ademas el sesgo de
-0,15 empujaba todo el mundo hacia lo templado-frio.

Ahora, solo en el muestreo del mundo (`biome_at_bds` conserva los valores crudos
para que el test de paridad siga comparando con BDS), la temperatura y la humedad
se llevan a las bandas de vanilla con un sesgo y una ganancia:

```
temperature = (raw + 0,15) * 1,4      // mediana 0, p90 ~0,52, max 0,95
humidity    = (raw - 0,02) * 1,0      // p10 -0,31, p90 0,31
```

Con eso el arbol ya alcanza el calor, pero salio otro desequilibrio, y de ahi sale
la regla que hay ahora en `fix_biome_elevation`:

- **Un area caliente y seca es arena, y cual de las dos arenas lo decide la
  temperatura.** El arbol daba el 15,6 % del mundo a las tierras malas y 0,02 % al
  desierto, porque su rama de desierto pide ademas el terreno erosionado que su
  rama de tierras malas ya se habia llevado. Asi que una columna seca
  (`humidity < -0,05`) y caliente (`temperature > 0,15`), o que el arbol ya
  llamaba tierras malas, queda como arena; por encima de
  `BADLANDS_MIN_TEMPERATURE = 0,60` es tierras malas (manteniendo la variante que
  eligio el arbol: mesetas, tierras malas erosionadas) y por debajo es desierto.
  La temperatura es un campo suave de cientos de bloques, asi que el limite entre
  las dos es una linea y cada masa es de una sola arena.
- **Sabana = lado humedo de esa banda.** El mismo sesgo calido hacia que el arbol
  diera 0 % de sabana; el lado mas humedo de la banda de tierras malas
  (`humidity > -0,05`, hasta y=110) vuelve a ser sabana.

Las dos primeras versiones de esta regla fallaron y estan aqui para que no se
repitan:

1. **Cortar por altura** (desierto el llano, tierras malas la meseta): como las
   tierras malas son justo las mesetas, cada una salia rodeada de una orla de
   arena amarilla. Es lo que el jugador reporto como "el desierto rojo y el normal
   siempre aparecen juntos".
2. **Cortar por un ruido regional** (512 bloques): los parches calidos y secos
   miden unos cien bloques, aun mas pequenos que la region, asi que el resultado
   era un mosaico de las dos arenas. Con 2 km de longitud de onda el mapa entero
   caia del mismo lado y las tierras malas desaparecian.

Reparto medido en la misma ventana de 3 km (semilla 12345):

| | antes | despues |
| --- | --- | --- |
| desierto | 0,00 % | **6,8 %** |
| tierras malas (6 variantes) | 7,4 % | 5,8 % |
| sabana | 2,4 % | 10,5 % |
| bosque (5 variantes) | 16,0 % | 9,4 % |
| llanura | 9,8 % | 7,5 % |
| nieve (ice plains, taiga nevada, picos, grove...) | 27,1 % | 33,0 % |
| selva | 4,6 % | 6,0 % |
| oceano, rio, playa | 22,1 % | 19,2 % |

En el mapa de 768 la superficie pasa de 4,6 % de arena a **15,4 % de arena y 10,0 %
de arena roja**, en dos o tres masas grandes cada una. El desierto ocupa el llano
(y p50 73) y las tierras malas la meseta (y p50 104), pero cada masa es de una sola
arena y solo comparten la linea de frontera; se ve en
`docs/maps/world-seed0-badlands.png` (22,7 % arena roja, 16,8 % arena).

Lo que sigue sin cuadrar: Ice Plains es el bioma mas grande con 16,3 % del mundo.
En vanilla las llanuras heladas son mas raras y ese frio lo ocupa la taiga nevada;
ahora que la temperatura llega a las bandas de vanilla, es el siguiente reparto
que hay que mirar.

### Biomas: los pone cubiomes

Todo lo anterior era nuestro: un arbol de biomas copiado de BDS y nuestro propio
campo de clima, con reglas a mano para que desiertos, tierras malas y sabanas
salieran en proporcion. Funcionaba a medias — el mismo clima tenia que servir para
el terreno y para el bioma, y cada arreglo de uno rompia el otro.

Ahora el bioma de cada columna de superficie lo decide
**[cubiomes](https://github.com/Cubitect/cubiomes)** (Cubitect, licencia MIT), que
es el generador de biomas de Minecraft de verdad:

- La libreria completa (C y cabeceras) esta copiada en
  `crates/bedrock-world/vendor/cubiomes`, con su licencia en
  `vendor/cubiomes/LICENSE`, y `crates/bedrock-world/build.rs` la compila con `cc`
  dentro de la crate.
- `src/generator/cubiomes.rs` es el envoltorio: `Cubiomes::new(seed)` prepara el
  generador del mundo (version 1.18.2, dimension overworld) y
  `biome_at(x, y, z)` devuelve el id Java del bioma, que se traduce al espacio de
  ids de Bedrock con `bds_biome_tree::java_to_bedrock`.
- `vendor/cubiomes/pm_shim.c` es el unico puente C: evita que Rust tenga que
  conocer la estructura interna de cubiomes y fija alli la version de Minecraft
  que se pide.

El reparto ya no se ajusta a mano: medido con `biome_histogram -- 24` (3 km, semilla
12345) el mundo es Ocean 15,7 %, Plains 22,6 %, Forest 11,3 %, Ice Plains 28,9 %,
Snowy Taiga 11,9 %, Dark Forest 5,2 %, Taiga 0,8 %, y el resto biomas menores. Es
lo que Minecraft responde para esa semilla en esa ventana, no una imitacion.

Lo que sigue siendo nuestro, y por que:

- **El terreno.** La forma del relieve sale de nuestro spline (`DENSITY_BASE`,
  `LAND_BIAS`, rios propios), que es lo que el jugador ya habia revisado. Los
  biomas vienen de cubiomes; el agua se decide por altura final, asi que un bioma
  de tierra sobre agua (o al reves) se reconcilia por altura, igual que antes.
- **Tres guardas**, todas medidas en el mapa de 768:
  1. Un bioma de cueva (lush caves, dripstone, deep dark) nunca queda en la
     superficie.
  2. Un rio de cubiomes sobre tierra seca se sustituye por el bioma de clima: sus
     rios siguen un ruido distinto al nuestro y salian como lineas de tierra de un
     bloque cruzando el mapa.
  3. Costa pedregosa y playa nevada solo valen a dos bloques del agua
     (`SHORE_MAX_HEIGHT = 64`), y los biomas de montaña piden `y >= 92`.

Verificado: `cargo test -q --release -p bedrock-world --lib` da 66/66 y
`cargo check -q -p server-core` no da errores; el mapa
`docs/maps/world-seed0-768.png` esta regenerado con esta version. No esta probado
en el juego.

#### La arena pequena es la de vanilla

El jugador pregunto por que siguen saliendo manchas de arena pequenas. La
comparacion directa lo responde: `--example cubiomes_map` pinta el mapa que
cubiomes da para la misma ventana y la misma semilla, con los mismos colores, y
sale `docs/maps/cubiomes-seed0-768.png` junto a `docs/maps/world-seed0-768.png`.

En la semilla 0, en 768x768 bloques alrededor de 0,0, **cubiomes no tiene ni un
desierto ni unas tierras malas**:

| bioma (cubiomes, semilla 0, 768x768) | % |
| --- | --- |
| savanna | 57,79 |
| forest | 12,04 |
| lukewarm_ocean | 10,17 |
| river | 8,84 |
| plains | 6,15 |
| beach | 4,85 |
| savanna_plateau | 0,15 |
| jungle | 0,01 |

O sea que la unica arena que existe ahi es la **playa**, y una playa es fina por
definicion. Nuestro mapa da 3,1 % de arena en la misma ventana: la misma playa,
siguiendo *nuestras* costas y lagos en vez de los de vanilla.

`--example seed_scan` recorre semillas y mide desierto + tierras malas en un radio
alrededor de 0,0, para elegir un mundo con arena grande:

| semilla | arena (desierto + tierras malas) |
| --- | --- |
| 138 | 85,6 % (35,5 desierto, 50,1 tierras malas) |
| 118 | 84,6 % (74,2 desierto, 10,4 tierras malas) |
| 155 | 76,9 % |
| 48 | 73,5 % |
| 0 | 0,0 % |

Con la semilla 118 el generador da 36,4 % de arena y 5,0 % de arena roja en
768x768 (`docs/maps/world-seed118-768.png` frente a
`docs/maps/cubiomes-seed118-768.png`), y la diferencia con el 52,7 % de desierto de
cubiomes es el agua: nuestros lagos ocupan columnas que en vanilla son desierto.

#### Estan todos los biomas de cubiomes?

Si, para la version que usamos. `--example biome_audit <semillas> <radio>` recorre el
overworld de N semillas y compara cada id que cubiomes devuelve con nuestras tablas
(`java_to_bedrock` y `get_biome_info`). Con 64 semillas y 2 km de radio:

- cubiomes produce **50 ids distintos**;
- **0 faltan** en nuestras tablas: todos tienen nombre y cubierta de superficie.

Las cubiertas salen de la tabla de `biome.rs`, comprobadas una a una en la salida
del audit: oceano y oceanos profundos grava, playa arena, costa pedregosa piedra,
nieve y picos capa de nieve, selvas y bosques hierba, tierras malas arena roja,
taiga de arboles gigantes podzol.

Dos limites, dichos claros:

- Fijamos la version en **1.18.2** (`MC_1_18_2` en `vendor/cubiomes/pm_shim.c`), la
  misma que el generador imita. Biomas posteriores (mangrove swamp 1.19, cherry
  grove 1.20, pale garden 1.21) **no existen** en esta version y por tanto no
  aparecen. Para tenerlos hay que subir la version y, antes, verificar sus ids de
  Bedrock en `BiomeIds.php` de PocketMine: no se inventan.
- cubiomes tambien sabe nombrar ids que el overworld 1.18 no genera nunca (las
  variantes "hills" antiguas, nether, end, the void). Si alguna apareciera, nuestra
  tabla no la conoce y cae en la entrada por defecto (oceano con grava); el audit
  confirma que hoy no aparece ninguna.

### Piedra en las cuestas

Las montanas salian verdes hasta arriba: la cubierta de superficie la decidia
solo el bioma, asi que una pared de tres bloques por bloque llevaba hierba en la
cima igual que una llanura. Vanilla ensena roca en las caras empinadas, y eso es
lo que hace ahora `ground_cover`:

- antes de tocar el chunk se guarda la altura de las 16x16 columnas;
- una columna cuya altura se separe `STONE_SLOPE = 3` bloques de cualquier vecino
  recibe `STONE_COVER` en vez de la cubierta del bioma;
- por encima de `MOUNTAIN_STONE_HEIGHT = 90` basta con `2`, porque nuestro ruido de
  relieve deja escalones de dos bloques por todo el llano y en la montana real un
  escalon de dos bloques ya es roca.

Medido en 384x384 bloques alrededor de (200, 300), semilla 0 (una zona de colinas):

| | piedra en superficie | hierba |
| --- | --- | --- |
| antes | 0,0 % | 68,6 % |
| despues | **4,3 %** | 64,6 % |

En el mapa de 768 completo la piedra pasa de 0,0 % a 1,8 %, y por bioma
(`biome_histogram -- 24`, semilla 12345): Plains 8 %, Forest 4 %, Dark Forest 3 %,
Snowy Taiga 2 %. La piedra aparece como bandas grises en las laderas y crestas
(`docs/maps/world-seed0-768.png`). El umbral general se dejo en 3 y no en 2 porque
con 2 el llano se llenaba de motas de grava y piedra en cada escalon del ruido de
detalle.

### Escaleras en el agua y zonas planas

Tres intentos, cada uno con su defecto, y este es el que se queda:

1. sin tocar el fondo, un lago era un cuenco de escalones de un bloque: el campo
   de altura es liso y cuantizado a bloques, y las curvas de nivel salen como
   anillos paralelos;
2. subiendo el fondo hacia la linea de agua (`deficit^2 / 16`) desaparecian las
   escaleras y aparecian **planicies**: la subida saturaba (todo lo mas profundo
   de 10 bloques acababa a 4), y ademas dejaba agua de un bloque con el suelo a la
   vista y bordes rectos donde el terreno llano cruzaba el nivel del mar;
3. lo que hay ahora no satura y no drena:

- **compresion suave del fondo**: `highest_solid += round((profundidad - 2) * 0.35)`.
  Es un escalado, no una subida con tope: el orden de profundidades se conserva,
  asi que una fosa sigue siendo una fosa y no puede salir una planicie. Una
  pendiente de 0,8 bloques por bloque pasa a 0,5, o sea curvas de nivel casi el
  doble de separadas;
- **sin agua de un bloque**: la columna que ya estaba a un bloque bajo el nivel
  del mar se cava uno mas. El agua conserva su forma y el banco es un escalon;
- **el ruido de profundidad ya no se apaga en la orilla**: la caida `SHORE_SMOOTH`
  se elimino (era la que dejaba el borde liso), gana dos octavas de 8 y 4 bloques
  y `DEPTH_NOISE_VERTICAL` pasa de 1,5 a 3,0.

Medido con `--example relief_render -- 0 384` (reparto del salto entre columnas
vecinas; "3+" son las paredes duras):

| | mismo | 1 bloque | 2 bloques | 3+ bloques |
| --- | --- | --- | --- | --- |
| sin compresion | 73,3 % | 20,7 % | 3,10 % | **2,96 %** |
| ahora | 73,9 % | 20,6 % | 4,11 % | **1,39 %** |

Y con `--example terrain_ascii` sobre chunks generados (semilla 0): agua de un
bloque **0** columnas en todas las ventanas probadas, cobertura de agua 25,3 %
igual que antes de todo esto, pico en 115 y los mismos 69 cuerpos de agua (no se
fragmenta ni se drena nada).

Un detalle que costo una vuelta: el agua se decide con `wet`, y `wet` se
calculaba **antes** de mover el fondo (repisa, regla de un bloque, escalon a nivel
del mar). Una columna que bajaba de 62 a 61 quedaba marcada como seca y se
generaba un hoyo de tierra un bloque bajo el agua, sin agua: eso era lo que el
jugador encontro en 632, 61, -84 (semilla 0). Ahora `wet` se calcula al final,
sobre la altura definitiva del fondo.

### Pendiente

Con las escaleras ya rotas en parte, lo que quedaba era la pendiente: nuestras
laderas suben mas que las de vanilla. Tres numeros la bajan sin cambiar el
caracter del mundo:

- el spline se escalaba `x2.0`; ahora `x1.6`;
- `RELIEF_BLOCKS` 26 -> 16 (amplitud del ruido de relieve a gran escala);
- `LAND_BIAS` 0.2 -> 0.16, porque bajar la ganancia deja la costa mas alta y el
  mar se encogia: con 0,16 la cobertura de agua vuelve al 24,9 % (antes 25,3 %).

Medido en el mapa de 768 (`map_render`): p90 de altura 82 -> 78, maximo 117 ->
103, agua 24,9 %. Y con `relief_render` en 384x384, el reparto de saltos entre
columnas vecinas pasa de 2,96 % a **2,39 %** de saltos de 3+ bloques: menos
paredes y mas pendiente repartida.

Orden de las reglas del fondo, que importa: primero el escalon a nivel del mar
(62 -> 61/63), despues la regla de "nada de agua de un bloque" (61 -> 60). Al
reves quedaban columnas de agua de un bloque justo donde el escalon bajaba.

### Profundidad cerca de la orilla

El agua se hacia honda demasiado rapido. Tres causas, tres cambios:

- `SEA_FLOOR_RAMP` 6 -> 14: la profundizacion del fondo abierto se reparte sobre
  catorce bloques en vez de seis, asi que la orilla no cae a pico;
- `DEPTH_NOISE_BLOCKS` 8 -> 5: el ruido que mueve el cruce de densidad era tan
  grande que columnas vecinas se saltaban cuatro bloques de golpe;
- la regla "nada de agua de un bloque" **se quito**: era justo lo que convertia
  la primera columna de orilla en dos o tres bloques de agua. Ahora la orilla
  lleva una franja de un bloque de profundidad, y como el escalon a nivel del mar
  reparte las columnas planas entre 61 y 63, esa franja queda festoneada en vez
  de ser una lamina plana.

Perfil real (semilla 0, z = -95, x = 630..690): `61 61 ... 60 60 60 59 59 57 57 ...
57 56 ... 56 54 ...` — de un bloque bajo el mar a ocho en sesenta bloques, con
escalones de uno o dos, no un pozo.

Coste honesto: vuelve a haber agua de un bloque (1204 columnas en una ventana de
96x48), pero como orilla festoneada y no como sabana; la cobertura total de agua
queda en 24,3 % y no se fragmenta (192 cuerpos, 162 por debajo de 256 bloques,
igual que antes).

## Decoradores submarinos, costra de las cuevas y costuras de bioma

### Hierba marina, algas y pepinillos de mar

No habia **ningun** decorador debajo del agua: los oceanos eran arena, grava y
agua. Las reglas que sigue el generador son las de vanilla (Minecraft Wiki:
*Seagrass*, *Kelp*, *Sea pickle*; verificado en la wiki el 2026-11-17):

| bloque | biomas | forma |
| --- | --- | --- |
| hierba marina (`seagrass`, `sea_grass_type=default`) | todos los oceanos, rios, rios helados, pantanos y manglares | una planta sobre el fondo |
| hierba marina alta (`double_bot` + `double_top`) | los mismos, con 3+ bloques de agua | dos bloques, siempre emparejados |
| alga (`kelp`, `kelp_age` 0..25) | todos los oceanos menos el calido y los helados | ~1 trozo por cada 18 chunks; la columna sube hasta un bloque bajo la superficie y la punta es `kelp_age=25` |
| pepinillo de mar (`sea_pickle`, `cluster_count` 1..3) | solo oceano calido (arrecifes) | 1/8 de los chunks, sobre arena o grava |

Ids: no estan copiados a mano. Salen de `data/canonical_block_states.nbt` con
`block_states::with_property` (`examples/block_probe.rs` imprime los 26 estados de
`kelp_age` y los tres `sea_grass_type`); si el cliente resuelve un id que no es el
de su estado canonico, rechaza el chunk. Los cuatro bloques y sus estados entran
en `is_transparent`/`is_solid` como plantas, si no la cobertura los trataba como
roca.

Densidades: por columna 0,10 en mar abierto y 0,22 en rios, rios helados,
pantanos y manglares. Donde el fondo no es arena, grava, tierra, arcilla,
arenisca o hierba no se planta nada, y con menos de dos bloques de agua tampoco.
Los oceanos helados quedan fuera (van tapados por el hielo); el rio helado no.

Medido con `examples/underwater_stats.rs` (semilla 0, chunks ya poblados):

- 144 chunks desde (-40, -40): 1615 de hierba marina, 354 altas, 49 algas, 0
  pepinillos (por ahi no hay oceano calido);
- 100 chunks desde (35, -8), la zona de la captura: 624, 160, 58 y 0;
- los tres tests nuevos (`underwater.rs`) cubren pares de hierba alta, la punta
  madura del alga y que el pepinillo solo salga en agua calida.

### El agua se comia las plantas acuaticas

Las plantas acuaticas no son un bloque seco encima del agua: en Bedrock ocupan el
bloque de agua en el que se generan (`seagrass`, `kelp` y `sea_pickle` no tienen
propiedad `waterlogged`, el bloque *es* la celda de agua). El motor de fluidos
(`server-core/src/fluid.rs`) las trataba con `physics::is_replaceable`, que
incluye `seagrass` y `kelp`: cualquier actualizacion de agua al lado las
sobrescribia con agua corriente y la planta desaparecia, y para el flujo la celda
contaba como seca, asi que el agua de al lado se adelgazaba y drenaba.

Ahora `can_be_flowed_into` devuelve `false` para las plantas acuaticas y
`effective_flow_decay`/`smallest_flow_decay` las cuentan como agua a nivel de
fuente. Tres tests lo fijan: `water_never_flows_over_a_water_plant`,
`a_water_plant_keeps_the_flow_next_to_it_from_drying_up` y la regresion
`breaking_the_water_next_to_a_plant_neither_erases_it_nor_the_sea` (cuenca con
plantas, romper el agua de al lado, 400 ticks: las plantas siguen y no queda aire
donde habia agua).

Medido con `server-core/examples/sea_air_scan.rs`, que genera un fondo marino de
verdad, rompe el bloque de agua junto a una planta y tickea 400 veces:

| | plantas antes | plantas despues |
| --- | --- | --- |
| sin el arreglo | 29 | 27 |
| con el arreglo | 29 | 29 |

Y en el mundo guardado del servidor (`world.json`, que es generacion + lista de
ediciones) aparecen ediciones de `flowing_water` escritas justo en las celdas de
hierba marina, que es el paso intermedio: planta -> agua corriente -> aire al
drenar. `examples/edit_audit.rs` audita ese archivo contra lo que genera el
generador y encuentra las ediciones de daño (aire o `flowing_water` sobre agua o
sobre una planta); con `--clean` las quita dejando `world.json.bak`.

### La tierra que parecia flotar en (644, 62, -44)

Comprobado con `examples/column_dump.rs`, que vuelca columnas enteras con nombre
y mide el espesor de terreno bajo la superficie (ignorando hojas, hierba y agua,
que si no cuentan como huecos). Columnas reales, semilla 0:

```
(644, -44):  air 63-127 | water 60-62 | dirt 57-59 | stone 1-56 | bedrock 0
(643, -44):  air 63-127 | water 60-62 | dirt 57-59 | stone 1-56 | bedrock 0
(649, -44):  air 63-127 | water 62    | dirt 59-61 | stone ...   | bedrock 0
(650, -44):  air 64-127 | grass 63    | dirt 59-62 | stone ...   | bedrock 0
```

En una ventana de 72x72 bloques centrada ahi (5184 columnas) el **espesor minimo
de terreno bajo la superficie es de 21 bloques** y **ninguna** columna tiene
costra de dos bloques o menos; lo mismo en las 340 columnas alrededor exacto del
punto. El unico aire bajo la superficie son cuevas a y = 28..42, quince o mas
bloques por debajo del fondo marino, coherente con `CavePopulator::max_y = 44`.

Conclusion: no hay terreno flotante ni costra fina. Lo que se ve en la captura es
una orilla escalonada: tierra a 63, un rellano a 61-62 con un solo bloque de agua
encima y agua de tres al lado. La hipotesis de la cueva rozando el fondo marino
era **falsa**; se probo una costra de cuatro bloques en `CavePopulator::carve` y
se quito, porque ahi no se activaba nunca. Nada del generador escribe bloques en
el aire: el terreno es una columna maciza hasta la roca base y los populadores
solo escriben sobre el fondo.

### Costura de bioma

`ground_cover` cogia el bioma de cada columna y aplicaba su cobertura tal cual, de
modo que donde la tabla de biomas cambia de golpe la nieve y la hierba se tocaban
en linea recta (676, 71, -69 y 696, 92, 27 en las capturas). Ahora, para las
columnas que tienen un vecino de otro bioma, un ruido de 2 bloques decide al 50 %
si la columna usa la cobertura del vecino (`dither_biome_borders`). La costura
queda festoneada en dos o tres bloques en vez de dibujada a regla. No cambia
biomas, alturas ni el guardado: solo que bloque de cobertura pone encima.

### Lo que no esta comprobado

- Nada de esto se ha visto en el cliente: los chunks ya guardados no se
  regeneran, asi que hay que borrar la region (o usar una semilla nueva) para
  verlo. Los conteos salen del generador, no de mirar el mundo.
- El dither suaviza la cobertura (nieve, hierba, arena), no la altura: un escape
  de roca entre dos biomas sigue siendo un escalon del terreno.
- El alga no se ancla a la superficie ni se rompe si se quita el agua de debajo:
  la fisica de bloques no forma parte de este crate.
- Los arrecifes de coral (bloques, abanicos) siguen sin generarse; el pepinillo
  sale igual porque vanilla tambien lo pone fuera de ellos.
