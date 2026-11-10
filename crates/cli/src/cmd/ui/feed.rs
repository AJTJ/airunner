//! `/api/feed?since=<offset>`: Air's decisions, notices and messages, oldest first
//! (air-o6x.1, air-o6x.2, air-o6x.6). The decisions are the event stream's lines with a
//! decision other than `observed` or `quiet`, which are the hook's state changes and status's
//! silence: 2,331 of one day's 2,927 lines (`docs/research/live-ui.md` §2a). The notices are
//! `deliveries` rows and the messages `messages` rows, each taken for the span of time the
//! returned decisions cover, so a page that passes `next_offset` back sees every item once.
//!
//! The page is live, not an archive: with no offset the answer is the newest [`CAP`] items of
//! today's file, and the offset it returns is day-scoped, `<day>:<byte position>`, so a later
//! call reads the rest of that day and then each newer day whole. The first build read the
//! whole history from its oldest file and a first load showed a week-old slice as "newest"
//! (air-o6x.6). A call returns at most [`CAP`] items and its offset stops at the last line it
//! consumed; a partial last line, still being written, waits for the next call.

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
    /// What to pass as `since` next time: `<day>:<byte position>` in that day's event file.
    /// The same text comes back when nothing is new.
    pub next_offset: String,
    /// The items, oldest first: with no offset the newest of today, else what came after it.
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

/// Where a call continues: a day's event file and a byte position in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offset {
    /// The day, `YYYY-MM-DD`, naming `<air_dir>/events/<day>.ndjson`.
    pub day: String,
    /// The byte position in that file, at a line start.
    pub pos: u64,
}

impl Offset {
    /// The text a page passes back.
    pub fn text(&self) -> String {
        format!("{}:{}", self.day, self.pos)
    }

    /// Pure over `<day>:<n>`; anything else is no offset.
    pub fn parse(s: &str) -> Option<Self> {
        let (day, n) = s.split_once(':')?;
        let shaped = day.len() == 10
            && day.bytes().enumerate().all(|(i, b)| {
                if i == 4 || i == 7 {
                    b == b'-'
                } else {
                    b.is_ascii_digit()
                }
            });
        if !shaped {
            return None;
        }
        Some(Self {
            day: day.to_string(),
            pos: n.parse().ok()?,
        })
    }
}

/// The `since` offset from a query string, percent-decoded. None (tail today) when absent,
/// `0`, or not a `<day>:<n>`.
pub fn since(query: Option<&str>) -> Option<Offset> {
    query
        .unwrap_or_default()
        .split('&')
        .find_map(|kv| kv.strip_prefix("since="))
        .map(percent_decode)
        .and_then(|s| Offset::parse(&s))
}

