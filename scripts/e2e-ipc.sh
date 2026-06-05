#!/usr/bin/env bash
# End-to-end smoke test for the Phase 2 IPC daemon + hook CLI + run wrapper.
# Uses an isolated HOME so it never touches the real ~/.agentpet.
set -euo pipefail

BIN="${1:-./target/debug/agentpet}"
SANDBOX="$(mktemp -d)"
export HOME="$SANDBOX"
trap 'kill "${DAEMON_PID:-0}" 2>/dev/null || true; rm -rf "$SANDBOX"' EXIT

echo "== sandbox HOME=$SANDBOX =="

echo
echo "== 1. queue fallback while daemon is DOWN =="
"$BIN" hook --agent claude --event UserPromptSubmit --session t1 --project /tmp/projA
ls "$SANDBOX/.agentpet/queue/" && echo "  -> event queued to disk (daemon down): OK"

echo
echo "== 2. start daemon (drains queue on startup) =="
"$BIN" > "$SANDBOX/daemon.out" 2> "$SANDBOX/daemon.err" &
DAEMON_PID=$!
sleep 1
echo "  daemon stderr:"; sed 's/^/    /' "$SANDBOX/daemon.err"
grep -q "projA" "$SANDBOX/daemon.err" && echo "  -> queued t1 (working) drained: OK"

echo
echo "== 3. live event over the socket: t1 -> done =="
"$BIN" hook --agent claude --event Stop --session t1 --project /tmp/projA
sleep 1
tail -n 4 "$SANDBOX/daemon.err" | sed 's/^/    /'
grep -q "done" "$SANDBOX/daemon.err" && echo "  -> live Stop delivered (done): OK"

echo
echo "== 4. second agent + SessionEnd removes it =="
"$BIN" hook --agent codex --event UserPromptSubmit --session t2 --project /tmp/projB
sleep 0.5
"$BIN" hook --agent claude --event SessionStart --session t3 --project /tmp/projC
sleep 0.5
"$BIN" hook --agent claude --event SessionEnd --session t3 --project /tmp/projC
sleep 1
tail -n 6 "$SANDBOX/daemon.err" | sed 's/^/    /'

echo
echo "== 5. run wrapper: working -> done, exit code forwarded =="
set +e; "$BIN" run -- sh -c 'exit 7'; CODE=$?; set -e
echo "  wrapper exit code: $CODE (expected 7)"
sleep 1
tail -n 4 "$SANDBOX/daemon.err" | sed 's/^/    /'

echo
echo "== single-instance guard =="
"$BIN" 2> "$SANDBOX/second.err" || true
grep -q "already running" "$SANDBOX/second.err" && echo "  -> second daemon refused: OK"

echo
echo "ALL CHECKS DONE"
