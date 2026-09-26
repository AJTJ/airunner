//! `air batch cut` — the verification lane's cut as a program (built 2026-09-25; docs/design.md §4.1).
//!
//! **The failure it removes** (an adopter, 2026-09-07, `private/adopter-corpus/`, 2026-09-14
//! `scripts/lane-merge.sh:5-35` and `FINDINGS-scripts-and-rules.md` §1.10). The lane cut every
//! batch by hand:
//!
//! - it judged merges by piped output, `git merge <sha> 2>&1 | tail -5`, whose exit status is
//!   `tail`'s, for eleven batches, so a conflicted merge and a clean one read the same;
//! - it skipped its dry-merge once, and the order the shas were typed decided which member
//!   "conflicted";
//! - three conflict predictions that night all failed, the third in a file no member named (an
//!   automated re-anchor step). A conflict is a property of a PAIR, and only a merge holding both
//!   operands can answer it.
//!
//! The adopter answered with a 159-line script around one member at a time. This is that script
//! with the set, the order and the record added:
//!
//! 1. **Members** are `air status`'s batch-ready set ([`status::batch_ready_for`]), read here and
//!    never from a message, minus the lane's own branch. **Order: oldest-ready first**, where
//!    "ready" is the committer time of the member's listed head (the moment the branch reached
//!    the sha the lane will merge), then worker name, then sha. A worker that commits again
//!    moves to the back, because the sha it was ready at is gone. [`order`] is the rule, and
//!    nothing about the order arguments were passed in survives it.
//! 2. **Pre-check** with `git merge-tree --write-tree` (git 2.38+), which merges in memory and
//!    writes nothing: each member against main, then against each EARLIER member already
//!    accepted. The first conflict drops the member, naming the other side and the paths
//!    ([`pre_check`]).
//! 3. **The cut** (not with `--dry-run`): `git merge --no-edit main`, then
//!    `git merge --no-edit <sha>` per accepted member at the listed sha. Each merge is judged by
//!    git's own exit status, then by the index (`git ls-files -u`) and by leftover conflict
//!    markers (`git diff --check <pre-merge head>`), never by reading output. A conflict the
//!    pre-check did not predict (three or more members together) is aborted, dropped and named;
//!    a merge that fails with no conflicting paths stops the cut, because it is not the member's
//!    doing (the adopter's exit 3, `lane-merge.sh:136-150`).
//!
//! Every drop is an event line (`command: batch-cut`, `decision: dropped`, the member, the
//! other side, the paths). An event line and not a ledger table: nothing reads drops back yet,
//! and a table with no reader is the do-less failure. The table the fleet design asks for (docs/design.md §10) is
//! added when a second red or a retry needs to be told from a first by a program.
//!
//! It does not verify and does not land: the next command is printed.
//!
//! Removal: when `git merge` itself reports the conflicting pair and the lane's cut needs no
//! set or order Air holds (docs/design.md §10), or with the lane (verify cheap enough that no batch
//! of more than one branch forms).

use std::path::Path;

use serde::Serialize;

use super::status::{self, BatchReady};
use super::worktree::{GIT_BUDGET, git_status};
use crate::git;

/// A batch-ready branch with the time the order rule reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub worker: String,
    pub head: String,
    pub beads: Vec<String>,
    /// Committer time of `head`, Unix seconds: when the branch reached the sha listed.
    pub ready_at: i64,
}

/// A member the cut left out, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Dropped {
    pub worker: String,
    pub head: String,
    pub beads: Vec<String>,
    /// `main`, the earlier member's worker name, or `batch` for a conflict the pairwise
    /// pre-check did not predict (it showed only when merged onto the batch so far).
    pub against: String,
    pub against_sha: String,
    pub paths: Vec<String>,
    /// `pre-check` (merge-tree) or `merge` (the real merge onto the batch).
    pub stage: &'static str,
}

/// THE order rule: oldest-ready first, then worker, then sha. Total, so the order of the input
/// never decides anything.
pub fn order(v: &mut [Candidate]) {
    v.sort_by(|a, b| {
        a.ready_at
            .cmp(&b.ready_at)
            .then_with(|| a.worker.cmp(&b.worker))
            .then_with(|| a.head.cmp(&b.head))
    });
}

