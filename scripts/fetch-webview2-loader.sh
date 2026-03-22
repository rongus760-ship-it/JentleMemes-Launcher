#!/bin/bash
# Скачивает WebView2Loader.dll для Windows-GNU сборки
# Нужно положить рядом с jentlememes-launcher.exe

set -e
OUT_DIR="${1:-src-tauri/target/x86_64-pc-windows-gnu/release}"
NUGET_URL="https://api.nuget.org/v3-flatcontainer/microsoft.web.webview2/1.0.3650.58/microsoft.web.webview2.1.0.3650.58.nupkg"
TMP=$(mktemp -d)

echo "Скачивание Microsoft.Web.WebView2..."
curl -sL "$NUGET_URL" -o "$TMP/pkg.nupkg"

echo "Распаковка..."
unzip -q -o "$TMP/pkg.nupkg" -d "$TMP/pkg"

echo "Копирование WebView2Loader.dll в $OUT_DIR"
mkdir -p "$OUT_DIR"
cp "$TMP/pkg/runtimes/win-x64/native/WebView2Loader.dll" "$OUT_DIR/"

rm -rf "$TMP"
echo "Готово! WebView2Loader.dll скопирован в $OUT_DIR"
