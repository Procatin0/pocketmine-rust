---
title: "README — carpeta wiki"
description: "README de docs/wiki/: qué contiene esta carpeta, cómo se navega y cómo se contribuye."
nav_order: 0
---

# README — `docs/wiki/`

> Este archivo es el **README de la carpeta `docs/wiki/`**.
> No confundir con el `README.md` de la raíz del proyecto (que no se toca
> desde aquí). El hub de lectura es [index.md](index.md).

## Qué hay aquí

| Archivo | Rol |
|---------|-----|
| [index.md](index.md) | **Hub** — índice, rutas de lectura y snippet de `nav` para MkDocs. |
| [01-introduccion.md](01-introduccion.md) | Qué es la Plugin API, filosofía Bedrock + Java, requisitos y ciclo de vida. |
| [02-quickstart.md](02-quickstart.md) | Tu primer plugin paso a paso + logs y colores. |
| `README.md` (este archivo) | Mapa de la carpeta y guía de contribución. |

## Convenciones

- Frontmatter YAML al inicio de cada `.md` (estilo MkDocs):
  `title`, `description`, `nav_order`.
- Enlaces **solo internos** a archivos que existen dentro de esta
  carpeta (`01-introduccion.md`, `02-quickstart.md`, `index.md`). Nada
  de enlaces rotos ni a rutas fuera de `wiki/`.
- Idioma: español. Código (PHP/Rust) y citas de log: en inglés tal cual
  salen del servidor.
- Ejemplos verificados contra el código real: `plugins/ExamplePlugin/`,
  `crates/php-plugin-bridge/` y `crates/server-core/src/logging.rs`.

## Cómo contribuir

1. Crea o edita un único archivo `.md` en esta carpeta.
2. Añade frontmatter y registra el enlace en [index.md](index.md).
3. Verifica que tu página no enlaza a nada inexistente (0 enlaces rotos).
4. No toques el `README.md` raíz ni otros docs fuera de `docs/wiki/`.

## Empezar

→ [Hub de la wiki](index.md) · [01 — Introducción](01-introduccion.md) ·
[02 — Quickstart](02-quickstart.md)
