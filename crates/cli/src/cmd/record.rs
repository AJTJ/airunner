//! `air record <kind> -- <command…>`: run the check, record (worker, HEAD, kind, exit).
//!
//! The exit is the fact. Around it Air records what the adopter's captures 4d1e52/9de453/38b0c1
//! showed a human cannot see in a log: the exact command, how long it took, how much it
//! printed, and whether the tree was dirty. Two facts are named beside the exit:
//! `command-changed` when this worker's previous run of the same kind used a different command
//! line, and the green and red counts at the commit when it has more than one run. Air states
//! these and draws no conclusion from them (air-qlx3: the `suspicious` and `flaky-at-head`
//! labels were opinions, and both were wrong in an adopter's round). Backgrounded commands
//! (`&`) are refused: Air spawns the check itself and must see it finish.
//!
//! Every kind goes through this one path. `precheck` is a worker's cheap check under a
//! verification lane (2026-09-25): recorded like a verify, and read only by the batch-ready
//! rule where the repo declares `"precheck": true`, never by a green query for `verify`.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use air_ledger::verify::{Kind, VerifyRun, new_id};

use crate::cmd::{emit, log_event, now, open};
use crate::git;

pub fn run(repo: &Path, kind: &str, command: &[String], json: bool) -> i32 {
    let Some(kind) = Kind::parse(kind) else {
        eprintln!("air record: kind must be one of verify | docs-check | fitness | precheck");
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
    // D2 (0.4.6 trial): the members `air batch cut` recorded at this head come first. A batch
    // of one fast-forwards, so its merge commits name nobody and the member heard nothing.
    let members = match ledger.batch_cut_members(&head, &worker) {
        Ok(Some(m)) => m,
        _ => main_sha
            .as_deref()
            .map(|tip| super::batch::members_of(repo, &worker, &head, tip))
            .unwrap_or_default(),
    };
    // air-88av: an unresolved merge is not a verdict about the code. The suite fails on
    // conflict markers, `air record` writes a RED at this sha, and that red is then read as
    // evidence by everything downstream — `flaky-at-head`, the close gate, the landing gate,
    // and any later question about whether this tree was ever green. A false red is the record
    // being wrong about a fact nobody will re-derive.
    //
    // Refused rather than recorded-and-flagged, because there is no verdict to keep: the run
    // would be measuring conflict markers. Distinguishable from a red by never writing a row
    // and by saying so — a reader must not see "no green at this sha" and conclude the suite
    // failed here.
    //
    // NOT a dirty-tree refusal. `air record` is deliberately usable on a dirty tree, the
    // `dirty` column exists for exactly that, and verifying uncommitted work is normal.
    // Unmerged is not dirty: it is a state git will not let you commit.
    match git::unmerged_files(repo) {
        Ok(paths) if !paths.is_empty() => {
            eprintln!(
                "air record: refusing to record — {} path(s) have an unresolved merge conflict, \
                 so a run here would measure conflict markers and write a red that is not a \
                 verdict about the code. NOTHING was recorded and this is not a failing suite. \
                 Resolve, then re-run:\n  {}",
                paths.len(),
                paths.join("\n  ")
            );
            return 2;
        }
        Ok(_) => {}
        // A lookup that cannot run is not evidence of a conflict; carry on and record.
        Err(e) => eprintln!("air record: could not check for unmerged paths ({e}); continuing"),
    }
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
    let previous_run = ledger.latest_run_any(&worker, kind).ok().flatten();
    let previous = previous_run.as_ref().and_then(|r| r.command.clone());
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
    // air-nwat: a Ctrl-C in the pane reaches this process as well as the check, and by default
    // it died before writing anything, so a verify stopped by hand left no row at all.
    let stop = Interrupt::register();
    let t0 = std::time::Instant::now();
    let (exit_code, output_bytes, tail) = match run_tee_to(
        prog,
        args,
        repo,
        std::io::stdout(),
        std::io::stderr(),
        stop.as_ref(),
    ) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air record: could not run {prog}: {e}");
            (-1, 0, Vec::new())
        }
    };
    let duration_ms = i64::try_from(t0.elapsed().as_millis()).unwrap_or(i64::MAX);
    let finished_at = now();
    let _ = ledger.verify_finished(&id);
    // air-5ik: keep what a NON-GREEN run printed. Red and killed both: a kill is not a verdict
    // (air-ppm), but its output is exactly what somebody wants to see, and it is the run most
    // likely to be a machine under load rather than a defect. A green leaves nothing.
    let log_path = super::runlog::keeps_output(exit_code)
        .then(|| super::runlog::write(ledger.dir(), &id, &tail))
        .flatten()
        .map(|p| p.display().to_string());
    // A signal that the check outlived is not an interruption: it finished and its exit stands.
    let interrupted = stop
        .as_ref()
        .and_then(Interrupt::which)
        .filter(|_| exit_code != 0)
        .map(|sig| format!("{INTERRUPTED} ({sig})"));
    let run = VerifyRun {
        id,
        worker: worker.clone(),
        sha: head.clone(),
        kind,
        exit_code,
        trigger: "record".into(),
        failing_step: interrupted.clone(),
        started_at,
        finished_at,
        log_path: log_path.clone(),
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
        air_ledger::verify::Verdict::Green => super::decisions::RECORD_GREEN,
        air_ledger::verify::Verdict::Red => super::decisions::RECORD_RED,
        air_ledger::verify::Verdict::Killed => super::decisions::RECORD_KILLED,
    };
    let mut flags: Vec<String> = Vec::new();
    if previous.as_deref().is_some_and(|p| p != command_line) {
        flags.push("command-changed".to_string());
    }
    if dirty {
        flags.push("dirty-tree".to_string());
    }
    // Counted at the commit, every worker (air-7wf): a peer's red at this sha and my green
    // are two verdicts on one commit. Stated as counts only (air-qlx3): the label that used to
    // go with them blamed "the repo's test" for an adopter's missing build directory.
    let (greens, reds) = ledger.runs_at(&head, kind).unwrap_or((0, 0));
    if greens.saturating_add(reds) > 1 {
        flags.push(format!("at this commit: {greens} green / {reds} red"));
    }
    if let Some(i) = &interrupted {
        flags.push(format!("{i} after {duration_ms} ms"));
    }
    let reason = if flags.is_empty() {
        format!("exit {exit_code} in {duration_ms} ms, {output_bytes} bytes")
    } else {
        format!(
            "exit {exit_code} in {duration_ms} ms, {output_bytes} bytes; {}",
            flags.join(", ")
        )
    };
    if !flags.is_empty() {
        eprintln!("air record: {}", flags.join(", "));
    }
    // air-80x.4: a red at a batch head is reported by member, so the lane can split by hand.
    // Nothing else changes on a red: no landing, no close, no claim. A batch is a verify; a
    // worker's precheck over a branch that merged a peer's is not one.
    if kind == Kind::Verify
        && run.verdict() == air_ledger::verify::Verdict::Red
        && !run.members.is_empty()
    {
        let red = super::batch::red_batches_of(std::slice::from_ref(&run));
        if let Some(b) = red.first() {
            eprintln!("air record: {}", super::batch::red_batch_line(b));
        }
    }
    log_event(
        &ledger,
        &worker,
        decision,
        &serde_json::json!({"kind": kind.as_str(), "sha": head, "command": command, "flags": flags}),
        &reason,
        "1 run",
    );
    // air-1vri.2: a batch's members hear its result now, not on their next wake.
    let batch = kind == Kind::Verify && !run.members.is_empty();
    if batch {
        super::fanout::batch_result(repo, &ledger, &run);
    }
    emit(json, &run, || {
        let note = if run.is_killed() {
            " (signalled before it could decide: no verdict recorded for this sha)"
        } else {
            ""
        };
        // air-5ik: the path, on the line the person who just ran it reads, so nobody has to
        // know the layout or that the store exists.
        let log = log_path
            .as_deref()
            .map(|p| format!("; output kept at {p}"))
            .unwrap_or_default();
        format!(
            "recorded {} {} for {} at {}: exit {}{note}{log}",
            decision.decision,
            kind.as_str(),
            worker,
            head.get(..7).unwrap_or(&head),
            exit_code
        )
    });
    // air-1vri.2: the lane's next step, from what Air knows now. Text only; `--json` stays the
    // run's own shape.
    if batch && !json {
        for line in after_batch(repo, &ledger, &worker, &run) {
            println!("{line}");
        }
    }
    // Mirror the check's exit so `air record verify -- make verify` behaves like `make verify`.
    if exit_code == 0 { 0 } else { 1 }
}

