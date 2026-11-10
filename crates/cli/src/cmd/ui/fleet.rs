//! `/api/fleet`: one card per session, from four sources, each named (air-o6x.1, air-o6x.2):
//! the ledger's `sessions` and `claims` rows; git for the worktree head; Claude Code's own
//! session file (`~/.claude/sessions/<pid>.json`) for busy, idle or waiting; the transcript's
//! tail, read in steps up to 1 MiB until it holds an assistant record (air-o6x.7), for the
//! call in flight, the last text and the context size; and the tmux pane
//! for its last lines. A source that is missing or unreadable leaves its fields null and the
//! card at the ledger's facts. Nothing here errors and nothing is written.
//!
//! Cost per poll: one `ps` for every session's pid at once, one `git rev-parse` per worktree
//! at most every ten seconds (a process-wide cache), one `stat` and one to three reads (64 KiB,
//! 256 KiB, 1 MiB) per transcript, one `tmux capture-pane` per pane. The research measured the
//! 64 KiB tail read at
//! 0.03 ms and the capture at 8.8 ms (`docs/research/live-ui.md` §2b, §3). The command text
//! comes from the transcript because Air's own hook lines carry only the tool's name.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use air_ledger::Ledger;
use serde::Serialize;

/// How much of a transcript is read per poll: the tail that holds the newest records. The
/// first step; [`TAIL_STEPS`] has the rest.
pub const TAIL_BYTES: u64 = 64 * 1024;
/// The tail sizes tried in turn until the tail holds an assistant record (air-o6x.7). Claude
/// Code writes several non-message records per turn (api-request, attachment, last-prompt,
/// mode, system), and an api-request record can carry the whole request, so on 2026-11-10 the
/// coordinator's last 64 KiB held 33 such lines and no assistant record while a 256 KiB tail
/// held twelve tool calls; every card showed no current tool. The hook's `model_of` made the
/// same mistake from the other end (air-06k7). Capped at 1 MiB: a transcript with no assistant
/// record in that much yields nulls, as before.
pub const TAIL_STEPS: [u64; 3] = [TAIL_BYTES, 256 * 1024, 1024 * 1024];
/// How long a worktree head is trusted before git is asked again.
const HEAD_TTL: Duration = Duration::from_secs(10);
/// The longest `detail` line a card carries.
const DETAIL_CHARS: usize = 200;
/// The pane lines a card carries.
const PANE_LINES: usize = 3;

/// The fleet as of `at`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Fleet {
    /// When this answer was computed (UTC, RFC 3339).
    pub at: String,
    /// One per row in `sessions`, ordered by worker name.
    pub sessions: Vec<Session>,
}

/// One session's card.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Session {
    /// The worker name (`worker-1`, `lane`, `coordinator`, `main`).
    pub worker: String,
    /// `worker`, `lane` or `coordinator`, as the launcher set it.
    pub role: String,
    /// `idle`, `running` or `working`, from the hook.
    pub state: String,
    /// The model the transcript's newest assistant record names, or empty.
    pub model: String,
    /// Whether the recorded pid is alive, with the recorded start time where one was recorded.
    pub pid_alive: bool,
    /// The branch head in its worktree, full sha, when git answered.
    pub head: Option<String>,
    /// The first eight characters of `head`.
    pub head_short: Option<String>,
    /// The bead ids of its open claims, oldest first.
    pub claims: Vec<String>,
    /// When the session started (UTC, RFC 3339).
    pub since: String,
    /// What the harness says the session waits on (`permission prompt`, `input needed`, ...),
    /// from its session file; null when it waits on nothing or the file is absent.
    pub waiting_for: Option<String>,
    /// The harness's own status for the session (`busy`, `idle`, `waiting`), from its session
    /// file; null when the pid is dead or the file is absent.
    pub harness_status: Option<String>,
    /// The tool call in flight, from the transcript tail: the newest `tool_use` with no
    /// `tool_result` yet.
    pub current_tool: Option<CurrentTool>,
    /// The first line of the newest assistant text in the transcript tail.
    pub last_text: Option<String>,
    /// The newest assistant record's input tokens: input plus cache read plus cache creation.
    pub context_tokens: Option<u64>,
    /// The last non-blank lines of its tmux pane, up to three; empty without tmux.
    pub pane_tail: Vec<String>,
}

