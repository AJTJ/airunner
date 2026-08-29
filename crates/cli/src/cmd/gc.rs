//! `air gc [--keep-days <n>] [--apply]`: a stated retention for `.air/events/` (air-i7s).
//!
//! The stream is one line per hook invocation and per command, and nothing has ever collected
//! it. It stood at 14.19 MB across 8 files on 2026-08-29.
//!
//! **It is not truncation, and the reason matters.** The raw event stream is the only artefact
//! that has caught the audit's own errors: alpha's air-okc pass counted from
//! `.air/events/*.ndjson` directly and found `air audit`'s fires metric wrong, and 0007 §11
//! records that re-reading the document found nothing while re-running the commands found
//! three errors. Deleting history cheaply would remove the one thing that can audit Air. So
//! this refuses to touch a day the ledger still points at, it says what it would do and does
//! nothing until told, and there is no automatic path.
//!
//! **Removal**: when `events::append` bounds the stream itself, or when nothing reads a day
//! older than the window — at which point the window is the thing to delete, not this command.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::cmd::{emit, open};

/// Days of event history kept by default.
///
/// Chosen against the rate **after** air-5uz, which is the whole point of the dependency:
/// 0.37 MB per active day, where the pre-fix figure was 2.77 MB (0007 §3). Ninety days of that
/// is about 33 MB, which is small enough that the window can be generous and the audit trail
/// stays long enough to re-derive a round. Against the pre-fix rate the same window would have
/// been 250 MB, which is how a number copied from before a fix silently becomes the wrong one.
pub const KEEP_DAYS: i64 = 90;

/// One day file and what is to be done with it.
#[derive(Debug, Clone, Serialize)]
pub struct Day {
    pub day: String,
    pub bytes: u64,
    /// `None` = collectable. `Some(reason)` = kept, and why.
    pub kept: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub keep_days: i64,
    /// Days strictly older than this are candidates.
    pub cutoff: String,
    pub applied: bool,
    pub days: Vec<Day>,
    pub collectable_bytes: u64,
    pub total_bytes: u64,
}

/// The date `keep_days` before `today`, as `YYYY-MM-DD`. A day is a candidate when it sorts
/// strictly before this.
pub fn cutoff_day(today: &str, keep_days: i64) -> String {
    // Hours, not days: jiff refuses calendar units on a bare `Timestamp`, because a day is
    // not a fixed length once a calendar is involved. Written with `.days()` first, which
    // compiled, always errored, and so collected nothing — the safe direction, and useless.
    // UTC has no DST, so 24 h is exactly a day here.
    let parsed = format!("{today}T00:00:00Z").parse::<jiff::Timestamp>();
    let back = jiff::Span::new().hours(keep_days.saturating_mul(24));
    match parsed.and_then(|t| t.checked_sub(back)) {
        Ok(t) => t.to_string().get(..10).unwrap_or(today).to_string(),
        // A clock or a span we cannot reason about collects nothing, rather than collecting
        // everything: this command's failure direction has to be "kept too much".
        Err(_) => "0000-00-00".to_string(),
    }
}

/// Which days the ledger still points at. A day is referenced when a landing, a verify run, an
/// open condition or an open claim is dated inside it — the rows whose raw corroboration is
/// the reason to keep the stream at all.
pub fn referenced_days(conn: &rusqlite::Connection) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for sql in [
        "SELECT substr(started_at,1,10) FROM landings",
        "SELECT substr(finished_at,1,10) FROM landings",
        "SELECT substr(started_at,1,10) FROM verify_runs",
        "SELECT substr(first_seen,1,10) FROM conditions WHERE cleared_at IS NULL",
        "SELECT substr(claimed_at,1,10) FROM claims WHERE released_at IS NULL",
    ] {
        let rows: Vec<String> = conn
            .prepare(sql)
            .and_then(|mut st| st.query_map([], |r| r.get(0))?.collect())
            .unwrap_or_default();
        out.extend(rows);
    }
    out
}

/// Pure: what `gc` would do. Split out so the probe asserts on the decision rather than on a
/// filesystem.
pub fn plan(
    days: &[(String, u64)],
    today: &str,
    keep_days: i64,
    referenced: &BTreeSet<String>,
) -> Plan {
    let cutoff = cutoff_day(today, keep_days);
    let days: Vec<Day> = days
        .iter()
        .map(|(day, bytes)| Day {
            day: day.clone(),
            bytes: *bytes,
            kept: if day.as_str() >= cutoff.as_str() {
                Some("inside the retention window")
            } else if referenced.contains(day) {
                Some("the ledger still points at this day")
            } else {
                None
            },
        })
        .collect();
    Plan {
        keep_days,
        cutoff,
        applied: false,
        collectable_bytes: days
            .iter()
            .filter(|d| d.kept.is_none())
            .map(|d| d.bytes)
            .sum(),
        total_bytes: days.iter().map(|d| d.bytes).sum(),
        days,
    }
}

