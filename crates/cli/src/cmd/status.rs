//! `air status [--attention]`: the coordinator's one screen, and the deterministic
//! conditions that mean "the owner or the coordinator is needed" (decisions 2026-08-20: the
//! coordinator is informed, not woken; the channel delivers exactly these).
//!
//! Split in two so the conditions are testable without git or a clock: `gather` builds a
//! `Snapshot` from the ledger and git; `attention` is a pure function of (snapshot, now,
//! thresholds).

use std::collections::BTreeMap;
use std::path::Path;

use air_ledger::Ledger;
use air_ledger::claims::Claim;
use air_ledger::leases::Lease;
use air_ledger::verify::Kind;
use serde::Serialize;

use crate::cmd::{emit, holdings, log_event, now, open};
use crate::git;

/// air-sze: every attention kind `attention()` can emit, in one place.
///
/// `attention()` constructs only from these constants, so a condition cannot reach the event log
/// without appearing here, and a probe compares this set against the `Fires::Condition` rows in
/// the mechanism registry. Five kinds shipped with no registry row until this bead
/// (`gone-with-claim`, `idle-with-claim`, `silent-with-claim`, `lease-held-by-dead-session`,
/// `lease-stale`): no removal condition, no fire count, and — unlike an unregistered decision —
/// invisible to `air audit` entirely, because the audit could only count what the registry
/// already named. An unregistered condition is not "uncounted", it is unseeable.
pub mod kinds {
    // `STUCK` was first here. Deleted 2026-09-06 (air-12k): its state was set only by a
    // permission prompt auto mode never shows. The deletion record is in `mechanisms.rs`.
    pub const IDLE_WITH_CLAIM: &str = "idle-with-claim";
    pub const IDLE_WITHOUT_CLAIM: &str = "idle-without-claim";
    pub const SILENT_WITH_CLAIM: &str = "silent-with-claim";
    pub const GONE_WITH_CLAIM: &str = "gone-with-claim";
    pub const HANDOVER_NOT_GREEN: &str = "handover-not-green";
    pub const LANDABLE: &str = "landable";
    /// air-ob0, narrowed by air-odv: history only, since no new rewind can occur.
    pub const REWOUND_AND_CARRIED: &str = "rewound-and-carried";
    pub const LANDED_NOT_CLOSED: &str = "landed-not-closed";
    /// air-gazh: the inverse of the one above. Closed ∩ landed was reported; closed ∩ NOT
    /// landed was not, and nothing else in this list covers it — `landable` needs a branch
    /// containing main so it goes quiet the moment main moves, and `landed-not-closed` needs a
    /// landing to have happened.
    pub const CLOSED_NOT_LANDED: &str = "closed-not-landed";
    // `owner-decision-waiting` was here (plan 0006; DELETED by air-uef, owner 2026-09-05).
    // Its subject was the owner capture queue, which is gone: the owner's queue is beads
    // labelled `owner`, counted on the `ready:` line of `air status`.
    pub const LEASE_HELD_BY_DEAD_SESSION: &str = "lease-held-by-dead-session";
    pub const LEASE_STALE: &str = "lease-stale";

    /// The whole set, compared against the registry by `air selftest`.
    pub const ALL: &[&str] = &[
        IDLE_WITH_CLAIM,
        IDLE_WITHOUT_CLAIM,
        SILENT_WITH_CLAIM,
        GONE_WITH_CLAIM,
        HANDOVER_NOT_GREEN,
        LANDABLE,
        REWOUND_AND_CARRIED,
        LANDED_NOT_CLOSED,
        CLOSED_NOT_LANDED,
        LEASE_HELD_BY_DEAD_SESSION,
        LEASE_STALE,
    ];
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Session {
    pub session_id: String,
    pub state: String,
    pub detail: Option<String>,
    pub changed_at: String,
    pub pid: Option<i64>,
    /// Filled by `gather` when a pid is known: is that process still running?
    pub pid_alive: Option<bool>,
    /// Which fleet this session belongs to (air-0lk). The hook writes it from `AIR_PROJECT`,
    /// so a cross-project refusal is derived from the ledger, not from a name's spelling.
    pub project: String,
    /// air-air: the model this session is running, read from its own transcript by the hook.
    /// Empty until the transcript has its first assistant message; never guessed from settings,
    /// because a session with no `--model` inherits whatever the harness gives it.
    pub model: String,
    /// air-9dg: did this session's hooks see `AIR_ENFORCE=1`? Written by the hook from its
    /// own environment, so it is what the gate ran with. `None` on rows from before v15.
    pub enforce: Option<bool>,
    /// Is there a transcript behind this row (air-3jv5)? Every session Claude Code starts
    /// carries a `transcript_path` in every hook payload — Air already reads it for the model —
    /// so a row without one was not written by a session.
    ///
    /// **Reported, never refused.** A hook that carries no transcript still gets its row and
    /// its event line, because the alternative fails the wrong way: refusing the row would
    /// lose a real worker from `air status` if a harness ever omitted the field, and losing a
    /// live worker is worse than showing a synthetic one. What changes is that the fleet is
    /// not TOLD a worker arrived, and a reader can see which rows no session is behind.
    pub has_transcript: bool,
    /// air-1n3, schema v20: why this session stopped, when it stopped for a reason other than
    /// finishing a turn. `(at, kind, text)` from a `Notification` or `StopFailure` hook.
    /// `None` on a session that has never been stopped that way, which is nearly all of them.
    ///
    /// The kind is what makes this actionable rather than merely sad: `quota_auto_resume_fired`
    /// says the harness is bringing the session back by itself and nothing should touch it,
    /// while `quota_auto_resume_stale`, `quota_auto_resume_disabled` and `stop_failure` say it
    /// is not.
    pub stopped: Option<(String, String, String)>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct WorkerView {
    pub worker: String,
    pub role: String,
    pub session: Option<Session>,
    pub head: Option<String>,
    /// Green under the repo's key (`cmd::green`): at the commit, or at its tree where the
    /// repo declares `verify_key: tree`. `None` when the head could not be read.
    pub green_at_head: Option<bool>,
    /// What is behind the word when a green was found by tree (air-7wf): which commit it was
    /// recorded at and by whom, and, where it does not count, why not. `None` for a plain
    /// commit green or a plain absence.
    pub green_detail: Option<String>,
    /// Where the newest non-green run at this head kept its output (air-5ik). `None` when the
    /// head is green, or when the run predates the store. It rides on the `not green` phrase
    /// rather than in `green_detail`, which is air-7wf's tree-versus-commit reason and means
    /// something else.
    pub red_log: Option<String>,
    pub claims: Vec<Claim>,
    /// Open claims bd shows in `awaiting_review`: still held (the files are still the
    /// worker's) but no longer work in progress (air-3eu).
    pub handed_over: Vec<Claim>,
    pub files_held: usize,
    /// The worker's live tmux session (`<project>-<worker>`), when one exists: `tmux ls` is
    /// machine-wide, so status is where the owner goes from a lane to its pane (air-5lg).
    pub tmux_session: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Snapshot {
    pub at: String,
    pub workers: Vec<WorkerView>,
    pub inbox_depth: usize,
    pub oldest_capture_at: Option<String>,
    /// Every lease, with the defect the CLI found (None = healthy).
    pub leases: Vec<(Lease, Option<String>)>,
    /// Who is waiting for a resource, by resource (air-q9c). A lease defect is a signal for
    /// whoever WANTS the thing, never for the holder, so this is the condition's audience.
    /// `lease_take` records a want only when it was refused by a HEALTHY lease, so a name in
    /// here is someone who asked and was told to wait.
    pub lease_wants: BTreeMap<String, Vec<String>>,
    /// Beads a landing merged but did not close, and that nobody has closed since. A ledger
    /// fact, never a bd status (air-ayp).
    pub landed_open: Vec<air_ledger::landings::LandedOpen>,
    /// air-gazh: the inverse. Beads bd has closed whose commits are in no tree but their
    /// author's worktree. Joined from two things already computed — `landable` (which names
    /// every bead on a branch not yet in main, blocked or not) and the ledger's `closed`
    /// release reason — so it costs no git walk and no bd call of its own.
    pub closed_not_landed: Vec<ClosedNotLanded>,
    /// Every session row (two sessions in one checkout are two entries; the per-worker view
    /// above keeps only the latest): (worker, role, session).
    pub sessions: Vec<(String, String, Session)>,
    /// `bd ready` count at this tick (None when bd did not answer): queue depth over time
    /// (plan 0006 C6; the round ran dry at 4 with only epics left). The RAW count, including
    /// beads no worker may take — it is a measurement of the queue, not of available work.
    pub ready_depth: Option<usize>,
    /// Of those, the ones a worker could actually claim: `ready_depth` minus the
    /// `owner`-labelled beads `air claim` refuses to workers (air-uir).
    ///
    /// Two numbers because they answer two questions. "How deep is the queue" wants every
    /// bead; "should I prompt an idle worker" wants only the ones that worker can take.
    /// `idle-without-claim` used `ready_depth` and so told the coordinator to interrupt a
    /// worker over work that did not exist for it — at round end on 2026-08-29 the one ready
    /// bead was `air-4t1`, labelled `owner`, which gate had already declined.
    pub claimable_depth: Option<usize>,
    /// Of `ready_depth`, the epics (air-f10): containers bd lists as ready that no worker may
    /// claim and the coordinator has to decompose. Shown, not dropped, so the coordinator
    /// does not have to ask bd for the number the line used to hide inside "claimable".
    pub epic_depth: Option<usize>,
    /// Of those epics, the ones with no OPEN child (air-84u): the count is a number, this is
    /// the action. air-80x sat undecomposed for hours and is open again with all six children
    /// closed, and nothing said so; the adopter's claimable depth fell to four with only epics
    /// left and their coordinator's decomposition was the bottleneck. Empty is the normal
    /// state and prints nothing; `None` when bd did not answer.
    pub epics_to_decompose: Option<Vec<EpicToDecompose>>,
    /// Beads blocked by one of their own ancestors (air-btz): they can never become ready, and
    /// nothing else in the fleet or the tracker says so. Empty is the normal state and prints
    /// nothing; `None` when bd did not answer.
    pub ancestor_deadlocks: Option<Vec<AncestorDeadlock>>,
    /// How many beads declare no `initiative: <CODE>` line, and how many were looked at
    /// (air-g5o). `None` when bd did not answer this tick.
    ///
    /// **A count, not a gate.** Nothing is refused for lacking one; the owner's shape is that
    /// a gate comes only if the number shows the rule is ignored.
    ///
    /// The denominator is the beads bd already told this tick about — the ready set plus the
    /// in-progress set, epics excluded — and NOT every open bead, because asking for those is
    /// another `bd` process at ~1.4 s on a command that is already seconds. The number is
    /// printed with its denominator so nobody reads it as a count of everything.
    pub without_initiative: Option<(usize, usize)>,
    /// Verifies running right now, oldest first (air-4cr). A land invalidates every one of
    /// them, so the coordinator needs this before merging and the worker never has to relay it.
    /// Dead pids are pruned by the gather that reads them.
    pub verifies_in_flight: Vec<air_ledger::verify::InFlight>,
    /// Landings that have merged into main and not yet reported an outcome (air-bxe), each
    /// with whether the `air land` process that wrote it is still alive. "Is the land done"
    /// is answered from here, never from a process listing.
    pub landings_in_flight: Vec<LandingInFlight>,
    /// Every process whose working directory is inside a fleet tree, by tree, each marked as
    /// part of a Claude Code session or not (`cmd::readers`, owner 2026-09-25). What
    /// `verifies_in_flight` cannot see: a run nobody recorded. `unknown` says why when the
    /// process listing did not answer.
    pub tree_readers: super::readers::TreeReaders,
    /// One warning per launched session whose process runs in the main checkout, where no role
    /// works since air-jc2p.1 (`readers::main_checkout_sessions`). Empty is the normal state.
    pub main_checkout_sessions: Vec<String>,
    /// Branches `air land --all` would take right now (air-03w). Filled from the same
    /// `select` the command runs, so the condition and the command cannot disagree. No bd
    /// call: `select` reads git and the ledger only.
    pub landable: Vec<Landing>,
    /// Branches that do NOT qualify, each naming the precondition it failed and the fix
    /// (air-72t7). air-6u5 added this to `Selection` precisely so nothing is silent, and the
    /// snapshot kept only `landings` — so `air status --json` said which branches can land and
    /// never why the others cannot, which is the shape air-6u5 called the worst answer
    /// available, one layer up.
    pub land_skipped: Vec<Skipped>,
    /// Real failures inside selection — git or the ledger — as distinct from "does not
    /// qualify" (air-72t7). `select` deliberately raises these rather than defaulting, and
    /// until now they reached no caller at all: an error read as an empty queue.
    pub land_errors: Vec<String>,
    /// Branches the verify lane may merge into its next batch (air-80x.3): head contains
    /// main, no green at that head, and the commits name a bead the worker holds. A fact the
    /// lane reads when it cuts a batch; no condition pushes it. Before this the list lived in
    /// messages, and in the adopter's 2026-08-29 round the batch never formed.
    pub batch_ready: Vec<BatchReady>,
    /// Every worker branch that is NOT batch-ready, with the fact it lacks (`--json`).
    pub not_batch_ready: Vec<NotBatchReady>,
    /// How long the lane's loop waited on messages in the last 24 hours (air-1vri.2).
    pub loops: super::loops::LoopTimes,
    /// The newest red verify at a batch head that no later green has superseded (air-80x.4),
    /// with the members it was recorded with. The lane splits by hand; nothing lands on it.
    pub red_batch: Option<super::batch::RedBatch>,
    /// The install record is older than this binary (air-d61); the same line `air doctor`
    /// prints, so the coordinator sees it without asking.
    pub install_lag: Option<super::install::InstallLag>,
    /// The fleet-wide stop, while one is set (air-1vri.1).
    pub fleet_stop: Option<air_ledger::fleet::FleetStop>,
    /// How bd reaches its data here and, in server mode, whether the port answered.
    pub bd_mode: Option<super::bd_server::Mode>,
    pub bd_server_up: bool,
    /// Who answers the port: a process that is not this project's server is not "up".
    pub bd_listener: super::bd_server::Listener,
    /// The binary this repo is pinned to, when it is (air-4usc).
    pub pin: Option<super::install::PinState>,
    /// Rewound merges that some worktree still carries (air-ob0). A rollback un-lands a branch
    /// from main and cannot un-merge it from anyone who took it, so this is the obligation a
    /// red land leaves behind. The message at rewind time is not the only copy.
    pub rewound_carried: Vec<RewoundCarried>,
    /// file -> workers holding it (only files with 2+ holders)
    pub overlaps: BTreeMap<String, Vec<String>>,
    pub errors: Vec<String>,
    /// How long `gather` took; on the event line so a slow status is measured, not felt.
    pub duration_ms: u64,
    /// Median cost of one `bd` process today, from the event log (air-869). None when
    /// nothing shelled out to bd today.
    pub bd_latency: Option<super::bd_latency::BdLatency>,
    /// Where the bd-derived counts came from: `live`, `cache` (bd deliberately not called this
    /// tick), or `stale` (bd was asked and did not answer). Said rather than guessed, because
    /// "0 ready" from a cache and "0 ready" from bd are different facts (air-cmn).
    pub bd_source: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Thresholds {
    pub idle_with_claim_min: i64,
    pub silent_with_claim_min: i64,
    /// A claim younger than this with no session row is a worker still launching, not gone.
    pub launch_grace_min: i64,
    /// Idle worker, no claim, beads ready (air-e7q; removal: zero firings in a round once
    /// the Stop nudge, air-09i, is in).
    pub idle_noclaim_min: i64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            idle_with_claim_min: 20,
            silent_with_claim_min: 20,
            launch_grace_min: 3,
            idle_noclaim_min: 5,
        }
    }
}

impl Thresholds {
    /// `AIR_ATTENTION_<NAME>_MIN` env overrides, for tuning without a rebuild.
    pub fn from_env() -> Self {
        let mut t = Self::default();
        let get = |k: &str, d: i64| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(d)
        };
        t.idle_with_claim_min = get("AIR_ATTENTION_IDLE_MIN", t.idle_with_claim_min);
        t.silent_with_claim_min = get("AIR_ATTENTION_SILENT_MIN", t.silent_with_claim_min);
        t.launch_grace_min = get("AIR_ATTENTION_LAUNCH_GRACE_MIN", t.launch_grace_min);
        t.idle_noclaim_min = get("AIR_ATTENTION_IDLE_NOCLAIM_MIN", t.idle_noclaim_min);
        t
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attention {
    /// The subject: a worker name, `owner`, or (for `review-waiting`) the bead id, so the
    /// channel's (subject, kind) de-dupe fires once per bead.
    pub worker: String,
    /// idle-with-claim | silent-with-claim | handover-not-green |
    /// lease-held-by-dead-session | lease-stale | idle-without-claim
    /// (inbox depth is a measurement in `status`, never a condition: audit 2026-08-21;
    /// review waits became a condition on 2026-08-22, air-e7q: three parties waited 20 min
    /// on a fact nobody was told)
    pub kind: &'static str,
    pub detail: String,
    pub for_minutes: i64,
    /// For a change-only kind: the VALUE this condition is reporting, with age deliberately
    /// left out. The channel pushes again only when this differs from what it last pushed
    /// (air-s7c). Empty means the kind escalates on age instead, the older behaviour.
    ///
    /// Age is not a change. Keying on "oldest 40 min" then "oldest 50 min" rebuilds the
    /// repeat under a new name, which is the thing the owner cut: 3 971 review-waiting
    /// pushes on 2026-08-22 were 13 distinct facts.
    #[serde(default)]
    pub fingerprint: String,
}

/// The command alone, for a line that already says what it is (air-6p5). Since air-3pz that
/// is `air land`: the coordinator's one allowed path onto main. It names the BRANCH (air-09b):
/// `air land <bead>` is refused the moment two branches carry the bead, and the batching lane
/// The adopter runs makes that the normal case, so the command a surface offers is the one that
/// cannot be ambiguous.
pub fn land_command(worker: &str) -> String {
    format!("air land --worker {worker}")
}

/// A green hand-over that only the owner can clear (air-6p5). The coordinator may not commit
/// on main and `air land` does not exist, so two green hand-overs waited on 2026-08-22 with
/// nothing saying so; the owner found out by reading a tmux pane (capture
/// 01M0KZETBMSHDXNX6PW15HVSJV). Removal: when `air land` exists and the coordinator may run
/// it, this drops to a count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Landing {
    /// The bead this landing carries, or `None` for a JOURNAL-ONLY branch (air-kexg) or a range
    /// of only the coordinator's commits ([`coordinator_only`]).
    ///
    /// A session journal (air-3xww) is per session, ungated, and explicitly not work on a
    /// bead, so a journal commit is the only commit a worker legitimately writes that names
    /// none — and a branch of them was refused with "no commit declares a bead", which is
    /// correct for work and wrong as the only outcome here. Two workers concluded independently
    /// that such a branch could land, by different reasoning, and no surface a worker can reach
    /// said otherwise.
    ///
    /// It is an `Option` rather than a sentinel string or a flag beside an empty one, because
    /// those encode one fact twice and put a value in a typed field every reader has to know
    /// not to believe. That is the shape of the workaround this replaces: amending a journal
    /// commit with `Bead: air-3xww`, honest while that bead was hours old and a lie the moment
    /// it was not.
    ///
    /// A journal branch correctly never matches a named bead — `air land <id>` cannot select
    /// one, because there is no id to name it by. `--worker` and `--all` are its routes.
    pub bead: Option<String>,
    pub worker: String,
    /// The worker's HEAD, the commit the recorded green is at.
    pub head: String,
    pub minutes: i64,
    /// The exact command, the repo's own until `air land` exists.
    pub command: String,
    /// The bead's acceptance clauses, from bd's `acceptance_criteria` field and the
    /// `## Acceptance Criteria` section, out of the same `bd list --json` this already makes.
    /// `air land` prints them beside its verdict on each; it closes nothing (air-ayp). Empty
    /// when the snapshot did not come from bd.
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Why `air land` would refuse this branch right now, or `None` when it would take it
    /// (air-y3v). Set by `land::branch_check`, the same predicate the command applies, so the
    /// list and the refusal cannot disagree. `command` above is the one that matches this.
    #[serde(default)]
    pub blocked: Option<String>,
}

/// A ready epic with nothing open under it (air-84u): the coordinator's next decomposition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct EpicToDecompose {
    pub epic: String,
    /// Children bd has, all of them closed. `0` is an epic that was never decomposed at all,
    /// which is the same duty and is why the count is reported rather than required.
    pub closed_children: usize,
}

/// Pure: is this epic the coordinator's to decompose now? Yes when no child is open — either
/// they are all closed, or there are none. A child in any status but `closed` is work in
/// flight (`awaiting_review` and `blocked` included), and an epic with one is not waiting on
/// anybody: this must never name an epic that has open children, because the whole value of
/// the line is that it costs nothing to trust.
pub fn to_decompose(epic: &str, children: &[air_bd::Issue]) -> Option<EpicToDecompose> {
    children
        .iter()
        .all(|c| c.status == "closed")
        .then(|| EpicToDecompose {
            epic: epic.to_string(),
            closed_children: children.len(),
        })
}

/// Pure: does this range carry ONLY session-journal commits (air-kexg)?
///
/// `changed` is every path the range touched; `journal_dir` is the repo's declared
/// `journal_dir` from `.claude/air.json`. True when the repo declares one, the range touched
/// something, and every path it touched is under that directory.
///
/// **The constraint is the point, not the permission.** A range MIXING journal commits with
/// anything else is unchanged and still needs a bead: this is a name for the one commit a
/// worker legitimately writes that names none, never a bypass for work that forgot its
/// trailer. Both halves are probed, because the permitting half is the one that would still
/// look right if the constraint rotted.
///
/// It reads a DECLARED field rather than guessing which paths look like a journal, so the
/// answer is a fact somebody wrote down. A repo that declares no `journal_dir` has no journal
/// case and nothing changes for it.
pub fn journal_only(changed: &[String], journal_dir: Option<&str>) -> bool {
    let Some(dir) = journal_dir.map(|d| d.trim_end_matches('/')) else {
        return false;
    };
    if dir.is_empty() || changed.is_empty() {
        return false;
    }
    changed
        .iter()
        .all(|p| p.strip_prefix(dir).is_some_and(|r| r.starts_with('/')))
}

/// Pure: is every commit in this range one of the coordinator's?
///
/// `commits` is the range's non-merge commits and `coordinator` is the non-merge commits of
/// the coordinator's branch since main. The batch-ready rule takes the coordinator's branch
/// with no bead, so a lane batch whose only non-merge commits came from it lands with none
/// too. A range with any other commit still needs a bead, and an empty range is not the
/// coordinator's.
pub fn coordinator_only(commits: &[String], coordinator: &[String]) -> bool {
    !commits.is_empty() && commits.iter().all(|c| coordinator.contains(c))
}

/// The fix for a range that names no bead. A worker amends its own commit. The lane cannot,
/// because amending moves its head off the sha its green is at and the commit is a member's,
/// so the member adds the trailer on its own branch and the lane cuts again.
pub fn no_bead_fix(worker: &str) -> String {
    if super::hook::role_for(worker) == "lane" {
        "the member whose commit names no bead adds a commit with a `Bead: <id>` trailer on its \
         own branch, then the lane runs `air batch cut` again (the coordinator's commits need none)"
            .to_string()
    } else {
        "add a `Bead: <id>` trailer to the commit that did the work (git commit --amend)"
            .to_string()
    }
}

/// A bead that can never become ready: it is blocked by one of its own ancestors (air-btz).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AncestorDeadlock {
    pub bead: String,
    /// The ancestor the `blocks` edge points at.
    pub ancestor: String,
    /// How far up: 1 is the parent, 2 the grandparent. Printed, because bd's own guard covers
    /// 1 and the reachable shape is 2 or more.
    pub depth: usize,
}

