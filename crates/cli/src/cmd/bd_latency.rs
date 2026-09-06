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
        let (Some(ms), Some(calls)) = (
            v.get("bd_ms").and_then(serde_json::Value::as_u64),
            v.get("bd_calls").and_then(serde_json::Value::as_u64),
        ) else {
            continue;
        };
        let Some(each) = ms.checked_div(calls) else {
            continue;
        };
        events = events.saturating_add(1);
        // One sample per bd process, so a command that ran four calls weighs four.
        for _ in 0..calls.min(64) {
            per_call.push(each);
        }
    }
    if per_call.is_empty() {
        return None;
    }
    per_call.sort_unstable();
    let median = *per_call.get(per_call.len() / 2)?;
    Some(BdLatency {
        median_ms: median,
        calls: u64::try_from(per_call.len()).unwrap_or(u64::MAX),
        events,
    })
}

/// Wall time of a command, as percentiles over the event lines that recorded one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Percentiles {
    pub runs: usize,
    pub p50_ms: u64,
    pub p90_ms: u64,
    pub p99_ms: u64,
    pub max_ms: u64,
}

/// How long `air status` took, from its own event lines (air-p61).
///
/// `status` stamps `duration_ms` on every run. The adopter's took ~20 s under load and they
/// wrapped it in a 60 s timeout in `reclaim.py`; this repo's is seconds too. A command the
/// coordinator runs on a loop being slow is a fact the audit should carry, not something each
/// reader re-derives.
pub fn status_from_events(text: &str) -> Option<Percentiles> {
    let mut ms: Vec<u64> = Vec::new();
    for line in text.lines() {
        if !line.contains("\"duration_ms\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if !v
            .get("command")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|c| c.starts_with("status"))
        {
            continue;
        }
        if let Some(d) = v
            .get("inputs")
            .and_then(|i| i.get("duration_ms"))
            .and_then(serde_json::Value::as_u64)
        {
            ms.push(d);
        }
    }
    percentiles(&mut ms)
}

/// Percentiles of a sample, sorted in place. `None` for an empty sample.
///
/// Integer arithmetic throughout: an index is `len * numerator / denominator`, so there is no
/// float conversion to round the wrong way on a large sample.
pub fn percentiles(ms: &mut [u64]) -> Option<Percentiles> {
    if ms.is_empty() {
        return None;
    }
    ms.sort_unstable();
    let last = ms.len().saturating_sub(1);
    let at = |num: usize, den: usize| -> u64 {
        let i = ms.len().saturating_mul(num).checked_div(den).unwrap_or(0);
        ms.get(i.min(last)).copied().unwrap_or(0)
    };
    Some(Percentiles {
        runs: ms.len(),
        p50_ms: at(1, 2),
        p90_ms: at(9, 10),
        p99_ms: at(99, 100),
        max_ms: ms.last().copied().unwrap_or(0),
    })
}

/// How long `air status` may wait for one `bd` process, derived from what bd actually costs
/// here rather than from a number somebody typed (air-p61).
///
/// The old value was a flat 2 s. Measured over 495,892 bd processes in this ledger, bd's p50
/// is 1430 ms and its p99 is 1644 ms — 356 ms of headroom, and 59 calls already exceeded it.
/// The adopter's median is 1760 ms, which is **above** our whole budget: at their latency the
/// status reconcile times out on ordinary calls, which is what air-19u was.
///
/// Four times the median, because bd's cost is a process start plus a store open and scales
/// with load rather than with the query. The floor keeps a cold ledger (no measurement yet)
/// where it was. The cap is the real constraint and it is not arbitrary: `air status` backs
/// the MCP channel, whose tool budget is 20 s, and air-19u is precisely the failure of the
/// channel getting nothing when the fleet is busiest. A budget that can approach the tool
/// budget trades one silence for another.
pub fn status_bd_budget(median_ms: Option<u64>) -> std::time::Duration {
    const FLOOR_MS: u64 = 2_000;
    const CAP_MS: u64 = 8_000;
    let want = median_ms
        .and_then(|m| m.checked_mul(4))
        .unwrap_or(FLOOR_MS)
        .clamp(FLOOR_MS, CAP_MS);
    std::time::Duration::from_millis(want)
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
