//! The monitor daemon: owns the live `SessionStore`, drains any events queued
//! while it was down, listens on the Unix socket, and prunes stale sessions on
//! a timer. Ports `AppDaemon.swift` + `EventSocketServer.swift`.
//!
//! For now it runs *headless* (logs state to stdout). Later phases attach the
//! GTK tray/monitor/pet by reusing this socket server and forwarding session
//! snapshots over an `async-channel` to the GTK main thread.

use agentpet_core::ipc;
use agentpet_core::mapper::StateMapper;
use agentpet_core::session::{AgentSession, SessionStore};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::{UnixListener, UnixStream};

type Store = Arc<Mutex<SessionStore>>;

/// Builds a Tokio runtime and runs the daemon to completion.
pub fn run_headless() -> ExitCode {
    let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("agentpet: failed to start runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    rt.block_on(serve())
}

async fn serve() -> ExitCode {
    let _ = std::fs::create_dir_all(ipc::base_dir());
    let store: Store = Arc::new(Mutex::new(SessionStore::new()));

    // Replay queued events with their original timestamps, then prune so
    // sessions that ended while we were down look stale and don't resurrect.
    drain_queue(&store);
    store.lock().unwrap().prune(crate::unix_now());
    log_sessions(&store);

    // Single-instance guard: a live daemon already answering on the socket wins.
    let sock = ipc::socket_path();
    if std::os::unix::net::UnixStream::connect(&sock).is_ok() {
        eprintln!("agentpet: a daemon is already running ({})", sock.display());
        return ExitCode::FAILURE;
    }
    let _ = std::fs::remove_file(&sock); // clear a stale socket file
    let listener = match UnixListener::bind(&sock) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("agentpet: cannot bind {}: {e}", sock.display());
            return ExitCode::FAILURE;
        }
    };
    eprintln!("agentpet daemon listening on {}", sock.display());

    spawn_prune_timer(store.clone());

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let store = store.clone();
                tokio::spawn(async move { handle_client(stream, store).await });
            }
            Err(e) => eprintln!("agentpet: accept error: {e}"),
        }
    }
}

/// Reads one client's full stream, decodes newline-delimited events, and applies
/// each to the store (mirrors the daemon's per-connection handling).
async fn handle_client(mut stream: UnixStream, store: Store) {
    let mut buf = Vec::new();
    if stream.read_to_end(&mut buf).await.is_err() {
        return;
    }
    let events = ipc::decode_lines(&buf);
    let mut changed = false;
    for ev in &events {
        // A session-end event removes the session (apply returns None for it),
        // so treat it as a change too — otherwise removals never refresh.
        let ended = StateMapper::is_session_end(ev.agent_kind, &ev.event_name);
        let before = store.lock().unwrap().session(&ev.session_id).map(|s| s.state);
        let updated = store.lock().unwrap().apply(ev, crate::unix_now());
        match updated {
            Some(s) => {
                println!("• {:<10} {:<20} ({})", state_label(&s), short_project(&s), ev.event_name);
                crate::notify::on_transition(before, &s);
                changed = true;
            }
            None if ended => {
                println!("• {:<10} {:<20} ({})", "ended", ev.session_id, ev.event_name);
                changed = true;
            }
            None => {}
        }
    }
    if changed {
        log_sessions(&store);
    }
}

fn spawn_prune_timer(store: Store) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        loop {
            tick.tick().await;
            let (before, after) = {
                let mut s = store.lock().unwrap();
                let before = s.sessions().len();
                s.prune(crate::unix_now());
                (before, s.sessions().len())
            };
            if before != after {
                log_sessions(&store);
            }
        }
    });
}

/// Drains queued event files (written while the daemon was down) in name order,
/// applying each with its *original* timestamp, then removing the file.
fn drain_queue(store: &Store) {
    let dir = ipc::queue_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if let Ok(data) = std::fs::read(&path) {
            for ev in ipc::decode_lines(&data) {
                store.lock().unwrap().apply(&ev, ev.timestamp);
            }
        }
        let _ = std::fs::remove_file(&path);
    }
}

fn log_sessions(store: &Store) {
    let sessions = store.lock().unwrap().sorted();
    if sessions.is_empty() {
        eprintln!("[agentpet] no active sessions");
        return;
    }
    eprintln!("[agentpet] {} session(s):", sessions.len());
    for s in &sessions {
        eprintln!("    {:<10} {}", state_label(s), short_project(s));
    }
}

fn state_label(s: &AgentSession) -> String {
    format!("{:?}", s.state).to_lowercase()
}

fn short_project(s: &AgentSession) -> String {
    s.project
        .as_deref()
        .map(|p| {
            std::path::Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string())
        })
        .unwrap_or_else(|| s.id.clone())
}
