//! Air's ledger: the facts git and `bd` cannot re-derive.
//!
//! Design (docs/plans/0001-first-slice.md §2): one SQLite file at the *main* checkout
//! (`<git-common-dir>/../.air/ledger.db`), WAL mode, shared by every worktree; an append-only
//! NDJSON events log next to it. Rows live until their state condition is false — no
//! time-based expiry (docs/decisions.md 2026-08-18).
//!
//! Everything here is synchronous and cheap: the hook binary opens the ledger, runs one or two
//! statements, and exits. Anything slow (`bd`, cross-worktree `git status`) lives in the CLI,
//! never on a hook path (tick 0315).

pub mod captures;
pub mod claims;
pub mod events;
pub mod leases;
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

fn configure(conn: &Connection) -> Result<()> {
    // WAL: concurrent readers with one writer across worktrees; NORMAL is durable enough for
    // a ledger that can be rebuilt (only verify history and receipts have lasting value).
    // Sources: https://sqlite.org/wal.html, https://sqlite.org/pragma.html (tick 0315).
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_millis(200))?;
    Ok(())
}
