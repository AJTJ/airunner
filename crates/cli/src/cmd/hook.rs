//! `air hook`: the Claude Code hook entrypoint. Reads hook JSON on stdin, dispatches by
//! event, and is **fail-open**: any internal error → exit 0 with an event line, never a
//! blocked tool call. Budget: p99 ≤ 150 ms (tick 0315); no `bd` calls here.
//!
//! First slice behaviour (plan 0001 §5):
//! - PostToolUse(Edit|Write): journal the touched file.
//! - PreToolUse(Edit|Write): warn (additionalContext) if a peer is journaled on that file.
//! - PreToolUse(Bash `bd close`/`bd update … -s awaiting_review|closed`): run the hand-over gate,
//!   advisory (context) unless AIR_ENFORCE=1 (then exit 2 with the reason).
//! - Stop / SubagentStop: advisory hand-over verdict as context ONLY when something is
//!   missing; quiet on the ok path and for the coordinator. One block: a worker with no claim
//!   while beads are ready is nudged once with the ids (air-09i; `stop_hook_active` is the
//!   loop guard). The cache gates whether to speak; the list itself is confirmed against bd
//!   at that moment, so the nudge never names a bead `air claim` would refuse (air-ouw).
//! - SessionStart / PostToolUse / PermissionRequest / SessionEnd: session state rows.
//!
//! Every invocation appends exactly one event line to `.air/events/` (`hook.<event>`), including
//! the silent ones (journal touches, session transitions, ignored events, fail-open). The
//! `sessions` table is current state; the event stream is the history (owner, 2026-08-20:
//! "all events should be new events, never overwritten").

use std::io::Read;
use std::path::{Path, PathBuf};

use air_hooks::{HookEvent, HookInput, HookOutcome, handover_verdict, journal, stop_nudge};
use air_ledger::Ledger;
use rusqlite::params;

use crate::cmd::{handover, log_event, now, open, ready_cache};
use crate::git;

pub fn run(repo: &Path) -> i32 {
    // Everything below is wrapped so a panic or error becomes "allow" + a log line.
    let mut raw = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut raw) {
        eprintln!("air hook: fail-open: {e}");
        return 0;
    }
    let result = std::panic::catch_unwind(|| inner(repo, &raw));
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
            log_fail_open(repo, &raw, &e);
            0
        }
        Err(_) => {
            eprintln!("air hook: fail-open: panic");
            log_fail_open(repo, &raw, "panic");
            0
        }
    }
}

/// Best effort: a fail-open must still leave a trace when the ledger itself is reachable.
fn log_fail_open(repo: &Path, raw: &str, error: &str) {
    let input = HookInput::parse(raw).ok();
    let cwd = input
        .as_ref()
        .and_then(|i| i.cwd.as_deref())
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.to_path_buf());
    if let Ok((ledger, worker)) = open(&cwd) {
        let name = input
            .as_ref()
            .map(|i| i.event().as_str())
            .unwrap_or("unparsed");
        let session = input
            .as_ref()
            .map(|i| i.session_id.clone())
            .unwrap_or_default();
        log_event(
            &ledger,
            &worker,
            &format!("hook.{name}"),
            &serde_json::json!({"session_id": session}),
            "fail-open",
            error,
            "0 checks",
        );
    }
}

fn inner(repo: &Path, raw: &str) -> Result<(HookEvent, HookOutcome), String> {
    let input = HookInput::parse(raw).map_err(|e| e.to_string())?;
    let cwd = input
        .cwd
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.to_path_buf());
    let (ledger, worker) = open(&cwd)?;
    let event = input.event();
    let d = dispatch(&ledger, &worker, &cwd, &input)?;
    let mut inputs = d.inputs;
    if let serde_json::Value::Object(m) = &mut inputs {
        m.insert("session_id".into(), input.session_id.clone().into());
        if let Some(t) = &input.tool_name {
            m.insert("tool".into(), t.clone().into());
        }
    }
    log_event(
        &ledger,
        &worker,
        &format!("hook.{}", event.as_str()),
        &inputs,
        &d.decision,
        &d.reason,
        &d.denominator,
    );
    Ok((event, d.outcome))
}

/// What one hook invocation decided, in the shape the event line needs.
pub struct Dispatched {
    pub outcome: HookOutcome,
    inputs: serde_json::Value,
    decision: String,
    reason: String,
    denominator: String,
}

impl Dispatched {
    fn new(outcome: HookOutcome, decision: &str, reason: impl Into<String>) -> Self {
        Self {
            outcome,
            inputs: serde_json::json!({}),
            decision: decision.to_string(),
            reason: reason.into(),
            denominator: "0 checks".to_string(),
        }
    }
    fn inputs(mut self, v: serde_json::Value) -> Self {
        self.inputs = v;
        self
    }
    fn denominator(mut self, d: impl Into<String>) -> Self {
        self.denominator = d.into();
        self
    }
}

