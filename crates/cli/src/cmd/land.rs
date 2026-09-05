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
//! Ported from adopter's `scripts/land.sh` (read 2026-08-22). Taken: main checkout only and
//! on `main` (`land.sh:487-499`); already-an-ancestor is "nothing to land", not an error
//! (`land.sh:533-535`). **Not taken any more**: merging into main and resetting it on red
//! (`land.sh:504-526`), which air-odv replaced — with it went the dirty-tree refusal, whose
//! only reason was that `git reset --hard` would eat uncommitted work. Not taken:
//! adopter's three attribution rules (branch / closing trailer / containment,
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

/// Who is asking, and which checkout they pointed at (air-29a).
///
/// `where_i_am` is derived from the process's working directory; `where_i_pointed` from
/// `--repo`. They are separate because only the first is a fact about the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caller<'a> {
    /// Worker name of the checkout the process is actually running in. `None` when that
    /// cannot be determined, which refuses.
    pub where_i_am: Option<&'a str>,
    /// Worker name of the checkout `--repo` resolves to.
    pub where_i_pointed: &'a str,
}

/// Who may run `air land`. Pure over the caller, so `air selftest` fires every refusal.
///
/// ## air-29a: the identity used to be an argument
///
/// This check has been at the top of `run` since air-3pz, and it was still bypassable, because
/// it was fed `worker_name_for(repo)` — and `repo` is `--repo`, which the caller supplies. From
/// a worktree, `air --repo <main-checkout> land --all` resolved the worker to `main`, `role_for`
/// said coordinator, and the land proceeded. Reproduced from this worktree on 2026-08-29: the
/// command got past this check and began evaluating branches, stopping only because the one
/// candidate happened to fail a precondition.
///
/// It is how worker beta landed `worktree-beta` at d10ddab on 2026-08-22 while checking its own
/// fix, which is the commit at the centre of the following week's investigation.
///
/// The `Bash(air land *)` deny pattern was never the backstop either: it matches command TEXT,
/// so `cargo run -p air -- land`, `./target/debug/air land` and an absolute path all miss it. A
/// parser that guards counts as absent until proven present (`anti-brittleness`), and neither
/// of these two was present.
///
/// So the role now comes from **where the process is**, which is a fact the OS holds rather than
/// one the caller writes. Spelling the command differently cannot change it; the only way to
/// satisfy it is to actually be in the main checkout, which is the authority being claimed.
/// Nothing here parses anything.
pub fn may_land(c: &Caller<'_>) -> Result<(), String> {
    let Some(here) = c.where_i_am else {
        return Err(
            "refused: `air land` cannot tell which checkout it is running in, and the role \
             decides who may land (fix: run it from the main checkout)."
                .to_string(),
        );
    };
    if super::hook::role_for(here) == "coordinator" {
        return Ok(());
    }
    // Name the bypass when that is what this is, rather than a generic refusal: a worker that
    // pointed `--repo` at the main checkout is the exact shape of air-29a.
    let pointed = if super::hook::role_for(c.where_i_pointed) == "coordinator" {
        "\n  `--repo` pointed at the main checkout, but the role comes from where the process \
         runs, not from an argument (air-29a). Spelling the command `cargo run -p air -- land` \
         or `./target/debug/air land` does not change it either."
            .to_string()
    } else {
        String::new()
    };
    Err(format!(
        "refused: `air land` runs in the main checkout, and {here} is a worker. Close your own \
         bead instead: `air handover` names anything missing, then `bd close <id> --reason \
         \"<proof>\"` (owner ruling, 2026-08-22).{pointed}"
    ))
}

/// The worker name of the checkout this process is running in, or `None` when there is no
/// answer (not in a git repo, or the cwd is gone). `None` refuses: an unknown caller is not a
/// coordinator.
pub fn where_i_am() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    air_ledger::paths::worker_name_for(&cwd).ok()
}

/// What `air land` says about the verifies running right now (air-4cr). Empty when nothing is
/// running, so the ok path is silent.
///
/// Landing moves main, and the hand-over gate wants a green at a HEAD containing main, so
/// every verify in flight is about to become worthless. adopter's coordinator did this to
/// three workers in one round with no signal available; their fix was a protocol where the
/// worker warns first, which is exactly the relayed fact Air exists to remove. A full verify
/// is ~420 s there and their landing rate was faster, so no cadence solves it.
///
/// Warn, never refuse (the bead's own default, owner's call): a coordinator may still have to
/// land, and this is a fact, not a gate. **Removal condition**: delete when a round's landings
/// show zero warnings, or show warnings that nothing ever waits on.
pub fn in_flight_warnings(flights: &[air_ledger::verify::InFlight], at: &str) -> Vec<String> {
    flights
        .iter()
        .map(|f| {
            format!(
                "warning: verify in flight, {} — landing now invalidates it and costs a re-run",
                super::status::in_flight_line(f, at)
            )
        })
        .collect()
}

