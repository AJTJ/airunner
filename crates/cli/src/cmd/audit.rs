//! `air audit [--since <date>]`: what the ledger says about every mechanism Air ships
//! (air-zyo).
//!
//! Three mechanisms outlived their reason in one round and all three were caught by the owner
//! noticing by hand. The `do-less` skill already prescribes the pass that would have caught
//! them ("for each hook, deny rule, attention condition, and roles.md paragraph, ask 1 and 5
//! again with the ledger open"), and it has never been run, because running it meant
//! hand-reading a day of NDJSON.
//!
//! **This command prints facts and stops.** No verdict, no score, no recommendation: the
//! judgement of what to remove belongs to the owner and the coordinator ("Air supplies facts.
//! It does not supply judgement").
//!
//! Scope was cut by the owner on 2026-08-22, after the first build, to exactly two things: the
//! registry of removal conditions, and a counter (firings in the window, last firing). A
//! "fired with no downstream action" metric was dropped as Air inferring intent it cannot
//! see — a number that looks authoritative and is not — and the finding that motivated the
//! bead turned out to be two shell commands over the NDJSON, so the expensive half was built
//! ahead of measured need. What remains is what removes the named pain: the conditions were
//! prose nobody could check.
//!
//! Read-only over the event log. No bd calls, no network. One event line per run, like every
//! other command, so the audit appears in its own output.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use super::mechanisms::{Fires, MECHANISMS, Mechanism, Removal};
use crate::cmd::{emit, open};

/// One mechanism's facts over the window. Every field is a count or a timestamp.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub id: &'static str,
    pub class: &'static str,
    pub what: &'static str,
    pub added: &'static str,
    pub source: &'static str,
    /// Times the mechanism was **evaluated to hold** inside the window. Not a count of
    /// anything anyone was told: a condition that holds while the channel polls is
    /// re-evaluated, and calling that a firing is what made `owner-decision-waiting` read as
    /// 1,685 against a single push all day (air-5uz).
    pub evaluations: usize,
    /// Distinct subjects it held for (a condition on two beads is two).
    pub subjects: usize,
    /// `evaluations - subjects`: evaluations that repeated a subject already counted.
    pub repeats: usize,
    /// Times it was actually **said to someone**: `channel.push` lines for a condition. For a
    /// decision mechanism every recorded firing is itself the act (a hook that stayed silent
    /// records a different decision word), so this equals `evaluations` there.
    pub pushes: usize,
    /// Last time it held in ANY recorded day, not just the window. `None` = never recorded.
    pub last_fired: Option<String>,
    pub removal: &'static str,
    /// `checkable` when the audit evaluated it; `judgement` when a person must; `none` when
    /// nothing was recorded. There is no permanent: a mechanism that cannot be removed is
    /// the throttle the do-less rule exists to prevent (air-s7c).
    pub removal_kind: &'static str,
    /// `Some(true)` = the ledger satisfies the recorded condition over this window.
    /// `None` = not machine-checkable, or nothing recorded.
    pub condition_met: Option<bool>,
    /// Set when the mechanism carries no recorded removal condition.
    pub defect: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Audit {
    pub since: String,
    pub days_scanned: usize,
    pub events_scanned: usize,
    pub rows: Vec<Row>,
    /// Firings this window could not attribute to any registry row, by command/decision.
    /// Reported as defects: a registry that silently omits a firing mechanism reads as
    /// complete when it is not, which is worse than no registry (air-0y9).
    pub unregistered: Vec<(String, usize)>,
    /// What the hand-over gate costs per bead, from the event log (air-2zq). None when the
    /// window recorded no verify runs.
    pub cost: Option<Cost>,
    /// What `peer-warning` changed, per warning (air-1ra).
    pub peer: PeerEffect,
    pub duration_ms: u64,
}

