//! Every `SendMessage`, content included (air-srv; owner ruling 2026-09-05, decisions.md).
//!
//! Agents solve problems together over `SendMessage`, and nothing of that reached the ledger
//! unless someone captured it by hand. The event line stays as air-q07 left it: recipient and
//! byte count, never the text. The text lives here, in a table, where `sqlite3 .air/ledger.db`
//! reads it and `air doctor` counts it. There is no read command; one becomes a bead when a
//! use turns up that sqlite does not serve.
//!
//! Only the SEND side is a tool call: an incoming message is user-turn text and no hook sees
//! it. Within one project every message is some session's send, so this table holds the whole
//! conversation; a message from another fleet's session is in that fleet's ledger.

use rusqlite::params;
use serde::Serialize;

use crate::{Ledger, Result};

/// One sent message as the hook saw it. `summary` is deliberately absent: it is model text
/// about the message, not the message (air-q07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub at: String,
    pub session_id: String,
    pub from_worker: String,
    pub from_role: String,
    pub project: String,
    pub to: String,
    pub bytes: i64,
    pub content: String,
}

const COLS: &str = "at, session_id, from_worker, from_role, project, \"to\", bytes, content";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        at: r.get(0)?,
        session_id: r.get(1)?,
        from_worker: r.get(2)?,
        from_role: r.get(3)?,
        project: r.get(4)?,
        to: r.get(5)?,
        bytes: r.get(6)?,
        content: r.get(7)?,
    })
}

impl Ledger {
    /// Append one message. Every call is one row: two identical sends are two messages, so
    /// there is no key and no dedupe.
    pub fn record_message(&self, m: &Message) -> Result<()> {
        self.conn.execute(
            "INSERT INTO messages (at, session_id, from_worker, from_role, project, \"to\", bytes, content) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                m.at,
                m.session_id,
                m.from_worker,
                m.from_role,
                m.project,
                m.to,
                m.bytes,
                m.content
            ],
        )?;
        Ok(())
    }

    /// Every recorded message, oldest first (ties keep insertion order).
    pub fn messages(&self) -> Result<Vec<Message>> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {COLS} FROM messages ORDER BY at, rowid"))?;
        let rows = stmt.query_map([], row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn msg(at: &str, content: &str) -> Message {
        Message {
            at: at.to_string(),
            session_id: "s1".to_string(),
            from_worker: "alpha".to_string(),
            from_role: "worker".to_string(),
            project: "air".to_string(),
            to: "main".to_string(),
            bytes: content.len() as i64,
            content: content.to_string(),
        }
    }

    #[test]
    fn every_send_is_one_row_and_identical_sends_are_two() {
        let l = Ledger::open_in_memory().unwrap();
        l.record_message(&msg("2026-09-05T00:00:01Z", "the plan"))
            .unwrap();
        l.record_message(&msg("2026-09-05T00:00:01Z", "the plan"))
            .unwrap();
        let all = l.messages().unwrap();
        assert_eq!(all.len(), 2, "no dedupe: a repeated message was sent twice");
        assert_eq!(all[0].content, "the plan");
        assert_eq!(all[0].to, "main");
        assert_eq!(all[0].bytes, 8);
    }
}
