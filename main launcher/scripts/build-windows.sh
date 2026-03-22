#!/bin/bash
# Build JentleMemes Launcher for Windows from Linux
#
# Usage:
#   ./build-windows.sh          — build everything (setup + standalone app)
#   ./build-windows.sh setup    — build only the NSIS setup installer
#   ./build-windows.sh app      — build only the standalone launcher .exe
#   ./build-windows.sh all      — same as no argument
#
# Prerequisites:
#   - nsis (makensis) — Arch/CachyOS: yay -S nsis | Ubuntu: sudo apt install nsis
#   - lld, llvm, clang
#   - rustup target add x86_64-pc-windows-msvc
#   - cargo install --locked cargo-xwin

set -e

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="x86_64-pc-windows-msvc"
DIST_DIR="$ROOT_DIR/dist"
STUB_BUILD="$ROOT_DIR/installer-stub"
MODE="${1:-all}"

LAUNCHER_VER=$(grep -m1 '^version' "$ROOT_DIR/src-tauri/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')

echo "========================================="
echo " JentleMemes Launcher — Windows Build"
echo " Mode: $MODE  |  Version: $LAUNCHER_VER"
echo "========================================="

mkdir -p "$DIST_DIR"

# ─────────────────────────────────────────────
#  Build the main launcher binary (shared step)
# ─────────────────────────────────────────────
build_launcher() {
    echo ""
    echo "[*] Building main launcher..."
    cd "$ROOT_DIR"
    npm run tauri build -- --runner cargo-xwin --target "$TARGET" --no-bundle

    LAUNCHER_BIN="$ROOT_DIR/src-tauri/target/$TARGET/release/jentlememes-launcher.exe"
    if [ ! -f "$LAUNCHER_BIN" ]; then
        echo "ERROR: Launcher binary not found at $LAUNCHER_BIN"
        exit 1
    fi
    echo "  -> $LAUNCHER_BIN"
}

# ─────────────────────────────────────────────
#  Standalone app: copy launcher .exe to dist/
# ─────────────────────────────────────────────
package_app() {
    LAUNCHER_BIN="$ROOT_DIR/src-tauri/target/$TARGET/release/jentlememes-launcher.exe"
    APP_EXE="$DIST_DIR/JentleMemesLauncher_${LAUNCHER_VER}.exe"
    cp "$LAUNCHER_BIN" "$APP_EXE"

    WV2_LOADER="$ROOT_DIR/src-tauri/target/$TARGET/release/WebView2Loader.dll"
    if [ -f "$WV2_LOADER" ]; then
        cp "$WV2_LOADER" "$DIST_DIR/"
    fi

    SIZE=$(du -h "$APP_EXE" | cut -f1)
    echo ""
    echo "  APP READY: $APP_EXE ($SIZE)"
}

# ─────────────────────────────────────────────
#  Setup: build installer + NSIS stub
# ─────────────────────────────────────────────
build_setup() {
    echo ""
    echo "[*] Building installer app..."
    cd "$ROOT_DIR/installer"
    npm install
    npm run tauri build -- --runner cargo-xwin --target "$TARGET" --no-bundle

    INSTALLER_BIN="$ROOT_DIR/installer/src-tauri/target/$TARGET/release/jentlememes-installer.exe"
    if [ ! -f "$INSTALLER_BIN" ]; then
        echo "ERROR: Installer binary not found at $INSTALLER_BIN"
        exit 1
    fi
    echo "  -> $INSTALLER_BIN"

    echo ""
    echo "[*] Preparing payload..."
    PAYLOAD_DIR="$STUB_BUILD/payload"
    rm -rf "$PAYLOAD_DIR"
    mkdir -p "$PAYLOAD_DIR"

    LAUNCHER_BIN="$ROOT_DIR/src-tauri/target/$TARGET/release/jentlememes-launcher.exe"
    cp "$LAUNCHER_BIN" "$PAYLOAD_DIR/"

    WV2_LOADER="$ROOT_DIR/src-tauri/target/$TARGET/release/WebView2Loader.dll"
    if [ -f "$WV2_LOADER" ]; then
        cp "$WV2_LOADER" "$PAYLOAD_DIR/"
    fi

    RESOURCES_DIR="$ROOT_DIR/src-tauri/target/$TARGET/release/resources"
    if [ -d "$RESOURCES_DIR" ]; then
        cp -r "$RESOURCES_DIR" "$PAYLOAD_DIR/"
    fi

    echo "  -> Contents:"
    ls -la "$PAYLOAD_DIR/"

    echo ""
    echo "[*] Downloading WebView2 bootstrapper..."
    WV2_BOOTSTRAP="$STUB_BUILD/MicrosoftEdgeWebview2Setup.exe"
    if [ ! -f "$WV2_BOOTSTRAP" ]; then
        curl -sL "https://go.microsoft.com/fwlink/p/?LinkId=2124703" -o "$WV2_BOOTSTRAP"
        echo "  -> Downloaded"
    else
        echo "  -> Already exists, skipping"
    fi

    cp "$INSTALLER_BIN" "$STUB_BUILD/jentlememes-installer.exe"

    echo ""
    echo "[*] Compiling NSIS installer..."
    makensis "$STUB_BUILD/stub.nsi"

    SETUP_EXE="$DIST_DIR/JentleMemes-Launcher-Setup.exe"
    if [ -f "$SETUP_EXE" ]; then
        SIZE=$(du -h "$SETUP_EXE" | cut -f1)
        echo ""
        echo "  SETUP READY: $SETUP_EXE ($SIZE)"
    else
        echo "ERROR: Setup file was not created"
        exit 1
    fi
}

# ─────────────────────────────────────────────
#  Dispatch
# ─────────────────────────────────────────────
case "$MODE" in
    app)
        build_launcher
        package_app
        ;;
    setup)
        build_launcher
        build_setup
        ;;
    all|"")
        build_launcher
        package_app
        build_setup
        ;;
    *)
        echo "Unknown mode: $MODE"
        echo "Usage: $0 [app|setup|all]"
        exit 1
        ;;
esac

echo ""
echo "========================================="
echo " BUILD COMPLETE!"
echo " Output: $DIST_DIR/"
ls -lh "$DIST_DIR/"*.exe 2>/dev/null || true
echo "========================================="