/// THE drop rule, pure: in [`order`], a member conflicting with main is dropped against main;
/// otherwise one conflicting with an EARLIER accepted member is dropped against the first such
/// member; otherwise it is accepted. A dropped member is never an "earlier member" for anyone.
///
/// `conflict(ours, theirs)` answers one pair: the conflicting paths, empty when clean, `Err`
/// when the question could not be asked (which stops the cut rather than dropping anybody).
pub fn pre_check<F>(
    mut cands: Vec<Candidate>,
    main_sha: &str,
    mut conflict: F,
) -> Result<(Vec<Candidate>, Vec<Dropped>), String>
where
    F: FnMut(&str, &str) -> Result<Vec<String>, String>,
{
    order(&mut cands);
    let mut accepted: Vec<Candidate> = Vec::new();
    let mut dropped = Vec::new();
    'member: for c in cands {
        let mut against: Vec<(&str, &str)> = vec![("main", main_sha)];
        against.extend(
            accepted
                .iter()
                .map(|a| (a.worker.as_str(), a.head.as_str())),
        );
        for (name, sha) in against {
            let paths = conflict(sha, &c.head)?;
            if !paths.is_empty() {
                dropped.push(Dropped {
                    worker: c.worker.clone(),
                    head: c.head.clone(),
                    beads: c.beads.clone(),
                    against: name.to_string(),
                    against_sha: sha.to_string(),
                    paths,
                    stage: "pre-check",
                });
                continue 'member;
            }
        }
        accepted.push(c);
    }
    Ok((accepted, dropped))
}

/// `git version 2.51.1 (Apple Git-…)` → does it have `merge-tree --write-tree` (2.38+)?
pub fn has_merge_tree(version_line: &str) -> bool {
    let v = version_line
        .split_whitespace()
        .find(|w| w.starts_with(|c: char| c.is_ascii_digit()))
        .unwrap_or("");
    let mut it = v.split('.').map(|p| {
        p.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u32>()
            .unwrap_or(0)
    });
    let (major, minor) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
    (major, minor) >= (2, 38)
}

/// What one run printed, and `--json`.
#[derive(Debug, Default, Serialize)]
pub struct Cut {
    pub dry_run: bool,
    pub lane: String,
    pub main: String,
    /// The lane's head after the cut; `None` on a dry run or when nothing was merged.
    pub head: Option<String>,
    pub members: Vec<Candidate>,
    pub dropped: Vec<Dropped>,
    /// Commits already on the lane's branch and not in main before the cut. They are in the
    /// batch too; a red batch's merges show here until the lane resets to main.
    pub carried: Vec<String>,
    pub next: Option<String>,
}

fn short(s: &str) -> &str {
    s.get(..8).unwrap_or(s)
}

pub fn render(c: &Cut) -> String {
    let mut out = String::new();
    if c.dry_run {
        out.push_str(&format!(
            "dry run, nothing changed: {} would merge main at {} and then:\n",
            c.lane,
            short(&c.main)
        ));
    } else if let Some(h) = &c.head {
        out.push_str(&format!(
            "batch cut: {} at {} (main {} and {} member(s))\n",
            c.lane,
            short(h),
            short(&c.main),
            c.members.len()
        ));
    } else {
        out.push_str(&format!(
            "no batch cut: nothing to merge onto main at {}\n",
            short(&c.main)
        ));
    }
    for m in &c.members {
        out.push_str(&format!(
            "  member: {} at {} ({})\n",
            m.worker,
            short(&m.head),
            m.beads.join(" ")
        ));
    }
    for d in &c.dropped {
        out.push_str(&format!(
            "  dropped: {} at {} ({}): conflicts with {} at {} in {} [{}]\n",
            d.worker,
            short(&d.head),
            d.beads.join(" "),
            d.against,
            short(&d.against_sha),
            d.paths.join(", "),
            d.stage
        ));
    }
    if !c.carried.is_empty() {
        out.push_str(&format!(
            "  carried: {} commit(s) were already on the lane's branch and not in main; they are \
             in this batch (a red batch's merges stay until you reset to main)\n",
            c.carried.len()
        ));
    }
    if c.members.is_empty() && c.dropped.is_empty() {
        out.push_str("  nothing is batch-ready (`air status --json` `not_batch_ready` says why)\n");
    }
    if let Some(n) = &c.next {
        out.push_str(&format!("next: {n}\n"));
    }
    if !c.dropped.is_empty() {
        out.push_str(
            "Name each drop to its worker; resolving it is theirs, in their worktree, never the \
             lane's.\n",
        );
    }
    out.trim_end().to_string()
}

