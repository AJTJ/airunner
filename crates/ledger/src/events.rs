//! Append-only NDJSON events: every question Air was asked, its answer, its reason, and its
//! denominator (plan 0001 §2 row 6; corpus: "code fails confidently" — an absent check must be
//! distinguishable from a passing one).

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{LedgerError, Result};

/// One event line. `at` is RFC 3339 UTC supplied by the caller.
#[derive(Debug, Serialize)]
pub struct Event<'a, T: Serialize> {
    pub at: &'a str,
    pub worker: &'a str,
    pub command: &'a str,
    /// Free-form, but always JSON — the CLI passes its parsed args.
    pub inputs: &'a T,
    pub decision: &'a str,
    pub reason: &'a str,
    /// e.g. "compared 4 worktrees, 6 pairs" — what the check actually looked at.
    pub denominator: &'a str,
    /// Milliseconds this command spent inside `bd` processes, and how many it ran. Absent
    /// when the command never shelled out to bd, so "no bd" and "fast bd" stay distinct
    /// (air-869: bd costs ~1.4 s per process here, and the cost was invisible).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bd_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bd_calls: Option<u64>,
}

/// Where today's events file lives: `<air_dir>/events/YYYY-MM-DD.ndjson`.
pub fn events_path(air_dir: &Path, day: &str) -> PathBuf {
    air_dir.join("events").join(format!("{day}.ndjson"))
}

/// Append one event. Creates the directory and file if needed. Never panics; errors are
/// returned so a hook can log-and-fail-open.
pub fn append<T: Serialize>(air_dir: &Path, day: &str, event: &Event<'_, T>) -> Result<()> {
    let path = events_path(air_dir, day);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| LedgerError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut line = serde_json::to_vec(event)?;
    line.push(b'\n');
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|source| LedgerError::Io {
            path: path.clone(),
            source,
        })?;
    f.write_all(&line)
        .map_err(|source| LedgerError::Io { path, source })?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn appends_one_json_line_per_event() {
        let dir = tempfile::tempdir().unwrap();
        let inputs = serde_json::json!({"bead": "fd-1"});
        let ev = Event {
            at: "2026-08-18T10:00:00Z",
            worker: "w1",
            command: "handover",
            inputs: &inputs,
            decision: "refuse",
            reason: "no verify at HEAD",
            denominator: "1 run checked",
            bd_ms: Some(1350),
            bd_calls: Some(1),
        };
        append(dir.path(), "2026-08-18", &ev).unwrap();
        append(dir.path(), "2026-08-18", &ev).unwrap();
        let text = std::fs::read_to_string(events_path(dir.path(), "2026-08-18")).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(v["decision"], "refuse");
        assert_eq!(v["inputs"]["bead"], "fd-1");
        assert_eq!(v["bd_ms"], 1350);
        // A command that never touched bd leaves the fields off entirely.
        let quiet = Event {
            bd_ms: None,
            bd_calls: None,
            ..ev
        };
        append(dir.path(), "2026-08-19", &quiet).unwrap();
        let text = std::fs::read_to_string(events_path(dir.path(), "2026-08-19")).unwrap();
        assert!(!text.contains("bd_ms"), "{text}");
    }
}
