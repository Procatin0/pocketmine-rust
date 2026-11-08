#!/usr/bin/env python3
"""Genera resources/tag_members/<protocolo>.json (miembros numéricos de tags).

Los nombres de tags ya vienen de resources/tags/<protocolo>.json (mcmeta);
aquí solo se resuelven los IDs de registro de los miembros de un subconjunto
de tags, porque sin miembros el cliente no muestra ningún tag en F3
(p. ej. agua sin #minecraft:water, grava sin #minecraft:mineable/shovel)
aunque el tag "exista" con la lista vacía.

Fuentes (minecraft-data, checkout en target/verify/node_modules):
  - blocks.json / items.json por versión: el campo `id` es el ID del
    registro estático `minecraft:block` / `minecraft:item` que usa
    `update_tags` (igual que `entities.json` -> `entity_type`, verificado
    con player=155 en 775 en ModernProfile::playerEntityType()).
  - El registro `minecraft:fluid` no está en minecraft-data: es estático
    y su orden sale del `<clinit>` de `Fluids` en el jar vanilla
    (verificado con `javap -c` sobre `target/server-26.1-inner.jar`,
    `net.minecraft.world.level.material.Fluids`): empty=0,
    flowing_water=1, water=2, flowing_lava=3, lava=4. Los variantes que
    fluyen SÍ son registros aparte (WaterFluid$Flowing/Source y
    LavaFluid$Flowing/Source), no el mismo fluido con level>0. Por eso
    F3 mostraba el agua (id 2, `WaterFluid$Source`) con `#minecraft:lava`:
    antes se mandaba lava->[2] y el cliente lee el 2 como agua.

Reglas (documentadas para no inventar miembros):
  - fluid (según `FluidTagsProvider` y `data/minecraft/tags/fluid/*.json`
    del jar vanilla 26.1): water->[1,2] (flowing_water+water),
    lava->[3,4] (flowing_lava+lava), supports_sugar_cane_adjacently->[1,2]
    (`#minecraft:water`) y bubble_column_can_occupy/supports_frogspawn/
    supports_lily_pad->[2] (solo `water`, fuente).
  - block mineable/<herramienta>: bloques cuyo `material` contiene
    `mineable/<herramienta>`; los bloques con material `incorrect_for_*`
    también son mineable/pickaxe en vanilla (minecraft-data solo guarda
    una de las dos etiquetas), así que se incluyen en pickaxe.
  - block sword_instantly_mines / wool: material exacto en blocks.json.
  - block climbable: ladder, vine, weeping_vines, twisting_vines,
    scaffolding, crying_obsidian (lista vanilla).
  - block fall_damage_resetting: powder_snow, honey_block (lista vanilla).
  - item axes/pickaxes/shovels/hoes/swords/boats/beds: sufijo exacto en
    items.json (`_axe`, ...); arrows: solo `arrow`.
El resto de tags se sigue enviando con la lista vacía (el nombre basta
para que `getOrThrow` no aborte con "Missing tag").

Uso: python3 tools/gen-tag-members.py   (desde plugins/JavaBridge/)
"""
import json
import os
import sys

BASE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ROOT = os.path.dirname(os.path.dirname(BASE))
MCDATA = os.path.join(
    ROOT, "target", "verify", "node_modules", "minecraft-data",
    "minecraft-data", "data", "pc",
)

# protocolo -> (versión minecraft-data, nombre versión)
VERSIONS = {
    769: "1.21.4",
    770: "1.21.5",
    773: "1.21.9",
    774: "1.21.11",
    775: "26.1",
}

# Orden de registro de `minecraft:fluid` en `Fluids.<clinit>` (jar
# vanilla 26.1, `javap -c ... net.minecraft.world.level.material.Fluids`):
# empty, flowing_water, water, flowing_lava, lava. Estable desde 1.13.
FLUID_IDS = {"empty": 0, "flowing_water": 1, "water": 2,
             "flowing_lava": 3, "lava": 4}
