//! The capture inbox: one frictionless line from a worker, not `ready`, triaged by the
//! coordinator before it becomes a bead (decisions 2026-08-18: workers capture, they do not
//! file; 2026-08-20: `bd create` hard-denied for workers).

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{Ledger, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Capture {
    pub id: String,
    pub worker: String,
    pub session_id: Option<String>,
    pub text: String,
    pub captured_at: String,
    pub status: String,
    pub resolved_at: Option<String>,
    pub bead: Option<String>,
    pub note: Option<String>,
    /// `coordinator` (default) or `owner` (the owner's decision queue; ruling E).
    pub audience: String,
}

const COLS: &str =
    "id, worker, session_id, text, captured_at, status, resolved_at, bead, note, audience";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Capture> {
    Ok(Capture {
        id: r.get(0)?,
        worker: r.get(1)?,
        session_id: r.get(2)?,
        text: r.get(3)?,
        captured_at: r.get(4)?,
        status: r.get(5)?,
        resolved_at: r.get(6)?,
        bead: r.get(7)?,
        note: r.get(8)?,
        audience: r.get(9)?,
    })
}

impl Ledger {
    pub fn capture(
        &self,
        id: &str,
        worker: &str,
        session_id: Option<&str>,
        text: &str,
        at: &str,
    ) -> Result<()> {
        self.capture_for(id, worker, session_id, text, at, "coordinator")
    }

    pub fn capture_for(
        &self,
        id: &str,
        worker: &str,
        session_id: Option<&str>,
        text: &str,
        at: &str,
        audience: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO captures (id, worker, session_id, text, captured_at, audience) VALUES (?1,?2,?3,?4,?5,?6)",
            params![id, worker, session_id, text, at, audience],
        )?;
        Ok(())
    }

    pub fn capture_by_id(&self, id: &str) -> Result<Option<Capture>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLS} FROM captures WHERE id=?1"),
                params![id],
                row,
            )
            .optional()?)
    }

    /// Open captures for the coordinator, oldest first (the inbox).
    pub fn inbox(&self) -> Result<Vec<Capture>> {
        self.inbox_for("coordinator")
    }

    /// Open captures for an audience (`coordinator` or `owner`), oldest first.
    pub fn inbox_for(&self, audience: &str) -> Result<Vec<Capture>> {
        let mut st = self.conn.prepare(&format!(
            "SELECT {COLS} FROM captures WHERE status='open' AND audience=?1 ORDER BY captured_at"
        ))?;
        let v = st
            .query_map(params![audience], row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Triage outcome. `status` is `promoted` (with `bead`) or `dropped` (with `note`).
    /// Returns false when the capture is not open.
    pub fn resolve_capture(
        &self,
        id: &str,
        status: &str,
        bead: Option<&str>,
        note: Option<&str>,
        at: &str,
    ) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE captures SET status=?2, bead=?3, note=?4, resolved_at=?5 \
             WHERE id=?1 AND status='open'",
            params![id, status, bead, note, at],
        )?;
        Ok(n > 0)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn inbox_orders_and_resolves_once() {
        let l = Ledger::open_in_memory().unwrap();
        l.capture("b", "w1", Some("s"), "second", "t2").unwrap();
        l.capture("a", "w2", None, "first", "t1").unwrap();
        let inbox = l.inbox().unwrap();
        assert_eq!(
            inbox.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert!(
            l.resolve_capture("a", "promoted", Some("fd-9"), None, "t3")
                .unwrap()
        );
        assert!(
            !l.resolve_capture("a", "dropped", None, Some("dup"), "t4")
                .unwrap()
        );
        assert_eq!(l.inbox().unwrap().len(), 1);
        let a = l.capture_by_id("a").unwrap().unwrap();
        assert_eq!(
            (a.status.as_str(), a.bead.as_deref()),
            ("promoted", Some("fd-9"))
        );
    }
}
