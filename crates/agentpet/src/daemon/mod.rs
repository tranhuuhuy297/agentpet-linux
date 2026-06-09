//! The monitor daemon: owns the live `SessionStore`, drains events queued while
//! it was down, listens on the Unix socket, prunes stale sessions on a timer,
//! and (when a `sink` is provided) ships a `UiUpdate` snapshot to the GTK thread
//! on every change. Ports `AppDaemon.swift` + `EventSocketServer.swift`.
//!
//! With `sink = None` it runs headless (logs to stderr) — useful for testing
//! without a display.

pub mod single_instance;

use crate::snapshot::UiUpdate;
use agentpet_core::ipc;
use agentpet_core::mapper::StateMapper;
use agentpet_core::session::{AgentSession, SessionStore};
use async_channel::Sender;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::{UnixListener, UnixStream};

type Store = Arc<Mutex<SessionStore>>;
type Sink = Option<Sender<UiUpdate>>;

/// Runs the daemon headless (no UI) to completion on a fresh Tokio runtime.
pub fn run_headless() -> ExitCode {
    let Some(_lock) = single_instance::acquire() else {
        eprintln!("agentpet: a daemon is already running");
        return ExitCode::FAILURE;
    };
    match build_runtime() {
        Ok(rt) => rt.block_on(serve(None)),
        Err(e) => {
            eprintln!("agentpet: failed to start runtime: {e}");
            ExitCode::FAILURE
        }
    }
}

pub fn build_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread().enable_all().build()
}

/// The socket server. Emits a snapshot through `sink` on every change.
pub async fn serve(sink: Sink) -> ExitCode {
    let _ = std::fs::create_dir_all(ipc::base_dir());
    crate::notify::init(); // notifications run on a dedicated thread, off the runtime
    let store: Store = Arc::new(Mutex::new(SessionStore::new()));

    // Replay queued events with their original timestamps, then prune so
    // sessions that ended while we were down look stale and don't resurrect.
    drain_queue(&store);
    store.lock().unwrap().prune(crate::unix_now());
    emit(&store, &sink);
    log_sessions(&store);

    // Single-instance guard: a live daemon already answering on the socket wins.
    let sock = ipc::socket_path();
    if std::os::unix::net::UnixStream::connect(&sock).is_ok() {
        eprintln!("agentpet: a daemon is already running ({})", sock.display());
        return ExitCode::FAILURE;
    }
    let _ = std::fs::remove_file(&sock);
    let listener = match UnixListener::bind(&sock) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("agentpet: cannot bind {}: {e}", sock.display());
            return ExitCode::FAILURE;
        }
    };
    eprintln!("agentpet daemon listening on {}", sock.display());

    spawn_prune_timer(store.clone(), sink.clone());

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let store = store.clone();
                let sink = sink.clone();
                tokio::spawn(async move { handle_client(stream, store, sink).await });
            }
            Err(e) => eprintln!("agentpet: accept error: {e}"),
        }
    }
}

async fn handle_client(mut stream: UnixStream, store: Store, sink: Sink) {
    let mut buf = Vec::new();
    if stream.read_to_end(&mut buf).await.is_err() {
        return;
    }
    let events = ipc::decode_lines(&buf);
    let mut changed = false;
    for ev in &events {
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
        emit(&store, &sink);
    }
}

fn spawn_prune_timer(store: Store, sink: Sink) {
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
                emit(&store, &sink);
            }
        }
    });
}

/// Sends a fresh snapshot to the GTK thread (no-op when headless).
fn emit(store: &Store, sink: &Sink) {
    if let Some(tx) = sink {
        let sessions = store.lock().unwrap().sorted();
        let _ = tx.try_send(UiUpdate::from_sessions(sessions));
    }
}

/// Drains queued event files (written while the daemon was down) in name order,
/// applying each fresh-enough one with its *original* timestamp and removing
/// every file. Files past the replay window are deleted without applying.
fn drain_queue(store: &Store) {
    let dir = ipc::queue_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    let now = crate::unix_now();
    for path in paths {
        // Skip events too old to matter (they'd be pruned on apply anyway), but
        // still delete the file so the queue is left empty.
        let expired = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| ipc::queue_file_expired(n, now))
            .unwrap_or(false);
        if !expired {
            if let Ok(data) = std::fs::read(&path) {
                for ev in ipc::decode_lines(&data) {
                    store.lock().unwrap().apply(&ev, ev.timestamp);
                }
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
