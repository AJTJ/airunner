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
    /// A worker's cheap check under a verification lane (precheck, 2026-09-25). Its own kind so that a
    /// precheck green is never read as a verify green: every green query takes the kind.
    Precheck,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Verify => "verify",
            Kind::DocsCheck => "docs-check",
            Kind::Fitness => "fitness",
            Kind::Precheck => "precheck",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "verify" => Some(Kind::Verify),
            "docs-check" => Some(Kind::DocsCheck),
            "fitness" => Some(Kind::Fitness),
            "precheck" => Some(Kind::Precheck),
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
    /// The tree id of `sha` (v13, air-7wf), so a green can be found again from a different
    /// commit over the same content: the landing commit `air land` builds is exactly that.
    /// `None` for rows written before v14, which never match a tree lookup.
    pub tree: Option<String>,
    /// The worker branch heads `sha` contained that main did not, at record time (air-80x.4,
    /// schema v18): a verify lane's batch names its members, so a red batch can be reported
    /// by member without a landing row. Empty for a verify at a worker's own head.
    pub members: Vec<crate::landings::Member>,
    /// What `main` pointed at when the run started (v19, air-9ij): the main this tree was
    /// built over. "Green G contains main" is the close gate's question, and asking it of
    /// CURRENT main makes a recorded fact expire the moment anyone writes to main — a
    /// landing, or an ordinary prose commit, which is what invalidated an adopter's whole
    /// batch on 2026-09-06. Asking it of this sha makes it durable. `None` before v19.
    pub main_sha: Option<String>,
}

/// Exit codes that mean the run was KILLED rather than that it failed (air-ppm): 128 + SIGKILL
/// and 128 + SIGTERM, which is what `make` exits with when it is the process signalled, what
/// a wrapper that declares a kill emits (the adopter's `run-logged.sh`), and what
/// `air record` itself records when its child died by that signal. Nothing in a normal verify
/// exits either. A child of make that was signalled makes make exit 2, which is
/// indistinguishable from a real failure by exit code alone; Air does not parse make's
/// "Terminated" line to find out (that is a fact taken from text somebody chose), so the
/// declared path is the wrapper's, and 143 is what it declares.
pub const KILLED_EXITS: [i32; 2] = [137, 143];

/// The SQL half of [`KILLED_EXITS`], for every query that decides green, red or flaky. A killed
/// run is no verdict: it is never green, never red, never one side of a flaky pair.
const NOT_KILLED: &str = "exit_code NOT IN (137, 143)";

/// What one run says about its sha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Green,
    Red,
    /// The process was signalled before it could decide. Not evidence either way.
    Killed,
}

impl VerifyRun {
    pub fn is_green(&self) -> bool {
        self.exit_code == 0
    }

    pub fn is_killed(&self) -> bool {
        KILLED_EXITS.contains(&self.exit_code)
    }

    pub fn verdict(&self) -> Verdict {
        if self.is_green() {
            Verdict::Green
        } else if self.is_killed() {
            Verdict::Killed
        } else {
            Verdict::Red
        }
    }
}

/// Where the green that stands for a commit was found (air-7wf).
///
/// The ledger reports the fact; whether a `Tree` green COUNTS is the caller's policy, because
/// it depends on a property of the target repo's verify that the ledger cannot see (see
/// `cmd::green` in the CLI). A commit-level verdict is always the more specific fact, so a
/// run at the commit itself, green or red, is never overridden by one at its tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "at", rename_all = "kebab-case")]
pub enum GreenAt {
    /// A green run recorded at this exact commit, by whichever worker.
    Commit(VerifyRun),
    /// No run at this commit, but a green run at another commit with the identical tree.
    Tree(VerifyRun),
}

impl GreenAt {
    pub fn run(&self) -> &VerifyRun {
        match self {
            GreenAt::Commit(r) | GreenAt::Tree(r) => r,
        }
    }
}

