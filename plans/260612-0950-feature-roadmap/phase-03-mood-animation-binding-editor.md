# Phase 03 — Mood → Animation Binding Editor

## Context Links
- Master plan: [plan.md](plan.md)
- Config bindings: `crates/agentpet-core/src/config.rs:44` (`bindings:
  HashMap<pack_id, HashMap<mood, clip_index>>`), `:116-121` (`clip_index` resolver)
- Sprite model: `crates/agentpet-core/src/sprite.rs:134-153` (`PetPack`, `clip_count`,
  `clip`), `:170-190` (`PetBindings`, `defaults` spread)
- Mood list: `crates/agentpet-core/src/state.rs:113` (`PetMood::ALL`)
- Pet clip pick: `crates/agentpet/src/pet/mod.rs:254-269` (`set_pack` uses
  `PetBindings::defaults`, NOT config), `:307` (`bindings.clip_index(mood)`)
- Pet tab: `crates/agentpet/src/ui/settings/pet_page.rs` (extend here)
- Thumbnail render: `pet_page.rs:382-392` (`load_thumbnail`), `mod.rs:395-417`
  (`to_surface`)

## Overview
- **Priority:** P2
- **Status:** pending
- Config `bindings` exists with a working resolver (`config.clip_index`,
  `config.rs:116`) but no UI writes it, AND the live pet never reads it: `set_pack`
  uses `PetBindings::defaults(clip_count)` (`mod.rs:262`), ignoring stored bindings.
  Add an "Animations" section on the Pet tab: per-mood dropdown of the selected
  pack's clips with a live animated preview, persisting to `bindings`; and make the
  pet honour stored bindings.

## Key Insights
- **Two gaps, both required:** (1) no editor writes `bindings`; (2) the pet ignores
  `bindings` — `PetWindow::set_pack` builds `PetBindings::defaults(clips.len())`
  (`mod.rs:262`), so even hand-edited config has no effect. Both must be fixed or
  the feature is invisible.
- Clips = sprite rows; each clip is a `Vec<RgbaImage>` of frames
  (`sprite.rs:34-76`, `:137`). `clip_count()` (`:142`) gives the dropdown length.
- `config.clip_index(pack_id, clip_count, mood)` (`config.rs:116`) already resolves
  stored-or-default with clamping — the pet should call THIS instead of
  `PetBindings::defaults`. Pass pack id + clip count down to `set_pack`.
- `PetBindings::by_mood` keys are `mood.raw()` strings (`sprite.rs:177`), matching
  `bindings`' inner keys — consistent serialization, no translation needed.
- Live preview: reuse the cairo animation loop. KISS option: render the selected
  clip's frames into a small `DrawingArea` with a timer (mirror `pet/mod.rs` draw),
  or step frames. Simpler still: show clip's first frame as a static thumbnail via
  `to_surface` + a 2-frame toggle. Choose a tiny `DrawingArea` that cycles frames at
  a fixed rate — keep it in one helper <120 lines.
- `pet_page.rs` already loads packs (`load_thumbnail:382`, `load_pack`) and knows the
  selected/assigned pack per agent (`ctx.pick`, `:225`). The binding editor targets
  the pack assigned to the currently-selected agent.

## Requirements
**Functional**
- "Animations" group on the Pet tab: one row per mood (`PetMood::ALL`), each a
  `DropDown` listing clip indices (e.g. "Clip 1 … Clip N") for the selected pack.
- Selecting a clip persists `bindings[pack_id][mood] = clip_index` and updates the
  preview.
- A live animated preview of the chosen clip per mood (or one shared preview that
  follows the focused row).
- The live pet uses stored bindings (via `config.clip_index`), updated on
  `ReloadPets`.

**Non-functional**
- Resolver stays in agentpet-core (already tested at `config.rs:172-181`); add tests
  if new core logic is introduced. No new heavyweight deps.
- New UI file <200 lines.

## Architecture
- **agentpet-core:** reuse `Config::clip_index` (no new logic needed). If a
  `set_binding(pack_id, mood, idx)` helper is added for symmetry with `set_pet_for`,
  unit-test it. Pure layer unchanged otherwise.
