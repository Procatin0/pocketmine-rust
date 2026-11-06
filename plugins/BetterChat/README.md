# BetterChat

Plugin avanzado de gestion de chat para servidores PocketMine-MP (API 5.x).

## Caracteristicas

- **Anti-Spam** — Cooldown entre mensajes, bloqueo de duplicados, limite de longitud y exceso de mayusculas
- **Filtro de palabras** — Lista configurable de palabras bloqueadas con reemplazo automatico
- **Anti-Links** — Bloquea o filtra URLs con whitelist de dominios permitidos
- **Sistema de mute** — Mutea jugadores temporal o permanentemente, persiste entre reinicios
- **Vote Mute** — Los jugadores votan para mutear usuarios toxicos democraticamente
- **Chat Groups** — Crea grupos/clanes con canales de chat privados
- **Minijuegos** — Trivia, unscramble y math con recompensas configurables y auto-inicio
- **Auto-Broadcast** — Cicla mensajes automaticamente cada X segundos
- **Codigos de color** — Soporte para `&` color codes con permiso
- **Menciones** — Notifica a jugadores cuando su nombre es mencionado en el chat
- **RankSystem** — Se integra con IvanCraft623/RankSystem cuando esta instalado
- **Config auto-update** — Actualiza la config automaticamente al actualizar el plugin sin perder settings

## Instalacion

1. Descarga la carpeta `BetterChat` o el `.phar`
2. Colocala en la carpeta `plugins/` de tu servidor
3. Reinicia el servidor
4. Edita los archivos de configuracion a tu gusto
5. Usa `/bchat reload` para aplicar cambios sin reiniciar

## Comandos

### `/bchat` — Comandos de admin

| Comando | Descripcion | Permiso |
|---|---|---|
| `/bchat reload` | Recarga la configuracion | `betterchat.command.reload` |
| `/bchat mute <jugador> [duracion]` | Mutea un jugador (30s, 5m, 2h, 1d) | `betterchat.command.mute` |
| `/bchat unmute <jugador>` | Desmutea un jugador | `betterchat.command.unmute` |
| `/bchat setprefix <jugador> <prefix>` | Asigna un prefix personalizado | `betterchat.command.setprefix` |
| `/bchat setsuffix <jugador> <suffix>` | Asigna un suffix personalizado | `betterchat.command.setsuffix` |
| `/bchat minigame <start\|stop> [tipo]` | Inicia o detiene un minijuego | `betterchat.command.minigame` |

### `/group` — Grupos de chat

| Comando | Descripcion |
|---|---|
| `/group create <nombre>` | Crea un grupo nuevo |
| `/group create <nombre> official` | Crea grupo oficial (solo admin) |
| `/group delete <nombre>` | Elimina tu grupo |
| `/group invite <grupo> <jugador>` | Invita un jugador |
| `/group accept <grupo>` | Acepta invitacion |
| `/group deny <grupo>` | Rechaza invitacion |
| `/group leave <grupo>` | Sale del grupo |
| `/group kick <grupo> <jugador>` | Expulsa miembro (solo dueno) |
| `/group chat [grupo]` | Activa/desactiva modo chat de grupo |
| `/group list` | Lista tus grupos |
| `/group info <grupo>` | Muestra info del grupo |

Tambien podes enviar directo: `@grupo mensaje` (sin necesidad de toggle)

### `/votemute` — Votar para mutear

| Comando | Descripcion |
|---|---|
| `/votemute <jugador>` | Inicia votacion para mutear |
| `/votemute yes` | Vota si |
| `/votemute no` | Vota no |

Requiere minimo de jugadores online. Pasa con porcentaje de "si" configurado. El jugador muteado recibe mute temporal automatico.

## Permisos

| Permiso | Descripcion | Default |
|---|---|---|
| `betterchat.admin` | Todos los comandos admin | OP |
| `betterchat.command.reload` | Recargar config | OP |
| `betterchat.command.mute` | Mutear jugadores | OP |
| `betterchat.command.unmute` | Desmutear jugadores | OP |
| `betterchat.command.setprefix` | Asignar prefix | OP |
| `betterchat.command.setsuffix` | Asignar suffix | OP |
| `betterchat.command.minigame` | Gestionar minijuegos | OP |
| `betterchat.color` | Usar codigos de color en chat | OP |
| `betterchat.bypass.spam` | Saltarse anti-spam | OP |
| `betterchat.bypass.filter` | Saltarse filtro de palabras | OP |
| `betterchat.bypass.links` | Saltarse filtro de links | OP |
| `betterchat.mention` | Recibir notificaciones de mencion | Todos |
| `betterchat.group.use` | Usar comandos de grupos | Todos |
| `betterchat.group.create` | Crear grupos | Todos |
| `betterchat.group.admin` | Grupos oficiales, sin limites | OP |
| `betterchat.votemute` | Iniciar y votar en muteos | Todos |
| `betterchat.votemute.bypass` | No puede ser vote-muteado | OP |

## Archivos de configuracion

| Archivo | Contenido |
|---|---|
| `config.yml` | Toda la configuracion del plugin |
| `minigames.yml` | Preguntas de trivia y palabras de unscramble |
| `broadcasts.yml` | Mensajes de auto-broadcast |
| `lang/en.yml` | Todos los mensajes visibles para usuarios |

## Requisitos

- PocketMine-MP API 5.0.0 o superior
- PHP 8.0+

## Dependencia opcional

- **[IvanCraft623/RankSystem](https://github.com/IvanCraft623/RankSystem)** — Cuando esta instalado, RankSystem maneja el formato del chat (prefix, suffix, rango). Las demas funciones de BetterChat siguen funcionando normalmente.
