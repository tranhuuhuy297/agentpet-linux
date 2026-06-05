//! `agentpet hook` — the tiny CLI agents call from their hook configs. Ports
//! `CLI.swift` (HookCLI). Explicit flags win; otherwise fall back to the
//! agent's hook payload on stdin, decoded with that agent's field convention.

use agentpet_core::payloads::{HookArguments, HookPayload};
use agentpet_core::state::AgentKind;
use std::io::Read;
use std::process::ExitCode;

pub fn run(args: &[String]) -> ExitCode {
    let now = crate::unix_now();
    let parsed = HookArguments::parse(args);
    let kind = parsed
        .agent
        .as_deref()
        .and_then(AgentKind::from_raw)
        .unwrap_or(AgentKind::Claude);

    let event = parsed.make_event(now).or_else(|| {
        let mut buf = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut buf);
        HookPayload::event(kind, &buf, now)
    });

    let Some(event) = event else {
        eprintln!(
            "usage: agentpet hook --event <name> --session <id> [--project <path>] [--agent <kind>] [--message <text>]\n         or pipe a hook JSON payload on stdin"
        );
        return ExitCode::from(2);
    };

    super::send_event(&event);
    ExitCode::SUCCESS
}
