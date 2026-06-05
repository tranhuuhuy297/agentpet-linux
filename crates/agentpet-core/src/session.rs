//! In-memory session store with time-based pruning. Ports `AgentSession.swift`
//! and `SessionStore.swift`.
//!
//! Pure logic, deliberately free of wall-clock reads: callers pass `now` so
//! behaviour is deterministic and testable.

use crate::event::AgentEvent;
use crate::mapper::StateMapper;
use crate::state::{AgentKind, AgentSource, AgentState, UnixTime};
use std::collections::HashMap;

/// Current known state of one agent session.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentSession {
    pub id: String,
    pub agent_kind: AgentKind,
    pub project: Option<String>,
    pub state: AgentState,
    pub message: Option<String>,
    pub source: AgentSource,
    pub updated_at: UnixTime,
    /// When the session entered its current `state`; resets on state change.
    pub state_since: UnixTime,
}

impl AgentSession {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        agent_kind: AgentKind,
        project: Option<String>,
        state: AgentState,
        message: Option<String>,
        source: AgentSource,
        updated_at: UnixTime,
    ) -> Self {
        Self {
            id: id.into(),
            agent_kind,
            project,
            state,
            message,
            source,
            updated_at,
            state_since: updated_at,
        }
    }
}

/// In-memory store of agent sessions, keyed by session id.
pub struct SessionStore {
    /// `done` sessions fall back to `idle` after this much quiet time.
    pub done_to_idle_after: UnixTime,
    /// `idle` sessions are removed after this much quiet time.
    pub remove_idle_after: UnixTime,
    /// Working/waiting sessions with no update for this long are removed: the
    /// agent almost certainly died without a `Stop` event.
    pub stale_active_after: UnixTime,
    /// A merely `registered` session (agent open but never started working) is
    /// dropped sooner.
    pub stale_registered_after: UnixTime,

    by_id: HashMap<String, AgentSession>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    pub fn new() -> Self {
        Self::with_timeouts(30.0, 600.0, 300.0, 90.0)
    }

    pub fn with_timeouts(
        done_to_idle_after: UnixTime,
        remove_idle_after: UnixTime,
        stale_active_after: UnixTime,
        stale_registered_after: UnixTime,
    ) -> Self {
        Self {
            done_to_idle_after,
            remove_idle_after,
            stale_active_after,
            stale_registered_after,
            by_id: HashMap::new(),
        }
    }

    /// Removes all sessions (e.g. after the user disconnects an integration).
    pub fn clear(&mut self) {
        self.by_id.clear();
    }

    /// Removes a single session (e.g. dismissing a stuck agent).
    pub fn remove(&mut self, id: &str) {
        self.by_id.remove(id);
    }

    /// Applies an event, creating or updating the matching session.
    /// Returns the updated session, or `None` if the event maps to no state.
    pub fn apply(&mut self, event: &AgentEvent, now: UnixTime) -> Option<AgentSession> {
        // A session-end event (agent quit/closed) removes the session at once,
        // so it doesn't linger as "done" until the idle timeout.
        if StateMapper::is_session_end(event.agent_kind, &event.event_name) {
            self.by_id.remove(&event.session_id);
            return None;
        }
        let state = StateMapper::state(event.agent_kind, &event.event_name)?;

        if let Some(existing) = self.by_id.get_mut(&event.session_id) {
            if existing.state != state {
                existing.state_since = now;
            }
            existing.state = state;
            existing.updated_at = now;
            if let Some(project) = &event.project {
                existing.project = Some(project.clone());
            }
            existing.message = event.message.clone();
            return Some(existing.clone());
        }

        let session = AgentSession::new(
            event.session_id.clone(),
            event.agent_kind,
            event.project.clone(),
            state,
            event.message.clone(),
            AgentSource::Hook,
            now,
        );
        self.by_id.insert(event.session_id.clone(), session.clone());
        Some(session)
    }

    /// Demotes stale `done` sessions to `idle`, removes long-idle ones, and
    /// drops active sessions that have gone quiet (agent died without `Stop`).
    pub fn prune(&mut self, now: UnixTime) {
        let ids: Vec<String> = self.by_id.keys().cloned().collect();
        for id in ids {
            let Some(session) = self.by_id.get(&id) else { continue };
            let quiet = now - session.updated_at;
            match session.state {
                AgentState::Done => {
                    if quiet >= self.done_to_idle_after {
                        let s = self.by_id.get_mut(&id).unwrap();
                        s.state = AgentState::Idle;
                        s.updated_at = now;
                        s.state_since = now;
                    }
                }
                AgentState::Idle => {
                    if quiet >= self.remove_idle_after {
                        self.by_id.remove(&id);
                    }
                }
                AgentState::Registered => {
                    if quiet >= self.stale_registered_after {
                        self.by_id.remove(&id);
                    }
                }
                AgentState::Working | AgentState::Waiting => {
                    if quiet >= self.stale_active_after {
                        self.by_id.remove(&id);
                    }
                }
            }
        }
    }

