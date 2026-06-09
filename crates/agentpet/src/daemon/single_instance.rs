//! Process-wide single-instance lock via an advisory `flock` on a lock file.
//!
//! Why not just probe the Unix socket? A connect-probe ("is anyone answering on
//! the socket?") has two problems: a TOCTOU window where two launches both see
//! "no daemon" and race to bind, and it depends on whether a stale socket file
//! was cleaned up after a crash. An exclusive `flock` avoids both — the kernel
//! holds it for the lifetime of the open file and drops it automatically when
//! the process exits or crashes, so a dead daemon never leaves the lock stuck.

use agentpet_core::ipc;
use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

fn lock_path() -> PathBuf {
    ipc::base_dir().join("agentpet.lock")
}

/// Tries to take the exclusive single-instance lock without blocking.
///
/// Returns the held `File` on success — the caller MUST keep it alive for the
/// whole process lifetime, since dropping (or exiting) releases the lock.
/// Returns `None` if another agentpet process already holds it.
pub fn acquire() -> Option<File> {
    let _ = std::fs::create_dir_all(ipc::base_dir());
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(lock_path())
        .ok()?;
    // LOCK_EX | LOCK_NB: take the exclusive lock or fail fast if it's held.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        Some(file)
    } else {
        None
    }
}