fn dispatch(
    ledger: &Ledger,
    worker: &str,
    cwd: &Path,
    input: &HookInput,
) -> Result<Dispatched, String> {
    Ok(match input.event() {
        HookEvent::SessionStart => {
            let prev = set_session(ledger, input, worker, "working", None)?;
            // Quiet: the event line records it; a human reads every line a hook prints.
            Dispatched::new(
                HookOutcome::Allow { context: None },
                "registered",
                transition(&prev, "working"),
            )
        }
        HookEvent::SessionEnd => {
            let prev = session_state(ledger, &input.session_id)?;
            ledger
                .conn()
                .execute(
                    "DELETE FROM sessions WHERE session_id=?1",
                    params![input.session_id],
                )
                .map_err(|e| e.to_string())?;
            Dispatched::new(
                HookOutcome::Allow { context: None },
                "ended",
                transition(&prev, "gone"),
            )
            .inputs(serde_json::json!({"reason": input.reason}))
        }
        HookEvent::PermissionRequest => {
            let prev = set_session(ledger, input, worker, "stuck", input.tool_name.as_deref())?;
            Dispatched::new(
                HookOutcome::Allow { context: None },
                "stuck",
                transition(&prev, "stuck"),
            )
        }
        // Friction Air did not cause: a denial by any rule, hook, or the human, or a tool
        // that ran and failed. Observation only; the command and the reason are the record.
        HookEvent::PermissionDenied | HookEvent::PostToolUseFailure => {
            let _ = set_session(ledger, input, worker, "working", None);
            let command = input
                .bash_command()
                .map(str::to_string)
                .or_else(|| input.edited_path())
                .unwrap_or_default();
            let why = input
                .reason
                .clone()
                .or_else(|| input.error.clone())
                .unwrap_or_default();
            let denied = input.event() == HookEvent::PermissionDenied;
            Dispatched::new(
                HookOutcome::Allow { context: None },
                if denied { "denied" } else { "failed" },
                why.chars().take(400).collect::<String>(),
            )
            .inputs(serde_json::json!({"command": command}))
        }
        HookEvent::PreToolUse => pre_tool_use(ledger, worker, cwd, input)?,
        HookEvent::PostToolUse => {
            let prev = set_session(ledger, input, worker, "working", None)?;
            // Every tool call is a sign of life for the leases this worktree holds.
            let _ = ledger.lease_beat(worker, &now());
            let mut d = Dispatched::new(
                HookOutcome::Allow { context: None },
                "observed",
                transition(&prev, "working"),
            );
            if let Some(abs) = input.edited_path()
                && let Ok(root) = git::toplevel(cwd)
                && let Some(rel) = journal::relative_to(&root, Path::new(&abs))
            {
                journal::touch(ledger, worker, &rel, Some(&input.session_id), &now())
                    .map_err(|e| e.to_string())?;
                d.decision = "journaled".to_string();
                d.inputs = serde_json::json!({"path": rel});
            }
            // A closed bead is not held by anyone (air-8p4). PostToolUse is the success
            // signal: a Bash command that exits non-zero arrives as `PostToolUseFailure`
            // instead, which this arm never sees (verified in `.air/events/2026-08-29.ndjson`,
            // three failed Bash calls, all filed as PostToolUseFailure).
            if let Some(cmd) = input.bash_command()
                && let Some(bead) = closes_bead(cmd)
                && ledger
                    .release_claim(&bead, worker, "closed", &now())
                    .unwrap_or(false)
            {
                d.decision = "released".to_string();
                d.reason = format!("{bead} closed; claim released");
                d.inputs = serde_json::json!({"bead": bead, "command": cmd});
            }
            d
        }
        HookEvent::Stop | HookEvent::SubagentStop if role_for(worker) == "coordinator" => {
            // The coordinator holds no lane and never hands over: no advisory (adopter
            // adoption log §9: the coordinator received a worker's hand-over advisory).
            let prev = set_session(ledger, input, worker, "idle", None)?;
            Dispatched::new(
                HookOutcome::Allow { context: None },
                "observed",
                format!(
                    "{}; coordinator: no hand-over check",
                    transition(&prev, "idle")
                ),
            )
        }
        HookEvent::Stop | HookEvent::SubagentStop => {
            let prev = set_session(ledger, input, worker, "idle", None)?;
            // Advisory only in this slice; never block, and never when stop_hook_active.
            let f = handover::facts(ledger, worker, cwd, None, true)?;
            let v = handover_verdict(&f);
            // Nothing to hand over if this worker holds no claim: say nothing (guardrails
            // audit 2026-08-21; a non-green stop after a WIP commit is not a gap).
            let holds_claim = ledger
                .open_claims()
                .map(|v| v.iter().any(|c| c.worker == worker))
                .unwrap_or(false);
            // Silence is the signal that all is well, and silence when nothing has changed:
            // the advisory is spoken once per (session, HEAD, missing checks, latest verify)
            // and again only when one of those moves. A blocked worker is not nagged every
            // turn about a blocker it cannot clear (adopter, 2026-08-21).
            let latest = ledger
                .latest_run(worker, &f.head, air_ledger::verify::Kind::Verify)
                .ok()
                .flatten()
                .map(|r| r.id)
                .unwrap_or_default();
            let fingerprint = if v.pass || !holds_claim {
                String::new()
            } else {
                let checks: Vec<&str> = v.missing.iter().map(|m| m.check).collect();
                format!("{}|{}|{latest}", f.head, checks.join(","))
            };
            let speak = ledger
                .emit_if_changed(&input.session_id, "stop", &fingerprint, &now())
                .unwrap_or(true);
            let context = if v.pass || !holds_claim || !speak {
                None
            } else {
                Some(format!("air: {}", v.message))
            };
            // Nudge (air-09i): no claim, beads ready, first stop: block once with the list.
            //
            // The cache is the CHEAP GATE, not the answer (air-ouw). It decides whether there
            // is anything to say at all, which costs nothing on the great majority of stops;
            // only when the nudge is actually about to speak does this confirm the list
            // against bd, because naming a bead `air claim` then refuses is Air contradicting
            // itself. Measured 2026-08-22 on this machine: 172 Stop hooks in a day, of which
            // 4 reached the nudge condition, and `bd ready --json` is ~0.7 s — so the truth
            // costs about 3 s a day, and the 168 silent stops still pay nothing.
            //
            // If bd does not answer inside the budget the nudge says NOTHING rather than
            // naming a list it cannot vouch for. A missed nudge costs one idle turn; a wrong
            // one costs the worker's trust in every later one.
            let now = now();
            let stop_hook_active = input.stop_hook_active.unwrap_or(false);
            let cached = ready_cache::read(cwd).map(|c| c.ids).unwrap_or_default();
            let would_speak = !holds_claim
                && !stop_hook_active
                && !cached.is_empty()
                && role_for(worker) == "worker";
            let ready = if would_speak {
                ready_cache::confirm(cwd).unwrap_or_default()
            } else {
                Vec::new()
            };
            let nudge = stop_nudge("worker", holds_claim, &ready, stop_hook_active);
            // Measurement: did a claim follow the previous nudge within 10 min?
            let followed = ledger
                .last_emission(&input.session_id, "nudge")
                .ok()
                .flatten()
                .map(|(_, at)| claim_followed(ledger, worker, &at));
            if nudge.is_some() {
                let _ = ledger.emit_if_changed(&input.session_id, "nudge", &now, &now);
            }
            let decision = if nudge.is_some() {
                "nudge"
            } else if v.pass {
                "pass"
            } else if !holds_claim {
                "no-claim"
            } else if speak {
                "would-refuse"
            } else {
                "would-refuse-repeat"
            };
            let outcome = match nudge {
                Some(reason) => HookOutcome::Block { reason },
                None => HookOutcome::Allow { context },
            };
            Dispatched::new(
                outcome,
                decision,
                format!("{}; {}", transition(&prev, "idle"), v.message),
            )
            .inputs(serde_json::json!({
                "head": f.head,
                "stop_hook_active": input.stop_hook_active,
                "ready": ready,
                // Whether this nudge's list came from a live bd call; the cost of that call
                // is on the same line as bd_ms/bd_calls (air-869), so keeping or dropping the
                // cache is decided on a number.
                "ready_confirmed": would_speak,
                "claim_followed_last_nudge": followed,
            }))
            .denominator("4 checks")
        }
        _ => Dispatched::new(
            HookOutcome::Allow { context: None },
            "ignored",
            "no handler",
        ),
    })
}

