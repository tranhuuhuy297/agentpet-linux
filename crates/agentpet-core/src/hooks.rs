//! Installs/removes AgentPet's hook entries in an agent's config. Ports
//! `HookInstaller.swift` and `AgentHooks.swift`.
//!
//! Claude Code and Codex share the nested `{"hooks": {...}}` shape. The
//! dictionary transforms are pure (and tested); the `*_to_disk` helpers wrap
//! them with file IO. Our entries are identified by their command string, so
//! install is idempotent and foreign hooks are never touched.

use crate::state::AgentKind;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// Where and which lifecycle events to register for an agent.
#[derive(Debug, Clone)]
pub struct AgentHookSpec {
    pub kind: AgentKind,
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
                events: vec![
                    "SessionStart", "UserPromptSubmit", "PreToolUse", "Notification", "Stop",
                    "SubagentStop", "SessionEnd",
                ],
                settings_path: home.join(".claude/settings.json"),
            },
            AgentKind::Codex => AgentHookSpec {
                kind,
                events: vec![
                    "SessionStart", "UserPromptSubmit", "PreToolUse", "PermissionRequest", "Stop",
                    "SubagentStop",
                ],
                settings_path: home.join(".codex/hooks.json"),
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

    // MARK: - Claude-nested shape (Claude / Codex)

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
    ///
    /// We edit a config the user owns and that other tools also write to, so
    /// before clobbering an existing file we snapshot it to `<name>.bak`. The
    /// suffix is appended to the full filename (`settings.json` ->
    /// `settings.json.bak`) rather than replacing the extension, so the backup
    /// is unambiguously the same file. The backup is best-effort: a failed copy
    /// must never block the write the user asked for.
    pub fn write_settings(settings: &Value, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        if path.exists() {
            let backup = backup_path(path);
            if let Err(e) = std::fs::copy(path, &backup) {
                eprintln!("agentpet: failed to back up {}: {e}", path.display());
            }
        }
        let data = serde_json::to_vec_pretty(settings).map_err(std::io::Error::other)?;
        std::fs::write(path, data)
    }

    pub fn install_to_disk(command: &str, path: &Path, events: &[&str]) -> std::io::Result<()> {
        let updated = Self::install(Self::read_settings(path), command, events);
        Self::write_settings(&updated, path)
    }

    pub fn uninstall_from_disk(path: &Path, events: &[&str]) -> std::io::Result<()> {
        let updated = Self::uninstall(Self::read_settings(path), events);
        Self::write_settings(&updated, path)
    }

    pub fn is_installed_on_disk(path: &Path, events: &[&str]) -> bool {
        Self::is_installed(&Self::read_settings(path), events)
    }

    /// Whether the *exact* desired command is already registered. Unlike
    /// `is_installed` (which matches any "agentpet"+"hook" entry, regardless of
    /// the embedded binary path), this checks the full command string. It is how
    /// we tell a stale hook — pointing at an old binary path — apart from a
    /// current one that needs no rewrite.
    pub fn is_installed_with_command(settings: &Value, events: &[&str], command: &str) -> bool {
        let Some(hooks) = settings.get("hooks").and_then(|v| v.as_object()) else {
            return false;
        };
        events.iter().all(|event| {
            hooks
                .get(*event)
                .and_then(|v| v.as_array())
                .map(|groups| groups.iter().any(|g| group_has_command(g, command)))
                .unwrap_or(false)
        })
    }

    pub fn is_installed_with_command_on_disk(
        path: &Path,
        events: &[&str],
        command: &str,
    ) -> bool {
        Self::is_installed_with_command(&Self::read_settings(path), events, command)
    }

    /// Self-heals a hook whose embedded binary path drifted from the running
    /// binary (moved install, AppImage remount, etc.).
    ///
    /// Two invariants make this safe to call unconditionally on every startup:
    /// - It only acts when our hook is *already* installed, so it never enables
    ///   an integration the user didn't turn on themselves.
    /// - It only rewrites when the exact desired command is absent, so a hook
    ///   that already matches is left untouched — avoiding needless `.bak`
    ///   churn from the backup-on-write behaviour.
    ///
    /// Returns `Ok(true)` when it rewrote the file (healed), `Ok(false)` when it
    /// left the file alone.
    pub fn resync_command_to_disk(
        command: &str,
        path: &Path,
        events: &[&str],
    ) -> std::io::Result<bool> {
        let settings = Self::read_settings(path);
        if !Self::is_installed(&settings, events) {
            return Ok(false);
        }
        if Self::is_installed_with_command(&settings, events, command) {
            return Ok(false);
        }
        Self::install_to_disk(command, path, events)?;
        Ok(true)
    }
}

