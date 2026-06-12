# Feature Roadmap — AgentPet Linux

Master plan covering 19 proposed features/UI improvements, grouped by theme and
ordered by impact-vs-effort. Each phase is independent unless noted — pick any
and implement standalone via `/ck:cook <phase-file>`.

## Status legend

`pending` → `in-progress` → `done`

## Group A — Quick wins (config fields already exist, just unwired)

| # | Phase | Status | Effort |
|---|-------|--------|--------|
| 01 | [Chat/speech bubbles](phase-01-chat-speech-bubbles.md) | done | M |
| 02 | [Custom notification sounds](phase-02-custom-notification-sounds.md) | pending | S |
| 03 | [Mood→animation binding editor](phase-03-mood-animation-binding-editor.md) | pending | M |
| 04 | [First-run onboarding](phase-04-first-run-onboarding.md) | pending | M |

## Group B — Pet UX

| # | Phase | Status | Effort |
|---|-------|--------|--------|
| 05 | [Pet wandering/movement](phase-05-pet-wandering-movement.md) | pending | L |
| 06 | [Persist pet drag position](phase-06-persist-pet-drag-position.md) | pending | S |
| 07 | [Pet click interactions](phase-07-pet-click-interactions.md) | pending | M |
| 08 | [Click-through ghost mode](phase-08-click-through-ghost-mode.md) | pending | S |
| 09 | [Waiting urgency escalation](phase-09-waiting-urgency-escalation.md) | pending | M |

## Group C — Monitor window

| # | Phase | Status | Effort |
|---|-------|--------|--------|
| 10 | [Actionable session rows](phase-10-actionable-monitor-session-rows.md) | pending | S |
| 11 | [Live tool activity display](phase-11-live-tool-activity-display.md) | pending | S |
| 12 | [Session history / daily stats](phase-12-session-history-daily-stats.md) | pending | L |
| 13 | [Monitor empty state](phase-13-monitor-empty-state.md) | pending | S |

## Group D — Settings & agents

| # | Phase | Status | Effort |
|---|-------|--------|--------|
| 14 | [More agent presets](phase-14-more-agent-presets.md) | pending | M |
| 15 | [Per-agent notification toggles](phase-15-per-agent-notification-toggles.md) | pending | S |
| 16 | [Tray count toggle](phase-16-tray-count-toggle.md) | pending | S |
| 17 | [In-app update check](phase-17-in-app-update-check.md) | pending | S |

## Group E — Platform

| # | Phase | Status | Effort |
|---|-------|--------|--------|
| 18 | [Wayland layer-shell backend](phase-18-wayland-layer-shell-backend.md) | pending | XL |
| 19 | [Do-not-disturb awareness](phase-19-do-not-disturb-awareness.md) | pending | S |

## Recommended order

1. Top 3 by impact/effort: **01** (speech bubbles), **11** (tool activity), **05** (wandering).
2. Batch the S-effort items (02, 06, 08, 10, 13, 15, 16, 17, 19) as filler between bigger phases.
3. **18** (layer-shell) last — biggest risk, touches window plumbing everywhere.

## Key dependencies

- 09 (urgency) reuses caption drawing touched by 01 — do 01 first if doing both.
- 11 (tool activity) extends `AgentSession`; 12 (stats) builds on the same struct — do 11 before 12.
- 15 depends on notification routing in `notify.rs`; independent of 02 but touches the same file.
- 18 affects 05/06/08 (window positioning APIs differ on layer-shell) — re-verify those after 18.

## Codebase entry points

- Domain logic (GTK-free, tested): `crates/agentpet-core/src/`
- Pet window: `crates/agentpet/src/pet/mod.rs`, `caption.rs`
- Monitor: `crates/agentpet/src/ui/monitor.rs`
- Settings: `crates/agentpet/src/ui/settings/`
- Notifications: `crates/agentpet/src/notify.rs`
- Tray: `crates/agentpet/src/ui/tray.rs`
- Config persistence: `crates/agentpet-core/src/config.rs`
- Test suite: `cargo test -p agentpet-core -p agentpet`
