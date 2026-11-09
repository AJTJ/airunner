//! `air done <id>… --reason-file <proof>`: a worker or the lane declares a bead done with
//! proof and no green, and the landing closes it (air-4xj.1).
//!
//! Under a verify lane a worker closes on the lane's batch green, which arrives one batch
//! after the work. Every bead that depends on it is held back by that batch, and the 0.4.9
//! trial waited at a dependent for the width of one batch for no reason the work gave. Here
//! the worker says "done" as soon as the work is committed; the gate checks what it can check
//! without a green (the bead's commit, the claim, a declared precheck), the proof goes on the
//! bead as a comment, the head the bead was judged at goes on the claim, and `air land` closes
//! the bead once the base branch contains that head. A bead already on the base branch when
//! it is declared is closed here, since no later landing will carry it.
//!
//! The claim stays open: it is the row the landing reads, and a done bead is still held for
//! `batch_ready_rule`. `air close` keeps working for a worker in this release; a later bead
//! refuses it in favour of this.

use std::path::Path;

use air_bd::WorkLedger;
use air_hooks::handover_verdict;
use air_ledger::Ledger;

use crate::cmd::{emit, log_event, now, open};
use crate::git;

pub const NO_REASON: &str = "air done: no proof. Pass it inline (--reason \"<proof>\"), or \
--reason-file <path> for proof too long to survive a command line.";

pub const BOTH_REASONS: &str = "air done: pass --reason OR --reason-file <path>, not both. \
Air will not guess which is the proof.";

/// ONE proof across every id, from either route, as `air close` takes its reason.
pub fn resolve_reason(reason: Option<&str>, file: Option<&Path>) -> Result<String, String> {
    super::capture::either(
        reason,
        file,
        "air done",
        "--reason-file",
        BOTH_REASONS,
        NO_REASON,
    )
}

/// One bead the gate let through: the head the landing waits for, and whether the base
/// branch already contains every commit naming the bead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passed {
    pub bead: String,
    /// The newest commit in `base..HEAD` carrying the bead's trailer, or, when there is none,
    /// the newest commit on the base branch carrying it. The bead's own commit, never the
    /// branch head: a worker that has already committed its next bead on top is declaring
    /// the earlier one, and the lane cut at the earlier one's sha (0.4.7 trial, D1).
    pub head: String,
    pub landed: bool,
}

/// The hand-over gate minus its green checks, for every bead named, with the worker's own
/// open claim as a further condition: the declaration is written on that claim, and a bead
/// carried by trailer alone has no row for the landing to read. `Err` carries one refusal per
/// failing bead and nothing is written. Stamps and clears the hand-over attempt counter as
/// `close::gate` does, since this is a real attempt.
pub fn gate(
    ledger: &Ledger,
    worker: &str,
    repo: &Path,
    beads: &[String],
) -> Result<Vec<Passed>, String> {
    let mut refusals = Vec::new();
    let mut passed = Vec::new();
    for b in beads {
        let held = ledger
            .open_claim(b)
            .map_err(|e| e.to_string())?
            .is_some_and(|c| c.worker == worker);
        if !held {
            refusals.push(format!(
                "{b}: {worker} holds no open claim on it, and `air done` records the \
                 declaration on your claim; `air claim {b}` first, or leave it to its holder"
            ));
            continue;
        }
        let mut f = super::handover::facts(ledger, worker, repo, Some(b), false)?;
        f.refused_command = Some("air done".to_string());
        f.require_green = false;
        let v = handover_verdict(&f);
        if v.pass {
            let _ = ledger.clear_handover_attempts(b, worker);
            let mine = super::batch::bead_commits(repo, b);
            let (head, landed) = match mine.first() {
                Some(c) => (c.sha.clone(), false),
                // The gate passed `bead-commit`, so a bead with nothing in the range has a
                // commit on the base branch.
                None => (super::batch::in_main(repo, b).unwrap_or(f.head), true),
            };
            passed.push(Passed {
                bead: b.clone(),
                head,
                landed,
            });
        } else {
            let _ = ledger.stamp_handover(b, worker, &now());
            refusals.push(format!("{b}: {}", v.message));
        }
    }
    if refusals.is_empty() {
        Ok(passed)
    } else {
        Err(format!(
            "{}\nNothing was recorded. `air handover --bead <id>` names each check and what it needs.",
            refusals.join("\n")
        ))
    }
}

/// Who may declare: a worker or the lane, and the owner's own shell. The coordinator does
/// not implement, and closes landed work with `air close`.
pub fn may_declare(role: &str) -> Result<(), String> {
    if super::is_worker_like(role) || role == "owner" {
        Ok(())
    } else {
        Err(format!(
            "air done is a worker's or the lane's declaration; the {role} closes landed work \
             with `air close`"
        ))
    }
}

/// The comment `air done` leaves on the bead: the head judged and the proof, which is what
/// the landing's close reason quotes.
pub fn comment_text(head: &str, proof: &str) -> String {
    format!("done at {head}: {proof}")
}

