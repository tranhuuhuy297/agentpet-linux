# Pet Size Setting — ✅ Done (2026-06-09)

Implemented across all 5 files; `cargo build`/`clippy` clean, 57 tests pass
(incl. new `clamp_pet_size` test). Code review: DONE_WITH_CONCERNS — applied
fixes for live-shrink nudge (`set_default_size`) and persist-path clamp.
Open: live-**shrink** on the target compositor needs a manual drag-down check
(can't be reproduced headlessly — needs an active agent + installed pet pack).


Add a global pet-size control to the Settings dialog's Pet tab, persist it, and
make the floating pet actually consume it (initial size + live resize).

## Decisions (user-confirmed)
- Placement: top of **Pet tab** (global row above per-agent picker).
- Control: **Scale slider, 80–200 px**, default 110, live preview while dragging.
- Scope: **global** — reuses existing `Config.pet_size` (no schema change).

## Key facts (scout)
- `Config.pet_size: f64` (default 110.0) already exists + has round-trip tests, but is **dead** — nothing reads it.
- `pet/mod.rs` hardcodes `const SIZE: i32 = 140` for window/drawing-area size + slot spacing.
- `draw()` already scales the sprite to the widget's `w`/`h`, so resizing the DrawingArea auto-scales the pet.
- Live-apply path: `UiCommand` → `gui/mod.rs` match → `Ui` method over live `PetWindow`s.

## Changes
1. `pet/mod.rs`: export `MIN_PET_SIZE=80`/`MAX_PET_SIZE=200` + `clamp_pet_size(f64)->i32`; `PetWindow::new` takes `size: i32`; store `area`; add `set_size(size)`; replace `SIZE` const with the param (window default, area content, slot spacing).
2. `snapshot.rs`: add `UiCommand::ResizePets(i32)` (carries px so resize is disk-free/instant).
3. `gui/mod.rs`: handle `ResizePets(px) => ui.resize_pets(px)`.
4. `ui/mod.rs`: `sync_pets` passes `clamp_pet_size(Config::load().pet_size)` to new pets; add `resize_pets(size)`.
5. `ui/settings/pet_page.rs`: add "Pet size" slider row at top; on `value-changed` → send `ResizePets(px)` (live) + debounced (250 ms) `Config` save.

## Success criteria
- Slider appears at top of Pet tab; dragging resizes live pets immediately.
- Size persists across restart (new pets spawn at saved size).
- `cargo build` clean; existing config tests still pass.
- No regression to pet drag/animation/keep-above or the per-agent picker.
