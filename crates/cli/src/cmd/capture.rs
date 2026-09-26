//! `air capture "<text>"`, `air inbox`, `air triage <id>… (--bead <id> | --drop "<why>")`.
//!
//! Workers capture; they never file (decisions 2026-08-18/20). Triage is the coordinator's:
//! it creates the bead itself with `bd create --validate … --estimate N` (acceptance is
//! required by the beads template, not here) and then links the capture with `--bead`.
//! Air checks that bead exists before it writes the link, and lets a wrong link be
//! corrected afterwards (air-76z).
//!
//! One inbox, one audience (air-uef; owner, 2026-09-05). `air capture --for owner` and
//! `air inbox --owner` are gone: the owner's queue is beads labelled `owner`, which the
//! coordinator files with its recommendation in the description, and `air claim` refuses to
//! workers. Two queues reached the owner before, and the prose one carried no id, no
//! acceptance and no recommendation: ten items sat six days in the adopter's 2026-08-29 round,
//! and here a capture sat a week for a bead that already existed and was already labelled.

use std::path::Path;

use air_bd::{BdError, WorkLedger};

use crate::cmd::{emit, log_event, now, open};

/// What `--for owner` is told (air-uef). The flag survives only so this sentence can be
/// said; removal: when a release has passed with no such refusal in any ledger, the flag
/// goes and clap's own error is enough.
pub const FOR_OWNER_REFUSAL: &str = "air capture: `--for owner` is gone (air-uef, owner ruling \
2026-09-05). Capture the question plainly: air capture \"<text>\". The coordinator files it as \
a bead labelled `owner`, with a recommendation, and those beads are the owner's queue.";

/// Neither route was given. Names both, because the positional is right for the one-liners
/// that are most captures and the file route is right for the case that sent you here.
pub const NOTHING_TO_CAPTURE: &str = "air capture: nothing to capture. Pass the text \
(air capture \"<text>\"), or --file <path> for a finding too long to survive a command line.";

/// Both routes were given. Air will not guess which one is the finding.
pub const BOTH_ROUTES: &str = "air capture: pass the text OR --file <path>, not both. Air will \
not guess which is the capture.";

/// air-45pw: `air capture` took one positional and nothing else, so a finding long enough to be
/// worth writing went through the harness's command classifier as a command line and was
/// refused for its shape. An adopter's worker shortened a finding in order to file it, and a
/// shortened capture looks exactly like a capture — the loss is invisible, which is why this is
/// a file route rather than a longer allowance.
///
/// This fleet never hit it: workers here capture through the MCP tool, whose argument is JSON
/// and never becomes a command line, so every capture triaged on 2026-09-06 ran to several
/// hundred words and none was refused. The CLI path is the one that bites and our own usage
/// pattern hides it completely.
///
/// The file's bytes are stored as they are, save for leading and trailing whitespace, which is
/// trimmed exactly as the positional route trims it — a trailing newline is how a file ends,
/// not something the person wrote. Nothing inside the text is touched: no line limit, no byte
/// limit, no truncation anywhere on this path.
///
/// Removal: when the harness accepts a several-hundred-word argument, or when no adopter files
/// through the CLI.
pub fn resolve_text(text: Option<&str>, file: Option<&Path>) -> Result<String, String> {
    either(
        text,
        file,
        "air capture",
        "--file",
        BOTH_ROUTES,
        NOTHING_TO_CAPTURE,
    )
}

