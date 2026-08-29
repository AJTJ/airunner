//! `air lease take|release|status|break|beat [<resource>]`: mutual exclusion for what two
//! agents cannot share. Ported from `adopter/scripts/lease.sh` (2026-08-21): identity is
//! the worktree; liveness is the holder's `claude` pid plus its start time; stale is
//! heartbeat age (`AIR_LEASE_STALE_SECS`, default 600; hooks refresh it on every tool call).
//! Default resource is `runtime`, adopter's name for "ports, device, Docker".

use std::path::Path;

use air_ledger::leases::{Holder, Lease, Take};

use crate::cmd::status::minutes_between;
use crate::cmd::{emit, log_event, now, open};

/// The `claude` process this command runs under: `CLAUDE_PID` when exported (verified by
/// adopter), else the nearest ancestor named `claude`, else our parent.
pub fn owner_pid() -> Option<i64> {
    // Verbatim override for tests and diagnostics; never set by launchers.
    if let Some(p) = std::env::var("AIR_LEASE_PID")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
    {
        return Some(p);
    }
    if let Some(p) = std::env::var("CLAUDE_PID")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        && pid_alive(p)
    {
        return Some(p);
    }
    let mut p = i64::from(std::os::unix::process::parent_id());
    for _ in 0..8 {
        if p <= 1 {
            break;
        }
        let comm = ps(p, "comm=").unwrap_or_default();
        if comm.rsplit('/').next() == Some("claude") {
            return Some(p);
        }
        p = ps(p, "ppid=")
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
    }
    Some(i64::from(std::os::unix::process::parent_id()))
}

fn ps(pid: i64, field: &str) -> Option<String> {
    let out = std::process::Command::new("ps")
        .args(["-o", field, "-p", &pid.to_string()])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

pub fn pid_alive(pid: i64) -> bool {
    ps(pid, "pid=").is_some()
}

/// `ps -o lstart=` normalised; guards against pid reuse.
pub fn pid_started(pid: i64) -> Option<String> {
    ps(pid, "lstart=").map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Why the current lease is not valid, or None when healthy (lease.sh `lock_defect`).
pub fn defect(l: &Lease, now: &str, stale_secs: i64) -> Option<String> {
    let pid = l.pid?;
    if !pid_alive(pid) {
        return Some(format!("dead (pid {pid} gone)"));
    }
    if let (Some(rec), Some(cur)) = (&l.pid_started, pid_started(pid))
        && *rec != cur
    {
        return Some(format!("dead (pid {pid} reused)"));
    }
    let age_min = minutes_between(&l.heartbeat_at, now).unwrap_or(0);
    if age_min.saturating_mul(60) > stale_secs {
        return Some(format!("stale (idle {age_min} min)"));
    }
    None
}

fn stale_secs() -> i64 {
    std::env::var("AIR_LEASE_STALE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600)
}

fn holder<'a>(
    worker: &'a str,
    session: Option<&'a str>,
    pid: Option<i64>,
    started: Option<&'a str>,
) -> Holder<'a> {
    Holder {
        worker,
        session_id: session,
        pid,
        pid_started: started,
    }
}

pub fn take(repo: &Path, resource: &str, reason: &str, json: bool) -> i32 {
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air lease: {e}");
            return 1;
        }
    };
    let t = now();
    let pid = owner_pid();
    let started = pid.and_then(pid_started);
    let session = std::env::var("CLAUDE_SESSION_ID").ok();
    let h = holder(&worker, session.as_deref(), pid, started.as_deref());
    let stale = stale_secs();
    let res = ledger.lease_take(resource, &h, reason, &t, |l| defect(l, &t, stale));
    match res {
        Ok(Take::Taken) => {
            let msg = format!("{resource} lease taken by {worker} ({reason})");
            log_event(
                &ledger,
                &worker,
                "lease.take",
                &serde_json::json!({"resource": resource, "reason": reason, "pid": pid}),
                "taken",
                &msg,
                "1 lease row",
            );
            emit(
                json,
                &serde_json::json!({"ok": true, "resource": resource, "holder": worker}),
                || msg.clone(),
            );
            0
        }
        Ok(Take::AlreadyMine) => {
            let msg = format!("{resource} lease already held by {worker}; heartbeat refreshed");
            log_event(
                &ledger,
                &worker,
                "lease.take",
                &serde_json::json!({"resource": resource}),
                "already-mine",
                &msg,
                "1 lease row",
            );
            emit(
                json,
                &serde_json::json!({"ok": true, "resource": resource, "holder": worker}),
                || msg.clone(),
            );
            0
        }
        Ok(Take::TakenAfter(why)) => {
            let msg = format!("{resource} lease was {why}; taken by {worker} ({reason})");
            log_event(
                &ledger,
                &worker,
                "lease.take",
                &serde_json::json!({"resource": resource, "reason": reason, "broke": why}),
                "taken-after-break",
                &msg,
                "1 lease row",
            );
            emit(
                json,
                &serde_json::json!({"ok": true, "resource": resource, "holder": worker, "broke": why}),
                || msg.clone(),
            );
            0
        }
        Ok(Take::Held(l)) => {
            let age = minutes_between(&l.heartbeat_at, &t).unwrap_or(0);
            let msg = format!(
                "{resource} lease is HELD by {} (active {age} min ago; reason: {}). Release your task claim and take work that does not need it, or ask the coordinator. Do not retry and do not route around it.",
                l.worker, l.reason
            );
            log_event(
                &ledger,
                &worker,
                "lease.take",
                &serde_json::json!({"resource": resource, "reason": reason, "holder": l.worker}),
                "denied",
                &msg,
                "1 lease row",
            );
            emit(
                json,
                &serde_json::json!({"ok": false, "resource": resource, "holder": l.worker, "reason": msg}),
                || msg.clone(),
            );
            1
        }
        Err(e) => {
            eprintln!("air lease: {e}");
            1
        }
    }
}

