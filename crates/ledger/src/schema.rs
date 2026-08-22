//! Schema for the ledger. Tables map 1:1 to docs/plans/0001-first-slice.md §2 (columns from
//! the measurement spec, tick 0430). Forward-only migrations keyed by `user_version`.

use rusqlite::Connection;

use crate::Result;

pub const CURRENT_VERSION: i64 = 10;

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

/// v3 (2026-08-21): what a verify run actually ran. adopter captures 4d1e52/9de453: a green
/// exit from a command that had stopped measuring what it claimed. The exit stays the fact;
/// the command line, duration, output size and a dirty-tree flag make a false green visible.
const V3: &str = r#"
ALTER TABLE verify_runs ADD COLUMN command TEXT;
ALTER TABLE verify_runs ADD COLUMN duration_ms INTEGER;
ALTER TABLE verify_runs ADD COLUMN output_bytes INTEGER;
ALTER TABLE verify_runs ADD COLUMN dirty INTEGER NOT NULL DEFAULT 0;
"#;

/// v4 (2026-08-21, owner rulings A and E): named-resource leases (ported from
/// `adopter/scripts/lease.sh`: identity is the worktree, liveness is the pid + its start
/// time, stale is heartbeat age) and an audience on captures (`coordinator` | `owner`).
const V4: &str = r#"
CREATE TABLE IF NOT EXISTS leases (
    resource      TEXT PRIMARY KEY,           -- e.g. runtime, :8080, simulator, chrome
    worker        TEXT NOT NULL,
    session_id    TEXT,
    pid           INTEGER,
    pid_started   TEXT,                       -- `ps -o lstart=` of pid, guards reuse
    reason        TEXT NOT NULL,
    taken_at      TEXT NOT NULL,
    heartbeat_at  TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS lease_wants (
    resource   TEXT NOT NULL,
    worker     TEXT NOT NULL,
    reason     TEXT,
    wanted_at  TEXT NOT NULL,
    PRIMARY KEY (resource, worker)
);
ALTER TABLE captures ADD COLUMN audience TEXT NOT NULL DEFAULT 'coordinator';
"#;

/// v5 (2026-08-21, adopter ad-lpqp): the `claude` pid on the session row so "gone" can
/// mean the process is gone, not "no hook yet".
const V5: &str = r#"
ALTER TABLE sessions ADD COLUMN pid INTEGER;
"#;

/// v6 (2026-08-21, adopter: Stop advisory repeated every turn while a worker was blocked):
/// what each session was last told, per key, so a hook speaks only on change.
const V6: &str = r#"
CREATE TABLE IF NOT EXISTS hook_emissions (
    session_id   TEXT NOT NULL,
    key          TEXT NOT NULL,               -- e.g. stop, peer:<path>
    fingerprint  TEXT NOT NULL,
    emitted_at   TEXT NOT NULL,
    PRIMARY KEY (session_id, key)
);
"#;

/// v7 (2026-08-21, plan 0006 C1): attention conditions as rows, so first-seen and cleared are
/// facts and time-to-unblock is a query. Open row = condition holds now.
const V7: &str = r#"
CREATE TABLE IF NOT EXISTS conditions (
    worker      TEXT NOT NULL,
    kind        TEXT NOT NULL,
    first_seen  TEXT NOT NULL,
    last_seen   TEXT NOT NULL,
    cleared_at  TEXT,
    detail      TEXT
);
CREATE INDEX IF NOT EXISTS conditions_open ON conditions(worker, kind, cleared_at);
"#;

/// v8: last successful bd answers `status` depends on, so a slow bd (20 s under load,
/// adopter 2026-08-22, air-19u) degrades to stale counts instead of an empty status.
const V8: &str = r#"
CREATE TABLE IF NOT EXISTS bd_cache (
    key      TEXT PRIMARY KEY,
    value    TEXT NOT NULL,
    seen_at  TEXT NOT NULL
);
"#;

/// v9 (2026-08-22, air-0lk): which project a session belongs to, so "is that peer one of
/// ours?" is answered from the ledger rather than by string-matching a name. `AIR_PROJECT` on
/// the launcher, the beads prefix underneath.
const V9: &str = r#"
ALTER TABLE sessions ADD COLUMN project TEXT NOT NULL DEFAULT '';
"#;

/// v10 (2026-08-22, air-ayp): which beads a landing merged but did NOT close, and why. A
/// landing may only close a bead whose acceptance it can point at evidence for; the rest land
/// merged-but-open, carried here rather than in a bd status (bd's blocking predicate never
/// consults the workflow class, so a bead parked in a custom status blocks its dependents
/// indefinitely).
const V10: &str = r#"
ALTER TABLE landings ADD COLUMN open_beads TEXT;
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
    if version < 3 {
        conn.execute_batch(V3)?;
        conn.pragma_update(None, "user_version", 3)?;
    }
    if version < 4 {
        conn.execute_batch(V4)?;
        conn.pragma_update(None, "user_version", 4)?;
    }
    if version < 5 {
        conn.execute_batch(V5)?;
        conn.pragma_update(None, "user_version", 5)?;
    }
    if version < 6 {
        conn.execute_batch(V6)?;
        conn.pragma_update(None, "user_version", 6)?;
    }
    if version < 7 {
        conn.execute_batch(V7)?;
        conn.pragma_update(None, "user_version", 7)?;
    }
    if version < 8 {
        conn.execute_batch(V8)?;
        conn.pragma_update(None, "user_version", 8)?;
    }
    if version < 9 {
        conn.execute_batch(V9)?;
        conn.pragma_update(None, "user_version", 9)?;
    }
    if version < 10 {
        conn.execute_batch(V10)?;
        conn.pragma_update(None, "user_version", 10)?;
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
                 ('verify_runs','edit_journal','claims','sessions','landings','captures','leases','lease_wants')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 8);
    }
}
