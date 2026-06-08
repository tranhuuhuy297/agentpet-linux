//! The wire event sent by the hook CLI to the daemon. Ports `AgentEvent.swift`.

use crate::state::{AgentKind, UnixTime};
use serde::{Deserialize, Serialize};

/// A single state-change report from an agent. `event_name` is the agent-native
/// event (e.g. Claude Code's "Stop"); `StateMapper` turns it into an
/// `AgentState`.
///
/// Field names are camelCase on the wire to mirror the macOS encoder, and the
/// timestamp is seconds since the Unix epoch (matching `.secondsSince1970`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    pub session_id: String,
    pub agent_kind: AgentKind,
    pub event_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub timestamp: UnixTime,
}

impl AgentEvent {
    pub fn new(
        session_id: impl Into<String>,
        agent_kind: AgentKind,
        event_name: impl Into<String>,
        project: Option<String>,
        message: Option<String>,
        timestamp: UnixTime,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            agent_kind,
            event_name: event_name.into(),
            project,
            message,
            timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_and_omits_nil_optionals() {
        let ev = AgentEvent::new("s1", AgentKind::Claude, "Stop", None, None, 1_000_000.0);
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("\"sessionId\":\"s1\""));
        assert!(json.contains("\"agentKind\":\"claude\""));
        assert!(json.contains("\"eventName\":\"Stop\""));
        assert!(!json.contains("project"), "nil optionals omitted, not null");
        let back: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ev);
    }

    #[test]
    fn decodes_with_project_and_message() {
        let ev = AgentEvent::new(
            "s1",
            AgentKind::Codex,
            "Stop",
            Some("/proj".into()),
            Some("Using Bash".into()),
            42.0,
        );
        let back: AgentEvent =
            serde_json::from_str(&serde_json::to_string(&ev).unwrap()).unwrap();
        assert_eq!(back.project.as_deref(), Some("/proj"));
        assert_eq!(back.message.as_deref(), Some("Using Bash"));
    }
}