/// A verify that has STARTED and not yet exited (air-4cr, plan 0008 item 13).
///
/// Kept in its own table rather than as a half-written `verify_runs` row on purpose: the
/// green-evidence queries (`green_at`, `runs_at`, `latest_run_at_commit`) decide the hand-over gate
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
             started_at, finished_at, log_path, command, duration_ms, output_bytes, dirty, tree, \
             members, main_sha) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
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
                run.tree,
                serde_json::to_string(&run.members)?,
                run.main_sha,
            ],
        )?;
        Ok(())
    }

    /// (green, red) counts of `kind` at `sha`, every worker: disagreement at one sha is
    /// flakiness made visible (the adopter's adoption log §9). Counted at the commit,
    /// not the tree (air-7wf): two commits over one tree that disagree could be flakiness OR
    /// a verify that reads history, and only at the commit is the disagreement unambiguous.
    pub fn runs_at(&self, sha: &str, kind: Kind) -> Result<(i64, i64)> {
        Ok(self.conn().query_row(
            &format!(
                "SELECT sum(exit_code = 0), sum(exit_code <> 0) FROM verify_runs \
                 WHERE sha=?1 AND kind=?2 AND {NOT_KILLED}"
            ),
            params![sha, kind.as_str()],
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
                 finished_at, log_path, command, duration_ms, output_bytes, dirty, tree, members, main_sha \
                 FROM verify_runs WHERE worker=?1 AND kind=?2 ORDER BY started_at DESC LIMIT 1",
                params![worker, kind.as_str()],
                row_to_run,
            )
            .optional()?)
    }

    /// The most recent run of `kind` at `sha`, by whichever worker ran it (air-7wf).
    ///
    /// The worker used to be part of the key. It said WHERE a run happened, never WHAT was
    /// verified — a sha is its content and its history — and the only recorded cross-worktree
    /// difference (`.git` file versus directory, 2026-08-23) is a test that reads where it
    /// runs, which roles.md already rules is a defect to fix rather than a reason to verify
    /// twice. Who ran it stays on the row; it is the audit trail, not the key.
    pub fn latest_run_at_commit(&self, sha: &str, kind: Kind) -> Result<Option<VerifyRun>> {
        let row = self
            .conn()
            .query_row(
                &format!(
                    "SELECT id, worker, sha, kind, exit_code, trigger, failing_step, \
                     started_at, finished_at, log_path, command, duration_ms, output_bytes, \
                     dirty, tree, members, main_sha FROM verify_runs WHERE sha=?1 AND kind=?2 AND {NOT_KILLED} \
                     ORDER BY finished_at DESC LIMIT 1"
                ),
                params![sha, kind.as_str()],
                row_to_run,
            )
            .optional()?;
        Ok(row)
    }

    /// The most recent run of `kind` over `tree`, at any commit, by any worker. Rows from
    /// before v14 have no tree and are never returned.
    pub fn latest_run_at_tree(&self, tree: &str, kind: Kind) -> Result<Option<VerifyRun>> {
        let row = self
            .conn()
            .query_row(
                &format!(
                    "SELECT id, worker, sha, kind, exit_code, trigger, failing_step, \
                     started_at, finished_at, log_path, command, duration_ms, output_bytes, \
                     dirty, tree, members, main_sha FROM verify_runs WHERE tree=?1 AND kind=?2 AND {NOT_KILLED} \
                     ORDER BY finished_at DESC LIMIT 1"
                ),
                params![tree, kind.as_str()],
                row_to_run,
            )
            .optional()?;
        Ok(row)
    }

    /// The newest `limit` green runs of `kind` by any worker, newest first (air-80x.1): the
    /// candidates a batch green is looked for among. Killed rows are not green and never
    /// appear; the caller decides which of these contain what.
    pub fn latest_greens(&self, kind: Kind, limit: usize) -> Result<Vec<VerifyRun>> {
        let mut st = self.conn().prepare(
            "SELECT id, worker, sha, kind, exit_code, trigger, failing_step, started_at, \
             finished_at, log_path, command, duration_ms, output_bytes, dirty, tree, members, main_sha \
             FROM verify_runs WHERE kind=?1 AND exit_code=0 \
             ORDER BY finished_at DESC LIMIT ?2",
        )?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let v = st
            .query_map(params![kind.as_str(), limit], row_to_run)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// The newest `limit` runs of `kind` by any worker, any verdict, newest first (air-80x.4):
    /// what `air status` reads to find a red batch and whether a later green superseded it.
    pub fn latest_runs(&self, kind: Kind, limit: usize) -> Result<Vec<VerifyRun>> {
        let mut st = self.conn().prepare(
            "SELECT id, worker, sha, kind, exit_code, trigger, failing_step, started_at, \
             finished_at, log_path, command, duration_ms, output_bytes, dirty, tree, members, main_sha \
             FROM verify_runs WHERE kind=?1 ORDER BY finished_at DESC LIMIT ?2",
        )?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let v = st
            .query_map(params![kind.as_str(), limit], row_to_run)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Every red batch of `kind` (recorded with members, not killed), newest first. Read by
    /// the batch-ready rule, which leaves out a branch still at a head a red batch took (0.4.6
    /// trial D3).
    pub fn red_batches(&self, kind: Kind) -> Result<Vec<VerifyRun>> {
        let sql = format!(
            "SELECT id, worker, sha, kind, exit_code, trigger, failing_step, started_at, \
             finished_at, log_path, command, duration_ms, output_bytes, dirty, tree, members, \
             main_sha FROM verify_runs WHERE kind=?1 AND exit_code != 0 AND {NOT_KILLED} \
             AND members IS NOT NULL AND members != '' AND members != '[]' \
             ORDER BY finished_at DESC"
        );
        let mut st = self.conn().prepare(&sql)?;
        let v = st
            .query_map(params![kind.as_str()], row_to_run)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// The newest run of `kind` that was a BATCH — recorded with members — and went red
    /// (air-cyf). Killed runs are excluded here rather than by the caller: a signalled run is
    /// no verdict, so it must not be mistaken for a standing red.
    ///
    /// Asked of the whole table rather than of a window. `red_batch_standing` used to read the
    /// last 20 verify runs and pick the red batch out of them, so a batch that stayed red for
    /// 20 further runs silently stopped being reported and a dropped report looked exactly
    /// like a fixed one. The row's own shape answers the question; a count of recent runs
    /// never did.
    pub fn latest_red_batch(&self, kind: Kind) -> Result<Option<VerifyRun>> {
        let sql = format!(
            "SELECT id, worker, sha, kind, exit_code, trigger, failing_step, started_at, \
             finished_at, log_path, command, duration_ms, output_bytes, dirty, tree, members, \
             main_sha FROM verify_runs WHERE kind=?1 AND exit_code != 0 AND {NOT_KILLED} \
             AND members IS NOT NULL AND members != '' AND members != '[]' \
             ORDER BY finished_at DESC LIMIT 1"
        );
        let row = self
            .conn()
            .query_row(&sql, params![kind.as_str()], row_to_run)
            .optional()?;
        Ok(row)
    }

    /// Every green run of `kind` that finished strictly after `at` (air-cyf). The supersession
    /// half of the same question: whichever green carried the red batch's members, it happened
    /// after the batch, and there is no bound on how many runs that took.
    pub fn greens_since(&self, kind: Kind, at: &str) -> Result<Vec<VerifyRun>> {
        let mut st = self.conn().prepare(
            "SELECT id, worker, sha, kind, exit_code, trigger, failing_step, started_at, \
             finished_at, log_path, command, duration_ms, output_bytes, dirty, tree, members, \
             main_sha FROM verify_runs WHERE kind=?1 AND exit_code=0 AND finished_at > ?2 \
             ORDER BY finished_at DESC",
        )?;
        let v = st
            .query_map(params![kind.as_str(), at], row_to_run)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// The most recent *green* run of `kind` for `worker` at any sha (for "peer is green at Y").
    pub fn latest_green(&self, worker: &str, kind: Kind) -> Result<Option<VerifyRun>> {
        let row = self
            .conn()
            .query_row(
                "SELECT id, worker, sha, kind, exit_code, trigger, failing_step, started_at, \
                 finished_at, log_path, command, duration_ms, output_bytes, dirty, tree, members, main_sha \
                 FROM verify_runs WHERE worker=?1 AND kind=?2 AND exit_code=0 \
                 ORDER BY finished_at DESC LIMIT 1",
                params![worker, kind.as_str()],
                row_to_run,
            )
            .optional()?;
        Ok(row)
    }

    /// The green that stands for `sha`, whose tree is `tree` when the caller knows it.
    ///
    /// The commit is consulted first and its latest verdict is final: a red at the commit is
    /// "not green" whatever the tree says, and a green at the commit is `Commit`. Only a commit
    /// with NO run falls through to the tree. `None` when neither has a green.
    pub fn green_at(&self, sha: &str, tree: Option<&str>, kind: Kind) -> Result<Option<GreenAt>> {
        if let Some(run) = self.latest_run_at_commit(sha, kind)? {
            return Ok(run.is_green().then_some(GreenAt::Commit(run)));
        }
        let Some(tree) = tree else {
            return Ok(None);
        };
        Ok(self
            .latest_run_at_tree(tree, kind)?
            .filter(VerifyRun::is_green)
            .map(GreenAt::Tree))
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
        tree: r.get(14)?,
        members: r
            .get::<_, Option<String>>(15)?
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        main_sha: r.get(16)?,
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
            tree: None,
            members: vec![],
            main_sha: None,
        }
    }

    fn run_over(worker: &str, sha: &str, tree: &str, exit: i32, at: &str) -> VerifyRun {
        VerifyRun {
            tree: Some(tree.into()),
            ..run(worker, sha, exit, at)
        }
    }

    fn is_green_at(ledger: &Ledger, sha: &str) -> bool {
        ledger.green_at(sha, None, Kind::Verify).unwrap().is_some()
    }

    #[rstest]
    fn runs_at_counts_disagreement_across_workers(ledger: Ledger) {
        ledger.record_verify(&run("w", "s1", 0, "t1")).unwrap();
        ledger.record_verify(&run("w", "s1", 1, "t2")).unwrap();
        // air-7wf: a second worker's verdict at the same sha is a verdict about the same
        // commit, so it counts in the same tally.
        ledger.record_verify(&run("v", "s1", 0, "t3")).unwrap();
        assert_eq!(ledger.runs_at("s1", Kind::Verify).unwrap(), (2, 1));
        assert_eq!(ledger.runs_at("none", Kind::Verify).unwrap(), (0, 0));
    }

    /// air-7wf: the key is the sha, not (worker, sha). A green by w1 at `aaa` is a green at
    /// `aaa`; it says nothing about `bbb`.
    #[rstest]
    fn green_at_requires_exact_sha_and_any_worker_counts(ledger: Ledger) {
        ledger
            .record_verify(&run("w1", "aaa", 0, "2026-08-18T10:00:00Z"))
            .unwrap();
        assert!(is_green_at(&ledger, "aaa"));
        assert!(!is_green_at(&ledger, "bbb"));
        let g = ledger.green_at("aaa", None, Kind::Verify).unwrap().unwrap();
        assert!(matches!(&g, GreenAt::Commit(r) if r.worker == "w1"));
    }

    #[rstest]
    fn latest_run_wins_over_older_green(ledger: Ledger) {
        ledger
            .record_verify(&run("w1", "aaa", 0, "2026-08-18T10:00:00Z"))
            .unwrap();
        ledger
            .record_verify(&run("w2", "aaa", 1, "2026-08-18T10:05:00Z"))
            .unwrap();
        // A later red run at the same sha means "not green now", whoever ran it.
        assert!(!is_green_at(&ledger, "aaa"));
    }

    /// air-7wf: a landing commit is a new sha over a verified tree. With the tree known, the
    /// green is found; a tree nobody verified is not; and a verdict AT the commit, even a red
    /// one, is never overridden by the tree's.
    #[rstest]
    fn a_green_follows_the_tree_only_when_the_commit_has_no_verdict(ledger: Ledger) {
        ledger
            .record_verify(&run_over("w1", "branch", "T", 0, "t1"))
            .unwrap();
        let landing = ledger.green_at("landing", Some("T"), Kind::Verify).unwrap();
        assert!(matches!(&landing, Some(GreenAt::Tree(r)) if r.sha == "branch"));
        // No run over this tree at all.
        assert!(
            ledger
                .green_at("other", Some("U"), Kind::Verify)
                .unwrap()
                .is_none()
        );
        // The commit's own red verdict stands over the tree's green.
        ledger
            .record_verify(&run_over("w2", "landing", "T", 1, "t2"))
            .unwrap();
        assert!(
            ledger
                .green_at("landing", Some("T"), Kind::Verify)
                .unwrap()
                .is_none()
        );
        // A pre-v13 row has no tree and never matches one.
        ledger.record_verify(&run("w1", "old", 0, "t0")).unwrap();
        assert!(
            ledger
                .latest_run_at_tree("old", Kind::Verify)
                .unwrap()
                .is_none()
        );
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
        // It is not a verdict: nothing about "aaa" has been decided yet.
        assert!(!is_green_at(&ledger, "aaa"));
        assert_eq!(ledger.runs_at("aaa", Kind::Verify).unwrap(), (0, 0));
        assert!(
            ledger
                .latest_run_at_commit("aaa", Kind::Verify)
                .unwrap()
                .is_none()
        );

        ledger.record_verify(&run("alpha", "aaa", 0, "t1")).unwrap();
        ledger.verify_finished("r1").unwrap();
        assert!(ledger.verifies_in_flight().unwrap().is_empty());
        assert!(is_green_at(&ledger, "aaa"));
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

    /// air-80x.4: a run's members round-trip, and `latest_runs` returns every verdict newest
    /// first (a red batch is found there, unlike `latest_greens`).
    #[rstest]
    fn members_round_trip_and_latest_runs_carries_every_verdict(ledger: Ledger) {
        let mut batch = run("lane", "batch", 2, "t2");
        batch.members = vec![crate::landings::Member {
            worker: "alpha".into(),
            sha: "a1".into(),
        }];
        ledger.record_verify(&run("alpha", "own", 0, "t1")).unwrap();
        ledger.record_verify(&batch).unwrap();
        let runs = ledger.latest_runs(Kind::Verify, 10).unwrap();
        assert_eq!(runs.len(), 2);
        let newest = runs.first().unwrap();
        assert_eq!(newest.sha, "batch");
        assert_eq!(newest.members, batch.members);
        assert!(runs.get(1).unwrap().members.is_empty());
        // Greens only, so the red batch is not there.
        assert_eq!(ledger.latest_greens(Kind::Verify, 10).unwrap().len(), 1);
    }

    /// air-ppm: a run that exited 143 or 137 was killed, not failed. It is no verdict: not
    /// green, not the latest run at its commit or tree, and not one side of a flaky pair. A
    /// genuine exit 2 is still red, which is what makes the first safe.
    #[rstest]
    fn a_killed_run_is_no_verdict_and_an_exit_2_is_still_red(ledger: Ledger) {
        ledger
            .record_verify(&run_over("w1", "aaa", "T", 0, "t1"))
            .unwrap();
        ledger
            .record_verify(&run_over("w1", "aaa", "T", 143, "t2"))
            .unwrap();
        assert_eq!(run("w", "aaa", 143, "t").verdict(), Verdict::Killed);
        assert_eq!(run("w", "aaa", 137, "t").verdict(), Verdict::Killed);
        // The kill after the green did not turn the commit red, nor the tree.
        assert!(is_green_at(&ledger, "aaa"));
        assert!(matches!(
            ledger.green_at("bbb", Some("T"), Kind::Verify).unwrap(),
            Some(GreenAt::Tree(_))
        ));
        // Not flaky: one green, zero red.
        assert_eq!(ledger.runs_at("aaa", Kind::Verify).unwrap(), (1, 0));
        // A kill at a commit with no other run decides nothing.
        ledger.record_verify(&run("w1", "ccc", 137, "t3")).unwrap();
        assert!(
            ledger
                .latest_run_at_commit("ccc", Kind::Verify)
                .unwrap()
                .is_none()
        );
        assert!(!is_green_at(&ledger, "ccc"));
        // A genuine failure is still red, and still flaky beside a green.
        ledger.record_verify(&run("w1", "aaa", 2, "t4")).unwrap();
        assert_eq!(run("w", "aaa", 2, "t").verdict(), Verdict::Red);
        assert!(!is_green_at(&ledger, "aaa"));
        assert_eq!(ledger.runs_at("aaa", Kind::Verify).unwrap(), (1, 1));
    }

    #[rstest]
    #[case("verify", Some(Kind::Verify))]
    #[case("docs-check", Some(Kind::DocsCheck))]
    #[case("fitness", Some(Kind::Fitness))]
    #[case("precheck", Some(Kind::Precheck))]
    #[case("nope", None)]
    fn kind_round_trips(#[case] s: &str, #[case] k: Option<Kind>) {
        assert_eq!(Kind::parse(s), k);
        if let Some(k) = k {
            assert_eq!(k.as_str(), s);
        }
    }
}