/// What the lane does after recording a batch: land a green one, or, after a red or a kill,
/// cut again from the branches batch-ready now.
fn after_batch(
    repo: &Path,
    ledger: &air_ledger::Ledger,
    lane: &str,
    run: &VerifyRun,
) -> Vec<String> {
    if run.is_green() {
        return vec![format!("next: air land --worker {lane}")];
    }
    let (ready, _, _) = super::status::batch_ready_for(ledger, repo);
    let ready: Vec<_> = ready.into_iter().filter(|b| b.worker != lane).collect();
    if super::caller_role() == "lane" {
        super::fanout::told_lane(ledger, lane, &ready, &now());
    }
    // air-ch9p: a kill is no verdict and has nothing to split; a batch of one has nothing to
    // split either, and its member stays out until it moves (air-hyli).
    let first = if run.is_killed() {
        "killed: no verdict and nothing lands; `air batch cut` cuts again".to_string()
    } else if run.members.len() == 1 {
        "red: nothing lands; its only member is batch-ready again after a new commit".to_string()
    } else {
        "red: nothing lands on this batch; split it by its members, then cut again".to_string()
    };
    let mut v = vec![first];
    v.extend(super::fanout::next_cut_lines(&ready));
    v
}

/// Run the check, streaming its output to ours while counting bytes. Returns (exit, bytes).
/// Run the check, mirroring its output, and answer `(exit, bytes, tail)`.
///
/// air-5ik: the tail is the last [`super::runlog::TAIL_BYTES`] of what the child printed,
/// stdout and stderr interleaved in the order they arrived — the same order the terminal
/// showed — so a red run can be read after the fact. It is buffered for EVERY run because the
/// verdict is not known until the child exits; only a non-green one is written to disk.
pub fn run_tee(prog: &str, args: &[String], repo: &Path) -> std::io::Result<(i32, i64, Vec<u8>)> {
    run_tee_to(prog, args, repo, std::io::stdout(), std::io::stderr(), None)
}

