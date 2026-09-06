//! A green at a verified descendant closes the bead it covers (air-80x.1).
//!
//! The gate wanted a green AT the worker's HEAD (or, since air-7wf, at its tree). A verify
//! lane's green is at the batch head, whose tree holds everyone's changes, so neither key
//! matched and the lane's green closed nothing: adopter's 2026-08-29 round parked a quarter
//! of the fleet as a lane whose batch never formed (air-learnings-round-2026-08-29.md, item 2).
//!
//! **The check is per bead, not per HEAD** (owner, 2026-09-05). A worker keeps committing after
//! the batch is cut, so "the batch contains HEAD" would refuse a bead the batch fully covered.
//! The fact that closes a bead: every commit in [`BRANCH_RANGE`] whose `Bead:` trailer names it
//! is an ancestor of a verified commit C, and C contains **the main it was recorded over**.
//! Recorded by any worker: the lane is not the author.
//!
//! **Both halves of that sentence moved once** (air-9ij, 2026-09-06), because both were
//! evaluated against a target that keeps moving:
//!
//! - "C contains main" was asked of CURRENT main at query time, so a green that contained main
//!   when it ran was silently disqualified the instant anyone wrote to main. An adopter's
//!   coordinator invalidated a whole batch with one prose commit. It is now asked of the run's
//!   own `main_sha`, recorded before the verify started. The LANDING gate still asks about
//!   current main and did not move: a close says the bead's work was verified, a landing says
//!   main will still be green, and only the second expires.
//! - An empty range was read as "this bead has no commits" when it also means "every commit of
//!   this bead is already in main". [`landed_by`] tells the two apart.
//!
//! Removal condition: never, while the gate exists; this is the gate's definition of green.

use std::path::Path;

use air_ledger::Ledger;
use air_ledger::verify::Kind;
use serde::Serialize;

use crate::git;

/// One commit in [`BRANCH_RANGE`] that carries the bead's trailer.
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
    /// The green contained MAIN AS IT STOOD WHEN THE RUN WAS RECORDED (air-9ij), from the
    /// run's own `main_sha`; current main only for rows written before that was stored.
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
    /// The bead has no commit left for main to be missing: Air landed it, and main still
    /// contains that merge (air-9ij). `(merge commit, the worker whose branch was merged)`.
    /// No green is looked for, because the landing already required one.
    pub landed: Option<(String, String)>,
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

/// The ONE range for "what has this branch done that main does not have yet" (air-9ij).
///
/// Every reader that asks which commits a branch is answerable for reads this range, and none
/// spells it out: `bead_commits` here, and the gate's carried-bead scan in `handover::facts`.
/// The range is deliberately still `main..HEAD` — the commits main already has need no
/// evidence, because nothing reaches main without the landing gate's green. What was wrong
/// was reading an EMPTY result as "this bead has no commits" when it also means "every commit
/// of this bead is already in main"; [`for_bead`] now tells the two apart.
pub const BRANCH_RANGE: &str = "main..HEAD";

/// The commits of `bead` in [`BRANCH_RANGE`], newest first, by their `Bead:` trailer only.
/// Empty when the bead has no commit here, which includes the case where it has landed —
/// [`for_bead`] is what distinguishes them, never this.
pub fn bead_commits(repo: &Path, bead: &str) -> Vec<BeadCommit> {
    let text =
        git::run(repo, &["log", "--format=%H%x1f%s%x1f%B%x1e", BRANCH_RANGE]).unwrap_or_default();
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

/// Air landed this bead, and main still contains the merge (air-9ij).
///
/// The judgement this encodes, recorded because it was a judgement: **when a bead's work is
/// already in main, the close passes.** `air land` refuses a branch without a recorded green
/// at a head containing main, and the commit it fast-forwards main onto has that green's
/// exact tree (air-odv), so the work reached main carrying precisely the proof this gate
/// asks for. Demanding a fresh green naming the merge would make Air refuse the close of a
/// bead Air itself landed and is already nagging about as `landed-not-closed`.
///
/// It is a landing ROW, not "HEAD is an ancestor of main", because the row names the bead by
/// the same `Bead:` trailer the rest of the gate reads. A worker that claimed a bead and
/// committed nothing has no row, so it still has nothing to close on — the looser branch-level
/// test would have handed it a pass. A repo that lands by hand writes no row and its workers
/// re-verify as before; that is the status quo, not a regression.
pub fn landed_by(ledger: &Ledger, repo: &Path, bead: &str) -> Option<(String, String)> {
    ledger.landings().ok()?.into_iter().find_map(|l| {
        let merge = l.merge_commit.clone()?;
        (l.landed()
            && l.beads.iter().any(|b| b == bead)
            && git::is_ancestor(repo, &merge, "main").unwrap_or(false))
        .then_some((merge, l.worker))
    })
}

/// The batch verdict for one bead on this branch, from the ledger's newest greens.
pub fn for_bead(ledger: &Ledger, repo: &Path, bead: &str) -> Result<Batch, String> {
    let commits = bead_commits(repo, bead);
    if commits.is_empty() {
        // air-9ij: an empty range is not "no such bead". It is also every bead whose commits
        // have landed, which is what a worker's own `git merge main` produces the moment its
        // batch is on main. Returning `Batch::default()` here refused those closes and said
        // there was no green at HEAD, naming nothing.
        return Ok(Batch {
            landed: landed_by(ledger, repo, bead),
            ..Batch::default()
        });
    }
    let greens = ledger
        .latest_greens(Kind::Verify, CANDIDATES)
        .map_err(|e| e.to_string())?;
    let candidates: Vec<Candidate> = greens
        .into_iter()
        .map(|g| {
            // air-9ij: the main the run was recorded over, not the main of this instant. A
            // green that contained main when it ran goes on containing it; main moving is a
            // fact about main. Rows written before v19 have no `main_sha` and fall back to
            // the old question, which is the only answer available for them.
            let against = g.main_sha.clone().unwrap_or_else(|| "main".to_string());
            let contains_main = git::is_ancestor(repo, &against, &g.sha).unwrap_or(false);
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
        if let Some((merge, worker)) = b.landed {
            // air-9ij: nothing of this bead is outstanding, so there is no green to look for.
            covered.push(format!(
                "every commit of {bead} is already in main, landed at {} (branch by {worker}), \
                 which required a green containing main",
                short(&merge)
            ));
            continue;
        }
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
