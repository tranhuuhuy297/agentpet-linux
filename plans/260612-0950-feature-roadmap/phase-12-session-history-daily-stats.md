# Phase 12 — Session History & Daily Stats

## Context Links
- Roadmap: `plans/260612-0950-feature-roadmap/plan.md`
- Session fields: `crates/agentpet-core/src/session.rs:14-24` (`state_since`, `updated_at`)
- Transition site: `crates/agentpet/src/daemon/mod.rs:108-124` (has `before` state + new `s`)
- Notify-on-transition precedent: `crates/agentpet/src/daemon/mod.rs:116` (`notify::on_transition`)
- Base dir: `crates/agentpet-core/src/ipc.rs:12-15` (`~/.agentpet`)
- Monitor footer: `crates/agentpet/src/ui/monitor.rs:59-86`

## Overview
- **Priority:** P3
- **Status:** pending
- **Description:** Track per-session turn metrics (turns, waits, working/waiting durations) at transition time and aggregate into a daily summary line ("today: 12 turns, 3 waits, avg wait 40s") shown in the Monitor. Persist to a small per-day file under `~/.agentpet/stats/`. NO charts (future work).

## Key Insights (verified)
- Sessions are pruned from memory (`session.rs:141-172`), so stats **must be written at transition time**, not derived later. The transition is observed in `daemon/mod.rs:108-124`, which already has both `before: Option<AgentState>` (`:110`) and the updated session `s` (`:113`) — the exact inputs a stats accumulator needs. This is the same hook point `notify::on_transition` uses (`:116`).
- **Turn** = `Working → Done` transition; **wait** = any `→ Waiting` transition. Durations from `now - state_since` of the *leaving* state. `state_since` resets on every state change (`session.rs:114-116`), giving the elapsed-in-prior-state at transition. Note: `s` returned by `apply` already has `state_since=now` for the *new* state, so the leaving-state duration must be computed in the daemon as `now - before_state_since` captured **before** `apply` (currently only `before` *state* is captured at `:110`; add a `before_state_since` capture).
- Base dir `~/.agentpet` is the established location (`ipc.rs:12`); add `stats/` alongside `queue/`.
- Pure accumulator fits core (deterministic, `now`-injected like the rest of the crate).

## Requirements
- Functional: count turns, waits, cumulative working/waiting seconds per local day; render today's aggregate as a compact monitor header or footer line. Persist so restarts keep the day's running total.
- Non-functional: pure aggregation + tests in core; daemon does IO; minimal file format; no new deps (serde_json present in both crates).

## Architecture
- **Core (`daily_stats.rs`)**: `DailyStats { date: String, turns, waits, working_secs, waiting_secs }` with `record(before: Option<AgentState>, after: AgentState, leaving_secs: f64)` mutating counters, and a `summary() -> String` ("today: N turns, M waits, avg wait Xs"). Pure; unit-tested.
- **Persistence**: one JSON file per day `~/.agentpet/stats/<YYYY-MM-DD>.json` (small, overwrite-on-update). Simpler than JSONL since we keep a single rolling aggregate per day (KISS). A thin `stats_store` helper in the binary crate loads-or-creates today's file, applies `record`, writes back.
- **Wiring**: in `daemon/mod.rs handle_client`, after a successful `apply` (`:113`), compute `leaving_secs = now - before_state_since`, call the stats store. Monitor reads today's file on render and shows `summary()`.
- **Data flow**: transition (daemon) → `DailyStats::record` → write `<today>.json`; monitor render → read `<today>.json` → `summary()` → header/footer label.

## Related Code Files
- **Modify:** `crates/agentpet/src/daemon/mod.rs` (capture `before_state_since`; call stats writer on transition), `crates/agentpet/src/ui/monitor.rs` (add a summary label to footer/header; read today's file on render), `crates/agentpet-core/src/lib.rs` (`pub mod daily_stats;`).
- **Create:** `crates/agentpet-core/src/daily_stats.rs` (pure struct + record + summary + serde + tests), `crates/agentpet/src/daemon/stats_store.rs` (load/apply/save today's file; date via `chrono`-free local date from `unix_now` + `localtime`-equivalent — keep simple with UTC date to avoid a tz dep).
- **Delete:** none.

## Implementation Steps
1. `daily_stats.rs`: define `DailyStats` (serde derive), `record(before, after, leaving_secs)`: if `before==Some(Working) && after==Done` → `turns+=1`; if `after==Waiting` → `waits+=1`; accumulate `working_secs`/`waiting_secs` by the *leaving* state. `summary()` formats the line; `avg wait` = `waiting_secs/waits` guarded against 0. Tests for each rule + empty-day summary.
2. `stats_store.rs`: `today_path() -> PathBuf` (`ipc::base_dir().join("stats").join(format!("{date}.json"))`, date as UTC `YYYY-MM-DD` from `unix_now`), `load_today()`, `apply_and_save(before, after, leaving_secs)`. `create_dir_all` the stats dir.
3. `daemon/mod.rs`: at `:110` also capture `before_state_since = store.lock().session(&ev.session_id).map(|s| s.state_since)`. After `apply` returns `Some(s)` (`:113`), compute `leaving_secs = before_state_since.map(|t| now - t).unwrap_or(0.0)` and call `stats_store::apply_and_save(before, s.state, leaving_secs)`. Best-effort; log on IO error.
4. `monitor.rs`: add a single dim `Label` to the footer (left of the spacer at `:64`) or a header line above the list; set its text from `daily_stats::DailyStats::summary()` read via `stats_store::load_today()` inside `render()` (cheap file read; cache acceptable if profiling demands).
5. Document chart visualization as future work in the roadmap (out of scope here).

## Todo List
- [ ] `daily_stats.rs` struct + record + summary + tests
- [ ] `stats_store.rs` load/apply/save + dir creation
- [ ] capture `before_state_since` + wire stats write in daemon
- [ ] monitor summary label reading today's file
- [ ] note charts as future work

## Success Criteria
- Completing a turn increments `turns`; entering Waiting increments `waits`; durations accumulate.
- `~/.agentpet/stats/<today>.json` persists across restarts and the monitor shows today's summary.
- `cargo test -p agentpet-core -p agentpet` passes (new core tests included).
- `cargo build --release -p agentpet` compiles clean.

## Risk Assessment
- **Double-count on rapid transitions** (Med/Low): rule keys on exact `before→after` pairs; idempotency not required since each transition is recorded once at `apply`.
- **Day boundary mid-session** (Low): leaving-state duration credited to the day the transition lands; acceptable for a summary line.
- **File write contention** (Low): only the single daemon thread writes; monitor only reads.
- **UTC vs local date drift** (Low): UTC chosen to avoid a tz dep; document the choice. Revisit only if users complain.

## Security Considerations
- Stats file holds counts/durations only — no project paths or content. Written under user-owned `~/.agentpet`; no exec, no untrusted path joins (date string is numeric-formatted, not user input).

## Next Steps
- Independent of phase 11; both touch the daemon transition path — keep stats and tool-clearing as separate, order-independent calls so either ships alone.
