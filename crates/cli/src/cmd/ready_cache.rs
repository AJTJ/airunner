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

/// Refresh the cache from bd; swallow errors (callers report bd failures themselves).
pub fn refresh(repo: &Path, bd: &air_bd::BdCli, now: &str) -> Option<Vec<String>> {
    let ids: Vec<String> = air_bd::WorkLedger::ready(bd)
        .ok()?
        .into_iter()
        .filter(|i| !i.labels.iter().any(|l| l == "human"))
        .map(|i| i.id)
        .collect();
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
}
