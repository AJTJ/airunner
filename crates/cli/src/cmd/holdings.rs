//! `air holdings [--file X]`: who has edits in which files, across worktrees.
//!
//! Derived, never stored (plan 0001 §2): uncommitted changes from `git status` per worktree,
//! committed divergence from `git diff --name-only main...HEAD`, plus journaled intent from
//! `edit_journal`, whose rows end once their file is clean and unchanged against main
//! ([`expire`], air-84lz). Unit is the file (tick 0400). Prints its denominator.
//!
//! Every tag names its tense (air-v7o; the adopter 2026-08-30, both directions in one day: a
//! worker nearly released a bead over an `uncommitted` that had evaporated, and another nearly
//! stood down over a `journaled` from hours earlier). `uncommitted` is an instant and is read
//! at the report's `at`; `journaled` is history and carries its age. A dirty file NO tool
//! edited (nothing journaled for that worker on that path) is named as such, because
//! `git status --untracked-files=all` lists build and test output beside real edits: the
//! report that started this was nine minutes of regenerated fixtures during a full verify.
//!
//! `--untracked-files=all` stays. Narrowing it would trade a false "someone is here" for a
//! false "nobody is here", and for the case that produced the report (two workers each
//! producing a plausible tree claiming the same catalog version) the second is the worse
//! direction (the adopter's reasoning, accepted on the bead). The journal is what separates an
//! edit from dirt; sensitivity is not the lever.

use std::collections::{BTreeMap, BTreeSet};

use std::path::Path;

use serde::Serialize;

use crate::cmd::{emit, log_event, open};
use crate::git;

#[derive(Debug, Default, Clone, Serialize)]
pub struct Holding {
    pub worker: String,
    /// Dirty in the worker's tree at the report's `at`. An instant, not a history.
    pub uncommitted: bool,
    pub committed: bool,
    /// A tool edit by this worker is journaled on the path (history).
    pub journaled: bool,
    /// When that worker's tool last touched the path (`edit_journal.last_seen`), so the
    /// reader can tell a live edit from a remembered one (air-v7o).
    pub last_edit: Option<String>,
    /// This worker has a verify running at `at` (air-4cr's row): a dirty file with no journaled
    /// edit is then most likely that verify's output.
    pub verify_in_flight: bool,
}

#[derive(Debug, Serialize)]
pub struct Report {
    /// The instant `uncommitted` is true of. Everything else is history with its own time.
    pub at: String,
    /// path -> holders
    pub files: BTreeMap<String, Vec<Holding>>,
    pub worktrees_compared: usize,
    pub errors: Vec<String>,
}

/// One holder's tags, each naming its tense (air-v7o). Pure, so a probe can render every
/// combination without a repo.
pub fn tags(h: &Holding, now: &str) -> String {
    let mut t = Vec::new();
    let age = h.last_edit.as_deref().map(|e| age_text(e, now));
    match (h.uncommitted, h.journaled) {
        (true, true) => t.push(format!(
            "uncommitted now, edited {}",
            age.unwrap_or_else(|| "at an unknown time".to_string())
        )),
        (true, false) => t.push(if h.verify_in_flight {
            "uncommitted now, no edit journaled; verify in flight, likely its output".to_string()
        } else {
            "uncommitted now, no edit journaled".to_string()
        }),
        (false, true) => t.push(format!(
            "journaled {}, clean now",
            age.unwrap_or_else(|| "at an unknown time".to_string())
        )),
        (false, false) => {}
    }
    if h.committed {
        t.push("committed".to_string());
    }
    t.join(", ")
}

/// "3 min ago", "2 h ago", "1 d ago"; "at an unknown time" when either clock is unreadable.
pub fn age_text(earlier: &str, now: &str) -> String {
    match super::status::minutes_between(earlier, now) {
        Some(m) if m < 60 => format!("{m} min ago"),
        Some(m) if m < 60 * 24 => format!("{} h ago", m / 60),
        Some(m) => format!("{} d ago", m / (60 * 24)),
        None => "at an unknown time".to_string(),
    }
}