/// Every `YYYY-MM-DD.ndjson` under the events dir with its size, oldest first.
pub fn event_days(air_dir: &Path) -> Vec<(String, u64)> {
    let mut days: Vec<(String, u64)> = std::fs::read_dir(air_dir.join("events"))
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
            Some((name, e.metadata().ok().map_or(0, |m| m.len())))
        })
        .collect();
    days.sort();
    days
}

pub fn run(repo: &Path, keep_days: Option<i64>, apply: bool, json: bool) -> i32 {
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air gc: {e}");
            return 1;
        }
    };
    let keep_days = keep_days.unwrap_or(KEEP_DAYS);
    let mut plan = plan(
        &event_days(ledger.dir()),
        &super::today(),
        keep_days,
        &referenced_days(ledger.conn()),
    );
    let mut removed = 0u64;
    if apply {
        for d in plan.days.iter().filter(|d| d.kept.is_none()) {
            let p = ledger
                .dir()
                .join("events")
                .join(format!("{}.ndjson", d.day));
            match std::fs::remove_file(&p) {
                Ok(()) => removed = removed.saturating_add(d.bytes),
                Err(e) => eprintln!("air gc: {}: {e}", p.display()),
            }
        }
        plan.applied = true;
    }
    super::log_event(
        &ledger,
        &worker,
        "gc",
        &serde_json::json!({"keep_days": keep_days, "cutoff": plan.cutoff, "apply": apply}),
        if apply { "collected" } else { "reported" },
        &format!(
            "{} day(s), {} byte(s) collectable; {}",
            plan.days.iter().filter(|d| d.kept.is_none()).count(),
            plan.collectable_bytes,
            if apply {
                format!("{removed} byte(s) removed")
            } else {
                "nothing removed (no --apply)".to_string()
            }
        ),
        &format!("{} day(s) on disk", plan.days.len()),
    );
    emit(json, &plan, || render(&plan));
    0
}

pub fn render(p: &Plan) -> String {
    let mut s = format!(
        "retention: {} day(s); days before {} are collectable ({} day(s) on disk, {} bytes)\n",
        p.keep_days,
        p.cutoff,
        p.days.len(),
        p.total_bytes
    );
    for d in &p.days {
        s.push_str(&format!(
            "  {} {:>10} bytes  {}\n",
            d.day,
            d.bytes,
            match d.kept {
                Some(why) => format!("keep: {why}"),
                None => "COLLECT".to_string(),
            }
        ));
    }
    s.push_str(&format!(
        "\n{} byte(s) collectable. {}\n",
        p.collectable_bytes,
        if p.applied {
            "removed."
        } else {
            "Nothing was removed: re-run with --apply. There is no automatic path, because the \
             raw stream is the only artefact that has caught the audit's own errors."
        }
    ));
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn refs(days: &[&str]) -> BTreeSet<String> {
        days.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn the_window_is_counted_back_from_today() {
        assert_eq!(cutoff_day("2026-08-29", 90), "2026-05-31");
        assert_eq!(cutoff_day("2026-08-29", 1), "2026-08-28");
    }

    /// A clock or span we cannot reason about must keep everything. The failure direction of a
    /// collector is the whole safety argument (`anti-brittleness`).
    #[test]
    fn an_unreadable_today_collects_nothing() {
        let p = plan(
            &[("2020-01-01".into(), 10)],
            "not-a-date",
            90,
            &BTreeSet::new(),
        );
        assert_eq!(p.collectable_bytes, 0);
        assert_eq!(p.days[0].kept, Some("inside the retention window"));
    }

    #[test]
    fn a_day_the_ledger_points_at_is_kept_however_old() {
        let days = [
            ("2026-01-01".to_string(), 100),
            ("2026-01-02".to_string(), 200),
            ("2026-08-29".to_string(), 400),
        ];
        let p = plan(&days, "2026-08-29", 90, &refs(&["2026-01-02"]));
        assert_eq!(p.days[0].kept, None, "old and unreferenced: collectable");
        assert_eq!(p.days[1].kept, Some("the ledger still points at this day"));
        assert_eq!(p.days[2].kept, Some("inside the retention window"));
        assert_eq!(p.collectable_bytes, 100);
        assert_eq!(p.total_bytes, 700);
    }
}
