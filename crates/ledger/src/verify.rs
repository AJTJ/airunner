//! `verify_runs`: "commit X passed/failed check K at time T for worker W" — the primitive
//! behind evidence-gated hand-over (plan 0001 §2 row 1, §4).

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{Ledger, Result};

/// Which check ran. Kept as a closed enum so `--json` output is stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Verify,
    DocsCheck,
    Fitness,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Verify => "verify",
            Kind::DocsCheck => "docs-check",
            Kind::Fitness => "fitness",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "verify" => Some(Kind::Verify),
            "docs-check" => Some(Kind::DocsCheck),
            "fitness" => Some(Kind::Fitness),
            _ => None,
        }
    }
}

/// One recorded run. `started_at`/`finished_at` are RFC 3339 UTC strings supplied by the
/// caller (tests inject fixed values; no wall clock inside the ledger).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyRun {
    pub id: String,
    pub worker: String,
    pub sha: String,
    pub kind: Kind,
    pub exit_code: i32,
    pub trigger: String,
    pub failing_step: Option<String>,
    pub started_at: String,
    pub finished_at: String,
    pub log_path: Option<String>,
    /// The exact argv that ran (v3). `None` for rows written before v3.
    pub command: Option<String>,
    pub duration_ms: Option<i64>,
    pub output_bytes: Option<i64>,
    /// The worktree had uncommitted changes when the run was recorded: the exit describes
    /// the tree, not HEAD.
    pub dirty: bool,
}

impl VerifyRun {
    pub fn is_green(&self) -> bool {
        self.exit_code == 0
    }
}

/// A verify that has STARTED and not yet exited (air-4cr, plan 0008 item 13).
///
/// Kept in its own table rather than as a half-written `verify_runs` row on purpose: the
/// green-evidence queries (`is_green_at`, `runs_at`, `latest_run`) decide the hand-over gate
/// and must never see a row whose exit code does not exist yet. This table holds no verdict,
/// only "someone is mid-verify, since T, as pid P".
///
/// The row is deleted when the run exits. A crashed `air record` leaves one behind, which is
/// why every reader filters on the pid being alive — the ledger cannot probe pids, so
/// liveness is passed in, exactly as `leases` does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InFlight {
    /// Same ULID the finished `verify_runs` row will carry, so the two are one run.
    pub id: String,
    pub worker: String,
    pub sha: String,
    pub kind: Kind,
    pub command: String,
    pub pid: Option<i64>,
    pub started_at: String,
}

