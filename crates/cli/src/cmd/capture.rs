//! `air capture "<text>"`, `air inbox`, `air triage <id>… (--bead <id> | --drop "<why>")`.
//!
//! Workers capture; they never file (decisions 2026-08-18/20). Triage is the coordinator's:
//! it creates the bead itself with `bd create --validate … --estimate N` (acceptance is
//! required by the beads template, not here) and then links the capture with `--bead`.
//! Air checks that bead exists before it writes the link, and lets a wrong link be
//! corrected afterwards (air-76z).

use std::path::Path;

use air_bd::{BdError, WorkLedger};

use crate::cmd::{emit, log_event, now, open};

pub fn capture(repo: &Path, text: &str, audience: &str, json: bool) -> i32 {
    let text = text.trim();
    if text.is_empty() {
        eprintln!("air capture: empty text");
        return 1;
    }
    if !matches!(audience, "coordinator" | "owner") {
        eprintln!("air capture: --for must be coordinator or owner");
        return 1;
    }
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air capture: {e}");
            return 1;
        }
    };
    let id = air_ledger::verify::new_id();
    let at = now();
    let session = std::env::var("CLAUDE_SESSION_ID").ok();
    if let Err(e) = ledger.capture_for(&id, &worker, session.as_deref(), text, &at, audience) {
        eprintln!("air capture: {e}");
        return 1;
    }
    let depth = ledger.inbox().map(|v| v.len()).unwrap_or(0);
    let msg = format!("captured {id} (inbox depth {depth}); keep working");
    log_event(
        &ledger,
        &worker,
        "capture",
        &serde_json::json!({"id": id, "text": text}),
        "captured",
        &msg,
        &format!("inbox depth {depth}"),
    );
    emit(
        json,
        &serde_json::json!({"ok": true, "id": id, "inbox_depth": depth}),
        || msg.clone(),
    );
    0
}

pub fn inbox(repo: &Path, owner: bool, json: bool) -> i32 {
    let (ledger, _worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air inbox: {e}");
            return 1;
        }
    };
    let audience = if owner { "owner" } else { "coordinator" };
    let items = match ledger.inbox_for(audience) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("air inbox: {e}");
            return 1;
        }
    };
    // The owner's queue is decisions *and* landings: only the owner may merge to main today,
    // and two green hand-overs waited on 2026-08-22 with nothing saying so (air-6p5). Derived
    // from bd plus the ledger every time, never stored twice.
    let landings = if owner {
        super::status::landings_for(repo)
    } else {
        Vec::new()
    };
    emit(
        json,
        &serde_json::json!({"captures": items, "landings": landings}),
        || {
            let mut s = String::new();
            if !landings.is_empty() {
                s.push_str(&format!(
                    "{} landing(s) waiting on the owner\n",
                    landings.len()
                ));
                for l in &landings {
                    s.push_str(&format!(
                        "{}  {}  from {}  ({} min)  {}\n",
                        l.bead,
                        l.head.get(..8).unwrap_or(&l.head),
                        l.worker,
                        l.minutes,
                        l.command
                    ));
                }
            }
            if items.is_empty() {
                if s.is_empty() {
                    return format!("{audience} queue empty");
                }
                return s;
            }
            s.push_str(&format!("{} open capture(s) for {audience}\n", items.len()));
            for c in &items {
                s.push_str(&format!(
                    "{}  {}  {}  {}\n",
                    c.id, c.captured_at, c.worker, c.text
                ));
            }
            s
        },
    );
    0
}

/// One triage decision, resolved from the argv.
#[derive(Debug, PartialEq, Eq)]
pub struct Resolution {
    pub id: String,
    pub status: &'static str,
    pub bead: Option<String>,
    pub note: Option<String>,
}

