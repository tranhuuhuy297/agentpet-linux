//! AgentPet for Linux — single-binary entry point.
//!
//! Dispatches on the first argument (mirrors the macOS `AppEntry` dispatcher):
//!   - `agentpet hook …`  → fast CLI that sends one event and exits
//!   - `agentpet run -- …` → wrapper that monitors any command
//!   - (no subcommand)     → the daemon (headless for now; the GTK tray/pet
//!                            layers are added in later phases)
//!
//! GTK is intentionally NOT linked here yet, so the hot `hook`/`run` paths start
//! instantly and the daemon builds without system GUI deps.

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

mod cli;
mod daemon;
mod gui;
mod notify;
mod pet;
mod petdex;
mod platform;
mod snapshot;
mod ui;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("hook") => cli::hook::run(&args[1..]),
        Some("run") => cli::run::run(&args[1..]),
        Some("uninstall") => cli::uninstall::run(),
        Some("update") => platform::update::run(),
        Some("--version") | Some("-v") => {
            println!("agentpet {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help") | Some("-h") => {
            print_usage();
            ExitCode::SUCCESS
        }
        // No subcommand: run the app. `--headless` runs the daemon without a UI
        // (for testing on machines with no display).
        _ if args.iter().any(|a| a == "--headless") => daemon::run_headless(),
        _ => gui::run_gui(),
    }
}

fn print_usage() {
    println!(
        "agentpet {}\n\n\
         USAGE:\n  \
         agentpet                         start the daemon (monitor)\n  \
         agentpet hook [flags]            report one agent event (called from hooks)\n  \
         agentpet run [flags] -- <cmd>    wrap any command, reporting working/done\n  \
         agentpet uninstall               remove all installed agent hooks + autostart\n  \
         agentpet update                  update to the latest release\n\n\
         hook flags: --event <name> --session <id> [--project <path>] [--agent <kind>] [--message <text>]\n\
         run  flags: [--session <id>] [--project <path>] [--agent <kind>]",
        env!("CARGO_PKG_VERSION")
    );
}

/// Current wall-clock time as seconds since the Unix epoch (the wire timestamp).
pub fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}