/// The branch a worker's worktree is on: `worktree-<name>` in both adopter and this repo.
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
        b.beads.push(l.bead.clone());
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
    pub main_checkout: bool,
}

// A `dirty` field was here until air-odv, refusing a landing when main had uncommitted tracked
// changes. Its only reason was that the rollback was `git reset --hard`, which would have eaten
// them (adopter land.sh:504-514). There is no rollback now: `merge --ff-only` refuses on its
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
/// be running from a worktree, where `main_checkout` is false and every branch would read as
/// unlandable.
pub fn check(site: &Site, f: &Facts<'_>) -> Result<bool, String> {
    if !site.main_checkout {
        return Err(
            "refused: `air land` runs in the main checkout; this is a worktree.".to_string(),
        );
    }
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
             would land (fix: in {w}'s worktree, `{}`)",
            branch_for(w),
            remerge_command()
        ));
    }
    match f.green_at {
        Some(sha) if sha == f.branch_head => Ok(true),
        Some(sha) => Err(format!(
            "refused: {w}'s recorded green is at {}, not the branch head {} (fix: in {w}'s \
             worktree, `air record verify -- make verify`)",
            sha.get(..8).unwrap_or(sha),
            f.branch_head.get(..8).unwrap_or(f.branch_head)
        )),
        None => Err(format!(
            "refused: no recorded green for {w} at {} (fix: in {w}'s worktree, `air record \
             verify -- make verify`)",
            f.branch_head.get(..8).unwrap_or(f.branch_head)
        )),
    }
}

/// What a branch behind main has to do before it can land. The command `air status` prints
/// instead of `air land` for such a branch (air-y3v), and the same
/// one `branch_check`'s refusal names, so the list and the refusal say the same thing.
pub fn remerge_command() -> String {
    "git merge main && air record verify -- make verify".to_string()
}

/// What one branch's landing did.
enum Outcome {
    Landed {
        merge: String,
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
        l.bead,
        l.blocked.as_deref().unwrap_or(""),
        l.command
    )
}

