//! `/api/attention`: what needs a person (air-o6x.1). The shape is final; air-o6x.3 fills it
//! from `fleet_stop`, `conditions`, `captures`, `leases` and `lease_wants`, never computing a
//! condition itself. This bead answers the empty shape.

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
    pub by: String,
    pub reason: String,
    pub at: String,
}

/// One open condition.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Condition {
    /// `idle-with-claim`, `done-refused`, ...
    pub kind: String,
    pub worker: String,
    pub subject: String,
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

/// Attention: the empty shape in this bead, whatever the ledger holds.
pub fn attention(_ledger: Option<&Ledger>, at: &str) -> Attention {
    Attention {
        at: at.to_string(),
        fleet_stop: None,
        conditions: Vec::new(),
        captures: Vec::new(),
        leases: Vec::new(),
        lease_wants: Vec::new(),
    }
}
