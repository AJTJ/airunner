//! `air handover [--bead X] [--enforce]`: gather facts, apply the pure gate.

use std::path::Path;

use air_hooks::{GateFacts, MainMove, Verdict, handover_verdict};
use air_ledger::Ledger;
use air_ledger::verify::Kind;

use crate::cmd::{emit, log_event, open};
use crate::git;

/// Gather the gate's facts for `repo`. Shared by the CLI and the Stop hook.
pub fn facts(
    ledger: &Ledger,
    worker: &str,
    repo: &Path,
    bead: Option<&str>,
    advisory: bool,
) -> Result<GateFacts, String> {
    let head = git::head(repo).map_err(|e| e.to_string())?;
    // ONE predicate for every surface (air-7wf): the same `green::at` that `air status` and
    // `air land` read, so the gate cannot refuse what status calls green or vice versa.
    let evidence = super::green::at(ledger, repo, &head, Kind::Verify)?;
    let green_at_head = evidence.holds();
    let tree_green = if green_at_head {
        None
    } else {
        evidence.detail()
    };
    // The last green sha is only informative when it differs from HEAD (a red run at HEAD
    // after an earlier green one is still "not green now").
    let last_green_sha = ledger
        .latest_green(worker, Kind::Verify)
        .map_err(|e| e.to_string())?
        .map(|r| r.sha)
        .filter(|s| *s != head);
    let main_is_ancestor = git::is_ancestor(repo, "main", "HEAD").unwrap_or(false);
    // air-5wq: the main this answer is true of, so a later refusal reads as main having moved
    // rather than as a contradiction. air-4up: when it is not an ancestor, the landing that
    // moved it, so the first refusal names the external cause instead of the third.
    let main_sha = git::run(repo, &["rev-parse", "main"]).unwrap_or_default();
    let main_moved = if main_is_ancestor {
        None
    } else {
        main_moved_by(ledger, repo, &crate::cmd::now())
    };
    // air-60x: what this branch carries, by the `Bead:` trailers in main..HEAD. The same fact
    // `air land` attributes a landing by (status.rs `select`), read here so the gate and
    // the landing agree on what makes a branch handable. Declared ids only: the prose guess
    // needs bd to narrow it and bd is never called on a hook path.
    let carried_beads: Vec<String> = {
        let mut v =
            super::attribution::attributed_in_range(repo, super::batch::BRANCH_RANGE).declared;
        v.dedup();
        v
    };
    let claimed = |b: &str| -> bool {
        ledger
            .conn()
            .query_row(
                "SELECT count(*) FROM claims WHERE bead=?1 AND worker=?2 AND released_at IS NULL",
                rusqlite::params![b, worker],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false)
    };
    let bead_claimed_or_carried = handable(bead, bead.is_some_and(claimed), &carried_beads);
    // Every bead this worker holds, once: the digest lookup reads it and the refusal names it
    // (air-xbl: it used to be computed for the lookup and thrown away before the message).
    let held: Vec<air_ledger::claims::Claim> = ledger
        .open_claims()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.worker == worker)
        .collect();
    let held_beads: Vec<String> = held.iter().map(|c| c.bead.clone()).collect();
    // air-80x.1: with no green at HEAD, a verify lane's batch may still cover the bead. Per
    // bead: the one named, else every bead this worker holds. Only on the slow path, so a
    // worker who verified at HEAD pays no git spawns here.
    let batch = if green_at_head {
        super::batch::Described::default()
    } else {
        let targets: Vec<String> = match bead {
            Some(b) => vec![b.to_string()],
            None => held_beads.clone(),
        };
        super::batch::describe(ledger, repo, &targets)
    };
    let digest_at = digest_location(repo);
    let digest = digest_at.as_ref().and_then(|d| {
        // Newer than this worker's oldest open claim, or than the branch point from main,
        // whichever is earlier: a re-claim after a bd timeout must not postdate a digest
        // that was written between the first claim and the re-claim (air-y8m).
        let claim = held.iter().map(|c| c.claimed_at.clone()).min();
        let since = claim.map(|c| {
            let bp = git::branch_point_time(repo, "main").ok();
            match (
                c.parse::<jiff::Timestamp>().ok(),
                bp.and_then(|b| b.parse::<jiff::Timestamp>().ok()),
            ) {
                (Some(ct), Some(bt)) => ct.min(bt).to_string(),
                _ => c,
            }
        });
        // The bead named on the command line, else every bead this worker still holds or
        // its branch carries: the check is "did you write the digest for the work you are
        // handing on". No bead at all means nothing to declare, and the check is skipped
        // rather than failed.
        let beads = digest_beads(bead, &held_beads, &carried_beads)?;
        // Tracked only for `digest_dir`: the shared `.air/digests/` is gitignored, and its
        // being one directory every worktree writes to is what air-ahl's tracking bought.
        let tracked = d.tracked.then(|| tracked_in(repo, &d.path));
        Some(digest_for_bead(
            &d.path,
            worker,
            &beads,
            since.as_deref(),
            frontmatter_cutoff(),
            tracked.as_ref(),
        ))
    });
    // air-ahl: `Untracked` is a refusal like `Missing`, with its own sentence.
    let digest_untracked = digest == Some(Digest::Untracked);
    let digest_present = digest.map(|d| d == Digest::Tracked);
    let runs_at_head = ledger.runs_at(&head, Kind::Verify).unwrap_or((0, 0));
    Ok(GateFacts {
        worker: worker.to_string(),
        // air-kcns: `air handover` IS the hand-over query, so there is no command to name and
        // the sentence keeps its old subject. The hook path sets this from what it matched.
        refused_command: None,
        head,
        green_at_head,
        tree_green,
        batch_green: batch.green,
        batch_predates: batch.predates,
        // air-hgi9: the scan's own counts, so a refusal says which not-green state it is.
        batch_absent: batch.absent,
        batch_absent_fix: batch.absent_fix,
        last_green_sha,
        main_is_ancestor,
        main_sha,
        main_moved,
        bead_claimed_or_carried,
        bead: bead.map(str::to_string),
        held_beads,
        carried_beads,
        runs_at_head,
        digest_present,
        digest_untracked,
        digest_dir: digest_at.map(|d| d.shown),
        advisory,
    })
}

