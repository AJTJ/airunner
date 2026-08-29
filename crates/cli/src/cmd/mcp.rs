//! `air mcp`: one stdio MCP server that is both the coordinator's channel (push) and the
//! tool/resource surface (pull). Decisions 2026-08-20.
//!
//! Protocol facts (https://code.claude.com/docs/en/channels-reference, accessed 2026-08-20):
//! newline-delimited JSON-RPC 2.0 over stdio; a server is a channel when `initialize` returns
//! `capabilities.experimental["claude/channel"] = {}`; it pushes with the notification
//! `notifications/claude/channel` `{content, meta}` (meta keys: identifiers only); the same
//! process may also serve `tools/*` and `resources/*`.
//!
//! Why hand-rolled and synchronous rather than an SDK: this process lives as long as the
//! coordinator session (days). It must not leak, must not block on a stuck child, and must
//! keep serving after any single bad line. The surface we need is six methods; owning the
//! read loop, the write lock, and the poll thread is smaller than auditing an async runtime
//! for the same guarantees (rust-safety skill; plan 0003 "no async in hooks").
//!
//! Push design: there is no documented way for an outside process to talk to a channel
//! server, and every attention condition is a clock condition anyway ("idle for N min").
//! So a poll thread re-evaluates `status::attention` from the ledger every
//! `AIR_CHANNEL_POLL_SECS` (default 30) and pushes only *new or escalated* conditions; the
//! de-dupe map is bounded by workers × kinds and pruned when a condition clears.
//!
//! Tools and resources shell out to this same binary with `--json` (one implementation, the
//! CLI; time-bounded by `wait-timeout`), so MCP can never disagree with the command line.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use crate::cmd::status::{self, Attention, Thresholds};

const PROTOCOL_VERSION: &str = "2025-06-18";
const TOOL_TIMEOUT: Duration = Duration::from_secs(20);
/// A line longer than this is an error, not a buffer we keep growing.
const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

/// Shared, locked stdout: the read loop and the poll thread both write whole lines.
#[derive(Clone)]
struct Out(Arc<Mutex<std::io::Stdout>>);

impl Out {
    fn send(&self, v: &Value) {
        // A poisoned lock (a panic while writing) must not silence the channel for the rest
        // of the session: recover the guard and keep writing.
        let mut o = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let _ = serde_json::to_writer(&mut *o, v);
        let _ = o.write_all(b"\n");
        let _ = o.flush();
    }
}

impl std::fmt::Debug for Out {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Out")
    }
}

pub fn run(repo: &Path) -> i32 {
    let out = Out(Arc::new(Mutex::new(std::io::stdout())));
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("air"));
    let ctx = Ctx {
        repo: repo.to_path_buf(),
        exe,
    };
    let poll_secs: u64 = std::env::var("AIR_CHANNEL_POLL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let poll_ms: u64 = std::env::var("AIR_CHANNEL_POLL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| poll_secs.saturating_mul(1000));
    {
        let out = out.clone();
        let repo = repo.to_path_buf();
        std::thread::Builder::new()
            .name("air-channel-poll".into())
            .spawn(move || poll_loop(&repo, &out, Duration::from_millis(poll_ms)))
            .map_err(|e| eprintln!("air mcp: poll thread: {e}"))
            .ok();
    }
    let stdin = std::io::stdin();
    let mut line = String::new();
    loop {
        line.clear();
        // Bounded read: take_line refuses to accumulate past MAX_LINE_BYTES.
        match read_bounded_line(&mut stdin.lock(), &mut line) {
            Ok(0) => return 0, // EOF: the session ended; exit cleanly.
            Ok(_) => {}
            Err(e) => {
                eprintln!("air mcp: stdin: {e}");
                return 0;
            }
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(msg) => {
                let id = msg.get("id").cloned().unwrap_or(Value::Null);
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle(&ctx, &msg)))
                {
                    Ok(Some(resp)) => out.send(&resp),
                    Ok(None) => {}
                    Err(_) => out.send(&error(
                        id,
                        -32603,
                        "internal error (panic); server continues",
                    )),
                }
            }
            Err(e) => out.send(&error(Value::Null, -32700, &format!("parse error: {e}"))),
        }
    }
}