/// What a run stopped by a signal is marked with, in `failing_step` (air-nwat).
pub const INTERRUPTED: &str = "interrupted";

/// The signals that stop `air record` by hand, caught so the run is still written.
///
/// SIGINT is the pane's Ctrl-C, which the check receives too, so the check stops itself and this
/// only has to wait for it. SIGTERM and SIGHUP are sent to this process alone (`kill`, a closed
/// pane), so the check is stopped on its behalf. A handler does not survive `exec`, so the check
/// still dies on the signals it would have died on.
#[derive(Debug)]
pub struct Interrupt {
    int: std::sync::Arc<std::sync::atomic::AtomicBool>,
    term: std::sync::Arc<std::sync::atomic::AtomicBool>,
    hup: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Interrupt {
    /// None when a handler could not be installed: the run proceeds exactly as before.
    pub fn register() -> Option<Self> {
        use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
        let me = Self {
            int: std::sync::Arc::default(),
            term: std::sync::Arc::default(),
            hup: std::sync::Arc::default(),
        };
        signal_hook::flag::register(SIGINT, std::sync::Arc::clone(&me.int)).ok()?;
        signal_hook::flag::register(SIGTERM, std::sync::Arc::clone(&me.term)).ok()?;
        signal_hook::flag::register(SIGHUP, std::sync::Arc::clone(&me.hup)).ok()?;
        Some(me)
    }

    /// The signal that arrived, if any.
    pub fn which(&self) -> Option<&'static str> {
        use std::sync::atomic::Ordering::Relaxed;
        if self.term.load(Relaxed) {
            Some("SIGTERM")
        } else if self.hup.load(Relaxed) {
            Some("SIGHUP")
        } else if self.int.load(Relaxed) {
            Some("SIGINT")
        } else {
            None
        }
    }

