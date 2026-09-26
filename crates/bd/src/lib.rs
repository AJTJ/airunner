//! Air's beads boundary.
//!
//! Rules (plan 0001 §7; tick 0300): read via `bd --json`; write only through `bd`; CAS and
//! leases are owned by Air's ledger because bd 1.2.2 has neither. The surface Air actually
//! calls is `ready`, `show`, `list`, `dep list`, `update --claim`, `update -s/-a`, `comment`
//! and `close`.
//!
//! **Air does not interpret the dependency graph** (air-3qg), and reading one edge type does
//! not change that. Air reads the ready list bd computes and never asks what an edge MEANS —
//! which matters because an edge means different things at different moments: an adopter's
//! file-contention edges say who may START in a shared file, and bd applies them at CLOSE
//! time, which blocked a bead whose work was finished and green. If Air ever grows edge
//! semantics of its own it has to separate the two, and the decomposition skill has to say
//! which kind it is filing.
//!
//! The one thing Air does read (air-btz, 2026-09-06, which is why the sentence above used to
//! end "`Issue` carries no edge field" and no longer can): [`Dep`], via [`WorkLedger::dep_list`],
//! to answer ONE structural question — is this bead blocked by its own ancestor? That is not an
//! interpretation of what the edge means; it is a shape that can never resolve whatever it
//! means, because an ancestor cannot finish until its descendants do. bd does not prevent it on
//! every route and does not report it at all
//! (`.claude/skills/beads/references/bd-facts.md`, "bd's dependency guard is two rules, not an ancestor walk").
//!
//! `bd` is slow (`ready --json` ≈ 1.1 s locally, tick 0315), so nothing here is called from a
//! hook path — CLI and reconcile paths only.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

#[derive(Debug, thiserror::Error)]
pub enum BdError {
    #[error("bd not runnable at {bin}: {source}")]
    Spawn {
        bin: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("bd timed out after {0:?}")]
    Timeout(Duration),
    #[error("bd exited {code}: {stderr}")]
    Failed { code: i32, stderr: String },
    #[error("bd output was not the JSON we expected: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, BdError>;

/// What every `bd` process cost this `air` process (air-869).
///
/// Measured 2026-08-22 on this machine, quiet: `bd version` (no database) 205 ms;
/// `bd ping` (opens the embedded Dolt store) ~660 ms; `bd show <id> --json` ~1350 ms;
/// `bd list --status closed --json` (10 issues) ~1110 ms, the same as `--status open`
/// (5 issues). The cost is per *process*, not per issue, and it spikes to 2-4 s when a
/// peer's `bd` holds `.beads/embeddeddolt/.lock`. So N single-id writes cost N x 1.4 s
/// and one batched write costs 1.4 s. Air records the number so the claim stays checkable.
///
/// A process-global accumulator; `log_event` stamps the part of it since the previous event
/// line ([`stats::take`]) so every call site does not have to carry it. That used to be
/// [`stats::snapshot`], the running total, on the assumption that `air` is one short-lived
/// process per command. `air mcp` is not: its poll thread emits an event every tick for the
/// life of the server, and every one of those lines carried the whole lifetime total again.
/// The adopter's 2026-08-30 log summed to 570,989 bd calls that way; the largest total any
/// process ever reached was 1,661, and a one-shot command costs 1 to 4 (air-bp0). A derived
/// number that reads like an observed one, and it was read as one.
pub mod stats {
    use super::{AtomicU64, Ordering};

    static MS: AtomicU64 = AtomicU64::new(0);
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static TAKEN_MS: AtomicU64 = AtomicU64::new(0);
    static TAKEN_CALLS: AtomicU64 = AtomicU64::new(0);

    /// Add one finished `bd` process. Timeouts and failures count: the wait was real.
    pub fn record(ms: u64) {
        MS.fetch_add(ms, Ordering::Relaxed);
        CALLS.fetch_add(1, Ordering::Relaxed);
    }

    /// (total ms, processes) for the whole life of this process. `(0, 0)` means it never
    /// shelled out to bd. The lifetime total, not what one event should carry.
    pub fn snapshot() -> (u64, u64) {
        (MS.load(Ordering::Relaxed), CALLS.load(Ordering::Relaxed))
    }

