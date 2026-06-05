//! CLI argument parsing and agent hook-stdin payload decoding. Ports
//! `HookArguments.swift`, `RunArguments.swift`, `ClaudeHookPayload.swift`, and
//! `HookPayloads.swift`.

use crate::event::AgentEvent;
use crate::state::{AgentKind, UnixTime};
use serde::Deserialize;

// MARK: - `agentpet hook` flags

/// Parsed `agentpet hook` flags. Unknown flags are ignored.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HookArguments {
    pub event: Option<String>,
    pub session: Option<String>,
    pub project: Option<String>,
    pub agent: Option<String>,
    pub message: Option<String>,
}

impl HookArguments {
    /// Parses `--key value` pairs from the argument list.
    pub fn parse(args: &[String]) -> Self {
        let mut result = HookArguments::default();
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            let value = args.get(i + 1).cloned();
            match flag {
                "--event" => result.event = value,
                "--session" => result.session = value,
                "--project" => result.project = value,
                "--agent" => result.agent = value,
                "--message" => result.message = value,
                _ => {
                    i += 1;
                    continue;
                }
            }
            i += 2;
        }
        result
    }

    /// Builds an `AgentEvent`, or `None` if required flags are missing.
    /// Defaults to `Claude` when `--agent` is absent.
    pub fn make_event(&self, now: UnixTime) -> Option<AgentEvent> {
        let event = self.event.clone()?;
        let session = self.session.clone()?;
        let kind = self
            .agent
            .as_deref()
            .and_then(AgentKind::from_raw)
            .unwrap_or(AgentKind::Claude);
        Some(AgentEvent::new(
            session,
            kind,
            event,
            self.project.clone(),
            self.message.clone(),
            now,
        ))
    }
}

// MARK: - `agentpet run [flags] -- <command...>`

/// Parsed `agentpet run` arguments. Flags appear before `--`; everything after
/// `--` is the command to run.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RunArguments {
    pub session: Option<String>,
    pub project: Option<String>,
    pub agent: Option<String>,
    pub command: Vec<String>,
}

impl RunArguments {
    pub fn parse(args: &[String]) -> Self {
        let mut result = RunArguments::default();
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            if flag == "--" {
                result.command = args[(i + 1)..].to_vec();
                break;
            }
            let value = args.get(i + 1).cloned();
            match flag {
                "--session" => {
                    result.session = value;
                    i += 2;
                }
                "--project" => {
                    result.project = value;
                    i += 2;
                }
                "--agent" => {
                    result.agent = value;
                    i += 2;
                }
                _ => i += 1,
            }
        }
        result
    }
}

// MARK: - Claude Code hook stdin

/// The JSON Claude Code writes to a hook's stdin. Only the needed fields are
/// decoded; the rest are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ClaudeHookPayload {
    #[serde(default, rename = "session_id")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default, rename = "hook_event_name")]
    pub hook_event_name: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default, rename = "tool_name")]
    pub tool_name: Option<String>,
}

impl ClaudeHookPayload {
    pub fn decode(data: &[u8]) -> Option<Self> {
        serde_json::from_slice(data).ok()
    }

    /// Builds an event, or `None` if session id / event name are missing.
    pub fn make_event(&self, now: UnixTime, kind: AgentKind) -> Option<AgentEvent> {
        let session_id = self.session_id.clone()?;
        let hook_event_name = self.hook_event_name.clone()?;
        // Surface the running tool name when there's no explicit message.
        let context = self
            .message
            .clone()
            .or_else(|| self.tool_name.as_ref().map(|t| format!("Using {t}")));
        Some(AgentEvent::new(
            session_id,
            kind,
            hook_event_name,
            self.cwd.clone(),
            context,
            now,
        ))
    }
}

// MARK: - Cursor hook stdin

/// The JSON Cursor writes to a hook's stdin (only the fields AgentPet needs).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CursorHookPayload {
    #[serde(default, rename = "conversation_id")]
    pub conversation_id: Option<String>,
    #[serde(default, rename = "hook_event_name")]
    pub hook_event_name: Option<String>,
    #[serde(default, rename = "workspace_roots")]
    pub workspace_roots: Option<Vec<String>>,
}

impl CursorHookPayload {
    pub fn decode(data: &[u8]) -> Option<Self> {
        serde_json::from_slice(data).ok()
    }

    pub fn make_event(&self, now: UnixTime) -> Option<AgentEvent> {
        let session_id = self.conversation_id.clone()?;
        let event_name = self.hook_event_name.clone()?;
        let project = self
            .workspace_roots
            .as_ref()
            .and_then(|roots| roots.first().cloned());
        Some(AgentEvent::new(session_id, AgentKind::Cursor, event_name, project, None, now))
    }
}