/// What enforcing a green costs, in the units the decision needs (air-2zq).
///
/// air-i59 made the gate blocking on evidence measured against the hand-over flow: one gate
/// per bead. air-7o3 replaced hand-over with close-with-proof, and the concern raised was that
/// the cost per bead had risen without anyone pricing it. **A mechanism justified by
/// measurement against one flow does not stay justified when the flow changes, and nothing
/// prompts the re-measurement.** So the audit reports it, and the next flow change re-reads it
/// here rather than re-deriving it.
///
/// `repeat_shas` is the number that decides whether the gate wastes work: a green recorded at
/// a HEAD that has not moved is still green, so a second close on an unchanged HEAD demands no
/// fresh verify. A high repeat count would mean it does.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Cost {
    /// `air record verify` runs in the window.
    pub verify_runs: usize,
    /// Distinct commits verified. Runs minus this is re-verification of an unchanged tree.
    pub distinct_shas: usize,
    /// Runs that re-verified a sha already verified by the same worker.
    pub repeat_runs: usize,
    /// `bd close` / `-s closed` / `-s awaiting_review` writes the gate saw pass.
    pub closes: usize,
    /// Verify runs per close. The per-bead price of enforcing a green.
    pub runs_per_close: Option<f64>,
}

/// One peer warning and what the warned session did next (air-1ra).
#[derive(Debug, Clone, Serialize)]
pub struct PeerWarning {
    pub at: String,
    pub worker: String,
    pub path: String,
    /// Edits that session journaled to the warned file AFTER the warning.
    pub edits_after: usize,
    /// `edits_after <= 1`. See `PeerEffect` for why the boundary is one and not zero.
    pub heeded: bool,
}

/// What `peer-warning` bought, as far as the record can say (air-1ra, plan 0008 item 20).
///
/// The mechanism had 33 firings and zero demonstrated effect in either direction, and the
/// absence of recorded harm was partly because the effect was not recorded. It turns out it
/// **was** recorded and never read: a `warn` line carries `session_id` and `path`, and so does
/// every `journaled` line, so "did that session edit the warned file afterwards" is a join over
/// events Air already writes. No new recording was added for this, and none is needed.
///
/// **Why the boundary is one edit and not zero.** A warning is `additionalContext` on a tool
/// call that is already happening; it cannot stop that call, whose `PostToolUse` lands after
/// the warning. So one following edit is the floor, not a choice. The distribution over
/// 2026-08-22 confirms it: 33 warnings, no warning with zero edits after, twelve with exactly
/// one, and a tail out to 44.
///
/// **This carries no threshold and no rule** — item 20's terms. `heeded` and `ignored` are
/// counts, the per-warning rows are printed so the buckets can be checked against the raw
/// record, and what to do about them is the owner's.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PeerEffect {
    pub warnings: usize,
    pub heeded: usize,
    pub ignored: usize,
    /// `None` = not recorded, and reported as such rather than as zero. Nothing in the ledger
    /// records a merge conflict or the paths it touched; `air land` is where a conflict is
    /// observed, so recording conflicted paths there is what would answer this.
    pub conflicts_in_warned_files: Option<usize>,
    pub warned: Vec<PeerWarning>,
}

/// Pure: pair each warning with the edits that session made to the warned file afterwards.
/// `warns` and `edits` are `(at, worker, session_id, path)`, in any order.
pub fn peer_effect(
    warns: &[(String, String, String, String)],
    edits: &[(String, String, String)],
) -> PeerEffect {
    let warned: Vec<PeerWarning> = warns
        .iter()
        .map(|(at, worker, sid, path)| {
            let edits_after = edits
                .iter()
                .filter(|(a, s, p)| s == sid && p == path && a > at)
                .count();
            PeerWarning {
                at: at.clone(),
                worker: worker.clone(),
                path: path.clone(),
                edits_after,
                heeded: edits_after <= 1,
            }
        })
        .collect();
    PeerEffect {
        warnings: warned.len(),
        heeded: warned.iter().filter(|w| w.heeded).count(),
        ignored: warned.iter().filter(|w| !w.heeded).count(),
        conflicts_in_warned_files: None,
        warned,
    }
}

/// What one mechanism accumulated over the scan.
#[derive(Default)]
struct Acc {
    /// Evaluations inside the window.
    evaluations: usize,
    /// Pushes inside the window (`channel.push` lines).
    pushes: usize,
    /// Distinct subjects inside the window.
    subjects: std::collections::BTreeSet<String>,
    /// Last trace in ANY recorded day, so "never fired" means never.
    last: Option<String>,
}