    /// (ms, processes) since the previous `take`: what THIS event cost. In a one-shot
    /// command it equals [`snapshot`]; in a long-lived process it is one tick's share.
    pub fn take() -> (u64, u64) {
        let (ms, calls) = snapshot();
        let prev_ms = TAKEN_MS.swap(ms, Ordering::Relaxed);
        let prev_calls = TAKEN_CALLS.swap(calls, Ordering::Relaxed);
        (ms.saturating_sub(prev_ms), calls.saturating_sub(prev_calls))
    }
}

/// The subset of a bead Air reads. Unknown fields are ignored so minor bd changes do not
/// break us; missing fields default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Issue {
    pub id: String,
    pub title: String,
    /// The whole body. When a bead was filed with `-d`, the criteria are a
    /// `## Acceptance Criteria` section in here (air-ayp).
    pub description: String,
    /// bd's first-class acceptance field, set by `bd create/update --acceptance`. bd OMITS
    /// THE KEY ENTIRELY when it is unset, which is why a key listing on beads that never set
    /// it reads as "there is no such field" — twice, in two projects, before the adopter's
    /// survey of all 711 of its beads inverted the conclusion (air-ayp, 2026-08-22).
    ///
    /// Which shape a repo uses depends on how it files beads, so both are real: this repo is
    /// section-only (0 of 33 carry the field), the adopter is field-mostly (647 of 711 field,
    /// 57 section, 0 both, 7 neither). `air land` runs in both, so it reads the union.
    #[serde(default)]
    pub acceptance_criteria: String,
    pub status: String,
    pub priority: i64,
    pub assignee: Option<String>,
    pub labels: Vec<String>,
    pub parent: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// bd's `issue_type`: `task`, `bug`, `feature`, `epic`, `chore`. Read since air-f10, when
    /// the claimable count offered two epics as work and a worker nearly claimed one. An epic
    /// is a container, not a task; `bd ready` lists it beside the tasks all the same.
    pub issue_type: String,
    /// How many issues this one depends on, bd's own count (air-btz). Read only as a GATE:
    /// with zero, there is no edge to fetch, so the dependency scan skips its `bd dep list`
    /// entirely. Never used as the answer — which edges they are is what `dep_list` says.
    #[serde(default)]
    pub dependency_count: i64,
    /// When bd's claim lease runs out (bd 1.3). `bd update --claim` takes a five-minute lease
    /// (`lease_expires_at` minus `heartbeat_at` on a fresh claim, bd 1.3.0, 2026-09-26) and Air
    /// never runs `bd heartbeat`, so it runs out five minutes after the claim. Read only to
    /// tell the coordinator when `air reclaim` can take a bead back.
    pub lease_expires_at: Option<String>,
}

/// One dependency edge as `bd dep list --json` reports it: `issue_id` depends on
/// `depends_on_id`, of `dep_type` (air-btz). The array is flat across every id asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dep {
    pub issue_id: String,
    pub depends_on_id: String,
    /// `blocks` | `parent-child` | `related` | … Only `blocks` stops a bead becoming ready;
    /// `parent-child` is bd's own hierarchy and is definitional.
    #[serde(rename = "type")]
    pub dep_type: String,
}

/// bd's name for the edge that actually blocks.
pub const BLOCKS: &str = "blocks";

/// bd's name for its own hierarchy edge, which every child has to its parent.
pub const PARENT_CHILD: &str = "parent-child";

