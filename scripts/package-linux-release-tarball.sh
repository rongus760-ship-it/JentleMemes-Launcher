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

SHA256="$(sha256sum "$OUTDIR/$ARCHIVE" | awk '{print $1}')"
echo "Created: $OUTDIR/$ARCHIVE"
echo "sha256:  $SHA256"

# --- копируем tarball к обоим PKGBUILD'ам и синхронизируем sha256sums ---
# Так makepkg сможет собрать пакет локально ещё до публикации GitHub Release:
# source=() смотрит сначала в каталог PKGBUILD, и только если там нет — качает.
sync_pkgbuild() {
  local dir="$1"
  local pkgbuild="$dir/PKGBUILD"
  [[ -f "$pkgbuild" ]] || { echo "  skip (no PKGBUILD): $dir"; return; }
  cp -f "$OUTDIR/$ARCHIVE" "$dir/$ARCHIVE"
  # Чинит поле sha256sums=('...') в PKGBUILD in-place
  sed -i -E "s|^sha256sums=\\('[0-9a-fA-F]+'\\)|sha256sums=('$SHA256')|" "$pkgbuild"
  # Обновляем pkgver в PKGBUILD (на случай когда он не совпадает с Cargo.toml)
  sed -i -E "s|^pkgver=.*|pkgver=$PKGVER|" "$pkgbuild"
  # Если рядом лежит .SRCINFO — регенерируем
  if command -v makepkg >/dev/null 2>&1 && [[ -f "$dir/.SRCINFO" ]]; then
    ( cd "$dir" && makepkg --printsrcinfo > .SRCINFO ) || true
  fi
  echo "  synced: $pkgbuild (pkgver=$PKGVER, sha256=${SHA256:0:12}…)"
}

echo ""
echo "Синхронизация PKGBUILD-ов:"
sync_pkgbuild "$ROOT/jentlememes-launcher-bin"
sync_pkgbuild "$ROOT/packaging/aur/jentlememes-launcher-bin"

echo ""
echo "Дальше:"
echo "  1. Загрузите $OUTDIR/$ARCHIVE в GitHub Release v$PKGVER как ассет"
echo "     с именем ровно: $ARCHIVE"
echo "  2. Локальная сборка AUR-пакета:"
echo "       cd jentlememes-launcher-bin && makepkg -si"