/// One parsed event line, reduced to what the audit counts.
struct Ev {
    at: String,
    command: String,
    decision: String,
    conditions: Vec<String>,
    /// What this firing was about, from the event's own inputs: the file, the bead, else the
    /// worker. Without it every decision-based mechanism reports exactly one subject, because
    /// the only thing left to key on is its own command name (air-s7c).
    subject: String,
    /// `inputs.session_id` and `inputs.path`, for the peer-warning join (air-1ra). Empty when
    /// the event carries neither, which is most of them.
    session_id: String,
    path: String,
    worker: String,
}

/// The most specific thing an event names. Peer warnings are per file, claim refusals per
/// bead; a mechanism with neither is at least per worker.
fn subject_of(v: &serde_json::Value) -> String {
    let inputs = v.get("inputs");
    let worker = v.get("worker").and_then(|w| w.as_str()).unwrap_or("-");
    for key in ["path", "bead"] {
        if let Some(x) = inputs.and_then(|i| i.get(key)).and_then(|x| x.as_str())
            && !x.is_empty()
        {
            // Scoped to the worker: these mechanisms are per session, so the same file
            // warned about in two worktrees is two subjects, not one repeated.
            return format!("{worker}:{x}");
        }
    }
    worker.to_string()
}

fn parse(line: &str) -> Option<Ev> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let field = |k: &str| {
        v.get("inputs")
            .and_then(|i| i.get(k))
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };
    Some(Ev {
        at: v.get("at")?.as_str()?.to_string(),
        command: v.get("command")?.as_str()?.to_string(),
        subject: subject_of(&v),
        session_id: field("session_id"),
        path: field("path"),
        worker: v
            .get("worker")
            .and_then(|w| w.as_str())
            .unwrap_or_default()
            .to_string(),
        decision: v
            .get("decision")
            .and_then(|d| d.as_str())
            .unwrap_or_default()
            .to_string(),
        conditions: v
            .get("inputs")
            .and_then(|i| i.get("conditions"))
            .and_then(|c| c.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// Does this event count as `m` firing? Returns the subject it fired about, when it did.
fn fired<'a>(m: &Mechanism, e: &'a Ev) -> Option<&'a str> {
    match m.fires {
        // Any of the mechanism's traces: one mechanism, several entry points.
        Fires::Decisions(traces) => traces
            .iter()
            .any(|(c, d)| e.command == *c && e.decision == *d)
            .then_some(e.subject.as_str()),
        Fires::Condition(kind) => e.conditions.iter().find_map(|c| {
            // Entries are `kind:subject`; the subject is what makes a firing distinct.
            let rest = c.strip_prefix(kind)?.strip_prefix(':')?;
            Some(rest)
        }),
    }
}

/// Decisions that record what happened rather than a mechanism acting on someone: an event
/// was observed, a check passed, a claim was written. Everything else is treated as a firing
/// and must have a registry row. Seeded from every decision word in the record on 2026-08-22;
/// add to it when a new one is genuinely bookkeeping, which is a deliberate act rather than
/// the default (air-0y9).
const BOOKKEEPING: &[&str] = &[
    "attention",
    "bd-refused",
    "captured",
    "claimed",
    "claimed-late",
    "clear",
    "closed",
    "dropped",
    "ended",
    "fail-open",
    "failed",
    "green",
    "journaled",
    "landed",
    "no-claim",
    "no-such-bead",
    "observed",
    "ok",
    "partial",
    "pass",
    "promoted",
    "quiet",
    "reclaimed",
    "red",
    "registered",
    "released",
    "reported",
    "stopped",
    "timeout",
    "triaged",
    // Records that a peer warning was deliberately NOT said: the suppression half of
    // `peer-warning`, and the evidence its once-per-session rule works. It acts on nobody, so
    // it is bookkeeping rather than a mechanism (air-s7c).
    "warn-repeat",
];

/// Every `command / decision` pair some mechanism claims. A firing outside this set has no
/// registry row, which the audit reports as a defect (air-0y9).
pub fn registered_traces() -> std::collections::BTreeSet<String> {
    MECHANISMS
        .iter()
        .flat_map(|m| match m.fires {
            Fires::Decisions(traces) => traces
                .iter()
                .map(|(c, d)| format!("{c} / {d}"))
                .collect::<Vec<_>>(),
            Fires::Condition(_) => Vec::new(),
        })
        .collect()
}

/// A condition firing names one subject per event, but one event can carry the same kind for
/// several subjects. Count them all.
fn subjects_in<'a>(m: &Mechanism, e: &'a Ev) -> Vec<&'a str> {
    match m.fires {
        Fires::Decisions(_) => fired(m, e).into_iter().collect(),
        Fires::Condition(kind) => e
            .conditions
            .iter()
            .filter_map(|c| c.strip_prefix(kind)?.strip_prefix(':'))
            .collect(),
    }
}

