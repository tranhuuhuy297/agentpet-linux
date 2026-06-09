#!/usr/bin/env bash
# Install or uninstall AgentPet for the current user under ~/.local. No root
# (sudo is used only to pull the GTK4 runtime/dev libs when they are missing).
#
#   ./install.sh              auto: build from source when run inside the cloned
#                             repo (cargo present), else download the release
#   ./install.sh --source     force a source build (needs the cloned repo)
#   ./install.sh --binary     force the prebuilt release download
#   ./install.sh uninstall    remove hooks/autostart + installed files
#
# Works piped straight from the web (no clone needed) — auto picks the binary:
#   curl -fsSL https://raw.githubusercontent.com/tranhuuhuy297/agentpet-linux/main/install.sh | bash
#
# Uninstall also wipes ~/.agentpet (socket, queue, downloaded pets); pass
# --keep-data to preserve it.
set -euo pipefail

REPO="tranhuuhuy297/agentpet-linux"
ASSET_MATCH="x86_64-unknown-linux-gnu.tar.gz"
RAW="https://raw.githubusercontent.com/$REPO/main"
# Directory the script lives in (".") when piped via curl | bash.
SRC_DIR="$(cd "$(dirname "$0")" 2>/dev/null && pwd || echo .)"

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"
ICON_DIR="$PREFIX/share/icons/hicolor/512x512/apps"

BIN="$BIN_DIR/agentpet"
# The desktop file basename MUST equal the GTK application_id so GNOME maps the
# running windows (pet/monitor/settings) to this entry and shows the real icon
# in the dock/alt-tab — not just the app-menu launcher.
DESKTOP="$APP_DIR/io.github.tranhuuhuy297.agentpet.desktop"
ICON="$ICON_DIR/agentpet.png"
# Pre-otter installs put an svg here; cleaned up on install/uninstall.
LEGACY_ICON="$PREFIX/share/icons/hicolor/scalable/apps/agentpet.svg"
# Earlier installs named the desktop file plainly; remove it so there is no
# duplicate launcher after the rename to match the application_id.
LEGACY_DESKTOP="$APP_DIR/agentpet.desktop"

