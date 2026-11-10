//! `/api/beads`: the board, by column (air-o6x.1). The shape is final; air-o6x.3 fills it from
//! bd at the slow tier and from `claims`. This bead answers the empty shape.

use air_ledger::Ledger;
use serde::Serialize;

/// The board as of `at`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Beads {
    /// When this answer was computed (UTC, RFC 3339).
    pub at: String,
    /// When bd was last asked, or none before the first answer.
    pub fetched_at: Option<String>,
    /// Claimable now.
    pub ready: Vec<Bead>,
    /// Held and not yet declared done.
    pub in_progress: Vec<Bead>,
    /// Declared done, waiting for the landing that carries the declared head.
    pub done_waiting: Vec<Bead>,
    /// Closed recently, newest first.
    pub closed_recent: Vec<Bead>,
    /// Open with an open `blocks` dependency.
    pub blocked: Vec<Bead>,
}

/// One bead.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Bead {
    pub id: String,
    pub title: String,
    /// bd's status: `open`, `in_progress`, `closed`.
    pub status: String,
    /// bd's type: `task`, `bug`, `feature`, `epic`, `chore`.
    #[serde(rename = "type")]
    pub kind: String,
    /// bd's priority, none when unset.
    pub priority: Option<i64>,
    pub assignee: Option<String>,
    pub labels: Vec<String>,
    /// The ids of the open beads that block it.
    pub blocked_by: Vec<String>,
    /// The ledger's open claim, if one.
    pub claim: Option<Claim>,
    /// The done declaration on the claim, if one.
    pub done: Option<Done>,
    /// The landing that put the declared head on main, if one.
    pub landed: Option<Landed>,
}

/// An open claim.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Claim {
    pub worker: String,
    /// When it was claimed (UTC, RFC 3339).
    pub since: String,
}

/// A done declaration.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Done {
    /// The head the bead was judged at.
    pub head: String,
    pub at: String,
}

/// The landing that carried the declared head.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Landed {
    pub sha: String,
    pub at: String,
}

impl Bead {
    /// A bead with its identity and nothing else filled. Built against by air-o6x.3; only the
    /// shape test uses it here.
    #[allow(dead_code)]
    pub fn empty(id: &str, title: &str, status: &str, kind: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            status: status.to_string(),
            kind: kind.to_string(),
            priority: None,
            assignee: None,
            labels: Vec::new(),
            blocked_by: Vec::new(),
            claim: None,
            done: None,
            landed: None,
        }
    }
}

/// The board: the empty shape in this bead, whatever the ledger holds.
pub fn beads(_ledger: Option<&Ledger>, at: &str) -> Beads {
    Beads {
        at: at.to_string(),
        fetched_at: None,
        ready: Vec::new(),
        in_progress: Vec::new(),
        done_waiting: Vec::new(),
        closed_recent: Vec::new(),
        blocked: Vec::new(),
    }
}