/// Every `YYYY-MM-DD.ndjson` under the events dir, oldest first.
fn event_days(air_dir: &Path) -> Vec<(String, std::path::PathBuf)> {
    let mut days: Vec<(String, std::path::PathBuf)> = std::fs::read_dir(air_dir.join("events"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let name = p
                .file_name()?
                .to_str()?
                .strip_suffix(".ndjson")?
                .to_string();
            Some((name, p))
        })
        .collect();
    days.sort();
    days
}

/// Build the audit from the event log. `since` is an inclusive `YYYY-MM-DD`.
pub fn gather(air_dir: &Path, since: &str) -> Audit {
    let days: Vec<(String, String)> = event_days(air_dir)
        .into_iter()
        .filter_map(|(day, path)| Some((day, std::fs::read_to_string(path).ok()?)))
        .collect();
    gather_from(&days, since)
}

/// The counting, over `(day, file contents)` pairs. Split out so the whole audit is testable
/// without a filesystem or a clock, which is also what lets `air selftest` assert on the real
/// rendered output rather than a reconstruction of it.
pub fn gather_from(days: &[(String, String)], since: &str) -> Audit {
    let t0 = std::time::Instant::now();

    // One accumulator per mechanism, walked by zip rather than by index: parallel vectors
    // addressed by position are how a row ends up reporting another mechanism's counts.
    let mut acc: Vec<Acc> = MECHANISMS.iter().map(|_| Acc::default()).collect();
    let mut seen_traces: BTreeMap<String, usize> = BTreeMap::new();
    let registered = registered_traces();
    let mut events_scanned = 0usize;
    let mut days_scanned = 0usize;
    // Closes the gate let through: one `pass` per bd status write it inspected (air-2zq).
    let mut closes = 0usize;
    // (at, worker, session_id, path) for the peer-warning join (air-1ra).
    let mut warns: Vec<(String, String, String, String)> = Vec::new();
    let mut edits: Vec<(String, String, String)> = Vec::new();

    for (day, text) in days {
        let in_window = day.as_str() >= since;
        if in_window {
            days_scanned = days_scanned.saturating_add(1);
        }
        for line in text.lines() {
            let Some(e) = parse(line) else { continue };
            if in_window {
                events_scanned = events_scanned.saturating_add(1);
            }
            if in_window && e.decision == "pass" && e.command == "hook.PreToolUse" {
                closes = closes.saturating_add(1);
            }
            if in_window && !e.session_id.is_empty() && !e.path.is_empty() {
                match e.decision.as_str() {
                    "warn" => warns.push((
                        e.at.clone(),
                        e.worker.clone(),
                        e.session_id.clone(),
                        e.path.clone(),
                    )),
                    "journaled" => {
                        edits.push((e.at.clone(), e.session_id.clone(), e.path.clone()));
                    }
                    _ => {}
                }
            }
            // A push carries the condition it pushed, so it attributes to the same
            // mechanism; it must never also be counted as one more evaluation of it.
            let is_push = e.command == "channel.push";
            let mut attributed = false;
            for (m, a) in MECHANISMS.iter().zip(acc.iter_mut()) {
                let subs = subjects_in(m, &e);
                if subs.is_empty() {
                    continue;
                }
                attributed = true;
                // `last_fired` spans every recorded day; the counts are the window only.
                a.last = Some(e.at.clone());
                if in_window {
                    if is_push {
                        a.pushes = a.pushes.saturating_add(subs.len());
                    } else {
                        a.evaluations = a.evaluations.saturating_add(subs.len());
                        for sub in subs {
                            a.subjects.insert(sub.to_string());
                        }
                    }
                }
            }
            // A firing no registry row claims. The test is inverted on purpose (air-0y9):
            // rather than listing the decisions that ARE mechanisms, which makes a new one
            // vanish silently, everything that is not bookkeeping counts, so a decision word
            // nobody has classified yet shows up as a defect. Loud and wrong beats quiet and
            // wrong for the failure this check exists to catch.
            if in_window
                && !attributed
                && !BOOKKEEPING.contains(&e.decision.as_str())
                && !registered.contains(&format!("{} / {}", e.command, e.decision))
            {
                let slot = seen_traces
                    .entry(format!("{} / {}", e.command, e.decision))
                    .or_default();
                *slot = slot.saturating_add(1);
            }
        }
    }

    let rows = MECHANISMS
        .iter()
        .zip(acc.iter())
        .map(|(m, a)| {
            let n = a.evaluations;
            let subs = a.subjects.len();
            // A decision mechanism acts when it fires: the hook spoke, the claim was refused.
            // Suppressed occurrences record a different decision word (`warn-repeat`), so
            // they never reach this count in the first place.
            let pushes = match m.fires {
                Fires::Decisions(_) => n,
                Fires::Condition(_) => a.pushes,
            };
            let (removal_kind, met) = match m.removal {
                Removal::Unstated => ("none", None),
                Removal::Judgement(_) => ("judgement", None),
                // "Remove when it stops firing" is answered by the counter and nothing else.
                Removal::ZeroFirings(_) => ("checkable", Some(n == 0)),
            };
            Row {
                id: m.id,
                class: m.class,
                what: m.what,
                added: m.added,
                source: m.source,
                evaluations: n,
                subjects: subs,
                repeats: n.saturating_sub(subs),
                pushes,
                last_fired: a.last.clone(),
                removal: m.removal.text(),
                removal_kind,
                condition_met: met,
                defect: matches!(m.removal, Removal::Unstated)
                    .then_some("no removal condition recorded"),
            }
        })
        .collect();

    Audit {
        since: since.to_string(),
        days_scanned,
        events_scanned,
        rows,
        unregistered: seen_traces.into_iter().collect(),
        // Filled in by `run` from the ledger's verify_runs; `gather_from` supplies the close
        // count, which is the only half the event log knows.
        cost: Some(Cost {
            closes,
            ..Cost::default()
        }),
        peer: peer_effect(&warns, &edits),
        duration_ms: u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX),
    }
}

