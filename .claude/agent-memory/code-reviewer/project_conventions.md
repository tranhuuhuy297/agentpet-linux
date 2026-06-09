---
name: project-conventions
description: agentpet-linux (Rust + GTK4) architecture conventions relevant to reviewing pet/UI/config changes
metadata:
  type: project
---

agentpet-linux is a Rust + GTK4 desktop pet app (crates: `agentpet`, `agentpet-core`).

Key conventions observed:
- UI commands flow over an **unbounded** `async_channel` (`cmd_tx`/`cmd_rx`) of `UiCommand` enum (crates/agentpet/src/snapshot.rs). GUI loop matches arms in gui/mod.rs. Sends use `try_send` — safe to ignore error only because channel is unbounded.
- Persistent settings: `agentpet_core::config::Config`, single JSON file at XDG path. `Config::load()` returns defaults on missing/corrupt; `save()` is a full-file `serde_json::to_vec_pretty` + `std::fs::write` (no atomic rename, no locking). `pet_size: f64` default 110.0.
- Pets are per-agent floating X11 (XWayland, `GDK_BACKEND=x11`) `ApplicationWindow`s, non-decorated, non-resizable, positioned via raw x11rb `move_window` on `connect_map`. Drawing via `DrawingArea` + cairo `draw()` that scales sprite to widget w/h.
- All GTK callbacks run on the main thread; `Rc`/`Cell`/`RefCell` used freely for per-widget state (single-threaded).
- Pet animation uses a weak-ref `glib::timeout_add_local` self-cancelling tick.

**How to apply:** When reviewing UI/pet/config changes, check: try_send is fine (unbounded); Config::save is non-atomic (interrupted write can truncate file — but load tolerates corruption by falling back to defaults, which silently discards user settings); main-thread-only so no cross-thread races on Rc/Cell.
