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
    emit(json, &items, || {
        if items.is_empty() {
            return format!("{audience} queue empty");
        }
        let mut s = format!("{} open capture(s) for {audience}\n", items.len());
        for c in &items {
            s.push_str(&format!(
                "{}  {}  {}  {}\n",
                c.id, c.captured_at, c.worker, c.text
            ));
        }
        s
    });
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

/// Map ids to `--bead`/`--drop` the way bd 1.2.2 maps `bd close`'s `--reason`: positionally,
/// in the order the flags appear, with one value allowed to cover every id (`bd close
/// --help`, read 2026-08-22). Pure, so the mapping is testable without a ledger (air-869).
pub fn plan(ids: &[String], beads: &[String], drops: &[String]) -> Result<Vec<Resolution>, String> {
    if ids.is_empty() {
        return Err("name at least one capture".to_string());
    }
    let given = beads.len().saturating_add(drops.len());
    if given == 0 {
        return Err("give --bead <id> or --drop \"<why>\" for each capture".to_string());
    }
    // One --drop for many ids is a real pass ("all duplicates"); one --bead for many is not,
    // because a bead belongs to one capture.
    if given == 1 && ids.len() > 1 {
        if beads.len() == 1 {
            return Err(format!(
                "{} captures but one --bead: a bead is one capture's, so repeat --bead once per capture",
                ids.len()
            ));
        }
        return Ok(ids
            .iter()
            .map(|id| Resolution {
                id: id.clone(),
                status: "dropped",
                bead: None,
                note: drops.first().cloned(),
            })
            .collect());
    }
    if given != ids.len() {
        return Err(format!(
            "{} capture(s) but {given} --bead/--drop value(s): they map positionally, so give one per capture",
            ids.len()
        ));
    }
    // Interleaving is lost by clap, so beads come first, then drops. Say so rather than
    // silently pairing the wrong ones.
    let mut out = Vec::with_capacity(ids.len());
    for (i, id) in ids.iter().enumerate() {
        let r = match beads.get(i) {
            Some(b) => Resolution {
                id: id.clone(),
                status: "promoted",
                bead: Some(b.clone()),
                note: None,
            },
            None => Resolution {
                id: id.clone(),
                status: "dropped",
                bead: None,
                note: drops.get(i.saturating_sub(beads.len())).cloned(),
            },
        };
        out.push(r);
    }
    Ok(out)
}

/// Beads the pass would point at that bd does not have. `Err` when bd could not answer at
/// all: the record must not point at an unverified id, so that refuses the pass too
/// (air-76z). One `bd show` process for every bead in the pass (air-869).
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

/// Resolve one capture or a whole pass. Every capture in one ledger transaction and one
/// event line: the post-round pass triaged a dozen one `air triage` at a time (air-869).
///
/// A promotion is verified against bd first (air-76z): `air triage C --bead fd-placeholder`
/// used to succeed before the bead existed, and refusing to touch a resolved capture left
/// the record pointing at nothing with no way to fix it. Now an unknown bead is refused, and
/// an already-triaged capture can be re-pointed, its old target named in the event line.
pub fn triage(repo: &Path, ids: &[String], beads: &[String], drops: &[String], json: bool) -> i32 {
    let plan = match plan(ids, beads, drops) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("air triage: {e}");
            return 1;
        }
    };
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

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn plan_maps_positionally_and_refuses_a_mismatch() {
        let p = plan(&v(&["a", "b", "c"]), &v(&["fd-1", "fd-2"]), &v(&["dup"])).unwrap();
        assert_eq!(p[0].bead.as_deref(), Some("fd-1"));
        assert_eq!(p[1].bead.as_deref(), Some("fd-2"));
        assert_eq!(
            (p[2].status, p[2].note.as_deref()),
            ("dropped", Some("dup"))
        );
        // One --drop covers every capture; one --bead cannot.
        let all = plan(&v(&["a", "b"]), &[], &v(&["dup"])).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().all(|r| r.status == "dropped"));
        assert!(plan(&v(&["a", "b"]), &v(&["fd-1"]), &[]).is_err());
        assert!(plan(&v(&["a", "b", "c"]), &v(&["fd-1", "fd-2"]), &[]).is_err());
        assert!(plan(&v(&["a"]), &[], &[]).is_err());
        assert!(plan(&[], &v(&["fd-1"]), &[]).is_err());
    }
}
