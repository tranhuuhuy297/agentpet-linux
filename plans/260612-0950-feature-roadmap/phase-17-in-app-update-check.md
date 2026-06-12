# Phase 17 — In-App Update Check (About tab)

## Context Links
- Master plan: [plan.md](plan.md)
- CLI updater: `crates/agentpet/src/platform/update.rs:7-35` (`run`, `update`;
  `self_update::backends::github::Update`, repo `tranhuuhuy297/agentpet-linux`,
  bin `agentpet`, `show_download_progress(true)`)
- About tab: `crates/agentpet/src/ui/settings/about.rs:7-72` (`build`), current
  static "Check for updates → `agentpet update`" row at `:55`
- GUI async facility: `crates/agentpet/src/gui/mod.rs:79,89`
  (`glib::MainContext::default().spawn_local`)
- `self_update` dep: `crates/agentpet/Cargo.toml:38` (rustls + tar + flate2)

## Overview
- **Priority:** P2  **Status:** pending
- Replace the static About row with a working **Check for updates** button: async
  check (off the GTK main loop), show "up to date" or "vX.Y available" with an Update
  button that runs the existing update path, plus progress/result feedback and a
  "restart to apply" hint. Handle offline/network errors gracefully.

## Key Insights (verified)
- `update.rs` today is CLI-print-oriented: `update()` returns `Result<String,…>` and
  is built around `.update()` which **checks AND replaces** in one call
  (`update.rs:21-28`). For the UI we need a **separate check-only** step so we can show
  "vX.Y available" before downloading. `self_update`'s `Update` builder exposes
  `get_latest_release()` (check-only) distinct from `.update()` (replace). Refactor
  `update.rs` into reusable functions — do NOT duplicate the builder config (DRY).
- GUI async is `glib::MainContext::default().spawn_local` (`gui/mod.rs:79`). But
  `self_update` is **blocking/synchronous** (rustls + reqwest blocking). Calling it
  inside `spawn_local` would block the GTK main loop. Mitigation: run the blocking
  check/update on a worker thread (`std::thread::spawn` or `gio::spawn_blocking`) and
  deliver the result back to GTK via an `async_channel` the About page awaits in
  `spawn_local` — same thread-bridge pattern the app already uses (`gui/mod.rs:43-50`).
- `current_version` = `env!("CARGO_PKG_VERSION")` (`update.rs:25`), same as the About
  version label (`about.rs:23`) — reuse for the "you're on vX" line.
- Binary replacement integrity: `self_update` over `rustls` (HTTPS) verifies the TLS
  channel; it does **not** verify a GPG/sha signature of the asset unless configured.
  Note this as a security consideration; releases are fetched from the project's own
  GitHub over HTTPS.

## Requirements
- Functional: button checks latest release async; shows up-to-date / update-available
  with version; an Update action downloads+replaces via existing path; progress +
  success/failure feedback; restart hint on success; graceful offline/error message.
- Non-functional: never block GTK main loop; reuse `update.rs` (refactor, no dup);
  About-tab additions keep `about.rs` reasonable (extract a `update_row` module if it
  pushes the file past ~150 lines — `about.rs` is 110 now).

## Architecture
- Refactor `update.rs`:
  - `fn configure() -> Result<Update>`: shared builder (repo/bin/version/progress).
  - `fn check() -> Result<UpdateStatus>`: `configure()?.build()?.get_latest_release()`
    → returns `{ current, latest, newer: bool }` (a small struct, not a printed line).
  - `fn apply() -> Result<String>`: existing `.update()` path (current `update()` body).
  - `run()` (CLI) calls `apply()` and prints — unchanged behaviour.
- About page data flow: click "Check" → `gio::spawn_blocking(check)` (or thread) →
  result over `async_channel` → `spawn_local` awaits → update label + reveal "Update"
  button if `newer`. Click "Update" → `spawn_blocking(apply)` → result → show
  "Updated to vX — restart AgentPet" or error text.

