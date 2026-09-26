//! `air land <bead>…` / `air land --all`: the coordinator merges a green branch into main.
//!
//! Incident (2026-08-22, air-3pz): two green hand-overs sat because only the owner may commit
//! on main and `air land` did not exist. The coordinator could see the work was done and could
//! not land it; the owner learned by reading a pane.
//!
//! **Main is never moved to a commit that has not been verified (air-odv).** The landing commit
//! is built off main with `commit-tree` and main is fast-forwarded onto it, so there is no
//! window in which main holds unverified code and nothing to roll back. The evidence is the
//! worker's own green at the branch head: `air land` refuses unless the branch CONTAINS main,
//! which makes the landing commit's tree byte-identical to the one that green describes.
//!
//! Ported from the adopter's `scripts/land.sh` (read 2026-08-22). Taken: main checkout only and
//! on `main` (`land.sh:487-499`); already-an-ancestor is "nothing to land", not an error
//! (`land.sh:533-535`). **Not taken any more**: merging into main and resetting it on red
//! (`land.sh:504-526`), which air-odv replaced — with it went the dirty-tree refusal, whose
//! only reason was that `git reset --hard` would eat uncommitted work. Not taken:
//! The adopter's three attribution rules (branch / closing trailer / containment,
//! `land.sh:46-71`). Air knows who handed each bead over from the claim row, which is the
//! fact those rules reconstruct from git.
//!
//! One `bd` process for every bead closed at the end, not one per bead: a bd process costs
//! ~1.4 s whatever it is asked (air-869, `air_bd::stats`). That is `air close`'s job and this
//! calls into it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use air_ledger::landings::Landing as LandingRow;
use air_ledger::verify::{Kind, new_id};

use crate::cmd::{acceptance, emit, log_event, now, open};
use crate::git;

/// Who may run `air land`: the verification lane, and the owner (a shell with no `AIR_ROLE`).
/// The role is the launcher's `AIR_ROLE` (`cmd::caller_role`), never the directory or
/// `--repo`, so where the command runs and how it is spelled cannot change the answer
/// (air-29a).
///
/// The coordinator lost it on the owner's ruling of 2026-09-14 (air-jc2p.2): main moves only by
/// the lane's landing. The failures behind it (docs/design.md §10): a coordinator's commit on
/// main invalidated four workers' landability at an adopter on 2026-09-06, and landing order
/// was the round's throughput limit while the coordinator landed by hand. Removed when the
/// lane goes; landing then returns to whoever lands a
/// worker's branch.
pub fn may_land(role: &str) -> Result<(), String> {
    match role {
        "lane" | "owner" => Ok(()),
        "coordinator" => Err(
            "refused: `air land` is the verification lane's, not the coordinator's (owner, \
             2026-09-14): main moves only by the lane's landing. Tell the lane its batch is \
             ready, or start one with `air lane --tmux`. The role comes from AIR_ROLE, which the \
             launcher sets."
                .to_string(),
        ),
        _ => Err(
            "refused: `air land` is not a worker's. Close your own bead instead: `air handover` \
             names anything missing, then `air close <id> --reason-file <proof>` (owner ruling, \
             2026-08-22). The role comes from AIR_ROLE, which the launcher sets."
                .to_string(),
        ),
    }
}

/// Why `air land` refuses while a verify is in flight (air-1bm), naming every run and the fix.
/// `None` when nothing is running, so the ok path is silent.
///
/// Landing moves main, and the hand-over gate wants a green at a HEAD containing main, so
/// EVERY verify in flight is about to become worthless — not only one on the landed branch,
/// since every other branch's contains-main precondition breaks and its tree changes on the
/// re-merge. The adopter's coordinator did this to three workers in one round with no signal
/// available (air-4cr); their fix was a protocol where the worker warns first.
///
/// This used to warn and then land. air-4cr defaulted to warn ("a coordinator may still have
/// to land, and a refusal here would be a gate over a fact") and wrote a removal condition
/// asking for a round's data. The data came the other way (the adopter 2026-08-30):
/// 1,199 s of completed verify destroyed in two incidents, six more runs invalidated, and an
/// operational rule ("check first, land later") tried three times and broken the fourth. w2's
/// framing is the fix: the gate and the action in one call are one artefact, and one artefact
/// cannot check another. Coordinator's ruling, 2026-09-05: refuse, with
/// an explicit `--despite-inflight` that lands anyway and is RECORDED on the landings row —
/// with four workers and a twelve-minute verify there is almost always a run in flight, so a
/// refusal with no way past starves landings (the adopter's round note, item 1). The override is
/// the measurement.
///
/// **Removal condition** (`mechanisms.rs` `land-in-flight-refusal`): a full round with zero
/// overrides, meaning the refusal is only ever waited out and could become a plain wait; or a
/// round with zero refusals under overlapping verifies, measured from the `landings` and
/// `verify_inflight` tables.
pub fn in_flight_refusal(flights: &[air_ledger::verify::InFlight], at: &str) -> Option<String> {
    if flights.is_empty() {
        return None;
    }
    let mut s = String::from(
        "refused: a verify is in flight, and landing now would destroy it: main moves, so its \
         green would be for a head that no longer contains main.",
    );
    for f in flights {
        s.push_str(&format!("\n  {}", in_flight_run_line(f, at)));
    }
    s.push_str(
        "\n  fix: wait for it (`air status` names it until it exits), or stop it by pid: \
         `kill <pid>` — never `pkill -f`, which reached every peer's argv (zz-ub34). To land \
         anyway and destroy it: add `--despite-inflight`; the runs destroyed are recorded on \
         the landing.",
    );
    Some(s)
}

/// Every process reading any tree of this repo, for `air land`'s main-checkout warning. All
/// trees, not only main, so a worktree nested under the main checkout is not counted as main.
pub fn main_readers(repo: &Path, ledger: &air_ledger::Ledger) -> super::readers::TreeReaders {
    let raw = super::readers::lookup();
    let trees: Vec<(String, std::path::PathBuf)> = match crate::git::worktrees(repo) {
        Ok(w) => w
            .into_iter()
            .map(|(p, _)| {
                let name = air_ledger::paths::worker_name_for(&p).unwrap_or_else(|_| {
                    p.file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default()
                });
                (name, p)
            })
            .collect(),
        Err(e) => {
            return super::readers::TreeReaders::unknown(format!("git worktree list: {e}"));
        }
    };
    let session_pids = ledger
        .conn()
        .prepare("SELECT pid FROM sessions WHERE pid IS NOT NULL")
        .and_then(|mut st| {
            st.query_map([], |r| r.get::<_, i64>(0))?
                .collect::<Result<std::collections::BTreeSet<i64>, _>>()
        })
        .unwrap_or_default();
    super::readers::gather(raw, &trees, &session_pids)
}