/// A tool call in flight.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CurrentTool {
    /// The tool's name (`Bash`, `Read`, ...).
    pub name: String,
    /// One line of its input: the Bash command's first line, the file path of an edit, read
    /// or write, else the input's first 120 characters; at most 200 characters.
    pub detail: Option<String>,
    /// When the call started: the assistant record's timestamp (UTC, RFC 3339).
    pub started_at: String,
}

impl Session {
    /// A card with the ledger's row and nothing else filled.
    pub fn empty(worker: &str, role: &str, state: &str, model: &str, since: &str) -> Self {
        Self {
            worker: worker.to_string(),
            role: role.to_string(),
            state: state.to_string(),
            model: model.to_string(),
            pid_alive: false,
            head: None,
            head_short: None,
            claims: Vec::new(),
            since: since.to_string(),
            waiting_for: None,
            harness_status: None,
            current_tool: None,
            last_text: None,
            context_tokens: None,
            pane_tail: Vec::new(),
        }
    }
}

/// What the transcript tail says. Pure over the text; see [`transcript_facts`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranscriptFacts {
    pub current_tool: Option<CurrentTool>,
    pub last_text: Option<String>,
    pub context_tokens: Option<u64>,
}

/// What the harness's session file says. Pure over the text; see [`session_file_facts`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HarnessFacts {
    pub status: Option<String>,
    pub waiting_for: Option<String>,
    /// The exact tmux pane (`<session>:@<window>.%<pane>`), when the session is in tmux.
    pub tmux: Option<String>,
}

/// A `sessions` row, as read.
struct Row {
    worker: String,
    role: String,
    state: String,
    model: String,
    pid: Option<i64>,
    pid_started: Option<String>,
    transcript: Option<String>,
    started_at: String,
    project: String,
}

/// The fleet: every `sessions` row as a card. `main` is the main checkout, where the worktrees
/// live. No ledger, no sessions.
pub fn fleet(ledger: Option<&Ledger>, main: &Path, at: &str) -> Fleet {
    let Some(l) = ledger else {
        return Fleet {
            at: at.to_string(),
            sessions: Vec::new(),
        };
    };
    let rows = rows(l);
    let pids: Vec<(i64, Option<String>)> = rows
        .iter()
        .filter_map(|r| r.pid.map(|p| (p, r.pid_started.clone())))
        .collect();
    let alive = alive_pids(&pids);
    let claims = claims_by_worker(l);
    Fleet {
        at: at.to_string(),
        sessions: rows
            .into_iter()
            .map(|r| card(r, main, &alive, &claims))
            .collect(),
    }
}

fn card(
    r: Row,
    main: &Path,
    alive: &HashSet<i64>,
    claims: &HashMap<String, Vec<String>>,
) -> Session {
    let mut s = Session::empty(&r.worker, &r.role, &r.state, &r.model, &r.started_at);
    s.pid_alive = r.pid.is_some_and(|p| alive.contains(&p));
    s.head = head_of(&worktree_of(main, &r.worker));
    s.head_short = s.head.as_deref().map(|h| h.chars().take(8).collect());
    s.claims = claims.get(&r.worker).cloned().unwrap_or_default();
    let harness = if s.pid_alive {
        r.pid
            .and_then(session_file)
            .and_then(|text| session_file_facts(&text))
    } else {
        None
    };
    let mut pane_target = None;
    if let Some(h) = harness {
        s.harness_status = h.status;
        s.waiting_for = h.waiting_for;
        pane_target = h.tmux;
    }
    if let Some(text) = r
        .transcript
        .as_deref()
        .and_then(|p| transcript_tail(Path::new(p)))
    {
        let f = transcript_facts(&text);
        s.current_tool = f.current_tool;
        s.last_text = f.last_text;
        s.context_tokens = f.context_tokens;
    }
    let project = if r.project.is_empty() {
        crate::cmd::tmux::project_prefix(main)
    } else {
        r.project.clone()
    };
    let target = pane_target.unwrap_or_else(|| crate::cmd::tmux::session_name(&project, &r.worker));
    s.pane_tail = crate::cmd::tmux::capture_pane(&target)
        .map(|t| pane_tail(&t))
        .unwrap_or_default();
    s
}