/// Did `worker` claim anything in the 10 minutes after `nudged_at`? The nudge measurement
/// (air-09i removal condition). Claims are keyed by RFC 3339 text, which sorts as time.
fn claim_followed(ledger: &Ledger, worker: &str, nudged_at: &str) -> bool {
    let Ok(t) = nudged_at.parse::<jiff::Timestamp>() else {
        return false;
    };
    let Ok(until) = t.checked_add(jiff::SignedDuration::from_mins(10)) else {
        return false;
    };
    let until = until.to_string();
    ledger
        .conn()
        .query_row(
            "SELECT count(*) FROM claims WHERE worker=?1 AND claimed_at > ?2 AND claimed_at <= ?3",
            params![worker, nudged_at, until],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false)
}

/// The bead id in a hand-over command: first token after `close`/`update` that is not a flag.
pub fn handover_bead(cmd: &str) -> Option<String> {
    let toks: Vec<&str> = cmd.split_whitespace().collect();
    let i = toks.iter().position(|t| *t == "close" || *t == "update")?;
    let mut skip_next = false;
    for t in toks.get(i.saturating_add(1)..)? {
        if skip_next {
            skip_next = false;
            continue;
        }
        if let Some(flag) = t.strip_prefix('-') {
            // A flag without `=` takes the next token as its value, except bare switches.
            skip_next = !flag.contains('=') && !matches!(*t, "--claim" | "--force" | "--json");
            continue;
        }
        if matches!(*t, "&&" | ";" | "||" | "|") {
            return None;
        }
        return Some((*t).to_string());
    }
    None
}

