//! `agentpet uninstall` — the inverse of the Settings agent toggles.
//!
//! Removes every AgentPet hook entry it wrote into each agent's config and
//! disables the launch-at-login autostart file. Removal is keyed on our own
//! command string, so foreign hooks in the same config are never touched. The
//! file-level uninstall (binary, desktop entry, icon, `~/.agentpet`) is handled
//! by `./install.sh uninstall` (or `./uninstall.sh`), which calls this first.

use std::process::ExitCode;

use agentpet_core::catalog::AgentCatalog;
use agentpet_core::hooks::{AgentHooks, HookInstaller};

use crate::platform::autostart;

pub fn run() -> ExitCode {
    let mut removed = 0;

    for agent in AgentCatalog::all() {
        let Some(spec) = AgentHooks::spec(agent.kind) else { continue };
        if !HookInstaller::is_installed_on_disk(&spec.settings_path, &spec.events, spec.style) {
            continue;
        }
        match HookInstaller::uninstall_from_disk(&spec.settings_path, &spec.events, spec.style) {
            Ok(()) => {
                println!("removed {} hooks ({})", agent.display_name, spec.settings_path.display());
                removed += 1;
            }
            Err(e) => eprintln!("agentpet: failed to remove {} hooks: {e}", agent.display_name),
        }
    }

    if autostart::is_enabled() {
        match autostart::disable() {
            Ok(()) => println!("disabled launch-at-login ({})", autostart::desktop_path().display()),
            Err(e) => eprintln!("agentpet: failed to disable autostart: {e}"),
        }
    }

    if removed == 0 {
        println!("no agent hooks were installed — nothing to remove");
    }
    ExitCode::SUCCESS
}