/// What Air needs from a work tracker. `BdCli` is the only implementation today; tests use
/// an in-memory fake.
pub trait WorkLedger {
    fn ready(&self) -> Result<Vec<Issue>>;
    fn in_progress(&self) -> Result<Vec<Issue>>;
    /// `bd list --status <status> --json` (custom statuses such as `awaiting_review` included).
    fn by_status(&self, status: &str) -> Result<Vec<Issue>>;
    /// `bd list --parent <id> --all --json`: every child of `id`, closed ones included
    /// (air-84u). `--all` is the point — without it bd hides closed children, and an epic
    /// whose children are all closed would be indistinguishable from one with none. `-n 0`
    /// lifts bd's default limit of 50, which would otherwise cap a large epic silently.
    fn children(&self, id: &str) -> Result<Vec<Issue>>;
    /// `bd list --status <s1,s2,…> -n 0 --json`: every issue in any of those stored statuses.
    /// Comma-separated in ONE argument, deliberately: bd 1.2.2's `--help` says repeating
    /// `-s` silently overwrites the previous value, so the repeated form would ask for the
    /// last status only and answer confidently.
    fn by_statuses(&self, statuses: &[&str]) -> Result<Vec<Issue>>;
    /// `bd dep list <id> <id> … --json`: the edges of every id in ONE process. Flat across
    /// all of them, so the caller reads `issue_id` to know whose each edge is.
    fn dep_list(&self, ids: &[String]) -> Result<Vec<Dep>>;
    fn show(&self, id: &str) -> Result<Option<Issue>>;
    /// `bd show <id> <id> … --json`: every id in ONE process (bd 1.2.2 `bd show [id...]`).
    /// bd OMITS an id it does not know and still exits 0 — checked 2026-08-22: stderr says
    /// `Error fetching zz-nope: no issue found matching "zz-nope"` and the exit code is 0 —
    /// so the caller must compare what came back with what it asked for. Order is bd's, not
    /// the caller's.
    fn show_all(&self, ids: &[String]) -> Result<Vec<Issue>>;
    /// `bd update <id> --claim` (assignee = actor, status = in_progress). Air's ledger checks
    /// CAS *before* calling this.
    fn claim(&self, id: &str, actor: &str) -> Result<()>;
    fn set_status(&self, id: &str, status: &str) -> Result<()>;
    /// `bd update <id> -s open -a ""`: back to open AND unassigned, in one process (air-0kk).
    /// In bd 1.2.x a pencilled assignee blocks every other worker's `--claim`, so a release
    /// that only reopened left the bead claimable by nobody but the worker that released it.
    fn reopen_unassigned(&self, id: &str, actor: &str) -> Result<()>;
    /// `bd reclaim --id <id> --older-than 0s`: take back a claim whose lease has run out, and
    /// answer how many beads bd reverted (0 when the lease is still live).
    fn reclaim(&self, id: &str, actor: &str) -> Result<u64>;
    fn comment(&self, id: &str, text: &str) -> Result<()>;
    /// `bd close <id> <id> … --reason <r>`: every id in ONE bd process. bd 1.2.2 documents
    /// `bd close [id...]` with "one --reason for all IDs" (`bd close --help`, read
    /// 2026-08-22). The per-process cost is the whole cost (see [`stats`]), so closing ten
    /// beads one at a time cost ten times what this costs (air-869).
    fn close_all(&self, ids: &[String], reason: &str, actor: &str) -> Result<()>;
}

/// The argv for a batched close: one process, every id, one reason. Pure so the count of
/// processes is checkable without running bd (`air selftest`).
/// The one bd process a release makes (air-0kk): status back to open and the assignee
/// cleared together, so the two cannot be left half-applied and a released bead is claimable
/// by anyone. Pure, so `air selftest` can read it.
///
/// `--actor` is the holder's. bd 1.3.0 refuses `-a ""` on a bead another actor holds
/// in_progress ("cannot reassign … held by …"), and bd's default actor is `$BEADS_ACTOR`, then
/// git's user name, then `$USER`, none of which has to be the actor Air claimed with.
pub fn reopen_argv(id: &str, actor: &str) -> Vec<String> {
    ["update", id, "-s", "open", "-a", "", "--actor", actor]
        .into_iter()
        .map(String::from)
        .collect()
}

