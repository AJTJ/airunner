//! `air land <bead>…` / `air land --all`: the coordinator merges a green branch into main.
//!
//! Incident (2026-08-22, air-3pz): two green hand-overs sat because only the owner may commit
//! on main and `air land` did not exist. The coordinator could see the work was done and could
//! not land it; the owner learned by reading a pane.
//!
//! Ported from adopter's `scripts/land.sh` (read 2026-08-22). Taken: main checkout only and
//! on `main` (`land.sh:487-499`); refuse a dirty tracked tree, because the rollback is
//! `git reset --hard` and it would discard the work (`land.sh:504-514`); already-an-ancestor
//! is "nothing to land", not an error (`land.sh:533-535`); run the repo's verify on the
//! *merged* result and reset main to the pre-merge sha on red (`land.sh:519-526`). Not taken:
//! adopter's three attribution rules (branch / closing trailer / containment,
//! `land.sh:46-71`). Air knows who handed each bead over from the claim row, which is the
//! fact those rules reconstruct from git.
//!
//! One `bd` process for every bead closed at the end, not one per bead: a bd process costs
//! ~1.4 s whatever it is asked (air-869, `air_bd::stats`). That is `air close`'s job and this
//! calls into it.

use std::collections::BTreeMap;
use std::path::Path;

use air_ledger::landings::Landing as LandingRow;
use air_ledger::verify::{Kind, VerifyRun, new_id};

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

/// The repo's verify command, from `.claude/air.json` (`{"verify_command": "make verify"}`),
/// default `make verify` (CLAUDE.md Essentials). Never a shell string: argv, so nothing is
/// re-parsed.
pub fn verify_command(repo: &Path) -> Vec<String> {
    let configured = air_ledger::paths::air_dir_for(repo)
        .ok()
        .and_then(|d| d.parent().map(|m| m.join(".claude/air.json")))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| {
            v.get("verify_command")
                .and_then(|c| c.as_str())
                .map(str::to_string)
        });
    match configured.filter(|s| !s.trim().is_empty()) {
        Some(s) => s.split_whitespace().map(str::to_string).collect(),
        None => vec!["make".into(), "verify".into()],
    }
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

