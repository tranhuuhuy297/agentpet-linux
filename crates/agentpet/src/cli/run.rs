//! `agentpet run [flags] -- <command...>` — wraps any CLI agent, reporting
//! `working` while it runs (with a heartbeat) and `done` when it exits. Ports
//! `RunCLI.swift`.

use agentpet_core::event::AgentEvent;
use agentpet_core::payloads::RunArguments;
use agentpet_core::state::AgentKind;
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

/// Re-send `working` at least this often so a long run isn't pruned as stale.
const HEARTBEAT: Duration = Duration::from_secs(60);

pub fn run(args: &[String]) -> ExitCode {
    let parsed = RunArguments::parse(args);
    if parsed.command.is_empty() {
        eprintln!("usage: agentpet run [--session <id>] [--project <path>] [--agent <kind>] -- <command...>");
        return ExitCode::from(2);
    }

    let session = parsed
        .session
        .clone()
        .unwrap_or_else(|| format!("cli-{}", super::unique_token()));
    let project = parsed.project.clone().or_else(|| {
        std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    });
    let kind = parsed
        .agent
        .as_deref()
        .and_then(AgentKind::from_raw)
        .unwrap_or(AgentKind::Cli);

    let make = |state: &str| {
        AgentEvent::new(session.clone(), kind, state, project.clone(), None, crate::unix_now())
    };

    super::send_event(&make("working"));

    // After spawning, ignore terminal signals so the child owns the foreground
    // and receives SIGINT/SIGTERM cleanly (mirrors RunCLI's signal handling).
    let mut child = match Command::new(&parsed.command[0]).args(&parsed.command[1..]).spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("agentpet run: failed to start {}: {e}", parsed.command[0]);
            super::send_event(&make("done"));
            return ExitCode::from(127);
        }
    };
    ignore_terminal_signals();

    let mut last_beat = Instant::now();
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(0),
            Ok(None) => {}
            Err(_) => break 0,
        }
        std::thread::sleep(Duration::from_millis(500));
        if last_beat.elapsed() >= HEARTBEAT {
            super::send_event(&make("working"));
            last_beat = Instant::now();
        }
    };

    super::send_event(&make("done"));
    ExitCode::from(code.clamp(0, 255) as u8)
}

fn ignore_terminal_signals() {
    // SAFETY: setting SIG_IGN for SIGINT/SIGTERM is async-signal-safe and the
    // standard way to detach the wrapper from terminal job-control signals.
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_IGN);
        libc::signal(libc::SIGTERM, libc::SIG_IGN);
    }
}