/// The one bd process `air reclaim` makes. It never passes `--force`: the owner ruled on
/// 2026-09-26 that the coordinator takes a bead back only when bd agrees the claim is
/// abandoned, and an expired lease is how bd says so. `bd reclaim` with no `--older-than`
/// waits a ten-minute grace past expiry; `0s` drops the grace, because the coordinator has
/// already judged the worker gone. Checked on bd 1.3.0 in a throwaway repo: before expiry it
/// exits 0 with `"count": 0` and changes nothing; after expiry it reopens the bead and clears
/// the assignee.
pub fn reclaim_argv(id: &str, actor: &str) -> Vec<String> {
    [
        "reclaim",
        "--id",
        id,
        "--older-than",
        "0s",
        "--actor",
        actor,
        "--json",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// argv for [`WorkLedger::by_statuses`], pure so the ONE-ARGUMENT comma-separated form is
/// checkable without running bd (air-btz). bd 1.2.2's own `bd list --help` says a repeated
/// `-s`/`--status` **silently overwrites the previous value**, so the repeated form asks for
/// the last status alone and answers with complete confidence — the failure this shape is
/// written to avoid, and the reason it is pinned by a probe rather than by a comment.
/// `-n 0` lifts bd's default limit of 50, which would otherwise cap the answer silently.
pub fn by_statuses_argv(statuses: &[&str]) -> Vec<String> {
    vec![
        "list".to_string(),
        "--status".to_string(),
        statuses.join(","),
        "-n".to_string(),
        "0".to_string(),
        "--json".to_string(),
    ]
}

/// argv for [`WorkLedger::dep_list`], pure so the batching is checkable without running bd:
/// every id in ONE process, as `bd dep list [issue-id...]` documents (air-btz). The per-process
/// cost is the whole cost, exactly as it is for [`close_argv`].
pub fn dep_list_argv(ids: &[String]) -> Vec<String> {
    let mut v: Vec<String> = vec!["dep".to_string(), "list".to_string()];
    v.extend(ids.iter().cloned());
    v.push("--json".to_string());
    v
}

pub fn close_argv(ids: &[String], reason: &str, actor: &str) -> Vec<String> {
    let mut v: Vec<String> = vec!["close".to_string()];
    v.extend(ids.iter().cloned());
    v.push("--reason".to_string());
    v.push(reason.to_string());
    if !actor.is_empty() {
        v.push("--actor".to_string());
        v.push(actor.to_string());
    }
    v
}

/// The budget for one bd process, and the base of a multi-id call's (air-8lj8).
///
/// **Fail direction: CLOSED.** A timeout here is a refused or failed command (a claim, a close,
/// a landing's acceptance read), so the cost of a budget too short is a real command refused
/// on a slow bd, and the cost of one too long is only a longer wait on a bd that has hung.
/// Owner, 2026-09-25: "if anything, our default time budgets should be very generous". It was
/// 10 s, and an adopter ran with `AIR_BD_TIMEOUT_MS=120000` exported for every call because its
/// bd (1,346 beads) costs ~2 s per call at the median. Sixty seconds is 30 times that median.
///
/// Not derived from a distribution: `air audit`'s `bd` row records every wait against it, and
/// the figure moves when that row's p99 comes near it.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Added once per id to a multi-id call's budget (air-8lj8, air-fzv). The adopter measured ~2 s
/// per id in one multi-id `bd` call; here, `bd show` with 14 ids took 21 s against 2 s for one
/// (2026-09-06). Five seconds is 2.5 times that, generous for the same reason as
/// [`DEFAULT_TIMEOUT`].
pub const PER_ID: Duration = Duration::from_secs(5);

/// The ONE budget rule for a bd call naming several ids: `base + per_id x ids`, saturating.
///
/// It exists because every multi-id call used to get a flat budget whatever the id count, and
/// each call site that noticed fixed it for itself: the acceptance read did (air-fzv), `air
/// close` never did. `show_all` and `close_all` call it, so a new multi-id call inherits it
/// rather than rediscovering it.
///
/// Removal: when bd's cost stops growing with the id count, which `air audit`'s `bd` rows
/// would show as a flat elapsed time across id counts.
pub fn budget_for(base: Duration, per_id: Duration, ids: usize) -> Duration {
    let n = u32::try_from(ids).unwrap_or(u32::MAX);
    base.saturating_add(per_id.saturating_mul(n))
}

/// Shell-out implementation.
#[derive(Debug, Clone)]
pub struct BdCli {
    pub bin: PathBuf,
    pub cwd: PathBuf,
    /// The budget for one bd process, and the BASE of a multi-id call's budget.
    pub timeout: Duration,
    /// Added to [`Self::timeout`] once per id by every multi-id call (`show_all`,
    /// `close_all`), through [`budget_for`]. Zero makes the budget flat: `AIR_BD_TIMEOUT_MS`
    /// does that, because it overrides the whole budget, and `air status` does it because the
    /// 20 s MCP tool limit caps it (air-8lj8).
    pub per_id: Duration,
    /// Which budget this bd is spending, for the record (air-d75). Five call sites set five
    /// different timeouts on the same struct — the status reconcile derives one from bd's
    /// measured median, the acceptance read grows one with the id count — and a single `bd`
    /// row would report the largest of them as if it were the budget every wait ran against.
    /// One of [`air_ledger::budgets`]'s `BD*` names.
    pub label: &'static str,
}

impl BdCli {
    pub fn new(cwd: &Path) -> Self {
        Self {
            bin: PathBuf::from("bd"),
            cwd: cwd.to_path_buf(),
            timeout: DEFAULT_TIMEOUT,
            per_id: PER_ID,
            label: air_ledger::budgets::BD,
        }
    }

    /// What a call naming `ids` ids may wait: [`budget_for`] over this client's figures.
    pub fn budget(&self, ids: usize) -> Duration {
        budget_for(self.timeout, self.per_id, ids)
    }

    fn run(&self, args: &[&str]) -> Result<String> {
        self.run_for(args, self.timeout)
    }

    /// One bd process under `budget`. Every call goes through here, so the recorded budget is
    /// the one the wait actually ran against, scaled or not.
    fn run_for(&self, args: &[&str], budget: Duration) -> Result<String> {
        let t0 = std::time::Instant::now();
        let out = self.run_inner(args, budget);
        let elapsed = t0.elapsed();
        stats::record(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX));
        air_ledger::budgets::record(
            self.label,
            elapsed,
            budget,
            matches!(out, Err(BdError::Timeout(_))),
        );
        out
    }

    fn run_inner(&self, args: &[&str], budget: Duration) -> Result<String> {
        let child = Command::new(&self.bin)
            .args(args)
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| BdError::Spawn {
                bin: self.bin.clone(),
                source,
            })?;
        let (status, stdout, stderr) =
            match wait_drained(child, budget).map_err(|source| BdError::Spawn {
                bin: self.bin.clone(),
                source,
            })? {
                Some(x) => x,
                None => return Err(BdError::Timeout(budget)),
            };
        if !status.success() {
            return Err(BdError::Failed {
                code: status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&stderr).trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&stdout).to_string())
    }
}

