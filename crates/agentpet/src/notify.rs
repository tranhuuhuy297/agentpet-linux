//! Desktop notifications on agent state transitions. Ports the notification
//! half of `NotificationManager.swift` / `AppDaemon.notifyIfNeeded`.
//!
//! Sound is intentionally deferred to the GTK phase (it needs `libasound2-dev`
//! via `rodio`, installed alongside the GUI deps).

use agentpet_core::session::AgentSession;
use agentpet_core::state::AgentState;

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

/// Shows a desktop notification; a missing notification daemon is non-fatal.
pub fn notify(title: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .appname("AgentPet")
        .show();
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
