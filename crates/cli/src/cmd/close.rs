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

/// Who may run `air close`. Pure, so `air selftest` can prove the refusal fires.
pub fn may_close(worker: &str) -> Result<(), String> {
    if super::hook::role_for(worker) == "coordinator" {
        return Ok(());
    }
    Err(format!(
        "refused: `air close` is the coordinator's landing pass, and {worker} is a worker. \
         Hand the bead over instead: `air handover` then `bd update <id> -s awaiting_review`."
    ))
}

pub fn run(repo: &Path, beads: &[String], reason: &str, json: bool) -> i32 {
    if beads.is_empty() {
        eprintln!("air close: name at least one bead");
        return 1;
    }
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
    let inputs = serde_json::json!({"beads": beads, "reason": reason});
    if let Err(msg) = may_close(&worker) {
        log_event(
            &ledger,
            &worker,
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
