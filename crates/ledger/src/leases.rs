//! Named-resource leases: mutual exclusion for what two agents cannot share (ports, the
//! simulator, Docker lifecycle, the browser). Ported from `the adopter's scripts/lease.sh`
//! (read 2026-08-21): identity is the worktree, liveness is the pid plus its start time
//! (pid reuse), stale is heartbeat age. Races are settled by SQLite (`BEGIN IMMEDIATE`), not
//! by `mkdir`. Owner ruling A, 2026-08-21.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{Ledger, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lease {
    pub resource: String,
    pub worker: String,
    pub session_id: Option<String>,
    pub pid: Option<i64>,
    pub pid_started: Option<String>,
    pub reason: String,
    pub taken_at: String,
    pub heartbeat_at: String,
}

/// What a taker must supply about itself.
#[derive(Debug, Clone, Default)]
pub struct Holder<'a> {
    pub worker: &'a str,
    pub session_id: Option<&'a str>,
    pub pid: Option<i64>,
    pub pid_started: Option<&'a str>,
}

/// Outcome of `take`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Take {
    Taken,
    /// Already held by this worker; heartbeat refreshed.
    AlreadyMine,
    /// Taken after the previous holder was found defective (reason given).
    TakenAfter(String),
    /// Held by someone else and healthy.
    Held(Lease),
}

const COLS: &str = "resource, worker, session_id, pid, pid_started, reason, taken_at, heartbeat_at";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Lease> {
    Ok(Lease {
        resource: r.get(0)?,
        worker: r.get(1)?,
        session_id: r.get(2)?,
        pid: r.get(3)?,
        pid_started: r.get(4)?,
        reason: r.get(5)?,
        taken_at: r.get(6)?,
        heartbeat_at: r.get(7)?,
    })
}

impl Ledger {
    pub fn lease(&self, resource: &str) -> Result<Option<Lease>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLS} FROM leases WHERE resource=?1"),
                params![resource],
                row,
            )
            .optional()?)
    }

    pub fn leases(&self) -> Result<Vec<Lease>> {
        let mut st = self
            .conn
            .prepare(&format!("SELECT {COLS} FROM leases ORDER BY resource"))?;
        let v = st
            .query_map([], row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Take `resource`. `defect` is the caller's verdict on the current holder (None =
    /// healthy); the ledger cannot probe pids itself, so liveness is passed in. Atomic.
    pub fn lease_take(
        &self,
        resource: &str,
        holder: &Holder<'_>,
        reason: &str,
        now: &str,
        defect: impl Fn(&Lease) -> Option<String>,
    ) -> Result<Take> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("BEGIN IMMEDIATE", []).ok();
        let current = tx
            .query_row(
                &format!("SELECT {COLS} FROM leases WHERE resource=?1"),
                params![resource],
                row,
            )
            .optional()?;
        let outcome = match current {
            Some(l) if l.worker == holder.worker => {
                tx.execute(
                    "UPDATE leases SET heartbeat_at=?2, session_id=?3, pid=?4, pid_started=?5 WHERE resource=?1",
                    params![resource, now, holder.session_id, holder.pid, holder.pid_started],
                )?;
                Take::AlreadyMine
            }
            Some(l) => match defect(&l) {
                Some(why) => {
                    write_lease(&tx, resource, holder, reason, now)?;
                    Take::TakenAfter(why)
                }
                None => {
                    tx.execute(
                        "INSERT INTO lease_wants (resource, worker, reason, wanted_at) VALUES (?1,?2,?3,?4) \
                         ON CONFLICT(resource, worker) DO UPDATE SET reason=excluded.reason, wanted_at=excluded.wanted_at",
                        params![resource, holder.worker, reason, now],
                    )?;
                    Take::Held(l)
                }
            },
            None => {
                write_lease(&tx, resource, holder, reason, now)?;
                Take::Taken
            }
        };
        tx.commit()?;
        Ok(outcome)
    }

    /// Release if held by `worker`. Returns false when it was not.
    pub fn lease_release(&self, resource: &str, worker: &str) -> Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM leases WHERE resource=?1 AND worker=?2",
            params![resource, worker],
        )?;
        Ok(n > 0)
    }

    /// Force-clear regardless of holder. Returns the previous holder, if any.
    pub fn lease_break(&self, resource: &str) -> Result<Option<Lease>> {
        let prev = self.lease(resource)?;
        self.conn
            .execute("DELETE FROM leases WHERE resource=?1", params![resource])?;
        Ok(prev)
    }

    /// Refresh heartbeats for every lease `worker` holds (called from hooks; cheap).
    pub fn lease_beat(&self, worker: &str, now: &str) -> Result<usize> {
        Ok(self.conn.execute(
            "UPDATE leases SET heartbeat_at=?2 WHERE worker=?1",
            params![worker, now],
        )?)
    }

    /// Forget that `worker` waits for `resource`: it was told the lease is free (air-1vri.3).
    pub fn clear_lease_want(&self, resource: &str, worker: &str) -> Result<bool> {
        Ok(self.conn.execute(
            "DELETE FROM lease_wants WHERE resource=?1 AND worker=?2",
            params![resource, worker],
        )? > 0)
    }

    /// Who is waiting for `resource`.
    pub fn lease_wants(&self, resource: &str) -> Result<Vec<(String, Option<String>, String)>> {
        let mut st = self.conn.prepare(
            "SELECT worker, reason, wanted_at FROM lease_wants WHERE resource=?1 ORDER BY wanted_at",
        )?;
        let v = st
            .query_map(params![resource], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }
}