// MARK: - Windsurf (Cascade) hook stdin

/// The JSON Windsurf writes to a hook's stdin.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WindsurfHookPayload {
    #[serde(default, rename = "trajectory_id")]
    pub trajectory_id: Option<String>,
    #[serde(default, rename = "agent_action_name")]
    pub agent_action_name: Option<String>,
}

impl WindsurfHookPayload {
    pub fn decode(data: &[u8]) -> Option<Self> {
        serde_json::from_slice(data).ok()
    }

    pub fn make_event(&self, now: UnixTime) -> Option<AgentEvent> {
        let session_id = self.trajectory_id.clone()?;
        let event_name = self.agent_action_name.clone()?;
        Some(AgentEvent::new(session_id, AgentKind::Windsurf, event_name, None, None, now))
    }
}

/// Decodes a hook's stdin payload into an event, choosing the field convention
/// by agent kind. opencode sends explicit flags instead of stdin.
pub struct HookPayload;

impl HookPayload {
    pub fn event(kind: AgentKind, stdin: &[u8], now: UnixTime) -> Option<AgentEvent> {
        match kind {
            AgentKind::Cursor => CursorHookPayload::decode(stdin)?.make_event(now),
            AgentKind::Windsurf => WindsurfHookPayload::decode(stdin)?.make_event(now),
            _ => ClaudeHookPayload::decode(stdin)?.make_event(now, kind),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn hook_args_parse_and_default_to_claude() {
        let args = argv(&["--event", "Stop", "--session", "s1", "--project", "/p"]);
        let parsed = HookArguments::parse(&args);
        let ev = parsed.make_event(7.0).unwrap();
        assert_eq!(ev.session_id, "s1");
        assert_eq!(ev.event_name, "Stop");
        assert_eq!(ev.project.as_deref(), Some("/p"));
        assert_eq!(ev.agent_kind, AgentKind::Claude);
    }

    #[test]
    fn hook_args_missing_required_yield_none() {
        assert!(HookArguments::parse(&argv(&["--event", "Stop"])).make_event(0.0).is_none());
        assert!(HookArguments::parse(&argv(&["--session", "s1"])).make_event(0.0).is_none());
    }

    #[test]
    fn hook_args_select_agent() {
        let parsed = HookArguments::parse(&argv(&[
            "--agent", "opencode", "--event", "working", "--session", "x",
        ]));
        assert_eq!(parsed.make_event(0.0).unwrap().agent_kind, AgentKind::Opencode);
    }

    #[test]
    fn run_args_split_on_double_dash() {
        let args = argv(&["--session", "id", "--project", "/p", "--", "aider", "--model", "x"]);
        let parsed = RunArguments::parse(&args);
        assert_eq!(parsed.session.as_deref(), Some("id"));
        assert_eq!(parsed.project.as_deref(), Some("/p"));
        assert_eq!(parsed.command, vec!["aider", "--model", "x"]);
    }

    #[test]
    fn claude_payload_uses_tool_name_when_no_message() {
        let json = br#"{"session_id":"s1","cwd":"/p","hook_event_name":"PreToolUse","tool_name":"Bash"}"#;
        let ev = HookPayload::event(AgentKind::Claude, json, 0.0).unwrap();
        assert_eq!(ev.session_id, "s1");
        assert_eq!(ev.project.as_deref(), Some("/p"));
        assert_eq!(ev.message.as_deref(), Some("Using Bash"));
    }

    #[test]
    fn cursor_payload_decode() {
        let json = br#"{"conversation_id":"c1","hook_event_name":"stop","workspace_roots":["/proj"],"model":"x"}"#;
        let ev = HookPayload::event(AgentKind::Cursor, json, 0.0).unwrap();
        assert_eq!(ev.session_id, "c1");
        assert_eq!(ev.event_name, "stop");
        assert_eq!(ev.project.as_deref(), Some("/proj"));
        assert_eq!(ev.agent_kind, AgentKind::Cursor);
    }

    #[test]
    fn windsurf_payload_decode() {
        let json = br#"{"trajectory_id":"t1","agent_action_name":"post_cascade_response","model_name":"x"}"#;
        let ev = HookPayload::event(AgentKind::Windsurf, json, 0.0).unwrap();
        assert_eq!(ev.session_id, "t1");
        assert_eq!(ev.event_name, "post_cascade_response");
        assert_eq!(ev.agent_kind, AgentKind::Windsurf);
    }
}
