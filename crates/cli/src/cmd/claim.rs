//! `air claim <bead> [--files a,b]` and `air release <bead> --reason <r> [--worker <w>]`.
//!
//! The only claim path (decisions 2026-08-20: "wrap beads, never watch it"). Order is fixed:
//! the ledger refuses first if someone else holds the bead here; then `bd show` (a `human`
//! bead is not a worker's to claim; bd's own rule that a pencilled `assignee` blocks every
//! other worker's `--claim` is printed with who is assigned; a closed bead is closed); then
//! `bd update --claim` (bd's atomic CAS decides races); only after bd succeeds is the ledger
//! row written. A bd timeout is not a refusal: bd's state is unknown, so Air re-reads
//! `bd show` and records the claim if it landed, otherwise says so (adopter ad-b68j,
//! ad-wowp, 2026-08-21).
//!
//! Closed is closed (owner, 2026-08-21): `air release` sends back to `open` only a bead bd
//! holds as `in_progress`; it never reopens a closed or handed-over bead. Unfinished work
//! after close is a new bead that references the old one. The coordinator may release a
//! gone peer's claim with `--worker`.

use std::path::Path;

use air_bd::{BdCli, BdError, Issue, WorkLedger};
use air_ledger::claims::RELEASE_REASONS;

use crate::cmd::{emit, log_event, now, open};

/// The label that marks a bead as awaiting the owner; not a worker's to claim (owner,
/// 2026-08-21: one label, `human`).
pub const OWNER_LABEL: &str = "human";

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
    if let Some(ms) = std::env::var("AIR_BD_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        bd.timeout = std::time::Duration::from_millis(ms);
    }
    bd
}

fn timeout_msg(what: &str, bead: &str) -> String {
    format!(
        "bd timed out during {what}; bd's state is unknown and nothing was written to the ledger: run `bd show {bead}` and re-run"
    )
}

/// Print, log, and return the exit code for one refusal or failure.
#[allow(clippy::too_many_arguments)]
fn fail(
    ledger: &air_ledger::Ledger,
    worker: &str,
    cmd: &str,
    inputs: serde_json::Value,
    decision: &str,
    msg: String,
    denom: &str,
    json: bool,
    code: i32,
) -> i32 {
    log_event(ledger, worker, cmd, &inputs, decision, &msg, denom);
    emit(
        json,
        &serde_json::json!({"ok": false, "reason": msg}),
        || msg.clone(),
    );
    code
}

pub fn claim(repo: &Path, bead: &str, files: &[String], json: bool) -> i32 {
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air claim: {e}");
            return 1;
        }
    };
    let inputs = |extra: serde_json::Value| {
        let mut v = serde_json::json!({"bead": bead});
        if let (Some(m), Some(e)) = (v.as_object_mut(), extra.as_object()) {
            for (k, x) in e {
                m.insert(k.clone(), x.clone());
            }
        }
        v
    };
    // 1. Ledger: is it already held here by someone else?
    match ledger.open_claim(bead) {
        Ok(Some(c)) if c.worker != worker => {
            let msg = format!(
                "refused: {bead} is claimed by {} since {} (fix: ask them, or the coordinator runs `air release {bead} --worker {} --reason reassigned`)",
                c.worker, c.claimed_at, c.worker
            );
            return fail(
                &ledger,
                &worker,
                "claim",
                inputs(serde_json::json!({})),
                "refuse",
                msg,
                "1 ledger row",
                json,
                2,
            );
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("air claim: ledger: {e}");
            return 1;
        }
    }
    let actor = actor_for(&worker);
    let bd = bd_for(repo);
    // 2. bd show: facts about the bead before any write.
    match bd.show(bead) {
        Ok(Some(issue)) => {
            if worker != "main" && issue.labels.iter().any(|l| l == OWNER_LABEL) {
                let msg = format!(
                    "refused: {bead} is labelled `{OWNER_LABEL}` (awaiting the owner); not a worker's to claim. Ask in the owner queue: air capture --for owner \"...\""
                );
                return fail(
                    &ledger,
                    &worker,
                    "claim",
                    inputs(serde_json::json!({})),
                    "refuse",
                    msg,
                    "bd show",
                    json,
                    2,
                );
            }
            if issue.status == "closed" {
                let msg = format!(
                    "refused: {bead} is closed; closed is closed. File a new bead that references it."
                );
                return fail(
                    &ledger,
                    &worker,
                    "claim",
                    inputs(serde_json::json!({})),
                    "refuse",
                    msg,
                    "bd show",
                    json,
                    2,
                );
            }
            if let Some(a) = issue
                .assignee
                .as_deref()
                .filter(|a| !a.is_empty() && *a != actor)
            {
                let msg = format!(
                    "refused by bd's rule: {bead} has assignee `{a}`, and in bd 1.2.x a pencilled assignee blocks every other worker's --claim. Either `{a}` claims it, or the coordinator clears the assignee (`bd update {bead} -a \"\"`)."
                );
                return fail(
                    &ledger,
                    &worker,
                    "claim",
                    inputs(serde_json::json!({"assignee": a})),
                    "refuse",
                    msg,
                    "bd show",
                    json,
                    2,
                );
            }
        }
        Ok(None) => {
            let msg = format!("{bead}: bd knows no such bead");
            return fail(
                &ledger,
                &worker,
                "claim",
                inputs(serde_json::json!({})),
                "no-such-bead",
                msg,
                "bd show",
                json,
                1,
            );
        }
        Err(BdError::Timeout(_)) => {
            return fail(
                &ledger,
                &worker,
                "claim",
                inputs(serde_json::json!({})),
                "timeout",
                timeout_msg("show", bead),
                "bd show",
                json,
                1,
            );
        }
        Err(e) => {
            eprintln!("air claim: bd show: {e}");
            return 1;
        }
    }
    // 3. bd: the atomic claim.
    match bd.claim(bead, &actor) {
        Ok(()) => {}
        Err(BdError::Timeout(_)) => {
            // bd may have completed the write after we stopped waiting: re-read before saying
            // anything about state.
            let landed = matches!(
                bd.show(bead),
                Ok(Some(Issue { ref assignee, ref status, .. }))
                    if assignee.as_deref() == Some(actor.as_str()) && status == "in_progress"
            );
            if !landed {
                return fail(
                    &ledger,
                    &worker,
                    "claim",
                    inputs(serde_json::json!({"actor": actor})),
                    "timeout",
                    timeout_msg("--claim", bead),
                    "bd exit",
                    json,
                    1,
                );
            }
            eprintln!(
                "air claim: bd timed out, but `bd show` confirms the claim landed; recording it"
            );
        }
        Err(e) => {
            let msg = format!("bd refused the claim; nothing recorded: {e}");
            return fail(
                &ledger,
                &worker,
                "claim",
                inputs(serde_json::json!({"actor": actor})),
                "bd-refused",
                msg,
                "bd exit",
                json,
                1,
            );
        }
    }
    // 4. Ledger row, after bd succeeded.
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
        &inputs(serde_json::json!({"actor": actor, "files": files})),
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

