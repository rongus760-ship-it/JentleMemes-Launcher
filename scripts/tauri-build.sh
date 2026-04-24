#!/usr/bin/env bash
# Полная сборка Tauri на Linux: linuxdeploy + AppImage без strip (RELR на Arch/Cachy).
# Использование: npm run tauri:build   или   bash scripts/tauri-build.sh -- -b deb
set -euo pipefail
cd "$(dirname "$0")/.."

export NO_STRIP=1
unset CI || true
export PATH="${HOME}/.local/bin:${PATH}"

bash scripts/ensure-linuxdeploy.sh

exec npx tauri build "$@"
