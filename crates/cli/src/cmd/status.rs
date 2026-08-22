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
    /// Beads in `awaiting_review` per bd (None when bd could not answer). A measurement only:
    /// there is no cap ("we set our goals and finish them", owner 2026-08-21).
    pub awaiting_review: Option<Vec<String>>,
    /// Review wait per open claim that has been handed over: (bead, worker, minutes).
    pub review_waits: Vec<(String, String, i64)>,
    /// Beads a landing merged but did not close, and that nobody has closed since. A ledger
    /// fact, never a bd status (air-ayp).
    pub landed_open: Vec<air_ledger::landings::LandedOpen>,
    /// Every session row (two sessions in one checkout are two entries; the per-worker view
    /// above keeps only the latest): (worker, role, session).
    pub sessions: Vec<(String, String, Session)>,
    /// `bd ready` count at this tick (None when bd did not answer): queue depth over time
    /// (plan 0006 C6; the round ran dry at 4 with only epics left).
    pub ready_depth: Option<usize>,
    /// file -> workers holding it (only files with 2+ holders)
    pub overlaps: BTreeMap<String, Vec<String>>,
    pub errors: Vec<String>,
    /// How long `gather` took; on the event line so a slow status is measured, not felt.
    pub duration_ms: u64,
    /// Median cost of one `bd` process today, from the event log (air-869). None when
    /// nothing shelled out to bd today.
    pub bd_latency: Option<super::bd_latency::BdLatency>,
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

/// The landing command for one bead, with the lead-in a bare condition line needs.
pub fn land_hint(bead: &str) -> String {
    format!("land it: {}", land_command(bead))
}

/// The command alone, for a line that already says what it is (air-6p5). Since air-3pz that
/// is `air land`: the coordinator's one allowed path onto main.
pub fn land_command(bead: &str) -> String {
    format!("air land {bead}")
}