fn rows(l: &Ledger) -> Vec<Row> {
    let Ok(mut stmt) = l.conn().prepare(
        "SELECT worker, role, state, model, pid, pid_started, transcript_path, started_at, \
         project FROM sessions ORDER BY worker",
    ) else {
        return Vec::new();
    };
    let rows = stmt.query_map([], |r| {
        Ok(Row {
            worker: r.get(0)?,
            role: r.get(1)?,
            state: r.get(2)?,
            model: r.get(3)?,
            pid: r.get(4)?,
            pid_started: r.get(5)?,
            transcript: r.get(6)?,
            started_at: r.get(7)?,
            project: r.get(8)?,
        })
    });
    match rows {
        Ok(it) => it.filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

fn claims_by_worker(l: &Ledger) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    let Ok(mut stmt) = l
        .conn()
        .prepare("SELECT worker, bead FROM claims WHERE released_at IS NULL ORDER BY claimed_at")
    else {
        return out;
    };
    let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
    else {
        return out;
    };
    for (worker, bead) in rows.filter_map(Result::ok) {
        out.entry(worker).or_default().push(bead);
    }
    out
}

/// The worktree a session works in: `main` for the main checkout, else
/// `.claude/worktrees/<worker>` under it.
pub fn worktree_of(main: &Path, worker: &str) -> PathBuf {
    if worker == "main" {
        main.to_path_buf()
    } else {
        main.join(".claude").join("worktrees").join(worker)
    }
}

/// Each worktree's head and when git last answered for it.
type HeadCache = Mutex<HashMap<PathBuf, (Instant, Option<String>)>>;

/// The head of a worktree, asked of git at most every [`HEAD_TTL`] per path.
fn head_of(path: &Path) -> Option<String> {
    static HEADS: OnceLock<HeadCache> = OnceLock::new();
    let cache = HEADS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(c) = cache.lock()
        && let Some((when, head)) = c.get(path)
        && when.elapsed() < HEAD_TTL
    {
        return head.clone();
    }
    let head = crate::git::head(path).ok().filter(|h| !h.is_empty());
    if let Ok(mut c) = cache.lock() {
        c.insert(path.to_path_buf(), (Instant::now(), head.clone()));
    }
    head
}

/// Which of the recorded pids are alive, with one `ps` for all of them. A pid whose row
/// recorded a start time counts only when `ps` reports that start time, so a reused pid
/// never reads as alive.
fn alive_pids(pids: &[(i64, Option<String>)]) -> HashSet<i64> {
    if pids.is_empty() {
        return HashSet::new();
    }
    let list = pids
        .iter()
        .map(|(p, _)| p.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let Ok(out) = std::process::Command::new("ps")
        .args(["-o", "pid=,lstart=", "-p", &list])
        .output()
    else {
        return HashSet::new();
    };
    alive_from_ps(&String::from_utf8_lossy(&out.stdout), pids)
}

/// Pure over `ps -o pid=,lstart=` output: the pids listed whose start time matches the
/// recorded one, or any listed pid where none was recorded.
pub fn alive_from_ps(text: &str, pids: &[(i64, Option<String>)]) -> HashSet<i64> {
    let listed: HashMap<i64, String> = text
        .lines()
        .filter_map(|l| {
            let mut words = l.split_whitespace();
            let pid: i64 = words.next()?.parse().ok()?;
            Some((pid, words.collect::<Vec<_>>().join(" ")))
        })
        .collect();
    pids.iter()
        .filter(|(pid, started)| match (listed.get(pid), started) {
            (Some(seen), Some(want)) => {
                seen == &want.split_whitespace().collect::<Vec<_>>().join(" ")
            }
            (Some(_), None) => true,
            (None, _) => false,
        })
        .map(|(pid, _)| *pid)
        .collect()
}

/// Claude Code's own record of a running session, `<config>/sessions/<pid>.json`, where the
/// config directory is `CLAUDE_CONFIG_DIR` or `~/.claude`.
fn session_file(pid: i64) -> Option<String> {
    let dir = match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".claude"),
    };
    std::fs::read_to_string(dir.join("sessions").join(format!("{pid}.json"))).ok()
}

/// Pure over a session file's text: `status`, `waitingFor` and the tmux pane. None for text
/// that is not a JSON object.
pub fn session_file_facts(text: &str) -> Option<HarnessFacts> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    v.as_object()?;
    let field = |k: &str| {
        v.get(k)
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    Some(HarnessFacts {
        status: field("status"),
        waiting_for: field("waitingFor"),
        tmux: field("tmux"),
    })
}

