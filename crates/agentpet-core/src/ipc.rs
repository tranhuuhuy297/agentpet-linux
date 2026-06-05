//! Unix-socket wire format + on-disk locations. Ports the encode/decode and
//! path logic from `EventCoding.swift`, `EventSender.swift`, and
//! `EventSocketServer.swift` (the syscall/socket IO itself lives in the binary
//! crate; here we keep the pure, testable parts: framing, decoding, paths,
//! queue-file naming).

use crate::event::AgentEvent;
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
}
