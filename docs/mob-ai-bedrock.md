# IA de animales en Bedrock — comportamiento observado y estado del port

Notas de investigación sobre **cómo se comportan** los animales pasivos de
Bedrock y qué de ese comportamiento implementa este servidor. Complementa
`crates/server-core/src/entity.rs` (`mob_tick`) y
`crates/server-core/src/entity_types.rs`.

## 0. Procedencia y licencias

Este documento es una **descripción de comportamiento escrita desde cero**. No
contiene ni reproduce archivos de behavior pack de Mojang: no hay JSON de
entidades, ni tablas de metas, ni listas de prioridades copiadas. Los archivos
de `Mojang/bedrock-samples` son material de Mojang («All rights reserved») y
están sujetos al EULA de Minecraft, que prohíbe redistribuir contenido del
juego; por eso aquí solo se describe el comportamiento en palabras propias y se
enlaza a documentación pública.

Las cifras que aparecen (radios, tiempos, velocidades relativas) son
**aproximaciones funcionales** tomadas de documentación pública y de la
observación del juego; varían entre versiones y no pretenden ser una copia de
los valores internos del juego. Fuentes en la sección 7.

## 1. Cómo se organiza la IA en Bedrock, en general

Un animal de Bedrock no tiene un único cerebro: su comportamiento está descrito
como una colección de **metas** pequeñas (vagar, mirar, huir, seguir comida,
reproducirse, pastar…) y un planificador elige cada tick la meta más prioritaria
que pueda ejecutarse. Las metas de movimiento y las de mirada compiten por el
control del cuerpo, y las de mayor urgencia (por ejemplo, flotar o huir)
interrumpen a las de paseo. El desplazamiento real lo resuelve la navegación por
bloques de la entidad, que puede rodear obstáculos, evitar agua o daño y saltar
un bloque de altura.

No hace falta reproducir esa arquitectura para tener animales creíbles: lo
importante es el **repertorio de conductas** y cuándo se disparan.

## 2. Repertorio típico de un animal pasivo

- **Reposo y deambulación.** Pasa ratos quieto, mirando alrededor, y cada
  cierto tiempo elige un punto cercano y camina hacia él. Los recorridos son
  cortos (del orden de una decena de bloques), no travesías largas.
- **Curiosidad por el jugador.** De vez en cuando gira la cabeza (y a veces el
  cuerpo) hacia un jugador cercano, a unos 6-8 bloques, con probabilidad baja
  (del orden del 2 % por comprobación). No lo persigue: solo lo mira un
  momento.
- **Huida al daño.** Si lo golpean, entra en pánico unos segundos y corre
  alejándose del punto del golpe, más rápido que su paso normal. El conejo es
  el caso extremo: huye muy rápido y también de lobos y monstruos aunque no le
  hayan pegado.
- **Tentación por comida.** Si el jugador sostiene el alimento adecuado,
  cambia el paseo por seguir al jugador hasta cierta distancia (se detiene a
  un par de bloques), mirándolo. Al dejar de sostenerlo, vuelve a su rutina.
- **Cría.** Dos adultos alimentados con su comida entran en modo de cría, se
  buscan y producen una cría; la cría sigue a uno de sus progenitores y crece
  con el tiempo. Las crías son más pequeñas y, en varias especies, tienen
  variantes de color heredadas.
- **Flotar.** En el agua, cualquier animal sube a la superficie en lugar de
  hundirse; es la conducta de mayor urgencia.
- **Salto de obstáculo.** Al toparse con un bloque de un altura, salta para
  superarlo; si el obstáculo es más alto, se detiene o cambia de rumbo.
- **Despawneo por distancia.** Los animales sin nombre ni correa desaparecen
  cuando el jugador se aleja lo suficiente (del orden de un centenar de
  bloques), para no llenar el mundo de entidades.
- **Pastoreo (oveja).** Come hierba del suelo y la convierte en tierra; si
  estaba esquilada, recupera la lana con ello.
- **Conductas especiales por especie.** El cerdo y el caballo se dejan montar
  y se controlan con su objeto; la gallina aletea al caer y pone huevos; el
  conejo salta en vez de caminar y come cultivos de zanahoria; el lobo y el
  gato domésticos siguen y defienden al jugador.

## 3. Comportamiento por animal (resumen cualitativo)

- **Vaca.** Pasta y deambula sin rumbo; sigue al jugador que lleva trigo; se
  deja ordeñar con un cubo; cría con trigo; las crías siguen a la madre; huye
  si la golpean.
- **Cerdo.** Igual de tranquilo; sigue zanahoria, patata o remolacha (y la
  caña con zanahoria); con silla se puede montar y dirigir con la caña; cría
  con esos mismos alimentos.
- **Oveja.** Además de lo común, come hierba (hierba alta incluida) y
  recupera lana; se esquila con tijeras; cría con trigo.
- **Gallina.** Deambula picoteando; es asustadiza y corre rápido al recibir
  daño; sigue semillas; aletea para no hacerse daño al caer; pone huevos.
- **Conejo.** Se desplaza a saltos; es el más nervioso: huye de jugadores,
  lobos y monstruos cercanos, y busca y come cultivos de zanahoria.
- **Caballo, burro, mula, llama.** Pastan y deambulan; se domestican y montan;
  la llama escupe si se la molesta y puede llevar carga. Fuera del alcance
  actual del port.

## 4. Lo que implementa este port

`Entity::mob_tick` (`crates/server-core/src/entity.rs`) implementa una versión
mínima y determinista de ese repertorio:

