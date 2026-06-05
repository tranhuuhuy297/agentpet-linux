//! Launch-at-login via an XDG autostart `.desktop` file. Ports `LoginItem.swift`.
//!
//! We use `~/.config/autostart/agentpet.desktop` rather than a systemd user
//! service: it starts only in a graphical session (so XWayland is up before the
//! pet maps) and is trivial to toggle by writing/removing one file.

use std::io;
use std::path::PathBuf;

const FILE_NAME: &str = "agentpet.desktop";

/// `~/.config/autostart/agentpet.desktop`.
pub fn desktop_path() -> PathBuf {
    autostart_dir().join(FILE_NAME)
}

fn autostart_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("autostart")
}

pub fn is_enabled() -> bool {
    desktop_path().exists()
}

/// Writes the autostart entry pointing at `exec_path` (the installed binary).
pub fn enable(exec_path: &str) -> io::Result<()> {
    write_desktop(&desktop_path(), exec_path)
}

pub fn disable() -> io::Result<()> {
    remove_desktop(&desktop_path())
}

// Path-parameterised helpers so the logic is unit-testable without touching the
// real ~/.config.
fn write_desktop(path: &std::path::Path, exec_path: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let contents = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=AgentPet\n\
         Comment=Monitor your AI coding agents\n\
         Exec={exec_path}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n"
    );
    std::fs::write(path, contents)
}

fn remove_desktop(path: &std::path::Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_then_disable_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("autostart").join(FILE_NAME);

        write_desktop(&path, "/usr/bin/agentpet").unwrap();
        assert!(path.exists());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("Exec=/usr/bin/agentpet"));
        assert!(text.contains("X-GNOME-Autostart-enabled=true"));

        remove_desktop(&path).unwrap();
        assert!(!path.exists());
        // Disabling an already-absent entry is a no-op, not an error.
        remove_desktop(&path).unwrap();
    }
}