/// (exit status, stdout, stderr) of a finished child.
type Drained = (std::process::ExitStatus, Vec<u8>, Vec<u8>);

/// Wait for a child with a timeout while draining its pipes on threads, so a child that
/// writes more than the pipe buffer (64 KB) cannot deadlock against us and be mistaken for
/// a hang. Kills and reaps on timeout. Returns (status, stdout, stderr).
fn wait_drained(
    mut child: std::process::Child,
    timeout: std::time::Duration,
) -> std::io::Result<Option<Drained>> {
    use std::io::Read;
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_t = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_end(&mut v);
        }
        v
    });
    let err_t = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_end(&mut v);
        }
        v
    });
    let status = match child.wait_timeout(timeout)? {
        Some(s) => s,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            // Do not join the drain threads: a grandchild (bd's own helper, or `sleep` in a
            // stub) may still hold the pipe open, and joining would wait for it, turning a
            // 2 s budget into a 25 s one (air-19u, seen in the full test run). The threads
            // end on their own when the pipe finally closes; their buffers are discarded.
            drop(out_t);
            drop(err_t);
            return Ok(None);
        }
    };
    let stdout = out_t.join().unwrap_or_default();
    let stderr = err_t.join().unwrap_or_default();
    Ok(Some((status, stdout, stderr)))
}

