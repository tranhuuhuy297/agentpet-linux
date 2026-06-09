//! The snapshot the socket-server (tokio) thread sends to the GTK main thread on
//! every session change, plus the commands the GTK side issues back. Keeping the
//! `SessionStore` on the server thread and shipping immutable snapshots avoids
//! sharing mutable state across the GTK boundary.

use agentpet_core::mood::MoodResolver;
use agentpet_core::session::AgentSession;
use agentpet_core::state::{AgentKind, AgentState, PetMood};

/// An immutable view of the current sessions, ready to render.
#[derive(Clone)]
pub struct UiUpdate {
    pub sessions: Vec<AgentSession>,
    /// One mood per agent kind that currently has a visible pet (Claude, Codex,
    /// …). Empty when nothing is active — every pet is then hidden.
    pub moods: Vec<(AgentKind, PetMood)>,
    /// Number of actively-working agents (the tray count).
    pub running: usize,
    /// Number of agents waiting on the user (turns the tray orange).
    pub waiting: usize,
}

impl UiUpdate {
    pub fn from_sessions(sessions: Vec<AgentSession>) -> Self {
        let moods = MoodResolver::aggregate_by_kind(&sessions);
        let running = sessions.iter().filter(|s| s.state == AgentState::Working).count();
        let waiting = sessions.iter().filter(|s| s.state == AgentState::Waiting).count();
        Self { sessions, moods, running, waiting }
    }
}

/// Commands the GTK side issues in response to tray/pet interaction.
#[derive(Clone, Copy, Debug)]
pub enum UiCommand {
    ShowMonitor,
    OpenSettings,
    Quit,
    /// Reload every live pet's pack (after a per-agent pet selection changes).
    ReloadPets,
    /// Resize every live pet (px) after the user moves the size slider. Carries
    /// the value so the resize is instant and touches no disk on the drag path.
    ResizePets(i32),
}
