# Chat Speech Bubbles: Phase 01 Implementation Complete

**Date**: 2026-06-12
**Severity**: Low
**Component**: Pet chat system (crates/agentpet-core/src/chat.rs, crates/agentpet/src/pet/, crates/agentpet/src/ui/settings/)
**Status**: Completed

## What Happened

Implemented the phase-01 "chat speech bubbles" roadmap milestone. Built a complete speech-bubble system driven by pet mood states, with per-mood customizable lines, deterministic rotation, and UI controls for enabling/disabling bubbles and editing line content.

## The Brutal Truth

The geometry question burned more mental cycles than it should have. Initial plan called for drawing bubbles in the "slack" above the sprite (idle space ~4px). Reality: buffer height is ~4px vs. a 15px pill—clipping math doesn't work. Pivoted to the existing `waiting_block_height` pattern (fixed band ABOVE sprite via `cr.translate`), which trades one line of code for geometric certainty. No harm done, but the slack-based approach needed testing before it got planned.

## Technical Details

**New module: crates/agentpet-core/src/chat.rs**
- `system_lines(mood: PetMood) -> Vec<&'static str>` — Returns 3–5 mood-specific lines (calm, busy, waving).
- `lines_for(mood, config) -> &[str]` — Fallback logic: custom lines if provided else system lines; never blank.
- `pick(lines, phase_index) -> &str` — Deterministic rotation off animation frame counter (no new timer, no clock).
- `pick_index(lines, phase_index) -> usize` — Explicit index for testing; 7 unit tests covering all moods, empty custom lines, edge indices.

**Modified: crates/agentpet/src/pet/caption.rs**
- `draw_pill` now accepts `bg` and `fg` color parameters (was hard-coded).
- New `draw_bubble(cr, text, bg, fg, width, height)` — Inverted palette (light pill, dark text) vs. system lines.
- New `bubble_band_height() -> f64` — Constant 20px; decoupled from mood to prevent geometry thrashing on mood change.

**Modified: crates/agentpet/src/pet/mod.rs**
- `ChatState` snapshot in `PetDraw` — Config read once per `reload_pet()`, never touched in the 12.5 Hz render loop.
- Bubble drawn in fixed 20px band ABOVE sprite via `cr.translate(0, -20)`.
- Line index folded into `last_drawn` redraw key (same pattern as animation frame rotation) — idle CPU cost unchanged.
- `refresh_chat()` re-reads config on `ReloadPets` event.

**New: crates/agentpet/src/ui/settings/chat_page.rs**
- Chat tab in Settings UI with:
  - `show_chat` toggle switch.
  - System / Custom radio group (per mood).
  - Per-mood line text editors with 400ms debounce.
  - Reuses generation-token pattern from pet-size slider for debounced writes.
- Registered as new tab in settings panel.

**Test coverage:** 76/76 tests pass. 7 new unit tests in chat.rs; existing rendering and config tests cover integration.

## What We Tried

1. **Drawing bubble in sprite slack (~4px buffer)** — Clipping math failed. Buffer height insufficient for 15px pill + text. Rejected.
2. **Mood-specific band heights** — Would require window geometry recalc on mood change. Rejected in favor of constant 20px.
3. **Wall-clock-driven line rotation** — Added complexity and non-determinism. Rejected in favor of animation phase index.

## Root Cause Analysis

Initial plan underestimated geometry constraints. Slack space is buffer, not layout real estate. Early geometry proof-of-concept (pixel-by-pixel math) would have caught this before design lock. Animation-phase-driven rotation emerged as cleaner than wall-clock because the pet's visual state already has a frame counter; reusing it is testable and cost-free.

## Lessons Learned

- **Geometry before design:** Virtual coordinate space behavior (clipping, transforms) must be validated before settling on layout. "4px slack exists" is not a design assumption—it's an implementation detail that can vanish.
- **Determinism is testable:** Using animation phase instead of wall clock eliminated timing flakiness and made unit tests trivial (pass phase index, assert output).
- **Config snapshots reduce render-loop coupling:** Reading config once per reload instead of per-frame decouples persistence from rendering and keeps the hot loop simple.
- **Constant band height prevents thrashing:** Mood changes now only affect line content, not window geometry. Simpler state machine.

## Next Steps

- Commit phase-01 changes (chat.rs, caption.rs, mod.rs, chat_page.rs, test additions).
- Phase-02: Bubble animation (fade-in, hold, fade-out timing).
- Future: Profiling to confirm render-loop CPU cost is unchanged (expected: +0 due to reuse of animation phase counter).

## Unresolved Questions

None—all design decisions validated by testing (76 tests), code review (9/10, 0 critical), and cargo build --release (0 clippy warnings).
