//! Schema for the ledger. Tables map 1:1 to docs/plans/0001-first-slice.md §2 (columns from
//! the measurement spec, tick 0430). Forward-only migrations keyed by `user_version`.

use rusqlite::Connection;

use crate::Result;

pub const CURRENT_VERSION: i64 = 2;

const V1: &str = r#"
CREATE TABLE IF NOT EXISTS verify_runs (
    id            TEXT PRIMARY KEY,           -- ulid
    worker        TEXT NOT NULL,
    sha           TEXT NOT NULL,
    kind          TEXT NOT NULL,              -- verify | docs-check | fitness
    exit_code     INTEGER NOT NULL,
    trigger       TEXT NOT NULL,              -- record | land | hook
    failing_step  TEXT,
    started_at    TEXT NOT NULL,              -- RFC 3339 UTC
    finished_at   TEXT NOT NULL,
    log_path      TEXT
);
CREATE INDEX IF NOT EXISTS verify_runs_worker_sha ON verify_runs(worker, sha, kind);

CREATE TABLE IF NOT EXISTS edit_journal (
    worker      TEXT NOT NULL,
    path        TEXT NOT NULL,                -- repo-relative
    session_id  TEXT,
    first_seen  TEXT NOT NULL,
    last_seen   TEXT NOT NULL,
    PRIMARY KEY (worker, path)
);

CREATE TABLE IF NOT EXISTS claims (
    bead               TEXT NOT NULL,
    worker             TEXT NOT NULL,
    claimed_at         TEXT NOT NULL,
    declared_files     TEXT,                  -- JSON array, optional
    preconditions      TEXT,                  -- JSON array of "peer@sha", optional
    first_handover_at  TEXT,
    last_handover_at   TEXT,
    handover_source    TEXT,                  -- cli | hook
    handover_attempts  INTEGER NOT NULL DEFAULT 0,
    released_at        TEXT,
    release_reason     TEXT,
    suggested_by_next_id TEXT,
    PRIMARY KEY (bead, worker)
);

CREATE TABLE IF NOT EXISTS sessions (
    session_id       TEXT PRIMARY KEY,
    worker           TEXT NOT NULL,
    transcript_path  TEXT,
    state            TEXT NOT NULL,           -- working | running | stuck | idle
    detail           TEXT,                    -- tool name for running
    changed_at       TEXT NOT NULL,
    started_at       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS landings (
    id             TEXT PRIMARY KEY,
    worker         TEXT NOT NULL,
    sha            TEXT NOT NULL,
    tip_sha        TEXT,
    result         TEXT NOT NULL,             -- landed | rewound | refused
    failing_step   TEXT,
    verify_run_id  TEXT,
    attempt_no     INTEGER NOT NULL DEFAULT 1,
    beads          TEXT,                      -- JSON array
    merge_commit   TEXT,
    started_at     TEXT NOT NULL,
    finished_at    TEXT NOT NULL
);
"#;

/// v2 (2026-08-20): the capture inbox (workers capture, the coordinator triages; decisions
/// 2026-08-18/20) and a derived `role` on sessions for `air status`.
const V2: &str = r#"
CREATE TABLE IF NOT EXISTS captures (
    id           TEXT PRIMARY KEY,           -- ulid
    worker       TEXT NOT NULL,
    session_id   TEXT,
    text         TEXT NOT NULL,
    captured_at  TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'open', -- open | promoted | dropped
    resolved_at  TEXT,
    bead         TEXT,                       -- when promoted
    note         TEXT                        -- triage note (why dropped / grouped where)
);
CREATE INDEX IF NOT EXISTS captures_status ON captures(status, captured_at);
ALTER TABLE sessions ADD COLUMN role TEXT NOT NULL DEFAULT 'worker';
"#;

/// Apply migrations up to `CURRENT_VERSION`. Idempotent.
pub fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch(V1)?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    if version < 2 {
        conn.execute_batch(V2)?;
        conn.pragma_update(None, "user_version", 2)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn migrate_is_idempotent_and_sets_version() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, CURRENT_VERSION);
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN \
                 ('verify_runs','edit_journal','claims','sessions','landings','captures')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 6);
    }
}
