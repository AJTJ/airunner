//! `/api/lane`: the verification lane's state (air-o6x.1, filled by air-o6x.3). Everything
//! here is read from the ledger's own tables and the pure functions `air status` and
//! `air record` already use (`batch::red_split_standing`, `batch::untested_alone`,
//! `loops::measure`), so the page and the terminal cannot disagree. Nothing is written, and
//! `air status` is never run (it writes a cache row and an event line on every call).

use air_ledger::Ledger;
use air_ledger::verify::Kind;
use serde::Serialize;

use crate::cmd::batch;

/// The lane as of `at`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Lane {
    /// When this answer was computed (UTC, RFC 3339).
    pub at: String,
    /// The lane session's worker name, or none when no lane session is recorded.
    pub lane: Option<String>,
    /// The verify running now, if one is.
    pub inflight: Option<Inflight>,
    /// The cuts, newest first (the newest 20).
    pub batch_cuts: Vec<BatchCut>,
    /// Recorded verifies and prechecks, newest first (the newest 30).
    pub verify_runs: Vec<VerifyRun>,
    /// The standing red batch, if one is.
    pub red: Option<Red>,
    /// Landings that put a branch on main, newest first (the newest 30).
    pub landings: Vec<Landing>,
    /// The two loop times `air status` prints, in seconds, over the last 24 hours.
    pub loops: Loops,
}

/// A verify in flight.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Inflight {
    /// The commit being verified.
    pub sha: String,
    /// The command.
    pub command: String,
    /// When it started (UTC, RFC 3339).
    pub started_at: String,
    /// Seconds since it started.
    pub elapsed_s: u64,
}

/// One cut.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BatchCut {
    /// The batch commit on the lane's branch.
    pub head: String,
    /// The main it was cut over.
    pub main_sha: String,
    /// When (UTC, RFC 3339).
    pub cut_at: String,
    /// The members merged, each at its sha.
    pub members: Vec<Member>,
}

/// One member of a batch.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Member {
    pub worker: String,
    pub sha: String,
    /// The beads the member's worker held when the cut was made, from the ledger's claims.
    /// The cut row records worker and sha only, and reading the trailers would need git in
    /// every worktree, so this is the claim table's answer: open at `cut_at` for that worker.
    pub beads: Vec<String>,
}

/// One recorded run.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VerifyRun {
    pub sha: String,
    /// `verify`, `precheck`, `docs-check` or `fitness`.
    pub kind: String,
    /// `green`, `red` or `killed`.
    pub verdict: String,
    /// The exit code, none for a killed run.
    pub exit: Option<i32>,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: Option<i64>,
    /// The members of the batch it verified as `worker@sha`, empty for a worker's own run.
    pub members: Vec<String>,
    /// The kept output of a red, under `.air/logs/`.
    pub log_path: Option<String>,
}

/// The standing red batch.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Red {
    pub sha: String,
    /// The members as `worker@sha`.
    pub members: Vec<String>,
    /// The members no run since has verified alone (air-67q), as `worker@sha`.
    pub untested_alone: Vec<String>,
    pub log_path: Option<String>,
}

/// One landing.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Landing {
    /// The landing commit.
    pub sha: String,
    /// The branch's worker.
    pub worker: String,
    pub beads: Vec<String>,
    pub finished_at: String,
    /// Main's tip before the merge, as the row has it; empty on a row that recorded none.
    pub main_before: String,
    /// The landing commit again, as the row's `merge_commit`.
    pub main_after: String,
}

/// The loop times, in seconds; none while there is no datum.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Loops {
    pub batch_ready_to_verify_s: Option<f64>,
    pub done_to_landed_s: Option<f64>,
}