/// Is the named bead this worker's to hand over (air-60x)? Yes when nothing was named (the
/// check is not applicable), when the worker holds an open claim on it, or when a commit in
/// `main..HEAD` declares it in a `Bead:` trailer. The trailer is what `air land` reads, so a
/// branch Air would land is a branch Air lets its author hand over: the adopter's w1 built a
/// better instrument for a defect w3 had already fixed and closed, on a branch carrying the
/// closed bead by trailer, green with main merged, and had no route through this gate. Their
/// coordinator landed it on Air's own stated criterion; that should not have needed judgement.
///
/// Supersession is named by the trailer rather than by a digest's `bead:` field because the
/// trailer is the attribution the landing records, and because digests are optional
/// (`digest_dir`) while every landing reads trailers. One fact, two readers (air-y3v).
pub fn handable(named: Option<&str>, claimed: bool, carried: &[String]) -> bool {
    match named {
        None => true,
        Some(b) => claimed || carried.iter().any(|c| c == b),
    }
}

/// Which beads the digest check looks for (air-xbl, air-60x): the bead named to the gate,
/// else every bead the worker holds or its branch carries by trailer; `None` when there is
/// none, which SKIPS the check.
///
/// Before this the no-claim case built an empty list, `digest_for_bead` matched nothing
/// against it, and the gate refused every hand-over from a worktree holding no claim, with a
/// fix naming a literal `<bead>`. The comment above it said any digest by the worker would
/// count. Neither was right: a digest declares a bead, and with no bead there is nothing for
/// it to declare. The adopter's batching lane (w4: claims nothing, merges other workers' green
/// shas, runs the full verify once) is the case; its work is those workers' beads, each with
/// its own digest, and its own hand-over has no bead of its own. A worker that DOES hold a
/// claim, or names a bead, is unchanged: it must still declare it.
pub fn digest_beads(
    named: Option<&str>,
    held: &[String],
    carried: &[String],
) -> Option<Vec<String>> {
    match named {
        Some(b) => Some(vec![b.to_string()]),
        None => {
            let mut v = held.to_vec();
            for c in carried {
                if !v.contains(c) {
                    v.push(c.clone());
                }
            }
            (!v.is_empty()).then_some(v)
        }
    }
}

