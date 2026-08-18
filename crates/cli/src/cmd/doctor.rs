//! `air doctor`: where the ledger is, how big, row counts, pragmas — so an absent or wrong
//! ledger is visible, not silent.

use std::path::Path;

use serde::Serialize;

use crate::cmd::{emit, open};

#[derive(Debug, Serialize)]
pub struct Report {
    pub air_dir: String,
    pub worker: String,
    pub ledger_bytes: u64,
    pub journal_mode: String,
    pub user_version: i64,
    pub rows: Vec<(String, i64)>,
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
    let mut rows = Vec::new();
    for t in [
        "verify_runs",
        "edit_journal",
        "claims",
        "sessions",
        "landings",
    ] {
        let n: i64 = ledger
            .conn()
            .query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))
            .unwrap_or(-1);
        rows.push((t.to_string(), n));
    }
    let report = Report {
        air_dir: ledger.dir().display().to_string(),
        worker,
        ledger_bytes,
        journal_mode,
        user_version,
        rows,
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
        s.trim_end().to_string()
    });
    0
}
