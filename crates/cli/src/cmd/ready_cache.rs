//! The `bd ready` list, cached at `<main>/.air/ready.json` by the commands that already call
//! bd (`air status`, `air handover`). `bd ready --json` is ~0.7 s here and the hook budget is
//! 100 ms (air-09i), so the Stop hook uses this to decide whether there is anything worth
//! speaking about at all.
//!
//! It is the GATE, never the answer (air-ouw). Anything derived from it goes stale the moment
//! the claimable set moves, and the set moves constantly: a peer claims a bead, a bead is
//! labelled `owner`, a bead lands. Twice on 2026-08-22 the nudge named beads from this file
//! that `air claim` then refused — once an `owner`-labelled bead, once a bead another worker
//! already held. Both were the same defect. `confirm` below is what the nudge actually names.
//!
//! There is deliberately no freshness window any more: it existed only to caveat a list that
//! might be wrong, and a caveat is not a decision. A window would not have caught either case,
//! since a cache written seconds ago is already wrong once a peer claims.

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadyCache {
    pub at: String,
    pub ids: Vec<String>,
}

fn path(repo: &Path) -> Option<std::path::PathBuf> {
    air_ledger::paths::air_dir_for(repo)
        .ok()
        .map(|d| d.join("ready.json"))
}

/// Best effort; a cache that cannot be written is the same as no cache.
pub fn write(repo: &Path, ids: &[String], now: &str) {
    let Some(p) = path(repo) else { return };
    let c = ReadyCache {
        at: now.to_string(),
        ids: ids.to_vec(),
    };
    if let Ok(s) = serde_json::to_string(&c) {
        let _ = std::fs::write(p, s);
    }
}

pub fn read(repo: &Path) -> Option<ReadyCache> {
    let s = std::fs::read_to_string(path(repo)?).ok()?;
    serde_json::from_str(&s).ok()
}

/// Which of bd's ready beads a worker may actually claim: everything except the ones
/// awaiting the owner's authority. The ONE place that rule is written (air-5hw) — `air
/// status` and this module both call it, so the label cannot mean one thing to the ready
/// cache and another to the status screen. `human` is not a gate: it means a person is
/// present and watching, which says nothing about who may finish the bead.
pub fn claimable(ready: &[air_bd::Issue]) -> Vec<String> {
    ready
        .iter()
        .filter(|i| !i.labels.iter().any(|l| l == crate::cmd::claim::OWNER_LABEL))
        .map(|i| i.id.clone())
        .collect()
}

/// The claimable list as bd has it *now*, for the one caller that must not be wrong: the Stop
/// nudge (air-ouw). Returns `None` when bd does not answer inside the budget, and the nudge
/// then says nothing rather than naming a list it cannot vouch for.
///
/// This is the only bd call on a hook path, and it is deliberate. The general rule stands —
/// `bd ready --json` is ~0.7 s against a 100 ms hook budget — but this runs only at the moment
/// the nudge would otherwise speak from cache: 4 of 172 Stop hooks on 2026-08-22. The cache
/// above still absorbs the other 168.
///
/// `AIR_NUDGE_BD_TIMEOUT_MS` (default 3000) bounds it; the hook's own timeout is 5 s and it
/// fails open, so a slow bd costs a missed nudge, never a blocked worker.
pub fn confirm(repo: &Path) -> Option<Vec<String>> {
    let mut bd = crate::cmd::claim::bd_for(repo);
    let ms = std::env::var("AIR_NUDGE_BD_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);
    bd.timeout = std::time::Duration::from_millis(ms);
    let ids = claimable(&air_bd::WorkLedger::ready(&bd).ok()?);
    write(repo, &ids, &crate::cmd::now());
    Some(ids)
}

/// Refresh the cache from bd; swallow errors (callers report bd failures themselves).
pub fn refresh(repo: &Path, bd: &air_bd::BdCli, now: &str) -> Option<Vec<String>> {
    let ids = claimable(&air_bd::WorkLedger::ready(bd).ok()?);
    write(repo, &ids, now);
    Some(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// air-5hw: `owner` is the gate, `human` is presence and gates nothing.
    #[test]
    fn claimable_excludes_owner_beads_only() {
        let issue = |id: &str, labels: &[&str]| air_bd::Issue {
            id: id.to_string(),
            labels: labels.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        let ready = [
            issue("fd-1", &[]),
            issue("fd-2", &["owner"]),
            issue("fd-3", &["human"]),
            issue("fd-4", &["runtime", "owner"]),
        ];
        assert_eq!(claimable(&ready), ["fd-1", "fd-3"]);
    }
}
