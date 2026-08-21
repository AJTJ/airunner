//! `air capture "<text>"`, `air inbox`, `air triage <id> (--bead <id> | --drop "<why>")`.
//!
//! Workers capture; they never file (decisions 2026-08-18/20). Triage is the coordinator's:
//! it creates the bead itself with `bd create --validate … --estimate N` (acceptance is
//! required by the beads template, not here) and then links the capture with `--bead`.

use std::path::Path;

use crate::cmd::{emit, log_event, now, open};

pub fn capture(repo: &Path, text: &str, json: bool) -> i32 {
    let text = text.trim();
    if text.is_empty() {
        eprintln!("air capture: empty text");
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
    if let Err(e) = ledger.capture(&id, &worker, session.as_deref(), text, &at) {
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

pub fn inbox(repo: &Path, json: bool) -> i32 {
    let (ledger, _worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air inbox: {e}");
            return 1;
        }
    };
    let items = match ledger.inbox() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("air inbox: {e}");
            return 1;
        }
    };
    emit(json, &items, || {
        if items.is_empty() {
            return "inbox empty".to_string();
        }
        let mut s = format!("{} open capture(s)\n", items.len());
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

pub fn triage(repo: &Path, id: &str, bead: Option<&str>, drop: Option<&str>, json: bool) -> i32 {
    let (status, bead, note) = match (bead, drop) {
        (Some(b), None) => ("promoted", Some(b), None),
        (None, Some(why)) => ("dropped", None, Some(why)),
        _ => {
            eprintln!("air triage: give exactly one of --bead <id> or --drop \"<why>\"");
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
    let at = now();
    match ledger.resolve_capture(id, status, bead, note, &at) {
        Ok(true) => {
            let msg = match bead {
                Some(b) => format!("{id} promoted to {b}"),
                None => format!("{id} dropped: {}", note.unwrap_or("")),
            };
            let depth = ledger.inbox().map(|v| v.len()).unwrap_or(0);
            log_event(
                &ledger,
                &worker,
                "triage",
                &serde_json::json!({"id": id, "status": status, "bead": bead, "note": note}),
                status,
                &msg,
                &format!("inbox depth {depth}"),
            );
            emit(
                json,
                &serde_json::json!({"ok": true, "id": id, "status": status, "bead": bead}),
                || msg.clone(),
            );
            0
        }
        Ok(false) => {
            eprintln!("air triage: {id} is not an open capture");
            2
        }
        Err(e) => {
            eprintln!("air triage: {e}");
            1
        }
    }
}
