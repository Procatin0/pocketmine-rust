---
title: "Wiki — Plugin API"
description: "Hub de la documentación de la Plugin API de PocketMine-Rust (Axolotl-PM): PHP bridge + crates Rust."
nav_order: 0
---

# Wiki — Plugin API

Bienvenido/a al hub de la **Plugin API** de **Axolotl-PM / PocketMine-Rust**.
Aquí se documenta cómo se escriben, cargan y ejecutan los plugins de PHP
sin tocar el núcleo Rust.

> Centro del servidor: un solo núcleo (`server-core`) atiende **Bedrock**
> (protocolo 2225, Minecraft 1.26.60) y **Java** (puerto `25565`) con la
> misma API de plugins. Los plugins se escriben en **PHP 8.2** y se ejecutan
> en un *worker* PHP conectado al servidor Rust por IPC JSON-lines.

## Contenido

| # | Página | Qué cubre |
|---|--------|-----------|
| 01 | [Introducción](01-introduccion.md) | Qué es la Plugin API, arquitectura PHP bridge + Rust, filosofía Bedrock + Java, requisitos (`plugins/`, `plugin.yml`), ciclo de vida y `PluginEnableOrder`. |
| 02 | [Quickstart — tu primer plugin](02-quickstart.md) | `Main.php` con `PluginBase::onEnable` / `onDisable`, ejecutar el servidor, ver logs de Axolotl-PM y colores. |

```text
wiki/
├── index.md              ← HUB (esta página)
├── README.md             ← README de la carpeta wiki (estructura y contribución)
├── 01-introduccion.md    ← Qué es y cómo encaja el puente
└── 02-quickstart.md      ← Primer plugin en 5 minutos
```

## ¿Por dónde empezar?

- **Nunca programaste un plugin PMMP?** → Lee
  [01 — Introducción](01-introduccion.md) y luego el
  [02 — Quickstart](02-quickstart.md).
- **Ya conoces PocketMine-MP 5?** → Salta directo al
  [02 — Quickstart](02-quickstart.md): el contrato es el mismo
  (`PluginBase`, `plugin.yml`, `api: ["5.0.0"]`, eventos, comandos).

## Navegación (MkDocs)

Esta carpeta está pensada como sección de MkDocs. Si activas el sitio,
declara la navegación en tu `mkdocs.yml` (no se crea aquí para no tocar
otros docs):

```yaml
# mkdocs.yml — snippet sugerido
nav:
  - Plugin API:
      - Wiki (hub): wiki/index.md
      - Introducción: wiki/01-introduccion.md
      - Quickstart: wiki/02-quickstart.md
```

> Cada página de esta carpeta lleva frontmatter YAML estilo MkDocs
> (`title`, `description`, `nav_order`) para que MkDocs Material la
> ordene sin necesidad de configuración extra.

## Reglas de esta wiki

1. **Un plugin = una carpeta (o `.phar`) en `plugins/` + `plugin.yml`.**
   Nada de modificar el código Rust para añadir lógica de negocio.
2. **PHP es la interfaz; Rust es el motor.** Si necesitas velocidad,
   paquetes de red o mundos, va al crate Rust correspondiente (ver la
   tabla en [01 — Introducción](01-introduccion.md)).
3. **Los logs mandan.** Si un plugin «no funciona», mira primero el log
   de Axolotl-PM y `server.log` (ver [02 — Quickstart](02-quickstart.md)).

## Siguiente paso

→ [01 — Introducción](01-introduccion.md)
