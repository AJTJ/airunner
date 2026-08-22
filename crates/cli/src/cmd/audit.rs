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
//! It does not supply judgement"). `air selftest` asserts the rendered output contains no
//! imperative sentence, so the line cannot be crossed by accident later.
//!
//! Read-only: the event log and nothing else. No bd calls, no network, no writes.
//!
//! Removal condition for the audit itself (air-zyo): removed when two consecutive rounds
//! produce zero stale mechanisms, meaning the tree is small enough that a coordinator sees the
//! whole thing without help. An audit nobody acts on is the ritual this exists to prevent.

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
    /// For a `NoDownstream` condition: how many of the named event happened in the window.
    pub downstream: Option<(&'static str, usize)>,
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
    /// Firings this window could not attribute to any mechanism in the registry, by
    /// command/decision. A mechanism that leaves a trace but is not registered shows up here.
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
        Fires::Decision { command, decision } => {
            (e.command == command && e.decision == decision).then_some(e.command.as_str())
        }
        Fires::Condition(kind) => e.conditions.iter().find_map(|c| {
            // Entries are `kind:subject`; the subject is what makes a firing distinct.
            let rest = c.strip_prefix(kind)?.strip_prefix(':')?;
            Some(rest)
        }),
    }
}

/// A condition firing names one subject per event, but one event can carry the same kind for
/// several subjects. Count them all.
fn subjects_in<'a>(m: &Mechanism, e: &'a Ev) -> Vec<&'a str> {
    match m.fires {
        Fires::Decision { .. } => fired(m, e).into_iter().collect(),
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
    let mut downstream: BTreeMap<&'static str, usize> = BTreeMap::new();
    // The distinct commands some mechanism watches for, deduplicated.
    let watched: std::collections::BTreeSet<&'static str> = MECHANISMS
        .iter()
        .filter_map(|m| match m.removal {
            Removal::NoDownstream { downstream: d, .. } => Some(d),
            _ => None,
        })
        .collect();
    let mut seen_traces: BTreeMap<String, usize> = BTreeMap::new();
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
                // Count the event once per distinct downstream command, not once per
                // mechanism naming it: `review-waiting` and `peer-warning` both watch `land`,
                // and counting per mechanism double-counted every landing.
                if let Some(d) = watched.iter().find(|d| e.command == **d) {
                    let slot = downstream.entry(*d).or_default();
                    *slot = slot.saturating_add(1);
                }
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
            // A decision that looks like a mechanism (a warning, a refusal, a nudge) but that
            // no registry entry claims. Plain observations are not mechanisms.
            if in_window
                && !attributed
                && matches!(
                    e.decision.as_str(),
                    "refuse" | "would-refuse" | "warn" | "warn-repeat" | "nudge" | "denied"
                )
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
            let down = match m.removal {
                Removal::NoDownstream { downstream: d, .. } => {
                    Some((d, downstream.get(d).copied().unwrap_or(0)))
                }
                _ => None,
            };
            let (removal_kind, met) = match m.removal {
                Removal::Unstated => ("none", None),
                Removal::Never(_) => ("never", None),
                Removal::Judgement(_) => ("judgement", None),
                // Recorded as "remove when it stops firing": the window answers it.
                Removal::ZeroFirings(_) => ("checkable", Some(n == 0)),
                // Recorded as "remove when it fires and nothing follows": both halves must
                // hold. A mechanism that never fired does not meet a condition about firing.
                Removal::NoDownstream { .. } => {
                    ("checkable", Some(n > 0 && down.map(|(_, c)| c) == Some(0)))
                }
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
                downstream: down,
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

/// Words that would turn a fact into an instruction. Checked at the start of a sentence, so
/// "removed when a round passes" (descriptive) is fine and "Remove this" is not.
const IMPERATIVE_LEADS: &[&str] = &[
    "consider", "remove", "delete", "drop", "keep", "add", "review", "check", "run", "use", "stop",
    "prefer", "avoid", "cut", "try", "make", "ensure",
];

/// Phrases that carry a recommendation wherever they appear.
const RECOMMENDING: &[&str] = &[
    "should be",
    "we should",
    "you should",
    "recommend",
    "candidate for removal",
    "worth removing",
    "no longer earns",
    "safe to remove",
];

/// Every place `text` reads as an instruction rather than a fact. Empty is the contract
/// `air audit` holds: it supplies facts, the pass over them is a person's (air-zyo).
pub fn imperative_hits(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut hits: Vec<String> = RECOMMENDING
        .iter()
        .filter(|p| lower.contains(**p))
        .map(|p| (*p).to_string())
        .collect();
    for raw in lower.split(['.', '\n', ';']) {
        let s = raw.trim_start_matches(|c: char| !c.is_alphanumeric());
        let Some(first) = s.split_whitespace().next() else {
            continue;
        };
        if IMPERATIVE_LEADS.contains(&first) {
            hits.push(format!("sentence starts with `{first}`"));
        }
    }
    hits
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
        if let Some((d, c)) = r.downstream {
            s.push_str(&format!("  downstream `{d}` in window: {c}\n"));
        }
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
    if !a.unregistered.is_empty() {
        s.push_str("\nfirings not attributed to any registered mechanism:\n");
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
    let defects = audit.rows.iter().filter(|r| r.defect.is_some()).count();
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
            "{} mechanism(s); {met} with the recorded condition met; {defects} without one",
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
        // Nothing landed, so the recorded condition holds over this window.
        assert_eq!(rw.downstream, Some(("land", 0)));
        assert_eq!(rw.condition_met, Some(true));

        // A mechanism that never fired is in the output, not omitted.
        let nudge = a.rows.iter().find(|r| r.id == "stop-nudge").unwrap();
        assert_eq!(nudge.fires, 0);
        assert!(nudge.last_fired.is_none());
        // ...and a "fires and nothing follows" condition is NOT met by never firing.
        let owner_q = a
            .rows
            .iter()
            .find(|r| r.id == "owner-decision-waiting")
            .unwrap();
        assert_eq!(owner_q.fires, 0);
        assert_eq!(owner_q.condition_met, Some(false));

        // A mechanism with nothing recorded is reported as a defect.
        assert!(a.rows.iter().any(|r| r.defect.is_some()));

        // Facts only: the rendered output instructs nobody.
        let text = render(&a);
        assert!(
            imperative_hits(&text).is_empty(),
            "{:?}",
            imperative_hits(&text)
        );
    }

    /// A landing in the window means the recorded condition no longer holds; `last_fired`
    /// looks at every recorded day, not just the window.
    #[test]
    fn downstream_action_and_last_fired_outside_the_window() {
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
        assert_eq!(rw.downstream, Some(("land", 1)));
        assert_eq!(rw.condition_met, Some(false), "something landed");
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

    #[test]
    fn imperative_hits_catches_a_verdict_and_passes_a_fact() {
        assert_eq!(
            imperative_hits("Consider removing this condition.").len(),
            1
        );
        assert!(!imperative_hits("This should be removed").is_empty());
        assert!(imperative_hits("fired 80 times over 2 subjects").is_empty());
        assert!(
            imperative_hits("removed when a full round passes with zero events").is_empty(),
            "descriptive `removed when` is a fact, not an instruction"
        );
    }
}