/// What the gate cost, from the verify runs and the closes it let through (air-2zq). Pure over
/// the `(worker, sha)` pairs, so `air selftest` can assert the repeat arithmetic without a
/// ledger.
pub fn cost_of(runs: &[(String, String)], closes: usize) -> Cost {
    let distinct: std::collections::BTreeSet<&(String, String)> = runs.iter().collect();
    // A ratio of two counts. `u32::try_from` keeps the conversion lossless for any count that
    // could plausibly appear, and a window with more than 4 billion runs reports None rather
    // than a silently rounded number.
    let per = match (u32::try_from(runs.len()), u32::try_from(closes)) {
        (Ok(r), Ok(c)) if c > 0 => Some(f64::from(r) / f64::from(c)),
        _ => None,
    };
    Cost {
        verify_runs: runs.len(),
        distinct_shas: distinct.len(),
        repeat_runs: runs.len().saturating_sub(distinct.len()),
        closes,
        runs_per_close: per,
    }
}

/// The text form. `pub` so `air selftest` can assert on the real output rather than a
/// reconstruction of it.
pub fn render(a: &Audit) -> String {
    let mut s = format!(
        "mechanisms: {} registered; window from {} ({} day(s), {} event(s), {} ms)\n\
         evaluated = times the condition was found to hold; pushed = times someone was \
         actually told. They are not the same number and the gap is not a fault (air-5uz).\n",
        a.rows.len(),
        a.since,
        a.days_scanned,
        a.events_scanned,
        a.duration_ms
    );
    for r in &a.rows {
        s.push_str(&format!(
            "\n{} [{}]  evaluated {} in window over {} subject(s), {} repeat(s); pushed {}\n",
            r.id, r.class, r.evaluations, r.subjects, r.repeats, r.pushes
        ));
        s.push_str(&format!("  is: {}\n", r.what));
        s.push_str(&format!(
            "  added: {} · recorded in {}\n",
            r.added, r.source
        ));
        s.push_str(&format!(
            "  last fired: {}\n",
            r.last_fired
                .as_deref()
                .unwrap_or("never in any recorded day")
        ));
        match r.defect {
            Some(d) => s.push_str(&format!("  defect: {d}\n")),
            None => {
                s.push_str(&format!("  removed when: {}\n", r.removal));
                s.push_str(&format!(
                    "  ledger says: {}\n",
                    match (r.removal_kind, r.condition_met) {
                        ("checkable", Some(true)) =>
                            "the recorded condition holds over this window",
                        ("checkable", Some(false)) =>
                            "the recorded condition does not hold over this window",
                        ("judgement", _) => "recorded condition is not machine-checkable",
                        _ => "nothing recorded",
                    }
                ));
            }
        }
    }
    if a.unregistered.is_empty() {
        s.push_str("\nevery firing in this window has a registry row.\n");
    } else {
        s.push_str("\ndefect: firing mechanisms with no registry row, so this report does not cover them:\n");
        for (k, n) in &a.unregistered {
            s.push_str(&format!("  {n:5}  {k}\n"));
        }
    }
    if let Some(c) = &a.cost {
        s.push_str(&format!(
            "\nhand-over gate cost: {} verify run(s) over {} distinct commit(s), {} repeat(s); \
             {} close(s) passed{}\n",
            c.verify_runs,
            c.distinct_shas,
            c.repeat_runs,
            c.closes,
            match c.runs_per_close {
                Some(r) => format!(" = {r:.2} verify run(s) per close"),
                None => String::new(),
            }
        ));
        s.push_str(
            "  repeats are the waste to watch: a green at a HEAD that has not moved still \
             counts, so a second close on an unchanged commit demands no fresh verify.\n",
        );
    }
    s.push_str(&render_peer(&a.peer));
    s
}

