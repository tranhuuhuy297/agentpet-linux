# Phase 04 — First-Run Onboarding

## Context Links
- Master plan: [plan.md](plan.md)
- Config: `crates/agentpet-core/src/config.rs:36` (`has_onboarded`, default false)
- Current first-run path: `crates/agentpet/src/gui/mod.rs:68-74` (opens Settings,
  sets `has_onboarded = true`)
- Hook-toggle logic to reuse: `crates/agentpet/src/ui/settings/general.rs:65-209`
  (`agent_row`, `confirm_then_install`, Codex `/hooks` trust warning at `:167-171`)
- Pet install logic to reuse: `crates/agentpet/src/ui/settings/pet_page.rs:329-360`
  (`build_install_guide`, `INSTALL_HINT`), `petdex.rs:15` (`INSTALL_HINT`),
  `:35` (`scan_installed`)
- UI host: `crates/agentpet/src/ui/mod.rs:38-57` (`Ui::build`, `show_settings`)

## Overview
- **Priority:** P2
- **Status:** pending
- `has_onboarded` gates a one-time flow. Today first launch just opens the full
  Settings window (`gui/mod.rs:71`) — overwhelming and non-guiding. Replace with a
  focused 3-step first-launch dialog (libadwaita-friendly): (1) enable an agent,
  (2) install a pet, (3) done. Skippable; set `has_onboarded` on completion or skip.

## Key Insights
- The trigger already exists at `gui/mod.rs:70-74`: `if !cfg.has_onboarded { … }`.
  Swap `ui.show_settings()` for `ui.show_onboarding()` and move the `has_onboarded`
  write into the flow's complete/skip handlers (NOT eagerly — current code sets it
  true immediately at `:72`, so onboarding is effectively never re-shown even if the
  user closes Settings instantly; the new flow should persist only on finish/skip).
- **Reuse, don't duplicate:** step 1's agent toggles are `agent_row` +
  `confirm_then_install` (general.rs); step 2's pet install is the guide +
  `INSTALL_HINT` + a Refresh that calls `scan_installed`. Extract shared builders so
  both Settings and onboarding call them (DRY) — avoid a parallel "enhanced" copy.
- Codex trust caveat (`general.rs:167-171`): the `/hooks` trust warning MUST appear
  in onboarding too, or Codex's pet silently never shows. Reuse the same
  `confirm_then_install` so the warning is inherited automatically.
- libadwaita: project styles "after the libadwaita design reference"
  (`settings/mod.rs:3-4`) but uses plain GTK widgets. `[UNVERIFIED]` whether the
  `libadwaita`/`adw` crate is a dependency — check `crates/agentpet/Cargo.toml`. If
  present, use `adw::Carousel`/`NavigationView` for steps; if NOT, build a simple
  GTK `Stack` + Next/Back/Skip buttons (KISS, no new dep). Prefer the latter unless
  adw is already linked.
- `Ui` owns `SettingsWindow` (`ui/mod.rs:31`); add an `OnboardingWindow` field built
  the same way, presented via a new `show_onboarding`.

## Requirements
**Functional**
- 3-step flow on first launch when `has_onboarded == false`:
  1. **Enable agents** — agent rows with install toggles (reuse `agent_row`), incl.
     Codex `/hooks` trust note.
  2. **Install a pet** — show `INSTALL_HINT` command + a Refresh button; on refresh,
     re-scan and show count (reuse pet_page scan logic).
  3. **Done** — short confirmation + "Open Settings" / "Close".
- Next/Back navigation; a Skip control on every step.
- `has_onboarded = true` persisted on Finish OR Skip (once), so the flow never
  re-appears.

**Non-functional**
- No new heavyweight dep unless `adw` is already linked. Shared builders extracted
  to avoid duplicating general/pet logic. New file <200 lines.

## Architecture
- **agentpet-core:** none needed (config field exists; the resolver is just the
  bool). Do NOT add core logic for pure UI flow (YAGNI).
- **agentpet (`settings/general.rs`):** make `agent_row` (and helpers) reachable
  from onboarding — either `pub(crate)` or extract a `agent_rows_group()` builder
  both call.
