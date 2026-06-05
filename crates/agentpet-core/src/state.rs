//! Normalised state/kind enums shared across the app. Ports `AgentState.swift`
//! and `PetMood.swift`.

use serde::{Deserialize, Serialize};

/// Wall-clock time as seconds since the Unix epoch. Passed in by callers
/// (never read from the clock inside the pure logic) so pruning is testable.
pub type UnixTime = f64;

/// Normalised lifecycle state of an agent session, independent of which agent
/// (Claude Code, Codex, ...) produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentState {
    /// Session announced itself but has not started working yet.
    Registered,
    /// Actively running (prompt submitted, tools executing).
    Working,
    /// Blocked on the user (needs input or a permission decision).
    Waiting,
    /// Finished a turn.
    Done,
    /// Done and quiet for a while; ambient/no attention needed.
    Idle,
}

impl AgentState {
    /// Parses a normalised state name (used by the `run` wrapper / opencode
    /// plugin, which send state names directly).
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "registered" => Some(Self::Registered),
            "working" => Some(Self::Working),
            "waiting" => Some(Self::Waiting),
            "done" => Some(Self::Done),
            "idle" => Some(Self::Idle),
            _ => None,
        }
    }

    /// Higher means more deserving of the user's attention (drives `sorted`).
    pub fn attention_priority(self) -> i32 {
        match self {
            Self::Working => 4,
            Self::Waiting => 3,
            Self::Done => 2,
            Self::Registered => 1,
            Self::Idle => 0,
        }
    }
}

/// Which agent a session belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    Claude,
    Codex,
    Gemini,
    Cursor,
    Opencode,
    Windsurf,
    /// Any CLI agent launched via the `agentpet run` wrapper.
    Cli,
    Unknown,
}

impl AgentKind {
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "gemini" => Some(Self::Gemini),
            "cursor" => Some(Self::Cursor),
            "opencode" => Some(Self::Opencode),
            "windsurf" => Some(Self::Windsurf),
            "cli" => Some(Self::Cli),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    pub fn raw(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Cursor => "cursor",
            Self::Opencode => "opencode",
            Self::Windsurf => "windsurf",
            Self::Cli => "cli",
            Self::Unknown => "unknown",
        }
    }
}

/// How a session's state was learned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentSource {
    /// Reported precisely by the agent through a hook.
    Hook,
    /// Inferred by passively observing processes (running / not running only).
    Passive,
}

/// The pet's mood, derived from the aggregate of all agent sessions. Also the
/// set of animation states a pet pack must provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PetMood {
    Idle,
    Working,
    Waiting,
    Done,
    Celebrate,
}

impl PetMood {
    /// Binding/animation order: clips are spread across moods in this sequence.
    pub const ALL: [PetMood; 5] = [
        PetMood::Idle,
        PetMood::Working,
        PetMood::Waiting,
        PetMood::Done,
        PetMood::Celebrate,
    ];

    pub fn raw(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Waiting => "waiting",
            Self::Done => "done",
            Self::Celebrate => "celebrate",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attention_priority_orders_working_highest() {
        assert!(AgentState::Working.attention_priority() > AgentState::Waiting.attention_priority());
        assert!(AgentState::Waiting.attention_priority() > AgentState::Done.attention_priority());
        assert!(AgentState::Done.attention_priority() > AgentState::Registered.attention_priority());
        assert!(AgentState::Registered.attention_priority() > AgentState::Idle.attention_priority());
    }

    #[test]
    fn kind_roundtrips_through_raw() {
        for k in [
            AgentKind::Claude,
            AgentKind::Codex,
            AgentKind::Gemini,
            AgentKind::Cursor,
            AgentKind::Opencode,
            AgentKind::Windsurf,
            AgentKind::Cli,
            AgentKind::Unknown,
        ] {
            assert_eq!(AgentKind::from_raw(k.raw()), Some(k));
        }
    }

    #[test]
    fn kind_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&AgentKind::Opencode).unwrap(),
            "\"opencode\""
        );
    }
}
