//! `air claim <bead> [--files a,b]` and `air release <bead> --reason <r> [--worker <w>]`.
//!
//! The only claim path (decisions 2026-08-20: "wrap beads, never watch it"). Order is fixed:
//! the ledger refuses first if someone else holds the bead here; then `bd show` (an `owner`
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
/// 2026-08-21: one label).
///
/// The word is `owner`, not `human` (owner, 2026-08-22). Two words, two meanings, kept
/// apart: `human` is about PRESENCE, a person in the loop who can watch and type into every
/// session, and `owner` is about AUTHORITY, whose decision is required. An owner-only
/// decision stays owner-only when the owner hands it to an agent, so `human` was the wrong
/// word for a gate. It rotted exactly that way in adopter, where a triage note records a
/// whole category of beads that "carries human but needs no owner ruling" (its
/// `docs/research/adopter-notes/notes/human-queue-triage.md`, category C). Every site that
/// decides claimability reads this constant, never a literal (air-5hw).
pub const OWNER_LABEL: &str = "owner";

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

/// Put the bead on the worker's tmux window so `tmux ls` and `air status` show the lane and
/// what it is doing now; `bead` empty clears it back to the worker name (air-5lg). A no-op
/// when the worker has no tmux session, which includes every `air claim` typed by a person in
/// their own terminal.
fn label_window(repo: &Path, worker: &str, bead: &str, title: &str) {
    let label = match (bead, title.trim()) {
        ("", _) => String::new(),
        (b, "") => b.to_string(),
        (b, t) => format!("{b} {t}"),
    };
    super::tmux::set_window_label(&super::tmux::project_prefix(repo), worker, &label);
}

/// The earlier of a bd timestamp (if it parses) and `now`; never later than `now`.
fn earliest(bd_time: Option<&str>, now: &str) -> String {
    match bd_time.and_then(|t| t.parse::<jiff::Timestamp>().ok()) {
        Some(t) if now.parse::<jiff::Timestamp>().is_ok_and(|n| t < n) => t.to_string(),
        _ => now.to_string(),
    }
}

