//! Schema for the ledger. Tables map 1:1 to docs/plans/0001-first-slice.md §2 (columns from
//! the measurement spec, tick 0430). Forward-only migrations keyed by `user_version`.

use rusqlite::Connection;

use crate::Result;

pub const CURRENT_VERSION: i64 = 20;

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
    state            TEXT NOT NULL,           -- working | running | idle (stuck: deleted, air-12k)
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

/// v3 (2026-08-21): what a verify run actually ran. The adopter's captures 4d1e52/9de453: a green
/// exit from a command that had stopped measuring what it claimed. The exit stays the fact;
/// the command line, duration, output size and a dirty-tree flag make a false green visible.
const V3: &str = r#"
ALTER TABLE verify_runs ADD COLUMN command TEXT;
ALTER TABLE verify_runs ADD COLUMN duration_ms INTEGER;
ALTER TABLE verify_runs ADD COLUMN output_bytes INTEGER;
ALTER TABLE verify_runs ADD COLUMN dirty INTEGER NOT NULL DEFAULT 0;
"#;

/// v4 (2026-08-21, owner rulings A and E): named-resource leases (ported from
/// `the adopter's scripts/lease.sh`: identity is the worktree, liveness is the pid + its start
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

/// v5 (2026-08-21, the adopter): the `claude` pid on the session row so "gone" can
/// mean the process is gone, not "no hook yet".
const V5: &str = r#"
ALTER TABLE sessions ADD COLUMN pid INTEGER;
"#;

/// v6 (2026-08-21, the adopter: Stop advisory repeated every turn while a worker was blocked):
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
/// The adopter 2026-08-22, air-19u) degrades to stale counts instead of an empty status.
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

/// v11 (2026-08-29, air-4cr + air-bxe): the two facts a coordinator had to relay or `pgrep`
/// for. `verify_inflight` is one row per verify that has STARTED and not yet exited, so
/// "someone is mid-verify" is a lookup instead of a warning a worker has to remember to send
/// (the adopter 2026-08-23: a full verify is ~420 s and the landing rate is faster, so no
/// cadence works). `landings.pid` lets an `in-flight` landing row say whether the process that
/// wrote it is still alive, which is what `pgrep` was being asked and answered wrongly twice.
const V11: &str = r#"
CREATE TABLE IF NOT EXISTS verify_inflight (
    id          TEXT PRIMARY KEY,            -- ulid, matches the verify_runs row written at exit
    worker      TEXT NOT NULL,
    sha         TEXT NOT NULL,
    kind        TEXT NOT NULL,
    command     TEXT NOT NULL,
    pid         INTEGER,                     -- the `air record` process; liveness, as sessions do
    started_at  TEXT NOT NULL
);
ALTER TABLE landings ADD COLUMN pid INTEGER;
"#;

/// v12 (2026-08-29, air-air): which model a session is running. The coordinator had no way to
/// answer "which model is this worker on" except by asking the worker, and a wrong model that is
/// invisible costs the round while a visible one costs a relaunch.
///
/// Recorded, never inferred: the value is read out of the session's own transcript, which carries
/// `"model":"<id>"` on every assistant message. So a session launched with no `--model` records
/// what it ACTUALLY inherited rather than what a settings file suggests it might have.
const V12: &str = r#"
ALTER TABLE sessions ADD COLUMN model TEXT NOT NULL DEFAULT '';
"#;