/// Who handed `bead` over and when, from the claim row, open or released. The claim row is the
/// only source (an open-claims scan was why review waits were always empty, air-e7q); since
/// air-3eu the row is usually still open, stamped by the reconcile, so `first_handover_at` is
/// what the coalesce finds. `fallback` is used when there is no row at all.
fn handover_of(ledger: &Ledger, bead: &str, fallback: &str) -> (String, String) {
    ledger
        .conn()
        .query_row(
            "SELECT worker, coalesce(first_handover_at, released_at, claimed_at) FROM claims \
             WHERE bead=?1 ORDER BY claimed_at DESC LIMIT 1",
            rusqlite::params![bead],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .unwrap_or_else(|_| ("?".to_string(), fallback.to_string()))
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

/// Pure: the landings a snapshot shows. A review wait whose worker has a recorded green at
/// HEAD is the owner's to merge; one that is not green is the worker's to fix, and shows as
/// `review-waiting` instead.
pub fn landings(s: &Snapshot) -> Vec<Landing> {
    let mut v: Vec<Landing> = s
        .review_waits
        .iter()
        .filter_map(|(bead, worker, minutes)| {
            let w = s.workers.iter().find(|w| &w.worker == worker)?;
            if w.green_at_head != Some(true) {
                return None;
            }
            Some(Landing {
                bead: bead.clone(),
                worker: worker.clone(),
                head: w.head.clone()?,
                minutes: *minutes,
                command: land_command(bead),
                // The snapshot has no descriptions; `landings_for` is the path that reads bd.
                acceptance: Vec::new(),
            })
        })
        .collect();
    sort_by_wait(&mut v);
    v
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
    let l = landings(s);
    let decisions = s.owner_queue_depth;
    if l.is_empty() && decisions == 0 {
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
    if !l.is_empty() {
        let named = l
            .iter()
            .map(|x| {
                format!(
                    "{} {} from {}",
                    x.bead,
                    x.head.get(..8).unwrap_or(&x.head),
                    x.worker
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "{} ready: `air land --all` ({named})\n",
            plural(l.len(), "landing")
        ));
        for x in &l {
            out.push_str(&format!(
                "  {} ({} min): `{}`\n",
                x.bead, x.minutes, x.command
            ));
        }
    }
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
/// Bead ids named in the commit messages of `range`, in first-mentioned order.
///
/// Deliberately loose: any `<prefix>-<suffix>` token. bd's own prefix is not read from config,
/// because a false positive costs nothing — `confirm_beads` drops any id bd does not know, and
/// bd omits an unknown id from `show` while still exiting 0 (verified air-76z). A missed bead,
/// by contrast, is a bead nobody reads the acceptance of.
pub fn bead_ids_in(repo: &Path, range: &str) -> Vec<String> {
    bead_ids_in_text(&git::run(repo, &["log", "--format=%s%n%b", range]).unwrap_or_default())
}

/// The pure half, so the rule is probe-able without a git repo.
pub fn bead_ids_in_text(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
        let Some((pre, suf)) = raw.split_once('-') else {
            continue;
        };
        // No length or digit rule beyond this: real ids here include `air-zyo` and `air-ouw`
        // with no digit at all, and test ids are as short as `fd-1`. Anything narrower drops
        // real beads, and a false positive costs one more argument to a single `bd show`.
        let looks_like_id = !pre.is_empty()
            && pre.len() <= 12
            && pre.chars().all(|c| c.is_ascii_lowercase())
            && !suf.is_empty()
            && suf.len() <= 12
            && suf.chars().all(|c| c.is_ascii_alphanumeric());
        if looks_like_id && out.len() < 64 && !out.iter().any(|x| x == raw) {
            out.push(raw.to_string());
        }
    }
    out
}

/// Which candidate ids are real beads, from Air's OWN ledger — no bd call.
///
/// **`bd show` does not batch.** Measured 2026-08-22: one id ~1.4 s, two ~2.4 s, seventeen
/// **19-21 s** — roughly per-id, unlike `bd close`, which really is one process for the whole
/// set (air-869). A first version confirmed candidates with `bd show <ids…>` and silently
/// returned nothing every time, because 17 ids blew the 2 s budget and the error was swallowed
/// into "no landings". That is the failure this repo keeps finding: a cost assumed rather than
/// measured, failing quiet.
///
/// So the frequent path pays nothing. Every bead a worker worked here has a claim row, which
/// is the same fact `air claim` writes, and it filters the loose commit-message extraction
/// down to real ids. `air land` fetches acceptance for the one branch it is landing, where
/// seconds are affordable beside a full verify.
fn known_beads(
    ledger: &air_ledger::Ledger,
    ids: &[String],
    worker: &str,
    since: &str,
) -> Vec<String> {
    if ids.is_empty() {
        return Vec::new();
    }
    // Claimed BY THIS WORKER, not merely known to the ledger. A branch's commit bodies cite
    // other people's beads constantly ("the flow changed (air-7o3)"), and without this the
    // report named eight beads for a branch that carried three.
    // ...and claimed SINCE THE BRANCH POINT. A worktree name is permanent, so without a time
    // bound the set only ever grows: a commit citing one of this worker's own older beads
    // ("AIR_ENFORCE=1 since air-i59") re-reported a bead landed rounds ago. The bound is the
    // branch point, which is the same instant the merge range starts from, so the two cannot
    // drift apart. **[adopter, measured]** they hit the unbounded version first.
    let mut st = match ledger
        .conn()
        .prepare("SELECT DISTINCT bead FROM claims WHERE worker=?1 AND claimed_at >= ?2")
    {
        Ok(st) => st,
        Err(_) => return Vec::new(),
    };
    let known: Vec<String> = st
        .query_map([worker, since], |r| r.get::<_, String>(0))
        .map(|rows| rows.filter_map(std::result::Result::ok).collect())
        .unwrap_or_default();
    ids.iter()
        .filter(|i| known.iter().any(|k| k == *i))
        .cloned()
        .collect()
}

/// What `air land` may land, and what it will report when it does.
///
/// **Selection is: a worktree branch carrying a recorded green at its head, and the beads its
/// merge range names** (air-7kp). It used to be `bd list --status awaiting_review`, which the
/// owner's 2026-08-22 ruling emptied: a worker closes its own bead with proof and never sets
/// that status, so `air land --all` found nothing and the pre-merge acceptance report printed
/// nothing, on every branch. Both came from this one filter.
///
/// Two properties worth stating, because the obvious alternatives lack them:
///
/// - **The range bounds itself.** Candidates come from `main..<head>`, so a bead leaves the
///   set the moment its work lands. A rule keyed on the worker instead — an assignee or a
///   worktree name — has nothing to bound it once beads are closed rather than transient, and
///   would report everything that worker ever closed, growing silently and invisibly to any
///   probe written against a fresh repo (adopter, measured).
/// - **Containment selects, it never closes.** `air land` closes nothing (air-ayp), so a wrong
///   id here prints a bead that did not belong rather than closing one; the cost is a person
///   reading a wrong line, which is why the cheap rule is the right one.
pub fn landings_for(repo: &Path) -> Vec<Landing> {
    let Ok((ledger, _)) = open(repo) else {
        return Vec::new();
    };
    let at = now();
    let mut v: Vec<Landing> = Vec::new();
    for (path, _) in git::worktrees(repo).unwrap_or_default() {
        let worker = air_ledger::paths::worker_name_for(&path).unwrap_or_default();
        if super::hook::role_for(&worker) != "worker" {
            continue;
        }
        let Ok(head) = git::head(&path) else { continue };
        if ledger.is_green_at(&worker, &head, Kind::Verify).ok() != Some(true) {
            continue;
        }
        let range = format!("main..{head}");
        let ids = bead_ids_in(repo, &range);

        // How long this branch has been waiting: its oldest commit since main. Under
        // close-with-proof there is no hand-over moment to measure from, and the branch point
        // is the honest substitute — it is also what keeps the set from growing.
        // Normalised through jiff: git's `%cI` carries an offset (`+00:00`) and the ledger's
        // times are `Z`-suffixed, so comparing the two as strings is wrong for any non-UTC
        // machine and wrong at the character level even on a UTC one.
        let since = git::branch_point_time(&path, "main")
            .ok()
            .and_then(|t| t.parse::<jiff::Timestamp>().ok())
            .map(|t| t.to_string())
            .unwrap_or_else(|| at.clone());
        for bead in known_beads(&ledger, &ids, &worker, &since) {
            v.push(Landing {
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
    sort_by_wait(&mut v);
    v
}

/// Acceptance clauses for the beads of ONE branch, fetched at land time.
///
/// This is the expensive call (`bd show` is ~1.4 s per id, see [`known_beads`]), and it is
/// affordable here only because landing already runs the repo's full verify. Both shapes are
/// read: bd's `acceptance_criteria` field when the bead set it — bd omits the key entirely
/// when unset — and the `## Acceptance Criteria` section of the description otherwise
/// (air-ayp).
pub fn acceptance_for(repo: &Path, beads: &[String]) -> Vec<Vec<String>> {
    if beads.is_empty() {
        return Vec::new();
    }
    let bd = super::claim::bd_for(repo);
    let issues = air_bd::WorkLedger::show_all(&bd, beads).unwrap_or_default();
    beads
        .iter()
        .map(|b| match issues.iter().find(|i| &i.id == b) {
            Some(i) => super::acceptance::clauses_of(&i.acceptance_criteria, &i.description),
            None => Vec::new(),
        })
        .collect()
}

/// Minutes between two RFC 3339 timestamps; None when either does not parse.
pub fn minutes_between(earlier: &str, later: &str) -> Option<i64> {
    let a: jiff::Timestamp = earlier.parse().ok()?;
    let b: jiff::Timestamp = later.parse().ok()?;
    b.duration_since(a).as_secs().checked_div(60)
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
        // `gone-with-claim` was deleted on 2026-08-22 (air-s7c): it fired zero times in the
        // audited window and never in any recorded day since it was added on 2026-08-20. A
        // mechanism that has never fired has never prevented anything. A dead session holding
        // a claim now falls through to the ordinary session states below, which do fire.
        match &w.session {
            Some(sess) => {
                let age = minutes_between(&sess.changed_at, now).unwrap_or(0);
                match sess.state.as_str() {
                    "stuck" if age >= t.stuck_min => out.push(Attention {
                        worker: w.worker.clone(),
                        kind: "stuck",
                        detail: format!(
                            "waiting on a permission prompt{} for {age} min; answer it in their terminal",
                            sess.detail.as_deref().map(|d| format!(" ({d})")).unwrap_or_default()
                        ),
                        for_minutes: age,
                        fingerprint: String::new(),
                    }),
                    "idle" if has_claim && age >= t.idle_with_claim_min => out.push(Attention {
                        worker: w.worker.clone(),
                        kind: "idle-with-claim",
                        detail: format!(
                            "idle {age} min holding {}; prompt them, or `air release` if abandoned",
                            beads()
                        ),
                        for_minutes: age,
                        fingerprint: String::new(),
                    }),
                    "idle"
                        if !has_claim
                            && w.role == "worker"
                            && s.ready_depth.is_some_and(|n| n > 0)
                            && age >= t.idle_noclaim_min =>
                    {
                        out.push(Attention {
                            worker: w.worker.clone(),
                            kind: "idle-without-claim",
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
                            kind: "silent-with-claim",
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
                if age < t.launch_grace_min {
                    continue; // just launched; the first hook has not fired yet
                }
                out.push(Attention {
                    worker: w.worker.clone(),
                    kind: "gone-with-claim",
                    detail: format!(
                        "no live session but holds {}; restart `air worker {}` or release",
                        beads(),
                        w.worker
                    ),
                    for_minutes: age,
                    fingerprint: String::new(),
                });
            }
            None => {}
        }
        for c in &w.claims {
            if c.handover_attempts > 0 && w.green_at_head == Some(false) {
                let since = c.last_handover_at.as_deref().unwrap_or(now);
                out.push(Attention {
                    worker: w.worker.clone(),
                    kind: "handover-not-green",
                    detail: format!(
                        "{} handed over {} time(s) without green verify at HEAD; last attempt {}",
                        c.bead, c.handover_attempts, since
                    ),
                    for_minutes: minutes_between(since, now).unwrap_or(0),
                    fingerprint: String::new(),
                });
            }
        }
    }
    for (l, defect) in &s.leases {
        let Some(d) = defect else { continue };
        let kind = if d.starts_with("dead") {
            "lease-held-by-dead-session"
        } else {
            "lease-stale"
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
    // A hand-over nobody was told about (air-e7q). Subject is the bead: once per bead.
    for (bead, worker, mins) in &s.review_waits {
        let head = s
            .workers
            .iter()
            .find(|w| &w.worker == worker)
            .and_then(|w| w.head.as_deref())
            .map(|h| h.get(..8).unwrap_or(h))
            .unwrap_or("?");
        out.push(Attention {
            worker: bead.clone(),
            kind: "review-waiting",
            detail: format!(
                "{bead} handed over by {worker} {mins} min ago (head {head}); {}",
                land_hint(bead)
            ),
            for_minutes: *mins,
            // The bead being in the waiting set is the whole fact; who handed it over and
            // from which head can change without the fact changing, and the age never counts.
            fingerprint: format!("{bead}/{worker}"),
        });
    }
    // air-ayp: a bead that landed while this merge contradicts one of its acceptance clauses.
    // Not "Air could not read it" — refuted. Subject is the bead, so the channel says it once
    // and says it again only when the reason changes.
    for o in &s.landed_open {
        let (bead, why) = (&o.bead, &o.why);
        out.push(Attention {
            worker: bead.clone(),
            kind: "landed-not-closed",
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
            kind: "owner-decision-waiting",
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
pub fn gather(repo: &Path) -> Result<Snapshot, String> {
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
                "SELECT worker, role, session_id, state, detail, changed_at, pid, project \
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
    // bd is enrichment, not the spine. It gets a short budget (2 s default, `AIR_BD_TIMEOUT_MS`)
    // and after one timeout no further bd call is made this tick; the counts fall back to the
    // last answer cached in the ledger. Under load bd took 20 s, the same as the MCP tool
    // budget, so the channel got nothing exactly when the fleet was busiest (adopter
    // 2026-08-22, air-19u).
    let mut bd = super::claim::bd_for(repo);
    if std::env::var_os("AIR_BD_TIMEOUT_MS").is_none() {
        bd.timeout = std::time::Duration::from_secs(2);
    }
    let mut bd_slow: Option<String> = None;
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
    let awaiting_review: Option<Vec<String>> =
        match bd_try(&bd, &mut bd_slow, &mut errors, "awaiting_review", |b| {
            air_bd::WorkLedger::by_status(b, "awaiting_review")
        }) {
            Some(v) => {
                let ids: Vec<String> = v.into_iter().map(|i| i.id).collect();
                let _ = ledger.bd_cache_put(
                    "awaiting_review",
                    &serde_json::to_string(&ids).unwrap_or_default(),
                    &at,
                );
                Some(ids)
            }
            None if bd_slow.is_some() => ledger
                .bd_cache_get("awaiting_review")
                .ok()
                .flatten()
                .and_then(|(v, _)| serde_json::from_str(&v).ok()),
            None => None,
        };
    // One row per bead bd holds in awaiting_review: who handed it over and how long ago, from
    // the claim row, open or released. The claim row is the only source (an open-claims scan
    // was why this was always empty, air-e7q); since air-3eu the row is usually still open,
    // stamped by the reconcile above, so `first_handover_at` is what the coalesce finds.
    let review_waits: Vec<(String, String, i64)> = awaiting_review
        .iter()
        .flatten()
        .map(|bead| {
            let (worker, since) = handover_of(&ledger, bead, &at);
            (
                bead.clone(),
                worker,
                minutes_between(&since, &at).unwrap_or(0),
            )
        })
        .collect();
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
    if let Some(slow) = &bd_slow {
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
        awaiting_review,
        review_waits,
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
        overlaps,
        errors,
        duration_ms: u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX),
        // From the lines already on disk: this tick's own bd cost is logged after gather,
        // so it lands in the next reading.
        bd_latency: super::bd_latency::for_day(ledger.dir(), &super::today()),
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

/// Conditions as rows (first-seen/cleared) and one event line that names every kind and
/// worker, plus the queue depth (plan 0006 C1, C6). Shared by the CLI and the channel poll.
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
            .map(|x| format!("{} since {}", x.state, x.changed_at))
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
    // Always rendered (air-e7q): what waits on whom.
    out.push_str(&format!("review: {} waiting\n", s.review_waits.len()));
    for (bead, worker, mins) in &s.review_waits {
        out.push_str(&format!(
            "  {bead} by {worker}, {mins} min; {}\n",
            land_hint(worker)
        ));
    }
    out.push_str(&format!(
        "ready: {}\n",
        s.ready_depth
            .map(|n| n.to_string())
            .unwrap_or_else(|| "? (bd did not answer)".into())
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

    /// air-6p5: two green hand-overs waited on the owner and nothing said so. The line names
    /// each bead, its sha, who handed it over, and the exact command; a quiet fleet is silent.
    #[test]
    fn waiting_on_owner_names_every_landing_and_is_empty_when_nothing_waits() {
        let mut green = worker("alpha", Some("working"), T_2, vec![], Some(true));
        green.head = Some("8c190753abcdef".into());
        let mut red = worker("beta", Some("working"), T_2, vec![], Some(false));
        red.head = Some("deadbeefcafe".into());
        let s = Snapshot {
            workers: vec![green, red],
            review_waits: vec![
                ("air-i59".into(), "alpha".into(), 18),
                // Handed over but not green at HEAD: the worker's to fix, not the owner's.
                ("air-869".into(), "beta".into(), 2),
            ],
            owner_queue_depth: 1,
            ..Default::default()
        };
        let out = waiting_on_owner(&s);
        assert!(
            out.starts_with("1 landing ready: `air land --all` (air-i59 8c190753 from alpha)\n"),
            "{out}"
        );
        assert!(
            out.contains("air-i59 (18 min): `air land air-i59`"),
            "{out}"
        );
        assert!(
            !out.contains("air-869"),
            "not green is not a landing: {out}"
        );
        assert!(
            out.contains("waiting on owner: 1 decision; `air inbox --owner`"),
            "{out}"
        );

        // Nothing waiting: nothing printed, so a quiet fleet stays quiet.
        let quiet = Snapshot {
            workers: s.workers.clone(),
            review_waits: vec![("air-869".into(), "beta".into(), 2)],
            ..Default::default()
        };
        assert_eq!(waiting_on_owner(&quiet), "");
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

    /// Reversed on 2026-08-22 (air-e7q): a hand-over nobody is told about is a condition,
    /// once per bead; the depth of the queue is still only a measurement (no cap).
    #[test]
    fn review_wait_is_a_condition_once_per_bead_and_depth_is_not() {
        let s = Snapshot {
            workers: vec![worker("w", Some("idle"), T_2, vec![], Some(true))],
            awaiting_review: Some(vec!["a".into(); 22]),
            review_waits: vec![("a".into(), "w".into(), 45), ("b".into(), "w".into(), 1)],
            ..Default::default()
        };
        let att = attention(&s, NOW, Thresholds::default());
        let kinds: Vec<(&str, &str)> = att.iter().map(|a| (a.worker.as_str(), a.kind)).collect();
        assert_eq!(kinds, [("a", "review-waiting"), ("b", "review-waiting")]);
        assert!(
            att[0]
                .detail
                .contains("handed over by w 45 min ago (head abc)")
        );
        assert!(att[0].detail.contains("land it:"));
        assert_eq!(att[0].for_minutes, 45);
        // Rendered, always: the count, one line per bead, ready depth, idle-no-claim.
        let text = render(&s, &att);
        assert!(
            text.contains("review: 2 waiting\n  a by w, 45 min; land it:"),
            "{text}"
        );
        assert!(text.contains("ready: ? (bd did not answer)"));
        assert!(text.contains("claims: -  idle, no claim"));
        let none = Snapshot {
            ready_depth: Some(3),
            ..Default::default()
        };
        assert!(render(&none, &[]).contains("review: 0 waiting\nready: 3\n"));
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
                "lease-held-by-dead-session",
                "lease-stale",
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
