# Changelog

All notable changes to AgentPet for Linux.

## 0.9.0 — 2026-06-09

### Readable pet state
- **The desktop pet now spells out its state.** Previously the only state cues
  were the pet pack's animation clip plus subtle speed/bob differences, so with a
  real Petdex pack "working", "waiting for input", and "done" looked nearly
  identical — and the caption showed only the agent name. The caption pill now
  renders a mood-coloured status dot, the agent name, and the state word (e.g.
  `● Claude Code · waiting`), reusing the Monitor's exact palette and wording so
  the pet and Monitor always agree. The caption font auto-shrinks to fit smaller
  pets instead of clipping at the window edge.
- **Landing page** hero pet caption gained the matching colour-coded status dot.

## 0.8.0 — 2026-06-09

### Pet on every workspace
- **Pets now follow you across virtual desktops.** A pet lived on the single
  workspace it spawned on, so switching workspaces made it disappear. Each pet
  window now sets `_NET_WM_STATE_STICKY` and `_NET_WM_DESKTOP = 0xFFFFFFFF`
  (all-desktops) alongside its existing keep-above / skip-taskbar traits, so it
  stays visible on whichever workspace you're on.

### Reopen the Monitor by clicking the app icon
- **Clicking the dock/launcher icon while AgentPet is running now opens the
  Monitor.** Previously a second launch just printed "already running" and
  exited, so the click did nothing (a problem when the tray isn't available). The
  second launch now sends a control frame over the daemon socket and the running
  instance surfaces its Monitor window, then exits.

### App grid + dock icon
- **AgentPet now appears in the Ubuntu app list with the otter icon, and the
  dock entry shows the otter too** — completing the icon work started in 0.7.0.
  Several install-side issues remained:
  - The desktop entry used `Exec=agentpet` (bare). gnome-shell's PATH doesn't
    include `~/.local/bin`, so it couldn't resolve the binary — which hid the
    entry from the app grid. `install.sh` now writes an absolute `Exec=` path.
  - The icon was named `agentpet`, mismatching the application id. It's now
    installed as `io.github.tranhuuhuy297.agentpet` (icon file, `Icon=` key, and
    desktop-file basename all share the reverse-DNS id), at 48–512 px.
  - The per-user `hicolor` dir had no `index.theme`; gnome-shell's stricter icon
    loader skips such a dir, so `Icon=` resolved to a generic gear even though
    GTK could find the PNG. `install.sh` now writes a minimal `index.theme`
    (512 dir marked Scalable so one icon serves small requests) and a flat
    `pixmaps` fallback, then force-rebuilds the icon cache.
  - Fixed `install_assets` aborting early under `set -e` (a trailing `&&` that
    returned non-zero on the source path), which had skipped the cache refresh,
    legacy cleanup, and app relaunch.

## 0.7.0 — 2026-06-09

### Single-instance guard
- **Only one AgentPet process can run at a time.** A second launch (e.g. the
  autostart entry firing at login while a pet is already open) used to build a
  duplicate tray icon and pet. Both the GUI and the headless daemon now take an
  exclusive, non-blocking advisory `flock` on `agentpet.lock` before any side
  effect and exit cleanly if it's already held — the kernel drops the lock on
  process exit or crash, so a dead instance never leaves it stuck. The GTK
  `activate` handler also guards against re-activation building the UI twice.

### Window icon (dock / alt-tab)
- **The otter shows in the dock and alt-tab immediately.** GTK4 dropped the
  per-window icon API and leaves the dock to resolve window → `.desktop` →
  `Icon=` through the theme cache, which only refreshes on relogin and shows
  nothing when run uninstalled (source checkout / AppImage). Each toplevel now
  stamps `_NET_WM_ICON` with the embedded otter pixels on map, sidestepping the
  theme/cache entirely. The desktop-file icon stays as the launcher fallback.

### Dock icon registration
- **Fixed the installer never refreshing the icon cache.** A per-user
  `~/.local/share/icons/hicolor` has no `index.theme`, so `gtk-update-icon-cache`
  failed with "No theme index file" and `install.sh` swallowed the error —
  leaving a stale cache that shadowed the freshly-installed otter PNG, so GNOME
  fell back to a generic dock/alt-tab icon. The refresh now force-rebuilds the
  cache (`gtk-update-icon-cache -q -f -t`) and, if even that fails, drops the
  stale cache and bumps the dir mtime so GTK/GNOME re-scan the directory and pick
  up the icon directly — so the otter app icon registers reliably. GNOME Shell caches the
  window→app→icon mapping in memory, so an existing session still needs a
  log-out/in to pick up the change.

### Pet size setting
- **A "Pet size" slider** (80–200 px) now sits at the top of the Settings → Pet
  tab. Dragging it resizes every live pet instantly and the choice persists in
  `config.json` (`pet_size`, default 110). The size is global across agents;
  newly-spawned pets read the saved value, so a restart keeps your size.
- Wired the previously-unused `pet_size` config field through to the pet window:
  `PetWindow` is now created at the configured size and resizes live via a new
  `UiCommand::ResizePets` (carries the px value, so the drag path touches no
  disk; the config write is debounced 250 ms after the last move).

## 0.6.0 — 2026-06-09

### Monitor row icons
- **Each monitor row now leads with two icons:** the coding agent's official
  mark (Claude Code's pixel creature, Codex's terminal-prompt cloud — embedded
  in the binary and drawn on a light rounded backing so the black Codex mark
  stays visible on dark themes; `run`-wrapped CLIs fall back to a ">_" monogram)
  and the agent's assigned pet sprite (first idle frame). The pet icon falls back
  to a state-coloured dot when no Petdex pack is installed.
- Pet icons are cached per agent kind (re-renders run every second for the live
  timers, so the spritesheet is sliced once, not per tick) and the cache is
  dropped on a pet-selection change so the new pet shows up immediately.

## 0.4.0 — 2026-06-08

### Pets from the Petdex CLI
- **Pets are now installed locally with the Petdex CLI**, not downloaded in-app.
  Run `npx petdex@latest install <slug>` (writes to `~/.petdex/pets/<slug>/`),
  then the Settings → Pet tab lists every installed pack and assigns one per
  agent. The tab shows the install command and a **Refresh** button, and re-scans
  on open so newly-installed pets appear without a restart.
- **Sprite thumbnails in the Pet tab.** Each row shows the pet's own sprite (first
  frame, cached per slug), falling back to a coloured blob only when a pack's
  spritesheet can't be decoded.
- **Link to the Petdex gallery** from the Pet tab's install guide, and the picked
  marker now shows the full agent name ("✓ Claude Code", not "✓ Claude").
- **Removed the in-app online gallery** (manifest fetch + downloads + the
  first-run starter-pet bootstrap) and the `reqwest` dependency with it; the app
  hosts no art and makes no network calls for pets. `petdex::scan_dir` reads the
  local pack directory and skips entries without a decodable `pet.json`.
- **WebP spritesheets now render.** Enabled the `image` crate's pure-Rust `webp`
  decoder — Petdex packs ship `spritesheet.webp`, which previously failed to slice
  and fell back to the coloured blob.

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