fn transition(prev: &Option<String>, next: &str) -> String {
    format!("{} -> {next}", prev.as_deref().unwrap_or("none"))
}

fn pre_tool_use(
    ledger: &Ledger,
    worker: &str,
    cwd: &Path,
    input: &HookInput,
) -> Result<Dispatched, String> {
    let prev = set_session(ledger, input, worker, "running", input.tool_name.as_deref())?;
    let moved = transition(&prev, "running");
    // Peer-on-file warning.
    if let Some(abs) = input.edited_path() {
        if let Ok(root) = git::toplevel(cwd)
            && let Some(rel) = journal::relative_to(&root, Path::new(&abs))
        {
            let peers = journal::peers_on(ledger, worker, &rel).map_err(|e| e.to_string())?;
            let inputs = serde_json::json!({"path": rel, "peers": peers});
            let denominator = format!("{} peer(s) journaled on path", peers.len());
            // Same peers on the same path: warned once per session, not on every edit.
            let fingerprint = if peers.is_empty() {
                String::new()
            } else {
                peers.join(",")
            };
            let speak = ledger
                .emit_if_changed(
                    &input.session_id,
                    &format!("peer:{rel}"),
                    &fingerprint,
                    &now(),
                )
                .unwrap_or(true);
            if !peers.is_empty() && !speak {
                return Ok(Dispatched::new(
                    HookOutcome::Allow { context: None },
                    "warn-repeat",
                    format!("{moved}; peer on file (already warned)"),
                )
                .inputs(inputs)
                .denominator(denominator));
            }
            if !peers.is_empty() {
                return Ok(Dispatched::new(
                    HookOutcome::Allow {
                        context: Some(format!(
                            "air: {} is also being edited by {} — coordinate before overlapping edits (run `air peer <name>` for their green sha)",
                            rel,
                            peers.join(", ")
                        )),
                    },
                    "warn",
                    format!("{moved}; peer on file"),
                )
                .inputs(inputs)
                .denominator(denominator));
            }
            return Ok(Dispatched::new(
                HookOutcome::Allow { context: None },
                "clear",
                format!("{moved}; no peer on file"),
            )
            .inputs(inputs)
            .denominator(denominator));
        }
        return Ok(Dispatched::new(
            HookOutcome::Allow { context: None },
            "clear",
            format!("{moved}; path outside repo"),
        ));
    }
    // A session may act only on its own project (air-0lk); talking to another is fine
    // (air-3oq).
    if let Some(d) = project_fence(input, &project_for(cwd)) {
        return Ok(d);
    }
    // Hand-over gate on bd status writes.
    if let Some(cmd) = input.bash_command()
        && is_handover_command(cmd)
    {
        let enforce = std::env::var("AIR_ENFORCE").is_ok_and(|v| v == "1");
        return handover_gate(ledger, worker, cwd, cmd, enforce);
    }
    Ok(Dispatched::new(
        HookOutcome::Allow { context: None },
        "observed",
        moved,
    ))
}

/// The hand-over gate for one `bd` status write (air-i59). `enforce` (worker launches set
/// `AIR_ENFORCE=1`) turns a failed check into a deny whose reason names the fixing command;
/// otherwise the same message is returned as context and the write is allowed. Pure over the
/// ledger and the repo, so `air selftest` can run it red and green.
pub fn handover_gate(
    ledger: &Ledger,
    worker: &str,
    cwd: &Path,
    cmd: &str,
    enforce: bool,
) -> Result<Dispatched, String> {
    let bead = handover_bead(cmd);
    let f = handover::facts(ledger, worker, cwd, bead.as_deref(), !enforce)?;
    let v = handover_verdict(&f);
    let stamped = match &bead {
        Some(b) => ledger.stamp_handover(b, worker, &now()).unwrap_or(false),
        None => false,
    };
    let decision = if v.pass {
        "pass"
    } else if v.block {
        "refuse"
    } else {
        "would-refuse"
    };
    let outcome = if v.block {
        HookOutcome::Block {
            reason: format!("air: {}", v.message),
        }
    } else if !v.pass {
        HookOutcome::Allow {
            context: Some(format!("air: {}", v.message)),
        }
    } else {
        HookOutcome::Allow { context: None }
    };
    Ok(Dispatched::new(outcome, decision, v.message.clone())
        .inputs(serde_json::json!({"command": cmd, "head": f.head, "enforce": enforce, "bead": bead, "claim_stamped": stamped}))
        .denominator("4 checks"))
}

