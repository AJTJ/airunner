//! Claims: the intent record, supplementary to beads (decisions 2026-08-20: `air claim`
//! wraps `bd update --claim`; bd stays the atomic CAS; this table keeps the history bd does
//! not: when, declared files, hand-over attempts, release reason).

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{Ledger, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Claim {
    pub bead: String,
    pub worker: String,
    pub claimed_at: String,
    pub declared_files: Vec<String>,
    pub first_handover_at: Option<String>,
    pub last_handover_at: Option<String>,
    pub handover_attempts: i64,
    pub released_at: Option<String>,
    pub release_reason: Option<String>,
}

/// Release reasons the ledger accepts (measurement spec §2.3; plan 0001 §2 row 3).
pub const RELEASE_REASONS: &[&str] = &[
    "landed",
    "abandoned",
    "reassigned",
    "superseded",
    "false-premise",
    "owner-gated",
    "unknown",
    // Set by reconciliation, never by a worker: bd's status said the claim was over.
    "closed",
    "handed-over",
    "reconciled",
];

const COLS: &str = "bead, worker, claimed_at, declared_files, first_handover_at, \
                    last_handover_at, handover_attempts, released_at, release_reason";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Claim> {
    let files: Option<String> = r.get(3)?;
    Ok(Claim {
        bead: r.get(0)?,
        worker: r.get(1)?,
        claimed_at: r.get(2)?,
        declared_files: files
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        first_handover_at: r.get(4)?,
        last_handover_at: r.get(5)?,
        handover_attempts: r.get(6)?,
        released_at: r.get(7)?,
        release_reason: r.get(8)?,
    })
}

impl Ledger {
    /// The open (unreleased) claim on `bead`, by anyone.
    pub fn open_claim(&self, bead: &str) -> Result<Option<Claim>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLS} FROM claims WHERE bead=?1 AND released_at IS NULL"),
                params![bead],
                row,
            )
            .optional()?)
    }

    /// All open claims, oldest first.
    pub fn open_claims(&self) -> Result<Vec<Claim>> {
        let mut st = self.conn.prepare(&format!(
            "SELECT {COLS} FROM claims WHERE released_at IS NULL ORDER BY claimed_at"
        ))?;
        let v = st
            .query_map([], row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Record a claim. Caller has already run `bd update --claim` successfully. A prior
    /// released row for the same (bead, worker) is replaced (re-claim after release).
    pub fn record_claim(
        &self,
        bead: &str,
        worker: &str,
        declared_files: &[String],
        at: &str,
    ) -> Result<()> {
        let files = serde_json::to_string(declared_files)?;
        self.conn.execute(
            "INSERT INTO claims (bead, worker, claimed_at, declared_files) VALUES (?1,?2,?3,?4) \
             ON CONFLICT(bead, worker) DO UPDATE SET claimed_at=excluded.claimed_at, \
             declared_files=excluded.declared_files, first_handover_at=NULL, \
             last_handover_at=NULL, handover_attempts=0, released_at=NULL, release_reason=NULL",
            params![bead, worker, at, files],
        )?;
        Ok(())
    }

    /// Stamp a hand-over attempt on the worker's open claim. No-op without a claim.
    pub fn stamp_handover(&self, bead: &str, worker: &str, at: &str) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE claims SET first_handover_at=COALESCE(first_handover_at, ?3), \
             last_handover_at=?3, handover_attempts=handover_attempts+1 \
             WHERE bead=?1 AND worker=?2 AND released_at IS NULL",
            params![bead, worker, at],
        )?;
        Ok(n > 0)
    }

    /// Close the worker's open claim with a reason. Returns false when there was none.
    pub fn release_claim(&self, bead: &str, worker: &str, reason: &str, at: &str) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE claims SET released_at=?3, release_reason=?4 \
             WHERE bead=?1 AND worker=?2 AND released_at IS NULL",
            params![bead, worker, at, reason],
        )?;
        Ok(n > 0)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn claim_stamp_release_round_trip() {
        let l = Ledger::open_in_memory().unwrap();
        l.record_claim("fd-1", "w1", &["a.rs".into()], "t0")
            .unwrap();
        let c = l.open_claim("fd-1").unwrap().unwrap();
        assert_eq!(
            (c.worker.as_str(), c.declared_files[0].as_str()),
            ("w1", "a.rs")
        );
        assert!(l.stamp_handover("fd-1", "w1", "t1").unwrap());
        assert!(l.stamp_handover("fd-1", "w1", "t2").unwrap());
        let c = l.open_claim("fd-1").unwrap().unwrap();
        assert_eq!(c.first_handover_at.as_deref(), Some("t1"));
        assert_eq!(c.last_handover_at.as_deref(), Some("t2"));
        assert_eq!(c.handover_attempts, 2);
        assert!(!l.stamp_handover("fd-1", "w2", "t3").unwrap());
        assert!(l.release_claim("fd-1", "w1", "landed", "t4").unwrap());
        assert!(l.open_claim("fd-1").unwrap().is_none());
        assert!(!l.release_claim("fd-1", "w1", "landed", "t5").unwrap());
        // Re-claim after release resets the counters.
        l.record_claim("fd-1", "w1", &[], "t6").unwrap();
        let c = l.open_claim("fd-1").unwrap().unwrap();
        assert_eq!((c.handover_attempts, c.claimed_at.as_str()), (0, "t6"));
    }
}
