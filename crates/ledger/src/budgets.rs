//! Every timing budget Air waits on, measured (air-d75).
//!
//! Air waits on ten budgets — a `bd` process, a `git` process, a SQLite lock, an MCP tool, its
//! own hook — and until this module none of them recorded what the wait actually cost. Three of
//! them fail toward PERMITTING: a hook killed at its cap, a `git` call past 1.5 s inside a hook,
//! and a SQLite lock held past 200 ms all make the hook fail open, so the one refusal silently
//! does not refuse and no event line says so. The event log showed zero of each, which is case
//! 3 of `do-less` (the input never arrived) and not evidence.
//!
//! The owner's ruling on 2026-09-06 was "measure all of them from henceforth", so this is not
//! scoped to the three: every site that waits on a budget records the elapsed time and whether
//! the budget was hit, and `air audit` prints count, percentiles and hits per budget.
//!
//! The shape follows [`crate::events`]'s `bd_ms`/`bd_calls` (air-869) and its correction
//! (air-bp0): [`take`] drains, so a long-lived process (`air mcp`) stamps each poll tick with
//! that tick's share rather than restamping a lifetime total on every line.
//!
//! It lives in `air-ledger` because that is where [`crate::events::Event`] is defined and
//! because it is the only crate with no Air dependencies, so `air-bd` can reach it.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// One `bd` process, at the default 60 s budget plus 5 s per id on a multi-id call
/// (`crates/bd/src/lib.rs` `budget_for`).
pub const BD: &str = "bd";
/// The short `bd show` after a `--claim` timeout (`AIR_BD_PROBE_TIMEOUT_MS`, default 30 s).
pub const BD_PROBE: &str = "bd-probe";
/// `bd ready` behind the Stop nudge (`AIR_NUDGE_BD_TIMEOUT_MS`, default 3 s, under the 5 s hook
/// cap).
pub const BD_NUDGE: &str = "bd-nudge";
/// A `bd` process inside `air status`, at the budget derived from bd's measured median.
pub const BD_STATUS: &str = "bd-status";
/// A `bd` process inside `air land`'s acceptance read, at the client's 60 s + 5 s per id.
pub const BD_ACCEPTANCE: &str = "bd-acceptance";
/// One `git` process from `crate::git` — 1.5 s, and on every hook path.
pub const GIT: &str = "git";
/// One `git` process from `air worker --create` / worktree removal — 120 s.
pub const GIT_WORKTREE: &str = "git-worktree";
/// A wait for the SQLite write lock, against the busy timeout.
pub const SQLITE_LOCK: &str = "sqlite-lock";
/// One `air` subprocess behind an MCP tool call — 20 s for a read, more for a bd write.
pub const MCP_TOOL: &str = "mcp-tool";
/// One `air hook` invocation, against the cap `air install` writes into `settings.json`.
/// Censored at the cap: Claude Code kills the hook, and a killed hook records nothing. See
/// `air audit`'s unpaired-hook count for what that censoring hides.
pub const HOOK: &str = "hook";
/// The process listing behind `air status`'s `readers:` lines and `air land`'s main-checkout
/// warning (`ps` plus `lsof -d cwd`, 4 s each).
pub const TREE_READERS: &str = "tree-readers";

/// Every budget name that can appear on an event line. `air audit` reports any name it reads
/// that is not covered by its catalogue, the way it already reports an unregistered mechanism:
/// a report that silently omits a budget reads as complete when it is not.
pub const NAMES: &[&str] = &[
    BD,
    BD_ACCEPTANCE,
    BD_NUDGE,
    BD_PROBE,
    BD_STATUS,
    GIT,
    GIT_WORKTREE,
    HOOK,
    MCP_TOOL,
    SQLITE_LOCK,
    TREE_READERS,
];

/// What one budget cost over one event's share of the process.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Waits {
    /// The budget in force, in milliseconds. The largest seen, for the sites that vary it
    /// (`bd-acceptance` grows with the id count, `bd-status` with bd's measured median).
    pub budget_ms: u64,
    /// Waits recorded.
    pub n: u64,
    /// Waits that reached the budget — the timeouts. Every one of them is a decision Air made
    /// on less information than it asked for.
    pub hits: u64,
    /// Every wait, in milliseconds, in the order recorded. Kept per observation rather than
    /// summed because the question a budget raises is about its tail, and a tail cannot be
    /// recovered from a mean.
    pub ms: Vec<u64>,
}