/// Pure: which beads carry a `blocks` edge on one of their own ancestors (air-btz).
///
/// An ancestor cannot finish until its descendants do — that is bd's hierarchy, not an edge —
/// so a descendant that waits on it waits forever. Both sides are stuck and the tracker shows
/// the bead as "not ready yet", which is what an ordinary queued bead looks like. An adopter
/// lost a night to this: every P1 in their queue unreachable, 42 beads offered to workers and
/// not one of them a P1.
///
/// bd 1.2.2 does NOT prevent this in general, measured 2026-09-06
/// (`.claude/skills/beads/references/bd-facts.md`, "bd's dependency guard is two rules, not an ancestor walk"). Its guard is two rules, neither
/// an ancestor walk: an existing `parent-child` row on the same pair, which always catches the
/// direct parent; and a dotted-id prefix test, which catches deeper ancestors only when the id
/// encodes the chain. `bd create --graph` assigns flat ids and links by `parent_key`, so a wave
/// filed from a plan file slips both, silently.
///
/// `parents` is child -> parent over the beads that are not finished; `edges` is bd's flat
/// dependency list. Only `blocks` counts: `parent-child` is the hierarchy itself, and naming
/// that would report every child in the repo.
///
/// A closed ancestor is absent from `parents` and so is not walked through. That is a real
/// limit and the safe direction: it under-reports rather than over-reports, and bd will not
/// close a parent whose children are open, which is the case that would matter.
pub fn ancestor_deadlocks(
    parents: &std::collections::BTreeMap<String, String>,
    edges: &[air_bd::Dep],
) -> Vec<AncestorDeadlock> {
    let mut out = Vec::new();
    for e in edges.iter().filter(|e| e.dep_type == air_bd::BLOCKS) {
        let mut at = e.issue_id.as_str();
        // Bounded by the map, so a parent cycle bd should never allow cannot spin here.
        for depth in 1..=parents.len() {
            let Some(up) = parents.get(at) else { break };
            if up == &e.depends_on_id {
                out.push(AncestorDeadlock {
                    bead: e.issue_id.clone(),
                    ancestor: up.clone(),
                    depth,
                });
                break;
            }
            at = up;
        }
    }
    out
}

/// A branch ready for the verify lane's next batch (air-80x.3).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BatchReady {
    pub worker: String,
    pub head: String,
    /// The beads the branch names by `Bead:` trailer that this worker holds a claim on.
    pub beads: Vec<String>,
}

/// Why a worker branch is not batch-ready: the one fact it lacks, first failing fact wins.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct NotBatchReady {
    pub worker: String,
    pub head: String,
    /// `green-at-head` | `red-at-head` | `dropped-at-head` | `no-claimed-bead` | `no-precheck` |
    /// `nothing-ahead` (the coordinator)
    pub check: &'static str,
    pub detail: String,
}

/// The facts the batch-ready rule reads, one worktree's worth.
#[derive(Debug, Clone, Default)]
pub struct BatchFacts {
    pub worker: String,
    pub head: String,
    /// `git merge-base --is-ancestor main <head>`. With `green_at_head`, landable on its own.
    pub contains_main: bool,
    /// `green::at(head)` holds. Landable, and so not for a batch, only while it contains main.
    pub green_at_head: bool,
    /// Beads declared by `Bead:` trailers in `main..head`.
    pub carried: Vec<String>,
    /// Beads this worker holds an open claim on.
    pub held: Vec<String>,
    /// The repo declares a precheck (`"precheck": true` in `.claude/air.json`), so a branch
    /// needs a green one at its head to be batch-ready.
    pub precheck_required: bool,
    /// A `precheck` run is green at `head` under the repo's green key. Read only when required.
    pub precheck_green_at_head: bool,
    /// The coordinator's worktree (air-jc2p.1): its commits are prose and filing, not bead
    /// work, so it needs no claimed bead, and nothing lands it on its own.
    pub coordinator: bool,
    /// `main..head` has a commit. Read only for the coordinator, whose branch would otherwise
    /// be batch-ready with nothing in it.
    pub ahead: bool,
    /// A red batch was recorded at this exact head and took it as a member (0.4.6
    /// trial D3).
    pub red_batch_at_head: bool,
    /// A cut dropped this exact head for a conflict and told its worker (0.4.9 trial).
    pub dropped_at_head: bool,
}

/// THE batch-ready rule, pure (air-80x.3): a branch that is not already landable (green at a
/// head that contains main) and whose commits name a bead the worker holds. In that order, so
/// the reason a branch is absent is the first fact it lacks. Nothing here is a judgement: each
/// fact is a git or ledger lookup the lane could make itself.
///
/// It does NOT require the head to contain main (owner 2026-09-25, from an adopter's fleet protocol). The lane
/// merges main forward at the cut, and a member that conflicts with main is dropped and named
/// like any conflict. Requiring it took every waiting branch out of the queue at every landing
/// until its worker re-merged; an adopter's workers were told to merge "for your own close and
/// for staleness, not for the cut" to work round exactly that.
///
/// Removal: when the harness or bd carries a branch-ready state Air can read instead.
pub fn batch_ready_rule(f: &BatchFacts) -> Result<BatchReady, NotBatchReady> {
    let short = f.head.get(..8).unwrap_or(&f.head);
    let not = |check: &'static str, detail: String| NotBatchReady {
        worker: f.worker.clone(),
        head: f.head.clone(),
        check,
        detail,
    };
    // The coordinator's branch (air-jc2p.1, owner 2026-09-14: its changes reach main through
    // the lane like a worker's). Its commits carry no bead, so it needs no claimed one, and
    // nothing lands it alone, so there is no landable-alone exit; what it must have is a commit
    // main lacks. Removed with the lane, when the coordinator would land its own branch again.
    if f.coordinator {
        if !f.ahead {
            return Err(not(
                "nothing-ahead",
                format!("{} at {short}: no commit that main lacks", f.worker),
            ));
        }
    } else if f.contains_main && f.green_at_head {
        return Err(not(
            "green-at-head",
            format!(
                "{} at {short} is already green with main merged: landable on its own, nothing to batch",
                f.worker
            ),
        ));
    }
    // D3 (0.4.6 trial): a head a red batch already took is not cut again until it moves. The
    // lane cut the same red member twice and had no command to leave it out.
    if f.red_batch_at_head {
        return Err(not(
            "red-at-head",
            format!(
                "{} at {short} went red as a batch at this head; it is batch-ready again after \
                 a new commit",
                f.worker
            ),
        ));
    }
    // 0.4.9 trial: a head a cut dropped for a conflict was offered to the lane again ten
    // seconds later, before its worker had committed anything, and dropped again. Same shape
    // as the red: the record says this head conflicts, and only a new commit changes that.
    if f.dropped_at_head {
        return Err(not(
            "dropped-at-head",
            format!(
                "{} at {short} was dropped from a batch for a conflict at this head; it is \
                 batch-ready again after a new commit",
                f.worker
            ),
        ));
    }
    let beads: Vec<String> = f
        .carried
        .iter()
        .filter(|b| f.held.contains(b))
        .cloned()
        .collect();
    if beads.is_empty() && !f.coordinator {
        return Err(not(
            "no-claimed-bead",
            format!(
                "{} at {short}: no commit in main..{short} names a bead {} holds a claim on (carried: {}; held: {})",
                f.worker,
                f.worker,
                if f.carried.is_empty() {
                    "none".to_string()
                } else {
                    f.carried.join(" ")
                },
                if f.held.is_empty() {
                    "none".to_string()
                } else {
                    f.held.join(" ")
                },
            ),
        ));
    }
    // Precheck (2026-09-25): where the repo declares a precheck, the lane cuts a head only on a
    // recorded green one AT that head. An adopter built exactly this from a log file its lane
    // script parsed, 2026-09-05..07: a worker was cut before its check finished, the
    // coordinator relayed "checked" for a check still running, and a hand-over that died before
    // its precheck left the previous run's green trailer naming a head two commits back. A row
    // keyed by sha has none of those: running is not green, and an older head is not this one.
    // Removed when the repo drops the key: a round under it with no batch red a member's
    // precheck would have caught means the precheck only delays the cut.
    if f.precheck_required && !f.precheck_green_at_head {
        return Err(not(
            "no-precheck",
            format!(
                "{} at {short}: no green precheck at this head, and this repo gates the batch on \
                 one (`precheck` in .claude/air.json); run `air record precheck -- <the repo's \
                 precheck command>` in {}'s worktree",
                f.worker, f.worker
            ),
        ));
    }
    Ok(BatchReady {
        worker: f.worker.clone(),
        head: f.head.clone(),
        beads,
    })
}

/// `precheck` from `.claude/air.json`: `true` gates batch-ready on a green `precheck` run at
/// the branch head (precheck, 2026-09-25). Absent or anything but `true` leaves the rule unchanged.
pub fn precheck_declared(repo: &Path) -> bool {
    super::handover::air_json(repo)
        .and_then(|j| j.get("precheck")?.as_bool())
        .unwrap_or(false)
}

/// Every worker worktree through [`batch_ready_rule`]: git and the ledger only, no bd. The
/// same green predicate `select` uses (air-7wf), so a branch is never both landable and
/// batch-ready.
pub fn batch_ready_for(
    ledger: &Ledger,
    repo: &Path,
) -> (Vec<BatchReady>, Vec<NotBatchReady>, Vec<String>) {
    let mut ready = Vec::new();
    let mut not = Vec::new();
    let mut errors = Vec::new();
    let worktrees = match git::worktrees(repo) {
        Ok(w) => w,
        Err(e) => {
            errors.push(format!("git worktree list: {e}"));
            return (ready, not, errors);
        }
    };
    // air-i6fd: from the ref, not the running cwd's HEAD. This path is where the defect was
    // VISIBLE — running from its own worktree, alerts' branch compared against itself and was
    // reported batch-ready in the same snapshot where `select` said `landable: []`. One bug in
    // two paths, disagreeing because being wrong the same way surfaces differently.
    let main_tip = match git::main_tip(repo) {
        Ok(t) => t,
        Err(e) => {
            errors.push(format!("git rev-parse main: {e}"));
            return (ready, not, errors);
        }
    };
    let claims = ledger.open_claims().unwrap_or_default();
    let red_batches = ledger.red_batches(Kind::Verify).unwrap_or_default();
    let dropped = ledger.batch_dropped_heads().unwrap_or_default();
    let precheck_required = precheck_declared(repo);
    for (path, _) in worktrees {
        let worker = air_ledger::paths::worker_name_for(&path).unwrap_or_default();
        // The main checkout is main; the lane's branch is the batch, not a member. The
        // coordinator's worktree is a member (air-jc2p.1).
        let coordinator = match super::hook::role_for(&worker) {
            "worker" => false,
            "coordinator" if worker != "main" => true,
            _ => continue,
        };
        let head = match git::head(&path) {
            Ok(h) => h,
            Err(e) => {
                errors.push(format!("{worker}: git rev-parse HEAD: {e}"));
                continue;
            }
        };
        let green_at_head = match super::green::at(ledger, &path, &head, Kind::Verify) {
            Ok(e) => e.holds(),
            Err(e) => {
                errors.push(format!("{worker}: green lookup: {e}"));
                continue;
            }
        };
        // Its own kind, so a precheck green never stands for a verify green nor the reverse.
        let precheck_green_at_head = precheck_required
            && match super::green::at(ledger, &path, &head, Kind::Precheck) {
                Ok(e) => e.holds(),
                Err(e) => {
                    errors.push(format!("{worker}: precheck lookup: {e}"));
                    false
                }
            };
        let ahead = coordinator
            && git::run(
                repo,
                &["rev-list", "--count", &format!("{main_tip}..{head}")],
            )
            .ok()
            .and_then(|n| n.trim().parse::<u64>().ok())
            .is_some_and(|n| n > 0);
        // Only a red AT this head, which a batch of one is: in a larger batch the red may be
        // another member's doing, and this head was never verified alone.
        let red_batch_at_head = red_batches
            .iter()
            .any(|r| r.sha == head && r.members.iter().any(|m| m.sha == head));
        let dropped_at_head = dropped.iter().any(|(w, h)| *w == worker && *h == head);
        let facts = BatchFacts {
            red_batch_at_head,
            dropped_at_head,
            precheck_required,
            precheck_green_at_head,
            coordinator,
            ahead,
            contains_main: git::is_ancestor(repo, &main_tip, &head).unwrap_or(false),
            green_at_head,
            carried: super::attribution::attributed_in_range(repo, &format!("main..{head}"))
                .declared,
            held: claims
                .iter()
                .filter(|c| c.worker == worker)
                .map(|c| c.bead.clone())
                .collect(),
            worker,
            head,
        };
        match batch_ready_rule(&facts) {
            Ok(b) => ready.push(b),
            Err(n) => not.push(n),
        }
    }
    (ready, not, errors)
}

