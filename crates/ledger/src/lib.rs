//! Air's ledger: the facts git and `bd` cannot re-derive.
//!
//! Design (docs/design.md): one SQLite file at the *main* checkout
//! (`<git-common-dir>/../.air/ledger.db`), WAL mode, shared by every worktree; an append-only
//! NDJSON events log next to it. Rows live until their state condition is false — no
//! time-based expiry (docs/decisions.md 2026-08-18).
//!
//! Everything here is synchronous and cheap: the hook binary opens the ledger, runs one or two
//! statements, and exits. Anything slow (`bd`, cross-worktree `git status`) lives in the CLI,
//! never on a hook path (tick 0315).

pub mod budgets;
pub mod captures;
pub mod claims;
pub mod deliveries;
pub mod events;
pub mod fleet;
pub mod landings;
pub mod leases;
pub mod messages;
pub mod paths;
pub mod schema;
pub mod verify;

use std::path::{Path, PathBuf};

use rusqlite::Connection;

/// Errors from the ledger. Typed per crate (plan 0003 §1, decided): callers match on
/// variants; the CLI edge turns them into one line of text or one JSON object.
#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("not inside a git repository: {0}")]
    NotARepo(PathBuf),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, LedgerError>;

/// An open ledger: a connection plus the directory it lives in (for the events log).
#[derive(Debug)]
pub struct Ledger {
    conn: Connection,
    dir: PathBuf,
}

impl Ledger {
    /// Open (creating if needed) the ledger under `air_dir` (normally `<repo>/.air`).
    /// Applies WAL + busy_timeout + synchronous=NORMAL and the schema.
    pub fn open_in(air_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(air_dir).map_err(|source| LedgerError::Io {
            path: air_dir.to_path_buf(),
            source,
        })?;
        let db_path = air_dir.join("ledger.db");
        let conn = Connection::open(&db_path)?;
        configure(&conn)?;
        schema::migrate(&conn)?;
        Ok(Self {
            conn,
            dir: air_dir.to_path_buf(),
        })
    }

    /// Open the ledger for the repository containing `cwd` (walks to the git common dir so
    /// every worktree shares one file).
    pub fn open_for_repo(cwd: &Path) -> Result<Self> {
        let air_dir = paths::air_dir_for(cwd)?;
        Self::open_in(&air_dir)
    }

