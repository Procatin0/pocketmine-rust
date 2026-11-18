#!/usr/bin/env bash
# Captura la verdad de referencia de un BDS real (bloques/biomas por chunk).
#
# Uso:  ./capture.sh <seed> <puerto> <directorio-bds> [segundos]
#
# Requisitos (ver docs/bds-biome-parity.md):
#   * bedrock-server-<version>.zip descomprimido en <directorio-bds>
#   * client-side-chunk-generation-enabled=false  (si no, el servidor no
#     genera chunks: se los pide al cliente)
#   * allow-list=false  y  online-mode=false  (si no, BDS se niega a arrancar)
#
# El script crea un mundo nuevo, fuerza la generación de un área 10x10 chunks
# con /tickingarea, espera a que el servidor la termine y copia el LevelDB a
# ./captures/<seed>/db para que lo lea `cargo run -p bds-probe -- ...`.

set -euo pipefail

seed="${1:?falta la semilla}"
port="${2:?falta el puerto}"
bds="${3:?falta el directorio del BDS}"
seconds="${4:-45}"
out="captures/${seed}"

cd "$bds"
rm -rf worlds
sed -i \
  -e "s/^level-seed=.*/level-seed=${seed}/" \
  -e "s/^server-port=.*/server-port=${port}/" \
  -e "s/^level-name=.*/level-name=capture/" \
  -e "s/^view-distance=.*/view-distance=4/" \
  -e "s/^online-mode=.*/online-mode=false/" \
  -e "s/^allow-list=.*/allow-list=false/" \
  -e "s/^allow-cheats=.*/allow-cheats=true/" \
  -e "s/^client-side-chunk-generation-enabled=.*/client-side-chunk-generation-enabled=false/" \
  server.properties

# 10x10 chunks (-0..159) caben en persistent-ticking-area-max-chunks=100.
( sleep 18; echo "tickingarea add 0 0 0 159 0 159 gen"; sleep "$seconds"; echo stop ) \
  | timeout $((seconds + 90)) ./bedrock_server > "capture-${seed}.log" 2>&1 || true

mkdir -p "$out"
cp -r worlds/capture/db "$out/db"
echo "captura lista: $out/db"
