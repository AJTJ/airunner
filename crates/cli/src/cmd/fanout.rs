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
use air_ledger::landings::Member;
use serde_json::json;

use crate::git;

use super::status::{BatchReady, Snapshot, WorkerView};

/// How old the cached ready list may be before the tick asks bd again. One `bd ready` a minute
/// from one process; the snapshot's own bd answers are cached for 10 minutes (air-cmn), which
/// is too slow to tell an idle worker about a bead the coordinator just filed.
const READY_REFRESH_SECS: i64 = 60;

/// The bd cache key holding the ready set as of the last tick, so a restarted channel does
/// not count what was already there as new. A worker without a claim is still told it once
/// (air-oerm).
const READY_SEEN: &str = "fanout_ready_seen";

/// Workers told about new beads: every worker with a live session holding no claim, whatever
/// its session state (air-ludo). A worker mid-turn reads it after that turn; it holds nothing,
/// so it is free. Filtering on `idle` reached nobody in the 2026-09-26 trial: workers launched
/// with no task never took a turn, so none was ever recorded idle and the fleet sat at
/// `ready: 3`. The lane and the coordinator are never on the list: the lane takes no bead and
/// the coordinator files them.
pub fn without_claim(s: &Snapshot) -> Vec<String> {
    s.workers
        .iter()
        .filter(|w: &&WorkerView| {
            w.role == "worker"
                && w.claims.is_empty()
                && w.handed_over.is_empty()
                && w.session
                    .as_ref()
                    .is_some_and(|x| x.liveness == super::lease::Liveness::Alive)
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

/// The ready notice: a count and where the current list is, never the ids (air-bcg). The
/// list was a snapshot, and said so (air-0blv), and workers still walked it: in the 2026-11-06
/// trial worker-1 lost three claim races and worker-2 two on beads the notice still listed.
/// `bd ready` takes about 0.12 s, so the worker reads the live list instead. The rotation of
/// the list per recipient (0.4.9 trial, `beads-ready-rotated`) went with the list; the
/// measurement that would bring a spread back is claim refusals on the first bead of
/// `bd ready` in `air audit`.
pub fn beads_ready_text(count: usize) -> String {
    let beads = if count == 1 { "bead is" } else { "beads are" };
    format!(
        "{count} {beads} claimable; run `bd ready` for the current list. If you hold no claim, \
         claim one with `air claim <id>`; the first claim wins, and a refusal names its reason."
    )
}

/// The bd cache key holding the workers without a claim as of the last tick (air-oerm).
const READY_RECIPIENTS: &str = "fanout_ready_recipients";

/// Queue "beads are ready" for every worker without a claim when the claimable set gained
/// a bead since the last tick, and for a worker that is newly without a claim (it joined, or
/// its claim ended) while the set is not empty. Returns the workers queued for.
///
/// air-oerm: an adopter's fleet started with 100 beads ready and four workers sat idle, because
/// the notice went out only when the set changed; every trial had hidden it by filing beads
/// after the workers joined. A worker told this exact set before is not told again, since a
/// delivery is keyed by the set, so a restarted channel repeats nothing.
pub fn fan_out_ready(
    ledger: &Ledger,
    worker: &str,
    s: &Snapshot,
    ready: &[String],
    at: &str,
) -> Vec<String> {
    let cached = |k: &str| -> Option<Vec<String>> {
        ledger
            .bd_cache_get(k)
            .ok()
            .flatten()
            .and_then(|(v, _)| serde_json::from_str(&v).ok())
    };
    let seen = cached(READY_SEEN);
    let before = cached(READY_RECIPIENTS);
    // Sorted so the cached recipient list compares by name, not by snapshot order.
    let mut recipients = without_claim(s);
    recipients.sort();
    for (k, v) in [
        (READY_SEEN, ready),
        (READY_RECIPIENTS, recipients.as_slice()),
    ] {
        if let Ok(v) = serde_json::to_string(v) {
            let _ = ledger.bd_cache_put(k, &v, at);
        }
    }
    let new = seen.map(|seen| added(&seen, ready)).unwrap_or_default();
    let joined = before.map_or_else(|| recipients.clone(), |b| added(&b, &recipients));
    if ready.is_empty() || (new.is_empty() && joined.is_empty()) {
        return Vec::new();
    }
    let key = ready.join(" ");
    let text = beads_ready_text(ready.len());
    let mut told = Vec::new();
    for w in &recipients {
        if new.is_empty() && !joined.contains(w) {
            continue;
        }
        let queued = ledger
            .enqueue_delivery(
                &Outgoing {
                    to: w,
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
            told.push(w.clone());
        }
    }
    // A change of the set is logged even when nobody was told, as before; a join only when
    // someone was.
    if new.is_empty() && told.is_empty() {
        return told;
    }
    super::log_event(
        ledger,
        worker,
        super::decisions::FANOUT_BEADS_READY,
        &json!({"ready": ready, "new": new, "joined": joined, "to": told}),
        // air-ch9p (air-23ve): queued, not told, and true as of this tick. A worker can claim
        // in the seconds before delivery, and the line read as if it had been told regardless.
        &format!(
            "{} new bead(s), {} worker(s) newly without a claim; queued for {} worker(s) \
             holding no claim at this tick",
            new.len(),
            joined.len(),
            told.len()
        ),
        if new.is_empty() {
            "a worker without a claim joined a non-empty ready set"
        } else {
            "1 change of the ready set"
        },
    );
    told
}

/// The claimable ready list, refreshed from bd when the cache is older than a minute. `None`
/// when bd did not answer and nothing was cached, so a silence is never read as "empty".
fn ready_now(repo: &Path, at: &str) -> Option<Vec<String>> {
    let cached = super::ready_cache::read(repo);
    let fresh = cached
        .as_ref()
        .and_then(|c| super::status::seconds_between(&c.at, at))
        .is_some_and(|age| age < READY_REFRESH_SECS);
    if fresh {
        return cached.map(|c| c.ids);
    }
    let bd = super::claim::bd_for(repo);
    super::ready_cache::refresh(repo, &bd, at).or_else(|| cached.map(|c| c.ids))
}

/// One pass of every producer from a command that just changed a fact (air-4lci): `air record`
/// (a head may now be batch-ready), `air claim`, `air release` and `air close` (the ready set
/// and who holds no claim). Every notice was produced by the coordinator's channel poll alone,
/// so while that session was stopped or its channel lost, nobody was told anything: workers
/// idled 54 min beside a claimable bead here, and an adopter's fleet two hours. The poll stays
/// as the sweeper; a delivery's key makes the second producer's copy a no-op.
///
/// `refresh_ready` re-reads bd's ready list first, because the command just changed it and
/// [`ready_now`] would otherwise answer from a cache up to a minute old. Silent, and never
/// fails the command: a notice missed here is sent by the next poll.
pub fn after_change(repo: &Path, refresh_ready: bool) {
    let Ok((ledger, me)) = super::open(repo) else {
        return;
    };
    if super::fleet::stopped(&ledger) {
        return;
    }
    if refresh_ready {
        let _ = super::ready_cache::refresh(repo, &super::claim::bd_for(repo), &super::now());
    }
    if let Ok(snap) = super::status::gather_with(repo, super::status::BdUse::Never) {
        tick(repo, &ledger, &me, &snap);
    }
}

/// One pass of every producer, after the coordinator's channel gathered `s`, or after a
/// command changed a fact ([`after_change`]).
pub fn tick(repo: &Path, ledger: &Ledger, worker: &str, s: &Snapshot) {
    // A stopped fleet starts nothing, so nothing invites it to (air-1vri.1).
    if super::fleet::stopped(ledger) {
        return;
    }
    let at = super::now();
    let ready = ready_now(repo, &at);
    fan_out_ready(ledger, worker, s, ready.as_deref().unwrap_or_default(), &at);
    if let Some(ready) = &ready {
        queue_empty(
            ledger,
            worker,
            s,
            ready,
            &|| epics_to_decompose_now(repo),
            &at,
        );
    }
    batch_ready_to_lane(ledger, worker, s, &at);
}

// ---------- the coordinator's two waits (air-1vri.5) ----------

/// The coordinator's delivery name: its session's identity since air-jc2p.1.
const COORDINATOR: &str = "coordinator";

/// The bd cache key holding when the coordinator was last told the queue is empty, or `""`
/// once the queue has had a bead again. It makes the notice once per emptying.
const QUEUE_EMPTY_TOLD: &str = "fanout_queue_empty_told";

/// What the coordinator reads when a worker captures: the first line; `air inbox` has the rest.
pub fn capture_text(from: &str, id: &str, text: &str) -> String {
    let first = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    format!(
        "capture from {from}: {first} (`air inbox` has it in full; `air triage {id}` with \
         `--bead <id>` or `--drop <why>` once decided)"
    )
}

/// After `air capture` recorded capture `id`: tell the coordinator once. A capture written in
/// the coordinator's checkout or the main one is the coordinator's or the owner's, and neither
/// needs it pushed. Returns whether a message was queued.
pub fn capture_to_coordinator(ledger: &Ledger, from: &str, id: &str, text: &str, at: &str) -> bool {
    if matches!(super::hook::role_for(from), "coordinator" | "owner") {
        return false;
    }
    let content = capture_text(from, id, text);
    let queued = ledger
        .enqueue_delivery(
            &Outgoing {
                to: COORDINATOR,
                kind: "capture",
                key: id,
                subject: from,
                content: &content,
                supersede: false,
            },
            at,
        )
        .unwrap_or(false);
    if queued {
        super::log_event(
            ledger,
            from,
            super::decisions::FANOUT_CAPTURE,
            &json!({"id": id, "to": COORDINATOR}),
            &format!("capture {id} from {from}; told the coordinator"),
            "1 capture",
        );
    }
    queued
}

pub fn queue_empty_text(free: usize, epics: Option<&[String]>) -> String {
    let epics = match epics {
        None => "unknown (bd did not answer)".to_string(),
        Some([]) => "none".to_string(),
        Some(v) => v.join(" "),
    };
    format!(
        // air-ch9p: the count is workers holding no claim, whatever their state, not idle ones.
        "the ready queue is empty: {free} worker(s) hold no claim; epics with no children: \
         {epics}. File the next wave, or decompose one of those epics."
    )
}

/// The ready epics with no children, asked of bd now. Called only when the queue-empty
/// notice is about to go out, so an ordinary tick pays nothing for it.
fn epics_to_decompose_now(repo: &Path) -> Option<Vec<String>> {
    let bd = super::claim::bd_for(repo);
    let ready = air_bd::WorkLedger::ready(&bd).ok()?;
    let mut v = Vec::new();
    for e in &super::ready_cache::split(&ready).epics {
        let kids = air_bd::WorkLedger::children(&bd, e).ok()?;
        if let Some(d) = super::status::to_decompose(e, &kids) {
            v.push(d.epic);
        }
    }
    Some(v)
}

/// Tell the coordinator the claimable set is empty while a worker holds no claim: once per
/// emptying, and not again until the set has had a bead. An empty set with every worker busy
/// says nothing yet; it speaks when the first of them comes free. Returns whether a message
/// was queued.
pub fn queue_empty(
    ledger: &Ledger,
    worker: &str,
    s: &Snapshot,
    ready: &[String],
    epics: &dyn Fn() -> Option<Vec<String>>,
    at: &str,
) -> bool {
    let told = ledger
        .bd_cache_get(QUEUE_EMPTY_TOLD)
        .ok()
        .flatten()
        .is_some_and(|(v, _)| !v.is_empty());
    if !ready.is_empty() {
        if told {
            let _ = ledger.bd_cache_put(QUEUE_EMPTY_TOLD, "", at);
        }
        return false;
    }
    let idle = without_claim(s);
    if told || idle.is_empty() {
        return false;
    }
    let epics = epics();
    let content = queue_empty_text(idle.len(), epics.as_deref());
    let queued = ledger
        .enqueue_delivery(
            &Outgoing {
                to: COORDINATOR,
                kind: "queue-empty",
                key: at,
                subject: &idle.join(" "),
                content: &content,
                supersede: true,
            },
            at,
        )
        .unwrap_or(false);
    let _ = ledger.bd_cache_put(QUEUE_EMPTY_TOLD, at, at);
    super::log_event(
        ledger,
        worker,
        super::decisions::FANOUT_QUEUE_EMPTY,
        &json!({"idle": idle, "epics": epics, "queued": queued}),
        &content,
        "1 emptying of the ready set",
    );
    queued
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
    // air-ch9p: the coordinator's branch carries no bead, and printed `()`.
    let beads = if b.beads.is_empty() {
        "no bead".to_string()
    } else {
        b.beads.join(" ")
    };
    format!("{} at {} ({beads})", b.worker, short(&b.head))
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

/// The beads a branch's commits name between `from` and `to`, by trailer.
fn beads_between(repo: &Path, from: &str, to: &str) -> Vec<String> {
    super::attribution::attributed_in_range(repo, &format!("{from}..{to}")).declared
}

/// A member's beads as a batch result sees them (air-5ynn).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemberBeads {
    /// Beads whose every trailer commit on the member's branch the batch contains.
    pub covered: Vec<String>,
    /// Beads in the batch with a later commit on the member's branch that the batch lacks.
    pub later: Vec<String>,
}

/// The member's beads, read against its CURRENT head (air-5ynn): a bead the batch took is
/// `covered` only when no commit naming it sits on the branch beyond the batch. An adopter's
/// worker was told "close now" twice for a bead with a newer commit the batch did not have,
/// and the close was refused both times.
fn member_beads(
    repo: &Path,
    ledger: &Ledger,
    main_sha: &str,
    batch: &str,
    m: &Member,
) -> MemberBeads {
    let taken = beads_between(repo, main_sha, &m.sha);
    let head = git::worktrees(repo)
        .unwrap_or_default()
        .into_iter()
        .find(|(p, _)| air_ledger::paths::worker_name_for(p).is_ok_and(|w| w == m.worker))
        .and_then(|(p, _)| git::head(&p).ok())
        .unwrap_or_else(|| m.sha.clone());
    let after = beads_between(repo, batch, &head);
    // Only beads the member still holds open (air-ch9p): a bead it closed before the batch
    // recorded is not one to close now, and the notice said so for six at once on 2026-09-28.
    let held: Vec<String> = ledger
        .open_claims()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.worker == m.worker)
        .map(|c| c.bead)
        .collect();
    let mut out = MemberBeads::default();
    for b in taken {
        if out.covered.contains(&b) || out.later.contains(&b) || !held.contains(&b) {
            continue;
        }
        if after.contains(&b) {
            out.later.push(b);
        } else {
            out.covered.push(b);
        }
    }
    out
}

/// What each member hears when the lane records a batch verify: the exit and the kept output
/// on a red. A green says nothing here (air-4xj.2): a member declared its bead done when the
/// work was committed, and the landing's "main moved" names what it closed; the "close them
/// now" note a green used to carry had nothing left to ask for. A kill is no verdict and says
/// nothing. Pure over the run and each member's beads.
pub fn batch_result_notes(
    run: &air_ledger::verify::VerifyRun,
    beads_of: &dyn Fn(&Member) -> MemberBeads,
) -> Vec<MemberNote> {
    use air_ledger::verify::Verdict;
    let verdict = run.verdict();
    if verdict == Verdict::Killed || verdict == Verdict::Green {
        return Vec::new();
    }
    let alone = run.members.len() == 1;
    run.members
        .iter()
        .filter(|m| !m.worker.is_empty())
        .map(|m| {
            let beads = beads_of(m);
            let mut all = beads.covered;
            all.extend(beads.later);
            let list = if all.is_empty() {
                "your commits".to_string()
            } else {
                all.join(" ")
            };
            let output = run.log_path.as_deref().unwrap_or("not kept");
            // air-ch9p (air-23ve): a batch of one has nothing to split, and since
            // air-hyli its member stays out of the next batch until it moves.
            let content = if alone {
                format!(
                    "batch red at {} (exit {}); your {} ({list}) was its only member. \
                     Output: {output}. Nothing lands on it. Your branch is batch-ready \
                     again once it has a new commit, or a later green contains it.",
                    short(&run.sha),
                    run.exit_code,
                    short(&m.sha),
                )
            } else {
                format!(
                    "batch red at {} (exit {}); your {} ({list}) was one of {} members. \
                     Output: {output}. Nothing lands on it and the lane splits the batch. \
                     Your branch is out of the next batch until a later green contains your \
                     commit (the split proves it) or you commit again; if the failure is in \
                     your change, fix it with a new commit.",
                    short(&run.sha),
                    run.exit_code,
                    short(&m.sha),
                    run.members.len(),
                )
            };
            MemberNote {
                to: m.worker.clone(),
                kind: "batch-red",
                key: run.id.clone(),
                beads: all.join(" "),
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
    let notes = batch_result_notes(run, &|m| member_beads(repo, ledger, &main_sha, &run.sha, m));
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

// ---------- main moved (air-1vri.4) ----------

/// How many changed paths the "main moved" notice names before it says how many more.
const MAIN_MOVED_PATHS: usize = 10;

/// Who hears "main moved": every worktree's session but the main checkout, the lane, and the
/// session that ran `air land` (its own output said it). The owner in a plain shell has no
/// session, so every session hears an owner's landing. Pure over the names.
pub fn main_moved_recipients(
    worktrees: &[String],
    lanes: &[String],
    me: &str,
    role: &str,
) -> Vec<String> {
    let mut v: Vec<String> = worktrees
        .iter()
        // `main` is the SESSION name of the primary checkout, not the base branch (air-h7l).
        .filter(|n| n.as_str() != "main" && !lanes.contains(n))
        .filter(|n| role == "owner" || n.as_str() != me)
        .cloned()
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The lane's names without a snapshot: the default name, and every session row whose role
/// is `lane`.
fn lane_names(ledger: &Ledger) -> Vec<String> {
    let mut v = vec!["lane".to_string()];
    if let Ok(mut st) = ledger
        .conn()
        .prepare("SELECT DISTINCT worker FROM sessions WHERE role='lane'")
        && let Ok(rows) = st.query_map([], |r| r.get::<_, String>(0))
    {
        v.extend(rows.flatten());
    }
    v
}

/// The paths, the first [`MAIN_MOVED_PATHS`] of them and a count of the rest.
pub fn paths_line(paths: &[String]) -> String {
    if paths.is_empty() {
        return "none".to_string();
    }
    let shown = paths
        .iter()
        .take(MAIN_MOVED_PATHS)
        .cloned()
        .collect::<Vec<_>>();
    let more = paths.len().saturating_sub(MAIN_MOVED_PATHS);
    if more == 0 {
        shown.join(", ")
    } else {
        format!("{} and {more} more", shown.join(", "))
    }
}

/// One session's side of a landing (air-4xj.2): the head of its own the landing carried,
/// when the batch commit named one (a batch of one fast-forwards and names none), the beads
/// of its own the landing closed, its done beads whose declared head is still not on the
/// base branch, and its done beads whose head is on it but bd refused to close (air-52u).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemberMove {
    pub sha: Option<String>,
    pub closed: Vec<String>,
    pub waiting: Vec<String>,
    pub refused: Vec<String>,
}

impl MemberMove {
    /// Nothing to say to this session beyond the fact.
    pub fn is_empty(&self) -> bool {
        self.sha.is_none()
            && self.closed.is_empty()
            && self.waiting.is_empty()
            && self.refused.is_empty()
    }
}

/// The text one session reads. Everyone gets the fact, and a member also reads that its head
/// is in and which of its beads the landing closed, so a member gets one notice rather than
/// this and a second "landed" one. It names only beads the landing closed, never a bead to go
/// and close (air-4xj.2: a worker declares done and the landing closes). Asks for no reply.
///
/// No command for merging (air-fehk). Workers read "`git merge main` brings these into your
/// branch" as an instruction and merged mid-batch, moving the head off the sha the lane was
/// testing: once in the 2026-09-29 trial, once at an adopter, each a lost batch cycle. Whether
/// to merge is the worker's call from the files listed (owner, 2026-09-29: "let the AI decide
/// what is right to do"); roles.md says when.
pub fn main_moved_text(
    base: &str,
    merge: &str,
    beads: &[String],
    paths: &[String],
    member: Option<&MemberMove>,
) -> String {
    let mut t = format!(
        "{base} moved to {}: landed {}; files changed: {}.",
        short(merge),
        if beads.is_empty() {
            "no declared bead".to_string()
        } else {
            beads.join(" ")
        },
        paths_line(paths)
    );
    if let Some(m) = member.filter(|m| !m.is_empty()) {
        let mut parts = Vec::new();
        if let Some(sha) = &m.sha {
            parts.push(format!("Your {} is in it", short(sha)));
        }
        if !m.closed.is_empty() {
            parts.push(format!(
                "{}closed: {}",
                if m.sha.is_none() { "Of yours, " } else { "" },
                m.closed.join(" ")
            ));
        }
        if !m.waiting.is_empty() {
            parts.push(format!(
                "{}done but waiting: {} (the declared head is not on {base} yet)",
                if m.sha.is_none() && m.closed.is_empty() {
                    "Of yours, "
                } else {
                    ""
                },
                m.waiting.join(" ")
            ));
        }
        // air-52u: its own words. The 2026-11-09 trial's notices said "waiting" for a head
        // that was on main and a close bd had refused, and the workers read a contradiction.
        if !m.refused.is_empty() {
            parts.push(format!(
                "{}done but refused: {} (bd refused the close; the next landing tries again)",
                if m.sha.is_none() && m.closed.is_empty() && m.waiting.is_empty() {
                    "Of yours, "
                } else {
                    ""
                },
                m.refused.join(" ")
            ));
        }
        t.push(' ');
        t.push_str(&parts.join("; "));
        t.push('.');
    }
    t.push_str(" No reply needed.");
    t
}

/// A member head a landing carried, with the main tip its batch merged onto.
pub type LandedMember = (air_ledger::landings::Member, String);

/// After `air land` moved main from `before` to `merge`: tell every session but the lane and
/// the caller, once per landing. `closed` and `waiting` are `(bead, worker)` pairs from the
/// landing's done pass, and each member reads its own. Returns who was queued.
#[allow(clippy::too_many_arguments)]
pub fn main_moved(
    repo: &Path,
    ledger: &Ledger,
    me: &str,
    role: &str,
    before: &str,
    merge: &str,
    beads: &[String],
    members: &[LandedMember],
    closed: &[(String, String)],
    waiting: &[(String, String)],
    refused: &[(String, String)],
) -> Vec<String> {
    let paths: Vec<String> = crate::git::run(
        repo,
        &["diff", "--name-only", &format!("{before}..{merge}")],
    )
    .map(|o| {
        o.lines()
            .map(str::to_string)
            .filter(|l| !l.is_empty())
            .collect()
    })
    .unwrap_or_default();
    let names: Vec<String> = crate::git::worktrees(repo)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(p, _)| air_ledger::paths::worker_name_for(&p).ok())
        .collect();
    let to = main_moved_recipients(&names, &lane_names(ledger), me, role);
    let at = super::now();
    let subject = beads.join(" ");
    let base = super::handover::base_branch(repo);
    let mut told = Vec::new();
    let of = |rows: &[(String, String)], w: &str| -> Vec<String> {
        rows.iter()
            .filter(|(_, who)| who == w)
            .map(|(b, _)| b.clone())
            .collect()
    };
    for w in &to {
        // A session hears its own side whether or not the batch commit named its head: a
        // batch of one fast-forwards and names no member, and its bead still closed.
        let member = MemberMove {
            sha: members
                .iter()
                .find(|(m, _)| &m.worker == w)
                .map(|(m, _)| m.sha.clone()),
            closed: of(closed, w),
            waiting: of(waiting, w),
            refused: of(refused, w),
        };
        let text = main_moved_text(&base, merge, beads, &paths, Some(&member));
        let queued = ledger
            .enqueue_delivery(
                &Outgoing {
                    to: w,
                    kind: "main-moved",
                    key: merge,
                    subject: &subject,
                    content: &text,
                    supersede: false,
                },
                &at,
            )
            .unwrap_or(false);
        if queued {
            told.push(w.clone());
        }
    }
    super::log_event(
        ledger,
        me,
        super::decisions::FANOUT_MAIN_MOVED,
        &json!({"merge": merge, "beads": beads, "paths": paths.len(), "to": told}),
        &format!(
            "{base} moved to {}; told {} session(s)",
            short(merge),
            told.len()
        ),
        "1 landing",
    );
    told
}

/// A lease came free (released, or broken): tell each worker recorded as wanting it, oldest
/// want first, once (air-1vri.3). The want is cleared when the message is delivered
/// ([`after_delivery`]) or when that worker takes the lease. Returns the workers queued for.
pub fn lease_free(ledger: &Ledger, worker: &str, resource: &str, at: &str) -> Vec<String> {
    let wants = ledger.lease_wants(resource).unwrap_or_default();
    let text = format!(
        "{resource} is free. If you still need it, take it with `air lease take {resource} \
         --reason \"<why>\"`; the first take wins."
    );
    let key = format!("{resource}@{at}");
    let told: Vec<String> = wants
        .iter()
        .filter(|(w, _, _)| w != worker)
        .filter(|(w, _, _)| {
            ledger
                .enqueue_delivery(
                    &Outgoing {
                        to: w,
                        kind: "lease-free",
                        key: &key,
                        subject: resource,
                        content: &text,
                        supersede: true,
                    },
                    at,
                )
                .unwrap_or(false)
        })
        .map(|(w, _, _)| w.clone())
        .collect();
    if !told.is_empty() {
        super::log_event(
            ledger,
            worker,
            super::decisions::FANOUT_LEASE_FREE,
            &json!({"resource": resource, "to": told}),
            &format!("{resource} is free; told {} waiting worker(s)", told.len()),
            &format!("{} want(s)", wants.len()),
        );
    }
    told
}

/// What delivering a message changes besides the row: a worker told a lease is free no longer
/// waits for it.
pub fn after_delivery(ledger: &Ledger, me: &str, d: &air_ledger::deliveries::Delivery) {
    if d.kind == "lease-free" {
        let _ = ledger.clear_lease_want(&d.subject, me);
    }
}

/// After `air batch cut` dropped a branch: tell its worker at once.
pub fn batch_dropped(ledger: &Ledger, lane: &str, base: &str, d: &super::batch_cut::Dropped) {
    let note = MemberNote {
        to: d.worker.clone(),
        kind: "batch-dropped",
        key: format!("{}@{}", d.head, d.against_sha),
        beads: d.beads.join(" "),
        content: format!(
            "dropped from batch: your {} ({}) conflicts with {} at {} in {}. {}",
            short(&d.head),
            d.beads.join(" "),
            d.against,
            short(&d.against_sha),
            d.paths.join(", "),
            super::batch_cut::drop_advice(d, base)
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// air-bcg: the notice carries a count and `bd ready`, never an id, so nothing in it is
    /// stale by the time a worker reads it. One bead reads as "bead is".
    #[test]
    fn beads_ready_carries_a_count_and_no_ids() {
        let ids = names(&["b-1", "b-2", "b-3"]);
        let text = beads_ready_text(ids.len());
        assert!(
            text.starts_with("3 beads are claimable; run `bd ready`"),
            "{text}"
        );
        assert!(ids.iter().all(|id| !text.contains(id.as_str())), "{text}");
        assert!(text.contains("claim one with `air claim <id>`"), "{text}");
        let one = beads_ready_text(1);
        assert!(one.starts_with("1 bead is claimable;"), "{one}");
    }

    /// air-1vri.5: the coordinator hears "the ready queue is empty" once per emptying while a
    /// worker holds no claim, never while every worker is busy, and again after a refill.
    #[test]
    fn queue_empty_is_told_to_the_coordinator_once_per_emptying() {
        use super::super::status::{Session, WorkerView};
        let ledger = Ledger::open_in_memory().unwrap();
        let worker = |name: &str, claims: &[&str]| WorkerView {
            worker: name.to_string(),
            role: "worker".to_string(),
            claims: claims
                .iter()
                .map(|c| air_ledger::claims::Claim {
                    bead: c.to_string(),
                    worker: name.to_string(),
                    claimed_at: String::new(),
                    declared_files: Vec::new(),
                    first_handover_at: None,
                    last_handover_at: None,
                    handover_attempts: 0,
                    released_at: None,
                    release_reason: None,
                    done_at: None,
                    done_head: None,
                    done_proof: None,
                })
                .collect(),
            session: Some(Session {
                liveness: crate::cmd::lease::Liveness::Alive,
                ..Default::default()
            }),
            ..Default::default()
        };
        let busy = Snapshot {
            workers: vec![worker("w1", &["zz-1"])],
            ..Default::default()
        };
        let idle = Snapshot {
            workers: vec![worker("w1", &[]), worker("w2", &["zz-2"])],
            ..Default::default()
        };
        let epics = || Some(names(&["zz-9"]));
        let step = |s: &Snapshot, ready: &[&str], at: &str| {
            queue_empty(&ledger, "coordinator", s, &names(ready), &epics, at)
        };
        let told = || -> Vec<(String, String)> {
            ledger
                .conn()
                .prepare("SELECT to_worker, content FROM deliveries WHERE kind='queue-empty'")
                .unwrap()
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        // Empty, every worker busy: nothing yet.
        assert!(!step(&busy, &[], "2026-09-26T10:00:00Z"));
        // A worker comes free: told once.
        assert!(step(&idle, &[], "2026-09-26T10:00:30Z"));
        // Still empty: not again.
        assert!(!step(&idle, &[], "2026-09-26T10:01:00Z"));
        assert!(!step(&idle, &[], "2026-09-26T10:01:30Z"));
        let rows = told();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].0, "coordinator");
        assert_eq!(
            rows[0].1,
            "the ready queue is empty: 1 worker(s) hold no claim; epics with no children: zz-9. \
             File the next wave, or decompose one of those epics."
        );
        // The coordinator's channel takes it; an undelivered one would be replaced instead.
        assert_eq!(
            ledger
                .take_deliveries("coordinator", "2026-09-26T10:01:40Z")
                .unwrap()
                .len(),
            1
        );
        // Refilled, then empty again: told again.
        assert!(!step(&idle, &["zz-3"], "2026-09-26T10:02:00Z"));
        assert!(step(&idle, &[], "2026-09-26T10:02:30Z"));
        let rows = told();
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert!(rows.iter().all(|(to, _)| to == "coordinator"), "{rows:?}");
        assert_eq!(
            queue_empty_text(2, None),
            "the ready queue is empty: 2 worker(s) hold no claim; epics with no children: unknown \
             (bd did not answer). File the next wave, or decompose one of those epics."
        );
        assert!(queue_empty_text(1, Some(&[])).contains("no children: none."));
    }

    #[test]
    fn capture_notice_skips_the_coordinators_own() {
        let ledger = Ledger::open_in_memory().unwrap();
        let at = "2026-09-26T10:00:00Z";
        assert!(!capture_to_coordinator(
            &ledger,
            "coordinator",
            "c1",
            "x",
            at
        ));
        assert!(capture_to_coordinator(
            &ledger,
            "w1",
            "c2",
            "\n  first\nsecond",
            at
        ));
        assert!(
            !capture_to_coordinator(&ledger, "w1", "c2", "first", at),
            "once"
        );
        let n: i64 = ledger
            .conn()
            .query_row("SELECT count(*) FROM deliveries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        assert!(
            capture_text("w1", "c2", "\n  first\nsecond").starts_with("capture from w1: first (")
        );
    }

    #[test]
    fn main_moved_reaches_everyone_but_the_lane_and_the_caller() {
        let all = names(&["main", "coordinator", "lane", "w1", "w2", "w4"]);
        let lanes = names(&["lane", "w4"]);
        // The lane ran it: every other session, never the main checkout or a lane.
        assert_eq!(
            main_moved_recipients(&all, &lanes, "w4", "lane"),
            names(&["coordinator", "w1", "w2"])
        );
        // The coordinator ran it: its own output told it.
        assert_eq!(
            main_moved_recipients(&all, &lanes, "coordinator", "coordinator"),
            names(&["w1", "w2"])
        );
        // The owner from a plain shell has no session to skip.
        assert_eq!(
            main_moved_recipients(&all, &lanes, "coordinator", "owner"),
            names(&["coordinator", "w1", "w2"])
        );
    }

    #[test]
    fn main_moved_text_truncates_paths_and_asks_for_no_reply() {
        let paths: Vec<String> = (0..12).map(|i| format!("f{i}")).collect();
        let t = main_moved_text("main", "abcdef0123", &names(&["b-1"]), &paths, None);
        assert!(
            t.starts_with("main moved to abcdef01: landed b-1; files changed: f0,"),
            "{t}"
        );
        assert!(t.contains("f9 and 2 more."), "{t}");
        assert!(!t.contains("Merge main"), "{t}");
        assert!(t.ends_with("No reply needed."), "{t}");
        let mine = names(&["b-1"]);
        let member = MemberMove {
            sha: Some("0011223344".into()),
            closed: mine.clone(),
            waiting: names(&["b-2"]),
            refused: names(&["b-3"]),
        };
        let w = main_moved_text("main", "abcdef0123", &mine, &paths[..1], Some(&member));
        assert!(
            w.contains(
                "files changed: f0. Your 00112233 is in it; closed: b-1; done but waiting: b-2 \
                 (the declared head is not on main yet); done but refused: b-3 (bd refused \
                 the close; the next landing tries again). No reply needed."
            ),
            "{w}"
        );
        // air-52u: refused alone is still "of yours", and never "waiting".
        let refused_only = MemberMove {
            refused: names(&["b-3"]),
            ..Default::default()
        };
        let r = main_moved_text(
            "main",
            "abcdef0123",
            &mine,
            &paths[..1],
            Some(&refused_only),
        );
        assert!(
            r.contains(
                "Of yours, done but refused: b-3 (bd refused the close; the next landing \
                 tries again). No reply needed."
            ),
            "{r}"
        );
        assert!(!r.contains("waiting"), "{r}");
        // air-fehk: the fact only; whether to merge is the worker's call. air-4xj.2: no bead
        // to go and close either.
        assert!(!w.contains("git merge main"), "{w}");
        assert!(!w.contains("close any"), "{w}");
        let bare = MemberMove {
            sha: Some("0011223344".into()),
            ..Default::default()
        };
        let b = main_moved_text("main", "abcdef0123", &mine, &paths[..1], Some(&bare));
        assert!(
            b.contains("Your 00112233 is in it. No reply needed."),
            "{b}"
        );
        // A batch of one fast-forwards and names no member head; the closed bead is still
        // its worker's to hear. Nothing of its own: the fact alone.
        let unnamed = MemberMove {
            sha: None,
            closed: mine.clone(),
            ..Default::default()
        };
        let u = main_moved_text("main", "abcdef0123", &mine, &paths[..1], Some(&unnamed));
        assert!(
            u.contains("f0. Of yours, closed: b-1. No reply needed."),
            "{u}"
        );
        let none = main_moved_text(
            "main",
            "abcdef0123",
            &mine,
            &paths[..1],
            Some(&MemberMove::default()),
        );
        assert!(
            none.ends_with("files changed: f0. No reply needed."),
            "{none}"
        );
    }
}
