//! CLI subcommands: the fast `hook` event sender and the `run` wrapper.

pub mod hook;
pub mod run;
pub mod uninstall;

use agentpet_core::event::AgentEvent;
use agentpet_core::ipc;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;

/// Sends an event to the daemon over the Unix socket, falling back to a queue
/// file when the daemon is down so no event is lost. Ports `EventSender.swift`.
pub fn send_event(event: &AgentEvent) {
    let Ok(line) = ipc::encode_line(event) else {
        return;
    };
    if write_to_socket(&line).is_ok() {
        return;
    }
    write_to_queue(&line);
}

fn write_to_socket(line: &[u8]) -> std::io::Result<()> {
    let mut stream = UnixStream::connect(ipc::socket_path())?;
    stream.write_all(line)
}

fn write_to_queue(line: &[u8]) {
    let dir = ipc::queue_dir();
    let _ = std::fs::create_dir_all(&dir);
    let now = crate::unix_now();
    // Clear events too old to ever replay usefully, so the queue can't grow
    // without bound when the daemon stays down for a long time.
    prune_expired_queue(&dir, now);
    let name = ipc::queue_file_name(now as i64, &unique_token());
    let _ = std::fs::write(dir.join(name), line);
}

/// Deletes queue files past `ipc::QUEUE_MAX_AGE_SECS` (anything the daemon would
/// prune the moment it replayed it). Unparsable/foreign names are left alone.
fn prune_expired_queue(dir: &Path, now: f64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_str()
            .map(|name| ipc::queue_file_expired(name, now))
            .unwrap_or(false)
        {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// A process-unique token for queue-file names (we avoid a uuid dependency;
/// uniqueness within a directory is all that's required).
pub fn unique_token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{}-{}", std::process::id(), nanos)
}