## Related Code Files
- Modify: `platform/update.rs` (split into `configure`/`check`/`apply`; keep `run`),
  `ui/settings/about.rs` (replace static row `:55` with a stateful update widget).
- Create: optionally `ui/settings/about_update.rs` (snake_case) if the widget +
  async wiring would push `about.rs` over ~150 lines; declare `mod about_update;` in
  `settings/mod.rs`.
- Delete: none.

## Implementation Steps
1. `update.rs`: extract `configure()` returning the shared
   `self_update::backends::github::Update` builder; add `check()` returning a
   `pub struct UpdateInfo { current: String, latest: String, newer: bool }`; keep
   `apply()` = current `update()` body; `run()` calls `apply()`.
2. Verify `get_latest_release()` (or equivalent check-only API) exists on this
   `self_update` version; if not, fall back to `.update()`'s returned status semantics
   and gate the UI accordingly (`[UNVERIFIED]` — confirm against `self_update` 0.41 API).
3. About UI: replace the `Check for updates` link row with a row holding a "Check for
   updates" `Button`, a status `Label`, and a hidden "Update" `Button`.
4. Wire async: on Check click, disable button + show "Checking…", run `check()` on a
   blocking worker, send result over an `async_channel::bounded(1)`, await in a
   `spawn_local` task; update label; reveal Update button when `newer`.
5. On Update click: show "Downloading…", run `apply()` on a worker, await result;
   on success show "Updated to vX — restart AgentPet to apply", on error show the
   message. Never panic; map errors to a user string.
6. Offline handling: `check()`/`apply()` errors (DNS/timeout/HTTP) surface as
   "Couldn't reach update server — check your connection."

## Todo List
- [ ] Refactor `update.rs` into `configure`/`check`/`apply` (+ `UpdateInfo`)
- [ ] Confirm check-only API on self_update 0.41
- [ ] About-tab update widget (button/label/update-button)
- [ ] async worker→channel→spawn_local wiring (no main-loop block)
- [ ] success/restart hint + error/offline messaging
- [ ] extract `about_update.rs` if `about.rs` exceeds ~150 lines

## Success Criteria
- `cargo test -p agentpet-core -p agentpet` green; `cargo build --release` compiles.
- Clicking Check never freezes the window (verify UI stays responsive during the call).
- Up-to-date release → "You're on the latest (vX)". Newer release → "vY available" +
  Update button; running it replaces the binary and shows the restart hint.
- Offline → friendly error, no crash, button re-enabled.
- `agentpet update` CLI behaviour unchanged (still prints + exit code).

## Risk Assessment
- Blocking call freezing GTK (L:High if done naively, I:High) → mitigate: run on a
  worker thread/`spawn_blocking`, bridge via `async_channel` (verified app pattern).
- `self_update` lacking a check-only API (L:Med I:Med) → step 2 verifies; fallback to
  status-after-update semantics.
- Binary replace failing mid-write / permissions (L:Low I:High) → `self_update` writes
  to a temp + atomic rename; surface its error verbatim; user can fall back to CLI.

## Security Considerations
- Binary replacement: download over HTTPS (rustls) from the project's own GitHub repo
  (`update.rs:22-24`). TLS protects transport; there is **no signature/sha
  verification** of the asset unless `self_update` is configured for it — document this
  and consider adding checksum verification in a follow-up if releases publish one.
- The Update action mutates the running executable — confirm with the user (the button
  click is the confirmation) and clearly state a restart is required; never auto-restart.

## Next Steps
- Optional: periodic background check on startup with a subtle "update available"
  badge (respect a config opt-out) — separate phase.

## Open Questions
- Exact check-only API name on `self_update` 0.41 (`get_latest_release` vs other);
  verify before implementing the check path.
- Does the GitHub release publish a checksum we can verify post-download? If so, wire
  it for integrity rather than trusting TLS alone.
