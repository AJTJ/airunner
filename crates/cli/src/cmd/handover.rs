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
    // `air land` attributes a landing by (status.rs `landings_for`), read here so the gate and
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
    let (batch_green, batch_predates) = if green_at_head {
        (None, None)
    } else {
        let targets: Vec<String> = match bead {
            Some(b) => vec![b.to_string()],
            None => held_beads.clone(),
        };
        super::batch::describe(ledger, repo, &targets)
    };
    let digest_dir = digest_dir(repo);
    let digest = digest_dir.as_deref().and_then(|d| {
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
        let dir = repo.join(d);
        Some(digest_for_bead(
            &dir,
            worker,
            &beads,
            since.as_deref(),
            frontmatter_cutoff(),
            &tracked_in(repo, &dir),
        ))
    });
    // air-ahl: `Untracked` is a refusal like `Missing`, with its own sentence.
    let digest_untracked = digest == Some(Digest::Untracked);
    let digest_present = digest.map(|d| d == Digest::Tracked);
    let runs_at_head = ledger.runs_at(&head, Kind::Verify).unwrap_or((0, 0));
    Ok(GateFacts {
        worker: worker.to_string(),
        head,
        green_at_head,
        tree_green,
        batch_green,
        batch_predates,
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
        digest_dir,
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

/// `digest_dir` from `.claude/air.json`; None disables the check.
pub fn digest_dir(repo: &Path) -> Option<String> {
    air_json(repo)?
        .get("digest_dir")?
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
/// nothing proved it had fired (air-agq, and `docs/research/prose-parsing-survey.md` §3).
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
/// caller. Passed in rather than looked up here so this stays pure over the filesystem: a
/// probe can drive every state without a git repo, and the one `git ls-files` happens once at
/// the call site instead of once per candidate file.
pub fn digest_for_bead(
    dir: &Path,
    worker: &str,
    beads: &[String],
    since: Option<&str>,
    cutoff: jiff::Timestamp,
    tracked: &std::collections::BTreeSet<String>,
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
        if tracked.contains(&e.file_name().to_string_lossy().to_string()) {
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
        "handover",
        &serde_json::json!({"bead": bead, "enforce": enforce, "head": f.head}),
        if v.pass {
            "pass"
        } else if v.block {
            "refuse"
        } else {
            "would-refuse"
        },
        &v.message,
        "4 checks",
    );
    emit(json, &v, || v.message.clone());
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
