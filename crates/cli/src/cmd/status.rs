//! `air status [--attention]`: the coordinator's one screen, and the deterministic
//! conditions that mean "a human or the coordinator is needed" (decisions 2026-08-20: the
//! coordinator is informed, not woken; the channel delivers exactly these).
//!
//! Split in two so the conditions are testable without git or a clock: `gather` builds a
//! `Snapshot` from the ledger and git; `attention` is a pure function of (snapshot, now,
//! thresholds).

use std::collections::BTreeMap;
use std::path::Path;

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
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct WorkerView {
    pub worker: String,
    pub role: String,
    pub session: Option<Session>,
    pub head: Option<String>,
    pub green_at_head: Option<bool>,
    pub claims: Vec<Claim>,
    pub files_held: usize,
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
    /// stuck | idle-with-claim | silent-with-claim | gone-with-claim | handover-not-green |
    /// owner-decision-waiting | lease-held-by-dead-session | lease-stale | review-waiting |
    /// idle-without-claim
    /// (inbox depth is a measurement in `status`, never a condition: audit 2026-08-21;
    /// review waits became a condition on 2026-08-22, air-e7q: three parties waited 20 min
    /// on a fact nobody was told)
    pub kind: &'static str,
    pub detail: String,
    pub for_minutes: i64,
}

/// The landing command for a worker's branch. The repo's own until `air land` exists
/// (CLAUDE.md); the worktree branch is named after the worker.
pub fn land_hint(worker: &str) -> String {
    format!("land it: on main, `git merge --no-ff {worker}` then verify")
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
        match &w.session {
            Some(sess) if sess.pid_alive == Some(false) && has_claim => {
                let age = minutes_between(&sess.changed_at, now).unwrap_or(0);
                out.push(Attention {
                    worker: w.worker.clone(),
                    kind: "gone-with-claim",
                    detail: format!(
                        "claude pid {} is gone but {} is still claimed; restart `air worker {}` or release",
                        sess.pid.unwrap_or(0),
                        beads(),
                        w.worker
                    ),
                    for_minutes: age,
                });
            }
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
                    }),
                    "idle" if has_claim && age >= t.idle_with_claim_min => out.push(Attention {
                        worker: w.worker.clone(),
                        kind: "idle-with-claim",
                        detail: format!(
                            "idle {age} min holding {}; prompt them, or `air release` if abandoned",
                            beads()
                        ),
                        for_minutes: age,
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
                land_hint(worker)
            ),
            for_minutes: *mins,
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
                "SELECT worker, role, session_id, state, detail, changed_at, pid FROM sessions \
                 ORDER BY changed_at DESC",
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
        if let Some(ip) = &in_progress
            && !ip.contains(&c.bead)
        {
            let reason = match bd_try(&bd, &mut bd_slow, &mut errors, "show", |b| {
                air_bd::WorkLedger::show(b, &c.bead)
            }) {
                Some(Some(i)) if i.status == "closed" => "closed",
                Some(Some(i)) if i.status == "awaiting_review" => "handed-over",
                _ => "reconciled",
            };
            let _ = ledger.release_claim(&c.bead, &c.worker, reason, &at);
            reconciled = reconciled.saturating_add(1);
            continue;
        }
        let v = views.entry(c.worker.clone()).or_insert_with(|| WorkerView {
            worker: c.worker.clone(),
            role: super::hook::role_for(&c.worker).to_string(),
            ..Default::default()
        });
        v.claims.push(c);
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
    // the claim row (released by the reconcile above with reason handed-over, so open claims
    // cannot be the source: that was why this was always empty, air-e7q).
    let review_waits: Vec<(String, String, i64)> = awaiting_review
        .iter()
        .flatten()
        .map(|bead| {
            let (worker, since) = ledger
                .conn()
                .query_row(
                    "SELECT worker, coalesce(first_handover_at, released_at, claimed_at) FROM claims \
                     WHERE bead=?1 ORDER BY claimed_at DESC LIMIT 1",
                    rusqlite::params![bead],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                )
                .unwrap_or_else(|_| ("?".to_string(), at.clone()));
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
    let ready_depth: Option<usize> = match bd_try(&bd, &mut bd_slow, &mut errors, "ready", |b| {
        air_bd::WorkLedger::ready(b)
    }) {
        Some(v) => {
            let ids: Vec<String> = v
                .iter()
                .filter(|i| !i.labels.iter().any(|l| l == "human"))
                .map(|i| i.id.clone())
                .collect();
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

fn render(s: &Snapshot, att: &[Attention]) -> String {
    let mut out = String::new();
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
            .collect();
        let idle_no_claim = w.role == "worker"
            && claims.is_empty()
            && w.session.as_ref().is_some_and(|x| x.state == "idle");
        out.push_str(&format!(
            "{:<12} {:<11} {}  head {} {}  files {}  claims: {}{}\n",
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
            }),
            head: Some("abc".into()),
            green_at_head: green,
            claims,
            files_held: 0,
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

    #[test]
    fn fresh_idle_worker_is_not_gone_and_dead_pid_is() {
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
        // Same row but the pid is gone: gone-with-claim, regardless of age.
        idle.session.as_mut().unwrap().pid_alive = Some(false);
        let s = Snapshot {
            workers: vec![idle],
            ..Default::default()
        };
        let a = attention(&s, NOW, Thresholds::default());
        assert_eq!(
            a.iter().map(|a| a.kind).collect::<Vec<_>>(),
            vec!["gone-with-claim"]
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
