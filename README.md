# AgentPet for Linux

A native **Rust + GTK4** port of [AgentPet](https://github.com/ntd4996/agentpet)
(macOS) for **Ubuntu 22.04+**. Watch several AI coding agents (Claude Code,
Codex, Gemini CLI, Cursor, opencode, Windsurf) running in parallel and see — at
a glance — which one is **working**, which is **done**, and which is **waiting
for your input**, via a tray monitor and an ambient desktop pet.

> Status: **working.** Core logic, CLI/daemon, the GTK pet + monitor + settings,
> the Petdex gallery, notifications, and packaging are all implemented and
> validated on GNOME Wayland (via XWayland).

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

## Install

Install the build dependencies (Ubuntu 22.04+):

```bash
sudo apt update && sudo apt install -y \
  libgtk-4-dev libadwaita-1-dev build-essential pkg-config \
  libasound2-dev libx11-dev libxcb1-dev
```

Then build + install for your user (no root):

```bash
./install.sh        # builds release, installs to ~/.local
```

Launch **AgentPet** from your app menu, or run `agentpet`. On first launch it
opens Settings — flip on the agents you use (writes their hook configs) and pick
a pet from the gallery.

- **Tray icon** needs the GNOME *AppIndicator and KStatusNotifierItem Support*
  extension. Without it the app still runs; reach Settings/Quit from the monitor
  window (right-click the pet).
- **Portable build:** `./scripts/build-appimage.sh` produces an AppImage that
  bundles GTK4/libadwaita for older distros.
- **Update:** `agentpet update` pulls the latest GitHub release.

### Uninstall

```bash
./uninstall.sh              # or: ./install.sh uninstall
```

The exact inverse of install: `agentpet uninstall` strips AgentPet's own hook
entries from every agent config (foreign hooks untouched) and disables
launch-at-login; then the three installed files and `~/.agentpet` are removed.
Pass `--keep-data` to preserve `~/.agentpet` (queue + downloaded pets).

## Usage

- **Claude Code, Codex, Gemini, Cursor, opencode, Windsurf:** toggle them on in
  Settings → General (installs the hook). The pet then reflects each session's
  real state, including "waiting for input".
- **Any other CLI agent:** `agentpet run -- <command>` (e.g. `agentpet run -- aider`).

## Workspace layout

```
crates/
  agentpet-core/   # pure, GTK-free, unit-tested domain logic
  agentpet/        # platform binary: clap-free dispatch, tokio IPC daemon,
                   # GTK pet/monitor/settings, ksni tray, Petdex client
  pet-spike/       # Phase-0 click-through window feasibility spike
```

Run the core test suite (no display server needed):

```bash
cargo test -p agentpet-core -p agentpet
```

## Credits

This project is a Linux port of [AgentPet](https://github.com/ntd4996/agentpet)
by [@ntd4996](https://github.com/ntd4996) — the original macOS app that defined
the concept, agent hook integrations, and pet/monitor UX. All credit for the
original idea and design goes to that project.

## License

MIT. Application code only; pet assets are owned by their submitters (served via
[Petdex](https://github.com/crafter-station/petdex)).