fn write_lease(
    tx: &rusqlite::Transaction<'_>,
    resource: &str,
    h: &Holder<'_>,
    reason: &str,
    now: &str,
) -> Result<()> {
    tx.execute(
        "INSERT INTO leases (resource, worker, session_id, pid, pid_started, reason, taken_at, heartbeat_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?7) \
         ON CONFLICT(resource) DO UPDATE SET worker=excluded.worker, session_id=excluded.session_id, \
         pid=excluded.pid, pid_started=excluded.pid_started, reason=excluded.reason, \
         taken_at=excluded.taken_at, heartbeat_at=excluded.heartbeat_at",
        params![resource, h.worker, h.session_id, h.pid, h.pid_started, reason, now],
    )?;
    tx.execute(
        "DELETE FROM lease_wants WHERE resource=?1 AND worker=?2",
        params![resource, h.worker],
    )?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    fn h(worker: &str) -> Holder<'static> {
        Holder {
            worker: Box::leak(worker.to_string().into_boxed_str()),
            session_id: Some("s"),
            pid: Some(1),
            pid_started: Some("x"),
        }
    }

    #[test]
    fn take_deny_steal_release() {
        let l = Ledger::open_in_memory().unwrap();
        let healthy = |_: &Lease| None;
        assert_eq!(
            l.lease_take("runtime", &h("a"), "api", "t0", healthy)
                .unwrap(),
            Take::Taken
        );
        assert_eq!(
            l.lease_take("runtime", &h("a"), "api", "t1", healthy)
                .unwrap(),
            Take::AlreadyMine
        );
        assert_eq!(l.lease("runtime").unwrap().unwrap().heartbeat_at, "t1");
        match l
            .lease_take("runtime", &h("b"), "sim", "t2", healthy)
            .unwrap()
        {
            Take::Held(x) => assert_eq!(x.worker, "a"),
            other => panic!("{other:?}"),
        }
        assert_eq!(l.lease_wants("runtime").unwrap()[0].0, "b");
        // Holder found dead: b takes it, and its want is cleared.
        let dead = |_: &Lease| Some("dead (pid gone)".to_string());
        assert_eq!(
            l.lease_take("runtime", &h("b"), "sim", "t3", dead).unwrap(),
            Take::TakenAfter("dead (pid gone)".into())
        );
        assert!(l.lease_wants("runtime").unwrap().is_empty());
        assert!(!l.lease_release("runtime", "a").unwrap());
        assert!(l.lease_release("runtime", "b").unwrap());
        assert!(l.lease("runtime").unwrap().is_none());
        // Independent resources do not interfere; beat touches only mine.
        l.lease_take(":8080", &h("a"), "api", "t4", healthy)
            .unwrap();
        l.lease_take("chrome", &h("b"), "shots", "t4", healthy)
            .unwrap();
        assert_eq!(l.lease_beat("a", "t9").unwrap(), 1);
        assert_eq!(l.lease("chrome").unwrap().unwrap().heartbeat_at, "t4");
        assert_eq!(l.lease_break("chrome").unwrap().unwrap().worker, "b");
    }
}