/// Parse `bd … --json` list output. Tolerates both a bare array and `{"issues": [...]}`.
pub fn parse_issues(json: &str) -> Result<Vec<Issue>> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let arr = match v {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(mut o) => match o.remove("issues") {
            Some(serde_json::Value::Array(a)) => a,
            // `bd show <one id> --json` answers a single object (see `show`); `show_all`
            // with one id must read it as a one-item list, or the batched reconcile in
            // `air status` would call a bead bd just answered for "unknown" (air-bp0).
            _ if o.contains_key("id") => vec![serde_json::Value::Object(o)],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    arr.into_iter()
        .map(|x| serde_json::from_value(x).map_err(BdError::from))
        .collect()
}

impl WorkLedger for BdCli {
    fn ready(&self) -> Result<Vec<Issue>> {
        parse_issues(&self.run(&["ready", "--json"])?)
    }

    fn in_progress(&self) -> Result<Vec<Issue>> {
        self.by_status("in_progress")
    }

    fn by_status(&self, status: &str) -> Result<Vec<Issue>> {
        parse_issues(&self.run(&["list", "--status", status, "--json"])?)
    }

    fn children(&self, id: &str) -> Result<Vec<Issue>> {
        parse_issues(&self.run(&["list", "--parent", id, "--all", "-n", "0", "--json"])?)
    }

    fn by_statuses(&self, statuses: &[&str]) -> Result<Vec<Issue>> {
        let argv = by_statuses_argv(statuses);
        let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
        parse_issues(&self.run(&argv)?)
    }

    fn dep_list(&self, ids: &[String]) -> Result<Vec<Dep>> {
        let argv = dep_list_argv(ids);
        let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
        Ok(serde_json::from_str(&self.run_for(&argv, self.budget(ids.len()))?).unwrap_or_default())
    }

    fn show(&self, id: &str) -> Result<Option<Issue>> {
        let out = self.run(&["show", id, "--json"])?;
        let v: serde_json::Value = serde_json::from_str(&out)?;
        // `bd show --json` returns a single object (or an array of one).
        let obj = match v {
            serde_json::Value::Array(mut a) if !a.is_empty() => a.remove(0),
            other => other,
        };
        if obj.is_null() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_value(obj)?))
    }

    fn show_all(&self, ids: &[String]) -> Result<Vec<Issue>> {
        let mut argv: Vec<&str> = vec!["show"];
        argv.extend(ids.iter().map(String::as_str));
        argv.push("--json");
        parse_issues(&self.run_for(&argv, self.budget(ids.len()))?)
    }

    fn claim(&self, id: &str, actor: &str) -> Result<()> {
        self.run(&["update", id, "--claim", "--actor", actor])
            .map(|_| ())
    }

    fn set_status(&self, id: &str, status: &str) -> Result<()> {
        self.run(&["update", id, "-s", status]).map(|_| ())
    }

    fn reopen_unassigned(&self, id: &str, actor: &str) -> Result<()> {
        let argv = reopen_argv(id, actor);
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();
        self.run(&args).map(|_| ())
    }

    fn reclaim(&self, id: &str, actor: &str) -> Result<u64> {
        let argv = reclaim_argv(id, actor);
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();
        let out = self.run(&args)?;
        let v: serde_json::Value = serde_json::from_str(&out)?;
        Ok(v.get("count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0))
    }

    fn comment(&self, id: &str, text: &str) -> Result<()> {
        self.run(&["comment", id, text]).map(|_| ())
    }

    fn close_all(&self, ids: &[String], reason: &str, actor: &str) -> Result<()> {
        let argv = close_argv(ids, reason, actor);
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();
        self.run_for(&args, self.budget(ids.len())).map(|_| ())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// air-bp0: one id through `show_all` gets bd's single-object answer, which is one issue.
    #[test]
    fn parses_a_bare_single_issue_as_one() {
        let one = r#"{"id":"zz-1","title":"a","status":"awaiting_review","labels":[]}"#;
        let got = parse_issues(one).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].status, "awaiting_review");
        // An object that is not an issue (no id) is still nothing.
        assert!(parse_issues(r#"{"count":3}"#).unwrap().is_empty());
        assert!(parse_issues("null").unwrap().is_empty());
    }

    #[test]
    fn parses_bare_array_and_wrapped_object() {
        let bare = r#"[{"id":"zz-1","title":"a","status":"open","priority":1,"labels":["x"]}]"#;
        let wrapped =
            r#"{"issues":[{"id":"zz-2","title":"b","status":"in_progress","assignee":"w1"}]}"#;
        let a = parse_issues(bare).unwrap();
        assert_eq!(a[0].id, "zz-1");
        assert_eq!(a[0].labels, vec!["x"]);
        let b = parse_issues(wrapped).unwrap();
        assert_eq!(b[0].assignee.as_deref(), Some("w1"));
        // Unknown fields and missing ones are tolerated.
        let c = parse_issues(r#"[{"id":"zz-3","weird":true}]"#).unwrap();
        assert_eq!(c[0].id, "zz-3");
        assert_eq!(c[0].priority, 0);
    }
}
