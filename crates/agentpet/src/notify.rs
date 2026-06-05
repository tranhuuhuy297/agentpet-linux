//! Desktop notifications (with a sound hint) on agent state transitions. Ports
//! `NotificationManager.swift` + `SoundSettings.swift`.
//!
//! notify-rust's `show()` is blocking and uses zbus-blocking internally, which
//! panics ("runtime within a runtime") if called on a tokio worker thread. So we
//! run all notifications on a dedicated plain OS thread, fed by this channel.
//! Sound is delegated to the notification daemon via the freedesktop sound
//! hint, avoiding an audio dependency.

use agentpet_core::config::Config;
use agentpet_core::session::AgentSession;
use agentpet_core::state::AgentState;
use std::sync::mpsc;
use std::sync::OnceLock;

struct Note {
    title: String,
    body: String,
    sound: Option<&'static str>,
}

static NOTIFIER: OnceLock<mpsc::Sender<Note>> = OnceLock::new();

/// Starts the notification worker thread. Call once at daemon startup.
pub fn init() {
    let (tx, rx) = mpsc::channel::<Note>();
    std::thread::spawn(move || {
        while let Ok(note) = rx.recv() {
            let mut n = notify_rust::Notification::new();
            n.summary(&note.title).body(&note.body).appname("AgentPet");
            if let Some(sound) = note.sound {
                n.sound_name(sound);
            }
            if let Err(e) = n.show() {
                eprintln!("agentpet: notification failed: {e}");
            }
        }
    });
    let _ = NOTIFIER.set(tx);
}

/// Posts a notification when a session transitions into `waiting` or `done`,
/// with a sound hint when the corresponding sound is enabled in config.
pub fn on_transition(before: Option<AgentState>, session: &AgentSession) {
    if Some(session.state) == before {
        return;
    }
    let cfg = Config::load();
    match session.state {
        AgentState::Waiting => send(
            format!("{} needs input", project_label(session)),
            session.message.clone().unwrap_or_else(|| "Waiting for you".into()),
            cfg.sound_waiting_on.then_some("message"),
        ),
        AgentState::Done => send(
            format!("{} finished", project_label(session)),
            "Agent completed its turn".into(),
            cfg.sound_done_on.then_some("complete"),
        ),
        _ => {}
    }
}

fn send(title: String, body: String, sound: Option<&'static str>) {
    if let Some(tx) = NOTIFIER.get() {
        let _ = tx.send(Note { title, body, sound });
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
