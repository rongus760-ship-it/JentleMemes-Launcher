#!/usr/bin/env bash
# Копирует собранные бинарники в dist-artifacts/ с явными расширениями (удобно для загрузки на сайт).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/src-tauri/target}/release"
OUT="$ROOT/dist-artifacts"
mkdir -p "$OUT"

LAUNCHER_VER=$(grep -m1 '^version' "$ROOT/src-tauri/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')
INST_VER=$(grep -m1 '^version' "$ROOT/installer/src-tauri/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')

if [[ -f "$TARGET/jentlememes-launcher" ]]; then
  cp -f "$TARGET/jentlememes-launcher" "$OUT/JentleMemesLauncher_${LAUNCHER_VER}.linux-x64.bin"
  echo "OK: $OUT/JentleMemesLauncher_${LAUNCHER_VER}.linux-x64.bin"
fi

if [[ -f "$TARGET/jentlememes-installer" ]]; then
  cp -f "$TARGET/jentlememes-installer" "$OUT/JentleMemes-Setup_${INST_VER}.linux-x64.bin"
  echo "OK: $OUT/JentleMemes-Setup_${INST_VER}.linux-x64.bin"
fi

for deb in "$TARGET/bundle/deb/"*.deb; do
  [[ -f "$deb" ]] || continue
  cp -f "$deb" "$OUT/"
  echo "OK: $OUT/$(basename "$deb")"
done

echo "Done. Upload Windows builds separately (*.exe from cross-compile)."