/// The landing that moved main past this branch (air-4up): the newest landed row whose merge
/// commit HEAD does not contain. One `is-ancestor` per row it looks at, and it stops at the
/// first. None when main moved some other way (a commit made on main by hand), or when git
/// cannot answer — an unreadable answer reads as "not a landing", never as an accusation.
fn main_moved_by(ledger: &Ledger, repo: &Path, now: &str) -> Option<MainMove> {
    let l = ledger
        .landings()
        .ok()?
        .into_iter()
        .filter(air_ledger::landings::Landing::landed)
        .find(|l| {
            l.merge_commit
                .as_deref()
                .is_some_and(|m| !git::is_ancestor(repo, m, "HEAD").unwrap_or(true))
        })?;
    let ago_secs = seconds_between(&l.finished_at, now);
    Some(MainMove {
        merge_commit: l.merge_commit.unwrap_or_default(),
        worker: l.worker,
        at: l.finished_at,
        ago_secs,
    })
}

fn seconds_between(earlier: &str, later: &str) -> Option<i64> {
    let a: jiff::Timestamp = earlier.parse().ok()?;
    let b: jiff::Timestamp = later.parse().ok()?;
    Some(b.duration_since(a).as_secs())
}

/// `<main>/.claude/air.json`, parsed; None when absent or unreadable.
pub fn air_json(repo: &Path) -> Option<serde_json::Value> {
    let air_dir = air_ledger::paths::air_dir_for(repo).ok()?;
    let path = air_dir.parent()?.join(".claude/air.json");
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Where the close gate looks for a digest, and whether git has to track it there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestAt {
    /// The directory to read.
    pub path: std::path::PathBuf,
    /// How a refusal names it: the configured relative dir, or the shared absolute one.
    pub shown: String,
    /// True for `digest_dir` (air-ahl's tracked check); false for the shared `.air/digests/`.
    pub tracked: bool,
}

/// The digest check's location, or None when the repo asks for no digest (air-1qnp).
///
/// Opt-in, both ways: `digest_dir` (a directory in the worktree, the file tracked by git, as
/// since air-ahl) wins; else `"digests": true` puts it at `<main>/.air/digests/`, one directory
/// every worktree writes to, so a digest cannot be stranded in one worktree and needs no
/// commit. Neither key: no digest is required. Owner, 2026-09-25: Air's generated records go
/// in the gitignored `.air/`, not in the adopter's tree.
pub fn digest_location(repo: &Path) -> Option<DigestAt> {
    let air_dir = air_ledger::paths::air_dir_for(repo).ok()?;
    digest_location_from(&air_json(repo)?, repo, &air_dir)
}

/// Pure over the parsed config, for probes.
pub fn digest_location_from(
    cfg: &serde_json::Value,
    repo: &Path,
    air_dir: &Path,
) -> Option<DigestAt> {
    if let Some(d) = cfg.get("digest_dir").and_then(|v| v.as_str()) {
        return Some(DigestAt {
            path: repo.join(d),
            shown: d.to_string(),
            tracked: true,
        });
    }
    if cfg.get("digests").and_then(serde_json::Value::as_bool) == Some(true) {
        let path = air_dir.join("digests");
        return Some(DigestAt {
            shown: path.display().to_string(),
            path,
            tracked: false,
        });
    }
    None
}

// `verify_lane()` was here until air-rr98 (2026-09-25). `verify_lane` in `.claude/air.json` is
// now a boolean that only picks the closing sequence in roles.md, and no code reads it: the
// lane is the session `air lane` started (`AIR_ROLE=lane`), so its name is never declared
// twice. Its last reader named the lane's worktree in `air batch cut`'s refusal, and nothing
// checked that name against the worktree `air lane` used.

/// `journal_dir` from `.claude/air.json`: an override for a repo that wants its session
/// journals tracked in its own tree (air-3xww). Absent, they go to `<main>/.air/journal/`
/// (`init::DEFAULT_JOURNAL_DIR`, air-1qnp), which the worktree fence lets every worktree write.
///
/// **Nothing in Air reads the files themselves**: no gate, no condition, no count. `air
/// status` reads the key only to let a branch of tracked journal entries land (air-kexg).
pub fn journal_dir(repo: &Path) -> Option<String> {
    air_json(repo)?
        .get("journal_dir")?
        .as_str()
        .map(str::to_string)
}

/// Digests written at or after this instant must declare their bead in front matter; older
/// ones may still be matched by filename and mtime.
///
/// **Delete the fallback once no digest in play predates this** (air-agq). Dated so it goes on
/// evidence rather than argument, the same shape as the `Bead:` trailer's fallback (air-4re).
/// `AIR_DIGEST_FRONTMATTER_SINCE` overrides it for tests.
pub const FRONTMATTER_SINCE: &str = "2026-08-23T00:00:00Z";

pub fn frontmatter_cutoff() -> jiff::Timestamp {
    std::env::var("AIR_DIGEST_FRONTMATTER_SINCE")
        .ok()
        .and_then(|s| s.parse().ok())
        .or_else(|| FRONTMATTER_SINCE.parse().ok())
        .unwrap_or(jiff::Timestamp::UNIX_EPOCH)
}

/// The bead a digest declares, from `bead:` in its front matter.
///
/// Front matter is a `---` line, then `key: value` lines, then `---`. This reads a **declared
/// field**, not prose: it does not look at the title, the filename, or anything the author
/// phrased freely. That is the whole difference from what it replaces.
pub fn declared_bead(text: &str) -> Option<String> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        let t = line.trim();
        if t == "---" {
            return None;
        }
        if let Some((k, v)) = t.split_once(':')
            && k.trim().eq_ignore_ascii_case("bead")
        {
            let id = v.trim();
            return (!id.is_empty() && !id.contains(char::is_whitespace)).then(|| id.to_string());
        }
    }
    None
}

