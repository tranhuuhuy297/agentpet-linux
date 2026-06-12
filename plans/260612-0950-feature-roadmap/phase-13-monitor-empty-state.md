# Phase 13 — Monitor Empty State

## Context Links
- Roadmap: `plans/260612-0950-feature-roadmap/plan.md`
- Empty branch today: `crates/agentpet/src/ui/monitor.rs:140-147`
- Render/rebuild: `crates/agentpet/src/ui/monitor.rs:136-152`
- Root layout (list + footer): `crates/agentpet/src/ui/monitor.rs:88-91`
- Settings open command: `crates/agentpet/src/ui/monitor.rs:72-77` (`UiCommand::OpenSettings`)
- Settings window: `crates/agentpet/src/ui/settings/mod.rs:21`

## Overview
- **Priority:** P3
- **Status:** pending
- **Description:** Replace the bare "No active agents" label with a friendly placeholder (icon, title, hint pointing at Settings → enable agents + Codex `/hooks` trust note, and an Open Settings button). Swap between list and empty state reactively as sessions appear/disappear.

## Key Insights (verified)
- An empty branch already exists in `render()` (`monitor.rs:140-147`) appending a dim "No active agents" label into the `ListBox`. This is the swap point.
- **No libadwaita dependency** — `Cargo.toml:26` declares only `gtk4 = "0.9"` with `v4_6`. So `adw::StatusPage` is unavailable; build the placeholder from plain GTK4 (`GtkBox` + `Image` + `Label` + `Button`). Do **not** add libadwaita (avoids a system dep, honors "no heavyweight new deps").
- The reactive swap already happens: `set_sessions` (`monitor.rs:113`) re-runs `render()` on every snapshot, and `render()` is also called on `show()` (`:122`) and the 1s tick (`:104`). Driving the empty state from inside `render()` means it appears/disappears automatically.
- Footer already wires `UiCommand::OpenSettings` (`monitor.rs:74`); reuse the same `cmd` sender for the empty-state button. `render()` currently has no `cmd` handle — pass one in or stash `cmd` on `MonitorWindow`.
- `MonitorWindow` holds `window`, `list`, `sessions`, `pet_icons` (`monitor.rs:32-37`); add a `cmd` field so `render()` can build a working button.

## Requirements
- Functional: when `sessions.is_empty()`, show centered icon + "No active agents" title + hint text (mentions Settings → enable agents, and the Codex `/hooks` trust step) + an "Open Settings" button that sends `OpenSettings`. When non-empty, show the list as today.
- Non-functional: plain GTK4 only; placeholder factory under 200 lines; no new deps.

## Architecture
- **Swap strategy (KISS):** keep the single `ScrolledWindow`+`ListBox`. In `render()`'s empty branch, instead of a dim label, append one centered placeholder `GtkBox` (icon `Image::from_icon_name("face-smile-symbolic")` or app icon, title `Label`, hint `Label` with `wrap`, `Button` "Open Settings"). The button closure clones the stored `cmd` and `try_send(UiCommand::OpenSettings)`.
- **Data flow:** snapshot → `set_sessions` → `render()` → empty? build placeholder : build rows. Button → `cmd` → `gui/mod.rs:93` → `ui.show_settings()` (`ui/mod.rs:55`).
- Alternative considered (a `GtkStack` swapping list vs page) rejected as YAGNI — re-rendering the list child already gives reactive swap for free.

## Related Code Files
- **Modify:** `crates/agentpet/src/ui/monitor.rs` (add `cmd` field to `MonitorWindow`; pass `cmd` into `render`; replace empty branch with placeholder factory `empty_state(cmd)`).
- **Create:** none (placeholder factory lives in `monitor.rs`; extract to `crates/agentpet/src/ui/monitor_empty_state.rs` only if `monitor.rs` exceeds ~200 lines after phase 10/11 edits).
- **Delete:** none.

## Implementation Steps
1. Add `cmd: async_channel::Sender<UiCommand>` to `MonitorWindow` (`monitor.rs:32`); store `cmd.clone()` in `new()` (`:40`) before it is moved into the footer closures.
2. Change `render` signature to take `cmd: &Sender<UiCommand>` (or capture via the struct); update its 3 call sites (`:104`, `:116`, `:122`) and the tick closure capture (`:99-101`).
3. Replace the empty branch (`:140-147`) with `list.append(&empty_state(cmd));`.
4. Implement `fn empty_state(cmd: &Sender<UiCommand>) -> GtkBox`: vertical box, centered (`set_valign(Center)`, `set_vexpand(true)`, margins), `Image::from_icon_name("face-smile-symbolic")` (pixel size ~48), bold title `Label` "No active agents", dim wrapped hint `Label` ("Enable an agent in Settings. For Codex, run `/hooks` and trust AgentPet."), and a `Button::with_label("Open Settings")` whose closure `try_send(UiCommand::OpenSettings)`.
5. Verify non-empty path unchanged (rows still render).

## Todo List
- [ ] add `cmd` field to `MonitorWindow`
- [ ] thread `cmd` through `render` + update call sites/tick closure
- [ ] `empty_state()` factory (icon/title/hint/button)
- [ ] wire Open Settings button to `OpenSettings`
- [ ] confirm reactive swap on snapshot empty/non-empty

## Success Criteria
- With zero sessions the monitor shows icon + title + hint + working Open Settings button.
- Clicking Open Settings presents the Settings window.
- Adding a session swaps to the list automatically (next snapshot); removing the last swaps back to the placeholder.
- `cargo test -p agentpet-core -p agentpet` passes.
- `cargo build --release -p agentpet` compiles clean.

## Risk Assessment
- **Render call-site/tick-closure breakage from new `render` arg** (Med): enumerate all 3 sites (`:104,:116,:122`) + the tick capture (`:99-101`); compile catches a miss.
- **Icon name missing on minimal themes** (Low): `face-smile-symbolic` is a standard freedesktop symbolic icon; fall back to the bundled app icon (`window_icon`) if needed.
- **Placeholder rebuilt every tick** (Low): same wipe-rebuild cost as the current label; negligible.

## Security Considerations
- No path handling, no IO, no external launch — purely UI. Hint text is static (no untrusted interpolation).

## Next Steps
- Independent; complements phase 10 (placeholder appears once the last session is dismissed). If phase 10 extracts `monitor_row_actions.rs`, consider co-locating `monitor_empty_state.rs` to keep `monitor.rs` under 200 lines.
