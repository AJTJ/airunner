//! How long the lane's loop waits on messages (air-1vri.2).
//!
//! The owner watched one bead wait minutes between done and closed in the second live trial,
//! because the lane learned a branch was batch-ready and the workers learned the lane's green
//! only on their own 5-minute wakes. These two numbers say whether the pushes fixed that:
//!
//! - batch-ready to its batch: from the moment Air queued "batch-ready" for the lane (or told it
//!   in `air land`'s output) to the start of the first lane verify whose members include that
//!   head. The cut itself leaves no row; the verify it feeds starts seconds later.
//! - green to close: from the moment Air queued "batch green" for a member to the release of
//!   that member's claim on each bead the message named.
//!
//! Read from the ledger's rows, never from the event log, so nothing parses prose. A wait that
//! has not ended is not a sample. Removed with the lane, or when the harness reports these waits
//! itself.

use serde::Serialize;

use air_ledger::Ledger;
use air_ledger::verify::Kind;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LoopTimes {
    /// Seconds from batch-ready to the batch verify's start, one per branch head.
    pub ready_to_batch: Vec<i64>,
    /// Seconds from a batch green's message to the member's close, one per bead.
    pub green_to_close: Vec<i64>,
}

/// The middle value, the lower of the two middles for an even count.
pub fn median(v: &[i64]) -> Option<i64> {
    let mut v = v.to_vec();
    v.sort_unstable();
    v.get(v.len().saturating_sub(1) / 2).copied()
}

/// Every wait that started at or after `since` and has ended.
pub fn measure(ledger: &Ledger, since: &str) -> LoopTimes {
    let mut out = LoopTimes::default();
    let Ok(rows) = ledger.deliveries_since(since) else {
        return out;
    };
    let runs = ledger.latest_runs(Kind::Verify, 500).unwrap_or_default();
    for d in rows.iter().filter(|d| d.kind == "batch-ready") {
        let started = runs
            .iter()
            .filter(|r| {
                r.started_at >= d.created_at && r.members.iter().any(|m| m.sha == d.subject)
            })
            .map(|r| r.started_at.as_str())
            .min();
        if let Some(s) = started.and_then(|s| super::status::seconds_between(&d.created_at, s)) {
            out.ready_to_batch.push(s);
        }
    }
    for d in rows.iter().filter(|d| d.kind == "batch-green") {
        for bead in d.subject.split_whitespace() {
            let released: Option<String> = ledger
                .conn()
                .query_row(
                    "SELECT released_at FROM claims WHERE bead=?1 AND worker=?2 \
                     AND released_at IS NOT NULL AND released_at >= ?3",
                    rusqlite::params![bead, d.to_worker, d.created_at],
                    |r| r.get(0),
                )
                .ok();
            if let Some(s) =
                released.and_then(|r| super::status::seconds_between(&d.created_at, &r))
            {
                out.green_to_close.push(s);
            }
        }
    }
    out
}

/// The `air status` line, or `None` when nothing was measured.
pub fn line(t: &LoopTimes) -> Option<String> {
    if t.ready_to_batch.is_empty() && t.green_to_close.is_empty() {
        return None;
    }
    let part = |v: &[i64], what: &str| {
        median(v).map_or_else(
            || format!("{what}: none yet"),
            |m| format!("{what}: median {m} s over {}", v.len()),
        )
    };
    Some(format!(
        "loops (24 h): {}; {}",
        part(&t.ready_to_batch, "batch-ready to its batch verify"),
        part(&t.green_to_close, "batch green to close")
    ))
}
