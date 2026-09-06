//! `air doctor`: where the ledger is, how big, row counts, pragmas — so an absent or wrong
//! ledger is visible, not silent. Also the bd gate (adopter adoption, 2026-08-21: bd 1.2.1
//! had corrupted the Dolt schema; 1.2.2 refused it and `bd list` returned 4 of 144 beads):
//! the installed bd version against the pin, and whether `bd list --json` actually answers.

use std::path::Path;

use serde::Serialize;

use crate::cmd::{emit, open};

/// The bd release Air is verified against (decisions 2026-08-18).
pub const BD_PINNED: &str = "1.2.2";

#[derive(Debug, Serialize)]
pub struct BdCheck {
    pub version: Option<String>,
    pub pinned: &'static str,
    pub version_ok: bool,
    /// `bd list --json` ran and parsed: Some(count); None with the error text in `error`.
    pub list_count: Option<usize>,
    pub error: Option<String>,
}

pub fn bd_check(repo: &Path) -> BdCheck {
    let bd = crate::cmd::claim::bd_for(repo);
    let version = std::process::Command::new(&bd.bin)
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .and_then(|s| {
            s.split_whitespace()
                .find(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit()))
                .map(str::to_string)
        });
    let version_ok = version.as_deref() == Some(BD_PINNED);
    let (list_count, error) =
        match air_bd::WorkLedger::in_progress(&bd).and_then(|_| air_bd::WorkLedger::ready(&bd)) {
            Ok(v) => (Some(v.len()), None),
            Err(e) => (None, Some(e.to_string())),
        };
    BdCheck {
        version,
        pinned: BD_PINNED,
        version_ok,
        list_count,
        error,
    }
}

/// A rule of Air's whose behaviour changes on a date, and whether that date has passed.
///
/// Two rules replaced a guess with a declaration and let the old artefacts age out
/// ([`crate::cmd::attribution::FALLBACK_BEFORE`],
/// [`crate::cmd::handover::FRONTMATTER_SINCE`]). Both dates passed on 2026-08-23 and nothing
/// said so: seven tests in `claim_cli.rs` had been written inside the fallback window and
/// silently fell outside it, main went red, and it stayed red for six days because a cutoff
/// passing is not an event anything watches (air-24e).
///
/// This is a REPORT, never a refusal: a passed cutoff is not a fault, it is a fallback that is
/// now dead and can be deleted along with whatever leans on it. Removal: when both fallbacks
/// are gone and no dated rule is left, this goes with them.
#[derive(Debug, Serialize)]
pub struct DatedRule {
    pub name: &'static str,
    pub date: &'static str,
    pub what: &'static str,
    pub expired: bool,
}

/// The dated rules, read from the constants themselves rather than copied (`anti-brittleness`:
/// a probe reads a rule's number from the rule).
pub fn dated_rules(now: jiff::Timestamp) -> Vec<DatedRule> {
    let mk = |name, date: &'static str, what| DatedRule {
        name,
        date,
        what,
        expired: date.parse::<jiff::Timestamp>().is_ok_and(|t| now >= t),
    };
    vec![
        mk(
            "attribution::FALLBACK_BEFORE",
            crate::cmd::attribution::FALLBACK_BEFORE,
            "a commit older than this may have its bead guessed from prose; newer commits need a `Bead:` trailer",
        ),
        mk(
            "handover::FRONTMATTER_SINCE",
            crate::cmd::handover::FRONTMATTER_SINCE,
            "a digest older than this may be matched by filename and mtime; newer digests must declare `bead:`",
        ),
    ]
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub bd: BdCheck,
    /// Dated rules and whether their cutoff has passed (air-24e).
    pub dated_rules: Vec<DatedRule>,
    pub air_dir: String,
    pub worker: String,
    pub ledger_bytes: u64,
    pub journal_mode: String,
    pub user_version: i64,
    pub rows: Vec<(String, i64)>,
    /// The event stream's stated retention and what is collectable under it (air-i7s). Here
    /// because a retention nobody can read is not a stated one.
    pub events: crate::cmd::gc::Plan,
    /// The install record is older than this binary (air-d61): the repo's hooks run a binary
    /// it was never told about.
    pub install_lag: Option<crate::cmd::install::InstallLag>,
}

