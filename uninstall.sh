#!/usr/bin/env bash
# Uninstall AgentPet for the current user. Thin wrapper so there's a discoverable
# uninstall command; the actual logic lives in install.sh (single source).
#
#   ./uninstall.sh                remove hooks/autostart + files + ~/.agentpet
#   ./uninstall.sh --keep-data    keep ~/.agentpet (queue + downloaded pets)
set -euo pipefail
exec "$(dirname "$0")/install.sh" uninstall "$@"
