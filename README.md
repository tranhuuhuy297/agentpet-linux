# AgentPet for Linux

A native **Rust + GTK4** app for **Ubuntu 22.04+**, inspired by
[AgentPet](https://github.com/ntd4996/agentpet) (macOS). Watch your AI coding agents (Claude Code and
Codex) running in parallel and see — at a glance — which one is **working**,
which is **done**, and which is **waiting for your input**, via a tray monitor
and a desktop pet per agent (run Claude Code and Codex together and you get two
pets, each reflecting its own state).

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
                                                  ├── desktop pets: one per active agent
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

One command (Ubuntu 22.04+) — downloads the prebuilt binary from the latest
release into `~/.local`, no build and no Rust toolchain (`sudo` only if the
GTK4 runtime is missing):

```bash
curl -fsSL https://raw.githubusercontent.com/tranhuuhuy297/agentpet-linux/main/install.sh | bash
```

Prefer to build from source? Clone and run `./install.sh` — when run inside the
cloned repo it **auto-detects the checkout and builds from source** (installing
any missing build deps, compiling the release into `~/.local`):

```bash
git clone https://github.com/tranhuuhuy297/agentpet-linux && cd agentpet-linux && ./install.sh
```

Force either mode with `./install.sh --source` (always build) or
`./install.sh --binary` (always download the prebuilt release).

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

- **Claude Code, Codex:** toggle them on in Settings → General (installs the
  hook). Each agent gets its own pet that reflects its real state, including
  "waiting for input"; pick a per-agent pet on the Settings → Pet tab.
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

This project takes its idea from [AgentPet](https://github.com/ntd4996/agentpet)
by [@ntd4996](https://github.com/ntd4996) — the original macOS app that defined
the concept, agent hook integrations, and pet/monitor UX — and builds an Ubuntu
version of it. All credit for the original idea and design goes to that project.

## License

MIT. Application code only; pet assets are owned by their submitters (served via
[Petdex](https://github.com/crafter-station/petdex)).