/// Appends `.bak` to the full filename (keeping the original extension) so the
/// backup sits beside the original as an obvious sibling.
fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".bak");
    path.with_file_name(name)
}

fn group_has_command(group: &Value, command: &str) -> bool {
    group
        .get("hooks")
        .and_then(|v| v.as_array())
        .map(|inner| {
            inner.iter().any(|item| {
                item.get("command").and_then(|c| c.as_str()) == Some(command)
            })
        })
        .unwrap_or(false)
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

    const CMD: &str = "\"/opt/agentpet/agentpet\" hook --agent claude";

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

    #[test]
    fn disk_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        for (kind, file) in [(AgentKind::Claude, "claude.json"), (AgentKind::Codex, "codex.json")] {
            let spec = AgentHooks::spec(kind).unwrap();
            let ev: Vec<&str> = spec.events.iter().copied().collect();
            let path = tmp.path().join(file);
            let command = format!("\"/opt/agentpet/agentpet\" hook --agent {}", kind.raw());

            assert!(!HookInstaller::is_installed_on_disk(&path, &ev), "{kind:?} clean");
            HookInstaller::install_to_disk(&command, &path, &ev).unwrap();
            assert!(HookInstaller::is_installed_on_disk(&path, &ev), "{kind:?} installed");
            HookInstaller::uninstall_from_disk(&path, &ev).unwrap();
            assert!(!HookInstaller::is_installed_on_disk(&path, &ev), "{kind:?} removed");
        }
    }

    #[test]
    fn write_backs_up_existing_file_only() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");
        let bak = tmp.path().join("settings.json.bak");

        // Fresh path: no backup is created.
        HookInstaller::write_settings(&json!({"v": 1}), &path).unwrap();
        assert!(!bak.exists(), "no backup for a non-existent target");
        assert!(path.exists());

        // Overwriting: the OLD content lands in the sibling `.bak`.
        let old = std::fs::read_to_string(&path).unwrap();
        HookInstaller::write_settings(&json!({"v": 2}), &path).unwrap();
        assert!(bak.exists(), "backup created before clobber");
        assert_eq!(std::fs::read_to_string(&bak).unwrap(), old, "backup holds old content");
        assert_ne!(std::fs::read_to_string(&path).unwrap(), old, "target was rewritten");
    }

    #[test]
    fn resync_heals_stale_path_noop_when_current_or_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let ev: Vec<&str> = AgentHooks::spec(AgentKind::Claude)
            .unwrap()
            .events
            .iter()
            .copied()
            .collect();

        let old_cmd = "\"/old/path/agentpet\" hook --agent claude";
        let new_cmd = "\"/new/path/agentpet\" hook --agent claude";

        // (c) Not installed → resync is a no-op and writes nothing.
        let absent = tmp.path().join("absent.json");
        assert!(!HookInstaller::resync_command_to_disk(new_cmd, &absent, &ev).unwrap());
        assert!(!absent.exists(), "resync must not create hooks the user never enabled");

        // (a) Installed with an OLD path → resync rewrites to the new command.
        let path = tmp.path().join("settings.json");
        HookInstaller::install_to_disk(old_cmd, &path, &ev).unwrap();
        assert!(!HookInstaller::is_installed_with_command_on_disk(&path, &ev, new_cmd));
        assert!(HookInstaller::resync_command_to_disk(new_cmd, &path, &ev).unwrap(), "healed");
        assert!(HookInstaller::is_installed_with_command_on_disk(&path, &ev, new_cmd));
        assert!(!HookInstaller::is_installed_with_command_on_disk(&path, &ev, old_cmd));

        // (b) Installed with the CORRECT command → resync is a no-op, file unchanged.
        let before = std::fs::read_to_string(&path).unwrap();
        assert!(!HookInstaller::resync_command_to_disk(new_cmd, &path, &ev).unwrap(), "no-op");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before, "file untouched when current");
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
