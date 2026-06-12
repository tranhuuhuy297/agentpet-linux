# Phase 16 — Tray Count Toggle (wire `show_count`)

## Context Links
- Master plan: [plan.md](plan.md)
- Config field: `crates/agentpet-core/src/config.rs:22` (`show_count`, default `true`)
- Tray: `crates/agentpet/src/ui/tray.rs:48-95` (`AgentTray` struct, `tool_tip`,
  `icon_name`, `icon_pixmap`)
- Live tray update: `crates/agentpet/src/ui/mod.rs:62-68` (`apply` → `tray.update(|t|
  { t.running; t.waiting })`)
- Update flow source: `crates/agentpet/src/snapshot.rs:18-29` (`UiUpdate.running/
  .waiting`), `crates/agentpet/src/daemon/mod.rs:151-156` (`emit`)
- General tab: `crates/agentpet/src/ui/settings/general.rs:15-54`

## Overview
- **Priority:** P3  **Status:** pending — small, tight.
- `show_count` exists in config but the tray ignores it — the tooltip always reports
  counts (`tray.rs:81-95`). Wire `show_count`: add a General-tab switch and have the
  tray respect it live.

## Key Insights (verified)
- The tray currently surfaces counts ONLY in the tooltip text (`tray.rs:82-88`:
  "N agent(s) working/need input"). There is no numeric badge on the icon (SNI/ksni
  has no count badge here) and the icon only switches to a warning glyph when
  `waiting > 0` (`tray.rs:66-79`). So `show_count` should govern **whether the tooltip
  exposes the numeric count** — when off, show a generic state ("AgentPet active" /
  "Agent needs input") without the number. The attention glyph stays (it's state, not
  count) — verify this interpretation with the user; it's the only count surface.
- Tray receives live updates via `tray.update(|t| …)` on the GTK thread
  (`ui/mod.rs:63-67`), driven by every `UiUpdate` from the daemon. To make the toggle
  live, the tray must hold a `show_count: bool` field that `apply` refreshes from
  `Config::load()` each update (cheap; config already loaded elsewhere per-update is
  acceptable, or pass it through `UiUpdate`). KISS: read `Config::load().show_count`
  inside `Ui::apply` and push it via the same `tray.update` closure.
- `ksni::blocking::Handle::update` (ksni 0.3.4 `blocking.rs:206`) takes
  `FnOnce(&mut T)` and triggers a property refresh — exactly how `running`/`waiting`
  already flow, so adding a `show_count` field needs no new mechanism.

## Requirements
- Functional: General-tab switch bound to `config.show_count`; tray tooltip honours it
  live (no restart). Default unchanged (`true` → counts shown).
- Non-functional: no new files; reuse the existing `tray.update` path (DRY).

## Architecture
- Data flow: user flips switch → `Config::load` → set `show_count` → `save`. Next
  `UiUpdate` (or an immediate nudge) → `Ui::apply` reads `Config::load().show_count`
  → `tray.update(|t| t.show_count = …)` → `tool_tip()` branches on it.
- Tray struct gains `show_count: bool` (init `true` in `spawn`, `tray.rs:138`).
  `tool_tip` builds description with vs. without the number based on it.

## Related Code Files
- Modify: `ui/tray.rs` (add `show_count` field; branch `tool_tip` description;
  init in `spawn`), `ui/mod.rs` (`apply` pushes `show_count` from `Config::load`),
  `ui/settings/general.rs` (add the General-tab switch in a small "Tray" group).
- Create: none.  Delete: none.

## Implementation Steps
1. `tray.rs`: add `pub show_count: bool` to `AgentTray`; init `true` in `spawn`
   (`:138`). In `tool_tip` (`:81`), when `!show_count` produce non-numeric strings
   ("Agent needs input" / "Agents active" / "No active agents").
2. `ui/mod.rs`: in `apply` (`:59`), read `let show_count = Config::load().show_count;`
   and set it inside the existing `tray.update` closure alongside `running`/`waiting`.
3. `general.rs`: add a boxed "Tray" group with a single switch "Show active count",
   `set_active(Config::load().show_count)`, `connect_state_set` → load/mutate/save
   (pattern from `pet_page.rs:312-316`). The toggle takes effect on the next snapshot;
   if instant feedback is wanted, no extra channel needed since updates arrive on
   activity — note that a fully-idle tray won't refresh until the next event (accept,
   or trigger a one-shot `tray.update` from the switch handler if the tray handle is
   reachable — it is not from Settings, so accept eventual refresh; KISS).
4. Add a unit-style assertion if feasible (tooltip formatting is on the tray struct;
   a small pure helper `tooltip_text(running, waiting, show_count) -> String` extracted
   from `tool_tip` is testable in `agentpet-core`-style — keep it in `tray.rs` with a
   `#[cfg(test)]` since it has no core deps).

## Todo List
- [ ] `AgentTray.show_count` field + init
- [ ] `tool_tip` branches on `show_count`
- [ ] `Ui::apply` pushes `show_count` from config
- [ ] General-tab "Tray" switch (load/save)
- [ ] tooltip-text helper + test

## Success Criteria
- `cargo test -p agentpet-core -p agentpet` green; `cargo build --release` compiles.
- Toggling the switch off then triggering agent activity: tray tooltip shows generic
  state without numbers; toggling on restores counts.
- Default (fresh config) shows counts exactly as today.

## Risk Assessment
- Toggle not refreshing a fully-idle tray immediately (L:Med I:Low) → documented:
  refresh on next snapshot; acceptable for a cosmetic tooltip. Optionally push a
  one-shot update if the tray handle becomes reachable from Settings (out of scope).
- Misreading intent of `show_count` (L:Med I:Med) → Open Question below; confirm the
  count surface (tooltip only) before implementing.

## Security Considerations
- None — reads/writes only the user's own config; no foreign files, no network.

## Next Steps
- If a numeric icon badge is later desired, that's a separate rendering phase (compose
  a count onto `tray_icons()` pixmaps) — out of scope here.

## Open Questions
- Confirm `show_count` is meant to govern the **tooltip count** (the only count
  surface today). The warning glyph for `waiting>0` is state, not count — keep it
  regardless. If the user expected an on-icon badge, that's a larger phase.