/// Row counts for every table the ledger actually has, asked of `sqlite_master` rather than
/// of a list somebody typed (air-w0e).
///
/// The list version reported 7 of the 11 tables at schema v10: `hook_emissions`, `conditions`,
/// `lease_wants` and `bd_cache` were invisible, which is how the zero-lease finding nearly
/// went unnoticed. A check that enumerates from a hardcoded list stops covering what it claims
/// the moment the thing it lists grows, and it does so silently, which is the worse half.
/// Enumerating means the table the next migration adds appears the day it is added and nobody
/// has to remember this file exists.
///
/// SQLite's own `sqlite_*` tables are left out: they are the engine's, not the ledger's.
/// Removal: when nothing reads row counts, this goes with the command.
pub fn table_rows(conn: &rusqlite::Connection) -> Vec<(String, i64)> {
    let names: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type='table' \
             AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .and_then(|mut st| st.query_map([], |r| r.get(0))?.collect())
        .unwrap_or_default();
    names
        .into_iter()
        .map(|t| {
            // The name came from `sqlite_master`, so it is a table this database has; -1 says
            // the count itself failed rather than pretending the table is empty.
            let n: i64 = conn
                .query_row(&format!("SELECT count(*) FROM \"{t}\""), [], |r| r.get(0))
                .unwrap_or(-1);
            (t, n)
        })
        .collect()
}

pub fn run(repo: &Path, json: bool) -> i32 {
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air doctor: {e}");
            return 1;
        }
    };
    let db = ledger.dir().join("ledger.db");
    let ledger_bytes = std::fs::metadata(&db).map(|m| m.len()).unwrap_or(0);
    let journal_mode: String = ledger
        .conn()
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap_or_else(|_| "?".into());
    let user_version: i64 = ledger
        .conn()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(-1);
    let rows = table_rows(ledger.conn());
    let report = Report {
        bd: bd_check(repo),
        dated_rules: dated_rules(
            crate::cmd::now()
                .parse()
                .unwrap_or(jiff::Timestamp::UNIX_EPOCH),
        ),
        air_dir: ledger.dir().display().to_string(),
        worker,
        ledger_bytes,
        journal_mode,
        user_version,
        rows,
        events: crate::cmd::gc::plan(
            &crate::cmd::gc::event_days(ledger.dir()),
            &crate::cmd::today(),
            crate::cmd::gc::KEEP_DAYS,
            &crate::cmd::gc::referenced_days(ledger.conn()),
        ),
        install_lag: crate::cmd::install::lag(ledger.dir()),
    };
    emit(json, &report, || {
        let mut s = format!(
            "air dir: {}\nworker: {}\nledger: {} bytes, journal_mode={}, schema v{}\n",
            report.air_dir,
            report.worker,
            report.ledger_bytes,
            report.journal_mode,
            report.user_version
        );
        for (t, n) in &report.rows {
            s.push_str(&format!("  {t}: {n}\n"));
        }
        let e = &report.events;
        s.push_str(&format!(
            "events: {} day(s), {} bytes; retention {} day(s), {} bytes collectable{}\n",
            e.days.len(),
            e.total_bytes,
            e.keep_days,
            e.collectable_bytes,
            if e.collectable_bytes > 0 {
                "; `air gc` to see what, `air gc --apply` to remove it"
            } else {
                ""
            }
        ));
        let b = &report.bd;
        s.push_str(&format!(
            "bd: {} (pinned {}): {}\n",
            b.version.as_deref().unwrap_or("not found"),
            b.pinned,
            if b.version_ok {
                "ok"
            } else {
                "MISMATCH; brew upgrade beads && brew pin beads"
            }
        ));
        match (&b.list_count, &b.error) {
            (Some(n), _) => s.push_str(&format!("bd list --json: ok ({n} ready)\n")),
            (None, Some(e)) => s.push_str(&format!(
                "bd list --json: FAILED: {e}\n  a refused schema or a removed subcommand breaks every bd-reading gate; fix bd before installing Air\n"
            )),
            _ => {}
        }
        for r in &report.dated_rules {
            if r.expired {
                s.push_str(&format!(
                    "dated rule {} ({}): EXPIRED — {}\n  the fallback is dead: delete it and anything still leaning on it\n",
                    r.name, r.date, r.what
                ));
            } else {
                s.push_str(&format!(
                    "dated rule {} ({}): active — {}\n",
                    r.name, r.date, r.what
                ));
            }
        }
        // air-d61: the record says what this repo was told; the binary says what runs.
        if let Some(l) = &report.install_lag {
            s.push_str(&format!("{}\n", crate::cmd::install::lag_line(l)));
        }
        s.trim_end().to_string()
    });
    // The bd gate is a refusal for `doctor`: exit 2 when bd cannot answer.
    if report.bd.list_count.is_none() { 2 } else { 0 }
}