/// `%XX` and `+` undone, as a browser's `encodeURIComponent` wrote them.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        let hex = |j: usize| bytes.get(j).and_then(|c| char::from(*c).to_digit(16));
        match (b, hex(i.saturating_add(1)), hex(i.saturating_add(2))) {
            (b'%', Some(h), Some(l)) => {
                out.push(u8::try_from(h.saturating_mul(16).saturating_add(l)).unwrap_or(b'?'));
                i = i.saturating_add(3);
            }
            (b'+', _, _) => {
                out.push(b' ');
                i = i.saturating_add(1);
            }
            _ => {
                out.push(b);
                i = i.saturating_add(1);
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One day's event file.
struct DayFile {
    day: String,
    path: PathBuf,
    len: u64,
}

/// The feed: the newest items of today with no offset, else what came after the offset.
pub fn feed(ledger: Option<&Ledger>, air_dir: &Path, at: &str, since: Option<Offset>) -> Feed {
    feed_capped(ledger, air_dir, at, since, CAP)
}

/// [`feed`] with the cap as a parameter, for tests.
pub fn feed_capped(
    ledger: Option<&Ledger>,
    air_dir: &Path,
    at: &str,
    since: Option<Offset>,
    cap: usize,
) -> Feed {
    let files = day_files(air_dir);
    let today = crate::cmd::today();
    let first_load = since.is_none();
    let (mut items, next, from) = match since {
        None => tail_today(&files, &today),
        Some(off) => {
            let from = last_at_before(&files, &off);
            let (items, next) = events_after(&files, &off, cap);
            (items, next, from)
        }
    };
    // The span the returned decisions cover: from the decision just before the first one
    // returned (none at the start of history) to the newest returned. Notices and messages
    // are taken inside it, so a later call, continuing from `next_offset`, never repeats one
    // and never skips one.
    let from = from.unwrap_or_default();
    let to = items.iter().map(|i| i.at.clone()).max();
    if let (Some(l), Some(to)) = (ledger, to) {
        items.extend(deliveries(l, &from, &to, cap));
        items.extend(messages(l, &from, &to, cap));
    }
    items.sort_by(|a, b| a.at.cmp(&b.at));
    // A first load keeps the newest `cap` of the merged set; its offset is the end of today,
    // so nothing dropped is lost to a later call. A continuation keeps everything it read,
    // since its offset has moved past those lines: at most `cap` decisions, and the notices
    // and messages of their span.
    if first_load {
        let drop = items.len().saturating_sub(cap);
        items.drain(..drop);
    }
    Feed {
        at: at.to_string(),
        next_offset: next.text(),
        items,
    }
}

/// The day files in date order, each with its size: the names sort by date.
fn day_files(air_dir: &Path) -> Vec<DayFile> {
    let Ok(dir) = std::fs::read_dir(air_dir.join("events")) else {
        return Vec::new();
    };
    let mut files: BTreeMap<String, DayFile> = BTreeMap::new();
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(day) = name.strip_suffix(".ndjson") else {
            continue;
        };
        if let Ok(m) = e.metadata() {
            files.insert(
                day.to_string(),
                DayFile {
                    day: day.to_string(),
                    path: e.path(),
                    len: m.len(),
                },
            );
        }
    }
    files.into_values().collect()
}

/// One file's whole lines from `pos`: each item with the byte position of its line's start,
/// and the position after the last whole line.
fn lines_from(path: &Path, pos: u64) -> (Vec<(u64, Option<Item>, String)>, u64) {
    let mut out = Vec::new();
    let Ok(mut f) = std::fs::File::open(path) else {
        return (out, pos);
    };
    if f.seek(SeekFrom::Start(pos)).is_err() {
        return (out, pos);
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return (out, pos);
    }
    let text = String::from_utf8_lossy(&buf);
    let mut at = pos;
    for line in text.split_inclusive('\n') {
        if !line.ends_with('\n') {
            break;
        }
        let line_at = serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|v| {
                v.get("at")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_default();
        out.push((at, event_item(line), line_at));
        at = at.saturating_add(u64::try_from(line.len()).unwrap_or(u64::MAX));
    }
    (out, at)
}

/// Every decision of today, the offset at the end of today's file, and the `at` of the last
/// line of the day before, which bounds the notices a first load carries. No file for today:
/// nothing, and today at 0.
fn tail_today(files: &[DayFile], today: &str) -> (Vec<Item>, Offset, Option<String>) {
    let start = Offset {
        day: today.to_string(),
        pos: 0,
    };
    let Some(file) = files.iter().find(|f| f.day == today) else {
        return (Vec::new(), start, None);
    };
    let (lines, end) = lines_from(&file.path, 0);
    let items = lines.into_iter().filter_map(|(_, item, _)| item).collect();
    let from = last_at_before(files, &start);
    (
        items,
        Offset {
            day: today.to_string(),
            pos: end,
        },
        from,
    )
}

/// The decisions after `off`: the rest of its day, then each newer day whole, up to `cap`;
/// and the offset to continue from. A day whose file is gone is skipped.
fn events_after(files: &[DayFile], off: &Offset, cap: usize) -> (Vec<Item>, Offset) {
    let mut items = Vec::new();
    let mut next = off.clone();
    for file in files.iter().filter(|f| f.day.as_str() >= off.day.as_str()) {
        let start = if file.day == off.day { off.pos } else { 0 };
        let (lines, end) = lines_from(&file.path, start);
        let mut consumed = start;
        let mut full = false;
        for (at, item, _) in lines {
            if items.len() >= cap {
                full = true;
                consumed = at;
                break;
            }
            if let Some(i) = item {
                items.push(i);
            }
        }
        if full {
            next = Offset {
                day: file.day.clone(),
                pos: consumed,
            };
            return (items, next);
        }
        next = Offset {
            day: file.day.clone(),
            pos: end,
        };
        if end < file.len {
            // A partial last line: stop here and let the next call continue.
            break;
        }
    }
    (items, next)
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

/// The `at` of the whole line that ends at `off`, read from the tail of its file (up to 64 KiB
/// back), or the last line of the day before when `off` is at a file's start. None at the
/// start of history.
fn last_at_before(files: &[DayFile], off: &Offset) -> Option<String> {
    let (path, within) = if off.pos > 0 {
        let f = files.iter().find(|f| f.day == off.day)?;
        (f.path.clone(), off.pos)
    } else {
        let prev = files
            .iter()
            .rev()
            .find(|f| f.day.as_str() < off.day.as_str())?;
        (prev.path.clone(), prev.len)
    };
    let mut f = std::fs::File::open(&path).ok()?;
    let from = within.saturating_sub(BACK_BYTES);
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut buf = vec![0u8; usize::try_from(within.saturating_sub(from)).ok()?];
    f.read_exact(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    let line = text.lines().rev().find(|l| !l.trim().is_empty())?;
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    v.get("at")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn limit(cap: usize) -> i64 {
    i64::try_from(cap).unwrap_or(i64::MAX)
}

fn deliveries(l: &Ledger, from: &str, to: &str, cap: usize) -> Vec<Item> {
    let Ok(mut stmt) = l.conn().prepare(
        "SELECT created_at, to_worker, kind, content FROM deliveries \
         WHERE created_at > ?1 AND created_at <= ?2 ORDER BY created_at DESC LIMIT ?3",
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
         WHERE at > ?1 AND at <= ?2 ORDER BY at DESC LIMIT ?3",
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

    fn kinds(f: &Feed) -> Vec<(String, String)> {
        f.items
            .iter()
            .map(|i| (i.kind.clone(), i.text.clone()))
            .collect()
    }

    /// No offset tails today, newest first-load, and skips observed and quiet lines; the
    /// offset passed back returns only what was appended; an offset from yesterday returns
    /// yesterday's remainder and all of today; no file for today gives nothing at today:0.
    #[test]
    fn no_offset_tails_today_and_an_offset_continues_across_the_day_roll() {
        let (_tmp, air) = air_dir();
        let today = crate::cmd::today();
        let y = air.join("events/2000-01-01.ndjson");
        let t = air.join(format!("events/{today}.ndjson"));
        // No file for today yet.
        let f = feed(None, &air, "now", None);
        assert!(f.items.is_empty());
        assert_eq!(f.next_offset, format!("{today}:0"));
        let y1 = line("2000-01-01T10:00:00Z", "w1", "claim", "claimed", "y one");
        let y2 = line(
            "2000-01-01T11:00:00Z",
            "w1",
            "hook.PreToolUse",
            "observed",
            "x",
        );
        let y3 = line("2000-01-01T12:00:00Z", "w1", "done", "done", "y three");
        std::fs::write(&y, format!("{y1}\n{y2}\n{y3}\n")).unwrap();
        std::fs::write(
            &t,
            format!(
                "{}\n{}\n{}\nnot json\n{}\n",
                line(
                    "2026-11-10T00:00:01Z",
                    "w1",
                    "claim",
                    "refuse",
                    "t one\\nmore"
                ),
                line("2026-11-10T00:00:02Z", "main", "status", "quiet", ""),
                line("2026-11-10T00:00:03Z", "lane", "land", "landed", "t two"),
                line(
                    "2026-11-10T00:00:04Z",
                    "w2",
                    "capture",
                    "captured",
                    "t three"
                ),
            ),
        )
        .unwrap();
        let f = feed(None, &air, "now", None);
        assert_eq!(
            kinds(&f),
            [
                ("claim".to_string(), "t one".to_string()),
                ("land".into(), "t two".into()),
                ("capture".into(), "t three".into())
            ],
            "only today, no observed, quiet or unparsable line"
        );
        let t_len = std::fs::metadata(&t).unwrap().len();
        assert_eq!(f.next_offset, format!("{today}:{t_len}"));
        // The newest `cap` of today.
        let two = feed_capped(None, &air, "now", None, 2);
        assert_eq!(
            kinds(&two),
            [
                ("land".to_string(), "t two".to_string()),
                ("capture".into(), "t three".into())
            ]
        );
        assert_eq!(two.next_offset, f.next_offset);
        // Nothing new: the same offset back; a partial line waits; a whole one arrives.
        let off = Offset::parse(&f.next_offset).unwrap();
        let again = feed(None, &air, "now", Some(off.clone()));
        assert!(again.items.is_empty());
        assert_eq!(again.next_offset, f.next_offset);
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new().append(true).open(&t).unwrap();
        write!(file, "{{\"at\":\"2026-11-10T00:00:05Z\",\"worker\":\"w2\"").unwrap();
        let partial = feed(None, &air, "now", Some(off.clone()));
        assert!(partial.items.is_empty());
        assert_eq!(partial.next_offset, f.next_offset);
        writeln!(
            file,
            ",\"command\":\"done\",\"decision\":\"done\",\"reason\":\"t four\"}}"
        )
        .unwrap();
        let newer = feed(None, &air, "now", Some(off));
        assert_eq!(kinds(&newer), [("done".to_string(), "t four".to_string())]);
        assert_eq!(
            newer.next_offset,
            format!("{today}:{}", std::fs::metadata(&t).unwrap().len())
        );
        // An offset from yesterday, after its first line: the rest of yesterday, then today.
        let from_y = Offset {
            day: "2000-01-01".into(),
            pos: u64::try_from(y1.len() + 1).unwrap(),
        };
        let rest = feed(None, &air, "now", Some(from_y.clone()));
        let got: Vec<&str> = rest.items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(got, ["y three", "t one", "t two", "t three", "t four"]);
        assert!(rest.next_offset.starts_with(&format!("{today}:")));
        // The cap stops at a whole line, here at the start of today since yesterday was read
        // out, and the next call continues there.
        let capped = feed_capped(None, &air, "now", Some(from_y), 1);
        assert_eq!(
            kinds(&capped),
            [("done".to_string(), "y three".to_string())]
        );
        assert_eq!(capped.next_offset, format!("{today}:0"));
        let then = feed(None, &air, "now", Offset::parse(&capped.next_offset));
        let got: Vec<&str> = then.items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(got, ["t one", "t two", "t three", "t four"]);
        // The query: absent, 0, a bare number and junk all tail today; the page's encoding
        // of the colon is undone.
        assert_eq!(since(None), None);
        assert_eq!(since(Some("since=0")), None);
        assert_eq!(since(Some("since=12")), None);
        assert_eq!(since(Some("since=bad:1")), None);
        assert_eq!(
            since(Some("x=1&since=2026-11-10%3A544163")),
            Some(Offset {
                day: "2026-11-10".into(),
                pos: 544163
            })
        );
        assert_eq!(
            Offset::parse("2026-11-10:7").unwrap().text(),
            "2026-11-10:7"
        );
    }

    /// Notices and messages ride the span of the decisions returned: on the first load from
    /// the day before's last line, then from the old newest, so none repeats and none is
    /// skipped.
    #[test]
    fn notices_and_messages_ride_the_span_of_the_decisions_returned() {
        let (_tmp, air) = air_dir();
        let today = crate::cmd::today();
        let d = air.join(format!("events/{today}.ndjson"));
        std::fs::write(
            &d,
            format!(
                "{}\n{}\n{}\n",
                line("2026-11-10T00:00:02Z", "w1", "hook.Stop", "observed", "old"),
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
                "2026-11-10T00:00:01Z",
                "beads-ready",
                "today, before any decision",
            ),
            (
                "2026-11-10T00:00:05Z",
                "beads-ready",
                "today, after the observed line",
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
                     VALUES ('w1', ?1, ?3, '', ?2, ?3)",
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
        let first = feed(Some(&ro), &air, "now", None);
        assert_eq!(
            kinds(&first),
            [
                (
                    "beads-ready".to_string(),
                    "today, before any decision".to_string()
                ),
                (
                    "beads-ready".into(),
                    "today, after the observed line".into()
                ),
                ("claim".into(), "claimed zz-1".into()),
                ("message".into(), "to w1: hold it".into()),
                ("batch-ready".into(), "inside the first span".into()),
                ("land".into(), "landed".into()),
            ]
        );
        assert_eq!(first.items[3].source, Source::Message);
        assert_eq!(first.items[3].worker, "coordinator");
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
        let next = feed(Some(&ro), &air, "now", Offset::parse(&first.next_offset));
        let got: Vec<&str> = next.items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(got, ["after the newest decision", "done zz-1"]);
        // No new decision: no notices either, whatever the table holds.
        let none = feed(Some(&ro), &air, "now", Offset::parse(&next.next_offset));
        assert!(none.items.is_empty());
        // A first load keeps the newest `cap` of the merged set.
        let one = feed_capped(Some(&ro), &air, "now", None, 1);
        assert_eq!(kinds(&one), [("done".to_string(), "done zz-1".to_string())]);
    }
}