/// Longest wait first, the order `air land --all` uses, so what `air status` lists is the
/// order it will land in (air-3pz).
fn sort_by_wait(v: &mut [Landing]) {
    v.sort_by(|a, b| b.minutes.cmp(&a.minutes).then_with(|| a.bead.cmp(&b.bead)));
}

/// The landings alone, without a full `gather`: bd's `awaiting_review` list, the claim row
/// that names who handed each over, and that worker's green at HEAD. Derived every time, so
/// two readers cannot disagree (air-6p5). bd absent or slow means no landings, not an error.
/// Which candidate ids this worker is responsible for, from Air's OWN ledger — no bd call.
///
/// **Claimed by this worker, and not already landed.** Both halves come from tables Air
/// writes, so neither moves when git does. There used to be a third: claimed since the branch
/// point. It was the bug that made `air land` unusable in the adopter (air-6u5) — **merging main
/// moves the branch point forward past the claim that started the work, and landing requires
/// merging main**, so preparing to land was what destroyed the attribution. air-4re had
/// already taken that narrowing off declared ids for the same reason; this takes it off the
/// guessed ones, which is what a repo with no trailers has.
///
/// "Not already landed" is the bound that replaces it, and it is exact rather than a
/// heuristic: Air records the beads of every landing itself.
///
/// `bd show` does not batch — 1 id ~1.4 s, 17 ids 19-21 s, measured 2026-08-22 — so no bd call
/// happens here at all. `air land` fetches acceptance for the one branch it is landing.
fn known_beads(
    ledger: &air_ledger::Ledger,
    ids: &[String],
    worker: &str,
) -> Result<Vec<String>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let claimed: Vec<String> = {
        let mut st = ledger
            .conn()
            .prepare("SELECT DISTINCT bead FROM claims WHERE worker=?1")
            .map_err(|e| format!("claims: {e}"))?;
        let rows = st
            .query_map([worker], |r| r.get::<_, String>(0))
            .map_err(|e| format!("claims: {e}"))?;
        rows.collect::<rusqlite::Result<_>>()
            .map_err(|e| format!("claims: {e}"))?
    };
    let landed: Vec<String> = ledger
        .landings()
        .map_err(|e| format!("landings: {e}"))?
        .into_iter()
        // air-8zn: positive on what landed. This filter was already positive but named one
        // result, so a bead on a `landed-refuted` row still counted as unlanded here.
        .filter(air_ledger::landings::Landing::landed)
        .flat_map(|l| l.beads)
        .collect();
    Ok(ids
        .iter()
        .filter(|i| claimed.iter().any(|c| c == *i) && !landed.iter().any(|l| l == *i))
        .cloned()
        .collect())
}

/// The narrowing, exposed for `air selftest`: it is the half of air-6u5 that a probe can
/// reach without a git repo, and the half that was wrong.
pub fn attributable_for_test(
    ledger: &air_ledger::Ledger,
    ids: &[String],
    worker: &str,
) -> Result<Vec<String>, String> {
    known_beads(ledger, ids, worker)
}

/// A bead bd has closed whose commits have not reached main (air-gazh).
///
/// **How this state is reached with no error anywhere.** A worker closes on a batch green,
/// legitimately: the close gate passes. The coordinator then commits to main, so that batch no
/// longer contains main and stops being landable. The lane folds the members into the next cut;
/// that one reds, the one after is killed, the one after lands different workers. The branch has
/// simply not landed — no refusal, no red, nothing wrong locally. From inside the worktree the
/// bead reads closed, the branch is green and the tree is clean, so **the party best placed to
/// notice is the last who will**: every local signal is correct. An adopter had six at once,
/// found by their coordinator while answering an unrelated question, and derived independently
/// by their worker from a different join — same six.
///
/// **Removal condition, and it is not "when it stops firing"** (air-gazh is explicit): this
/// state becomes unreachable when landing stops depending on a branch containing main at the
/// moment someone looks — that is, when a green recorded for a tree can be landed after main
/// moves without a re-merge, so a closed bead's commits cannot be stranded by main moving
/// underneath them. Delete it then. Until then a quiet round is the mechanism working, not
/// evidence against it: this repo had zero on 2026-09-07 only because its coordinator landed
/// every branch within minutes of its close, and a fleet that batches leaves this residue
/// whenever a batch stops being landable.
#[derive(Debug, Clone, Serialize)]
pub struct ClosedNotLanded {
    pub bead: String,
    pub worker: String,
    pub head: String,
    /// Why the branch is not landable right now, when that is known — the same sentence
    /// `landable` shows. `None` when the branch could land as it stands and simply has not.
    pub blocked: Option<String>,
}

/// A branch that is not landable, and why. Never silent (air-6u5).
#[derive(Debug, Clone, Serialize)]
pub struct Skipped {
    pub worker: String,
    /// The precondition that failed, as a name a person can grep for.
    pub check: &'static str,
    pub detail: String,
    /// The command that would make it landable.
    pub fix: String,
}

/// Everything selection found: what can land, what cannot and why, and what broke.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Selection {
    pub landings: Vec<Landing>,
    pub skipped: Vec<Skipped>,
    /// Worktree branches whose head main already contains, as `(worker, head)`: nothing to
    /// land and nothing to fix (0.4.9 trial). Read by `air handover`, which used to say "NOT
    /// landable" beside "the close would pass" for work already on main, because a head merged
    /// by a lane's batch carries no green at its own sha.
    pub in_main: Vec<(String, String)>,
    /// Real failures — git or the ledger — as distinct from "does not qualify". An error here
    /// must never read as an empty queue.
    pub errors: Vec<String>,
}

/// What `air land` may land, why each other branch may not, and what failed.
///
/// **Selection is: a worktree branch carrying a recorded green at its head, and the beads its
/// merge range names** (air-7kp) — declared by a `Bead:` trailer, or guessed from prose on
/// commits predating the trailer and narrowed to this worker's unlanded claims.
///
/// **Nothing here is silent** (air-6u5). Every branch that does not qualify says which
/// precondition it failed and the command that fixes it, and every git or ledger failure is an
/// error rather than an absence. `air land --all` returning `{"landed": [], "ok": true}` was
/// the worst answer available: there was no output to disbelieve, so a caller concluded the
/// queue was empty. The adopter hit it with every precondition verified by hand and fell back to
/// their own `make land`; this repo hit it twice the same day.
pub fn select(repo: &Path) -> Selection {
    let mut out = Selection::default();
    let (ledger, _) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            out.errors.push(format!("ledger: {e}"));
            return out;
        }
    };
    let at = now();
    let worktrees = match git::worktrees(repo) {
        Ok(w) => w,
        Err(e) => {
            out.errors.push(format!("git worktree list: {e}"));
            return out;
        }
    };
    // air-i6fd: main's tip, from the REF and once for the whole scan, never from the running
    // cwd's HEAD. From a worktree this used to be that worktree's own head, so the running
    // worker's branch was compared against ITSELF — `is_ancestor(head, head)` is always true,
    // so it took the already-in-main path and left through `Ok(false) => continue`, the one
    // exit here that says nothing, appearing in neither `landable` nor `skipped`. Every other
    // branch had `contains_main` measured against a commit that was never main.
    //
    // An error RETURNS rather than defaulting. `unwrap_or_default()` turned a git failure into
    // an empty sha that silently disqualified every branch as "does not contain main" — a
    // failure reading as an absence, which is the one thing air-6u5 says this must never do.
    let main_tip = match git::main_tip(repo) {
        Ok(t) => t,
        Err(e) => {
            out.errors.push(format!("git rev-parse main: {e}"));
            return out;
        }
    };
    // The coordinator's own worktree head, found the way `batch_ready_for` finds the
    // coordinator's branch as a batch member: the worktree whose name has the coordinator's
    // role, other than the main checkout. Used only for a range that names no bead.
    let coordinator_head = worktrees.iter().find_map(|(p, _)| {
        let w = air_ledger::paths::worker_name_for(p).ok()?;
        if w != "main" && super::hook::role_for(&w) == "coordinator" {
            git::head(p).ok()
        } else {
            None
        }
    });
    for (path, _) in worktrees {
        let worker = air_ledger::paths::worker_name_for(&path).unwrap_or_default();
        // The coordinator's checkout (`main`, or its own worktree) is not a candidate, and
        // never was: its commits reach main in a lane's batch. The lane's branch IS one, since
        // `air land --worker <lane>` lands the batch through here (air-jc2p.4).
        if super::hook::role_for(&worker) == "coordinator" {
            continue;
        }
        let head = match git::head(&path) {
            Ok(h) => h,
            Err(e) => {
                out.errors
                    .push(format!("{worker}: git rev-parse HEAD: {e}"));
                continue;
            }
        };
        // Already in main: nothing waiting, nothing to say, and asked BEFORE the green, since
        // a head a lane's batch merged has no green at its own sha and used to be reported as
        // not landable for want of one (0.4.9 trial).
        if git::is_ancestor(repo, &head, &main_tip).unwrap_or(false) {
            out.in_main.push((worker.clone(), head.clone()));
            continue;
        }
        match super::green::at(&ledger, &path, &head, Kind::Verify).map(|e| e.holds()) {
            Ok(true) => {}
            Ok(false) => {
                out.skipped.push(Skipped {
                    check: "green-at-head",
                    detail: format!(
                        "{worker} has no recorded green at its head {}",
                        head.get(..8).unwrap_or(&head)
                    ),
                    // air-155w: advice about a WORKER, printed on the coordinator's
                    // surface. Under a verify lane that worker must not record a green, so
                    // this names the condition and leaves who satisfies it to the repo's flow.
                    fix: "a green at that head; `air handover` in that worktree names \
                          what it needs"
                        .to_string(),
                    worker: worker.clone(),
                });
                continue;
            }
            Err(e) => {
                out.errors.push(format!("{worker}: green lookup: {e}"));
                continue;
            }
        }
        // air-y3v: THE predicate `air land` applies to a branch, not a second implementation
        // of it. `select` used to stop at the green above, so every land invalidated this list
        // for every other branch and the surfaces offered `air land <bead>` for branches that
        // would be refused. `Site` is deliberately not built here: `air status` may be running
        // from a worktree and has no business asserting where a future `air land` will run.
        let facts = super::land::Facts {
            worker: &worker,
            branch_exists: true,    // the head above came from this worktree
            already_in_main: false, // left above
            contains_main: git::is_ancestor(repo, &main_tip, &head).unwrap_or(false),
            branch_head: &head,
            green_at: Some(&head), // established by the green check above
        };
        let blocked = match super::land::branch_check(&facts) {
            Ok(true) => None,
            // Already in main: nothing waiting, nothing to say.
            Ok(false) => continue,
            Err(why) => Some(why),
        };
        let range = format!("main..{head}");
        let found = super::attribution::attributed_in_range(repo, &range);
        // A declared bead stands as it is; only a guessed one is narrowed (air-4re).
        let mut ids = found.declared;
        match known_beads(&ledger, &found.guessed, &worker) {
            Ok(g) => ids.extend(g),
            Err(e) => {
                out.errors.push(format!("{worker}: {e}"));
                continue;
            }
        }
        // air-kexg: a branch whose ONLY commits are session-journal entries carries no bead by
        // design, and is landable anyway. The journal (air-3xww) is explicitly not work on a
        // bead, so a journal commit is the one commit a worker legitimately writes that names
        // none. Everything else with no bead is refused exactly as before, and the refusal now
        // names this case rather than sending a worker to amend a commit that is not about a
        // bead — which is what two workers were told tonight, and what the coordinator worked
        // around by having them name a bead that was already closed and that they had not
        // touched.
        let changed = git::changed_since(&path, "main").unwrap_or_default();
        let journal = super::handover::journal_dir(repo);
        let journal_branch = ids.is_empty() && journal_only(&changed, journal.as_deref());
        // A range of only the coordinator's commits carries no bead either. The batch-ready
        // rule takes the coordinator's branch with none, so the lane cuts it and records a
        // green, and refusing it here stranded that green (the 0.4.4 live trial).
        let coordinator_branch = ids.is_empty()
            && !journal_branch
            && coordinator_head.as_deref().is_some_and(|c| {
                let revs = |range: String| -> Vec<String> {
                    git::run(repo, &["rev-list", "--no-merges", &range])
                        .map(|o| o.lines().map(str::to_string).collect())
                        .unwrap_or_default()
                };
                coordinator_only(
                    &revs(format!("{main_tip}..{head}")),
                    &revs(format!("{main_tip}..{c}")),
                )
            });
        let journal_branch = journal_branch || coordinator_branch;
        if ids.is_empty() && !journal_branch {
            let hint = journal
                .as_deref()
                .map(|d| {
                    format!(
                        "; a branch of journal entries alone (everything under {d}/) needs no \
                         bead and lands as it is, but this range touches more than that"
                    )
                })
                .unwrap_or_default();
            out.skipped.push(Skipped {
                check: "no-bead-named",
                detail: format!(
                    "{worker} is green at {} but no commit in main..{} declares a bead{hint}",
                    head.get(..8).unwrap_or(&head),
                    head.get(..8).unwrap_or(&head)
                ),
                // The "or land it by name" this used to offer never worked: a bead absent
                // from the range is refused as "no green branch names it" (air-09b). The lane
                // is not told to amend: that moves its head off the sha its green is at.
                fix: no_bead_fix(&worker),
                worker: worker.clone(),
            });
            continue;
        }
        // Oldest commit since main: how long this branch has waited. Used for ORDERING only —
        // never to decide whether a bead belongs, which is what air-6u5 was.
        let since = git::branch_point_time(&path, "main")
            .ok()
            .and_then(|t| t.parse::<jiff::Timestamp>().ok())
            .map(|t| t.to_string())
            .unwrap_or_else(|| at.clone());
        // air-kexg: a journal-only branch produces ONE landing carrying no bead, rather than
        // none at all — which is why it used to vanish from the list entirely.
        let carried: Vec<Option<String>> = if journal_branch {
            vec![None]
        } else {
            ids.into_iter().map(Some).collect()
        };
        for bead in carried {
            out.landings.push(Landing {
                // air-y3v: the command a reader can actually run. A branch behind main is
                // still SHOWN — the coordinator needs to know work is waiting — but what it
                // needs is a re-merge, and offering `air land` there is what cost the owner
                // three cycles in an hour.
                command: match &blocked {
                    None => land_command(&worker),
                    Some(_) => super::land::remerge_command(),
                },
                blocked: blocked.clone(),
                // Filled by `air land` for the branch it is landing (`acceptance_for`), not
                // here: this runs on every `air status` and bd is far too slow per id.
                acceptance: Vec::new(),
                bead,
                head: head.clone(),
                minutes: minutes_between(&since, &at).unwrap_or(0),
                worker: worker.clone(),
            });
        }
    }
    sort_by_wait(&mut out.landings);
    out
}

/// Just the landable list, for the read-only callers (`air status`).
/// `air land` uses [`select`], because it is the caller that must not read an error as empty.
/// Acceptance clauses for the beads of ONE branch, fetched at land time.
///
/// This is the expensive call (`bd show` is ~1.4 s per id, see [`known_beads`]), and it is
/// affordable here only because landing already runs the repo's full verify. Both shapes are
/// read: bd's `acceptance_criteria` field when the bead set it — bd omits the key entirely
/// when unset — and the `## Acceptance Criteria` section of the description otherwise
/// (air-ayp).
///
/// A bd failure is an ERROR, never an empty clause list (air-6u5, capture
/// 01M0NGZQX9DW6NAABS5ES30SPQ). Swallowing it would print "states no acceptance criteria" for
/// every bead in the merge, which reads as a bead nobody wrote criteria for rather than as bd
/// not answering — the same mistake as `{"landed": [], "ok": true}` one layer down.
pub fn acceptance_for(repo: &Path, beads: &[String]) -> Result<Vec<Vec<String>>, String> {
    if beads.is_empty() {
        return Ok(Vec::new());
    }
    let mut bd = super::claim::bd_for(repo);
    // Labelled whether or not the budget is overridden: the label names the SITE, and the
    // recorded `budget_ms` names whatever budget was actually in force there. The budget is
    // the client's own, scaled by the id count inside `show_all` (air-fzv, then air-8lj8 moved
    // the scaling into `air_bd::budget_for` so every multi-id call gets it).
    bd.label = air_ledger::budgets::BD_ACCEPTANCE;
    acceptance_with(&bd, beads, super::claim::bd_overridden())
}

/// `38 s`, `0.5 s`: whole seconds where they are whole, one decimal otherwise.
pub fn duration_line(d: std::time::Duration) -> String {
    let ms = d.as_millis();
    match ms.checked_rem(1000) {
        Some(0) => format!("{} s", ms.checked_div(1000).unwrap_or(0)),
        _ => format!("{:.1} s", d.as_secs_f64()),
    }
}

/// The read itself, against the client's own budget for this many ids. The error names the id
/// count, the budget and the override, so a refusal built on it says what was hit and how to
/// raise it.
pub fn acceptance_with(
    bd: &air_bd::BdCli,
    beads: &[String],
    overridden: bool,
) -> Result<Vec<Vec<String>>, String> {
    let issues = air_bd::WorkLedger::show_all(bd, beads).map_err(|e| {
        format!(
            "bd show for {}: {e}",
            super::claim::budget_words(beads.len(), bd.budget(beads.len()), overridden)
        )
    })?;
    Ok(beads
        .iter()
        .map(|b| match issues.iter().find(|i| &i.id == b) {
            Some(i) => super::acceptance::clauses_of(&i.acceptance_criteria, &i.description),
            None => Vec::new(),
        })
        .collect())
}

/// Minutes between two RFC 3339 timestamps; None when either does not parse.
pub fn minutes_between(earlier: &str, later: &str) -> Option<i64> {
    let a: jiff::Timestamp = earlier.parse().ok()?;
    let b: jiff::Timestamp = later.parse().ok()?;
    b.duration_since(a).as_secs().checked_div(60)
}

/// A landing whose merge is in main and whose outcome is not recorded yet (air-bxe).
#[derive(Debug, Clone, Serialize)]
pub struct LandingInFlight {
    pub landing: air_ledger::landings::Landing,
    /// Is the `air land` process that wrote this row still running? `None` when it recorded no
    /// pid. `Some(false)` is the interesting one: that land was killed, main still holds the
    /// merge, and the rollback never ran.
    pub alive: Option<bool>,
}

