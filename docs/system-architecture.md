# System Architecture

AgentPet watches AI coding-agent sessions (Claude Code, Codex) and reflects
their aggregate state through a desktop pet, a tray icon, a monitor window, and
notifications. Agents **push** state via a tiny hook CLI; AgentPet only
aggregates and displays — it never polls or reads agent logs.

## Workspace layout

```
crates/
  agentpet-core/   # pure, GTK-free, unit-tested domain logic
  agentpet/        # platform binary: CLI dispatch, tokio IPC daemon,
                   # GTK pet/monitor/settings, ksni tray, Petdex client
  pet-spike/       # Phase-0 click-through window feasibility spike
```

`agentpet-core` is deliberately free of GTK/X11 and of wall-clock reads in hot
paths (callers pass `now`), so the state machine, mappers, mood, hooks, and wire
format are deterministic and testable without a display.

## End-to-end flow

```
agent lifecycle event
  → `agentpet hook --agent <kind>`            (cli/hook.rs — fast path, no GTK)
      → 1 JSON line over Unix socket           (~/.agentpet/agentpet.sock)
         · daemon down? drop a file in          (~/.agentpet/queue/)
      → daemon (daemon/mod.rs)
          ├─ StateMapper: agent event → AgentState   (registered/working/waiting/done/idle)
          ├─ SessionStore: create/update, keyed by session_id
          ├─ prune timer (10s): demote/remove stale sessions
          └─ on change → snapshot to GTK thread
                ├─ MoodResolver → one pet mood
                ├─ tray (ksni): otter icon + count
                ├─ monitor window: per-session live timers
                └─ notifications + sound
```

### Hook CLI (push)
`agentpet hook` parses flags or reads the agent's hook-stdin JSON, builds one
`AgentEvent`, and writes it to the socket — **fire-and-forget** (no wait for the
daemon). If the socket is down it writes the event to the queue instead, so no
event is lost. The hook path links no GTK and spawns no tokio runtime, so it
starts instantly.

### Normalisation
`StateMapper` maps each agent's native event names to a shared `AgentState`:
`registered → working → waiting → done → idle`. `Notification` (Claude) and
`PermissionRequest` (Codex) map to **waiting** — the "needs your input" signal.
Session-end events remove the session immediately.

### Session store + pruning
`SessionStore` keys sessions by `session_id`. A 10s prune timer compensates for
agents that exit without a clean stop:
- `done` → `idle` after 30s
- `idle` removed after 600s
- `registered` removed after 90s
- `working`/`waiting` removed after 300s (`stale_active_after`)

### Queue (offline replay, bounded)
While the daemon is down, hook events land in `~/.agentpet/queue/` as
`<seconds>-<token>.json`. On startup `drain_queue` replays fresh files (original
timestamps) and deletes all of them. Files older than `QUEUE_MAX_AGE_SECS`
(300s, = the prune window) are deleted without replaying — they would be pruned
on apply anyway — which bounds the directory. `write_to_queue` also prunes
expired files as it writes.

### Mood aggregation — one pet
`MoodResolver::aggregate` reduces **all** sessions across **all** agents to a
single pet mood by priority: `working > waiting > done > idle`. There is exactly
one pet; per-agent/per-session detail lives in the monitor window and the tray
count.

## Hook installation
Enabling an agent in Settings writes a command into that agent's config
(`~/.claude/settings.json`, `~/.codex/hooks.json`) embedding the absolute binary
path. Entries are identified by the command containing `agentpet`+`hook`, so
install is idempotent and foreign hooks are never touched. Safety: a
confirmation dialog before the first write, a `<name>.bak` backup before
overwrite, and startup self-heal that rewrites a stale embedded path to the
current binary.

## Display-server strategy
The app forces `GDK_BACKEND=x11` so windows run under XWayland as keep-above
windows GNOME maps reliably (works on Ubuntu X11 and Wayland). The floating
pet's always-on-top / skip-taskbar / click-through bits are set via raw X11
(`x11rb`) on the window XID. The tray uses StatusNotifierItem (`ksni`) and needs
the GNOME AppIndicator extension.

## Install / update / release
- **Install:** `install.sh` downloads the prebuilt release binary by default
  (`curl … | bash` works); `--source` builds locally. It stops a running
  instance so the new binary takes the socket, then relaunches.
- **Update:** `agentpet update` self-updates from the latest GitHub Release
  (`self_update`, matching the target-triple tarball).
- **Release CI:** a `v*` tag triggers `release.yml` to build and publish the
  binary tarball; `deploy-pages.yml` publishes the `docs/` landing page.