/// One in-flight run as the refusal and the landings row name it: who, how long, what, where,
/// and the pid to stop.
pub fn in_flight_run_line(f: &air_ledger::verify::InFlight, at: &str) -> String {
    match f.pid {
        Some(p) => format!("{} (pid {p})", super::status::in_flight_line(f, at)),
        None => super::status::in_flight_line(f, at),
    }
}

/// What a landing does with bd's answer about its beads' acceptance (air-bh4). An answer is
/// the clause lists, one per bead, empty where a bead states none. No answer is a REFUSAL,
/// never an empty list: "could not evaluate" and "evaluated and found nothing" must not read
/// alike, and a check that degrades to a no-op is a green over an empty population,
/// indistinguishable from a green over a full one (the adopter's w1, 2026-08-31).
///
/// Refusing here is cheap because it happens before the merge: nothing has moved, and the fix
/// is to run the same command again when bd answers. Pure, so the probe reaches the branch.
pub fn acceptance_read(
    result: Result<Vec<Vec<String>>, String>,
    beads: &[String],
) -> Result<Vec<Vec<String>>, String> {
    match result {
        Ok(c) => Ok(c),
        Err(e) => Err(format!(
            "refused: bd did not answer for {} ({e}), so their acceptance could not be read. \
             Nothing was changed: a landing whose check did not run must not be recorded as \
             one that checked and found nothing (fix: run the same `air land` again when bd \
             answers; `air status` shows bd's median cost today)",
            beads.join(" ")
        )),
    }
}

/// The branch a worker's worktree is on: `worktree-<name>` in both the adopter and this repo.
fn branch_for(worker: &str) -> String {
    format!("worktree-{worker}")
}

/// One branch to land, and every bead it carries.
#[derive(Debug, Clone)]
struct Batch {
    worker: String,
    beads: Vec<String>,
    /// Each bead's acceptance clauses, from the same `bd list --json` the landing list came
    /// from (air-ayp). Parallel to `beads`.
    acceptance: Vec<Vec<String>>,
    /// Minutes the oldest of those beads has waited; `--all` lands oldest first.
    oldest_minutes: i64,
}

/// Group the landings by branch, oldest wait first. One merge per branch, however many beads
/// that branch carries: alpha handed over five beads on one branch on 2026-08-22.
fn batches(landings: &[super::status::Landing]) -> Vec<Batch> {
    let mut by_worker: BTreeMap<String, Batch> = BTreeMap::new();
    for l in landings {
        let b = by_worker.entry(l.worker.clone()).or_insert_with(|| Batch {
            worker: l.worker.clone(),
            beads: Vec::new(),
            acceptance: Vec::new(),
            oldest_minutes: 0,
        });
        // air-kexg: a journal-only landing, or one of only the coordinator's commits, carries
        // no bead, so it adds none here and the batch lands with an empty list, which is what
        // the row should record.
        if let Some(id) = l.bead.clone() {
            b.beads.push(id);
        }
        b.acceptance.push(l.acceptance.clone());
        b.oldest_minutes = b.oldest_minutes.max(l.minutes);
    }
    let mut v: Vec<Batch> = by_worker.into_values().collect();
    v.sort_by(|a, b| {
        b.oldest_minutes
            .cmp(&a.oldest_minutes)
            .then_with(|| a.worker.cmp(&b.worker))
    });
    v
}

/// Where `air land` is being run from. Nothing here is about any particular branch, so it is
/// the half `air status` cannot answer and must not pretend to (air-y3v).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    pub on_main: bool,
}

// A `main_checkout` field was here until air-rr98, refusing "`air land` runs in the main
// checkout; this is a worktree". It could not fire: `run` resolves the main checkout from git's
// common dir before building a `Site`, so the directory checked was always the main checkout,
// and a repo git cannot resolve fails earlier, opening the ledger.

// A `dirty` field was here until air-odv, refusing a landing when main had uncommitted tracked
// changes. Its only reason was that the rollback was `git reset --hard`, which would have eaten
// them (the adopter land.sh:504-514). There is no rollback now: `merge --ff-only` refuses on its
// own if a local change is actually in the way and leaves the tree alone when it is not. The
// refusal blocked three lands on 2026-08-29 to protect against a reset that no longer happens.

/// Why one BRANCH cannot be landed, with the command that fixes it. Pure over the facts, so
/// `air selftest` can fire every one without a repo (air-3pz).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts<'a> {
    pub worker: &'a str,
    pub branch_exists: bool,
    /// The branch is already an ancestor of main: nothing to land.
    pub already_in_main: bool,
    /// Main is an ancestor of the branch: the worker merged main before handing over.
    pub contains_main: bool,
    /// The branch head, and the sha the recorded green is at.
    pub branch_head: &'a str,
    pub green_at: Option<&'a str>,
}

/// `Ok(true)` land it, `Ok(false)` nothing to do, `Err(msg)` refuse with the fix.
///
/// The site gates first, then [`branch_check`]. Split at air-y3v so `air status` can apply
/// exactly the branch half without inventing site facts it has no business asserting: it may
/// be running from a worktree, where main is not checked out and every branch would read as
/// unlandable.
pub fn check(site: &Site, f: &Facts<'_>) -> Result<bool, String> {
    if !site.on_main {
        return Err("refused: main is not checked out here (fix: `git checkout main`)".to_string());
    }
    branch_check(f)
}

/// Everything about whether THIS BRANCH can land, and nothing about where the caller is.
///
/// ## air-y3v: one predicate, or the two surfaces lie to each other
///
/// `air status` and the owner inbox (gone since air-uef) used to decide landability themselves — a recorded
/// green at the branch head, and nothing else — while `air land` also required the branch to
/// contain main. Every land invalidates that second condition for every other branch, so the
/// list went stale the instant a land succeeded and offered `air land <bead>` for branches
/// that would be refused. The owner lost three land cycles to it in one hour on 2026-08-29:
///
///     $ <the owner inbox, gone since air-uef>
///       air-1ra  ede1b151  from ledger  (14 min)  air land air-1ra
///     $ air land air-1ra
///     ledger: refused: `worktree-ledger` does not contain main
///
/// Two implementations of one fact will drift; these had. So `select` calls this, and
/// `check` calls this, and there is nothing left to keep in agreement.
pub fn branch_check(f: &Facts<'_>) -> Result<bool, String> {
    let w = f.worker;
    if !f.branch_exists {
        return Err(format!(
            "refused: no branch `{}` (fix: the worker's worktree must be on it)",
            branch_for(w)
        ));
    }
    if f.already_in_main {
        return Ok(false);
    }
    if !f.contains_main {
        return Err(format!(
            "refused: `{}` does not contain main, so the recorded green is not a green of what \
             would land (fix: in {w}'s worktree, {})",
            branch_for(w),
            remerge_command()
        ));
    }
    match f.green_at {
        Some(sha) if sha == f.branch_head => Ok(true),
        Some(sha) => Err(format!(
            "refused: {w}'s recorded green is at {}, not the branch head {} (fix: a green \
             at {w}'s branch head; `air handover` in their worktree names what it needs)",
            sha.get(..8).unwrap_or(sha),
            f.branch_head.get(..8).unwrap_or(f.branch_head)
        )),
        None => Err(format!(
            "refused: no recorded green for {w} at {} (fix: a green at that head; \
             `air handover` in their worktree names what it needs)",
            f.branch_head.get(..8).unwrap_or(f.branch_head)
        )),
    }
}

