//! `air close <id>… --reason "<why>"`: how every role closes a bead, in ONE bd process.
//!
//! Incident (owner, 2026-08-22): closing ten landed beads took minutes because every close
//! was its own `bd` process, and a bd process costs ~1.4 s here whatever it is asked to do
//! (see `air_bd::stats` for the measurement). bd 1.2.2 already accepts `bd close <id> <id> …
//! --reason <r>` (`bd close --help`, read 2026-08-22); Air never used it. This does, and
//! closes the matching ledger claims in one transaction.
//!
//! A worker or the lane closes here too, and for them the close runs the hand-over gate first
//! (owner ruling, 2026-09-26). Until then workers closed with raw `bd close`, and the gate
//! recognised that close by reading the command text. In the 0.4.5 live trial a worker ran a
//! commit and `bd close <id> …` on separate lines of one Bash call; the matcher missed it and
//! the bead closed with no green containing its commits. The same matcher refused a heredoc
//! that only mentioned `bd close`. Here the check runs when the close runs, however the command
//! line is written. The text matcher was deleted the same day: a worker's and the lane's bd
//! writes are denied by their launchers instead.
//!
//! `AIR_ENFORCE` does not apply here: a worker's `air close` refuses on a failed gate whether or
//! not it is set. Advisory mode exists for the text matcher, which can misfire on a command
//! that only looks like a close; this command cannot, and every launched worker and lane has
//! `AIR_ENFORCE=1` anyway, so the two paths refuse the same closes in a launched session.
//! Removed when bd can run a check before it closes (a pre-close hook), at which point the
//! gate moves there and raw `bd close` is safe again.

use std::path::Path;

use air_bd::{BdError, WorkLedger};
use air_hooks::handover_verdict;
use air_ledger::Ledger;

use crate::cmd::{emit, log_event, now, open};

/// Neither route was given (air-lyjr). Names both, because `--reason` is right for the one
/// line that closes an obvious bead and the file route is right for the case that sent you
/// here.
pub const NO_REASON: &str = "air close: no reason. Pass it inline (--reason \"<why>\"), or \
--reason-file <path> for proof too long to survive a command line.";

/// Both routes were given. Air will not guess which one is the reason.
pub const BOTH_REASONS: &str = "air close: pass --reason OR --reason-file <path>, not both. \
Air will not guess which is the reason.";

/// ONE reason across every id, from either route (air-lyjr).
///
/// `air close` takes several beads and `--reason` already applied one string to all of them, so
/// `--reason-file` is the same string read from a file and nothing about the fan-out changes.
/// Stated because the file route invites the other reading — one file per bead — and a reader
/// who assumed it would be closing beads with each other's proof.
///
/// Why this exists at all: the command that demands the longest proof in this repo was the one
/// that refused it. A close here carries a command and its output, and a reason of that length
/// goes through the harness's classifier as a command line and is refused for its shape — the
/// obvious next move being to shorten the proof, which is the failure. Observed on `bd close`
/// while closing air-gazh at ~2,500 characters; bd's own `--reason-file` took the identical
/// text on the next attempt with nothing about it changed. air-45pw is the same defect one
/// command over, on `air capture`.
///
/// Removal: when the harness accepts a several-hundred-word argument.
pub fn resolve_reason(reason: Option<&str>, file: Option<&Path>) -> Result<String, String> {
    super::capture::either(
        reason,
        file,
        "air close",
        "--reason-file",
        BOTH_REASONS,
        NO_REASON,
    )
}

/// Whether this caller's close runs through the hand-over gate: a worker or the lane, by the
/// launcher's `AIR_ROLE`. The coordinator and the owner close what has already landed, and
/// the gate is about a worker's own branch, so it would be checking the wrong tree for them.
pub fn gated(role: &str) -> bool {
    super::is_worker_like(role)
}

/// The hand-over gate over a worker's close, for every bead named: the same facts and the same
/// verdict `air handover` computes, never advisory. `Err` carries the gate's
/// refusal for each bead that fails, one per line, and then nothing is closed. Stamps and clears
/// the hand-over attempt counter, since this is a real attempt.
pub fn gate(ledger: &Ledger, worker: &str, repo: &Path, beads: &[String]) -> Result<(), String> {
    let mut refusals = Vec::new();
    for b in beads {
        let mut f = super::handover::facts(ledger, worker, repo, Some(b), false)?;
        f.refused_command = Some("air close".to_string());
        let v = handover_verdict(&f);
        if v.pass {
            let _ = ledger.clear_handover_attempts(b, worker);
        } else {
            let _ = ledger.stamp_handover(b, worker, &now());
            refusals.push(format!("{b}: {}", v.message));
        }
    }
    if refusals.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{}\nNothing was closed. `air handover --bead <id>` names each check and what it needs.",
            refusals.join("\n")
        ))
    }
}

