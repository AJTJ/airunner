//! `air holdings [--file X]`: who has edits in which files, across worktrees.
//!
//! Derived, never stored (plan 0001 §2): uncommitted changes from `git status` per worktree,
//! committed divergence from `git diff --name-only main...HEAD`, plus journaled intent from
//! `edit_journal`. Unit is the file (tick 0400). Prints its denominator.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::cmd::{emit, log_event, open};
use crate::git;

#[derive(Debug, Default, Serialize)]
pub struct Holding {
    pub worker: String,
    pub uncommitted: bool,
    pub committed: bool,
    pub journaled: bool,
}

#[derive(Debug, Serialize)]
pub struct Report {
    /// path -> holders
    pub files: BTreeMap<String, Vec<Holding>>,
    pub worktrees_compared: usize,
    pub errors: Vec<String>,
}

pub fn compute(repo: &Path, only: Option<&str>) -> Result<Report, String> {
    let (ledger, _worker) = open(repo)?;
    let mut files: BTreeMap<String, Vec<Holding>> = BTreeMap::new();
    let mut errors = Vec::new();
    let wts = git::worktrees(repo).map_err(|e| e.to_string())?;
    for (path, _branch) in &wts {
        let name = air_ledger::paths::worker_name_for(path).unwrap_or_else(|_| {
            path.file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        });
        let mut mark = |p: String, f: fn(&mut Holding)| {
            if only.is_some_and(|o| o != p) || is_tooling_path(&p) {
                return;
            }
            let holders = files.entry(p).or_default();
            if let Some(h) = holders.iter_mut().find(|h| h.worker == name) {
                f(h);
            } else {
                let mut h = Holding {
                    worker: name.clone(),
                    ..Default::default()
                };
                f(&mut h);
                holders.push(h);
            }
        };
        match git::dirty_files(path) {
            Ok(v) => v
                .into_iter()
                .for_each(|p| mark(p, |h| h.uncommitted = true)),
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
        if name != "main" {
            match git::changed_since(path, "main") {
                Ok(v) => v.into_iter().for_each(|p| mark(p, |h| h.committed = true)),
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
    }
    // Journaled intent.
    let mut stmt = ledger
        .conn()
        .prepare("SELECT worker, path FROM edit_journal")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (w, p) = row.map_err(|e| e.to_string())?;
        if only.is_some_and(|o| o != p) {
            continue;
        }
        let holders = files.entry(p).or_default();
        if let Some(h) = holders.iter_mut().find(|h| h.worker == w) {
            h.journaled = true;
        } else {
            holders.push(Holding {
                worker: w,
                journaled: true,
                ..Default::default()
            });
        }
    }
    Ok(Report {
        files,
        worktrees_compared: wts.len(),
        errors,
    })
}

pub fn run(repo: &Path, only: Option<&str>, json: bool) -> i32 {
    let report = match compute(repo, only) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("air holdings: {e}");
            return 1;
        }
    };
    if let Ok((ledger, worker)) = open(repo) {
        log_event(
            &ledger,
            &worker,
            "holdings",
            &serde_json::json!({"file": only}),
            "ok",
            &format!("{} files with holders", report.files.len()),
            &format!("compared {} worktrees", report.worktrees_compared),
        );
    }
    emit(json, &report, || {
        let mut out = format!("compared {} worktrees\n", report.worktrees_compared);
        for (p, hs) in &report.files {
            let who: Vec<String> = hs
                .iter()
                .map(|h| {
                    let mut tags = Vec::new();
                    if h.uncommitted {
                        tags.push("uncommitted");
                    }
                    if h.committed {
                        tags.push("committed");
                    }
                    if h.journaled {
                        tags.push("journaled");
                    }
                    format!("{}[{}]", h.worker, tags.join(","))
                })
                .collect();
            out.push_str(&format!("{p}: {}\n", who.join(" ")));
        }
        for e in &report.errors {
            out.push_str(&format!("error: {e}\n"));
        }
        out.trim_end().to_string()
    });
    0
}

/// Paths that are tooling state, never a worker's holding: Air's ledger, beads' store and its
/// recovery backups, nested worktrees (adopter: a 320 MB `.beads.backup-pre-recovery/`
/// showed up as "uncommitted main files").
pub fn is_tooling_path(p: &str) -> bool {
    p.starts_with(".air/")
        || p.starts_with(".beads")
        || p.starts_with(".claude/worktrees/")
        || p == ".air"
}

#[cfg(test)]
mod tests {
    use super::is_tooling_path;

    #[test]
    fn tooling_paths_are_not_holdings() {
        assert!(is_tooling_path(".beads.backup-pre-recovery/x.db"));
        assert!(is_tooling_path(".beads/issues.jsonl"));
        assert!(is_tooling_path(".air/ledger.db"));
        assert!(is_tooling_path(".claude/worktrees/a/src.rs"));
        assert!(!is_tooling_path("src/.beads_like.rs"));
        assert!(!is_tooling_path(".claude/settings.json"));
    }
}
