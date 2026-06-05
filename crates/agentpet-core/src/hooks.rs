//! Installs/removes AgentPet's hook entries in an agent's config. Ports
//! `HookInstaller.swift` and `AgentHooks.swift`.
//!
//! Claude Code, Codex, and Gemini share the nested `{"hooks": {...}}` shape;
//! Cursor and Windsurf use flatter shapes; opencode uses a generated JS plugin
//! file. The dictionary transforms are pure (and tested); the `*_to_disk`
//! helpers wrap them with file IO. Our entries are identified by their command
//! string, so install is idempotent and foreign hooks are never touched.

use crate::state::AgentKind;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// How an agent's hook configuration is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookStyle {
    /// Claude / Codex / Gemini: `{"hooks": {Event: [{"hooks": [{"type": "command", "command": ...}]}]}}`.
    ClaudeNested,
    /// Cursor `~/.cursor/hooks.json`: `{"version": 1, "hooks": {event: [{"command": ..., "type": "command"}]}}`.
    CursorFlat,
    /// Windsurf `~/.codeium/windsurf/hooks.json`: `{"hooks": {event: [{"command": ..., "show_output": false}]}}`.
    WindsurfFlat,
    /// opencode: a JS plugin file dropped in `~/.config/opencode/plugin/`.
    OpencodePlugin,
}

/// Where and which lifecycle events to register for an agent.
#[derive(Debug, Clone)]
pub struct AgentHookSpec {
    pub kind: AgentKind,
    pub style: HookStyle,
    pub events: Vec<&'static str>,
    pub settings_path: PathBuf,
}

pub struct AgentHooks;

impl AgentHooks {
    pub fn spec(kind: AgentKind) -> Option<AgentHookSpec> {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let spec = match kind {
            AgentKind::Claude => AgentHookSpec {
                kind,
                style: HookStyle::ClaudeNested,
                events: vec![
                    "SessionStart", "UserPromptSubmit", "PreToolUse", "Notification", "Stop",
                    "SubagentStop", "SessionEnd",
                ],
                settings_path: home.join(".claude/settings.json"),
            },
            AgentKind::Codex => AgentHookSpec {
                kind,
                style: HookStyle::ClaudeNested,
                events: vec![
                    "SessionStart", "UserPromptSubmit", "PreToolUse", "PermissionRequest", "Stop",
                    "SubagentStop",
                ],
                settings_path: home.join(".codex/hooks.json"),
            },
            AgentKind::Gemini => AgentHookSpec {
                kind,
                style: HookStyle::ClaudeNested,
                events: vec![
                    "SessionStart", "BeforeAgent", "BeforeTool", "AfterTool", "Notification",
                    "AfterAgent", "SessionEnd",
                ],
                settings_path: home.join(".gemini/settings.json"),
            },
            AgentKind::Cursor => AgentHookSpec {
                kind,
                style: HookStyle::CursorFlat,
                events: vec![
                    "sessionStart", "beforeSubmitPrompt", "preToolUse", "stop", "subagentStop",
                    "sessionEnd",
                ],
                settings_path: home.join(".cursor/hooks.json"),
            },
            AgentKind::Windsurf => AgentHookSpec {
                kind,
                style: HookStyle::WindsurfFlat,
                events: vec!["pre_user_prompt", "post_cascade_response"],
                settings_path: home.join(".codeium/windsurf/hooks.json"),
            },
            AgentKind::Opencode => AgentHookSpec {
                kind,
                style: HookStyle::OpencodePlugin,
                // The JS plugin hardcodes its own session.created/session.idle
                // hooks, so no event list is registered through the installer.
                events: vec![],
                settings_path: home.join(".config/opencode/plugin/agentpet.js"),
            },
            AgentKind::Cli | AgentKind::Unknown => return None,
        };
        Some(spec)
    }
}

pub struct HookInstaller;