/// The same rule for any command with an inline route and a file route (air-lyjr).
///
/// Extracted rather than copied when `air close` needed it. The reason is the third clause: a
/// file that cannot be read is an ERROR and never an empty value, and that is the branch a
/// second implementation gets wrong quietly — a missing path recorded as an empty reason closes
/// the bead with no proof, and reads afterwards exactly like a close nobody wrote a reason for.
/// One implementation means one place where that is true.
///
/// Trimming matches the inline route: a trailing newline is how a file ends, not something the
/// person wrote. Nothing inside the text is touched — no line limit, no byte limit, no
/// truncation anywhere on this path.
pub fn either(
    inline: Option<&str>,
    file: Option<&Path>,
    cmd: &str,
    flag: &str,
    both: &str,
    neither: &str,
) -> Result<String, String> {
    match (inline, file) {
        (Some(_), Some(_)) => Err(both.to_string()),
        (None, None) => Err(neither.to_string()),
        (Some(t), None) => Ok(t.trim().to_string()),
        (None, Some(p)) => match std::fs::read_to_string(p) {
            Ok(s) => Ok(s.trim().to_string()),
            Err(e) => Err(format!("{cmd}: cannot read {flag} {}: {e}", p.display())),
        },
    }
}

/// Where this capture is being written (air-6dj4), looked up once, here.
///
/// **In the CLI command, never on a hook path.** `air capture` is a CLI command and stays one;
/// nothing in `air hook` calls it, and this git call must not become the first thing that does.
/// The hooks budget ~100 ms and fail open, and a `rev-parse` per hook invocation is the kind of
/// cost that arrives invisibly (air-cmn, air-bp0 removed exactly this shape from the status
/// poll).
///
/// An absence is RECORDED with its reason rather than left empty: no repo, an unborn branch, a
/// git that did not answer. An empty sha and "there is no head" must not read alike, which is
/// the branch this could get wrong quietly — a capture with `head_sha = ""` would render as a
/// head nobody can look up, and nothing downstream would ever raise it.
pub(crate) fn head_now(repo: &Path) -> Option<air_ledger::captures::Head> {
    Some(match crate::git::head(repo) {
        Ok(sha) if !sha.trim().is_empty() => air_ledger::captures::Head::At(sha.trim().to_string()),
        // git answered with nothing: an unborn branch (`git init` with no commit) reaches here.
        Ok(_) => air_ledger::captures::Head::Absent("git named no commit (unborn branch?)".into()),
        Err(e) => air_ledger::captures::Head::Absent(format!("git rev-parse HEAD: {e}")),
    })
}

