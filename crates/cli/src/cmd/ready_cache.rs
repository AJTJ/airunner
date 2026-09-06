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

/// bd's ready set, partitioned. The three lists are disjoint and together they are exactly
/// bd's answer, so the counts `air status` prints are projections of ONE list and cannot
/// disagree with bd about which beads (air-f10: the adopter flagged that a matching total is
/// not a matching set, and Air has no ready set of its own to differ with).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadySplit {
    /// What a worker may claim: a task with no `owner` label.
    pub claimable: Vec<String>,
    /// Containers in the ready set: not claimable, and the coordinator's to decompose. Kept
    /// visible rather than silently dropped (the adopter's reasoning, air-f10: the coordinator
    /// needed that count and had to get it from bd).
    pub epics: Vec<String>,
    /// Awaiting the owner's authority: the owner's queue (air-uef).
    pub owner: Vec<String>,
}

/// Partition bd's ready beads. The ONE place the rule is written (air-5hw): `air status`,
/// the Stop nudge and `idle-without-claim` all read this, so the label and the type cannot
/// mean one thing to the cache and another to the status screen. `human` is not a gate: it
/// means a person is present and watching, which says nothing about who may finish the bead.
///
/// An `owner`-labelled epic is the owner's, not a container to decompose: the label is the
/// authority and comes first.
pub fn split(ready: &[air_bd::Issue]) -> ReadySplit {
    let mut s = ReadySplit::default();
    for i in ready {
        if i.labels.iter().any(|l| l == crate::cmd::claim::OWNER_LABEL) {
            s.owner.push(i.id.clone());
        } else if i.issue_type == EPIC {
            s.epics.push(i.id.clone());
        } else {
            s.claimable.push(i.id.clone());
        }
    }
    s
}

/// bd's type for a container.
pub const EPIC: &str = "epic";

/// Which of bd's ready beads a worker may actually claim: [`split`]'s first list.
pub fn claimable(ready: &[air_bd::Issue]) -> Vec<String> {
    split(ready).claimable
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
    bd.label = air_ledger::budgets::BD_NUDGE;
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

    /// air-5hw: `owner` is the gate, `human` is presence and gates nothing. air-f10: an epic
    /// is a container, not a task, and an owner-labelled one is the owner's first.
    #[test]
    fn claimable_excludes_owner_beads_and_epics() {
        let issue = |id: &str, labels: &[&str], kind: &str| air_bd::Issue {
            id: id.to_string(),
            labels: labels.iter().map(|s| s.to_string()).collect(),
            issue_type: kind.to_string(),
            ..Default::default()
        };
        let ready = [
            issue("zz-1", &[], "task"),
            issue("zz-2", &["owner"], "task"),
            issue("zz-3", &["human"], "bug"),
            issue("zz-4", &["runtime", "owner"], "task"),
            issue("zz-5", &[], "epic"),
            issue("zz-6", &["owner"], "epic"),
        ];
        assert_eq!(claimable(&ready), ["zz-1", "zz-3"]);
        let s = split(&ready);
        assert_eq!(s.epics, ["zz-5"]);
        assert_eq!(s.owner, ["zz-2", "zz-4", "zz-6"]);
        assert_eq!(
            s.claimable.len() + s.epics.len() + s.owner.len(),
            ready.len()
        );
    }
}
