//! `/api/fleet`: one card per session (air-o6x.1). The shape is final; air-o6x.2 fills the
//! transcript, session-file and pane fields, which this bead leaves empty. Every field is
//! named here because the page bead builds against this file.

use air_ledger::Ledger;
use serde::Serialize;

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
    /// Whether the recorded pid is alive with the recorded start time.
    pub pid_alive: bool,
    /// The branch head in its worktree, full sha, when read.
    pub head: Option<String>,
    /// The first eight characters of `head`.
    pub head_short: Option<String>,
    /// The bead ids of its open claims.
    pub claims: Vec<String>,
    /// When the session started (UTC, RFC 3339).
    pub since: String,
    /// What the session waits on, when `air status` would say so (a prompt, a lease).
    pub waiting_for: Option<String>,
    /// The harness's own status line for the session, when known.
    pub harness_status: Option<String>,
    /// The tool call in flight, from the transcript tail.
    pub current_tool: Option<CurrentTool>,
    /// The last assistant text in the transcript, one line.
    pub last_text: Option<String>,
    /// The context tokens the newest assistant record reports.
    pub context_tokens: Option<u64>,
    /// The last lines of its tmux pane.
    pub pane_tail: Vec<String>,
}

/// A tool call in flight.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CurrentTool {
    /// The tool's name (`Bash`, `Read`, ...).
    pub name: String,
    /// One line of its input: the command, the path.
    pub detail: Option<String>,
    /// When the call started (UTC, RFC 3339).
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

/// The fleet: the `sessions` rows, each as an otherwise empty card. No ledger, no sessions.
pub fn fleet(ledger: Option<&Ledger>, at: &str) -> Fleet {
    Fleet {
        at: at.to_string(),
        sessions: ledger.map(sessions).unwrap_or_default(),
    }
}

fn sessions(l: &Ledger) -> Vec<Session> {
    let Ok(mut stmt) = l
        .conn()
        .prepare("SELECT worker, role, state, model, started_at FROM sessions ORDER BY worker")
    else {
        return Vec::new();
    };
    let rows = stmt.query_map([], |r| {
        Ok(Session::empty(
            &r.get::<_, String>(0)?,
            &r.get::<_, String>(1)?,
            &r.get::<_, String>(2)?,
            &r.get::<_, String>(3)?,
            &r.get::<_, String>(4)?,
        ))
    });
    match rows {
        Ok(it) => it.filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}