/// One capture, one resolution (air-zlq, 2026-08-29). Pure, so the mapping is testable
/// without a ledger.
///
/// Batch mode was here: several ids mapped positionally to repeated `--bead`/`--drop`, with
/// one `--drop` allowed to cover every id. It is gone, and the reason is measured rather than
/// assumed.
///
/// The verification is the point of `air triage` — an id bd does not have must refuse the
/// pass (air-76z) — and it runs under a 5 s probe budget. Air was never making serial bd calls
/// for it: `show_all` is one `bd show a b c --json` process (`crates/bd/src/lib.rs:279`). The
/// cost is inside bd, and it is per-id, not per-process. Measured here 2026-08-29:
///
/// | ids | `bd show … --json` |
/// |---|---|
/// | 1 | 1.6 s, 1.8 s |
/// | 2 | 2.4 s, 5.7 s |
/// | 5 | 9.6 s |
/// | 26 | 27.9 s |
///
/// So the ceiling under the budget is about three ids, and batching saved the process — which
/// air-869 measured at ~1.4 s and which was never the cost here. A batch that works for three
/// of thirty-four is a feature whose successful case is indistinguishable from not having it.
///
/// If batching is ever wanted back, the thing to fix is bd's per-id cost, not Air's argv.
pub fn plan(id: &str, bead: Option<&str>, drop: Option<&str>) -> Result<Resolution, String> {
    match (bead, drop) {
        (Some(_), Some(_)) => Err(
            "give --bead <id> or --drop \"<why>\", not both: a capture is promoted or dropped"
                .to_string(),
        ),
        (Some(b), None) => Ok(Resolution {
            id: id.to_string(),
            status: "promoted",
            bead: Some(b.to_string()),
            note: None,
        }),
        (None, Some(w)) => Ok(Resolution {
            id: id.to_string(),
            status: "dropped",
            bead: None,
            note: Some(w.to_string()),
        }),
        (None, None) => Err("give --bead <id> or --drop \"<why>\" for this capture".to_string()),
    }
}

/// Beads the pass would point at that bd does not have. `Err` when bd could not answer at
/// all: the record must not point at an unverified id, so that refuses the pass too
/// (air-76z). One `bd show` process.
fn unknown_beads(repo: &Path, plan: &[Resolution]) -> Result<Vec<String>, String> {
    let want: Vec<String> = plan.iter().filter_map(|r| r.bead.clone()).collect();
    if want.is_empty() {
        return Ok(Vec::new());
    }
    let bd = super::claim::probe_bd(repo);
    let known = match bd.show_all(&want) {
        Ok(v) => v,
        Err(BdError::Timeout(d)) => {
            return Err(format!(
                "bd did not answer in {} s, so no bead was verified and nothing was triaged: \
                 the record must not point at an unverified id. Re-run when bd answers.",
                d.as_secs_f64()
            ));
        }
        Err(e) => return Err(format!("bd show: {e}; nothing was triaged")),
    };
    Ok(missing_ids(&want, &known))
}

/// Which of `want` bd did not return. This comparison IS the check (air-76z): bd omits an id
/// it does not know and still exits 0, so an exit code proves nothing here.
pub fn missing_ids(want: &[String], known: &[air_bd::Issue]) -> Vec<String> {
    want.iter()
        .filter(|w| !known.iter().any(|i| &&i.id == w))
        .cloned()
        .collect()
}

