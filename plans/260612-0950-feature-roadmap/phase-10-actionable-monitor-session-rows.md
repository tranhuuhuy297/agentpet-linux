# Phase 10 — Actionable Monitor Session Rows

## Context Links
- Roadmap: `plans/260612-0950-feature-roadmap/plan.md`
- Monitor: `crates/agentpet/src/ui/monitor.rs`
- Snapshot/commands: `crates/agentpet/src/snapshot.rs`
- Command handling: `crates/agentpet/src/gui/mod.rs:86-100`
- Daemon store + socket: `crates/agentpet/src/daemon/mod.rs`
- IPC control-frame pattern: `crates/agentpet-core/src/ipc.rs:39-45`

## Overview
- **Priority:** P2
- **Status:** pending
- **Description:** Add per-row actions to monitor rows — Open project folder, Copy path, Dismiss session — while keeping rows glanceable. Reveal compact icon buttons on hover (`GtkRevealer` driven by a row `EventControllerMotion`).

## Key Insights (verified)
- Rows are built in `row()` at `monitor.rs:154`; the list is `SelectionMode::None` (`monitor.rs:51`). Full re-render every tick (`monitor.rs:102-107`, `render()` `:136`) — actions must survive a wipe-and-rebuild each second.
- `session.project` is the cwd (Claude `cwd` → event `project`, `payloads.rs:145`); it is `Option<String>` (`session.rs:17`). Hide "Open folder" when `None`.
- `gio` is already imported (`ui/mod.rs:20`) — use `gio::AppInfo::launch_default_for_uri` for folder open and GTK `Display::clipboard()` for copy. No new dep.
- UI has **no direct store access**: the daemon owns `SessionStore` behind `Arc<Mutex>` on the tokio thread (`daemon/mod.rs:52`); GTK talks to it only over the Unix socket. `UiCommand` (`snapshot.rs:33`) is routed to `gui/mod.rs` on the GTK thread, which also has no store handle. Therefore **Dismiss must travel over the socket to the daemon**, mirroring `CONTROL_SHOW_MONITOR` (`ipc.rs:39`).
- `SessionStore::remove(id)` already exists (`session.rs:98`).

## Requirements
- Functional: each row exposes Open folder (when project set), Copy path (when project set), Dismiss (always). Buttons hidden until hover; rows stay single-glance otherwise.
- Non-functional: no per-tick fl128 cost regression; no new heavyweight deps; path handling safe (no shell).

## Architecture
- **Open/Copy**: handled entirely in GTK (`monitor.rs`), need session.project string captured into the click closure.
- **Dismiss data flow**: row button → new `ipc` control frame `CONTROL_DISMISS` carrying the session id → sent over socket by a small helper in `monitor.rs` (connect to `ipc::socket_path()`, write frame). Daemon `handle_client` (`daemon/mod.rs:92`) parses it: `store.lock().remove(id)`, then `emit(&store,&sink)` so the row disappears reactively. Frame format: `b"\x00dismiss\t<id>\n"` (NUL guard + tab-delimited id), parsed by a new `ipc::dismiss_target(buf) -> Option<&str>`.

## Related Code Files
- **Modify:** `crates/agentpet-core/src/ipc.rs` (add `CONTROL_DISMISS_PREFIX`, `encode_dismiss(id)`, `dismiss_target(buf)`, with unit tests), `crates/agentpet/src/ui/monitor.rs` (row action buttons + revealer + send helper), `crates/agentpet/src/daemon/mod.rs` (`handle_client` dispatch on dismiss frame).
- **Create:** none (keep within existing files; if `monitor.rs` exceeds ~200 lines, extract row actions into `crates/agentpet/src/ui/monitor_row_actions.rs`).
- **Delete:** none.

## Implementation Steps
1. In `ipc.rs`: add `pub const CONTROL_DISMISS_PREFIX: &[u8] = b"\x00dismiss\t";` plus `encode_dismiss(id: &str) -> Vec<u8>` and `dismiss_target(buf: &[u8]) -> Option<&str>` (strip prefix + trailing `\n`, reject if no NUL guard). Add unit tests covering round-trip, missing newline, non-dismiss buffers, and that `decode_lines` still ignores it.
2. In `daemon/mod.rs handle_client` (after the `is_show_monitor` check, `:100`): if `ipc::dismiss_target(&buf)` is `Some(id)`, `store.lock().unwrap().remove(id)`, set `changed`/`emit`, return.
3. In `monitor.rs`: extend `row()` to append a trailing `GtkBox` of icon `Button`s wrapped in a `Revealer` (`set_reveal_child(false)`). Add an `EventControllerMotion` on the row toggling reveal on enter/leave.
4. Open folder button: `set_icon_name("folder-open-symbolic")`; closure calls `gio::AppInfo::launch_default_for_uri(&format!("file://{}", encoded_path), None::<&gio::AppLaunchContext>)`. Build only when `s.project.is_some()`.
5. Copy path button: `set_icon_name("edit-copy-symbolic")`; closure writes the raw path to `WidgetExt::clipboard()` text. Build only when project set.
6. Dismiss button: `set_icon_name("window-close-symbolic")`, `add_css_class("flat")`; closure connects to `ipc::socket_path()` and writes `ipc::encode_dismiss(&id)`.
7. If `monitor.rs` crosses ~200 lines, move steps 3–6 builders into `monitor_row_actions.rs` and `pub(crate) use` them.

## Todo List
- [ ] `ipc.rs` dismiss frame helpers + tests
- [ ] daemon dismiss dispatch in `handle_client`
- [ ] row revealer + motion controller
- [ ] open-folder button (conditional)
- [ ] copy-path button (conditional)
- [ ] dismiss button + socket send helper
- [ ] split row actions file if over 200 lines

## Success Criteria
- Hovering a row reveals 1–3 icon buttons; leaving hides them.
- Open folder launches the file manager at the cwd; absent when project `None`.
- Copy path puts the cwd on the clipboard.
- Dismiss removes the session from the store and the row vanishes within one snapshot.
- `cargo test -p agentpet-core -p agentpet` passes (incl. new ipc tests).
- `cargo build --release -p agentpet` compiles clean.

## Risk Assessment
- **Revealer state lost on per-tick rebuild** (Med/Low): acceptable — hover re-triggers reveal; do not persist reveal state across renders.
- **Stale id race** (Low): dismissing an already-pruned id is a no-op (`remove` on missing key).
- **Socket not bound** (Low): write is best-effort like `signal_show_monitor` (`gui/mod.rs:112`); failure leaves the row, user retries.

## Security Considerations
- **Path → URI**: percent-encode the path before `file://` to avoid malformed/injection URIs; never pass project through a shell. `launch_default_for_uri` spawns the registered handler, no shell interpolation.
- Dismiss id is opaque and only used as a HashMap key (`session.rs:99`); no path/exec use.

## Next Steps
- Independent of phases 11–13. Pairs well with phase-13 (empty state appears once the last session is dismissed).
