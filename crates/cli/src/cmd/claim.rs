//! `air claim <bead> [--files a,b]` and `air release <bead> --reason <r> [--worker <w>]`.
//!
//! The only claim path (decisions 2026-08-20: "wrap beads, never watch it"). Order is fixed:
//! the ledger refuses first if someone else holds the bead here; then `bd show` (an `owner`
//! bead is not a worker's to claim; bd's own rule that a pencilled `assignee` blocks every
//! other worker's `--claim` is printed with who is assigned; a closed bead is closed); then
//! `bd update --claim` (bd's atomic CAS decides races); only after bd succeeds is the ledger
//! row written. A bd timeout is not a refusal: bd's state is unknown, so Air re-reads
//! `bd show` and records the claim if it landed, otherwise says so (the adopter,
//!, 2026-08-21).
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
/// word for a gate. It rotted exactly that way in the adopter, where a triage note records a
/// whole category of beads that "carries human but needs no owner ruling" (its
/// `private/research/adopter-notes/notes/human-queue-triage.md`, category C). Every site that
/// decides claimability reads this constant, never a literal (air-5hw).
pub const OWNER_LABEL: &str = "owner";

/// Actor string passed to bd: `BEADS_ACTOR` if set, else the worker name.
fn actor_for(worker: &str) -> String {
    std::env::var("BEADS_ACTOR")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| worker.to_string())
}

/// `AIR_BD_BIN` overrides the `bd` binary (tests use a fake). `AIR_BD_TIMEOUT_MS` overrides
/// the WHOLE budget, per-id allowance included: a number somebody exported is the number the
/// wait runs against, whatever the id count.
pub fn bd_for(repo: &Path) -> BdCli {
    let mut bd = BdCli::new(repo);
    if let Ok(bin) = std::env::var("AIR_BD_BIN")
        && !bin.is_empty()
    {
        bd.bin = bin.into();
    }
    if let Some(ms) = bd_timeout_override() {
        bd.timeout = std::time::Duration::from_millis(ms);
        bd.per_id = std::time::Duration::ZERO;
    }
    bd
}