/// What `air close` says when bd did not close. A timeout names the id count, the budget and
/// the override (air-8lj8): the budget scales with the id count now, so "timed out after 60 s"
/// alone would not say whether 60 s was the right size for this many ids.
pub fn refusal(e: &BdError, ids: usize, overridden: bool) -> String {
    match e {
        BdError::Timeout(d) => format!(
            "bd timed out closing {}; bd's state is unknown and no claim was released: run \
             `bd list --status closed --json` and re-run `air close` with whatever is still open",
            super::claim::budget_words(ids, *d, overridden)
        ),
        other => format!("bd refused the close; nothing released: {other}"),
    }
}

pub fn run(
    repo: &Path,
    beads: &[String],
    reason: Option<&str>,
    reason_file: Option<&Path>,
    json: bool,
) -> i32 {
    if beads.is_empty() {
        eprintln!("air close: name at least one bead");
        return 1;
    }
    // air-lyjr: an unreadable file is an ERROR here, never an empty reason. A bead closed with
    // an empty reason reads afterwards exactly like one nobody wrote proof for.
    let reason = match resolve_reason(reason, reason_file) {
        Ok(r) => r,
        Err(e) => {
            emit(json, &serde_json::json!({"ok": false, "reason": e}), || {
                e.clone()
            });
            return 2;
        }
    };
    let reason = reason.as_str();
    if reason.trim().is_empty() {
        eprintln!("air close: --reason must say why (bd records it on every id)");
        return 1;
    }
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air close: {e}");
            return 1;
        }
    };
    let role = super::caller_role();
    let inputs =
        serde_json::json!({"beads": beads, "reason": reason, "role": role, "repo_worker": worker});
    if gated(role)
        && let Err(msg) = gate(&ledger, &worker, repo, beads)
    {
        log_event(
            &ledger,
            &worker,
            super::decisions::CLOSE_REFUSE,
            &inputs,
            &msg,
            &format!("{} bead(s), 4 checks each", beads.len()),
        );
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        return 2;
    }
    // A worker's close is recorded under its own name when the launcher's actor is missing.
    let actor = match std::env::var("BEADS_ACTOR").unwrap_or_default() {
        a if a.is_empty() && gated(role) => worker.clone(),
        a => a,
    };
    let bd = super::claim::bd_for(repo);
    // One process for every id. A partial failure is bd's to report: it names the id it
    // choked on and nothing is written to the ledger, so a re-run is safe.
    if let Err(e) = bd.close_all(beads, reason, &actor) {
        let msg = refusal(&e, beads.len(), super::claim::bd_overridden());
        log_event(
            &ledger,
            &worker,
            super::decisions::CLOSE_BD_REFUSED,
            &inputs,
            &msg,
            "1 bd process",
        );
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        return 1;
    }
    // One transaction for every claim these beads carried, whoever held them.
    let at = now();
    let why = if gated(role) { "closed" } else { "landed" };
    let released = match ledger.release_claims_on(beads, why, &at) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "air close: bd closed {} bead(s) but the ledger release failed: {e}. Re-run \
                 `air close` (closing an already-closed bead is a no-op for the ledger).",
                beads.len()
            );
            return 1;
        }
    };
    let msg = format!(
        "closed {} bead(s) in 1 bd process ({}); released {} claim(s): {}",
        beads.len(),
        beads.join(" "),
        released.len(),
        if released.is_empty() {
            "-".to_string()
        } else {
            released
                .iter()
                .map(|(b, w)| format!("{b} by {w}"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    log_event(
        &ledger,
        &worker,
        super::decisions::CLOSE_CLOSED,
        &inputs,
        &msg,
        &format!("{} bead(s), 1 bd process", beads.len()),
    );
    emit(
        json,
        &serde_json::json!({
            "ok": true,
            "beads": beads,
            "bd_processes": 1,
            "released": released.iter().map(|(b, w)| serde_json::json!({"bead": b, "worker": w})).collect::<Vec<_>>(),
        }),
        || msg.clone(),
    );
    0
}
