//! Maps agent-native event names to normalised `AgentState`. Ports
//! `StateMapper.swift` 1:1 — keep the per-agent tables in sync with the source.

use crate::state::{AgentKind, AgentState};

pub struct StateMapper;

impl StateMapper {
    /// Events that mean the whole session ended (the agent was quit/closed), so
    /// the session should be removed immediately rather than lingering as done.
    pub fn is_session_end(kind: AgentKind, event_name: &str) -> bool {
        match kind {
            AgentKind::Claude => event_name == "SessionEnd",
            AgentKind::Gemini => event_name == "SessionEnd",
            AgentKind::Cursor => event_name == "sessionEnd",
            _ => false,
        }
    }

    pub fn state(kind: AgentKind, event_name: &str) -> Option<AgentState> {
        // Generic: any caller (e.g. the `agentpet run` wrapper) can send a
        // normalised state name directly.
        if let Some(direct) = AgentState::from_raw(event_name) {
            return Some(direct);
        }

        match kind {
            AgentKind::Claude => match event_name {
                "SessionStart" => Some(AgentState::Registered),
                "UserPromptSubmit" | "PreToolUse" | "PostToolUse" => Some(AgentState::Working),
                "Notification" => Some(AgentState::Waiting),
                "Stop" | "SubagentStop" => Some(AgentState::Done),
                _ => None,
            },
            AgentKind::Codex => match event_name {
                "SessionStart" => Some(AgentState::Registered),
                "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "SubagentStart" => {
                    Some(AgentState::Working)
                }
                "PermissionRequest" => Some(AgentState::Waiting),
                "Stop" | "SubagentStop" => Some(AgentState::Done),
                _ => None,
            },
            AgentKind::Gemini => match event_name {
                "SessionStart" => Some(AgentState::Registered),
                "BeforeAgent" | "BeforeModel" | "BeforeTool" | "AfterTool"
                | "BeforeToolSelection" | "AfterModel" => Some(AgentState::Working),
                "Notification" => Some(AgentState::Waiting),
                "AfterAgent" | "SessionEnd" => Some(AgentState::Done),
                _ => None,
            },
            AgentKind::Cursor => match event_name {
                "sessionStart" => Some(AgentState::Registered),
                "beforeSubmitPrompt" | "preToolUse" | "beforeShellExecution" => {
                    Some(AgentState::Working)
                }
                "stop" | "subagentStop" | "sessionEnd" => Some(AgentState::Done),
                _ => None,
            },
            AgentKind::Windsurf => match event_name {
                "pre_user_prompt" => Some(AgentState::Working),
                "post_cascade_response" | "post_cascade_response_with_transcript" => {
                    Some(AgentState::Done)
                }
                _ => None,
            },
            AgentKind::Opencode => match event_name {
                // The plugin sends normalised states directly (handled above);
                // these map the raw opencode event names as a fallback.
                "session.created" => Some(AgentState::Working),
                "session.idle" => Some(AgentState::Done),
                _ => None,
            },
            AgentKind::Cli | AgentKind::Unknown => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_event_mapping() {
        assert_eq!(StateMapper::state(AgentKind::Claude, "SessionStart"), Some(AgentState::Registered));
        assert_eq!(StateMapper::state(AgentKind::Claude, "UserPromptSubmit"), Some(AgentState::Working));
        assert_eq!(StateMapper::state(AgentKind::Claude, "PreToolUse"), Some(AgentState::Working));
        assert_eq!(StateMapper::state(AgentKind::Claude, "PostToolUse"), Some(AgentState::Working));
        assert_eq!(StateMapper::state(AgentKind::Claude, "Notification"), Some(AgentState::Waiting));
        assert_eq!(StateMapper::state(AgentKind::Claude, "Stop"), Some(AgentState::Done));
        assert_eq!(StateMapper::state(AgentKind::Claude, "SubagentStop"), Some(AgentState::Done));
    }

    #[test]
    fn unknown_event_is_ignored() {
        assert_eq!(StateMapper::state(AgentKind::Claude, "Bogus"), None);
        assert_eq!(StateMapper::state(AgentKind::Codex, "Bogus"), None);
        assert_eq!(StateMapper::state(AgentKind::Unknown, "Stop"), None);
    }

    #[test]
    fn direct_state_name_maps_for_any_kind() {
        assert_eq!(StateMapper::state(AgentKind::Cli, "working"), Some(AgentState::Working));
        assert_eq!(StateMapper::state(AgentKind::Cli, "done"), Some(AgentState::Done));
        assert_eq!(StateMapper::state(AgentKind::Unknown, "waiting"), Some(AgentState::Waiting));
    }

    #[test]
    fn codex_mapping() {
        assert_eq!(StateMapper::state(AgentKind::Codex, "SessionStart"), Some(AgentState::Registered));
        assert_eq!(StateMapper::state(AgentKind::Codex, "PreToolUse"), Some(AgentState::Working));
        assert_eq!(StateMapper::state(AgentKind::Codex, "PermissionRequest"), Some(AgentState::Waiting));
        assert_eq!(StateMapper::state(AgentKind::Codex, "Stop"), Some(AgentState::Done));
    }

    #[test]
    fn gemini_mapping() {
        assert_eq!(StateMapper::state(AgentKind::Gemini, "BeforeTool"), Some(AgentState::Working));
        assert_eq!(StateMapper::state(AgentKind::Gemini, "Notification"), Some(AgentState::Waiting));
        assert_eq!(StateMapper::state(AgentKind::Gemini, "AfterAgent"), Some(AgentState::Done));
    }

    #[test]
    fn cursor_and_windsurf_mapping() {
        assert_eq!(StateMapper::state(AgentKind::Cursor, "sessionStart"), Some(AgentState::Registered));
        assert_eq!(StateMapper::state(AgentKind::Cursor, "beforeSubmitPrompt"), Some(AgentState::Working));
        assert_eq!(StateMapper::state(AgentKind::Cursor, "stop"), Some(AgentState::Done));
        assert_eq!(StateMapper::state(AgentKind::Windsurf, "pre_user_prompt"), Some(AgentState::Working));
        assert_eq!(StateMapper::state(AgentKind::Windsurf, "post_cascade_response"), Some(AgentState::Done));
    }

    #[test]
    fn opencode_normalised_state_pass_through() {
        assert_eq!(StateMapper::state(AgentKind::Opencode, "done"), Some(AgentState::Done));
        assert_eq!(StateMapper::state(AgentKind::Opencode, "working"), Some(AgentState::Working));
        assert_eq!(StateMapper::state(AgentKind::Opencode, "session.idle"), Some(AgentState::Done));
    }

    #[test]
    fn is_session_end() {
        assert!(StateMapper::is_session_end(AgentKind::Claude, "SessionEnd"));
        assert!(StateMapper::is_session_end(AgentKind::Gemini, "SessionEnd"));
        assert!(StateMapper::is_session_end(AgentKind::Cursor, "sessionEnd"));
        assert!(!StateMapper::is_session_end(AgentKind::Claude, "Stop"));
        assert!(!StateMapper::is_session_end(AgentKind::Codex, "Stop"));
    }
}
