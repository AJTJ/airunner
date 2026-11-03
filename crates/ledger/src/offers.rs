//! `offers` (air-m8u): "the coordinator offered its head S to the lane at T".
//!
//! The coordinator's branch carries no bead, so nothing on it says "this is done". Under
//! today's rule every commit main lacks is batch-ready, and at an adopter each coordinator
//! commit cost the lane a cut, a verify and two notices while a note was still being written
//! (adopter, 2026-10-28). With `"coordinator_autobatch"` absent or false, a head is batch-ready
//! only once a row here names it; `air ready` writes the row. A fact (was this head offered),
//! never a judgement. Removed with the lane, or when the coordinator stops committing to its
//! own branch.

use rusqlite::params;

use crate::{Ledger, Result};

impl Ledger {
    /// Record that `worker` offered `sha`. Offering the same head again keeps one row.
    pub fn offer(&self, worker: &str, sha: &str, at: &str) -> Result<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO offers (worker, sha, at) VALUES (?1, ?2, ?3)",
            params![worker, sha, at],
        )?;
        Ok(())
    }

    /// Whether `worker` has offered exactly `sha`. A later commit is a new head and needs its
    /// own row.
    pub fn offered(&self, worker: &str, sha: &str) -> Result<bool> {
        let n: i64 = self.conn().query_row(
            "SELECT count(*) FROM offers WHERE worker = ?1 AND sha = ?2",
            params![worker, sha],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::Ledger;

    #[test]
    fn an_offer_names_one_head_and_a_new_head_needs_its_own() {
        let l = Ledger::open_in_memory().unwrap();
        assert!(!l.offered("coordinator", "aaaa").unwrap());
        l.offer("coordinator", "aaaa", "2026-11-03T10:00:00Z")
            .unwrap();
        l.offer("coordinator", "aaaa", "2026-11-03T10:00:01Z")
            .unwrap();
        assert!(l.offered("coordinator", "aaaa").unwrap());
        assert!(!l.offered("coordinator", "bbbb").unwrap(), "a later commit");
        assert!(
            !l.offered("worker-1", "aaaa").unwrap(),
            "another worker's head"
        );
        let rows: i64 = l
            .conn()
            .query_row("SELECT count(*) FROM offers", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "offering twice keeps one row");
    }
}
