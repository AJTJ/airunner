//! A green at a verified descendant closes the bead it covers (air-80x.1).
//!
//! The gate wanted a green AT the worker's HEAD (or, since air-7wf, at its tree). A verify
//! lane's green is at the batch head, whose tree holds everyone's changes, so neither key
//! matched and the lane's green closed nothing: the adopter's 2026-08-29 round parked a quarter
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
//!   this bead is already in main". [`in_main`] tells the two apart, from git (air-5ynn).
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

/// What the scan looked at, when it found nothing (air-hgi9).
///
/// Four distinct states rendered as one sentence, `no green verify recorded at HEAD`: no green
/// exists at all, none contains the main it must, none touches the bead's commits, and the bead
/// has no commit here. An adopter's w3 met three of them in one night. They have opposite
/// correct responses — wait for the next batch, versus stop waiting — and the reader could not
/// tell which it was looking at.
///
/// Every field is counted from the loop [`cover`] already runs; nothing extra is read, and no
/// git command is added. Only ever consulted when nothing covered the bead, which is why
/// `covering` short-circuits without finishing the count.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Scanned {
    /// Recorded greens considered: every one recorded after the bead's oldest commit.
    pub candidates: usize,
    /// Of those, the ones containing the main they were recorded over.
    pub with_main: usize,
    /// Of those, the ones containing at least one commit of the bead.
    pub touching: usize,
    /// The bead's commits in [`BRANCH_RANGE`].
    pub commits: usize,
}

/// A bead whose every commit is in main (air-5ynn).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct InMain {
    /// The newest commit on main naming the bead in a `Bead:` trailer.
    pub newest: String,
    /// The landing row that put it there, when written: (merge commit, worker). `None` in the
    /// seconds between `air land`'s fast-forward and its row, and for a hand landing.
    pub landing: Option<(String, String)>,
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
    /// The bead has no commit left for main to be missing: main has every commit naming it
    /// (air-9ij, air-5ynn). No green is looked for, because main moves only through a green.
    pub landed: Option<InMain>,
    /// What the scan looked at (air-hgi9). Meaningful only when nothing covered the bead.
    pub scanned: Scanned,
}