/// Landings still in flight, newest first, each with the liveness of the process that started
/// it (air-bxe). Unlike a verify row, one of these is NEVER pruned: a killed land left main
/// changed, and deleting the only evidence of that is the failure, not the tidy-up.
pub fn landings_in_flight(ledger: &Ledger) -> Vec<LandingInFlight> {
    ledger
        .landings_in_flight()
        .unwrap_or_default()
        .into_iter()
        .map(|l| LandingInFlight {
            alive: l.pid.map(super::lease::pid_alive),
            landing: l,
        })
        .collect()
}

/// One line for a landing in flight: what merged, when, and whether anything is still working
/// on it. The second half is the part `pgrep` was being asked for.
pub fn landing_in_flight_line(f: &LandingInFlight, at: &str) -> String {
    let l = &f.landing;
    let merge = l.merge_commit.as_deref().unwrap_or("");
    let elapsed = seconds_between(&l.finished_at, at)
        .map(|s| format!("{s}s ago"))
        .unwrap_or_else(|| "at an unreadable time".into());
    let state = match f.alive {
        // air-odv: `air land` runs no verify and arms no rollback; the fast-forward is the
        // only step that moves main, so a gone process either moved it or did not.
        Some(false) => format!(
            "the `air land` process (pid {}) is GONE before it reported. If main is at {merge}, \
             the landing happened; if main is still at {}, nothing moved and `air land` may \
             run again",
            l.pid.unwrap_or(0),
            l.tip_sha.as_deref().unwrap_or("<tip>")
        ),
        Some(true) => format!("landing now (pid {})", l.pid.unwrap_or(0)),
        None => "no pid recorded, so nothing can say whether it is still running".into(),
    };
    format!(
        "{} ({}) merged at {} {elapsed}, main was {}: {state}",
        l.worker,
        l.beads.join(" "),
        merge.get(..8).unwrap_or(merge),
        l.tip_sha
            .as_deref()
            .map(|t| t.get(..8).unwrap_or(t).to_string())
            .unwrap_or_else(|| "-".into()),
    )
}

/// A rewound merge that is still sitting in somebody's worktree (air-ob0).
#[derive(Debug, Clone, Default, Serialize)]
pub struct RewoundCarried {
    /// The merge commit `air land` reset main away from.
    pub merge_commit: String,
    pub worker: String,
    /// Workers whose branch HEAD still contains it.
    pub carried_by: Vec<String>,
    pub rewound_at: String,
}

/// Which worktrees' HEADs contain `sha`, by worker name. One `merge-base --is-ancestor` each,
/// over the worktree list Air already enumerates (air-ob0).
pub fn carrying(repo: &Path, sha: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (path, _) in git::worktrees(repo).unwrap_or_default() {
        let Ok(head) = git::head(&path) else { continue };
        if head == sha || git::is_ancestor(&path, sha, &head).unwrap_or(false) {
            let name = air_ledger::paths::worker_name_for(&path).unwrap_or_else(|_| {
                path.file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            });
            out.push(name);
        }
    }
    out
}

/// Rewound landings whose merge is still carried by somebody (air-ob0), newest first.
///
/// The adopter, 2026-08-23: *"A rollback un-lands a branch from main but cannot un-merge it from
/// anyone who took it."* A worker who merged main during the armed window — the documented
/// thing to do when main moves — keeps the rewound commits. That is a recorded green for a tree
/// main will never have, with `air handover` passing and `air land` merging it back in.
///
/// So the window is not unverified code in main. It is unverified code that has already
/// propagated, to exactly the workers following the rule.
///
/// Self-clearing, with no expiry to choose: a rewound merge that is back in main (it re-landed)
/// or that nobody carries any more simply stops matching.
pub fn rewound_carried(
    repo: &Path,
    ledger: &Ledger,
    main_head: Option<&str>,
) -> Vec<RewoundCarried> {
    let mut out = Vec::new();
    for l in ledger.landings().unwrap_or_default() {
        if l.result != "rewound" {
            continue;
        }
        let Some(merge) = l.merge_commit.as_deref().filter(|m| !m.is_empty()) else {
            continue;
        };
        // Back in main means it landed on a later attempt: nothing to warn about.
        if main_head.is_some_and(|h| git::is_ancestor(repo, merge, h).unwrap_or(false)) {
            continue;
        }
        let carried_by = carrying(repo, merge);
        if carried_by.is_empty() {
            continue;
        }
        out.push(RewoundCarried {
            merge_commit: merge.to_string(),
            worker: l.worker.clone(),
            carried_by,
            rewound_at: l.finished_at.clone(),
        });
    }
    out
}

/// What a rewind owes the worktrees that took the un-landed commits (air-ob0). Empty when
/// nobody carries them, which is the ordinary case.
pub fn rewind_propagation(merge: &str, carried_by: &[String]) -> Vec<String> {
    if carried_by.is_empty() {
        return Vec::new();
    }
    vec![format!(
        "the un-landed commits are already in {}: their branch contains {} and their recorded \
         green is for a tree main will never have. Each must `git reset` or re-merge main and \
         re-verify before handing over — `air handover` will pass on it as it stands.",
        carried_by.join(", "),
        merge.get(..8).unwrap_or(merge)
    )]
}

/// The holders of one file that can actually collide (air-pwvk).
///
/// A holder with nothing uncommitted and no commits outside `main` has nothing to collide
/// with: its work is landed and the row is a memory. The `overlap:` line named every holder a
/// row existed for, so on a long-lived shared file it named essentially everyone who had ever
/// touched it, forever — six holders on `CLAUDE.md` in this repo, **every one `clean now`, true
/// positives zero**, and the same shape reached an adopter's worker three times in a night with
/// no true positive either.
///
/// **This is not a threshold and no age cutoff fixes it.** The entries were not wrong: each
/// carried its own age and its own `clean now`, rendered since air-et0o. A reader facing six
/// clean holders is not judging a fact, they are skipping a list — which is measurable rather
/// than arguable, because the coordinator ran every `air status` of that round through
/// `grep -vE '^overlap'`. A line whose reader has built a filter for it has already failed, and
/// the filter is the measurement.
///
/// `clean now` alone is the wrong test and the bead said so: a holder that committed and has
/// not landed still overlaps. Both halves are already computed by `holdings::compute` —
/// `uncommitted` from `git status` in that worktree, `committed` from `main...HEAD` — and the
/// predicate used neither.
///
/// `air holdings` is untouched and still shows every holder, journaled-only included:
/// suppressing a summary and hiding a fact are different things.
pub fn colliding(holders: &[holdings::Holding]) -> Vec<&holdings::Holding> {
    holders
        .iter()
        .filter(|h| h.uncommitted || h.committed)
        .collect()
}

/// Verifies running right now, oldest first, with dead pids pruned on the way out (air-4cr).
/// Shared by `air status` and `air land`, so both answer the question the same way.
pub fn verifies_in_flight(ledger: &Ledger) -> Vec<air_ledger::verify::InFlight> {
    ledger
        .in_flight_pruned(super::lease::pid_alive)
        .unwrap_or_default()
}

/// One line for a verify in flight: who, how long, and at which sha. Seconds, not minutes —
/// a verify is ~420 s in the adopter's repo, so a minutes-only reading rounds most of it to 0.
pub fn in_flight_line(f: &air_ledger::verify::InFlight, at: &str) -> String {
    let elapsed = seconds_between(&f.started_at, at)
        .map(|s| format!("{s}s"))
        .unwrap_or_else(|| "unknown".into());
    format!(
        "{} started {} ago: {} at {}",
        f.worker,
        elapsed,
        f.command,
        f.sha.get(..8).unwrap_or(&f.sha)
    )
}

/// Is one of `s.verifies_in_flight` this worker's? (air-t6ap.)
///
/// A verify is progress, and it is progress that must not be interrupted: a land invalidates
/// every run in flight (air-4cr) and a lane that claims a bead mid-batch cannot cut the batch.
/// The dead-pid pruning happens in `verifies_in_flight`, so a row that reaches the snapshot is
/// a live run.
pub fn verify_running(s: &Snapshot, worker: &str) -> bool {
    s.verifies_in_flight.iter().any(|f| f.worker == worker)
}

/// Is something that is not a session running in this worker's tree? (`cmd::readers`.)
///
/// The same shape as [`verify_running`], for the runs `air record` never saw: an adopter's
/// worker under a lane ran an unrecorded precheck, "mid-precheck" and "doing nothing" read as
/// the same row, and the coordinator prompted a busy worker (2026-09-07). An unknown lookup
/// answers false, so the condition fires as it did before this existed.
pub fn tree_busy(s: &Snapshot, worker: &str) -> bool {
    s.tree_readers.busy(worker)
}

/// 24 hours before an RFC 3339 timestamp, in the same format; the epoch when it does not
/// parse, which widens the window rather than hiding rows.
pub fn day_before(at: &str) -> String {
    at.parse::<jiff::Timestamp>()
        .ok()
        .and_then(|t| t.checked_sub(jiff::SignedDuration::from_hours(24)).ok())
        .map_or_else(|| "1970-01-01T00:00:00Z".to_string(), |t| t.to_string())
}

/// Seconds between two RFC 3339 timestamps; None when either does not parse.
pub fn seconds_between(earlier: &str, later: &str) -> Option<i64> {
    let a: jiff::Timestamp = earlier.parse().ok()?;
    let b: jiff::Timestamp = later.parse().ok()?;
    Some(b.duration_since(a).as_secs())
}

