//! `/api/feed?since=<offset>`: Air's decisions, notices and messages, oldest first
//! (air-o6x.1, air-o6x.2). The decisions are the event stream's lines with a decision other
//! than `observed` or `quiet`, which are the hook's state changes and status's silence: 2,331
//! of one day's 2,927 lines (`docs/research/live-ui.md` §2a). The notices are `deliveries`
//! rows and the messages `messages` rows, each taken for the span of time the returned
//! decisions cover, so a page that passes `next_offset` back sees every item once.
//!
//! The offset is a position in the day files concatenated in date order, in bytes, so it
//! survives the day rolling at midnight UTC: the page keeps passing back what it was given.
//! A call returns at most [`CAP`] items; the offset then stops at the last line returned, and
//! the next call continues from there.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use air_ledger::Ledger;
use serde::Serialize;

/// The most items one call returns.
pub const CAP: usize = 200;
/// The longest text line an item carries.
const TEXT_CHARS: usize = 300;
/// How far back a lookup for the line before an offset reads.
const BACK_BYTES: u64 = 64 * 1024;

/// The feed as of `at`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Feed {
    /// When this answer was computed (UTC, RFC 3339).
    pub at: String,
    /// What to pass as `since` next time; equal to `since` when nothing is new.
    pub next_offset: u64,
    /// New items since `since`, oldest first.
    pub items: Vec<Item>,
}

/// One item.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Item {
    pub at: String,
    pub source: Source,
    /// The worker it concerns or came from.
    pub worker: String,
    /// The event's command, the delivery's kind, or `message`.
    pub kind: String,
    /// One line: the event's reason, the delivery's text, or `to <worker>: <text>`.
    pub text: String,
}

/// Where an item came from.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Event,
    Delivery,
    Message,
}

/// The `since` offset from a query string, 0 when absent or not a number.
pub fn since(query: Option<&str>) -> u64 {
    query
        .unwrap_or_default()
        .split('&')
        .find_map(|kv| kv.strip_prefix("since="))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// The feed from `since`: the decisions after that offset, then the notices and messages in
/// the span those decisions cover.
pub fn feed(ledger: Option<&Ledger>, air_dir: &Path, at: &str, since: u64) -> Feed {
    feed_capped(ledger, air_dir, at, since, CAP)
}

/// [`feed`] with the cap as a parameter, for tests.
pub fn feed_capped(
    ledger: Option<&Ledger>,
    air_dir: &Path,
    at: &str,
    since: u64,
    cap: usize,
) -> Feed {
    let files = day_files(air_dir);
    let (mut items, next_offset) = events_from(&files, since, cap);
    // The span the returned decisions cover: from the decision just before `since` (none at
    // the start) to the newest returned. Notices and messages are taken inside it, so a
    // later call, continuing from `next_offset`, never repeats one and never skips one.
    let from = last_at_before(&files, since).unwrap_or_default();
    let to = items.iter().map(|i| i.at.clone()).max();
    if let (Some(l), Some(to)) = (ledger, to) {
        let left = cap.saturating_sub(items.len());
        items.extend(deliveries(l, &from, &to, left));
        let left = cap.saturating_sub(items.len());
        items.extend(messages(l, &from, &to, left));
    }
    items.sort_by(|a, b| a.at.cmp(&b.at));
    items.truncate(cap);
    Feed {
        at: at.to_string(),
        next_offset,
        items,
    }
}

/// The day files in date order, each with its size: the names sort by date.
fn day_files(air_dir: &Path) -> Vec<(PathBuf, u64)> {
    let Ok(dir) = std::fs::read_dir(air_dir.join("events")) else {
        return Vec::new();
    };
    let mut files: BTreeMap<String, (PathBuf, u64)> = BTreeMap::new();
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".ndjson") {
            continue;
        }
        if let Ok(m) = e.metadata() {
            files.insert(name, (e.path(), m.len()));
        }
    }
    files.into_values().collect()
}