/// The peer-warning section (air-1ra). Its own function because the definitions have to travel
/// with the numbers: a bucket whose boundary is not printed beside it is the derived-reads-like-
/// observed failure again.
fn render_peer(p: &PeerEffect) -> String {
    if p.warnings == 0 {
        return "\npeer-warning effect: no warnings in this window.\n".to_string();
    }
    let mut s = format!(
        "\npeer-warning effect: {} warning(s); {} heeded, {} ignored\n",
        p.warnings, p.heeded, p.ignored
    );
    s.push_str(
        "  heeded = the warned session made no further edit to that file beyond the one \
         already in flight. A warning is additionalContext on a tool call and cannot stop \
         that call, so one following edit is the floor, not a choice.\n",
    );
    s.push_str(&format!(
        "  conflicts in warned files: {}\n",
        match p.conflicts_in_warned_files {
            Some(n) => n.to_string(),
            None => "not recorded. Nothing in the ledger records a merge conflict or the paths \
                     it touched; `air land` is where one is observed, so recording conflicted \
                     paths there is what would answer this."
                .to_string(),
        }
    ));
    for w in &p.warned {
        s.push_str(&format!(
            "  {} {:<12} {:<44} {} edit(s) after: {}\n",
            w.at.get(..19).unwrap_or(&w.at),
            w.worker,
            w.path,
            w.edits_after,
            if w.heeded { "heeded" } else { "IGNORED" }
        ));
    }
    s
}