/// v13 (2026-09-05, air-srv): every `SendMessage`, content included. Owner ruling: "Let's record
/// every message in a database then, if it is just a hook on SendMessage, then it's easy."
/// Agents solve problems together over `SendMessage` and none of it reached the ledger unless
/// someone captured it by hand. This reverses half of air-q07: the event line still carries
/// recipient and byte count and never the text; the text lives here. `summary` is not stored:
/// it is model text about the message, not the message. Only the send side is a tool call, so
/// within one project this table is the whole conversation.
const V13: &str = r#"
CREATE TABLE IF NOT EXISTS messages (
    at           TEXT NOT NULL,
    session_id   TEXT NOT NULL,
    from_worker  TEXT NOT NULL,
    from_role    TEXT NOT NULL,
    project      TEXT NOT NULL,
    "to"         TEXT NOT NULL,
    bytes        INTEGER NOT NULL,
    content      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS messages_at ON messages(at);
"#;

/// v14 (2026-09-05, air-7wf): the tree a verify run verified, beside the commit. `air land`
/// builds the landing commit from the branch's tree (air-odv), so every landing is a NEW sha
/// over a tree that already carries a green, and main read "not green" after all three
/// landings on 2026-08-30. Recorded at write time rather than resolved at read time: it is a
/// fact about the run, it survives the commit becoming unreachable, and it keeps a git
/// shell-out off the gate's hot path. NULL on rows written before this version, which reads as
/// "tree unknown" and never matches.
const V14: &str = r#"
ALTER TABLE verify_runs ADD COLUMN tree TEXT;
CREATE INDEX IF NOT EXISTS verify_runs_tree ON verify_runs(tree, kind);
"#;

/// v15 (2026-09-05, air-9dg): whether the session's hooks see `AIR_ENFORCE=1`. The adopter ran
/// five hours believing close-with-proof was enforced while a second `--settings` had replaced
/// the env block that carried it, and nothing either project reads said so. Written by the
/// hook from ITS OWN environment on every session write, so it records what the gate actually
/// runs with, not what a launcher meant to pass. NULL on rows from before this version, which
/// `air status` reads as unknown and says nothing about; 0 on a worker is the finding.
const V15: &str = r#"
ALTER TABLE sessions ADD COLUMN enforce INTEGER;
"#;

/// v16 (2026-09-05, air-1bm): the verifies a landing chose to destroy. `air land` refuses while
/// a verify is in flight and `--despite-inflight` lands anyway; the runs it ran over are kept
/// on the row (JSON array of "<worker> at <sha> started <when>"), so "how often did the
/// coordinator choose to destroy a run rather than wait" is a query over this table, which is
/// the removal condition of the refusal. `[]` on every ordinary landing.
const V16: &str = r#"
ALTER TABLE landings ADD COLUMN despite_inflight TEXT;
"#;

/// v17 (2026-09-05, air-80x.2): the worker branch heads a landed batch contained, as a JSON
/// array of `{"worker","sha"}`. A verify lane merges several branches and lands once; the
/// members are what "which branches did this landing carry" and a red batch's report (child
/// 4) read. `[]` on an ordinary single-branch landing.
const V17: &str = r#"
ALTER TABLE landings ADD COLUMN members TEXT;
"#;

/// v18 (2026-09-05, air-80x.4): the members a verify run's commit contained, recorded by
/// `air record` at run time (same JSON as `landings.members`). A red batch has no landing
/// row, so this is where its members live for the report; `[]` at a worker's own head.
const V18: &str = r#"
ALTER TABLE verify_runs ADD COLUMN members TEXT;
"#;

/// v19 (2026-09-06, air-9ij): `main`'s sha when the run started, read by `air record` before
/// the command ran. "The green contains main" used to be evaluated against CURRENT main at
/// query time, so a green recorded over the main of its moment was silently disqualified the
/// instant main moved — by a landing OR by an ordinary commit on main, which is how an
/// adopter's coordinator invalidated a whole batch with one prose commit. This is the durable
/// half of that fact. `NULL` on rows written before v19, which fall back to current main.
const V19: &str = r#"
ALTER TABLE verify_runs ADD COLUMN main_sha TEXT;
"#;

/// v20 (2026-09-06, air-1n3): why a session stopped, when the reason was not "it finished".
/// A `Notification` or `StopFailure` hook writes `stopped_at`, `stopped_kind` (the
/// notification type, or `stop_failure`) and `stopped_text` (what the harness showed).
///
/// The night this comes from: seven sessions on one machine were stopped by one account
/// limit. Five had the harness's own auto-continue armed and were working again within 70
/// seconds of the reset; two did not, and the one that also had no scheduled task sat dead
/// for 79 minutes. Air saw only "silent with a claim" and could not tell those apart, so the
/// coordinator took it to the owner instead of acting. These three columns are the fact that
/// distinction needs. NULL on every row until a session is actually stopped this way.
const V20: &str = r#"
ALTER TABLE sessions ADD COLUMN stopped_at TEXT;
ALTER TABLE sessions ADD COLUMN stopped_kind TEXT;
ALTER TABLE sessions ADD COLUMN stopped_text TEXT;
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
    if version < 11 {
        conn.execute_batch(V11)?;
        conn.pragma_update(None, "user_version", 11)?;
    }
    if version < 12 {
        conn.execute_batch(V12)?;
        conn.pragma_update(None, "user_version", 12)?;
    }
    if version < 13 {
        conn.execute_batch(V13)?;
        conn.pragma_update(None, "user_version", 13)?;
    }
    if version < 14 {
        conn.execute_batch(V14)?;
        conn.pragma_update(None, "user_version", 14)?;
    }
    if version < 15 {
        conn.execute_batch(V15)?;
        conn.pragma_update(None, "user_version", 15)?;
    }
    if version < 16 {
        conn.execute_batch(V16)?;
        conn.pragma_update(None, "user_version", 16)?;
    }
    if version < 17 {
        conn.execute_batch(V17)?;
        conn.pragma_update(None, "user_version", 17)?;
    }
    if version < 18 {
        conn.execute_batch(V18)?;
        conn.pragma_update(None, "user_version", 18)?;
    }
    if version < 19 {
        conn.execute_batch(V19)?;
        conn.pragma_update(None, "user_version", 19)?;
    }
    if version < 20 {
        conn.execute_batch(V20)?;
        conn.pragma_update(None, "user_version", 20)?;
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
                 ('verify_runs','edit_journal','claims','sessions','landings','captures','leases','lease_wants','verify_inflight','messages')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 10);
    }

    /// air-air: v12 adds `sessions.model` by ALTER, so the case that matters is an EXISTING
    /// database with rows in it, not a fresh one. A migration tested only from empty is tested
    /// in the one state no real ledger is ever in.
    #[test]
    fn v12_adds_the_model_column_to_a_populated_sessions_table() {
        let conn = Connection::open_in_memory().unwrap();
        // Stop at v11, the state every live ledger was in before this change.
        conn.execute_batch(V1).unwrap();
        conn.execute_batch(V2).unwrap();
        conn.execute(
            "INSERT INTO sessions (session_id, worker, state, changed_at, started_at) \
             VALUES ('s1','diligence','working','t','t')",
            [],
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 11).unwrap();

        migrate(&conn).unwrap();

        // The row survived, and its model is empty rather than absent: an honest unknown that
        // the next hook fills in from the transcript.
        let (worker, model): (String, String) = conn
            .query_row(
                "SELECT worker, model FROM sessions WHERE session_id='s1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(worker, "diligence");
        assert_eq!(model, "");
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, CURRENT_VERSION);
    }

    /// air-80x.4: v18 adds `verify_runs.members` by ALTER; a run from before reads as none.
    #[test]
    fn v18_adds_run_members_and_old_rows_read_as_none() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(V1).unwrap();
        conn.execute(
            "INSERT INTO verify_runs (id, worker, sha, kind, exit_code, trigger, started_at, \
             finished_at) VALUES ('r1','w','aaa','verify',2,'record','t','t')",
            [],
        )
        .unwrap();
        for v in [V13, V14, V15, V16, V17] {
            conn.execute_batch(v).unwrap();
        }
        conn.pragma_update(None, "user_version", 17).unwrap();

        migrate(&conn).unwrap();

        let v: Option<String> = conn
            .query_row("SELECT members FROM verify_runs WHERE id='r1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(v, None);
    }

    /// air-80x.2: v17 adds `landings.members` by ALTER; a row from before reads as no members.
    #[test]
    fn v17_adds_members_and_old_rows_read_as_none() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(V1).unwrap();
        conn.execute(
            "INSERT INTO landings (id, worker, sha, result, started_at, finished_at) \
             VALUES ('L1','w','aaa','landed','t','t')",
            [],
        )
        .unwrap();
        for v in [V13, V14, V15, V16] {
            conn.execute_batch(v).unwrap();
        }
        conn.pragma_update(None, "user_version", 16).unwrap();

        migrate(&conn).unwrap();

        let v: Option<String> = conn
            .query_row("SELECT members FROM landings WHERE id='L1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(v, None);
    }

    /// air-1bm: v16 adds `landings.despite_inflight` by ALTER; a row from before reads as no
    /// override, never as an error.
    #[test]
    fn v16_adds_despite_inflight_and_old_rows_read_as_no_override() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(V1).unwrap();
        conn.execute(
            "INSERT INTO landings (id, worker, sha, result, started_at, finished_at) \
             VALUES ('L1','w','aaa','landed','t','t')",
            [],
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 15).unwrap();
        // v13 to v15 must exist for the migration to run over them; apply them as v15 would.
        conn.execute_batch(V13).unwrap();
        conn.execute_batch(V14).unwrap();
        conn.execute_batch(V15).unwrap();

        migrate(&conn).unwrap();

        let v: Option<String> = conn
            .query_row(
                "SELECT despite_inflight FROM landings WHERE id='L1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v, None);
    }

    /// air-7wf: v14 adds `verify_runs.tree` by ALTER. A run recorded before it has no tree, and
    /// a tree lookup must not match it: NULL is "unknown", never "any".
    #[test]
    fn v14_adds_a_tree_column_that_old_rows_leave_null() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(V1).unwrap();
        conn.execute_batch(V13).unwrap();
        conn.execute(
            "INSERT INTO verify_runs (id, worker, sha, kind, exit_code, trigger, started_at, \
             finished_at) VALUES ('r1','w','aaa','verify',0,'record','t','t')",
            [],
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 13).unwrap();

        migrate(&conn).unwrap();

        let tree: Option<String> = conn
            .query_row("SELECT tree FROM verify_runs WHERE id='r1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(tree, None);
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM verify_runs WHERE tree = 'anything'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
    }
}
