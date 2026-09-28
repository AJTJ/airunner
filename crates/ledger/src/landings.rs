//! Landings: what `air land` did, and what it undid (air-3pz; plan 0001 §2 row 5).
//!
//! One row per attempt on one branch, `landed` or `rewound`, with the verify run that decided
//! it and the beads it carried. A rewind is as much a fact as a landing: the adopter's land.sh
//! resets main and says so (`land.sh:504-514`), and without a row the only trace is scrollback.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{Ledger, Result};

/// A bead a landing merged but did not close, and the clause it could not discharge. Written
/// by `air land` until air-cizz (2026-09-28) deleted the acceptance verdicts; a landing now
/// writes none, and old rows keep theirs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenBead {
    pub bead: String,
    /// Every clause Air could not discharge, refuted or unreadable, so the record is complete.
    pub why: String,
    /// True when at least one clause is not merely unreadable but CONTRADICTED by what the
    /// merge contains — a bead naming a file the merge did not touch. That is a wrong close;
    /// "Air could not read it" is not (air-ayp).
    #[serde(default)]
    pub refuted: bool,
    /// Of `why`, only the refuted clauses (air-ppf). Empty on rows written before the field
    /// existed.
    #[serde(default)]
    pub contradicted: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Landing {
    pub id: String,
    /// The worker whose branch was merged.
    pub worker: String,
    /// The branch head that was merged.
    pub sha: String,
    /// Main's tip before the merge, the sha a rewind returns to.
    pub tip_sha: Option<String>,
    /// `in-flight` | `landed` | `landed-refuted` | `rewound` | `refused` (air-bxe).
    ///
    /// `in-flight` is written the moment the merge lands in main, before the verify that
    /// decides whether it stays. The merge commit exists for minutes with the rollback armed,
    /// and the adopter's coordinator called a land done three times inside that window, then
    /// fell back to `pgrep` — which misled them twice, because `pgrep` printing nothing makes
    /// the `ps` after it list every process they own.
    pub result: String,
    pub failing_step: Option<String>,
    pub verify_run_id: Option<String>,
    pub attempt_no: i64,
    /// The beads this ATTEMPT covered, on every result including `refused` (air-8zn, decided
    /// and kept). A refused row answers "which beads did this attempt carry", which is what
    /// `attempt_no` counts and what a coordinator reads after a refusal. It is NOT a statement
    /// that they are on main: every reader that wants landed beads selects on
    /// [`Landing::landed`], positively, never by excluding one result.
    pub beads: Vec<String>,
    /// Of those, the ones it merged but did NOT close, each with why Air could not discharge
    /// the acceptance (air-ayp). Carried here, never as a bd status: bd's blocking predicate
    /// does not consult the workflow class, so a bead parked in a custom done-class status
    /// blocks every dependent indefinitely.
    pub open_beads: Vec<OpenBead>,
    /// The merge commit, when one was made (absent on a refusal).
    pub merge_commit: Option<String>,
    /// The `air land` process that wrote the row (air-bxe). An `in-flight` row whose pid is
    /// gone is a land that was killed — the adopter lost one to a closed pipe (`air land | head`)
    /// that merged, verified and never recorded anything, leaving main green at a sha with no
    /// landing. Liveness is the reader's to probe, as it is for leases and sessions.
    pub pid: Option<i64>,
    pub started_at: String,
    /// When the row was last written. On an `in-flight` row this is the merge time, not an
    /// end: the row is deliberately not a claim that anything finished.
    pub finished_at: String,
    /// The verifies this landing was told were in flight and ran over anyway
    /// (`--despite-inflight`, air-1bm), one line each. Empty on every landing that was not an
    /// override, which is the measurement: a round of empty ones means the refusal is only
    /// ever waited out.
    pub despite_inflight: Vec<String>,
    /// The worker branch heads this landing's branch contained that were not yet in main
    /// (air-80x.2): a verify lane's batch lands once and this says which branches rode in
    /// it. Empty on a single-branch landing. A red batch's report reads the same list.
    pub members: Vec<Member>,
}

/// One branch a batch contained at landing time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub worker: String,
    pub sha: String,
}