/// What a branch behind main has to do before it can land. `air status` prints it instead of
/// `air land` for such a branch (air-y3v), and `branch_check`'s refusal names the same one, so
/// the list and the refusal say the same thing.
///
/// The summary `air land` prints when a landing carries clauses Air could not discharge.
///
/// A const so the probe reads the SHIPPED string. air-jy99: this said "either reopen it or file
/// what is left", and an adopter's flow forbids reopening — Air told their coordinator to do what
/// their own rules deny. The clause is rendered twice, here and in `status.rs`, and air-155w's
/// defect survived its first fix because one renderer was corrected and the other kept teaching
/// the forbidden thing. So this states the CONDITION and hands the decision back.
///
/// The "not a contradiction" half is air-k6uh's residual: that bead fixed the per-bead line and
/// left this summary asserting the stronger claim.
pub const REFUTED_SUMMARY: &str = "bead(s) landed with a clause naming a file this merge did not change — a lookup that did \
     not answer, not a contradiction. `air status` names them: read the bead, then see that \
     nothing left over is untracked. How a closed bead's remainder gets tracked is this repo's \
     flow to say; Air prescribes nothing here.";

/// air-155w: this was character-for-character the string that taught an adopter's worker to
/// record a green under a verify lane, on the COORDINATOR's surface — which is why it was easy
/// to miss. The coordinator may record a verify; this line is advice about a WORKER, and under
/// a lane that worker must not. The merge is required under both flows and stays a command; the
/// green becomes the condition it always was.
pub fn remerge_command() -> String {
    "`git merge main`, then a green at the new head (the worker's own, or their lane's)".to_string()
}

/// What to say and record after `git merge --ff-only` returned an error (air-htmn).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FfVerdict {
    /// The fast-forward completed; the error was about the wait, not the ref.
    Landed,
    /// Main genuinely did not move.
    Refused(String),
    /// The look itself failed, so neither is established.
    Unknown(String),
}

/// Pure: given the fast-forward's error text and what a look at main said, what happened.
///
/// `landed` is `Some(true)` when the landing commit is an ancestor of main, `Some(false)` when
/// it is not, and `None` when the check could not be made — the ancestry probe runs through the
/// same 1500 ms bound that produced the original error, so it can time out too.
///
/// **Pure so the probe drives the decision rather than constructing the outcome it asserts.**
/// The bead anticipated that: a probe that has to build the post-fast-forward state itself is
/// testing its own constructor, which is the trap air-682 names and which I hit twice earlier
/// tonight. Here the inputs are the two facts and the output is the verdict, so neither is
/// derived from the other.
pub fn after_fast_forward(err: &str, landed: Option<bool>) -> FfVerdict {
    match landed {
        Some(true) => FfVerdict::Landed,
        Some(false) => FfVerdict::Refused(format!(
            "could not fast-forward main onto the landing commit, so main is untouched. Usually \
             main moved, or a local change is in the way: {err}"
        )),
        None => FfVerdict::Unknown(format!(
            "the fast-forward returned an error and Air could not then read main, so whether \
             the landing happened is NOT established: {err}. Check with `git merge-base \
             --is-ancestor <landing commit> main` before running `air land` again; nothing was \
             recorded either way, and the in-flight row stands until it is."
        )),
    }
}

/// What one branch's landing did.
enum Outcome {
    Landed {
        merge: String,
        /// Main before the landing, and the worker branch heads the landed branch carried
        /// (air-80x.2), so each member can be told (air-1vri.2).
        tip: String,
        members: Vec<air_ledger::landings::Member>,
        /// Beads whose acceptance the merge could not fully discharge, with the clauses. A
        /// record of what the print said; nothing here closes or blocks anything (air-ayp).
        noted: Vec<air_ledger::landings::OpenBead>,
    },
    Nothing,
    // `Rewound` was here until air-odv. Nothing can rewind main any more: the landing commit
    // is built off main and main is fast-forwarded onto it only when it is already green. The
    // `rewound` RESULT STRING stays in the ledger for the two rows that recorded one.
    Refused(String),
}

/// One line saying why a listed branch is blocked, and the fix, for a refusal that names it.
fn blocked_line(l: &super::status::Landing) -> String {
    format!(
        "\n  {} [{}]: {}\n    fix: {}",
        l.worker,
        l.bead.as_deref().unwrap_or("no bead"),
        l.blocked.as_deref().unwrap_or(""),
        l.command
    )
}

