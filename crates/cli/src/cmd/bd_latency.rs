//! What `bd` cost, read back out of the event log (air-869).
//!
//! `log_event` stamps `bd_ms`/`bd_calls` on every event line whose command shelled out to
//! bd. This reads a day's file and reports the median cost of one bd process, so the claim
//! "bd is ~1.4 s per process here" stays a measurement the coordinator can check rather
//! than a number in a doc. Lines without `bd_ms` never called bd and are not counted: a
//! command that skipped bd is not a fast bd.

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BdLatency {
    /// Median milliseconds for one bd process.
    pub median_ms: u64,
    /// How many bd processes the samples came from.
    pub calls: u64,
    /// How many event lines contributed.
    pub events: usize,
}

/// Median over the bd processes described by these event lines. `None` when no line called
/// bd. Malformed lines are skipped, never fatal: the event log is append-only and a partial
/// last line is normal.
pub fn from_events(text: &str) -> Option<BdLatency> {
    let mut per_call: Vec<u64> = Vec::new();
    let mut events = 0usize;
    for line in text.lines() {
        // Cheap pre-filter: most lines (hooks) never touched bd.
        if !line.contains("\"bd_ms\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let (Some(ms), Some(calls)) = (v["bd_ms"].as_u64(), v["bd_calls"].as_u64()) else {
            continue;
        };
        if calls == 0 {
            continue;
        }
        events += 1;
        // One sample per bd process, so a command that ran four calls weighs four.
        for _ in 0..calls.min(64) {
            per_call.push(ms / calls);
        }
    }
    if per_call.is_empty() {
        return None;
    }
    per_call.sort_unstable();
    let median = *per_call.get(per_call.len() / 2)?;
    Some(BdLatency {
        median_ms: median,
        calls: per_call.len() as u64,
        events,
    })
}

/// The same, for the ledger's events file of `day`.
pub fn for_day(air_dir: &Path, day: &str) -> Option<BdLatency> {
    let path = air_ledger::events::events_path(air_dir, day);
    from_events(&std::fs::read_to_string(path).ok()?)
}

/// One line for `air status`.
pub fn line(l: &BdLatency) -> String {
    format!(
        "bd: median {} ms per call over {} call(s) in {} command(s) today\n",
        l.median_ms, l.calls, l.events
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn median_counts_processes_and_ignores_lines_that_never_called_bd() {
        let text = concat!(
            r#"{"command":"hook","decision":"allow"}"#,
            "\n",
            r#"{"command":"claim","bd_ms":1400,"bd_calls":1}"#,
            "\n",
            r#"{"command":"status","bd_ms":6000,"bd_calls":3}"#,
            "\n",
            "not json at all\n",
            r#"{"command":"close","bd_ms":100,"bd_calls":0}"#,
            "\n",
        );
        let l = from_events(text).unwrap();
        // Samples: 1400, 2000, 2000, 2000 -> median 2000; the hook line and the zero-call
        // line contribute nothing.
        assert_eq!(
            (l.median_ms, l.calls, l.events),
            (2000, 4, 2),
            "{}",
            line(&l)
        );
        assert!(from_events(r#"{"command":"hook"}"#).is_none());
    }
}
