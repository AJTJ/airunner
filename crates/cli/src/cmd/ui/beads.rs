//! `/api/beads`: the board, by column (air-o6x.1, filled by air-o6x.3). bd is asked at most
//! twice every 30 seconds, `bd list --status open,in_progress,closed -n 0 --json` and
//! `bd ready --json`, through `crates/bd` so the per-process timeout and the JSON reading are
//! the ones every command uses; never `bd serve`. The ledger's claims and landings are joined
//! in, read-only. Every open bead lands in exactly one column, and `fetched_at` says how old
//! the bd answer is.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use air_bd::{Issue, WorkLedger};
use air_ledger::Ledger;
use serde::Serialize;

/// How long a bd answer is served before bd is asked again.
pub const BD_EVERY: Duration = Duration::from_secs(30);

/// The last bd answer, with the repo it came from and when it was asked; `None` inside when
/// bd failed, so a broken bd is asked again after [`BD_EVERY`] and not on every request.
static CACHE: Mutex<Option<(std::path::PathBuf, Instant, Option<Fetched>)>> = Mutex::new(None);

/// The board as of `at`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Beads {
    /// When this answer was computed (UTC, RFC 3339).
    pub at: String,
    /// When bd was last asked, or none before the first answer.
    pub fetched_at: Option<String>,
    /// Claimable now.
    pub ready: Vec<Bead>,
    /// Held and not yet declared done.
    pub in_progress: Vec<Bead>,
    /// Declared done, waiting for the landing that carries the declared head.
    pub done_waiting: Vec<Bead>,
    /// Closed in the last 24 hours, newest first.
    pub closed_recent: Vec<Bead>,
    /// Open with an open `blocks` dependency, or open and offered by nothing else.
    pub blocked: Vec<Bead>,
}

/// One bead.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Bead {
    pub id: String,
    pub title: String,
    /// bd's status: `open`, `in_progress`, `closed`.
    pub status: String,
    /// bd's type: `task`, `bug`, `feature`, `epic`, `chore`.
    #[serde(rename = "type")]
    pub kind: String,
    /// bd's priority, none when unset.
    pub priority: Option<i64>,
    pub assignee: Option<String>,
    pub labels: Vec<String>,
    /// The ids of the open beads that block it.
    pub blocked_by: Vec<String>,
    /// The ledger's open claim, if one.
    pub claim: Option<Claim>,
    /// The done declaration on the claim, if one.
    pub done: Option<Done>,
    /// The landing that put the declared head on main, if one.
    pub landed: Option<Landed>,
}

/// An open claim.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Claim {
    pub worker: String,
    /// When it was claimed (UTC, RFC 3339).
    pub since: String,
}

/// A done declaration.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Done {
    /// The head the bead was judged at.
    pub head: String,
    pub at: String,
}

/// The landing that carried the declared head.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Landed {
    pub sha: String,
    pub at: String,
}

impl Bead {
    /// A bead with its identity and nothing else filled; the shape test in `mod.rs` builds one.
    #[allow(dead_code)]
    pub fn empty(id: &str, title: &str, status: &str, kind: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            status: status.to_string(),
            kind: kind.to_string(),
            priority: None,
            assignee: None,
            labels: Vec::new(),
            blocked_by: Vec::new(),
            claim: None,
            done: None,
            landed: None,
        }
    }
}

/// What bd answered: every issue in the three statuses, and the ready ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub at: String,
    pub issues: Vec<Issue>,
    pub ready: Vec<String>,
}

fn empty(at: &str) -> Beads {
    Beads {
        at: at.to_string(),
        fetched_at: None,
        ready: Vec::new(),
        in_progress: Vec::new(),
        done_waiting: Vec::new(),
        closed_recent: Vec::new(),
        blocked: Vec::new(),
    }
}

/// The board: the empty shape without a ledger or without a `.beads/` beside it, else bd's
/// answer (at most once per [`BD_EVERY`]) joined with the ledger's claims and landings.
pub fn beads(ledger: Option<&Ledger>, at: &str) -> Beads {
    let Some(l) = ledger else {
        return empty(at);
    };
    let Some(repo) = l.dir().parent().map(Path::to_path_buf) else {
        return empty(at);
    };
    if !repo.join(".beads").is_dir() {
        return empty(at);
    }
    let Some(fetched) = fetched(&repo, at) else {
        return empty(at);
    };
    let claims = l.open_claims().unwrap_or_default();
    let landings = l.landings().unwrap_or_default();
    join(&fetched, &claims, &landings, at)
}