pub fn release(repo: &Path, bead: &str, reason: &str, as_worker: Option<&str>, json: bool) -> i32 {
    if !RELEASE_REASONS.contains(&reason) {
        eprintln!(
            "air release: --reason must be one of {}",
            RELEASE_REASONS.join("|")
        );
        return 1;
    }
    let (ledger, me) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air release: {e}");
            return 1;
        }
    };
    // The coordinator may release a peer's claim (a gone worker's bead); workers only their own.
    let worker = match as_worker {
        Some(w) if w != me => {
            if me != "main" {
                eprintln!("air release: --worker is for the coordinator (main checkout)");
                return 1;
            }
            w.to_string()
        }
        _ => me.clone(),
    };
    let inputs = serde_json::json!({"bead": bead, "reason": reason, "worker": worker});
    let bd = bd_for(repo);
    // Closed is closed: only a bead bd holds as in_progress goes back to open.
    let status = match bd.show(bead) {
        Ok(Some(i)) => i.status,
        Ok(None) => {
            let msg = format!("{bead}: bd knows no such bead");
            return fail(
                &ledger,
                &me,
                "release",
                inputs,
                "no-such-bead",
                msg,
                "bd show",
                json,
                1,
            );
        }
        Err(BdError::Timeout(_)) => {
            return fail(
                &ledger,
                &me,
                "release",
                inputs,
                "timeout",
                timeout_msg("show", bead),
                "bd show",
                json,
                1,
            );
        }
        Err(e) => {
            eprintln!("air release: bd show: {e}");
            return 1;
        }
    };
    if status == "closed" {
        let msg = format!(
            "refused: {bead} is closed; closed is closed. Unfinished work is a new bead that references {bead}."
        );
        return fail(
            &ledger, &me, "release", inputs, "refuse", msg, "bd show", json, 2,
        );
    }
    if status == "in_progress" && reason != "landed" {
        match bd.set_status(bead, "open") {
            Ok(()) => {}
            Err(BdError::Timeout(_)) => {
                return fail(
                    &ledger,
                    &me,
                    "release",
                    inputs,
                    "timeout",
                    timeout_msg("-s open", bead),
                    "bd exit",
                    json,
                    1,
                );
            }
            Err(e) => {
                let msg = format!("bd refused to reopen {bead}; nothing recorded: {e}");
                return fail(
                    &ledger,
                    &me,
                    "release",
                    inputs,
                    "bd-refused",
                    msg,
                    "bd exit",
                    json,
                    1,
                );
            }
        }
    }
    // Any other status (awaiting_review, open): bd is left alone; only the ledger row closes.
    let at = now();
    match ledger.release_claim(bead, &worker, reason, &at) {
        Ok(true) => {
            let msg = format!(
                "released {bead} held by {worker} ({reason}) at {at}; bd status was {status}"
            );
            log_event(
                &ledger,
                &me,
                "release",
                &inputs,
                "released",
                &msg,
                "1 ledger row",
            );
            emit(
                json,
                &serde_json::json!({"ok": true, "bead": bead, "reason": reason, "worker": worker}),
                || msg.clone(),
            );
            0
        }
        Ok(false) => {
            let msg = format!("no open claim on {bead} by {worker} in the ledger");
            fail(
                &ledger,
                &me,
                "release",
                inputs,
                "no-claim",
                msg,
                "0 ledger rows",
                json,
                2,
            )
        }
        Err(e) => {
            eprintln!("air release: ledger: {e}");
            1
        }
    }
}
