//! `air record <kind> -- <command…>`: run the check, record (worker, HEAD, kind, exit).

use std::path::Path;
use std::process::{Command, Stdio};

use air_ledger::verify::{Kind, VerifyRun, new_id};

use crate::cmd::{emit, log_event, now, open};
use crate::git;

pub fn run(repo: &Path, kind: &str, command: &[String], json: bool) -> i32 {
    let Some(kind) = Kind::parse(kind) else {
        eprintln!("air record: kind must be one of verify | docs-check | fitness");
        return 1;
    };
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air record: {e}");
            return 1;
        }
    };
    let head = match git::head(repo) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("air record: {e}");
            return 1;
        }
    };
    let Some((prog, args)) = command.split_first() else {
        eprintln!("air record: missing command after --");
        return 1;
    };
    let started_at = now();
    // Inherit stdio so the operator sees the check's own output; we only need the exit code.
    let status = Command::new(prog)
        .args(args)
        .current_dir(repo)
        .stdin(Stdio::null())
        .status();
    let finished_at = now();
    let exit_code = match status {
        Ok(s) => s.code().unwrap_or(-1),
        Err(e) => {
            eprintln!("air record: could not run {prog}: {e}");
            -1
        }
    };
    let run = VerifyRun {
        id: new_id(),
        worker: worker.clone(),
        sha: head.clone(),
        kind,
        exit_code,
        trigger: "record".into(),
        failing_step: None,
        started_at,
        finished_at,
        log_path: None,
    };
    if let Err(e) = ledger.record_verify(&run) {
        eprintln!("air record: could not write ledger: {e}");
        return 1;
    }
    let decision = if run.is_green() { "green" } else { "red" };
    log_event(
        &ledger,
        &worker,
        "record",
        &serde_json::json!({"kind": kind.as_str(), "sha": head, "command": command}),
        decision,
        &format!("exit {exit_code}"),
        "1 run",
    );
    emit(json, &run, || {
        format!(
            "recorded {} {} for {} at {}: exit {}",
            decision,
            kind.as_str(),
            worker,
            head.get(..7).unwrap_or(&head),
            exit_code
        )
    });
    // Mirror the check's exit so `air record verify -- make verify` behaves like `make verify`.
    if exit_code == 0 { 0 } else { 1 }
}
