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

use super::status::{BatchReady, Snapshot, WorkerView};

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
    batch_ready_to_lane(ledger, worker, s, &at);
}

// ---------- the lane and its members (air-1vri.2) ----------

fn short(s: &str) -> &str {
    s.get(..8).unwrap_or(s)
}

/// The lane sessions: every session row whose role is `lane`, plus a worktree whose name reads
/// as the lane. Usually one name.
pub fn lanes(s: &Snapshot) -> Vec<String> {
    let mut v: Vec<String> = s
        .sessions
        .iter()
        .filter(|(_, role, _)| role == "lane")
        .map(|(w, _, _)| w.clone())
        .chain(
            s.workers
                .iter()
                .filter(|w| w.role == "lane")
                .map(|w| w.worker.clone()),
        )
        .collect();
    v.sort();
    v.dedup();
    v
}

fn batch_ready_line(b: &BatchReady) -> String {
    format!("{} at {} ({})", b.worker, short(&b.head), b.beads.join(" "))
}

/// What the lane does next, given the batch-ready set as of now: the set and `air batch cut`,
/// or that nothing is batch-ready. `air land` and a batch `air record` end with this.
pub fn next_cut_lines(ready: &[BatchReady]) -> Vec<String> {
    if ready.is_empty() {
        return vec![
            "nothing is batch-ready; Air tells the lane when a branch becomes batch-ready"
                .to_string(),
        ];
    }
    let mut v: Vec<String> = ready
        .iter()
        .map(|b| format!("batch-ready: {}", batch_ready_line(b)))
        .collect();
    v.push("next: air batch cut".to_string());
    v
}

fn batch_ready_outgoing<'a>(
    to: &'a str,
    key: &'a str,
    head: &'a str,
    text: &'a str,
) -> Outgoing<'a> {
    Outgoing {
        to,
        kind: "batch-ready",
        key,
        subject: head,
        content: text,
        supersede: false,
    }
}

/// Record that the lane was told these branches in a command's own output, so the channel does
/// not tell it again. The row is delivered at the moment it is written. Callers record it only
/// when the lane itself ran the command; the owner reading `air land` is not the lane.
pub fn told_lane(ledger: &Ledger, lane: &str, ready: &[BatchReady], at: &str) {
    for b in ready.iter().filter(|b| b.worker != lane) {
        let key = format!("{}@{}", b.worker, b.head);
        let text = format!("batch-ready: {}", batch_ready_line(b));
        let _ = ledger.record_told(&batch_ready_outgoing(lane, &key, &b.head, &text), at);
    }
}

/// Queue each newly batch-ready branch for the lane, once per branch head. Returns how many
/// were queued.
pub fn batch_ready_to_lane(ledger: &Ledger, worker: &str, s: &Snapshot, at: &str) -> usize {
    let mut queued = Vec::new();
    for lane in lanes(s) {
        for b in s.batch_ready.iter().filter(|b| b.worker != lane) {
            let key = format!("{}@{}", b.worker, b.head);
            let text = format!(
                "batch-ready: {}. When you are not mid-batch, run `air batch cut`.",
                batch_ready_line(b)
            );
            if ledger
                .enqueue_delivery(&batch_ready_outgoing(&lane, &key, &b.head, &text), at)
                .unwrap_or(false)
            {
                queued.push(key);
            }
        }
    }
    if !queued.is_empty() {
        super::log_event(
            ledger,
            worker,
            super::decisions::FANOUT_BATCH_READY,
            &json!({"branches": queued}),
            &format!(
                "{} branch(es) newly batch-ready, told the lane",
                queued.len()
            ),
            &format!("{} batch-ready branch(es)", s.batch_ready.len()),
        );
    }
    queued.len()
}

/// One message to one member worker about a batch it was in.
pub struct MemberNote {
    pub to: String,
    pub kind: &'static str,
    pub key: String,
    /// The member's beads, space-separated; the green-to-close measurement reads it.
    pub beads: String,
    pub content: String,
}

/// The beads a member's commits name between main and its sha.
fn member_beads(repo: &Path, main_sha: &str, sha: &str) -> Vec<String> {
    super::attribution::attributed_in_range(repo, &format!("{main_sha}..{sha}")).declared
}

