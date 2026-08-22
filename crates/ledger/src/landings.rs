//! Landings: what `air land` did, and what it undid (air-3pz; plan 0001 §2 row 5).
//!
//! One row per attempt on one branch, `landed` or `rewound`, with the verify run that decided
//! it and the beads it carried. A rewind is as much a fact as a landing: adopter's land.sh
//! resets main and says so (`land.sh:504-514`), and without a row the only trace is scrollback.

use rusqlite::params;
use serde::Serialize;

use crate::{Ledger, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Landing {
    pub id: String,
    /// The worker whose branch was merged.
    pub worker: String,
    /// The branch head that was merged.
    pub sha: String,
    /// Main's tip before the merge, the sha a rewind returns to.
    pub tip_sha: Option<String>,
    /// `landed` | `rewound` | `refused`.
    pub result: String,
    pub failing_step: Option<String>,
    pub verify_run_id: Option<String>,
    pub attempt_no: i64,
    /// The beads this landing carried.
    pub beads: Vec<String>,
    /// The merge commit, when one was made (absent on a refusal).
    pub merge_commit: Option<String>,
    pub started_at: String,
    pub finished_at: String,
}

impl Ledger {
    /// Record one landing attempt. `attempt_no` is derived: how many times this worker's
    /// branch has been tried before, plus one.
    pub fn record_landing(&self, l: &Landing) -> Result<()> {
        let beads = serde_json::to_string(&l.beads)?;
        self.conn.execute(
            "INSERT INTO landings (id, worker, sha, tip_sha, result, failing_step, \
             verify_run_id, attempt_no, beads, merge_commit, started_at, finished_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                l.id,
                l.worker,
                l.sha,
                l.tip_sha,
                l.result,
                l.failing_step,
                l.verify_run_id,
                l.attempt_no,
                beads,
                l.merge_commit,
                l.started_at,
                l.finished_at
            ],
        )?;
        Ok(())
    }

    /// How many landing attempts this worker's branch has had, at any sha.
    pub fn landing_attempts(&self, worker: &str) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM landings WHERE worker=?1",
            params![worker],
            |r| r.get(0),
        )?)
    }

    /// Every landing, newest first.
    pub fn landings(&self) -> Result<Vec<Landing>> {
        let mut st = self.conn.prepare(
            "SELECT id, worker, sha, tip_sha, result, failing_step, verify_run_id, attempt_no, \
             beads, merge_commit, started_at, finished_at FROM landings \
             ORDER BY finished_at DESC",
        )?;
        let v = st
            .query_map([], |r| {
                let beads: Option<String> = r.get(8)?;
                Ok(Landing {
                    id: r.get(0)?,
                    worker: r.get(1)?,
                    sha: r.get(2)?,
                    tip_sha: r.get(3)?,
                    result: r.get(4)?,
                    failing_step: r.get(5)?,
                    verify_run_id: r.get(6)?,
                    attempt_no: r.get(7)?,
                    beads: beads
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                    merge_commit: r.get(9)?,
                    started_at: r.get(10)?,
                    finished_at: r.get(11)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn row(id: &str, result: &str) -> Landing {
        Landing {
            id: id.into(),
            worker: "alpha".into(),
            sha: "aaa".into(),
            tip_sha: Some("bbb".into()),
            result: result.into(),
            failing_step: None,
            verify_run_id: Some("v1".into()),
            attempt_no: 1,
            beads: vec!["fd-1".into()],
            merge_commit: Some("ccc".into()),
            started_at: "t0".into(),
            finished_at: "t1".into(),
        }
    }

    #[test]
    fn a_rewind_is_recorded_as_plainly_as_a_landing() {
        let l = Ledger::open_in_memory().unwrap();
        assert_eq!(l.landing_attempts("alpha").unwrap(), 0);
        l.record_landing(&row("1", "rewound")).unwrap();
        let mut second = row("2", "landed");
        second.attempt_no = 2;
        second.finished_at = "t2".into();
        l.record_landing(&second).unwrap();
        assert_eq!(l.landing_attempts("alpha").unwrap(), 2);
        let all = l.landings().unwrap();
        assert_eq!(
            all.iter().map(|x| x.result.as_str()).collect::<Vec<_>>(),
            vec!["landed", "rewound"],
            "newest first"
        );
        assert_eq!(all[0].beads, vec!["fd-1".to_string()]);
    }
}
