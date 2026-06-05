//! Desktop notifications on agent state transitions. Ports the notification
//! half of `NotificationManager.swift` / `AppDaemon.notifyIfNeeded`.
//!
//! Sound is intentionally deferred to the GTK phase (it needs `libasound2-dev`
//! via `rodio`, installed alongside the GUI deps).

use agentpet_core::session::AgentSession;
use agentpet_core::state::AgentState;
use std::sync::mpsc;
use std::sync::OnceLock;

/// notify-rust's `show()` is blocking and uses zbus-blocking internally, which
/// panics ("runtime within a runtime") if called on a tokio worker thread. So we
/// run all notifications on a dedicated plain OS thread, fed by this channel.
static NOTIFIER: OnceLock<mpsc::Sender<(String, String)>> = OnceLock::new();

/// Starts the notification worker thread. Call once at daemon startup.
pub fn init() {
    let (tx, rx) = mpsc::channel::<(String, String)>();
    std::thread::spawn(move || {
        while let Ok((title, body)) = rx.recv() {
            match notify_rust::Notification::new()
                .summary(&title)
                .body(&body)
                .appname("AgentPet")
                .show()
            {
                Ok(_) => {}
                Err(e) => eprintln!("agentpet: notification failed: {e}"),
            }
        }
    });
    let _ = NOTIFIER.set(tx);
}

/// Posts a notification when a session transitions into `waiting` or `done`.
/// `before` is the session's prior state (`None` if it's brand new).
pub fn on_transition(before: Option<AgentState>, session: &AgentSession) {
    if Some(session.state) == before {
        return;
    }
    match session.state {
        AgentState::Waiting => notify(
            &format!("{} needs input", project_label(session)),
            session.message.as_deref().unwrap_or("Waiting for you"),
        ),
        AgentState::Done => notify(
            &format!("{} finished", project_label(session)),
            "Agent completed its turn",
        ),
        _ => {}
    }
}

/// The last path component of the project, or the session id as a fallback.
fn project_label(session: &AgentSession) -> String {
    session
        .project
        .as_deref()
        .map(|p| {
            std::path::Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string())
        })
        .unwrap_or_else(|| session.id.clone())
}

/// Queues a desktop notification on the worker thread. No-op if `init()` wasn't
/// called (e.g. in tests); a missing notification daemon is non-fatal.
pub fn notify(title: &str, body: &str) {
    if let Some(tx) = NOTIFIER.get() {
        let _ = tx.send((title.to_string(), body.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentpet_core::state::{AgentKind, AgentSource};

    fn session(state: AgentState, project: Option<&str>) -> AgentSession {
        AgentSession::new(
            "s1",
            AgentKind::Claude,
            project.map(String::from),
            state,
            None,
            AgentSource::Hook,
            0.0,
        )
    }

    #[test]
    fn project_label_uses_last_path_component() {
        assert_eq!(project_label(&session(AgentState::Done, Some("/home/me/proj"))), "proj");
        assert_eq!(project_label(&session(AgentState::Done, None)), "s1");
    }
}
