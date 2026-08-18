//! `air hook`: the Claude Code hook entrypoint. Reads hook JSON on stdin, dispatches by
//! event, and is **fail-open**: any internal error → exit 0 with an event line, never a
//! blocked tool call. Budget: p99 ≤ 150 ms (tick 0315); no `bd` calls here.
//!
//! First slice behaviour (plan 0001 §5):
//! - PostToolUse(Edit|Write): journal the touched file.
//! - PreToolUse(Edit|Write): warn (additionalContext) if a peer is journaled on that file.
//! - PreToolUse(Bash `bd close`/`bd update … -s awaiting_review|closed`): run the hand-over gate,
//!   advisory (context) unless AIR_ENFORCE=1 (then exit 2 with the reason).
//! - Stop / SubagentStop: advisory hand-over summary as context; never blocks in this slice.
//! - SessionStart / PostToolUse / PermissionRequest / SessionEnd: session state rows.

use std::io::Read;
use std::path::{Path, PathBuf};

use air_hooks::{HookEvent, HookInput, HookOutcome, handover_verdict, journal};
use air_ledger::Ledger;
use rusqlite::params;

use crate::cmd::{handover, log_event, now, open};
use crate::git;

pub fn run(repo: &Path) -> i32 {
    // Everything below is wrapped so a panic or error becomes "allow" + a log line.
    let result = std::panic::catch_unwind(|| inner(repo));
    match result {
        Ok(Ok((event, outcome))) => {
            let code = outcome.exit_code();
            match &outcome {
                HookOutcome::Allow { context: Some(_) } => {
                    // JSON on stdout carries additionalContext.
                    if let Some(v) = outcome.stdout_json(event) {
                        println!("{v}");
                    }
                }
                HookOutcome::Allow { context: None } => {}
                HookOutcome::Block { reason } => eprintln!("{reason}"),
            }
            code
        }
        Ok(Err(e)) => {
            eprintln!("air hook: fail-open: {e}");
            0
        }
        Err(_) => {
            eprintln!("air hook: fail-open: panic");
            0
        }
    }
}

fn inner(repo: &Path) -> Result<(HookEvent, HookOutcome), String> {
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .map_err(|e| e.to_string())?;
    let input = HookInput::parse(&raw).map_err(|e| e.to_string())?;
    let cwd = input
        .cwd
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.to_path_buf());
    let (ledger, worker) = open(&cwd)?;
    let event = input.event();
    let outcome = match event {
        HookEvent::SessionStart => {
            set_session(&ledger, &input, &worker, "working", None)?;
            HookOutcome::Allow {
                context: Some(format!("air: worker `{worker}` session registered")),
            }
        }
        HookEvent::SessionEnd => {
            ledger
                .conn()
                .execute(
                    "DELETE FROM sessions WHERE session_id=?1",
                    params![input.session_id],
                )
                .map_err(|e| e.to_string())?;
            HookOutcome::Allow { context: None }
        }
        HookEvent::PermissionRequest => {
            set_session(
                &ledger,
                &input,
                &worker,
                "stuck",
                input.tool_name.as_deref(),
            )?;
            HookOutcome::Allow { context: None }
        }
        HookEvent::PreToolUse => pre_tool_use(&ledger, &worker, &cwd, &input)?,
        HookEvent::PostToolUse => {
            set_session(&ledger, &input, &worker, "working", None)?;
            if let Some(abs) = input.edited_path()
                && let Ok(root) = git::toplevel(&cwd)
                && let Some(rel) = journal::relative_to(&root, Path::new(&abs))
            {
                journal::touch(&ledger, &worker, &rel, Some(&input.session_id), &now())
                    .map_err(|e| e.to_string())?;
            }
            HookOutcome::Allow { context: None }
        }
        HookEvent::Stop | HookEvent::SubagentStop => {
            set_session(&ledger, &input, &worker, "idle", None)?;
            // Advisory only in this slice; never block, and never when stop_hook_active.
            let f = handover::facts(&ledger, &worker, &cwd, None, true)?;
            let v = handover_verdict(&f);
            log_event(
                &ledger,
                &worker,
                "hook.stop",
                &serde_json::json!({"head": f.head}),
                if v.pass { "pass" } else { "would-refuse" },
                &v.message,
                "3 checks",
            );
            HookOutcome::Allow {
                context: Some(format!("air: {}", v.message)),
            }
        }
        _ => HookOutcome::Allow { context: None },
    };
    Ok((event, outcome))
}

