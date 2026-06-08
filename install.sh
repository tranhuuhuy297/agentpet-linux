#!/usr/bin/env bash
# Install or uninstall AgentPet for the current user under ~/.local. No root.
#
#   ./install.sh              build release + install (default)
#   ./install.sh uninstall    remove hooks/autostart + installed files
#
# Uninstall also wipes ~/.agentpet (socket, queue, downloaded pets). Pass
# --keep-data to preserve it. Prereq for install: the GTK4 dev packages (README).
set -euo pipefail
cd "$(dirname "$0")"

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"
ICON_DIR="$PREFIX/share/icons/hicolor/512x512/apps"

BIN="$BIN_DIR/agentpet"
DESKTOP="$APP_DIR/agentpet.desktop"
ICON="$ICON_DIR/agentpet.png"
# Pre-otter installs put an svg here; cleaned up on install/uninstall.
LEGACY_ICON="$PREFIX/share/icons/hicolor/scalable/apps/agentpet.svg"

refresh_caches() {
  command -v update-desktop-database >/dev/null && update-desktop-database "$APP_DIR" 2>/dev/null || true
  command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f "$PREFIX/share/icons/hicolor" 2>/dev/null || true
}

# Stop a running AgentPet app/daemon so the new binary can take over (the running
# one holds the Unix socket and the single-instance guard would reject the new
# launch). Only the GUI/daemon instance is killed; short-lived `hook`/`run`/
# `update` CLI invocations are left alone. Echoes "1" if something was stopped.
stop_running_app() {
  command -v pgrep >/dev/null || return 0
  local stopped=""
  local pid args
  for pid in $(pgrep -x agentpet 2>/dev/null); do
    args=$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null) || continue
    case "$args" in
      *" hook "*|*" run "*|*" uninstall"*|*" update"*) ;;  # leave CLI invocations
      *) kill "$pid" 2>/dev/null && stopped="1" ;;
    esac
  done
  echo "$stopped"
}

do_install() {
  echo "==> Building release binary…"
  cargo build --release -p agentpet

  echo "==> Stopping any running AgentPet…"
  local was_running
  was_running="$(stop_running_app)"

  echo "==> Installing to $PREFIX"
  install -Dm755 target/release/agentpet "$BIN"
  install -Dm644 assets/agentpet.desktop "$DESKTOP"
  install -Dm644 assets/agentpet.png "$ICON"
  rm -f "$LEGACY_ICON"
  refresh_caches

  # Relaunch the updated app if it had been running and we have a display.
  if [ -n "$was_running" ] && [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
    echo "==> Relaunching AgentPet…"
    nohup "$BIN" >/dev/null 2>&1 &
    disown 2>/dev/null || true
  fi

  echo
  echo "Installed:"
  printf '  %s\n  %s\n  %s\n\n' "$BIN" "$DESKTOP" "$ICON"
  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "NOTE: $BIN_DIR is not on your PATH — add it to run 'agentpet' directly." ;;
  esac
  echo "Launch from your app menu (AgentPet), or run: agentpet"
  echo "Tray icon needs the GNOME 'AppIndicator and KStatusNotifierItem Support' extension."
  echo "Uninstall later with: ./uninstall.sh"
}

do_uninstall() {
  local keep_data="${1:-}"

  echo "==> Stopping any running AgentPet…"
  stop_running_app >/dev/null

  # Remove the agent hooks + autostart the app wrote (inverse of the Settings
  # toggles). Prefer the installed binary; fall back to a freshly built one so
  # uninstall works even if the binary was never installed to ~/.local.
  echo "==> Removing agent hooks + autostart…"
  if [ -x "$BIN" ]; then
    "$BIN" uninstall || true
  elif [ -x target/release/agentpet ]; then
    target/release/agentpet uninstall || true
  else
    echo "    (agentpet binary not found — skipping hook cleanup; build it to clean hooks)"
  fi

  echo "==> Removing installed files…"
  rm -f "$BIN" "$DESKTOP" "$ICON" "$LEGACY_ICON"
  refresh_caches

  if [ "$keep_data" = "--keep-data" ]; then
    echo "    (kept ~/.agentpet)"
  else
    rm -rf "$HOME/.agentpet"
    echo "    removed ~/.agentpet"
  fi

  echo
  echo "AgentPet uninstalled."
}

case "${1:-install}" in
  uninstall|remove) do_uninstall "${2:-}" ;;
  install|"")       do_install ;;
  *) echo "usage: $0 [install | uninstall [--keep-data]]" >&2; exit 2 ;;
esac