- **Deambulación.** Cada 20-80 ticks elige una dirección nueva con un generador
  pseudoaleatorio sembrado con el id y los ticks vividos de la entidad, y la
  mantiene mientras esté en el suelo y no esté herida. Camina a una velocidad
  fija moderada (0.09).
- **Salto.** Si en el tick hubo colisión horizontal (el movimiento quedó a
  cero), aplica un impulso vertical (0.42) para subir un bloque.
- **Orientación.** La dirección de paseo se trata como un yaw de Bedrock y el
  cuerpo se gira hacia ella, de modo que el animal mira hacia donde camina.
- **Mirar al jugador.** Con una probabilidad baja por tick, un animal sano
  que tiene a un jugador a menos de 8 bloques gira **solo la cabeza** hacia
  el más cercano, mantiene la mirada unos 2 segundos y después la devuelve al
  rumbo del cuerpo. El cuerpo sigue deambulando mientras tanto.
- **Herido.** Mientras dura el empuje del ataque no se pisa el movimiento con
  el paseo; al terminar, retoma la deambulación.
- **Muerte.** Animación de muerte, loot y experiencia, con el mismo
  `Living::onDeath()` que el resto de mobs.
- **Red.** El movimiento se difunde con el paquete de movimiento de actores,
  incluyendo rotación; también se envía cuando solo cambia la orientación (un
  animal bloqueado contra una pared que gira sin desplazarse). El paquete
  lleva además la marca de «en el suelo»: sin ella el cliente cree que el
  animal está cayendo todo el rato y, por ejemplo, la gallina aletea sin
  parar aunque esté quieta en tierra firme.

Lo que **no** hay todavía, y su consecuencia visible:

| Falta | Efecto |
|---|---|
| Navegación por bloques / pathfinding | caminan en línea recta y saltan al chocar; no rodean obstáculos ni evitan agua o cactus |
| Mirar alrededor sin jugadores | entre miradas al jugador no giran la cabeza de forma ociosa |
| Pánico y huida | al recibir daño no huyen; siguen paseando |
| Tentación por comida | no siguen al jugador que lleva trigo, semillas o zanahoria |
| Cría y crecimiento | no hay crías, ni escala, ni seguimiento de la madre |
| Pastoreo y cultivos | la oveja no come hierba y el conejo no come zanahorias |
| Despawneo por distancia | los animales no desaparecen al alejarse |
| Monturas | el cerdo y el caballo no se pueden montar |

## 5. Bug encontrado durante la investigación

Los animales **no giraban el cuerpo hacia donde caminaban**:

1. `mob_tick` fijaba el movimiento como `(cos θ, sin θ)` pero el yaw como `θ`.
   En Bedrock la dirección de vista para un yaw dado es `(-sin yaw, cos yaw)`,
   así que el cuerpo quedaba girado 90° respecto al desplazamiento: el animal
   se movía de lado conservando el rumbo con el que apareció.
2. La sincronización solo enviaba movimiento cuando cambiaba la posición; un
   animal bloqueado que cambiaba de rumbo sin moverse no recibía paquete de
   rotación.

Correcciones: el paseo usa `(-sin, cos)` y el envío compara también la
rotación. La regresión está cubierta por
`world_state::tests::a_walking_mob_faces_where_it_walks`.

La mirada al jugador se sincroniza en el mismo paquete de movimiento: el
`head_yaw` del actor viaja separado del `yaw` del cuerpo, y el servidor reenvía
el paquete cuando cambia cualquiera de los dos
(`session::mob_look_sync_tests::a_mob_looking_at_a_player_sends_its_head_yaw`).

## 6. Ruta sugerida hacia un comportamiento más fiel

1. ~~Mirar al jugador~~ (hecho) y añadir **mirar alrededor** cuando no hay
   jugadores cerca.
2. **Pánico y huida** al recibir daño, reutilizando el paseo actual.
3. **Tentación** por el alimento que lleva el jugador y **seguimiento de
   crías**.
4. **Pastoreo** (oveja) y **consumo de cultivos** (conejo): ya existe edición
   de bloques.
5. **Cría y crecimiento**: escala de crías, herencia de variantes y
   recompensa de experiencia.
6. **Navegación por bloques** (A* sobre la rejilla) y **despawneo por
   distancia**.

## 7. Fuentes

Comportamiento general y por especie (documentación comunitaria):

- Minecraft Wiki — Animal: <https://minecraft.wiki/w/Animal>
- Minecraft Wiki — Cría: <https://minecraft.wiki/w/Breeding>
- Minecraft Wiki — Vaca, Cerdo, Oveja, Gallina, Conejo:
  <https://minecraft.wiki/w/Cow> · <https://minecraft.wiki/w/Pig> ·
  <https://minecraft.wiki/w/Sheep> · <https://minecraft.wiki/w/Chicken> ·
  <https://minecraft.wiki/w/Rabbit>

Documentación oficial de creadores (Microsoft Learn) y referencia de
componentes:

- Introducción al comportamiento de entidades:
  <https://learn.microsoft.com/en-us/minecraft/creator/documents/entitybehaviorintroduction>
- Referencia de metas (mirar al jugador, pasear, tentar, comer, evitar mobs):
  <https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitygoals/minecraftbehavior_look_at_player> ·
  <https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitygoals/minecraftbehavior_random_stroll> ·
  <https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitygoals/minecraftbehavior_tempt> ·
  <https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitygoals/minecraftbehavior_eat_block>
- Referencia general de entidades de la comunidad: <https://bedrock.dev/docs/stable/Entities>

Referencia del port:

- `target/axolotl-pm/src/entity/Mob.php` (PocketMine-MP/Axolotl): sin IA
  vanilla; solo gravedad y drag.
