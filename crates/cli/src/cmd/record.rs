//! `air record <kind> -- <command…>`: run the check, record (worker, HEAD, kind, exit).
//!
//! The exit is the fact. Around it Air records what adopter's captures 4d1e52/9de453/38b0c1
//! showed a human cannot see in a log: the exact command, how long it took, how much it
//! printed, and whether the tree was dirty. A run is flagged `suspicious` when it was too
//! fast or printed nothing, and `command-changed` when this worker's previous run of the same
//! kind used a different command line. Backgrounded commands (`&`) are refused: Air spawns
//! the check itself and must see it finish.

use std::io::{Read, Write};
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
    if command.iter().any(|a| a == "&" || a == "nohup") || prog == "nohup" {
        eprintln!(
            "air record: refusing a backgrounded command; run it in the foreground so the exit is real"
        );
        return 1;
    }
    let command_line = command.join(" ");
    let previous = ledger
        .latest_run_any(&worker, kind)
        .ok()
        .flatten()
        .and_then(|r| r.command);
    // Air's own directory never counts as dirt (it is gitignored in a configured repo).
    let dirty = git::dirty_files(repo)
        .map(|v| v.iter().any(|p| !p.starts_with(".air/")))
        .unwrap_or(false);
    let started_at = now();
    let t0 = std::time::Instant::now();
    let (exit_code, output_bytes) = match run_tee(prog, args, repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air record: could not run {prog}: {e}");
            (-1, 0)
        }
    };
    let duration_ms = i64::try_from(t0.elapsed().as_millis()).unwrap_or(i64::MAX);
    let finished_at = now();
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
        command: Some(command_line.clone()),
        duration_ms: Some(duration_ms),
        output_bytes: Some(output_bytes),
        dirty,
    };
    if let Err(e) = ledger.record_verify(&run) {
        eprintln!("air record: could not write ledger: {e}");
        return 1;
    }
    let decision = if run.is_green() { "green" } else { "red" };
    let mut flags: Vec<&str> = Vec::new();
    if run.is_green() && (duration_ms < SUSPICIOUS_MS || output_bytes == 0) {
        flags.push("suspicious");
    }
    if previous.as_deref().is_some_and(|p| p != command_line) {
        flags.push("command-changed");
    }
    if dirty {
        flags.push("dirty-tree");
    }
    let (greens, reds) = ledger.runs_at(&worker, &head, kind).unwrap_or((0, 0));
    let flaky = greens > 0 && reds > 0;
    let flaky_note = if flaky {
        format!(
            "; flaky at HEAD: {greens} green / {reds} red (the repo's test is the bug; file it, then re-run)"
        )
    } else {
        String::new()
    };
    if flaky {
        flags.push("flaky-at-head");
    }
    let reason = if flags.is_empty() {
        format!("exit {exit_code} in {duration_ms} ms, {output_bytes} bytes")
    } else {
        format!(
            "exit {exit_code} in {duration_ms} ms, {output_bytes} bytes; {}{flaky_note}",
            flags.join(", ")
        )
    };
    if !flags.is_empty() {
        eprintln!("air record: {}{flaky_note}", flags.join(", "));
    }
    log_event(
        &ledger,
        &worker,
        "record",
        &serde_json::json!({"kind": kind.as_str(), "sha": head, "command": command, "flags": flags}),
        decision,
        &reason,
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

/// A green faster than this, or with no output, is flagged (adopter capture 38b0c1).
const SUSPICIOUS_MS: i64 = 2_000;

/// Run the check, streaming its output to ours while counting bytes. Returns (exit, bytes).
pub fn run_tee(prog: &str, args: &[String], repo: &Path) -> std::io::Result<(i32, i64)> {
    let mut child = Command::new(prog)
        .args(args)
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    fn pump<R: Read + Send + 'static, W: Write + Send + 'static>(
        mut r: R,
        mut w: W,
    ) -> std::thread::JoinHandle<i64> {
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut n_total: i64 = 0;
            while let Ok(n) = r.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let _ = w.write_all(buf.get(..n).unwrap_or(&[]));
                let _ = w.flush();
                n_total = n_total.saturating_add(i64::try_from(n).unwrap_or(0));
            }
            n_total
        })
    }
    let out = child.stdout.take().map(|o| pump(o, std::io::stdout()));
    let err = child.stderr.take().map(|e| pump(e, std::io::stderr()));
    let status = child.wait()?;
    let out_bytes = out.and_then(|h| h.join().ok()).unwrap_or(0);
    let err_bytes = err.and_then(|h| h.join().ok()).unwrap_or(0);
    let bytes = out_bytes.saturating_add(err_bytes);
    Ok((status.code().unwrap_or(-1), bytes))
}