/// `read_line` with a ceiling: a runaway line is discarded and reported, never buffered whole.
fn read_bounded_line<R: BufRead>(r: &mut R, buf: &mut String) -> std::io::Result<usize> {
    let mut total = 0usize;
    let mut overflow = false;
    loop {
        let avail = r.fill_buf()?;
        if avail.is_empty() {
            return Ok(total);
        }
        let (chunk, done) = match avail.iter().position(|b| *b == b'\n') {
            Some(i) => (avail.get(..=i).unwrap_or(avail), true),
            None => (avail, false),
        };
        let n = chunk.len();
        total = total.saturating_add(n);
        if !overflow && total <= MAX_LINE_BYTES {
            buf.push_str(&String::from_utf8_lossy(chunk));
        } else {
            overflow = true;
        }
        r.consume(n);
        if done {
            if overflow {
                buf.clear();
                return Err(std::io::Error::other(format!(
                    "line exceeded {MAX_LINE_BYTES} bytes; dropped"
                )));
            }
            return Ok(total);
        }
    }
}

#[derive(Debug, Clone)]
struct Ctx {
    repo: PathBuf,
    exe: PathBuf,
}

fn error(id: Value, code: i64, msg: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": msg}})
}

fn result(id: Value, v: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": v})
}

/// Dispatch one message. Notifications (no id) return None.
fn handle(ctx: &Ctx, msg: &Value) -> Option<Value> {
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let id = msg.get("id").cloned();
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let Some(id) = id else {
        // Notifications: initialized, cancelled, … nothing to do.
        return None;
    };
    Some(match method {
        "initialize" => result(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {},
                    "resources": {},
                    "experimental": {"claude/channel": {}}
                },
                "serverInfo": {"name": "air", "version": env!("CARGO_PKG_VERSION")},
                // The list is the conditions that EXIST, checked against `status::kinds::ALL`
                // rather than against memory. `review-waiting` left with air-okc. A surface
                // describing something untrue is air-ha8's defect, and an MCP instructions
                // string is a surface.
                "instructions": "Air: hub and referee for the fleet. Tools mirror the `air` CLI; the channel delivers attention conditions (stuck, idle-with-claim, silent-with-claim, gone-with-claim, idle-without-claim, handover-not-green, landed-not-closed, owner-decision-waiting, lease-held-by-dead-session, lease-stale) as they arise."
            }),
        ),
        "ping" => result(id, json!({})),
        "tools/list" => result(id, json!({"tools": tools()})),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match call_tool(ctx, name, &args) {
                Ok((text, is_error)) => result(
                    id,
                    json!({"content": [{"type": "text", "text": text}], "isError": is_error}),
                ),
                Err(e) => error(id, -32602, &e),
            }
        }
        "resources/list" => result(id, json!({"resources": resources()})),
        "resources/read" => {
            let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
            match read_resource(ctx, uri) {
                Ok(text) => result(
                    id,
                    json!({"contents": [{"uri": uri, "mimeType": "application/json", "text": text}]}),
                ),
                Err(e) => error(id, -32002, &e),
            }
        }
        "prompts/list" => result(id, json!({"prompts": []})),
        _ => error(id, -32601, &format!("method not found: {method}")),
    })
}

/// Tool catalogue: name, description, JSON schema, and the CLI argv it maps to.
struct Tool {
    name: &'static str,
    description: &'static str,
    schema: Value,
}

