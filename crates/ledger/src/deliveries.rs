//! Messages Air delivers into a session, one row per message and recipient (air-1vri).
//!
//! Air writes a row here when it has something to tell one session: new beads are ready, a
//! branch is batch-ready, a batch went green, the fleet is stopped. Each session's `air mcp`
//! channel reads the rows addressed to it, pushes them into its session, and marks them
//! delivered. The recipient is a checkout name (`w1`, `lane`, `coordinator`), because that is
//! the identity `air mcp` has: it runs in the session's cwd, and `paths::worker_name_for` turns
//! that into the name.
//!
//! `(to_worker, kind, key)` is unique, so a producer that runs twice, or two producers that see
//! the same change, write one row. That is what "once per change" means here: the key names the
//! change. A kind may also be superseding: a new row deletes the recipient's UNDELIVERED rows of
//! the same kind, so a session that was away comes back to the latest "beads are ready", not to
//! every one it missed. Delivered rows stay; they are the record the measurements read.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{Ledger, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Delivery {
    pub id: i64,
    pub to_worker: String,
    pub kind: String,
    /// What this row is about, for de-duplication and for measurement: a ready set, a sha,
    /// `worker@sha`. Never shown to the session.
    pub key: String,
    /// Machine-readable subject the measurements read (the beads of a batch result, the head
    /// of a batch-ready branch). Empty when the kind has none.
    pub subject: String,
    pub content: String,
    pub created_at: String,
    pub delivered_at: Option<String>,
}

const COLS: &str = "id, to_worker, kind, key, subject, content, created_at, delivered_at";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Delivery> {
    Ok(Delivery {
        id: r.get(0)?,
        to_worker: r.get(1)?,
        kind: r.get(2)?,
        key: r.get(3)?,
        subject: r.get(4)?,
        content: r.get(5)?,
        created_at: r.get(6)?,
        delivered_at: r.get(7)?,
    })
}

/// One message to queue.
#[derive(Debug, Clone, Default)]
pub struct Outgoing<'a> {
    pub to: &'a str,
    pub kind: &'a str,
    pub key: &'a str,
    pub subject: &'a str,
    pub content: &'a str,
    /// Delete the recipient's undelivered rows of this kind first.
    pub supersede: bool,
}

impl Ledger {
    /// Queue one message. `Ok(false)` when a row with the same recipient, kind and key already
    /// exists, delivered or not: the change it names was already told.
    pub fn enqueue_delivery(&self, m: &Outgoing<'_>, at: &str) -> Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        let exists: Option<i64> = tx
            .query_row(
                "SELECT id FROM deliveries WHERE to_worker=?1 AND kind=?2 AND key=?3",
                params![m.to, m.kind, m.key],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_some() {
            return Ok(false);
        }
        if m.supersede {
            tx.execute(
                "DELETE FROM deliveries WHERE to_worker=?1 AND kind=?2 AND delivered_at IS NULL",
                params![m.to, m.kind],
            )?;
        }
        tx.execute(
            "INSERT INTO deliveries (to_worker, kind, key, subject, content, created_at) \
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![m.to, m.kind, m.key, m.subject, m.content, at],
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// Record a message the recipient was already told some other way (a command's own
    /// output), delivered at `at`, so no channel tells it again. `Ok(false)` when it was
    /// already recorded.
    pub fn record_told(&self, m: &Outgoing<'_>, at: &str) -> Result<bool> {
        let n = self.conn.execute(
            "INSERT OR IGNORE INTO deliveries \
             (to_worker, kind, key, subject, content, created_at, delivered_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?6)",
            params![m.to, m.kind, m.key, m.subject, m.content, at],
        )?;
        Ok(n > 0)
    }

    /// Take every undelivered row for `to`, oldest first, and mark them delivered at `at`. One
    /// transaction, so two readers for one name never both deliver a row.
    pub fn take_deliveries(&self, to: &str, at: &str) -> Result<Vec<Delivery>> {
        let tx = self.conn.unchecked_transaction()?;
        let rows: Vec<Delivery> = {
            let mut st = tx.prepare(&format!(
                "SELECT {COLS} FROM deliveries WHERE to_worker=?1 AND delivered_at IS NULL \
                 ORDER BY id"
            ))?;
            st.query_map(params![to], row)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for d in &rows {
            tx.execute(
                "UPDATE deliveries SET delivered_at=?1 WHERE id=?2",
                params![at, d.id],
            )?;
        }
        tx.commit()?;
        Ok(rows
            .into_iter()
            .map(|mut d| {
                d.delivered_at = Some(at.to_string());
                d
            })
            .collect())
    }

    /// Every row created at or after `since`, oldest first, delivered or not.
    pub fn deliveries_since(&self, since: &str) -> Result<Vec<Delivery>> {
        let mut st = self.conn.prepare(&format!(
            "SELECT {COLS} FROM deliveries WHERE created_at >= ?1 ORDER BY id"
        ))?;
        let v = st
            .query_map(params![since], row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn out<'a>(to: &'a str, kind: &'a str, key: &'a str, supersede: bool) -> Outgoing<'a> {
        Outgoing {
            to,
            kind,
            key,
            subject: "",
            content: key,
            supersede,
        }
    }

    #[test]
    fn a_change_is_queued_once_and_taken_once_by_its_recipient() {
        let l = Ledger::open_in_memory().unwrap();
        assert!(
            l.enqueue_delivery(&out("w1", "k", "a", false), "t1")
                .unwrap()
        );
        assert!(
            !l.enqueue_delivery(&out("w1", "k", "a", false), "t2")
                .unwrap()
        );
        assert!(
            l.enqueue_delivery(&out("w2", "k", "a", false), "t1")
                .unwrap()
        );
        assert!(l.take_deliveries("w3", "t3").unwrap().is_empty());
        let got = l.take_deliveries("w1", "t3").unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].delivered_at.as_deref(), Some("t3"));
        assert!(l.take_deliveries("w1", "t4").unwrap().is_empty());
        // Delivered rows still de-duplicate: the change was told.
        assert!(
            !l.enqueue_delivery(&out("w1", "k", "a", false), "t5")
                .unwrap()
        );
    }

    #[test]
    fn a_superseding_kind_keeps_only_the_latest_undelivered_row() {
        let l = Ledger::open_in_memory().unwrap();
        l.enqueue_delivery(&out("w1", "ready", "a", true), "t1")
            .unwrap();
        l.enqueue_delivery(&out("w1", "ready", "ab", true), "t2")
            .unwrap();
        l.enqueue_delivery(&out("w1", "other", "x", false), "t2")
            .unwrap();
        let got = l.take_deliveries("w1", "t3").unwrap();
        let keys: Vec<&str> = got.iter().map(|d| d.key.as_str()).collect();
        assert_eq!(keys, ["ab", "x"]);
        assert_eq!(l.deliveries_since("t0").unwrap().len(), 2);
    }
}
