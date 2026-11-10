//! `air ready`: the coordinator offers its branch's head to the lane (air-m8u).
//!
//! The coordinator's commits carry no `Bead:` trailer, so nothing on its branch says it is
//! done. With `"coordinator_autobatch"` absent or false in `.claude/air.json`, the branch is
//! batch-ready only at a head the coordinator has offered, which this command records in the
//! ledger's `offers` table; with it true every commit main lacks is batch-ready, as before,
//! and the command records the offer anyway. A later commit is a new head and needs a new
//! offer. Refused to workers and the lane by role, the way `air land` is.
//!
//! Recorded failure: at an adopter each coordinator commit cost the lane a cut, a verify and
//! two notices while the coordinator was still iterating on a note (adopter, 2026-10-28).
//! Removed with the lane, or when the coordinator stops committing to its own branch.

use std::path::Path;

use crate::cmd::{now, open};
use crate::git;

/// Who may offer a head: the coordinator, and the owner's own shell.
pub fn may_offer(role: &str) -> Result<(), String> {
    match role {
        "coordinator" | "owner" => Ok(()),
        "lane" => Err(
            "refused: `air ready` is the coordinator's; the lane's branch is the batch, not a \
             member. The role comes from AIR_ROLE, which the launcher sets."
                .to_string(),
        ),
        _ => Err(
            "refused: `air ready` is the coordinator's. A worker's branch is batch-ready on \
             its `Bead:` trailers (`air status --json` names the first fact it lacks). The \
             role comes from AIR_ROLE, which the launcher sets."
                .to_string(),
        ),
    }
}

pub fn run(repo: &Path, json: bool) -> i32 {
    if let Err(msg) = may_offer(super::caller_role()) {
        eprintln!("air ready: {msg}");
        return 2;
    }
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air ready: {e}");
            return 1;
        }
    };
    let sha = match git::head(repo) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("air ready: {e}");
            return 1;
        }
    };
    let at = now();
    if let Err(e) = ledger.offer(&worker, &sha, &at) {
        eprintln!("air ready: ledger: {e}");
        return 1;
    }
    let short = sha.get(..8).unwrap_or(&sha);
    let autobatch = super::handover::coordinator_autobatch(repo);
    if json {
        println!(
            "{}",
            serde_json::json!({"worker": worker, "sha": sha, "at": at, "coordinator_autobatch": autobatch})
        );
    } else if autobatch {
        println!(
            "offered {worker} at {short}; `coordinator_autobatch` is true, so every commit {} \
             lacks is batch-ready already",
            super::handover::base_branch(repo)
        );
    } else {
        println!("offered {worker} at {short} to the lane; a later commit needs a new `air ready`");
    }
    // So the lane hears `batch-ready` now rather than at the next poll.
    super::fanout::after_change(repo, false);
    0
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn only_the_coordinator_and_the_owner_offer() {
        assert!(may_offer("coordinator").is_ok());
        assert!(may_offer("owner").is_ok());
        assert!(may_offer("worker").unwrap_err().contains("coordinator's"));
        assert!(may_offer("lane").unwrap_err().contains("the batch"));
    }
}