/// What each worktree has that main does not, by worker name: `(dirty, committed)` from
/// `git status` and `main...HEAD`. `Err` when git could not say for that worktree.
type Live = BTreeMap<String, Result<(Vec<String>, Vec<String>), String>>;

fn live(wts: &[(std::path::PathBuf, Option<String>)]) -> Live {
    let mut out = Live::new();
    for (path, _branch) in wts {
        let name = air_ledger::paths::worker_name_for(path).unwrap_or_else(|_| {
            path.file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        });
        let dirty = git::dirty_files(path).map_err(|e| format!("{}: {e}", path.display()));
        let committed = if name == "main" {
            Ok(Vec::new())
        } else {
            git::changed_since(path, "main").map_err(|e| format!("{}: {e}", path.display()))
        };
        out.insert(name, dirty.and_then(|d| committed.map(|c| (d, c))));
    }
    out
}

/// Pure: the journal rows whose holding has ended (air-84lz). A holding ends when the file is
/// clean in the holder's worktree and its branch does not change it against main: its change is
/// in main, or was dropped. A holder with no worktree has no tree to hold anything in. A
/// worktree git could not read keeps its rows, since the safe direction is to keep warning.
pub fn ended<'a>(rows: &'a [(String, String)], live: &Live) -> Vec<&'a (String, String)> {
    rows.iter()
        .filter(|(w, p)| match live.get(w) {
            None => true,
            Some(Ok((dirty, committed))) => !dirty.contains(p) && !committed.contains(p),
            Some(Err(_)) => false,
        })
        .collect()
}

/// Delete the journal rows whose holding has ended, so neither `air holdings` nor the peer
/// warning names them again (air-84lz). An adopter's warnings cited edits 18 h old and long
/// landed, and a gone session held 481 files. Run when `air holdings` or `air status` reads
/// and when `air land` moves main. Returns how many ended.
pub fn expire(ledger: &air_ledger::Ledger, repo: &Path) -> usize {
    let Ok(wts) = git::worktrees(repo) else {
        return 0;
    };
    expire_with(ledger, &live(&wts))
}

