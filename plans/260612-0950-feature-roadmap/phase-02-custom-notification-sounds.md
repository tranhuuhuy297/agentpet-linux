# Phase 02 — Custom Notification Sounds

## Context Links
- Master plan: [plan.md](plan.md)
- Config: `crates/agentpet-core/src/config.rs:46-49`
  (`sound_waiting_on`, `sound_done_on`, `sound_waiting_path`, `sound_done_path`)
- Notifications: `crates/agentpet/src/notify.rs` (worker thread + `on_transition`)
- Settings sound surface: currently none — add to General tab
  (`crates/agentpet/src/ui/settings/general.rs`).
- Related: phase-15 (per-agent notif toggles) touches the same `notify.rs` routing.

## Overview
- **Priority:** P2
- **Status:** pending
- `sound_waiting_path` / `sound_done_path` exist in config but `notify.rs` always
  passes the hardcoded freedesktop sound hints `"message"` / `"complete"`
  (`notify.rs:53,58`). Add Settings file-picker rows (GTK `FileDialog`) for each
  event sound, and make `notify.rs` use the file via notify-rust's `sound-file`
  hint when a valid path is set, falling back to the current named hints.

## Key Insights
- `notify.rs:16-20` `Note.sound` is `Option<&'static str>` (a named hint). A file
  path is `String`/owned, so the struct must carry an owned variant. Minimal change:
  change to `Option<Sound>` where `Sound = Named(&'static str) | File(String)`.
- notify-rust supports a `sound-file` hint via `Notification::hint(Hint::SoundFile(
  path))` (string absolute path). Confirm the installed notify-rust version exposes
  `Hint::SoundFile` — `[UNVERIFIED]` exact API; check `Cargo.lock`/docs before
  coding. If unavailable, set the raw hint `n.hint(Hint::Custom("sound-file", path))`
  per freedesktop spec.
- `on_transition` already calls `Config::load()` (`notify.rs:48`) — paths are read
  there; no new disk read added on the hot path beyond the existing one.
- File existence must be validated at send time, not just at pick time (user could
  delete/move the file later). On missing file, fall back to the named hint so the
  user still gets audible feedback.
- General tab is built as boxed groups (`general.rs:15-54`); add a "Sounds" group
  with two file-picker rows using the same `group_title`/`boxed`/row pattern.

## Requirements
**Functional**
- Two file-picker rows (Waiting sound, Done sound) on the General (or Pet) Settings
  tab. Each shows the current filename and a "Choose…"/"Clear" affordance.
- Picking a file persists its absolute path to `sound_waiting_path` /
  `sound_done_path`. Clear resets to `None`.
- `notify.rs`: if a path is set AND the file exists → play that file via the
  sound-file hint; else fall back to the existing named hint (`message`/`complete`).
- Respect the existing `sound_waiting_on` / `sound_done_on` on/off gates.

**Non-functional**
- No audio-playback dependency added — delegate to the notification daemon (matches
  the file-header rationale, `notify.rs:7-8`).
- File validation is a pure helper, unit-tested.

## Architecture
- **agentpet-core:** add `Config` helpers (pure, tested):
  `sound_for(&self, state: AgentState) -> Option<SoundChoice>` returning
  `File(path)` when on + path set + exists, else `Named("message"/"complete")` when
  on, else `None`. Existence check takes a path so tests inject a temp file. Keep
  the named-hint strings in core so notify.rs and tests agree. (Alternatively keep
  the resolver in notify.rs with a pure inner fn — KISS; choose core only if reused.)
- **agentpet (`notify.rs`):** replace `Note.sound: Option<&'static str>` with an
  owned `Sound` enum; in `on_transition` build it from the resolver; in the worker
  set the named hint OR the sound-file hint accordingly.
- **agentpet (Settings):** `FileDialog` (async) → on response, save path to config.
  Reuse `Config::load()` → mutate → `save()` (same pattern as `pet_page.rs:102-108`).
- **Data flow:** pick → config JSON → next `on_transition` reads config → resolver →
  daemon plays.

## Related Code Files
**Modify**
- `crates/agentpet/src/notify.rs` — `Sound` enum, resolver call, worker hint logic.
- `crates/agentpet/src/ui/settings/general.rs` — add "Sounds" group with two rows;
  OR add to `pet_page.rs` if General grows too large (keep file <250 lines).
- `crates/agentpet-core/src/config.rs` — add `sound_for` helper + tests (if placed
  in core).

**Create:** prefer none (extend general.rs); only add
`crates/agentpet/src/ui/settings/sound_rows.rs` if general.rs exceeds ~250 lines.

**Delete:** none.

## Implementation Steps
1. Confirm notify-rust's sound-file hint API from `Cargo.lock` + docs; pick
   `Hint::SoundFile` or `Hint::Custom("sound-file", path)`.
2. Add `Config::sound_for(state, path_exists_fn)` (pure) + unit tests in config.rs:
   path-set-and-exists → File; path-set-but-missing → Named fallback; off → None.
3. Refactor `notify.rs` `Note.sound` to an owned `Sound { Named(&'static str),
   File(String) }`; build via resolver in `on_transition`; apply hint in worker.
4. Add the "Sounds" group to general.rs: two rows, each a `Label` (filename) + a
   "Choose…" `Button` opening `FileDialog`, and a "Clear" `Button`.
5. Persist on selection (`Config::load → set → save`); update the row label live.
6. `cargo test -p agentpet-core -p agentpet`; `cargo build --release`.

## Todo List
- [ ] Verify notify-rust sound-file hint API
- [ ] `Config::sound_for` + tests (exists / missing-fallback / off)
- [ ] `Sound` enum + worker hint logic in notify.rs
- [ ] FileDialog rows in general.rs (choose + clear + live label)
- [ ] Persist paths to config
- [ ] Tests + release build pass

## Success Criteria
- `cargo test -p agentpet-core -p agentpet` passes (incl. resolver tests).
- `cargo build --release` compiles.
- Setting a real `.oga`/`.wav` and triggering a waiting/done transition plays that
  file; deleting the file then triggering falls back to the named hint (no crash).
- Clearing a path restores the default named-hint behaviour.

## Risk Assessment
- **notify-rust hint API mismatch (Med×Low):** verify before coding (step 1);
  fallback to raw `Custom("sound-file", …)` hint per spec.
- **Daemon ignores sound-file hint (Med×Med):** some daemons honour only named
  sounds. Acceptable degradation: user simply hears nothing extra; named-hint
  fallback covers the common case. Document in row subtext.
- **Stale/missing path (High×Low):** existence re-checked at send time → graceful
  fallback. Covered by a unit test.

## Security Considerations
- Path is user-chosen via the system file dialog; it is only handed to the
  notification daemon as a sound source — no shell, no exec. Validate it is a file
  (not a dir) before persisting.

## Next Steps
- Independent of phase-15 but shares `notify.rs`; sequence to avoid merge conflicts
  (do 02 before 15, or coordinate the `Note`/routing refactor).