fn pre_tool_use(
    ledger: &Ledger,
    worker: &str,
    cwd: &Path,
    input: &HookInput,
) -> Result<HookOutcome, String> {
    set_session(ledger, input, worker, "running", input.tool_name.as_deref())?;
    // Peer-on-file warning.
    if let Some(abs) = input.edited_path() {
        if let Ok(root) = git::toplevel(cwd)
            && let Some(rel) = journal::relative_to(&root, Path::new(&abs))
        {
            let peers = journal::peers_on(ledger, worker, &rel).map_err(|e| e.to_string())?;
            if !peers.is_empty() {
                return Ok(HookOutcome::Allow {
                    context: Some(format!(
                        "air: {} is also being edited by {} — coordinate before overlapping edits (run `air peer <name>` for their green sha)",
                        rel,
                        peers.join(", ")
                    )),
                });
            }
        }
        return Ok(HookOutcome::Allow { context: None });
    }
    // Hand-over gate on bd status writes.
    if let Some(cmd) = input.bash_command()
        && is_handover_command(cmd)
    {
        let enforce = std::env::var("AIR_ENFORCE").is_ok_and(|v| v == "1");
        let f = handover::facts(ledger, worker, cwd, None, !enforce)?;
        let v = handover_verdict(&f);
        log_event(
            ledger,
            worker,
            "hook.handover",
            &serde_json::json!({"command": cmd, "head": f.head, "enforce": enforce}),
            if v.pass {
                "pass"
            } else if v.block {
                "refuse"
            } else {
                "would-refuse"
            },
            &v.message,
            "3 checks",
        );
        if v.block {
            return Ok(HookOutcome::Block {
                reason: format!("air: {}", v.message),
            });
        }
        if !v.pass {
            return Ok(HookOutcome::Allow {
                context: Some(format!("air: {}", v.message)),
            });
        }
    }
    Ok(HookOutcome::Allow { context: None })
}

/// Does this shell command hand a bead over? `bd close …`, or `bd update … -s/--status
/// awaiting_review|closed`. Deliberately narrow: WIP commits and merges are never matched.
pub fn is_handover_command(cmd: &str) -> bool {
    let toks: Vec<&str> = cmd.split_whitespace().collect();
    // `bd` must start a command: first token, or right after a shell separator.
    let starts: Vec<usize> = toks
        .iter()
        .enumerate()
        .filter(|(i, t)| {
            **t == "bd"
                && (*i == 0
                    || matches!(
                        toks.get(i.wrapping_sub(1)),
                        Some(&"&&")
                            | Some(&";")
                            | Some(&"||")
                            | Some(&"|")
                            | Some(&"(")
                            | Some(&"{")
                    ))
        })
        .map(|(i, _)| i)
        .collect();
    for i in starts {
        let rest = toks.get(i.saturating_add(1)..).unwrap_or(&[]);
        match rest.first() {
            Some(&"close") => return true,
            Some(&"update") => {
                let hit = rest.windows(2).any(|w| {
                    matches!(w, [a, b] if (*a == "-s" || *a == "--status")
                        && (*b == "awaiting_review" || *b == "closed"))
                }) || rest
                    .iter()
                    .any(|t| *t == "--status=awaiting_review" || *t == "--status=closed");
                if hit {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn set_session(
    ledger: &Ledger,
    input: &HookInput,
    worker: &str,
    state: &str,
    detail: Option<&str>,
) -> Result<(), String> {
    let t = now();
    ledger
        .conn()
        .execute(
            "INSERT INTO sessions (session_id, worker, transcript_path, state, detail, changed_at, started_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?6) \
             ON CONFLICT(session_id) DO UPDATE SET state=excluded.state, detail=excluded.detail, \
             changed_at=excluded.changed_at, transcript_path=COALESCE(excluded.transcript_path, sessions.transcript_path)",
            params![input.session_id, worker, input.transcript_path, state, detail, t],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_handover_command;

    #[test]
    fn matches_only_handover_writes() {
        assert!(is_handover_command("bd close fd-1 --reason done"));
        assert!(is_handover_command("bd update fd-1 -s awaiting_review"));
        assert!(is_handover_command("bd update fd-1 --status closed"));
        assert!(is_handover_command(
            "bd update fd-1 --status=awaiting_review"
        ));
        assert!(!is_handover_command("bd update fd-1 -s in_progress"));
        assert!(!is_handover_command("bd update fd-1 --claim"));
        assert!(!is_handover_command("git commit -am wip"));
        assert!(!is_handover_command("git merge worktree-x"));
        assert!(!is_handover_command("echo bd close"));
    }
}
