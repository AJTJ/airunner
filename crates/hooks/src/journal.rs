//! Edit journal (plan 0001 §2 row 2): "worker W started touching path P" — written by the
//! `PostToolUse(Edit|Write)` hook at zero token cost. Paths are stored repo-relative; the
//! unit is the file (tick 0400: file-level is what every agent-era system uses).

use std::path::Path;

use air_ledger::{Ledger, Result};
use rusqlite::params;

/// Upsert (worker, path): first_seen on insert, last_seen always. `now` is RFC 3339 UTC.
pub fn touch(
    ledger: &Ledger,
    worker: &str,
    repo_relative: &str,
    session_id: Option<&str>,
    now: &str,
) -> Result<()> {
    ledger.conn().execute(
        "INSERT INTO edit_journal (worker, path, session_id, first_seen, last_seen) \
         VALUES (?1, ?2, ?3, ?4, ?4) \
         ON CONFLICT(worker, path) DO UPDATE SET last_seen = excluded.last_seen, \
         session_id = COALESCE(excluded.session_id, edit_journal.session_id)",
        params![worker, repo_relative, session_id, now],
    )?;
    Ok(())
}

/// Other workers currently journaled on `repo_relative` (for the PreToolUse warning).
pub fn peers_on(ledger: &Ledger, worker: &str, repo_relative: &str) -> Result<Vec<String>> {
    let mut stmt = ledger.conn().prepare(
        "SELECT worker FROM edit_journal WHERE path = ?1 AND worker != ?2 ORDER BY worker",
    )?;
    let rows = stmt.query_map(params![repo_relative, worker], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Make an absolute tool path repo-relative given the worktree root. Returns None if the
/// path is outside the root (we never journal those).
pub fn relative_to(root: &Path, abs: &Path) -> Option<String> {
    abs.strip_prefix(root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn touch_upserts_and_peers_exclude_self() {
        let l = Ledger::open_in_memory().unwrap();
        touch(&l, "w1", "src/a.rs", Some("s1"), "2026-08-18T10:00:00Z").unwrap();
        touch(&l, "w1", "src/a.rs", None, "2026-08-18T10:05:00Z").unwrap();
        touch(&l, "w2", "src/a.rs", Some("s2"), "2026-08-18T10:06:00Z").unwrap();
        let (first, last): (String, String) = l
            .conn()
            .query_row(
                "SELECT first_seen, last_seen FROM edit_journal WHERE worker='w1' AND path='src/a.rs'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(first, "2026-08-18T10:00:00Z");
        assert_eq!(last, "2026-08-18T10:05:00Z");
        assert_eq!(peers_on(&l, "w1", "src/a.rs").unwrap(), vec!["w2"]);
        assert!(peers_on(&l, "w1", "src/b.rs").unwrap().is_empty());
    }

    #[test]
    fn relative_to_rejects_outside_paths() {
        let root = Path::new("/repo/wt");
        assert_eq!(
            relative_to(root, Path::new("/repo/wt/src/a.rs")).as_deref(),
            Some("src/a.rs")
        );
        assert!(relative_to(root, Path::new("/elsewhere/a.rs")).is_none());
    }
}