fn table() -> &'static Mutex<BTreeMap<String, Waits>> {
    static T: OnceLock<Mutex<BTreeMap<String, Waits>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Record one wait. `hit` is the caller's own verdict — it knows whether the child was killed,
/// the lock gave up, or the answer arrived — because an elapsed time equal to the budget is
/// not by itself a timeout.
///
/// Never panics and never fails: a poisoned lock is recovered into, since losing a measurement
/// must not be able to change what Air decides.
pub fn record(name: &str, elapsed: Duration, budget: Duration, hit: bool) {
    let ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    let budget_ms = u64::try_from(budget.as_millis()).unwrap_or(u64::MAX);
    let mut t = table().lock().unwrap_or_else(|e| e.into_inner());
    let w = t.entry(name.to_string()).or_default();
    w.budget_ms = w.budget_ms.max(budget_ms);
    w.n = w.n.saturating_add(1);
    if hit {
        w.hits = w.hits.saturating_add(1);
    }
    w.ms.push(ms);
}

/// Record a wait that is still going: `restart` opens a new sample, and every call after it
/// raises that sample to `elapsed` instead of adding another. For a retry loop (the SQLite
/// busy handler) this is the only way to end up with one sample per wait, because the loop is
/// told when a wait begins and never told that one succeeded.
///
/// It reads the last sample for `name`, so two threads of ONE process waiting on the same
/// budget at the same time interleave into each other's sample. Air is one command per
/// process apart from `air mcp`'s poll thread; the mis-attribution costs a sample, not a
/// decision.
pub fn record_progress(name: &str, elapsed: Duration, budget: Duration, restart: bool, hit: bool) {
    if restart {
        record(name, elapsed, budget, hit);
        return;
    }
    let ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    let mut t = table().lock().unwrap_or_else(|e| e.into_inner());
    let Some(w) = t.get_mut(name) else {
        drop(t);
        record(name, elapsed, budget, hit);
        return;
    };
    if let Some(last) = w.ms.last_mut() {
        *last = (*last).max(ms);
    }
    if hit {
        w.hits = w.hits.saturating_add(1);
    }
}

/// The waits since the previous `take`: what THIS event should carry. Drains, so a poll loop
/// stamps one tick's share (air-bp0, where a restamped lifetime total summed to numbers no
/// fleet ever made).
pub fn take() -> BTreeMap<String, Waits> {
    let mut t = table().lock().unwrap_or_else(|e| e.into_inner());
    std::mem::take(&mut t)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// The table is process-global, so this is ONE test: two would race each other's `take`.
    #[test]
    fn records_hits_and_drains_on_take() {
        let _ = take(); // whatever else in this binary left behind
        let s = |ms| Duration::from_millis(ms);
        record(GIT, s(9), s(1500), false);
        record(GIT, s(1500), s(1500), true);
        record(BD, s(1400), Duration::from_secs(10), false);
        // A site that varies its budget reports the largest it waited on, not the last.
        record("t-vary", s(1), Duration::from_secs(10), false);
        record("t-vary", s(1), Duration::from_secs(14), false);
        record("t-vary", s(1), Duration::from_secs(12), false);

        let first = take();
        assert_eq!(first[GIT].n, 2);
        assert_eq!(first[GIT].hits, 1);
        assert_eq!(first[GIT].budget_ms, 1500);
        assert_eq!(first[GIT].ms, vec![9, 1500]);
        assert_eq!(first[BD].budget_ms, 10_000);
        assert_eq!(first["t-vary"].budget_ms, 14_000);
        // Drained: the same waits are not counted twice on the next event line.
        assert!(take().is_empty());

        // A retry loop reports ONE sample per wait, raised as the wait grows, not one per
        // retry. Recording per retry would have made a single 40 ms lock look like four.
        record_progress(SQLITE_LOCK, s(1), s(200), true, false);
        record_progress(SQLITE_LOCK, s(11), s(200), false, false);
        record_progress(SQLITE_LOCK, s(40), s(200), false, false);
        record_progress(SQLITE_LOCK, s(3), s(200), true, false);
        let locks = take();
        assert_eq!(locks[SQLITE_LOCK].n, 2);
        assert_eq!(locks[SQLITE_LOCK].ms, vec![40, 3]);
        assert_eq!(locks[SQLITE_LOCK].hits, 0);
    }
}
