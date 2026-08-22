//! Landings: what `air land` did, and what it undid (air-3pz; plan 0001 §2 row 5).
//!
//! One row per attempt on one branch, `landed` or `rewound`, with the verify run that decided
//! it and the beads it carried. A rewind is as much a fact as a landing: adopter's land.sh
//! resets main and says so (`land.sh:504-514`), and without a row the only trace is scrollback.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{Ledger, Result};

/// A bead a landing merged but did not close, still open, with everything a reader needs to
/// act: `air status` renders it and the `landed-not-closed` condition names it (air-ayp).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LandedOpen {
    pub bead: String,
    /// The worker whose branch carried it.
    pub worker: String,
    pub merge_commit: String,
    /// The acceptance clause Air could not point at evidence for.
    pub why: String,
    pub landed_at: String,
}

/// A bead a landing merged but did not close, and the clause it could not discharge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenBead {
    pub bead: String,
    pub why: String,
    /// True when at least one clause is not merely unreadable but CONTRADICTED by what the
    /// merge contains — a bead naming a file the merge did not touch. That is a wrong close;
    /// "Air could not read it" is not (air-ayp).
    #[serde(default)]
    pub refuted: bool,
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
    /// `landed` | `rewound` | `refused`.
    pub result: String,
    pub failing_step: Option<String>,
    pub verify_run_id: Option<String>,
    pub attempt_no: i64,
    /// The beads this landing carried.
    pub beads: Vec<String>,
    /// Of those, the ones it merged but did NOT close, each with why Air could not discharge
    /// the acceptance (air-ayp). Carried here, never as a bd status: bd's blocking predicate
    /// does not consult the workflow class, so a bead parked in a custom done-class status
    /// blocks every dependent indefinitely.
    pub open_beads: Vec<OpenBead>,
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
        let open_beads = serde_json::to_string(&l.open_beads)?;
        self.conn.execute(
            "INSERT INTO landings (id, worker, sha, tip_sha, result, failing_step, \
             verify_run_id, attempt_no, beads, merge_commit, started_at, finished_at, \
             open_beads) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
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
                open_beads
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

    /// Beads that landed carrying an acceptance clause the merge CONTRADICTS, and that nobody
    /// has dealt with since (air-ayp).
    ///
    /// The worker closes its own bead with proof before the branch lands, so `air land` closes
    /// nothing and "merged but not closed" is not by itself a defect. What is worth carrying
    /// past the print is a REFUTED clause: a bead claiming a file the merge did not touch.
    ///
    /// "Dealt with since" is the claim row being released for any reason — the status reconcile
    /// releases it as `closed` once bd says so (air-3eu), `air close` as `landed`. That keeps
    /// the whole answer inside the ledger: no bd call, so `air status` can ask on every tick.
    /// Newest first.
    ///
    /// Nothing here writes a bd status. A bead named by one of these rows is exactly as open
    /// in bd as it was before the merge, so the merge did not change what it blocks.
    pub fn landed_open(&self) -> Result<Vec<LandedOpen>> {
        let mut out: Vec<LandedOpen> = Vec::new();
        for l in self.landings()? {
            for ob in l.open_beads.iter().filter(|o| o.refuted) {
                let dealt_with: bool = self
                    .conn
                    .query_row(
                        "SELECT count(*) FROM claims WHERE bead=?1 AND released_at IS NOT NULL",
                        params![ob.bead],
                        |r| r.get::<_, i64>(0),
                    )
                    .unwrap_or(0)
                    > 0;
                if dealt_with || out.iter().any(|o| o.bead == ob.bead) {
                    continue;
                }
                out.push(LandedOpen {
                    bead: ob.bead.clone(),
                    worker: l.worker.clone(),
                    merge_commit: l.merge_commit.clone().unwrap_or_default(),
                    why: ob.why.clone(),
                    landed_at: l.finished_at.clone(),
                });
            }
        }
        Ok(out)
    }

    /// Every landing, newest first.
    pub fn landings(&self) -> Result<Vec<Landing>> {
        let mut st = self.conn.prepare(
            "SELECT id, worker, sha, tip_sha, result, failing_step, verify_run_id, attempt_no, \
             beads, merge_commit, started_at, finished_at, open_beads FROM landings \
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
                    open_beads: r
                        .get::<_, Option<String>>(12)?
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
            id: id.into(),
            worker: "alpha".into(),
            sha: "aaa".into(),
            tip_sha: Some("bbb".into()),
            result: result.into(),
            failing_step: None,
            verify_run_id: Some("v1".into()),
            attempt_no: 1,
            beads: vec!["fd-1".into()],
            open_beads: vec![],
            merge_commit: Some("ccc".into()),
            started_at: "t0".into(),
            finished_at: "t1".into(),
        }
    }

    /// air-ayp: `air land` closes nothing, so "merged and still open" is not by itself a
    /// defect. What is carried past the print is a REFUTED clause — a bead claiming a file the
    /// merge did not touch — and it stops being reported once somebody deals with the bead. No
    /// bd status is written either way, so the bead blocks exactly what it blocked before.
    #[test]
    fn only_a_refuted_clause_is_reported_and_only_until_the_bead_is_dealt_with() {
        let l = Ledger::open_in_memory().unwrap();
        let mut r = row("1", "landed-refuted");
        r.beads = vec!["fd-1".into(), "fd-2".into(), "fd-3".into()];
        r.open_beads = vec![
            // Air could not read this one. Not a wrong close, so not reported.
            OpenBead {
                bead: "fd-2".into(),
                why: "\"the owner rules on X\": nothing Air can look up".into(),
                refuted: false,
            },
            // This one the merge contradicts.
            OpenBead {
                bead: "fd-3".into(),
                why: "\"docs/absent.md says it\": the merge did not change docs/absent.md".into(),
                refuted: true,
            },
        ];
        l.record_landing(&r).unwrap();

        let open = l.landed_open().unwrap();
        assert_eq!(
            open.iter().map(|o| o.bead.as_str()).collect::<Vec<_>>(),
            vec!["fd-3"],
            "unreadable is not the same signal as contradicted"
        );
        assert_eq!(
            (open[0].worker.as_str(), open[0].merge_commit.as_str()),
            ("alpha", "ccc")
        );
        assert!(open[0].why.contains("docs/absent.md"));
        // It round-trips through the row, so the reason survives a restart.
        assert_eq!(l.landings().unwrap()[0].open_beads, r.open_beads);

        // An open claim is not "dealt with".
        l.record_claim("fd-3", "alpha", &[], "t0").unwrap();
        assert_eq!(l.landed_open().unwrap().len(), 1);

        // Any release is: the status reconcile releases as `closed` once bd says so
        // (air-3eu), `air close` as `landed`. Either way somebody looked.
        l.release_claim("fd-3", "alpha", "closed", "t2").unwrap();
        assert!(l.landed_open().unwrap().is_empty());
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
