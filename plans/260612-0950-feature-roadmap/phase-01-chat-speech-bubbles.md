# Phase 01 — Chat / Speech Bubbles

## Context Links
- Master plan: [plan.md](plan.md)
- Config: `crates/agentpet-core/src/config.rs:32,40,42` (`show_chat`, `chat_source`, `chat_custom`)
- Caption drawing: `crates/agentpet/src/pet/caption.rs` (cairo pills) — extend here
- Pet draw loop: `crates/agentpet/src/pet/mod.rs:119-130` (draw_func), `:144-169` (tick)
- Mood enum: `crates/agentpet-core/src/state.rs:101-130` (`PetMood`, `raw()`, `ALL`)
- Related: phase-09 (urgency) reuses caption drawing — do 01 first.

## Overview
- **Priority:** P1 (top impact/effort in roadmap)
- **Status:** done (2026-06-12 — tests 76/76, release build clean, code review 9/10)
- Pet shows a small speech bubble above the sprite with a short mood-based line
  ("on it!", "need you 👀", "done ✅"). Lines come from a built-in system set or a
  user-supplied custom set (`chat_source`). Line text + rotation is pure logic in
  agentpet-core (unit-tested); bubble pixels are drawn in cairo alongside the
  existing caption. A Settings section adds a toggle, source picker, and per-mood
  custom-line editor.

## Key Insights
- `show_chat` defaults true (`config.rs:59`) but is read nowhere — grep confirms no
  consumer. Bubble must be gated on it so the default-true does not surprise users
  → ship a sensible system line set.
- Caption already auto-shrinks fonts and draws rounded pills (`caption.rs:125-165`
  `draw_pill`, `:190` `rounded_rect`) — reuse the same primitives, do NOT add a new
  drawing stack.
- The draw_func lays the sprite against `base` (square) and draws caption below
  (`mod.rs:122-129`). The bubble goes ABOVE the sprite, so it needs vertical
  headroom: either draw within the existing top padding (sprite is scaled to
  `0.92` of min(w,h) at `mod.rs:317`, leaving ~8% slack) or add a fixed top band to
  canvas height (mirrors `waiting_block_height` pattern). KISS: draw inside the
  existing top slack first; only grow canvas if clipping shows in testing.
- Tick redraw is gated on a `last_drawn` key (`mod.rs:158-164`). Rotating the line
  on a timer must fold the active line index into that key or the bubble text will
  not refresh. Drive rotation from `phase` (already advancing) — no new timer.
- `PetMood::ALL` (`state.rs:113`) gives the canonical mood list + `raw()` keys that
  match `chat_custom`'s `HashMap<String, Vec<String>>` keys.

## Requirements
**Functional**
- System line set: ≥1 line per mood (idle, working, waiting, done, celebrate).
- `chat_source == "custom"`: use `chat_custom[mood]`; empty/missing → fall back to
  system lines for that mood (never show a blank bubble).
- Bubble hidden entirely when `show_chat == false`.
- Lines rotate over time (multiple lines per mood cycle); single-line moods stay
  static.
- Settings: toggle, system/custom radio, per-mood multiline editor (one line per
  text row), persisted to `chat_custom`.

**Non-functional**
- Pure selection/rotation logic in agentpet-core, ≥4 unit tests.
- No new heavyweight deps. Idle CPU unchanged (redraw stays gated on the key).
- New files <200 lines.

## Architecture
- **agentpet-core (`src/chat.rs`, NEW):** `ChatLines` — owns system defaults +
  resolves the active line. Pure functions:
  - `system_lines(mood: PetMood) -> &'static [&'static str]`
  - `lines_for(cfg: &Config, mood) -> Vec<String>` (custom-or-system fallback)
  - `pick(lines: &[String], phase: f64, secs_per_line: f64) -> Option<&str>`
    (deterministic index from phase → testable without a clock).
  Re-export in `lib.rs`. Config stays the data owner; chat.rs reads it.
- **agentpet (`pet/caption.rs`):** add `draw_bubble(cr, w, sprite_h, text)` — a
  centred rounded pill anchored above the sprite (reuse `rounded_rect`, font
  auto-shrink copied from `draw_pill`). Tail triangle optional (YAGNI: skip v1).
