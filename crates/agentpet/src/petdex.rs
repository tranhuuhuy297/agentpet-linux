//! Local Petdex pet library: lists the pet packs the user has installed with the
//! official Petdex CLI (`npx petdex@latest install <slug>`), which writes each
//! pack to `~/.petdex/pets/<slug>/` (a `pet.json` manifest + spritesheet).
//!
//! AgentPet hosts no art and performs no downloads — installation is delegated
//! to the Petdex CLI; here we only *read* that directory so the Settings → Pet
//! tab can offer the installed packs for per-agent selection. Pure scanning is
//! split out (`scan_dir`) and unit-tested; `scan_installed` wraps it with the
//! `$HOME`-relative path.

use agentpet_core::sprite::PetManifest;
use std::path::{Path, PathBuf};

/// The exact command users run to install a pet (shown as a Settings guide).
pub const INSTALL_HINT: &str = "npx petdex@latest install <slug>";

/// `~/.petdex/pets` — where the Petdex CLI installs pet packs.
pub fn installed_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".petdex").join("pets")
}

/// One locally-installed pet pack, as surfaced in the Pet tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPet {
    /// Directory name under `~/.petdex/pets` (the Petdex slug).
    pub slug: String,
    /// Pack id from `pet.json` (`id`) — what gets persisted as the agent's pick.
    pub id: String,
    /// Human-readable name from `pet.json` (`displayName`), else the slug.
    pub display_name: String,
}

/// Scans `~/.petdex/pets` for installed packs (see [`scan_dir`]).
pub fn scan_installed() -> Vec<InstalledPet> {
    scan_dir(&installed_dir())
}

/// Reads every `<dir>/<slug>/pet.json`, skipping entries without a decodable
/// manifest, and returns the packs sorted by display name (case-insensitive).
/// Pure (takes the directory) so it's unit-testable without touching `$HOME`.
pub fn scan_dir(dir: &Path) -> Vec<InstalledPet> {
    let mut pets = Vec::new();
    // A missing/unreadable dir is treated as "nothing installed" — the common
    // case is a user who hasn't run the Petdex CLI yet, and the empty-state
    // status line tells them how to install.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return pets;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(bytes) = std::fs::read(path.join("pet.json")) else {
            continue;
        };
        let Some(manifest) = PetManifest::decode(&bytes) else {
            continue;
        };
        let slug = entry.file_name().to_string_lossy().into_owned();
        let display_name = if manifest.display_name.is_empty() {
            slug.clone()
        } else {
            manifest.display_name
        };
        pets.push(InstalledPet { slug, id: manifest.id, display_name });
    }
    pets.sort_by_key(|p| p.display_name.to_lowercase());
    pets
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_pack(root: &Path, slug: &str, json: &str) {
        let dir = root.join(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pet.json"), json).unwrap();
    }

    #[test]
    fn scans_installed_packs_sorted_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        write_pack(
            tmp.path(),
            "snow-plum-lillia",
            r#"{"id":"snow-plum-lillia","displayName":"Snow Plum Lillia","spritesheetPath":"spritesheet.webp","version":"1.1.0"}"#,
        );
        write_pack(
            tmp.path(),
            "boba",
            r#"{"id":"boba","displayName":"Boba","spritesheetPath":"spritesheet.webp"}"#,
        );

        let pets = scan_dir(tmp.path());
        assert_eq!(pets.len(), 2);
        // Sorted case-insensitively by display name: "Boba" before "Snow…".
        assert_eq!(pets[0].id, "boba");
        assert_eq!(pets[0].display_name, "Boba");
        assert_eq!(pets[1].slug, "snow-plum-lillia");
        assert_eq!(pets[1].display_name, "Snow Plum Lillia");
    }

    #[test]
    fn skips_entries_without_a_decodable_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        write_pack(tmp.path(), "good", r#"{"id":"good","displayName":"Good","spritesheetPath":"s.webp"}"#);
        write_pack(tmp.path(), "broken", "not json");
        std::fs::create_dir_all(tmp.path().join("empty")).unwrap(); // no pet.json at all

        let pets = scan_dir(tmp.path());
        assert_eq!(pets.len(), 1, "only the decodable pack is listed");
        assert_eq!(pets[0].id, "good");
    }

    #[test]
    fn missing_directory_yields_no_pets() {
        assert!(scan_dir(Path::new("/nonexistent/petdex/pets")).is_empty());
    }
}