impl HookInstaller {
    /// Default events for the Claude-nested shape when none is supplied.
    pub const DEFAULT_EVENTS: &'static [&'static str] = &[
        "SessionStart", "UserPromptSubmit", "PreToolUse", "Notification", "Stop", "SubagentStop",
    ];

    pub fn default_settings_path() -> PathBuf {
        PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".claude/settings.json")
    }

    /// Our entries are identified by their command string.
    pub fn is_ours(command: &str) -> bool {
        command.contains("agentpet") && command.contains("hook")
    }

    // MARK: - Claude-nested shape (Claude / Codex / Gemini)

    pub fn is_installed(settings: &Value, events: &[&str]) -> bool {
        let Some(hooks) = settings.get("hooks").and_then(|v| v.as_object()) else {
            return false;
        };
        events.iter().any(|event| {
            hooks
                .get(*event)
                .and_then(|v| v.as_array())
                .map(|groups| groups.iter().any(Self::group_is_ours))
                .unwrap_or(false)
        })
    }

    pub fn install(settings: Value, command: &str, events: &[&str]) -> Value {
        let mut s = into_object(settings);
        let mut hooks = take_object(&mut s, "hooks");
        for &event in events {
            let mut groups: Vec<Value> = take_array(&mut hooks, event)
                .into_iter()
                .filter(|g| !Self::group_is_ours(g))
                .collect();
            groups.push(json!({"hooks": [{"type": "command", "command": command}]}));
            hooks.insert(event.to_string(), Value::Array(groups));
        }
        s.insert("hooks".to_string(), Value::Object(hooks));
        Value::Object(s)
    }

    pub fn uninstall(settings: Value, events: &[&str]) -> Value {
        let mut s = into_object(settings);
        let mut hooks = match s.remove("hooks") {
            Some(Value::Object(m)) => m,
            _ => return Value::Object(s),
        };
        for &event in events {
            if let Some(Value::Array(groups)) = hooks.remove(event) {
                let kept: Vec<Value> =
                    groups.into_iter().filter(|g| !Self::group_is_ours(g)).collect();
                if !kept.is_empty() {
                    hooks.insert(event.to_string(), Value::Array(kept));
                }
            }
        }
        if !hooks.is_empty() {
            s.insert("hooks".to_string(), Value::Object(hooks));
        }
        Value::Object(s)
    }

    fn group_is_ours(group: &Value) -> bool {
        group
            .get("hooks")
            .and_then(|v| v.as_array())
            .map(|inner| {
                inner.iter().any(|item| {
                    item.get("command")
                        .and_then(|c| c.as_str())
                        .map(Self::is_ours)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    }

    // MARK: - Flat shape (Cursor / Windsurf)

    fn flat_item_is_ours(item: &Value) -> bool {
        item.get("command")
            .and_then(|c| c.as_str())
            .map(Self::is_ours)
            .unwrap_or(false)
    }

    pub fn install_flat(settings: Value, command: &str, events: &[&str], style: HookStyle) -> Value {
        let mut s = into_object(settings);
        if style == HookStyle::CursorFlat {
            s.entry("version".to_string()).or_insert(json!(1));
        }
        let mut hooks = take_object(&mut s, "hooks");
        for &event in events {
            let mut items: Vec<Value> = take_array(&mut hooks, event)
                .into_iter()
                .filter(|it| !Self::flat_item_is_ours(it))
                .collect();
            let mut entry = Map::new();
            entry.insert("command".to_string(), json!(command));
            match style {
                HookStyle::CursorFlat => {
                    entry.insert("type".to_string(), json!("command"));
                }
                HookStyle::WindsurfFlat => {
                    entry.insert("show_output".to_string(), json!(false));
                }
                _ => {}
            }
            items.push(Value::Object(entry));
            hooks.insert(event.to_string(), Value::Array(items));
        }
        s.insert("hooks".to_string(), Value::Object(hooks));
        Value::Object(s)
    }

    pub fn uninstall_flat(settings: Value, events: &[&str]) -> Value {
        let mut s = into_object(settings);
        let mut hooks = match s.remove("hooks") {
            Some(Value::Object(m)) => m,
            _ => return Value::Object(s),
        };
        for &event in events {
            if let Some(Value::Array(items)) = hooks.remove(event) {
                let kept: Vec<Value> =
                    items.into_iter().filter(|it| !Self::flat_item_is_ours(it)).collect();
                if !kept.is_empty() {
                    hooks.insert(event.to_string(), Value::Array(kept));
                }
            }
        }
        if !hooks.is_empty() {
            s.insert("hooks".to_string(), Value::Object(hooks));
        }
        Value::Object(s)
    }

    pub fn is_installed_flat(settings: &Value, events: &[&str]) -> bool {
        let Some(hooks) = settings.get("hooks").and_then(|v| v.as_object()) else {
            return false;
        };
        events.iter().any(|event| {
            hooks
                .get(*event)
                .and_then(|v| v.as_array())
                .map(|items| items.iter().any(Self::flat_item_is_ours))
                .unwrap_or(false)
        })
    }

    // MARK: - opencode JS plugin

    /// Extracts the agentpet binary path from a hook command like
    /// `"/path/to/agentpet" hook --agent opencode` (the first quoted token).
    pub fn binary_path(command: &str) -> String {
        if let Some(first) = command.find('"') {
            let rest = &command[first + 1..];
            if let Some(second) = rest.find('"') {
                return rest[..second].to_string();
            }
        }
        command.split(' ').next().unwrap_or(command).to_string()
    }

    pub fn opencode_plugin(binary: &str) -> String {
        let bin = js_string(binary);
        format!(
            r#"// AgentPet integration (auto-generated, safe to delete to uninstall).
// Reports opencode session lifecycle to AgentPet's menu bar app.
const AGENTPET_BIN = {bin}
export const AgentPet = async ({{ directory }}) => {{
  const sid = "opencode:" + (directory || "default")
  const send = (state) => {{
    try {{
      Bun.spawn([AGENTPET_BIN, "hook", "--agent", "opencode",
                 "--event", state, "--session", sid, "--project", directory || ""])
    }} catch (e) {{}}
  }}
  return {{
    "session.created": async () => {{ send("working") }},
    "session.idle": async () => {{ send("done") }},
  }}
}}
"#
        )
    }

    // MARK: - Disk IO

    pub fn read_settings(path: &Path) -> Value {
        std::fs::read(path)
            .ok()
            .and_then(|data| serde_json::from_slice::<Value>(&data).ok())
            .filter(|v| v.is_object())
            .unwrap_or_else(|| Value::Object(Map::new()))
    }

    /// Writes pretty-printed, sorted-key JSON (serde_json's default Map is a
    /// BTreeMap, so keys are emitted sorted — matching the macOS `.sortedKeys`).
    pub fn write_settings(settings: &Value, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let data = serde_json::to_vec_pretty(settings).map_err(std::io::Error::other)?;
        std::fs::write(path, data)
    }

    pub fn install_to_disk(
        command: &str,
        path: &Path,
        events: &[&str],
        style: HookStyle,
    ) -> std::io::Result<()> {
        match style {
            HookStyle::ClaudeNested => {
                let updated = Self::install(Self::read_settings(path), command, events);
                Self::write_settings(&updated, path)
            }
            HookStyle::CursorFlat | HookStyle::WindsurfFlat => {
                let updated = Self::install_flat(Self::read_settings(path), command, events, style);
                Self::write_settings(&updated, path)
            }
            HookStyle::OpencodePlugin => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                let js = Self::opencode_plugin(&Self::binary_path(command));
                std::fs::write(path, js)
            }
        }
    }

    pub fn uninstall_from_disk(path: &Path, events: &[&str], style: HookStyle) -> std::io::Result<()> {
        match style {
            HookStyle::ClaudeNested => {
                let updated = Self::uninstall(Self::read_settings(path), events);
                Self::write_settings(&updated, path)
            }
            HookStyle::CursorFlat | HookStyle::WindsurfFlat => {
                let updated = Self::uninstall_flat(Self::read_settings(path), events);
                Self::write_settings(&updated, path)
            }
            HookStyle::OpencodePlugin => {
                if Self::is_installed_on_disk(path, events, style) {
                    let _ = std::fs::remove_file(path);
                }
                Ok(())
            }
        }
    }

    pub fn is_installed_on_disk(path: &Path, events: &[&str], style: HookStyle) -> bool {
        match style {
            HookStyle::ClaudeNested => Self::is_installed(&Self::read_settings(path), events),
            HookStyle::CursorFlat | HookStyle::WindsurfFlat => {
                Self::is_installed_flat(&Self::read_settings(path), events)
            }
            HookStyle::OpencodePlugin => std::fs::read_to_string(path)
                .map(|s| Self::is_ours(&s))
                .unwrap_or(false),
        }
    }
}

/// JSON-encodes a string for safe embedding in JS source.
fn js_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
}

fn into_object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

fn take_object(map: &mut Map<String, Value>, key: &str) -> Map<String, Value> {
    match map.remove(key) {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

fn take_array(map: &mut Map<String, Value>, key: &str) -> Vec<Value> {
    match map.remove(key) {
        Some(Value::Array(a)) => a,
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: &str = "\"/opt/agentpet/agentpet\" hook --agent cursor";

    // MARK: - Claude-nested shape

    #[test]
    fn claude_install_idempotent_and_foreign_preserved() {
        let events = AgentHooks::spec(AgentKind::Claude).unwrap().events;
        let ev: Vec<&str> = events.iter().copied().collect();
        let existing = json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "echo hi"}]}]}});
        let once = HookInstaller::install(existing, CMD, &ev);
        let twice = HookInstaller::install(once, CMD, &ev);
        let stop = twice["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2, "foreign + ours, no duplicate");
        assert!(HookInstaller::is_installed(&twice, &ev));

        let removed = HookInstaller::uninstall(twice, &ev);
        let stop_after = removed["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop_after.len(), 1, "foreign kept");
        assert!(!HookInstaller::is_installed(&removed, &ev));
    }

    // MARK: - Cursor flat shape

    #[test]
    fn cursor_install_shape() {
        let events = AgentHooks::spec(AgentKind::Cursor).unwrap().events;
        let ev: Vec<&str> = events.iter().copied().collect();
        let result = HookInstaller::install_flat(json!({}), CMD, &ev, HookStyle::CursorFlat);
        assert_eq!(result["version"].as_i64(), Some(1));
        assert!(HookInstaller::is_installed_flat(&result, &ev));
        let stop = result["hooks"]["stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1);
        assert_eq!(stop[0]["type"].as_str(), Some("command"));
        assert!(stop[0]["command"].as_str().unwrap().contains("agentpet"));
    }

    #[test]
    fn cursor_idempotent_and_foreign_preserved() {
        let events = AgentHooks::spec(AgentKind::Cursor).unwrap().events;
        let ev: Vec<&str> = events.iter().copied().collect();
        let existing = json!({"hooks": {"stop": [{"command": "echo hi"}]}});
        let once = HookInstaller::install_flat(existing, CMD, &ev, HookStyle::CursorFlat);
        let twice = HookInstaller::install_flat(once, CMD, &ev, HookStyle::CursorFlat);
        let stop = twice["hooks"]["stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2, "foreign + ours, no duplicate");
        let removed = HookInstaller::uninstall_flat(twice, &ev);
        let stop_after = removed["hooks"]["stop"].as_array().unwrap();
        assert_eq!(stop_after.len(), 1, "foreign kept");
        assert!(!HookInstaller::is_installed_flat(&removed, &ev));
    }

    // MARK: - Windsurf flat shape

    #[test]
    fn windsurf_install_shape() {
        let events = AgentHooks::spec(AgentKind::Windsurf).unwrap().events;
        let ev: Vec<&str> = events.iter().copied().collect();
        let cmd = "\"/x/agentpet\" hook --agent windsurf";
        let result = HookInstaller::install_flat(json!({}), cmd, &ev, HookStyle::WindsurfFlat);
        assert!(result.get("version").is_none(), "Windsurf has no version field");
        let resp = result["hooks"]["post_cascade_response"].as_array().unwrap();
        assert_eq!(resp[0]["command"].as_str(), Some(cmd));
        assert_eq!(resp[0]["show_output"].as_bool(), Some(false));
        assert!(HookInstaller::is_installed_flat(&result, &ev));
    }

    // MARK: - opencode plugin

    #[test]
    fn opencode_binary_path_extraction() {
        assert_eq!(
            HookInstaller::binary_path("\"/opt/agentpet/agentpet\" hook --agent opencode"),
            "/opt/agentpet/agentpet"
        );
        assert_eq!(HookInstaller::binary_path("/usr/bin/agentpet hook"), "/usr/bin/agentpet");
    }

    #[test]
    fn opencode_plugin_content() {
        let js = HookInstaller::opencode_plugin("/x/agentpet");
        assert!(js.contains("session.idle"));
        assert!(js.contains("session.created"));
        assert!(js.contains("--agent"));
        assert!(js.contains("opencode"));
        assert!(HookInstaller::is_ours(&js.replace('\n', " ")));
    }

    // MARK: - Disk round-trip for each style

    #[test]
    fn disk_round_trip_all_styles() {
        let tmp = tempfile::tempdir().unwrap();
        let cases = [
            (AgentKind::Claude, "claude.json"),
            (AgentKind::Cursor, "cursor.json"),
            (AgentKind::Windsurf, "windsurf.json"),
            (AgentKind::Opencode, "plugin/agentpet.js"),
        ];
        for (kind, file) in cases {
            let spec = AgentHooks::spec(kind).unwrap();
            let ev: Vec<&str> = spec.events.iter().copied().collect();
            let path = tmp.path().join(file);
            let command = format!("\"/opt/agentpet/agentpet\" hook --agent {}", kind.raw());

            assert!(!HookInstaller::is_installed_on_disk(&path, &ev, spec.style), "{kind:?} clean");
            HookInstaller::install_to_disk(&command, &path, &ev, spec.style).unwrap();
            assert!(HookInstaller::is_installed_on_disk(&path, &ev, spec.style), "{kind:?} installed");
            HookInstaller::uninstall_from_disk(&path, &ev, spec.style).unwrap();
            assert!(!HookInstaller::is_installed_on_disk(&path, &ev, spec.style), "{kind:?} removed");
        }
    }

    #[test]
    fn written_settings_have_sorted_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("s.json");
        let v = json!({"zebra": 1, "alpha": 2, "mango": 3});
        HookInstaller::write_settings(&v, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let a = text.find("alpha").unwrap();
        let m = text.find("mango").unwrap();
        let z = text.find("zebra").unwrap();
        assert!(a < m && m < z, "keys serialized in sorted order");
    }
}