- **agentpet (`pet/mod.rs`):** in draw_func, after the sprite, if `show_chat` &&
  bubble text resolved, call `draw_bubble`. Cache `Config` chat fields in the
  `PetWindow` (load once in `new`, refreshed via existing `ReloadPets` path) — do
  NOT `Config::load()` inside draw_func (runs at 12.5 Hz).
- **Data flow:** mood (Cell) + phase (Cell) → `ChatLines::pick` → text → cairo.
  Settings writes `chat_*` to disk → `UiCommand::ReloadPets` → pet re-reads config.

## Related Code Files
**Modify**
- `crates/agentpet-core/src/lib.rs` — `pub mod chat;` + re-export.
- `crates/agentpet/src/pet/caption.rs` — add `draw_bubble`.
- `crates/agentpet/src/pet/mod.rs` — bubble fields on `PetWindow`, call in draw_func,
  fold line index into `last_drawn` key, refresh on `set_pack`/reload.
- `crates/agentpet/src/ui/settings/mod.rs` — register chat section (own page or
  group on Pet tab; reuse Stack).
- `crates/agentpet/src/ui/mod.rs` — ensure `ReloadPets` refreshes bubble config
  (already reloads packs; extend `reload_pet`).

**Create**
- `crates/agentpet-core/src/chat.rs` (<150 lines incl. tests).
- `crates/agentpet/src/ui/settings/chat_page.rs` (<200 lines) — toggle, source
  radio, per-mood editors.

**Delete:** none.

## Implementation Steps
1. Add chat fields to `PetWindow` is unnecessary — instead store an
   `Rc<RefCell<ChatConfig>>` snapshot (show_chat + resolved lines per mood) so the
   draw_func never touches disk.
2. Write `agentpet-core/src/chat.rs`: system defaults table keyed by `PetMood`,
   `lines_for`, deterministic `pick(phase, secs_per_line)`. Re-export in lib.rs.
3. Add `draw_bubble` to `caption.rs` (centred pill above sprite, font auto-shrink).
4. Wire `pet/mod.rs`: load chat snapshot in `new`; in draw_func resolve line via
   `pick(phase, …)` and draw when `show_chat`; add line index to `last_drawn` key.
5. Add a refresh path so `ReloadPets` re-reads chat config into the snapshot.
6. Build `chat_page.rs`: GTK toggle (Switch), source radio, per-mood `TextView`
   editors; persist to `chat_custom`/`chat_source`/`show_chat`; send `ReloadPets`.
7. Register the page in `settings/mod.rs` Stack.
8. `cargo test -p agentpet-core -p agentpet`; `cargo build --release`.

## Todo List
- [x] `chat.rs` with system defaults + resolver + `pick`
- [x] Unit tests: custom-overrides-system, empty-custom-falls-back, pick-rotates, pick-stable-single-line (7 tests total)
- [x] `draw_bubble` in caption.rs
- [x] pet/mod.rs snapshot + draw + redraw key
- [x] `chat_page.rs` Settings UI
- [x] Register page in settings/mod.rs + reload path
- [x] Tests + release build pass

## Success Criteria
- `cargo test -p agentpet-core -p agentpet` passes (incl. new chat tests).
- `cargo build --release` compiles.
- Toggling `show_chat` shows/hides the bubble live after reopening Settings.
- Custom lines for a mood appear; clearing them falls back to system lines.
- Idle CPU unchanged (no redraw when line index + frame + bob are static).

## Risk Assessment
- **Bubble clips sprite/screen top (Med×Med):** sprite uses 8% slack; if tight, add
  a fixed top band to canvas height like `waiting_block_height`. Mitigate by
  testing at MIN_PET_SIZE (80px).
- **Disk read in hot draw loop (Low×High):** avoided by snapshotting config; never
  `Config::load()` in draw_func.
- **Emoji glyph width in cairo (Low×Low):** font auto-shrink already clamps width;
  emoji render via the system font fallback.

## Security Considerations
- Custom lines are user text rendered only in the user's own pet window — no
  injection surface. Persisted to the existing config JSON (no new file perms).

## Next Steps
- Unblocks phase-09 (waiting urgency) which reuses caption drawing.
- Consider a small tail triangle on the bubble as a polish follow-up (deferred).
