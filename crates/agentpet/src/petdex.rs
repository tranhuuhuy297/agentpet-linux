//! Petdex online pet library client. Ports `PetBrowser.swift` + `PetInstaller`.
//!
//! The gallery and the first-run starter pet both come from Petdex's public
//! manifest; AgentPet hosts no art of its own. Pure parsing is split out and
//! unit-tested; the async fetch/download wrap it with network IO.

use agentpet_core::ipc;
use serde::Deserialize;
use std::path::PathBuf;

/// Petdex's public manifest endpoint.
pub const MANIFEST_URL: &str = "https://petdex.crafter.run/api/manifest";
/// Preferred first-run starter pet (a non-franchise original); falls back to any.
pub const STARTER_SLUG: &str = "boba";

/// One entry in the Petdex manifest (only the fields we use).
#[derive(Debug, Clone, Deserialize)]
pub struct RemotePet {
    pub slug: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub kind: Option<String>,
    #[serde(rename = "submittedBy")]
    pub submitted_by: Option<String>,
    #[serde(rename = "spritesheetUrl")]
    pub spritesheet_url: String,
    #[serde(rename = "petJsonUrl")]
    pub pet_json_url: String,
}

impl RemotePet {
    pub fn name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.slug)
    }
    pub fn author(&self) -> &str {
        self.submitted_by.as_deref().unwrap_or("community")
    }
}

/// Tolerantly parses the manifest body into pets, skipping malformed entries
/// (mirrors the macOS `Lenient` decode wrapper).
pub fn parse_manifest(bytes: &[u8]) -> Vec<RemotePet> {
    let Ok(root) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return Vec::new();
    };
    let Some(items) = root.get("pets").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|v| serde_json::from_value::<RemotePet>(v.clone()).ok())
        .collect()
}

/// Fetches and parses the live manifest.
pub async fn fetch_manifest() -> Result<Vec<RemotePet>, reqwest::Error> {
    let bytes = reqwest::get(MANIFEST_URL).await?.bytes().await?;
    Ok(parse_manifest(&bytes))
}

/// Minimal `pet.json` shape needed to know the spritesheet filename + id.
#[derive(Deserialize)]
struct PackMeta {
    id: Option<String>,
    #[serde(rename = "spritesheetPath")]
    spritesheet_path: String,
}

/// Downloads a pack (`pet.json` + spritesheet) into `~/.agentpet/pets/<slug>/`.
/// Returns the installed pack id (the manifest `id`, or the slug).
pub async fn download(pet: &RemotePet) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let dir = pets_dir().join(&pet.slug);
    std::fs::create_dir_all(&dir)?;

    let pet_json = reqwest::get(&pet.pet_json_url).await?.bytes().await?;
    let meta: PackMeta = serde_json::from_slice(&pet_json)?;
    std::fs::write(dir.join("pet.json"), &pet_json)?;

    let sheet = reqwest::get(&pet.spritesheet_url).await?.bytes().await?;
    std::fs::write(dir.join(&meta.spritesheet_path), &sheet)?;

    Ok(meta.id.unwrap_or_else(|| pet.slug.clone()))
}

/// `~/.agentpet/pets`.
pub fn pets_dir() -> PathBuf {
    ipc::base_dir().join("pets")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_and_skips_bad_entries() {
        let body = br#"{
            "pets": [
                {"slug":"boba","displayName":"Boba","kind":"character",
                 "submittedBy":"alice","spritesheetUrl":"https://x/boba.png","petJsonUrl":"https://x/boba.json"},
                {"slug":"broken"},
                {"slug":"cube","spritesheetUrl":"https://x/c.png","petJsonUrl":"https://x/c.json"}
            ]
        }"#;
        let pets = parse_manifest(body);
        assert_eq!(pets.len(), 2, "the entry missing required URLs is skipped");
        assert_eq!(pets[0].slug, "boba");
        assert_eq!(pets[0].name(), "Boba");
        assert_eq!(pets[0].author(), "alice");
        assert_eq!(pets[1].name(), "cube", "falls back to slug when no displayName");
        assert_eq!(pets[1].author(), "community");
    }

    #[test]
    fn parse_handles_garbage() {
        assert!(parse_manifest(b"not json").is_empty());
        assert!(parse_manifest(br#"{"nope": 1}"#).is_empty());
    }
}
