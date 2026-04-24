#!/usr/bin/env bash
# Собирает jentlememes-launcher-${pkgver}-linux-x86_64.tar.gz для GitHub Release
# (ассет, который ждёт jentlememes-launcher-bin/PKGBUILD).
#
# Использование:
#   bash scripts/package-linux-release-tarball.sh
#   bash scripts/package-linux-release-tarball.sh 2.0.0 /path/to/jentlememes-launcher
#
# Результат: dist/jentlememes-launcher-<pkgver>-linux-x86_64.tar.gz
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PKGVER="${1:-$(grep -m1 '^version' "$ROOT/src-tauri/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')}"
BIN="${2:-${CARGO_TARGET_DIR:-$ROOT/src-tauri/target}/release/jentlememes-launcher}"
DESKTOP="$ROOT/jentlememes-launcher-bin/jentlememes-launcher.desktop"
OUTDIR="$ROOT/dist"
STAGE="$OUTDIR/.tarball-stage-${PKGVER}"
ARCHIVE="jentlememes-launcher-${PKGVER}-linux-x86_64.tar.gz"

if [[ ! -f "$BIN" ]]; then
  echo "error: binary not found: $BIN" >&2
  echo "  build first: npm run tauri:build  (or cargo build --release in src-tauri)" >&2
  exit 1
fi
if [[ ! -f "$DESKTOP" ]]; then
  echo "error: desktop file not found: $DESKTOP" >&2
  exit 1
fi

rm -rf "$STAGE"
mkdir -p "$STAGE"
cp -f "$BIN" "$STAGE/jentlememes-launcher"
chmod 755 "$STAGE/jentlememes-launcher"
cp -f "$DESKTOP" "$STAGE/jentlememes-launcher.desktop"
mkdir -p "$OUTDIR"
rm -f "$OUTDIR/$ARCHIVE"
( cd "$STAGE" && tar --owner=0 --group=0 -czf "$ROOT/dist/$ARCHIVE" \
    jentlememes-launcher jentlememes-launcher.desktop )
rm -rf "$STAGE"

echo "Created: $OUTDIR/$ARCHIVE"
sha256sum "$OUTDIR/$ARCHIVE"
echo ""
echo "Дальше: загрузите этот файл в GitHub Release как ассет с именем ровно:"
echo "  $ARCHIVE"
