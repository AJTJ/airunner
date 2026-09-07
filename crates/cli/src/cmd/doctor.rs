//! `air doctor`: where the ledger is, how big, row counts, pragmas — so an absent or wrong
//! ledger is visible, not silent. Also the bd gate (the adopter's adoption, 2026-08-21: bd 1.2.1
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
    /// Which binary this is (air-dwq5): version, the commit it was built from, and its surface
    /// version. `air doctor` is where somebody goes when a repo behaves unexpectedly, and "am I
    /// running the binary I think I am" was the one question it could not answer.
    pub air: serde_json::Value,
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
    /// The running binary is not the one this checkout would build (air-ilh4). `None` outside
    /// Air's own checkout, where the comparison has no meaning, and `None` when they agree.
    pub build_gap: Option<BuildGap>,
}

/// The running binary against the checkout it is being run in, when that checkout is Air's
/// own (air-ilh4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildGap {
    /// `CARGO_PKG_VERSION` of the binary that is executing.
    pub running: String,
    /// The version `[workspace.package]` declares in this checkout's root `Cargo.toml`.
    pub checkout: String,
}

/// The version this checkout would build, but ONLY if this checkout is Air's (air-ilh4).
///
/// Two declared reads, no guessing: `crates/cli/Cargo.toml` must declare `name = "air"`, and
/// the version comes from `[workspace.package]` in the root `Cargo.toml`. Either missing is
/// `None`, which is the answer for every adopting repo — there `main` is their code and the
/// comparison has no meaning, so a line here would fire in every repo that installs Air and
/// say nothing true in any of them.
///
/// Not `git`: the question is which program a number came from, and that is the version the
/// tree declares, not what any ref points at. A checkout mid-rebase still declares one.
pub fn air_checkout_version(repo: &Path) -> Option<String> {
    let cli = std::fs::read_to_string(repo.join("crates/cli/Cargo.toml")).ok()?;
    // The binary crate names itself. An adopting repo with a `crates/cli` of its own does not
    // call it `air`, and if it does, it IS shipping something called air and the line is
    // arguably right anyway.
    if !cli.lines().any(|l| l.trim() == r#"name = "air""#) {
        return None;
    }
    let root = std::fs::read_to_string(repo.join("Cargo.toml")).ok()?;
    let mut in_workspace_package = false;
    for line in root.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_workspace_package = t == "[workspace.package]";
            continue;
        }
        if in_workspace_package {
            if let Some(v) = t.strip_prefix("version = ") {
                return Some(v.trim_matches('"').to_string());
            }
        }
    }
    None
}

/// The gap, or `None` when there is none to state: not Air's checkout, or the two agree.
///
/// **Silent when they agree**, which is a reading of the bead rather than the whole of it. The
/// clause asks `air doctor` to name both versions; when they are the same string there is no
/// ambiguity about which program produced a number, and a line saying so every run is the
/// noise this round has spent the night removing from conditions. If the coordinator wants it
/// unconditional, the change is deleting the `!=`.
pub fn build_gap(repo: &Path, running: &str) -> Option<BuildGap> {
    let checkout = air_checkout_version(repo)?;
    (checkout != running).then(|| BuildGap {
        running: running.to_string(),
        checkout,
    })
}

/// What the gap costs a reader, said once rather than left to be re-derived (air-ilh4).
///
/// The round of 2026-09-07 ran `air` 0.2.19 against a checkout that had reached 0.3.5, and
/// quoted both interchangeably for twelve hours: every number read from `air status`, `air
/// audit` or the ledger described the frozen binary, and every digest's probe count described
/// the tree. It cost a red verify whose `log_path` was null and could not be diagnosed —
/// air-5ik, which writes run logs, landed after that binary was cut — and it nearly cost a
/// filed defect reading "the runlog mechanism fires 1 in 13", where twelve of the thirteen
/// were recorded by a binary with no runlog at all.
///
/// Says which side each number comes from; refuses nothing and tells nobody to install. The
/// freeze is deliberate.
pub fn build_gap_line(g: &BuildGap) -> String {
    format!(
        "running air {} in a checkout that builds {}: numbers from `air status`, `air audit` \
         and the ledger are {}'s; a probe count from `make verify` is {}'s. Say which when you \
         quote one.",
        g.running, g.checkout, g.running, g.checkout
    )
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
        air: crate::cmd::install::version_json(),
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
        build_gap: build_gap(repo, env!("CARGO_PKG_VERSION")),
    };
    emit(json, &report, || {
        let mut s = format!(
            "{}\nair dir: {}\nworker: {}\nledger: {} bytes, journal_mode={}, schema v{}\n",
            crate::cmd::install::version_line(),
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
        // air-ilh4: and the binary against the tree it is being run in, which is the gap that
        // decides which program a number describes. Air's own checkout only.
        if let Some(g) = &report.build_gap {
            s.push_str(&format!("{}\n", build_gap_line(g)));
        }
        s.trim_end().to_string()
    });
    // The bd gate is a refusal for `doctor`: exit 2 when bd cannot answer.
    if report.bd.list_count.is_none() { 2 } else { 0 }
}
