//! The capture inbox: one frictionless line from a worker, not `ready`, triaged by the
//! coordinator before it becomes a bead (decisions 2026-08-18: workers capture, they do not
//! file; 2026-08-20: `bd create` hard-denied for workers).
//!
//! One audience. The `audience` column stays (forward-only migrations) and is always
//! `coordinator` since air-uef (owner, 2026-09-05): the owner's queue is beads labelled
//! `owner`, filed by the coordinator with a recommendation, never worker prose. Rows an older
//! binary wrote with `audience = 'owner'` are listed by [`Ledger::inbox`] like any other open
//! capture, so nothing already captured is lost.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{Ledger, Result};

/// One triage decision as the ledger takes it: `(capture id, status, bead, note)`.
pub type TriageItem = (String, String, Option<String>, Option<String>);

/// What a capture pointed at before a triage: `(status, bead)`.
pub type Was = (String, Option<String>);

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
    /// Always `coordinator` on a new row (air-uef). Older rows may still say `owner`.
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
        self.conn.execute(
            "INSERT INTO captures (id, worker, session_id, text, captured_at, audience) VALUES (?1,?2,?3,?4,?5,'coordinator')",
            params![id, worker, session_id, text, at],
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

    /// Every open capture, oldest first (the inbox). Whatever audience a row was written
    /// with: the coordinator triages all of them (air-uef).
    pub fn inbox(&self) -> Result<Vec<Capture>> {
        let mut st = self.conn.prepare(&format!(
            "SELECT {COLS} FROM captures WHERE status='open' ORDER BY captured_at"
        ))?;
        let v = st
            .query_map([], row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Triage a whole pass in ONE transaction: each item is `(capture id, status, bead,
    /// note)`. Returns, per item and in order, what the capture pointed at *before* as
    /// `(status, bead)`, or `None` when there is no capture with that id.
    ///
    /// An already-triaged capture is re-pointed rather than refused (air-76z): a coordinator
    /// promoted two captures to placeholder ids that were never created, and refusing to
    /// touch a closed capture left the record wrong with no way to fix it. Correcting a
    /// pointer is cheap and the old value comes back here, so the event line keeps the
    /// history. One transaction and one event line for the pass (air-869).
    pub fn resolve_captures(&self, items: &[TriageItem], at: &str) -> Result<Vec<Option<Was>>> {
        let tx = self.conn.unchecked_transaction()?;
        let mut was = Vec::with_capacity(items.len());
        {
            let mut before = tx.prepare("SELECT status, bead FROM captures WHERE id=?1")?;
            let mut set = tx.prepare(
                "UPDATE captures SET status=?2, bead=?3, note=?4, resolved_at=?5 WHERE id=?1",
            )?;
            for (id, status, bead, note) in items {
                let prev: Option<Was> = before
                    .query_row(params![id], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()?;
                if prev.is_some() {
                    set.execute(params![id, status, bead, note, at])?;
                }
                was.push(prev);
            }
        }
        tx.commit()?;
        Ok(was)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn item(id: &str, status: &str, bead: Option<&str>, note: Option<&str>) -> TriageItem {
        (
            id.to_string(),
            status.to_string(),
            bead.map(str::to_string),
            note.map(str::to_string),
        )
    }

    #[test]
    fn inbox_orders_oldest_first_and_triage_closes_it() {
        let l = Ledger::open_in_memory().unwrap();
        l.capture("b", "w1", Some("s"), "second", "t2").unwrap();
        l.capture("a", "w2", None, "first", "t1").unwrap();
        let inbox = l.inbox().unwrap();
        assert_eq!(
            inbox.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        let was = l
            .resolve_captures(&[item("a", "promoted", Some("zz-9"), None)], "t3")
            .unwrap();
        assert_eq!(was[0], Some(("open".to_string(), None)));
        assert_eq!(l.inbox().unwrap().len(), 1);
        let a = l.capture_by_id("a").unwrap().unwrap();
        assert_eq!(
            (a.status.as_str(), a.bead.as_deref()),
            ("promoted", Some("zz-9"))
        );
    }

    /// air-869: one triage pass, one transaction. air-76z: an id no capture has is reported
    /// (None) rather than failing the pass, and an already-triaged capture is re-pointed
    /// with its old value returned.
    #[test]
    fn resolve_captures_triages_a_pass_and_repoints() {
        let l = Ledger::open_in_memory().unwrap();
        for (id, text) in [("a", "one"), ("b", "two"), ("c", "three")] {
            l.capture(id, "w1", None, text, "t0").unwrap();
        }
        let items = vec![
            item("a", "promoted", Some("zz-1"), None),
            item("b", "dropped", None, Some("dup")),
            item("nope", "promoted", Some("zz-3"), None),
        ];
        let was = l.resolve_captures(&items, "t1").unwrap();
        assert_eq!(was[0], Some(("open".to_string(), None)));
        assert_eq!(was[1], Some(("open".to_string(), None)));
        assert_eq!(was[2], None, "no capture with that id");
        assert_eq!(
            l.capture_by_id("b").unwrap().unwrap().note.as_deref(),
            Some("dup")
        );

        // Re-point `a` from a bead that was never created to the real one, and promote the
        // dropped `b`. Both say what they used to point at.
        let fix = vec![
            item("a", "promoted", Some("zz-real"), None),
            item("b", "promoted", Some("zz-2"), None),
        ];
        let was = l.resolve_captures(&fix, "t2").unwrap();
        assert_eq!(
            was[0],
            Some(("promoted".to_string(), Some("zz-1".to_string())))
        );
        assert_eq!(was[1], Some(("dropped".to_string(), None)));
        assert_eq!(
            l.capture_by_id("a").unwrap().unwrap().bead.as_deref(),
            Some("zz-real")
        );
        assert_eq!(
            l.capture_by_id("b").unwrap().unwrap().status.as_str(),
            "promoted"
        );
        // Re-pointing never puts a capture back in the inbox.
        assert!(l.inbox().unwrap().iter().all(|c| c.id == "c"));
    }
}