fn tool_defs() -> Vec<Tool> {
    vec![
        Tool {
            name: "air_status",
            description: "The coordinator's one screen: every worker's session state, HEAD, green-at-HEAD, open claims with hand-over attempts, files held, overlaps, and the capture inbox depth.",
            schema: json!({"type":"object","properties":{}}),
        },
        Tool {
            name: "air_attention",
            description: "Only the conditions that need the owner or the coordinator right now (empty array when the fleet is quiet).",
            schema: json!({"type":"object","properties":{}}),
        },
        Tool {
            name: "air_holdings",
            description: "Who has edits in which files across worktrees (uncommitted, committed since main, journaled). Optional file filter.",
            schema: json!({"type":"object","properties":{"file":{"type":"string","description":"repo-relative path"}}}),
        },
        Tool {
            name: "air_handover",
            description: "Is this worktree ready to hand over? Reports what is missing and the fixing command. Advisory.",
            schema: json!({"type":"object","properties":{"bead":{"type":"string"}}}),
        },
        Tool {
            name: "air_claim",
            description: "Claim a bead: runs `bd update --claim` (atomic) and records the claim. The only claim path. Refuses if another worker holds it.",
            schema: json!({"type":"object","required":["bead"],"properties":{"bead":{"type":"string"},"files":{"type":"array","items":{"type":"string"},"description":"repo-relative files you expect to touch"}}}),
        },
        Tool {
            name: "air_release",
            description: "Give a bead back: bd status → open and the claim closed with a reason.",
            schema: json!({"type":"object","required":["bead","reason"],"properties":{"bead":{"type":"string"},"reason":{"type":"string","enum":["landed","abandoned","reassigned","superseded","false-premise","owner-gated","unknown"]}}}),
        },
        Tool {
            name: "air_capture",
            description: "One line into the inbox for the coordinator to triage. Workers capture; they never create beads. Never blocks you.",
            schema: json!({"type":"object","required":["text"],"properties":{"text":{"type":"string"}}}),
        },
        Tool {
            name: "air_inbox",
            description: "Open captures, oldest first (coordinator).",
            schema: json!({"type":"object","properties":{}}),
        },
        Tool {
            name: "air_triage",
            description: "Resolve ONE capture: promote it to a bead you have already created with `bd create --validate --estimate N` (give bead), or drop it with a reason (give drop). The bead is checked against bd first and an id bd does not have is refused, so create the bead before triaging to it. A capture that was already triaged is re-pointed, which is how a wrong pointer gets corrected. One capture per call: `bd show` costs about a second per id, so a batch of 26 took 27.9 s against a 5 s budget (air-zlq, measured 2026-08-29).",
            schema: json!({"type":"object","required":["id"],"properties":{
                "id":{"type":"string"},
                "bead":{"type":"string"},
                "drop":{"type":"string"}}}),
        },
        Tool {
            name: "air_close",
            description: "Coordinator: close landed beads and release their claims. Every id goes in ONE bd process, and bd costs about 1.4 s per process however many ids it is given, so close a landing pass in one call, not one call per bead.",
            schema: json!({"type":"object","required":["bead","reason"],"properties":{
                "bead":{"anyOf":[{"type":"string"},{"type":"array","items":{"type":"string"}}]},
                "reason":{"type":"string"}}}),
        },
    ]
}

fn tools() -> Vec<Value> {
    tool_defs()
        .into_iter()
        .map(|t| json!({"name": t.name, "description": t.description, "inputSchema": t.schema}))
        .collect()
}

fn resources() -> Vec<Value> {
    [
        ("air://status", "Fleet status snapshot (JSON)"),
        ("air://attention", "Current attention conditions (JSON array)"),
        ("air://inbox", "Open captures (JSON: captures, landings)"),
        ("air://holdings", "File holdings across worktrees (JSON)"),
        (
            "air://owner-queue",
            "What waits on the owner: decisions and green landings with their commands (JSON)",
        ),
        ("air://leases", "Held resources with defects and waiters (JSON)"),
    ]
    .iter()
    .map(|(uri, d)| json!({"uri": uri, "name": uri.trim_start_matches("air://"), "description": d, "mimeType": "application/json"}))
    .collect()
}