    /// In-memory ledger for tests and probes. Same schema, no file, no events log on disk
    /// (events go to a temp dir owned by the caller if needed).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::migrate(&conn)?;
        Ok(Self {
            conn,
            dir: std::env::temp_dir(),
        })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Ledger {
    /// Last successful bd answer for `key`: (value, seen_at). `status` falls back to this
    /// when bd does not answer in time (air-19u).
    pub fn bd_cache_get(&self, key: &str) -> Result<Option<(String, String)>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .conn
            .query_row(
                "SELECT value, seen_at FROM bd_cache WHERE key=?1",
                rusqlite::params![key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    pub fn bd_cache_put(&self, key: &str, value: &str, now: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO bd_cache (key, value, seen_at) VALUES (?1,?2,?3) \
             ON CONFLICT(key) DO UPDATE SET value=excluded.value, seen_at=excluded.seen_at",
            rusqlite::params![key, value, now],
        )?;
        Ok(())
    }

    /// Record the set of conditions holding now: opens rows for new (worker, kind), touches
    /// `last_seen` on existing ones, and clears rows whose condition is gone. Pure bookkeeping;
    /// the caller evaluated the conditions. Returns (opened, cleared).
    pub fn record_conditions(
        &self,
        current: &[(String, &str, String)],
        now: &str,
    ) -> Result<(usize, usize)> {
        let mut opened = 0usize;
        let mut cleared = 0usize;
        let mut open_rows = self
            .conn
            .prepare("SELECT worker, kind FROM conditions WHERE cleared_at IS NULL")?;
        let existing: Vec<(String, String)> = open_rows
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (w, k, detail) in current {
            if existing.iter().any(|(ew, ek)| ew == w && ek == k) {
                self.conn.execute(
                    "UPDATE conditions SET last_seen=?3, detail=?4 WHERE worker=?1 AND kind=?2 AND cleared_at IS NULL",
                    rusqlite::params![w, k, now, detail],
                )?;
            } else {
                self.conn.execute(
                    "INSERT INTO conditions (worker, kind, first_seen, last_seen, detail) VALUES (?1,?2,?3,?3,?4)",
                    rusqlite::params![w, k, now, detail],
                )?;
                opened = opened.saturating_add(1);
            }
        }
        for (ew, ek) in &existing {
            if !current.iter().any(|(w, k, _)| w == ew && *k == ek.as_str()) {
                self.conn.execute(
                    "UPDATE conditions SET cleared_at=?3 WHERE worker=?1 AND kind=?2 AND cleared_at IS NULL",
                    rusqlite::params![ew, ek, now],
                )?;
                cleared = cleared.saturating_add(1);
            }
        }
        Ok((opened, cleared))
    }

    /// Should a hook say this now? True when `fingerprint` differs from what this session was
    /// last told under `key` (or nothing was); records it. A repeat of the identical message
    /// is silent; it re-arms when the fingerprint changes. Pass an empty fingerprint to clear
    /// (the condition went away) so the next occurrence speaks again.
    pub fn emit_if_changed(
        &self,
        session_id: &str,
        key: &str,
        fingerprint: &str,
        now: &str,
    ) -> Result<bool> {
        if fingerprint.is_empty() {
            self.conn.execute(
                "DELETE FROM hook_emissions WHERE session_id=?1 AND key=?2",
                rusqlite::params![session_id, key],
            )?;
            return Ok(false);
        }
        let n = self.conn.execute(
            "INSERT INTO hook_emissions (session_id, key, fingerprint, emitted_at) VALUES (?1,?2,?3,?4) \
             ON CONFLICT(session_id, key) DO UPDATE SET fingerprint=excluded.fingerprint, emitted_at=excluded.emitted_at \
             WHERE hook_emissions.fingerprint <> excluded.fingerprint",
            rusqlite::params![session_id, key, fingerprint, now],
        )?;
        Ok(n > 0)
    }

    /// What a session was last told for `key`: (fingerprint, emitted_at), if anything.
    pub fn last_emission(&self, session_id: &str, key: &str) -> Result<Option<(String, String)>> {
        let mut st = self.conn.prepare(
            "SELECT fingerprint, emitted_at FROM hook_emissions WHERE session_id=?1 AND key=?2",
        )?;
        let mut rows = st.query(rusqlite::params![session_id, key])?;
        Ok(match rows.next()? {
            Some(r) => Some((r.get(0)?, r.get(1)?)),
            None => None,
        })
    }
}

/// How long a writer waits for the WAL write lock before giving up with `SQLITE_BUSY`.
///
/// **Fail direction: OPEN, and that is why it is measured.** A hook that cannot get the lock
/// errors; `air hook` turns any internal error into exit 0 with a `fail-open` line, so the one
/// refusal Air makes silently does not refuse. The old value was 200 ms and nothing recorded a
/// single lock wait, so "zero busy errors" was case 3 of `do-less` — the input never arrived —
/// rather than evidence that 200 ms was enough.
///
/// Measured (`crates/ledger/tests/lock_waits.rs`, 6 writers — a coordinator, four workers and
/// a hook — × 60 immediate transactions against one WAL file, five runs on this Mac): 5–8
/// waits per 360 writes, p50 1 ms, and a max of 45, 95, 97, 100 and 131 ms.
///
/// **That tail is the reason this moved.** Contention is rare, but when it happens the wait
/// lands within a factor of two of the old 200 ms — one slow run and 200 ms is exceeded, and
/// what happens then is not a slow hook but a hook that fails open. The old value was sized
/// against nothing; the new one is sized against a measured max of 131 ms plus the cases the
/// measurement cannot reach (a WAL checkpoint, a cold `.air`, four concurrent verifies
/// competing for the disk). One second costs a slow hook at worst; the alternative costs a
/// refusal that does not happen. A budget that guards a refusal fails closed or is measured;
/// this one is now both.
///
/// The recorded number is an upper bound on the wait, not the wait: [`busy`] only observes at
/// its sleep boundaries, so a lock freed 60 ms in is recorded at the next boundary. It bounds
/// in the safe direction for sizing.
///
/// Moved by: `air audit`'s `sqlite-lock` row. If p99 approaches this, the fleet is contending
/// and the fix is fewer writers, not a longer wait; if a `hits` count is ever non-zero, a
/// refusal was skipped and this is too short.
pub const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1000);

/// SQLite's own backoff schedule, in milliseconds (`sqliteDefaultBusyCallback` in `main.c`),
/// kept so replacing `busy_timeout` with a measured handler does not also change the retry
/// behaviour underneath it.
const BACKOFF_MS: &[u64] = &[1, 2, 5, 10, 15, 20, 25, 25, 25, 50, 50, 100];

thread_local! {
    /// When the wait this thread is currently inside began. Set on the handler's first call
    /// for a locking event, which is the only signal SQLite gives that a new wait started.
    static WAIT_START: std::cell::Cell<Option<std::time::Instant>> = const {
        std::cell::Cell::new(None)
    };
}

/// `busy_timeout`'s behaviour, written out so the wait is recorded (air-d75).
///
/// `count` is how many times this handler has already run for the same locking event, so
/// `count == 0` is a new wait. Returning `true` retries; `false` gives up with `SQLITE_BUSY`.
///
/// The wait is recorded after each sleep rather than at the end, because SQLite never says a
/// wait succeeded — it simply stops calling. So the last recorded value for a successful wait
/// is the total time slept, which is the wait, and [`budgets::record_progress`] keeps it as
/// one sample rather than one per retry.
fn busy(count: i32) -> bool {
    let restart = count == 0;
    if restart {
        WAIT_START.with(|s| s.set(Some(std::time::Instant::now())));
    }
    let start = WAIT_START
        .with(std::cell::Cell::get)
        .unwrap_or_else(std::time::Instant::now);
    let so_far = start.elapsed();
    let left = BUSY_TIMEOUT.saturating_sub(so_far);
    if left.is_zero() {
        budgets::record_progress(budgets::SQLITE_LOCK, so_far, BUSY_TIMEOUT, restart, true);
        return false;
    }
    let step = usize::try_from(count).unwrap_or(usize::MAX);
    let delay = BACKOFF_MS
        .get(step.min(BACKOFF_MS.len().saturating_sub(1)))
        .copied()
        .unwrap_or(100);
    std::thread::sleep(std::time::Duration::from_millis(delay).min(left));
    budgets::record_progress(
        budgets::SQLITE_LOCK,
        start.elapsed(),
        BUSY_TIMEOUT,
        restart,
        false,
    );
    true
}

fn configure(conn: &Connection) -> Result<()> {
    // WAL: concurrent readers with one writer across worktrees; NORMAL is durable enough for
    // a ledger that can be rebuilt (only verify history and receipts have lasting value).
    // Sources: https://sqlite.org/wal.html, https://sqlite.org/pragma.html (tick 0315).
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    // Not `busy_timeout`: same behaviour, but the wait lands in the record (air-d75).
    conn.busy_handler(Some(busy))?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod emission_tests {
    use super::Ledger;

    #[test]
    fn conditions_open_touch_and_clear() {
        let l = Ledger::open_in_memory().unwrap();
        let cur = vec![("a".to_string(), "idle-with-claim", "x".to_string())];
        assert_eq!(l.record_conditions(&cur, "t1").unwrap(), (1, 0));
        assert_eq!(l.record_conditions(&cur, "t2").unwrap(), (0, 0));
        assert_eq!(l.record_conditions(&[], "t3").unwrap(), (0, 1));
        let (first, cleared): (String, Option<String>) = l
            .conn()
            .query_row("SELECT first_seen, cleared_at FROM conditions", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((first.as_str(), cleared.as_deref()), ("t1", Some("t3")));
        assert_eq!(
            l.record_conditions(&cur, "t4").unwrap(),
            (1, 0),
            "re-opens as a new row"
        );
    }

    #[test]
    fn speaks_once_per_change_and_rearms_on_clear() {
        let l = Ledger::open_in_memory().unwrap();
        assert!(
            l.emit_if_changed("s", "stop", "head1:verify", "t1")
                .unwrap()
        );
        assert!(
            !l.emit_if_changed("s", "stop", "head1:verify", "t2")
                .unwrap()
        );
        assert!(
            l.emit_if_changed("s", "stop", "head2:verify", "t3")
                .unwrap()
        );
        assert!(!l.emit_if_changed("s", "stop", "", "t4").unwrap()); // cleared
        assert!(
            l.emit_if_changed("s", "stop", "head2:verify", "t5")
                .unwrap()
        ); // re-armed
        assert!(
            l.emit_if_changed("other", "stop", "head2:verify", "t5")
                .unwrap()
        ); // per session
    }
}
