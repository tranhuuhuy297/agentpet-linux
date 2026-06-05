//! Reduces all sessions to a single pet mood. Ports `MoodResolver` from
//! `PetMood.swift`.

use crate::session::AgentSession;
use crate::state::{AgentState, PetMood};

pub struct MoodResolver;

impl MoodResolver {
    /// Reduces all sessions to a single mood by attention priority.
    /// `Celebrate` is never returned here; it is a transient the pet controller
    /// plays when entering `Done` (see the app layer).
    pub fn aggregate(sessions: &[AgentSession]) -> PetMood {
        // Running work takes priority: the pet reflects what is active now.
        // `Registered` (agent open but idle) is not "working".
        if sessions.iter().any(|s| s.state == AgentState::Working) {
            return PetMood::Working;
        }
        if sessions.iter().any(|s| s.state == AgentState::Waiting) {
            return PetMood::Waiting;
        }
        if sessions.iter().any(|s| s.state == AgentState::Done) {
            return PetMood::Done;
        }
        PetMood::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AgentKind, AgentSource};

    fn session(state: AgentState, id: &str) -> AgentSession {
        AgentSession::new(id, AgentKind::Claude, None, state, None, AgentSource::Hook, 0.0)
    }

    #[test]
    fn empty_is_idle() {
        assert_eq!(MoodResolver::aggregate(&[]), PetMood::Idle);
    }

    #[test]
    fn working_wins() {
        let s = [
            session(AgentState::Working, "a"),
            session(AgentState::Waiting, "b"),
            session(AgentState::Done, "c"),
        ];
        assert_eq!(MoodResolver::aggregate(&s), PetMood::Working);
    }

    #[test]
    fn waiting_beats_done() {
        let s = [session(AgentState::Done, "a"), session(AgentState::Waiting, "b")];
        assert_eq!(MoodResolver::aggregate(&s), PetMood::Waiting);
    }

    #[test]
    fn registered_is_not_working() {
        assert_eq!(MoodResolver::aggregate(&[session(AgentState::Registered, "a")]), PetMood::Idle);
        assert_eq!(
            MoodResolver::aggregate(&[
                session(AgentState::Registered, "a"),
                session(AgentState::Working, "b"),
            ]),
            PetMood::Working
        );
    }

    #[test]
    fn done_only() {
        let s = [session(AgentState::Done, "a"), session(AgentState::Idle, "b")];
        assert_eq!(MoodResolver::aggregate(&s), PetMood::Done);
    }
}
