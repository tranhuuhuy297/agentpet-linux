#!/usr/bin/env bash
# Build a portable AppImage that bundles GTK4 + libadwaita, so AgentPet runs on
# older distros (e.g. Ubuntu 22.04, whose libadwaita is only 1.1) without dev
# packages installed.
#
# Requires (downloaded automatically if missing): linuxdeploy + the GTK plugin.
# Run on a machine with the GTK4 dev packages already installed.
set -euo pipefail
cd "$(dirname "$0")/.."

APP=AgentPet
TOOLS="${TOOLS_DIR:-/tmp/agentpet-appimage-tools}"
APPDIR="$(pwd)/AppDir"
mkdir -p "$TOOLS"

fetch() { # url dest
  [ -f "$2" ] || { echo "==> downloading $(basename "$2")"; curl -fsSL "$1" -o "$2"; chmod +x "$2"; }
}
LD="$TOOLS/linuxdeploy-x86_64.AppImage"
LD_GTK="$TOOLS/linuxdeploy-plugin-gtk.sh"
fetch "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage" "$LD"
fetch "https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh" "$LD_GTK"

echo "==> building release binary"
cargo build --release -p agentpet

rm -rf "$APPDIR"
echo "==> staging AppDir"
install -Dm755 target/release/agentpet "$APPDIR/usr/bin/agentpet"
install -Dm644 assets/agentpet.desktop "$APPDIR/usr/share/applications/agentpet.desktop"
install -Dm644 assets/agentpet.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/agentpet.svg"

# Force the X11 backend (the pet relies on XWayland) from inside the AppImage.
cat > "$APPDIR/apprun-hook.sh" <<'EOF'
export GDK_BACKEND=x11
EOF

echo "==> running linuxdeploy + gtk plugin"
export DEPLOY_GTK_VERSION=4
"$LD" --appdir "$APPDIR" \
  --plugin gtk \
  --custom-apprun "$APPDIR/apprun-hook.sh" \
  --desktop-file "$APPDIR/usr/share/applications/agentpet.desktop" \
  --icon-file "$APPDIR/usr/share/icons/hicolor/scalable/apps/agentpet.svg" \
  --output appimage

echo
echo "Built: $(ls -1 AgentPet*.AppImage 2>/dev/null || echo '(check linuxdeploy output)')"
echo "Verify on a clean Ubuntu 22.04 VM (no dev packages) before releasing."
