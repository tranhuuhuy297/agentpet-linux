//! Aggregates the on-screen surfaces (pet + monitor + tray) and applies each
//! `UiUpdate` to them. Lives entirely on the GTK main thread.

pub mod monitor;
pub mod tray;

use crate::pet::PetWindow;
use crate::snapshot::{UiCommand, UiUpdate};
use agentpet_core::config::Config;
use agentpet_core::sprite::{load_pack, PetPack};
use gtk4::gio;
use gtk4::prelude::*;

pub struct Ui {
    pet: PetWindow,
    monitor: monitor::MonitorWindow,
    tray: Option<ksni::blocking::Handle<tray::AgentTray>>,
    // Keeps the GtkApplication alive even when no window is visible (the pet is
    // always present, but this guards against it being closed).
    _hold: gio::ApplicationHoldGuard,
}

impl Ui {
    pub fn build(app: &gtk4::Application, cmd: async_channel::Sender<UiCommand>) -> Self {
        let hold = app.hold();
        let pet = PetWindow::new(app, cmd.clone());
        let monitor = monitor::MonitorWindow::new(app);
        let tray = tray::spawn(cmd);
        Ui { pet, monitor, tray, _hold: hold }
    }

    pub fn apply(&self, update: &UiUpdate) {
        self.pet.set_mood(update.mood);
        self.monitor.set_sessions(&update.sessions);
        let (running, waiting) = (update.running, update.waiting);
        if let Some(tray) = &self.tray {
            tray.update(move |t| {
                t.running = running;
                t.waiting = waiting;
            });
        }
    }

    pub fn show_monitor(&self) {
        self.monitor.show();
    }

    /// Re-scans installed pet packs and loads the selected one into the pet
    /// (falling back to the first available, or the blob if none).
    pub fn reload_pet(&self) {
        self.pet.set_pack(load_selected_pack().as_ref());
    }
}

/// Loads the pack whose manifest id matches the config selection, else the first
/// installed pack that slices successfully.
fn load_selected_pack() -> Option<PetPack> {
    let cfg = Config::load();
    let mut first = None;
    if let Ok(entries) = std::fs::read_dir(crate::petdex::pets_dir()) {
        for entry in entries.flatten() {
            if let Some(pack) = load_pack(&entry.path()) {
                if cfg.selected_pet_id.as_deref() == Some(pack.manifest.id.as_str()) {
                    return Some(pack);
                }
                if first.is_none() {
                    first = Some(pack);
                }
            }
        }
    }
    first
}