    pub fn sessions(&self) -> Vec<AgentSession> {
        self.by_id.values().cloned().collect()
    }

    /// Sessions ordered by attention priority then recency, for display.
    pub fn sorted(&self) -> Vec<AgentSession> {
        let mut list: Vec<AgentSession> = self.by_id.values().cloned().collect();
        list.sort_by(|a, b| {
            let pa = a.state.attention_priority();
            let pb = b.state.attention_priority();
            if pa != pb {
                return pb.cmp(&pa);
            }
            b.updated_at.total_cmp(&a.updated_at)
        });
        list
    }

    pub fn session(&self, id: &str) -> Option<&AgentSession> {
        self.by_id.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: UnixTime = 1_000_000.0;

    fn event(name: &str, session: &str, project: Option<&str>) -> AgentEvent {
        AgentEvent::new(
            session,
            AgentKind::Claude,
            name,
            project.map(|p| p.to_string()),
            None,
            T0,
        )
    }

    #[test]
    fn apply_creates_session() {
        let mut store = SessionStore::new();
        let s = store.apply(&event("SessionStart", "s1", Some("/proj")), T0).unwrap();
        assert_eq!(s.state, AgentState::Registered);
        assert_eq!(s.project.as_deref(), Some("/proj"));
        assert_eq!(s.source, AgentSource::Hook);
        assert_eq!(store.sessions().len(), 1);
    }

    #[test]
    fn apply_updates_existing_and_keeps_project_when_nil() {
        let mut store = SessionStore::new();
        store.apply(&event("SessionStart", "s1", Some("/proj")), T0);
        let updated = store.apply(&event("Stop", "s1", None), T0 + 5.0).unwrap();
        assert_eq!(updated.state, AgentState::Done);
        assert_eq!(updated.project.as_deref(), Some("/proj"), "project persists when event omits it");
        assert_eq!(store.sessions().len(), 1);
    }

    #[test]
    fn apply_ignores_unmapped_event() {
        let mut store = SessionStore::new();
        assert!(store.apply(&event("Bogus", "s1", None), T0).is_none());
        assert_eq!(store.sessions().len(), 0);
    }

    #[test]
    fn session_end_removes_session() {
        let mut store = SessionStore::new();
        assert!(store.apply(&event("SessionStart", "s1", Some("/p")), T0).is_some());
        assert_eq!(store.sessions().len(), 1);
        assert!(store.apply(&event("SessionEnd", "s1", Some("/p")), T0).is_none());
        assert_eq!(store.sessions().len(), 0);
    }

    #[test]
    fn prune_demotes_done_to_idle() {
        let mut store = SessionStore::new();
        store.apply(&event("Stop", "s1", None), T0);
        store.prune(T0 + 10.0);
        assert_eq!(store.session("s1").map(|s| s.state), Some(AgentState::Done));
        store.prune(T0 + 40.0);
        assert_eq!(store.session("s1").map(|s| s.state), Some(AgentState::Idle));
    }

    #[test]
    fn prune_removes_long_idle() {
        let mut store = SessionStore::new();
        store.apply(&event("Stop", "s1", None), T0);
        store.prune(T0 + 40.0); // -> idle at T0+40
        store.prune(T0 + 40.0 + 600.0);
        assert!(store.session("s1").is_none());
    }

    #[test]
    fn prune_removes_stale_active_session() {
        let mut store = SessionStore::new();
        store.apply(&event("UserPromptSubmit", "s1", None), T0); // working
        store.prune(T0 + 120.0);
        assert!(store.session("s1").is_some());
        store.prune(T0 + 300.0);
        assert!(store.session("s1").is_none());
    }

    #[test]
    fn prune_removes_stale_registered_sooner() {
        let mut store = SessionStore::new();
        store.apply(&event("SessionStart", "s1", None), T0); // registered, never worked
        store.prune(T0 + 60.0);
        assert!(store.session("s1").is_some());
        store.prune(T0 + 90.0);
        assert!(store.session("s1").is_none());
    }

    #[test]
    fn clear_removes_all() {
        let mut store = SessionStore::new();
        store.apply(&event("UserPromptSubmit", "a", None), T0);
        store.apply(&event("UserPromptSubmit", "b", None), T0);
        store.clear();
        assert!(store.sessions().is_empty());
    }

    #[test]
    fn sorted_by_attention_priority() {
        let mut store = SessionStore::new();
        store.apply(&event("UserPromptSubmit", "working", None), T0);
        store.apply(&event("Notification", "waiting", None), T0);
        store.apply(&event("Stop", "done", None), T0);
        let order: Vec<String> = store.sorted().into_iter().map(|s| s.id).collect();
        assert_eq!(order, vec!["working", "waiting", "done"]);
    }
}