/// The bead this command CLOSES, if it closes one (air-8p4).
///
/// Narrower than [`is_handover_command`] on purpose: `-s awaiting_review` is a hand-over, not
/// an ending, and air-3eu is explicit that a handed-over bead is still the worker's until it
/// lands. Only `bd close` and `-s/--status closed` end a claim.
///
/// The failure this closes: a claim row survived `bd close`, so `handover-not-green` and the
/// idle conditions kept firing on a bead that was closed and landed — three repeats of one
/// alert on ad-gwyv.1, and a coordinator spending a setup window establishing that a row was
/// stale rather than a worker stuck. `air status` reconciled it against bd eventually, but only
/// when bd answered inside its 2 s budget, which under load it does not.
///
/// Removal: when nothing computes a condition from an open claim row.
pub fn closes_bead(cmd: &str) -> Option<String> {
    if !is_handover_command(cmd) {
        return None;
    }
    let toks: Vec<&str> = cmd.split_whitespace().collect();
    let closing = toks.contains(&"close")
        || toks
            .windows(2)
            .any(|w| matches!(w, [a, b] if (*a == "-s" || *a == "--status") && *b == "closed"))
        || toks.contains(&"--status=closed");
    closing.then(|| handover_bead(cmd)).flatten()
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

/// Role is a property of the checkout (research: agent-roles-and-confinement §1): the main
/// checkout is the coordinator, every worktree is a worker.
pub fn role_for(worker: &str) -> &'static str {
    if worker == "main" {
        "coordinator"
    } else {
        "worker"
    }
}

/// Current state of a session row, if any.
fn session_state(ledger: &Ledger, session_id: &str) -> Result<Option<String>, String> {
    ledger
        .conn()
        .query_row(
            "SELECT state FROM sessions WHERE session_id=?1",
            params![session_id],
            |r| r.get::<_, String>(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e.to_string()),
        })
}

/// The cross-project refusal on the PreToolUse path (air-0lk): a `tmux` command naming another
/// project's session. `None` when there is nothing to refuse. It does not depend on
/// `AIR_ENFORCE`: the failure being prevented is acting on a stranger's fleet, not a sloppy
/// hand-over. Pure over (input, project) so `air selftest` fires it without a tmux server.
///
/// `SendMessage` was fenced here too until air-3oq. It is not any more, permanently: the rule
/// is about acting on another project, not talking to it, and the denial broke the
/// cross-project channel silently and uninterpretably.
pub fn project_fence(input: &HookInput, project: &str) -> Option<Dispatched> {
    // Its own decision string, not the bare "refuse" the hand-over gate uses: `air audit`
    // counts firings by (command, decision), so sharing one would file every fence refusal
    // under the gate's row and under the gate's removal condition (air-3oq).
    let refuse = |why: String, inputs: serde_json::Value| {
        Dispatched::new(
            HookOutcome::Block {
                reason: format!("air: {why}"),
            },
            "refuse-cross-project",
            why,
        )
        .inputs(inputs)
        .denominator("1 project")
    };
    if let Some(cmd) = input.bash_command()
        && let Some(why) = super::project::tmux_refusal(cmd, project)
    {
        return Some(refuse(
            why,
            serde_json::json!({"command": cmd, "project": project}),
        ));
    }
    None
}

/// The whole project decision, with the environment passed in rather than read (air-7ah).
///
/// `AIR_PROJECT` from the launcher wins, because that is the launcher stating which fleet the
/// session belongs to; the beads prefix resolved from the checkout is the fallback — the same
/// resolver `air worker` uses for tmux session names (air-5lg), not a second one.
///
/// The env is a parameter so a test can decide it. `project_for` read it directly, and the
/// hook test asserting a scratch repo's prefix silently got the ambient `AIR_PROJECT` of
/// whatever session ran it. That passed everywhere except inside a launched session — which is
/// the only place `air land` runs — so landing was blocked for every branch, and the test that
/// should have caught it was the thing hiding it. Mutating the process environment in the test
/// instead would be worse: `cargo test` runs in parallel threads.
pub fn project_from(env: Option<&str>, cwd: &Path) -> String {
    match env.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => p.to_string(),
        None => super::tmux::project_prefix(cwd),
    }
}

/// This session's project (air-0lk). Never empty in a launched session; empty only when Air
/// cannot tell, and an empty project refuses every cross-project name, which is the safe
/// direction.
pub fn project_for(cwd: &Path) -> String {
    project_from(std::env::var("AIR_PROJECT").ok().as_deref(), cwd)
}

