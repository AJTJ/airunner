//! `/api/lane`: the verification lane's state (air-o6x.1). The shape is final; air-o6x.3
//! fills it from `verify_inflight`, `batch_cuts`, `verify_runs`, the red batch and `landings`.
//! This bead answers the empty shape.

use air_ledger::Ledger;
use serde::Serialize;

/// The lane as of `at`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Lane {
    /// When this answer was computed (UTC, RFC 3339).
    pub at: String,
    /// The lane session's worker name, or none when no lane session is recorded.
    pub lane: Option<String>,
    /// The verify running now, if one is.
    pub inflight: Option<Inflight>,
    /// The cuts, newest first.
    pub batch_cuts: Vec<BatchCut>,
    /// Recorded verifies and prechecks, newest first.
    pub verify_runs: Vec<VerifyRun>,
    /// The standing red batch, if one is.
    pub red: Option<Red>,
    /// Landings, newest first.
    pub landings: Vec<Landing>,
    /// The two loop times `air status` prints, in seconds, over the last 24 hours.
    pub loops: Loops,
}

/// A verify in flight.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Inflight {
    /// The commit being verified.
    pub sha: String,
    /// The command.
    pub command: String,
    /// When it started (UTC, RFC 3339).
    pub started_at: String,
    /// Seconds since it started.
    pub elapsed_s: u64,
}

/// One cut.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BatchCut {
    /// The batch commit on the lane's branch.
    pub head: String,
    /// The main it was cut over.
    pub main_sha: String,
    /// When (UTC, RFC 3339).
    pub cut_at: String,
    /// The members merged, each at its sha.
    pub members: Vec<Member>,
}

/// One member of a batch.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Member {
    pub worker: String,
    pub sha: String,
    /// The beads its range names by trailer.
    pub beads: Vec<String>,
}

/// One recorded run.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VerifyRun {
    pub sha: String,
    /// `verify`, `precheck`, `docs-check` or `fitness`.
    pub kind: String,
    /// `green`, `red` or `killed`.
    pub verdict: String,
    /// The exit code, none for a killed run.
    pub exit: Option<i32>,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: Option<i64>,
    /// The members of the batch it verified, empty for a worker's own run.
    pub members: Vec<String>,
    /// The kept output of a red, under `.air/logs/`.
    pub log_path: Option<String>,
}

/// The standing red batch.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Red {
    pub sha: String,
    pub members: Vec<String>,
    /// The members no run since has verified alone (air-67q).
    pub untested_alone: Vec<String>,
    pub log_path: Option<String>,
}

/// One landing.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Landing {
    /// The landing commit.
    pub sha: String,
    /// The branch's worker.
    pub worker: String,
    pub beads: Vec<String>,
    pub finished_at: String,
    pub main_before: String,
    pub main_after: String,
}

/// The loop times, in seconds; none while there is no datum.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Loops {
    pub batch_ready_to_verify_s: Option<f64>,
    pub done_to_landed_s: Option<f64>,
}

/// The lane: the empty shape in this bead, whatever the ledger holds.
pub fn lane(_ledger: Option<&Ledger>, at: &str) -> Lane {
    Lane {
        at: at.to_string(),
        lane: None,
        inflight: None,
        batch_cuts: Vec::new(),
        verify_runs: Vec::new(),
        red: None,
        landings: Vec::new(),
        loops: Loops::default(),
    }
}