pub fn run(repo: &Path, since: Option<&str>, json: bool) -> i32 {
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air audit: {e}");
            return 1;
        }
    };
    let since = since.map(str::to_string).unwrap_or_else(super::today);
    let mut audit = gather(ledger.dir(), &since);
    // The other half of the cost: what the gate made workers run. The event log knows the
    // closes; the verify runs are the ledger's (air-2zq).
    let runs: Vec<(String, String)> = ledger
        .conn()
        .prepare(
            "SELECT worker, sha FROM verify_runs WHERE kind='verify' AND started_at >= ?1 \
             ORDER BY started_at",
        )
        .and_then(|mut st| {
            st.query_map(rusqlite::params![since], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })
        .unwrap_or_default();
    audit.cost = Some(cost_of(&runs, audit.cost.as_ref().map_or(0, |c| c.closes)));
    let defects = audit
        .rows
        .iter()
        .filter(|r| r.defect.is_some())
        .count()
        .saturating_add(audit.unregistered.len());
    let met = audit
        .rows
        .iter()
        .filter(|r| r.condition_met == Some(true))
        .count();
    super::log_event(
        &ledger,
        &worker,
        "audit",
        &serde_json::json!({"since": since}),
        "reported",
        &format!(
            "{} mechanism(s); {met} with the recorded condition met; {defects} defect(s) (no condition recorded, or firing with no registry row)",
            audit.rows.len()
        ),
        &format!(
            "{} event(s) over {} day(s)",
            audit.events_scanned, audit.days_scanned
        ),
    );
    emit(json, &audit, || render(&audit));
    0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn day(name: &str, lines: &[&str]) -> (String, String) {
        (name.to_string(), lines.join("\n") + "\n")
    }

    /// The round this bead came from: review-waiting fired on many beads, over and over, and
    /// nothing landed. The audit reproduces that ratio from the event log alone.
    #[test]
    fn reproduces_the_review_waiting_ratio_and_finds_never_fired() {
        let mut lines: Vec<String> = Vec::new();
        for i in 0..40 {
            lines.push(format!(
                r#"{{"at":"2026-08-22T0{}:00:00Z","worker":"main","command":"status.attention","inputs":{{"conditions":["review-waiting:air-i59","review-waiting:air-3eu"]}},"decision":"attention"}}"#,
                i % 10
            ));
        }
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let a = gather_from(&[day("2026-08-22", &refs)], "2026-08-22");

        let rw = a.rows.iter().find(|r| r.id == "review-waiting").unwrap();
        // 40 events x 2 beads = 80 firings about 2 subjects: 78 of them repeats.
        assert_eq!((rw.evaluations, rw.subjects, rw.repeats), (80, 2, 78));
        // air-s7c recorded a condition for it, and it needs a person: whether a push led to
        // an action is not something the ledger can see.
        assert!(rw.defect.is_none());
        assert_eq!((rw.removal_kind, rw.condition_met), ("judgement", None));

        // A mechanism that never fired is in the output, not omitted.
        let nudge = a.rows.iter().find(|r| r.id == "stop-nudge").unwrap();
        assert_eq!(nudge.evaluations, 0);
        assert!(nudge.last_fired.is_none());
        // A recorded "remove when it stops firing" condition is answered by the counter.
        let idle = a
            .rows
            .iter()
            .find(|r| r.id == "idle-without-claim")
            .unwrap();
        assert_eq!((idle.evaluations, idle.condition_met), (0, Some(true)));

        // A mechanism with nothing recorded is still reported as a defect (`stuck`, which
        // the 2026-08-22 pass deliberately left out of scope).
        assert!(a.rows.iter().any(|r| r.id == "stuck" && r.defect.is_some()));

        // The rendered form names the mechanism and its counts.
        let text = render(&a);
        assert!(text.contains("review-waiting"), "{text}");
    }

    /// air-5uz: a push and an evaluation are different facts and are counted apart. The
    /// number that nearly got `owner-decision-waiting` deleted was 1,685 evaluations read as
    /// 1,685 firings, against one push all day.
    #[test]
    fn a_push_is_counted_as_a_push_and_never_as_one_more_evaluation() {
        let a = gather_from(
            &[day(
                "2026-08-22",
                &[
                    r#"{"at":"2026-08-22T01:00:00Z","worker":"main","command":"status.attention","inputs":{"conditions":["owner-decision-waiting:owner"]},"decision":"attention"}"#,
                    r#"{"at":"2026-08-22T01:00:01Z","worker":"main","command":"channel.push","inputs":{"conditions":["owner-decision-waiting:owner"],"for_minutes":5},"decision":"pushed"}"#,
                    r#"{"at":"2026-08-22T02:00:00Z","worker":"main","command":"status.attention","inputs":{"conditions":["owner-decision-waiting:owner"]},"decision":"attention"}"#,
                ],
            )],
            "2026-08-22",
        );
        let r = a
            .rows
            .iter()
            .find(|r| r.id == "owner-decision-waiting")
            .unwrap();
        assert_eq!((r.evaluations, r.pushes), (2, 1));
        // And the push does not read as a mechanism nobody registered.
        assert!(a.unregistered.is_empty(), "{:?}", a.unregistered);
        let text = render(&a);
        assert!(text.contains("evaluated 2"), "{text}");
        assert!(text.contains("pushed 1"), "{text}");
    }

    /// A decision mechanism acts when it fires, so the two counts agree there rather than
    /// reporting a hook that spoke as never having reached anyone.
    #[test]
    fn a_decision_mechanism_pushes_every_time_it_fires() {
        let a = gather_from(
            &[day(
                "2026-08-22",
                &[
                    r#"{"at":"2026-08-22T01:00:00Z","worker":"alpha","command":"hook.PreToolUse","inputs":{"path":"a.rs"},"decision":"warn"}"#,
                ],
            )],
            "2026-08-22",
        );
        let r = a.rows.iter().find(|r| r.id == "peer-warning").unwrap();
        assert_eq!((r.evaluations, r.pushes), (1, 1));
    }

    /// A landing in the window means the recorded condition no longer holds; `last_fired`
    /// looks at every recorded day, not just the window.
    #[test]
    fn last_fired_looks_outside_the_window() {
        let days = [
            day(
                "2026-08-20",
                &[
                    r#"{"at":"2026-08-20T01:00:00Z","worker":"main","command":"status","inputs":{"conditions":["review-waiting:air-1"]},"decision":"attention"}"#,
                ],
            ),
            day(
                "2026-08-22",
                &[
                    r#"{"at":"2026-08-22T01:00:00Z","worker":"main","command":"status","inputs":{"conditions":["review-waiting:air-2"]},"decision":"attention"}"#,
                    r#"{"at":"2026-08-22T02:00:00Z","worker":"main","command":"land","decision":"landed"}"#,
                ],
            ),
        ];
        let a = gather_from(&days, "2026-08-22");
        let rw = a.rows.iter().find(|r| r.id == "review-waiting").unwrap();
        assert_eq!(rw.evaluations, 1);
        assert_eq!(rw.last_fired.as_deref(), Some("2026-08-22T01:00:00Z"));

        // A window after every recorded day: no firings counted, but last_fired still sees
        // the older ones, which is what makes "never fired" mean never.
        let older = gather_from(&days, "2026-08-23");
        let rw = older
            .rows
            .iter()
            .find(|r| r.id == "review-waiting")
            .unwrap();
        assert_eq!((rw.evaluations, rw.subjects), (0, 0));
        assert_eq!(rw.last_fired.as_deref(), Some("2026-08-22T01:00:00Z"));
    }

    /// A firing that looks like a mechanism but is in no registry entry is surfaced, so the
    /// registry going stale is visible rather than silent.
    #[test]
    fn unregistered_firings_are_reported() {
        let a = gather_from(
            &[day(
                "2026-08-22",
                &[
                    r#"{"at":"2026-08-22T01:00:00Z","worker":"beta","command":"hook.Stop","decision":"refuse"}"#,
                    r#"{"at":"2026-08-22T01:00:01Z","worker":"beta","command":"hook.PostToolUse","decision":"observed"}"#,
                ],
            )],
            "2026-08-22",
        );
        assert_eq!(a.unregistered, vec![("hook.Stop / refuse".to_string(), 1)]);
    }
}
