---
title: "02 — Quickstart: tu primer plugin"
description: "Crea un plugin PHP (Main.php + PluginBase onEnable/onDisable), ejecútalo con cargo run -p server-core y verifica los logs Axolotl-PM."
nav_order: 2
---

# 02 — Quickstart: tu primer plugin

[← 01 — Introducción](01-introduccion.md) · [Hub de la wiki](index.md)

En 5 minutos tendrás un plugin `Demo` cargado, con un comando `/hola`
y un log en la consola. Todo en PHP, sin tocar Rust.

## 1. Crea la estructura en `plugins/`

```text
plugins/Demo/
├── plugin.yml
└── src/Demo/Main.php
```

## 2. `plugins/Demo/plugin.yml`

```yaml
# plugins/Demo/plugin.yml
name: Demo
version: "1.0.0"
main: Demo\Main
api: ["5.0.0"]
author: TuNombre
description: "Plugin de demostración de la wiki."
# load: postworld   # ← opcional: startup = antes de los mundos

commands:
  hola:
    description: "Saluda al remitente."
    usage: "/hola"
    aliases: ["hi"]
    permission: demo.command.hola

permissions:
  demo.command.hola:
    description: "Permite usar /hola."
    default: true
```

> `main: Demo\Main` significa: busca `src/Demo/Main.php` (PSR-4) y
> carga la clase `Demo\Main` que extiende `PluginBase`.

## 3. `plugins/Demo/src/Demo/Main.php`

```php
<?php

declare(strict_types=1);

namespace Demo;

use pocketmine\command\Command;
use pocketmine\command\CommandSender;
use pocketmine\plugin\PluginBase;
use pocketmine\utils\TextFormat;

final class Main extends PluginBase{

	/** Se llama al habilitar el plugin (fase STARTUP o POSTWORLD). */
	public function onEnable() : void{
		$this->getLogger()->info(TextFormat::GREEN . "Demo habilitado.");
	}

	/** Se llama al deshabilitar (al apagar el servidor, orden inverso). */
	public function onDisable() : void{
		$this->getLogger()->info("Demo deshabilitado.");
	}

	/** Executor del comando declarado en plugin.yml. */
	public function onCommand(CommandSender $sender, Command $command, string $label, array $args) : bool{
		if($command->getName() !== "hola"){
			return false;
		}
		$sender->sendMessage(TextFormat::AQUA . "¡Hola, " . $sender->getName() . "!");
		return true;
	}
}
```

Qué está pasando:

| Hook / llamada | Cuándo y para qué |
|----------------|-------------------|
| `onEnable()` | Registro de comandos, listeners, scheduler, primer log. |
| `onDisable()` | Limpieza y guardado (al apagar). |
| `onCommand()` | Lo invoca el puente cuando alguien escribe `/hola`. |
| `TextFormat::GREEN`, `::AQUA` | Códigos `§a`, `§b` que el log y el cliente colorean. |

> Referencia completa en el repo: `plugins/ExamplePlugin/src/pocketmine/example/Main.php`
> (mismo patrón más `saveDefaultConfig()`, `registerEvents()` y el evento
> `PlayerJoinEvent`).

## 4. Arranca el servidor

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo run -p server-core
```

Solo con PHP 8.2 en el `PATH` basta: el worker PHP se lanza solo
(`create_dir_all("plugins")` si no existe, crea `plugin_data/Demo/`).

## 5. Qué deberías ver en los logs

Las tres fases del ciclo dejan huella. Copia de referencia (hora y versión
pueden variar):

```text
[10:00:01.012] [Server thread/INFO]: Loading Demo v1.0.0
[10:00:01.020] [Server thread/INFO]: Enabling Demo v1.0.0
[10:00:01.021] [Server thread/INFO]: Demo habilitado.
[10:00:01.050] [Server thread/INFO]: Preparing world "world"
```

- `Loading D…` → fase load (`onLoad`, §4 de 01).
- `Enabling D…` → fase enable (`onEnable`); `postworld` va **después** de
  `Preparing world …`, `startup` va **antes**.
- Entrando en el juego y escribiendo `/hola` en consola o en el cliente
  verás la respuesta `¡Hola, <nombre>!`.

Si algo falla al cargar, el servidor se detiene y deja pistas:

```text
[Server thread/CRITICAL]: Could not load plugin 'plugins/Demo': Invalid plugin manifest: main
[Server thread/EMERGENCY]: Failed to load some plugins, … (someLoadErrors)
```

Errores típicos: `main` que no coincide con el namespace/clase real,
`load: early` (solo valen `startup`/`postworld`) o un `Main.php`
con error de sintaxis PHP (`php -l plugins/Demo/src/Demo/Main.php`).

## 6. Logs de Axolotl-PM y colores (guía rápida)

| Nivel | Color | Cuándo |
|-------|-------|--------|
| `EMERGENCY` / `ALERT` / `CRITICAL` | rojo `§c` (`\x1b[91m`) | El servidor no puede seguir (errores de carga/enable). |
| `ERROR` | rojo oscuro `§4` (`\x1b[31m`) | Error de plugin sin parada. |
| `WARNING` | amarillo `§e` (`\x1b[93m`) | Aviso (p. ej. comando no encontrado en `plugin.yml`). |
| `NOTICE` | aqua `§b` (`\x1b[36m`) | Aviso estructurado. |
| `INFO` | blanco `§f` (`\x1b[97m`) | `Loading/Enabling/Disabling …`, tus `->info()`. |
| `DEBUG` | gris `§8` (`\x1b[90m`) | Solo con `debug.level > 1` o `POCKETMINE_LOG_LEVEL=debug`. |

- Formato: `[HH:MM:SS.vvv] [Server thread/NIVEL]: mensaje` (hora en aqua).
- En terminal los `§` se vuelven ANSI; **en `server.log` salen limpios**,
  con prefijo `YYYY-MM-DD`. Al superar 32 MiB se archiva en `log_archive/`.
- Tus `TextFormat::RED`/`GREEN`/`AQUA`… en mensajes a jugadores y en
  logs comparten los mismos códigos `§`.

## 7. Siguiente paso

- Añade un listener: `use pocketmine\event\Listener;`,
  `implements Listener`, `registerEvents($this, $this)` en `onEnable`
  y un método `public function onPlayerJoin(PlayerJoinEvent $e)`.
- Lee el ciclo completo y `STARTUP` vs `POSTWORLD` en
  [01 — Introducción](01-introduccion.md).