pub fn capture(
    repo: &Path,
    text: Option<&str>,
    file: Option<&Path>,
    audience: &str,
    json: bool,
) -> i32 {
    let text = match resolve_text(text, file) {
        Ok(t) => t,
        Err(e) => {
            emit(json, &serde_json::json!({"ok": false, "reason": e}), || {
                e.clone()
            });
            return 2;
        }
    };
    let text = text.as_str();
    if text.is_empty() {
        eprintln!("air capture: empty text");
        return 1;
    }
    if audience != "coordinator" {
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": FOR_OWNER_REFUSAL}),
            || FOR_OWNER_REFUSAL.to_string(),
        );
        return 2;
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
    let head = head_now(repo);
    if let Err(e) = ledger.capture(&id, &worker, session.as_deref(), text, &at, head.as_ref()) {
        eprintln!("air capture: {e}");
        return 1;
    }
    // air-1vri.5: the coordinator hears it now rather than on its next wake.
    super::fanout::capture_to_coordinator(&ledger, &worker, &id, text, &at);
    let depth = ledger.inbox().map(|v| v.len()).unwrap_or(0);
    let msg = format!("captured {id} (inbox depth {depth}); keep working");
    log_event(
        &ledger,
        &worker,
        super::decisions::CAPTURE_CAPTURED,
        &serde_json::json!({"id": id, "text": text}),
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

/// Where a capture was written, for the inbox row (air-6dj4).
///
/// **Three states, three renderings, and that is the point of the bead.** A sha is shown short;
/// an absence Air observed says so WITH its reason; and a row from before v21 says nothing at
/// all, because Air never looked and printing "no head" there would assert something nobody
/// established. An empty sha would render as a commit that cannot be found and nothing
/// downstream would ever raise it — the failure looking exactly like the success.
pub(crate) fn where_written(head: Option<&air_ledger::captures::Head>) -> String {
    use air_ledger::captures::Head;
    match head {
        Some(Head::At(sha)) => format!(" at {}", sha.get(..8).unwrap_or(sha)),
        Some(Head::Absent(why)) => format!(" [no head: {why}]"),
        None => String::new(),
    }
}

pub fn inbox(repo: &Path, json: bool) -> i32 {
    let (ledger, _worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air inbox: {e}");
            return 1;
        }
    };
    // Every open capture, whatever audience an older binary wrote it with (air-uef).
    let items = match ledger.inbox() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("air inbox: {e}");
            return 1;
        }
    };
    emit(json, &serde_json::json!({"captures": items}), || {
        if items.is_empty() {
            return "inbox empty".to_string();
        }
        let mut s = format!("{} open capture(s)\n", items.len());
        for c in &items {
            s.push_str(&format!(
                "{}  {}  {}{}  {}\n",
                c.id,
                c.captured_at,
                c.worker,
                where_written(c.head.as_ref()),
                c.text
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
/// A promotion is verified against bd first (air-76z): `air triage C --bead zz-placeholder`
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
    let refuse = |decision: super::decisions::Trace, msg: String, denom: &str| -> i32 {
        log_event(
            &ledger,
            &worker,
            decision,
            &serde_json::json!({"ids": ids, "beads": beads}),
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
                super::decisions::TRIAGE_NO_SUCH_BEAD,
                format!(
                    "refused: bd knows no bead {}; create it first (`bd create --validate \
                     --estimate N`) and re-run. Nothing was triaged.",
                    missing.join(", ")
                ),
                &format!("{bead_count} bead(s) checked in 1 bd process"),
            );
        }
        Ok(_) => {}
        Err(msg) => return refuse(super::decisions::TRIAGE_UNKNOWN, msg, "1 bd process"),
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
        if missed.is_empty() {
            super::decisions::TRIAGE_TRIAGED
        } else {
            super::decisions::TRIAGE_PARTIAL
        },
        &serde_json::json!({
            "ids": ids,
            "resolved": lines.len(),
            "repointed": repointed,
            "missed": missed,
        }),
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
        let p = plan("c1", Some("zz-1"), None).unwrap();
        assert_eq!((p.status, p.bead.as_deref()), ("promoted", Some("zz-1")));
        let d = plan("c1", None, Some("dup")).unwrap();
        assert_eq!((d.status, d.note.as_deref()), ("dropped", Some("dup")));
        // Promoted or dropped, never both, and never neither.
        assert!(plan("c1", Some("zz-1"), Some("dup")).is_err());
        assert!(plan("c1", None, None).is_err());
    }

    /// air-45pw: the same either/or as triage, one command earlier. Both refusals name both
    /// routes, because the person reading one has just had a capture refused and needs the
    /// other way in — `--help` is the second place they look, not the first.
    #[test]
    fn capture_takes_one_route_and_each_refusal_names_both() {
        let dir = std::env::temp_dir().join(format!("air-cap-{}", air_ledger::verify::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("finding.md");
        // A trailing newline is how a file ends, not something the person wrote; interior
        // blank lines and whitespace are content and survive untouched.
        std::fs::write(&f, "  first\n\n   indented second\n").unwrap();

        assert_eq!(
            resolve_text(None, Some(&f)).unwrap(),
            "first\n\n   indented second"
        );
        assert_eq!(resolve_text(Some("  a line  "), None).unwrap(), "a line");

        for e in [
            resolve_text(None, None).unwrap_err(),
            resolve_text(Some("a line"), Some(&f)).unwrap_err(),
        ] {
            assert!(e.contains("--file"), "{e}");
            assert!(e.contains("text"), "{e}");
        }

        // A path that is not there is named, not silently filed as an empty capture.
        let missing = resolve_text(None, Some(&dir.join("nope.md"))).unwrap_err();
        assert!(missing.contains("nope.md"), "{missing}");

        std::fs::remove_dir_all(&dir).ok();
    }
}
