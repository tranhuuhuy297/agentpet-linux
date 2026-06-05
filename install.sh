#!/usr/bin/env bash
# Build AgentPet (release) and install it for the current user under ~/.local.
# No root required. Prereq: the GTK4 dev packages (see README).
set -euo pipefail
cd "$(dirname "$0")/.."

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"
ICON_DIR="$PREFIX/share/icons/hicolor/scalable/apps"

echo "==> Building release binary…"
cargo build --release -p agentpet

echo "==> Installing to $PREFIX"
install -Dm755 target/release/agentpet "$BIN_DIR/agentpet"
install -Dm644 assets/agentpet.desktop "$APP_DIR/agentpet.desktop"
install -Dm644 assets/agentpet.svg "$ICON_DIR/agentpet.svg"

# Refresh desktop/icon caches (best-effort).
command -v update-desktop-database >/dev/null && update-desktop-database "$APP_DIR" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f "$PREFIX/share/icons/hicolor" 2>/dev/null || true

echo
echo "Installed:"
echo "  $BIN_DIR/agentpet"
echo "  $APP_DIR/agentpet.desktop"
echo "  $ICON_DIR/agentpet.svg"
echo
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "NOTE: $BIN_DIR is not on your PATH — add it to run 'agentpet' directly." ;;
esac
echo "Launch from your app menu (AgentPet), or run: agentpet"
echo "Tray icon needs the GNOME 'AppIndicator and KStatusNotifierItem Support' extension."