/// bd's answer for `repo`, from the cache while it is younger than [`BD_EVERY`].
fn fetched(repo: &Path, at: &str) -> Option<Fetched> {
    if let Ok(guard) = CACHE.lock()
        && let Some((r, when, f)) = guard.as_ref()
        && r == repo
        && when.elapsed() < BD_EVERY
    {
        return f.clone();
    }
    let bd = crate::cmd::claim::bd_for(repo);
    let f = bd
        .by_statuses(&["open", "in_progress", "closed"])
        .ok()
        .and_then(|issues| {
            let ready = bd.ready().ok()?.into_iter().map(|i| i.id).collect();
            Some(Fetched {
                at: at.to_string(),
                issues,
                ready,
            })
        });
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((repo.to_path_buf(), Instant::now(), f.clone()));
    }
    f
}

/// Pure: the columns from bd's answer and the ledger's rows. Every open bead goes to exactly
/// one column, decided in this order: a claim declared done waits for its landing; an open
/// claim or bd's `in_progress` is in progress; bd's ready list is ready; everything else open
/// is blocked, with the open `blocks` edges bd listed. A closed bead is recent when bd
/// updated it in the last 24 hours before `at`.
pub fn join(
    f: &Fetched,
    claims: &[air_ledger::claims::Claim],
    landings: &[air_ledger::landings::Landing],
    at: &str,
) -> Beads {
    let since = crate::cmd::status::day_before(at);
    let closed_ids: std::collections::HashSet<&str> = f
        .issues
        .iter()
        .filter(|i| i.status == "closed")
        .map(|i| i.id.as_str())
        .collect();
    let mut out = empty(at);
    out.fetched_at = Some(f.at.clone());
    for i in &f.issues {
        let claim = claims.iter().find(|c| c.bead == i.id);
        let landed = landings
            .iter()
            .filter(|l| l.landed())
            .find(|l| l.beads.contains(&i.id))
            .map(|l| Landed {
                sha: l.merge_commit.clone().unwrap_or_else(|| l.sha.clone()),
                at: l.finished_at.clone(),
            });
        let blocked_by: Vec<String> = i
            .dependencies
            .iter()
            .filter(|d| d.dependency_type == "blocks" && !closed_ids.contains(d.id.as_str()))
            .map(|d| d.id.clone())
            .collect();
        let bead = Bead {
            id: i.id.clone(),
            title: i.title.clone(),
            status: i.status.clone(),
            kind: i.issue_type.clone(),
            priority: Some(i.priority),
            assignee: i.assignee.clone(),
            labels: i.labels.clone(),
            blocked_by,
            claim: claim.map(|c| Claim {
                worker: c.worker.clone(),
                since: c.claimed_at.clone(),
            }),
            done: claim.and_then(|c| {
                Some(Done {
                    head: c.done_head.clone()?,
                    at: c.done_at.clone()?,
                })
            }),
            landed,
        };
        if i.status == "closed" {
            if i.updated_at.as_deref().is_some_and(|u| u >= since.as_str()) {
                out.closed_recent.push(bead);
            }
        } else if bead.done.is_some() {
            out.done_waiting.push(bead);
        } else if bead.claim.is_some() || i.status == "in_progress" {
            out.in_progress.push(bead);
        } else if f.ready.contains(&i.id) {
            out.ready.push(bead);
        } else {
            out.blocked.push(bead);
        }
    }
    let updated = |id: &str| {
        f.issues
            .iter()
            .find(|i| i.id == id)
            .and_then(|i| i.updated_at.clone())
    };
    out.closed_recent
        .sort_by_key(|b| std::cmp::Reverse(updated(&b.id)));
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use air_bd::ShownDep;

    fn issue(id: &str, status: &str, updated: &str) -> Issue {
        Issue {
            id: id.into(),
            title: format!("t {id}"),
            status: status.into(),
            priority: 2,
            issue_type: "task".into(),
            updated_at: Some(updated.into()),
            ..Issue::default()
        }
    }

    fn claim(bead: &str, worker: &str, done: Option<&str>) -> air_ledger::claims::Claim {
        air_ledger::claims::Claim {
            bead: bead.into(),
            worker: worker.into(),
            claimed_at: "2026-11-10T00:00:00Z".into(),
            declared_files: Vec::new(),
            first_handover_at: None,
            last_handover_at: None,
            handover_attempts: 0,
            released_at: None,
            release_reason: None,
            done_at: done.map(|_| "2026-11-10T00:30:00Z".into()),
            done_head: done.map(Into::into),
            done_proof: None,
        }
    }

    /// Six beads, one per column plus one open bead offered by nothing, each in exactly one
    /// column; the closed one carries its landing and the blocked one its open blocker.
    #[test]
    fn every_open_bead_lands_in_exactly_one_column() {
        let at = "2026-11-10T12:00:00Z";
        let mut blocked = issue("zz-5", "open", "2026-11-10T01:00:00Z");
        blocked.dependencies = vec![
            ShownDep {
                id: "zz-2".into(),
                status: String::new(),
                dependency_type: "blocks".into(),
            },
            ShownDep {
                id: "zz-4".into(),
                status: String::new(),
                dependency_type: "blocks".into(),
            },
            ShownDep {
                id: "zz-0".into(),
                status: String::new(),
                dependency_type: "parent-child".into(),
            },
        ];
        let f = Fetched {
            at: at.into(),
            issues: vec![
                issue("zz-1", "open", "2026-11-10T01:00:00Z"),
                issue("zz-2", "in_progress", "2026-11-10T01:00:00Z"),
                issue("zz-3", "in_progress", "2026-11-10T01:00:00Z"),
                issue("zz-4", "closed", "2026-11-10T02:00:00Z"),
                blocked,
                issue("zz-6", "open", "2026-11-10T01:00:00Z"),
                issue("zz-7", "closed", "2026-11-01T00:00:00Z"),
            ],
            ready: vec!["zz-1".into()],
        };
        let claims = vec![
            claim("zz-2", "worker-1", None),
            claim("zz-3", "worker-2", Some("abcd1234")),
        ];
        let landings = vec![air_ledger::landings::Landing {
            id: "l".into(),
            worker: "worker-3".into(),
            sha: "4444".into(),
            tip_sha: None,
            result: "landed".into(),
            failing_step: None,
            verify_run_id: None,
            attempt_no: 1,
            beads: vec!["zz-4".into()],
            open_beads: Vec::new(),
            merge_commit: Some("mmmm".into()),
            pid: None,
            started_at: at.into(),
            finished_at: "2026-11-10T02:00:00Z".into(),
            despite_inflight: Vec::new(),
            members: Vec::new(),
        }];
        let b = join(&f, &claims, &landings, at);
        let ids = |v: &[Bead]| v.iter().map(|b| b.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&b.ready), vec!["zz-1"]);
        assert_eq!(ids(&b.in_progress), vec!["zz-2"]);
        assert_eq!(ids(&b.done_waiting), vec!["zz-3"]);
        assert_eq!(
            ids(&b.closed_recent),
            vec!["zz-4"],
            "zz-7 closed too long ago"
        );
        assert_eq!(ids(&b.blocked), vec!["zz-5", "zz-6"]);
        assert_eq!(
            b.blocked[0].blocked_by,
            vec!["zz-2"],
            "zz-4 is closed, parent-child is not a block"
        );
        assert!(b.blocked[1].blocked_by.is_empty());
        assert_eq!(b.done_waiting[0].done.as_ref().unwrap().head, "abcd1234");
        assert_eq!(b.in_progress[0].claim.as_ref().unwrap().worker, "worker-1");
        assert_eq!(b.closed_recent[0].landed.as_ref().unwrap().sha, "mmmm");
        assert_eq!(b.fetched_at.as_deref(), Some(at));
        let total = b.ready.len() + b.in_progress.len() + b.done_waiting.len() + b.blocked.len();
        assert_eq!(total, 5, "every open bead once");
    }

    /// A ledger beside no `.beads/` asks bd nothing and answers the empty shape.
    #[test]
    fn a_repo_without_beads_is_not_asked() {
        let tmp = tempfile::tempdir().unwrap();
        let l = Ledger::open_in(&tmp.path().join(".air")).unwrap();
        let b = beads(Some(&l), "2026-11-10T00:00:00Z");
        assert_eq!(b, empty("2026-11-10T00:00:00Z"));
    }
}
