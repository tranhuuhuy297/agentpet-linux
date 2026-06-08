# Changelog

All notable changes to AgentPet for Linux.

## 0.3.0 — 2026-06-08

### Per-agent pets
- **One pet per agent kind.** The single aggregate pet is replaced by a floating
  pet per active agent: run Claude Code and Codex at once and you get two pets,
  each reflecting its own state (e.g. Claude "working" while Codex "waiting") —
  the old single pet collapsed both into one mood. `MoodResolver::aggregate_by_kind`
  groups sessions by `AgentKind` and reduces each group on its own.
- **Visible only while live.** A pet appears when its agent has a working/waiting/
  finishing session and disappears once that agent goes idle or ends, so an idle
  desktop shows no pets. Pets sit in stable per-kind slots to avoid overlap.
- **Per-agent pet packs.** Each agent can use a different pet. The Settings → Pet
  tab gains an agent selector; choosing a pet assigns it to that agent
  (`config.agent_pet_ids`), falling back to the global `selected_pet_id` default.
  Installing a pet for one agent no longer changes another agent's pet.

### Install / packaging
- **`install.sh` auto-detects source vs. binary.** Run inside the cloned repo
  (with `cargo` present) it now builds from source automatically — so a plain
  `./install.sh` ships local changes; `curl … | bash` still grabs the prebuilt
  release. Force either mode with `--source` / `--binary`.

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