/// The pure part. Every condition names the worker, how long, and what to do.
pub fn attention(s: &Snapshot, now: &str, t: Thresholds) -> Vec<Attention> {
    let mut out = Vec::new();
    for w in &s.workers {
        let has_claim = !w.claims.is_empty();
        let beads = || {
            w.claims
                .iter()
                .map(|c| c.bead.as_str())
                .collect::<Vec<_>>()
                .join(",")
        };
        // air-sze: `gone-with-claim` was NOT deleted, whatever the comment here used to say.
        // It said air-s7c removed it on 2026-08-22 because "a dead session holding a claim now
        // falls through to the ordinary session states below, which do fire". The arm below
        // still emits it, and that reason could never have held for it: this is the branch for
        // a worker with NO session row, so there are no session states below to fall through
        // to. A crashed worker still holding a bead is what nothing else reports. It is now in
        // the registry with a removal condition instead of being described as gone.
        match &w.session {
            Some(sess) => {
                let age = minutes_between(&sess.changed_at, now).unwrap_or(0);
                // A `"stuck"` arm stood first here until 2026-09-06 (air-12k). Its state was
                // written only by `HookEvent::PermissionRequest`, which auto mode never sends,
                // so the arm matched nothing in any recorded day; the deletion record and the
                // zero's cause (case 3b, air-byw) are in `mechanisms.rs`.
                match sess.state.as_str() {
                    "idle" if has_claim && age >= t.idle_with_claim_min => out.push(Attention {
                        worker: w.worker.clone(),
                        kind: kinds::IDLE_WITH_CLAIM,
                        detail: format!(
                            "idle {age} min holding {}; prompt them, or `air reclaim` if abandoned",
                            beads()
                        ),
                        for_minutes: age,
                        fingerprint: String::new(),
                    }),
                    // "prompt them" needs somebody to prompt. `gather` already asks the OS
                    // whether the session's process is alive; the condition never read the
                    // answer, so the two longest-lived rows in this ledger were dead sessions
                    // held open for 4 885 minutes each ("idle 4885 min, 2 beads ready; prompt
                    // them", conditions 2026-08-22T21:50 -> 2026-08-29T15:54). A dead session
                    // holding a claim still surfaces as `idle-with-claim`, which is air-s7c's
                    // point and is untouched here: there the claim is what needs a person.
                    // Removal: when a dead session is pruned on the pid alone, this is dead
                    // code and goes with it.
                    // air-uir: on the CLAIMABLE count, not bd's raw one. This is a condition
                    // whose whole output is "go interrupt a worker", so firing it over work
                    // the worker cannot take is the cheapest possible way to teach both of
                    // them to ignore conditions.
                    //
                    // Noted plainly because the 2026-08-22 capture was right to: this MOVES
                    // the threshold rather than renaming a field. The condition now fires
                    // strictly less often, and what it means is narrower and truer — "there is
                    // work this worker could start", not "the queue is non-empty".
                    //
                    // air-t6ap: and not while this worker's own verify is running. An
                    // adopter's verify lane was offered 58 claimable beads 945 SECONDS into a
                    // batch verify, with `air status` printing `verify in flight: w4 started
                    // 945s ago` three lines above the condition contradicting it. A lane holds
                    // no bead while it batches — roles.md says so in those words (air-80x.6) —
                    // so the condition fired on a documented state, and its remedy is worse
                    // than useless there: a lane that claims a bead mid-batch cannot cut the
                    // batch, and three workers were waiting on that run. Their coordinator
                    // checked the run was alive and did not prompt; an unattended one would
                    // have interrupted it at minute fifteen.
                    //
                    // "No claim" was standing in for "nothing in progress", and a running
                    // verify is progress. Same shape as air-uir moving this very condition
                    // from bd's raw depth to the claimable count: keyed to the instance rather
                    // than to the meaning.
                    "idle"
                        if !has_claim
                            && w.role == "worker"
                            && sess.pid_alive != Some(false)
                            && !verify_running(s, &w.worker)
                            && !tree_busy(s, &w.worker)
                            && s.claimable_depth.is_some_and(|n| n > 0)
                            && age >= t.idle_noclaim_min =>
                    {
                        out.push(Attention {
                            worker: w.worker.clone(),
                            kind: kinds::IDLE_WITHOUT_CLAIM,
                            detail: format!(
                                "idle {age} min, {} bead(s) they can claim",
                                s.claimable_depth.unwrap_or(0)
                            ),
                            for_minutes: age,
                            fingerprint: String::new(),
                        });
                    }
                    "working" | "running" if has_claim && age >= t.silent_with_claim_min => {
                        out.push(Attention {
                            worker: w.worker.clone(),
                            kind: kinds::SILENT_WITH_CLAIM,
                            detail: format!(
                                "no hook event for {age} min while holding {}; session may have died",
                                beads()
                            ),
                            for_minutes: age,
                            fingerprint: String::new(),
                        });
                    }
                    _ => {}
                }
            }
            None if has_claim => {
                let oldest = w
                    .claims
                    .iter()
                    .map(|c| c.claimed_at.as_str())
                    .min()
                    .unwrap_or(now);
                let age = minutes_between(oldest, now).unwrap_or(0);
                // The launch grace suppresses `gone-with-claim` only — a worker whose first
                // hook has not fired yet is not gone. It used to `continue`, which skipped the
                // whole rest of the loop, so a non-green hand-over on a session-less worker
                // was silent for the grace window and was invisible to any probe that built a
                // snapshot without a session row (found while probing air-eiv).
                if age >= t.launch_grace_min {
                    out.push(Attention {
                        worker: w.worker.clone(),
                        kind: kinds::GONE_WITH_CLAIM,
                        detail: format!(
                            "no live session but holds {}; restart `air worker {}` or `air reclaim`",
                            beads(),
                            w.worker
                        ),
                        for_minutes: age,
                        fingerprint: String::new(),
                    });
                }
            }
            None => {}
        }
        // One line per worker, not one per claim (air-0j4). A worker's HEAD is one sha, so
        // every claim it holds is not-green for the SAME reason and the same fix; the adopter's
        // status printed eleven lines for one worker, which is one fact eleven times. The
        // single-claim wording is unchanged, because that is the case that reads well already.
        //
        // Removal: when no worker ever holds two claims at once, this collapses nothing and
        // the loop above can go back to pushing per claim.
        let red: Vec<&Claim> = if w.green_at_head == Some(false) {
            w.claims
                .iter()
                .filter(|c| c.handover_attempts > 0)
                .collect()
        } else {
            Vec::new()
        };
        // Longest wait first, so `for_minutes` is the oldest attempt rather than an arbitrary
        // one, and the beads read in the order they have been waiting.
        let oldest = red
            .iter()
            .filter_map(|c| c.last_handover_at.as_deref())
            .min()
            .unwrap_or(now);
        let detail = match red.as_slice() {
            [] => None,
            [c] => Some(format!(
                "{} handed over {} time(s) without green verify at HEAD; last attempt {}",
                c.bead,
                c.handover_attempts,
                c.last_handover_at.as_deref().unwrap_or(now)
            )),
            many => Some(format!(
                "{} beads handed over without green verify at HEAD ({}); {} attempts in total; oldest {}",
                many.len(),
                many.iter()
                    .map(|c| c.bead.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                many.iter().map(|c| c.handover_attempts).sum::<i64>(),
                oldest
            )),
        };
        if let Some(detail) = detail {
            out.push(Attention {
                worker: w.worker.clone(),
                kind: kinds::HANDOVER_NOT_GREEN,
                detail,
                for_minutes: minutes_between(oldest, now).unwrap_or(0),
                fingerprint: String::new(),
            });
        }
    }
    // A lease defect is addressed to whoever WANTS the resource, and never to the holder
    // (air-q9c). It used to name the holder and tell them to `air lease break` the lease they
    // were using; the adopter saw six of those in a day while the simulator and API were
    // genuinely running (their). Staleness is a signal for other agents by
    // construction — the holder knows perfectly well they hold it.
    //
    // No audience, no condition: `lease_take` takes a defective lease automatically
    // (`Take::TakenAfter`), so a defect nobody is waiting on needs no one to act. A name in
    // `lease_wants` is someone who asked and was refused by a lease that was healthy then and
    // is not now — for them the fix is simply to ask again.
    //
    // Removal: when nothing computes a condition from a lease.
    for (l, defect) in &s.leases {
        let Some(d) = defect else { continue };
        let kind = if d.starts_with("dead") {
            kinds::LEASE_HELD_BY_DEAD_SESSION
        } else {
            kinds::LEASE_STALE
        };
        let waiting = s.lease_wants.get(&l.resource);
        for who in waiting.into_iter().flatten().filter(|w| **w != l.worker) {
            out.push(Attention {
                worker: who.clone(),
                kind,
                detail: format!(
                    "{} is held by {} and is {d} (their reason: {}); it is yours to take now — `air lease take {} --reason \"<why>\"` takes a defective lease",
                    l.resource, l.worker, l.reason, l.resource
                ),
                for_minutes: minutes_between(&l.heartbeat_at, now).unwrap_or(0),
                fingerprint: format!("{}/{}", l.resource, l.worker),
            });
        }
    }
    // air-03w: a branch that is landable NOW. Since air-7o3 the worker closes its own bead with
    // proof and never sets `awaiting_review`, so `review-waiting` above is a condition whose
    // subject this repo stopped using; nothing told the coordinator a branch was ready. The
    // worker signalling is the intent (roles.md); this is the failsafe, so a missed signal is
    // not a lost one.
    //
    // Subject is the WORKER, because one branch is one merge however many beads it carries.
    // Fingerprint is the branch head: once when it first goes green with main contained, again
    // only when the head moves, never while it sits. Age is not a change (air-s7c).
    //
    // Removal condition (mechanisms.rs `landable`): delete when a round shows every landable
    // branch landed before this pushed — i.e. the worker's signal is arriving reliably and the
    // failsafe caught nothing.
    {
        let mut by_worker: BTreeMap<&str, (&str, Vec<&str>, i64)> = BTreeMap::new();
        // air-y3v: `landable` carries blocked branches too, so the surfaces can show them with
        // the command that unblocks them. Only an unblocked one is landABLE, and announcing
        // otherwise is the defect this condition would otherwise reintroduce.
        // 0.4.8 trial: the lane's own branch is the lane's next step, not an attention matter;
        // both of that trial's `landable` rows were for it and cleared in 32 s with nothing
        // for the coordinator to do.
        let lanes = super::fanout::lanes(s);
        for l in s
            .landable
            .iter()
            .filter(|l| l.blocked.is_none() && !lanes.contains(&l.worker))
        {
            let e = by_worker
                .entry(&l.worker)
                .or_insert((&l.head, Vec::new(), 0));
            // air-kexg: a journal landing has no bead to list; the branch still shows.
            if let Some(id) = l.bead.as_deref() {
                e.1.push(id);
            }
            e.2 = e.2.max(l.minutes);
        }
        for (worker, (head, beads, minutes)) in by_worker {
            out.push(Attention {
                worker: worker.to_string(),
                kind: kinds::LANDABLE,
                detail: format!(
                    "{worker} is green at {} with main merged, carrying {}; landing is the \
                     lane's or the owner's (`air land --worker {worker}`)",
                    head.get(..8).unwrap_or(head),
                    beads.join(" ")
                ),
                for_minutes: minutes,
                fingerprint: format!("{worker}@{head}"),
            });
        }
    }
    // air-ob0, narrowed by air-odv: a rewound merge somebody still carries. No NEW rewind can
    // occur — main is fast-forwarded onto an already-green commit — so this reports history,
    // and it empties itself when nobody holds those commits any more. Registered with that as
    // its removal condition so `air audit` can answer it rather than someone arguing it.
    for r in &s.rewound_carried {
        out.push(Attention {
            worker: r.carried_by.join(", "),
            kind: kinds::REWOUND_AND_CARRIED,
            detail: rewind_propagation(&r.merge_commit, &r.carried_by).join(" "),
            for_minutes: minutes_between(&r.rewound_at, now).unwrap_or(0),
            // The commit and who holds it is the whole fact; its age is not a change.
            fingerprint: format!("{}@{}", r.merge_commit, r.carried_by.join(",")),
        });
    }
    // air-ayp: a bead that landed while this merge contradicts one of its acceptance clauses.
    // Not "Air could not read it" — refuted. Subject is the bead, so the channel says it once
    // and says it again only when the reason changes.
    //
    // air-jy99: the sentence names no action whose permissibility depends on the repo's flow.
    // It used to end "either reopen it or file what is left", and an adopter's CLAUDE.md says
    // "closed is closed — never reopen": Air was instructing their coordinator to do what their
    // own rules forbid. This is air-155w's ruling at a second surface — a flow-dependent fix
    // states a CONDITION, not a command — and the same renderer in `land.rs` said it too, which
    // is exactly how air-155w's own defect survived its first fix. No config key decides this:
    // reading a repo's flow is the thing air-155w's comment calls "a claim about a decision made
    // somewhere else". Removal: when Air is told a flow, which is not planned.
    //
    // air-ppf: the sentence names `contradicted`, never `why`. `why` is the whole record,
    // refuted and unreadable clauses together, and rendering it here put "nothing Air can
    // look up" under a headline asserting a contradiction; two sound closes read as wrong
    // ones on 2026-08-30. The unreadable clauses stay on the row and in the `air land` print.
    for o in &s.landed_open {
        let (bead, why) = (&o.bead, &o.contradicted);
        out.push(Attention {
            worker: bead.clone(),
            kind: kinds::LANDED_NOT_CLOSED,
            detail: format!(
                "{bead} landed in {} with an acceptance clause naming a file this merge did \
                 not change: {why}. That is a lookup that did not answer, NOT a contradiction \
                 (air-k6uh: six of nine such firings were clauses that held, satisfied in \
                 another commit or naming a path the clause only mentions). Read the bead: the \
                 worker closes its own with proof, so either it is done elsewhere, or what is \
                 left is untracked and must not stay that way. How a closed bead's remainder \
                 gets tracked is this repo's flow to say; Air reads no flow and prescribes \
                 nothing here (air-jy99). Landed from {}.",
                o.merge_commit.get(..8).unwrap_or(&o.merge_commit),
                o.worker
            ),
            for_minutes: 0,
            fingerprint: format!("{bead}/{why}"),
        });
    }

    // air-gazh: the inverse of the condition above, and the state no other condition reaches.
    // `landable` goes quiet the moment main moves; `landed-not-closed` needs a landing. A bead
    // closed on a batch green whose batch then stopped containing main satisfies neither, and
    // an adopter had six of them at once with no error anywhere in how they got there.
    //
    // It is a CONDITION rather than only a printed line because the reporting incident is
    // exactly a coordinator who had the fleet view and was not looking: theirs surfaced while
    // answering an unrelated question. A line nobody reads is what this already had.
    for c in &s.closed_not_landed {
        let (bead, worker) = (&c.bead, &c.worker);
        let short = c.head.get(..8).unwrap_or(&c.head);
        let why = match &c.blocked {
            Some(b) => format!("that branch cannot land as it stands: {b}"),
            None => "that branch could land as it stands and has not".to_string(),
        };
        out.push(Attention {
            worker: worker.clone(),
            kind: kinds::CLOSED_NOT_LANDED,
            detail: format!(
                "{bead} is closed but its commits are in no tree but {worker}'s worktree, at                  {short}: {why}. Nothing is wrong locally — the bead reads closed, the branch                  is green and the tree is clean — which is why the person best placed to see                  this is the last who will (air-gazh). The work exists in one place and no                  backup of that place is a landing."
            ),
            for_minutes: 0,
            fingerprint: format!("{bead}/{}", c.head),
        });
    }
    out
}

/// Build the snapshot: one row per worktree (plus any worker known only from the ledger).
/// Whether this gather may shell out to bd (air-cmn).
///
/// The channel poll ran a full `gather` every ~8 s, and every one of them called bd:
/// `in_progress`, then `show` once per open claim (one `show` for all of them since
/// air-bp0), then `awaiting_review`, then `ready`. That
/// came to about 5,700 bd calls and 2.3 hours a day waiting on bd, around the clock, to deliver
/// roughly 45 pushes (0007 §3). Only `idle-without-claim` needs any of it, for `ready_depth`.
///
/// So the poll asks for `Cached`, and pays for bd on a cadence measured in minutes instead of
/// seconds. Nothing new catches the fallback: `bd_cache` and the "answer from the cache" arms
/// already existed for a slow bd (air-19u), and this arms the same path deliberately.
#[derive(Debug, Clone, Copy)]
pub enum BdUse {
    /// Call bd under the usual short budget. A person asked, so give them today's answer.
    Fresh,
    /// Answer from `bd_cache` while the cached counts are younger than this many minutes; pay
    /// for bd only when they are older.
    CachedFor(i64),
}

/// Is the cached bd answer young enough to use? `false` when nothing is cached, so the first
/// tick after a restart still pays once and fills the cache.
pub fn cache_is_fresh(ledger: &air_ledger::Ledger, at: &str, max_age_min: i64) -> bool {
    ledger
        .bd_cache_get("ready_depth")
        .ok()
        .flatten()
        .and_then(|(_, seen)| minutes_between(&seen, at))
        .is_some_and(|age| age < max_age_min)
}

pub fn gather(repo: &Path) -> Result<Snapshot, String> {
    gather_with(repo, BdUse::Fresh)
}

pub fn gather_with(repo: &Path, bd_use: BdUse) -> Result<Snapshot, String> {
    let t0 = std::time::Instant::now();
    // Started first and joined last: `lsof` costs most of a second (`cmd::readers`), and
    // nothing below needs it until the snapshot is assembled.
    let readers_job = std::thread::spawn(super::readers::lookup);
    let (ledger, _me) = open(repo)?;
    let at = now();
    let mut trees: Vec<(String, std::path::PathBuf)> = Vec::new();
    let mut errors = Vec::new();
    let mut views: BTreeMap<String, WorkerView> = BTreeMap::new();

    // Worktrees from git.
    match git::worktrees(repo) {
        Ok(wts) => {
            for (path, _branch) in wts {
                let name = air_ledger::paths::worker_name_for(&path).unwrap_or_else(|_| {
                    path.file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default()
                });
                trees.push((name.clone(), path.clone()));
                let head = git::head(&path).ok();
                // air-7wf: the same predicate the gate and `air land` read, so this line
                // cannot say "not green" about a commit the gate would pass.
                let evidence = head
                    .as_deref()
                    .and_then(|h| super::green::at(&ledger, &path, h, Kind::Verify).ok());
                let green_at_head = evidence.as_ref().map(super::green::Evidence::holds);
                // air-5ik: where the red at this head kept its output. Only asked for when the
                // head is NOT green, so a healthy fleet pays no query for it.
                let red_log = (green_at_head == Some(false))
                    .then(|| {
                        head.as_deref()
                            .and_then(|h| ledger.latest_run_at_commit(h, Kind::Verify).ok())
                            .flatten()
                            .filter(|r| !r.is_green())
                            .and_then(|r| r.log_path)
                    })
                    .flatten();
                views.insert(
                    name.clone(),
                    WorkerView {
                        role: super::hook::role_for(&name).to_string(),
                        worker: name,
                        head,
                        green_at_head,
                        green_detail: evidence.as_ref().and_then(super::green::Evidence::detail),
                        red_log,
                        ..Default::default()
                    },
                );
            }
        }
        Err(e) => errors.push(format!("git worktree list: {e}")),
    }

    // Sessions (latest row per worker, plus every row for join/leave detection).
    let mut all_sessions: Vec<(String, String, Session)> = Vec::new();
    {
        let mut st = ledger
            .conn()
            .prepare(
                "SELECT worker, role, session_id, state, detail, changed_at, pid, project, model, \
                        enforce, stopped_at, stopped_kind, stopped_text, transcript_path \
                 FROM sessions ORDER BY changed_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    Session {
                        session_id: r.get(2)?,
                        state: r.get(3)?,
                        detail: r.get(4)?,
                        changed_at: r.get(5)?,
                        pid: r.get(6)?,
                        pid_alive: None,
                        project: r.get(7)?,
                        model: r.get(8)?,
                        enforce: r.get::<_, Option<i64>>(9)?.map(|v| v == 1),
                        has_transcript: r
                            .get::<_, Option<String>>(13)?
                            .is_some_and(|t| !t.trim().is_empty()),
                        stopped: match (
                            r.get::<_, Option<String>>(10)?,
                            r.get::<_, Option<String>>(11)?,
                        ) {
                            (Some(at), Some(kind)) => Some((
                                at,
                                kind,
                                r.get::<_, Option<String>>(12)?.unwrap_or_default(),
                            )),
                            _ => None,
                        },
                    },
                ))
            })
            .map_err(|e| e.to_string())?;
        let worktree_names: Vec<String> = views.keys().cloned().collect();
        let mut pruned = Vec::new();
        for row in rows {
            let (worker, role, mut sess) = row.map_err(|e| e.to_string())?;
            sess.pid_alive = sess.pid.map(super::lease::pid_alive);
            // A session whose worktree no longer exists and whose process is not alive is
            // a leftover (SessionEnd is not guaranteed on crash or worktree removal). Prune it
            // so it never reads as a live worker (dogfooding, 2026-08-22).
            if !worktree_names.contains(&worker) && sess.pid_alive != Some(true) {
                pruned.push(sess.session_id.clone());
                continue;
            }
            all_sessions.push((worker.clone(), role.clone(), sess.clone()));
            let v = views.entry(worker.clone()).or_insert_with(|| WorkerView {
                worker: worker.clone(),
                role: role.clone(),
                ..Default::default()
            });
            if v.session.is_none() {
                v.role = role.clone();
                v.session = Some(sess);
            }
        }
        for id in &pruned {
            let _ = ledger.conn().execute(
                "DELETE FROM sessions WHERE session_id=?1",
                rusqlite::params![id],
            );
        }
        if !pruned.is_empty() {
            errors.push(format!(
                "pruned {} session(s) with no worktree and no live process",
                pruned.len()
            ));
        }
    }

    // Open claims, reconciled against bd first: a bead that bd no longer holds as
    // in_progress (closed, awaiting_review, reopened) is not "held" by anyone, whatever the
    // ledger row says. The row is released with the bd status as reason so the history is
    // honest and no condition ever fires on it (the adopter's round: ~38 noise pushes, A3).
    //
    // bd is enrichment, not the spine. It gets a short budget (`AIR_BD_TIMEOUT_MS` overrides)
    // and after one timeout no further bd call is made this tick; the counts fall back to the
    // last answer cached in the ledger. Under load bd took 20 s, the same as the MCP tool
    // budget, so the channel got nothing exactly when the fleet was busiest (the adopter
    // 2026-08-22, air-19u).
    //
    // The budget is DERIVED from what bd costs here today, not a constant (air-p61): a flat
    // 2 s left 356 ms of headroom over bd's measured p99 and sat below the adopter's median
    // entirely. `status_bd_budget` reads the same measurement `air status` prints.
    let today_latency = super::bd_latency::for_day(ledger.dir(), &super::today());
    let mut bd = super::claim::bd_for(repo);
    bd.label = air_ledger::budgets::BD_STATUS;
    if std::env::var_os("AIR_BD_TIMEOUT_MS").is_none() {
        bd.timeout = super::bd_latency::status_bd_budget(today_latency.map(|l| l.median_ms));
    }
    // Flat, not scaled by id count (air-8lj8): this reconcile's calls run in sequence under
    // the 20 s MCP tool limit, and a per-id allowance on the `show_all` below would let one
    // call alone pass it. A timeout here falls back to the cache, so it costs freshness, not a
    // refused command.
    bd.per_id = std::time::Duration::ZERO;
    // `bd_try` skips every later call once this is set, and each call site already falls back
    // to `bd_cache`. Setting it up front is how "do not call bd this tick" is expressed: one
    // decision, no second code path to keep in step with the first.
    let mut bd_slow: Option<String> = None;
    let mut bd_skipped = false;
    if let BdUse::CachedFor(mins) = bd_use
        && cache_is_fresh(&ledger, &at, mins)
    {
        bd_slow = Some(format!(
            "bd not called: cached counts are under {mins} min old"
        ));
        bd_skipped = true;
    }
    let in_progress_issues: Option<Vec<air_bd::Issue>> = bd_try(
        &bd,
        &mut bd_slow,
        &mut errors,
        "in_progress (claims not reconciled)",
        air_bd::WorkLedger::in_progress,
    );
    let in_progress: Option<std::collections::BTreeSet<String>> = in_progress_issues
        .as_ref()
        .map(|v| v.iter().map(|i| i.id.clone()).collect());
    let mut reconciled = 0usize;
    // air-x1ha: claims kept because bd could not resolve the id at all, which is the opposite
    // of a bead bd no longer holds. Named rather than counted, and the line says what was
    // looked up, because the id itself is the defect a reader has to see.
    let mut unresolved: Vec<String> = Vec::new();
    let open_claims = ledger.open_claims().map_err(|e| e.to_string())?;
    // Every claim bd no longer holds in_progress, looked up in ONE `bd show a b c` rather
    // than one process per bead (air-bp0): the cost is per process, ~2 s to open the store,
    // and the query is close to free, so K claims cost K × 2 s before and 2 s now. bd omits
    // an id it does not know and exits 0, so an id missing from the answer reads as
    // "unknown", exactly what a single `show` answered with `None`.
    let missing: Vec<String> = match &in_progress {
        Some(ip) => open_claims
            .iter()
            .filter(|c| !ip.contains(&c.bead))
            .map(|c| c.bead.clone())
            .collect(),
        None => Vec::new(),
    };
    let shown: Option<Vec<air_bd::Issue>> = if missing.is_empty() {
        None
    } else {
        bd_try(&bd, &mut bd_slow, &mut errors, "show", |b| {
            air_bd::WorkLedger::show_all(b, &missing)
        })
    };
    for c in open_claims {
        let mut handed_over = false;
        if let Some(ip) = &in_progress
            && !ip.contains(&c.bead)
        {
            let status: Option<Option<air_bd::Issue>> = shown
                .as_ref()
                .map(|v| v.iter().find(|i| i.id == c.bead).cloned());
            // air-3eu: `awaiting_review` is not the end of a claim. A handed-over bead is
            // still the worker's until it lands, and the row carries the declared files the
            // coordinator needs for overlap. Releasing it left a worker "not claimed" while
            // still editing, after a stray flip to awaiting_review and back (tty-fix
            // 2026-08-22 06:02, capture 01M0M114Q9XQYR3Z66KFSCDXQA). Mark it instead; when bd
            // says in_progress again the row is untouched, original time and all.
            if matches!(&status, Some(Some(i)) if i.status == "awaiting_review") {
                let _ = ledger.mark_handed_over(&c.bead, &c.worker, &at);
                handed_over = true;
            } else {
                // air-x1ha: releasing needs bd to have SAID something about the id. Three
                // cases used to collapse into one, and two of them mean the opposite of the
                // third.
                //
                // `Some(Some(i))` — bd knows it and no longer holds it in progress: release,
                // which is what this reconcile is for.
                //
                // `Some(None)` — bd omits an id it does not know and still exits 0, so this
                // is "bd never had this id". A worker typed a prefix, bd claimed the full id,
                // Air's row went under the prefix, and this released it while the work
                // continued. The row is kept and named instead.
                //
                // `None` — the `show` did not answer at all. That is a failed lookup, not a
                // fact about the bead, and reading it as one released every open claim whose
                // bead was not in the in-progress list because one bd call timed out.
                match &status {
                    Some(Some(i)) => {
                        let reason = if i.status == "closed" {
                            "closed"
                        } else {
                            "reconciled"
                        };
                        let _ = ledger.release_claim(&c.bead, &c.worker, reason, &at);
                        reconciled = reconciled.saturating_add(1);
                        continue;
                    }
                    Some(None) => unresolved.push(format!("{} ({})", c.bead, c.worker)),
                    // Already reported by `bd_try`; the row simply stays.
                    None => {}
                }
            }
        }
        let v = views.entry(c.worker.clone()).or_insert_with(|| WorkerView {
            worker: c.worker.clone(),
            role: super::hook::role_for(&c.worker).to_string(),
            ..Default::default()
        });
        // Held either way, but kept apart so the attention conditions mean what they meant
        // when the reconcile released these rows: a worker waiting on review is not idle with
        // a claim, gone with a claim, or handing over red.
        if handed_over {
            v.handed_over.push(c);
        } else {
            v.claims.push(c);
        }
    }

    // Which lanes have a pane the owner can attach to (air-5lg). One `tmux ls`; absent tmux
    // and no running server both read as "none", which is what an empty list means anyway.
    {
        let live = super::tmux::sessions();
        let project = super::tmux::project_prefix(repo);
        for (name, v) in &mut views {
            let want = super::tmux::session_name(&project, name);
            v.tmux_session = live.iter().find(|s| **s == want).cloned();
        }
    }

    // Files held and overlaps (derived; may be slow-ish, CLI only).
    let mut overlaps = BTreeMap::new();
    match holdings::compute(repo, None) {
        Ok(rep) => {
            for (file, holders) in rep.files {
                for h in &holders {
                    if let Some(v) = views.get_mut(&h.worker) {
                        v.files_held = v.files_held.saturating_add(1);
                    }
                }
                // air-pwvk: only the holders that can collide. `files_held` above still
                // counts every one, because "how many files is this worker in" is a different
                // question from "who could collide here".
                let live = colliding(&holders);
                if live.len() > 1 {
                    // air-v7o: the same tags `air holdings` prints, so the two cannot
                    // diverge and the coordinator reads a tense here too. Every holder that
                    // survives `colliding` carries `uncommitted now` or `committed` or both,
                    // so the line says which of the two each one is without a new string.
                    overlaps.insert(
                        file,
                        live.iter()
                            .map(|h| format!("{}[{}]", h.worker, holdings::tags(h, &rep.at)))
                            .collect(),
                    );
                }
            }
            errors.extend(rep.errors);
        }
        Err(e) => errors.push(format!("holdings: {e}")),
    }

    // Review queue from bd. Absent bd is reported, not fatal; slow bd answers from the cache.
    if reconciled > 0 {
        errors.push(format!(
            "reconciled {reconciled} claim(s) whose bead bd no longer holds in_progress"
        ));
    }
    // air-x1ha, and the standing requirement that a line reporting nothing found says what it
    // looked FOR: the ids, not a count, because the id is the defect.
    if !unresolved.is_empty() {
        errors.push(format!(
            "kept {} claim(s) bd could not resolve: `bd show` returned nothing for {}. \
             An id bd never had is not an id bd no longer holds, so the row stays. Usually a \
             claim recorded under a typed PREFIX before air-x1ha, while bd holds the full id: \
             re-claim under the id bd knows, or `air release <id> --reason unknown`",
            unresolved.len(),
            unresolved.join(", ")
        ));
    }
    // A healthy answer also refreshes `.air/ready.json` for the Stop hook (air-09i); a slow
    // bd leaves that file as it was.
    // Beads bd shows back in the work queue. Used to clear a `landed-not-closed` report once
    // somebody reopened the bead (air-dlw); no extra bd call, these lists are already here.
    let mut back_in_queue: std::collections::BTreeSet<String> =
        in_progress.iter().flatten().cloned().collect();
    // air-uir: both counts come off the SAME `bd ready` answer and the same `claimable`
    // filter the Stop nudge uses, so the two can never disagree about one tick's beads.
    let mut claimable_depth: Option<usize> = None;
    let mut epic_depth: Option<usize> = None;
    // air-84u: which of those epics has nothing open under it. One `bd list --parent` per
    // READY epic, so the cost is proportional to the condition it reports: with no ready epic
    // — the normal state — it is zero processes, and it is never paid to print nothing. A
    // slow bd skips it through `bd_try` and the line is silent rather than stale, which is
    // right for an invitation nobody is refused for ignoring.
    let mut epics_to_decompose: Option<Vec<EpicToDecompose>> = None;
    // air-btz: a bead blocked by its own ancestor waits forever and reads as ordinary
    // queueing. It is NOT in the ready set by definition, and `Issue` carries no edges, so it
    // cannot be derived from what is already fetched — the bead said otherwise and the bead
    // was wrong (the coordinator, 2026-09-06, on both this and air-84u). Two calls, each
    // gated so the common repo pays one: the unfinished set, and the edges of the parented
    // beads that bd says have any.
    let mut ancestor_deadlocks: Option<Vec<AncestorDeadlock>> = None;
    // air-g5o: counted over the answers bd has ALREADY given this tick (ready plus
    // in-progress), so the number costs no extra bd process. `None` until `ready` answers, and
    // the in-progress half joins it only if that answered too, so a partial tick reports
    // nothing rather than a denominator that quietly shrank.
    let mut without_initiative: Option<(usize, usize)> = None;
    let ready_depth: Option<usize> = match bd_try(&bd, &mut bd_slow, &mut errors, "ready", |b| {
        air_bd::WorkLedger::ready(b)
    }) {
        Some(v) => {
            let mut pool: Vec<air_bd::Issue> = v.clone();
            if let Some(ip) = &in_progress_issues {
                pool.extend(ip.iter().cloned());
            }
            without_initiative = Some(super::metis::without_initiative(&pool));
            back_in_queue.extend(v.iter().map(|i| i.id.clone()));
            // air-f10: one partition of one answer; the counts are its lengths.
            let split = super::ready_cache::split(&v);
            let ids = split.claimable;
            claimable_depth = Some(ids.len());
            epic_depth = Some(split.epics.len());
            // air-84u. `bd ready` lists an epic whatever is under it — children do not block
            // their parent — so the ready set alone cannot answer this and the children have
            // to be asked for. Checked against this repo on 2026-09-06: air-80x is ready with
            // all six children closed.
            epics_to_decompose = Some(
                split
                    .epics
                    .iter()
                    .filter_map(|e| {
                        let kids = bd_try(&bd, &mut bd_slow, &mut errors, "children", |b| {
                            air_bd::WorkLedger::children(b, e)
                        })?;
                        to_decompose(e, &kids)
                    })
                    .collect(),
            );
            // air-btz, on the same branch as the ready answer so a slow bd skips both.
            ancestor_deadlocks = deadlock_scan(&bd, &mut bd_slow, &mut errors);
            let _ = ledger.bd_cache_put("claimable_depth", &ids.len().to_string(), &at);
            let _ = ledger.bd_cache_put("epic_depth", &split.epics.len().to_string(), &at);
            super::ready_cache::write_with(
                repo,
                &ids,
                &super::ready_cache::priorities_of(&v, &ids),
                &super::now(),
            );
            let _ = ledger.bd_cache_put("ready_depth", &v.len().to_string(), &at);
            Some(v.len())
        }
        None if bd_slow.is_some() => {
            // Both degrade together, or `idle-without-claim` would compare today's absence
            // against yesterday's count.
            claimable_depth = ledger
                .bd_cache_get("claimable_depth")
                .ok()
                .flatten()
                .and_then(|(v, _)| v.parse().ok());
            epic_depth = ledger
                .bd_cache_get("epic_depth")
                .ok()
                .flatten()
                .and_then(|(v, _)| v.parse().ok());
            ledger
                .bd_cache_get("ready_depth")
                .ok()
                .flatten()
                .and_then(|(v, _)| v.parse().ok())
        }
        None => None,
    };
    // A bd that did not answer is a fault and is reported. A bd we chose not to call is not,
    // so it is named in `bd_source` instead of in `errors`: an alarm that fires on correct
    // behaviour is the shape plan 0008 §3 is entirely about.
    if let Some(slow) = &bd_slow
        && !bd_skipped
    {
        let seen = ledger
            .bd_cache_get("ready_depth")
            .ok()
            .flatten()
            .map(|(_, t)| t)
            .unwrap_or_else(|| "never".into());
        errors.push(format!(
            "{slow}; ready/review counts stale (last seen {seen})"
        ));
    }
    let inbox = ledger.inbox().map_err(|e| e.to_string())?;
    let stale = std::env::var("AIR_LEASE_STALE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600);
    let leases: Vec<(Lease, Option<String>)> = ledger
        .leases()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|l| {
            let d = super::lease::defect(&l, &at, stale);
            (l, d)
        })
        .collect();
    // Only for a lease that has a defect: a healthy lease raises nothing, so its waiters are
    // not an audience yet and there is no reason to read them (air-q9c).
    let lease_wants: BTreeMap<String, Vec<String>> = leases
        .iter()
        .filter(|(_, d)| d.is_some())
        .filter_map(|(l, _)| {
            let who: Vec<String> = ledger
                .lease_wants(&l.resource)
                .ok()?
                .into_iter()
                .map(|(worker, _, _)| worker)
                .collect();
            (!who.is_empty()).then(|| (l.resource.clone(), who))
        })
        .collect();
    // air-80x.3: the verify lane's one fact, from git and the ledger only.
    let batch = batch_ready_for(&ledger, repo);
    errors.extend(batch.2.iter().cloned());
    // air-72t7: ONE selection, and all three of its fields reach the snapshot. Dropping
    // `skipped` and `errors` here is what made `air status --json` say which branches can land
    // and never why the others cannot, and made an error `select` deliberately raises reach no
    // caller at all.
    let selection = select(repo);
    let session_pids: std::collections::BTreeSet<i64> =
        all_sessions.iter().filter_map(|(_, _, x)| x.pid).collect();
    let tree_readers = super::readers::gather(
        readers_job
            .join()
            .unwrap_or_else(|_| Err("the process lookup panicked".into())),
        &trees,
        &session_pids,
    );
    // air-jc2p.3: a launched session whose process runs in the main checkout. Live rows only.
    let live: Vec<(String, String, i64)> = all_sessions
        .iter()
        .filter(|(_, _, x)| x.pid_alive != Some(false))
        .filter_map(|(w, role, x)| x.pid.map(|p| (w.clone(), role.clone(), p)))
        .collect();
    let main_checkout_sessions = super::readers::main_checkout_sessions(&tree_readers, &live);
    let loops = super::loops::measure(&ledger, &day_before(&at));
    let bd_main = super::worktree::main_checkout(repo);
    let bd_mode = super::bd_server::mode(&bd_main);
    let bd_listener = super::bd_server::view(&bd_main, &bd_mode);
    Ok(Snapshot {
        main_checkout_sessions,
        at,
        workers: views.into_values().collect(),
        inbox_depth: inbox.len(),
        oldest_capture_at: inbox.first().map(|c| c.captured_at.clone()),
        leases,
        lease_wants,
        // A ledger read, so it survives an absent bd (air-ayp). The ledger already drops a
        // bead whose claim was released as closed (0.4.10 trial); a bead bd shows back in the
        // work queue has been dealt with too: somebody reopened it.
        landed_open: ledger
            .landed_open()
            .unwrap_or_default()
            .into_iter()
            .filter(|o| !back_in_queue.contains(&o.bead))
            .collect(),
        sessions: all_sessions,
        ready_depth,
        claimable_depth,
        epic_depth,
        without_initiative,
        // air-4cr. Reading is also the pruning: a crashed `air record` leaves a row and the
        // next status clears it, so no expiry window has to be chosen or tuned.
        verifies_in_flight: verifies_in_flight(&ledger),
        landings_in_flight: landings_in_flight(&ledger),
        tree_readers,
        red_batch: super::batch::red_batch_standing(&ledger, repo),
        install_lag: super::install::lag(ledger.dir()),
        fleet_stop: ledger.fleet_stop().ok().flatten(),
        bd_mode: Some(bd_mode.clone()),
        bd_server_up: super::bd_server::is_up(&bd_listener),
        bd_listener,
        pin: super::install::pin_state(ledger.dir()),
        // air-i6fd, third site: the parameter is named `main_head` and was fed the running
        // cwd's HEAD. From a worktree that asked "is this rewound landing back in main?" of
        // the worktree's own branch, and the answer suppresses the warning — so a worker whose
        // branch contained the merge saw nothing about work main had lost.
        rewound_carried: rewound_carried(repo, &ledger, git::main_tip(repo).ok().as_deref()),
        // air-03w: the same selection `air land --all` runs, so the condition cannot claim a
        // branch is landable that the command would then skip. air-72t7: and its WHOLE answer,
        // so a reader can see why the others cannot.
        closed_not_landed: closed_not_landed(&ledger, &selection.landings),
        landable: selection.landings,
        land_skipped: selection.skipped,
        land_errors: selection.errors,
        epics_to_decompose,
        ancestor_deadlocks,
        batch_ready: batch.0,
        not_batch_ready: batch.1,
        loops,
        overlaps,
        errors,
        duration_ms: u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX),
        // From the lines already on disk: this tick's own bd cost is logged after gather,
        // so it lands in the next reading. Read once, above, because the bd budget is
        // derived from it (air-p61).
        bd_latency: today_latency,
        bd_source: match (bd_skipped, bd_slow.is_some()) {
            (true, _) => "cache",
            (false, true) => "stale",
            (false, false) => "live",
        },
    })
}

