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
    let green_at_head = ledger
        .is_green_at(worker, &head, Kind::Verify)
        .map_err(|e| e.to_string())?;
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
    let digest_dir = digest_dir(repo);
    let digest_present = digest_dir.as_deref().map(|d| {
        // Newer than this worker's oldest open claim; with no claim, any digest by this
        // worker counts (the check is about the hand-over, not a specific bead).
        let since = ledger.open_claims().ok().and_then(|v| {
            v.into_iter()
                .filter(|c| c.worker == worker)
                .map(|c| c.claimed_at)
                .min()
        });
        digest_newer_than(&repo.join(d), worker, since.as_deref())
    });
    let runs_at_head = ledger
        .runs_at(worker, &head, Kind::Verify)
        .unwrap_or((0, 0));
    Ok(GateFacts {
        worker: worker.to_string(),
        head,
        green_at_head,
        last_green_sha,
        main_is_ancestor,
        bead_claimed_by_worker,
        bead: bead.map(str::to_string),
        runs_at_head,
        digest_present,
        digest_dir,
        advisory,
    })
}

/// `digest_dir` from `<main>/.claude/air.json`; None disables the check.
pub fn digest_dir(repo: &Path) -> Option<String> {
    let air_dir = air_ledger::paths::air_dir_for(repo).ok()?;
    let path = air_dir.parent()?.join(".claude/air.json");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    v.get("digest_dir")?.as_str().map(str::to_string)
}

/// Is there a `*<worker>*.md` in `dir` modified after `since` (RFC 3339)? Pure over the fs.
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
    if let Some(b) = bead {
        let _ = ledger.stamp_handover(b, &worker, &crate::cmd::now());
    }
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