# Miembros según `FluidTagsProvider` + `data/minecraft/tags/fluid/*.json`
# del jar vanilla: water=lava incluyen su variante fluyente;
# supports_sugar_cane_adjacently es `#minecraft:water`; el resto, solo la
# fuente. (No existe tag de fluido `supports_sugar_cane`, solo de bloque.)
FLUID_TAG_MEMBERS = {
    "water": ["water", "flowing_water"],
    "lava": ["lava", "flowing_lava"],
    "bubble_column_can_occupy": ["water"],
    "supports_frogspawn": ["water"],
    "supports_lily_pad": ["water"],
    "supports_sugar_cane_adjacently": ["water", "flowing_water"],
}

CLIMBABLE = ["ladder", "vine", "weeping_vines", "twisting_vines",
             "scaffolding", "crying_obsidian"]
FALL_DAMAGE_RESETTING = ["powder_snow", "honey_block"]

ITEM_SUFFIX_TAGS = {
    "axes": "_axe",
    "pickaxes": "_pickaxe",
    "shovels": "_shovel",
    "hoes": "_hoe",
    "swords": "_sword",
    "boats": "_boat",
    "beds": "_bed",
}


def load_json(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def ids_for(names, by_name):
    return sorted({by_name[n] for n in names if n in by_name})


def main():
    out_dir = os.path.join(BASE, "resources", "tag_members")
    os.makedirs(out_dir, exist_ok=True)
    for protocol, version in VERSIONS.items():
        tags_file = os.path.join(BASE, "resources", "tags", f"{protocol}.json")
        tag_names = load_json(tags_file)["registries"]
        blocks = load_json(os.path.join(MCDATA, version, "blocks.json"))
        items = load_json(os.path.join(MCDATA, version, "items.json"))
        block_id = {b["name"]: b["id"] for b in blocks}
        item_id = {i["name"]: i["id"] for i in items}

        members = {}

        # --- fluid ---
        fluid = {}
        for tag, names in FLUID_TAG_MEMBERS.items():
            if tag in tag_names.get("fluid", []):
                fluid[tag] = ids_for(names, FLUID_IDS)
        if fluid:
            members["fluid"] = fluid

        # --- block ---
        block = {}
        for tool in ("pickaxe", "axe", "shovel", "hoe"):
            tag = f"mineable/{tool}"
            if tag not in tag_names.get("block", []):
                continue
            names = []
            for b in blocks:
                parts = str(b.get("material", "")).split(";")
                if f"mineable/{tool}" in parts:
                    names.append(b["name"])
                elif tool == "pickaxe" and any(
                        p.startswith("incorrect_for_") for p in parts):
                    names.append(b["name"])
            block[tag] = ids_for(names, block_id)
        if "climbable" in tag_names.get("block", []):
            block["climbable"] = ids_for(CLIMBABLE, block_id)
        if "fall_damage_resetting" in tag_names.get("block", []):
            block["fall_damage_resetting"] = ids_for(FALL_DAMAGE_RESETTING, block_id)
        for tag, material in (("sword_instantly_mines", "sword_instantly_mines"),
                              ("wool", "wool")):
            if tag in tag_names.get("block", []):
                names = [b["name"] for b in blocks
                         if b.get("material") == material]
                block[tag] = ids_for(names, block_id)
        if block:
            members["block"] = block

        # --- item ---
        item = {}
        for tag, suffix in ITEM_SUFFIX_TAGS.items():
            if tag in tag_names.get("item", []):
                names = [i["name"] for i in items if i["name"].endswith(suffix)]
                item[tag] = ids_for(names, item_id)
        if "arrows" in tag_names.get("item", []):
            item["arrows"] = ids_for(["arrow"], item_id)
        if item:
            members["item"] = item

        payload = {
            "protocol": protocol,
            "version": version,
            "source": "minecraft-data blocks/items.json + vanilla fluid registry (Fluids.<clinit> in server-26.1-inner.jar)",
            "registries": members,
        }
        out = os.path.join(out_dir, f"{protocol}.json")
        with open(out, "w", encoding="utf-8") as fh:
            json.dump(payload, fh, separators=(",", ":"), sort_keys=True)
            fh.write("\n")
        total = sum(len(v) for reg in members.values() for v in reg.values())
        print(f"{protocol} ({version}): "
              f"{ {reg: len(tags) for reg, tags in members.items()} } "
              f"{total} miembros -> {out}")


if __name__ == "__main__":
    sys.exit(main())
