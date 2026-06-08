//! Unix-socket wire format + on-disk locations. Ports the encode/decode and
//! path logic from `EventCoding.swift`, `EventSender.swift`, and
//! `EventSocketServer.swift` (the syscall/socket IO itself lives in the binary
//! crate; here we keep the pure, testable parts: framing, decoding, paths,
//! queue-file naming).

use crate::event::AgentEvent;
use crate::state::UnixTime;
use std::path::PathBuf;

/// `~/.agentpet` — base directory for the socket, queue, and pet packs.
pub fn base_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".agentpet")
}

/// `~/.agentpet/agentpet.sock` — the daemon's Unix domain socket.
pub fn socket_path() -> PathBuf {
    base_dir().join("agentpet.sock")
}

/// `~/.agentpet/queue` — events written here while the daemon is down.
pub fn queue_dir() -> PathBuf {
    base_dir().join("queue")
}

/// Encodes one event as a newline-terminated JSON line (the wire frame).
pub fn encode_line(event: &AgentEvent) -> serde_json::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(event)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Decodes newline-delimited JSON into events, skipping empty/undecodable lines
/// (mirrors the daemon's tolerant `decodeLines`).
pub fn decode_lines(data: &[u8]) -> Vec<AgentEvent> {
    data.split(|&b| b == b'\n')
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_slice(line).ok())
        .collect()
}

/// Queue filename `<int-seconds>-<uuid>.json`. Sorting these names
/// lexicographically replays events in (roughly) arrival order, as the daemon
/// expects. `now_secs`/`uuid` are passed in to keep this pure.
pub fn queue_file_name(now_secs: i64, uuid: &str) -> String {
    format!("{now_secs}-{uuid}.json")
}

/// How long a queued event stays worth replaying. Matches the daemon's
/// `stale_active_after` prune window: a working/waiting session quiet for this
/// long is dropped, so an event older than this would be pruned the instant the
/// daemon replayed it. Capping the queue here bounds the directory (it can't
/// grow without limit while the daemon is down) without discarding anything the
/// daemon would have kept.
pub const QUEUE_MAX_AGE_SECS: UnixTime = 300.0;

/// Age in seconds of a queue file, parsed from the leading `<seconds>-` prefix
/// that `queue_file_name` writes. `None` when the name doesn't begin with an
/// integer second count (a foreign/garbage file we must not touch).
pub fn queue_file_age_secs(filename: &str, now: UnixTime) -> Option<UnixTime> {
    let secs: i64 = filename.split_once('-')?.0.parse().ok()?;
    Some(now - secs as UnixTime)
}

/// Whether a queue file is too old to be worth replaying (see
/// `QUEUE_MAX_AGE_SECS`). Names we can't parse are treated as not-expired so
/// foreign files are left alone.
pub fn queue_file_expired(filename: &str, now: UnixTime) -> bool {
    queue_file_age_secs(filename, now)
        .map(|age| age > QUEUE_MAX_AGE_SECS)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AgentKind;

    #[test]
    fn line_roundtrips() {
        let ev = AgentEvent::new("s1", AgentKind::Claude, "Stop", Some("/p".into()), None, 12.0);
        let line = encode_line(&ev).unwrap();
        assert_eq!(*line.last().unwrap(), b'\n');
        let decoded = decode_lines(&line);
        assert_eq!(decoded, vec![ev]);
    }

    #[test]
    fn decode_skips_blank_and_garbage_lines() {
        let a = AgentEvent::new("a", AgentKind::Codex, "Stop", None, None, 1.0);
        let b = AgentEvent::new("b", AgentKind::Codex, "Stop", None, None, 2.0);
        let mut buf = encode_line(&a).unwrap();
        buf.extend_from_slice(b"\n");           // blank line
        buf.extend_from_slice(b"not json\n");   // garbage
        buf.extend_from_slice(&encode_line(&b).unwrap());
        let decoded = decode_lines(&buf);
        assert_eq!(decoded, vec![a, b]);
    }

    #[test]
    fn queue_name_sorts_by_time() {
        let early = queue_file_name(100, "z");
        let late = queue_file_name(200, "a");
        assert!(early < late, "older timestamp sorts first regardless of uuid");
    }

    #[test]
    fn queue_file_age_and_expiry() {
        // Token itself contains '-' (pid-nanos); age is parsed from the first field.
        let name = queue_file_name(100, "47305-882910");
        assert_eq!(name, "100-47305-882910.json");
        assert_eq!(queue_file_age_secs(&name, 130.0), Some(30.0));
        assert!(!queue_file_expired(&name, 100.0 + QUEUE_MAX_AGE_SECS), "exactly at cap is kept");
        assert!(queue_file_expired(&name, 100.0 + QUEUE_MAX_AGE_SECS + 1.0), "past cap is expired");

        // Foreign / unparsable names are left untouched (never expired).
        assert_eq!(queue_file_age_secs("notanumber-x.json", 0.0), None);
        assert!(!queue_file_expired("README.md", 1_000_000.0));
    }
}