refresh_caches() {
  command -v update-desktop-database >/dev/null && update-desktop-database "$APP_DIR" 2>/dev/null || true
  # A per-user hicolor dir has no index.theme, so plain gtk-update-icon-cache
  # fails ("No theme index file") and leaves the previous cache in place — a
  # stale cache then shadows the freshly-installed PNG and the dock falls back to
  # a generic icon. --ignore-theme-index builds the cache without an index; if
  # even that fails, drop any stale cache and bump the dir mtime so GTK/GNOME
  # re-scan the directory and pick up the icon directly.
  local hicolor="$PREFIX/share/icons/hicolor"
  if command -v gtk-update-icon-cache >/dev/null; then
    gtk-update-icon-cache -q -f -t "$hicolor" 2>/dev/null \
      || { rm -f "$hicolor/icon-theme.cache" 2>/dev/null; touch "$hicolor" 2>/dev/null; }
  fi
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

# Prebuilt binary needs the GTK4 runtime (not bundled). Install it only when
# missing, and only on apt systems.
ensure_runtime() {
  # ldconfig usually lives in /usr/sbin, which isn't on a normal user's PATH.
  local ldc; ldc="$(command -v ldconfig || true)"
  if [ -z "$ldc" ]; then
    for d in /usr/sbin /sbin; do [ -x "$d/ldconfig" ] && ldc="$d/ldconfig" && break; done
  fi
  if [ -n "$ldc" ] && "$ldc" -p 2>/dev/null | grep -q 'libgtk-4\.so'; then
    return 0
  fi
  # Fallback before assuming it's missing: look for the shared object directly.
  if ls /usr/lib/*/libgtk-4.so* /usr/lib/libgtk-4.so* /lib/*/libgtk-4.so* >/dev/null 2>&1; then
    return 0
  fi
  if command -v apt-get >/dev/null; then
    echo "==> Installing GTK4 runtime (sudo)…"
    sudo apt-get update && sudo apt-get install -y libgtk-4-1 \
      || echo "WARNING: couldn't install the GTK4 runtime; install libgtk-4-1 manually."
  else
    echo "WARNING: GTK4 runtime (libgtk-4) not found — install it so AgentPet can run."
  fi
}

# Source build needs the GTK4/X11/ALSA dev libs. Install any missing on apt
# systems; elsewhere list them and let the build surface the error.
ensure_build_deps() {
  if command -v pkg-config >/dev/null && pkg-config --exists gtk4 2>/dev/null; then
    return 0
  fi
  local pkgs="libgtk-4-dev libadwaita-1-dev build-essential pkg-config libasound2-dev libx11-dev libxcb1-dev"
  if command -v apt-get >/dev/null; then
    echo "==> Installing build dependencies (sudo)…"
    sudo apt-get update && sudo apt-get install -y $pkgs
  else
    echo "WARNING: GTK4 dev libraries not found and this isn't an apt-based system."
    echo "         Install the equivalents of: $pkgs"
  fi
}

# Install the desktop entry + icon from the local repo when present, else fetch
# them from the repo (so curl | bash with no checkout still gets them).
install_assets() {
  if [ -f "$SRC_DIR/assets/io.github.tranhuuhuy297.agentpet.desktop" ]; then
    install -Dm644 "$SRC_DIR/assets/io.github.tranhuuhuy297.agentpet.desktop" "$DESKTOP"
    install -Dm644 "$SRC_DIR/assets/agentpet.png" "$ICON"
  else
    local tmp; tmp="$(mktemp -d)"
    curl -fsSL "$RAW/assets/io.github.tranhuuhuy297.agentpet.desktop" -o "$tmp/d" 2>/dev/null && install -Dm644 "$tmp/d" "$DESKTOP" || true
    curl -fsSL "$RAW/assets/agentpet.png" -o "$tmp/i" 2>/dev/null && install -Dm644 "$tmp/i" "$ICON" || true
    rm -rf "$tmp"
  fi
}

# Place a freshly produced binary ($1) plus assets, restarting the app if it
# was running. Shared by the binary-download and source-build paths.
finalize_install() {
  local src="$1" was_running
  echo "==> Stopping any running AgentPet…"
  was_running="$(stop_running_app)"

  echo "==> Installing to $PREFIX"
  install -Dm755 "$src" "$BIN"
  install_assets
  rm -f "$LEGACY_ICON" "$LEGACY_DESKTOP"
  refresh_caches

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
  echo "Update later with: agentpet update.  Uninstall with: ./uninstall.sh"
}

# Default path: download the prebuilt binary from the latest release.
do_install_binary() {
  command -v curl >/dev/null || { echo "curl is required for the prebuilt install." >&2; exit 1; }
  ensure_runtime

  echo "==> Finding the latest AgentPet release…"
  local url tmp
  url="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
         | grep -o "https://[^\"]*$ASSET_MATCH" | head -1 || true)"
  if [ -z "$url" ]; then
    echo "ERROR: no prebuilt $ASSET_MATCH on the latest release." >&2
    echo "       Build from source instead: ./install.sh --source" >&2
    exit 1
  fi

  echo "==> Downloading $url"
  tmp="$(mktemp -d)"
  curl -fsSL "$url" -o "$tmp/agentpet.tar.gz"
  tar -xzf "$tmp/agentpet.tar.gz" -C "$tmp"
  finalize_install "$tmp/agentpet"
  rm -rf "$tmp"
}

# Opt-in path: build from the cloned repo.
do_install_source() {
  cd "$SRC_DIR"
  [ -f crates/agentpet/Cargo.toml ] || { echo "ERROR: --source must run inside the cloned repo." >&2; exit 1; }
  command -v cargo >/dev/null || { echo "ERROR: cargo (Rust) not found — install Rust or use --binary." >&2; exit 1; }
  ensure_build_deps
  echo "==> Building release binary…"
  cargo build --release -p agentpet
  finalize_install target/release/agentpet
}

# True when sitting in the AgentPet source tree — checked via a repo-specific
# path, not just any stray Cargo.toml (curl | bash leaves SRC_DIR as ".").
in_source_tree() {
  [ -f "$SRC_DIR/crates/agentpet/Cargo.toml" ]
}

# Default path: build from source when in the checkout (so local changes ship),
# otherwise download the prebuilt release.
do_install_auto() {
  if in_source_tree && command -v cargo >/dev/null; then
    echo "==> Local source checkout detected — building from source."
    echo "    (use ./install.sh --binary for the prebuilt release instead)"
    do_install_source
  else
    do_install_binary
  fi
}

do_uninstall() {
  local keep_data="${1:-}"
  cd "$SRC_DIR"

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
    echo "    (agentpet binary not found — skipping hook cleanup)"
  fi

  echo "==> Removing installed files…"
  rm -f "$BIN" "$DESKTOP" "$ICON" "$LEGACY_ICON" "$LEGACY_DESKTOP"
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
  --source|source)  do_install_source ;;
  --binary|binary)  do_install_binary ;;
  install|"")       do_install_auto ;;
  *) echo "usage: $0 [install | --source | --binary | uninstall [--keep-data]]" >&2; exit 2 ;;
esac
