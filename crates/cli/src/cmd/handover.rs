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
    Ok(GateFacts {
        worker: worker.to_string(),
        head,
        green_at_head,
        last_green_sha,
        main_is_ancestor,
        bead_claimed_by_worker,
        bead: bead.map(str::to_string),
        advisory,
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
        "3 checks",
    );
    emit(json, &v, || v.message.clone());
    if v.block { 2 } else { 0 }
}