/// Is there a digest in `dir` for one of `beads`?
///
/// A digest counts when it **declares** the bead in front matter — nothing about its filename,
/// its title or its mtime is consulted, because a digest that says which bead it is about is
/// the answer rather than evidence for a guess.
///
/// The old rule (a `*<worker>*.md` newer than the claim) survives only for files written
/// before [`FRONTMATTER_SINCE`]. It let a digest for a DIFFERENT bead satisfy the gate, and
/// let `touch` on any old one do the same; it guards, so it failed toward permitting, and
/// nothing proved it had fired (air-agq).
/// What the directory holds for these beads (air-ahl): nothing, a file git does not track, or
/// a tracked one. Three states because the fixes are three different sentences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Digest {
    /// No file in the directory declares any of the beads.
    Missing,
    /// One does, and git does not track it — so it exists for nobody but this worktree. An
    /// adopter's worker used exactly this to satisfy the gate without a commit, and told them
    /// (air-ahl, 2026-09-06).
    Untracked,
    Tracked,
}

/// `tracked` is the set of file NAMES git tracks in the digest directory, supplied by the
/// caller; `None` for the shared `.air/digests/`, where a declaring file existing is enough
/// and answers `Tracked` (air-1qnp). Passed in rather than looked up here so this stays pure over the filesystem: a
/// probe can drive every state without a git repo, and the one `git ls-files` happens once at
/// the call site instead of once per candidate file.
pub fn digest_for_bead(
    dir: &Path,
    worker: &str,
    beads: &[String],
    since: Option<&str>,
    cutoff: jiff::Timestamp,
    tracked: Option<&std::collections::BTreeSet<String>>,
) -> Digest {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Digest::Missing;
    };
    let since_ts: Option<jiff::Timestamp> = since.and_then(|s| s.parse().ok());
    let mut untracked = false;
    let declares = |e: &std::fs::DirEntry| -> bool {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.ends_with(".md") {
            return false;
        }
        let modified = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|m| jiff::Timestamp::try_from(m).ok());
        let text = std::fs::read_to_string(e.path()).unwrap_or_default();
        match declared_bead(&text) {
            // Declared: it names the bead or it is not this bead's digest. mtime is not
            // consulted — the file says what it is about.
            Some(b) => beads.contains(&b),
            // Undeclared: the old guess, and only for a file that predates the cutoff.
            None => {
                modified.is_some_and(|m| m < cutoff)
                    && name.contains(worker)
                    && match (since_ts, modified) {
                        (Some(s), Some(m)) => m > s,
                        (None, _) => true,
                        _ => false,
                    }
            }
        }
    };
    for e in rd.flatten() {
        if !declares(&e) {
            continue;
        }
        if tracked.is_none_or(|t| t.contains(&e.file_name().to_string_lossy().to_string())) {
            return Digest::Tracked;
        }
        // Keep looking: another file may declare the same bead and be tracked. Only report
        // untracked when no tracked one exists, or a stray scratch copy would mask a real
        // digest sitting beside it.
        untracked = true;
    }
    if untracked {
        Digest::Untracked
    } else {
        Digest::Missing
    }
}