/// Beads bd has closed whose commits are in no tree but their author's worktree (air-gazh).
///
/// **This is a join, not a scan.** `landable` already names every bead on a worktree branch that
/// is not yet in main — `select` drops a branch already in main through its one silent exit, so
/// a landed bead cannot appear here at all — and the ledger already records `closed` as a
/// release reason when the reconcile saw bd say so. Both halves existed and nothing joined them.
/// A second git walk was the obvious shape and would have been a second implementation of
/// `select`'s predicate, which is what air-y3v was.
///
/// The closed set is read once and used as a set, so this is O(landable) with no per-bead
/// lookup: the beads of unlanded branches are few and the closed rows are one query.
fn closed_not_landed(ledger: &Ledger, landable: &[Landing]) -> Vec<ClosedNotLanded> {
    let closed: std::collections::BTreeSet<String> = match ledger.closed_claims() {
        Ok(v) => v.into_iter().map(|c| c.bead).collect(),
        // A failed lookup is not a fact about any bead (air-x1ha). Reporting nothing here is
        // reporting "nothing found", which is why the error path stays empty rather than
        // guessing: `gather` already surfaces ledger failures on its own errors list.
        Err(_) => return Vec::new(),
    };
    let mut out: Vec<ClosedNotLanded> = Vec::new();
    for l in landable {
        // A journal-only branch carries `None` and closes no bead (air-kexg).
        let Some(bead) = &l.bead else { continue };
        if !closed.contains(bead) {
            continue; // open on an unlanded branch is the normal state, and says nothing
        }
        out.push(ClosedNotLanded {
            bead: bead.clone(),
            worker: l.worker.clone(),
            head: l.head.clone(),
            blocked: l.blocked.clone(),
        });
    }
    out.sort_by(|a, b| (&a.worker, &a.bead).cmp(&(&b.worker, &b.bead)));
    out
}

/// The ancestor-deadlock scan (air-btz), and what it costs, gated so the common repo pays one
/// bd call and a repo with no hierarchy at all pays one.
///
/// Call 1, always: every bead that is not finished, for its `parent`. A repo whose beads have
/// no parents stops here — there is no hierarchy, so there is no ancestor to be blocked by.
/// Call 2, only when bd's own `dependency_count` says at least one PARENTED bead has an edge
/// at all: those beads' edges, batched into one process. So the second call is paid only by a
/// repo that has both hierarchy and edges under it, which is the only repo that can have the
/// shape.
///
/// It is not derivable from what `air status` already fetches, and the bead that asked for it
/// said otherwise: a bead blocked by its ancestor is not in the ready set — that is the whole
/// problem — and `air_bd::Issue` carries no edges. Measured here 2026-09-06 under load 67:
/// call 1 is 2040 ms and call 2 does not run, because none of this repo's 11 unfinished beads
/// has a parent.
fn deadlock_scan(
    bd: &air_bd::BdCli,
    slow: &mut Option<String>,
    errors: &mut Vec<String>,
) -> Option<Vec<AncestorDeadlock>> {
    // Every stored status bd documents except `closed`. Comma-separated in ONE argument: bd
    // 1.2.2's own `--help` says a repeated `-s` silently overwrites, so the repeated form
    // would ask for `deferred` alone and answer with complete confidence.
    let unfinished = bd_try(bd, slow, errors, "unfinished", |b| {
        air_bd::WorkLedger::by_statuses(b, &["open", "in_progress", "blocked", "deferred"])
    })?;
    let parents: std::collections::BTreeMap<String, String> = unfinished
        .iter()
        .filter_map(|i| i.parent.clone().map(|p| (i.id.clone(), p)))
        .collect();
    let with_edges: Vec<String> = unfinished
        .iter()
        .filter(|i| i.parent.is_some() && i.dependency_count > 0)
        .map(|i| i.id.clone())
        .collect();
    if with_edges.is_empty() {
        return Some(Vec::new());
    }
    let edges = bd_try(bd, slow, errors, "dep-list", |b| {
        air_bd::WorkLedger::dep_list(b, &with_edges)
    })?;
    Some(ancestor_deadlocks(&parents, &edges))
}

/// One bd call under the status budget. After a timeout every later call is skipped (`slow`
/// set once, with the budget); other failures are recorded and the call answers None.
fn bd_try<T>(
    bd: &air_bd::BdCli,
    slow: &mut Option<String>,
    errors: &mut Vec<String>,
    what: &str,
    f: impl FnOnce(&air_bd::BdCli) -> air_bd::Result<T>,
) -> Option<T> {
    if slow.is_some() {
        return None;
    }
    match f(bd) {
        Ok(v) => Some(v),
        Err(air_bd::BdError::Timeout(d)) => {
            *slow = Some(format!("bd did not answer in {} s", d.as_secs_f64()));
            None
        }
        Err(e) => {
            errors.push(format!("bd {what}: {e}"));
            None
        }
    }
}

/// What has to change before the event log says the condition set again: the set itself,
/// each entry with its own value. Age is deliberately not in it — keying on "oldest 40 min"
/// then "oldest 50 min" rebuilds the repeat under a new name (air-s7c).
pub fn conditions_fingerprint(att: &[Attention]) -> String {
    let mut parts: Vec<String> = att
        .iter()
        .map(|a| format!("{}:{}:{}", a.kind, a.worker, a.fingerprint))
        .collect();
    parts.sort();
    parts.join("|")
}