/// What `air land <bead>…` or `air land --worker <name>…` selects, out of the same list
/// `air status` shows. Pure over the selection, so both of the adopter's observed cases are
/// probed without a repo (air-09b).
///
/// A branch is the unit `air land` merges (one merge per branch, however many beads it
/// carries), and `--worker` names that unit directly. A bead is a handle on a branch only
/// while exactly one branch carries it. The adopter, 2026-08-30, twice in one round: a bead
/// carried by a batching lane AND by the worker whose commits it batched. Named, the bead
/// landed the oldest-waiting branch — the worker's — main moved, the lane was refused for
/// main-moved, and four beads did not land. Then, with the worker's branch blocked, naming the
/// bead was refused outright instead of reaching the lane that could land.
///
/// So a bead on more than one branch is refused, and every carrier is named with the command
/// that lands it or the fix that unblocks it. Not resolved by ordering, and not by state
/// either: landing "whichever one is landable" is the same silent pick as landing the oldest,
/// and the branch the coordinator meant may be the blocked one. A bead on ONE blocked branch
/// is refused with that branch's reason, exactly as before.
pub fn resolve(
    beads: &[String],
    workers: &[String],
    ready: &[super::status::Landing],
    blocked: &[super::status::Landing],
    skipped: &[super::status::Skipped],
) -> Result<Vec<super::status::Landing>, String> {
    if !workers.is_empty() {
        let mut out: Vec<super::status::Landing> = Vec::new();
        for w in workers {
            let mine: Vec<super::status::Landing> =
                ready.iter().filter(|l| &l.worker == w).cloned().collect();
            if !mine.is_empty() {
                out.extend(mine);
                continue;
            }
            if let Some(b) = blocked.iter().find(|l| &l.worker == w) {
                return Err(format!("refused: {w} not landable yet.{}", blocked_line(b)));
            }
            if let Some(sk) = skipped.iter().find(|s| &s.worker == w) {
                return Err(format!(
                    "refused: {w} has nothing landable.\n  {} [{}]: {}\n    fix: {}",
                    sk.worker, sk.check, sk.detail, sk.fix
                ));
            }
            return Err(format!(
                "refused: no branch `{}` in the selection: no such worktree, or everything on \
                 it is already in main. `air status` lists every branch it can see and why \
                 each may or may not land.",
                branch_for(w)
            ));
        }
        return Ok(out);
    }
    // air-dnr: a bead SELECTS a branch; it does not filter the branch's beads. The merge is
    // per branch and carries everything in `main..<head>` whatever was typed, so the record
    // has to say so too. The adopter, 2026-08-30: `air land <one-bead>` on a lane carrying five
    // recorded `beads = [that one]`, and four beads landed with no acceptance check and no
    // wrong-close detection — the external check air-ayp exists for, skipped for most of the
    // batch. Chosen branches are collected here and expanded to every ready landing on them
    // below, exactly as `--worker` does.
    let mut chosen: BTreeSet<&str> = BTreeSet::new();
    let mut ambiguous: Vec<String> = Vec::new();
    let mut named_blocked: Vec<&super::status::Landing> = Vec::new();
    let mut missing: Vec<&str> = Vec::new();
    for bead in beads {
        let carriers: BTreeSet<&str> = ready
            .iter()
            .chain(blocked)
            .filter(|l| l.bead.as_deref() == Some(bead.as_str()))
            .map(|l| l.worker.as_str())
            .collect();
        if carriers.len() > 1 {
            let mut s = format!("\n  {bead} is carried by {} branches:", carriers.len());
            for w in carriers {
                match blocked
                    .iter()
                    .find(|l| l.worker == w && l.bead.as_deref() == Some(bead.as_str()))
                {
                    Some(b) => s.push_str(&format!(
                        "\n    {w}: blocked: {}\n      fix: {}",
                        b.blocked.as_deref().unwrap_or(""),
                        b.command
                    )),
                    None => s.push_str(&format!("\n    {w}: landable: `air land --worker {w}`")),
                }
            }
            ambiguous.push(s);
            continue;
        }
        if carriers.is_empty() {
            missing.push(bead);
            continue;
        }
        match blocked
            .iter()
            .find(|l| l.bead.as_deref() == Some(bead.as_str()))
        {
            Some(b) => named_blocked.push(b),
            None => chosen.extend(carriers),
        }
    }
    let out: Vec<super::status::Landing> = ready
        .iter()
        .filter(|l| chosen.contains(l.worker.as_str()))
        .cloned()
        .collect();
    if !ambiguous.is_empty() {
        return Err(format!(
            "refused: a bead names a branch only while one branch carries it; name the branch \
             with `--worker`.{}",
            ambiguous.join("")
        ));
    }
    // air-y3v: a bead whose branch the list already knows is blocked is refused with THAT
    // reason, not with "no green branch names it". The surface said re-merge; so does this.
    if !named_blocked.is_empty() {
        return Err(format!(
            "refused: {} not landable yet.{}",
            named_blocked
                .iter()
                .filter_map(|l| l.bead.as_deref())
                .collect::<Vec<_>>()
                .join(", "),
            named_blocked
                .iter()
                .map(|l| blocked_line(l))
                .collect::<Vec<_>>()
                .join("")
        ));
    }
    if !missing.is_empty() {
        return Err(format!(
            "refused: no green branch names {} in its merge range. `air status` lists what \
             is landable (a branch with a recorded green at its head; its beads are the \
             ones its commits declare in a `Bead:` trailer).{}",
            missing.join(", "),
            // Say what each branch failed on, so the named-bead refusal diagnoses as well
            // as the --all one (air-6u5).
            skipped
                .iter()
                .map(|sk| {
                    format!(
                        "\n  {} [{}]: {}\n    fix: {}",
                        sk.worker, sk.check, sk.detail, sk.fix
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        ));
    }
    Ok(out)
}

pub fn run(
    repo: &Path,
    beads: &[String],
    workers: &[String],
    all: bool,
    despite_inflight: bool,
    json: bool,
) -> i32 {
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air land: {e}");
            return 1;
        }
    };
    let role = super::caller_role();
    let inputs = serde_json::json!({
        "beads": beads, "workers": workers, "all": all, "role": role, "repo_worker": worker
    });
    if let Err(msg) = may_land(role) {
        log_event(
            &ledger,
            &worker,
            super::decisions::LAND_REFUSE_ROLE,
            &inputs,
            &msg,
            "role",
        );
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        return 2;
    }
    // A stopped fleet lands nothing (air-1vri.1): a green recorded during the stop waits.
    if let Some(msg) = super::fleet::refusal(&ledger, "air land") {
        log_event(
            &ledger,
            &worker,
            super::decisions::LAND_FLEET_STOPPED,
            &inputs,
            &msg,
            "1 ledger row",
        );
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        return 2;
    }
    // Landing moves main in the main checkout, wherever the command was run from.
    let main = super::worktree::main_checkout(repo);
    let repo = main.as_path();
    if beads.is_empty() && workers.is_empty() && !all {
        eprintln!("air land: name a bead, `--worker <name>`, or `--all`");
        return 1;
    }
    // The landings are derived, never stored: the same facts `air status` shows (air-6p5).
    let sel = super::status::select(repo);
    // air-6u5: a failure during selection is NEVER an empty queue. `{"landed": [], "ok": true}`
    // is the worst answer available — there is nothing to disbelieve, so a caller concludes
    // the queue is empty. The adopter read exactly that with every precondition verified by hand
    // and fell back to their own `make land`.
    if !sel.errors.is_empty() {
        let msg = format!(
            "refused: could not work out what is landable, so nothing was attempted:\n  {}",
            sel.errors.join("\n  ")
        );
        log_event(
            &ledger,
            &worker,
            super::decisions::LAND_ERROR,
            &inputs,
            &msg,
            "selection",
        );
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        return 1;
    }
    // air-y3v: `select` now returns branches it can see are blocked, so the coordinator's
    // surfaces can SHOW them with the command that unblocks them. They are not candidates.
    // A named bead that is blocked is refused below with the reason the list already gave.
    let (ready, blocked): (Vec<_>, Vec<_>) =
        sel.landings.into_iter().partition(|l| l.blocked.is_none());
    // ...and nothing landable is a REPORT, not silence: every branch says which precondition
    // it failed and the command that fixes it.
    if all && ready.is_empty() {
        let mut msg = String::from("nothing is landable right now.");
        for l in &blocked {
            msg.push_str(&blocked_line(l));
        }
        if sel.skipped.is_empty() && blocked.is_empty() {
            msg.push_str(" No worker worktree exists to land from.");
        }
        for sk in &sel.skipped {
            msg.push_str(&format!(
                "\n  {} [{}]: {}\n    fix: {}",
                sk.worker, sk.check, sk.detail, sk.fix
            ));
        }
        log_event(
            &ledger,
            &worker,
            super::decisions::LAND_NONE_LANDABLE,
            &inputs,
            &msg,
            &format!("{} branch(es) checked", sel.skipped.len()),
        );
        emit(
            json,
            &serde_json::json!({"ok": false, "landed": [], "reason": msg, "skipped": sel.skipped}),
            || msg.clone(),
        );
        return 2;
    }
    let wanted: Vec<super::status::Landing> = if all {
        ready
    } else {
        match resolve(beads, workers, &ready, &blocked, &sel.skipped) {
            Ok(w) => w,
            Err(msg) => {
                log_event(
                    &ledger,
                    &worker,
                    super::decisions::LAND_REFUSE,
                    &inputs,
                    &msg,
                    "selection",
                );
                emit(
                    json,
                    &serde_json::json!({"ok": false, "reason": msg}),
                    || msg.clone(),
                );
                return 2;
            }
        }
    };
    if wanted.is_empty() {
        emit(json, &serde_json::json!({"ok": true, "landed": []}), || {
            "nothing to land".to_string()
        });
        return 0;
    }

    let mut landed: Vec<String> = Vec::new();
    let mut held_open: Vec<air_ledger::landings::OpenBead> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut code = 0;
    // air-1bm: a verify in flight REFUSES the landing (it used to warn and land, air-4cr), and
    // `--despite-inflight` is the recorded way past. The whole reasoning is on
    // `in_flight_refusal`; what happens here is only which of the two paths was taken.
    let at = now();
    // A precheck is not destroyed by main moving: it answers for the worker's head, and
    // batch-ready does not ask that head to contain main (an adopter's fleet protocol, 2026-09-25). Refusing on one would
    // hold landings for every worker's cheap check under a lane, and protect nothing.
    let flights: Vec<_> = super::status::verifies_in_flight(&ledger)
        .into_iter()
        .filter(|f| f.kind != Kind::Precheck)
        .collect();
    let despite: Vec<String> = flights.iter().map(|f| in_flight_run_line(f, &at)).collect();
    if let Some(msg) = in_flight_refusal(&flights, &at) {
        if !despite_inflight {
            log_event(
                &ledger,
                &worker,
                super::decisions::LAND_REFUSE_IN_FLIGHT,
                &inputs,
                &msg,
                &format!("{} verify(ies) in flight", flights.len()),
            );
            emit(
                json,
                &serde_json::json!({"ok": false, "landed": [], "reason": msg, "in_flight": flights}),
                || msg.clone(),
            );
            return 2;
        }
        // The override is the measurement: an event line now, and the runs on every row this
        // invocation writes.
        let note = format!(
            "landing despite {} verify(ies) in flight (--despite-inflight), which this \
             destroys:\n  {}",
            flights.len(),
            despite.join("\n  ")
        );
        log_event(
            &ledger,
            &worker,
            super::decisions::LAND_DESPITE_INFLIGHT,
            &inputs,
            &note,
            &format!("{} verify(ies) in flight", flights.len()),
        );
        lines.push(note.clone());
        if !json {
            eprintln!("{note}");
        }
    }
    // Not a refusal, and after the in-flight one so that stays exactly as it was: the processes
    // `air record` never saw that have their cwd in the main checkout (`cmd::readers`). An
    // adopter's landing was about to move main under a lane 1m41s into an unrecorded run
    // (2026-09-07), and the in-flight refusal cannot see one. Silent when there is none; the
    // event line carries how many processes were examined either way.
    let mut readers = main_readers(repo, &ledger);
    let main_path = super::readers::resolved(repo);
    super::readers::drop_ignored(&mut readers, &main_path);
    if let Some(w) = super::readers::main_warning(&readers, &main_path) {
        // Its own event line, so `air audit` counts it (`mechanisms.rs` `land-main-readers`).
        log_event(
            &ledger,
            &worker,
            super::decisions::LAND_MAIN_READERS,
            &inputs,
            &w,
            &format!("{} process(es) examined", readers.examined),
        );
        lines.push(w.clone());
        if !json {
            eprintln!("{w}");
        }
    } else {
        lines.push(format!(
            "main checkout: no unrecorded process reading it ({} process(es) examined)",
            readers.examined
        ));
    }
    // air-1vri.4: main before the first landing, main after the last, and every member head,
    // so one "main moved" notice covers this whole invocation.
    let mut moved: (
        Option<String>,
        Option<String>,
        Vec<super::fanout::LandedMember>,
    ) = (None, None, Vec::new());
    for batch in batches(&wanted) {
        match land_one(repo, &ledger, &batch, &despite, json) {
            Outcome::Landed {
                merge,
                noted,
                tip,
                members,
            } => {
                moved.0.get_or_insert(tip.clone());
                moved.1 = Some(merge.clone());
                moved
                    .2
                    .extend(members.into_iter().map(|m| (m, tip.clone())));
                lines.push(format!(
                    "landed {} ({}) at {}",
                    batch.worker,
                    batch.beads.join(" "),
                    merge.get(..8).unwrap_or(&merge)
                ));
                // Only the clauses naming a file the merge did not change (air-ppf): the
                // print above already showed the unreadable ones with their own `?` marker.
                // air-k6uh: reported as the lookup it is, never as a contradiction.
                for o in noted.iter().filter(|o| o.refuted) {
                    lines.push(format!(
                        "  {} UNCONFIRMED: {} — a lookup that did not answer, not a \
                         contradiction; read the bead",
                        o.bead, o.contradicted
                    ));
                }
                landed.extend(batch.beads.clone());
                held_open.extend(noted);
            }
            Outcome::Nothing => lines.push(format!("{}: already in main", batch.worker)),
            Outcome::Refused(why) => {
                lines.push(format!("{}: {why}", batch.worker));
                code = 2;
                break;
            }
        }
    }
    // Nothing is closed here. The worker closes its own bead with proof before the branch
    // lands (owner ruling, 2026-08-22); this command merges, verifies, and reports.
    let refuted = held_open.iter().filter(|o| o.refuted).count();
    if refuted > 0 {
        lines.push(format!("{refuted} {REFUTED_SUMMARY}"));
    }
    // air-1vri.4: every other session hears that main moved, once per landing.
    if let (Some(before), Some(merge)) = (&moved.0, &moved.1) {
        super::fanout::main_moved(
            repo, &ledger, &worker, role, before, merge, &landed, &moved.2,
        );
    }
    // air-1vri.2: the lane's next step, as of this landing. It ran the command, so it learns
    // here and the channel does not tell it again.
    let mut next_ready = Vec::new();
    if !landed.is_empty() {
        let (ready, _, _) = super::status::batch_ready_for(&ledger, repo);
        next_ready = ready.into_iter().filter(|b| b.worker != worker).collect();
        if role == "lane" {
            super::fanout::told_lane(&ledger, &worker, &next_ready, &now());
        }
        lines.extend(super::fanout::next_cut_lines(&next_ready));
    }
    let msg = lines.join("\n");
    log_event(
        &ledger,
        &worker,
        if code == 0 {
            super::decisions::LAND_LANDED
        } else {
            super::decisions::LAND_STOPPED
        },
        &inputs,
        &msg,
        &format!(
            "{} bead(s) merged, {} with clauses Air could not discharge",
            landed.len(),
            held_open.len()
        ),
    );
    emit(
        json,
        &serde_json::json!({
            "ok": code == 0,
            "landed": landed,
            "not_discharged": held_open,
            "batch_ready": next_ready,
            "log": lines,
        }),
        || msg.clone(),
    );
    code
}

/// The landing commit's title: `Land <branch>: <beads>`, or `Land <branch>` with none.
fn landing_title(branch: &str, beads: &[String]) -> String {
    if beads.is_empty() {
        format!("Land {branch}")
    } else {
        format!("Land {branch}: {}", beads.join(" "))
    }
}

/// Record the landing commit as green when its tree is byte-identical to the verified
/// commit's: the same run, at the new sha. Otherwise record nothing. Every trial from 0.4.5 to
/// 0.4.8 read main as "not green" after each landing, because under `verify_key: commit` the
/// new sha had no run of its own.
fn record_landing_green(
    repo: &Path,
    ledger: &air_ledger::Ledger,
    verified: &str,
    landed: &str,
    main_before: &str,
) {
    let tree = |c: &str| git::run(repo, &["rev-parse", &format!("{c}^{{tree}}")]).ok();
    let (Some(t), Some(l)) = (tree(verified), tree(landed)) else {
        return;
    };
    if t != l {
        return;
    }
    let Ok(Some(green)) = ledger.green_at(verified, Some(&t), Kind::Verify) else {
        return;
    };
    let run = air_ledger::verify::VerifyRun {
        id: new_id(),
        sha: landed.to_string(),
        trigger: "land".to_string(),
        tree: Some(l),
        members: Vec::new(),
        main_sha: Some(main_before.to_string()),
        ..green.run().clone()
    };
    let _ = ledger.record_verify(&run);
}

/// Land one branch: build the merge commit off main and fast-forward onto it. No verify
/// runs here and there is nothing to rewind (air-odv).
fn land_one(
    repo: &Path,
    ledger: &air_ledger::Ledger,
    batch: &Batch,
    despite: &[String],
    json: bool,
) -> Outcome {
    let branch = branch_for(&batch.worker);
    let started_at = now();
    let tip = match git::head(repo) {
        Ok(h) => h,
        Err(e) => return Outcome::Refused(format!("cannot read main's HEAD: {e}")),
    };
    let branch_head = git::run(repo, &["rev-parse", &format!("{branch}^{{commit}}")]).ok();
    // The same predicate `select` and the gate read (air-7wf, after air-y3v): a branch is
    // green here exactly when `air status` said it was.
    let green = branch_head.as_deref().and_then(|h| {
        super::green::at(ledger, repo, h, Kind::Verify)
            .ok()
            .filter(super::green::Evidence::holds)
            .map(|_| h.to_string())
    });
    let site = Site {
        on_main: git::run(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).is_ok_and(|b| b == "main"),
    };
    let facts = Facts {
        worker: &batch.worker,
        branch_exists: branch_head.is_some(),
        already_in_main: branch_head
            .as_deref()
            .and_then(|h| git::is_ancestor(repo, h, &tip).ok())
            .unwrap_or(false),
        contains_main: branch_head
            .as_deref()
            .and_then(|h| git::is_ancestor(repo, &tip, h).ok())
            .unwrap_or(false),
        branch_head: branch_head.as_deref().unwrap_or(""),
        green_at: green.as_deref(),
    };
    // air-80x.2: which worker branches this branch contains that main does not. A verify
    // lane's batch lands once; the row says which branches rode in it, and a red batch's
    // report reads the same list. Empty for an ordinary single branch.
    let members = branch_head
        .as_deref()
        .map(|h| super::batch::members_of(repo, &batch.worker, h, &tip))
        .unwrap_or_default();
    // air-bxe: ONE row per attempt, written more than once. The id and the attempt number are
    // fixed here so the `in-flight` write and the outcome write are the same row; deriving
    // `attempt_no` inside the closure would count the row it is about to update.
    let row_id = new_id();
    let attempt_no = ledger
        .landing_attempts(&batch.worker)
        .unwrap_or(0)
        .saturating_add(1);
    let record_full = |result: &str,
                       merge: Option<String>,
                       verify: Option<String>,
                       step: Option<String>,
                       open: &[air_ledger::landings::OpenBead]| {
        let _ = ledger.record_landing(&LandingRow {
            id: row_id.clone(),
            worker: batch.worker.clone(),
            sha: facts.branch_head.to_string(),
            tip_sha: Some(tip.clone()),
            result: result.to_string(),
            failing_step: step,
            verify_run_id: verify,
            attempt_no,
            beads: batch.beads.clone(),
            open_beads: open.to_vec(),
            merge_commit: merge,
            // The `air land` process, so an `in-flight` row can say whether it is still
            // running. This is the fact `pgrep` was asked for and got wrong twice.
            pid: Some(i64::from(std::process::id())),
            started_at: started_at.clone(),
            finished_at: now(),
            despite_inflight: despite.to_vec(),
            members: members.clone(),
        });
    };
    let record =
        |result: &str, merge: Option<String>, verify: Option<String>, step: Option<String>| {
            record_full(result, merge, verify, step, &[]);
        };
    match check(&site, &facts) {
        Ok(false) => return Outcome::Nothing,
        Err(why) => {
            record("refused", None, None, Some("check".into()));
            return Outcome::Refused(why);
        }
        Ok(true) => {}
    }
    // ── air-odv ────────────────────────────────────────────────────────────────────────
    // Build the merge OFF main, then fast-forward. Main is never moved to a commit that has
    // not been verified, so there is no armed window and no rewind.
    //
    // This used to `git merge --no-ff` into main, run the repo's verify there, and
    // `git reset --hard` on red. Between the merge and the verdict main held unverified code.
    // Twice a land was killed inside that window and left main at an unverified merge with no
    // landings row (d10ddab on 2026-08-22, 35660df today); both happened to be green, and
    // nothing guaranteed that.
    //
    // The second verify was also re-verifying a tree that had just been verified. `air land`
    // already refuses unless the branch CONTAINS main, so `main..branch` is a straight line
    // and the merge result's tree is byte-identical to the branch head's — the tree the worker
    // recorded a green for. It only looked necessary because the gate is keyed to a sha and a
    // merge commit is a new sha over the same tree.
    //
    // So: `commit-tree` builds the merge commit from the branch's tree with main as first
    // parent, touching no working tree, and `merge --ff-only` moves main onto it. This is what
    // merge queues do (Bors, Zuul, GitHub's merge queue); mutating the shared branch into a
    // state you may have to undo is the thing they exist to avoid.
    //
    // The invariant is re-checked HERE rather than trusted from `select`: main can move
    // between the two (another land, the owner), which would turn the removed rewind window
    // into a race. If it has moved, the branch no longer contains main and this refuses;
    // `--ff-only` is the second backstop, because a main that moved cannot be fast-forwarded.
    let tip_now = match git::head(repo) {
        Ok(h) => h,
        Err(e) => return Outcome::Refused(format!("cannot read main's HEAD: {e}")),
    };
    let head = facts.branch_head;
    if !git::is_ancestor(repo, &tip_now, head).unwrap_or(false) {
        record("refused", None, None, Some("main-moved".into()));
        return Outcome::Refused(format!(
            "refused: main is at {} and `{branch}` does not contain it, so its green is not a \
             green of what would land. Main moved between the landable list and this merge. \
             Nothing was changed (fix: in {}'s worktree, {})",
            tip_now.get(..8).unwrap_or(&tip_now),
            batch.worker,
            remerge_command()
        ));
    }
    // air-bh4: the acceptance text is read BEFORE anything moves, so bd not answering refuses
    // with main untouched — the same shape as `air claim` (four arms, every one returns
    // before a write) and `air triage`. It used to be read after the fast-forward and, on a
    // bd timeout, degrade to an empty clause list: the row then said "the bead states no
    // acceptance criteria", a positive false statement about a bead Air never read, and the
    // one external check on close-with-proof recorded a clean result for a check that did
    // not run (the adopter, 2026-08-31). `bd show` costs ~1.4 s per id, which is fine
    // beside a landing and ruinous on every `air status` (air-7kp).
    let clauses = match acceptance_read(
        super::status::acceptance_for(repo, &batch.beads),
        &batch.beads,
    ) {
        Ok(c) => c,
        Err(why) => {
            record("refused", None, None, Some("acceptance-unread".into()));
            return Outcome::Refused(why);
        }
    };
    let message = landing_title(&branch, &batch.beads);
    let tree = match git::run(repo, &["rev-parse", &format!("{head}^{{tree}}")]) {
        Ok(t) => t,
        Err(e) => return Outcome::Refused(format!("cannot read `{branch}`'s tree: {e}")),
    };
    let merge = match git::run(
        repo,
        &[
            "commit-tree",
            &tree,
            "-p",
            &tip_now,
            "-p",
            head,
            "-m",
            &message,
        ],
    ) {
        Ok(sha) => sha,
        Err(e) => {
            record("refused", None, None, Some("commit-tree".into()));
            return Outcome::Refused(format!(
                "could not build the landing commit off main; main is untouched: {e}"
            ));
        }
    };
    // air-bxe, narrowed by air-odv: the row still says a landing started and has not reported,
    // which is what `pgrep` was being asked. What it no longer means is "main holds unverified
    // code with a rollback armed" — the fast-forward below is the only thing that moves main,
    // and it moves it onto a commit whose tree is already green.
    record("in-flight", Some(merge.clone()), None, None);
    if let Err(e) = git::run(repo, &["merge", "--ff-only", &merge]) {
        // air-htmn: LOOK before saying anything about main. `git merge --ff-only` updates the
        // ref atomically and `git::run` kills the child on timeout, which does not undo a ref
        // update — so an `Err` here means "I stopped waiting", never "it did not happen". An
        // adopter's coordinator was told "main is untouched" while main was at the landing
        // commit, and ran `air land` a second time; the second call's correct refusal is the
        // only reason they found out the first had worked.
        //
        // The worse half was the row: `record("refused", …)` ran on this branch too, so the
        // ledger said refused for a landing that happened, and `landings()`, `landed_open()`
        // and `air status` all read that row afterwards. A wrong sentence is read once.
        match after_fast_forward(&e.to_string(), git::is_ancestor(repo, &merge, "HEAD").ok()) {
            FfVerdict::Landed => {}
            FfVerdict::Refused(msg) => {
                record("refused", None, None, Some("fast-forward".into()));
                return Outcome::Refused(msg);
            }
            FfVerdict::Unknown(msg) => {
                // Deliberately records NOTHING. The in-flight row above already says a landing
                // started and has not reported (air-bxe), which is exactly true here, and
                // writing "refused" would be this bug again one layer over.
                return Outcome::Refused(msg);
            }
        }
    }
    // The landing commit was built FROM the branch's tree, so main's tree is now byte-identical
    // to the one this worker recorded a green at. That equality is the whole reason no verify
    // runs here, so assert it rather than reason about it — and against what git says main's
    // tree is now, not against the variable it was built from.
    match git::run(repo, &["rev-parse", "HEAD^{tree}"]) {
        Ok(landed) if landed == tree => {}
        Ok(landed) => {
            return Outcome::Refused(format!(
                "main's tree after the fast-forward is {landed}, not `{branch}`'s {tree}. That \
                 should be impossible; it would mean the recorded green does not describe what \
                 landed."
            ));
        }
        Err(e) => return Outcome::Refused(format!("cannot read main's tree after landing: {e}")),
    }
    record_landing_green(repo, ledger, head, &merge, &tip_now);
    // ── air-ayp ────────────────────────────────────────────────────────────────────────
    // Layer 1, the part that is true under either closure model: read every bead's acceptance
    // and print it beside Air's verdict, so a wrong close is visible at the moment it lands.
    // Nothing closes on branch containment.
    let changed = git::run(repo, &["diff", "--name-only", &format!("{tip}..{merge}")])
        .map(|s| s.lines().map(str::to_string).collect::<Vec<_>>())
        .unwrap_or_default();
    // air-dqa: every file at the landed commit, so a path-like token that is no file is
    // reported as unreadable rather than as a file the merge failed to touch. One git call
    // per landing. If it fails the list is empty and every missing path reads as unresolvable,
    // which is the safe direction: silence over a false accusation.
    let tree = git::run(repo, &["ls-tree", "-r", "--name-only", &merge])
        .map(|s| s.lines().map(str::to_string).collect::<Vec<_>>())
        .unwrap_or_default();
    let ev = acceptance::Evidence {
        green_at_landed: true, // the run above, recorded at `merge`
        changed: &changed,
        tree: &tree,
    };
    // `clauses` was read before the merge (air-bh4), so every bead here was actually read.
    let judged: Vec<acceptance::Judged> = batch
        .beads
        .iter()
        .enumerate()
        .map(|(i, bead)| {
            acceptance::judge_clauses(bead, clauses.get(i).cloned().unwrap_or_default(), &ev)
        })
        .collect();
    if !json {
        print!("{}", acceptance::report(&judged));
    }
    // `air land` closes nothing (owner ruling, 2026-08-22: the worker closes its own bead with
    // proof). What the print said is kept on the row so it outlives the scrollback, and a
    // REFUTED clause is the wrong-close signal the coordinator is told about.
    let noted: Vec<air_ledger::landings::OpenBead> = judged
        .iter()
        .filter(|j| !j.all_discharged())
        .map(|j| air_ledger::landings::OpenBead {
            bead: j.bead.clone(),
            why: j.why_open(),
            refuted: j.refuted(),
            contradicted: j.why_contradicted(),
        })
        .collect();
    record_full(
        if noted.iter().any(|o| o.refuted) {
            "landed-refuted"
        } else {
            "landed"
        },
        Some(merge.clone()),
        // air-odv: no verify runs here any more, so there is no run to point at. The evidence
        // for this landing is the worker's own green at the branch head, which `check` required
        // and whose tree is what main now carries.
        None,
        None,
        &noted,
    );
    Outcome::Landed {
        merge,
        noted,
        tip,
        members,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// 0.4.8 trial: the landing commit is green only when its tree is the verified one's.
    #[test]
    fn a_landing_commit_is_green_only_over_the_verified_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let g = |args: &[&str]| git::run(repo, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "air"]);
        g(&["config", "user.email", "air@example.invalid"]);
        std::fs::write(repo.join("a"), "a\n").unwrap();
        g(&["add", "a"]);
        g(&["commit", "-q", "-m", "a"]);
        let verified = g(&["rev-parse", "HEAD"]);
        let tree = g(&["rev-parse", "HEAD^{tree}"]);
        let same = g(&["commit-tree", &tree, "-p", &verified, "-m", "same tree"]);
        std::fs::write(repo.join("a"), "b\n").unwrap();
        g(&["commit", "-q", "-am", "b"]);
        let other = g(&["rev-parse", "HEAD"]);
        let ledger = air_ledger::Ledger::open_in_memory().unwrap();
        ledger
            .record_verify(&air_ledger::verify::VerifyRun {
                id: new_id(),
                worker: "lane".into(),
                sha: verified.clone(),
                kind: Kind::Verify,
                exit_code: 0,
                trigger: "manual".into(),
                failing_step: None,
                started_at: "2026-09-26T00:00:00Z".into(),
                finished_at: "2026-09-26T00:00:01Z".into(),
                log_path: None,
                command: None,
                duration_ms: None,
                output_bytes: None,
                dirty: false,
                tree: Some(tree),
                members: Vec::new(),
                main_sha: None,
            })
            .unwrap();
        record_landing_green(repo, &ledger, &verified, &other, &verified);
        assert_eq!(ledger.runs_at(&other, Kind::Verify).unwrap(), (0, 0));
        record_landing_green(repo, &ledger, &verified, &same, &verified);
        assert_eq!(ledger.runs_at(&same, Kind::Verify).unwrap(), (1, 0));
        assert_eq!(landing_title("worktree-lane", &[]), "Land worktree-lane");
        assert_eq!(
            landing_title("worktree-lane", &["zz-1".into()]),
            "Land worktree-lane: zz-1"
        );
    }

    fn here() -> Site {
        Site { on_main: true }
    }

    fn ok_facts<'a>(head: &'a str, green: Option<&'a str>) -> Facts<'a> {
        Facts {
            worker: "alpha",
            branch_exists: true,
            already_in_main: false,
            contains_main: true,
            branch_head: head,
            green_at: green,
        }
    }

    #[test]
    fn every_refusal_names_the_command_that_fixes_it() {
        assert_eq!(check(&here(), &ok_facts("abc", Some("abc"))), Ok(true));

        let mut s = here();
        s.on_main = false;
        assert!(
            check(&s, &ok_facts("abc", Some("abc")))
                .unwrap_err()
                .contains("git checkout main")
        );

        let mut f = ok_facts("abc", Some("abc"));
        f.branch_exists = false;
        assert!(check(&here(), &f).unwrap_err().contains("worktree-alpha"));

        // Already in main is not an error: there is simply nothing to do.
        let mut f = ok_facts("abc", Some("abc"));
        f.already_in_main = true;
        assert_eq!(check(&here(), &f), Ok(false));

        let mut f = ok_facts("abc", Some("abc"));
        f.contains_main = false;
        let e = check(&here(), &f).unwrap_err();
        // air-155w: the merge is required under both flows and stays a command; recording
        // a green is the clause a verify lane forbids and is now a condition.
        assert!(e.contains("`git merge main`"), "{e}");
        assert!(!e.contains("air record verify"), "{e}");
        // air-y3v: and it is the same string the landable list offers, so the surface and the
        // refusal cannot say different things.
        assert!(e.contains(&remerge_command()), "{e}");

        // Green recorded, but at an older commit than the branch head.
        let e = check(&here(), &ok_facts("abcdef99", Some("999999aa"))).unwrap_err();
        assert!(e.contains("999999aa") && e.contains("abcdef99"), "{e}");
        let e = check(&here(), &ok_facts("abcdef99", None)).unwrap_err();
        assert!(e.contains("no recorded green"), "{e}");
    }

    /// air-y3v: `branch_check` is the whole branch half, and `check` is the site gates plus
    /// exactly it. If they ever diverge, `air status` and `air land` start disagreeing again.
    #[test]
    fn check_is_the_site_gates_plus_branch_check() {
        for f in [
            ok_facts("abc", Some("abc")),
            Facts {
                contains_main: false,
                ..ok_facts("abc", Some("abc"))
            },
            Facts {
                already_in_main: true,
                ..ok_facts("abc", Some("abc"))
            },
            Facts {
                green_at: None,
                ..ok_facts("abc", Some("abc"))
            },
        ] {
            assert_eq!(check(&here(), &f), branch_check(&f));
        }
    }

    /// One merge per branch however many beads it carries, oldest wait first.
    #[test]
    fn batches_group_by_branch_and_land_the_longest_wait_first() {
        let l = |bead: &str, worker: &str, minutes: i64| super::super::status::Landing {
            bead: Some(bead.into()),
            worker: worker.into(),
            head: "abc".into(),
            minutes,
            command: String::new(),
            acceptance: Vec::new(),
            blocked: None,
        };
        let b = batches(&[
            l("air-1", "alpha", 5),
            l("air-2", "beta", 40),
            l("air-3", "alpha", 30),
        ]);
        assert_eq!(
            b.iter().map(|x| x.worker.as_str()).collect::<Vec<_>>(),
            vec!["beta", "alpha"]
        );
        assert_eq!(b[1].beads, vec!["air-1".to_string(), "air-3".to_string()]);
        assert_eq!(b[1].oldest_minutes, 30);
    }

    /// air-29a, air-jc2p.2: the role is the launcher's, so a worker is refused wherever it runs
    /// and whatever `--repo` says; the lane and the owner may land, the coordinator may not.
    #[test]
    fn only_the_lane_and_the_owner_land() {
        assert!(may_land("lane").is_ok());
        assert!(may_land("owner").is_ok());
        let e = may_land("coordinator").unwrap_err();
        assert!(e.contains("air lane"), "{e}");
        let e = may_land("worker").unwrap_err();
        assert!(e.contains("air handover"), "{e}");
        assert_eq!(crate::cmd::role_from(Some("lane")), "lane");
        assert_eq!(
            crate::cmd::role_from(Some("typo")),
            "worker",
            "unknown fails closed"
        );
        assert_eq!(crate::cmd::role_from(None), "owner");
        assert_eq!(crate::cmd::role_from(Some("coordinator")), "coordinator");
    }
}