/// One string or an array of them, empty entries dropped: the batch tools take either
/// (air-869, so a whole landing or triage pass is one call).
fn list_arg(args: &Value, k: &str) -> Vec<String> {
    match args.get(k) {
        Some(Value::String(s)) if !s.is_empty() => vec![s.clone()],
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn str_arg<'a>(args: &'a Value, k: &str) -> Option<&'a str> {
    args.get(k)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// Map a tool call to `air --json <argv>`; returns (text, is_error).
fn call_tool(ctx: &Ctx, name: &str, args: &Value) -> Result<(String, bool), String> {
    let mut argv: Vec<String> = vec!["--json".into()];
    match name {
        "air_status" => argv.push("status".into()),
        "air_attention" => argv.extend(["status".into(), "--attention".into()]),
        "air_holdings" => {
            argv.push("holdings".into());
            if let Some(f) = str_arg(args, "file") {
                argv.extend(["--file".into(), f.into()]);
            }
        }
        "air_handover" => {
            argv.push("handover".into());
            if let Some(b) = str_arg(args, "bead") {
                argv.extend(["--bead".into(), b.into()]);
            }
        }
        "air_claim" => {
            let bead = str_arg(args, "bead").ok_or("bead is required")?;
            argv.extend(["claim".into(), bead.into()]);
            if let Some(files) = args.get("files").and_then(Value::as_array) {
                let list: Vec<&str> = files.iter().filter_map(Value::as_str).collect();
                if !list.is_empty() {
                    argv.extend(["--files".into(), list.join(",")]);
                }
            }
        }
        "air_release" => {
            let bead = str_arg(args, "bead").ok_or("bead is required")?;
            let reason = str_arg(args, "reason").ok_or("reason is required")?;
            argv.extend([
                "release".into(),
                bead.into(),
                "--reason".into(),
                reason.into(),
            ]);
        }
        "air_capture" => {
            let text = str_arg(args, "text").ok_or("text is required")?;
            argv.extend(["capture".into(), text.into()]);
            if let Some(a) = str_arg(args, "audience") {
                argv.extend(["--for".into(), a.into()]);
            }
        }
        "air_inbox" => {
            argv.push("inbox".into());
            if args.get("owner").and_then(Value::as_bool) == Some(true) {
                argv.push("--owner".into());
            }
        }
        "air_lease_take" => {
            argv.extend([
                "lease".into(),
                "take".into(),
                str_arg(args, "resource").unwrap_or("runtime").into(),
            ]);
            if let Some(r) = str_arg(args, "reason") {
                argv.extend(["--reason".into(), r.into()]);
            }
        }
        "air_lease_release" => {
            argv.extend([
                "lease".into(),
                "release".into(),
                str_arg(args, "resource").unwrap_or("runtime").into(),
            ]);
        }
        "air_lease_status" => argv.extend(["lease".into(), "status".into()]),
        // One capture per call (air-zlq): the batch it used to build could not finish
        // verification inside the budget past about three ids.
        "air_triage" => {
            let Some(id) = str_arg(args, "id") else {
                return Err("id is required".into());
            };
            argv.extend(["triage".into(), id.into()]);
            match (str_arg(args, "bead"), str_arg(args, "drop")) {
                (Some(_), Some(_)) => return Err("give bead or drop, not both".into()),
                (Some(b), None) => argv.extend(["--bead".into(), b.into()]),
                (None, Some(d)) => argv.extend(["--drop".into(), d.into()]),
                (None, None) => return Err("give bead or drop".into()),
            }
        }
        "air_close" => {
            let beads = list_arg(args, "bead");
            if beads.is_empty() {
                return Err("bead is required".into());
            }
            let reason = str_arg(args, "reason").ok_or("reason is required")?;
            argv.push("close".into());
            argv.extend(beads);
            argv.extend(["--reason".into(), reason.into()]);
        }
        _ => return Err(format!("unknown tool: {name}")),
    }
    let (code, stdout, stderr) = run_self(ctx, &argv)?;
    let text = if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        stdout
    };
    Ok((text, code != 0))
}

fn read_resource(ctx: &Ctx, uri: &str) -> Result<String, String> {
    let argv: &[&str] = match uri {
        "air://status" => &["--json", "status"],
        "air://attention" => &["--json", "status", "--attention"],
        "air://inbox" => &["--json", "inbox"],
        "air://holdings" => &["--json", "holdings"],
        "air://owner-queue" => &["--json", "inbox", "--owner"],
        "air://leases" => &["--json", "lease", "status"],
        _ => return Err(format!("unknown resource: {uri}")),
    };
    let argv: Vec<String> = argv.iter().map(|s| (*s).to_string()).collect();
    let (code, stdout, stderr) = run_self(ctx, &argv)?;
    if code != 0 {
        return Err(format!("air exited {code}: {}", stderr.trim()));
    }
    Ok(stdout)
}