/// Conditions as rows (first-seen/cleared) and one event line that names every kind and
/// worker, plus the queue depth (plan 0006 C1, C6). Shared by the CLI and the channel poll.
///
/// The poll path writes that line **on change only** (air-5uz). It evaluates every few
/// seconds, so on 2026-08-25 it wrote 7,667 of the day's 8,242 event lines, and `air audit`
/// read the total as firings: `owner-decision-waiting` showed 1,685 against one push all day,
/// and a deletion was nearly proposed on that number. Nothing is lost by the silence — the
/// `conditions` table already carries first-seen, last-seen and cleared for every condition,
/// which is where a duration query belongs. The gate is `hook_emissions`, the same one the
/// Stop and peer hooks use for "say it once".
///
/// A person running `air status` still gets one line per invocation: that path is one line a
/// day, not 1,728, and an invocation is itself the fact being recorded.
/// The line `air status` writes: which command, and whether anything held.
fn status_trace(attention_only: bool, quiet: bool) -> super::decisions::Trace {
    use super::decisions::{
        STATUS_ATTENTION, STATUS_ATTENTION_ATTENTION, STATUS_ATTENTION_QUIET, STATUS_QUIET,
    };
    match (attention_only, quiet) {
        (true, true) => STATUS_ATTENTION_QUIET,
        (true, false) => STATUS_ATTENTION_ATTENTION,
        (false, true) => STATUS_QUIET,
        (false, false) => STATUS_ATTENTION,
    }
}

pub fn record_and_log(
    ledger: &air_ledger::Ledger,
    worker: &str,
    snap: &Snapshot,
    att: &[Attention],
    attention_only: bool,
) {
    let current: Vec<(String, &str, String)> = att
        .iter()
        .map(|a| (a.worker.clone(), a.kind, a.detail.clone()))
        .collect();
    let (opened, cleared) = ledger
        .record_conditions(&current, &snap.at)
        .unwrap_or((0, 0));
    let kinds: Vec<String> = att
        .iter()
        .map(|a| format!("{}:{}", a.kind, a.worker))
        .collect();
    // An unchanged set has nothing left to say. An empty fingerprint (no conditions) clears
    // the row, so the next occurrence speaks again.
    if attention_only
        && !ledger
            .emit_if_changed(
                worker,
                "status.conditions",
                &conditions_fingerprint(att),
                &snap.at,
            )
            .unwrap_or(true)
    {
        return;
    }
    log_event(
        ledger,
        worker,
        status_trace(attention_only, att.is_empty()),
        &serde_json::json!({"conditions": kinds, "opened": opened, "cleared": cleared, "ready_depth": snap.ready_depth, "inbox": snap.inbox_depth, "duration_ms": snap.duration_ms}),
        &if att.is_empty() {
            "no conditions".to_string()
        } else {
            kinds.join(", ")
        },
        &format!(
            "{} workers, {} overlaps, ready {}",
            snap.workers.len(),
            snap.overlaps.len(),
            snap.ready_depth
                .map(|n| n.to_string())
                .unwrap_or_else(|| "?".into())
        ),
    );
}

/// The text form of a snapshot with no attention conditions, so `air selftest` can prove the
/// facts survive their pushes being deleted (air-s7c).
pub fn render_for_probe(s: &Snapshot) -> String {
    render(s, &[])
}

/// How a stopped session reads on its `air status` line (air-1n3).
///
/// The distinction the phrase has to carry is not "stopped or not" but "coming back or not",
/// because they call for opposite actions: a session the harness is auto-resuming must be left
/// alone (typing at it cancels the recovery), and one it is not needs a wake. `kind` is the
/// harness's own declared notification type, never read out of the message text.
pub fn stopped_phrase(kind: &str, at: &str) -> String {
    match kind {
        "quota_auto_resume_fired" => {
            format!("STOPPED at {at} (limit; the harness is resuming it, leave it alone)")
        }
        "quota_auto_resume_stale" | "quota_auto_resume_disabled" => {
            format!("STOPPED at {at} ({kind}; the harness is NOT resuming it)")
        }
        "stop_failure" => format!("STOPPED at {at} (turn ended on an API error)"),
        other => format!("STOPPED at {at} ({other})"),
    }
}

fn render(s: &Snapshot, att: &[Attention]) -> String {
    let mut out = String::new();
    // First, because it changes what every other line means (air-1vri.1).
    if let Some(f) = &s.fleet_stop {
        out.push_str(&format!(
            "FLEET STOPPED: {}; `air fleet resume` ends it\n",
            f.line()
        ));
    }
    if let Some(m) = &s.bd_mode {
        out.push_str(&format!("{}\n", super::bd_server::line(m, &s.bd_listener)));
    }
    for w in &s.workers {
        let sess = w
            .session
            .as_ref()
            // air-air: the model rides on the session line, so "which model is this worker on"
            // is a glance rather than a question put to the worker. A session whose transcript
            // has not yet named one says `model ?` — an honest unknown, never a default filled
            // in from settings.
            .map(|x| {
                let model = if x.model.is_empty() {
                    "?".to_string()
                } else {
                    x.model.clone()
                };
                // air-9dg: a worker whose hooks do not see AIR_ENFORCE=1 has the one refusal
                // switched off, and until this line nothing said so. Only `Some(false)` on a
                // worker speaks: a pre-v15 row is unknown and the coordinator never enforces.
                let unenforced = if super::is_worker_like(&w.role) && x.enforce == Some(false) {
                    " UNENFORCED (hooks do not see AIR_ENFORCE=1; relaunch via air worker)"
                } else {
                    ""
                };
                // air-1n3: a session the harness stopped, and whether the harness is
                // bringing it back. Before this the ledger could only say "silent", and on
                // 2026-09-06 that cost a lane 79 minutes because silent-and-recovering and
                // silent-and-dead read identically.
                // air-3jv5: a row no session is behind. A hook demonstrated by piping a
                // synthetic event at the real ledger wrote a worker a reader could not tell
                // from a live one, in the table `air status`, the attention conditions and the
                // stopped-session line all read.
                let synthetic = if x.has_transcript {
                    ""
                } else {
                    " NO TRANSCRIPT (no session is behind this row: a synthetic hook, or a \
                     harness that sent none)"
                };
                let stopped = x
                    .stopped
                    .as_ref()
                    .map(|(at, kind, _)| format!(" {}", stopped_phrase(kind, at)))
                    .unwrap_or_default();
                format!(
                    "{} since {} [{model}]{unenforced}{synthetic}{stopped}",
                    x.state, x.changed_at
                )
            })
            .unwrap_or_else(|| "no session".to_string());
        let green = match (w.green_at_head, w.green_detail.as_deref()) {
            (Some(true), None) => "green".to_string(),
            (Some(true), Some(d)) => format!("green ({d})"),
            (Some(false), None) => "not green".to_string(),
            (Some(false), Some(d)) => format!("not green ({d})"),
            (None, _) => "unknown".to_string(),
        };
        // air-5ik: a red is the one run somebody reads, so say where it is rather than leaving
        // the reader to know the layout.
        let green = match w.red_log.as_deref() {
            Some(p) => format!("{green} (output: {p})"),
            None => green,
        };
        let claims: Vec<String> = w
            .claims
            .iter()
            .map(|c| format!("{} (handovers {})", c.bead, c.handover_attempts))
            .chain(
                w.handed_over
                    .iter()
                    .map(|c| format!("{} (awaiting review)", c.bead)),
            )
            .collect();
        // A bead waiting on review is not work in progress: the worker is still idle and
        // should be nudged, which is what `idle-without-claim` also decides (air-3eu).
        // A run in flight is progress, whatever its kind: an adopter's coordinator nudged a
        // worker as idle 400 s into a precheck Air could not see (precheck, 2026-09-25).
        let idle_no_claim = w.role == "worker"
            && w.claims.is_empty()
            && w.session.as_ref().is_some_and(|x| x.state == "idle")
            && !verify_running(s, &w.worker);
        out.push_str(&format!(
            "{:<12} {:<11} {}  head {} {}  files {}  claims: {}{}{}\n",
            w.worker,
            w.role,
            sess,
            w.head
                .as_deref()
                .map(|h| h.get(..8).unwrap_or(h))
                .unwrap_or("-"),
            green,
            w.files_held,
            if claims.is_empty() {
                "-".to_string()
            } else {
                claims.join(", ")
            },
            if let Some(t) = &w.tmux_session {
                format!("  tmux {t}")
            } else {
                String::new()
            },
            if idle_no_claim {
                "  idle, no claim"
            } else {
                ""
            }
        ));
    }
    // Silent when nothing is running: a coordinator who lands into an empty screen is right
    // to. Present, it is the one thing that makes landing now cost someone 420 s (air-4cr).
    for f in &s.verifies_in_flight {
        out.push_str(&format!(
            "{} in flight: {}\n",
            f.kind.as_str(),
            in_flight_line(f, &s.at)
        ));
    }
    // Silent when no tree has anything but sessions in it; `--json` `tree_readers.examined`
    // says how many processes were looked at, so none is not "not looked". An unknown lookup
    // always prints, with why.
    for l in super::readers::status_lines(&s.tree_readers) {
        out.push_str(&format!("{l}\n"));
    }
    for l in &s.main_checkout_sessions {
        out.push_str(&format!("{l}\n"));
    }
    // air-80x.4: a red batch stays on the screen until a newer batch supersedes it.
    if let Some(b) = &s.red_batch {
        out.push_str(&format!("{}\n", super::batch::red_batch_line(b)));
    }
    // air-d61: one line while the record lags; gone the moment `air install --write` runs.
    if let Some(l) = &s.install_lag {
        out.push_str(&format!("{}\n", super::install::lag_line(l)));
    }
    for l in s.pin.iter().flat_map(super::install::pin_lines) {
        out.push_str(&format!("{l}\n"));
    }
    // air-bxe: the merge commit exists for minutes before the verify decides whether it stays.
    for f in &s.landings_in_flight {
        out.push_str(&format!(
            "landing in flight: {}\n",
            landing_in_flight_line(f, &s.at)
        ));
    }
    // air-y3v: work that is waiting but not yet landable. Shown, because the coordinator needs
    // to know it exists — and never with `air land` beside it, because that is the command
    // that cost the owner three cycles in an hour. `command` is what actually unblocks it.
    for l in s.landable.iter().filter(|l| l.blocked.is_some()) {
        out.push_str(&format!(
            "waiting, not landable: {} ({}) at {} — {}; `{}`\n",
            l.worker,
            l.bead.as_deref().unwrap_or("no bead"),
            l.head.get(..8).unwrap_or(&l.head),
            l.blocked
                .as_deref()
                .unwrap_or("")
                .trim_start_matches("refused: "),
            l.command
        ));
    }
    // air-gazh: printed as well as pushed. The condition is the part that matters — the
    // reporting incident is a coordinator with the fleet view not looking — but a reader who
    // runs `air status` after the push has gone quiet still needs to see the state.
    for c in &s.closed_not_landed {
        out.push_str(&format!(
            "closed, not landed: {} ({}) at {} — {}\n",
            c.worker,
            c.bead,
            c.head.get(..8).unwrap_or(&c.head),
            c.blocked
                .as_deref()
                .unwrap_or("could land as it stands")
                .trim_start_matches("refused: "),
        ));
    }
    // air-80x.3: what the verify lane merges next. A fact, printed where the lane reads it;
    // the reasons a branch is absent are in `--json` (`not_batch_ready`).
    for b in &s.batch_ready {
        out.push_str(&format!(
            "batch-ready: {} at {} ({})\n",
            b.worker,
            b.head.get(..8).unwrap_or(&b.head),
            b.beads.join(" ")
        ));
    }
    if let Some(l) = super::loops::line(&s.loops) {
        out.push_str(&format!("{l}\n"));
    }
    // air-ob0: a rewind un-lands from main and cannot un-merge from whoever took it.
    for r in &s.rewound_carried {
        for line in rewind_propagation(&r.merge_commit, &r.carried_by) {
            out.push_str(&format!("rewound and still carried: {line}\n"));
        }
    }
    // air-3jv5: rows no session is behind, counted across EVERY session row rather than only
    // the ones rendered. The per-worker view keeps the latest row per worker, so a synthetic
    // row is invisible there the moment the real session emits a hook — which is how the one
    // that pushed `session_joined` sat in the table unseen. The count is the "say what it
    // looked at" half: it names how many rows were examined, so silence means checked-and-none
    // rather than not-looked.
    let (no_transcript, examined) = (
        s.sessions
            .iter()
            .filter(|(_, _, x)| !x.has_transcript)
            .count(),
        s.sessions.len(),
    );
    if no_transcript > 0 {
        out.push_str(&format!(
            "sessions: {no_transcript} of {examined} row(s) have no transcript behind them \
             (synthetic hooks, or a harness that sent none). They are never announced as a \
             worker joining; `sqlite3 .air/ledger.db \"select session_id, worker from sessions \
             where transcript_path is null or transcript_path = \'\'\"` names them.\n"
        ));
    }
    out.push_str(&format!(
        "ready: {}{}{}\n",
        s.ready_depth
            .map(|n| n.to_string())
            .unwrap_or_else(|| "? (bd did not answer)".into()),
        // air-uir: say WHICH count this is whenever the two differ. A coordinator reading
        // "ready: 1" at round end had to open the bead to find the queue was empty for every
        // worker, which is the same defect one layer up from the condition itself.
        match (s.ready_depth, s.claimable_depth) {
            // air-uef: the owner-labelled count IS the owner's queue, the one number that
            // says what waits on the owner, printed where the coordinator already looks.
            // air-f10: epics are named apart, never folded into "claimable": "2 claimable"
            // read as two workers' worth of work when the true count was zero and both were
            // containers. The split keeps them visible for the coordinator to decompose.
            (Some(r), Some(c)) if r != c => {
                let epics = s.epic_depth.unwrap_or(0);
                let owner = r.saturating_sub(c).saturating_sub(epics);
                let mut parts = vec![format!("{c} claimable")];
                if epics > 0 {
                    // air-3vkg: the COUNT, with no instruction attached. `epic_depth` is
                    // every epic in the ready set, decomposed or not, and this line used to
                    // call all of them "to decompose". An adopter audited all six of theirs
                    // on the strength of that phrase and every one was already at its correct
                    // frontier — including one fully cut with thirteen claimable children.
                    //
                    // The honest number is `epics_to_decompose` (air-84u: no OPEN child), and
                    // it prints two lines below as `epic ready to decompose: <id>`, so the
                    // instruction is carried by the line that can tell. Third instance in one
                    // day of a line whose discriminating fact is on the same struct.
                    //
                    // The count stays on `epic_depth` rather than moving to the honest field,
                    // and the reason is the cache path: `epic_depth` is cached (:2000) and
                    // restored when bd is slow (:2013), while `epics_to_decompose` needs a
                    // `children` call per epic and is `None` there. Re-pointing the count
                    // would make it silent exactly when bd is slow, which is when a
                    // coordinator is most likely reading a cached status. Checked in the
                    // source rather than taken from the bead, which said so as a reading.
                    //
                    // Nothing marks the cache path here because the ready line already
                    // does: `(cached; bd not called this tick)` is appended below, so
                    // "asked, none to decompose" and "not asked" are already distinguishable
                    // by a reader of the same line.
                    parts.push(format!("{epics} epic(s), not claimable"));
                }
                if owner > 0 {
                    parts.push(format!(
                        "{owner} owner-labelled: the owner's queue, which `air claim` \
                         refuses to workers"
                    ));
                }
                format!(" ({})", parts.join("; "))
            }
            _ => String::new(),
        },
        // Which source, always: "0 ready" from a cache and "0 ready" from bd are different
        // facts, and only one of them is today's (air-cmn).
        match s.bd_source {
            "cache" => " (cached; bd not called this tick)",
            "stale" => " (stale; bd did not answer)",
            _ => "",
        }
    ));
    // air-84u: under the count, the epic the count is about. "1 epic to decompose" is a
    // number the coordinator has to go and resolve against bd; this is the action. Silent
    // when there is none, which is the normal state.
    for e in s.epics_to_decompose.iter().flatten() {
        out.push_str(&format!(
            "epic ready to decompose: {} (0 open children, {} closed)\n",
            e.epic, e.closed_children
        ));
    }
    // air-btz: the deadlock nobody can see. Both sides wait forever, and every other reading
    // of it — bd's, the coordinator's, the worker's — is "not ready yet".
    for d in s.ancestor_deadlocks.iter().flatten() {
        let rel = if d.depth == 1 { "parent" } else { "ancestor" };
        out.push_str(&format!(
            "deadlock: {} is blocked by {}, its own {rel}, which cannot finish until {} does. \
             Neither side can move; bd reports this as \"not ready\". Fix: \
             bd dep remove {} {}\n",
            d.bead, d.ancestor, d.bead, d.bead, d.ancestor
        ));
    }
    // air-g5o: a count with its denominator beside it, and no verdict. Printed only when
    // there is something to say — a fleet that declares every initiative should not carry a
    // line saying so on every tick.
    if let Some((missing, considered)) = s.without_initiative
        && missing > 0
    {
        out.push_str(&format!(
            "beads without initiative: {missing} of {considered} bd named this tick (ready + \
             in progress, epics excluded) declare no `initiative: <CODE>` line. A count, not \
             a gate.\n"
        ));
    }
    out.push_str(&format!("inbox: {} open\n", s.inbox_depth));
    if let Some(l) = &s.bd_latency {
        out.push_str(&super::bd_latency::line(l));
    }
    for (l, d) in &s.leases {
        out.push_str(&format!(
            "lease: {} held by {} ({}){}\n",
            l.resource,
            l.worker,
            l.reason,
            d.as_deref().map(|d| format!(" [{d}]")).unwrap_or_default()
        ));
    }
    for (f, who) in &s.overlaps {
        out.push_str(&format!("overlap: {f} held by {}\n", who.join(", ")));
    }
    if att.is_empty() {
        out.push_str("attention: none\n");
    }
    for a in att {
        out.push_str(&format!(
            "attention [{}] {}: {}\n",
            a.kind, a.worker, a.detail
        ));
    }
    for e in &s.errors {
        out.push_str(&format!("error: {e}\n"));
    }
    out
}

