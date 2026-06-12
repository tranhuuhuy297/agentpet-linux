# Phase 15 — Per-Agent Notification Toggles

## Context Links
- Master plan: [plan.md](plan.md)
- Global notify flags: `crates/agentpet-core/src/config.rs:38` (`notifications_enabled`),
  `:47-48` (`sound_waiting_on`, `sound_done_on`)
- Transition firing: `crates/agentpet/src/notify.rs:44-62` (`on_transition`, loads
  `Config`, fires on `Waiting`/`Done`)
- AgentKind: `crates/agentpet-core/src/state.rs:60-87`
- General tab: `crates/agentpet/src/ui/settings/general.rs:37-42` (catalog rows)
- Session carries `agent_kind`: `crates/agentpet-core/src/session.rs` (verify field;
  used at `notify.rs` test `session()`)

## Overview
- **Priority:** P3  **Status:** pending
- Notifications are global today. Add per-agent-kind overrides: each agent can mute
  "notify on waiting" and/or "notify on done" independently, defaulting ON (no
  behaviour change for existing users). Filtering is pure → `agentpet-core` with
  tests; `notify.rs` consults it.

## Key Insights (verified)
- `on_transition` (`notify.rs:44`) already has `session.state` and the full
  `AgentSession` (which carries `agent_kind`) — it loads `Config` fresh each call
  (`:48`), so a new config field is read with zero plumbing changes.
- Existing global `sound_*_on` gate only the *sound hint*, not whether a notification
  fires (`notify.rs:53,58`). The new per-agent toggles gate whether the notification
  is **posted at all**. Keep both: per-agent toggle = post-or-not; global
  `sound_*_on` = sound-or-silent. (Verify whether `notifications_enabled` is even
  consulted — grep shows it is NOT read in `notify.rs`; flag below.)
- Config uses `#[serde(default)]` (`config.rs:19`) so adding a `HashMap` field is
  backward-compatible: old config files load with the field defaulted (empty map →
  treated as all-on).

## Requirements
- Functional: per agent-kind, toggle "notify on done" and "notify on waiting".
  Absent/empty = both on. `notify.rs` skips posting when the relevant toggle is off.
- Non-functional: pure `should_notify(kind, state, &config) -> bool` in
  `agentpet-core` with unit tests; `notify.rs` calls it; UI on General tab; no file
  >200 lines; default-on preserves current behaviour.

## Architecture
- Config field: `agent_notify: HashMap<String, AgentNotifyPrefs>` keyed by
  `AgentKind::raw()`, where `AgentNotifyPrefs { waiting: bool, done: bool }` defaults
  both `true`. Mirrors existing keyed maps (`agent_pet_ids` at `config.rs:30`).
- Pure fn `Config::should_notify(&self, kind, state) -> bool`: looks up prefs (default
  all-on), returns the matching flag for `Waiting`/`Done`, `false` for other states.
- `notify.rs:on_transition` early-returns when `!cfg.should_notify(session.agent_kind,
  session.state)` before building the `Note`.
- UI: a small "Notifications" boxed group on the General tab with one row per
  catalog agent, each row holding two switches (Waiting / Done). KISS: reuse the
  boxed-row pattern from `general.rs` rather than libadwaita expanders.

## Related Code Files
- Modify: `config.rs` (add `AgentNotifyPrefs` struct + `agent_notify` field +
  `Default` + `should_notify` + tests), `notify.rs` (consult `should_notify`),
  `ui/settings/general.rs` (Notifications group with per-agent switches).
- Create: none (keep `AgentNotifyPrefs` in `config.rs` — small, cohesive).
- Delete: none.

## Implementation Steps
1. `config.rs`: add `#[derive(Serialize,Deserialize,Clone,PartialEq,Debug)] struct
   AgentNotifyPrefs { #[serde(default=on)] waiting: bool, done: bool }` with both
   defaulting true; add `agent_notify: HashMap<String, AgentNotifyPrefs>` field +
   `Default` (empty map).
2. `config.rs`: add `should_notify(&self, kind: AgentKind, state: AgentState) -> bool`
   — match `Waiting`→prefs.waiting, `Done`→prefs.done, else `false`; missing key ⇒
   defaults (true). Add setter `set_agent_notify(kind, waiting, done)`.
3. `config.rs` tests: default all-on; muting Done for Claude leaves Waiting + other
   kinds on; round-trip through disk; old config (no field) loads all-on.
4. `notify.rs`: in `on_transition`, after computing `cfg`, early-return when
   `!cfg.should_notify(session.agent_kind, session.state)` for `Waiting`/`Done`.
5. `general.rs`: add a "Notifications" boxed group, one row per `AgentCatalog::all()`
   agent, two `Switch`es (Waiting/Done) wired to load/save `Config` via
   `set_agent_notify` (same load→mutate→save pattern as `pet_page.rs:312-316`).
6. Decide on `notifications_enabled`: if currently unused, either wire it as a master
   switch here or leave untouched (note decision in plan — do not silently change).

## Todo List
- [ ] `AgentNotifyPrefs` + `agent_notify` field + Default
- [ ] `should_notify` + `set_agent_notify`
- [ ] config tests (default-on, mute, roundtrip, legacy)
- [ ] notify.rs early-return
- [ ] General-tab Notifications group with per-agent switches
- [ ] resolve `notifications_enabled` master-switch question

## Success Criteria
- `cargo test -p agentpet-core -p agentpet` green; `cargo build --release` compiles.
- Default config: notifications fire exactly as today (no regression).
- Muting "Done" for Claude: Claude done is silent; Claude waiting + Codex unaffected.
- Config round-trips the new field; a pre-existing config file with no field loads
  with all toggles on.

## Risk Assessment
- Silent regression to global behaviour (L:Low I:High) → mitigate: default-on +
  explicit legacy-config test.
- Two-switch rows crowding the General tab (L:Med I:Low) → group under a clear
  "Notifications" header; consider collapsing into a future expander if it grows.

## Security Considerations
- None beyond writing the user's own `~/.config/agentpet/config.json` (already done
  elsewhere). No foreign files touched.

## Next Steps
- Optional later: per-project (not just per-kind) muting; surface a "mute all" master.

## Open Questions
- Is `config.notifications_enabled` consulted anywhere? Grep showed it absent from
  `notify.rs`; confirm before deciding whether this phase wires it as a master switch
  or leaves it as a no-op field (do not change its semantics without user sign-off).