fn bd_timeout_override() -> Option<u64> {
    std::env::var("AIR_BD_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
}

/// How a bd timeout names its budget, for every refusal built on one (air-8lj8): the id
/// count, the budget in force, and how to raise it. Pure over `overridden` so the probe can
/// read both shapes.
pub fn budget_words(ids: usize, budget: std::time::Duration, overridden: bool) -> String {
    use crate::cmd::status::duration_line;
    let how = if overridden {
        "AIR_BD_TIMEOUT_MS, set in this environment".to_string()
    } else {
        format!(
            "{} + {} per id; AIR_BD_TIMEOUT_MS overrides it, in milliseconds",
            duration_line(air_bd::DEFAULT_TIMEOUT),
            duration_line(air_bd::PER_ID)
        )
    };
    format!(
        "{ids} id(s) within a budget of {} ({how})",
        duration_line(budget)
    )
}

/// Whether `AIR_BD_TIMEOUT_MS` is in force in this process.
pub fn bd_overridden() -> bool {
    bd_timeout_override().is_some()
}

/// The probe budget's default (air-8lj8: was 5 s). **Fail direction: CLOSED**: a probe that
/// times out reports the claim as timed out with bd's state unknown, and a triage pass that
/// cannot verify its ids is refused. Half the client's default, because a read is cheaper than
/// the write it follows; still capped at the main budget.
const PROBE_DEFAULT_MS: u64 = 30_000;

fn probe_ms() -> u64 {
    std::env::var("AIR_BD_PROBE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(PROBE_DEFAULT_MS)
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

/// A shorter-budget bd for a read that must not hold a command up: `AIR_BD_PROBE_TIMEOUT_MS`
/// (default 30000), never longer than the main timeout. The per-id allowance is kept unless
/// the variable is set, which like `AIR_BD_TIMEOUT_MS` replaces the whole budget.
pub fn probe_bd(repo: &Path) -> BdCli {
    let bd = bd_for(repo);
    let per_id = if std::env::var_os("AIR_BD_PROBE_TIMEOUT_MS").is_some() {
        std::time::Duration::ZERO
    } else {
        bd.per_id
    };
    BdCli {
        timeout: std::time::Duration::from_millis(probe_ms()).min(bd.timeout),
        per_id,
        label: air_ledger::budgets::BD_PROBE,
        ..bd
    }
}

/// After a `--claim` timeout: did bd's write land? Probes `bd show` with a short separate
/// timeout (`AIR_BD_PROBE_TIMEOUT_MS`, default 30000, capped at the main timeout), twice.
fn claim_landed(bd: &BdCli, bead: &str, actor: &str) -> bool {
    let mut probe = bd.clone();
    probe.timeout = std::time::Duration::from_millis(probe_ms()).min(bd.timeout);
    probe.label = air_ledger::budgets::BD_PROBE;
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
    trace: super::decisions::Trace,
    inputs: serde_json::Value,
    msg: String,
    denom: &str,
    json: bool,
    code: i32,
) -> i32 {
    log_event(ledger, worker, trace, &inputs, &msg, denom);
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
    // 0. A stopped fleet starts nothing (air-1vri.1).
    if let Some(msg) = super::fleet::refusal(&ledger, "air claim") {
        return fail(
            &ledger,
            &worker,
            super::decisions::CLAIM_FLEET_STOPPED,
            inputs(serde_json::json!({})),
            msg,
            "1 ledger row",
            json,
            2,
        );
    }
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
                super::decisions::CLAIM_REFUSE,
                inputs(serde_json::json!({})),
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
    // air-x1ha: the id bd RESOLVED, which is not always the string that was typed. bd accepts
    // an unambiguous prefix and answers with the canonical id (checked against bd 1.2.2,
    // 2026-09-06: `bd show zz-bd --json` returns `"id": "zz-bdz"`), and it refuses an
    // ambiguous one outright, so an answer is always exactly one bead.
    let mut canonical = bead.to_string();
    let (shown, _) = retry_once(
        || bd.show(bead),
        || {
            log_event(
                &ledger,
                &worker,
                super::decisions::CLAIM_TIMEOUT_RETRY,
                &inputs(serde_json::json!({})),
                "bd timed out during show; retrying once",
                "bd show",
            );
        },
    );
    match shown {
        Ok(Some(issue)) => {
            title.clone_from(&issue.title);
            if !issue.id.is_empty() {
                canonical = issue.id.clone();
            }
            if issue.status == "in_progress" && issue.assignee.as_deref() == Some(actor.as_str()) {
                already_mine = Some(issue.updated_at.clone());
            }
            if super::is_worker_like(super::caller_role())
                && issue.labels.iter().any(|l| l == OWNER_LABEL)
            {
                let msg = format!(
                    "refused: {bead} is labelled `{OWNER_LABEL}` (awaiting the owner); not a worker's to claim. Take other work; if it needs a decision, `air capture` the question and the coordinator files it."
                );
                return fail(
                    &ledger,
                    &worker,
                    super::decisions::CLAIM_REFUSE,
                    inputs(serde_json::json!({})),
                    msg,
                    "bd show",
                    json,
                    2,
                );
            }
            // air-f10: an epic is a container. A worker offered one by a count that could
            // not tell it from a task nearly claimed it, and a claim pencils an assignee
            // onto the container that nobody else can then take (air-0kk's residue through
            // the front door). Refused for everyone: the coordinator decomposes, it does not
            // claim.
            if issue.issue_type == super::ready_cache::EPIC {
                let msg = format!(
                    "refused: {bead} is an epic, a container rather than a task; claiming it would put an assignee on work nobody else can take. Decompose it (`bd create` its children) and claim a child."
                );
                return fail(
                    &ledger,
                    &worker,
                    super::decisions::CLAIM_REFUSE,
                    inputs(serde_json::json!({})),
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
                    super::decisions::CLAIM_REFUSE,
                    inputs(serde_json::json!({})),
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
                // air-6wv2: say that nobody is actually holding it, which is a FACT here
                // rather than a guess, because step 1 above already refused if Air had an open
                // claim on this bead by anyone else. Reaching this line means bd has an
                // assignee and Air has no claim behind it.
                //
                // I wrote this as a conditional first — live assignee versus leftover — and
                // running it showed the live branch is unreachable: with a claim recorded, the
                // ledger check fires and names the holder and `air release`. Dead code stating
                // a distinction the function had already made.
                //
                // bd exposes nothing that marks a reopen: no `reopened_at`, and `closed_at`
                // cannot be observed on an open bead without creating and reopening one, which
                // a worker may not do. So the reopen is offered as a possible cause and not
                // asserted. That bd keeps an assignee through a CLOSE is verified (a closed
                // bead here returns `"status":"closed","assignee":"verify"`); that it survives
                // a REOPEN is the observed instance that filed this bead (air-vsvt came back
                // carrying `alerts`), not a property anyone has tested in isolation.
                let msg = format!(
                    "refused by bd's rule: {bead} has assignee `{a}`, and in bd 1.2.x a pencilled assignee blocks every other worker's --claim. Air has no open claim behind that assignee, so it may be left over rather than live work: bd keeps an assignee through a close, and a reopened bead can come back pencilled in with nobody having assigned it. Either `{a}` claims it, or the coordinator clears the assignee (`bd update {bead} -a \"\"`)."
                );
                return fail(
                    &ledger,
                    &worker,
                    super::decisions::CLAIM_REFUSE,
                    inputs(serde_json::json!({"assignee": a})),
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
                super::decisions::CLAIM_NO_SUCH_BEAD,
                inputs(serde_json::json!({})),
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
                super::decisions::CLAIM_TIMEOUT,
                inputs(serde_json::json!({})),
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
    // air-x1ha: from here on the bead is the id BD RESOLVED, never the string that was typed.
    // A worker typed `air-ahl`, bd claimed `air-ahlf`, Air wrote its row under the prefix, and
    // the next status reconcile asked bd about the prefix, got nothing, and released the claim
    // while the work continued: the coordinator saw an abandoned bead and the worker saw
    // nothing at all. Two stores holding different ids for one bead is what air-uir prevents a
    // layer up; this is the same failure a layer down.
    let typed = bead;
    let bead: &str = &canonical;
    // The ledger's own CAS ran above on the typed string, which cannot match a row stored
    // under the canonical id. Not a new guard — the same guard, asked about the id that is
    // actually stored, which is the only version of it that still works now rows are
    // canonical. Skipped when nothing was resolved, since the first check already ran on it.
    if bead != typed {
        match ledger.open_claim(bead) {
            Ok(Some(c)) if c.worker != worker => {
                let msg = format!(
                    "refused: you typed {typed}, which bd resolves to {bead}, and {bead} is claimed by {} since {} (fix: ask them, or the coordinator runs `air release {bead} --worker {} --reason reassigned`)",
                    c.worker, c.claimed_at, c.worker
                );
                return fail(
                    &ledger,
                    &worker,
                    super::decisions::CLAIM_REFUSE,
                    inputs(serde_json::json!({"resolved": bead})),
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
            super::decisions::CLAIM_RECLAIMED,
            &inputs(
                serde_json::json!({"actor": actor, "files": files, "kept_row": existing.is_some()}),
            ),
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
    let mut decision = super::decisions::CLAIM_CLAIMED;
    // air-gsj: one internal retry on a timeout. `--claim` is idempotent for the same actor,
    // so a retry after a write that did land is a no-op there and an Ok here.
    let (claimed, retried) = retry_once(
        || bd.claim(bead, &actor),
        || {
            log_event(
                &ledger,
                &worker,
                super::decisions::CLAIM_TIMEOUT_RETRY,
                &inputs(serde_json::json!({"actor": actor})),
                "bd timed out during --claim; retrying once (a fresh process starts at the floor)",
                "bd exit",
            );
        },
    );
    match claimed {
        Ok(()) => {
            if retried {
                decision = super::decisions::CLAIM_CLAIMED_RETRIED;
            }
        }
        Err(BdError::Timeout(_)) => {
            // bd may have completed the write after we stopped waiting: reconcile before
            // saying anything about state, with a short separate probe, twice (under load
            // one probe can time out too; the adopter 2026-08-22, load avg ~90).
            if !claim_landed(&bd, bead, &actor) {
                return fail(
                    &ledger,
                    &worker,
                    super::decisions::CLAIM_TIMEOUT,
                    inputs(serde_json::json!({"actor": actor})),
                    timeout_msg("--claim", bead),
                    "bd exit",
                    json,
                    1,
                );
            }
            eprintln!(
                "air claim: bd timed out, but `bd show` confirms the claim landed; recording it"
            );
            decision = super::decisions::CLAIM_CLAIMED_LATE;
        }
        Err(e) => {
            let msg = format!("bd refused the claim; nothing recorded: {e}");
            return fail(
                &ledger,
                &worker,
                super::decisions::CLAIM_BD_REFUSED,
                inputs(serde_json::json!({"actor": actor})),
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
    let msg = match decision.decision {
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
        decision,
        &inputs(serde_json::json!({"actor": actor, "files": files})),
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
            if super::is_worker_like(super::caller_role()) {
                eprintln!("air release: --worker is for the coordinator, not a worker");
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
                super::decisions::RELEASE_NO_SUCH_BEAD,
                inputs,
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
                super::decisions::RELEASE_TIMEOUT,
                inputs,
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
            &ledger,
            &me,
            super::decisions::RELEASE_REFUSE,
            inputs,
            msg,
            "bd show",
            json,
            2,
        );
    }
    if status == "in_progress" && reason != "landed" {
        // air-0kk: open AND unassigned, in ONE bd process. Reopening alone left the assignee
        // pencilled in, and in bd 1.2.x that blocks every other worker's `--claim`: the bead
        // sat in `bd ready` claimable by nobody but the worker that had just released it
        // (the adopter; here air-an9 after gate's session was gone). One process, so
        // the status and the assignee cannot be left half-applied.
        match bd.reopen_unassigned(bead) {
            Ok(()) => {}
            Err(BdError::Timeout(_)) => {
                return fail(
                    &ledger,
                    &me,
                    super::decisions::RELEASE_TIMEOUT,
                    inputs,
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
                    super::decisions::RELEASE_BD_REFUSED,
                    inputs,
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
                super::decisions::RELEASE_RELEASED,
                &inputs,
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
                super::decisions::RELEASE_NO_CLAIM,
                inputs,
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