/// What each member hears when the lane records a batch verify: `close` on a green, the exit
/// and the kept output on a red. A kill is no verdict and says nothing. Pure over the run and
/// each member's beads.
pub fn batch_result_notes(
    run: &air_ledger::verify::VerifyRun,
    beads_of: &dyn Fn(&str) -> Vec<String>,
) -> Vec<MemberNote> {
    use air_ledger::verify::Verdict;
    let verdict = run.verdict();
    if verdict == Verdict::Killed {
        return Vec::new();
    }
    run.members
        .iter()
        .filter(|m| !m.worker.is_empty())
        .map(|m| {
            let beads = beads_of(&m.sha);
            let list = if beads.is_empty() {
                "your commits".to_string()
            } else {
                beads.join(" ")
            };
            let (kind, content) = if verdict == Verdict::Green {
                (
                    "batch-green",
                    format!(
                        "batch green at {} contains your {} ({list}). Close them now with \
                         `bd close <id> --reason-file <proof>`; the proof is the lane's green at \
                         {}. `air handover` says whether the close will pass.",
                        short(&run.sha),
                        short(&m.sha),
                        run.sha
                    ),
                )
            } else {
                (
                    "batch-red",
                    format!(
                        "batch red at {} (exit {}); your {} ({list}) was a member. Output: {}. \
                         Nothing lands on it and the lane splits the batch. If the failure is \
                         in your change, fix it with a new commit.",
                        short(&run.sha),
                        run.exit_code,
                        short(&m.sha),
                        run.log_path.as_deref().unwrap_or("not kept")
                    ),
                )
            };
            MemberNote {
                to: m.worker.clone(),
                kind,
                key: run.id.clone(),
                beads: beads.join(" "),
                content,
            }
        })
        .collect()
}

fn queue_notes(ledger: &Ledger, worker: &str, notes: &[MemberNote], why: &str, at: &str) {
    let mut to = Vec::new();
    for n in notes {
        let queued = ledger
            .enqueue_delivery(
                &Outgoing {
                    to: &n.to,
                    kind: n.kind,
                    key: &n.key,
                    subject: &n.beads,
                    content: &n.content,
                    supersede: false,
                },
                at,
            )
            .unwrap_or(false);
        if queued {
            to.push(format!("{}:{}", n.kind, n.to));
        }
    }
    if !to.is_empty() {
        super::log_event(
            ledger,
            worker,
            super::decisions::FANOUT_BATCH_RESULT,
            &json!({"to": to}),
            why,
            &format!("{} member(s)", notes.len()),
        );
    }
}

/// After `air record` wrote a batch verify: tell each member the result.
pub fn batch_result(repo: &Path, ledger: &Ledger, run: &air_ledger::verify::VerifyRun) {
    let Some(main_sha) = run.main_sha.clone() else {
        return;
    };
    let notes = batch_result_notes(run, &|sha| member_beads(repo, &main_sha, sha));
    queue_notes(
        ledger,
        &run.worker,
        &notes,
        &format!(
            "batch {} at {}",
            if run.is_green() { "green" } else { "red" },
            short(&run.sha)
        ),
        &super::now(),
    );
}

/// After `air land` landed a batch: tell each member its beads are in main.
pub fn batch_landed(
    repo: &Path,
    ledger: &Ledger,
    lane: &str,
    merge: &str,
    tip: &str,
    members: &[air_ledger::landings::Member],
) {
    let notes: Vec<MemberNote> = members
        .iter()
        .filter(|m| !m.worker.is_empty())
        .map(|m| {
            let beads = member_beads(repo, tip, &m.sha);
            MemberNote {
                to: m.worker.clone(),
                kind: "batch-landed",
                key: merge.to_string(),
                beads: beads.join(" "),
                content: format!(
                    "landed in main at {}: your {} ({}). Close any of those beads still open, \
                     with the lane's green as proof.",
                    short(merge),
                    short(&m.sha),
                    if beads.is_empty() {
                        "your commits".to_string()
                    } else {
                        beads.join(" ")
                    }
                ),
            }
        })
        .collect();
    queue_notes(
        ledger,
        lane,
        &notes,
        &format!("landed at {}", short(merge)),
        &super::now(),
    );
}

/// After `air batch cut` dropped a branch: tell its worker at once.
pub fn batch_dropped(ledger: &Ledger, lane: &str, d: &super::batch_cut::Dropped) {
    let note = MemberNote {
        to: d.worker.clone(),
        kind: "batch-dropped",
        key: format!("{}@{}", d.head, d.against_sha),
        beads: d.beads.join(" "),
        content: format!(
            "dropped from batch: your {} ({}) conflicts with {} at {} in {}. Resolve it in your \
             worktree (merge {} and fix the conflict), commit, and your branch is batch-ready \
             again.",
            short(&d.head),
            d.beads.join(" "),
            d.against,
            short(&d.against_sha),
            d.paths.join(", "),
            if d.against == "main" || d.against == "batch" {
                "main".to_string()
            } else {
                format!("{}'s branch", d.against)
            }
        ),
    };
    queue_notes(
        ledger,
        lane,
        std::slice::from_ref(&note),
        &format!("{} dropped from batch", d.worker),
        &super::now(),
    );
}
