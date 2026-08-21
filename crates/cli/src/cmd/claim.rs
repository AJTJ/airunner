//! `air claim <bead> [--files a,b]` and `air release <bead> --reason <r>`.
//!
//! The only claim path (decisions 2026-08-20: "wrap beads, never watch it"). Order is fixed:
//! the ledger refuses first if someone else holds the bead here; then `bd update --claim`
//! (bd's atomic CAS decides races); only after bd succeeds is the ledger row written. If bd
//! fails nothing is written and the bd error is printed. If the ledger write fails after bd
//! succeeded, that is said explicitly so the operator knows which half to repair.

use std::path::Path;

use air_bd::{BdCli, WorkLedger};
use air_ledger::claims::RELEASE_REASONS;

use crate::cmd::{emit, log_event, now, open};

/// Actor string passed to bd: `BEADS_ACTOR` if set, else the worker name.
fn actor_for(worker: &str) -> String {
    std::env::var("BEADS_ACTOR")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| worker.to_string())
}

/// `AIR_BD_BIN` overrides the `bd` binary (tests use a fake).
pub fn bd_for(repo: &Path) -> BdCli {
    let mut bd = BdCli::new(repo);
    if let Ok(bin) = std::env::var("AIR_BD_BIN")
        && !bin.is_empty()
    {
        bd.bin = bin.into();
    }
    bd
}

pub fn claim(repo: &Path, bead: &str, files: &[String], json: bool) -> i32 {
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air claim: {e}");
            return 1;
        }
    };
    // 1. Ledger: is it already held here by someone else?
    match ledger.open_claim(bead) {
        Ok(Some(c)) if c.worker != worker => {
            let msg = format!(
                "refused: {bead} is claimed by {} since {} (fix: ask them, or `air release` from their worktree)",
                c.worker, c.claimed_at
            );
            log_event(
                &ledger,
                &worker,
                "claim",
                &serde_json::json!({"bead": bead}),
                "refuse",
                &msg,
                "1 ledger row",
            );
            emit(
                json,
                &serde_json::json!({"ok": false, "reason": msg}),
                || msg.clone(),
            );
            return 2;
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("air claim: ledger: {e}");
            return 1;
        }
    }
    // 2. bd: the atomic claim.
    let actor = actor_for(&worker);
    if let Err(e) = bd_for(repo).claim(bead, &actor) {
        let msg = format!("bd refused the claim; nothing recorded: {e}");
        log_event(
            &ledger,
            &worker,
            "claim",
            &serde_json::json!({"bead": bead, "actor": actor}),
            "bd-refused",
            &msg,
            "bd exit",
        );
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        return 1;
    }
    // 3. Ledger row, after bd succeeded.
    let at = now();
    if let Err(e) = ledger.record_claim(bead, &worker, files, &at) {
        eprintln!(
            "air claim: bd claim succeeded but the ledger write failed: {e}. Re-run `air claim {bead}` (bd --claim is idempotent for the same actor)."
        );
        return 1;
    }
    let msg = format!("claimed {bead} as {worker} (actor {actor}) at {at}");
    log_event(
        &ledger,
        &worker,
        "claim",
        &serde_json::json!({"bead": bead, "actor": actor, "files": files}),
        "claimed",
        &msg,
        "bd + 1 ledger row",
    );
    emit(
        json,
        &serde_json::json!({"ok": true, "bead": bead, "worker": worker, "claimed_at": at}),
        || msg.clone(),
    );
    0
}

pub fn release(repo: &Path, bead: &str, reason: &str, json: bool) -> i32 {
    if !RELEASE_REASONS.contains(&reason) {
        eprintln!(
            "air release: --reason must be one of {}",
            RELEASE_REASONS.join("|")
        );
        return 1;
    }
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air release: {e}");
            return 1;
        }
    };
    // `landed` is stamped by land; any other reason sends the bead back to open in bd.
    if reason != "landed"
        && let Err(e) = bd_for(repo).set_status(bead, "open")
    {
        let msg = format!("bd refused to reopen {bead}; nothing recorded: {e}");
        log_event(
            &ledger,
            &worker,
            "release",
            &serde_json::json!({"bead": bead, "reason": reason}),
            "bd-refused",
            &msg,
            "bd exit",
        );
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        return 1;
    }
    let at = now();
    match ledger.release_claim(bead, &worker, reason, &at) {
        Ok(true) => {
            let msg = format!("released {bead} ({reason}) at {at}");
            log_event(
                &ledger,
                &worker,
                "release",
                &serde_json::json!({"bead": bead, "reason": reason}),
                "released",
                &msg,
                "1 ledger row",
            );
            emit(
                json,
                &serde_json::json!({"ok": true, "bead": bead, "reason": reason}),
                || msg.clone(),
            );
            0
        }
        Ok(false) => {
            let msg = format!(
                "no open claim on {bead} by {worker} in the ledger (bd status was still set to open)"
            );
            log_event(
                &ledger,
                &worker,
                "release",
                &serde_json::json!({"bead": bead, "reason": reason}),
                "no-claim",
                &msg,
                "0 ledger rows",
            );
            emit(
                json,
                &serde_json::json!({"ok": false, "reason": msg}),
                || msg.clone(),
            );
            2
        }
        Err(e) => {
            eprintln!("air release: ledger: {e}");
            1
        }
    }
}
