//! `air handover [--bead X] [--enforce]`: gather facts, apply the pure gate.

use std::path::Path;

use air_hooks::{GateFacts, Verdict, handover_verdict};
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
    let bead_claimed_by_worker = match bead {
        None => true, // no bead named: the claim check is not applicable
        Some(b) => ledger
            .conn()
            .query_row(
                "SELECT count(*) FROM claims WHERE bead=?1 AND worker=?2 AND released_at IS NULL",
                rusqlite::params![b, worker],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false),
    };
    // Every bead this worker holds, once: the digest lookup reads it and the refusal names it
    // (air-xbl: it used to be computed for the lookup and thrown away before the message).
    let held: Vec<air_ledger::claims::Claim> = ledger
        .open_claims()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.worker == worker)
        .collect();
    let held_beads: Vec<String> = held.iter().map(|c| c.bead.clone()).collect();
    let digest_dir = digest_dir(repo);
    let digest_present = digest_dir.as_deref().and_then(|d| {
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
        // The bead named on the command line, else every bead this worker still holds: the
        // check is "did you write the digest for the work you are handing on". No bead at
        // all means nothing to declare, and the check is skipped rather than failed.
        let beads = digest_beads(bead, &held_beads)?;
        Some(digest_for_bead(
            &repo.join(d),
            worker,
            &beads,
            since.as_deref(),
            frontmatter_cutoff(),
        ))
    });
    let runs_at_head = ledger.runs_at(&head, Kind::Verify).unwrap_or((0, 0));
    Ok(GateFacts {
        worker: worker.to_string(),
        head,
        green_at_head,
        tree_green,
        last_green_sha,
        main_is_ancestor,
        bead_claimed_by_worker,
        bead: bead.map(str::to_string),
        held_beads,
        runs_at_head,
        digest_present,
        digest_dir,
        advisory,
    })
}

/// Which beads the digest check looks for (air-xbl): the bead named to the gate, else every
/// bead the worker holds; `None` when there is neither, which SKIPS the check.
///
/// Before this the no-claim case built an empty list, `digest_for_bead` matched nothing
/// against it, and the gate refused every hand-over from a worktree holding no claim, with a
/// fix naming a literal `<bead>`. The comment above it said any digest by the worker would
/// count. Neither was right: a digest declares a bead, and with no bead there is nothing for
/// it to declare. adopter's batching lane (w4: claims nothing, merges other workers' green
/// shas, runs the full verify once) is the case; its work is those workers' beads, each with
/// its own digest, and its own hand-over has no bead of its own. A worker that DOES hold a
/// claim, or names a bead, is unchanged: it must still declare it.
pub fn digest_beads(named: Option<&str>, held: &[String]) -> Option<Vec<String>> {
    match named {
        Some(b) => Some(vec![b.to_string()]),
        None if held.is_empty() => None,
        None => Some(held.to_vec()),
    }
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
pub fn digest_for_bead(
    dir: &Path,
    worker: &str,
    beads: &[String],
    since: Option<&str>,
    cutoff: jiff::Timestamp,
) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    let since_ts: Option<jiff::Timestamp> = since.and_then(|s| s.parse().ok());
    rd.flatten().any(|e| {
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
    })
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