/// Run this binary with `--repo <repo>`; time-bounded, always reaped.
fn run_self(ctx: &Ctx, argv: &[String]) -> Result<(i32, String, String), String> {
    let child = Command::new(&ctx.exe)
        .arg("--repo")
        .arg(&ctx.repo)
        .args(argv)
        .current_dir(&ctx.repo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", ctx.exe.display()))?;
    let (status, stdout, stderr) =
        match crate::git::wait_drained(child, TOOL_TIMEOUT).map_err(|e| e.to_string())? {
            Some(x) => x,
            None => {
                return Err(format!(
                    "air {} timed out after {TOOL_TIMEOUT:?}",
                    argv.join(" ")
                ));
            }
        };
    Ok((
        status.code().unwrap_or(-1),
        String::from_utf8_lossy(&stdout).to_string(),
        String::from_utf8_lossy(&stderr).to_string(),
    ))
}

// ---------- channel push ----------

/// What has been pushed, keyed by (worker, kind): the minutes at which we last notified, and
/// the value fingerprint we notified about. Bounded by workers × kinds; entries vanish when
/// the condition clears.
pub type Pushed = BTreeMap<(String, &'static str), (i64, String)>;

/// Pure: which conditions to push now.
///
/// A condition that carries a `fingerprint` is **change-only** (air-s7c): pushed when it is
/// new, and again only when that value differs from the one last pushed. Age is deliberately
/// not part of it — re-pushing because the oldest item got older is the repeat under a new
/// name, and it is what produced 3 971 `review-waiting` pushes carrying 13 distinct facts on
/// 2026-08-22.
///
/// A condition with no fingerprint keeps the older behaviour: new ones always, existing ones
/// again once their duration has at least doubled (escalation without spam).
///
/// Either way this suppresses the PUSH only. Every evaluation is still written to the event
/// log by `record_and_log`, because that ratio is what made the finding visible in the first
/// place. Clears entries whose condition is gone.
pub fn select_new(pushed: &mut Pushed, current: &[Attention]) -> Vec<Attention> {
    let mut out = Vec::new();
    let mut seen: Vec<(String, &'static str)> = Vec::with_capacity(current.len());
    for a in current {
        let key = (a.worker.clone(), a.kind);
        seen.push(key.clone());
        let again = match pushed.get(&key) {
            None => true,
            Some((prev_min, prev_fp)) => {
                if a.fingerprint.is_empty() {
                    a.for_minutes >= prev_min.saturating_mul(2).max(prev_min.saturating_add(10))
                } else {
                    a.fingerprint != *prev_fp
                }
            }
        };
        if again {
            pushed.insert(key, (a.for_minutes, a.fingerprint.clone()));
            out.push(a.clone());
        }
    }
    pushed.retain(|k, _| seen.contains(k));
    out
}

/// Record that a condition was actually **said** to the coordinator, as distinct from
/// evaluated (air-5uz).
///
/// Until this existed the only trace of the channel was the per-tick line `record_and_log`
/// writes, so `air audit` counted evaluations and called them fires: 10,722 log lines against
/// 45 real pushes on 2026-08-22. The gate is `hook_emissions`, the same table the Stop and
/// peer hooks use, so a channel that starts again does not re-say what the last process
/// already said; the event line is what makes the push countable.
fn record_push(ledger: Option<&(air_ledger::Ledger, String)>, at: &str, a: &Attention) {
    let Some((ledger, worker)) = ledger else {
        return;
    };
    // Age-escalating kinds carry no value fingerprint; for those the minute count IS the
    // change `select_new` just decided on, so it is what must differ to speak again.
    let fingerprint = if a.fingerprint.is_empty() {
        format!("min:{}", a.for_minutes)
    } else {
        a.fingerprint.clone()
    };
    let key = format!("channel:{}:{}", a.kind, a.worker);
    if !ledger
        .emit_if_changed(worker, &key, &fingerprint, at)
        .unwrap_or(true)
    {
        return;
    }
    crate::cmd::log_event(
        ledger,
        worker,
        "channel.push",
        &json!({
            "conditions": [format!("{}:{}", a.kind, a.worker)],
            "for_minutes": a.for_minutes,
        }),
        "pushed",
        &a.detail,
        "1 condition pushed",
    );
}

fn channel_event(a: &Attention) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/claude/channel",
        "params": {
            "content": format!("[{}] {}: {}", a.kind, a.worker, a.detail),
            "meta": {
                "kind": a.kind.replace('-', "_"),
                "worker": a.worker,
                "for_minutes": a.for_minutes.to_string()
            }
        }
    })
}