/// What `air land <bead>…` or `air land --worker <name>…` selects, out of the same list
/// `air status` shows. Pure over the selection, so both of adopter's observed cases are
/// probed without a repo (air-09b).
///
/// A branch is the unit `air land` merges (one merge per branch, however many beads it
/// carries), and `--worker` names that unit directly. A bead is a handle on a branch only
/// while exactly one branch carries it. adopter, 2026-08-30, twice in one round: a bead
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
    let mut out: Vec<super::status::Landing> = Vec::new();
    let mut ambiguous: Vec<String> = Vec::new();
    let mut named_blocked: Vec<&super::status::Landing> = Vec::new();
    let mut missing: Vec<&str> = Vec::new();
    for bead in beads {
        let carriers: BTreeSet<&str> = ready
            .iter()
            .chain(blocked)
            .filter(|l| &l.bead == bead)
            .map(|l| l.worker.as_str())
            .collect();
        if carriers.len() > 1 {
            let mut s = format!("\n  {bead} is carried by {} branches:", carriers.len());
            for w in carriers {
                match blocked.iter().find(|l| l.worker == w && &l.bead == bead) {
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
        match blocked.iter().find(|l| &l.bead == bead) {
            Some(b) => named_blocked.push(b),
            None => out.extend(ready.iter().filter(|l| &l.bead == bead).cloned()),
        }
    }
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
                .map(|l| l.bead.as_str())
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

pub fn run(repo: &Path, beads: &[String], workers: &[String], all: bool, json: bool) -> i32 {
    let (ledger, worker) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air land: {e}");
            return 1;
        }
    };
    // air-29a: the role comes from where this process is, never from `--repo`.
    let here = where_i_am();
    let caller = Caller {
        where_i_am: here.as_deref(),
        where_i_pointed: &worker,
    };
    let inputs = serde_json::json!({
        "beads": beads, "workers": workers, "all": all, "caller": here, "repo_worker": worker
    });
    if let Err(msg) = may_land(&caller) {
        // Logged under the CALLER, so a bypass attempt is attributed to whoever made it
        // rather than to `main`.
        let actor = here.as_deref().unwrap_or("unknown");
        log_event(&ledger, actor, "land", &inputs, "refuse", &msg, "role");
        emit(
            json,
            &serde_json::json!({"ok": false, "reason": msg}),
            || msg.clone(),
        );
        return 2;
    }
    if beads.is_empty() && workers.is_empty() && !all {
        eprintln!("air land: name a bead, `--worker <name>`, or `--all`");
        return 1;
    }
    // The landings are derived, never stored: the same facts `air status` shows (air-6p5).
    let sel = super::status::select(repo);
    // air-6u5: a failure during selection is NEVER an empty queue. `{"landed": [], "ok": true}`
    // is the worst answer available — there is nothing to disbelieve, so a caller concludes
    // the queue is empty. adopter read exactly that with every precondition verified by hand
    // and fell back to their own `make land`.
    if !sel.errors.is_empty() {
        let msg = format!(
            "refused: could not work out what is landable, so nothing was attempted:\n  {}",
            sel.errors.join("\n  ")
        );
        log_event(
            &ledger,
            &worker,
            "land",
            &inputs,
            "error",
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
            "land",
            &inputs,
            "none-landable",
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
                    "land",
                    &inputs,
                    "refuse",
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
    // air-4cr. Landing moves main, and the hand-over gate wants a green at a HEAD containing
    // main, so every verify running right now is about to become worthless. adopter's
    // coordinator did this to three workers in one round and had no signal; their fix was a
    // protocol where the worker warns first. Warn, do not refuse (bead air-4cr, owner's
    // default): a coordinator may still have to land, and a refusal here would be a gate over
    // a fact. Removal condition: delete this warning when a round's landings show it firing
    // zero times, or when it fires and nothing ever waits on it.
    lines.extend(in_flight_warnings(
        &super::status::verifies_in_flight(&ledger),
        &now(),
    ));
    if !json {
        for l in &lines {
            eprintln!("{l}");
        }
    }
    for batch in batches(&wanted) {
        match land_one(repo, &ledger, &batch, json) {
            Outcome::Landed { merge, noted } => {
                lines.push(format!(
                    "landed {} ({}) at {}",
                    batch.worker,
                    batch.beads.join(" "),
                    merge.get(..8).unwrap_or(&merge)
                ));
                // The refuted clauses only (air-ppf): the print above already showed the
                // unreadable ones with their own `?` marker.
                for o in noted.iter().filter(|o| o.refuted) {
                    lines.push(format!("  {} REFUTED: {}", o.bead, o.contradicted));
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
        lines.push(format!(
            "{refuted} bead(s) landed with a clause this merge CONTRADICTS. `air status` names \
             them: read the bead, then either reopen it or file what is left."
        ));
    }
    let msg = lines.join("\n");
    log_event(
        &ledger,
        &worker,
        "land",
        &inputs,
        if code == 0 { "landed" } else { "stopped" },
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
            "log": lines,
        }),
        || msg.clone(),
    );
    code
}

/// Land one branch: build the merge commit off main and fast-forward onto it. No verify
/// runs here and there is nothing to rewind (air-odv).
fn land_one(repo: &Path, ledger: &air_ledger::Ledger, batch: &Batch, json: bool) -> Outcome {
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
        main_checkout: !repo.join(".git").is_file(),
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
             Nothing was changed (fix: in {}'s worktree, `{}`)",
            tip_now.get(..8).unwrap_or(&tip_now),
            batch.worker,
            remerge_command()
        ));
    }
    let message = format!("Land {branch}: {}", batch.beads.join(" "));
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
        record("refused", None, None, Some("fast-forward".into()));
        return Outcome::Refused(format!(
            "could not fast-forward main onto the landing commit, so main is untouched. Usually \
             main moved, or a local change is in the way: {e}"
        ));
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
    // The acceptance text is fetched HERE, for this branch's beads only: `bd show` costs
    // ~1.4 s per id, which is fine beside a full verify and ruinous on every `air status`
    // (air-7kp).
    let clauses = match super::status::acceptance_for(repo, &batch.beads) {
        Ok(c) => c,
        Err(e) => {
            // The merge already happened and verified; refusing now would be worse than
            // saying what is unknown. Report it as unread rather than as absent.
            eprintln!("air land: could not read acceptance from bd, so no clause was checked: {e}");
            Vec::new()
        }
    };
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
    Outcome::Landed { merge, noted }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn here() -> Site {
        Site {
            on_main: true,
            main_checkout: true,
        }
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
        s.main_checkout = false;
        assert!(
            check(&s, &ok_facts("abc", Some("abc")))
                .unwrap_err()
                .contains("main checkout")
        );

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
        assert!(e.contains("git merge main && air record verify"), "{e}");
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
            bead: bead.into(),
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

    /// air-29a: the role is where the process is. Pointing `--repo` at the main checkout does
    /// not make a worker the coordinator, and an unknown location is not one either.
    #[test]
    fn a_worker_may_not_land_however_it_points_repo() {
        let at = |here: Option<&'static str>, pointed: &'static str| Caller {
            where_i_am: here,
            where_i_pointed: pointed,
        };
        assert!(may_land(&at(Some("main"), "main")).is_ok());
        // Standing in main, --repo naming a worktree: still the coordinator.
        assert!(may_land(&at(Some("main"), "alpha")).is_ok());
        let e = may_land(&at(Some("alpha"), "alpha")).unwrap_err();
        assert!(e.contains("air handover"), "{e}");
        // The incident's own invocation, and the refusal says which bypass it is.
        let e = may_land(&at(Some("alpha"), "main")).unwrap_err();
        assert!(e.contains("air-29a") && e.contains("cargo run"), "{e}");
        assert!(may_land(&at(None, "main")).is_err(), "fails closed");
    }
}
