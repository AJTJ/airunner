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
}

impl VerifyRun {
    pub fn is_green(&self) -> bool {
        self.exit_code == 0
    }
}

impl Ledger {
    /// Insert a run. The id is generated here (ULID) unless the caller set one.
    pub fn record_verify(&self, run: &VerifyRun) -> Result<()> {
        self.conn().execute(
            "INSERT INTO verify_runs (id, worker, sha, kind, exit_code, trigger, failing_step, \
             started_at, finished_at, log_path) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
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
            ],
        )?;
        Ok(())
    }

    /// The most recent run of `kind` for (`worker`, `sha`), if any.
    pub fn latest_run(&self, worker: &str, sha: &str, kind: Kind) -> Result<Option<VerifyRun>> {
        let row = self
            .conn()
            .query_row(
                "SELECT id, worker, sha, kind, exit_code, trigger, failing_step, started_at, \
                 finished_at, log_path FROM verify_runs WHERE worker=?1 AND sha=?2 AND kind=?3 \
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
                 finished_at, log_path FROM verify_runs WHERE worker=?1 AND kind=?2 AND exit_code=0 \
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
        }
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
