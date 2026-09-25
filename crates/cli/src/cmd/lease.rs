//! `air lease take|release|status|break|beat [<resource>]`: mutual exclusion for what two
//! agents cannot share. Ported from `the adopter's scripts/lease.sh` (2026-08-21): identity is
//! the worktree; liveness is the holder's `claude` pid plus its start time; stale is
//! heartbeat age (`AIR_LEASE_STALE_SECS`, default 600; hooks refresh it on every tool call).
//! Default resource is `runtime`, the adopter's name for "ports, device, Docker".

use std::path::Path;

use air_ledger::leases::{Holder, Lease, Take};

use crate::cmd::status::minutes_between;
use crate::cmd::{emit, log_event, now, open};

/// The `claude` process this command runs under: `CLAUDE_PID` when exported (verified by
/// The adopter), else the nearest ancestor named `claude`, else our parent.
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

pub fn stale_secs() -> i64 {
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
/// from a contradiction, which is how the adopter's took two incidents to find.
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
    // air-uae: name the store, always. The adopter ran two that disagreed — `air lease take`
    // wrote the ledger while their PreToolUse guard read
    // `$(git --git-common-dir)/<prefix>-leases/<resource>/` — so `make api` was denied naming the
    // command that had just succeeded. Neither side ever said where it was
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

// ── Which commands need which lease (`"leases"` in `.claude/air.json`) ──────────────────────
//
// `air lease` stored WHO holds a resource and never decided WHICH command needs one, so an
// adopter wrote that half themselves: a PreToolUse guard, a rule engine and a lease script,
// about 1,500 lines, whose core job is "a command touching `runtime` (ports, the simulator,
// adb, Docker lifecycle, `make api`) is refused unless this worktree holds the lease". Two of
// its recorded failures are what Air removes by owning the question:
//
// * **Two stores.** Their guard read its own lock directory while `air lease take` wrote the
//   ledger, so a correctly held lease was refused naming the command that had just succeeded
//   (2026-08-29; see `store_line` above). A check that lives beside the store cannot read a
//   different one.
// * **Guessing the rule from its name.** On 2026-08-14 an agent reasoned that `make test` must
//   need the lease, never ran it, and asked a peer to run its tests; nothing had refused it.
//   Their answer was an `--explain "<cmd>"` asked of the same rules the guard enforces.
//   `air lease needs` is that, over the same patterns the hook reads.
//
// The patterns are the harness's own `Bash(...)` deny syntax (`worker_deny`), matched the way
// the harness documents matching them (docs/research/harness-facts.md §3 row 10): `*` spans
// spaces, each `&&`/`||`/`;`/`|` segment is matched on its own, leading `VAR=value`
// assignments and the `timeout`/`nice`/`nohup` wrappers are stripped, and a trailing ` *`
// also matches the bare command. Anchored at the segment's start, so `git grep adb` is a grep:
// the adopter's guard was wrong four times by matching a word that was being described rather
// than run. Quoted text and heredoc bodies are data and are never split into segments.
// `bash -c` and scripts are opaque, as they are to the harness.
//
// Removal: when the harness can scope a tool permission to a held resource, or when a round
// with `leases` declared records zero `lease-refuse`/`lease-would-refuse` decisions while
// commands matching the patterns ran (`lease-held` decisions are that subject occurring).

/// `"leases"` from `.claude/air.json`: `{"runtime": ["make api*", "adb *"]}`. Empty when absent,
/// unreadable, or not an object of string lists, which is "nothing needs a lease".
pub fn declared(repo: &Path) -> Vec<(String, Vec<String>)> {
    crate::cmd::handover::air_json(repo)
        .as_ref()
        .and_then(|v| v.get("leases"))
        .map(declared_from)
        .unwrap_or_default()
}

/// [`declared`] over an already-parsed value, so probes need no file.
pub fn declared_from(v: &serde_json::Value) -> Vec<(String, Vec<String>)> {
    v.as_object()
        .map(|m| {
            m.iter()
                .map(|(k, pats)| {
                    let pats = pats
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|p| p.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    (k.clone(), pats)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One lease a command needs: which resource, the pattern that matched, the segment it matched.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Need {
    pub resource: String,
    pub pattern: String,
    pub segment: String,
}

/// Every declared lease this command needs, one entry per resource (the first match wins).
pub fn needs(declared: &[(String, Vec<String>)], cmd: &str) -> Vec<Need> {
    let segs = segments(cmd);
    declared
        .iter()
        .filter_map(|(resource, pats)| {
            pats.iter().find_map(|p| {
                segs.iter().find(|s| pattern_matches(p, s)).map(|s| Need {
                    resource: resource.clone(),
                    pattern: p.clone(),
                    segment: s.clone(),
                })
            })
        })
        .collect()
}

/// The simple commands in a shell line, wrappers and leading assignments stripped, whitespace
/// collapsed. Splits on `&&`, `||`, `;`, `|`, `&` and newlines outside quotes; heredoc bodies
/// are skipped.
pub fn segments(cmd: &str) -> Vec<String> {
    let chars: Vec<char> = cmd.chars().collect();
    let at = |j: usize| chars.get(j).copied();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut heredocs: Vec<String> = Vec::new();
    let mut i = 0usize;
    while let Some(c) = at(i) {
        let next = at(i.saturating_add(1));
        if let Some(q) = quote {
            cur.push(c);
            if c == '\\' && q == '"' {
                cur.extend(next);
                i = i.saturating_add(2);
                continue;
            }
            if c == q {
                quote = None;
            }
            i = i.saturating_add(1);
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                cur.push(c);
            }
            '\\' => {
                cur.push(c);
                cur.extend(next);
                i = i.saturating_add(2);
                continue;
            }
            '<' if next == Some('<') && at(i.saturating_add(2)) != Some('<') => {
                // `<<EOF`, `<<-EOF`, `<<'EOF'`: remember the delimiter; the body is data.
                let mut j = i.saturating_add(2);
                if at(j) == Some('-') {
                    j = j.saturating_add(1);
                }
                while at(j) == Some(' ') {
                    j = j.saturating_add(1);
                }
                let mut word = String::new();
                while let Some(w) = at(j) {
                    if w.is_whitespace() || matches!(w, ';' | '&' | '|' | ')') {
                        break;
                    }
                    if !matches!(w, '\'' | '"' | '\\') {
                        word.push(w);
                    }
                    j = j.saturating_add(1);
                }
                if !word.is_empty() {
                    heredocs.push(word);
                }
                cur.extend(chars.get(i..j).unwrap_or(&[]));
                i = j;
                continue;
            }
            '\n' => {
                out.push(std::mem::take(&mut cur));
                i = i.saturating_add(1);
                // Skip each pending heredoc body, up to and including its delimiter line.
                for delim in std::mem::take(&mut heredocs) {
                    while i < chars.len() {
                        let end = (i..chars.len())
                            .find(|&k| at(k) == Some('\n'))
                            .unwrap_or(chars.len());
                        let line: String = chars.get(i..end).unwrap_or(&[]).iter().collect();
                        i = end.saturating_add(1);
                        if line.trim() == delim {
                            break;
                        }
                    }
                }
                continue;
            }
            ';' => out.push(std::mem::take(&mut cur)),
            '|' => {
                out.push(std::mem::take(&mut cur));
                if next == Some('|') {
                    i = i.saturating_add(1);
                }
            }
            '&' => {
                let prev = i.checked_sub(1).and_then(at);
                if next == Some('&') {
                    out.push(std::mem::take(&mut cur));
                    i = i.saturating_add(1);
                } else if matches!(prev, Some('>') | Some('<')) || next == Some('>') {
                    cur.push(c); // `2>&1`, `&>file`: a redirection, not a separator.
                } else {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
        i = i.saturating_add(1);
    }
    out.push(cur);
    out.iter()
        .map(|s| {
            strip_wrappers(
                s.trim()
                    .trim_start_matches(['(', '{', ' '])
                    .trim_end_matches([')', '}', ' ']),
            )
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Leading `VAR=value` assignments and the `timeout`/`nice`/`nohup` wrappers, removed the way
/// the harness removes them before matching a deny rule.
fn strip_wrappers(seg: &str) -> String {
    let toks: Vec<&str> = seg.split_whitespace().collect();
    let flag = |i: usize| toks.get(i).is_some_and(|x| x.starts_with('-'));
    let mut i = 0usize;
    while let Some(t) = toks.get(i) {
        let assignment = t.split_once('=').is_some_and(|(k, _)| {
            !k.is_empty()
                && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !k.starts_with(|c: char| c.is_ascii_digit())
        });
        if assignment || *t == "nohup" {
            i = i.saturating_add(1);
        } else if *t == "timeout" {
            i = i.saturating_add(1);
            while flag(i) {
                // `-k 5` and `-s TERM` carry a value; `--kill-after=5` does not.
                let valued = matches!(
                    toks.get(i),
                    Some(&"-k" | &"-s" | &"--kill-after" | &"--signal")
                );
                i = i.saturating_add(if valued { 2 } else { 1 });
            }
            i = i.saturating_add(1); // the duration
        } else if *t == "nice" {
            i = i.saturating_add(1);
            if toks.get(i) == Some(&"-n") {
                i = i.saturating_add(2);
            } else if flag(i) {
                i = i.saturating_add(1);
            }
        } else {
            break;
        }
    }
    toks.get(i..).unwrap_or(&[]).join(" ")
}

/// Does `pattern` (the harness's deny syntax, with or without the `Bash(...)` wrapper) match
/// this segment? `*` matches any run of characters, spaces included; a trailing ` *` also
/// matches the bare command, so `adb *` covers `adb` and never `adbx`.
pub fn pattern_matches(pattern: &str, segment: &str) -> bool {
    let p = pattern.trim();
    let p = p
        .strip_prefix("Bash(")
        .and_then(|x| x.strip_suffix(')'))
        .unwrap_or(p);
    let p = p.split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some(bare) = p.strip_suffix(" *")
        && !bare.contains('*')
        && segment == bare
    {
        return true;
    }
    glob(p.as_bytes(), segment.as_bytes())
}

/// Wildcard match, `*` only, iterative with one backtrack point.
fn glob(p: &[u8], s: &[u8]) -> bool {
    let (mut pi, mut si) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while si < s.len() {
        match p.get(pi) {
            Some(b'*') => {
                star = Some((pi, si));
                pi = pi.saturating_add(1);
            }
            Some(c) if Some(c) == s.get(si) => {
                pi = pi.saturating_add(1);
                si = si.saturating_add(1);
            }
            _ => match star {
                Some((sp, ss)) => {
                    pi = sp.saturating_add(1);
                    si = ss.saturating_add(1);
                    star = Some((sp, si));
                }
                None => return false,
            },
        }
    }
    p.get(pi..).unwrap_or(&[]).iter().all(|c| *c == b'*')
}

/// Where a needed lease stands for `me`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// `me` holds it. Staleness is ignored for the holder: a long foreground command makes no
    /// heartbeat while it runs, and a holder must never be told by its own guard that it does
    /// not hold its lease (an adopter's guard reads "mine" through staleness for that reason).
    Mine,
    /// `me` holds it from a session that is gone; `take` re-claims it.
    MineDead(String),
    /// Someone else holds it; `Some(defect)` when `take` would break it.
    Other(Box<Lease>, Option<String>),
    Free,
}

/// Pure over the row: `defect` is [`defect`] with the clock and pid probe supplied.
pub fn standing(
    lease: Option<Lease>,
    me: &str,
    defect: impl Fn(&Lease) -> Option<String>,
) -> Standing {
    match lease {
        None => Standing::Free,
        Some(l) if l.worker == me => match defect(&l) {
            Some(d) if d.starts_with("dead") => Standing::MineDead(d),
            _ => Standing::Mine,
        },
        Some(l) => {
            let d = defect(&l);
            Standing::Other(Box::new(l), d)
        }
    }
}

/// The live verdict for `me` on one resource, read from the ledger.
pub fn standing_now(
    ledger: &air_ledger::Ledger,
    resource: &str,
    me: &str,
) -> Result<Standing, String> {
    let row = ledger.lease(resource).map_err(|e| e.to_string())?;
    let t = now();
    let stale = stale_secs();
    Ok(standing(row, me, |l| defect(l, &t, stale)))
}

/// What to tell a session that needs `need` and stands at `s`. None when it holds the lease.
pub fn advice(need: &Need, s: &Standing, me: &str) -> Option<String> {
    let r = &need.resource;
    let take = format!("air lease take {r} --reason \"<why>\"");
    let head = format!(
        "`{}` needs the `{r}` lease (pattern `{}` under \"leases\" in .claude/air.json), and {me} does not hold it.",
        need.segment, need.pattern
    );
    match s {
        Standing::Mine => None,
        Standing::MineDead(d) => Some(format!(
            "`{}` needs the `{r}` lease. {me} holds it from a session that is {d}; re-claim it: {take}",
            need.segment
        )),
        Standing::Free => Some(format!(
            "{head} It is free: {take}, then re-run; release it when done."
        )),
        Standing::Other(l, Some(d)) => Some(format!(
            "{head} It is held by {} but {d}, so taking it clears it: {take}",
            l.worker
        )),
        Standing::Other(l, None) => Some(format!(
            "{head} It is HELD by {} (reason: {}); `{take}` records that you want it (shown in `air lease status`), and succeeds once it is released. Meanwhile take work that does not need it, or ask the coordinator; do not route around it.",
            l.worker, l.reason
        )),
    }
}

/// `air lease needs "<command>"`: which lease(s) the command needs and whether this session
/// holds them, from the rules the hook enforces. Exit 0 whatever the answer: asking must never
/// fail a build.
pub fn needs_cmd(repo: &Path, cmd: &str, json: bool) -> i32 {
    let (ledger, derived) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air lease: {e}");
            return 1;
        }
    };
    // The hook's identity, so the answer is the one the hook will give.
    let me = crate::cmd::hook::identity_from(
        std::env::var("AIR_ROLE").ok().as_deref(),
        std::env::var("BEADS_ACTOR").ok().as_deref(),
        &derived,
    );
    let decl = declared(repo);
    let rows: Vec<serde_json::Value> = needs(&decl, cmd)
        .into_iter()
        .map(|n| {
            let s = standing_now(&ledger, &n.resource, &me).unwrap_or(Standing::Free);
            let holder = match &s {
                Standing::Mine | Standing::MineDead(_) => Some(me.clone()),
                Standing::Other(l, _) => Some(l.worker.clone()),
                Standing::Free => None,
            };
            serde_json::json!({
                "resource": n.resource, "pattern": n.pattern, "segment": n.segment,
                "held": s == Standing::Mine, "holder": holder, "advice": advice(&n, &s, &me),
            })
        })
        .collect();
    let patterns: usize = decl.iter().map(|(_, p)| p.len()).sum();
    emit(
        json,
        &serde_json::json!({"worker": me, "needs": rows, "patterns_declared": patterns}),
        || {
            if rows.is_empty() {
                return format!(
                    "needs no lease ({patterns} pattern(s) under \"leases\" in .claude/air.json)"
                );
            }
            rows.iter()
                .map(|r| {
                    let res = r["resource"].as_str().unwrap_or("");
                    match r["advice"].as_str() {
                        None => format!("needs {res}: held by you ({me}); run it"),
                        Some(a) => format!("needs {res}: {a}"),
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        },
    );
    0
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn segments_split_like_the_harness_and_keep_quotes_and_heredocs_whole() {
        assert_eq!(segments("cd app && make api"), vec!["cd app", "make api"]);
        assert_eq!(
            segments("FOO=1 timeout -k 5 60 nice -n 3 nohup make api 2>&1 | tee x; ls || true"),
            vec!["make api 2>&1", "tee x", "ls", "true"]
        );
        assert_eq!(
            segments("bd create -d \"then make api && adb shell\""),
            vec!["bd create -d \"then make api && adb shell\""]
        );
        assert_eq!(
            segments("git commit -F- <<'EOF'\nadb shell\nEOF\ngit log"),
            vec!["git commit -F- <<'EOF'", "git log"]
        );
    }

    #[test]
    fn patterns_are_the_deny_syntax_anchored_at_the_command() {
        assert!(pattern_matches("make api*", "make api"));
        assert!(pattern_matches("make api*", "make api-dev"));
        assert!(pattern_matches("Bash(adb *)", "adb"));
        assert!(pattern_matches("Bash(adb *)", "adb shell ls"));
        assert!(!pattern_matches("adb *", "adbx"));
        assert!(!pattern_matches("adb *", "git grep adb"));
        assert!(pattern_matches("docker * up*", "docker compose up -d"));
        let decl = declared_from(&serde_json::json!({"runtime": ["make api*"], "db": ["psql *"]}));
        let n = needs(&decl, "make test && make api");
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].resource, "runtime");
        assert!(needs(&decl, "make test").is_empty());
    }

    #[test]
    fn the_gate_refuses_a_session_without_the_lease() {
        use air_hooks::HookOutcome;
        use air_ledger::leases::Holder;
        let l = air_ledger::Ledger::open_in_memory().unwrap();
        let decl = declared_from(&serde_json::json!({"runtime": ["make api*"]}));
        let h = Holder {
            worker: "w2",
            ..Holder::default()
        };
        l.lease_take("runtime", &h, "api", "t0", |_| None).unwrap();
        let d = crate::cmd::hook::lease_gate(
            &l,
            "w1",
            "worker",
            &decl,
            "cd app && make api",
            true,
            |_| None,
        )
        .unwrap()
        .map(|d| d.outcome);
        assert!(matches!(d, Some(HookOutcome::Block { .. })), "{d:?}");
    }
}
