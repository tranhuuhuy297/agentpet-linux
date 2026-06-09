//! GTK application entry: runs the socket server on a Tokio thread, builds the
//! UI on the GTK main thread, and bridges them with async-channels. Ports
//! `AgentPetApp.swift` + the macOS `AppDaemon` wiring.

use crate::snapshot::{UiCommand, UiUpdate};
use gtk4::glib;
use gtk4::prelude::*;
use std::process::ExitCode;
use std::rc::Rc;

const APP_ID: &str = "io.github.tranhuuhuy297.agentpet";

pub fn run_gui() -> ExitCode {
    // Mirror the spike: force the X11 backend so the pet runs under XWayland.
    // SAFETY: set before any GDK/display init.
    unsafe { std::env::set_var("GDK_BACKEND", "x11") };

    // Authoritative single-instance guard. The socket server below carries its
    // own probe, but it runs on a background thread whose exit code is dropped —
    // so without this, a second launch (e.g. the autostart entry firing while a
    // pet is already open at login) would still build a duplicate tray icon and
    // pet. Take the lock before any side effect and bail if one is already held.
    let _lock = match crate::daemon::single_instance::acquire() {
        Some(lock) => lock,
        None => {
            eprintln!("agentpet is already running");
            return ExitCode::SUCCESS;
        }
    };

    // A hook stores the absolute binary path captured when the user toggled it
    // on. If the binary later moves (reinstall to a new prefix, AppImage
    // remount, manual move) the stored path goes stale while the toggle still
    // reads as "installed". Heal already-enabled agents to the current path on
    // startup. Best-effort: never block launch on a config rewrite.
    resync_agent_hooks();

    let (ui_tx, ui_rx) = async_channel::unbounded::<UiUpdate>();
    let (cmd_tx, cmd_rx) = async_channel::unbounded::<UiCommand>();

    // Socket server + session store on a dedicated Tokio thread.
    std::thread::spawn(move || match crate::daemon::build_runtime() {
        Ok(rt) => rt.block_on(async move {
            crate::daemon::serve(Some(ui_tx)).await;
        }),
        Err(e) => eprintln!("agentpet: runtime error: {e}"),
    });

    let app = gtk4::Application::builder().application_id(APP_ID).build();
    // GTK fires `activate` once per launch *and again* whenever the app is
    // re-activated (a duplicate launch forwards activate to this primary
    // instance). Building the UI on every activate spawns a second tray icon
    // and a second pet in the same process. Build the surfaces exactly once.
    let built = std::cell::Cell::new(false);
    app.connect_activate(move |app| {
        if built.replace(true) {
            return;
        }
        let ui = Rc::new(crate::ui::Ui::build(app, cmd_tx.clone()));
        ui.reload_pet(); // load any already-installed pack at startup

        // First-run onboarding: open Settings so the user connects an agent.
        let mut cfg = agentpet_core::config::Config::load();
        if !cfg.has_onboarded {
            ui.show_settings();
            cfg.has_onboarded = true;
            let _ = cfg.save();
        }

        // Apply session snapshots as they arrive.
        {
            let (ui, ui_rx) = (ui.clone(), ui_rx.clone());
            glib::MainContext::default().spawn_local(async move {
                while let Ok(update) = ui_rx.recv().await {
                    ui.apply(&update);
                }
            });
        }

        // Handle tray/pet commands.
        {
            let (ui, app, cmd_rx) = (ui.clone(), app.clone(), cmd_rx.clone());
            glib::MainContext::default().spawn_local(async move {
                while let Ok(cmd) = cmd_rx.recv().await {
                    match cmd {
                        UiCommand::ShowMonitor => ui.show_monitor(),
                        UiCommand::OpenSettings => ui.show_settings(),
                        UiCommand::Quit => app.quit(),
                        UiCommand::ReloadPets => ui.reload_pet(),
                        UiCommand::ResizePets(px) => ui.resize_pets(px),
                    }
                }
            });
        }
    });

    // No positional args — don't treat anything as files to open.
    let _ = app.run_with_args::<&str>(&[]);
    ExitCode::SUCCESS
}

/// Rewrites every already-enabled agent hook so its embedded binary path points
/// at the currently running binary. The command is built exactly like the
/// Settings toggle does, so a healed hook is byte-for-byte what the toggle would
/// write. `resync_command_to_disk` skips agents that aren't installed, so this
/// never enables an integration on the user's behalf.
fn resync_agent_hooks() {
    use agentpet_core::catalog::AgentCatalog;
    use agentpet_core::hooks::{AgentHooks, HookInstaller};

    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "agentpet".to_string());

    for agent in AgentCatalog::all() {
        let Some(spec) = AgentHooks::spec(agent.kind) else { continue };
        let command = format!("\"{}\" hook --agent {}", exe, agent.kind.raw());
        match HookInstaller::resync_command_to_disk(&command, &spec.settings_path, &spec.events) {
            Ok(true) => eprintln!(
                "agentpet: updated {} hook to current binary path",
                agent.display_name
            ),
            Ok(false) => {}
            Err(e) => eprintln!("agentpet: failed to resync {} hook: {e}", agent.display_name),
        }
    }
}
