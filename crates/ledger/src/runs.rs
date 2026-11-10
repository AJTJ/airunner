//! `runs`: "session S ran command C at commit X on machine M, and it exited E" (air-opq.3).
//!
//! A run is not a verify. No green query reads this table, the close gate never looks here,
//! and nothing in it is a verdict; `verify_runs` keeps that job. What this row adds is the
//! machine: an adopter's spike tables held numbers with no record of where they were measured
//! (adopter, 2026-10-29/30). Removed with `air run`.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{Ledger, Result};

/// The machine a run happened on, as one probe command read it there before the run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Machine {
    pub hostname: String,
    /// What the target says it is running, from `AIR_IMAGE` in its environment; None when it
    /// says nothing.
    pub image: Option<String>,
    /// `uname -m`.
    pub arch: String,
    /// `nproc`, or `sysctl -n hw.ncpu`.
    pub nproc: i64,
    /// `model name` from `/proc/cpuinfo`, or `machdep.cpu.brand_string`.
    pub cpu: String,
}

/// One recorded run. Times are RFC 3339 UTC strings the caller supplies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub session: String,
    pub sha: String,
    /// The tree of `sha`; None when git could not say.
    pub tree: Option<String>,
    /// The argv that ran, joined by spaces.
    pub command: String,
    pub exit_code: i32,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: i64,
    pub output_bytes: i64,
    /// `local`, or the ssh target as `<user@host>:<path>`.
    pub target: String,
    pub machine: Machine,
    /// The paths `--pull` brought back into the worktree, relative to it (air-opq.4). Empty
    /// without `--pull`, and on every local run.
    #[serde(default)]
    pub pulled: Vec<String>,
    /// The bytes of the files under those paths, as they are in the worktree after the pull.
    #[serde(default)]
    pub pulled_bytes: i64,
}

impl Ledger {
    pub fn record_run(&self, run: &Run) -> Result<()> {
        self.conn().execute(
            "INSERT INTO runs (id, session, sha, tree, command, exit_code, started_at, \
             finished_at, duration_ms, output_bytes, target, hostname, image, arch, nproc, cpu, \
             pulled, pulled_bytes) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
            params![
                run.id,
                run.session,
                run.sha,
                run.tree,
                run.command,
                run.exit_code,
                run.started_at,
                run.finished_at,
                run.duration_ms,
                run.output_bytes,
                run.target,
                run.machine.hostname,
                run.machine.image,
                run.machine.arch,
                run.machine.nproc,
                run.machine.cpu,
                serde_json::to_string(&run.pulled).unwrap_or_else(|_| "[]".into()),
                run.pulled_bytes,
            ],
        )?;
        Ok(())
    }

    /// Every run of `session`, oldest first.
    pub fn runs_of(&self, session: &str) -> Result<Vec<Run>> {
        let mut st = self.conn().prepare(
            "SELECT id, session, sha, tree, command, exit_code, started_at, finished_at, \
             duration_ms, output_bytes, target, hostname, image, arch, nproc, cpu, pulled, \
             pulled_bytes FROM runs WHERE session = ?1 ORDER BY started_at, id",
        )?;
        let rows = st.query_map(params![session], |r| {
            Ok(Run {
                id: r.get(0)?,
                session: r.get(1)?,
                sha: r.get(2)?,
                tree: r.get(3)?,
                command: r.get(4)?,
                exit_code: r.get(5)?,
                started_at: r.get(6)?,
                finished_at: r.get(7)?,
                duration_ms: r.get(8)?,
                output_bytes: r.get(9)?,
                target: r.get(10)?,
                machine: Machine {
                    hostname: r.get(11)?,
                    image: r.get(12)?,
                    arch: r.get(13)?,
                    nproc: r.get(14)?,
                    cpu: r.get(15)?,
                },
                pulled: r
                    .get::<_, Option<String>>(16)?
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
                pulled_bytes: r.get(17)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn run(session: &str, at: &str, exit: i32) -> Run {
        Run {
            id: crate::verify::new_id(),
            session: session.into(),
            sha: "a".repeat(40),
            tree: Some("b".repeat(40)),
            command: "cargo test".into(),
            exit_code: exit,
            started_at: at.into(),
            finished_at: at.into(),
            duration_ms: 1200,
            output_bytes: 345,
            target: "me@box:~/air/p".into(),
            machine: Machine {
                hostname: "box".into(),
                image: Some("dev:1".into()),
                arch: "x86_64".into(),
                nproc: 16,
                cpu: "AMD EPYC".into(),
            },
            pulled: vec![],
            pulled_bytes: 0,
        }
    }

    #[test]
    fn insert_and_read_back_by_session_oldest_first() {
        let l = Ledger::open_in_memory().unwrap();
        let later = run("worker-1", "2026-11-03T10:00:01Z", 1);
        let earlier = run("worker-1", "2026-11-03T10:00:00Z", 0);
        let other = run("lane", "2026-11-03T09:00:00Z", 0);
        for r in [&later, &earlier, &other] {
            l.record_run(r).unwrap();
        }
        assert_eq!(l.runs_of("worker-1").unwrap(), vec![earlier, later]);
        assert_eq!(l.runs_of("lane").unwrap(), vec![other]);
        assert!(l.runs_of("worker-2").unwrap().is_empty());
    }

    #[test]
    fn pulled_paths_and_bytes_round_trip() {
        let l = Ledger::open_in_memory().unwrap();
        let mut r = run("worker-1", "2026-11-03T10:00:00Z", 0);
        r.pulled = vec!["results/a.txt".into(), "results/dir".into()];
        r.pulled_bytes = 1234;
        l.record_run(&r).unwrap();
        assert_eq!(l.runs_of("worker-1").unwrap(), vec![r]);
    }

    #[test]
    fn a_run_is_not_a_verify() {
        let l = Ledger::open_in_memory().unwrap();
        l.record_run(&run("worker-1", "2026-11-03T10:00:00Z", 0))
            .unwrap();
        let n: i64 = l
            .conn()
            .query_row("SELECT count(*) FROM verify_runs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "no green comes from a run");
    }
}
