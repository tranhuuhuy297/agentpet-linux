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
/// The asset CDN gates downloads on this Referer (returns 403 otherwise).
const ASSET_REFERER: &str = "https://petdex.crafter.run/";
const USER_AGENT: &str = concat!("AgentPet/", env!("CARGO_PKG_VERSION"));

/// Shared HTTP client carrying our user-agent.
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .unwrap_or_default()
}

/// GETs a URL with the Referer the Petdex CDN requires.
async fn get_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let resp = client
        .get(url)
        .header(reqwest::header::REFERER, ASSET_REFERER)
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.bytes().await?.to_vec())
}

/// One entry in the Petdex manifest (only the fields we use).
#[derive(Debug, Clone, Deserialize)]
pub struct RemotePet {
    pub slug: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    /// Petdex category (character/creature/object); retained for a future filter.
    #[allow(dead_code)]
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

/// Maps an HTTP failure to a user-facing message that says *who* is at fault
/// (Petdex's hosting, the user's connection, or the request) and what to do.
/// Pure so it's unit-testable; `friendly_error` extracts the inputs.
fn describe_http_failure(status: Option<u16>, unreachable: bool) -> String {
    match status {
        Some(429) => "Petdex's server is rate-limited (their hosting hit its quota) — \
                      not a problem on your end. Try again in a few hours."
            .into(),
        Some(s) if (500..600).contains(&s) => {
            format!("Petdex's server is having problems (HTTP {s}). Try again later.")
        }
        Some(s) => format!("Petdex rejected the request (HTTP {s})."),
        None if unreachable => "Couldn't reach Petdex — check your internet connection.".into(),
        None => "Network error while talking to Petdex.".into(),
    }
}

/// Walks an error's source chain to the underlying `reqwest::Error` (if any)
/// and renders it via [`describe_http_failure`]. Non-network errors (bad JSON,
/// disk write) fall through to their own message.
pub fn friendly_error(e: &(dyn std::error::Error + 'static)) -> String {
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(err) = cur {
        if let Some(req) = err.downcast_ref::<reqwest::Error>() {
            let status = req.status().map(|s| s.as_u16());
            return describe_http_failure(status, req.is_connect() || req.is_timeout());
        }
        cur = err.source();
    }
    format!("Download failed: {e}")
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
    let bytes = get_bytes(&client(), MANIFEST_URL).await?;
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

    // Fetch first, write second — so a failed download never leaves a partial
    // pack on disk.
    let http = client();
    let result = async {
        let pet_json = get_bytes(&http, &pet.pet_json_url).await?;
        let meta: PackMeta = serde_json::from_slice(&pet_json)?;
        let sheet = get_bytes(&http, &pet.spritesheet_url).await?;
        std::fs::write(dir.join("pet.json"), &pet_json)?;
        std::fs::write(dir.join(&meta.spritesheet_path), &sheet)?;
        Ok::<String, Box<dyn std::error::Error + Send + Sync>>(meta.id.unwrap_or_else(|| pet.slug.clone()))
    }
    .await;

    if result.is_err() {
        let _ = std::fs::remove_dir_all(&dir); // clean up the empty/partial dir
    }
    result
}

/// `~/.agentpet/pets`.
pub fn pets_dir() -> PathBuf {
    ipc::base_dir().join("pets")
}

/// Services gallery requests on the tokio runtime: fetches the manifest and
/// downloads packs, reporting results back to the GTK side. On a successful
/// download it selects the pack and signals a pet reload.
pub async fn gallery_worker(
    rx: async_channel::Receiver<crate::snapshot::GalleryRequest>,
    tx: async_channel::Sender<crate::snapshot::GalleryResult>,
    reload: async_channel::Sender<()>,
) {
    use crate::snapshot::{GalleryRequest, GalleryResult};
    while let Ok(req) = rx.recv().await {
        match req {
            GalleryRequest::Fetch => {
                let result = match fetch_manifest().await {
                    Ok(pets) => GalleryResult::Manifest(pets),
                    Err(e) => GalleryResult::Failed(format!(
                        "Couldn't load the pet library — {}",
                        friendly_error(&e)
                    )),
                };
                let _ = tx.send(result).await;
            }
            GalleryRequest::Download(pet) => {
                let result = match download(&pet).await {
                    Ok(id) => {
                        let mut cfg = agentpet_core::config::Config::load();
                        cfg.selected_pet_id = Some(id.clone());
                        let _ = cfg.save();
                        let _ = reload.send(()).await;
                        GalleryResult::Downloaded(id)
                    }
                    Err(e) => GalleryResult::Failed(format!(
                        "Couldn't install '{}' — {}",
                        pet.name(),
                        friendly_error(e.as_ref())
                    )),
                };
                let _ = tx.send(result).await;
            }
        }
    }
}

/// True if at least one pet pack is already installed.
pub fn has_installed_pack() -> bool {
    std::fs::read_dir(pets_dir())
        .map(|rd| rd.flatten().any(|e| e.path().join("pet.json").exists()))
        .unwrap_or(false)
}

/// On first launch (no pack installed), downloads the starter pet and selects
/// it. Always signals `reload` so the GTK side loads whatever is present.
/// Network failures are non-fatal (the blob fallback keeps the pet visible).
pub async fn bootstrap_if_needed(reload: async_channel::Sender<()>) {
    if has_installed_pack() {
        let _ = reload.send(()).await;
        return;
    }
    let Ok(pets) = fetch_manifest().await else {
        return;
    };
    let pick = pets.iter().find(|p| p.slug == STARTER_SLUG).or_else(|| pets.first());
    if let Some(pet) = pick {
        match download(pet).await {
            Ok(id) => {
                let mut cfg = agentpet_core::config::Config::load();
                cfg.selected_pet_id = Some(id);
                let _ = cfg.save();
                let _ = reload.send(()).await;
                eprintln!("agentpet: installed starter pet '{}'", pet.slug);
            }
            Err(e) => {
                eprintln!("agentpet: starter pet download failed: {}", friendly_error(e.as_ref()))
            }
        }
    }
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

    #[test]
    fn http_failures_name_the_culprit() {
        // Petdex's hosting at fault: rate limit and server errors.
        assert!(describe_http_failure(Some(429), false).contains("rate-limited"));
        assert!(describe_http_failure(Some(503), false).contains("HTTP 503"));
        // Request rejected (e.g. 403 missing Referer).
        assert!(describe_http_failure(Some(403), false).contains("HTTP 403"));
        // User's connection at fault.
        assert!(describe_http_failure(None, true).contains("internet connection"));
        // Unknown network failure.
        assert!(describe_http_failure(None, false).contains("Network error"));
    }

    #[test]
    fn friendly_error_falls_through_for_non_network_errors() {
        let e = serde_json::from_slice::<PackMeta>(b"not json").err().unwrap();
        assert!(friendly_error(&e).starts_with("Download failed:"));
    }
}
