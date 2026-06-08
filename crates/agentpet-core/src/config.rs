//! Persistent app settings, replacing macOS `UserDefaults` with a single XDG
//! JSON file (`~/.config/agentpet/config.json`). Keys mirror the `agentpet.*`
//! defaults used across the Swift app; this grows as UI phases land.

use crate::state::{AgentKind, PetMood};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// `~/.config/agentpet/config.json`.
pub fn config_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("agentpet").join("config.json")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Show the running-agent count next to the tray icon.
    pub show_count: bool,
    /// Show the chat bubble below the tray icon.
    pub show_chat_menu_bar: bool,
    /// Default pet pack id (`pet.json` `id`), used for any agent without its own
    /// pick and as the first-run/bootstrap selection.
    pub selected_pet_id: Option<String>,
    /// Per-agent pet pack id, keyed by `AgentKind::raw()`. Lets each agent (e.g.
    /// Claude vs Codex) show a different pet; falls back to `selected_pet_id`.
    pub agent_pet_ids: HashMap<String, String>,
    /// Show the pet's chat bubble.
    pub show_chat: bool,
    /// Pet render size in points.
    pub pet_size: f64,
    /// First-run onboarding completed.
    pub has_onboarded: bool,
    /// Desktop notifications enabled.
    pub notifications_enabled: bool,
    /// `"system"` (built-in lines) or `"custom"` (user-provided).
    pub chat_source: String,
    /// Custom chat lines per mood (used when `chat_source == "custom"`).
    pub chat_custom: HashMap<String, Vec<String>>,
    /// Per-pack mood→clip-index bindings, keyed by pack id then mood name.
    pub bindings: HashMap<String, HashMap<String, usize>>,
    /// Sound on/off + optional custom file per event.
    pub sound_waiting_on: bool,
    pub sound_done_on: bool,
    pub sound_waiting_path: Option<String>,
    pub sound_done_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_count: true,
            show_chat_menu_bar: false,
            selected_pet_id: None,
            agent_pet_ids: HashMap::new(),
            show_chat: true,
            pet_size: 110.0,
            has_onboarded: false,
            notifications_enabled: true,
            chat_source: "system".to_string(),
            chat_custom: HashMap::new(),
            bindings: HashMap::new(),
            sound_waiting_on: true,
            sound_done_on: true,
            sound_waiting_path: None,
            sound_done_path: None,
        }
    }
}

impl Config {
    /// Loads from `config_path()`, returning defaults if absent/unreadable.
    pub fn load() -> Self {
        Self::load_from(&config_path())
    }

    pub fn load_from(path: &std::path::Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default()
    }

    /// Saves to `config_path()`, creating the directory as needed.
    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&config_path())
    }

    pub fn save_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let data = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, data)
    }

    /// Pet pack id to show for `kind`: its own pick if set, otherwise the global
    /// `selected_pet_id` default.
    pub fn pet_id_for(&self, kind: AgentKind) -> Option<&str> {
        self.agent_pet_ids
            .get(kind.raw())
            .or(self.selected_pet_id.as_ref())
            .map(String::as_str)
    }

    /// Assigns a pet pack to one agent and persists it.
    pub fn set_pet_for(&mut self, kind: AgentKind, pack_id: impl Into<String>) {
        self.agent_pet_ids.insert(kind.raw().to_string(), pack_id.into());
    }

    /// Clip index for a pet+mood, falling back to the default spread when the
    /// pack has no stored binding.
    pub fn clip_index(&self, pack_id: &str, clip_count: usize, mood: PetMood) -> usize {
        if let Some(stored) = self.bindings.get(pack_id).and_then(|m| m.get(mood.raw())) {
            return (*stored).min(clip_count.saturating_sub(1));
        }
        crate::sprite::PetBindings::defaults(clip_count).clip_index(mood)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut cfg = Config::default();
        cfg.selected_pet_id = Some("boba".into());
        cfg.pet_size = 160.0;
        cfg.save_to(&path).unwrap();

        let loaded = Config::load_from(&path);
        assert_eq!(loaded.selected_pet_id.as_deref(), Some("boba"));
        assert_eq!(loaded.pet_size, 160.0);
        assert!(loaded.show_count, "default preserved");
    }

    #[test]
    fn missing_file_yields_defaults() {
        let cfg = Config::load_from(std::path::Path::new("/nonexistent/agentpet/config.json"));
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn pet_id_for_falls_back_to_default_then_uses_agent_pick() {
        let mut cfg = Config::default();
        cfg.selected_pet_id = Some("boba".into());
        // No per-agent pick yet: every agent uses the default.
        assert_eq!(cfg.pet_id_for(AgentKind::Claude), Some("boba"));
        assert_eq!(cfg.pet_id_for(AgentKind::Codex), Some("boba"));
        // Assigning Codex its own pet leaves Claude on the default.
        cfg.set_pet_for(AgentKind::Codex, "cube");
        assert_eq!(cfg.pet_id_for(AgentKind::Codex), Some("cube"));
        assert_eq!(cfg.pet_id_for(AgentKind::Claude), Some("boba"));
    }

    #[test]
    fn agent_pet_ids_roundtrip_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut cfg = Config::default();
        cfg.set_pet_for(AgentKind::Claude, "boba");
        cfg.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path).pet_id_for(AgentKind::Claude), Some("boba"));
    }

    #[test]
    fn stored_binding_overrides_default_and_clamps() {
        let mut cfg = Config::default();
        cfg.bindings
            .entry("boba".into())
            .or_default()
            .insert(PetMood::Working.raw().into(), 9);
        assert_eq!(cfg.clip_index("boba", 3, PetMood::Working), 2, "clamped to clip_count-1");
        assert_eq!(cfg.clip_index("other", 5, PetMood::Working), 1, "default spread when unset");
    }
}
