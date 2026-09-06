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
    // Written by the reconcile until air-3eu; awaiting_review now marks the claim instead of
    // releasing it, so this only appears on rows from before that. Kept so old rows read back.
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

    /// Stamp a FAILED hand-over attempt on the worker's open claim. No-op without a claim.
    ///
    /// air-zqmi: this used to be stamped on every hand-over command the gate saw, passes
    /// included, so `handover_attempts` counted attempts rather than failures while the
    /// condition reading it says "handed over N times without green verify at HEAD". An
    /// adopter's w3 closed three beads cleanly and the channel reported it had handed over
    /// without a green; the coordinator asked for the refusal text and there was none, because
    /// there had been no refusal.
    ///
    /// Same class as air-eiv, one layer over: that was a QUERY counting as an attempt
    /// (`air handover`, the documented diagnostic, raising the alarm on the worker running
    /// it), this is a SUCCESS counting as one. The rule both settle on: only a hand-over that
    /// did not go through is an attempt.
    pub fn stamp_handover(&self, bead: &str, worker: &str, at: &str) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE claims SET first_handover_at=COALESCE(first_handover_at, ?3), \
             last_handover_at=?3, handover_attempts=handover_attempts+1 \
             WHERE bead=?1 AND worker=?2 AND released_at IS NULL",
            params![bead, worker, at],
        )?;
        Ok(n > 0)
    }

    /// A hand-over went through, so there are no outstanding failed attempts on this claim
    /// (air-zqmi). Without this the counter is a high-water mark: one refusal early on, then a
    /// clean close, and the condition still reports a worker that has already succeeded.
    ///
    /// Keeps `first_handover_at`, which records when the worker first tried and is history
    /// rather than state.
    pub fn clear_handover_attempts(&self, bead: &str, worker: &str) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE claims SET handover_attempts=0 \
             WHERE bead=?1 AND worker=?2 AND released_at IS NULL",
            params![bead, worker],
        )?;
        Ok(n > 0)
    }

    /// Mark the worker's open claim handed over, without counting a hand-over attempt: the
    /// status reconcile learned from bd that the bead reached `awaiting_review`, which is not
    /// the worker running `air handover`. Idempotent — a later tick does not move the time,
    /// and the claim stays open because a handed-over bead is still the worker's until it
    /// lands (air-3eu).
    pub fn mark_handed_over(&self, bead: &str, worker: &str, at: &str) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE claims SET first_handover_at=?3, \
             last_handover_at=COALESCE(last_handover_at, ?3) \
             WHERE bead=?1 AND worker=?2 AND released_at IS NULL AND first_handover_at IS NULL",
            params![bead, worker, at],
        )?;
        Ok(n > 0)
    }

    /// Close every open claim on these beads with one reason, in ONE transaction, whoever
    /// holds them. The coordinator's landing pass releases N claims here (air-869); doing it
    /// one `air release` at a time cost N ledger opens and N bd processes.
    /// Returns the (bead, worker) pairs actually released, in the order given.
    pub fn release_claims_on(
        &self,
        beads: &[String],
        reason: &str,
        at: &str,
    ) -> Result<Vec<(String, String)>> {
        let tx = self.conn.unchecked_transaction()?;
        let mut released = Vec::new();
        {
            let mut find = tx.prepare(
                "SELECT worker FROM claims WHERE bead=?1 AND released_at IS NULL ORDER BY worker",
            )?;
            let mut close = tx.prepare(
                "UPDATE claims SET released_at=?3, release_reason=?4 \
                 WHERE bead=?1 AND worker=?2 AND released_at IS NULL",
            )?;
            for bead in beads {
                let workers: Vec<String> = find
                    .query_map(params![bead], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                for w in workers {
                    if close.execute(params![bead, w, at, reason])? > 0 {
                        released.push((bead.clone(), w));
                    }
                }
            }
        }
        tx.commit()?;
        Ok(released)
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
        l.record_claim("zz-1", "w1", &["a.rs".into()], "t0")
            .unwrap();
        let c = l.open_claim("zz-1").unwrap().unwrap();
        assert_eq!(
            (c.worker.as_str(), c.declared_files[0].as_str()),
            ("w1", "a.rs")
        );
        assert!(l.stamp_handover("zz-1", "w1", "t1").unwrap());
        assert!(l.stamp_handover("zz-1", "w1", "t2").unwrap());
        let c = l.open_claim("zz-1").unwrap().unwrap();
        assert_eq!(c.first_handover_at.as_deref(), Some("t1"));
        assert_eq!(c.last_handover_at.as_deref(), Some("t2"));
        assert_eq!(c.handover_attempts, 2);
        assert!(!l.stamp_handover("zz-1", "w2", "t3").unwrap());
        assert!(l.release_claim("zz-1", "w1", "landed", "t4").unwrap());
        assert!(l.open_claim("zz-1").unwrap().is_none());
        assert!(!l.release_claim("zz-1", "w1", "landed", "t5").unwrap());
        // Re-claim after release resets the counters.
        l.record_claim("zz-1", "w1", &[], "t6").unwrap();
        let c = l.open_claim("zz-1").unwrap().unwrap();
        assert_eq!((c.handover_attempts, c.claimed_at.as_str()), (0, "t6"));
    }

    /// air-3eu: the status reconcile marks, it does not release, and a second tick is a no-op.
    #[test]
    fn mark_handed_over_is_idempotent_and_leaves_the_claim_open() {
        let l = Ledger::open_in_memory().unwrap();
        l.record_claim("zz-2", "w1", &["a.rs".into()], "t0")
            .unwrap();
        assert!(l.mark_handed_over("zz-2", "w1", "t1").unwrap());
        assert!(!l.mark_handed_over("zz-2", "w1", "t2").unwrap());
        let c = l.open_claim("zz-2").unwrap().unwrap();
        assert_eq!(c.first_handover_at.as_deref(), Some("t1"));
        assert_eq!(c.last_handover_at.as_deref(), Some("t1"));
        assert_eq!(
            (c.handover_attempts, c.claimed_at.as_str(), c.released_at),
            (0, "t0", None),
            "no attempt counted, claim still open, original time kept"
        );
        // A worker's own hand-over still counts as an attempt on top of the mark.
        assert!(l.stamp_handover("zz-2", "w1", "t3").unwrap());
        let c = l.open_claim("zz-2").unwrap().unwrap();
        assert_eq!(
            (c.handover_attempts, c.first_handover_at.as_deref()),
            (1, Some("t1"))
        );
    }

    /// air-869: the landing pass releases every claim in one transaction, and says which
    /// beads had no open claim (they are absent from the returned list, not an error).
    #[test]
    fn release_claims_on_closes_many_in_one_transaction() {
        let l = Ledger::open_in_memory().unwrap();
        for (bead, worker) in [("zz-1", "alpha"), ("zz-2", "beta"), ("zz-3", "alpha")] {
            l.record_claim(bead, worker, &[], "t0").unwrap();
        }
        let beads: Vec<String> = ["zz-1", "zz-2", "zz-9"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let out = l.release_claims_on(&beads, "landed", "t1").unwrap();
        assert_eq!(
            out,
            vec![
                ("zz-1".to_string(), "alpha".to_string()),
                ("zz-2".to_string(), "beta".to_string()),
            ]
        );
        assert!(l.open_claim("zz-1").unwrap().is_none());
        assert!(l.open_claim("zz-3").unwrap().is_some(), "untouched");
        // Idempotent: a second pass releases nothing.
        assert!(
            l.release_claims_on(&beads, "landed", "t2")
                .unwrap()
                .is_empty()
        );
    }
}
