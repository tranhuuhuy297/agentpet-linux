# Phase 14 — More Agent Presets (Gemini CLI, OpenCode, Custom)

## Context Links
- Master plan: [plan.md](plan.md)
- Catalog: `crates/agentpet-core/src/catalog.rs:18-25` (`AgentCatalog::all`, hardcodes Claude+Codex)
- Hook spec + installer: `crates/agentpet-core/src/hooks.rs:24-46` (`AgentHooks::spec`),
  `:51-131` (Claude-nested `install`/`uninstall`/`is_installed`/`group_is_ours`)
- Disk IO + backup: `crates/agentpet-core/src/hooks.rs:134-231` (`*_to_disk`, `write_settings` `.bak`)
- Event mapping: `crates/agentpet-core/src/mapper.rs:11-44` (`is_session_end`, `state`)
- AgentKind enum: `crates/agentpet-core/src/state.rs:60-87` (`from_raw`, `raw`)
- General tab (catalog-driven rows): `crates/agentpet/src/ui/settings/general.rs:37-41`
- Research: [Gemini CLI hooks reference](https://geminicli.com/docs/hooks/reference/),
  [Gemini hooks docs](https://geminicli.com/docs/hooks/), [OpenCode plugins](https://opencode.ai/docs/plugins/)

## Overview
- **Priority:** P2  **Status:** pending
- Add catalog presets for **Gemini CLI** (fully — verified hook shape), **OpenCode**
  (stub/follow-up — different non-JSON plugin model), and a **Custom agent** row
  (docs-only, no config written). Each real preset needs: `AgentKind` variant,
  catalog entry, `mapper.rs` event→state row, `hooks.rs` spec, and General-tab row
  auto-appears (rows are catalog-driven at `general.rs:37`).

## Key Insights (verified)
- **Gemini CLI uses the SAME nested shape as Claude/Codex**: `{"hooks": {Event:
  [{"matcher"?, "hooks":[{"type":"command","command":...}]}]}}` in
  `~/.gemini/settings.json` (verified via reference docs). So the existing
  `HookInstaller::install/uninstall` (`hooks.rs:81-115`) works unchanged — no new
  writer needed. Gemini also reads `.gemini/settings.json` project-local, but we
  target the user file `~/.gemini/settings.json` (mirrors Claude `~/.claude`).
- **Gemini event names** (verified): `SessionStart`, `SessionEnd`, `BeforeTool`,
  `AfterTool`, `BeforeModel`, `Notification`, `AfterAgent`. Mapping:
  `SessionStart`→Registered, `BeforeModel`/`BeforeTool`→Working,
  `Notification`→Waiting, `AfterAgent`→Done; `SessionEnd`→`is_session_end`.
- `AgentHooks::spec` returns `None` for `Cli`/`Unknown` (`hooks.rs:43`) → the Custom
  row writes no config. `catalog.rs:33-41` test asserts every catalog agent has a
  hook spec, so a docs-only Custom row needs that test relaxed (see step 6).
- General-tab rows iterate `AgentCatalog::all()` and skip kinds with no spec
  (`general.rs:38` `let Some(spec) = … else { continue }`). A spec-less Custom row
  is silently dropped today — needs a dedicated non-toggle row variant (step 5).
- **OpenCode does NOT use a JSON hook file**: plugins are JS/TS modules exporting a
  hooks object reacting to events like `session.idle` (`[UNVERIFIED]` exact install
  path / `~/.config/opencode` plugin dir). This does not fit the JSON `HookInstaller`
  at all → **defer to follow-up**, ship as an unsupported catalog note only.

## Requirements
- Functional: Gemini CLI toggle installs/removes hooks in `~/.gemini/settings.json`,
  idempotent, foreign hooks preserved (inherited from `HookInstaller`). Gemini
  sessions report real state. Custom row documents `agentpet run -- <command>`.
- Non-functional: pure logic + unit tests in `agentpet-core`; no new files >200 lines;
  reuse `HookInstaller` (DRY) — do NOT fork a Gemini-specific writer.

## Architecture
- Data flow (Gemini): user flips General toggle → `confirm_then_install`
  (`general.rs:152`) → `HookInstaller::install_to_disk(cmd, ~/.gemini/settings.json,
  events)` → Gemini fires hook → CLI sends `AgentEvent{agent_kind: Gemini}` →
  `StateMapper::state(Gemini, …)` → `SessionStore`.
- Custom flow: catalog row with `is_supported:true`, no spec; General renders a
  docs-only row (no switch) showing `agentpet run -- <command>`; nothing written.

## Related Code Files
- Modify: `state.rs` (add `AgentKind::Gemini`; `from_raw`/`raw`), `catalog.rs`
  (Gemini + OpenCode-note + Custom rows; relax test), `hooks.rs` (`spec` Gemini arm),
  `mapper.rs` (Gemini `state` + `is_session_end` arms), `ui/settings/general.rs`
  (docs-only row branch for spec-less supported agents), `ui/mod.rs:184-191`
  (`kind_slot` Gemini), `gui/mod.rs:resync_agent_hooks` (heals all catalog kinds — verify it iterates catalog).
- Create: none (reuse existing).
- Delete: none.

## Implementation Steps
1. `state.rs`: add `AgentKind::Gemini` variant; extend `from_raw`/`raw` ("gemini");
   update `kind_roundtrips_through_raw` test.
2. `hooks.rs`: add `AgentKind::Gemini` arm to `spec` with the 5 events
   (`SessionStart`, `BeforeModel`, `BeforeTool`, `Notification`, `AfterAgent`) and
   `settings_path = ~/.gemini/settings.json`. Extend `disk_round_trip` test loop.
3. `mapper.rs`: add Gemini arm to `state` (mapping above) + `is_session_end`
   (`AfterAgent`? no — use `SessionEnd`); add `gemini_mapping` test.
4. `catalog.rs`: add Gemini `is_supported:true`; OpenCode `is_supported:false, note:
   Some("Plugin-based — coming soon")`; Custom `is_supported:true, note: Some("Run any
   command: agentpet run -- <command>")`.
5. `general.rs`: branch in the row loop — if `spec` is `None` but `is_supported`,
   render a docs-only row (no Switch) showing the note/command instead of `continue`.
6. `catalog.rs` test: relax `every_catalog_agent_has_a_hook_spec` to skip
   `Cli`/`Unknown`-kind docs-only rows (assert spec only for hook-backed kinds).
7. Verify `gui/mod.rs` `resync_agent_hooks` heals Gemini (re-grep its catalog loop).
8. Mark OpenCode real integration as explicit follow-up (separate future phase).

## Todo List
- [ ] `AgentKind::Gemini` + raw/from_raw + test
- [ ] `hooks.rs` Gemini spec + round-trip test
- [ ] `mapper.rs` Gemini state + session-end + test
- [ ] catalog: Gemini / OpenCode-note / Custom rows
- [ ] general.rs docs-only row branch
- [ ] relax catalog spec test
- [ ] verify resync + kind_slot cover Gemini
- [ ] OpenCode follow-up noted

## Success Criteria
- `cargo test -p agentpet-core -p agentpet` green (new mapper/hooks/catalog tests pass).
- `cargo build --release` compiles.
- General tab shows Gemini (toggle), OpenCode (greyed note), Custom (docs row).
- Toggling Gemini writes/removes only AgentPet entries in `~/.gemini/settings.json`;
  a pre-existing foreign hook survives (covered by inherited `HookInstaller` test).

## Risk Assessment
- Gemini hook shape drift (L:Low I:High) → mitigate: events table cited from official
  reference; if Gemini changes shape, only `spec`/`mapper` arms change.
- Custom row breaking the spec-asserting test (L:Med I:Low) → step 6 relaxes it.
- OpenCode scope creep (L:Med I:Med) → explicitly deferred; ships as note only.

## Security Considerations
- Editing a foreign config (`~/.gemini/settings.json`): reuse `write_settings`
  (`hooks.rs:151`) which backs up to `.bak` before clobber and only ever
  inserts/removes entries whose command matches `is_ours` (`hooks.rs:62`) — foreign
  hooks never touched. Confirmation dialog (`general.rs:152`) names the exact file.

## Next Steps
- Follow-up phase: real OpenCode plugin integration (write JS plugin to
  `~/.config/opencode/plugin/`? — verify dir before planning).

## Open Questions
- OpenCode plugin install path + exact event→state names (`[UNVERIFIED]`).
- Does Gemini require a `matcher` key for lifecycle events, or is an empty/omitted
  matcher accepted? Reference shows matcher on `BeforeTool`; lifecycle events may not
  need it. Verify against a live `~/.gemini/settings.json` before shipping.
