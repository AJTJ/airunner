//! `air fleet stop [--reason]` and `air fleet resume` (air-1vri.1).
//!
//! Owner, 2026-09-26: "the coordinator should be able to stop all work as well with one message
//! to air. Ensure that all role files know about this." One command sets the stop in the ledger
//! and Air tells every other session through its channel. While the fleet is stopped `air
//! claim`, `air batch cut` and `air land` refuse, naming the stop and who set it, and the Stop
//! nudge and the fan-out say nothing. A verify already running finishes and is recorded.
//! Sessions are not killed: the owner stays in control of processes.
//!
//! Only the coordinator and the owner may stop or resume. Workers and the lane are refused here
//! and by their deny list (`Bash(air fleet *)`).
//!
//! Removal: when the harness offers a fleet-wide pause Air can read, or when a round's owner
//! stops the fleet by other means every time.

use std::path::Path;

use air_ledger::Ledger;
use air_ledger::deliveries::Outgoing;
use air_ledger::fleet::FleetStop;
use serde_json::json;

/// Who may stop or resume the fleet.
pub fn may_steer(role: &str) -> Result<(), String> {
    match role {
        "coordinator" | "owner" => Ok(()),
        other => Err(format!(
            "air fleet: refused: stopping and resuming the fleet is the coordinator's and the \
             owner's; this session is the {other}. Tell the coordinator with `air capture`."
        )),
    }
}

/// The refusal a stopped fleet gives `what` (a command), or `None` while it runs.
pub fn refusal(ledger: &Ledger, what: &str) -> Option<String> {
    let s = ledger.fleet_stop().ok().flatten()?;
    Some(format!(
        "{what}: refused: {}. Nothing new starts until `air fleet resume`.",
        s.line()
    ))
}

/// Is the fleet stopped? A ledger that cannot be read answers no, so a broken read never stops
/// work by itself.
pub fn stopped(ledger: &Ledger) -> bool {
    ledger.fleet_stop().ok().flatten().is_some()
}

pub fn stop_text(s: &FleetStop) -> String {
    let why = if s.reason.is_empty() {
        String::new()
    } else {
        format!(" ({})", s.reason)
    };
    format!(
        "fleet stop from the {}{why}. Finish the step you are on, commit your work in progress, \
         and start nothing new: `air claim`, `air batch cut` and `air land` refuse until `air \
         fleet resume`. A verify already running finishes and is recorded. Your session stays \
         open; wait for the resume.",
        s.by_role
    )
}

pub fn resume_text(by_role: &str) -> String {
    format!(
        "fleet resumed by the {by_role}. Continue where you stopped: a worker takes up its claim \
         or claims a ready bead, and the lane cuts the next batch."
    )
}

/// Every checkout that may have a session to tell: each worktree but the main checkout and the
/// sender's own.
pub fn recipients(repo: &Path, me: &str) -> Vec<String> {
    let mut v: Vec<String> = crate::git::worktrees(repo)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(p, _)| air_ledger::paths::worker_name_for(&p).ok())
        .filter(|n| n != "main" && n != me)
        .collect();
    v.sort();
    v.dedup();
    v
}

fn tell(ledger: &Ledger, to: &[String], key: &str, text: &str, at: &str) -> Vec<String> {
    to.iter()
        .filter(|w| {
            ledger
                .enqueue_delivery(
                    &Outgoing {
                        to: w,
                        kind: "fleet",
                        key,
                        subject: "",
                        content: text,
                        // A resume replaces a stop nobody has read yet, and the reverse.
                        supersede: true,
                    },
                    at,
                )
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

fn open_for(repo: &Path) -> Result<(Ledger, String, &'static str), i32> {
    let (ledger, me) = super::open(repo).map_err(|e| {
        eprintln!("air fleet: {e}");
        1
    })?;
    Ok((ledger, me, super::caller_role()))
}

fn refused(ledger: &Ledger, me: &str, role: &str, msg: &str, json: bool) -> i32 {
    super::log_event(
        ledger,
        me,
        super::decisions::FLEET_REFUSE_ROLE,
        &json!({"role": role}),
        msg,
        "role",
    );
    super::emit(json, &json!({"ok": false, "reason": msg}), || {
        msg.to_string()
    });
    2
}

pub fn stop(repo: &Path, reason: Option<&str>, json: bool) -> i32 {
    let (ledger, me, role) = match open_for(repo) {
        Ok(x) => x,
        Err(code) => return code,
    };
    if let Err(msg) = may_steer(role) {
        return refused(&ledger, &me, role, &msg, json);
    }
    let at = super::now();
    let wanted = FleetStop {
        stopped_at: at.clone(),
        by_worker: me.clone(),
        by_role: role.to_string(),
        reason: reason.unwrap_or("").trim().to_string(),
    };
    let s = match ledger.set_fleet_stop(&wanted) {
        Ok(None) => wanted,
        Ok(Some(existing)) => {
            let msg = format!("air fleet stop: nothing changed; {}", existing.line());
            super::emit(
                json,
                &json!({"ok": true, "already": true, "stop": existing}),
                || msg.clone(),
            );
            return 0;
        }
        Err(e) => {
            eprintln!("air fleet stop: {e}");
            return 1;
        }
    };
    let told = tell(
        &ledger,
        &recipients(repo, &me),
        &format!("stop:{at}"),
        &stop_text(&s),
        &at,
    );
    super::log_event(
        &ledger,
        &me,
        super::decisions::FLEET_STOP,
        &json!({"reason": s.reason, "told": told}),
        &s.line(),
        &format!("{} session(s) told", told.len()),
    );
    super::emit(json, &json!({"ok": true, "stop": s, "told": told}), || {
        format!(
            "{}.\ntold: {}\n`air claim`, `air batch cut` and `air land` refuse until `air fleet \
             resume`.",
            s.line(),
            if told.is_empty() {
                "no other worktree".to_string()
            } else {
                told.join(" ")
            }
        )
    });
    0
}

pub fn resume(repo: &Path, json: bool) -> i32 {
    let (ledger, me, role) = match open_for(repo) {
        Ok(x) => x,
        Err(code) => return code,
    };
    if let Err(msg) = may_steer(role) {
        return refused(&ledger, &me, role, &msg, json);
    }
    let was = match ledger.clear_fleet_stop() {
        Ok(Some(s)) => s,
        Ok(None) => {
            super::emit(json, &json!({"ok": true, "already": true}), || {
                "air fleet resume: the fleet was not stopped; nothing changed".to_string()
            });
            return 0;
        }
        Err(e) => {
            eprintln!("air fleet resume: {e}");
            return 1;
        }
    };
    let at = super::now();
    let told = tell(
        &ledger,
        &recipients(repo, &me),
        &format!("resume:{at}"),
        &resume_text(role),
        &at,
    );
    super::log_event(
        &ledger,
        &me,
        super::decisions::FLEET_RESUME,
        &json!({"was": was, "told": told}),
        &format!("resumed; {}", was.line()),
        &format!("{} session(s) told", told.len()),
    );
    super::emit(json, &json!({"ok": true, "was": was, "told": told}), || {
        format!(
            "fleet resumed (it was stopped since {}).\ntold: {}",
            was.stopped_at,
            if told.is_empty() {
                "no other worktree".to_string()
            } else {
                told.join(" ")
            }
        )
    });
    0
}
