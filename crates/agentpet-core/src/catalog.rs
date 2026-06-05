//! The list of integrable agents shown in Settings/onboarding. Ports
//! `AgentCatalog.swift`.

use crate::state::AgentKind;

/// A coding agent AgentPet can integrate with, and whether that integration is
/// available yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIntegration {
    pub kind: AgentKind,
    pub display_name: &'static str,
    pub is_supported: bool,
    pub note: Option<&'static str>,
}

pub struct AgentCatalog;

impl AgentCatalog {
    pub fn all() -> Vec<AgentIntegration> {
        vec![
            AgentIntegration { kind: AgentKind::Claude, display_name: "Claude Code", is_supported: true, note: None },
            AgentIntegration { kind: AgentKind::Codex, display_name: "Codex", is_supported: true, note: None },
            AgentIntegration { kind: AgentKind::Gemini, display_name: "Gemini CLI", is_supported: true, note: None },
            AgentIntegration { kind: AgentKind::Cursor, display_name: "Cursor", is_supported: true, note: None },
            AgentIntegration { kind: AgentKind::Opencode, display_name: "opencode", is_supported: true, note: None },
            AgentIntegration {
                kind: AgentKind::Windsurf,
                display_name: "Windsurf",
                is_supported: true,
                note: Some("No \"needs input\" alerts (Windsurf has no such hook)"),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::AgentHooks;

    #[test]
    fn every_catalog_agent_has_a_hook_spec() {
        for integration in AgentCatalog::all() {
            assert!(
                AgentHooks::spec(integration.kind).is_some(),
                "{:?} should have a hook spec",
                integration.kind
            );
        }
    }
}