/// Pure: which candidate, if any, covers the bead. Candidates are newest first, so the first
/// covering one wins and the first partial one is what a refusal names.
pub fn cover(candidates: &[Candidate], commits: &[BeadCommit]) -> Batch {
    let mut out = Batch::default();
    out.scanned.candidates = candidates.len();
    out.scanned.commits = commits.len();
    if commits.is_empty() {
        return out;
    }
    for c in candidates.iter().filter(|c| c.contains_main) {
        out.scanned.with_main = out.scanned.with_main.saturating_add(1);
        if c.contains.iter().any(|x| *x) {
            out.scanned.touching = out.scanned.touching.saturating_add(1);
        }
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

/// Every green that can contain the bead's work: the ones recorded after its oldest commit was
/// made (air-06k7). A green recorded before a commit existed cannot contain it, so this reads
/// every candidate there is and no others. It was the newest 12 greens, a number nobody ruled:
/// a covering green 13th newest was refused, and the refusal said "12". Each candidate costs
/// one `git merge-base` plus one per bead commit, only on the path where HEAD has no green.
///
/// The commit time is git's committer time in whole seconds, and a run's `finished_at` has
/// sub-seconds, so the bound is one second earlier. When git cannot say, every green is read.
fn greens_for(
    ledger: &Ledger,
    repo: &Path,
    oldest: Option<&BeadCommit>,
) -> Result<Vec<air_ledger::verify::VerifyRun>, String> {
    let since = oldest
        .and_then(|c| git::run(repo, &["log", "-1", "--format=%ct", &c.sha]).ok())
        .and_then(|t| t.trim().parse::<i64>().ok())
        .and_then(|t| jiff::Timestamp::from_second(t.saturating_sub(1)).ok())
        .map(|t| t.to_string())
        .unwrap_or_default();
    ledger
        .greens_since(Kind::Verify, &since)
        .map_err(|e| e.to_string())
}

/// The newest commit on main whose `Bead:` trailer names `bead`, if any (air-5ynn).
///
/// **When a bead has no commit in [`BRANCH_RANGE`] and main has one naming it, every commit of
/// the bead is in main, and the close passes.** Main moves only through a landing, and `air
/// land` refuses a branch without a recorded green at a head containing main, so being in main
/// is the proof by itself. (The owner landing by hand is the owner's call.)
///
/// This used to be asked of the landing ROW (air-9ij), which `air land` writes after the
/// fast-forward. A close in the seconds between the two found the commits gone from
/// `main..HEAD` and no row naming them, and was refused: an adopter counted five in one day,
/// each right after "close them now", each passing on a retry. Git is the fact; the row is a
/// record of it. A worker that claimed a bead and committed nothing still has nothing here,
/// so it still has nothing to close on.
pub fn in_main(repo: &Path, bead: &str) -> Option<String> {
    let text = git::run(
        repo,
        &[
            "log",
            "--format=%H%x1f%B%x1e",
            "-F",
            &format!("--grep={bead}"),
            "main",
        ],
    )
    .ok()?;
    text.split('\u{1e}').find_map(|rec| {
        let (sha, body) = rec.trim_start_matches('\n').split_once('\u{1f}')?;
        super::attribution::trailer_ids(body)
            .iter()
            .any(|b| b == bead)
            .then(|| sha.trim().to_string())
    })
}

/// The landing that put `bead` in main, when the ledger has its row: (merge, worker). Only
/// for the message; [`in_main`] decides.
fn landed_by(ledger: &Ledger, repo: &Path, bead: &str) -> Option<(String, String)> {
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
        let landed = in_main(repo, bead).map(|newest| InMain {
            newest,
            landing: landed_by(ledger, repo, bead),
        });
        return Ok(Batch {
            landed,
            ..Batch::default()
        });
    }
    let greens = greens_for(ledger, repo, commits.last())?;
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
/// for every worker worktree, the sha of its branch that `head` actually contains, `own` (the
/// batch's own worker) excluded. Git ancestry only; nothing is read from commit messages.
/// `air record` writes it on the run and `air land` on the landing, so a red batch and a
/// landed one name their members the same way.
///
/// **The sha is the MERGE BASE, not the worktree's current head** (air-vsvt). It used to ask
/// "is your head an ancestor of the batch", which is a question about where the branch is NOW,
/// answered while recording a fact about what the batch WAS. A worker that commits between the
/// lane's merge and the lane's `air record` — a window of minutes, and the lane's own verify is
/// the slowest thing in the round — stops being an ancestor and drops out of the list, and
/// because the list is recorded on the row it is then wrong for good. An adopter's lane saw
/// exactly that five times in one night. Reproduced here before changing anything: a batch that
/// merged alpha and beta recorded only beta, because alpha had committed once more.
///
/// The merge base does not move when the branch commits again: it is the sha the batch took.
/// For a branch the batch never took it is the fork point, which is in main and so is excluded
/// by the same test as before — so this cannot invent a member either, which the old shape
/// could when a worktree's head happened to sit on another branch's commit.
///
/// One `git merge-base` per worker replaces two `merge-base --is-ancestor` calls, so it is also
/// one spawn cheaper per worker.
/// The shas a batch commit was built from: the non-first parents of every merge it contains,
/// bounded by the main the run recorded (air-vsvt).
///
/// This is the commit's own account of what it took, and it does not move when branches do.
/// The bound is `tip`, which callers pass from the run's recorded `main_sha` — a fact written
/// before the verify started, not a read of where main is now.
pub fn merged_shas(repo: &Path, head: &str, tip: &str) -> Vec<String> {
    let range = format!("{tip}..{head}");
    let text =
        git::run(repo, &["rev-list", "--min-parents=2", "--parents", &range]).unwrap_or_default();
    let mut out = Vec::new();
    for line in text.lines() {
        // `<commit> <parent1> <parent2>...`: the first parent is the lane's own line, the rest
        // are what it merged in.
        for p in line.split_whitespace().skip(2) {
            let p = p.to_string();
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    // `rev-list` walks newest-first; report in the order the lane merged them. Not cosmetic —
    // an existing probe pins the member order, and reversing here keeps that contract rather
    // than rewriting its assertion to fit a new one.
    out.reverse();
    out
}

/// The members a batch took, from the batch commit rather than from where branches are now
/// (air-vsvt).
///
/// **What changed and why.** This used to enumerate the worktrees, read each one's CURRENT
/// head, and keep the merge-base if it was not yet in main. Reproduced in a fixture: a member
/// that resets to main **drops out**, with main held still, and so does one whose work lands.
/// Both are the same fact — the branch no longer contains the work the batch took — and the
/// row that results looks complete: plausible workers, plausible shas, nothing saying a member
/// is missing. An adopter's red batch named the two members who had stopped and omitted the
/// one who was fixing the failure.
///
/// **What inference cannot do, proven rather than assumed.** The parent set gives the exact
/// shas. It cannot say which BRANCH offered one: if w4 forks off w3 and the lane merges only
/// w3, both branches present w3's sha to the batch, and the tie-break — w3's head still
/// equalling that sha — is gone the moment w3 commits again. So the shas are recoverable from
/// the commit and the names are not, which is the argument for the lane recording its own
/// merges. Until it does, this resolves names best-effort.
///
/// **A sha the batch demonstrably took is never dropped for want of a name.** An unattributed
/// member is honest and visible; an omission is neither, and every symptom on this bead is an
/// omission wearing a complete-looking row.
///
/// **Known limitation, found by running the probe rather than reasoning about it.** A name can
/// migrate to a fork. Once w3 abandons its work, w4 is the only branch still containing w3's
/// sha, so it becomes the unambiguous holder and inherits the entry. No amount of ancestry
/// fixes this — it is the same fact as above, that ancestry recovers shas and not names — and
/// it is the second reason the lane should record what it merged. The SHA stays correct in
/// every case, which is the half a report about a historical event actually needs.
pub fn members_of(
    repo: &Path,
    own: &str,
    head: &str,
    tip: &str,
) -> Vec<air_ledger::landings::Member> {
    // Who each worktree is, and what it currently contains. Used ONLY to put a name to a sha
    // the batch commit already named; never to decide membership.
    let mut branches: Vec<(String, String)> = Vec::new();
    for (path, _) in git::worktrees(repo).unwrap_or_default() {
        let worker = air_ledger::paths::worker_name_for(&path).unwrap_or_default();
        // Every branch but the lane's own and the main checkout: the coordinator's branch is
        // a batch member like a worker's (air-jc2p.1).
        if worker == own || worker == "main" {
            continue;
        }
        if let Ok(wt_head) = git::head(&path) {
            branches.push((worker, wt_head));
        }
    }
    merged_shas(repo, head, tip)
        .into_iter()
        // Main's own merge is not a member, and neither is anything already in main.
        .filter(|sha| !git::is_ancestor(repo, sha, tip).unwrap_or(false))
        .map(|sha| {
            // The branch that offered this sha, when exactly one still contains it. Two
            // candidates means a fork off another worker's branch and no way to tell them
            // apart; zero means the branch has moved off its own work. Both keep the sha.
            let mut holders = branches
                .iter()
                .filter(|(_, h)| git::is_ancestor(repo, &sha, h).unwrap_or(false));
            let worker = match (holders.next(), holders.next()) {
                (Some((w, _)), None) => w.clone(),
                _ => String::new(),
            };
            air_ledger::landings::Member { worker, sha }
        })
        .collect()
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
    /// Where the run's output was kept (air-hpp8). A non-green run keeps its tail
    /// (`runlog::keeps_output`), and that file is what a member actually wants: the adopter's
    /// worker found the batch's verdict by opening the lane's log from another worktree,
    /// having not been told. Carried here so the answer and the evidence arrive together.
    /// `None` when the run kept nothing.
    pub log_path: Option<String>,
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
            log_path: r.log_path.clone(),
        })
        .collect()
}

/// Pure: is `red` superseded by any of `greens`? A green supersedes it when it carries every
/// one of its members, so the work the batch failed on has since been verified together.
pub fn superseded_by(
    repo: &Path,
    red: &RedBatch,
    greens: &[air_ledger::verify::VerifyRun],
) -> bool {
    greens.iter().filter(|g| g.finished_at > red.at).any(|g| {
        red.members
            .iter()
            .all(|m| git::is_ancestor(repo, &m.sha, &g.sha).unwrap_or(false))
    })
}

/// The newest red batch still standing: none of its members has since been carried by a
/// newer green run (a later batch, or the worker's own verify at that head). One list, one
/// answer; the lane reads it in `air status` until a newer batch supersedes it.
///
/// **No window** (air-cyf). This used to read the last 20 verify runs and pick the red batch
/// out of them, so a batch that stayed red across 20 further runs stopped being reported with
/// nothing said — and a report that was dropped looked exactly like one that was fixed, which
/// is a failure toward permitting in the one place the fleet is told nothing may land. The 20
/// had no test and no reason recorded beside it.
///
/// Both halves are now asked of the rows themselves: the newest red run that carried members,
/// and the green runs finished after it. Neither is bounded by a count, and both are one
/// indexed query. A busy day cannot age a standing red out of view.
pub fn red_batch_standing(ledger: &Ledger, repo: &Path) -> Option<RedBatch> {
    let run = ledger.latest_red_batch(Kind::Verify).ok()??;
    let red = red_batches_of(std::slice::from_ref(&run))
        .into_iter()
        .next()?;
    let greens = ledger.greens_since(Kind::Verify, &red.at).ok()?;
    (!superseded_by(repo, &red, &greens)).then_some(red)
}

/// One line for a red batch, as `air record` prints it and `air status` repeats it.
pub fn red_batch_line(b: &RedBatch) -> String {
    let members: Vec<String> = b
        .members
        .iter()
        .map(|m| format!("{}@{}", m.worker, short(&m.sha)))
        .collect();
    // air-ch9p: a batch of one has nothing to split.
    let then = if b.members.len() == 1 {
        "its only member is batch-ready again after a new commit"
    } else {
        "the lane splits by hand"
    };
    format!(
        "batch red at {} ({}): members {}; nothing lands on it, {then}",
        short(&b.sha),
        b.worker,
        members.join(", ")
    )
}

/// The two sentences the gate carries (air-80x.1): the ok line when every bead in `beads`
/// is covered by a batch green, else the refusal detail from the first bead a batch predates.
/// `(None, None)` when no bead has a batch green or a partial one.
/// What a scan found, for the refusal (air-hgi9). At most one is `Some`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Described {
    /// A green that covers every bead: the close passes.
    pub green: Option<String>,
    /// A green containing main covers some of a bead's commits but not the newest.
    pub predates: Option<String>,
    /// Nothing covered, and WHY — which of the remaining states this is. The three it tells
    /// apart used to be one sentence, and two of them have opposite correct responses.
    pub absent: Option<String>,
    /// The fixing line for [`Self::absent`]. Carried beside it rather than re-derived by the
    /// caller, so the two cannot come from different branches of the same question.
    pub absent_fix: Option<String>,
}

/// Why no candidate covered `bead`, in the terms a worker acts on (air-hgi9).
///
/// Pure over the counts, so a probe drives every branch without a ledger or a repo. The order
/// is narrowest-cause-first: a bead with no commits here is not a verify problem at all, and a
/// scan with no greens to look at is not a coverage problem.
pub fn why_absent(bead: &str, s: &Scanned) -> String {
    if s.commits == 0 {
        return format!(
            "no commit in `{BRANCH_RANGE}` carries a `Bead: {bead}` trailer, and no landing \
             carries it either, so there is nothing for a green to cover"
        );
    }
    if s.candidates == 0 {
        return format!(
            "no green verify is recorded to check: none was recorded after the oldest of the {} \
             commit(s) of {bead}",
            s.commits
        );
    }
    if s.with_main == 0 {
        return format!(
            "{} recorded green(s) checked, and none contains the main it was recorded over, so \
             none can stand for {bead}",
            s.candidates
        );
    }
    if s.touching == 0 {
        return format!(
            "{} green(s) contain main, and none contains any of the {} commit(s) of {bead}: \
             every one was recorded before this work",
            s.with_main, s.commits
        );
    }
    format!(
        "{} green(s) contain main and touch {bead}, none covering all {} of its commit(s)",
        s.with_main, s.commits
    )
}

/// The fixing line for each of those, which is the half that differs (air-hgi9). "Wait for the
/// next batch" and "stop waiting" are opposite instructions and used to share a sentence.
///
/// Each states what must become TRUE rather than which command makes it so (air-155w): under a
/// lane the worker runs no verify, and a fix naming one is followed successfully by the wrong
/// worker.
pub fn fix_absent(bead: &str, s: &Scanned) -> String {
    if s.commits == 0 {
        // The green stays the subject: with no lane, a green at this head is the fix, and a
        // probe caught the version that named only the trailer. air-ch9p: under a lane it is
        // the trailer, because a branch whose commits name no bead it holds is never
        // batch-ready, so no batch green comes. The line names both and which is which.
        return format!(
            "a green at this head; a batch covers only commits whose message carries \
             `Bead: {bead}`, so under a lane that trailer comes first"
        );
    }
    if s.candidates == 0 {
        return "a green containing main and your commits; none is recorded since your oldest \
                commit, and `air handover` names what your flow needs to produce one"
            .to_string();
    }
    if s.with_main == 0 {
        return "a green whose run contained main; where a lane runs, that is the next batch \
                that merges main before it verifies"
            .to_string();
    }
    "a green recorded at or after your commits: the ones on record are all older, so where a \
     lane runs it is the next batch rather than this one"
        .to_string()
}

pub fn describe(ledger: &Ledger, repo: &Path, beads: &[String]) -> Described {
    let mut covered: Vec<String> = Vec::new();
    let mut out = Described::default();
    for bead in beads {
        let b = for_bead(ledger, repo, bead).unwrap_or_default();
        if let Some(m) = b.landed {
            // air-9ij: nothing of this bead is outstanding, so there is no green to look for.
            covered.push(match m.landing {
                Some((merge, worker)) => format!(
                    "every commit of {bead} is already in main, landed at {} (branch by \
                     {worker}), which required a green containing main",
                    short(&merge)
                ),
                None => format!(
                    "every commit of {bead} is already in main (newest {})",
                    short(&m.newest)
                ),
            });
            continue;
        }
        match (b.covering, b.predates) {
            (Some((sha, worker)), _) => covered.push(format!(
                "green at {} (batch by {worker}) contains every commit of {bead}",
                short(&sha)
            )),
            (None, Some((sha, worker, missing, subject))) => {
                out.predates = Some(format!(
                    "the batch's green at {} (by {worker}) predates your commit {} \
                     \"{subject}\" for {bead}: it does not contain it",
                    short(&sha),
                    short(&missing)
                ));
                return out;
            }
            (None, None) => {
                // air-hgi9: the state that used to be silent. The counts say which it is.
                out.absent = Some(why_absent(bead, &b.scanned));
                out.absent_fix = Some(fix_absent(bead, &b.scanned));
                return out;
            }
        }
    }
    if beads.is_empty() || covered.len() != beads.len() {
        return out;
    }
    out.green = Some(covered.join("; "));
    out
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
        // The VERDICT is what this pins. `scanned` is diagnostic and is asserted separately
        // below (air-hgi9); comparing whole `Batch` values made every new diagnostic field a
        // false failure here, which is how a test starts resisting information.
        let lacks_main = cover(&[cand("x", false, &[true])], &commits);
        assert_eq!((&lacks_main.covering, &lacks_main.predates), (&None, &None));
        let no_commits = cover(&[cand("x", true, &[true])], &[]);
        assert_eq!((&no_commits.covering, &no_commits.predates), (&None, &None));
    }

    /// air-06k7: the covering green is found however many greens were recorded after it. The
    /// gate read the newest 12, so a batch green followed by twelve others refused the close.
    #[test]
    fn a_covering_green_thirteenth_newest_still_covers() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let g = |args: &[&str]| git::run(repo, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "air"]);
        g(&["config", "user.email", "air@example.invalid"]);
        g(&["commit", "-q", "--allow-empty", "-m", "root"]);
        let main = g(&["rev-parse", "HEAD"]);
        g(&["checkout", "-q", "-b", "work"]);
        g(&[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "feat: x\n\nBead: zz-13\n",
        ]);
        let batch = g(&["rev-parse", "HEAD"]);
        let ledger = Ledger::open_in_memory().unwrap();
        let green = |sha: &str, at: &str| {
            ledger
                .record_verify(&air_ledger::verify::VerifyRun {
                    id: format!("run-{at}"),
                    worker: "lane".into(),
                    sha: sha.into(),
                    kind: Kind::Verify,
                    exit_code: 0,
                    trigger: "test".into(),
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
                    main_sha: Some(main.clone()),
                })
                .unwrap();
        };
        green(&batch, "2099-01-01T00:00:00Z");
        for i in 1..=12 {
            green(&main, &format!("2099-01-01T00:{i:02}:00Z"));
        }
        let b = for_bead(&ledger, repo, "zz-13").unwrap();
        assert_eq!(b.covering, Some((batch, "lane".to_string())), "{b:?}");
        assert_eq!(b.scanned.candidates, 13);
    }

    /// air-5ynn: a bead whose trailer commits are all in main passes with no landing row. That
    /// is the state between `air land`'s fast-forward and its row, where an adopter's closes
    /// were refused. A bead nothing on main names still has nothing to close on.
    #[test]
    fn a_bead_in_main_is_covered_without_a_landing_row() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let g = |args: &[&str]| git::run(repo, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "air"]);
        g(&["config", "user.email", "air@example.invalid"]);
        g(&["commit", "-q", "--allow-empty", "-m", "root"]);
        g(&["checkout", "-q", "-b", "work"]);
        g(&[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "feat: x\n\nBead: zz-11\n",
        ]);
        let work = g(&["rev-parse", "HEAD"]);
        g(&["checkout", "-q", "main"]);
        g(&["merge", "-q", "--ff-only", "work"]);
        g(&["checkout", "-q", "work"]);
        let ledger = Ledger::open_in_memory().unwrap();

        let b = for_bead(&ledger, repo, "zz-11").unwrap();
        assert_eq!(
            b.landed,
            Some(InMain {
                newest: work,
                landing: None
            })
        );
        let d = describe(&ledger, repo, &["zz-11".to_string()]);
        assert!(
            d.green
                .as_deref()
                .is_some_and(|m| m.contains("already in main")),
            "{d:?}"
        );
        // A prefix of the id is not the id.
        assert_eq!(for_bead(&ledger, repo, "zz-1").unwrap().landed, None);
    }

    /// air-hgi9: the counts that let a refusal say WHICH not-green state it is, taken from the
    /// loop `cover` already runs. Four states that were one sentence; two of them have
    /// opposite correct responses.
    #[test]
    fn the_scan_counts_what_it_looked_at() {
        let commits = vec![commit("c2"), commit("c1")];
        // Nothing recorded to look at.
        assert_eq!(cover(&[], &commits).scanned.candidates, 0);
        // Recorded, but disqualified on the main it was recorded over (air-9ij): counted as a
        // candidate, never as one containing main.
        let s = cover(&[cand("x", false, &[true, true])], &commits).scanned;
        assert_eq!((s.candidates, s.with_main, s.commits), (1, 0, 2));
        // Contains main, touches nothing of this bead.
        let s = cover(&[cand("y", true, &[false, false])], &commits).scanned;
        assert_eq!((s.with_main, s.touching), (1, 0));
        // Contains main and touches it, but does not cover it: that is `predates`, and the
        // count says the candidate was reached rather than filtered out.
        let s = cover(&[cand("z", true, &[false, true])], &commits).scanned;
        assert_eq!((s.with_main, s.touching), (1, 1));
    }
}
