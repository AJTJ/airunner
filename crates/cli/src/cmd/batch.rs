//! A green at a verified descendant closes the bead it covers (air-80x.1).
//!
//! The gate wanted a green AT the worker's HEAD (or, since air-7wf, at its tree). A verify
//! lane's green is at the batch head, whose tree holds everyone's changes, so neither key
//! matched and the lane's green closed nothing: adopter's 2026-08-29 round parked a quarter
//! of the fleet as a lane whose batch never formed (air-learnings-round-2026-08-29.md, item 2).
//!
//! **The check is per bead, not per HEAD** (owner, 2026-09-05). A worker keeps committing after
//! the batch is cut, so "the batch contains HEAD" would refuse a bead the batch fully covered.
//! The fact that closes a bead: every commit in `main..HEAD` whose `Bead:` trailer names it is
//! an ancestor of a verified commit C, and C contains main. Recorded by any worker: the lane
//! is not the author.
//!
//! Removal condition: never, while the gate exists; this is the gate's definition of green.

use std::path::Path;

use air_ledger::Ledger;
use air_ledger::verify::Kind;
use serde::Serialize;

use crate::git;

/// One commit in `main..HEAD` that carries the bead's trailer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BeadCommit {
    pub sha: String,
    pub subject: String,
}

/// A recorded green, as it stands to those commits. `contains` is parallel to the commits it
/// was checked against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub sha: String,
    pub worker: String,
    pub contains_main: bool,
    pub contains: Vec<bool>,
}

/// What the candidates say about one bead.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Batch {
    /// The newest green that contains main and every commit of the bead: (sha, worker).
    pub covering: Option<(String, String)>,
    /// The newest green containing main that covers some of the bead's commits but not all:
    /// (batch sha, batch worker, the newest commit it lacks, that commit's subject). The
    /// refusal names it: the batch predates the worker's last commit.
    pub predates: Option<(String, String, String, String)>,
}

/// Pure: which candidate, if any, covers the bead. Candidates are newest first, so the first
/// covering one wins and the first partial one is what a refusal names.
pub fn cover(candidates: &[Candidate], commits: &[BeadCommit]) -> Batch {
    let mut out = Batch::default();
    if commits.is_empty() {
        return out;
    }
    for c in candidates.iter().filter(|c| c.contains_main) {
        let covers_all = c.contains.len() == commits.len() && c.contains.iter().all(|x| *x);
        if covers_all {
            out.covering = Some((c.sha.clone(), c.worker.clone()));
            return out;
        }
        if out.predates.is_none()
            && c.contains.iter().any(|x| *x)
            && let Some((i, _)) = c.contains.iter().enumerate().find(|(_, x)| !**x)
            && let Some(m) = commits.get(i)
        {
            out.predates = Some((
                c.sha.clone(),
                c.worker.clone(),
                m.sha.clone(),
                m.subject.clone(),
            ));
        }
    }
    out
}

/// The commits of `bead` in `main..HEAD`, newest first, by their `Bead:` trailer only.
pub fn bead_commits(repo: &Path, bead: &str) -> Vec<BeadCommit> {
    let text =
        git::run(repo, &["log", "--format=%H%x1f%s%x1f%B%x1e", "main..HEAD"]).unwrap_or_default();
    text.split('\u{1e}')
        .filter_map(|rec| {
            let mut it = rec.trim_start_matches('\n').splitn(3, '\u{1f}');
            let sha = it.next()?.trim().to_string();
            let subject = it.next()?.to_string();
            let body = it.next().unwrap_or("");
            let names_it = super::attribution::trailer_ids(body)
                .iter()
                .any(|b| b == bead);
            (!sha.is_empty() && names_it).then_some(BeadCommit { sha, subject })
        })
        .collect()
}

/// How many recent greens are tried. A batch is one of the last few verifies in a round;
/// each candidate costs one `git merge-base` plus one per bead commit, only on the path where
/// HEAD itself has no green.
pub const CANDIDATES: usize = 12;

/// The batch verdict for one bead on this branch, from the ledger's newest greens.
pub fn for_bead(ledger: &Ledger, repo: &Path, bead: &str) -> Result<Batch, String> {
    let commits = bead_commits(repo, bead);
    if commits.is_empty() {
        return Ok(Batch::default());
    }
    let greens = ledger
        .latest_greens(Kind::Verify, CANDIDATES)
        .map_err(|e| e.to_string())?;
    let candidates: Vec<Candidate> = greens
        .into_iter()
        .map(|g| {
            let contains_main = git::is_ancestor(repo, "main", &g.sha).unwrap_or(false);
            let contains = if contains_main {
                commits
                    .iter()
                    .map(|c| git::is_ancestor(repo, &c.sha, &g.sha).unwrap_or(false))
                    .collect()
            } else {
                vec![false; commits.len()]
            };
            Candidate {
                sha: g.sha,
                worker: g.worker,
                contains_main,
                contains,
            }
        })
        .collect();
    Ok(cover(&candidates, &commits))
}

fn short(sha: &str) -> &str {
    sha.get(..8).unwrap_or(sha)
}