pub fn run(
    repo: &Path,
    beads: &[String],
    reason: Option<&str>,
    reason_file: Option<&Path>,
    json: bool,
) -> i32 {
    if beads.is_empty() {
        eprintln!("air done: name at least one bead");
        return 1;
    }
    let reason = match resolve_reason(reason, reason_file) {
        Ok(r) => r,
        Err(e) => {
            emit(json, &serde_json::json!({"ok": false, "reason": e}), || {
                e.clone()
            });
            return 2;
        }
    };
    let proof = reason.trim();
    if proof.is_empty() {
        eprintln!("air done: the proof must say what was checked (bd records it on every id)");
        return 1;
    }
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air done: {e}");
            return 1;
        }
    };
    if let Err(msg) = super::close::check_shape(beads) {
        let msg = format!("air done: {msg}; nothing was recorded");
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        return 2;
    }
    let role = super::caller_role();
    let inputs =
        serde_json::json!({"beads": beads, "reason": proof, "role": role, "repo_worker": worker});
    let refuse = |msg: String| -> i32 {
        log_event(
            &ledger,
            &worker,
            super::decisions::DONE_REFUSE,
            &inputs,
            &msg,
            &format!("{} bead(s), 3 checks each", beads.len()),
        );
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        2
    };
    if let Err(msg) = may_declare(role) {
        return refuse(msg);
    }
    let passed = match gate(&ledger, &worker, repo, beads) {
        Ok(p) => p,
        Err(msg) => {
            // An id bd does not have is the refusal, not whatever the gate made of it.
            return refuse(match super::close::check_known(repo, beads) {
                Err(unknown) => format!("air done: {unknown}; nothing was recorded"),
                Ok(()) => msg,
            });
        }
    };
    let actor = match std::env::var("BEADS_ACTOR").unwrap_or_default() {
        a if a.is_empty() => worker.clone(),
        a => a,
    };
    let bd = super::claim::bd_for(repo);
    let at = now();
    let mut lines = Vec::new();
    // One `bd comment` per id, then the claim, so a bead that is on the bead is on the claim.
    for p in &passed {
        if let Err(e) = bd.comment(&p.bead, &comment_text(&p.head, proof)) {
            let msg = format!(
                "air done: bd refused the comment on {}; nothing recorded for it or any bead \
                 after it, re-run for those: {e}",
                p.bead
            );
            log_event(
                &ledger,
                &worker,
                super::decisions::DONE_BD_REFUSED,
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
        if let Err(e) = ledger.mark_done(&p.bead, &worker, &p.head, proof, &at) {
            eprintln!(
                "air done: the comment is on {} but the ledger write failed: {e}. Re-run.",
                p.bead
            );
            return 1;
        }
    }
    let base = super::handover::base_branch(repo);
    // A bead the base branch already holds has no landing ahead of it to close it.
    let landed: Vec<String> = passed
        .iter()
        .filter(|p| p.landed)
        .map(|p| p.bead.clone())
        .collect();
    let waiting: Vec<&Passed> = passed.iter().filter(|p| !p.landed).collect();
    for p in &waiting {
        lines.push(format!(
            "done: {} at {} (proof on the bead); `air land` closes it once {base} contains \
             that commit",
            p.bead,
            short(&p.head)
        ));
    }
    if !landed.is_empty() {
        let main_sha = git::run(repo, &["rev-parse", &base]).unwrap_or_default();
        let why = format!("landed at {}; {proof}", short(&main_sha));
        // The worker closes its own bead here, and bd's assignee is this actor: no `--force`
        // (air-52u: the force is the landing's).
        if let Err(e) = bd.close_all(&landed, &why, &actor, false) {
            let msg = format!(
                "air done: {} already on {base}, and bd refused the close: {e}; the done \
                 declaration is recorded, re-run to close",
                landed.join(" ")
            );
            log_event(
                &ledger,
                &worker,
                super::decisions::DONE_BD_REFUSED,
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
        let _ = ledger.release_claims_on(&landed, "landed", &at);
        lines.push(format!(
            "closed: {} (already on {base} at {})",
            landed.join(" "),
            short(&main_sha)
        ));
    }
    let msg = lines.join("\n");
    log_event(
        &ledger,
        &worker,
        super::decisions::DONE_DONE,
        &inputs,
        &msg,
        &format!(
            "{} bead(s), {} bd process(es)",
            passed.len(),
            passed.len().saturating_add(usize::from(!landed.is_empty()))
        ),
    );
    emit(
        json,
        &serde_json::json!({
            "ok": true,
            "done": waiting.iter().map(|p| serde_json::json!({"bead": p.bead, "head": p.head})).collect::<Vec<_>>(),
            "closed": landed,
        }),
        || msg.clone(),
    );
    0
}

fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

#[cfg(test)]
mod tests {
    use super::{comment_text, may_declare};

    #[test]
    fn a_worker_the_lane_and_the_owner_may_declare_and_the_coordinator_may_not() {
        assert!(may_declare("worker").is_ok());
        assert!(may_declare("lane").is_ok());
        assert!(may_declare("owner").is_ok());
        assert!(may_declare("coordinator").is_err());
        assert_eq!(comment_text("abc", "it works"), "done at abc: it works");
    }
}
