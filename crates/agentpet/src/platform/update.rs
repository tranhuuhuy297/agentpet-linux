//! `agentpet update` — replaces the running binary with the latest GitHub
//! release. Ports the Sparkle auto-update role for Linux. Activates once the
//! repo publishes tagged releases with a tar.gz asset containing `agentpet`.

use std::process::ExitCode;

pub fn run() -> ExitCode {
    match update() {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("agentpet: update failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn update() -> Result<String, Box<dyn std::error::Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("tranhuuhuy297")
        .repo_name("agentpet-linux")
        .bin_name("agentpet")
        .current_version(env!("CARGO_PKG_VERSION"))
        .show_download_progress(true)
        .build()?
        .update()?;

    Ok(if status.updated() {
        format!("Updated to {} — restart AgentPet to apply.", status.version())
    } else {
        format!("Already up to date ({}).", status.version())
    })
}