/// The lane: the empty shape without a ledger, else every field from the tables.
pub fn lane(ledger: Option<&Ledger>, at: &str) -> Lane {
    let Some(l) = ledger else {
        return Lane {
            at: at.to_string(),
            lane: None,
            inflight: None,
            batch_cuts: Vec::new(),
            verify_runs: Vec::new(),
            red: None,
            landings: Vec::new(),
            loops: Loops::default(),
        };
    };
    let repo = l.dir().parent().map(std::path::Path::to_path_buf);
    let since = crate::cmd::status::day_before(at);
    let times = crate::cmd::loops::measure(l, &since);
    Lane {
        at: at.to_string(),
        lane: lane_session(l),
        inflight: inflight(l, at),
        batch_cuts: batch_cuts(l),
        verify_runs: verify_runs(l),
        red: repo.as_deref().and_then(|r| red(l, r)),
        landings: landings(l),
        loops: Loops {
            batch_ready_to_verify_s: seconds(&times.ready_to_batch),
            done_to_landed_s: seconds(&times.done_to_landed),
        },
    }
}

/// The median of a loop's waits as seconds, none without a datum.
fn seconds(waits: &[i64]) -> Option<f64> {
    let s = crate::cmd::loops::median(waits)?;
    i32::try_from(s).ok().map(f64::from)
}

fn lane_session(l: &Ledger) -> Option<String> {
    l.conn()
        .query_row(
            "SELECT worker FROM sessions WHERE role='lane' ORDER BY changed_at DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok()
}

fn inflight(l: &Ledger, at: &str) -> Option<Inflight> {
    let f = l.verifies_in_flight().ok()?.into_iter().next()?;
    let elapsed = crate::cmd::status::seconds_between(&f.started_at, at).unwrap_or(0);
    Some(Inflight {
        sha: f.sha,
        command: f.command,
        started_at: f.started_at,
        elapsed_s: u64::try_from(elapsed).unwrap_or(0),
    })
}

/// `worker@sha`, the form `air record` and `air status` print a member in.
fn member_label(m: &air_ledger::landings::Member) -> String {
    format!("{}@{}", m.worker, m.sha.get(..8).unwrap_or(&m.sha))
}

/// Every claim, open or released, as (bead, worker, claimed_at, released_at).
fn all_claims(l: &Ledger) -> Vec<(String, String, String, Option<String>)> {
    let mut v = Vec::new();
    for c in l
        .open_claims()
        .unwrap_or_default()
        .into_iter()
        .chain(l.closed_claims().unwrap_or_default())
    {
        v.push((c.bead, c.worker, c.claimed_at, c.released_at));
    }
    v
}

fn batch_cuts(l: &Ledger) -> Vec<BatchCut> {
    let claims = all_claims(l);
    let Ok(mut st) = l.conn().prepare(
        "SELECT head, main_sha, members, cut_at FROM batch_cuts ORDER BY cut_at DESC LIMIT 20",
    ) else {
        return Vec::new();
    };
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    });
    let Ok(rows) = rows else {
        return Vec::new();
    };
    rows.filter_map(Result::ok)
        .map(|(head, main_sha, members, cut_at)| {
            let members: Vec<air_ledger::landings::Member> =
                serde_json::from_str(&members).unwrap_or_default();
            let members = members
                .into_iter()
                .map(|m| {
                    let beads = claims
                        .iter()
                        .filter(|(_, w, claimed, released)| {
                            *w == m.worker
                                && claimed.as_str() <= cut_at.as_str()
                                && released.as_deref().is_none_or(|r| r > cut_at.as_str())
                        })
                        .map(|(b, ..)| b.clone())
                        .collect();
                    Member {
                        worker: m.worker,
                        sha: m.sha,
                        beads,
                    }
                })
                .collect();
            BatchCut {
                head,
                main_sha,
                cut_at,
                members,
            }
        })
        .collect()
}

fn verify_runs(l: &Ledger) -> Vec<VerifyRun> {
    let Ok(mut st) = l.conn().prepare(
        "SELECT sha, kind, exit_code, started_at, finished_at, duration_ms, members, log_path \
         FROM verify_runs ORDER BY finished_at DESC LIMIT 30",
    ) else {
        return Vec::new();
    };
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i32>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, Option<i64>>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<String>>(7)?,
        ))
    });
    let Ok(rows) = rows else {
        return Vec::new();
    };
    rows.filter_map(Result::ok)
        .map(
            |(sha, kind, exit_code, started_at, finished_at, duration_ms, members, log_path)| {
                let killed = air_ledger::verify::KILLED_EXITS.contains(&exit_code);
                let verdict = if exit_code == 0 {
                    "green"
                } else if killed {
                    "killed"
                } else {
                    "red"
                };
                let members: Vec<air_ledger::landings::Member> = members
                    .and_then(|m| serde_json::from_str(&m).ok())
                    .unwrap_or_default();
                VerifyRun {
                    sha,
                    kind,
                    verdict: verdict.to_string(),
                    exit: (!killed).then_some(exit_code),
                    started_at,
                    finished_at,
                    duration_ms,
                    members: members.iter().map(member_label).collect(),
                    log_path,
                }
            },
        )
        .collect()
}