/// The decision lines from the concatenated position `since`, up to `cap` items, and the
/// position after the last line consumed. A partial last line, still being written, is left
/// for the next call.
fn events_from(files: &[(PathBuf, u64)], since: u64, cap: usize) -> (Vec<Item>, u64) {
    let mut items = Vec::new();
    let mut pos = since;
    let mut start = 0u64;
    for (path, len) in files {
        let end = start.saturating_add(*len);
        if pos >= end {
            start = end;
            continue;
        }
        let Ok(mut f) = std::fs::File::open(path) else {
            start = end;
            continue;
        };
        let within = pos.saturating_sub(start);
        if f.seek(SeekFrom::Start(within)).is_err() {
            start = end;
            continue;
        }
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_err() {
            start = end;
            continue;
        }
        let text = String::from_utf8_lossy(&buf);
        let mut consumed = 0usize;
        for line in text.split_inclusive('\n') {
            if !line.ends_with('\n') {
                break;
            }
            consumed = consumed.saturating_add(line.len());
            if let Some(item) = event_item(line) {
                items.push(item);
            }
            if items.len() >= cap {
                break;
            }
        }
        pos = pos.saturating_add(u64::try_from(consumed).unwrap_or(u64::MAX));
        if items.len() >= cap {
            break;
        }
        // The rest of this file was read whole; the next file starts at its own offset.
        start = end;
        if pos < end {
            // A partial last line: stop here and let the next call continue.
            break;
        }
    }
    (items, pos)
}

/// Pure over one event line: an item for every decision but the quiet ones.
pub fn event_item(line: &str) -> Option<Item> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let s = |k: &str| {
        v.get(k)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
    };
    if matches!(s("decision"), "observed" | "quiet") {
        return None;
    }
    let reason = s("reason");
    let first = reason.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    Some(Item {
        at: s("at").to_string(),
        source: Source::Event,
        worker: s("worker").to_string(),
        kind: s("command").to_string(),
        text: clip(first.trim(), TEXT_CHARS),
    })
}

/// The `at` of the whole line that ends at `position`, read from the tail of the file
/// holding it (up to 64 KiB back). None at the start of the stream.
fn last_at_before(files: &[(PathBuf, u64)], position: u64) -> Option<String> {
    if position == 0 {
        return None;
    }
    let mut start = 0u64;
    for (i, (path, len)) in files.iter().enumerate() {
        let end = start.saturating_add(*len);
        if position > end {
            start = end;
            continue;
        }
        let mut within = position.saturating_sub(start);
        let mut path = path.clone();
        if within == 0 {
            // At the very start of this file: the line before is the previous file's last.
            let (prev, prev_len) = files.get(i.checked_sub(1)?)?;
            path = prev.clone();
            within = *prev_len;
        }
        let mut f = std::fs::File::open(&path).ok()?;
        let from = within.saturating_sub(BACK_BYTES);
        f.seek(SeekFrom::Start(from)).ok()?;
        let mut buf = vec![0u8; usize::try_from(within.saturating_sub(from)).ok()?];
        f.read_exact(&mut buf).ok()?;
        let text = String::from_utf8_lossy(&buf);
        let line = text.lines().rev().find(|l| !l.trim().is_empty())?;
        let v: serde_json::Value = serde_json::from_str(line).ok()?;
        return v
            .get("at")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
    }
    None
}

fn limit(cap: usize) -> i64 {
    i64::try_from(cap).unwrap_or(i64::MAX)
}