/// Resolve one capture (air-zlq: one at a time, see [`plan`] for the measurement).
///
/// A promotion is verified against bd first (air-76z): `air triage C --bead fd-placeholder`
/// used to succeed before the bead existed, and refusing to touch a resolved capture left
/// the record pointing at nothing with no way to fix it. Now an unknown bead is refused, and
/// an already-triaged capture can be re-pointed, its old target named in the event line.
pub fn triage(repo: &Path, id: &str, bead: Option<&str>, drop: Option<&str>, json: bool) -> i32 {
    let plan = match plan(id, bead, drop) {
        Ok(p) => vec![p],
        Err(e) => {
            eprintln!("air triage: {e}");
            return 1;
        }
    };
    let ids = &[id.to_string()];
    let beads: Vec<String> = bead.into_iter().map(str::to_string).collect();
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air triage: {e}");
            return 1;
        }
    };
    let refuse = |decision: &str, msg: String, denom: &str| -> i32 {
        log_event(
            &ledger,
            &worker,
            "triage",
            &serde_json::json!({"ids": ids, "beads": beads}),
            decision,
            &msg,
            denom,
        );
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        2
    };
    let bead_count = plan.iter().filter(|r| r.bead.is_some()).count();
    match unknown_beads(repo, &plan) {
        Ok(missing) if !missing.is_empty() => {
            return refuse(
                "no-such-bead",
                format!(
                    "refused: bd knows no bead {}; create it first (`bd create --validate \
                     --estimate N`) and re-run. Nothing was triaged.",
                    missing.join(", ")
                ),
                &format!("{bead_count} bead(s) checked in 1 bd process"),
            );
        }
        Ok(_) => {}
        Err(msg) => return refuse("unknown", msg, "1 bd process"),
    }
    let at = now();
    let items: Vec<air_ledger::captures::TriageItem> = plan
        .iter()
        .map(|r| {
            (
                r.id.clone(),
                r.status.to_string(),
                r.bead.clone(),
                r.note.clone(),
            )
        })
        .collect();
    let was = match ledger.resolve_captures(&items, &at) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("air triage: {e}");
            return 1;
        }
    };
    let mut lines = Vec::new();
    let mut repointed = Vec::new();
    let mut missed = Vec::new();
    for (r, prev) in plan.iter().zip(&was) {
        let Some((prev_status, prev_bead)) = prev else {
            missed.push(r.id.clone());
            continue;
        };
        let now_reads = match (&r.bead, &r.note) {
            (Some(b), _) => format!("bead {b}"),
            (None, Some(w)) => format!("dropped: {w}"),
            (None, None) => "dropped".to_string(),
        };
        if prev_status == "open" {
            lines.push(format!("{} -> {now_reads}", r.id));
            continue;
        }
        let from = match prev_bead {
            Some(b) => format!("bead {b}"),
            None => prev_status.clone(),
        };
        repointed.push(serde_json::json!({"id": r.id, "from": from, "to": now_reads}));
        lines.push(format!("{} re-pointed from {from} to {now_reads}", r.id));
    }
    let depth = ledger.inbox().map(|v| v.len()).unwrap_or(0);
    let mut msg = lines.join("\n");
    if !missed.is_empty() {
        if !msg.is_empty() {
            msg.push('\n');
        }
        msg.push_str(&format!("no such capture: {}", missed.join(" ")));
    }
    log_event(
        &ledger,
        &worker,
        "triage",
        &serde_json::json!({
            "ids": ids,
            "resolved": lines.len(),
            "repointed": repointed,
            "missed": missed,
        }),
        if missed.is_empty() {
            "triaged"
        } else {
            "partial"
        },
        &msg,
        &format!(
            "{} capture(s), {bead_count} bead(s) verified, inbox depth {depth}",
            plan.len()
        ),
    );
    emit(
        json,
        &serde_json::json!({
            "ok": missed.is_empty(),
            "resolved": lines.len(),
            "repointed": repointed,
            "missed": missed,
            "inbox_depth": depth,
        }),
        || msg.clone(),
    );
    if missed.is_empty() { 0 } else { 2 }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// air-zlq: one capture, one resolution, and the two ways of giving neither or both are
    /// refused rather than guessed at.
    #[test]
    fn plan_resolves_one_capture_and_refuses_an_ambiguous_pass() {
        let p = plan("c1", Some("fd-1"), None).unwrap();
        assert_eq!((p.status, p.bead.as_deref()), ("promoted", Some("fd-1")));
        let d = plan("c1", None, Some("dup")).unwrap();
        assert_eq!((d.status, d.note.as_deref()), ("dropped", Some("dup")));
        // Promoted or dropped, never both, and never neither.
        assert!(plan("c1", Some("fd-1"), Some("dup")).is_err());
        assert!(plan("c1", None, None).is_err());
    }
}
