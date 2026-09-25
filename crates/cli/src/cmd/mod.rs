//! Subcommands. Each returns a process exit code (0 ok, 1 error, 2 refused).

pub mod acceptance;
pub mod attribution;
pub mod audit;
pub mod batch;
pub mod bd_latency;
pub mod budgets;
pub mod capture;
pub mod claim;
pub mod close;
pub mod doctor;
pub mod gc;
pub mod green;
pub mod handover;
pub mod holdings;
pub mod hook;
pub mod init;
pub mod install;
pub mod land;
pub mod launch;
pub mod lease;
pub mod mcp;
pub mod mechanisms;
pub mod metis;
pub mod privacy;
pub mod readers;
pub mod ready_cache;
pub mod record;
pub mod runlog;
pub mod selftest;
pub mod status;
pub mod tmux;
pub mod worktree;

use std::path::Path;

use air_ledger::{Ledger, paths};

/// Current time as RFC 3339 UTC. The ledger never reads the clock itself; commands do, once.
pub fn now() -> String {
    jiff::Timestamp::now().to_string()
}

pub fn today() -> String {
    now().get(..10).unwrap_or("1970-01-01").to_string()
}

/// Open the shared ledger and resolve the worker name for `repo`.
pub fn open(repo: &Path) -> Result<(Ledger, String), String> {
    let ledger = Ledger::open_for_repo(repo).map_err(|e| e.to_string())?;
    let worker = paths::worker_name_for(repo).map_err(|e| e.to_string())?;
    Ok((ledger, worker))
}

/// The role this process runs as, from `AIR_ROLE`, which only the launchers set.
///
/// Permissions never come from the directory a command runs in: any role can work in a
/// worktree, and `--repo` is an argument the caller writes. No `AIR_ROLE` means a shell Air did
/// not start, which is the owner, who may do anything. A value Air does not know is a worker.
pub fn caller_role() -> &'static str {
    role_from(std::env::var("AIR_ROLE").ok().as_deref())
}

/// `caller_role` over a given value, so tests do not depend on the environment they run in.
pub fn role_from(value: Option<&str>) -> &'static str {
    match value {
        None | Some("") => "owner",
        Some("coordinator") => "coordinator",
        _ => "worker",
    }
}

/// Print a value as JSON or as its text form.
pub fn emit<T: serde::Serialize>(json: bool, value: &T, text: impl FnOnce() -> String) {
    if json {
        match serde_json::to_string_pretty(value) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("air: json error: {e}"),
        }
    } else {
        println!("{}", text());
    }
}

/// Append an event line; errors are reported to stderr, never fatal. Commands that shelled
/// out to `bd` carry `bd_ms`/`bd_calls` (air-869): bd costs ~1.4 s per process on this
/// machine and the cost was invisible in the record.
pub fn log_event<T: serde::Serialize>(
    ledger: &Ledger,
    worker: &str,
    command: &str,
    inputs: &T,
    decision: &str,
    reason: &str,
    denominator: &str,
) {
    let at = now();
    // This event's own share, not the process's running total: `air mcp` emits one line per
    // poll tick for the life of the server, and the total restamped on each of them summed
    // to numbers no fleet ever made (air-bp0).
    let (bd_ms, bd_calls) = air_bd::stats::take();
    // Same take-not-snapshot semantics, and for the same reason (air-d75, air-bp0).
    let waits = air_ledger::budgets::take();
    let ev = air_ledger::events::Event {
        at: &at,
        worker,
        command,
        inputs,
        decision,
        reason,
        denominator,
        bd_ms: (bd_calls > 0).then_some(bd_ms),
        bd_calls: (bd_calls > 0).then_some(bd_calls),
        budgets: (!waits.is_empty()).then_some(&waits),
    };
    if let Err(e) = air_ledger::events::append(ledger.dir(), &today(), &ev) {
        eprintln!("air: could not append event: {e}");
    }
}