/// The conflicting paths of one in-memory merge, or empty when clean.
fn merge_tree(repo: &Path, ours: &str, theirs: &str) -> Result<Vec<String>, String> {
    let (code, out, err) = git_status(
        repo,
        &[
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--no-messages",
            ours,
            theirs,
        ],
        GIT_BUDGET,
    )?;
    match code {
        0 => Ok(Vec::new()),
        1 => {
            // Line 1 is the tree oid; every following line is a conflicted path.
            let mut paths: Vec<String> = out
                .lines()
                .skip(1)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect();
            paths.sort();
            paths.dedup();
            if paths.is_empty() {
                return Err(format!(
                    "git merge-tree {} {} reported a conflict and named no path: {}",
                    short(ours),
                    short(theirs),
                    err.trim()
                ));
            }
            Ok(paths)
        }
        n => Err(format!(
            "git merge-tree {} {} exited {n}: {}",
            short(ours),
            short(theirs),
            err.trim()
        )),
    }
}

/// How one real merge went, judged by the index and the tree, never by output.
enum Merged {
    Clean,
    Conflict(Vec<String>),
}

fn merge(repo: &Path, sha: &str) -> Result<Merged, String> {
    let pre = git::head(repo).map_err(|e| e.to_string())?;
    let (code, _out, err) = git_status(repo, &["merge", "--no-edit", sha], GIT_BUDGET)?;
    let unmerged = git_status(repo, &["ls-files", "-u"], GIT_BUDGET)?.1;
    let mut paths: Vec<String> = unmerged
        .lines()
        .filter_map(|l| l.split('\t').nth(1))
        .map(str::to_string)
        .collect();
    paths.sort();
    paths.dedup();
    if code != 0 {
        let _ = git_status(repo, &["merge", "--abort"], GIT_BUDGET);
        if paths.is_empty() {
            return Err(format!(
                "git merge {} failed with no conflicting paths, so it is not the member's \
                 conflict; the cut stopped. git said: {}",
                short(sha),
                err.trim()
            ));
        }
        return Ok(Merged::Conflict(paths));
    }
    // Exit 0 and still unmerged paths, or markers in what it committed: a merge driver or a
    // rerere resolution can do either. The merge is already a commit, so undo it by moving back
    // to the pre-merge head; the tree was refused dirty before the cut began.
    let check = git_status(repo, &["diff", "--check", &pre, "HEAD"], GIT_BUDGET)?.1;
    let mut marked: Vec<String> = check
        .lines()
        .filter(|l| l.contains("leftover conflict marker"))
        .filter_map(|l| l.split(':').next())
        .map(str::to_string)
        .collect();
    marked.extend(paths);
    marked.sort();
    marked.dedup();
    if marked.is_empty() {
        return Ok(Merged::Clean);
    }
    let (code, _, err) = git_status(repo, &["reset", "--hard", &pre], GIT_BUDGET)?;
    if code != 0 {
        return Err(format!(
            "merged {} with conflict markers in {} and could not move back to {}: {}",
            short(sha),
            marked.join(", "),
            short(&pre),
            err.trim()
        ));
    }
    Ok(Merged::Conflict(marked))
}