- **agentpet (`settings/pet_page.rs`):** expose a small reusable
  `install_guide_with_refresh()` (wraps `build_install_guide` + a scan/count label)
  for step 2, or call `scan_installed` directly in onboarding.
- **agentpet (`ui/onboarding.rs`, NEW):** `OnboardingWindow` — a `Stack` of 3 pages
  + a button bar (Back / Next / Skip / Finish). On Finish/Skip: persist
  `has_onboarded`, close, optionally present Settings.
- **agentpet (`ui/mod.rs`):** add the window + `show_onboarding()`.
- **agentpet (`gui/mod.rs:68-74`):** call `ui.show_onboarding()` instead of
  `show_settings()`; remove the eager `has_onboarded = true` write (flow owns it).
- **Data flow:** launch → `gui` checks `has_onboarded` → onboarding window → step
  actions reuse existing install/scan logic → Finish/Skip writes config.

## Related Code Files
**Modify**
- `crates/agentpet/src/gui/mod.rs` — trigger `show_onboarding`; drop eager write.
- `crates/agentpet/src/ui/mod.rs` — `OnboardingWindow` field + `show_onboarding`.
- `crates/agentpet/src/ui/settings/general.rs` — expose agent-rows builder
  (`pub(crate)`), keep Codex trust warning shared.
- `crates/agentpet/src/ui/settings/pet_page.rs` — expose an install-guide/refresh
  builder for reuse (or document direct `scan_installed` use).

**Create**
- `crates/agentpet/src/ui/onboarding.rs` (<200 lines): the 3-step window.

**Delete:** none.

## Implementation Steps
1. Confirm whether `adw`/`libadwaita` is in `crates/agentpet/Cargo.toml`. If not,
   use plain GTK `Stack` + button bar (no new dep).
2. Extract a shared `agent_rows_group()` from general.rs (used by Settings + step 1);
   keep `confirm_then_install` (Codex trust) intact and reused.
3. Extract/expose the install-guide+Refresh+count widget from pet_page.rs for step 2
   (or call `scan_installed` directly).
4. Build `ui/onboarding.rs`: `Stack` with 3 pages, Back/Next/Skip/Finish bar;
   Finish/Skip → `Config::load(); cfg.has_onboarded = true; cfg.save()`; close.
5. Add `OnboardingWindow` to `Ui` + `show_onboarding()`.
6. Update `gui/mod.rs:68-74` to call `show_onboarding`; remove the eager
   `has_onboarded = true` write so the flow controls persistence.
7. `cargo test -p agentpet-core -p agentpet`; `cargo build --release`.

## Todo List
- [ ] Check Cargo.toml for adw; pick Stack vs adw
- [ ] Extract shared agent-rows builder (Codex trust preserved)
- [ ] Reuse install-guide/scan for step 2
- [ ] `onboarding.rs` 3-step window + nav + skip
- [ ] `Ui::show_onboarding`; gui trigger swap; drop eager write
- [ ] Tests + release build pass

## Success Criteria
- `cargo test -p agentpet-core -p agentpet` passes.
- `cargo build --release` compiles.
- Fresh config (`has_onboarded:false`) launches the 3-step flow, not raw Settings.
- Enabling Codex in step 1 shows the `/hooks` trust warning (same dialog).
- Finish OR Skip sets `has_onboarded:true`; relaunch does not re-show onboarding.

## Risk Assessment
- **Duplicating general/pet logic (Med×Med):** mandated reuse via extracted
  builders; a parallel copy would drift from the real toggles. Reviewer must reject
  any new "enhanced" duplicate.
- **adw dep creep (Low×Med):** only use adw if already linked; default to plain GTK.
- **has_onboarded set too early (Med×Low):** the bug exists today (`gui/mod.rs:72`
  writes before the user does anything). Fix: persist only on Finish/Skip.
- **Window race at startup (Low×Med):** built in `connect_activate` like Settings;
  reuse the same single-build guard (`gui/mod.rs:60-62`).

## Security Considerations
- Step 1 writes agent hook configs into user-owned files — already gated behind the
  explicit confirm dialog (`confirm_then_install`, general.rs:152). Reusing it keeps
  the consent step; do not auto-enable agents.

## Next Steps
- After onboarding lands, consider a "Replay onboarding" action in Settings/About
  (deferred — YAGNI until requested).