/// Why a landing cannot be attempted, with the command that fixes it. Pure over the facts, so
/// `air selftest` can fire every one without a repo (air-3pz).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts<'a> {
    pub worker: &'a str,
    pub on_main: bool,
    pub main_checkout: bool,
    /// Tracked files with uncommitted changes; the rollback would discard them. Untracked
    /// files survive `git reset --hard` and are not counted.
    pub dirty: &'a [String],
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
pub fn check(f: &Facts<'_>) -> Result<bool, String> {
    let w = f.worker;
    if !f.main_checkout {
        return Err(
            "refused: `air land` runs in the main checkout; this is a worktree.".to_string(),
        );
    }
    if !f.on_main {
        return Err("refused: main is not checked out here (fix: `git checkout main`)".to_string());
    }
    if !f.dirty.is_empty() {
        return Err(format!(
            "refused: main has uncommitted changes to {} tracked file(s) and a red verify \
             rewinds with `git reset --hard`, which would discard them (fix: commit or move \
             them aside): {}",
            f.dirty.len(),
            f.dirty.join(", ")
        ));
    }
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
             would land (fix: in {w}'s worktree, `git merge main && air record verify -- {}`)",
            branch_for(w),
            "make verify"
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

/// What one branch's landing did.
enum Outcome {
    Landed {
        merge: String,
        /// Beads whose acceptance the merge could not fully discharge, with the clauses. A
        /// record of what the print said; nothing here closes or blocks anything (air-ayp).
        noted: Vec<air_ledger::landings::OpenBead>,
    },
    Nothing,
    Rewound(String),
    Refused(String),
}

pub fn run(repo: &Path, beads: &[String], all: bool, json: bool) -> i32 {
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
    let inputs =
        serde_json::json!({"beads": beads, "all": all, "caller": here, "repo_worker": worker});
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
    if beads.is_empty() && !all {
        eprintln!("air land: name a bead, or `air land --all`");
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
    let ready = sel.landings;
    // ...and nothing landable is a REPORT, not silence: every branch says which precondition
    // it failed and the command that fixes it.
    if all && ready.is_empty() {
        let mut msg = String::from("nothing is landable right now.");
        if sel.skipped.is_empty() {
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
        let missing: Vec<&String> = beads
            .iter()
            .filter(|b| !ready.iter().any(|l| &&l.bead == b))
            .collect();
        if !missing.is_empty() {
            let msg = format!(
                "refused: no green branch names {} in its merge range. `air status` lists what \
                 is landable (a branch with a recorded green at its head; its beads are the \
                 ones its commits declare in a `Bead:` trailer).{}",
                missing
                    .iter()
                    .map(|b| b.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                // Say what each branch failed on, so the named-bead refusal diagnoses as well
                // as the --all one (air-6u5).
                sel.skipped
                    .iter()
                    .map(|sk| {
                        format!(
                            "\n  {} [{}]: {}\n    fix: {}",
                            sk.worker, sk.check, sk.detail, sk.fix
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("")
            );
            log_event(
                &ledger,
                &worker,
                "land",
                &inputs,
                "refuse",
                &msg,
                "bd + ledger",
            );
            emit(
                json,
                &serde_json::json!({"ok": false, "reason": msg}),
                || msg.clone(),
            );
            return 2;
        }
        ready
            .into_iter()
            .filter(|l| beads.contains(&l.bead))
            .collect()
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
                for o in noted.iter().filter(|o| o.refuted) {
                    lines.push(format!("  {} REFUTED: {}", o.bead, o.why));
                }
                landed.extend(batch.beads.clone());
                held_open.extend(noted);
            }
            Outcome::Nothing => lines.push(format!("{}: already in main", batch.worker)),
            Outcome::Rewound(why) => {
                lines.push(format!("{}: {why}", batch.worker));
                code = 1;
                break; // stop at the first red: main is back where it was, and the rest wait
            }
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

/// Merge one branch, verify the merged result, and rewind on red.
fn land_one(repo: &Path, ledger: &air_ledger::Ledger, batch: &Batch, json: bool) -> Outcome {
    let branch = branch_for(&batch.worker);
    let started_at = now();
    let tip = match git::head(repo) {
        Ok(h) => h,
        Err(e) => return Outcome::Refused(format!("cannot read main's HEAD: {e}")),
    };
    let branch_head = git::run(repo, &["rev-parse", &format!("{branch}^{{commit}}")]).ok();
    // Tracked only, and never the tracker's own export: `.beads/issues.jsonl` is rewritten by
    // every bd write, including one a peer runs, so a guard that trips on it trips always
    // (adopter land.sh:506-513). `.air/` is gitignored in a configured repo.
    let dirty: Vec<String> = git::dirty_tracked(repo)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| !p.starts_with(".air/") && !p.starts_with(".beads/"))
        .collect();
    let green = branch_head.as_deref().and_then(|h| {
        ledger
            .is_green_at(&batch.worker, h, Kind::Verify)
            .ok()
            .filter(|g| *g)
            .map(|_| h.to_string())
    });
    let facts = Facts {
        worker: &batch.worker,
        on_main: git::run(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).is_ok_and(|b| b == "main"),
        main_checkout: !repo.join(".git").is_file(),
        dirty: &dirty,
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
    match check(&facts) {
        Ok(false) => return Outcome::Nothing,
        Err(why) => {
            record("refused", None, None, Some("check".into()));
            return Outcome::Refused(why);
        }
        Ok(true) => {}
    }
    // The merge aborts on conflict and leaves main untouched: conflict resolution belongs to
    // whoever has the context, which is the worker (land.sh:519-526).
    let message = format!("Land {branch}: {}", batch.beads.join(" "));
    if let Err(e) = git::run(repo, &["merge", "--no-ff", "-m", &message, &branch]) {
        let _ = git::run(repo, &["merge", "--abort"]);
        record("refused", None, None, Some("merge".into()));
        return Outcome::Refused(format!(
            "merge conflicted and was aborted; main is untouched. Ask {} to `git merge main` and \
             resolve it: {e}",
            batch.worker
        ));
    }
    let merge = git::head(repo).unwrap_or_default();
    // air-bxe: main has moved and the rollback is armed from here until the verify reports.
    // Say so BEFORE the verify, not after it. That window is minutes long, and adopter's
    // coordinator called a land done inside it three times; `git merge-base --is-ancestor`
    // answers "is it in main", which is true for the whole window and so answers nothing.
    // A land killed here (theirs died on a closed pipe, `air land | head`) leaves this row
    // rather than leaving main green at a sha no landing mentions.
    record("in-flight", Some(merge.clone()), None, None);
    // Verify the MERGED result. A green on the branch alone is not a green of what landed.
    let cmd = verify_command(repo);
    let Some((prog, args)) = cmd.split_first() else {
        return Outcome::Refused("no verify command configured".into());
    };
    let verify_started = now();
    let t0 = std::time::Instant::now();
    if !json {
        println!("verifying the merged result: {}", cmd.join(" "));
    }
    let (exit_code, output_bytes) = super::record::run_tee(prog, args, repo).unwrap_or((-1, 0));
    let run = VerifyRun {
        id: new_id(),
        worker: "main".into(),
        sha: merge.clone(),
        kind: Kind::Verify,
        exit_code,
        trigger: "land".into(),
        failing_step: None,
        started_at: verify_started,
        finished_at: now(),
        log_path: None,
        command: Some(cmd.join(" ")),
        duration_ms: i64::try_from(t0.elapsed().as_millis()).ok(),
        output_bytes: Some(output_bytes),
        dirty: false,
    };
    let _ = ledger.record_verify(&run);
    if exit_code != 0 {
        // Rewind: main goes back exactly where it was, and the branch is untouched.
        let _ = git::run(repo, &["reset", "--hard", &tip]);
        // air-ob0: who still holds the un-landed commits. Asked AFTER the reset, so the main
        // checkout — which contained the merge a moment ago and is the whole point of the
        // reset — is not named as a carrier. The commit is unreferenced now but still
        // reachable from any worktree HEAD that merged it, which is exactly the set wanted.
        let carried_by = super::status::carrying(repo, &merge);
        record(
            "rewound",
            Some(merge.clone()),
            Some(run.id),
            Some(cmd.join(" ")),
        );
        let mut why = format!(
            "verify exited {exit_code} on the merged result; main is back at {} and `{}` is \
             untouched. The branch is green alone and red merged: ask {} to `git merge main` and \
             re-verify.",
            tip.get(..8).unwrap_or(&tip),
            branch,
            batch.worker
        );
        // A rollback un-lands from main and cannot un-merge from anyone who already took it.
        // `air status` holds the same set afterwards, so this message is not the only copy.
        for line in super::status::rewind_propagation(&merge, &carried_by) {
            why.push_str(&format!("\n  {line}"));
        }
        return Outcome::Rewound(why);
    }
    // ── air-ayp ────────────────────────────────────────────────────────────────────────
    // Layer 1, the part that is true under either closure model: read every bead's acceptance
    // and print it beside Air's verdict, so a wrong close is visible at the moment it lands.
    // Nothing closes on branch containment.
    let changed = git::run(repo, &["diff", "--name-only", &format!("{tip}..{merge}")])
        .map(|s| s.lines().map(str::to_string).collect::<Vec<_>>())
        .unwrap_or_default();
    let ev = acceptance::Evidence {
        green_at_landed: true, // the run above, recorded at `merge`
        changed: &changed,
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
        })
        .collect();
    record_full(
        if noted.iter().any(|o| o.refuted) {
            "landed-refuted"
        } else {
            "landed"
        },
        Some(merge.clone()),
        Some(run.id),
        None,
        &noted,
    );
    Outcome::Landed { merge, noted }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn ok_facts<'a>(dirty: &'a [String], head: &'a str, green: Option<&'a str>) -> Facts<'a> {
        Facts {
            worker: "alpha",
            on_main: true,
            main_checkout: true,
            dirty,
            branch_exists: true,
            already_in_main: false,
            contains_main: true,
            branch_head: head,
            green_at: green,
        }
    }

    #[test]
    fn every_refusal_names_the_command_that_fixes_it() {
        let none: Vec<String> = vec![];
        assert_eq!(check(&ok_facts(&none, "abc", Some("abc"))), Ok(true));

        let mut f = ok_facts(&none, "abc", Some("abc"));
        f.main_checkout = false;
        assert!(check(&f).unwrap_err().contains("main checkout"));

        let mut f = ok_facts(&none, "abc", Some("abc"));
        f.on_main = false;
        assert!(check(&f).unwrap_err().contains("git checkout main"));

        let dirty = vec!["src/a.rs".to_string()];
        let f = ok_facts(&dirty, "abc", Some("abc"));
        let e = check(&f).unwrap_err();
        assert!(
            e.contains("git reset --hard") && e.contains("src/a.rs"),
            "{e}"
        );

        let mut f = ok_facts(&none, "abc", Some("abc"));
        f.branch_exists = false;
        assert!(check(&f).unwrap_err().contains("worktree-alpha"));

        // Already in main is not an error: there is simply nothing to do.
        let mut f = ok_facts(&none, "abc", Some("abc"));
        f.already_in_main = true;
        assert_eq!(check(&f), Ok(false));

        let mut f = ok_facts(&none, "abc", Some("abc"));
        f.contains_main = false;
        let e = check(&f).unwrap_err();
        assert!(e.contains("git merge main && air record verify"), "{e}");

        // Green recorded, but at an older commit than the branch head.
        let e = check(&ok_facts(&none, "abcdef99", Some("999999aa"))).unwrap_err();
        assert!(e.contains("999999aa") && e.contains("abcdef99"), "{e}");
        let e = check(&ok_facts(&none, "abcdef99", None)).unwrap_err();
        assert!(e.contains("no recorded green"), "{e}");
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

    #[test]
    fn verify_command_defaults_to_make_verify() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(verify_command(dir.path()), vec!["make", "verify"]);
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
