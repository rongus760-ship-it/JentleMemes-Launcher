#!/usr/bin/env bash
# Tauri AppImage bundling needs linuxdeploy on PATH (often missing on minimal setups).
# Usage: source this file or run before tauri build:
#   bash scripts/ensure-linuxdeploy.sh && export PATH="$HOME/.local/bin:$PATH"
set -euo pipefail

BIN_DIR="${LINUXDEPLOY_BIN_DIR:-$HOME/.local/bin}"
mkdir -p "$BIN_DIR"

APPIMG="$BIN_DIR/linuxdeploy-x86_64.AppImage"
if [[ ! -f "$APPIMG" ]]; then
  echo "Downloading linuxdeploy to $APPIMG ..."
  curl -fsSL -o "$APPIMG" \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
  chmod +x "$APPIMG"
fi

# Bundler invokes plain "linuxdeploy"
if [[ ! -e "$BIN_DIR/linuxdeploy" ]]; then
  ln -sf "$APPIMG" "$BIN_DIR/linuxdeploy"
fi

echo "linuxdeploy OK: $BIN_DIR/linuxdeploy -> $APPIMG"
echo "Add to PATH for this shell: export PATH=\"$BIN_DIR:\$PATH\""
