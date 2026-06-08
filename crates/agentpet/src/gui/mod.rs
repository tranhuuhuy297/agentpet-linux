//! GTK application entry: runs the socket server on a Tokio thread, builds the
//! UI on the GTK main thread, and bridges them with async-channels. Ports
//! `AgentPetApp.swift` + the macOS `AppDaemon` wiring.

use crate::snapshot::{GalleryRequest, GalleryResult, UiCommand, UiUpdate};
use gtk4::glib;
use gtk4::prelude::*;
use std::process::ExitCode;
use std::rc::Rc;

const APP_ID: &str = "online.thenightwatcher.agentpet";

pub fn run_gui() -> ExitCode {
    // Mirror the spike: force the X11 backend so the pet runs under XWayland.
    // SAFETY: set before any GDK/display init.
    unsafe { std::env::set_var("GDK_BACKEND", "x11") };

    // A hook stores the absolute binary path captured when the user toggled it
    // on. If the binary later moves (reinstall to a new prefix, AppImage
    // remount, manual move) the stored path goes stale while the toggle still
    // reads as "installed". Heal already-enabled agents to the current path on
    // startup. Best-effort: never block launch on a config rewrite.
    resync_agent_hooks();

    let (ui_tx, ui_rx) = async_channel::unbounded::<UiUpdate>();
    let (cmd_tx, cmd_rx) = async_channel::unbounded::<UiCommand>();
    // Signals the GTK side to (re)load the selected pet pack after a download.
    let (reload_tx, reload_rx) = async_channel::unbounded::<()>();
    // Gallery requests (GTK → tokio) and results (tokio → GTK).
    let (gallery_tx, gallery_rx) = async_channel::unbounded::<GalleryRequest>();
    let (gallery_result_tx, gallery_result_rx) = async_channel::unbounded::<GalleryResult>();

    // Socket server + session store on a dedicated Tokio thread, alongside the
    // starter-pet bootstrap and the gallery worker (both network-bound).
    let reload_for_gallery = reload_tx.clone();
    std::thread::spawn(move || match crate::daemon::build_runtime() {
        Ok(rt) => rt.block_on(async move {
            tokio::spawn(crate::petdex::bootstrap_if_needed(reload_tx));
            tokio::spawn(crate::petdex::gallery_worker(
                gallery_rx,
                gallery_result_tx,
                reload_for_gallery,
            ));
            crate::daemon::serve(Some(ui_tx)).await;
        }),
        Err(e) => eprintln!("agentpet: runtime error: {e}"),
    });

    let app = gtk4::Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| {
        let ui = Rc::new(crate::ui::Ui::build(app, cmd_tx.clone(), gallery_tx.clone()));
        ui.reload_pet(); // load any already-installed pack at startup

        // Gallery results (manifest / download outcomes) → settings window.
        {
            let (ui, rx) = (ui.clone(), gallery_result_rx.clone());
            glib::MainContext::default().spawn_local(async move {
                while let Ok(result) = rx.recv().await {
                    ui.apply_gallery_result(result);
                }
            });
        }

        // First-run onboarding: open Settings so the user connects an agent.
        let mut cfg = agentpet_core::config::Config::load();
        if !cfg.has_onboarded {
            ui.show_settings();
            cfg.has_onboarded = true;
            let _ = cfg.save();
        }

        // Reload the pet pack when the bootstrap/gallery signals one is ready.
        {
            let (ui, reload_rx) = (ui.clone(), reload_rx.clone());
            glib::MainContext::default().spawn_local(async move {
                while reload_rx.recv().await.is_ok() {
                    ui.reload_pet();
                }
            });
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