fn deliveries(l: &Ledger, from: &str, to: &str, cap: usize) -> Vec<Item> {
    let Ok(mut stmt) = l.conn().prepare(
        "SELECT created_at, to_worker, kind, content FROM deliveries \
         WHERE created_at > ?1 AND created_at <= ?2 ORDER BY created_at LIMIT ?3",
    ) else {
        return Vec::new();
    };
    let rows = stmt.query_map(rusqlite::params![from, to, limit(cap)], |r| {
        Ok(Item {
            at: r.get(0)?,
            source: Source::Delivery,
            worker: r.get(1)?,
            kind: r.get(2)?,
            text: first_line(&r.get::<_, String>(3)?),
        })
    });
    match rows {
        Ok(it) => it.filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

fn messages(l: &Ledger, from: &str, to: &str, cap: usize) -> Vec<Item> {
    let Ok(mut stmt) = l.conn().prepare(
        "SELECT at, from_worker, \"to\", content FROM messages \
         WHERE at > ?1 AND at <= ?2 ORDER BY at LIMIT ?3",
    ) else {
        return Vec::new();
    };
    let rows = stmt.query_map(rusqlite::params![from, to, limit(cap)], |r| {
        let to_whom: String = r.get(2)?;
        Ok(Item {
            at: r.get(0)?,
            source: Source::Message,
            worker: r.get(1)?,
            kind: "message".to_string(),
            text: format!("to {to_whom}: {}", first_line(&r.get::<_, String>(3)?)),
        })
    });
    match rows {
        Ok(it) => it.filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

fn first_line(s: &str) -> String {
    clip(
        s.lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim(),
        TEXT_CHARS,
    )
}

fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn line(at: &str, worker: &str, command: &str, decision: &str, reason: &str) -> String {
        format!(
            r#"{{"at":"{at}","worker":"{worker}","command":"{command}","decision":"{decision}","reason":"{reason}"}}"#
        )
    }

    fn air_dir() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let air = tmp.path().join(".air");
        std::fs::create_dir_all(air.join("events")).unwrap();
        (tmp, air)
    }

    /// Observed and quiet lines are left out, an offset is honoured across the day roll, a
    /// partial last line waits, and a line that does not parse is skipped.
    #[test]
    fn the_events_feed_skips_the_quiet_lines_and_honours_the_offset() {
        let (_tmp, air) = air_dir();
        let d1 = air.join("events/2026-11-09.ndjson");
        let d2 = air.join("events/2026-11-10.ndjson");
        std::fs::write(
            &d1,
            format!(
                "{}\n{}\n{}\n",
                line(
                    "2026-11-09T10:00:00Z",
                    "w1",
                    "hook.PreToolUse",
                    "observed",
                    "x"
                ),
                line(
                    "2026-11-09T10:00:01Z",
                    "w1",
                    "claim",
                    "refuse",
                    "taken by w2\\nmore"
                ),
                "not json",
            ),
        )
        .unwrap();
        std::fs::write(
            &d2,
            format!(
                "{}\n{}\n",
                line(
                    "2026-11-10T00:00:01Z",
                    "lane",
                    "land",
                    "landed",
                    "landed w1 (zz-1)"
                ),
                line("2026-11-10T00:00:02Z", "main", "status", "quiet", ""),
            ),
        )
        .unwrap();
        let f = feed(None, &air, "now", 0);
        let kinds: Vec<(&str, &str)> = f
            .items
            .iter()
            .map(|i| (i.kind.as_str(), i.text.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [("claim", "taken by w2"), ("land", "landed w1 (zz-1)")]
        );
        assert_eq!(f.items[0].source, Source::Event);
        let total = std::fs::metadata(&d1).unwrap().len() + std::fs::metadata(&d2).unwrap().len();
        assert_eq!(f.next_offset, total);
        // Nothing new: the offset comes back unchanged, and a partial line waits.
        let again = feed(None, &air, "now", f.next_offset);
        assert!(again.items.is_empty());
        assert_eq!(again.next_offset, f.next_offset);
        let mut file = std::fs::OpenOptions::new().append(true).open(&d2).unwrap();
        use std::io::Write as _;
        write!(file, "{{\"at\":\"2026-11-10T00:00:03Z\",\"worker\":\"w2\"").unwrap();
        let partial = feed(None, &air, "now", f.next_offset);
        assert!(partial.items.is_empty());
        assert_eq!(partial.next_offset, f.next_offset);
        writeln!(
            file,
            ",\"command\":\"done\",\"decision\":\"done\",\"reason\":\"done: zz-2\"}}"
        )
        .unwrap();
        let newer = feed(None, &air, "now", f.next_offset);
        assert_eq!(newer.items.len(), 1);
        assert_eq!(newer.items[0].kind, "done");
        assert_eq!(
            newer.next_offset,
            std::fs::metadata(&d1).unwrap().len() + std::fs::metadata(&d2).unwrap().len()
        );
        // The cap stops at a whole line and the next call continues from there.
        let capped = feed_capped(None, &air, "now", 0, 1);
        assert_eq!(capped.items.len(), 1);
        assert_eq!(capped.items[0].kind, "claim");
        let rest = feed_capped(None, &air, "now", capped.next_offset, 10);
        assert_eq!(rest.items.len(), 2);
        assert_eq!(rest.items[0].kind, "land");
        assert_eq!(since(Some("since=12")), 12);
        assert_eq!(since(None), 0);
    }

    /// Notices and messages are taken for the span the returned decisions cover: in the
    /// first call up to the newest decision, in the next from there, so none repeats and
    /// none is skipped.
    #[test]
    fn notices_and_messages_ride_the_span_of_the_decisions_returned() {
        let (_tmp, air) = air_dir();
        let d = air.join("events/2026-11-10.ndjson");
        std::fs::write(
            &d,
            format!(
                "{}\n{}\n",
                line(
                    "2026-11-10T00:00:10Z",
                    "w1",
                    "claim",
                    "claimed",
                    "claimed zz-1"
                ),
                line("2026-11-10T00:00:20Z", "lane", "land", "landed", "landed"),
            ),
        )
        .unwrap();
        let l = Ledger::open_in(&air).unwrap();
        for (at, kind, text) in [
            (
                "2026-11-10T00:00:05Z",
                "beads-ready",
                "early: before any decision",
            ),
            (
                "2026-11-10T00:00:15Z",
                "batch-ready",
                "inside the first span\nsecond line",
            ),
            (
                "2026-11-10T00:00:25Z",
                "main-moved",
                "after the newest decision",
            ),
        ] {
            l.conn()
                .execute(
                    "INSERT INTO deliveries (to_worker, kind, key, subject, content, created_at) \
                     VALUES ('w1', ?1, ?1, '', ?2, ?3)",
                    rusqlite::params![kind, text, at],
                )
                .unwrap();
        }
        l.conn()
            .execute(
                "INSERT INTO messages (at, session_id, from_worker, from_role, project, \"to\", bytes, content) \
                 VALUES ('2026-11-10T00:00:12Z', 's', 'coordinator', 'coordinator', 'p', 'w1', 5, 'hold it\nplease')",
                [],
            )
            .unwrap();
        let ro = Ledger::open_read_only(&air).unwrap();
        let first = feed(Some(&ro), &air, "now", 0);
        let got: Vec<(&str, &str)> = first
            .items
            .iter()
            .map(|i| (i.kind.as_str(), i.text.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                ("beads-ready", "early: before any decision"),
                ("claim", "claimed zz-1"),
                ("message", "to w1: hold it"),
                ("batch-ready", "inside the first span"),
                ("land", "landed"),
            ]
        );
        assert_eq!(first.items[2].source, Source::Message);
        assert_eq!(first.items[2].worker, "coordinator");
        // A later decision: the next call carries the notice after the old newest and
        // nothing from before it.
        let mut file = std::fs::OpenOptions::new().append(true).open(&d).unwrap();
        use std::io::Write as _;
        writeln!(
            file,
            "{}",
            line("2026-11-10T00:00:30Z", "w1", "done", "done", "done zz-1")
        )
        .unwrap();
        let next = feed(Some(&ro), &air, "now", first.next_offset);
        let got: Vec<&str> = next.items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(got, ["after the newest decision", "done zz-1"]);
        // No new decision: no notices either, whatever the table holds.
        let none = feed(Some(&ro), &air, "now", next.next_offset);
        assert!(none.items.is_empty());
    }
}