/// The standing red: the split in progress when a batch of more than one stands (air-67q),
/// else the newest standing red batch, with the members no run since has verified alone.
fn red(l: &Ledger, repo: &std::path::Path) -> Option<Red> {
    if let Some(split) = batch::red_split_standing(l, repo) {
        return Some(Red {
            sha: split.batch.sha.clone(),
            members: split.batch.members.iter().map(member_label).collect(),
            untested_alone: split.untested.iter().map(member_label).collect(),
            log_path: split.batch.log_path,
        });
    }
    let b = batch::red_batch_standing(l, repo)?;
    let since = l.runs_since(Kind::Verify, &b.at).unwrap_or_default();
    let contains = |g: &str, m: &str| crate::git::is_ancestor(repo, m, g).unwrap_or(false);
    let untested = batch::untested_alone(&b, &since, &contains);
    Some(Red {
        sha: b.sha.clone(),
        members: b.members.iter().map(member_label).collect(),
        untested_alone: untested.iter().map(member_label).collect(),
        log_path: b.log_path,
    })
}

fn landings(l: &Ledger) -> Vec<Landing> {
    l.landings()
        .unwrap_or_default()
        .into_iter()
        .filter(air_ledger::landings::Landing::landed)
        .take(30)
        .map(|x| Landing {
            sha: x.merge_commit.clone().unwrap_or_else(|| x.sha.clone()),
            worker: x.worker,
            beads: x.beads,
            finished_at: x.finished_at,
            main_before: x.tip_sha.unwrap_or_default(),
            main_after: x.merge_commit.unwrap_or(x.sha),
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use air_ledger::landings::Member as M;
    use air_ledger::verify::{InFlight, VerifyRun as Run};

    fn run(id: &str, sha: &str, exit: i32, members: Vec<M>, at: &str) -> Run {
        Run {
            id: id.into(),
            worker: "lane".into(),
            sha: sha.into(),
            kind: Kind::Verify,
            exit_code: exit,
            trigger: "record".into(),
            failing_step: None,
            started_at: at.into(),
            finished_at: at.into(),
            log_path: (exit != 0).then(|| format!("/logs/{id}.log")),
            command: Some("make verify".into()),
            duration_ms: Some(1000),
            output_bytes: None,
            dirty: false,
            tree: None,
            members,
            main_sha: None,
        }
    }

    fn m(w: &str, sha: &str) -> M {
        M {
            worker: w.into(),
            sha: sha.into(),
        }
    }

    /// A seeded ledger: one inflight, one red batch of three with one member since verified
    /// alone, one landing, one cut, a lane session and a claim open at the cut. Read back
    /// through a read-only open, as the server does.
    #[test]
    fn the_lane_is_read_from_the_tables_and_the_red_lists_the_two_untested_members() {
        let tmp = tempfile::tempdir().unwrap();
        let air = tmp.path().join(".air");
        let w = Ledger::open_in(&air).unwrap();
        w.conn()
            .execute(
                "INSERT INTO sessions (session_id, worker, state, changed_at, started_at, role) \
                 VALUES ('s1', 'lane', 'idle', 't0', 't0', 'lane')",
                [],
            )
            .unwrap();
        w.record_claim("zz-1", "worker-1", &[], "2026-11-10T00:00:00Z")
            .unwrap();
        w.record_batch_cut(
            "cccccccc1111",
            "lane",
            "mmmmmmmm",
            &[m("worker-1", "aaaaaaaa1111"), m("worker-2", "bbbbbbbb2222")],
            "2026-11-10T00:10:00Z",
        )
        .unwrap();
        let three = vec![
            m("worker-1", "aaaaaaaa1111"),
            m("worker-2", "bbbbbbbb2222"),
            m("worker-3", "dddddddd3333"),
        ];
        w.record_verify(&run("r1", "cccccccc1111", 1, three, "2026-11-10T00:12:00Z"))
            .unwrap();
        // worker-2 verified alone since, red; the other two are still untested.
        w.record_verify(&run(
            "r2",
            "bbbbbbbb2222",
            1,
            vec![m("worker-2", "bbbbbbbb2222")],
            "2026-11-10T00:14:00Z",
        ))
        .unwrap();
        w.verify_started(&InFlight {
            id: "f1".into(),
            worker: "lane".into(),
            sha: "eeeeeeee".into(),
            kind: Kind::Verify,
            command: "make verify".into(),
            pid: None,
            started_at: "2026-11-10T00:20:00Z".into(),
        })
        .unwrap();
        w.record_landing(&air_ledger::landings::Landing {
            id: "l1".into(),
            worker: "worker-9".into(),
            sha: "99999999".into(),
            tip_sha: Some("mmmmmmmm".into()),
            result: "landed".into(),
            failing_step: None,
            verify_run_id: None,
            attempt_no: 1,
            beads: vec!["zz-9".into()],
            open_beads: Vec::new(),
            merge_commit: Some("ffffffff".into()),
            pid: None,
            started_at: "2026-11-10T00:05:00Z".into(),
            finished_at: "2026-11-10T00:06:00Z".into(),
            despite_inflight: Vec::new(),
            members: Vec::new(),
        })
        .unwrap();
        drop(w);

        let r = Ledger::open_read_only(&air).unwrap();
        let l = lane(Some(&r), "2026-11-10T00:21:30Z");
        assert_eq!(l.lane.as_deref(), Some("lane"));
        let f = l.inflight.unwrap();
        assert_eq!((f.sha.as_str(), f.elapsed_s), ("eeeeeeee", 90));
        assert_eq!(l.batch_cuts.len(), 1);
        assert_eq!(l.batch_cuts[0].members[0].beads, vec!["zz-1".to_string()]);
        assert!(l.batch_cuts[0].members[1].beads.is_empty());
        assert_eq!(l.verify_runs.len(), 2);
        assert_eq!(l.verify_runs[0].verdict, "red");
        assert_eq!(l.verify_runs[0].members, vec!["worker-2@bbbbbbbb"]);
        let red = l.red.unwrap();
        assert_eq!(red.sha, "cccccccc1111");
        assert_eq!(red.members.len(), 3);
        assert_eq!(
            red.untested_alone,
            vec![
                "worker-1@aaaaaaaa".to_string(),
                "worker-3@dddddddd".to_string()
            ]
        );
        assert_eq!(red.log_path.as_deref(), Some("/logs/r1.log"));
        assert_eq!(l.landings.len(), 1);
        assert_eq!(l.landings[0].sha, "ffffffff");
        assert_eq!(l.landings[0].main_before, "mmmmmmmm");
        assert_eq!(l.landings[0].beads, vec!["zz-9".to_string()]);
        assert_eq!(l.loops, Loops::default());
    }

    #[test]
    fn a_killed_run_has_no_exit_and_a_green_one_is_green() {
        let tmp = tempfile::tempdir().unwrap();
        let air = tmp.path().join(".air");
        let w = Ledger::open_in(&air).unwrap();
        w.record_verify(&run("g", "11111111", 0, vec![], "2026-11-10T00:00:00Z"))
            .unwrap();
        let killed = air_ledger::verify::KILLED_EXITS[0];
        w.record_verify(&run(
            "k",
            "22222222",
            killed,
            vec![],
            "2026-11-10T00:01:00Z",
        ))
        .unwrap();
        let l = lane(Some(&w), "2026-11-10T00:02:00Z");
        assert_eq!(l.verify_runs[0].verdict, "killed");
        assert_eq!(l.verify_runs[0].exit, None);
        assert_eq!(l.verify_runs[1].verdict, "green");
        assert_eq!(l.verify_runs[1].exit, Some(0));
        assert!(l.red.is_none());
    }
}