- **agentpet (`pet/mod.rs`):** change `set_pack` to accept the pack id (or load
  bindings from `Config`) and build `PetBindings` from `config.clip_index` for each
  mood instead of `::defaults`. Provide `set_bindings(PetBindings)` or fold into
  `set_pack`. `reload_pet` (`ui/mod.rs:131`) already re-runs `set_pack` → bindings
  refresh for free once `set_pack` reads config.
- **agentpet (`ui/mod.rs:195` `load_pack_for_kind`):** already returns the pack;
  pass `pack.manifest.id` so the pet can resolve bindings.
- **agentpet (`settings/animation_rows.rs`, NEW):** builds the mood rows + preview;
  reads the assigned pack for the selected agent, lists clips, persists on change,
  sends `UiCommand::ReloadPets`.
- **Data flow:** dropdown → `bindings[pack][mood]` in config → `ReloadPets` →
  `set_pack` rebuilds `PetBindings` via `clip_index` → pet animates the chosen clip.

## Related Code Files
**Modify**
- `crates/agentpet/src/pet/mod.rs` — `set_pack` builds bindings from config
  (`clip_index`) keyed by pack id + clip count; not `::defaults`.
- `crates/agentpet/src/ui/mod.rs` — pass pack id into `set_pack`; `reload_pet`
  unchanged in shape.
- `crates/agentpet/src/ui/settings/pet_page.rs` — append the Animations group;
  re-render rows on agent-switch (`connect_selected_notify`, `:242`).
- `crates/agentpet-core/src/config.rs` — optional `set_binding` helper + test.

**Create**
- `crates/agentpet/src/ui/settings/animation_rows.rs` (<200 lines): per-mood
  dropdowns + live preview `DrawingArea`.

**Delete:** none.

## Implementation Steps
1. In `pet/mod.rs::set_pack`, accept `pack_id: &str`; after slicing, build
   `PetBindings { by_mood }` where each mood's index = `Config::load().clip_index(
   pack_id, clips.len(), mood)`. Update callers in `ui/mod.rs` (lines 104, 133).
2. (Optional) add `Config::set_binding(pack_id, mood, idx)` + a unit test.
3. Create `animation_rows.rs`: resolve the selected agent's assigned pack id +
   `clip_count`; render one `DropDown` per mood preset to the stored/default index.
4. On dropdown change: persist `bindings[pack][mood]`, send `ReloadPets`, refresh
   the preview.
5. Add a small preview `DrawingArea` cycling the selected clip's frames (reuse
   `to_surface`); follow the focused mood row.
6. Append the group in `pet_page.rs`; rebuild rows when the agent dropdown changes.
7. `cargo test -p agentpet-core -p agentpet`; `cargo build --release`.

## Todo List
- [ ] Pet honours stored bindings via `config.clip_index` in `set_pack`
- [ ] Update `set_pack` callers to pass pack id
- [ ] (opt) `Config::set_binding` + test
- [ ] `animation_rows.rs`: per-mood dropdowns
- [ ] Live animated preview
- [ ] Persist + `ReloadPets`; rebuild on agent switch
- [ ] Tests + release build pass

## Success Criteria
- `cargo test -p agentpet-core -p agentpet` passes.
- `cargo build --release` compiles.
- Choosing a clip for a mood updates the stored config AND the live pet animates
  that clip when it enters the mood.
- No assigned pack / 0 clips → rows disabled with a clear empty state (no panic;
  `clip` clamps at `sprite.rs:147-152`).

## Risk Assessment
- **Pet ignored bindings before (High×High if unfixed):** the editor is useless
  unless `set_pack` reads config — step 1 is the load-bearing fix. Verified at
  `mod.rs:262`.
- **Preview animation CPU (Low×Med):** cap the preview timer rate and stop it when
  Settings is hidden (`set_hide_on_close`, `settings/mod.rs:31`).
- **Pack swap mid-edit (Low×Med):** rows rebuild on agent-switch; bindings are keyed
  by pack id so a different pack's bindings never bleed across.

## Security Considerations
- None new — indices are clamped (`config.rs:118`, `sprite.rs:151`); no file/path or
  user-text input beyond integer clip selection.

## Next Steps
- Pairs well with phase-01 (chat) on the Settings Pet tab; independent otherwise.
