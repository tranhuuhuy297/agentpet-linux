//! Aggregates the on-screen surfaces (pets + monitor + tray) and applies each
//! `UiUpdate` to them. Lives entirely on the GTK main thread.
//!
//! There is one pet window per *active* agent kind (Claude, Codex, …): a pet
//! appears when its agent has a live, attention-worthy session and is closed
//! when that agent goes idle. Each agent renders its own configured pet pack.

pub mod monitor;
pub mod settings;
pub mod tray;

use crate::pet::PetWindow;
use crate::snapshot::{UiCommand, UiUpdate};
use agentpet_core::catalog::AgentCatalog;
use agentpet_core::config::Config;
use agentpet_core::sprite::{load_pack, PetPack};
use agentpet_core::state::AgentKind;
use gtk4::gio;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

pub struct Ui {
    app: gtk4::Application,
    cmd: async_channel::Sender<UiCommand>,
    /// One pet window per currently-active agent kind.
    pets: RefCell<HashMap<AgentKind, PetWindow>>,
    monitor: monitor::MonitorWindow,
    settings: settings::SettingsWindow,
    tray: Option<ksni::blocking::Handle<tray::AgentTray>>,
    // Keeps the GtkApplication alive even when no pet window is visible (pets
    // come and go with agent activity, so without this the app could exit).
    _hold: gio::ApplicationHoldGuard,
}

impl Ui {
    pub fn build(app: &gtk4::Application, cmd: async_channel::Sender<UiCommand>) -> Self {
        let hold = app.hold();
        let monitor = monitor::MonitorWindow::new(app, cmd.clone());
        let settings = settings::SettingsWindow::new(app, cmd.clone());
        let tray = tray::spawn(cmd.clone());
        Ui {
            app: app.clone(),
            cmd,
            pets: RefCell::new(HashMap::new()),
            monitor,
            settings,
            tray,
            _hold: hold,
        }
    }

    pub fn show_settings(&self) {
        self.settings.show();
    }

    pub fn apply(&self, update: &UiUpdate) {
        self.sync_pets(&update.moods);
        self.monitor.set_sessions(&update.sessions);
        let (running, waiting) = (update.running, update.waiting);
        if let Some(tray) = &self.tray {
            tray.update(move |t| {
                t.running = running;
                t.waiting = waiting;
            });
        }
    }

    /// Reconciles the live pet windows with the agents that should be showing
    /// one: update existing pets' moods, spawn pets for newly-active agents,
    /// and close pets whose agent is no longer active.
    fn sync_pets(&self, moods: &[(AgentKind, agentpet_core::state::PetMood)]) {
        let mut pets = self.pets.borrow_mut();
        let active: Vec<AgentKind> = moods.iter().map(|(k, _)| *k).collect();

        // Close pets for agents that went idle/ended.
        pets.retain(|kind, pet| {
            let keep = active.contains(kind);
            if !keep {
                pet.close();
            }
            keep
        });

        // Create or update a pet for each active agent.
        for (kind, mood) in moods {
            if let Some(pet) = pets.get(kind) {
                pet.set_mood(*mood);
            } else {
                let pet = PetWindow::new(
                    &self.app,
                    self.cmd.clone(),
                    kind_slot(*kind),
                    agent_display_name(*kind),
                );
                pet.set_pack(load_pack_for_kind(*kind).as_ref());
                pet.set_mood(*mood);
                pets.insert(*kind, pet);
            }
        }
    }

    pub fn show_monitor(&self) {
        self.monitor.show();
    }

    /// Reloads each live pet's configured pack (after a download or a per-agent
    /// pet selection change). Pets that aren't currently shown pick up the new
    /// pack the next time their agent becomes active.
    pub fn reload_pet(&self) {
        for (kind, pet) in self.pets.borrow().iter() {
            pet.set_pack(load_pack_for_kind(*kind).as_ref());
        }
    }
}

/// Human-readable agent name drawn under its pet. Uses the Settings catalog
/// name where available, with fallbacks for the wrapper-only kinds.
fn agent_display_name(kind: AgentKind) -> &'static str {
    AgentCatalog::all()
        .into_iter()
        .find(|a| a.kind == kind)
        .map(|a| a.display_name)
        .unwrap_or(match kind {
            AgentKind::Cli => "CLI",
            _ => "Agent",
        })
}

/// Stable horizontal placement slot for an agent's pet, so each agent keeps a
/// consistent spot. Mirrors `AgentKind`'s declaration order.
fn kind_slot(kind: AgentKind) -> i32 {
    match kind {
        AgentKind::Claude => 0,
        AgentKind::Codex => 1,
        AgentKind::Cli => 2,
        AgentKind::Unknown => 3,
    }
}

/// Loads the pack configured for `kind` (its own pick, else the global default),
/// falling back to the first installed pack that slices successfully.
fn load_pack_for_kind(kind: AgentKind) -> Option<PetPack> {
    let cfg = Config::load();
    let want = cfg.pet_id_for(kind);
    let mut first = None;
    if let Ok(entries) = std::fs::read_dir(crate::petdex::installed_dir()) {
        for entry in entries.flatten() {
            if let Some(pack) = load_pack(&entry.path()) {
                if want == Some(pack.manifest.id.as_str()) {
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
