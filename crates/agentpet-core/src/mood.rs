//! Reduces all sessions to a single pet mood. Ports `MoodResolver` from
//! `PetMood.swift`.

use crate::session::AgentSession;
use crate::state::{AgentKind, AgentState, PetMood};

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

    /// One mood per agent kind that currently warrants its own visible pet.
    /// Sessions are grouped by `agent_kind`, each group reduced via `aggregate`,
    /// and kinds whose mood is `Idle` (nothing to show) are dropped — so a pet
    /// exists only while that agent has a live, attention-worthy session.
    /// Ordered by `AgentKind` so each agent's pet keeps a stable placement slot.
    pub fn aggregate_by_kind(sessions: &[AgentSession]) -> Vec<(AgentKind, PetMood)> {
        let mut kinds: Vec<AgentKind> = sessions.iter().map(|s| s.agent_kind).collect();
        kinds.sort();
        kinds.dedup();
        kinds
            .into_iter()
            .filter_map(|kind| {
                let group: Vec<AgentSession> =
                    sessions.iter().filter(|s| s.agent_kind == kind).cloned().collect();
                let mood = Self::aggregate(&group);
                (mood != PetMood::Idle).then_some((kind, mood))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AgentKind, AgentSource};

    fn session(state: AgentState, id: &str) -> AgentSession {
        AgentSession::new(id, AgentKind::Claude, None, state, None, AgentSource::Hook, 0.0)
    }

    fn session_of(kind: AgentKind, state: AgentState, id: &str) -> AgentSession {
        AgentSession::new(id, kind, None, state, None, AgentSource::Hook, 0.0)
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

    #[test]
    fn by_kind_is_empty_when_no_sessions() {
        assert!(MoodResolver::aggregate_by_kind(&[]).is_empty());
    }

    #[test]
    fn by_kind_gives_each_agent_its_own_mood() {
        // Claude working, Codex waiting -> two pets, each its real mood
        // (the old single-pet aggregate would have shown only "working").
        let s = [
            session_of(AgentKind::Claude, AgentState::Working, "c1"),
            session_of(AgentKind::Codex, AgentState::Waiting, "x1"),
        ];
        assert_eq!(
            MoodResolver::aggregate_by_kind(&s),
            vec![(AgentKind::Claude, PetMood::Working), (AgentKind::Codex, PetMood::Waiting)]
        );
    }

    #[test]
    fn by_kind_aggregates_within_a_kind() {
        // Two Claude sessions collapse into one Claude pet (working wins).
        let s = [
            session_of(AgentKind::Claude, AgentState::Waiting, "c1"),
            session_of(AgentKind::Claude, AgentState::Working, "c2"),
        ];
        assert_eq!(
            MoodResolver::aggregate_by_kind(&s),
            vec![(AgentKind::Claude, PetMood::Working)]
        );
    }

    #[test]
    fn by_kind_drops_idle_kinds() {
        // An agent whose sessions are all idle/registered gets no pet.
        let s = [
            session_of(AgentKind::Claude, AgentState::Idle, "c1"),
            session_of(AgentKind::Claude, AgentState::Registered, "c2"),
            session_of(AgentKind::Codex, AgentState::Working, "x1"),
        ];
        assert_eq!(
            MoodResolver::aggregate_by_kind(&s),
            vec![(AgentKind::Codex, PetMood::Working)]
        );
    }
}