/// The last `max` bytes of a file as text, starting at the first whole line when the read
/// began mid-file. None when the file cannot be read. The card reads through
/// [`transcript_tail`]; this one-step read is the tests' fixture.
#[cfg(test)]
pub fn tail(path: &Path, max: u64) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    tail_of(&mut f, len, max)
}

/// The last `max` of `len` bytes of an open file, as [`tail`] cuts them.
fn tail_of(f: &mut std::fs::File, len: u64, max: u64) -> Option<String> {
    let start = len.saturating_sub(max);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if start > 0 {
        text = match text.find('\n') {
            Some(i) => text
                .get(i.saturating_add(1)..)
                .unwrap_or_default()
                .to_string(),
            None => String::new(),
        };
    }
    Some(text)
}

/// Whether a tail holds an assistant record, by the same cheap test `transcript_facts` uses
/// before it parses a line.
pub fn holds_assistant(text: &str) -> bool {
    text.contains("\"type\":\"assistant\"")
}

/// The transcript's tail for the card (air-o6x.7): each size in [`TAIL_STEPS`] in turn, one
/// stat and at most three reads, stopping at the first tail that holds an assistant record or
/// reaches the start of the file. The last step's tail is returned whatever it holds. None
/// when the file cannot be read.
pub fn transcript_tail(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let mut last = None;
    for step in TAIL_STEPS {
        let text = tail_of(&mut f, len, step)?;
        let whole = step >= len;
        if holds_assistant(&text) || whole {
            return Some(text);
        }
        last = Some(text);
    }
    last
}

