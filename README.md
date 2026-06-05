# AgentPet for Linux

A native **Rust + GTK4** port of [AgentPet](https://github.com/ntd4996/agentpet)
(macOS) for **Ubuntu 22.04+**. Watch several AI coding agents (Claude Code,
Codex, Gemini CLI, Cursor, opencode, Windsurf) running in parallel and see — at
a glance — which one is **working**, which is **done**, and which is **waiting
for your input**, via a tray monitor and an ambient desktop pet.

> Status: **early WIP.** The pure domain logic (`agentpet-core`) is implemented
> and tested; the GTK/tray/pet platform layer is being built next.

## How it works

Agents call a tiny `agentpet hook` CLI from their hook configs. Each call sends
one JSON event over a Unix socket (`~/.agentpet/agentpet.sock`) to a daemon —
or, if the daemon is down, drops it in `~/.agentpet/queue/` to be replayed on
startup. The daemon normalises every agent's events into a common state machine
and drives the tray icon, the monitor window, desktop notifications, and the
pet's mood.

```
agent hook → `agentpet hook …` → Unix socket → daemon (SessionStore)
                                                  ├── tray (ksni): paw + count
                                                  ├── monitor window: live timers
                                                  ├── desktop pet: aggregate mood
                                                  └── notifications + sound
```

Any other CLI agent can be wrapped: `agentpet run -- <command>` reports
working while it runs and done when it exits.

## Display-server strategy

Like [`cliccy`](https://github.com/tranhuuhuy297/cliccy), the app forces
`GDK_BACKEND=x11` so its windows run under **XWayland** as normal keep-above
windows that GNOME maps reliably (works on both Ubuntu X11 and Wayland
sessions). The floating pet's always-on-top / skip-taskbar / click-through bits
are set via raw X11 (`x11rb`) on the window XID, since GTK4 removed those WM
hints. The tray uses **StatusNotifierItem** (`ksni`) and requires the GNOME
**AppIndicator** extension.

## Workspace layout

```
crates/
  agentpet-core/   # pure, GTK-free, unit-tested domain logic
  agentpet/        # platform binary (GTK4/X11/tray/IPC) — added in a later phase
```

Run the core test suite (no display server needed):

```bash
cargo test -p agentpet-core
```

## License

MIT. Application code only; pet assets are owned by their submitters (served via
[Petdex](https://github.com/crafter-station/petdex)).
