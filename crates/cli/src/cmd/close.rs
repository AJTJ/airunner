//! `air close <id>… --reason "<why>"`: how every role closes a bead, in ONE bd process.
//!
//! Incident (owner, 2026-08-22): closing ten landed beads took minutes because every close
//! was its own `bd` process, and a bd process costs ~1.4 s here whatever it is asked to do
//! (see `air_bd::stats` for the measurement). bd 1.2.2 already accepts `bd close <id> <id> …
//! --reason <r>` (`bd close --help`, read 2026-08-22); Air never used it. This does, and
//! closes the matching ledger claims in one transaction.
//!
//! A worker's or the lane's close is refused here and names `air done` (air-4xj.2): a worker
//! declares its bead done with proof as soon as the work is committed, and the landing that
//! puts that commit on the base branch closes the bead (`done.rs`, `land.rs`). Until
//! 2026-09-26 workers closed with raw `bd close`, and the gate recognised that close by reading
//! the command text; in the 0.4.5 live trial a worker ran a commit and `bd close <id> …` on
//! separate lines of one Bash call, the matcher missed it, and the bead closed with no green
//! containing its commits. From then until air-4xj.2 a worker's `air close` ran the hand-over
//! gate with its green checks, so a bead closed one batch after its work and every dependent
//! waited that batch (the 2026-11-06 pod trial, the lease scenario). The refusal is in the
//! command, whatever the command line looks like; a worker's and the lane's bd writes are
//! denied by their launchers. `AIR_ENFORCE` does not apply: the refusal holds whether or not
//! it is set. Removed when bd can run a check before it closes (a pre-close hook), at which
//! point `air done`'s gate moves there and raw `bd close` is safe again.

use std::path::Path;

use air_bd::{BdError, WorkLedger};

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

/// Whether this caller is refused the close: a worker or the lane, by the launcher's
/// `AIR_ROLE`. The coordinator and the owner close what has already landed; a worker declares
/// done and the landing closes (air-4xj.2).
pub fn gated(role: &str) -> bool {
    super::is_worker_like(role)
}

/// The refusal a worker's or the lane's `air close` gets, naming the command that replaces it,
/// or `None` for a role that closes here. Pure over the role, so the probe needs no repo.
pub fn worker_refusal(role: &str) -> Option<String> {
    if !gated(role) {
        return None;
    }
    Some(format!(
        "refused: a {role} declares done with `air done <id> --reason-file <proof>`; the bead \
         closes when the landing puts that head on the base branch. `air handover --bead <id>` \
         names anything the declaration still needs. The coordinator and the owner close landed \
         work with `air close`."
    ))
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

/// Ids that cannot be a bead id: empty, or containing whitespace (air-a8ra). Pure.
///
/// An adopter's worker passed a bead id and a filename as one quoted argument eight times, and
/// Air ran the whole gate on the string and advised a `Bead:` trailer naming both words.
pub fn malformed_ids(ids: &[String]) -> Vec<String> {
    ids.iter()
        .filter(|id| id.is_empty() || id.contains(char::is_whitespace))
        .cloned()
        .collect()
}

/// Refuse an id that is not a bead id, naming it (air-a8ra). Pure, so it runs before
/// anything else.
pub fn check_shape(ids: &[String]) -> Result<(), String> {
    let bad = malformed_ids(ids);
    if bad.is_empty() {
        return Ok(());
    }
    Err(format!(
        "not a bead id: {}; give each id as its own argument",
        bad.iter()
            .map(|b| format!("\"{b}\""))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Refuse an id bd does not have, naming it (air-a8ra), as `air triage` does (air-76z). One
/// `bd show` under the probe budget, so it is asked only when the gate is about to refuse:
/// that is where an unknown id turned into advice about trailers, and a close that passes
/// stays one bd process (air-869). When bd does not answer, this says nothing and the
/// gate's own refusal stands.
pub fn check_known(repo: &Path, ids: &[String]) -> Result<(), String> {
    let Ok(known) = super::claim::probe_bd(repo).show_all(ids) else {
        return Ok(());
    };
    let missing = super::capture::missing_ids(ids, &known);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("bd has no bead {}", missing.join(", ")))
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
    if let Err(msg) = check_shape(beads) {
        let msg = format!("air close: {msg}; nothing was closed");
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        return 2;
    }
    let role = super::caller_role();
    let inputs =
        serde_json::json!({"beads": beads, "reason": reason, "role": role, "repo_worker": worker});
    // Before anything else (air-4xj.2): a worker's close is not a close, whatever it names.
    if let Some(msg) = worker_refusal(role) {
        log_event(
            &ledger,
            &worker,
            super::decisions::CLOSE_REFUSE,
            &inputs,
            &msg,
            &format!("{} bead(s), role", beads.len()),
        );
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        return 2;
    }
    let actor = std::env::var("BEADS_ACTOR").unwrap_or_default();
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
    let released = match ledger.release_claims_on(beads, "landed", &at) {
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

#[cfg(test)]
mod tests {
    use super::{malformed_ids, worker_refusal};

    /// air-4xj.2: the refusal names `air done`, and only a worker or the lane gets one.
    #[test]
    fn a_worker_and_the_lane_are_refused_naming_air_done_and_the_others_are_not() {
        for role in ["worker", "lane"] {
            let msg = worker_refusal(role).unwrap_or_default();
            assert!(
                msg.contains("`air done <id> --reason-file <proof>`"),
                "{role}: {msg}"
            );
        }
        assert_eq!(worker_refusal("coordinator"), None);
        assert_eq!(worker_refusal("owner"), None);
    }

    /// air-a8ra: a bead id and a filename passed as one argument is not an id.
    #[test]
    fn an_id_with_whitespace_or_nothing_in_it_is_malformed() {
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            malformed_ids(&ids(&["air-1 foo", "air-2", "", "air-3\tx"])),
            ids(&["air-1 foo", "", "air-3\tx"])
        );
        assert!(malformed_ids(&ids(&["air-1", "fd-xxxx"])).is_empty());
    }
}