/// Pure over a transcript tail: the newest `tool_use` with no `tool_result` for its id is the
/// call in flight; the newest assistant text block's first line; the newest assistant
/// record's input tokens. A line that does not parse is skipped, and an unknown shape yields
/// nulls, never an error.
pub fn transcript_facts(text: &str) -> TranscriptFacts {
    let mut tool_uses: Vec<(String, CurrentTool)> = Vec::new();
    let mut answered: HashSet<String> = HashSet::new();
    let mut facts = TranscriptFacts::default();
    for line in text.lines() {
        let is_assistant = line.contains("\"type\":\"assistant\"");
        if !is_assistant && !line.contains("\"type\":\"user\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let kind = v.get("type").and_then(serde_json::Value::as_str);
        let blocks = v
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(serde_json::Value::as_array);
        match kind {
            Some("user") => {
                for b in blocks.into_iter().flatten() {
                    if b.get("type").and_then(serde_json::Value::as_str) == Some("tool_result")
                        && let Some(id) = b.get("tool_use_id").and_then(serde_json::Value::as_str)
                    {
                        answered.insert(id.to_string());
                    }
                }
            }
            Some("assistant") => {
                let at = v
                    .get("timestamp")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if let Some(u) = v.get("message").and_then(|m| m.get("usage")) {
                    let n = |k: &str| u.get(k).and_then(serde_json::Value::as_u64).unwrap_or(0);
                    facts.context_tokens = Some(
                        n("input_tokens")
                            .saturating_add(n("cache_read_input_tokens"))
                            .saturating_add(n("cache_creation_input_tokens")),
                    );
                }
                for b in blocks.into_iter().flatten() {
                    match b.get("type").and_then(serde_json::Value::as_str) {
                        Some("text") => {
                            if let Some(first) = b
                                .get("text")
                                .and_then(serde_json::Value::as_str)
                                .and_then(|t| t.lines().find(|l| !l.trim().is_empty()))
                            {
                                facts.last_text = Some(clip(first.trim(), DETAIL_CHARS));
                            }
                        }
                        Some("tool_use") => {
                            let id = b
                                .get("id")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default()
                                .to_string();
                            let name = b
                                .get("name")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("?")
                                .to_string();
                            let detail = b.get("input").and_then(|i| tool_detail(&name, i));
                            tool_uses.push((
                                id,
                                CurrentTool {
                                    name,
                                    detail,
                                    started_at: at.clone(),
                                },
                            ));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    facts.current_tool = tool_uses
        .into_iter()
        .rev()
        .find(|(id, _)| !answered.contains(id))
        .map(|(_, t)| t);
    facts
}

/// One line for a tool's input: the Bash command's first line, an edit's, read's or write's
/// file path, else the input's first 120 characters.
fn tool_detail(name: &str, input: &serde_json::Value) -> Option<String> {
    let s = |k: &str| input.get(k).and_then(serde_json::Value::as_str);
    let raw = match name {
        "Bash" => s("command")?.lines().next()?.to_string(),
        "Edit" | "Write" | "Read" | "NotebookEdit" | "MultiEdit" => s("file_path")?.to_string(),
        _ => serde_json::to_string(input)
            .ok()?
            .chars()
            .take(120)
            .collect(),
    };
    let t = raw.trim();
    (!t.is_empty()).then(|| clip(t, DETAIL_CHARS))
}

fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// The last three non-blank lines of a pane's text, trailing space removed.
pub fn pane_tail(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .rev()
        .take(PANE_LINES)
        .map(str::to_string)
        .collect();
    lines.reverse();
    lines
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn assistant(ts: &str, blocks: &str, usage: &str) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{ts}","message":{{"model":"m","content":[{blocks}],"usage":{usage}}}}}"#
        )
    }
    fn result(id: &str) -> String {
        format!(
            r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"{id}","content":"ok"}}]}}}}"#
        )
    }

    /// A tool call with no result yet is in flight; one with its result is not; a line that
    /// does not parse is skipped; the last text and the usage come from the newest records.
    #[test]
    fn the_tail_names_the_call_in_flight_the_last_text_and_the_context() {
        let a = assistant(
            "2026-11-10T01:00:00Z",
            r#"{"type":"tool_use","id":"A","name":"Bash","input":{"command":"cargo test -p air\nsecond line"}}"#,
            r#"{"input_tokens":10,"cache_read_input_tokens":20,"cache_creation_input_tokens":5}"#,
        );
        let b = assistant(
            "2026-11-10T01:00:05Z",
            r#"{"type":"text","text":"\nBuilding the parser now.\nMore."},{"type":"tool_use","id":"B","name":"Edit","input":{"file_path":"/r/x.rs","old_string":"a","new_string":"b"}}"#,
            r#"{"input_tokens":1,"cache_read_input_tokens":2,"cache_creation_input_tokens":3}"#,
        );
        let text = format!("{a}\n{}\n{{not json\n{b}\n", result("A"));
        let f = transcript_facts(&text);
        assert_eq!(
            f.current_tool,
            Some(CurrentTool {
                name: "Edit".into(),
                detail: Some("/r/x.rs".into()),
                started_at: "2026-11-10T01:00:05Z".into()
            })
        );
        assert_eq!(f.last_text.as_deref(), Some("Building the parser now."));
        assert_eq!(f.context_tokens, Some(6));
        // Every call answered: nothing in flight, the facts otherwise the same.
        let done = format!("{text}{}\n", result("B"));
        let f = transcript_facts(&done);
        assert_eq!(f.current_tool, None);
        assert_eq!(f.context_tokens, Some(6));
        // A Bash call in flight names its command's first line; another tool its input.
        let c = assistant(
            "t",
            r#"{"type":"tool_use","id":"C","name":"Bash","input":{"command":"make verify\nx"}},{"type":"tool_use","id":"D","name":"Grep","input":{"pattern":"fn main"}}"#,
            "{}",
        );
        let f = transcript_facts(&format!("{c}\n{}\n", result("D")));
        let t = f.current_tool.unwrap();
        assert_eq!(
            (t.name.as_str(), t.detail.as_deref()),
            ("Bash", Some("make verify"))
        );
        let f = transcript_facts(&format!("{c}\n{}\n", result("C")));
        let t = f.current_tool.unwrap();
        assert_eq!(t.name, "Grep");
        assert_eq!(t.detail.as_deref(), Some(r#"{"pattern":"fn main"}"#));
        // An unknown shape: nulls, no error.
        assert_eq!(
            transcript_facts("{\"type\":\"assistant\",\"message\":7}\n"),
            TranscriptFacts::default()
        );
        assert_eq!(transcript_facts(""), TranscriptFacts::default());
    }

    #[test]
    fn the_session_file_gives_status_waiting_and_the_pane() {
        let f = session_file_facts(
            r#"{"pid":1,"status":"waiting","waitingFor":"permission prompt","tmux":"p-w:@1.%1","name":"p-w"}"#,
        )
        .unwrap();
        assert_eq!(f.status.as_deref(), Some("waiting"));
        assert_eq!(f.waiting_for.as_deref(), Some("permission prompt"));
        assert_eq!(f.tmux.as_deref(), Some("p-w:@1.%1"));
        let idle = session_file_facts(r#"{"status":"idle"}"#).unwrap();
        assert_eq!(idle.waiting_for, None);
        assert_eq!(idle.tmux, None);
        assert_eq!(session_file_facts("[]"), None);
        assert_eq!(session_file_facts("not json"), None);
    }

    #[test]
    fn ps_output_decides_liveness_with_the_recorded_start_time() {
        let ps = "  12 Mon Nov  9 10:00:00 2026\n  34 Mon Nov  9 11:00:00 2026\n";
        let pids = vec![
            (12, Some("Mon Nov 9 10:00:00 2026".to_string())),
            (34, Some("Tue Nov 10 11:00:00 2026".to_string())),
            (56, None),
            (12, None),
        ];
        let alive = alive_from_ps(ps, &pids);
        assert!(alive.contains(&12));
        assert!(!alive.contains(&34), "a reused pid is dead");
        assert!(!alive.contains(&56), "not listed");
        assert!(alive_from_ps("", &pids).is_empty());
    }

    /// air-o6x.7: a tool call without its result sits before 100 KiB of non-assistant records.
    /// The 64 KiB tail holds no assistant record, so the read grows and the card names the call;
    /// a small file is read once, and a file with no assistant record in 1 MiB yields nulls.
    #[test]
    fn the_tail_grows_past_non_assistant_records_until_it_holds_one() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("t.jsonl");
        let call = assistant(
            "2026-11-10T01:00:00Z",
            r#"{"type":"tool_use","id":"A","name":"Bash","input":{"command":"python3 -c 'import time; time.sleep(12)'"}}"#,
            r#"{"input_tokens":4,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}"#,
        );
        let filler = format!(
            r#"{{"type":"api-request","timestamp":"t","data":"{}"}}"#,
            "x".repeat(1000)
        );
        let mut text = format!("{call}\n");
        for _ in 0..105 {
            text.push_str(&filler);
            text.push('\n');
        }
        std::fs::write(&p, &text).unwrap();
        assert!(text.len() > 100 * 1024);
        let short = tail(&p, TAIL_BYTES).unwrap();
        assert!(
            !holds_assistant(&short),
            "the first step alone misses the call"
        );
        let grown = transcript_tail(&p).unwrap();
        assert!(holds_assistant(&grown));
        let f = transcript_facts(&grown);
        let t = f.current_tool.unwrap();
        assert_eq!(t.name, "Bash");
        assert_eq!(
            t.detail.as_deref(),
            Some("python3 -c 'import time; time.sleep(12)'")
        );
        assert_eq!(f.context_tokens, Some(4));
        // A small file comes back whole on the first step.
        let small = tmp.path().join("s.jsonl");
        std::fs::write(&small, format!("{call}\n")).unwrap();
        assert_eq!(
            transcript_tail(&small).as_deref(),
            Some(format!("{call}\n").as_str())
        );
        // No assistant record at all: the whole file, and nulls from it.
        let none = tmp.path().join("n.jsonl");
        std::fs::write(&none, &text[call.len() + 1..]).unwrap();
        let got = transcript_tail(&none).unwrap();
        assert!(!holds_assistant(&got));
        assert_eq!(transcript_facts(&got), TranscriptFacts::default());
        assert_eq!(transcript_tail(&tmp.path().join("missing")), None);
    }

    #[test]
    fn the_tail_starts_at_a_whole_line_and_the_pane_keeps_three_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("t.jsonl");
        std::fs::write(&p, "first line\nsecond line\nthird\n").unwrap();
        assert_eq!(
            tail(&p, 1000).as_deref(),
            Some("first line\nsecond line\nthird\n")
        );
        assert_eq!(tail(&p, 13).as_deref(), Some("third\n"));
        assert_eq!(tail(&p, 3).as_deref(), Some(""));
        assert_eq!(tail(&tmp.path().join("missing"), 10), None);
        assert_eq!(
            pane_tail("a  \n\n b\nc\n\nd   \n\n"),
            vec![" b".to_string(), "c".into(), "d".into()]
        );
        assert_eq!(
            worktree_of(Path::new("/m"), "worker-2"),
            PathBuf::from("/m/.claude/worktrees/worker-2")
        );
        assert_eq!(worktree_of(Path::new("/m"), "main"), PathBuf::from("/m"));
    }
}