/// A short-budget bd for a read that must not hold a command up: `AIR_BD_PROBE_TIMEOUT_MS`
/// (default 5000), never longer than the main timeout.
pub fn probe_bd(repo: &Path) -> BdCli {
    let bd = bd_for(repo);
    let ms = std::env::var("AIR_BD_PROBE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000);
    BdCli {
        timeout: std::time::Duration::from_millis(ms).min(bd.timeout),
        ..bd
    }
}

/// After a `--claim` timeout: did bd's write land? Probes `bd show` with a short separate
/// timeout (`AIR_BD_PROBE_TIMEOUT_MS`, default 5000, capped at the main timeout), twice.
fn claim_landed(bd: &BdCli, bead: &str, actor: &str) -> bool {
    let mut probe = bd.clone();
    let ms = std::env::var("AIR_BD_PROBE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000);
    probe.timeout = std::time::Duration::from_millis(ms).min(bd.timeout);
    (0..2).any(|_| {
        matches!(
            probe.show(bead),
            Ok(Some(Issue { ref assignee, ref status, .. }))
                if assignee.as_deref() == Some(actor) && status == "in_progress"
        )
    })
}

fn timeout_msg(what: &str, bead: &str) -> String {
    format!(
        "bd timed out during {what}, twice (Air retried once). A timeout, not a refusal: bd's \
         state is unknown and nothing was written to the ledger. Run `bd show {bead}` and \
         re-run `air claim {bead}`"
    )
}

/// One more try on a TIMEOUT, and only on a timeout (air-gsj). A refusal or any other error
/// is bd's answer and is returned as it came. Returns the result and whether a retry happened;
/// `on_retry` runs between the two attempts so the caller can record the fact.
///
/// Why a retry and not a longer wait: the adopter measured bd's cost as a ~2 s floor PER
/// INVOCATION with a contention tail — the calls that cross 5 s land at 11.8, 22.7, 27.1 and
/// 44.2 s (air-bp0) — so any usable threshold is crossed by the same stalls, while a fresh
/// process starts at the floor again. w1 retried a claim by hand three times on 2026-08-31 and
/// another worker took the bead in between; the message read as a denial.
pub fn retry_once<T>(
    mut f: impl FnMut() -> air_bd::Result<T>,
    on_retry: impl FnOnce(),
) -> (air_bd::Result<T>, bool) {
    match f() {
        Err(BdError::Timeout(_)) => {
            on_retry();
            (f(), true)
        }
        r => (r, false),
    }
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
    // Some(updated_at) when bd already holds the bead in_progress by this actor.
    let mut already_mine: Option<Option<String>> = None;
    // For the tmux window label (air-5lg); empty when bd could not answer.
    let mut title = String::new();
    let (shown, _) = retry_once(
        || bd.show(bead),
        || {
            log_event(
                &ledger,
                &worker,
                "claim",
                &inputs(serde_json::json!({})),
                "timeout-retry",
                "bd timed out during show; retrying once",
                "bd show",
            );
        },
    );
    match shown {
        Ok(Some(issue)) => {
            title.clone_from(&issue.title);
            if issue.status == "in_progress" && issue.assignee.as_deref() == Some(actor.as_str()) {
                already_mine = Some(issue.updated_at.clone());
            }
            if worker != "main" && issue.labels.iter().any(|l| l == OWNER_LABEL) {
                let msg = format!(
                    "refused: {bead} is labelled `{OWNER_LABEL}` (awaiting the owner); not a worker's to claim. Take other work; if it needs a decision, `air capture` the question and the coordinator files it."
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
    // 2b. Already ours in bd (a re-claim after a timeout, or a retry): no bd write, and the
    // ledger row keeps its original claim time, never a newer one, so the digest check is
    // not postdated (air-y8m). With no row, the best original time bd gives is `updated_at`.
    if already_mine.is_some() {
        let existing = ledger
            .open_claim(bead)
            .ok()
            .flatten()
            .filter(|c| c.worker == worker)
            .map(|c| c.claimed_at);
        let at = match &existing {
            Some(t) => t.clone(),
            None => {
                let at = earliest(already_mine.as_ref().and_then(|u| u.as_deref()), &now());
                if let Err(e) = ledger.record_claim(bead, &worker, files, &at) {
                    eprintln!("air claim: ledger write failed: {e}");
                    return 1;
                }
                at
            }
        };
        label_window(repo, &worker, bead, &title);
        let msg = format!(
            "reclaimed {bead} as {worker} (actor {actor}); bd already held it, claim time kept at {at}"
        );
        log_event(
            &ledger,
            &worker,
            "claim",
            &inputs(
                serde_json::json!({"actor": actor, "files": files, "kept_row": existing.is_some()}),
            ),
            "reclaimed",
            &msg,
            "bd show + 1 ledger row",
        );
        emit(
            json,
            &serde_json::json!({"ok": true, "bead": bead, "worker": worker, "claimed_at": at, "reclaimed": true}),
            || msg.clone(),
        );
        return 0;
    }
    // 3. bd: the atomic claim. The claim time is when it was issued, not when bd answered.
    let at = now();
    let mut decision = "claimed";
    // air-gsj: one internal retry on a timeout. `--claim` is idempotent for the same actor,
    // so a retry after a write that did land is a no-op there and an Ok here.
    let (claimed, retried) = retry_once(
        || bd.claim(bead, &actor),
        || {
            log_event(
                &ledger,
                &worker,
                "claim",
                &inputs(serde_json::json!({"actor": actor})),
                "timeout-retry",
                "bd timed out during --claim; retrying once (a fresh process starts at the floor)",
                "bd exit",
            );
        },
    );
    match claimed {
        Ok(()) => {
            if retried {
                decision = "claimed-retried";
            }
        }
        Err(BdError::Timeout(_)) => {
            // bd may have completed the write after we stopped waiting: reconcile before
            // saying anything about state, with a short separate probe, twice (under load
            // one probe can time out too; adopter 2026-08-22, load avg ~90).
            if !claim_landed(&bd, bead, &actor) {
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
            decision = "claimed-late";
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
    if let Err(e) = ledger.record_claim(bead, &worker, files, &at) {
        eprintln!(
            "air claim: bd claim succeeded but the ledger write failed: {e}. Re-run `air claim {bead}` (bd --claim is idempotent for the same actor)."
        );
        return 1;
    }
    label_window(repo, &worker, bead, &title);
    let msg = match decision {
        "claimed-late" => format!(
            "claimed {bead} as {worker} (actor {actor}) at {at} (bd was slow; reconciled by `bd show`)"
        ),
        "claimed-retried" => format!(
            "claimed {bead} as {worker} (actor {actor}) at {at} (bd timed out once; the retry landed)"
        ),
        _ => format!("claimed {bead} as {worker} (actor {actor}) at {at}"),
    };
    log_event(
        &ledger,
        &worker,
        "claim",
        &inputs(serde_json::json!({"actor": actor, "files": files})),
        decision,
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
        // air-0kk: open AND unassigned, in ONE bd process. Reopening alone left the assignee
        // pencilled in, and in bd 1.2.x that blocks every other worker's `--claim`: the bead
        // sat in `bd ready` claimable by nobody but the worker that had just released it
        // (adopter ad-tdv8; here air-an9 after gate's session was gone). One process, so
        // the status and the assignee cannot be left half-applied.
        match bd.reopen_unassigned(bead) {
            Ok(()) => {}
            Err(BdError::Timeout(_)) => {
                return fail(
                    &ledger,
                    &me,
                    "release",
                    inputs,
                    "timeout",
                    timeout_msg("-s open -a \"\"", bead),
                    "bd exit",
                    json,
                    1,
                );
            }
            Err(e) => {
                let msg = format!(
                    "bd refused to reopen and unassign {bead} (one `bd update -s open -a \"\"`); \
                     nothing recorded, and the bead is still in_progress by you: {e}"
                );
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
            label_window(repo, &worker, "", "");
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
