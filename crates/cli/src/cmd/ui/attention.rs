//! `/api/attention`: what needs a person (air-o6x.1, filled by air-o6x.3). Read from
//! `fleet_stop`, `conditions`, `captures`, `leases` and `lease_wants`, never computed here:
//! the coordinator's channel evaluates the conditions every 30 seconds and writes that table
//! (`mcp.rs`), so the page shows what the coordinator was told and nothing it was not.

use air_ledger::Ledger;
use serde::Serialize;

/// Attention as of `at`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Attention {
    /// When this answer was computed (UTC, RFC 3339).
    pub at: String,
    /// The standing fleet stop, if one.
    pub fleet_stop: Option<FleetStop>,
    /// Open attention conditions, as the coordinator's channel computed them.
    pub conditions: Vec<Condition>,
    /// Open captures.
    pub captures: Vec<Capture>,
    /// Held leases.
    pub leases: Vec<Lease>,
    /// Workers waiting for a lease.
    pub lease_wants: Vec<Want>,
}

/// A fleet stop.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FleetStop {
    /// Who set it: the role and the checkout name, as `coordinator (coordinator)`.
    pub by: String,
    pub reason: String,
    pub at: String,
}

/// One open condition.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Condition {
    /// `idle-with-claim`, `done-refused`, ...
    pub kind: String,
    /// The row's subject: a worker name, or `owner`.
    pub worker: String,
    /// The same as `worker`: the channel keys a condition by (subject, kind).
    pub subject: String,
    /// The condition's text as the coordinator was told it.
    pub text: String,
    /// When it was first seen (UTC, RFC 3339).
    pub since: String,
}

/// One open capture.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Capture {
    pub id: String,
    pub worker: String,
    pub at: String,
    /// The first line.
    pub text: String,
}

/// One held lease.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Lease {
    pub resource: String,
    pub holder: String,
    pub reason: String,
    pub since: String,
}

/// One wait for a lease.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Want {
    pub resource: String,
    pub worker: String,
    pub since: String,
}

/// Attention: the empty shape without a ledger, else the five tables as they stand.
pub fn attention(ledger: Option<&Ledger>, at: &str) -> Attention {
    let Some(l) = ledger else {
        return Attention {
            at: at.to_string(),
            fleet_stop: None,
            conditions: Vec::new(),
            captures: Vec::new(),
            leases: Vec::new(),
            lease_wants: Vec::new(),
        };
    };
    Attention {
        at: at.to_string(),
        fleet_stop: l.fleet_stop().ok().flatten().map(|s| FleetStop {
            by: format!("{} ({})", s.by_role, s.by_worker),
            reason: s.reason,
            at: s.stopped_at,
        }),
        conditions: conditions(l),
        captures: l
            .inbox()
            .unwrap_or_default()
            .into_iter()
            .map(|c| Capture {
                id: c.id,
                worker: c.worker,
                at: c.captured_at,
                text: c.text.lines().next().unwrap_or_default().to_string(),
            })
            .collect(),
        leases: l
            .leases()
            .unwrap_or_default()
            .into_iter()
            .map(|x| Lease {
                resource: x.resource,
                holder: x.worker,
                reason: x.reason,
                since: x.taken_at,
            })
            .collect(),
        lease_wants: wants(l),
    }
}

fn conditions(l: &Ledger) -> Vec<Condition> {
    let Ok(mut st) = l.conn().prepare(
        "SELECT worker, kind, first_seen, detail FROM conditions WHERE cleared_at IS NULL \
         ORDER BY first_seen",
    ) else {
        return Vec::new();
    };
    let rows = st.query_map([], |r| {
        let worker: String = r.get(0)?;
        Ok(Condition {
            kind: r.get(1)?,
            subject: worker.clone(),
            worker,
            text: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            since: r.get(2)?,
        })
    });
    match rows {
        Ok(it) => it.filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

fn wants(l: &Ledger) -> Vec<Want> {
    let Ok(mut st) = l
        .conn()
        .prepare("SELECT resource, worker, wanted_at FROM lease_wants ORDER BY wanted_at")
    else {
        return Vec::new();
    };
    let rows = st.query_map([], |r| {
        Ok(Want {
            resource: r.get(0)?,
            worker: r.get(1)?,
            since: r.get(2)?,
        })
    });
    match rows {
        Ok(it) => it.filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Every row kind written through the ledger's own writers comes back through the
    /// read-only open: a stop, an open and a cleared condition, an open and a promoted capture,
    /// a lease and a want.
    #[test]
    fn the_five_tables_round_trip_and_cleared_or_resolved_rows_are_left_out() {
        let tmp = tempfile::tempdir().unwrap();
        let air = tmp.path().join(".air");
        let w = Ledger::open_in(&air).unwrap();
        w.set_fleet_stop(&air_ledger::fleet::FleetStop {
            stopped_at: "2026-11-10T00:00:00Z".into(),
            by_worker: "coordinator".into(),
            by_role: "coordinator".into(),
            reason: "upgrade".into(),
        })
        .unwrap();
        w.record_conditions(
            &[
                (
                    "worker-1".into(),
                    "idle-with-claim",
                    "idle 7 min with zz-1".into(),
                ),
                ("worker-2".into(), "done-refused", "refused once".into()),
            ],
            "2026-11-10T00:01:00Z",
        )
        .unwrap();
        // worker-2's condition clears on the next tick.
        w.record_conditions(
            &[(
                "worker-1".into(),
                "idle-with-claim",
                "idle 8 min with zz-1".into(),
            )],
            "2026-11-10T00:02:00Z",
        )
        .unwrap();
        w.capture(
            "c1",
            "worker-3",
            None,
            "blocked on X\nmore",
            "2026-11-10T00:03:00Z",
            None,
        )
        .unwrap();
        w.capture("c2", "worker-3", None, "old", "2026-11-10T00:00:30Z", None)
            .unwrap();
        w.conn()
            .execute("UPDATE captures SET status='promoted' WHERE id='c2'", [])
            .unwrap();
        w.conn()
            .execute(
                "INSERT INTO leases (resource, worker, reason, taken_at, heartbeat_at) \
                 VALUES ('serve', 'worker-1', 'the trial', 't1', 't1')",
                [],
            )
            .unwrap();
        w.conn()
            .execute(
                "INSERT INTO lease_wants (resource, worker, wanted_at) VALUES ('serve', 'worker-2', 't2')",
                [],
            )
            .unwrap();
        drop(w);

        let r = Ledger::open_read_only(&air).unwrap();
        let a = attention(Some(&r), "2026-11-10T00:05:00Z");
        let stop = a.fleet_stop.unwrap();
        assert_eq!(
            (stop.by.as_str(), stop.reason.as_str()),
            ("coordinator (coordinator)", "upgrade")
        );
        assert_eq!(a.conditions.len(), 1);
        assert_eq!(a.conditions[0].kind, "idle-with-claim");
        assert_eq!(a.conditions[0].text, "idle 8 min with zz-1");
        assert_eq!(a.conditions[0].since, "2026-11-10T00:01:00Z");
        assert_eq!(a.captures.len(), 1);
        assert_eq!(
            (a.captures[0].id.as_str(), a.captures[0].text.as_str()),
            ("c1", "blocked on X")
        );
        assert_eq!(a.leases.len(), 1);
        assert_eq!(
            (a.leases[0].holder.as_str(), a.leases[0].since.as_str()),
            ("worker-1", "t1")
        );
        assert_eq!(a.lease_wants.len(), 1);
        assert_eq!(a.lease_wants[0].worker, "worker-2");
    }
}