/// Pure: session ids that appeared or vanished since the last tick. The first tick seeds
/// and reports nothing (the coordinator just started; existing sessions are not news).
pub fn session_changes(
    known: &mut Option<std::collections::BTreeSet<String>>,
    current: &[(String, String, status::Session)],
) -> Vec<(&'static str, String, String)> {
    let now: std::collections::BTreeSet<String> = current
        .iter()
        .map(|(_, _, s)| s.session_id.clone())
        .collect();
    let mut events = Vec::new();
    if let Some(prev) = known.as_ref() {
        for (w, role, s) in current {
            if !prev.contains(&s.session_id) {
                events.push((
                    "session_joined",
                    w.clone(),
                    format!(
                        "{role} {w} joined (session {})",
                        s.session_id.get(..8).unwrap_or(&s.session_id)
                    ),
                ));
            }
        }
        for id in prev.difference(&now) {
            events.push((
                "session_left",
                String::new(),
                format!("session {} left", id.get(..8).unwrap_or(id)),
            ));
        }
    }
    *known = Some(now);
    events
}

fn poll_loop(repo: &Path, out: &Out, every: Duration) {
    let mut pushed = Pushed::new();
    let mut known_sessions: Option<std::collections::BTreeSet<String>> = None;
    let thresholds = Thresholds::from_env();
    loop {
        // One bad tick (a panic in git parsing, a malformed row) must not end the thread:
        // the process lives as long as the coordinator session.
        let tick = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Ledger-only on an ordinary tick; bd at most once every 10 minutes
            // (air-cmn). Only `idle-without-claim` needs bd at all, for `ready_depth`.
            match status::gather_with(repo, status::BdUse::CachedFor(10)) {
                Ok(snap) => {
                    let att = status::attention(&snap, &snap.at, thresholds);
                    let opened = crate::cmd::open(repo).ok();
                    if let Some((ledger, worker)) = opened.as_ref() {
                        status::record_and_log(ledger, worker, &snap, &att, true);
                    }
                    for a in select_new(&mut pushed, &att) {
                        record_push(opened.as_ref(), &snap.at, &a);
                        out.send(&channel_event(&a));
                    }
                    for (kind, worker, text) in session_changes(&mut known_sessions, &snap.sessions)
                    {
                        out.send(&json!({
                            "jsonrpc": "2.0",
                            "method": "notifications/claude/channel",
                            "params": {
                                "content": format!("[{kind}] {text}"),
                                "meta": {"kind": kind, "worker": worker}
                            }
                        }));
                    }
                }
                Err(e) => eprintln!("air mcp: poll: {e}"),
            }
        }));
        if tick.is_err() {
            eprintln!("air mcp: poll: tick panicked; continuing");
        }
        std::thread::sleep(every);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn att(worker: &str, kind: &'static str, mins: i64) -> Attention {
        Attention {
            worker: worker.into(),
            kind,
            detail: String::new(),
            for_minutes: mins,
            // No fingerprint: these tests cover the age-escalation path, which is what a
            // kind without a change-only value still uses.
            fingerprint: String::new(),
        }
    }

    #[test]
    fn pushes_new_then_escalates_then_clears() {
        let mut p = Pushed::new();
        let first = select_new(&mut p, &[att("a", "stuck", 5)]);
        assert_eq!(first.len(), 1);
        // Same condition a little later: silent.
        assert!(select_new(&mut p, &[att("a", "stuck", 9)]).is_empty());
        // Doubled (and +10): pushed again.
        assert_eq!(select_new(&mut p, &[att("a", "stuck", 15)]).len(), 1);
        // Condition gone: map empties; nothing pushed.
        assert!(select_new(&mut p, &[]).is_empty());
        assert!(p.is_empty());
        // Reappears: new again.
        assert_eq!(select_new(&mut p, &[att("a", "stuck", 5)]).len(), 1);
    }

    #[test]
    fn session_joins_and_leaves_are_one_shot_after_seeding() {
        let sess = |id: &str| status::Session {
            session_id: id.into(),
            state: "working".into(),
            detail: None,
            changed_at: String::new(),
            pid: None,
            pid_alive: None,
            project: String::new(),
            model: String::new(),
        };
        let mut known = None;
        let a = vec![("main".to_string(), "coordinator".to_string(), sess("aaaa"))];
        assert!(
            session_changes(&mut known, &a).is_empty(),
            "first tick seeds silently"
        );
        let ab = vec![
            a[0].clone(),
            ("w1".to_string(), "worker".to_string(), sess("bbbb")),
        ];
        let ev = session_changes(&mut known, &ab);
        assert_eq!(ev.len(), 1);
        assert_eq!((ev[0].0, ev[0].1.as_str()), ("session_joined", "w1"));
        assert!(session_changes(&mut known, &ab).is_empty(), "no repeat");
        let ev = session_changes(&mut known, &a);
        assert_eq!(ev[0].0, "session_left");
    }

    #[test]
    fn pushed_map_stays_bounded_over_many_ticks() {
        let mut p = Pushed::new();
        for tick in 0..10_000i64 {
            let cur = vec![
                att("a", "stuck", tick),
                att("b", "idle-with-claim", tick / 2),
                att(
                    if tick % 2 == 0 { "c" } else { "d" },
                    "silent-with-claim",
                    1,
                ),
            ];
            let _ = select_new(&mut p, &cur);
            assert!(p.len() <= 3, "tick {tick}: {}", p.len());
        }
    }

    #[test]
    fn bounded_line_reader_drops_oversized_lines_and_continues() {
        let big = "x".repeat(MAX_LINE_BYTES + 10);
        let input = format!("{big}\n{{\"ok\":1}}\n");
        let mut r = std::io::Cursor::new(input.into_bytes());
        let mut buf = String::new();
        assert!(read_bounded_line(&mut r, &mut buf).is_err());
        assert!(buf.is_empty());
        buf.clear();
        assert!(read_bounded_line(&mut r, &mut buf).unwrap() > 0);
        assert_eq!(buf.trim(), "{\"ok\":1}");
        assert_eq!(read_bounded_line(&mut r, &mut buf).unwrap(), 0);
    }

    #[test]
    fn initialize_declares_channel_and_tools() {
        let ctx = Ctx {
            repo: PathBuf::from("."),
            exe: PathBuf::from("air"),
        };
        let resp = handle(
            &ctx,
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        )
        .unwrap();
        assert!(resp["result"]["capabilities"]["experimental"]["claude/channel"].is_object());
        assert!(
            handle(
                &ctx,
                &json!({"jsonrpc":"2.0","method":"notifications/initialized"})
            )
            .is_none()
        );
        let list = handle(&ctx, &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"air_claim") && names.contains(&"air_attention"));
        let pl = handle(
            &ctx,
            &json!({"jsonrpc":"2.0","id":9,"method":"prompts/list"}),
        )
        .unwrap();
        assert!(
            pl["result"]["prompts"].as_array().unwrap().is_empty(),
            "no prompts: audit 2026-08-21"
        );
        let bad = handle(&ctx, &json!({"jsonrpc":"2.0","id":3,"method":"nope"})).unwrap();
        assert_eq!(bad["error"]["code"], -32601);
        let meta = channel_event(&att("w", "idle-with-claim", 3));
        assert_eq!(meta["params"]["meta"]["kind"], "idle_with_claim");
    }
}
