//! The snapshot the socket-server (tokio) thread sends to the GTK main thread on
//! every session change, plus the commands the GTK side issues back. Keeping the
//! `SessionStore` on the server thread and shipping immutable snapshots avoids
//! sharing mutable state across the GTK boundary.

use agentpet_core::mood::MoodResolver;
use agentpet_core::session::AgentSession;
use agentpet_core::state::{AgentState, PetMood};

/// An immutable view of the current sessions, ready to render.
#[derive(Clone)]
pub struct UiUpdate {
    pub sessions: Vec<AgentSession>,
    pub mood: PetMood,
    /// Number of actively-working agents (the tray count).
    pub running: usize,
    /// Number of agents waiting on the user (turns the tray orange).
    pub waiting: usize,
}

impl UiUpdate {
    pub fn from_sessions(sessions: Vec<AgentSession>) -> Self {
        let mood = MoodResolver::aggregate(&sessions);
        let running = sessions.iter().filter(|s| s.state == AgentState::Working).count();
        let waiting = sessions.iter().filter(|s| s.state == AgentState::Waiting).count();
        Self { sessions, mood, running, waiting }
    }
}

/// Commands the GTK side issues in response to tray/pet interaction.
#[derive(Clone, Copy, Debug)]
pub enum UiCommand {
    ShowMonitor,
    OpenSettings,
    Quit,
}

/// Gallery requests from the GTK side to the tokio worker (network-bound).
#[derive(Clone, Debug)]
pub enum GalleryRequest {
    Fetch,
    Download(crate::petdex::RemotePet),
}

/// Gallery results from the tokio worker back to the GTK side.
#[derive(Clone, Debug)]
pub enum GalleryResult {
    Manifest(Vec<crate::petdex::RemotePet>),
    Downloaded(String),
    Failed(String),
}