/// The worker branches `head` contains that `tip` (main) does not, by worktree (air-80x.2):
/// every worker worktree whose head is an ancestor of `head` and not of main, `own` (the
/// batch's own worker) excluded. Git ancestry only; nothing is read from commit messages.
/// `air record` writes it on the run and `air land` on the landing, so a red batch and a
/// landed one name their members the same way.
pub fn members_of(
    repo: &Path,
    own: &str,
    head: &str,
    tip: &str,
) -> Vec<air_ledger::landings::Member> {
    let mut out = Vec::new();
    for (path, _) in git::worktrees(repo).unwrap_or_default() {
        let worker = air_ledger::paths::worker_name_for(&path).unwrap_or_default();
        if worker == own || super::hook::role_for(&worker) != "worker" {
            continue;
        }
        let Ok(wt_head) = git::head(&path) else {
            continue;
        };
        let in_batch = git::is_ancestor(repo, &wt_head, head).unwrap_or(false);
        let in_main = git::is_ancestor(repo, &wt_head, tip).unwrap_or(false);
        if in_batch && !in_main {
            out.push(air_ledger::landings::Member {
                worker,
                sha: wt_head,
            });
        }
    }
    out
}

/// A red verify at a batch head (air-80x.4): the run, and the members it was recorded with.
/// Nothing lands, closes or claims on it; what Air adds is the report, so the lane can split
/// by hand. Auto-bisect waits for a count of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RedBatch {
    pub sha: String,
    pub worker: String,
    pub at: String,
    pub members: Vec<air_ledger::landings::Member>,
}

/// Pure: the red batches among `runs` (newest first), newest first. A run is a batch when it
/// was recorded with members; a red one with none is an ordinary red at a worker's own head
/// and is not reported here.
pub fn red_batches_of(runs: &[air_ledger::verify::VerifyRun]) -> Vec<RedBatch> {
    runs.iter()
        .filter(|r| r.verdict() == air_ledger::verify::Verdict::Red)
        .filter(|r| !r.members.is_empty())
        .map(|r| RedBatch {
            sha: r.sha.clone(),
            worker: r.worker.clone(),
            at: r.finished_at.clone(),
            members: r.members.clone(),
        })
        .collect()
}

/// The newest red batch still standing: none of its members has since been carried by a
/// newer green run (a later batch, or the worker's own verify at that head). One list, one
/// answer; the lane reads it in `air status` until a newer batch supersedes it.
pub fn red_batch_standing(ledger: &Ledger, repo: &Path) -> Option<RedBatch> {
    let runs = ledger.latest_runs(Kind::Verify, 20).ok()?;
    let red = red_batches_of(&runs).into_iter().next()?;
    let superseded = runs
        .iter()
        .filter(|r| r.is_green() && r.finished_at > red.at)
        .any(|g| {
            red.members
                .iter()
                .all(|m| git::is_ancestor(repo, &m.sha, &g.sha).unwrap_or(false))
        });
    (!superseded).then_some(red)
}

/// One line for a red batch, as `air record` prints it and `air status` repeats it.
pub fn red_batch_line(b: &RedBatch) -> String {
    let members: Vec<String> = b
        .members
        .iter()
        .map(|m| format!("{}@{}", m.worker, short(&m.sha)))
        .collect();
    format!(
        "batch red at {} ({}): members {}; nothing lands on it, the lane splits by hand",
        short(&b.sha),
        b.worker,
        members.join(", ")
    )
}

/// The two sentences the gate carries (air-80x.1): the ok line when every bead in `beads`
/// is covered by a batch green, else the refusal detail from the first bead a batch predates.
/// `(None, None)` when no bead has a batch green or a partial one.
pub fn describe(
    ledger: &Ledger,
    repo: &Path,
    beads: &[String],
) -> (Option<String>, Option<String>) {
    let mut covered: Vec<String> = Vec::new();
    let mut predates: Option<String> = None;
    for bead in beads {
        let b = for_bead(ledger, repo, bead).unwrap_or_default();
        match (b.covering, b.predates) {
            (Some((sha, worker)), _) => covered.push(format!(
                "green at {} (batch by {worker}) contains every commit of {bead}",
                short(&sha)
            )),
            (None, Some((sha, worker, missing, subject))) => {
                if predates.is_none() {
                    predates = Some(format!(
                        "the batch's green at {} (by {worker}) predates your commit {} \
                         \"{subject}\" for {bead}: it does not contain it",
                        short(&sha),
                        short(&missing)
                    ));
                }
                return (None, predates);
            }
            (None, None) => return (None, predates),
        }
    }
    if beads.is_empty() || covered.len() != beads.len() {
        return (None, predates);
    }
    (Some(covered.join("; ")), None)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn commit(sha: &str) -> BeadCommit {
        BeadCommit {
            sha: sha.into(),
            subject: format!("work {sha}"),
        }
    }

    fn cand(sha: &str, main: bool, contains: &[bool]) -> Candidate {
        Candidate {
            sha: sha.into(),
            worker: "lane".into(),
            contains_main: main,
            contains: contains.to_vec(),
        }
    }

    #[test]
    fn the_newest_green_that_contains_main_and_every_commit_covers() {
        let commits = vec![commit("c2"), commit("c1")];
        let b = cover(
            &[
                cand("late", true, &[true, true]),
                cand("early", true, &[false, true]),
            ],
            &commits,
        );
        assert_eq!(b.covering, Some(("late".into(), "lane".into())));
        assert_eq!(b.predates, None);
    }

    #[test]
    fn a_batch_cut_before_the_last_commit_predates_it_by_name() {
        let commits = vec![commit("c2"), commit("c1")];
        let b = cover(&[cand("early", true, &[false, true])], &commits);
        assert_eq!(b.covering, None);
        assert_eq!(
            b.predates,
            Some(("early".into(), "lane".into(), "c2".into(), "work c2".into()))
        );
    }

    #[test]
    fn a_green_that_lacks_main_never_covers_and_no_commits_means_no_batch() {
        let commits = vec![commit("c1")];
        assert_eq!(
            cover(&[cand("x", false, &[true])], &commits),
            Batch::default()
        );
        assert_eq!(cover(&[cand("x", true, &[true])], &[]), Batch::default());
    }
}
