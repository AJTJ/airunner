//! What Air notices on the coordinator's channel tick and queues for other sessions (air-1vri).
//!
//! The coordinator's `air mcp` already gathers a snapshot every 30 seconds. After it, [`tick`]
//! compares what it sees with what it saw last time and queues a delivery for each change a
//! session should hear about. Delivery itself is each recipient's own channel (`mcp.rs`,
//! `deliver_once`). The owner's ruling, 2026-09-26: the coordinator sends one message to Air and
//! Air fans it out, so the coordinator relays nothing it can leave to Air.
//!
//! Removal: when bd or Claude Code offers a ready-work subscription a session can read itself.

use std::path::Path;

use air_ledger::Ledger;
use air_ledger::deliveries::Outgoing;
use serde_json::json;

use super::status::{Snapshot, WorkerView};

/// How old the cached ready list may be before the tick asks bd again. One `bd ready` a minute
/// from one process; the snapshot's own bd answers are cached for 10 minutes (air-cmn), which
/// is too slow to tell an idle worker about a bead the coordinator just filed.
const READY_REFRESH_SECS: i64 = 60;

/// The bd cache key holding the ready set as of the last tick, so a restarted channel does
/// not announce what was already there.
const READY_SEEN: &str = "fanout_ready_seen";

/// Workers told about new beads: live, idle, holding nothing, and not running a check of their
/// own. The lane and the coordinator are never on the list: the lane takes no bead and the
/// coordinator files them.
pub fn idle_without_claim(s: &Snapshot) -> Vec<String> {
    s.workers
        .iter()
        .filter(|w: &&WorkerView| {
            w.role == "worker"
                && w.claims.is_empty()
                && w.handed_over.is_empty()
                && w.session
                    .as_ref()
                    .is_some_and(|x| x.state == "idle" && x.pid_alive != Some(false))
                && !super::status::verify_running(s, &w.worker)
        })
        .map(|w| w.worker.clone())
        .collect()
}

/// The beads in `now` that were not in `before`. A shrinking set (a bead claimed) is no news.
pub fn added(before: &[String], now: &[String]) -> Vec<String> {
    now.iter()
        .filter(|b| !before.contains(b))
        .cloned()
        .collect()
}

pub fn beads_ready_text(ids: &[String]) -> String {
    format!(
        "beads are ready: {}. If you hold no claim, claim one with `air claim <id>`; the first \
         claim wins, and a refused claim means someone else took it.",
        ids.join(" ")
    )
}

/// Queue "beads are ready" for every idle worker without a claim when the claimable set gained
/// a bead since the last tick. Returns the workers queued for.
pub fn fan_out_ready(
    ledger: &Ledger,
    worker: &str,
    s: &Snapshot,
    ready: &[String],
    at: &str,
) -> Vec<String> {
    let seen: Option<Vec<String>> = ledger
        .bd_cache_get(READY_SEEN)
        .ok()
        .flatten()
        .and_then(|(v, _)| serde_json::from_str(&v).ok());
    if let Ok(v) = serde_json::to_string(ready) {
        let _ = ledger.bd_cache_put(READY_SEEN, &v, at);
    }
    // The first tick after a restart seeds the set and says nothing: the beads were there
    // before this channel was, and the Stop nudge tells a worker about them when it stops.
    let Some(seen) = seen else {
        return Vec::new();
    };
    let new = added(&seen, ready);
    if new.is_empty() {
        return Vec::new();
    }
    let key = ready.join(" ");
    let text = beads_ready_text(ready);
    let mut told = Vec::new();
    for w in idle_without_claim(s) {
        let queued = ledger
            .enqueue_delivery(
                &Outgoing {
                    to: &w,
                    kind: "beads-ready",
                    key: &key,
                    subject: &key,
                    content: &text,
                    supersede: true,
                },
                at,
            )
            .unwrap_or(false);
        if queued {
            told.push(w);
        }
    }
    super::log_event(
        ledger,
        worker,
        super::decisions::FANOUT_BEADS_READY,
        &json!({"ready": ready, "new": new, "to": told}),
        &format!(
            "{} new bead(s); told {} idle worker(s) without a claim",
            new.len(),
            told.len()
        ),
        "1 change of the ready set",
    );
    told
}

/// The claimable ready list, refreshed from bd when the cache is older than a minute.
fn ready_now(repo: &Path, at: &str) -> Vec<String> {
    let cached = super::ready_cache::read(repo);
    let fresh = cached
        .as_ref()
        .and_then(|c| super::status::seconds_between(&c.at, at))
        .is_some_and(|age| age < READY_REFRESH_SECS);
    if fresh {
        return cached.map(|c| c.ids).unwrap_or_default();
    }
    let bd = super::claim::bd_for(repo);
    super::ready_cache::refresh(repo, &bd, at)
        .or_else(|| cached.map(|c| c.ids))
        .unwrap_or_default()
}

/// One pass of every producer, after the coordinator's channel gathered `s`.
pub fn tick(repo: &Path, ledger: &Ledger, worker: &str, s: &Snapshot) {
    let at = super::now();
    let ready = ready_now(repo, &at);
    fan_out_ready(ledger, worker, s, &ready, &at);
}
