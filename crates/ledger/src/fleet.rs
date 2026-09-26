//! The fleet-wide stop (air-1vri.1): one row while the fleet is stopped, none while it runs.
//!
//! Owner, 2026-09-26: the coordinator stops all work with one message to Air. `air fleet stop`
//! writes the row and `air fleet resume` deletes it; what happened when is in the event log and
//! in the `deliveries` rows that told each session.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{Ledger, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FleetStop {
    pub stopped_at: String,
    /// The checkout name of whoever stopped it (`coordinator`, or `main` for the owner's shell).
    pub by_worker: String,
    /// `coordinator` or `owner`.
    pub by_role: String,
    pub reason: String,
}

impl FleetStop {
    /// One line naming the stop and who set it, for every refusal and for `air status`.
    pub fn line(&self) -> String {
        let why = if self.reason.is_empty() {
            String::new()
        } else {
            format!(": {}", self.reason)
        };
        format!(
            "the fleet is stopped since {} by the {} ({}){why}",
            self.stopped_at, self.by_role, self.by_worker
        )
    }
}

impl Ledger {
    pub fn fleet_stop(&self) -> Result<Option<FleetStop>> {
        Ok(self
            .conn
            .query_row(
                "SELECT stopped_at, by_worker, by_role, reason FROM fleet_stop WHERE id=1",
                [],
                |r| {
                    Ok(FleetStop {
                        stopped_at: r.get(0)?,
                        by_worker: r.get(1)?,
                        by_role: r.get(2)?,
                        reason: r.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    /// Stop the fleet. A second stop keeps the first one's time and author and returns it.
    pub fn set_fleet_stop(&self, s: &FleetStop) -> Result<Option<FleetStop>> {
        if let Some(existing) = self.fleet_stop()? {
            return Ok(Some(existing));
        }
        self.conn.execute(
            "INSERT OR IGNORE INTO fleet_stop (id, stopped_at, by_worker, by_role, reason) \
             VALUES (1,?1,?2,?3,?4)",
            params![s.stopped_at, s.by_worker, s.by_role, s.reason],
        )?;
        Ok(None)
    }

    /// Resume: remove the stop, returning what it was.
    pub fn clear_fleet_stop(&self) -> Result<Option<FleetStop>> {
        let was = self.fleet_stop()?;
        self.conn.execute("DELETE FROM fleet_stop", [])?;
        Ok(was)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn stop_is_one_row_until_resumed() {
        let l = Ledger::open_in_memory().unwrap();
        assert!(l.fleet_stop().unwrap().is_none());
        let s = FleetStop {
            stopped_at: "t1".into(),
            by_worker: "coordinator".into(),
            by_role: "coordinator".into(),
            reason: "demo".into(),
        };
        assert!(l.set_fleet_stop(&s).unwrap().is_none());
        let later = FleetStop {
            stopped_at: "t2".into(),
            ..s.clone()
        };
        assert_eq!(l.set_fleet_stop(&later).unwrap(), Some(s.clone()));
        assert_eq!(l.fleet_stop().unwrap(), Some(s.clone()));
        assert!(s.line().contains("stopped since t1 by the coordinator"));
        assert_eq!(l.clear_fleet_stop().unwrap(), Some(s));
        assert!(l.fleet_stop().unwrap().is_none());
    }
}