impl Landing {
    /// Did this attempt put its beads on main? `landed` and `landed-refuted` did; `in-flight`
    /// has not yet; `refused` and `rewound` did not.
    ///
    /// air-8zn: `landed_open` used to exclude `in-flight` and treat every other result as a
    /// landing, so a `refused` row — written with the branch's full bead list when main moved
    /// under the second branch of an `air land --all` — was read as the newest word on five
    /// beads. A denylist of one misses every result it did not name; this names what counts.
    pub fn landed(&self) -> bool {
        matches!(self.result.as_str(), "landed" | "landed-refuted")
    }
}

impl Ledger {
    /// Record the members `air batch cut` merged into the lane's `head` (schema v24, 0.4.6
    /// trial D2). A second cut that leaves the same head replaces the row.
    pub fn record_batch_cut(
        &self,
        head: &str,
        lane: &str,
        main_sha: &str,
        members: &[Member],
        at: &str,
    ) -> Result<()> {
        let members = serde_json::to_string(members)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO batch_cuts (head, lane, main_sha, members, cut_at) \
             VALUES (?1,?2,?3,?4,?5)",
            params![head, lane, main_sha, members, at],
        )?;
        Ok(())
    }

    /// The members a cut by `lane` recorded at `head`, when one did.
    pub fn batch_cut_members(&self, head: &str, lane: &str) -> Result<Option<Vec<Member>>> {
        use rusqlite::OptionalExtension;
        let text: Option<String> = self
            .conn
            .query_row(
                "SELECT members FROM batch_cuts WHERE head=?1 AND lane=?2",
                params![head, lane],
                |r| r.get(0),
            )
            .optional()?;
        match text {
            Some(t) => Ok(Some(serde_json::from_str(&t)?)),
            None => Ok(None),
        }
    }

    /// Write one landing attempt. `attempt_no` is derived: how many times this worker's
    /// branch has been tried before, plus one.
    ///
    /// Keyed on the id, so the same landing is written twice (air-bxe): once as `in-flight`
    /// the moment the merge exists, and once with the outcome. Re-writing the id updates the
    /// row rather than adding one, which keeps `attempt_no` and `landing_attempts` honest.
    pub fn record_landing(&self, l: &Landing) -> Result<()> {
        let beads = serde_json::to_string(&l.beads)?;
        let open_beads = serde_json::to_string(&l.open_beads)?;
        let despite = serde_json::to_string(&l.despite_inflight)?;
        let members = serde_json::to_string(&l.members)?;
        self.conn.execute(
            "INSERT INTO landings (id, worker, sha, tip_sha, result, failing_step, \
             verify_run_id, attempt_no, beads, merge_commit, started_at, finished_at, \
             open_beads, pid, despite_inflight, members) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16) \
             ON CONFLICT(id) DO UPDATE SET result=excluded.result, \
             failing_step=excluded.failing_step, verify_run_id=excluded.verify_run_id, \
             beads=excluded.beads, merge_commit=excluded.merge_commit, \
             finished_at=excluded.finished_at, open_beads=excluded.open_beads, \
             pid=excluded.pid, despite_inflight=excluded.despite_inflight, \
             members=excluded.members",
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
                l.finished_at,
                open_beads,
                l.pid,
                despite,
                members
            ],
        )?;
        Ok(())
    }

    /// Landings that have not reported an outcome: the merge is in main and either the verify
    /// is still running or the process running it is gone (air-bxe). Newest first.
    ///
    /// This is what "is the land done" reads. Nobody greps a process list for a fact the
    /// ledger holds, and `git merge-base --is-ancestor` cannot answer it either: it says the
    /// commit is in main, which is true for the whole armed window.
    pub fn landings_in_flight(&self) -> Result<Vec<Landing>> {
        Ok(self
            .landings()?
            .into_iter()
            .filter(|l| l.result == "in-flight")
            .collect())
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
             beads, merge_commit, started_at, finished_at, open_beads, pid, despite_inflight, \
             members FROM landings ORDER BY finished_at DESC",
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
                    pid: r.get(13)?,
                    started_at: r.get(10)?,
                    finished_at: r.get(11)?,
                    open_beads: r
                        .get::<_, Option<String>>(12)?
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                    despite_inflight: r
                        .get::<_, Option<String>>(14)?
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                    members: r
                        .get::<_, Option<String>>(15)?
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
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
            despite_inflight: vec![],
            members: vec![],
            id: id.into(),
            worker: "alpha".into(),
            sha: "aaa".into(),
            tip_sha: Some("bbb".into()),
            result: result.into(),
            failing_step: None,
            verify_run_id: Some("v1".into()),
            attempt_no: 1,
            beads: vec!["zz-1".into()],
            open_beads: vec![],
            merge_commit: Some("ccc".into()),
            pid: Some(4242),
            started_at: "t0".into(),
            finished_at: "t1".into(),
        }
    }

    /// air-bxe: the row exists from the merge onward, so "is the land done" is a lookup and
    /// never a process listing. Re-writing the id updates that row rather than adding one.
    #[test]
    fn a_landing_is_in_flight_from_the_merge_until_it_reports() {
        let l = Ledger::open_in_memory().unwrap();
        let mut r = row("1", "in-flight");
        l.record_landing(&r).unwrap();
        assert_eq!(l.landings_in_flight().unwrap().len(), 1);
        assert_eq!(l.landings_in_flight().unwrap()[0].pid, Some(4242));
        assert_eq!(l.landing_attempts("alpha").unwrap(), 1);

        r.result = "landed".into();
        r.finished_at = "t9".into();
        l.record_landing(&r).unwrap();
        assert!(l.landings_in_flight().unwrap().is_empty());
        assert_eq!(
            l.landing_attempts("alpha").unwrap(),
            1,
            "reporting an outcome is not a second attempt"
        );
        assert_eq!(l.landings().unwrap()[0].result, "landed");
    }

    /// A land killed mid-verify leaves the row saying so. Under the old shape it left nothing,
    /// and the adopter ended up with main green at a sha no landing row mentioned.
    #[test]
    fn a_killed_land_leaves_a_row_that_says_in_flight() {
        let l = Ledger::open_in_memory().unwrap();
        l.record_landing(&row("1", "in-flight")).unwrap();
        // Nothing else is written: the process died.
        let stuck = l.landings_in_flight().unwrap();
        assert_eq!(stuck.len(), 1);
        assert_eq!(stuck[0].merge_commit.as_deref(), Some("ccc"));
        assert_eq!(
            stuck[0].tip_sha.as_deref(),
            Some("bbb"),
            "and where to rewind to"
        );
    }

    /// air-ayp: `air land` closes nothing, so "merged and still open" is not by itself a
    /// defect. What is carried past the print is a REFUTED clause — a bead claiming a file the
    /// merge did not touch — and it stops being reported once somebody deals with the bead. No
    /// bd status is written either way, so the bead blocks exactly what it blocked before.
    /// air-80x.2: a batch's members round-trip through the row, and a single-branch landing
    /// has none.
    #[test]
    fn a_batch_records_its_members_and_a_single_branch_has_none() {
        let l = Ledger::open_in_memory().unwrap();
        let mut r = row("1", "landed");
        r.worker = "lane".into();
        r.members = vec![
            Member {
                worker: "alpha".into(),
                sha: "a1".into(),
            },
            Member {
                worker: "beta".into(),
                sha: "b2".into(),
            },
        ];
        l.record_landing(&r).unwrap();
        assert_eq!(l.landings().unwrap()[0].members, r.members);
        let mut plain = row("2", "landed");
        plain.finished_at = "t9".into();
        l.record_landing(&plain).unwrap();
        assert!(l.landings().unwrap()[0].members.is_empty());
    }

    /// air-1bm: the runs a `--despite-inflight` landing destroyed round-trip through the row,
    /// and the in-flight write and the outcome write of one landing keep them.
    #[test]
    fn an_override_records_the_runs_it_destroyed() {
        let l = Ledger::open_in_memory().unwrap();
        let mut r = row("1", "in-flight");
        r.despite_inflight = vec!["w2 started 906s ago: make verify at 40076426 (pid 77)".into()];
        l.record_landing(&r).unwrap();
        r.result = "landed".into();
        l.record_landing(&r).unwrap();
        let rows = l.landings().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].despite_inflight, r.despite_inflight);
        // An ordinary landing carries none, which is the count the removal condition reads.
        let mut plain = row("2", "landed");
        plain.finished_at = "t9".into();
        l.record_landing(&plain).unwrap();
        assert!(l.landings().unwrap()[0].despite_inflight.is_empty());
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
        assert_eq!(all[0].beads, vec!["zz-1".to_string()]);
    }
}
