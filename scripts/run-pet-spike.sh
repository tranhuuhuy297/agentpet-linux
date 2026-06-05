#!/usr/bin/env bash
# Build and run the Phase-0 pet-window feasibility spike.
#
# Prereq (one-time): install GTK4 + X11 dev packages:
#   sudo apt update && sudo apt install -y \
#     libgtk-4-dev build-essential pkg-config libx11-dev libxcb1-dev
#
# Usage:
#   scripts/run-pet-spike.sh                 # interactive draggable sprite
#   scripts/run-pet-spike.sh --click-through # input passes through to apps below
set -euo pipefail
cd "$(dirname "$0")/.."

# Keep build artifacts out of ./target so repo guardrails stay happy.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/agentpet-out}"

echo "Building pet-spike (needs libgtk-4-dev)…"
cargo build -p agentpet-pet-spike

echo "Launching… (Ctrl-C to quit)"
exec "$CARGO_TARGET_DIR/debug/pet-spike" "$@"
