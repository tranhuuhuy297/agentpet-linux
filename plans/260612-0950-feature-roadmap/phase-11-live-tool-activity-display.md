# Phase 11 — Live Tool Activity Display

## Context Links
- Roadmap: `plans/260612-0950-feature-roadmap/plan.md`
- Payload field: `crates/agentpet-core/src/payloads.rs:124` (`tool_name`)
- Event mapping: `crates/agentpet-core/src/mapper.rs:26-44`
- Session struct: `crates/agentpet-core/src/session.rs:13-48`
- Apply logic: `crates/agentpet-core/src/session.rs:104-137`
- Monitor row: `crates/agentpet/src/ui/monitor.rs:154-192`
- Pet caption rows: `crates/agentpet/src/ui/mod.rs:157-167`

## Overview
- **Priority:** P2
- **Status:** pending
- **Description:** Persist the current tool on a session (`PreToolUse` sets, `PostToolUse`/`Stop` clears) and render it as friendly live activity ("Running Bash…", "Editing files…"). Optional small ring buffer of last 3–5 tools.

## Key Insights (verified)
- `ClaudeHookPayload.tool_name` exists (`payloads.rs:124`) but is only folded into a fallback message `"Using {t}"` (`payloads.rs:136-140`) and never persisted on the session.
- **`AgentEvent` does not carry `tool_name`** (`event.rs:14-23`) — only session/kind/event/project/message/timestamp. To reach the store, tool must be added to `AgentEvent` (new optional field) and populated by `make_event` (`payloads.rs:133`, `HookArguments::make_event` `:47`).
- Tool lifecycle maps cleanly to event names: `PreToolUse` → set tool; `PostToolUse`/`Stop`/`SubagentStop` → clear (`mapper.rs:28,30`). Codex mirrors this (`mapper.rs:35,39`).
- `AgentSession.message` already drives the monitor activity line (`monitor.rs:166`) and pet caption is built from sessions (`ui/mod.rs:157`). Reusing `tool` for caption is cheap.
- Core is pure (callers pass `now`) — all logic unit-testable in `agentpet-core`.

## Requirements
- Functional: session shows live tool while a tool runs; clears when the turn ends. Friendly mapping for Bash/Edit/Write/Read/Grep/Task/WebFetch; unknown tools show raw name. Optional `recent_tools` ring (cap 5).
- Non-functional: pure logic + tests in core; no new deps; monitor render stays one line.

## Architecture
Data flow: hook stdin `tool_name` → `AgentEvent.tool` (new `Option<String>`) → `SessionStore::apply` sets/clears `AgentSession.tool` (+ pushes to `recent_tools` ring) → snapshot → monitor row + pet caption.
- New core helper `tool_activity(tool: &str) -> String` maps known tools to verb phrases; fallback `format!("Running {tool}…")`.
- `apply` (`session.rs:104`) gains tool handling: on a `Working` transition with `event.tool=Some`, set `session.tool` and push to ring; on `Done`/clear states set `session.tool=None`.

## Related Code Files
- **Modify:** `crates/agentpet-core/src/event.rs` (add `tool: Option<String>` field + `new` arg), `crates/agentpet-core/src/payloads.rs` (pass `tool_name` into events for both `ClaudeHookPayload` and `HookArguments`), `crates/agentpet-core/src/session.rs` (`tool` + `recent_tools` fields; set/clear in `apply`; tests), `crates/agentpet/src/ui/monitor.rs` (render tool in activity line), `crates/agentpet/src/ui/mod.rs` (feed tool to caption if present).
- **Create:** `crates/agentpet-core/src/tool_activity.rs` (pure mapping fn + tests) — wired via `lib.rs` `pub mod tool_activity;`.
- **Delete:** none.

## Implementation Steps
1. `event.rs`: add `pub tool: Option<String>`, default-skip-serialize like `project`/`message` (`event.rs:18-21`); extend `AgentEvent::new` signature. Update all 9 `new` callers (verified by grep): production `payloads.rs:55`, `payloads.rs:141`, `cli/run.rs:37`; tests `event.rs:51`, `event.rs:63`, `ipc.rs:95`, `ipc.rs:114`, `ipc.rs:115`, `session.rs:204`. (Alternative to a wider signature: keep `new` as-is and add a `with_tool(self, tool)` builder to touch fewer callers — pick whichever keeps the diff smallest.)
2. `payloads.rs`: in both `make_event`s pass `self.tool_name.clone()` (Claude) / new `--tool` flag (HookArguments, optional) as the `tool` arg. Keep the existing `"Using {t}"` message fallback for back-compat.
3. New `tool_activity.rs`: `pub fn tool_activity(tool: &str) -> String` with a match (Bash→"Running Bash…", Edit/Write→"Editing files…", Read→"Reading files…", Grep→"Searching…", Task→"Delegating…", WebFetch→"Fetching…", _→"Running {tool}…"). Unit tests for known + unknown.
4. `session.rs`: add `pub tool: Option<String>` and `pub recent_tools: Vec<String>` (cap 5, oldest dropped). In `apply`: when `event.tool` is `Some` and state is `Working`, set tool + push ring; when state is `Done`/`Idle`/`Registered`/`Waiting` clear `tool`. `AgentSession::new` initializes both empty. Tests: set on PreToolUse, clear on Stop, ring caps at 5.
5. `monitor.rs:166`: prefer `s.tool.as_ref().map(|t| tool_activity(t))` over `message` when working; else fall back to current behaviour. Optionally append a tiny recent-tools strip (small dim label) — gate behind glanceability; can be deferred.
6. `ui/mod.rs`: if cheap, include current tool in the pet caption (only when `Working`).

## Todo List
- [ ] `AgentEvent.tool` field + update all `new` callers
- [ ] payloads pass tool through
- [ ] `tool_activity.rs` + tests
- [ ] session `tool`/`recent_tools` set/clear in `apply` + tests
- [ ] monitor renders friendly tool activity
- [ ] (optional) pet caption tool line
- [ ] (optional) recent-tools strip

## Success Criteria
- During a tool run the monitor shows "Running Bash…" etc.; clears to state word on Stop.
- `recent_tools` never exceeds 5.
- `cargo test -p agentpet-core -p agentpet` passes (new core tests included).
- `cargo build --release -p agentpet` compiles clean.

## Risk Assessment
- **Caller churn from `AgentEvent::new` signature change** (Med): enumerate every call site before editing (grep `AgentEvent::new`); a missed test caller fails compile, caught immediately.
- **PostToolUse arrives before PreToolUse cleared** (Low): both are `Working`; last-write-wins on `tool` is acceptable.
- **Ring buffer growth** (Low): hard cap 5 in core, asserted by test.

## Security Considerations
- `tool_name` is attacker-influenceable only by the local agent; rendered via existing `glib_escape` (`monitor.rs:382`) — keep escaping. No path/exec use of the tool string.

## Next Steps
- Independent of phase 12, but phase 12 may reuse the transition plumbing in `apply`. Design `apply` changes so tool handling and stats hooks are orthogonal.
