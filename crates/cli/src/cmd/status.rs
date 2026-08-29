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
    pub const STUCK: &str = "stuck";
    pub const IDLE_WITH_CLAIM: &str = "idle-with-claim";
    pub const IDLE_WITHOUT_CLAIM: &str = "idle-without-claim";
    pub const SILENT_WITH_CLAIM: &str = "silent-with-claim";
    pub const GONE_WITH_CLAIM: &str = "gone-with-claim";
    pub const HANDOVER_NOT_GREEN: &str = "handover-not-green";
    pub const LANDABLE: &str = "landable";
    pub const LANDED_NOT_CLOSED: &str = "landed-not-closed";
    pub const OWNER_DECISION_WAITING: &str = "owner-decision-waiting";
    pub const LEASE_HELD_BY_DEAD_SESSION: &str = "lease-held-by-dead-session";
    pub const LEASE_STALE: &str = "lease-stale";

    /// The whole set, compared against the registry by `air selftest`.
    pub const ALL: &[&str] = &[
        STUCK,
        IDLE_WITH_CLAIM,
        IDLE_WITHOUT_CLAIM,
        SILENT_WITH_CLAIM,
        GONE_WITH_CLAIM,
        HANDOVER_NOT_GREEN,
        LANDABLE,
        LANDED_NOT_CLOSED,
        OWNER_DECISION_WAITING,
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
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct WorkerView {
    pub worker: String,
    pub role: String,
    pub session: Option<Session>,
    pub head: Option<String>,
    pub green_at_head: Option<bool>,
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
    pub owner_queue_depth: usize,
    pub oldest_owner_capture_at: Option<String>,
    /// Every lease, with the defect the CLI found (None = healthy).
    pub leases: Vec<(Lease, Option<String>)>,
    /// Beads a landing merged but did not close, and that nobody has closed since. A ledger
    /// fact, never a bd status (air-ayp).
    pub landed_open: Vec<air_ledger::landings::LandedOpen>,
    /// Every session row (two sessions in one checkout are two entries; the per-worker view
    /// above keeps only the latest): (worker, role, session).
    pub sessions: Vec<(String, String, Session)>,
    /// `bd ready` count at this tick (None when bd did not answer): queue depth over time
    /// (plan 0006 C6; the round ran dry at 4 with only epics left).
    pub ready_depth: Option<usize>,
    /// Verifies running right now, oldest first (air-4cr). A land invalidates every one of
    /// them, so the coordinator needs this before merging and the worker never has to relay it.
    /// Dead pids are pruned by the gather that reads them.
    pub verifies_in_flight: Vec<air_ledger::verify::InFlight>,
    /// Landings that have merged into main and not yet reported an outcome (air-bxe), each
    /// with whether the `air land` process that wrote it is still alive. "Is the land done"
    /// is answered from here, never from a process listing.
    pub landings_in_flight: Vec<LandingInFlight>,
    /// Branches `air land --all` would take right now (air-03w). Filled from the same
    /// `select` the command runs, so the condition and the command cannot disagree. No bd
    /// call: `select` reads git and the ledger only.
    pub landable: Vec<Landing>,
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
    pub stuck_min: i64,
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
            stuck_min: 5,
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
        t.stuck_min = get("AIR_ATTENTION_STUCK_MIN", t.stuck_min);
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
    /// stuck | idle-with-claim | silent-with-claim | handover-not-green |
    /// owner-decision-waiting | lease-held-by-dead-session | lease-stale | review-waiting |
    /// idle-without-claim
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
/// is `air land`: the coordinator's one allowed path onto main.
pub fn land_command(bead: &str) -> String {
    format!("air land {bead}")
}

/// A green hand-over that only the owner can clear (air-6p5). The coordinator may not commit
/// on main and `air land` does not exist, so two green hand-overs waited on 2026-08-22 with
/// nothing saying so; the owner found out by reading a tmux pane (capture
/// 01M0KZETBMSHDXNX6PW15HVSJV). Removal: when `air land` exists and the coordinator may run
/// it, this drops to a count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Landing {
    pub bead: String,
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
}

/// Longest wait first, the order `air land --all` uses, so what `air status` lists is the
/// order it will land in (air-3pz).
fn sort_by_wait(v: &mut [Landing]) {
    v.sort_by(|a, b| b.minutes.cmp(&a.minutes).then_with(|| a.bead.cmp(&b.bead)));
}

/// Pure: the block at the top of `air status`. Empty when nothing waits, so a quiet fleet
/// prints nothing.
///
/// Landings are the coordinator's since air-3pz (`air land --all`), so they are no longer
/// "waiting on owner"; only decisions are. The bead list stays because it is what the
/// coordinator relays when the owner asks what is outstanding (air-6p5).
pub fn waiting_on_owner(s: &Snapshot) -> String {
    let decisions = s.owner_queue_depth;
    if decisions == 0 {
        return String::new();
    }
    let plural = |n: usize, word: &str| {
        if n == 1 {
            format!("{n} {word}")
        } else {
            format!("{n} {word}s")
        }
    };
    let mut out = String::new();
    if decisions > 0 {
        out.push_str(&format!(
            "waiting on owner: {}; `air inbox --owner`\n",
            plural(decisions, "decision")
        ));
    }
    out
}

/// The landings alone, without a full `gather`: bd's `awaiting_review` list, the claim row
/// that names who handed each over, and that worker's green at HEAD. Derived every time, so
/// the owner's feed and `air status` cannot disagree (air-6p5). bd absent or slow means no
/// landings, not an error: `air inbox --owner` still shows the decisions.
/// Which candidate ids this worker is responsible for, from Air's OWN ledger — no bd call.
///
/// **Claimed by this worker, and not already landed.** Both halves come from tables Air
/// writes, so neither moves when git does. There used to be a third: claimed since the branch
/// point. It was the bug that made `air land` unusable in adopter (air-6u5) — **merging main
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
        .filter(|l| l.result == "landed")
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
/// queue was empty. adopter hit it with every precondition verified by hand and fell back to
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
    for (path, _) in worktrees {
        let worker = air_ledger::paths::worker_name_for(&path).unwrap_or_default();
        if super::hook::role_for(&worker) != "worker" {
            continue; // the coordinator's own checkout is not a candidate, and never was
        }
        let head = match git::head(&path) {
            Ok(h) => h,
            Err(e) => {
                out.errors
                    .push(format!("{worker}: git rev-parse HEAD: {e}"));
                continue;
            }
        };
        match ledger.is_green_at(&worker, &head, Kind::Verify) {
            Ok(true) => {}
            Ok(false) => {
                out.skipped.push(Skipped {
                    check: "green-at-head",
                    detail: format!(
                        "{worker} has no recorded green at its head {}",
                        head.get(..8).unwrap_or(&head)
                    ),
                    fix: "in that worktree: air record verify -- <the repo's verify>".to_string(),
                    worker: worker.clone(),
                });
                continue;
            }
            Err(e) => {
                out.errors.push(format!("{worker}: green lookup: {e}"));
                continue;
            }
        }
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
        if ids.is_empty() {
            out.skipped.push(Skipped {
                check: "no-bead-named",
                detail: format!(
                    "{worker} is green at {} but no commit in main..{} declares a bead",
                    head.get(..8).unwrap_or(&head),
                    head.get(..8).unwrap_or(&head)
                ),
                fix: "add a `Bead: <id>` trailer to the commit that did the work (git commit --amend), or land it by name: air land <bead>"
                    .to_string(),
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
        for bead in ids {
            out.landings.push(Landing {
                command: land_command(&bead),
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

/// Just the landable list, for the read-only callers (`air status`, `air inbox --owner`).
/// `air land` uses [`select`], because it is the caller that must not read an error as empty.
pub fn landings_for(repo: &Path) -> Vec<Landing> {
    select(repo).landings
}

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
    let bd = super::claim::bd_for(repo);
    let issues = air_bd::WorkLedger::show_all(&bd, beads)
        .map_err(|e| format!("bd show for {}: {e}", beads.join(" ")))?;
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
        Some(false) => format!(
            "the `air land` process (pid {}) is GONE: it was killed mid-verify, main still \
             holds the merge and the rollback never ran. Check main, then `git reset --hard {}` \
             to undo it or re-run `air land`",
            l.pid.unwrap_or(0),
            l.tip_sha.as_deref().unwrap_or("<tip>")
        ),
        Some(true) => format!("verifying now (pid {})", l.pid.unwrap_or(0)),
        None => "no pid recorded, so nothing can say whether it is still running".into(),
    };
    format!(
        "{} ({}) merged at {} {elapsed}, rollback armed to {}: {state}",
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
/// adopter, 2026-08-23: *"A rollback un-lands a branch from main but cannot un-merge it from
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

/// Verifies running right now, oldest first, with dead pids pruned on the way out (air-4cr).
/// Shared by `air status` and `air land`, so both answer the question the same way.
pub fn verifies_in_flight(ledger: &Ledger) -> Vec<air_ledger::verify::InFlight> {
    ledger
        .in_flight_pruned(super::lease::pid_alive)
        .unwrap_or_default()
}

/// One line for a verify in flight: who, how long, and at which sha. Seconds, not minutes —
/// a verify is ~420 s in adopter's repo, so a minutes-only reading rounds most of it to 0.
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
                match sess.state.as_str() {
                    "stuck" if age >= t.stuck_min => out.push(Attention {
                        worker: w.worker.clone(),
                        kind: kinds::STUCK,
                        detail: format!(
                            "waiting on a permission prompt{} for {age} min; answer it in their terminal",
                            sess.detail.as_deref().map(|d| format!(" ({d})")).unwrap_or_default()
                        ),
                        for_minutes: age,
                        fingerprint: String::new(),
                    }),
                    "idle" if has_claim && age >= t.idle_with_claim_min => out.push(Attention {
                        worker: w.worker.clone(),
                        kind: kinds::IDLE_WITH_CLAIM,
                        detail: format!(
                            "idle {age} min holding {}; prompt them, or `air release` if abandoned",
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
                    "idle"
                        if !has_claim
                            && w.role == "worker"
                            && sess.pid_alive != Some(false)
                            && s.ready_depth.is_some_and(|n| n > 0)
                            && age >= t.idle_noclaim_min =>
                    {
                        out.push(Attention {
                            worker: w.worker.clone(),
                            kind: kinds::IDLE_WITHOUT_CLAIM,
                            detail: format!(
                                "idle {age} min, {} beads ready; prompt them",
                                s.ready_depth.unwrap_or(0)
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
                            "no live session but holds {}; restart `air worker {}` or release",
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
        // every claim it holds is not-green for the SAME reason and the same fix; adopter's
        // status printed eleven lines for one worker, which is one fact eleven times. The
        // single-claim wording is unchanged, because that is the case that reads well already.
        //
        // Removal: when no worker ever holds two claims at once, this collapses nothing and
        // the loop above can go back to pushing per claim.
        let stuck: Vec<&Claim> = if w.green_at_head == Some(false) {
            w.claims
                .iter()
                .filter(|c| c.handover_attempts > 0)
                .collect()
        } else {
            Vec::new()
        };
        // Longest wait first, so `for_minutes` is the oldest attempt rather than an arbitrary
        // one, and the beads read in the order they have been waiting.
        let oldest = stuck
            .iter()
            .filter_map(|c| c.last_handover_at.as_deref())
            .min()
            .unwrap_or(now);
        let detail = match stuck.as_slice() {
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
    for (l, defect) in &s.leases {
        let Some(d) = defect else { continue };
        let kind = if d.starts_with("dead") {
            kinds::LEASE_HELD_BY_DEAD_SESSION
        } else {
            kinds::LEASE_STALE
        };
        out.push(Attention {
            worker: l.worker.clone(),
            kind,
            detail: format!(
                "{} lease held by {} is {d} (reason: {}); `air lease break {}` or let the next taker break it",
                l.resource, l.worker, l.reason, l.resource
            ),
            for_minutes: minutes_between(&l.heartbeat_at, now).unwrap_or(0),
            fingerprint: String::new(),
        });
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
        for l in &s.landable {
            let e = by_worker
                .entry(&l.worker)
                .or_insert((&l.head, Vec::new(), 0));
            e.1.push(&l.bead);
            e.2 = e.2.max(l.minutes);
        }
        for (worker, (head, beads, minutes)) in by_worker {
            out.push(Attention {
                worker: worker.to_string(),
                kind: kinds::LANDABLE,
                detail: format!(
                    "{worker} is green at {} with main merged, carrying {}; `air land --all`",
                    head.get(..8).unwrap_or(head),
                    beads.join(" ")
                ),
                for_minutes: minutes,
                fingerprint: format!("{worker}@{head}"),
            });
        }
    }
    // air-ayp: a bead that landed while this merge contradicts one of its acceptance clauses.
    // Not "Air could not read it" — refuted. Subject is the bead, so the channel says it once
    // and says it again only when the reason changes.
    for o in &s.landed_open {
        let (bead, why) = (&o.bead, &o.why);
        out.push(Attention {
            worker: bead.clone(),
            kind: kinds::LANDED_NOT_CLOSED,
            detail: format!(
                "{bead} landed in {} with an acceptance clause this merge CONTRADICTS: {why}. \
                 The worker closes its own bead with proof, so read the bead: either reopen it \
                 or file what is left. Landed from {}.",
                o.merge_commit.get(..8).unwrap_or(&o.merge_commit),
                o.worker
            ),
            for_minutes: 0,
            fingerprint: format!("{bead}/{why}"),
        });
    }
    if let Some(oldest) = &s.oldest_owner_capture_at {
        out.push(Attention {
            worker: "owner".to_string(),
            kind: kinds::OWNER_DECISION_WAITING,
            detail: format!(
                "{} decision(s) waiting for the owner, oldest {} min; `air inbox --owner`",
                s.owner_queue_depth,
                minutes_between(oldest, now).unwrap_or(0)
            ),
            for_minutes: minutes_between(oldest, now).unwrap_or(0),
            // How many are waiting, not how long the oldest has waited.
            fingerprint: format!("depth:{}", s.owner_queue_depth),
        });
    }
    out
}

/// Build the snapshot: one row per worktree (plus any worker known only from the ledger).
/// Whether this gather may shell out to bd (air-cmn).
///
/// The channel poll ran a full `gather` every ~8 s, and every one of them called bd:
/// `in_progress`, then `show` once per open claim, then `awaiting_review`, then `ready`. That
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
    let (ledger, _me) = open(repo)?;
    let at = now();
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
                let head = git::head(&path).ok();
                let green = match &head {
                    Some(h) => ledger.is_green_at(&name, h, Kind::Verify).ok(),
                    None => None,
                };
                views.insert(
                    name.clone(),
                    WorkerView {
                        role: super::hook::role_for(&name).to_string(),
                        worker: name,
                        head,
                        green_at_head: green,
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
                "SELECT worker, role, session_id, state, detail, changed_at, pid, project, model \
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
    // honest and no condition ever fires on it (adopter round: ~38 noise pushes, A3).
    //
    // bd is enrichment, not the spine. It gets a short budget (`AIR_BD_TIMEOUT_MS` overrides)
    // and after one timeout no further bd call is made this tick; the counts fall back to the
    // last answer cached in the ledger. Under load bd took 20 s, the same as the MCP tool
    // budget, so the channel got nothing exactly when the fleet was busiest (adopter
    // 2026-08-22, air-19u).
    //
    // The budget is DERIVED from what bd costs here today, not a constant (air-p61): a flat
    // 2 s left 356 ms of headroom over bd's measured p99 and sat below adopter's median
    // entirely. `status_bd_budget` reads the same measurement `air status` prints.
    let today_latency = super::bd_latency::for_day(ledger.dir(), &super::today());
    let mut bd = super::claim::bd_for(repo);
    if std::env::var_os("AIR_BD_TIMEOUT_MS").is_none() {
        bd.timeout = super::bd_latency::status_bd_budget(today_latency.map(|l| l.median_ms));
    }
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
    let in_progress: Option<std::collections::BTreeSet<String>> = bd_try(
        &bd,
        &mut bd_slow,
        &mut errors,
        "in_progress (claims not reconciled)",
        air_bd::WorkLedger::in_progress,
    )
    .map(|v| v.into_iter().map(|i| i.id).collect());
    let mut reconciled = 0usize;
    for c in ledger.open_claims().map_err(|e| e.to_string())? {
        let mut handed_over = false;
        if let Some(ip) = &in_progress
            && !ip.contains(&c.bead)
        {
            let status = bd_try(&bd, &mut bd_slow, &mut errors, "show", |b| {
                air_bd::WorkLedger::show(b, &c.bead)
            });
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
                let reason = match &status {
                    Some(Some(i)) if i.status == "closed" => "closed",
                    _ => "reconciled",
                };
                let _ = ledger.release_claim(&c.bead, &c.worker, reason, &at);
                reconciled = reconciled.saturating_add(1);
                continue;
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
                if holders.len() > 1 {
                    overlaps.insert(file, holders.into_iter().map(|h| h.worker).collect());
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
    // A healthy answer also refreshes `.air/ready.json` for the Stop hook (air-09i); a slow
    // bd leaves that file as it was.
    // Beads bd shows back in the work queue. Used to clear a `landed-not-closed` report once
    // somebody reopened the bead (air-dlw); no extra bd call, these lists are already here.
    let mut back_in_queue: std::collections::BTreeSet<String> =
        in_progress.iter().flatten().cloned().collect();
    let ready_depth: Option<usize> = match bd_try(&bd, &mut bd_slow, &mut errors, "ready", |b| {
        air_bd::WorkLedger::ready(b)
    }) {
        Some(v) => {
            back_in_queue.extend(v.iter().map(|i| i.id.clone()));
            let ids = super::ready_cache::claimable(&v);
            super::ready_cache::write(repo, &ids, &super::now());
            let _ = ledger.bd_cache_put("ready_depth", &v.len().to_string(), &at);
            Some(v.len())
        }
        None if bd_slow.is_some() => ledger
            .bd_cache_get("ready_depth")
            .ok()
            .flatten()
            .and_then(|(v, _)| v.parse().ok()),
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
    let owner_q = ledger.inbox_for("owner").map_err(|e| e.to_string())?;
    let stale = std::env::var("AIR_LEASE_STALE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600);
    let leases = ledger
        .leases()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|l| {
            let d = super::lease::defect(&l, &at, stale);
            (l, d)
        })
        .collect();
    Ok(Snapshot {
        at,
        workers: views.into_values().collect(),
        inbox_depth: inbox.len(),
        oldest_capture_at: inbox.first().map(|c| c.captured_at.clone()),
        owner_queue_depth: owner_q.len(),
        oldest_owner_capture_at: owner_q.first().map(|c| c.captured_at.clone()),
        leases,
        // A ledger read, so it survives an absent bd (air-ayp). A bead bd shows back in the
        // work queue has been dealt with: somebody reopened it. Deriving it from the claim row
        // instead is what made this silent under close-with-proof (air-dlw).
        landed_open: ledger
            .landed_open()
            .unwrap_or_default()
            .into_iter()
            .filter(|o| !back_in_queue.contains(&o.bead))
            .collect(),
        sessions: all_sessions,
        ready_depth,
        // air-4cr. Reading is also the pruning: a crashed `air record` leaves a row and the
        // next status clears it, so no expiry window has to be chosen or tuned.
        verifies_in_flight: verifies_in_flight(&ledger),
        landings_in_flight: landings_in_flight(&ledger),
        rewound_carried: rewound_carried(repo, &ledger, git::head(repo).ok().as_deref()),
        // air-03w: the same selection `air land --all` runs, so the condition cannot claim a
        // branch is landable that the command would then skip.
        landable: landings_for(repo),
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
        if attention_only {
            "status.attention"
        } else {
            "status"
        },
        &serde_json::json!({"conditions": kinds, "opened": opened, "cleared": cleared, "ready_depth": snap.ready_depth, "inbox": snap.inbox_depth, "owner_queue": snap.owner_queue_depth, "duration_ms": snap.duration_ms}),
        if att.is_empty() { "quiet" } else { "attention" },
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

fn render(s: &Snapshot, att: &[Attention]) -> String {
    // First, because it is the only thing here nobody else can clear (air-6p5).
    let mut out = waiting_on_owner(s);
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
                format!("{} since {} [{model}]", x.state, x.changed_at)
            })
            .unwrap_or_else(|| "no session".to_string());
        let green = match w.green_at_head {
            Some(true) => "green",
            Some(false) => "not green",
            None => "unknown",
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
        let idle_no_claim = w.role == "worker"
            && w.claims.is_empty()
            && w.session.as_ref().is_some_and(|x| x.state == "idle");
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
        out.push_str(&format!("verify in flight: {}\n", in_flight_line(f, &s.at)));
    }
    // air-bxe: the merge commit exists for minutes before the verify decides whether it stays.
    for f in &s.landings_in_flight {
        out.push_str(&format!(
            "landing in flight: {}\n",
            landing_in_flight_line(f, &s.at)
        ));
    }
    // air-ob0: a rewind un-lands from main and cannot un-merge from whoever took it.
    for r in &s.rewound_carried {
        for line in rewind_propagation(&r.merge_commit, &r.carried_by) {
            out.push_str(&format!("rewound and still carried: {line}\n"));
        }
    }
    out.push_str(&format!(
        "ready: {}{}\n",
        s.ready_depth
            .map(|n| n.to_string())
            .unwrap_or_else(|| "? (bd did not answer)".into()),
        // Which source, always: "0 ready" from a cache and "0 ready" from bd are different
        // facts, and only one of them is today's (air-cmn).
        match s.bd_source {
            "cache" => " (cached; bd not called this tick)",
            "stale" => " (stale; bd did not answer)",
            _ => "",
        }
    ));
    out.push_str(&format!(
        "inbox: {} open; owner queue: {} open\n",
        s.inbox_depth, s.owner_queue_depth
    ));
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
            if attention_only {
                "status.attention"
            } else {
                "status"
            },
            &serde_json::json!({}),
            if att.is_empty() { "quiet" } else { "attention" },
            &format!("{} condition(s)", att.len()),
            &format!(
                "{} workers, {} overlaps, inbox {}",
                snap.workers.len(),
                snap.overlaps.len(),
                snap.inbox_depth
            ),
        );
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
    emit(
        json,
        &serde_json::json!({"snapshot": snap, "attention": att}),
        || render(&snap, &att),
    );
    0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

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
            }),
            head: Some("abc".into()),
            green_at_head: green,
            claims,
            handed_over: vec![],
            files_held: 0,
            tmux_session: None,
        }
    }

    const NOW: &str = "2026-08-20T12:00:00Z";
    const T_30: &str = "2026-08-20T11:30:00Z";
    const T_2: &str = "2026-08-20T11:58:00Z";

    #[test]
    fn quiet_fleet_raises_nothing() {
        let s = Snapshot {
            workers: vec![
                worker(
                    "a",
                    Some("working"),
                    T_2,
                    vec![claim("fd-1", "a", T_30, 0)],
                    Some(true),
                ),
                worker("b", Some("idle"), T_30, vec![], Some(true)), // idle without claim is fine
            ],
            ..Default::default()
        };
        assert!(attention(&s, NOW, Thresholds::default()).is_empty());
    }

    #[test]
    fn each_condition_fires_with_its_threshold() {
        let s = Snapshot {
            workers: vec![
                worker("stuck", Some("stuck"), T_30, vec![], None),
                worker(
                    "idle",
                    Some("idle"),
                    T_30,
                    vec![claim("fd-2", "idle", T_30, 0)],
                    None,
                ),
                worker(
                    "silent",
                    Some("running"),
                    T_30,
                    vec![claim("fd-3", "silent", T_30, 0)],
                    None,
                ),
                worker(
                    "gone",
                    None,
                    T_30,
                    vec![claim("fd-4", "gone", T_30, 0)],
                    None,
                ),
                worker(
                    "red",
                    Some("working"),
                    T_2,
                    vec![claim("fd-5", "red", T_2, 2)],
                    Some(false),
                ),
            ],
            inbox_depth: 3,
            oldest_capture_at: Some(T_30.into()),
            ..Default::default()
        };
        let att = attention(&s, NOW, Thresholds::default());
        let kinds: Vec<(&str, &str)> = att.iter().map(|a| (a.worker.as_str(), a.kind)).collect();
        assert_eq!(
            kinds,
            vec![
                ("stuck", "stuck"),
                ("idle", "idle-with-claim"),
                ("silent", "silent-with-claim"),
                ("gone", "gone-with-claim"),
                ("red", "handover-not-green"),
            ]
        );
        assert_eq!(att[0].for_minutes, 30);
        // Tighten nothing, loosen everything: all time-based ones go quiet.
        let loose = Thresholds {
            stuck_min: 60,
            idle_with_claim_min: 60,
            silent_with_claim_min: 60,
            launch_grace_min: 3,
            idle_noclaim_min: 60,
        };
        let att = attention(&s, NOW, loose);
        let kinds: Vec<&str> = att.iter().map(|a| a.kind).collect();
        assert_eq!(kinds, vec!["gone-with-claim", "handover-not-green"]);
    }

    /// air-6p5 asked this line to name every landing waiting on the owner. Landings moved to
    /// the coordinator (air-3pz, `air land --all`), and the list it printed was derived from
    /// `review_waits`, which air-okc deleted with the `review-waiting` condition. What is left
    /// is the half that was still true: decisions are the only thing that waits on the owner.
    #[test]
    fn waiting_on_owner_names_decisions_and_is_empty_when_nothing_waits() {
        let s = Snapshot {
            owner_queue_depth: 1,
            ..Default::default()
        };
        assert_eq!(
            waiting_on_owner(&s),
            "waiting on owner: 1 decision; `air inbox --owner`\n"
        );
        let two = Snapshot {
            owner_queue_depth: 2,
            ..Default::default()
        };
        assert!(waiting_on_owner(&two).contains("2 decisions"));
        // A quiet fleet stays quiet.
        assert_eq!(waiting_on_owner(&Snapshot::default()), "");
    }

    /// air-3eu: the claim on a handed-over bead stays open so the coordinator still sees the
    /// files, but it is not work in progress. None of the with-a-claim conditions may read it
    /// as one, or every worker waiting on review would raise them for the whole round.
    #[test]
    fn a_bead_waiting_on_review_is_not_a_claim_in_progress() {
        let mut w = worker("w", Some("idle"), T_30, vec![], Some(false));
        w.handed_over = vec![claim("fd-1", "w", T_30, 1)];
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
                T_2,
                vec![claim("fd-1", "new", T_2, 0)],
                None,
            )],
            ..Default::default()
        };
        assert!(attention(&fresh, NOW, Thresholds::default()).is_empty());
        // Idle at the prompt with a live pid and a claim, 2 min old: quiet.
        let mut idle = worker(
            "w",
            Some("idle"),
            T_2,
            vec![claim("fd-2", "w", T_30, 0)],
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
        idle.session.as_mut().unwrap().changed_at = T_30.to_string();
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
            workers: vec![worker("w", Some("idle"), T_2, vec![], Some(true))],
            ..Default::default()
        };
        let att = attention(&s, NOW, Thresholds::default());
        let text = render(&s, &att);
        assert!(!text.contains("review:"), "the review line is gone: {text}");
        assert!(text.contains("ready: ? (bd did not answer)"), "{text}");
        assert!(text.contains("claims: -  idle, no claim"), "{text}");
        let none = Snapshot {
            ready_depth: Some(3),
            ..Default::default()
        };
        assert!(render(&none, &[]).contains("ready: 3\n"));
    }

    #[test]
    fn idle_without_claim_fires_only_with_ready_beads_past_the_threshold() {
        let mut s = Snapshot {
            workers: vec![
                worker("w", Some("idle"), T_30, vec![], Some(true)),
                worker("fresh", Some("idle"), T_2, vec![], Some(true)),
                WorkerView {
                    role: "coordinator".into(),
                    ..worker("main", Some("idle"), T_30, vec![], None)
                },
            ],
            ready_depth: Some(5),
            ..Default::default()
        };
        let att = attention(&s, NOW, Thresholds::default());
        assert_eq!(att.len(), 1, "{att:?}");
        assert_eq!(
            (att[0].worker.as_str(), att[0].kind),
            ("w", "idle-without-claim")
        );
        assert_eq!(att[0].detail, "idle 30 min, 5 beads ready; prompt them");
        let t = Thresholds {
            idle_noclaim_min: 1,
            ..Thresholds::default()
        };
        assert_eq!(attention(&s, NOW, t).len(), 2);
        s.ready_depth = Some(0);
        assert!(attention(&s, NOW, Thresholds::default()).is_empty());
        s.ready_depth = None;
        assert!(attention(&s, NOW, Thresholds::default()).is_empty());
    }

    /// air-d10. A dead session is not an idle worker: there is nobody to prompt, and the two
    /// longest-lived rows in this repo's ledger were exactly this, open 4 885 minutes each.
    #[test]
    fn idle_without_claim_is_quiet_when_the_session_process_is_gone() {
        let mk = |alive: Option<bool>| {
            let mut w = worker("w", Some("idle"), T_30, vec![], Some(true));
            let sess = w.session.as_mut().unwrap();
            sess.pid = Some(1);
            sess.pid_alive = alive;
            Snapshot {
                workers: vec![w],
                ready_depth: Some(5),
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
    fn lease_defects_and_owner_queue_fire() {
        let lease = |res: &str, beat: &str| Lease {
            resource: res.into(),
            worker: "a".into(),
            session_id: None,
            pid: Some(1),
            pid_started: None,
            reason: "api".into(),
            taken_at: T_30.into(),
            heartbeat_at: beat.into(),
        };
        let s = Snapshot {
            leases: vec![
                (lease(":8080", T_30), Some("dead (pid 1 gone)".into())),
                (lease("chrome", T_30), Some("stale (idle 30 min)".into())),
                (lease("runtime", T_2), None),
            ],
            owner_queue_depth: 2,
            oldest_owner_capture_at: Some(T_2.into()),
            ..Default::default()
        };
        let kinds: Vec<&str> = attention(&s, NOW, Thresholds::default())
            .iter()
            .map(|a| a.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                kinds::LEASE_HELD_BY_DEAD_SESSION,
                kinds::LEASE_STALE,
                "owner-decision-waiting"
            ]
        );
    }

    #[test]
    fn unparseable_timestamps_never_panic_or_fire() {
        let s = Snapshot {
            workers: vec![worker("x", Some("stuck"), "garbage", vec![], None)],
            ..Default::default()
        };
        assert!(attention(&s, NOW, Thresholds::default()).is_empty());
        assert_eq!(minutes_between("garbage", NOW), None);
        assert_eq!(minutes_between(NOW, T_30), Some(-30));
    }
}
