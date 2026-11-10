//! `/api/feed?since=<offset>`: the decision and notice feed (air-o6x.1). The shape is final;
//! air-o6x.2 fills it from the event stream, `deliveries` and `messages`, remembering the byte
//! offset per file. This bead answers the empty shape and echoes the offset.

use air_ledger::Ledger;
use serde::Serialize;

/// The feed as of `at`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Feed {
    /// When this answer was computed (UTC, RFC 3339).
    pub at: String,
    /// What to pass as `since` next time; equal to `since` when nothing is new.
    pub next_offset: u64,
    /// New items since `since`, oldest first.
    pub items: Vec<Item>,
}

/// One item.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Item {
    pub at: String,
    pub source: Source,
    /// The worker it concerns or came from.
    pub worker: String,
    /// The event's command and decision, the delivery's kind, or `message`.
    pub kind: String,
    /// One line.
    pub text: String,
}

/// Where an item came from. Built against by air-o6x.2; only the shape test constructs one
/// here.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Event,
    Delivery,
    Message,
}

/// The `since` offset from a query string, 0 when absent or not a number.
pub fn since(query: Option<&str>) -> u64 {
    query
        .unwrap_or_default()
        .split('&')
        .find_map(|kv| kv.strip_prefix("since="))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// The feed: nothing new in this bead, so the offset comes back unchanged.
pub fn feed(_ledger: Option<&Ledger>, at: &str, since: u64) -> Feed {
    Feed {
        at: at.to_string(),
        next_offset: since,
        items: Vec::new(),
    }
}
