//! `air close <id>… --reason "<why>"` — the coordinator's landing pass in ONE bd process.
//!
//! Incident (owner, 2026-08-22): closing ten landed beads took minutes because every close
//! was its own `bd` process, and a bd process costs ~1.4 s here whatever it is asked to do
//! (see `air_bd::stats` for the measurement). bd 1.2.2 already accepts `bd close <id> <id> …
//! --reason <r>` (`bd close --help`, read 2026-08-22); Air never used it. This does, and
//! closes the matching ledger claims in one transaction.
//!
//! Coordinator only. Not a rule for its own sake: the one refusal (`awaiting_review`/close
//! needs a recorded green at HEAD with main merged) lives on the worker's hand-over path, so
//! a worker closing its own bead here would walk around it. The coordinator closes what it
//! has already landed. Removed when `air land` (air-3pz) owns the landing pass and this
//! helper goes with it.

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

/// Who may run `air close`. Pure, so `air selftest` can prove the refusal fires.
/// air-29a: the same guard as `land::may_land`, with the same input, so it had the same hole.
/// `worker` here is now the caller's actual location (`land::where_i_am`), never
/// `worker_name_for(--repo)`. Fixed alongside air-29a rather than filed after it: it is one
/// line of the identical defect, and leaving it would mean the finding was fixed in one of the
/// two places a reader would look.
pub fn may_close(worker: Option<&str>) -> Result<(), String> {
    let Some(worker) = worker else {
        return Err(
            "refused: `air close` cannot tell which checkout it is running in, and the role \
             decides who may close a landing pass (fix: run it from the main checkout)."
                .to_string(),
        );
    };
    if super::hook::role_for(worker) == "coordinator" {
        return Ok(());
    }
    Err(format!(
        "refused: `air close` is the coordinator's landing pass, and {worker} is a worker. \
         Close your own bead with proof instead: `air handover` names anything missing, then \
         `bd close <id> --reason \"<proof>\"` (owner ruling, 2026-08-22). The role comes from \
         where this process runs, so `--repo` does not change it (air-29a)."
    ))
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
    // air-29a: the role is where this process is, not what `--repo` says.
    let here = super::land::where_i_am();
    let inputs = serde_json::json!({"beads": beads, "reason": reason, "caller": here, "repo_worker": worker});
    if let Err(msg) = may_close(here.as_deref()) {
        log_event(
            &ledger,
            here.as_deref().unwrap_or("unknown"),
            "close",
            &inputs,
            "refuse",
            &msg,
            &format!("{} bead(s)", beads.len()),
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
        let msg = match e {
            BdError::Timeout(d) => format!(
                "bd timed out after {} s closing {} bead(s); bd's state is unknown and no claim \
                 was released: run `bd list --status closed --json` and re-run `air close` with \
                 whatever is still open",
                d.as_secs_f64(),
                beads.len()
            ),
            other => format!("bd refused the close; nothing released: {other}"),
        };
        log_event(
            &ledger,
            &worker,
            "close",
            &inputs,
            "bd-refused",
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
        "close",
        &inputs,
        "closed",
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