impl Ledger {
    /// Insert a run. The id is generated here (ULID) unless the caller set one.
    pub fn record_verify(&self, run: &VerifyRun) -> Result<()> {
        self.conn().execute(
            "INSERT INTO verify_runs (id, worker, sha, kind, exit_code, trigger, failing_step, \
             started_at, finished_at, log_path, command, duration_ms, output_bytes, dirty) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                run.id,
                run.worker,
                run.sha,
                run.kind.as_str(),
                run.exit_code,
                run.trigger,
                run.failing_step,
                run.started_at,
                run.finished_at,
                run.log_path,
                run.command,
                run.duration_ms,
                run.output_bytes,
                run.dirty,
            ],
        )?;
        Ok(())
    }

    /// (green, red) counts of `kind` for (`worker`, `sha`): disagreement at one sha is
    /// flakiness made visible (adopter adoption log §9, ad-jklh).
    pub fn runs_at(&self, worker: &str, sha: &str, kind: Kind) -> Result<(i64, i64)> {
        Ok(self.conn().query_row(
            "SELECT sum(exit_code = 0), sum(exit_code <> 0) FROM verify_runs \
             WHERE worker=?1 AND sha=?2 AND kind=?3",
            params![worker, sha, kind.as_str()],
            |r| {
                Ok((
                    r.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                ))
            },
        )?)
    }

    /// The most recent run of `kind` for `worker` at any sha (for "did the command change").
    pub fn latest_run_any(&self, worker: &str, kind: Kind) -> Result<Option<VerifyRun>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT id, worker, sha, kind, exit_code, trigger, failing_step, started_at, \
                 finished_at, log_path, command, duration_ms, output_bytes, dirty FROM verify_runs \
                 WHERE worker=?1 AND kind=?2 ORDER BY started_at DESC LIMIT 1",
                params![worker, kind.as_str()],
                row_to_run,
            )
            .optional()?)
    }

    /// The most recent run of `kind` for (`worker`, `sha`), if any.
    pub fn latest_run(&self, worker: &str, sha: &str, kind: Kind) -> Result<Option<VerifyRun>> {
        let row = self
            .conn()
            .query_row(
                "SELECT id, worker, sha, kind, exit_code, trigger, failing_step, started_at, \
                 finished_at, log_path, command, duration_ms, output_bytes, dirty FROM verify_runs WHERE worker=?1 AND sha=?2 AND kind=?3 \
                 ORDER BY finished_at DESC LIMIT 1",
                params![worker, sha, kind.as_str()],
                row_to_run,
            )
            .optional()?;
        Ok(row)
    }

    /// The most recent *green* run of `kind` for `worker` at any sha (for "peer is green at Y").
    pub fn latest_green(&self, worker: &str, kind: Kind) -> Result<Option<VerifyRun>> {
        let row = self
            .conn()
            .query_row(
                "SELECT id, worker, sha, kind, exit_code, trigger, failing_step, started_at, \
                 finished_at, log_path, command, duration_ms, output_bytes, dirty FROM verify_runs WHERE worker=?1 AND kind=?2 AND exit_code=0 \
                 ORDER BY finished_at DESC LIMIT 1",
                params![worker, kind.as_str()],
                row_to_run,
            )
            .optional()?;
        Ok(row)
    }

    /// Is there a green run of `kind` recorded for exactly (`worker`, `sha`)?
    pub fn is_green_at(&self, worker: &str, sha: &str, kind: Kind) -> Result<bool> {
        Ok(self
            .latest_run(worker, sha, kind)?
            .is_some_and(|r| r.is_green()))
    }

    /// Record that a verify has started. Paired with `verify_finished` on every exit path.
    pub fn verify_started(&self, f: &InFlight) -> Result<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO verify_inflight (id, worker, sha, kind, command, pid, \
             started_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                f.id,
                f.worker,
                f.sha,
                f.kind.as_str(),
                f.command,
                f.pid,
                f.started_at
            ],
        )?;
        Ok(())
    }

    /// The run with this id is over (green, red, or the command could not start).
    pub fn verify_finished(&self, id: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM verify_inflight WHERE id=?1", params![id])?;
        Ok(())
    }

    /// Every open in-flight row, oldest first. Callers decide liveness (`in_flight_pruned`).
    pub fn verifies_in_flight(&self) -> Result<Vec<InFlight>> {
        let mut st = self.conn().prepare(
            "SELECT id, worker, sha, kind, command, pid, started_at FROM verify_inflight \
             ORDER BY started_at ASC",
        )?;
        let v = st
            .query_map([], |r| {
                let kind_s: String = r.get(3)?;
                Ok(InFlight {
                    id: r.get(0)?,
                    worker: r.get(1)?,
                    sha: r.get(2)?,
                    kind: Kind::parse(&kind_s).unwrap_or(Kind::Verify),
                    command: r.get(4)?,
                    pid: r.get(5)?,
                    started_at: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// In-flight rows whose process is still alive, deleting the rest. `alive` answers "is
    /// this pid still running"; a row with no pid is kept, since nothing disproves it.
    ///
    /// This is the whole answer to "a crashed verify does not leave a permanent in-flight
    /// row": every reader prunes, so the first `air status` or `air land` after a crash
    /// clears it. No timer, no expiry window.
    pub fn in_flight_pruned(&self, alive: impl Fn(i64) -> bool) -> Result<Vec<InFlight>> {
        let mut live = Vec::new();
        for f in self.verifies_in_flight()? {
            if f.pid.is_none_or(&alive) {
                live.push(f);
            } else {
                self.verify_finished(&f.id)?;
            }
        }
        Ok(live)
    }
}

fn row_to_run(r: &rusqlite::Row<'_>) -> rusqlite::Result<VerifyRun> {
    let kind_s: String = r.get(3)?;
    let kind = Kind::parse(&kind_s).unwrap_or(Kind::Verify);
    Ok(VerifyRun {
        id: r.get(0)?,
        worker: r.get(1)?,
        sha: r.get(2)?,
        kind,
        exit_code: r.get(4)?,
        trigger: r.get(5)?,
        failing_step: r.get(6)?,
        started_at: r.get(7)?,
        finished_at: r.get(8)?,
        log_path: r.get(9)?,
        command: r.get(10)?,
        duration_ms: r.get(11)?,
        output_bytes: r.get(12)?,
        dirty: r.get::<_, i64>(13)? != 0,
    })
}

/// A fresh ULID string for row ids.
pub fn new_id() -> String {
    ulid::Ulid::new().to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use rstest::{fixture, rstest};

    use super::*;

    #[fixture]
    fn ledger() -> Ledger {
        Ledger::open_in_memory().unwrap()
    }

    fn run(worker: &str, sha: &str, exit: i32, at: &str) -> VerifyRun {
        VerifyRun {
            id: new_id(),
            worker: worker.into(),
            sha: sha.into(),
            kind: Kind::Verify,
            exit_code: exit,
            trigger: "record".into(),
            failing_step: None,
            started_at: at.into(),
            finished_at: at.into(),
            log_path: None,
            command: None,
            duration_ms: None,
            output_bytes: None,
            dirty: false,
        }
    }

    #[rstest]
    fn runs_at_counts_disagreement(ledger: Ledger) {
        ledger.record_verify(&run("w", "s1", 0, "t1")).unwrap();
        ledger.record_verify(&run("w", "s1", 1, "t2")).unwrap();
        ledger.record_verify(&run("w", "s1", 0, "t3")).unwrap();
        assert_eq!(ledger.runs_at("w", "s1", Kind::Verify).unwrap(), (2, 1));
        assert_eq!(ledger.runs_at("w", "none", Kind::Verify).unwrap(), (0, 0));
    }

    #[rstest]
    fn green_at_requires_exact_sha(ledger: Ledger) {
        ledger
            .record_verify(&run("w1", "aaa", 0, "2026-08-18T10:00:00Z"))
            .unwrap();
        assert!(ledger.is_green_at("w1", "aaa", Kind::Verify).unwrap());
        assert!(!ledger.is_green_at("w1", "bbb", Kind::Verify).unwrap());
        assert!(!ledger.is_green_at("w2", "aaa", Kind::Verify).unwrap());
    }

    #[rstest]
    fn latest_run_wins_over_older_green(ledger: Ledger) {
        ledger
            .record_verify(&run("w1", "aaa", 0, "2026-08-18T10:00:00Z"))
            .unwrap();
        ledger
            .record_verify(&run("w1", "aaa", 1, "2026-08-18T10:05:00Z"))
            .unwrap();
        // A later red run at the same sha means "not green now".
        assert!(!ledger.is_green_at("w1", "aaa", Kind::Verify).unwrap());
    }

    #[rstest]
    fn latest_green_reports_last_good_sha(ledger: Ledger) {
        ledger
            .record_verify(&run("w1", "aaa", 0, "2026-08-18T10:00:00Z"))
            .unwrap();
        ledger
            .record_verify(&run("w1", "bbb", 1, "2026-08-18T10:05:00Z"))
            .unwrap();
        let g = ledger.latest_green("w1", Kind::Verify).unwrap().unwrap();
        assert_eq!(g.sha, "aaa");
        assert!(ledger.latest_green("w9", Kind::Verify).unwrap().is_none());
    }

    fn flight(id: &str, worker: &str, pid: Option<i64>) -> InFlight {
        InFlight {
            id: id.into(),
            worker: worker.into(),
            sha: "aaa".into(),
            kind: Kind::Verify,
            command: "make verify".into(),
            pid,
            started_at: "2026-08-29T19:00:00Z".into(),
        }
    }

    /// air-4cr: an in-flight row is visible while the run lasts, gone when it exits, and
    /// invisible to every query that decides whether a tree is green.
    #[rstest]
    fn an_in_flight_run_is_visible_and_is_not_evidence(ledger: Ledger) {
        ledger
            .verify_started(&flight("r1", "alpha", Some(1)))
            .unwrap();
        assert_eq!(ledger.verifies_in_flight().unwrap().len(), 1);
        // It is not a verdict: nothing about "alpha at aaa" has been decided yet.
        assert!(!ledger.is_green_at("alpha", "aaa", Kind::Verify).unwrap());
        assert_eq!(
            ledger.runs_at("alpha", "aaa", Kind::Verify).unwrap(),
            (0, 0)
        );
        assert!(
            ledger
                .latest_run("alpha", "aaa", Kind::Verify)
                .unwrap()
                .is_none()
        );

        ledger.record_verify(&run("alpha", "aaa", 0, "t1")).unwrap();
        ledger.verify_finished("r1").unwrap();
        assert!(ledger.verifies_in_flight().unwrap().is_empty());
        assert!(ledger.is_green_at("alpha", "aaa", Kind::Verify).unwrap());
    }

    /// air-4cr: a crashed `air record` leaves a row, and the next reader clears it. The row
    /// with no pid survives, because nothing disproves it.
    #[rstest]
    fn a_dead_pid_is_pruned_by_whoever_reads_next(ledger: Ledger) {
        ledger
            .verify_started(&flight("live", "alpha", Some(1)))
            .unwrap();
        ledger
            .verify_started(&flight("dead", "beta", Some(2)))
            .unwrap();
        ledger
            .verify_started(&flight("nopid", "gamma", None))
            .unwrap();
        let live = ledger.in_flight_pruned(|pid| pid == 1).unwrap();
        assert_eq!(
            live.iter().map(|f| f.worker.as_str()).collect::<Vec<_>>(),
            vec!["alpha", "gamma"]
        );
        // The prune is a write: the dead row is gone for the next reader too.
        assert_eq!(ledger.verifies_in_flight().unwrap().len(), 2);
    }

    #[rstest]
    #[case("verify", Some(Kind::Verify))]
    #[case("docs-check", Some(Kind::DocsCheck))]
    #[case("fitness", Some(Kind::Fitness))]
    #[case("nope", None)]
    fn kind_round_trips(#[case] s: &str, #[case] k: Option<Kind>) {
        assert_eq!(Kind::parse(s), k);
        if let Some(k) = k {
            assert_eq!(k.as_str(), s);
        }
    }
}