    /// A signal the check did not receive itself, so it must be stopped for it.
    fn stops_the_check(&self) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        self.term.load(Relaxed) || self.hup.load(Relaxed)
    }
}

/// [`run_tee`] with the two sinks named instead of hardcoded (air-e21v).
///
/// The sinks were `stdout()` and `stderr()` in the body, which is right for `air record verify`
/// — a worker wants the output live — and wrong for any caller whose own stdout means
/// something. A probe called it with a child that echoes to both streams, and that `out` landed
/// on `air selftest`'s stdout ahead of the JSON array, so `air selftest --json` stopped being
/// parseable and `--prove` reported every declared mutation BROKEN. Ordinary `air selftest` was
/// unaffected and all probes passed, so the gate worked and only the mutation evidence was
/// dead — the evidence this round leaned on repeatedly.
///
/// Failing loudly rather than silently is the mercy in it: the JSON did not become subtly
/// wrong, it stopped parsing at line 1 column 1.
pub fn run_tee_to<O: Write + Send + 'static, E: Write + Send + 'static>(
    prog: &str,
    args: &[String],
    repo: &Path,
    out_sink: O,
    err_sink: E,
    stop: Option<&Interrupt>,
) -> std::io::Result<(i32, i64, Vec<u8>)> {
    let mut child = Command::new(prog)
        .args(args)
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // One buffer for both streams, so the kept tail reads in arrival order rather than as two
    // transcripts a reader has to interleave by hand. The lock is held per 8 KiB chunk, which
    // is the same granularity the writes already have.
    let tail = std::sync::Arc::new(std::sync::Mutex::new(super::runlog::Tail::new(
        super::runlog::TAIL_BYTES,
    )));
    fn pump<R: Read + Send + 'static, W: Write + Send + 'static>(
        mut r: R,
        mut w: W,
        tail: std::sync::Arc<std::sync::Mutex<super::runlog::Tail>>,
    ) -> std::thread::JoinHandle<i64> {
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut n_total: i64 = 0;
            while let Ok(n) = r.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let chunk = buf.get(..n).unwrap_or(&[]);
                let _ = w.write_all(chunk);
                let _ = w.flush();
                if let Ok(mut t) = tail.lock() {
                    t.push(chunk);
                }
                n_total = n_total.saturating_add(i64::try_from(n).unwrap_or(0));
            }
            n_total
        })
    }
    let out = child
        .stdout
        .take()
        .map(|o| pump(o, out_sink, std::sync::Arc::clone(&tail)));
    let err = child
        .stderr
        .take()
        .map(|e| pump(e, err_sink, std::sync::Arc::clone(&tail)));
    let status = match stop {
        None => child.wait()?,
        Some(stop) => {
            use wait_timeout::ChildExt;
            let mut killed = false;
            loop {
                if let Some(s) = child.wait_timeout(std::time::Duration::from_millis(100))? {
                    break s;
                }
                if !killed && stop.stops_the_check() {
                    let _ = child.kill();
                    killed = true;
                }
            }
        }
    };
    let out_bytes = out.and_then(|h| h.join().ok()).unwrap_or(0);
    let err_bytes = err.and_then(|h| h.join().ok()).unwrap_or(0);
    let bytes = out_bytes.saturating_add(err_bytes);
    // Both pumps have joined, so the only other Arc is gone and the lock is uncontended.
    let kept = std::sync::Arc::try_unwrap(tail)
        .ok()
        .and_then(|m| m.into_inner().ok())
        .map(super::runlog::Tail::into_bytes)
        .unwrap_or_default();
    Ok((exit_of(&status), bytes, kept))
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
