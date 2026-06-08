# Changelog

All notable changes to AgentPet for Linux.

## 0.2.0 — 2026-06-08

### Agent integration
- **Focus on Claude Code and Codex only.** Removed Gemini CLI, Cursor, opencode,
  and Windsurf. Both remaining agents share the same nested `{"hooks": {...}}`
  config shape, so the flat (Cursor/Windsurf) and JS-plugin (opencode) hook
  styles and their payload decoders were removed too.

### Hook-config safety
- **Confirmation dialog** the first time an agent is enabled in Settings, naming
  the exact file AgentPet will edit. Cancel writes nothing and reverts the
  switch.
- **Backup before overwrite.** `write_settings` snapshots an existing agent
  config to `<name>.bak` before clobbering it (covers install and uninstall).
- **Self-heal binary path.** On startup, a hook whose embedded binary path no
  longer matches the running binary is rewritten to the current path (idempotent
  — no rewrite when already correct, so no needless `.bak` churn).

### Event queue
- **Bounded queue.** Events queued while the daemon is down are dropped once
  older than the prune window (`QUEUE_MAX_AGE_SECS = 300s`, matching
  `stale_active_after`). The queue can no longer grow without bound when the app
  stays closed; recent events (incl. "waiting for input") still replay on the
  next start.

### Install / packaging
- **One-command install.** `install.sh` defaults to downloading the prebuilt
  release binary (no clone, no Rust toolchain); `curl … | bash` is supported.
  `./install.sh --source` builds from source instead. Desktop entry + icon come
  from the local repo, or are fetched from raw when run standalone.
- **Restart on (un)install.** `install.sh` stops a running AgentPet (leaving
  `hook`/`run`/`update` CLI invocations alone) so the new binary takes over the
  socket, and relaunches it after install.
- GTK4 runtime is auto-installed (apt, sudo only when missing); source builds
  auto-install the dev libraries.

### UI
- Otter line-art tray icon (pre-rendered pixmap, scaled to SNI sizes).
- Near-idle CPU: the pet advances/redraws only when its visible output changes;
  the monitor ticks timers only while the window is visible.

### Release / CI
- `release.yml` builds the release binary and publishes a GitHub Release with a
  target-triple tarball (`agentpet-<tag>-x86_64-unknown-linux-gnu.tar.gz`) that
  `agentpet update` and the installer consume.
- GitHub Pages landing page (`docs/`) deploys on push via `deploy-pages.yml`.
- Workflow JS actions pinned to Node 24.

### Notes
- Prebuilt binaries are **x86_64** only; arm64 users build with
  `./install.sh --source`.