/// The file names git tracks in `dir`, for [`digest_for_bead`]. Empty when git cannot answer,
/// which reads as "nothing is tracked" and refuses — the direction a guard must fail in.
pub fn tracked_in(repo: &Path, dir: &Path) -> std::collections::BTreeSet<String> {
    crate::git::run(repo, &["ls-files", "--", &dir.to_string_lossy()])
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.rsplit('/').next())
        .map(str::to_string)
        .collect()
}

/// Is there a `*<worker>*.md` in `dir` modified after `since` (RFC 3339)? Pure over the fs.
///
/// **Superseded by [`digest_for_bead`]** (air-agq) and kept only for its tests; it is the
/// guess that could not tell one bead's digest from another's.
#[cfg(test)]
pub fn digest_newer_than(dir: &Path, worker: &str, since: Option<&str>) -> bool {
    let since_ts: Option<jiff::Timestamp> = since.and_then(|s| s.parse().ok());
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    rd.flatten().any(|e| {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.ends_with(".md") || !name.contains(worker) {
            return false;
        }
        match (since_ts, e.metadata().and_then(|m| m.modified()).ok()) {
            (Some(since), Some(m)) => jiff::Timestamp::try_from(m).is_ok_and(|t| t > since),
            (None, _) => true,
            _ => false,
        }
    })
}

/// What `air land` would say about ONE worker's branch, rendered from `select`'s own answer
/// (air-33rn).
///
/// **This is a READ, not a second copy of the landing decision.** Two workers concluded
/// independently that a journal-only branch could land, both were wrong, and neither surface a
/// worker can reach said so — each learned it from the coordinator running `air land`, which
/// workers are denied. The fix is not to re-derive landability here: `select` already computes
/// the whole answer as `Skipped { check, detail, fix }`, so this looks the worker up in that
/// answer and renders it. Restating the decision is air-avj's shape, and this round has hit it
/// three times; the rule there was to name the thing that knows rather than repeat it.
///
/// Pure over a `Selection` so a probe drives every branch with no repo and no fixture — which
/// also sidesteps the trap alerts hit on air-kexg, where a shared fixture's worker already
/// carried work, so the range was genuinely mixed and the test proved the constraint while
/// claiming to prove the permission.
pub fn landing_line(sel: &crate::cmd::status::Selection, worker: &str) -> String {
    // An error about this worker outranks everything: it means selection could not tell, and
    // "not landable" would read as a verdict it never reached (air-6u5, air-72t7).
    if let Some(e) = sel
        .errors
        .iter()
        .find(|e| e.starts_with(&format!("{worker}:")))
    {
        return format!("landing: cannot tell — {e}");
    }
    if let Some(l) = sel.landings.iter().find(|l| l.worker == worker) {
        // air-kexg landed `bead: Option<String>` while this was being written: a journal-only
        // branch IS landable and carries no bead. Saying "carrying none" rather than printing
        // an empty slot is the difference between an answer and a gap.
        return match l.bead.as_deref() {
            Some(b) => format!(
                "landing: landable at {}, carrying {b}",
                l.head.get(..8).unwrap_or(&l.head)
            ),
            None => format!(
                "landing: landable at {}, carrying no bead (journal-only branch)",
                l.head.get(..8).unwrap_or(&l.head)
            ),
        };
    }
    if let Some(s) = sel.skipped.iter().find(|s| s.worker == worker) {
        return format!(
            "landing: NOT landable — {}: {}; needs {}",
            s.check, s.detail, s.fix
        );
    }
    "landing: not a landing candidate (no worktree branch of yours is in selection)".to_string()
}