pub fn release(repo: &Path, resource: &str, json: bool) -> i32 {
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air lease: {e}");
            return 1;
        }
    };
    match ledger.lease_release(resource, &worker) {
        Ok(true) => {
            let msg = format!("{resource} lease released by {worker}");
            log_event(
                &ledger,
                &worker,
                "lease.release",
                &serde_json::json!({"resource": resource}),
                "released",
                &msg,
                "1 lease row",
            );
            emit(json, &serde_json::json!({"ok": true}), || msg.clone());
            0
        }
        Ok(false) => {
            let msg = format!("{worker} does not hold {resource}");
            log_event(
                &ledger,
                &worker,
                "lease.release",
                &serde_json::json!({"resource": resource}),
                "not-held",
                &msg,
                "0 lease rows",
            );
            emit(
                json,
                &serde_json::json!({"ok": false, "reason": msg}),
                || msg.clone(),
            );
            1
        }
        Err(e) => {
            eprintln!("air lease: {e}");
            1
        }
    }
}

pub fn break_lease(repo: &Path, resource: &str, force: bool, json: bool) -> i32 {
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air lease: {e}");
            return 1;
        }
    };
    let t = now();
    let cur = match ledger.lease(resource) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("air lease: {e}");
            return 1;
        }
    };
    let Some(cur) = cur else {
        emit(json, &serde_json::json!({"ok": true, "was": null}), || {
            format!("{resource}: not held")
        });
        return 0;
    };
    let why = defect(&cur, &t, stale_secs());
    if why.is_none() && !force {
        let msg = format!(
            "{resource} is held by {} and healthy; pass --force to break it",
            cur.worker
        );
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        return 1;
    }
    let _ = ledger.lease_break(resource);
    let msg = format!(
        "{resource} lease broken (was {}: {})",
        cur.worker,
        why.clone().unwrap_or_else(|| "healthy, forced".into())
    );
    log_event(
        &ledger,
        &worker,
        "lease.break",
        &serde_json::json!({"resource": resource, "was": cur.worker, "force": force}),
        "broken",
        &msg,
        "1 lease row",
    );
    emit(
        json,
        &serde_json::json!({"ok": true, "was": cur.worker, "why": why}),
        || msg.clone(),
    );
    0
}

pub fn beat(repo: &Path) -> i32 {
    match open(repo).and_then(|(l, w)| l.lease_beat(&w, &now()).map_err(|e| e.to_string())) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("air lease: {e}");
            1
        }
    }
}

/// air-uae: where Air keeps leases, printed on every `air lease status` whether or not any are
/// held. A second store in the target repo is then visible in one command instead of inferred
/// from a contradiction, which is how adopter's took two incidents to find.
pub fn store_line(air_dir: &Path) -> String {
    format!(
        "lease store: {}/ledger.db (leases table)",
        air_dir.display()
    )
}

pub fn status(repo: &Path, json: bool) -> i32 {
    let (ledger, _worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air lease: {e}");
            return 1;
        }
    };
    let t = now();
    let stale = stale_secs();
    let leases = ledger.leases().unwrap_or_default();
    let rows: Vec<serde_json::Value> = leases
        .iter()
        .map(|l| {
            let wants = ledger.lease_wants(&l.resource).unwrap_or_default();
            serde_json::json!({
                "resource": l.resource, "holder": l.worker, "reason": l.reason,
                "taken_at": l.taken_at, "heartbeat_at": l.heartbeat_at,
                "defect": defect(l, &t, stale),
                "wanted_by": wants.iter().map(|w| w.0.clone()).collect::<Vec<_>>(),
            })
        })
        .collect();
    // air-uae: name the store, always. the adopter ran two that disagreed — `air lease take`
    // wrote the ledger while their PreToolUse guard read
    // `$(git --git-common-dir)/ad-leases/<resource>/` — so `make api` was denied naming the
    // command that had just succeeded (ad-3wnp, ad-gpj0). Neither side ever said where it was
    // looking, so the disagreement had to be inferred from the contradiction. A second store is
    // now visible in one command instead.
    let store = store_line(ledger.dir());
    emit(json, &rows, || {
        if rows.is_empty() {
            return format!("no leases held\n{store}");
        }
        rows.iter()
            .map(|r| {
                format!(
                    "{:<10} {:<12} {}{}{}",
                    r["resource"].as_str().unwrap_or(""),
                    r["holder"].as_str().unwrap_or(""),
                    r["reason"].as_str().unwrap_or(""),
                    r["defect"]
                        .as_str()
                        .map(|d| format!("  [{d}]"))
                        .unwrap_or_default(),
                    if r["wanted_by"].as_array().is_some_and(|a| !a.is_empty()) {
                        format!("  wanted by {}", r["wanted_by"])
                    } else {
                        String::new()
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
            + &format!("\n{store}")
    });
    0
}