pub fn run(repo: &Path, dry_run: bool, json: bool) -> i32 {
    let (ledger, lane) = match super::open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air batch cut: {e}");
            return 1;
        }
    };
    let refuse = |why: String| -> i32 {
        super::log_event(
            &ledger,
            &lane,
            "batch-cut",
            &serde_json::json!({"dry_run": dry_run}),
            "refuse",
            &why,
            "0 members",
        );
        super::emit(
            json,
            &serde_json::json!({"ok": false, "reason": why}),
            || why.clone(),
        );
        2
    };
    if lane == "main" {
        // The worktree is named by `air lane`, not by `verify_lane`, which is only the switch
        // for the closing sequence (air-rr98).
        return refuse(
            "air batch cut: refused in the main checkout. A cut merges into the branch it runs \
             on, and main moves only by `air land`. Run it in the lane's worktree (`air lane` \
             starts it in `.claude/worktrees/lane` unless given another name)."
                .to_string(),
        );
    }
    let version = git::run(repo, &["version"]).unwrap_or_default();
    if !has_merge_tree(&version) {
        return refuse(format!(
            "air batch cut: refused: the pre-check needs `git merge-tree --write-tree` (git 2.38 \
             or later); this is `{}`. Upgrade git.",
            version.trim()
        ));
    }
    let main = match git::main_tip(repo) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("air batch cut: git rev-parse main: {e}");
            return 1;
        }
    };
    if !dry_run {
        match git_status(
            repo,
            &["status", "--porcelain", "--untracked-files=no"],
            GIT_BUDGET,
        ) {
            Ok((0, out, _)) if out.trim().is_empty() => {}
            Ok((_, out, err)) => {
                return refuse(format!(
                    "air batch cut: refused: {lane}'s tree has uncommitted changes, so a conflict \
                     could not be told from what was already here. Commit or discard them \
                     first, or ask with --dry-run, which reads no tree.\n{}{}",
                    out.trim_end(),
                    err.trim_end()
                ));
            }
            Err(e) => {
                eprintln!("air batch cut: {e}");
                return 1;
            }
        }
    }

    let (ready, _not, errors) = status::batch_ready_for(&ledger, repo);
    for e in &errors {
        eprintln!("air batch cut: {e}");
    }
    let mut cands = Vec::new();
    for BatchReady {
        worker,
        head,
        beads,
    } in ready.into_iter().filter(|b| b.worker != lane)
    {
        let ready_at = git::run(repo, &["log", "-1", "--format=%ct", &head])
            .ok()
            .and_then(|s| s.trim().parse::<i64>().ok())
            .unwrap_or(i64::MAX);
        cands.push(Candidate {
            worker,
            head,
            beads,
            ready_at,
        });
    }
    let (accepted, mut dropped) = match pre_check(cands, &main, |a, b| merge_tree(repo, a, b)) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air batch cut: {e}");
            return 1;
        }
    };

    let carried: Vec<String> = git::run(repo, &["rev-list", &format!("{main}..HEAD")])
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect();
    let mut cut = Cut {
        dry_run,
        lane: lane.clone(),
        main: main.clone(),
        carried,
        ..Default::default()
    };
    if dry_run {
        cut.members = accepted;
        cut.next = Some("air batch cut".to_string());
    } else if !accepted.is_empty() {
        // Main first: a member is judged against the main it will land on.
        match merge(repo, &main) {
            Ok(Merged::Clean) => {}
            Ok(Merged::Conflict(paths)) => {
                return refuse(format!(
                    "air batch cut: refused: {lane}'s own branch conflicts with main in {}, so \
                     no member can be judged against main. The lane's branch holds nothing \
                     worth keeping between batches: `git reset --hard main`, then cut again.",
                    paths.join(", ")
                ));
            }
            Err(e) => {
                eprintln!("air batch cut: {e}");
                return 1;
            }
        }
        for m in accepted {
            match merge(repo, &m.head) {
                Ok(Merged::Clean) => cut.members.push(m),
                Ok(Merged::Conflict(paths)) => dropped.push(Dropped {
                    worker: m.worker,
                    head: m.head,
                    beads: m.beads,
                    against: "batch".to_string(),
                    against_sha: git::head(repo).unwrap_or_default(),
                    paths,
                    stage: "merge",
                }),
                Err(e) => {
                    eprintln!("air batch cut: {e}");
                    cut.dropped = dropped;
                    eprintln!("{}", render(&cut));
                    return 1;
                }
            }
        }
        if !cut.members.is_empty() {
            cut.head = git::head(repo).ok();
            cut.next = Some("air record verify -- <the repo's verify command>".to_string());
        }
    }
    cut.dropped = dropped;
    let considered = cut.members.len().saturating_add(cut.dropped.len());

    for d in &cut.dropped {
        super::log_event(
            &ledger,
            &lane,
            "batch-cut",
            d,
            "dropped",
            &format!(
                "dropped from batch: {} at {} conflicts with {} at {} in {}",
                d.worker,
                short(&d.head),
                d.against,
                short(&d.against_sha),
                d.paths.join(", ")
            ),
            &format!("{considered} member(s) considered"),
        );
    }
    super::log_event(
        &ledger,
        &lane,
        "batch-cut",
        &serde_json::json!({"dry_run": dry_run, "members": cut.members, "head": cut.head}),
        if dry_run { "dry-run" } else { "cut" },
        &format!(
            "{} member(s), {} dropped",
            cut.members.len(),
            cut.dropped.len()
        ),
        &format!("{considered} batch-ready member(s)"),
    );
    super::emit(json, &cut, || render(&cut));
    0
}