fn expire_with(ledger: &air_ledger::Ledger, live: &Live) -> usize {
    let rows: Vec<(String, String)> = ledger
        .conn()
        .prepare("SELECT worker, path FROM edit_journal")
        .and_then(|mut st| {
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default();
    let mut n = 0usize;
    for (w, p) in ended(&rows, live) {
        if ledger
            .conn()
            .execute(
                "DELETE FROM edit_journal WHERE worker=?1 AND path=?2",
                rusqlite::params![w, p],
            )
            .is_ok_and(|k| k > 0)
        {
            n = n.saturating_add(1);
        }
    }
    n
}

pub fn compute(repo: &Path, only: Option<&str>) -> Result<Report, String> {
    let (ledger, _worker) = open(repo)?;
    let at = crate::cmd::now();
    let mut files: BTreeMap<String, Vec<Holding>> = BTreeMap::new();
    let mut errors = Vec::new();
    let wts = git::worktrees(repo).map_err(|e| e.to_string())?;
    let live = live(&wts);
    expire_with(&ledger, &live);
    for (name, state) in &live {
        let mut mark = |p: String, f: fn(&mut Holding)| {
            if only.is_some_and(|o| o != p) || is_tooling_path(&p) {
                return;
            }
            let holders = files.entry(p).or_default();
            if let Some(h) = holders.iter_mut().find(|h| h.worker == *name) {
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
        match state {
            Ok((dirty, committed)) => {
                dirty
                    .iter()
                    .for_each(|p| mark(p.clone(), |h| h.uncommitted = true));
                committed
                    .iter()
                    .for_each(|p| mark(p.clone(), |h| h.committed = true));
            }
            Err(e) => errors.push(e.clone()),
        }
    }
    // Journaled intent, with when the tool last touched the path.
    let mut stmt = ledger
        .conn()
        .prepare("SELECT worker, path, last_seen FROM edit_journal")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (w, p, last) = row.map_err(|e| e.to_string())?;
        if only.is_some_and(|o| o != p) {
            continue;
        }
        let holders = files.entry(p).or_default();
        if let Some(h) = holders.iter_mut().find(|h| h.worker == w) {
            h.journaled = true;
            h.last_edit = Some(last);
        } else {
            holders.push(Holding {
                worker: w,
                journaled: true,
                last_edit: Some(last),
                ..Default::default()
            });
        }
    }
    // A verify running now: its worker's unjournaled dirt is most likely its output.
    let verifying: BTreeSet<String> = super::status::verifies_in_flight(&ledger)
        .into_iter()
        .map(|f| f.worker)
        .collect();
    for hs in files.values_mut() {
        for h in hs.iter_mut() {
            h.verify_in_flight = verifying.contains(&h.worker);
        }
    }
    Ok(Report {
        at,
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
            super::decisions::HOLDINGS_OK,
            &serde_json::json!({"file": only}),
            &format!("{} files with holders", report.files.len()),
            &format!("compared {} worktrees", report.worktrees_compared),
        );
    }
    emit(json, &report, || {
        let mut out = format!(
            "compared {} worktrees at {}\n",
            report.worktrees_compared, report.at
        );
        for (p, hs) in &report.files {
            let who: Vec<String> = hs
                .iter()
                .map(|h| format!("{}[{}]", h.worker, tags(h, &report.at)))
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
/// recovery backups, nested worktrees (the adopter: a 320 MB `.beads.backup-pre-recovery/`
/// showed up as "uncommitted main files").
pub fn is_tooling_path(p: &str) -> bool {
    p.starts_with(".air/")
        || p.starts_with(".beads")
        || p.starts_with(".claude/worktrees/")
        || p == ".air"
}

#[cfg(test)]
mod tests {
    use super::{Holding, Live, age_text, ended, is_tooling_path, tags};

    /// air-v7o: every tag says when. An edit and build dirt are told apart by the journal.
    #[test]
    fn tags_name_their_tense() {
        let now = "2026-08-30T21:15:00Z";
        let edited = Holding {
            worker: "w1".into(),
            uncommitted: true,
            journaled: true,
            last_edit: Some("2026-08-30T21:12:00Z".into()),
            ..Default::default()
        };
        assert_eq!(tags(&edited, now), "uncommitted now, edited 3 min ago");
        let dirt = Holding {
            worker: "w3".into(),
            uncommitted: true,
            verify_in_flight: true,
            ..Default::default()
        };
        assert_eq!(
            tags(&dirt, now),
            "uncommitted now, no edit journaled; verify in flight, likely its output"
        );
        let remembered = Holding {
            worker: "w1".into(),
            journaled: true,
            committed: true,
            last_edit: Some("2026-08-30T15:15:00Z".into()),
            ..Default::default()
        };
        assert_eq!(
            tags(&remembered, now),
            "journaled 6 h ago, clean now, committed"
        );
        assert_eq!(age_text("garbage", now), "at an unknown time");
    }

    /// air-84lz: a holding ends when the file is clean in the holder's tree and its branch does
    /// not change it against main; not before.
    #[test]
    fn a_holding_ends_once_its_change_is_on_main_and_the_tree_is_clean() {
        let row = |w: &str, p: &str| (w.to_string(), p.to_string());
        let rows = vec![
            row("w1", "landed.rs"),
            row("w1", "on-branch.rs"),
            row("w1", "dirty.rs"),
            row("gone", "x.rs"),
            row("unreadable", "y.rs"),
        ];
        let mut live = Live::new();
        live.insert(
            "w1".into(),
            Ok((vec!["dirty.rs".into()], vec!["on-branch.rs".into()])),
        );
        live.insert("unreadable".into(), Err("git failed".into()));
        let gone: Vec<&str> = ended(&rows, &live)
            .into_iter()
            .map(|(w, p)| if w == "gone" { "gone" } else { p.as_str() })
            .collect();
        assert_eq!(gone, ["landed.rs", "gone"]);
    }

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