/// Whether the standing red batch has this worker in it (air-hpp8), read from the value
/// `air status` already computes.
///
/// A lane cuts a batch, it goes red, and every member has to be told by the lane remembering
/// to message each of them. Twice in one night in the adopter's fleet a member was left off —
/// the second time after the lane had already amended its practice to "every member and the
/// coordinator" — and that worker spent 132 seconds running a suite to test a hypothesis the
/// batch log had already refuted, then found the answer by opening the lane's log from another
/// worktree. Their form of it: **a fact that must reach N parties by one party remembering N
/// sends will eventually reach N-1.**
///
/// What makes it a bead is not that it failed twice. It is that the member **cannot look**:
/// `red_batch_standing` has the sha, the lane, the time and the members, `air status` renders
/// it, and a worker running `air handover` in its own worktree was told nothing. Same shape as
/// air-33rn, and fixed the same way — a lookup over the value that already exists, never a
/// second copy of the decision, and never a push channel. **The lane messaging people stays
/// fine; the defect was that it was the only route.**
///
/// # The silence is not a claim of non-membership
///
/// `None` covers two different states and says nothing in both, deliberately:
///
/// - the worker is genuinely not among members that WERE recorded, and
/// - the members list is empty, so Air does not know who was in it.
///
/// 345 of 347 verify runs in this ledger carry no members at all, because the pre-air-vsvt
/// `members_of` dropped any member whose head had moved. So an empty list is exactly what "not
/// in a batch" and "in a batch, members dropped" both look like, and an `else` arm here would
/// turn a dropped list into "you were not in it" — a wrong answer wearing the same grammar as
/// a right one.
///
/// Distinguishing the two in the OUTPUT is air-sdjo's, because it needs the lane to declare
/// what it merged. Three routes without that were checked and all three are guesses (owner's
/// coordinator ruled reading A, 2026-09-06):
///
/// 1. Ask git whether this head is an ancestor of the red sha. Exact, needs no members — and
///    recomputes membership, which is the one thing this bead forbids by name.
/// 2. Report any standing red with no members as "Air cannot tell whether you were in it".
///    That fires on 345 of 347 runs, framing every ordinary red as a possible batch: a
///    mechanism speaking on the ok path, which is how a line teaches its reader to skip it.
/// 3. Read `verify_lane` from `.claude/air.json` and treat an unmembered red by that worker as
///    a batch. Declared rather than guessed, and it would work in the adopter's repo — but
///    **this** repo batched through `verify` on 2026-09-06 with no such key, so the most
///    principled-looking option would have been silent for exactly the case it exists for.
///
/// Pure over the batch so a probe drives every arm from a constructed `RedBatch` with no repo
/// and no fixture.
pub fn red_batch_line(batch: Option<&crate::cmd::batch::RedBatch>, worker: &str) -> Option<String> {
    let b = batch?;
    if !b.members.iter().any(|m| m.worker == worker) {
        return None;
    }
    let mine = b
        .members
        .iter()
        .find(|m| m.worker == worker)
        .map(|m| m.sha.get(..8).unwrap_or(&m.sha).to_string())
        .unwrap_or_default();
    // The log is the point as much as the verdict: the adopter's worker had to read it from
    // another worktree to learn what the batch had already established.
    let log = match b.log_path.as_deref() {
        Some(p) => format!("the lane's output is at {p}"),
        None => "the lane's run kept no output".to_string(),
    };
    Some(format!(
        "batch: your branch at {mine} was in the RED batch {} cut by {} at {}; nothing lands \
         on it and the lane splits by hand — {log}",
        b.sha.get(..8).unwrap_or(&b.sha),
        b.worker,
        b.at,
    ))
}