pub fn run(repo: &Path, attention_only: bool, json: bool) -> i32 {
    let snap = match gather(repo) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("air status: {e}");
            return 1;
        }
    };
    let att = attention(&snap, &snap.at, Thresholds::from_env());
    if let Ok((ledger, worker)) = open(repo) {
        log_event(
            &ledger,
            &worker,
            status_trace(attention_only, att.is_empty()),
            &serde_json::json!({}),
            &format!("{} condition(s)", att.len()),
            &format!(
                "{} workers, {} overlaps, inbox {}",
                snap.workers.len(),
                snap.overlaps.len(),
                snap.inbox_depth
            ),
        );
        // The main-checkout warning is printed only by the full status, so it is counted
        // there: one line per invocation that printed it (air-hqj8).
        if !attention_only && !snap.main_checkout_sessions.is_empty() {
            log_event(
                &ledger,
                &worker,
                super::decisions::STATUS_MAIN_CHECKOUT_SESSION,
                &serde_json::json!({"sessions": snap.main_checkout_sessions}),
                &snap.main_checkout_sessions.join("; "),
                &format!("{} session(s)", snap.main_checkout_sessions.len()),
            );
        }
    }
    if attention_only {
        emit(json, &att, || {
            att.iter()
                .map(|a| format!("[{}] {}: {}", a.kind, a.worker, a.detail))
                .collect::<Vec<_>>()
                .join("\n")
        });
        return 0;
    }
    // air-dwq5: which binary produced this snapshot. `air status --json` is what a reader
    // reconstructing a round parses, and a snapshot that cannot be attributed to a build is a
    // snapshot whose behaviour cannot be looked up — a whole round of changes shipped under
    // one crate version tonight, because lanes cut no release rows.
    emit(
        json,
        &serde_json::json!({
            "snapshot": snap,
            "attention": att,
            "air": super::install::version_json(),
        }),
        || render(&snap, &att),
    );
    0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// 0.4.9 trial: a head a cut dropped for a conflict was listed batch-ready ten seconds
    /// later and dropped again. Like a red at the head, a recorded drop holds the head out
    /// until it moves; the worker's next commit is a new head and is ready as before.
    #[test]
    fn a_dropped_head_is_not_batch_ready_until_it_moves() {
        let base = BatchFacts {
            worker: "w2".into(),
            head: "2d2ea33dd9cc3571".into(),
            carried: vec!["zz-2".into()],
            held: vec!["zz-2".into()],
            ..Default::default()
        };
        assert!(batch_ready_rule(&base).is_ok());
        let dropped = batch_ready_rule(&BatchFacts {
            dropped_at_head: true,
            ..base.clone()
        })
        .unwrap_err();
        assert_eq!(dropped.check, "dropped-at-head");
        assert!(
            dropped.detail.contains("w2 at 2d2ea33d"),
            "{}",
            dropped.detail
        );
        assert!(
            dropped.detail.contains("after a new commit"),
            "{}",
            dropped.detail
        );
        // The record names the old head; a new head is not it.
        let moved = BatchFacts {
            head: "c413fb4a00000000".into(),
            ..base
        };
        assert!(batch_ready_rule(&moved).is_ok());
    }

    fn claim(bead: &str, worker: &str, at: &str, attempts: i64) -> Claim {
        Claim {
            bead: bead.into(),
            worker: worker.into(),
            claimed_at: at.into(),
            declared_files: vec![],
            first_handover_at: None,
            last_handover_at: Some(at.into()),
            handover_attempts: attempts,
            released_at: None,
            release_reason: None,
        }
    }

    fn worker(
        name: &str,
        state: Option<&str>,
        changed: &str,
        claims: Vec<Claim>,
        green: Option<bool>,
    ) -> WorkerView {
        WorkerView {
            worker: name.into(),
            role: "worker".into(),
            session: state.map(|s| Session {
                session_id: "s".into(),
                state: s.into(),
                detail: Some("Bash".into()),
                changed_at: changed.into(),
                pid: None,
                pid_alive: None,
                project: String::new(),
                model: String::new(),
                enforce: None,
                has_transcript: true,
                stopped: None,
            }),
            head: Some("abc".into()),
            green_at_head: green,
            green_detail: None,
            red_log: None,
            claims,
            handed_over: vec![],
            files_held: 0,
            tmux_session: None,
        }
    }

    /// The one instant these tests hold. Every age is DERIVED from `Thresholds::default()`
    /// (air-an9), never restated beside it: the fixtures used to be literal timestamps chosen
    /// to sit either side of the defaults at the time, so moving a threshold flipped
    /// assertions without the code under test changing. Same shape as the two dated cutoffs
    /// that made main red for six days (air-24e), and the same fix air-jc0 gave the probes.
    const NOW: &str = "2026-08-20T12:00:00Z";

    fn minutes_before(minutes: i64) -> String {
        let t: jiff::Timestamp = NOW.parse().unwrap();
        let span = jiff::Span::new().try_minutes(minutes).unwrap();
        t.checked_sub(span).unwrap().to_string()
    }

    fn every_line() -> [i64; 4] {
        let t = Thresholds::default();
        [
            t.idle_with_claim_min,
            t.silent_with_claim_min,
            t.launch_grace_min,
            t.idle_noclaim_min,
        ]
    }

    /// An age past every threshold, whatever they are.
    fn past_every_line_min() -> i64 {
        every_line()
            .into_iter()
            .max()
            .unwrap_or(0)
            .saturating_add(10)
    }

    /// An age under every threshold, whatever they are.
    fn under_every_line_min() -> i64 {
        every_line()
            .into_iter()
            .min()
            .unwrap_or(0)
            .saturating_sub(1)
    }

    /// A timestamp past every line: what used to be `&past()`.
    fn past() -> String {
        minutes_before(past_every_line_min())
    }

    /// A timestamp under every line: what used to be `&under()`.
    fn under() -> String {
        minutes_before(under_every_line_min())
    }

    #[test]
    fn quiet_fleet_raises_nothing() {
        let s = Snapshot {
            workers: vec![
                worker(
                    "a",
                    Some("working"),
                    &under(),
                    vec![claim("zz-1", "a", &past(), 0)],
                    Some(true),
                ),
                worker("b", Some("idle"), &past(), vec![], Some(true)), // idle without claim is fine
            ],
            ..Default::default()
        };
        assert!(attention(&s, NOW, Thresholds::default()).is_empty());
    }

    #[test]
    fn each_condition_fires_with_its_threshold() {
        let s = Snapshot {
            workers: vec![
                worker(
                    "idle",
                    Some("idle"),
                    &past(),
                    vec![claim("zz-2", "idle", &past(), 0)],
                    None,
                ),
                worker(
                    "silent",
                    Some("running"),
                    &past(),
                    vec![claim("zz-3", "silent", &past(), 0)],
                    None,
                ),
                worker(
                    "gone",
                    None,
                    &past(),
                    vec![claim("zz-4", "gone", &past(), 0)],
                    None,
                ),
                worker(
                    "red",
                    Some("working"),
                    &under(),
                    vec![claim("zz-5", "red", &under(), 2)],
                    Some(false),
                ),
            ],
            inbox_depth: 3,
            oldest_capture_at: Some(past()),
            ..Default::default()
        };
        let att = attention(&s, NOW, Thresholds::default());
        let kinds: Vec<(&str, &str)> = att.iter().map(|a| (a.worker.as_str(), a.kind)).collect();
        assert_eq!(
            kinds,
            vec![
                ("idle", "idle-with-claim"),
                ("silent", "silent-with-claim"),
                ("gone", "gone-with-claim"),
                ("red", "handover-not-green"),
            ]
        );
        assert_eq!(att[0].for_minutes, past_every_line_min());
        // Tighten nothing, loosen everything past the oldest fixture: all time-based ones go
        // quiet. The launch grace stays, so `gone` (a claim older than it, no session) still
        // fires.
        let beyond = past_every_line_min().saturating_add(1);
        let loose = Thresholds {
            idle_with_claim_min: beyond,
            silent_with_claim_min: beyond,
            launch_grace_min: Thresholds::default().launch_grace_min,
            idle_noclaim_min: beyond,
        };
        let att = attention(&s, NOW, loose);
        let kinds: Vec<&str> = att.iter().map(|a| a.kind).collect();
        assert_eq!(kinds, vec!["gone-with-claim", "handover-not-green"]);
    }

    /// air-3eu: the claim on a handed-over bead stays open so the coordinator still sees the
    /// files, but it is not work in progress. None of the with-a-claim conditions may read it
    /// as one, or every worker waiting on review would raise them for the whole round.
    #[test]
    fn a_bead_waiting_on_review_is_not_a_claim_in_progress() {
        let mut w = worker("w", Some("idle"), &past(), vec![], Some(false));
        w.handed_over = vec![claim("zz-1", "w", &past(), 1)];
        let s = Snapshot {
            workers: vec![w.clone()],
            ready_depth: Some(0),
            ..Default::default()
        };
        assert!(attention(&s, NOW, Thresholds::default()).is_empty());
        // Same worker, same bead, but bd still says in_progress: the conditions do fire.
        let mut w2 = w;
        w2.claims = std::mem::take(&mut w2.handed_over);
        let s = Snapshot {
            workers: vec![w2],
            ready_depth: Some(0),
            ..Default::default()
        };
        assert_eq!(
            attention(&s, NOW, Thresholds::default())
                .iter()
                .map(|a| a.kind)
                .collect::<Vec<_>>(),
            vec!["idle-with-claim", "handover-not-green"]
        );
    }

    #[test]
    fn fresh_idle_worker_is_quiet_and_a_stale_claim_is_reported() {
        // Claim 1 minute old, no session row yet: launching, not gone.
        let fresh = Snapshot {
            workers: vec![worker(
                "new",
                None,
                &under(),
                vec![claim("zz-1", "new", &under(), 0)],
                None,
            )],
            ..Default::default()
        };
        assert!(attention(&fresh, NOW, Thresholds::default()).is_empty());
        // Idle at the prompt with a live pid and a claim, 2 min old: quiet.
        let mut idle = worker(
            "w",
            Some("idle"),
            &under(),
            vec![claim("zz-2", "w", &past(), 0)],
            None,
        );
        idle.session.as_mut().unwrap().pid = Some(1);
        idle.session.as_mut().unwrap().pid_alive = Some(true);
        let s = Snapshot {
            workers: vec![idle.clone()],
            ..Default::default()
        };
        assert!(attention(&s, NOW, Thresholds::default()).is_empty());
        // Same row with the pid gone: `gone-with-claim` was deleted on 2026-08-22 (air-s7c)
        // for never having fired in any recorded day, so a dead pid raises nothing on its
        // own. The row now falls through to the ordinary idle states, which are age-gated,
        // so a 2-minute-old one is still quiet.
        idle.session.as_mut().unwrap().pid_alive = Some(false);
        let s = Snapshot {
            workers: vec![idle.clone()],
            ..Default::default()
        };
        assert!(attention(&s, NOW, Thresholds::default()).is_empty());
        // ...and once it is old enough, it is reported as an idle worker holding a claim,
        // which is the condition that does fire.
        idle.session.as_mut().unwrap().changed_at = past();
        let s = Snapshot {
            workers: vec![idle],
            ..Default::default()
        };
        assert_eq!(
            attention(&s, NOW, Thresholds::default())
                .iter()
                .map(|a| a.kind)
                .collect::<Vec<_>>(),
            vec!["idle-with-claim"]
        );
    }

    /// air-okc deleted the `review-waiting` condition that air-e7q added, its `review_waits`
    /// input, the `awaiting_review` bd call that fed it, and the `review: N waiting` line.
    /// It last fired 2026-08-22T19:31 and never again in five recorded days, because air-7o3
    /// replaced hand-over with close-with-proof the same day and the state it reports stopped
    /// existing. What is left is what `air status` still renders around it.
    #[test]
    fn status_renders_ready_depth_and_claims_without_a_review_line() {
        let s = Snapshot {
            workers: vec![worker("w", Some("idle"), &under(), vec![], Some(true))],
            ..Default::default()
        };
        let att = attention(&s, NOW, Thresholds::default());
        let text = render(&s, &att);
        assert!(!text.contains("review:"), "the review line is gone: {text}");
        assert!(text.contains("ready: ? (bd did not answer)"), "{text}");
        assert!(text.contains("claims: -  idle, no claim"), "{text}");
        let none = Snapshot {
            ready_depth: Some(3),
            claimable_depth: Some(3),
            ..Default::default()
        };
        assert!(render(&none, &[]).contains("ready: 3\n"));

        // air-uir: when the two differ the line says which is which, because "ready: 1" at
        // round end sent a coordinator to open the bead to find the queue was empty.
        let split = Snapshot {
            ready_depth: Some(3),
            claimable_depth: Some(1),
            ..Default::default()
        };
        let text = render(&split, &[]);
        assert!(
            text.contains("ready: 3 (1 claimable; 2 owner-labelled"),
            "{text}"
        );
    }

    #[test]
    fn idle_without_claim_fires_only_with_ready_beads_past_the_threshold() {
        let mut s = Snapshot {
            workers: vec![
                worker("w", Some("idle"), &past(), vec![], Some(true)),
                worker("fresh", Some("idle"), &under(), vec![], Some(true)),
                WorkerView {
                    role: "coordinator".into(),
                    ..worker("main", Some("idle"), &past(), vec![], None)
                },
            ],
            ready_depth: Some(5),
            claimable_depth: Some(5),
            ..Default::default()
        };
        let att = attention(&s, NOW, Thresholds::default());
        assert_eq!(att.len(), 1, "{att:?}");
        assert_eq!(
            (att[0].worker.as_str(), att[0].kind),
            ("w", "idle-without-claim")
        );
        assert_eq!(
            att[0].detail,
            format!(
                "idle {} min, 5 bead(s) they can claim",
                past_every_line_min()
            )
        );
        // A line under even the fresh fixture, so both idle workers are past it.
        let t = Thresholds {
            idle_noclaim_min: under_every_line_min().saturating_sub(1),
            ..Thresholds::default()
        };
        assert_eq!(attention(&s, NOW, t).len(), 2);
        s.claimable_depth = Some(0);
        assert!(attention(&s, NOW, Thresholds::default()).is_empty());
        s.claimable_depth = None;
        assert!(attention(&s, NOW, Thresholds::default()).is_empty());

        // air-uir: the raw queue being non-empty is NOT the trigger. Five ready beads, none a
        // worker may claim, is the round-end state that fired this falsely on 2026-08-29 —
        // `air-4t1` was labelled `owner` and gate had already declined it.
        s.ready_depth = Some(5);
        s.claimable_depth = Some(0);
        assert!(
            attention(&s, NOW, Thresholds::default()).is_empty(),
            "a full queue of owner-labelled beads is nothing to prompt anyone about"
        );
    }

    /// air-d10. A dead session is not an idle worker: there is nobody to prompt, and the two
    /// longest-lived rows in this repo's ledger were exactly this, open 4 885 minutes each.
    #[test]
    fn idle_without_claim_is_quiet_when_the_session_process_is_gone() {
        let mk = |alive: Option<bool>| {
            let mut w = worker("w", Some("idle"), &past(), vec![], Some(true));
            let sess = w.session.as_mut().unwrap();
            sess.pid = Some(1);
            sess.pid_alive = alive;
            Snapshot {
                workers: vec![w],
                ready_depth: Some(5),
                claimable_depth: Some(5),
                ..Default::default()
            }
        };
        // Alive, and unknown (no pid exported): both still fire.
        for alive in [Some(true), None] {
            let att = attention(&mk(alive), NOW, Thresholds::default());
            assert_eq!(
                att.iter().map(|a| a.kind).collect::<Vec<_>>(),
                vec!["idle-without-claim"],
                "{alive:?}"
            );
        }
        assert!(attention(&mk(Some(false)), NOW, Thresholds::default()).is_empty());
    }

    #[test]
    fn lease_defects_reach_the_waiter_and_never_the_holder() {
        let lease = |res: &str, beat: &str| Lease {
            resource: res.into(),
            worker: "a".into(),
            session_id: None,
            pid: Some(1),
            pid_started: None,
            reason: "api".into(),
            taken_at: past(),
            heartbeat_at: beat.into(),
        };
        let leases = || {
            vec![
                (lease(":8080", &past()), Some("dead (pid 1 gone)".into())),
                (lease("chrome", &past()), Some("stale (idle 30 min)".into())),
                (lease("runtime", &under()), None),
            ]
        };
        // Nobody waiting: two defects and not a word. `air lease take` takes a defective
        // lease on its own, so a defect with no audience needs no one to act (air-q9c).
        let quiet = Snapshot {
            leases: leases(),
            ..Default::default()
        };
        assert_eq!(attention(&quiet, NOW, Thresholds::default()), vec![]);

        // `b` asked for both and was refused while they were healthy. Now they are not.
        let s = Snapshot {
            leases: leases(),
            lease_wants: [
                (":8080".to_string(), vec!["b".to_string()]),
                ("chrome".to_string(), vec!["b".to_string()]),
                // A healthy lease raises nothing however many are waiting.
                ("runtime".to_string(), vec!["b".to_string()]),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let att = attention(&s, NOW, Thresholds::default());
        assert_eq!(
            att.iter().map(|a| a.kind).collect::<Vec<_>>(),
            vec![kinds::LEASE_HELD_BY_DEAD_SESSION, kinds::LEASE_STALE]
        );
        // Addressed to the waiter, never to the holder, and it names the action as theirs.
        assert_eq!(att[0].worker, "b");
        assert_eq!(att[1].worker, "b");
        assert!(att[0].detail.contains("it is yours to take now"), "{att:?}");
        assert!(att[0].detail.contains("held by a"), "{att:?}");
        assert!(!att[0].detail.contains("lease break"), "{att:?}");

        // The holder waiting on their own lease is not an audience for it.
        let self_only = Snapshot {
            leases: leases(),
            lease_wants: [(":8080".to_string(), vec!["a".to_string()])]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        assert_eq!(attention(&self_only, NOW, Thresholds::default()), vec![]);
    }

    #[test]
    fn unparseable_timestamps_never_panic_or_fire() {
        let s = Snapshot {
            workers: vec![worker(
                "x",
                Some("idle"),
                "garbage",
                vec![claim("zz-9", "x", "garbage", 0)],
                None,
            )],
            ..Default::default()
        };
        assert!(attention(&s, NOW, Thresholds::default()).is_empty());
        assert_eq!(minutes_between("garbage", NOW), None);
        assert_eq!(
            minutes_between(NOW, &past()),
            past_every_line_min().checked_neg()
        );
    }
}
