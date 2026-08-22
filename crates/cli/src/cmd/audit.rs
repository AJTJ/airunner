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
    /// Firings inside the window.
    pub fires: usize,
    /// Distinct subjects it fired about (a condition on two beads is two).
    pub subjects: usize,
    /// `fires - subjects`: firings that repeated a subject already reported.
    pub repeats: usize,
    /// Last time it fired in ANY recorded day, not just the window. `None` = never recorded.
    pub last_fired: Option<String>,
    pub removal: &'static str,
    /// `checkable` when the audit evaluated it; `judgement` when a person must; `none` when
    /// nothing was recorded; `never` when it was recorded as permanent.
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
    pub duration_ms: u64,
}

/// What one mechanism accumulated over the scan.
#[derive(Default)]
struct Acc {
    /// Firings inside the window.
    fires: usize,
    /// Distinct subjects inside the window.
    subjects: std::collections::BTreeSet<String>,
    /// Last firing in ANY recorded day, so "never fired" means never.
    last: Option<String>,
}

/// One parsed event line, reduced to what the audit counts.
struct Ev {
    at: String,
    command: String,
    decision: String,
    conditions: Vec<String>,
}

fn parse(line: &str) -> Option<Ev> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    Some(Ev {
        at: v.get("at")?.as_str()?.to_string(),
        command: v.get("command")?.as_str()?.to_string(),
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
            .then_some(e.command.as_str()),
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
                    a.fires = a.fires.saturating_add(subs.len());
                    for sub in subs {
                        a.subjects.insert(sub.to_string());
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
            let n = a.fires;
            let subs = a.subjects.len();
            let (removal_kind, met) = match m.removal {
                Removal::Unstated => ("none", None),
                Removal::Never(_) => ("never", None),
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
                fires: n,
                subjects: subs,
                repeats: n.saturating_sub(subs),
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
        duration_ms: u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX),
    }
}

/// The text form. `pub` so `air selftest` can assert on the real output rather than a
/// reconstruction of it.
pub fn render(a: &Audit) -> String {
    let mut s = format!(
        "mechanisms: {} registered; window from {} ({} day(s), {} event(s), {} ms)\n",
        a.rows.len(),
        a.since,
        a.days_scanned,
        a.events_scanned,
        a.duration_ms
    );
    for r in &a.rows {
        s.push_str(&format!(
            "\n{} [{}]  fired {} in window over {} subject(s), {} repeat(s)\n",
            r.id, r.class, r.fires, r.subjects, r.repeats
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
                        ("never", _) => "recorded as permanent",
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
    let audit = gather(ledger.dir(), &since);
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
        assert_eq!((rw.fires, rw.subjects, rw.repeats), (80, 2, 78));
        // Nothing was recorded for review-waiting, so it reads as a defect rather than
        // getting a condition invented for it.
        assert!(rw.defect.is_some());
        assert_eq!(rw.condition_met, None);

        // A mechanism that never fired is in the output, not omitted.
        let nudge = a.rows.iter().find(|r| r.id == "stop-nudge").unwrap();
        assert_eq!(nudge.fires, 0);
        assert!(nudge.last_fired.is_none());
        // A recorded "remove when it stops firing" condition is answered by the counter.
        let idle = a
            .rows
            .iter()
            .find(|r| r.id == "idle-without-claim")
            .unwrap();
        assert_eq!((idle.fires, idle.condition_met), (0, Some(true)));

        // A mechanism with nothing recorded is reported as a defect.
        assert!(a.rows.iter().any(|r| r.defect.is_some()));

        // The rendered form names the mechanism and its counts.
        let text = render(&a);
        assert!(text.contains("review-waiting"), "{text}");
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
        assert_eq!(rw.fires, 1);
        assert_eq!(rw.last_fired.as_deref(), Some("2026-08-22T01:00:00Z"));

        // A window after every recorded day: no firings counted, but last_fired still sees
        // the older ones, which is what makes "never fired" mean never.
        let older = gather_from(&days, "2026-08-23");
        let rw = older
            .rows
            .iter()
            .find(|r| r.id == "review-waiting")
            .unwrap();
        assert_eq!((rw.fires, rw.subjects), (0, 0));
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