pub fn run(repo: &Path, bead: Option<&str>, enforce: bool, json: bool) -> i32 {
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air handover: {e}");
            return 1;
        }
    };
    let f = match facts(&ledger, &worker, repo, bead, !enforce) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("air handover: {e}");
            return 1;
        }
    };
    let v: Verdict = handover_verdict(&f);
    // NOT stamped here (air-eiv). `air handover` is documented as the way to find what is
    // missing, and it used to increment `handover_attempts` on the claim -- the same counter
    // `handover-not-green` reads -- so running the diagnostic raised the alarm and the
    // coordinator went after a worker who was doing exactly what the docs say. A query does
    // not count as an attempt. The hook path still stamps, in `hook::handover_gate`, because
    // there a `bd` status write is actually being made.
    //
    // Removal: when nothing counts hand-over attempts, the stamp goes from both paths.
    // A stop usually follows a hand-over: refresh the ready list the Stop hook reads
    // (air-09i). One bd call, outside any hook budget.
    let _ = crate::cmd::ready_cache::refresh(
        repo,
        &crate::cmd::claim::bd_for(repo),
        &crate::cmd::now(),
    );
    log_event(
        &ledger,
        &worker,
        if v.pass {
            super::decisions::HANDOVER_PASS
        } else if v.block {
            super::decisions::HANDOVER_REFUSE
        } else {
            super::decisions::HANDOVER_WOULD_REFUSE
        },
        &serde_json::json!({"bead": bead, "enforce": enforce, "head": f.head}),
        &v.message,
        "4 checks",
    );
    // air-33rn: the landing answer, from the ONE selection `air land` runs. A worker cannot
    // run `air land`, so without this the only way to learn its branch is unlandable was to ask
    // the coordinator — which is how two workers spent tonight believing a journal-only branch
    // would land.
    let landing = landing_line(&crate::cmd::status::select(repo), &worker);
    // air-hpp8: and whether a standing red batch has this branch in it. Same lookup `air
    // status` does, so the two surfaces cannot disagree.
    let batch = red_batch_line(
        crate::cmd::batch::red_batch_standing(&ledger, repo).as_ref(),
        &worker,
    );
    // ADDED to the verdict, never wrapping it. Nesting it under a key moved `missing` to
    // `handover.missing` and broke four integration tests that parse this — a surface change
    // nobody asked for, to add a field. A new key costs every existing caller nothing.
    let mut out = serde_json::to_value(&v).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(o) = out.as_object_mut() {
        o.insert("landing".to_string(), serde_json::json!(landing));
        // Only when there is something to say. An always-present `null` would read as "you
        // are not in a batch", which is the claim this must never make.
        if let Some(b) = &batch {
            o.insert("batch".to_string(), serde_json::json!(b));
        }
    }
    emit(json, &out, || match &batch {
        Some(b) => format!("{}\n{landing}\n{b}", v.message),
        None => format!("{}\n{landing}", v.message),
    });
    if v.block { 2 } else { 0 }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod digest_tests {
    use super::digest_newer_than;

    #[test]
    fn digest_must_match_worker_and_be_newer_than_claim() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("2026-08-21-frontend-icons.md"), "x").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        assert!(digest_newer_than(dir.path(), "frontend", None));
        assert!(!digest_newer_than(dir.path(), "backend", None));
        // A claim in the future: nothing is newer.
        assert!(!digest_newer_than(
            dir.path(),
            "frontend",
            Some("2999-01-01T00:00:00Z")
        ));
        assert!(digest_newer_than(
            dir.path(),
            "frontend",
            Some("2000-01-01T00:00:00Z")
        ));
        assert!(!digest_newer_than(
            &dir.path().join("missing"),
            "frontend",
            None
        ));
    }
}