/// Upsert the session row; returns the state it had before (None for a new session) so the
/// caller can put the transition on the event line. `worker`/`role` are updated on every
/// hook: `claude --worktree` can fire SessionStart with `cwd` still at the main checkout, and
/// a row stuck on `main` made a live worker look gone (adopter ad-lpqp).
fn set_session(
    ledger: &Ledger,
    input: &HookInput,
    worker: &str,
    state: &str,
    detail: Option<&str>,
) -> Result<Option<String>, String> {
    let prev = session_state(ledger, &input.session_id)?;
    let t = now();
    // Which fleet this session belongs to, so "is that peer one of ours?" is a ledger question
    // (air-0lk). `AIR_PROJECT` when the launcher set it; the beads prefix otherwise.
    let project = project_for(Path::new(input.cwd.as_deref().unwrap_or(".")));
    // The pid is the `claude` process when Claude Code exports it; hooks run in its env.
    let pid: Option<i64> = std::env::var("CLAUDE_PID")
        .ok()
        .and_then(|v| v.parse().ok());
    ledger
        .conn()
        .execute(
            "INSERT INTO sessions (session_id, worker, transcript_path, state, detail, changed_at, started_at, role, pid, project) \
             VALUES (?1,?2,?3,?4,?5,?6,?6,?7,?8,?9) \
             ON CONFLICT(session_id) DO UPDATE SET state=excluded.state, detail=excluded.detail, \
             changed_at=excluded.changed_at, transcript_path=COALESCE(excluded.transcript_path, sessions.transcript_path), \
             worker=excluded.worker, role=excluded.role, pid=COALESCE(excluded.pid, sessions.pid), \
             project=excluded.project",
            params![input.session_id, worker, input.transcript_path, state, detail, t, role_for(worker), pid, project],
        )
        .map_err(|e| e.to_string())?;
    Ok(prev)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::{inner, is_handover_command};
    use std::path::Path;
    use std::process::Command;

    /// A scratch repo with one commit; the ledger lands in `<dir>/.air`.
    fn scratch_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let g = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        g(&["init", "-q", "-b", "main"]);
        g(&["commit", "-q", "--allow-empty", "-m", "a"]);
        dir
    }

    fn fire(repo: &Path, body: serde_json::Value) {
        let mut v = body;
        v["session_id"] = "s1".into();
        v["cwd"] = repo.to_string_lossy().to_string().into();
        inner(repo, &v.to_string()).unwrap();
    }

    fn events(repo: &Path) -> Vec<serde_json::Value> {
        let dir = repo.join(".air").join("events");
        let mut lines = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
            lines.extend(text.lines().map(|l| serde_json::from_str(l).unwrap()));
        }
        lines
    }

    #[test]
    fn every_invocation_appends_one_event_with_the_transition() {
        let dir = scratch_repo();
        let repo = dir.path().canonicalize().unwrap();
        let repo = repo.as_path();
        let file = repo.join("src.rs").to_string_lossy().to_string();
        fire(repo, serde_json::json!({"hook_event_name": "SessionStart"}));
        fire(
            repo,
            serde_json::json!({"hook_event_name": "PreToolUse", "tool_name": "Edit",
            "tool_input": {"file_path": file}}),
        );
        fire(
            repo,
            serde_json::json!({"hook_event_name": "PostToolUse", "tool_name": "Edit",
            "tool_input": {"file_path": file}}),
        );
        fire(
            repo,
            serde_json::json!({"hook_event_name": "PermissionRequest", "tool_name": "Bash"}),
        );
        fire(repo, serde_json::json!({"hook_event_name": "Notification"}));
        fire(
            repo,
            serde_json::json!({"hook_event_name": "PermissionDenied", "tool_name": "Bash",
            "tool_input": {"command": "cargo test"}, "reason": "denied by lease-guard"}),
        );
        fire(repo, serde_json::json!({"hook_event_name": "Stop"}));
        fire(
            repo,
            serde_json::json!({"hook_event_name": "SessionEnd", "reason": "exit"}),
        );

        let ev = events(repo);
        let got: Vec<(String, String, String)> = ev
            .iter()
            .map(|e| {
                (
                    e["command"].as_str().unwrap().to_string(),
                    e["decision"].as_str().unwrap().to_string(),
                    e["reason"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        assert_eq!(got.len(), 8, "one line per invocation: {got:?}");
        assert_eq!(got[0].0, "hook.SessionStart");
        assert_eq!(got[0].2, "none -> working");
        assert_eq!(
            (got[1].0.as_str(), got[1].1.as_str()),
            ("hook.PreToolUse", "clear")
        );
        assert_eq!(got[1].2, "working -> running; no peer on file");
        assert_eq!(
            (got[2].0.as_str(), got[2].1.as_str()),
            ("hook.PostToolUse", "journaled")
        );
        assert_eq!(ev[2]["inputs"]["path"], "src.rs");
        assert_eq!(
            (got[3].1.as_str(), got[3].2.as_str()),
            ("stuck", "working -> stuck")
        );
        assert_eq!(
            (got[4].0.as_str(), got[4].1.as_str()),
            ("hook.Notification", "ignored")
        );
        assert_eq!(
            (got[5].0.as_str(), got[5].1.as_str(), got[5].2.as_str()),
            ("hook.PermissionDenied", "denied", "denied by lease-guard")
        );
        assert_eq!(ev[5]["inputs"]["command"], "cargo test");
        assert_eq!(
            (got[6].0.as_str(), got[6].1.as_str()),
            ("hook.Stop", "observed")
        );
        assert_eq!(got[6].2, "working -> idle; coordinator: no hand-over check");
        assert_eq!(
            (got[7].1.as_str(), got[7].2.as_str()),
            ("ended", "idle -> gone")
        );
        assert!(ev.iter().all(|e| e["inputs"]["session_id"] == "s1"));
    }

    /// air-0lk: the session row carries its project, so `air status --json` answers "is that
    /// peer one of ours?" from the ledger rather than from a name's spelling.
    #[test]
    fn a_session_row_records_its_project() {
        let dir = scratch_repo();
        let repo = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(repo.join(".beads")).unwrap();
        std::fs::write(repo.join(".beads/config.yaml"), "issue-prefix: \"zz\"\n").unwrap();

        // The decision, with the environment supplied rather than inherited (air-7ah). This
        // read the ambient `AIR_PROJECT`, so it asserted "zz" everywhere except inside a
        // launched session — where `air land` runs, and where it therefore failed.
        assert_eq!(
            super::project_from(None, &repo),
            "zz",
            "falls back to the prefix"
        );
        assert_eq!(
            super::project_from(Some("air"), &repo),
            "air",
            "the launcher's value wins over the checkout's prefix"
        );
        assert_eq!(
            super::project_from(Some("  "), &repo),
            "zz",
            "blank is not a value"
        );

        // And the hook writes that decision to the session row rather than something else.
        fire(
            &repo,
            serde_json::json!({"hook_event_name": "SessionStart"}),
        );
        let conn = rusqlite::Connection::open(repo.join(".air/ledger.db")).unwrap();
        let project: String = conn
            .query_row("SELECT project FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(project, super::project_for(&repo));
        assert!(!project.is_empty(), "a session row always names a project");
    }

    #[test]
    fn stop_is_silent_when_all_is_well_and_speaks_on_a_gap() {
        use super::{HookOutcome, dispatch};
        use air_ledger::verify::{Kind, VerifyRun, new_id};
        let dir = scratch_repo();
        let repo = dir.path().canonicalize().unwrap();
        // A linked worktree is a worker; the main checkout would be the coordinator.
        let wt = repo.join("wt");
        let out = Command::new("git")
            .args([
                "-C",
                repo.to_str().unwrap(),
                "worktree",
                "add",
                "-q",
                "-b",
                "w",
                wt.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(out.status.success());
        let (ledger, worker) = crate::cmd::open(&wt).unwrap();
        assert_eq!(worker, "wt");
        let stop_with = |ledger: &air_ledger::Ledger, active: bool| {
            let input = air_hooks::HookInput::parse(
                &serde_json::json!({"session_id": "s", "hook_event_name": "Stop", "cwd": wt.to_string_lossy(), "stop_hook_active": active}).to_string(),
            )
            .unwrap();
            dispatch(ledger, "wt", &wt, &input).unwrap()
        };
        let stop = |ledger: &air_ledger::Ledger| stop_with(ledger, false);
        // No claim, no ready cache: nothing to hand over, silent.
        let d = stop(&ledger);
        assert_eq!(d.decision, "no-claim");
        assert!(matches!(d.outcome, HookOutcome::Allow { context: None }));
        // air-ouw: a cache alone no longer produces a nudge. The cache says there may be
        // something worth saying; the list is then confirmed against bd, and if bd cannot
        // answer — as here, where there is no bd for this scratch repo — the hook says
        // NOTHING rather than naming beads `air claim` might refuse. That silence is the
        // fix: the nudge used to read this file out loud, and twice on 2026-08-22 it named
        // a bead that was already claimed or `owner`-labelled.
        //
        // The nudge's own text and its filtering are covered by `stop_nudge`'s unit tests in
        // `crates/hooks/src/gate.rs` and by two `air selftest` probes, neither of which needs
        // a bd.
        crate::cmd::ready_cache::write(&wt, &["fd-1".into(), "fd-2".into()], &crate::cmd::now());
        let d = stop(&ledger);
        assert_eq!(d.decision, "no-claim", "a cache alone must not nudge");
        assert!(matches!(d.outcome, HookOutcome::Allow { context: None }));
        assert_eq!(d.inputs["ready_confirmed"], true, "it did try bd");
        // Nothing it printed can name a bead, since it never had a confirmed list.
        assert_eq!(d.inputs["ready"].as_array().map(Vec::len), Some(0));
        let d = stop_with(&ledger, true);
        assert_eq!(d.decision, "no-claim");
        assert!(matches!(d.outcome, HookOutcome::Allow { context: None }));
        // ...and it does not even reach for bd when Claude Code is already continuing.
        assert_eq!(d.inputs["ready_confirmed"], false);
        // A claim ends the nudging, and the measurement records that one followed.
        ledger
            .record_claim("fd-1", "wt", &[], &crate::cmd::now())
            .unwrap();
        // Claim held, no green recorded: speaks once, then the identical gap is silent.
        let d = stop(&ledger);
        assert!(matches!(d.outcome, HookOutcome::Allow { context: Some(_) }));
        assert_eq!(d.decision, "would-refuse");
        // Null, not true: the measurement asks whether a claim followed a NUDGE, and no
        // nudge fired here because bd never confirmed a list (air-ouw). "No nudge to follow"
        // and "a nudge that was ignored" stay distinguishable, which is the point of the
        // field.
        assert!(d.inputs["claim_followed_last_nudge"].is_null());
        let d = stop(&ledger);
        assert!(
            matches!(d.outcome, HookOutcome::Allow { context: None }),
            "{:?}",
            d.outcome
        );
        assert_eq!(d.decision, "would-refuse-repeat");
        // A new (red) verify at HEAD is a change: speaks again.
        ledger
            .record_verify(&VerifyRun {
                id: new_id(),
                worker: "wt".into(),
                sha: crate::git::head(&wt).unwrap(),
                kind: Kind::Verify,
                exit_code: 1,
                trigger: "test".into(),
                failing_step: None,
                started_at: "2026-01-01T00:00:01Z".into(),
                finished_at: "2026-01-01T00:00:01Z".into(),
                log_path: None,
                command: None,
                duration_ms: None,
                output_bytes: None,
                dirty: false,
            })
            .unwrap();
        let d = stop(&ledger);
        assert_eq!(d.decision, "would-refuse");
        // Green at HEAD: silent, but still on the event line as "pass".
        let head = crate::git::head(&wt).unwrap();
        ledger
            .record_verify(&VerifyRun {
                id: new_id(),
                worker: "wt".into(),
                sha: head,
                kind: Kind::Verify,
                exit_code: 0,
                trigger: "test".into(),
                failing_step: None,
                started_at: "2026-01-01T00:00:02Z".into(),
                finished_at: "2026-01-01T00:00:02Z".into(),
                log_path: None,
                command: None,
                duration_ms: None,
                output_bytes: None,
                dirty: false,
            })
            .unwrap();
        let d = stop(&ledger);
        assert!(
            matches!(d.outcome, HookOutcome::Allow { context: None }),
            "{:?}",
            d.outcome
        );
        assert_eq!(d.decision, "pass");
    }

    #[test]
    fn extracts_the_bead_from_handover_commands() {
        use super::handover_bead;
        assert_eq!(
            handover_bead("bd close fd-1 --reason done").as_deref(),
            Some("fd-1")
        );
        assert_eq!(
            handover_bead("bd update fd-2 -s awaiting_review").as_deref(),
            Some("fd-2")
        );
        assert_eq!(
            handover_bead("bd update --status closed fd-3").as_deref(),
            Some("fd-3")
        );
        assert_eq!(handover_bead("make verify"), None);
    }

    /// air-8p4, end to end through the hook: the close releases the claim, the hand-over does
    /// not, and the event line names the bead so the release is auditable.
    #[test]
    fn a_successful_close_releases_the_claim_and_awaiting_review_does_not() {
        let dir = scratch_repo();
        let repo = dir.path().canonicalize().unwrap();
        let repo = repo.as_path();
        let open = || crate::cmd::open(repo).unwrap().0.open_claims().unwrap();
        let post = |cmd: &str| {
            fire(
                repo,
                serde_json::json!({"hook_event_name": "PostToolUse", "tool_name": "Bash",
                "tool_input": {"command": cmd}}),
            );
        };
        fire(repo, serde_json::json!({"hook_event_name": "SessionStart"}));
        {
            let (l, worker) = crate::cmd::open(repo).unwrap();
            l.record_claim("fd-1", &worker, &[], "t0").unwrap();
            l.record_claim("fd-2", &worker, &[], "t0").unwrap();
        }

        // A hand-over is not an ending: the row stays (air-3eu).
        post("bd update fd-2 -s awaiting_review");
        assert_eq!(open().len(), 2, "awaiting_review must not release");

        post("bd close fd-1 --reason done");
        let held: Vec<String> = open().into_iter().map(|c| c.bead).collect();
        assert_eq!(held, vec!["fd-2".to_string()], "close must release fd-1");

        let released = events(repo)
            .iter()
            .any(|e| e["decision"] == "released" && e["inputs"]["bead"] == "fd-1");
        assert!(released, "the release must be on the event line");
    }

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
