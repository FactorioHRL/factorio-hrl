#!/usr/bin/env bash
# Copies entity graphics and icons from a local Factorio install into ./data, plus the data dump.
# Usage: scripts/setup-data.sh <game-dir> <data-raw-dump.json>
# On WSL the game dir is e.g. "/mnt/c/Program Files (x86)/Steam/steamapps/common/Factorio".
set -euo pipefail

if [ $# -ne 2 ]; then
  echo "usage: $0 <factorio-dir> <data-raw-dump.json>" >&2
  exit 1
fi
game="$1"
dump="$2"
root="$(cd "$(dirname "$0")/.." && pwd)"

[ -d "$game/data/base" ] || { echo "'$game' is not a Factorio directory (no data/base)" >&2; exit 1; }
[ -f "$dump" ] || { echo "dump '$dump' not found" >&2; exit 1; }

mkdir -p "$root/data"
cp "$dump" "$root/data/data-raw-dump.json"
for mod in "$game"/data/*/; do
  name="$(basename "$mod")"
  for sub in entity icons; do
    src="$mod/graphics/$sub"
    [ -d "$src" ] || continue
    mkdir -p "$root/data/game/data/$name/graphics"
    cp -r "$src" "$root/data/game/data/$name/graphics/"
  done
done
echo "done: $(du -sh "$root/data" | cut -f1) in $root/data"
