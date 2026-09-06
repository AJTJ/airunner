//! `air record <kind> -- <command…>`: run the check, record (worker, HEAD, kind, exit).
//!
//! The exit is the fact. Around it Air records what the adopter's captures 4d1e52/9de453/38b0c1
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
    // air-7wf: the tree is recorded with the run, so a green can be found again from the
    // landing commit `air land` builds over it. A tree that cannot be read is recorded as
    // unknown, which never matches; the exit is still the fact.
    let tree = super::green::tree_of(repo, &head).ok();
    // air-80x.4: the worker branch heads this commit contains that main does not, recorded
    // with the run so a red at a batch head names its members without a landing row.
    // air-9ij: main's own sha goes on the row too, read BEFORE the command runs, because the
    // tree about to be checked was built over this main and not over whatever main is when
    // somebody asks later. That is what makes "the green contains main" a recorded fact
    // instead of one that expires the next time anyone writes to main.
    let main_sha = git::run(repo, &["rev-parse", "main"]).ok();
    let members = main_sha
        .as_deref()
        .map(|tip| super::batch::members_of(repo, &worker, &head, tip))
        .unwrap_or_default();
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
    // air-4cr: publish the run BEFORE it starts, so "someone is mid-verify" is a lookup.
    // The adopter's coordinator invalidated three workers' verifies by landing under them and
    // had no way to know; their fix was a hand protocol where the worker warns first. A full
    // verify is ~420 s and the landing rate is faster, so no cadence works — only the fact.
    let id = new_id();
    let in_flight = air_ledger::verify::InFlight {
        id: id.clone(),
        worker: worker.clone(),
        sha: head.clone(),
        kind,
        command: command_line.clone(),
        pid: Some(i64::from(std::process::id())),
        started_at: started_at.clone(),
    };
    if let Err(e) = ledger.verify_started(&in_flight) {
        // Never fail the check over the announcement: the exit is the fact, this is courtesy.
        eprintln!("air record: could not publish the in-flight row: {e}");
    }
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
    let _ = ledger.verify_finished(&id);
    let run = VerifyRun {
        id,
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
        tree,
        members,
        main_sha,
    };
    if let Err(e) = ledger.record_verify(&run) {
        eprintln!("air record: could not write ledger: {e}");
        return 1;
    }
    // air-ppm: a kill is not a verdict. The exit is still recorded and still mirrored below;
    // only what it is CALLED changes, and the ledger's green/red/flaky queries skip it.
    let decision = match run.verdict() {
        air_ledger::verify::Verdict::Green => "green",
        air_ledger::verify::Verdict::Red => "red",
        air_ledger::verify::Verdict::Killed => "killed",
    };
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
    // Counted at the commit, every worker (air-7wf): a peer's red at this sha and my green
    // are two verdicts on one commit, which is the disagreement this flag exists to show.
    let (greens, reds) = ledger.runs_at(&head, kind).unwrap_or((0, 0));
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
    // air-80x.4: a red at a batch head is reported by member, so the lane can split by hand.
    // Nothing else changes on a red: no landing, no close, no claim.
    if run.verdict() == air_ledger::verify::Verdict::Red && !run.members.is_empty() {
        let red = super::batch::red_batches_of(std::slice::from_ref(&run));
        if let Some(b) = red.first() {
            eprintln!("air record: {}", super::batch::red_batch_line(b));
        }
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
        let note = if run.is_killed() {
            " (signalled before it could decide: no verdict recorded for this sha)"
        } else {
            ""
        };
        format!(
            "recorded {} {} for {} at {}: exit {}{note}",
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

/// A green faster than this, or with no output, is flagged (the adopter's capture 38b0c1).
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
    Ok((exit_of(&status), bytes))
}

/// The child's exit as a shell would report it: its code, or 128 + the signal that killed it
/// (air-ppm). A signalled child used to become -1, which `is_green` read as red; 128 + signal
/// is the same number `make` and every shell produce for that death, so SIGTERM and SIGKILL
/// land on [`air_ledger::verify::KILLED_EXITS`] whether the kill reached the child through
/// `make` or directly.
#[cfg(unix)]
pub fn exit_of(status: &std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    match (status.code(), status.signal()) {
        (Some(c), _) => c,
        (None, Some(sig)) => 128i32.saturating_add(sig),
        (None, None) => -1,
    }
}

#[cfg(not(unix))]
pub fn exit_of(status: &std::process::ExitStatus) -> i32 {
    status.code().unwrap_or(-1)
}
