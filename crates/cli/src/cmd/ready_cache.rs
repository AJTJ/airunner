//! The `bd ready` list, cached at `<main>/.air/ready.json` by the commands that already call
//! bd (`air status`, `air handover`), so the Stop hook can name ready beads without a bd call:
//! `bd ready --json` measured 1.1 s here (2026-08-22), the hook budget is 100 ms (air-09i).

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Older than this and the hook says "ready list may be stale".
pub const FRESH_SECS: i64 = 300;

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

/// Pure: is a cache written at `at` still fresh at `now` (RFC 3339 both)?
pub fn is_fresh(at: &str, now: &str) -> bool {
    let (Ok(a), Ok(n)) = (
        at.parse::<jiff::Timestamp>(),
        now.parse::<jiff::Timestamp>(),
    ) else {
        return false;
    };
    n.duration_since(a).as_secs() <= FRESH_SECS
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

/// Refresh the cache from bd; swallow errors (callers report bd failures themselves).
pub fn refresh(repo: &Path, bd: &air_bd::BdCli, now: &str) -> Option<Vec<String>> {
    let ids = claimable(&air_bd::WorkLedger::ready(bd).ok()?);
    write(repo, &ids, now);
    Some(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshness_window() {
        assert!(is_fresh("2026-08-22T05:00:00Z", "2026-08-22T05:04:59Z"));
        assert!(!is_fresh("2026-08-22T05:00:00Z", "2026-08-22T05:05:01Z"));
        assert!(!is_fresh("garbage", "2026-08-22T05:05:01Z"));
    }

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
